// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A execucao de `rustnf nfce ...`: ler a entrada, chamar `rustnf_app::nfce` (e o caminho da NF-e que a NFC-e compartilha),
//! escrever a saida e devolver o codigo de saida. Nenhuma regra fiscal mora aqui.

use std::io::BufRead;
use std::path::Path;

use rustnf::nfe::sefaz::Transporte;
use rustnf_app::cofre::Cofre;
use rustnf_app::config::{Config, Efetiva};
use rustnf_app::nfce::{self, danfce, Emissao, Modo};
use rustnf_app::nfe::{self, Contexto};
use rustnf_app::notas::nfe::NotasNfe;
use rustnf_app::segredo::Segredo;
use serde_json::{json, Value};

use super::comandos::{escrever_json, escrever_stdout};
use super::nfe::concluir;
use super::{
    configuracao, resolver, AcaoCsc, AcaoNfce, ArgCscSo, ArgDanfce, ArgEmitirNfce, ArgPendentesNfce,
};
use crate::entrada;
use crate::erro::Erro;

/// A configuracao com o ambiente que a chamada pediu (`--ambiente`), sem mexer no que a NF-e usa.
fn config_da(efetiva: &Efetiva, ambiente: Option<super::AmbienteNfe>) -> Config {
    let mut c = efetiva.config.clone();
    if let Some(a) = ambiente {
        c.nfce.ambiente = a.texto().into();
    }
    c
}

/// A pasta da NFC-e (`nfce/` dentro da `[notas].pasta`), se a configuracao tiver `[notas].pasta`.
pub(super) fn pasta_se_ha(c: &Config) -> Result<Option<NotasNfe>, Erro> {
    pasta_da(c)
}

fn pasta_da(c: &Config) -> Result<Option<NotasNfe>, Erro> {
    if c.notas.pasta.is_empty() {
        return Ok(None);
    }
    Ok(Some(nfce::abrir_pasta(&c.notas.pasta)?))
}

fn pasta_exigida(c: &Config) -> Result<NotasNfe, Erro> {
    pasta_da(c)?.ok_or_else(|| {
        Erro::Uso(
            "sem pasta de notas: configure [notas].pasta no arquivo ou RUSTNF_NOTAS_PASTA".into(),
        )
    })
}

/// Executa `rustnf nfce <acao>`. As que usam a SEFAZ abrem o certificado (mesmo `[sefin].pfx` e mesma senha da NFS-e e da
/// NF-e) e montam o cliente real; as outras so' leem a configuracao, o cofre e a pasta de notas.
///
/// # Errors
///
/// Qualquer [`Erro`]; o codigo de saida sai de [`Erro::codigo_saida`].
pub(super) async fn executar(
    acao: AcaoNfce,
    config: Option<&Path>,
    cofre: &dyn Cofre,
) -> Result<(), Erro> {
    let Some(cert) = acao.cert() else {
        return localmente(acao, config, cofre);
    };
    let efetiva = configuracao::carregar(config)?;
    let cfg = config_da(&efetiva, cert.ambiente).como_nfce();
    // a UF e o resto da configuracao se conferem antes de abrir o certificado
    nfe::config_sefaz(&cfg)?;
    // o CSC (QR Code v2) tem de estar no cofre ANTES de numerar qualquer nota
    let qr = if matches!(acao, AcaoNfce::Emitir(_)) {
        nfce::opcoes_qr(cofre, &cfg)?
    } else {
        rustnf::nfe::montagem::OpcoesQr::default()
    };
    let pfx = resolver::pfx(&efetiva, cert.pfx.as_deref())?;
    let senha = resolver::senha(&efetiva, cofre, &cert.senha, &pfx, acao.pedido_do_stdin())?;
    let certificado = super::certificado::abrir(&pfx, &senha)?;
    let cliente = nfe::cliente_real(&certificado, &cfg)?;
    let notas = pasta_da(&cfg)?;
    let ctx = Contexto::novo(&cliente, &certificado, &cfg, notas.as_ref()).com_qr(qr);
    nfce_com(&ctx, acao).await
}

fn localmente(acao: AcaoNfce, config: Option<&Path>, cofre: &dyn Cofre) -> Result<(), Erro> {
    match acao {
        AcaoNfce::Csc { acao } => csc(acao, config, cofre),
        AcaoNfce::Pendentes(a) => pendentes_local(&a, config),
        AcaoNfce::Danfce(a) => danfce_arquivo(&a, config),
        _ => Err(Erro::Interno),
    }
}

/// As acoes que usam a SEFAZ, com o contexto ja montado (os testes passam um cliente de transporte falso). O contexto e'
/// o da NFC-e: configuracao de `Config::como_nfce`, pasta `nfce/` e QR Code.
///
/// # Errors
///
/// Os de `rustnf_app::nfce` e `rustnf_app::nfe`; o resultado da SEFAZ que nao e' sucesso vira o erro com o codigo de saida
/// certo, depois de impresso.
pub async fn nfce_com<T: Transporte>(ctx: &Contexto<'_, T>, acao: AcaoNfce) -> Result<(), Erro> {
    match acao {
        AcaoNfce::Emitir(a) => emitir(ctx, &a).await,
        AcaoNfce::Consultar(a) => {
            let c = nfe::consultar(ctx, &a.chave).await?;
            concluir(&c.para_json(), a.json, c.erro())
        }
        AcaoNfce::Cancelar(a) => {
            if let Some(notas) = ctx.notas {
                if let Some(aviso) = nfce::aviso_de_prazo(notas, &a.chave, ctx.agora) {
                    eprintln!("aviso: {aviso}");
                }
            }
            let r = nfe::cancelar(ctx, &a.chave, &a.justificativa, a.protocolo.as_deref()).await?;
            concluir(&r.para_json(), a.json, r.erro())
        }
        AcaoNfce::Status(a) => {
            let s = ctx.cliente.status_nfce().await?;
            let v = nfe::estado_do_servico(ctx.cliente, &s);
            concluir(
                &v,
                a.json,
                (!s.em_operacao()).then(|| Erro::Rede(s.frase.clone())),
            )
        }
        AcaoNfce::Pendentes(a) => retomar(ctx, &a).await,
        _ => Err(Erro::Interno),
    }
}

async fn emitir<T: Transporte>(ctx: &Contexto<'_, T>, a: &ArgEmitirNfce) -> Result<(), Erro> {
    let pedido: Value = serde_json::from_str(&entrada::ler_texto(&a.pedido, "o pedido")?)?;
    let modo = match (a.offline, a.offline_se_necessario) {
        (true, _) => Modo::Offline,
        (_, true) => Modo::OfflineSeNecessario,
        _ => Modo::Normal,
    };
    let e = nfce::emitir(ctx, pedido, !a.ignorar_desconhecidos, modo).await?;
    if let Emissao::Offline(g) = &e {
        eprintln!(
            "aviso: NFC-e em CONTINGENCIA OFF-LINE ({}). Entregue o DANFC-e ao consumidor e transmita ate {}.",
            g.motivo, g.prazo
        );
    }
    if a.danfce {
        gravar_cupom(ctx, &e)?;
    }
    concluir(&e.para_json(), a.json, e.erro())
}

/// Grava o DANFC-e em PDF ao lado da nota, se ela tem cupom (autorizada ou off-line). Nota rejeitada ou sem resposta nao tem.
fn gravar_cupom<T: Transporte>(ctx: &Contexto<'_, T>, e: &Emissao) -> Result<(), Erro> {
    let Some(notas) = ctx.notas else {
        return Ok(());
    };
    let tem_cupom = match e {
        Emissao::Offline(_) => true,
        Emissao::Enviada(x) => x.situacao == Some(rustnf_app::notas::nfe::Situacao::Autorizada),
    };
    if !tem_cupom {
        eprintln!("aviso: esta nota nao tem DANFC-e (so' a autorizada e a off-line tem)");
        return Ok(());
    }
    let (xml, cancelada) = danfce::xml_da_pasta(notas, e.chave())?;
    let op = danfce::opcoes(ctx.config, danfce::Pedido::default(), cancelada)?;
    let caminho = danfce::guardar(notas, e.chave(), &danfce::pdf(&xml, &op)?)?;
    eprintln!("DANFC-e gravado: {}", caminho.to_string_lossy());
    Ok(())
}

async fn retomar<T: Transporte>(ctx: &Contexto<'_, T>, a: &ArgPendentesNfce) -> Result<(), Erro> {
    let feitos = nfe::retomar(ctx).await?;
    if a.json {
        escrever_json(&Value::Array(
            feitos.iter().map(nfe::Retomada::para_json).collect(),
        ))?;
    }
    for f in &feitos {
        if let Some(aviso) = &f.aviso {
            eprintln!("aviso: {aviso}");
        }
        if !a.json {
            let situacao = f.situacao.map_or(
                "saiu da pasta (rejeitada)",
                rustnf_app::notas::nfe::Situacao::texto,
            );
            let linha = format!(
                "nfce {}\tserie {}\t{}\t{}\t{}\t{}\n",
                f.numero, f.serie, f.acao, f.status, situacao, f.chave
            );
            escrever_stdout(linha.as_bytes())?;
        }
    }
    if feitos.is_empty() && !a.json {
        eprintln!("nenhuma NFC-e pendente");
    }
    let erro_da_retomada = nfe::erro_da_retomada(&feitos);
    // o que sobrou, em palavras, e o fechamento do caixa
    if let Some(notas) = ctx.notas {
        let sobra = nfce::listar_pendentes(notas, ctx.agora)?;
        if !a.json {
            for p in &sobra {
                eprintln!("{}", p.frase());
            }
        }
        if a.fechar_caixa {
            if let Some(e) = nfce::erro_de_fechamento(&sobra) {
                return Err(e);
            }
        }
    }
    erro_da_retomada.map_or(Ok(()), Err)
}

fn pendentes_local(a: &ArgPendentesNfce, config: Option<&Path>) -> Result<(), Erro> {
    let efetiva = configuracao::carregar(config)?;
    let cfg = config_da(&efetiva, a.cert.ambiente);
    let lista = nfce::listar_pendentes_agora(&pasta_exigida(&cfg)?)?;
    if a.json {
        escrever_json(&Value::Array(
            lista.iter().map(nfce::Pendente::para_json).collect(),
        ))?;
    } else if lista.is_empty() {
        escrever_stdout(b"nenhuma NFC-e pendente\n")?;
    }
    if !a.json {
        for p in &lista {
            let marca = if p.destaque() { "!! " } else { "" };
            escrever_stdout(format!("{marca}{}\n", p.frase()).as_bytes())?;
        }
    }
    match (a.fechar_caixa, nfce::erro_de_fechamento(&lista)) {
        (true, Some(e)) => Err(e),
        _ => Ok(()),
    }
}

/// A chave da nota (44 digitos) se o alvo for uma; senao o alvo e' um arquivo ou `-`.
fn chave_do_alvo(alvo: &str) -> Option<String> {
    let digitos: String = alvo.chars().filter(char::is_ascii_digit).collect();
    (digitos.len() == 44 && !alvo.contains(['.', '/', '\\']) && alvo != "-").then_some(digitos)
}

fn gravar(saida: &str, bytes: &[u8], o_que: &str) -> Result<(), Erro> {
    if saida == "-" {
        return escrever_stdout(bytes);
    }
    std::fs::write(saida, bytes).map_err(|_| {
        Erro::Entrada(format!(
            "nao foi possivel gravar o arquivo de saida do {o_que}"
        ))
    })?;
    eprintln!("{o_que} gravado: {} bytes", bytes.len());
    Ok(())
}

fn danfce_arquivo(a: &ArgDanfce, config: Option<&Path>) -> Result<(), Erro> {
    let efetiva = configuracao::carregar(config)?;
    let pedido = danfce::Pedido {
        largura: a.largura.as_deref().and_then(|l| l.parse().ok()),
        colunas: a.colunas.as_deref().and_then(|c| c.parse().ok()),
        resumido: a.resumido,
        via_estabelecimento: a.via_estabelecimento,
    };
    let (xml, cancelada) = match chave_do_alvo(&a.alvo) {
        Some(chave) => danfce::xml_da_pasta(&pasta_exigida(&efetiva.config)?, &chave)?,
        None => (entrada::ler_texto(&a.alvo, "o XML da NFC-e")?, a.cancelada),
    };
    let op = danfce::opcoes(&efetiva.config, pedido, cancelada)?;
    match (&a.pdf, &a.escpos) {
        (Some(saida), _) => gravar(saida, &danfce::pdf(&xml, &op)?, "DANFC-e (PDF)"),
        (None, Some(saida)) => gravar(saida, &danfce::escpos(&xml, &op)?, "DANFC-e (ESC/POS)"),
        (None, None) => Err(Erro::Uso(
            "informe --pdf <arquivo> ou --escpos <arquivo>".into(),
        )),
    }
}

/// O CSC lido do ambiente ou de uma linha do stdin.
fn ler_csc(env: Option<&str>) -> Result<Segredo, Erro> {
    if let Some(v) = env {
        return entrada::senha_do_ambiente(v);
    }
    let mut linha = String::new();
    std::io::stdin()
        .lock()
        .read_line(&mut linha)
        .map_err(|_| Erro::Uso("nao foi possivel ler o CSC do stdin".into()))?;
    let csc = linha.trim_end_matches(['\r', '\n']).to_string();
    if csc.is_empty() {
        return Err(Erro::Uso(
            "o CSC do stdin veio vazio: passe-o por uma linha do stdin ou por --csc-env <VARIAVEL>"
                .into(),
        ));
    }
    Ok(Segredo::novo(csc))
}

fn config_do_csc(c: &Config, so: &ArgCscSo) -> Config {
    let mut cfg = c.clone();
    if let Some(a) = so.ambiente {
        cfg.nfce.ambiente = a.texto().into();
    }
    if let Some(id) = &so.id_csc {
        cfg.nfce.id_csc.clone_from(id);
    }
    cfg
}

fn csc(acao: AcaoCsc, config: Option<&Path>, cofre: &dyn Cofre) -> Result<(), Erro> {
    let e = configuracao::carregar(config)?;
    match acao {
        AcaoCsc::Guardar(a) => {
            let cfg = config_do_csc(&e.config, &a.so);
            nfce::csc::guardar(cofre, &cfg, &ler_csc(a.csc_env.as_deref())?)?;
            eprintln!(
                "CSC guardado no cofre do sistema (ambiente {}, idCSC {})",
                cfg.nfce.ambiente, cfg.nfce.id_csc
            );
            Ok(())
        }
        AcaoCsc::Testar(a) => {
            let cfg = config_do_csc(&e.config, &a);
            let existe = nfce::csc::existe(cofre, &cfg)?;
            let v = json!({"csc_guardado": existe, "ambiente": cfg.nfce.ambiente, "id_csc": cfg.nfce.id_csc});
            escrever_json(&v)?;
            if existe {
                Ok(())
            } else {
                Err(Erro::Recusa {
                    campo: "idCSC".into(),
                    mensagem: format!(
                        "nao ha CSC guardado para o ambiente {} e o idCSC {}: rode `rustnf nfce csc guardar`",
                        cfg.nfce.ambiente, cfg.nfce.id_csc
                    ),
                })
            }
        }
        AcaoCsc::Apagar(a) => {
            let cfg = config_do_csc(&e.config, &a);
            nfce::csc::apagar(cofre, &cfg)?;
            eprintln!("CSC apagado do cofre do sistema");
            Ok(())
        }
    }
}
