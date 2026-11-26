// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Servidor MCP em `POST /mcp`, transporte Streamable HTTP, so' com resposta JSON (sem SSE) e sem estado.
//!
//! Especificacao implementada: **2026-07-28** (revisao vigente na data da leitura, 2026-11-20), pagina do
//! transporte <https://modelcontextprotocol.io/specification/2026-07-28/basic/transports/streamable-http>:
//! cada requisicao e' independente e leva a versao, o cliente e as capacidades em `_meta`; cabecalhos
//! `MCP-Protocol-Version`, `Mcp-Method` e `Mcp-Name` conferidos contra o corpo (`-32020` com HTTP 400);
//! versao desconhecida devolve `-32022` listando as aceitas; metodo desconhecido, HTTP 404 e `-32601`;
//! `server/discover`, `tools/list` e `tools/call`; `resultType: "complete"`; `ttlMs` e `cacheScope` nos resultados
//! de lista e de descoberta; `Origin` invalido, HTTP 403 (conferido antes, em [`crate::http`]).
//!
//! Compatibilidade com a revisao anterior, **2025-11-25**
//! (<https://modelcontextprotocol.io/specification/2025-11-25/basic/lifecycle>): `initialize` responde com a versao
//! `2025-11-25` e as capacidades (sem `MCP-Session-Id`, que a especificacao deixa opcional), a notificacao
//! `notifications/initialized` recebe `202 Accepted`, e os pedidos seguintes levam `MCP-Protocol-Version: 2025-11-25`.
//!
//! Fora do que foi implementado: SSE (respostas em fluxo e `subscriptions/listen`), `GET` e `DELETE` no endpoint
//! (405), recursos, modelos de mensagem, logging, completions, tarefas, ciclos de entrada adicional (MRTR), paginacao
//! (`nextCursor`), versoes anteriores a 2025-11-25 e autorizacao OAuth (as ferramentas de emissao usam um Bearer
//! fixo do operador).

mod ferramentas;
mod ferramentas_nfe;
mod schemas;
pub mod schemas_nfe;

use axum::http::header::AUTHORIZATION;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use base64::Engine;
use serde_json::{json, Map, Value};

use crate::erro::{Erro, ErroHttp};
use crate::http::auth::bearer;
use crate::http::{resposta_json, Estado};

pub use ferramentas::{buscar, listar, Ferramenta};
pub use schemas::{
    atualizacao, cancelamento, consulta, consulta_dps, consulta_evento, danfse, dps,
    dps_de_emissao, operacao, registro_de_evento,
};

/// Revisao da especificacao implementada por inteiro.
pub const VERSAO_MODERNA: &str = "2026-07-28";
/// Revisao anterior, atendida com `initialize`.
pub const VERSAO_LEGADA: &str = "2025-11-25";
/// Pagina do transporte na revisao implementada.
pub const ESPECIFICACAO: &str =
    "https://modelcontextprotocol.io/specification/2026-07-28/basic/transports/streamable-http";

const META_VERSAO: &str = "io.modelcontextprotocol/protocolVersion";
const META_CAPACIDADES: &str = "io.modelcontextprotocol/clientCapabilities";
const META_SERVIDOR: &str = "io.modelcontextprotocol/serverInfo";

const PARSE: i64 = -32700;
const REQUISICAO: i64 = -32600;
const METODO: i64 = -32601;
const PARAMETROS: i64 = -32602;
const CABECALHO: i64 = -32020;
const VERSAO_NAO_SUPORTADA: i64 = -32022;

enum Saida {
    /// 202 sem corpo.
    Aceito,
    /// Resposta JSON-RPC com o status HTTP.
    Rpc(StatusCode, Value),
    /// Erro HTTP fora do JSON-RPC (401).
    Http(Erro),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Era {
    Moderna,
    Legada,
}

fn erro_rpc(
    status: StatusCode,
    id: &Value,
    codigo: i64,
    mensagem: &str,
    dados: Option<Value>,
) -> Saida {
    let mut erro = json!({"code": codigo, "message": mensagem});
    if let Some(d) = dados {
        erro["data"] = d;
    }
    Saida::Rpc(status, json!({"jsonrpc": "2.0", "id": id, "error": erro}))
}

fn sucesso(id: &Value, resultado: Value) -> Saida {
    Saida::Rpc(
        StatusCode::OK,
        json!({"jsonrpc": "2.0", "id": id, "result": resultado}),
    )
}

fn nao_suportada(id: &Value, pedida: Option<&str>) -> Saida {
    erro_rpc(
        StatusCode::BAD_REQUEST,
        id,
        VERSAO_NAO_SUPORTADA,
        "Unsupported protocol version",
        Some(json!({"supported": [VERSAO_MODERNA, VERSAO_LEGADA], "requested": pedida})),
    )
}

fn identidade() -> Value {
    json!({"name": "rustnf", "title": "rustnf: NFS-e nacional", "version": env!("CARGO_PKG_VERSION")})
}

/// Acrescenta o que a revisao moderna exige em todo resultado.
fn completar(era: Era, mut resultado: Value) -> Value {
    if era == Era::Moderna {
        resultado["resultType"] = json!("complete");
        resultado["_meta"] = json!({ META_SERVIDOR: identidade() });
    }
    resultado
}

/// `ttlMs` e `cacheScope`, obrigatorios nos resultados de lista e de descoberta da revisao moderna.
fn com_cache(era: Era, mut resultado: Value, escopo: &str) -> Value {
    if era == Era::Moderna {
        resultado["ttlMs"] = json!(0);
        resultado["cacheScope"] = json!(escopo);
    }
    completar(era, resultado)
}

fn cabecalho<'a>(h: &'a HeaderMap, nome: &str) -> Option<&'a str> {
    h.get(nome).and_then(|v| v.to_str().ok())
}

/// `Mcp-Name` pode vir em Base64 (`=?base64?...?=`) quando o valor nao cabe em ASCII simples.
fn decodificar_nome(valor: &str) -> Option<String> {
    match valor
        .strip_prefix("=?base64?")
        .and_then(|r| r.strip_suffix("?="))
    {
        Some(b64) => {
            let bytes = base64::engine::general_purpose::STANDARD.decode(b64).ok()?;
            String::from_utf8(bytes).ok()
        }
        None => Some(valor.to_string()),
    }
}

fn divergencia(id: &Value, mensagem: &str) -> Saida {
    erro_rpc(StatusCode::BAD_REQUEST, id, CABECALHO, mensagem, None)
}

/// Confere o que a revisao moderna exige de toda requisicao: `_meta`, cabecalhos espelhados e versao.
fn validar_moderna(
    h: &HeaderMap,
    metodo: &str,
    params: Option<&Value>,
    id: &Value,
) -> Result<(), Saida> {
    let meta = params
        .and_then(|p| p.get("_meta"))
        .and_then(Value::as_object);
    let versao = meta
        .and_then(|m| m.get(META_VERSAO))
        .and_then(Value::as_str);
    let capacidades = meta
        .and_then(|m| m.get(META_CAPACIDADES))
        .is_some_and(Value::is_object);
    let Some(versao) = versao.filter(|_| capacidades) else {
        return Err(erro_rpc(
            StatusCode::BAD_REQUEST,
            id,
            PARAMETROS,
            "faltam campos obrigatorios em _meta (protocolVersion e clientCapabilities)",
            None,
        ));
    };
    match cabecalho(h, "mcp-protocol-version") {
        None => {
            return Err(divergencia(
                id,
                "Header mismatch: MCP-Protocol-Version ausente",
            ))
        }
        Some(c) if c != versao => {
            return Err(divergencia(
                id,
                "Header mismatch: MCP-Protocol-Version difere do _meta do corpo",
            ))
        }
        Some(_) => {}
    }
    if versao != VERSAO_MODERNA {
        return Err(nao_suportada(id, Some(versao)));
    }
    if cabecalho(h, "mcp-method") != Some(metodo) {
        return Err(divergencia(
            id,
            "Header mismatch: Mcp-Method ausente ou difere do corpo",
        ));
    }
    if metodo == "tools/call" {
        let nome = params.and_then(|p| p.get("name")).and_then(Value::as_str);
        let enviado = cabecalho(h, "mcp-name").and_then(decodificar_nome);
        if nome.is_none() || enviado.as_deref() != nome {
            return Err(divergencia(
                id,
                "Header mismatch: Mcp-Name ausente ou difere do corpo",
            ));
        }
    }
    Ok(())
}

/// A revisao anterior: so' a versao do cabecalho, quando ele vem.
fn validar_legada(h: &HeaderMap, id: &Value) -> Result<(), Saida> {
    match cabecalho(h, "mcp-protocol-version") {
        Some(v) if v == VERSAO_LEGADA => Ok(()),
        outra => Err(nao_suportada(id, outra)),
    }
}

fn inicializar(id: &Value, params: Option<&Value>) -> Saida {
    let valido = params.is_some_and(|p| {
        p.get("protocolVersion").is_some_and(Value::is_string)
            && p.get("capabilities").is_some_and(Value::is_object)
            && p.get("clientInfo").is_some_and(Value::is_object)
    });
    if !valido {
        return erro_rpc(
            StatusCode::OK,
            id,
            PARAMETROS,
            "initialize exige protocolVersion, capabilities e clientInfo",
            None,
        );
    }
    sucesso(
        id,
        json!({
            "protocolVersion": VERSAO_LEGADA,
            "capabilities": {"tools": {"listChanged": false}},
            "serverInfo": identidade(),
            "instructions": "Emissor de NFS-e nacional e de NF-e (modelo 55). Ferramentas de leitura sempre; as que emitem, consultam ou registram eventos (emitir_nfse, emitir_nfe, cancelar_nfe e as demais com efeito fiscal ou de rede) so' com o servidor iniciado com emissao e o Bearer do operador.",
        }),
    )
}

fn descobrir(id: &Value) -> Saida {
    sucesso(
        id,
        com_cache(
            Era::Moderna,
            json!({
                "supportedVersions": [VERSAO_MODERNA, VERSAO_LEGADA],
                "capabilities": {"tools": {}},
                "instructions": "Emissor de NFS-e nacional e de NF-e (modelo 55). Ferramentas de leitura sempre; as que emitem, consultam ou registram eventos (emitir_nfse, emitir_nfe, cancelar_nfe e as demais com efeito fiscal ou de rede) so' com o servidor iniciado com emissao e o Bearer do operador.",
            }),
            "public",
        ),
    )
}

async fn chamar(
    estado: &Estado,
    era: Era,
    id: &Value,
    params: Option<&Value>,
    autenticado: bool,
) -> Saida {
    let Some(nome) = params.and_then(|p| p.get("name")).and_then(Value::as_str) else {
        return erro_rpc(
            StatusCode::OK,
            id,
            PARAMETROS,
            "tools/call exige params.name",
            None,
        );
    };
    let args = match params.and_then(|p| p.get("arguments")) {
        None | Some(Value::Null) => json!({}),
        Some(a) if a.is_object() => a.clone(),
        Some(_) => {
            return erro_rpc(
                StatusCode::OK,
                id,
                PARAMETROS,
                "arguments deve ser um objeto",
                None,
            )
        }
    };
    let desconhecida = || {
        erro_rpc(
            StatusCode::OK,
            id,
            PARAMETROS,
            &format!("Unknown tool: {nome}"),
            None,
        )
    };
    let Some(f) = buscar(nome) else {
        return desconhecida();
    };
    if f.emissao {
        if estado.emissao.is_none() {
            return desconhecida();
        }
        if !autenticado {
            return Saida::Http(Erro::NaoAutorizado);
        }
    }
    sucesso(
        id,
        completar(era, ferramentas::executar(f, estado, args).await),
    )
}

async fn processar(estado: &Estado, h: &HeaderMap, bytes: &[u8], autenticado: bool) -> Saida {
    let sem_id = Value::Null;
    let Ok(mensagem) = serde_json::from_slice::<Value>(bytes) else {
        return erro_rpc(StatusCode::BAD_REQUEST, &sem_id, PARSE, "Parse error", None);
    };
    let Some(obj) = mensagem.as_object() else {
        return erro_rpc(
            StatusCode::BAD_REQUEST,
            &sem_id,
            REQUISICAO,
            "a mensagem deve ser um unico objeto JSON-RPC (lotes nao sao aceitos)",
            None,
        );
    };
    if obj.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return erro_rpc(
            StatusCode::BAD_REQUEST,
            &sem_id,
            REQUISICAO,
            "jsonrpc deve ser \"2.0\"",
            None,
        );
    }
    let Some(metodo) = obj.get("method").and_then(Value::as_str) else {
        // resposta do cliente: a revisao anterior manda aceitar com 202
        return if obj.contains_key("result") || obj.contains_key("error") {
            Saida::Aceito
        } else {
            erro_rpc(
                StatusCode::BAD_REQUEST,
                &sem_id,
                REQUISICAO,
                "mensagem sem method",
                None,
            )
        };
    };
    let Some(id) = obj.get("id") else {
        return Saida::Aceito; // notificacao
    };
    if !(id.is_string() || id.is_i64() || id.is_u64()) {
        return erro_rpc(
            StatusCode::BAD_REQUEST,
            &sem_id,
            REQUISICAO,
            "id deve ser texto ou inteiro",
            None,
        );
    }
    let params = obj.get("params");
    let meta_tem_versao = params
        .and_then(|p| p.get("_meta"))
        .and_then(Value::as_object)
        .is_some_and(|m: &Map<String, Value>| m.contains_key(META_VERSAO));
    let era = if metodo == "server/discover"
        || meta_tem_versao
        || cabecalho(h, "mcp-protocol-version") == Some(VERSAO_MODERNA)
    {
        Era::Moderna
    } else {
        Era::Legada
    };
    if era == Era::Legada && metodo == "initialize" {
        return inicializar(id, params);
    }
    let validada = match era {
        Era::Moderna => validar_moderna(h, metodo, params, id),
        Era::Legada => validar_legada(h, id),
    };
    if let Err(saida) = validada {
        return saida;
    }
    match (metodo, era) {
        ("server/discover", _) => descobrir(id),
        ("ping", Era::Legada) => sucesso(id, json!({})),
        ("tools/list", _) => {
            let com_emissao = estado.emissao.is_some() && autenticado;
            sucesso(
                id,
                com_cache(era, json!({"tools": listar(com_emissao)}), "private"),
            )
        }
        ("tools/call", _) => chamar(estado, era, id, params, autenticado).await,
        _ => erro_rpc(
            if era == Era::Moderna {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::OK
            },
            id,
            METODO,
            "Method not found",
            None,
        ),
    }
}

/// Trata uma requisicao `POST /mcp` ja com o corpo lido.
pub async fn tratar(estado: &Estado, cabecalhos: &HeaderMap, corpo: &[u8]) -> Response {
    // Bearer presente e errado e' 401; ausente so' esconde as ferramentas de emissao.
    let mut autenticado = false;
    if let Some(em) = &estado.emissao {
        if cabecalhos.contains_key(AUTHORIZATION) {
            match bearer(cabecalhos) {
                Some(t) if em.token.confere(t) => autenticado = true,
                _ => return ErroHttp(Erro::NaoAutorizado).into_response(),
            }
        }
    }
    match processar(estado, cabecalhos, corpo, autenticado).await {
        Saida::Aceito => StatusCode::ACCEPTED.into_response(),
        Saida::Rpc(status, corpo) => resposta_json(status, &corpo),
        Saida::Http(e) => ErroHttp(e).into_response(),
    }
}
