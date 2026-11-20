// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! MCP em memoria (`POST /mcp`), na revisao 2026-07-28 da especificacao e na compatibilidade com a 2025-11-25:
//! `initialize`, `tools/list` com e sem emissao, `tools/call`, erros JSON-RPC, cabecalhos e `Origin`.

mod comum;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use base64::Engine;
use comum::*;
use serde_json::{json, Value};

const MODERNA: &str = "2026-07-28";
const LEGADA: &str = "2025-11-25";

fn meta() -> Value {
    json!({
        "io.modelcontextprotocol/protocolVersion": MODERNA,
        "io.modelcontextprotocol/clientInfo": {"name": "TesteCliente", "version": "1.0.0"},
        "io.modelcontextprotocol/clientCapabilities": {}
    })
}

/// Requisicao da revisao moderna, com `_meta` e os cabecalhos espelhados.
fn moderna(metodo: &str, mut params: Value, extra: &[(&str, &str)]) -> Request<Body> {
    params["_meta"] = meta();
    let corpo = json!({"jsonrpc": "2.0", "id": 1, "method": metodo, "params": params});
    let nome = params["name"].as_str().map(str::to_string);
    let mut cab: Vec<(&str, &str)> = vec![
        ("content-type", "application/json"),
        ("accept", "application/json, text/event-stream"),
        ("mcp-protocol-version", MODERNA),
        ("mcp-method", metodo),
    ];
    if let Some(n) = nome.as_deref() {
        cab.push(("mcp-name", n));
    }
    cab.extend_from_slice(extra);
    requisicao("POST", "/mcp", &cab, corpo.to_string().into_bytes())
}

/// Requisicao da revisao anterior: sem `_meta`, so' o cabecalho de versao.
fn legada(metodo: &str, params: Value, extra: &[(&str, &str)]) -> Request<Body> {
    let corpo = json!({"jsonrpc": "2.0", "id": 7, "method": metodo, "params": params});
    let mut cab: Vec<(&str, &str)> = vec![
        ("content-type", "application/json"),
        ("mcp-protocol-version", LEGADA),
    ];
    cab.extend_from_slice(extra);
    requisicao("POST", "/mcp", &cab, corpo.to_string().into_bytes())
}

fn bruta(corpo: &str, cab: &[(&str, &str)]) -> Request<Body> {
    requisicao("POST", "/mcp", cab, corpo.as_bytes().to_vec())
}

fn nomes(v: &Value) -> Vec<String> {
    v["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect()
}

fn auth() -> String {
    format!("Bearer {TOKEN}")
}

#[tokio::test]
async fn initialize_e_initialized_na_revisao_anterior() {
    let app = app_leitura();
    let init = legada(
        "initialize",
        json!({"protocolVersion": LEGADA, "capabilities": {}, "clientInfo": {"name": "t", "version": "1"}}),
        &[],
    );
    // o initialize nao leva o cabecalho de versao: tira-o
    let (st, cab, corpo) = enviar(
        &app,
        bruta(
            &json!({"jsonrpc": "2.0", "id": 1, "method": "initialize",
                    "params": {"protocolVersion": LEGADA, "capabilities": {}, "clientInfo": {"name": "t", "version": "1"}}})
                .to_string(),
            &[("content-type", "application/json")],
        ),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let v = json_do_corpo(&corpo);
    assert_eq!(v["id"], 1);
    assert_eq!(v["result"]["protocolVersion"], LEGADA);
    assert!(v["result"]["capabilities"]["tools"].is_object());
    assert_eq!(v["result"]["serverInfo"]["name"], "rustnf");
    assert!(!cab.contains_key("mcp-session-id"));
    // com o cabecalho de versao tambem responde
    let (st, _, _) = enviar(&app, init).await;
    assert_eq!(st, StatusCode::OK);

    let (st, _, corpo) = enviar(
        &app,
        bruta(
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
            &[
                ("content-type", "application/json"),
                ("mcp-protocol-version", LEGADA),
            ],
        ),
    )
    .await;
    assert_eq!(st, StatusCode::ACCEPTED);
    assert!(corpo.is_empty());

    let (st, _, corpo) = enviar(
        &app,
        bruta(
            r#"{"jsonrpc":"2.0","id":2,"method":"initialize","params":{}}"#,
            &[],
        ),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(json_do_corpo(&corpo)["error"]["code"], -32602);
}

#[tokio::test]
async fn ping_na_revisao_anterior() {
    let (st, _, corpo) = enviar(&app_leitura(), legada("ping", json!({}), &[])).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(json_do_corpo(&corpo)["result"], json!({}));
}

#[tokio::test]
async fn server_discover_na_revisao_moderna() {
    let (st, _, corpo) = enviar(&app_leitura(), moderna("server/discover", json!({}), &[])).await;
    assert_eq!(st, StatusCode::OK);
    let r = &json_do_corpo(&corpo)["result"];
    assert_eq!(r["resultType"], "complete");
    assert_eq!(r["supportedVersions"], json!([MODERNA, LEGADA]));
    assert!(r["capabilities"]["tools"].is_object());
    assert_eq!(r["ttlMs"], 0);
    assert_eq!(r["cacheScope"], "public");
    assert_eq!(
        r["_meta"]["io.modelcontextprotocol/serverInfo"]["name"],
        "rustnf"
    );
}

#[tokio::test]
async fn tools_list_sem_emissao_so_tem_as_de_leitura() {
    let app = app_leitura();
    for req in [
        moderna("tools/list", json!({}), &[]),
        legada("tools/list", json!({}), &[]),
    ] {
        let (st, _, corpo) = enviar(&app, req).await;
        assert_eq!(st, StatusCode::OK);
        let v = json_do_corpo(&corpo);
        assert_eq!(
            nomes(&v),
            [
                "calcular_cbs_ibs",
                "conferir_dps",
                "montar_dps",
                "gerar_danfse"
            ]
        );
        for t in v["result"]["tools"].as_array().unwrap() {
            assert_eq!(t["inputSchema"]["type"], "object", "{}", t["name"]);
            assert!(t["description"].is_string());
        }
    }
    // a lista da revisao moderna leva as dicas de cache; "private" porque varia com a autorizacao
    let (_, _, corpo) = enviar(&app, moderna("tools/list", json!({}), &[])).await;
    let r = &json_do_corpo(&corpo)["result"];
    assert_eq!(r["resultType"], "complete");
    assert_eq!(r["ttlMs"], 0);
    assert_eq!(r["cacheScope"], "private");
}

#[tokio::test]
async fn tools_list_com_emissao_so_mostra_as_de_emissao_com_o_bearer() {
    let app = app_emissao(Falso::default());
    let (_, _, corpo) = enviar(&app, moderna("tools/list", json!({}), &[])).await;
    assert_eq!(nomes(&json_do_corpo(&corpo)).len(), 4);

    let a = auth();
    let (st, _, corpo) = enviar(
        &app,
        moderna("tools/list", json!({}), &[("authorization", a.as_str())]),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let n = nomes(&json_do_corpo(&corpo));
    assert_eq!(n.len(), 6);
    assert!(n.contains(&"emitir_nfse".to_string()) && n.contains(&"cancelar_nfse".to_string()));

    let (st, _, _) = enviar(
        &app,
        moderna(
            "tools/list",
            json!({}),
            &[("authorization", "Bearer errado-errado-errado-errado-errado")],
        ),
    )
    .await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn calcular_cbs_ibs_devolve_o_resultado_estruturado() {
    let previa = casos("reforma_previa.json");
    let args = json!({"tributos": previa["tributos"][0]["entrada"].clone()});
    let app = app_leitura();
    for req in [
        moderna(
            "tools/call",
            json!({"name": "calcular_cbs_ibs", "arguments": args}),
            &[],
        ),
        legada(
            "tools/call",
            json!({"name": "calcular_cbs_ibs", "arguments": args}),
            &[],
        ),
    ] {
        let (st, _, corpo) = enviar(&app, req).await;
        assert_eq!(st, StatusCode::OK);
        let r = &json_do_corpo(&corpo)["result"];
        assert_eq!(r["isError"], false, "{r}");
        assert_eq!(
            r["structuredContent"]["tributos"]["ibs"],
            previa["tributos"][0]["esperado"]["ibs"]
        );
        assert_eq!(r["content"][0]["type"], "text");
    }
}

#[tokio::test]
async fn entrada_invalida_de_ferramenta_e_resultado_com_iserror() {
    let (st, _, corpo) = enviar(
        &app_leitura(),
        moderna("tools/call", json!({"name": "calcular_cbs_ibs", "arguments": {"base": {"ano": 2026, "op_simples": 9}}}), &[]),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let v = json_do_corpo(&corpo);
    assert!(v.get("error").is_none());
    assert_eq!(v["result"]["isError"], true);
}

#[tokio::test]
async fn conferir_dps_com_recusa_volta_como_iserror() {
    let app = app_leitura();
    let (ruim, campo) = dps_recusada();
    let (st, _, corpo) = enviar(
        &app,
        moderna(
            "tools/call",
            json!({"name": "conferir_dps", "arguments": ruim}),
            &[],
        ),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let v = json_do_corpo(&corpo);
    assert!(
        v.get("error").is_none(),
        "regra fiscal nao e' erro de protocolo: {v}"
    );
    assert_eq!(v["result"]["isError"], true);
    assert!(v["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains(&campo));

    let (_, _, corpo) = enviar(
        &app,
        moderna(
            "tools/call",
            json!({"name": "conferir_dps", "arguments": dps_valida()}),
            &[],
        ),
    )
    .await;
    assert_eq!(json_do_corpo(&corpo)["result"]["isError"], false);
}

#[tokio::test]
async fn montar_dps_devolve_o_xml() {
    let (_, _, corpo) = enviar(
        &app_leitura(),
        legada(
            "tools/call",
            json!({"name": "montar_dps", "arguments": dps_valida()}),
            &[],
        ),
    )
    .await;
    let r = &json_do_corpo(&corpo)["result"];
    assert_eq!(r["isError"], false);
    assert_eq!(r["content"][0]["text"], xml_da_dps_valida());
}

#[tokio::test]
async fn gerar_danfse_devolve_o_pdf_em_base64_num_recurso() {
    let (st, _, corpo) = enviar(
        &app_leitura(),
        moderna("tools/call", json!({"name": "gerar_danfse", "arguments": {"xml": nfse_xml_para_danfse(), "canhoto": false}}), &[]),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let r = &json_do_corpo(&corpo)["result"];
    assert_eq!(r["isError"], false, "{r}");
    let recurso = &r["content"][1];
    assert_eq!(recurso["type"], "resource");
    assert_eq!(recurso["resource"]["mimeType"], "application/pdf");
    let pdf = base64::engine::general_purpose::STANDARD
        .decode(recurso["resource"]["blob"].as_str().unwrap())
        .unwrap();
    assert!(pdf.starts_with(b"%PDF-"));
}

#[tokio::test]
async fn ferramentas_de_emissao_so_executam_com_a_opcao_e_o_bearer() {
    // servidor sem emissao: a ferramenta nao existe
    let (st, _, corpo) = enviar(
        &app_leitura(),
        moderna(
            "tools/call",
            json!({"name": "emitir_nfse", "arguments": dps_valida()}),
            &[],
        ),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(json_do_corpo(&corpo)["error"]["code"], -32602);

    // com emissao e sem Bearer: 401
    let ch = chave();
    let falso = Falso::novo(vec![
        ("POST", "/nfse".into(), autorizada(&ch)),
        (
            "POST",
            "/eventos".into(),
            json_resp(
                201,
                json!({"eventoXmlGZipB64": rustnf::nfse::compactar("<evento/>")}),
            ),
        ),
    ]);
    let app = app_emissao(falso.clone());
    let (st, _, _) = enviar(
        &app,
        moderna(
            "tools/call",
            json!({"name": "emitir_nfse", "arguments": dps_valida()}),
            &[],
        ),
    )
    .await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);
    assert!(falso.chamadas().is_empty());

    // com o Bearer: emite
    let a = auth();
    let (st, _, corpo) = enviar(
        &app,
        moderna(
            "tools/call",
            json!({"name": "emitir_nfse", "arguments": dps_valida()}),
            &[("authorization", a.as_str())],
        ),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let r = &json_do_corpo(&corpo)["result"];
    assert_eq!(r["isError"], false, "{r}");
    assert_eq!(r["structuredContent"]["status"], "autorizada");
    assert_eq!(r["structuredContent"]["chave"], ch);

    let (_, _, corpo) = enviar(
        &app,
        moderna(
            "tools/call",
            json!({"name": "cancelar_nfse", "arguments": {"chave": ch, "motivo": "1", "justificativa": "Valor digitado errado na nota"}}),
            &[("authorization", a.as_str())],
        ),
    )
    .await;
    let r = &json_do_corpo(&corpo)["result"];
    assert_eq!(r["isError"], false, "{r}");
    assert_eq!(r["structuredContent"]["status"], "registrado");

    // recusa de regra na emissao: resultado com isError, nao erro de protocolo
    let (ruim, _) = dps_recusada();
    let (_, _, corpo) = enviar(
        &app,
        moderna(
            "tools/call",
            json!({"name": "emitir_nfse", "arguments": ruim}),
            &[("authorization", a.as_str())],
        ),
    )
    .await;
    assert_eq!(json_do_corpo(&corpo)["result"]["isError"], true);
}

#[tokio::test]
async fn ferramenta_desconhecida_e_erro_de_protocolo() {
    let (st, _, corpo) = enviar(
        &app_leitura(),
        moderna(
            "tools/call",
            json!({"name": "nao_existe", "arguments": {}}),
            &[],
        ),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(json_do_corpo(&corpo)["error"]["code"], -32602);
}

#[tokio::test]
async fn metodo_desconhecido_e_erro_json_rpc() {
    let app = app_leitura();
    let (st, _, corpo) = enviar(&app, moderna("resources/list", json!({}), &[])).await;
    assert_eq!(st, StatusCode::NOT_FOUND);
    let v = json_do_corpo(&corpo);
    assert_eq!(v["error"]["code"], -32601);
    assert_eq!(v["id"], 1);
    assert_eq!(v["jsonrpc"], "2.0");

    let (st, _, corpo) = enviar(&app, legada("logging/setLevel", json!({}), &[])).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(json_do_corpo(&corpo)["error"]["code"], -32601);
}

#[tokio::test]
async fn origin_invalido_e_403_com_erro_json_rpc_sem_id() {
    let (st, _, corpo) = enviar(
        &app_leitura(),
        moderna(
            "tools/list",
            json!({}),
            &[("origin", "https://site.exemplo.com.br")],
        ),
    )
    .await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    let v = json_do_corpo(&corpo);
    assert!(v["error"]["message"].is_string());
    assert!(v.get("id").is_none());
    // origem local passa
    let (st, _, _) = enviar(
        &app_leitura(),
        moderna(
            "tools/list",
            json!({}),
            &[("origin", "http://localhost:6274")],
        ),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
}

#[tokio::test]
async fn cabecalhos_que_nao_batem_com_o_corpo_sao_400_32020() {
    let app = app_leitura();
    // Mcp-Method diferente do corpo
    let mut req = moderna("tools/list", json!({}), &[]);
    req.headers_mut()
        .insert("mcp-method", "tools/call".parse().unwrap());
    let (st, _, corpo) = enviar(&app, req).await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    assert_eq!(json_do_corpo(&corpo)["error"]["code"], -32020);

    // sem MCP-Protocol-Version
    let mut req = moderna("tools/list", json!({}), &[]);
    req.headers_mut().remove("mcp-protocol-version");
    let (st, _, corpo) = enviar(&app, req).await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    assert_eq!(json_do_corpo(&corpo)["error"]["code"], -32020);

    // Mcp-Name diferente do params.name
    let mut req = moderna(
        "tools/call",
        json!({"name": "montar_dps", "arguments": {}}),
        &[],
    );
    req.headers_mut()
        .insert("mcp-name", "outra".parse().unwrap());
    let (st, _, corpo) = enviar(&app, req).await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    assert_eq!(json_do_corpo(&corpo)["error"]["code"], -32020);
}

#[tokio::test]
async fn mcp_name_em_base64_e_decodificado() {
    let codificado = format!(
        "=?base64?{}?=",
        base64::engine::general_purpose::STANDARD.encode("conferir_dps")
    );
    let mut req = moderna(
        "tools/call",
        json!({"name": "conferir_dps", "arguments": dps_valida()}),
        &[],
    );
    req.headers_mut()
        .insert("mcp-name", codificado.parse().unwrap());
    let (st, _, corpo) = enviar(&app_leitura(), req).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(json_do_corpo(&corpo)["result"]["isError"], false);
}

#[tokio::test]
async fn versao_nao_suportada_lista_as_aceitas() {
    let corpo = json!({"jsonrpc": "2.0", "id": 3, "method": "tools/list", "params": {"_meta": {
        "io.modelcontextprotocol/protocolVersion": "1900-01-01",
        "io.modelcontextprotocol/clientCapabilities": {}
    }}});
    let (st, _, resp) = enviar(
        &app_leitura(),
        bruta(
            &corpo.to_string(),
            &[
                ("content-type", "application/json"),
                ("mcp-protocol-version", "1900-01-01"),
                ("mcp-method", "tools/list"),
            ],
        ),
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    let v = json_do_corpo(&resp);
    assert_eq!(v["error"]["code"], -32022);
    assert_eq!(v["error"]["data"]["supported"], json!([MODERNA, LEGADA]));
    assert_eq!(v["error"]["data"]["requested"], "1900-01-01");

    // cabecalho de versao desconhecido, sem _meta: tambem nao e' aceito
    let (st, _, resp) = enviar(
        &app_leitura(),
        bruta(
            r#"{"jsonrpc":"2.0","id":4,"method":"tools/list"}"#,
            &[("mcp-protocol-version", "2024-11-05")],
        ),
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    assert_eq!(json_do_corpo(&resp)["error"]["code"], -32022);
}

#[tokio::test]
async fn meta_incompleto_e_400_32602() {
    let corpo = json!({"jsonrpc": "2.0", "id": 5, "method": "tools/list", "params": {"_meta": {
        "io.modelcontextprotocol/protocolVersion": MODERNA
    }}});
    let (st, _, resp) = enviar(
        &app_leitura(),
        bruta(
            &corpo.to_string(),
            &[
                ("mcp-protocol-version", MODERNA),
                ("mcp-method", "tools/list"),
            ],
        ),
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    assert_eq!(json_do_corpo(&resp)["error"]["code"], -32602);
}

#[tokio::test]
async fn corpo_malformado_ou_em_lote_e_recusado() {
    let app = app_leitura();
    let (st, _, corpo) = enviar(&app, bruta("{ isto nao e json", &[])).await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    let v = json_do_corpo(&corpo);
    assert_eq!(v["error"]["code"], -32700);
    assert!(v["id"].is_null());

    let (st, _, corpo) = enviar(
        &app,
        bruta(r#"[{"jsonrpc":"2.0","id":1,"method":"ping"}]"#, &[]),
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    assert_eq!(json_do_corpo(&corpo)["error"]["code"], -32600);

    let (st, _, _) = enviar(&app, bruta(r#"{"id":1,"method":"ping"}"#, &[])).await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn get_e_delete_no_endpoint_sao_405() {
    let app = app_leitura();
    for metodo in ["GET", "DELETE"] {
        let (st, _, _) = enviar(
            &app,
            requisicao(metodo, "/mcp", &[("accept", "text/event-stream")], vec![]),
        )
        .await;
        assert_eq!(st, StatusCode::METHOD_NOT_ALLOWED, "{metodo}");
    }
}

#[tokio::test]
async fn id_de_sessao_enviado_e_ignorado() {
    let (st, cab, _) = enviar(
        &app_leitura(),
        moderna("tools/list", json!({}), &[("mcp-session-id", "qualquer")]),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert!(!cab.contains_key("mcp-session-id"));
}
