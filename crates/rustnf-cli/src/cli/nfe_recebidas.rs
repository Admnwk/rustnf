// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A execucao de `rustnf nfe recebidas ...`: ler a entrada, chamar `rustnf_app::nfe`, escrever a saida e devolver o codigo
//! de saida. Nenhuma regra fiscal mora aqui.

use std::path::Path;

use rustnf::nfe::sefaz::evento::TipoManifestacao;
use rustnf::nfe::sefaz::Transporte;
use rustnf_app::nfe::{self, Contexto, FiltroRecebidas};
use rustnf_app::notas::nfe::{Recebida, SituacaoRecebida};
use serde_json::Value;

use super::comandos::{escrever_json, escrever_stdout};
use super::nfe::{concluir, notas_exigida};
use super::{configuracao, AcaoRecebidas, SituacaoRecebidaArg, TipoManifestacaoArg};
use crate::erro::Erro;

impl From<SituacaoRecebidaArg> for SituacaoRecebida {
    fn from(s: SituacaoRecebidaArg) -> Self {
        match s {
            SituacaoRecebidaArg::Resumo => Self::Resumo,
            SituacaoRecebidaArg::Completa => Self::Completa,
            SituacaoRecebidaArg::Ciencia => Self::Ciencia,
            SituacaoRecebidaArg::Confirmada => Self::Confirmada,
            SituacaoRecebidaArg::Desconhecida => Self::Desconhecida,
            SituacaoRecebidaArg::NaoRealizada => Self::NaoRealizada,
            SituacaoRecebidaArg::Cancelada => Self::Cancelada,
            SituacaoRecebidaArg::Denegada => Self::Denegada,
        }
    }
}

impl From<TipoManifestacaoArg> for TipoManifestacao {
    fn from(t: TipoManifestacaoArg) -> Self {
        match t {
            TipoManifestacaoArg::Ciencia => Self::Ciencia,
            TipoManifestacaoArg::Confirmacao => Self::Confirmacao,
            TipoManifestacaoArg::Desconhecimento => Self::Desconhecimento,
            TipoManifestacaoArg::NaoRealizada => Self::NaoRealizada,
        }
    }
}

/// As acoes que so' leem a pasta: `listar` e `cursor`.
pub(super) fn localmente(acao: AcaoRecebidas, config: Option<&Path>) -> Result<(), Erro> {
    let e = configuracao::carregar(config)?;
    let notas = notas_exigida(&e)?;
    match acao {
        AcaoRecebidas::Listar(a) => {
            let lista = nfe::listar(
                &notas,
                &FiltroRecebidas {
                    situacao: a.situacao.map(SituacaoRecebida::from),
                    busca: a.busca.unwrap_or_default(),
                },
            )?;
            if a.json {
                return escrever_json(&serde_json::to_value(&lista)?);
            }
            if lista.is_empty() {
                eprintln!("nenhuma NF-e recebida na pasta");
            }
            for r in &lista {
                escrever_stdout(linha_da_recebida(r).as_bytes())?;
            }
            Ok(())
        }
        AcaoRecebidas::Cursor { json } => {
            let v = nfe::cursor(&notas, &e.config)?;
            if json {
                return escrever_json(&v);
            }
            escrever_stdout(texto_do_cursor(&v).as_bytes())
        }
        _ => Err(Erro::Interno),
    }
}

/// As acoes que usam o Ambiente Nacional, com o contexto ja montado (os testes passam um cliente de transporte falso).
pub(super) async fn com<T: Transporte>(
    ctx: &Contexto<'_, T>,
    acao: AcaoRecebidas,
) -> Result<(), Erro> {
    match acao {
        AcaoRecebidas::Sincronizar(a) => {
            let s = nfe::sincronizar(ctx, a.lotes).await?;
            concluir(&s.para_json(), a.json, s.erro())
        }
        AcaoRecebidas::Baixar(a) => {
            let b = nfe::baixar(ctx, &a.chave, a.ciencia).await?;
            concluir(&b.para_json(), a.json, b.erro())
        }
        AcaoRecebidas::Manifestar(a) => {
            let m =
                nfe::manifestar(ctx, &a.chave, a.tipo.into(), a.justificativa.as_deref()).await?;
            concluir(&m.para_json(), a.json, m.erro())
        }
        _ => Err(Erro::Interno),
    }
}

fn linha_da_recebida(r: &Recebida) -> String {
    let data = r.emissao.get(..10).unwrap_or(&r.emissao);
    format!(
        "{data}\t{}\t{}\t{} ({})\t{}\n",
        r.valor,
        r.situacao.texto(),
        r.emitente_nome,
        r.emitente,
        r.chave
    )
}

fn texto_do_cursor(v: &Value) -> String {
    let texto = |campo: &str| v[campo].as_str().unwrap_or("-").to_string();
    let mut t = format!(
        "interessado: {} ({})\nultimo NSU: {}\nmaior NSU conhecido: {}\npode consultar de novo depois de: {}\n",
        texto("documento"),
        texto("ambiente"),
        texto("ult_nsu"),
        texto("max_nsu"),
        texto("espera_ate"),
    );
    let faixas = v["lacunas"].as_array().map_or(0, Vec::len);
    let ilegiveis = v["ilegiveis"].as_array().map_or(0, Vec::len);
    t.push_str(&format!(
        "faixas de NSU a buscar: {faixas}\ndocumentos que nao abriram (serao relidos): {ilegiveis}\n"
    ));
    t
}
