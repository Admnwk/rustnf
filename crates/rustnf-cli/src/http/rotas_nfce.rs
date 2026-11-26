// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! As rotas `/nfce/`: ler o corpo, chamar [`ServidorNfce`](super::ServidorNfce), escrever a resposta. A NFC-e que a SEFAZ nao
//! autorizou volta com o JSON completo (chave, cStat, situacao na pasta) e o envelope `erro` junto, no status HTTP do erro,
//! como na NF-e: a chave da nota pendente nunca se perde. Nenhuma rota le, grava ou apaga o CSC.

use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::header::{CONTENT_DISPOSITION, CONTENT_TYPE};
use axum::http::{HeaderMap, HeaderValue, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use rustnf_app::nfce::danfce::Pedido as PedidoCupom;
use serde_json::Value;

use super::nfce::separar_modo;
use super::nfe::PedidoCancelar;
use super::rotas::{estrito, exigir_token, ler_corpo, parametro};
use super::rotas_nfe::{booleano, corpo_json, saida};
use super::{resposta_json, Estado};
use crate::erro::{Erro, ErroHttp};

type Resposta = Result<Response, ErroHttp>;

/// As rotas fixas de `/nfce/`: tudo o mais abaixo de `/nfce/` e' a chave de uma nota.
const FIXAS: [&str; 5] = ["danfce", "emitir", "cancelar", "status", "pendentes"];

/// O caminho e' a consulta de uma NFC-e (`/nfce/{chave}`)? A chave nao vai para o registro do servidor.
pub(super) fn e_consulta_de_nota(caminho: &str) -> bool {
    caminho
        .strip_prefix("/nfce/")
        .is_some_and(|resto| !resto.is_empty() && !resto.contains('/') && !FIXAS.contains(&resto))
}

/// `?nome=80` e afins: numero pequeno; ausente = `None`.
fn numero(uri: &Uri, nome: &str) -> Result<Option<u8>, Erro> {
    parametro(uri, nome)
        .map(|v| {
            v.parse::<u8>()
                .map_err(|_| Erro::Entrada(format!("{nome}: informe um numero")))
        })
        .transpose()
}

pub(super) async fn danfce(
    State(estado): State<Arc<Estado>>,
    uri: Uri,
    cabecalhos: HeaderMap,
    corpo: Body,
) -> Resposta {
    let escpos = match parametro(&uri, "formato") {
        None | Some("pdf") => false,
        Some("escpos") => true,
        Some(_) => return Err(Erro::Entrada("formato: use pdf ou escpos".into()).into()),
    };
    let pedido = PedidoCupom {
        largura: numero(&uri, "largura")?,
        colunas: numero(&uri, "colunas")?,
        resumido: booleano(&uri, "resumido")?.unwrap_or(false),
        via_estabelecimento: booleano(&uri, "via_estabelecimento")?.unwrap_or(false),
    };
    let cancelada = booleano(&uri, "cancelada")?.unwrap_or(false);
    let bytes = ler_corpo(&cabecalhos, corpo, estado.config.limite_corpo).await?;
    let xml = String::from_utf8(bytes)
        .map_err(|_| Erro::Entrada("o corpo deve ser o XML da NFC-e (nfeProc) em UTF-8".into()))?;
    let bytes = estado.nfce.danfce(xml, pedido, cancelada, escpos).await?;
    let (tipo, disposicao) = if escpos {
        (
            "application/octet-stream",
            "attachment; filename=\"danfce.bin\"",
        )
    } else {
        ("application/pdf", "inline; filename=\"danfce.pdf\"")
    };
    Ok((
        StatusCode::OK,
        [
            (CONTENT_TYPE, HeaderValue::from_static(tipo)),
            (CONTENT_DISPOSITION, HeaderValue::from_static(disposicao)),
        ],
        bytes,
    )
        .into_response())
}

pub(super) async fn emitir(
    State(estado): State<Arc<Estado>>,
    uri: Uri,
    cabecalhos: HeaderMap,
    corpo: Body,
) -> Resposta {
    exigir_token(&estado, &cabecalhos)?;
    let pedido: Value = corpo_json(&estado, &cabecalhos, corpo).await?;
    let (pedido, modo) = separar_modo(pedido)?;
    // por padrao o campo desconhecido recusa o pedido antes de qualquer envio; `?estrito=false` o ignora (o CSC, nunca)
    let s = estado
        .nfce
        .emitir(pedido, estrito(&uri, true)?, modo)
        .await?;
    Ok(saida(&s))
}

pub(super) async fn cancelar(
    State(estado): State<Arc<Estado>>,
    cabecalhos: HeaderMap,
    corpo: Body,
) -> Resposta {
    exigir_token(&estado, &cabecalhos)?;
    let p: PedidoCancelar = corpo_json(&estado, &cabecalhos, corpo).await?;
    Ok(saida(&estado.nfce.cancelar(&p).await?))
}

pub(super) async fn consultar(
    State(estado): State<Arc<Estado>>,
    cabecalhos: HeaderMap,
    Path(chave): Path<String>,
) -> Resposta {
    exigir_token(&estado, &cabecalhos)?;
    Ok(saida(&estado.nfce.consultar(&chave).await?))
}

pub(super) async fn status(State(estado): State<Arc<Estado>>, cabecalhos: HeaderMap) -> Resposta {
    exigir_token(&estado, &cabecalhos)?;
    Ok(saida(&estado.nfce.status().await?))
}

pub(super) async fn pendentes(
    State(estado): State<Arc<Estado>>,
    cabecalhos: HeaderMap,
) -> Resposta {
    exigir_token(&estado, &cabecalhos)?;
    Ok(resposta_json(StatusCode::OK, &estado.nfce.pendentes()?))
}

pub(super) async fn retomar(State(estado): State<Arc<Estado>>, cabecalhos: HeaderMap) -> Resposta {
    exigir_token(&estado, &cabecalhos)?;
    Ok(saida(&estado.nfce.retomar().await?))
}
