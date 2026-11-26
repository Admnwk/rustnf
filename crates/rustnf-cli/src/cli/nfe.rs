// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A execucao de `rustnf nfe ...`: ler a entrada, chamar `rustnf_app::nfe`, escrever a saida e devolver o codigo de saida.
//! Nenhuma regra fiscal mora aqui.

use std::path::Path;

use rustnf::nfe::sefaz::Transporte;
use rustnf_app::cofre::Cofre;
use rustnf_app::config::Efetiva;
use rustnf_app::nfe::{self, Contexto};
use rustnf_app::notas::nfe::{
    csv, Filtro, NotasNfe, Registro, Situacao, INDICE_ANTERIOR, LISTA_CSV,
};
use serde_json::{json, Value};

use super::comandos::{escrever_json, escrever_stdout};
use super::{
    configuracao, resolver, AcaoNfe, AcaoNotasNfe, ArgConferirNfe, ArgDanfeNfe, ArgPedidoNfe,
    SituacaoNfe,
};
use crate::entrada;
use crate::erro::Erro;

impl From<SituacaoNfe> for Situacao {
    fn from(s: SituacaoNfe) -> Self {
        match s {
            SituacaoNfe::Autorizada => Self::Autorizada,
            SituacaoNfe::Denegada => Self::Denegada,
            SituacaoNfe::Cancelada => Self::Cancelada,
            SituacaoNfe::Pendente => Self::Pendente,
            SituacaoNfe::Inutilizada => Self::Inutilizada,
            SituacaoNfe::Epec => Self::Epec,
        }
    }
}

/// A pasta de notas da NF-e, se a configuracao tiver `[notas].pasta`.
pub(super) fn notas_da(e: &Efetiva) -> Result<Option<NotasNfe>, Erro> {
    if e.config.notas.pasta.is_empty() {
        return Ok(None);
    }
    Ok(Some(NotasNfe::abrir(&e.config.notas.pasta)?))
}

fn notas_exigida(e: &Efetiva) -> Result<NotasNfe, Erro> {
    notas_da(e)?.ok_or_else(|| {
        Erro::Uso(
            "sem pasta de notas: configure [notas].pasta no arquivo ou RUSTNF_NOTAS_PASTA".into(),
        )
    })
}

/// Executa `rustnf nfe <acao>`. As que usam a SEFAZ abrem o certificado (mesmo `[sefin].pfx` e mesma senha da NFS-e) e
/// montam o cliente real; as outras so' leem a configuracao e a pasta de notas.
///
/// # Errors
///
/// Qualquer [`Erro`]; o codigo de saida sai de [`Erro::codigo_saida`].
pub(super) async fn executar(
    acao: AcaoNfe,
    config: Option<&Path>,
    cofre: &dyn Cofre,
) -> Result<(), Erro> {
    if acao.cert().is_none() {
        return localmente(acao, config);
    }
    let efetiva = configuracao::carregar(config)?;
    let (efetiva, certificado) = {
        let cert = acao.cert().ok_or(Erro::Interno)?;
        let mut efetiva = efetiva;
        if let Some(a) = cert.ambiente {
            efetiva.config.nfe.ambiente = a.texto().into();
        }
        // a UF e o resto da configuracao se conferem antes de abrir o certificado
        nfe::config_sefaz(&efetiva.config)?;
        let pfx = resolver::pfx(&efetiva, cert.pfx.as_deref())?;
        let senha = resolver::senha(&efetiva, cofre, &cert.senha, &pfx, acao.pedido_do_stdin())?;
        let certificado = super::certificado::abrir(&pfx, &senha)?;
        (efetiva, certificado)
    };
    let cliente = nfe::cliente_real(&certificado, &efetiva.config)?;
    let notas = notas_da(&efetiva)?;
    let ctx = Contexto::novo(&cliente, &certificado, &efetiva.config, notas.as_ref());
    nfe_com(&ctx, acao).await
}

fn localmente(acao: AcaoNfe, config: Option<&Path>) -> Result<(), Erro> {
    match acao {
        AcaoNfe::Montar(a) => montar(&a, config),
        AcaoNfe::Conferir(a) => conferir(&a, config),
        AcaoNfe::Danfe(a) => danfe(&a, config),
        AcaoNfe::Notas { acao } => notas(acao, config),
        AcaoNfe::Pendentes(a) => listar_pendentes(a.json, config),
        _ => Err(Erro::Interno),
    }
}

/// As acoes que usam a SEFAZ, com o contexto ja montado (os testes passam um cliente de transporte falso).
///
/// # Errors
///
/// Os de `rustnf_app::nfe`; o resultado da SEFAZ que nao e' sucesso vira o erro com o codigo de saida certo, depois de
/// impresso.
pub async fn nfe_com<T: Transporte>(ctx: &Contexto<'_, T>, acao: AcaoNfe) -> Result<(), Erro> {
    match acao {
        AcaoNfe::Emitir(a) => {
            let e = nfe::emitir(ctx, ler_pedido(&a.pedido)?, !a.ignorar_desconhecidos).await?;
            concluir(&e.para_json(), a.json, e.erro())
        }
        AcaoNfe::Epec(a) => {
            let r = nfe::epec(ctx, ler_pedido(&a.pedido)?, !a.ignorar_desconhecidos).await?;
            concluir(&r.para_json(), a.json, r.erro())
        }
        AcaoNfe::Consultar(a) => {
            let c = nfe::consultar(ctx, &a.chave).await?;
            concluir(&c.para_json(), a.json, c.erro())
        }
        AcaoNfe::Cancelar(a) => {
            let r = nfe::cancelar(ctx, &a.chave, &a.justificativa, a.protocolo.as_deref()).await?;
            concluir(&r.para_json(), a.json, r.erro())
        }
        AcaoNfe::Corrigir(a) => {
            let r = nfe::corrigir(ctx, &a.chave, &a.texto, a.sequencia).await?;
            concluir(&r.para_json(), a.json, r.erro())
        }
        AcaoNfe::Inutilizar(a) => {
            let r =
                nfe::inutilizar(ctx, a.serie.as_deref(), a.inicio, a.fim, &a.justificativa).await?;
            concluir(&r.para_json(), a.json, r.erro())
        }
        AcaoNfe::Status(a) => {
            let s = nfe::status(ctx.cliente, a.svc).await?;
            let mut v = nfe::estado_do_servico(ctx.cliente, &s);
            v["svc"] = json!(a.svc);
            concluir(
                &v,
                a.json,
                (!s.em_operacao()).then(|| Erro::Rede(s.frase.clone())),
            )
        }
        AcaoNfe::Pendentes(a) => retomar(ctx, a.json).await,
        _ => Err(Erro::Interno),
    }
}

fn ler_pedido(arquivo: &str) -> Result<Value, Erro> {
    Ok(serde_json::from_str(&entrada::ler_texto(
        arquivo, "o pedido",
    )?)?)
}

/// Avisa no stderr o que a montagem achou de estranho e os campos que o leiaute nao conhece.
fn avisar(v: &Value) {
    for a in v["avisos"].as_array().into_iter().flatten() {
        let item = a["item"]
            .as_u64()
            .map_or(String::new(), |i| format!(" (item {i})"));
        eprintln!(
            "aviso [{}]{item}: {}",
            a["codigo"].as_str().unwrap_or(""),
            a["mensagem"].as_str().unwrap_or("")
        );
    }
    for c in v["campos_desconhecidos"].as_array().into_iter().flatten() {
        eprintln!(
            "aviso: campo desconhecido no pedido, ignorado: {} (confira o nome)",
            c.as_str().unwrap_or("")
        );
    }
}

/// Imprime o resultado (texto ou JSON) e devolve o erro do resultado, se houver: o resultado completo sai no stdout mesmo
/// quando a saida e' 3 ou 4.
fn concluir(v: &Value, json: bool, erro: Option<Erro>) -> Result<(), Erro> {
    avisar(v);
    if json {
        escrever_json(v)?;
    } else {
        escrever_stdout(texto_do(v).as_bytes())?;
    }
    erro.map_or(Ok(()), Err)
}

/// O resumo em texto: a frase e os dados que a pessoa confere.
fn texto_do(v: &Value) -> String {
    let mut t = format!("{}\n", v["frase"].as_str().unwrap_or(""));
    for (rotulo, campo) in [
        ("chave", "chave"),
        ("serie", "serie"),
        ("numero", "numero"),
        ("tipo do evento", "tipo"),
        ("sequencia", "sequencia"),
        ("protocolo", "protocolo"),
        ("na pasta", "situacao"),
        ("uf", "uf"),
        ("ambiente", "ambiente"),
    ] {
        let valor = match &v[campo] {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            _ => String::new(),
        };
        if !valor.is_empty() {
            t.push_str(&format!("{rotulo}: {valor}\n"));
        }
    }
    t
}

fn montar(a: &ArgPedidoNfe, config: Option<&Path>) -> Result<(), Erro> {
    let efetiva = configuracao::carregar(config)?;
    let notas = notas_da(&efetiva)?;
    let m = nfe::montar_pedido(
        &efetiva.config,
        notas.as_ref(),
        ler_pedido(&a.pedido)?,
        !a.ignorar_desconhecidos,
    )?;
    avisar(&json!({
        "avisos": nfe::avisos_json(&m.avisos),
        "campos_desconhecidos": m.desconhecidos,
    }));
    escrever_stdout(format!("{}\n", m.xml).as_bytes())
}

fn conferir(a: &ArgConferirNfe, config: Option<&Path>) -> Result<(), Erro> {
    let efetiva = configuracao::carregar(config)?;
    let notas = notas_da(&efetiva)?;
    let pedido = ler_pedido(&a.pedido)?;
    let r = nfe::montar_pedido(
        &efetiva.config,
        notas.as_ref(),
        pedido,
        !a.ignorar_desconhecidos,
    );
    let v = nfe::conferencia_json(&r);
    if r.is_ok() {
        avisar(&v);
    }
    if a.json {
        escrever_json(&v)?;
    } else if r.is_ok() {
        escrever_stdout(b"ok\n")?;
    }
    r.map(|_| ())
}

async fn retomar<T: Transporte>(ctx: &Contexto<'_, T>, json: bool) -> Result<(), Erro> {
    let feitos = nfe::retomar(ctx).await?;
    if json {
        escrever_json(&Value::Array(
            feitos.iter().map(nfe::Retomada::para_json).collect(),
        ))?;
    } else if feitos.is_empty() {
        eprintln!("nenhuma nota pendente");
    }
    for f in &feitos {
        if let Some(aviso) = &f.aviso {
            eprintln!("aviso: {aviso}");
        }
        if !json {
            let situacao = f
                .situacao
                .map_or("saiu da pasta (rejeitada)", Situacao::texto);
            let linha = format!(
                "nfe {}\tserie {}\t{}\t{}\t{}\t{}\n",
                f.numero, f.serie, f.acao, f.status, situacao, f.chave
            );
            escrever_stdout(linha.as_bytes())?;
        }
    }
    nfe::erro_da_retomada(&feitos).map_or(Ok(()), Err)
}

fn listar_pendentes(json: bool, config: Option<&Path>) -> Result<(), Erro> {
    let efetiva = configuracao::carregar(config)?;
    let lista = notas_exigida(&efetiva)?.a_resolver()?;
    if json {
        return escrever_json(&serde_json::to_value(&lista)?);
    }
    if lista.is_empty() {
        eprintln!("nenhuma nota pendente");
    }
    for r in &lista {
        escrever_stdout(linha_da_nota(r, true).as_bytes())?;
    }
    if !lista.is_empty() {
        eprintln!("para consultar a SEFAZ e fechar essas notas: rustnf nfe pendentes --retomar");
    }
    Ok(())
}

fn linha_da_nota(r: &Registro, com_consultas: bool) -> String {
    let data = r.emissao.get(..10).unwrap_or(&r.emissao);
    let extra = if com_consultas {
        format!("\tconsultas {}", r.tentativas)
    } else {
        String::new()
    };
    format!(
        "{data}\tnfe {}\tserie {}\t{}\t{}\t{}{extra}\t{}\n",
        r.numero,
        r.serie,
        r.valor,
        r.situacao.texto(),
        r.destinatario_nome,
        r.chave
    )
}

fn gravar_pdf(saida: &str, pdf: &[u8]) -> Result<(), Erro> {
    if saida == "-" {
        return escrever_stdout(pdf);
    }
    std::fs::write(saida, pdf)
        .map_err(|_| Erro::Entrada("nao foi possivel gravar o PDF no arquivo de saida".into()))?;
    eprintln!("DANFE gravado: {} bytes", pdf.len());
    Ok(())
}

fn danfe(a: &ArgDanfeNfe, config: Option<&Path>) -> Result<(), Erro> {
    let efetiva = configuracao::carregar(config)?;
    let canhoto = !a.sem_canhoto && efetiva.config.nfe.canhoto;
    if let Some(chave) = &a.chave {
        let notas = notas_exigida(&efetiva)?;
        let mut cfg = efetiva.config.clone();
        cfg.nfe.canhoto = canhoto;
        let pdf = nfe::danfe_da_pasta(&notas, &cfg, chave)?;
        return match &a.saida {
            Some(saida) => gravar_pdf(saida, &pdf),
            None => {
                let limpa: String = chave.chars().filter(char::is_ascii_digit).collect();
                let caminho = notas.guardar_danfe(&limpa, &pdf)?;
                eprintln!("DANFE gravado: {} bytes", pdf.len());
                escrever_stdout(format!("{}\n", caminho.to_string_lossy()).as_bytes())
            }
        };
    }
    let (Some(arquivo), Some(saida)) = (&a.nfe, &a.saida) else {
        return Err(Erro::Uso(
            "informe o XML da NF-e e o PDF de saida (-o), ou a nota da pasta com --chave".into(),
        ));
    };
    let xml = entrada::ler_texto(arquivo, "o XML da NF-e")?;
    gravar_pdf(saida, &nfe::danfe_do_xml(&xml, canhoto, a.cancelada, None)?)
}

/// `[2, 3, 4, 7]` -> `2 a 4, 7`.
fn faixas_de(numeros: &[u64]) -> String {
    let mut partes: Vec<String> = Vec::new();
    let mut i = 0;
    while i < numeros.len() {
        let mut j = i;
        while j + 1 < numeros.len() && numeros[j + 1] == numeros[j] + 1 {
            j += 1;
        }
        partes.push(if j > i {
            format!("{} a {}", numeros[i], numeros[j])
        } else {
            numeros[i].to_string()
        });
        i = j + 1;
    }
    partes.join(", ")
}

fn notas(acao: AcaoNotasNfe, config: Option<&Path>) -> Result<(), Erro> {
    let e = configuracao::carregar(config)?;
    let notas = notas_exigida(&e)?;
    match acao {
        AcaoNotasNfe::Numero { serie, proximo } => {
            let serie = serie
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| e.config.nfe.serie.clone());
            if let Some(n) = proximo {
                notas.definir_proximo(&serie, n)?;
            }
            let n = notas.proximo_numero(&serie)?;
            escrever_stdout(format!("serie {serie}: proxima NF-e {n}\n").as_bytes())?;
            let pulados = notas.pulados(&serie)?;
            if !pulados.is_empty() {
                escrever_stdout(
                    format!(
                        "numeros pulados (se nao vao mais ser usados, inutilize com `rustnf nfe inutilizar`): {}\n",
                        faixas_de(&pulados)
                    )
                    .as_bytes(),
                )?;
            }
            Ok(())
        }
        AcaoNotasNfe::Exportar { mes, para } => {
            let lista = Filtro::do_mes(&mes)?.aplicar(notas.listar()?);
            if lista.is_empty() {
                eprintln!("nenhuma nota nesse mes: nada foi copiado");
                return Ok(());
            }
            let feito = notas.exportar(&lista, Path::new(&para))?;
            for chave in &feito.sem_xml {
                eprintln!("aviso: a nota {chave} esta na lista, mas nenhum XML dela esta na pasta de notas");
            }
            escrever_stdout(
                format!(
                    "{} nota(s) na lista ({LISTA_CSV}), {} XML copiado(s)\n",
                    lista.len(),
                    feito.copiadas
                )
                .as_bytes(),
            )
        }
        AcaoNotasNfe::Reindexar => {
            let feito = notas.reindexar()?;
            if feito.indice_ilegivel {
                eprintln!(
                    "aviso: o indice que havia estava ilegivel (ficou guardado como {INDICE_ANTERIOR}); o que so' ele sabia (quantas consultas a retomada fez) se perdeu"
                );
            }
            if feito.ignorados > 0 {
                eprintln!(
                    "aviso: {} arquivo(s) .xml ficaram de fora (nao sao uma NF-e nem um evento legivel, ou estao fora do lugar)",
                    feito.ignorados
                );
            }
            escrever_stdout(
                format!(
                    "indice refeito: {} nota(s), {} cancelada(s)\n",
                    feito.notas, feito.canceladas
                )
                .as_bytes(),
            )
        }
        AcaoNotasNfe::Listar {
            json,
            csv: em_csv,
            de,
            ate,
            situacao,
            busca,
        } => {
            if situacao == Some(SituacaoNfe::Inutilizada) {
                return listar_faixas(&notas, json, em_csv);
            }
            let filtro = Filtro::novo(
                de.as_deref(),
                ate.as_deref(),
                situacao.map(Situacao::from),
                busca.as_deref().unwrap_or(""),
            )?;
            let lista = filtro.aplicar(notas.listar()?);
            if json {
                return escrever_json(&serde_json::to_value(&lista)?);
            }
            if em_csv {
                return escrever_stdout(csv(&lista).as_bytes());
            }
            if lista.is_empty() {
                eprintln!("nenhuma nota na pasta");
            }
            for r in &lista {
                escrever_stdout(linha_da_nota(r, false).as_bytes())?;
            }
            Ok(())
        }
    }
}

/// `notas listar --situacao inutilizada`: as faixas, que nao sao notas.
fn listar_faixas(notas: &NotasNfe, json: bool, em_csv: bool) -> Result<(), Erro> {
    if em_csv {
        return Err(Erro::Uso(
            "a lista das faixas inutilizadas sai em texto ou em --json, nao em --csv".into(),
        ));
    }
    let faixas = notas.inutilizacoes()?;
    if json {
        return escrever_json(&serde_json::to_value(&faixas)?);
    }
    if faixas.is_empty() {
        eprintln!("nenhuma faixa inutilizada");
    }
    for f in &faixas {
        let linha = format!(
            "{}\tserie {}\tnumeros {} a {}\t{}\tprotocolo {}\n",
            f.ano,
            f.serie,
            f.inicio,
            f.fim,
            f.situacao.texto(),
            f.protocolo
        );
        escrever_stdout(linha.as_bytes())?;
    }
    Ok(())
}
