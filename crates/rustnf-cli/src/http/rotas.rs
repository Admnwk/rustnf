// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Um handler por rota. Ler o corpo, chamar o nucleo, escrever a resposta.

use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::header::{CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_TYPE};
use axum::http::{HeaderMap, HeaderValue, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use rustnf::nfse::Dps;
use serde::de::DeserializeOwned;
use serde_json::json;

use super::auth::bearer;
use super::{resposta_json, Emissao, Estado};
use crate::emissao;
use crate::erro::{Erro, ErroHttp};
use crate::nucleo::{self, SituacaoNota};

type Resposta = Result<Response, ErroHttp>;

/// Le o corpo ate o teto; passou dele, 413 sem ler o resto quando o `Content-Length` ja avisa.
async fn ler_corpo(cabecalhos: &HeaderMap, corpo: Body, limite: usize) -> Result<Vec<u8>, Erro> {
    let declarado = cabecalhos
        .get(CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<usize>().ok());
    if declarado.is_some_and(|n| n > limite) {
        return Err(Erro::Grande(limite));
    }
    axum::body::to_bytes(corpo, limite)
        .await
        .map(|b| b.to_vec())
        .map_err(|_| Erro::Grande(limite))
}

fn json_de<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, Erro> {
    Ok(serde_json::from_slice(bytes)?)
}

fn exigir_token<'a>(estado: &'a Estado, cabecalhos: &HeaderMap) -> Result<&'a Emissao, Erro> {
    let em = estado
        .emissao
        .as_ref()
        .ok_or_else(|| Erro::NaoEncontrada("rota inexistente".into()))?;
    match bearer(cabecalhos) {
        Some(t) if em.token.confere(t) => Ok(em),
        _ => Err(Erro::NaoAutorizado),
    }
}

pub(super) async fn nao_encontrado() -> ErroHttp {
    ErroHttp(Erro::NaoEncontrada("rota inexistente".into()))
}

pub(super) async fn health(State(estado): State<Arc<Estado>>) -> Response {
    resposta_json(
        StatusCode::OK,
        &json!({
            "status": "ok",
            "versao": env!("CARGO_PKG_VERSION"),
            "emissao": estado.emissao.is_some(),
        }),
    )
}

pub(super) async fn calcular(
    State(estado): State<Arc<Estado>>,
    cabecalhos: HeaderMap,
    corpo: Body,
) -> Resposta {
    let bytes = ler_corpo(&cabecalhos, corpo, estado.config.limite_corpo).await?;
    let op: nucleo::Operacao = json_de(&bytes)?;
    Ok(resposta_json(StatusCode::OK, &nucleo::calcular(&op)?))
}

pub(super) async fn dps_conferir(
    State(estado): State<Arc<Estado>>,
    cabecalhos: HeaderMap,
    corpo: Body,
) -> Resposta {
    let bytes = ler_corpo(&cabecalhos, corpo, estado.config.limite_corpo).await?;
    let dps: Dps = json_de(&bytes)?;
    nucleo::conferir_dps(&dps)?;
    Ok(resposta_json(StatusCode::OK, &nucleo::ok_simples()))
}

pub(super) async fn dps_montar(
    State(estado): State<Arc<Estado>>,
    cabecalhos: HeaderMap,
    corpo: Body,
) -> Resposta {
    let bytes = ler_corpo(&cabecalhos, corpo, estado.config.limite_corpo).await?;
    let dps: Dps = json_de(&bytes)?;
    let xml = nucleo::montar_dps(&dps)?;
    Ok(resposta_json(StatusCode::OK, &json!({"xml": xml})))
}

/// Valor de um parametro da query (`?situacao=cancelada&canhoto=false`); valores simples, sem percentual.
fn parametro<'a>(uri: &'a Uri, nome: &str) -> Option<&'a str> {
    uri.query()?
        .split('&')
        .filter_map(|par| par.split_once('='))
        .find(|(k, _)| *k == nome)
        .map(|(_, v)| v)
}

pub(super) async fn danfse(
    State(estado): State<Arc<Estado>>,
    uri: Uri,
    cabecalhos: HeaderMap,
    corpo: Body,
) -> Resposta {
    let situacao = parametro(&uri, "situacao")
        .map(SituacaoNota::ler)
        .transpose()?
        .unwrap_or_default();
    let canhoto = match parametro(&uri, "canhoto") {
        None | Some("true") => true,
        Some("false") => false,
        Some(_) => return Err(Erro::Entrada("canhoto: use true ou false".into()).into()),
    };
    let bytes = ler_corpo(&cabecalhos, corpo, estado.config.limite_corpo).await?;
    let xml = String::from_utf8(bytes)
        .map_err(|_| Erro::Entrada("o corpo deve ser o XML da NFS-e em UTF-8".into()))?;
    let pdf = tokio::task::spawn_blocking(move || nucleo::gerar_danfse(&xml, situacao, canhoto))
        .await
        .map_err(|_| Erro::Interno)??;
    Ok((
        StatusCode::OK,
        [
            (CONTENT_TYPE, HeaderValue::from_static("application/pdf")),
            (
                CONTENT_DISPOSITION,
                HeaderValue::from_static("inline; filename=\"danfse.pdf\""),
            ),
        ],
        pdf,
    )
        .into_response())
}

pub(super) async fn emitir(
    State(estado): State<Arc<Estado>>,
    cabecalhos: HeaderMap,
    corpo: Body,
) -> Resposta {
    let em = exigir_token(&estado, &cabecalhos)?;
    let bytes = ler_corpo(&cabecalhos, corpo, estado.config.limite_corpo).await?;
    let dps: Dps = json_de(&bytes)?;
    let r = emissao::emitir(em.emissor.as_ref(), dps, estado.config.folga_relogio).await?;
    Ok(resposta_json(StatusCode::OK, &r))
}

pub(super) async fn cancelar(
    State(estado): State<Arc<Estado>>,
    cabecalhos: HeaderMap,
    corpo: Body,
) -> Resposta {
    let em = exigir_token(&estado, &cabecalhos)?;
    let bytes = ler_corpo(&cabecalhos, corpo, estado.config.limite_corpo).await?;
    let p: nucleo::PedidoCancelamento = json_de(&bytes)?;
    let r = emissao::cancelar(em.emissor.as_ref(), &p.chave, &p.motivo, &p.justificativa).await?;
    Ok(resposta_json(StatusCode::OK, &r))
}

pub(super) async fn consultar(
    State(estado): State<Arc<Estado>>,
    cabecalhos: HeaderMap,
    Path(chave): Path<String>,
) -> Resposta {
    let em = exigir_token(&estado, &cabecalhos)?;
    let r = emissao::consultar(em.emissor.as_ref(), &chave).await?;
    Ok(resposta_json(StatusCode::OK, &r))
}

pub(super) async fn mcp(
    State(estado): State<Arc<Estado>>,
    cabecalhos: HeaderMap,
    corpo: Body,
) -> Resposta {
    let bytes = ler_corpo(&cabecalhos, corpo, estado.config.limite_corpo).await?;
    Ok(crate::mcp::tratar(&estado, &cabecalhos, &bytes).await)
}
