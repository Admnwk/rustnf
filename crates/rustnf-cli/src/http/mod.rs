// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! API HTTP (`rustnf serve`): rotas JSON, MCP em `POST /mcp`, teto de corpo, tempo limite, validacao de `Origin`,
//! sem CORS. As rotas de emissao so' existem quando o servidor sobe com emissao e token.

pub mod auth;
mod rotas;

use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::header::{CONTENT_TYPE, ORIGIN};
use axum::http::{HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use serde_json::{json, Value};

pub use auth::Token;

use crate::emissao::Emissor;

/// Ajustes do servidor.
#[derive(Debug, Clone)]
pub struct Config {
    /// Teto do corpo da requisicao, em bytes.
    pub limite_corpo: usize,
    /// Tempo limite de cada requisicao.
    pub tempo_limite: Duration,
    /// Origens extras aceitas no cabecalho `Origin`, alem de `localhost`, `127.0.0.1` e `[::1]`.
    pub origens_permitidas: Vec<String>,
    /// Folga do relogio, em segundos, para o `dhEmi` quando o pedido nao traz.
    pub folga_relogio: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            limite_corpo: 1024 * 1024,
            tempo_limite: Duration::from_secs(60),
            origens_permitidas: Vec::new(),
            folga_relogio: 60,
        }
    }
}

/// Emissor e token: so' existem juntos.
pub struct Emissao {
    /// Fala com a Sefin Nacional.
    pub emissor: Arc<dyn Emissor>,
    /// Token das rotas de emissao.
    pub token: Token,
}

/// O que as rotas compartilham.
pub struct Estado {
    /// Ajustes.
    pub config: Config,
    /// Presente so' com `--permitir-emissao`.
    pub emissao: Option<Emissao>,
}

impl Estado {
    /// Servidor sem emissao: so' calculo, conferencia, montagem, DANFSe e MCP de leitura.
    #[must_use]
    pub fn somente_leitura(config: Config) -> Self {
        Self {
            config,
            emissao: None,
        }
    }

    /// Servidor com emissao, consulta e cancelamento, protegidos pelo token.
    #[must_use]
    pub fn com_emissao(config: Config, emissor: Arc<dyn Emissor>, token: Token) -> Self {
        Self {
            config,
            emissao: Some(Emissao { emissor, token }),
        }
    }
}

/// Resposta JSON.
pub(crate) fn resposta_json(status: StatusCode, corpo: &Value) -> Response {
    (
        status,
        [(CONTENT_TYPE, HeaderValue::from_static("application/json"))],
        corpo.to_string(),
    )
        .into_response()
}

/// O roteador completo. Serve em memoria nos testes (`tower::ServiceExt::oneshot`) e, em producao, por
/// [`servir`](crate::cli::servir).
pub fn roteador(estado: Estado) -> Router {
    let estado = Arc::new(estado);
    let mut r = Router::new()
        .route("/health", get(rotas::health))
        .route("/calcular", post(rotas::calcular))
        .route("/dps/conferir", post(rotas::dps_conferir))
        .route("/dps/montar", post(rotas::dps_montar))
        .route("/danfse", post(rotas::danfse))
        .route("/mcp", post(rotas::mcp));
    if estado.emissao.is_some() {
        r = r
            .route("/emitir", post(rotas::emitir))
            .route("/cancelar", post(rotas::cancelar))
            .route("/nfse/{chave}", get(rotas::consultar));
    }
    r.fallback(rotas::nao_encontrado)
        .layer(middleware::from_fn_with_state(
            estado.clone(),
            limite_de_tempo,
        ))
        .layer(middleware::from_fn(padronizar_erro))
        .layer(middleware::from_fn_with_state(
            estado.clone(),
            validar_origem,
        ))
        .layer(middleware::from_fn(registrar))
        .with_state(estado)
}

/// Nome da rota para o registro: a chave de acesso nunca vai para o log.
fn rota_para_log(caminho: &str) -> &str {
    if caminho.starts_with("/nfse/") {
        "/nfse/{chave}"
    } else {
        caminho
    }
}

async fn registrar(req: Request, next: Next) -> Response {
    let metodo = req.method().clone();
    let caminho = rota_para_log(req.uri().path()).to_string();
    let resposta = next.run(req).await;
    eprintln!("{metodo} {caminho} {}", resposta.status().as_u16());
    resposta
}

async fn limite_de_tempo(State(e): State<Arc<Estado>>, req: Request, next: Next) -> Response {
    match tokio::time::timeout(e.config.tempo_limite, next.run(req)).await {
        Ok(r) => r,
        Err(_) => resposta_json(
            StatusCode::GATEWAY_TIMEOUT,
            &json!({"erro": {"codigo": "tempo_esgotado", "mensagem": "a requisicao passou do tempo limite"}}),
        ),
    }
}

/// Erro sem corpo JSON (404 de rota, 405 de metodo, 415 e afins do axum) vira o envelope unico.
async fn padronizar_erro(req: Request, next: Next) -> Response {
    let r = next.run(req).await;
    let status = r.status();
    if !(status.is_client_error() || status.is_server_error())
        || r.headers().contains_key(CONTENT_TYPE)
    {
        return r;
    }
    let (codigo, mensagem) = match status.as_u16() {
        404 => ("nao_encontrada", "rota inexistente"),
        405 => ("metodo_nao_permitido", "metodo nao permitido nesta rota"),
        408 | 504 => ("tempo_esgotado", "a requisicao passou do tempo limite"),
        413 => ("corpo_grande", "corpo maior que o limite"),
        415 => ("tipo_nao_suportado", "tipo de conteudo nao suportado"),
        400..=499 => ("requisicao_invalida", "requisicao invalida"),
        _ => ("erro_interno", "falha interna do programa"),
    };
    let (mut partes, _) = r.into_parts();
    partes.headers.remove(axum::http::header::CONTENT_LENGTH);
    let mut nova = resposta_json(
        status,
        &json!({"erro": {"codigo": codigo, "mensagem": mensagem}}),
    );
    for (nome, valor) in &partes.headers {
        nova.headers_mut().append(nome.clone(), valor.clone());
    }
    nova
}

fn computador_local(host: &str) -> bool {
    matches!(host, "localhost" | "127.0.0.1" | "[::1]")
}

/// `Origin` aceito: so' o proprio computador, mais as origens configuradas.
fn origem_permitida(origem: &str, extras: &[String]) -> bool {
    if extras.iter().any(|e| e.eq_ignore_ascii_case(origem)) {
        return true;
    }
    let Some(resto) = origem
        .strip_prefix("http://")
        .or_else(|| origem.strip_prefix("https://"))
    else {
        return false;
    };
    let autoridade = resto.split('/').next().unwrap_or("");
    let host = if autoridade.starts_with('[') {
        autoridade.split_inclusive(']').next().unwrap_or("")
    } else {
        autoridade.split(':').next().unwrap_or("")
    };
    computador_local(&host.to_ascii_lowercase())
}

/// Defesa contra DNS rebinding (MCP, transporte Streamable HTTP): `Origin` presente e invalido = 403.
async fn validar_origem(State(e): State<Arc<Estado>>, req: Request<Body>, next: Next) -> Response {
    if let Some(origem) = req.headers().get(ORIGIN) {
        let ok = origem
            .to_str()
            .is_ok_and(|o| origem_permitida(o, &e.config.origens_permitidas));
        if !ok {
            return if req.uri().path() == "/mcp" {
                resposta_json(
                    StatusCode::FORBIDDEN,
                    &json!({"jsonrpc": "2.0", "error": {"code": -32600, "message": "Origin nao permitida"}}),
                )
            } else {
                resposta_json(
                    StatusCode::FORBIDDEN,
                    &json!({"erro": {"codigo": "origem_invalida", "mensagem": "Origin nao permitida"}}),
                )
            };
        }
    }
    next.run(req).await
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn origem_local_passa_e_o_resto_nao() {
        let nenhuma: Vec<String> = Vec::new();
        assert!(origem_permitida("http://localhost:3000", &nenhuma));
        assert!(origem_permitida("http://127.0.0.1", &nenhuma));
        assert!(origem_permitida("https://[::1]:8443", &nenhuma));
        assert!(!origem_permitida("http://exemplo.com.br", &nenhuma));
        assert!(!origem_permitida(
            "http://localhost.exemplo.com.br",
            &nenhuma
        ));
        assert!(!origem_permitida("null", &nenhuma));
        assert!(!origem_permitida("file://localhost", &nenhuma));
        assert!(origem_permitida(
            "https://app.exemplo.com.br",
            &["https://app.exemplo.com.br".to_string()]
        ));
    }

    #[test]
    fn rota_de_nota_nao_vai_com_a_chave_para_o_log() {
        assert_eq!(rota_para_log("/nfse/123"), "/nfse/{chave}");
        assert_eq!(rota_para_log("/health"), "/health");
    }
}
