// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A execucao de cada subcomando: ler a entrada, chamar o nucleo, escrever a saida.

use std::io::Write;
use std::path::Path;

use rustnf_app::cofre::{cofre_do_ambiente, Cofre};
use rustnf_app::config::Efetiva;
use rustnf_app::pedido;
use serde_json::{json, Value};

use super::{
    configuracao, resolver, servir, AcaoDps, ArgCalcular, ArgCancelar, ArgConsultar, ArgDanfse,
    ArgEmitir, ArgSefin, Cli, Comando, SituacaoArg,
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
        Comando::Serve(a) => servir(a, config, cofre).await,
        Comando::Config { acao } => configuracao::config(acao, config),
        Comando::Senha { acao } => configuracao::senha(acao, config, cofre),
        Comando::Notas { acao } => configuracao::notas(acao, config),
    }
}

fn dps(acao: AcaoDps) -> Result<(), Erro> {
    match acao {
        AcaoDps::Montar { pedido } => {
            let dps = nucleo::ler_dps(&entrada::ler_texto(&pedido, "o pedido")?)?;
            let xml = nucleo::montar_dps(&dps)?;
            escrever_stdout(format!("{xml}\n").as_bytes())
        }
        AcaoDps::Conferir { pedido, json } => {
            let dps = nucleo::ler_dps(&entrada::ler_texto(&pedido, "o pedido")?)?;
            let r = nucleo::conferir_dps(&dps);
            if json {
                escrever_json(&match &r {
                    Ok(()) => nucleo::ok_simples(),
                    Err(e) => {
                        let mut v = e.envelope()["erro"].clone();
                        v["ok"] = json!(false);
                        v
                    }
                })?;
            } else if r.is_ok() {
                escrever_stdout(b"ok\n")?;
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

fn danfse(a: &ArgDanfse, config: Option<&Path>) -> Result<(), Erro> {
    let xml = entrada::ler_texto(&a.nfse, "o XML da NFS-e")?;
    let situacao = match a.situacao {
        SituacaoArg::Valida => SituacaoNota::Valida,
        SituacaoArg::Cancelada => SituacaoNota::Cancelada,
        SituacaoArg::Substituida => SituacaoNota::Substituida,
    };
    let canhoto = !a.sem_canhoto && configuracao::carregar(config)?.config.danfse.canhoto;
    let pdf = nucleo::gerar_danfse(&xml, situacao, canhoto)?;
    if a.saida == "-" {
        return escrever_stdout(&pdf);
    }
    std::fs::write(&a.saida, &pdf)
        .map_err(|_| Erro::Entrada("nao foi possivel gravar o PDF no arquivo de saida".into()))?;
    eprintln!("DANFSe gravado: {} bytes", pdf.len());
    Ok(())
}

fn imprimir(v: &Value, json: bool) -> Result<(), Erro> {
    if let Some(aviso) = v.get("aviso").and_then(Value::as_str) {
        eprintln!("aviso: {aviso}");
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
    let efetiva = configuracao::carregar(config)?;
    let emissor = emissor_de(&a.sefin, &efetiva, cofre, a.pedido == "-")?;
    let v = emitir_valor(emissor.as_ref(), pedido, &efetiva, a.folga_relogio).await?;
    imprimir(&v, a.json)
}

async fn consultar(a: ArgConsultar, config: Option<&Path>, cofre: &dyn Cofre) -> Result<(), Erro> {
    let efetiva = configuracao::carregar(config)?;
    let emissor = emissor_de(&a.sefin, &efetiva, cofre, false)?;
    let v = emissao::consultar(emissor.as_ref(), &a.chave).await?;
    imprimir(&v, a.json)
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
