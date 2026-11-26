// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A execucao de cada subcomando: ler a entrada, chamar o nucleo, escrever a saida.

use std::io::Write;
use std::path::Path;

use rustnf::nfse::sefin::{ClienteNfse, Transporte};
use rustnf_app::atualizacao::{atualizar_pedido, Pedido, Relatorio};
use rustnf_app::cofre::{cofre_do_ambiente, Cofre};
use rustnf_app::config::Efetiva;
use rustnf_app::eventos;
use rustnf_app::notas::Notas;
use rustnf_app::pedido;
use serde_json::{json, Value};

use super::{
    configuracao, resolver, servir, AcaoCertificado, AcaoDps, AcaoEvento, AcaoNotas, AcaoSefin,
    ArgAtualizar, ArgCalcular, ArgCancelar, ArgCertificado, ArgConsultar, ArgDanfse, ArgEmitir,
    ArgEventoConsultar, ArgEventoRegistrar, ArgSefin, ArgTestar, Cli, Comando, SituacaoArg,
};
use crate::emissao::{self, Emissor};
use crate::entrada;
use crate::erro::Erro;
use crate::nucleo::{self, SituacaoNota};

pub use rustnf_app::cofre::VARIAVEL_COFRE;

pub(super) fn escrever_stdout(bytes: &[u8]) -> Result<(), Erro> {
    let mut saida = std::io::stdout().lock();
    saida
        .write_all(bytes)
        .and_then(|()| saida.flush())
        .map_err(|_| Erro::Interno)
}

pub(super) fn escrever_json(v: &Value) -> Result<(), Erro> {
    let texto = serde_json::to_string_pretty(v)?;
    escrever_stdout(format!("{texto}\n").as_bytes())
}

/// Executa o subcomando. O `Err` vira diagnostico no stderr e o codigo de saida em `main`.
///
/// # Errors
///
/// Qualquer [`Erro`]; o codigo de saida sai de [`Erro::codigo_saida`].
pub async fn executar(cli: Cli) -> Result<(), Erro> {
    let cofre = cofre_do_ambiente();
    executar_com(cli, cofre.as_ref()).await
}

/// Como [`executar`], com o cofre dado por quem chama (os testes passam um cofre em memoria).
///
/// # Errors
///
/// Os mesmos de [`executar`].
pub async fn executar_com(cli: Cli, cofre: &dyn Cofre) -> Result<(), Erro> {
    let Cli { config, comando } = cli;
    let config = config.as_deref();
    match comando {
        Comando::Dps { acao } => dps(acao),
        Comando::Calcular(a) => calcular(&a, config),
        Comando::Danfse(a) => danfse(&a, config),
        Comando::Emitir(a) => emitir(a, config, cofre).await,
        Comando::Consultar(a) => consultar(a, config, cofre).await,
        Comando::Cancelar(a) => cancelar(a, config, cofre).await,
        Comando::Evento { acao } => evento(acao, config, cofre).await,
        Comando::Nfe { acao } => super::nfe::executar(acao, config, cofre).await,
        Comando::Serve(a) => servir(a, config, cofre).await,
        Comando::Config { acao } => configuracao::config(acao, config),
        Comando::Senha { acao } => configuracao::senha(acao, config, cofre),
        Comando::Notas {
            acao: AcaoNotas::Atualizar(a),
        } => atualizar(a, config, cofre).await,
        Comando::Notas { acao } => configuracao::notas(acao, config),
        Comando::Certificado {
            acao: AcaoCertificado::Mostrar(a),
        } => certificado_mostrar(&a, config, cofre),
        Comando::Sefin {
            acao: AcaoSefin::Testar(a),
        } => sefin_testar(&a, config, cofre).await,
        Comando::Completar { shell } => completar(shell),
    }
}

/// `rustnf completar <shell>`: o script de completar comandos, gerado da propria definicao da linha de comando.
fn completar(shell: clap_complete::Shell) -> Result<(), Erro> {
    use clap::CommandFactory;
    let mut script = Vec::new();
    clap_complete::generate(shell, &mut Cli::command(), "rustnf", &mut script);
    escrever_stdout(&script)
}

/// `rustnf certificado mostrar`: abre o certificado e diz de quem e', ate quando vale e se assina hoje.
fn certificado_mostrar(
    a: &ArgCertificado,
    config: Option<&Path>,
    cofre: &dyn Cofre,
) -> Result<(), Erro> {
    let efetiva = configuracao::carregar(config)?;
    let pfx = resolver::pfx(&efetiva, a.pfx.as_deref())?;
    let senha = resolver::senha(&efetiva, cofre, &a.senha, &pfx, false)?;
    let certificado = super::certificado::abrir(&pfx, &senha)?;
    let emitente = &efetiva.config.emitente;
    let documento = [emitente.cnpj.as_str(), emitente.cpf.as_str()]
        .into_iter()
        .find(|d| !d.is_empty());
    let r = certificado.resumo_de_agora(documento);
    if a.json {
        escrever_json(&serde_json::to_value(&r)?)?;
    } else {
        for aviso in &r.avisos {
            eprintln!("aviso: {aviso}");
        }
        let texto = format!(
            "{}\ntitular: {}\ndocumento: {}\nemissor: {}{}\nvalidade: de {} a {} ({} dias)\n",
            r.frase,
            r.titular,
            r.documento,
            r.emissor,
            if r.icp_brasil {
                " (ICP-Brasil)"
            } else {
                " (fora da ICP-Brasil)"
            },
            r.valido_de,
            r.valido_ate,
            r.dias_restantes,
        );
        escrever_stdout(texto.as_bytes())?;
    }
    if r.ok {
        Ok(())
    } else {
        Err(Erro::recusa("", r.frase))
    }
}

/// O teste da conexao de um cliente da Sefin ja montado: `{"ok": true, "ambiente", "frase"}`.
///
/// # Errors
///
/// [`Erro::Recusa`] se o certificado nao assina hoje (nada vai para a rede); [`Erro::Rede`] se a Sefin nao
/// respondeu ou recusou o certificado.
pub async fn testar_conexao<T: Transporte>(cliente: &ClienteNfse<T>) -> Result<Value, Erro> {
    let resumo = cliente.certificado.resumo_de_agora(None);
    if !resumo.ok {
        return Err(Erro::recusa("", resumo.frase));
    }
    let (ok, frase) = cliente.testar().await;
    if !ok {
        return Err(Erro::Rede(frase));
    }
    Ok(json!({
        "ok": true,
        "ambiente": if cliente.ambiente == 1 { "producao" } else { "restrita" },
        "frase": frase,
    }))
}

/// `rustnf sefin testar`: uma chamada real a Sefin Nacional, com o certificado e o ambiente da configuracao.
async fn sefin_testar(a: &ArgTestar, config: Option<&Path>, cofre: &dyn Cofre) -> Result<(), Erro> {
    let efetiva = configuracao::carregar(config)?;
    let pfx = resolver::pfx(&efetiva, a.sefin.pfx.as_deref())?;
    let ambiente = resolver::ambiente(&efetiva, a.sefin.ambiente)?;
    let senha = resolver::senha(&efetiva, cofre, &a.sefin.senha, &pfx, false)?;
    let certificado = super::certificado::abrir(&pfx, &senha)?;
    let cliente = rustnf_app::certificado::cliente(
        certificado,
        ambiente.codigo(),
        resolver::assinatura(&efetiva, a.sefin.assinatura),
        resolver::tempo_sefin(&efetiva),
    )?;
    let v = testar_conexao(&cliente).await?;
    if a.json {
        return escrever_json(&v);
    }
    let texto = format!(
        "{}\nambiente: {}\n",
        v["frase"].as_str().unwrap_or_default(),
        v["ambiente"].as_str().unwrap_or_default()
    );
    escrever_stdout(texto.as_bytes())
}

/// Avisa no stderr os campos do pedido que a DPS nao le.
fn avisar_desconhecidos(campos: &[String]) {
    for campo in campos {
        eprintln!("aviso: campo desconhecido no pedido, ignorado: {campo} (confira o nome)");
    }
}

/// Os campos desconhecidos do pedido ja lido como DPS. Com `estrito`, o pedido que tem algum e' recusado.
fn desconhecidos_do(texto: &str, estrito: bool) -> Result<Vec<String>, Erro> {
    // o texto ja foi lido como DPS: aqui ele so' e' lido de novo como JSON generico
    let pedido: Value = serde_json::from_str(texto)?;
    if estrito {
        nucleo::recusar_desconhecidos(&pedido)?;
    }
    Ok(nucleo::campos_desconhecidos(&pedido))
}

fn dps(acao: AcaoDps) -> Result<(), Erro> {
    match acao {
        AcaoDps::Esquema { emissao } => escrever_json(&if emissao {
            crate::mcp::dps_de_emissao()
        } else {
            crate::mcp::dps()
        }),
        AcaoDps::Montar { pedido, estrito } => {
            let texto = entrada::ler_texto(&pedido, "o pedido")?;
            let dps = nucleo::ler_dps(&texto)?;
            let desconhecidos = desconhecidos_do(&texto, estrito)?;
            let xml = nucleo::montar_dps(&dps)?;
            avisar_desconhecidos(&desconhecidos);
            escrever_stdout(format!("{xml}\n").as_bytes())
        }
        AcaoDps::Conferir {
            pedido,
            json,
            estrito,
        } => {
            let texto = entrada::ler_texto(&pedido, "o pedido")?;
            let dps = nucleo::ler_dps(&texto)?;
            let desconhecidos = desconhecidos_do(&texto, false)?;
            let r = if estrito {
                desconhecidos_do(&texto, true).map(|_| ())
            } else {
                Ok(())
            }
            .and_then(|()| nucleo::conferir_dps(&dps));
            if json {
                escrever_json(&match &r {
                    Ok(()) => {
                        let mut v = nucleo::ok_simples();
                        if !desconhecidos.is_empty() {
                            v["campos_desconhecidos"] = json!(desconhecidos);
                        }
                        v
                    }
                    Err(e) => {
                        let mut v = e.envelope()["erro"].clone();
                        v["ok"] = json!(false);
                        v
                    }
                })?;
            } else if r.is_ok() {
                escrever_stdout(b"ok\n")?;
            }
            if r.is_ok() {
                avisar_desconhecidos(&desconhecidos);
            }
            r
        }
    }
}

fn calcular(a: &ArgCalcular, config: Option<&Path>) -> Result<(), Erro> {
    let op = nucleo::ler_operacao(&entrada::ler_texto(&a.operacao, "a operacao")?)?;
    // sem `publicadas` no pedido, valem as `[[aliquotas]]` da configuracao
    let publicadas = configuracao::carregar(config)?.config.publicadas();
    let v = nucleo::calcular_com(&op, &publicadas)?;
    escrever_json(&v)?;
    if v["recusado"].as_bool() == Some(true) {
        return Err(Erro::recusa(
            "",
            "a previa registrou recusas de regra (veja bases.erros)",
        ));
    }
    Ok(())
}

/// Grava o PDF no arquivo de saida; `-` escreve no stdout.
fn gravar_pdf(saida: &str, pdf: &[u8]) -> Result<(), Erro> {
    if saida == "-" {
        return escrever_stdout(pdf);
    }
    std::fs::write(saida, pdf)
        .map_err(|_| Erro::Entrada("nao foi possivel gravar o PDF no arquivo de saida".into()))?;
    eprintln!("DANFSe gravado: {} bytes", pdf.len());
    Ok(())
}

fn danfse(a: &ArgDanfse, config: Option<&Path>) -> Result<(), Erro> {
    let situacao = a.situacao.map(|s| match s {
        SituacaoArg::Valida => SituacaoNota::Valida,
        SituacaoArg::Cancelada => SituacaoNota::Cancelada,
        SituacaoArg::Substituida => SituacaoNota::Substituida,
    });
    if let Some(chave) = &a.chave {
        return danfse_da_pasta(a, chave, situacao, config);
    }
    let (Some(nfse), Some(saida)) = (&a.nfse, &a.saida) else {
        return Err(Erro::Uso(
            "informe o XML da NFS-e e o PDF de saida (-o), ou a nota da pasta com --chave".into(),
        ));
    };
    let xml = entrada::ler_texto(nfse, "o XML da NFS-e")?;
    let canhoto = !a.sem_canhoto && configuracao::carregar(config)?.config.danfse.canhoto;
    let pdf = nucleo::gerar_danfse(&xml, situacao.unwrap_or_default(), canhoto)?;
    gravar_pdf(saida, &pdf)
}

/// O DANFSe de uma nota da pasta de notas, pela chave: sai do XML guardado, com a marca de cancelada quando a nota
/// esta cancelada na pasta. Sem `-o`, o PDF fica ao lado do XML e o caminho dele sai no stdout.
fn danfse_da_pasta(
    a: &ArgDanfse,
    chave: &str,
    situacao: Option<SituacaoNota>,
    config: Option<&Path>,
) -> Result<(), Erro> {
    let efetiva = configuracao::carregar(config)?;
    let notas = configuracao::notas_exigida(&efetiva)?;
    // a chave pode vir colada com espacos ou pontuacao
    let chave: String = chave
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_uppercase())
        .collect();
    let registro = notas.registro(&chave)?;
    let xml = notas.ler_xml(&chave)?;
    let situacao = situacao.unwrap_or(registro.situacao.para_danfse());
    let canhoto = !a.sem_canhoto && efetiva.config.danfse.canhoto;
    let pdf = nucleo::gerar_danfse(&xml, situacao, canhoto)?;
    match &a.saida {
        Some(saida) => gravar_pdf(saida, &pdf),
        None => {
            let caminho = notas.guardar_danfse(&chave, &pdf)?;
            eprintln!("DANFSe gravado: {} bytes", pdf.len());
            escrever_stdout(format!("{}\n", caminho.to_string_lossy()).as_bytes())
        }
    }
}

fn imprimir(v: &Value, json: bool) -> Result<(), Erro> {
    if let Some(aviso) = v.get("aviso").and_then(Value::as_str) {
        eprintln!("aviso: {aviso}");
    }
    if let Some(campos) = v.get("campos_desconhecidos").and_then(Value::as_array) {
        let campos: Vec<String> = campos
            .iter()
            .filter_map(|c| c.as_str().map(String::from))
            .collect();
        avisar_desconhecidos(&campos);
    }
    if json {
        escrever_json(v)
    } else {
        escrever_stdout(format!("{}\n", nucleo::resumo_texto(v)).as_bytes())
    }
}

fn emissor_de(
    sefin: &ArgSefin,
    efetiva: &Efetiva,
    cofre: &dyn Cofre,
    stdin_ocupado: bool,
) -> Result<std::sync::Arc<dyn Emissor>, Erro> {
    resolver::emissor(efetiva, cofre, sefin, stdin_ocupado)
}

/// A emissao a partir do JSON do pedido: completa o pedido com o `[emitente]`, numera e guarda a nota pela pasta
/// de notas da configuracao e emite. `folga` e' a opcao da chamada; sem ela vale a da configuracao.
///
/// # Errors
///
/// Como [`rustnf_app::pedido::emitir_pedido`].
pub async fn emitir_valor(
    emissor: &dyn Emissor,
    pedido: Value,
    efetiva: &Efetiva,
    folga: Option<u32>,
) -> Result<Value, Erro> {
    let notas = configuracao::notas_da(efetiva)?;
    pedido::emitir_pedido(
        emissor,
        pedido,
        &efetiva.config.emitente,
        folga.unwrap_or(efetiva.config.sefin.folga_relogio_s),
        notas.as_ref(),
    )
    .await
}

async fn emitir(a: ArgEmitir, config: Option<&Path>, cofre: &dyn Cofre) -> Result<(), Erro> {
    let texto = entrada::ler_texto(&a.pedido, "o pedido")?;
    // erro de sintaxe do JSON antes de qualquer outra coisa, como sempre foi
    let pedido: Value = serde_json::from_str(&texto)?;
    // campo de nome errado nao entra no XML: a nota sairia sem ele. Por padrao, recusa antes de abrir o certificado
    if !a.ignorar_desconhecidos {
        nucleo::recusar_desconhecidos(&pedido).map_err(|e| match e {
            Erro::Recusa { campo, mensagem } => Erro::recusa(
                campo,
                format!("{mensagem}; para emitir sem ele, use --ignorar-desconhecidos"),
            ),
            outro => outro,
        })?;
    }
    let efetiva = configuracao::carregar(config)?;
    let emissor = emissor_de(&a.sefin, &efetiva, cofre, a.pedido == "-")?;
    let v = emitir_valor(emissor.as_ref(), pedido, &efetiva, a.folga_relogio).await?;
    imprimir(&v, a.json)
}

/// O que `rustnf consultar` procura.
enum Alvo {
    /// Uma NFS-e pela chave de acesso.
    Chave(String),
    /// A DPS pelo Id (45 posicoes).
    Dps(String),
}

/// A chave, o Id da DPS ou a serie e o numero que a chamada deu; serie e numero viram o Id pelo `[emitente]`.
fn alvo_da_consulta(a: &ArgConsultar, efetiva: &Efetiva) -> Result<Alvo, Erro> {
    match (&a.chave, &a.dps, &a.numero) {
        (Some(chave), _, _) => Ok(Alvo::Chave(chave.clone())),
        (None, Some(id), _) => Ok(Alvo::Dps(id.clone())),
        (None, None, Some(numero)) => Ok(Alvo::Dps(eventos::id_da_dps(
            &efetiva.config.emitente,
            a.serie.as_deref(),
            numero,
        )?)),
        (None, None, None) => Err(Erro::Uso(
            "informe --chave, --dps ou --numero (com --serie, se nao for a do [emitente])".into(),
        )),
    }
}

/// `rustnf consultar` com o emissor ja montado.
///
/// # Errors
///
/// Como [`emissao::consultar`] e [`emissao::consultar_dps`].
pub async fn consultar_com(
    emissor: &dyn Emissor,
    a: &ArgConsultar,
    efetiva: &Efetiva,
) -> Result<(), Erro> {
    let v = match alvo_da_consulta(a, efetiva)? {
        Alvo::Chave(chave) => emissao::consultar(emissor, &chave).await?,
        Alvo::Dps(id) => emissao::consultar_dps(emissor, &id, a.com_nota).await?,
    };
    imprimir(&v, a.json)
}

async fn consultar(a: ArgConsultar, config: Option<&Path>, cofre: &dyn Cofre) -> Result<(), Erro> {
    let efetiva = configuracao::carregar(config)?;
    // o Id sai da configuracao: se nao sair, para antes de abrir o certificado
    alvo_da_consulta(&a, &efetiva)?;
    let emissor = emissor_de(&a.sefin, &efetiva, cofre, false)?;
    consultar_com(emissor.as_ref(), &a, &efetiva).await
}

async fn cancelar(a: ArgCancelar, config: Option<&Path>, cofre: &dyn Cofre) -> Result<(), Erro> {
    let efetiva = configuracao::carregar(config)?;
    let emissor = emissor_de(&a.sefin, &efetiva, cofre, false)?;
    let notas = configuracao::notas_da(&efetiva)?;
    let v = pedido::cancelar_pedido(
        emissor.as_ref(),
        &a.chave,
        &a.motivo,
        &a.justificativa,
        notas.as_ref(),
    )
    .await?;
    imprimir(&v, a.json)
}

async fn evento(acao: AcaoEvento, config: Option<&Path>, cofre: &dyn Cofre) -> Result<(), Erro> {
    match acao {
        AcaoEvento::Tipos { json } => tipos_de_evento(json),
        AcaoEvento::Consultar(a) => {
            let efetiva = configuracao::carregar(config)?;
            let emissor = emissor_de(&a.sefin, &efetiva, cofre, false)?;
            evento_consultar_com(emissor.as_ref(), &a).await
        }
        AcaoEvento::Registrar(a) => {
            // o tipo que o contribuinte nao registra por aqui para antes de abrir o certificado
            emissao::conferir_tipo_de_registro(&a.tipo)?;
            let efetiva = configuracao::carregar(config)?;
            let emissor = emissor_de(&a.sefin, &efetiva, cofre, false)?;
            let notas = configuracao::notas_da(&efetiva)?;
            evento_registrar_com(emissor.as_ref(), &a, notas.as_ref()).await
        }
    }
}

/// `rustnf evento consultar` com o emissor ja montado.
///
/// # Errors
///
/// Como [`emissao::consultar_evento`].
pub async fn evento_consultar_com(
    emissor: &dyn Emissor,
    a: &ArgEventoConsultar,
) -> Result<(), Erro> {
    let v = emissao::consultar_evento(emissor, &a.chave, &a.tipo, a.sequencia).await?;
    imprimir(&v, a.json)
}

/// `rustnf evento registrar` com o emissor e a pasta de notas ja prontos.
///
/// # Errors
///
/// Como [`eventos::registrar_evento_pedido`].
pub async fn evento_registrar_com(
    emissor: &dyn Emissor,
    a: &ArgEventoRegistrar,
    notas: Option<&Notas>,
) -> Result<(), Erro> {
    let v = eventos::registrar_evento_pedido(
        emissor,
        &a.tipo,
        &a.chave,
        a.motivo.as_deref().unwrap_or(""),
        a.justificativa.as_deref().unwrap_or(""),
        notas,
    )
    .await?;
    imprimir(&v, a.json)
}

/// `rustnf evento tipos`: o que o contribuinte registra, sem usar a rede.
fn tipos_de_evento(json: bool) -> Result<(), Erro> {
    let tipos = emissao::tipos_de_evento();
    if json {
        return escrever_json(&tipos);
    }
    let mut texto = String::new();
    for t in tipos.as_array().into_iter().flatten() {
        let campo = |nome: &str| t.get(nome).and_then(Value::as_str).unwrap_or("");
        texto.push_str(&format!(
            "{}\t{}\t{}\t({})\n",
            campo("tipo"),
            campo("autor"),
            campo("descricao"),
            campo("comando"),
        ));
    }
    escrever_stdout(texto.as_bytes())
}

/// O relatorio da atualizacao: o que mudou e o que falhou, e a contagem.
fn imprimir_relatorio(r: &Relatorio, json: bool) -> Result<(), Erro> {
    if json {
        return escrever_json(&r.para_json());
    }
    let mut texto = format!(
        "{} nota(s) conferida(s): {} mudou(aram), {} igual(is), {} falha(s)\n",
        r.conferidas,
        r.mudaram.len(),
        r.iguais,
        r.falhas.len()
    );
    for m in &r.mudaram {
        texto.push_str(&format!(
            "mudou\t{}\t{} -> {}\n",
            m.chave,
            m.de.texto(),
            m.para.texto()
        ));
    }
    for f in &r.falhas {
        texto.push_str(&format!("falha\t{}\t{}\n", f.chave, f.motivo));
    }
    escrever_stdout(texto.as_bytes())
}

/// `rustnf notas atualizar` com o emissor e a pasta de notas ja prontos: imprime o relatorio e, havendo nota que nao
/// pode ser conferida, devolve o erro (saida 4).
///
/// # Errors
///
/// Os de [`atualizar_pedido`] e [`Erro::Rede`] se alguma nota foi para as falhas.
pub async fn atualizar_com(
    emissor: &dyn Emissor,
    a: &ArgAtualizar,
    notas: &Notas,
) -> Result<(), Erro> {
    let pedido = Pedido {
        de: a.de.clone(),
        ate: a.ate.clone(),
        situacao: a.situacao.map(Into::into),
        busca: a.busca.clone(),
        intervalo_ms: a.intervalo_ms,
    };
    let relatorio = atualizar_pedido(emissor, notas, &pedido).await?;
    imprimir_relatorio(&relatorio, a.json)?;
    if relatorio.falhas.is_empty() {
        Ok(())
    } else {
        Err(Erro::Rede(format!(
            "{} nota(s) nao puderam ser conferidas (veja as falhas acima)",
            relatorio.falhas.len()
        )))
    }
}

async fn atualizar(a: ArgAtualizar, config: Option<&Path>, cofre: &dyn Cofre) -> Result<(), Erro> {
    let efetiva = configuracao::carregar(config)?;
    let notas = configuracao::notas_exigida(&efetiva)?;
    let emissor = emissor_de(&a.sefin, &efetiva, cofre, false)?;
    atualizar_com(emissor.as_ref(), &a, &notas).await
}
