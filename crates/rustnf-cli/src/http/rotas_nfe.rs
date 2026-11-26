// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! As rotas `/nfe/`: ler o corpo, chamar [`ServidorNfe`](super::ServidorNfe), escrever a resposta. O que a SEFAZ nao
//! autorizou volta com o JSON completo (chave, cStat, situacao na pasta) e o envelope `erro` junto, no status HTTP do erro.

use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::header::{CONTENT_DISPOSITION, CONTENT_TYPE};
use axum::http::{HeaderMap, HeaderValue, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use serde_json::Value;

use super::nfe::{PedidoCancelar, PedidoCorrigir, PedidoInutilizar, Saida};
use super::nfe_recebidas::{
    sem_codificacao, situacao_do_filtro, PedidoBaixar, PedidoManifestar, PedidoSincronizar,
};
use super::rotas::{estrito, exigir_token, json_de, ler_corpo, parametro};
use super::{resposta_json, Estado};
use crate::erro::{Erro, ErroHttp, StatusHttp};

type Resposta = Result<Response, ErroHttp>;

/// As rotas fixas de `/nfe/`: tudo o mais abaixo de `/nfe/` e' a chave de uma nota.
const FIXAS: [&str; 11] = [
    "conferir",
    "montar",
    "danfe",
    "emitir",
    "epec",
    "cancelar",
    "corrigir",
    "inutilizar",
    "status",
    "pendentes",
    "recebidas",
];

/// O caminho e' a consulta de uma nota (`/nfe/{chave}`)? A chave nao vai para o registro do servidor.
pub(super) fn e_consulta_de_nota(caminho: &str) -> bool {
    caminho
        .strip_prefix("/nfe/")
        .is_some_and(|resto| !resto.is_empty() && !resto.contains('/') && !FIXAS.contains(&resto))
}

fn saida(s: &Saida) -> Response {
    let status = s.erro.as_ref().map_or(200, StatusHttp::status_http);
    resposta_json(
        StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
        &s.corpo_com_erro(),
    )
}

/// `?nome=true|false`; ausente = `None`.
fn booleano(uri: &Uri, nome: &str) -> Result<Option<bool>, Erro> {
    match parametro(uri, nome) {
        None => Ok(None),
        Some("true") => Ok(Some(true)),
        Some("false") => Ok(Some(false)),
        Some(_) => Err(Erro::Entrada(format!("{nome}: use true ou false"))),
    }
}

async fn corpo_json<T: serde::de::DeserializeOwned>(
    estado: &Estado,
    cabecalhos: &HeaderMap,
    corpo: Body,
) -> Result<T, Erro> {
    let bytes = ler_corpo(cabecalhos, corpo, estado.config.limite_corpo).await?;
    json_de(&bytes)
}

pub(super) async fn conferir(
    State(estado): State<Arc<Estado>>,
    uri: Uri,
    cabecalhos: HeaderMap,
    corpo: Body,
) -> Resposta {
    let pedido: Value = corpo_json(&estado, &cabecalhos, corpo).await?;
    Ok(saida(&estado.nfe.conferir(pedido, estrito(&uri, true)?)))
}

pub(super) async fn montar(
    State(estado): State<Arc<Estado>>,
    uri: Uri,
    cabecalhos: HeaderMap,
    corpo: Body,
) -> Resposta {
    let pedido: Value = corpo_json(&estado, &cabecalhos, corpo).await?;
    let v = estado.nfe.montar(pedido, estrito(&uri, true)?)?;
    Ok(resposta_json(StatusCode::OK, &v))
}

pub(super) async fn danfe(
    State(estado): State<Arc<Estado>>,
    uri: Uri,
    cabecalhos: HeaderMap,
    corpo: Body,
) -> Resposta {
    let canhoto = booleano(&uri, "canhoto")?;
    let cancelada = booleano(&uri, "cancelada")?.unwrap_or(false);
    let bytes = ler_corpo(&cabecalhos, corpo, estado.config.limite_corpo).await?;
    let xml = String::from_utf8(bytes)
        .map_err(|_| Erro::Entrada("o corpo deve ser o XML da NF-e (nfeProc) em UTF-8".into()))?;
    let pdf = estado.nfe.danfe(xml, canhoto, cancelada).await?;
    Ok((
        StatusCode::OK,
        [
            (CONTENT_TYPE, HeaderValue::from_static("application/pdf")),
            (
                CONTENT_DISPOSITION,
                HeaderValue::from_static("inline; filename=\"danfe.pdf\""),
            ),
        ],
        pdf,
    )
        .into_response())
}

async fn emitir_ou_epec(
    estado: Arc<Estado>,
    uri: Uri,
    cabecalhos: HeaderMap,
    corpo: Body,
    epec: bool,
) -> Resposta {
    exigir_token(&estado, &cabecalhos)?;
    let pedido: Value = corpo_json(&estado, &cabecalhos, corpo).await?;
    // por padrao o campo desconhecido recusa o pedido antes de qualquer envio; `?estrito=false` o ignora
    let s = estado
        .nfe
        .emitir(pedido, estrito(&uri, true)?, epec)
        .await?;
    Ok(saida(&s))
}

pub(super) async fn emitir(
    State(estado): State<Arc<Estado>>,
    uri: Uri,
    cabecalhos: HeaderMap,
    corpo: Body,
) -> Resposta {
    emitir_ou_epec(estado, uri, cabecalhos, corpo, false).await
}

pub(super) async fn epec(
    State(estado): State<Arc<Estado>>,
    uri: Uri,
    cabecalhos: HeaderMap,
    corpo: Body,
) -> Resposta {
    emitir_ou_epec(estado, uri, cabecalhos, corpo, true).await
}

pub(super) async fn cancelar(
    State(estado): State<Arc<Estado>>,
    cabecalhos: HeaderMap,
    corpo: Body,
) -> Resposta {
    exigir_token(&estado, &cabecalhos)?;
    let p: PedidoCancelar = corpo_json(&estado, &cabecalhos, corpo).await?;
    Ok(saida(&estado.nfe.cancelar(&p).await?))
}

pub(super) async fn corrigir(
    State(estado): State<Arc<Estado>>,
    cabecalhos: HeaderMap,
    corpo: Body,
) -> Resposta {
    exigir_token(&estado, &cabecalhos)?;
    let p: PedidoCorrigir = corpo_json(&estado, &cabecalhos, corpo).await?;
    Ok(saida(&estado.nfe.corrigir(&p).await?))
}

pub(super) async fn inutilizar(
    State(estado): State<Arc<Estado>>,
    cabecalhos: HeaderMap,
    corpo: Body,
) -> Resposta {
    exigir_token(&estado, &cabecalhos)?;
    let p: PedidoInutilizar = corpo_json(&estado, &cabecalhos, corpo).await?;
    Ok(saida(&estado.nfe.inutilizar(&p).await?))
}

pub(super) async fn consultar(
    State(estado): State<Arc<Estado>>,
    cabecalhos: HeaderMap,
    Path(chave): Path<String>,
) -> Resposta {
    exigir_token(&estado, &cabecalhos)?;
    Ok(saida(&estado.nfe.consultar(&chave).await?))
}

pub(super) async fn status(
    State(estado): State<Arc<Estado>>,
    uri: Uri,
    cabecalhos: HeaderMap,
) -> Resposta {
    exigir_token(&estado, &cabecalhos)?;
    let svc = booleano(&uri, "svc")?.unwrap_or(false);
    Ok(saida(&estado.nfe.status(svc).await?))
}

pub(super) async fn pendentes(
    State(estado): State<Arc<Estado>>,
    cabecalhos: HeaderMap,
) -> Resposta {
    exigir_token(&estado, &cabecalhos)?;
    Ok(resposta_json(StatusCode::OK, &estado.nfe.pendentes()?))
}

pub(super) async fn retomar(State(estado): State<Arc<Estado>>, cabecalhos: HeaderMap) -> Resposta {
    exigir_token(&estado, &cabecalhos)?;
    Ok(saida(&estado.nfe.retomar().await?))
}

// ---------------------------------------------------------------------------------------- documentos recebidos

pub(super) async fn recebidas_sincronizar(
    State(estado): State<Arc<Estado>>,
    cabecalhos: HeaderMap,
    corpo: Body,
) -> Resposta {
    exigir_token(&estado, &cabecalhos)?;
    // o corpo e' opcional: sem ele vale o padrao
    let bytes = ler_corpo(&cabecalhos, corpo, estado.config.limite_corpo).await?;
    let p: PedidoSincronizar = if bytes.iter().all(u8::is_ascii_whitespace) {
        PedidoSincronizar::default()
    } else {
        json_de(&bytes)?
    };
    Ok(saida(&estado.nfe.sincronizar_recebidas(&p).await?))
}

pub(super) async fn recebidas_baixar(
    State(estado): State<Arc<Estado>>,
    cabecalhos: HeaderMap,
    corpo: Body,
) -> Resposta {
    exigir_token(&estado, &cabecalhos)?;
    let p: PedidoBaixar = corpo_json(&estado, &cabecalhos, corpo).await?;
    Ok(saida(&estado.nfe.baixar_recebida(&p).await?))
}

pub(super) async fn recebidas_manifestar(
    State(estado): State<Arc<Estado>>,
    cabecalhos: HeaderMap,
    corpo: Body,
) -> Resposta {
    exigir_token(&estado, &cabecalhos)?;
    let p: PedidoManifestar = corpo_json(&estado, &cabecalhos, corpo).await?;
    Ok(saida(&estado.nfe.manifestar_recebida(&p).await?))
}

pub(super) async fn recebidas_listar(
    State(estado): State<Arc<Estado>>,
    uri: Uri,
    cabecalhos: HeaderMap,
) -> Resposta {
    exigir_token(&estado, &cabecalhos)?;
    let situacao = situacao_do_filtro(parametro(&uri, "situacao"))?;
    let v = estado.nfe.listar_recebidas(
        situacao,
        &sem_codificacao(parametro(&uri, "busca").unwrap_or("")),
    )?;
    Ok(resposta_json(StatusCode::OK, &v))
}

pub(super) async fn recebidas_cursor(
    State(estado): State<Arc<Estado>>,
    cabecalhos: HeaderMap,
) -> Resposta {
    exigir_token(&estado, &cabecalhos)?;
    Ok(resposta_json(
        StatusCode::OK,
        &estado.nfe.cursor_recebidas()?,
    ))
}
