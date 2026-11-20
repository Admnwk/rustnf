// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A execucao de cada subcomando: ler a entrada, chamar o nucleo, escrever a saida.

use std::io::Write;

use serde_json::{json, Value};

use super::{
    certificado, servir, AcaoDps, ArgCalcular, ArgCancelar, ArgConsultar, ArgDanfse, ArgEmitir,
    ArgSefin, Cli, Comando, SituacaoArg,
};
use crate::emissao;
use crate::entrada;
use crate::erro::Erro;
use crate::nucleo::{self, SituacaoNota};

fn escrever_stdout(bytes: &[u8]) -> Result<(), Erro> {
    let mut saida = std::io::stdout().lock();
    saida
        .write_all(bytes)
        .and_then(|()| saida.flush())
        .map_err(|_| Erro::Interno)
}

fn escrever_json(v: &Value) -> Result<(), Erro> {
    let texto = serde_json::to_string_pretty(v)?;
    escrever_stdout(format!("{texto}\n").as_bytes())
}

/// Executa o subcomando. O `Err` vira diagnostico no stderr e o codigo de saida em `main`.
///
/// # Errors
///
/// Qualquer [`Erro`]; o codigo de saida sai de [`Erro::codigo_saida`].
pub async fn executar(cli: Cli) -> Result<(), Erro> {
    match cli.comando {
        Comando::Dps { acao } => dps(acao),
        Comando::Calcular(a) => calcular(&a),
        Comando::Danfse(a) => danfse(&a),
        Comando::Emitir(a) => emitir(a).await,
        Comando::Consultar(a) => consultar(a).await,
        Comando::Cancelar(a) => cancelar(a).await,
        Comando::Serve(a) => servir(a).await,
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

fn calcular(a: &ArgCalcular) -> Result<(), Erro> {
    let op = nucleo::ler_operacao(&entrada::ler_texto(&a.operacao, "a operacao")?)?;
    let v = nucleo::calcular(&op)?;
    escrever_json(&v)?;
    if v["recusado"].as_bool() == Some(true) {
        return Err(Erro::recusa(
            "",
            "a previa registrou recusas de regra (veja bases.erros)",
        ));
    }
    Ok(())
}

fn danfse(a: &ArgDanfse) -> Result<(), Erro> {
    let xml = entrada::ler_texto(&a.nfse, "o XML da NFS-e")?;
    let situacao = match a.situacao {
        SituacaoArg::Valida => SituacaoNota::Valida,
        SituacaoArg::Cancelada => SituacaoNota::Cancelada,
        SituacaoArg::Substituida => SituacaoNota::Substituida,
    };
    let pdf = nucleo::gerar_danfse(&xml, situacao, !a.sem_canhoto)?;
    if a.saida == "-" {
        return escrever_stdout(&pdf);
    }
    std::fs::write(&a.saida, &pdf)
        .map_err(|_| Erro::Entrada("nao foi possivel gravar o PDF no arquivo de saida".into()))?;
    eprintln!("DANFSe gravado: {} bytes", pdf.len());
    Ok(())
}

fn imprimir(v: &Value, json: bool) -> Result<(), Erro> {
    if json {
        escrever_json(v)
    } else {
        escrever_stdout(format!("{}\n", nucleo::resumo_texto(v)).as_bytes())
    }
}

fn emissor_de(
    sefin: &ArgSefin,
    stdin_ocupado: bool,
) -> Result<std::sync::Arc<dyn emissao::Emissor>, Erro> {
    let senha = certificado::senha(&sefin.senha, stdin_ocupado)?;
    certificado::emissor(&sefin.pfx, &senha, sefin.ambiente)
}

async fn emitir(a: ArgEmitir) -> Result<(), Erro> {
    let dps = nucleo::ler_dps(&entrada::ler_texto(&a.pedido, "o pedido")?)?;
    let emissor = emissor_de(&a.sefin, a.pedido == "-")?;
    let v = emissao::emitir(emissor.as_ref(), dps, a.folga_relogio).await?;
    imprimir(&v, a.json)
}

async fn consultar(a: ArgConsultar) -> Result<(), Erro> {
    let emissor = emissor_de(&a.sefin, false)?;
    let v = emissao::consultar(emissor.as_ref(), &a.chave).await?;
    imprimir(&v, a.json)
}

async fn cancelar(a: ArgCancelar) -> Result<(), Erro> {
    let emissor = emissor_de(&a.sefin, false)?;
    let v = emissao::cancelar(emissor.as_ref(), &a.chave, &a.motivo, &a.justificativa).await?;
    imprimir(&v, a.json)
}
