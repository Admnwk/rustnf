// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! As ferramentas MCP dos documentos recebidos (`POST /mcp`), sobre o transporte falso: quais aparecem com e sem o Bearer,
//! uma chamada por ferramenta, o bloqueio 656 como `isError` e o campo desconhecido recusado sem envio. Nenhum teste chama
//! o Ambiente Nacional.

mod comum;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use comum::dfe_falso::*;
use comum::nfe_falso::*;
use comum::*;
use serde_json::{json, Value};

const MODERNA: &str = "2026-07-28";
const REGISTRADO: &str = "Evento registrado e vinculado a NF-e";
const NOVAS: [&str; 4] = [
    "sincronizar_recebidas",
    "listar_recebidas",
    "baixar_recebida",
    "manifestar_recebida",
];

fn requisicao_mcp(metodo: &str, mut params: Value, extra: &[(&str, &str)]) -> Request<Body> {
    params["_meta"] = json!({
        "io.modelcontextprotocol/protocolVersion": MODERNA,
        "io.modelcontextprotocol/clientInfo": {"name": "TesteCliente", "version": "1.0.0"},
        "io.modelcontextprotocol/clientCapabilities": {}
    });
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

fn auth() -> String {
    format!("Bearer {TOKEN}")
}

/// `tools/call` com o Bearer; devolve o `result`.
async fn chamar(app: &axum::Router, nome: &str, argumentos: Value) -> Value {
    let a = auth();
    let (st, _, corpo) = enviar(
        app,
        requisicao_mcp(
            "tools/call",
            json!({"name": nome, "arguments": argumentos}),
            &[("authorization", a.as_str())],
        ),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&corpo));
    json_do_corpo(&corpo)["result"].clone()
}

fn estruturado(r: &Value) -> Value {
    r["structuredContent"].clone()
}

#[tokio::test]
async fn as_quatro_ferramentas_so_aparecem_com_a_emissao_e_o_bearer() {
    let p = Pasta::nova();
    let (app, _, _) = app_nfe(&p, vec![]);
    let (_, _, corpo) = enviar(&app, requisicao_mcp("tools/list", json!({}), &[])).await;
    let sem = json_do_corpo(&corpo)["result"]["tools"].clone();
    for n in NOVAS {
        assert!(
            !sem.as_array().unwrap().iter().any(|t| t["name"] == n),
            "{n} sem o Bearer"
        );
    }
    let a = auth();
    let (_, _, corpo) = enviar(
        &app,
        requisicao_mcp("tools/list", json!({}), &[("authorization", a.as_str())]),
    )
    .await;
    let com = json_do_corpo(&corpo)["result"]["tools"].clone();
    for n in NOVAS {
        let t = com
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["name"] == n)
            .unwrap_or_else(|| panic!("falta {n}"));
        assert_eq!(t["inputSchema"]["type"], "object");
        assert_eq!(t["inputSchema"]["additionalProperties"], false);
    }
    // a manifestacao se descreve como o efeito fiscal que e'
    let d = |n: &str| {
        com.as_array()
            .unwrap()
            .iter()
            .find(|t| t["name"] == n)
            .unwrap()["description"]
            .as_str()
            .unwrap()
            .to_string()
    };
    assert!(d("manifestar_recebida").contains("Operacao com efeito fiscal"));
    assert!(d("manifestar_recebida").contains("EVENTO FISCAL"));
    assert!(d("listar_recebidas").contains("nao usa a rede"));

    // sem --permitir-emissao nenhuma delas existe
    let leitura = app_nfe_leitura(&p);
    let (_, _, corpo) = enviar(
        &leitura,
        requisicao_mcp("tools/list", json!({}), &[("authorization", a.as_str())]),
    )
    .await;
    let n = json_do_corpo(&corpo)["result"]["tools"].clone();
    for nome in NOVAS {
        assert!(
            !n.as_array().unwrap().iter().any(|t| t["name"] == nome),
            "{nome}"
        );
    }
}

#[tokio::test]
async fn sincronizar_listar_manifestar_e_baixar_pelo_mcp() {
    let p = Pasta::nova();
    let (proc, chave) = nota_recebida(&config_nfe(&p), 1);
    let (app, falso, _) = app_nfe(
        &p,
        vec![
            (DIST, lote(1, 1, &[doc_resumo(1, &chave)])),
            ("NFeRecepcaoEvento4", evento("135", REGISTRADO, NPROT)),
            (DIST, lote(2, 2, &[doc_completa(2, &proc)])),
        ],
    );
    let r = chamar(&app, "sincronizar_recebidas", json!({})).await;
    assert_eq!(r["isError"], false, "{r}");
    assert_eq!(estruturado(&r)["status"], "em_dia");
    assert_eq!(estruturado(&r)["tipos"]["resumos"], 1);

    let r = chamar(&app, "listar_recebidas", json!({"situacao": "resumo"})).await;
    assert_eq!(r["isError"], false, "{r}");
    assert_eq!(estruturado(&r)["notas"][0]["chave"], chave);

    let r = chamar(
        &app,
        "manifestar_recebida",
        json!({"chave": chave, "tipo": "ciencia"}),
    )
    .await;
    assert_eq!(r["isError"], false, "{r}");
    assert_eq!(estruturado(&r)["situacao"], "ciencia");

    let r = chamar(&app, "baixar_recebida", json!({"chave": chave})).await;
    assert_eq!(r["isError"], false, "{r}");
    assert_eq!(estruturado(&r)["completa"], true);
    assert_eq!(falso.restantes(), 0);
}

#[tokio::test]
async fn o_656_e_o_campo_desconhecido_voltam_como_iserror_sem_enviar() {
    let p = Pasta::nova();
    let (app, falso, _) = app_nfe(&p, vec![(DIST, bloqueio(0))]);
    let r = chamar(&app, "sincronizar_recebidas", json!({})).await;
    assert_eq!(r["isError"], true, "{r}");
    assert_eq!(estruturado(&r)["status"], "consumo_indevido");
    assert_eq!(falso.restantes(), 0);

    for (nome, args) in [
        ("sincronizar_recebidas", json!({"lote": 1})),
        ("baixar_recebida", json!({"chave": "1", "ciencias": true})),
        (
            "manifestar_recebida",
            json!({"chave": "1", "tipo": "talvez"}),
        ),
        ("listar_recebidas", json!({"situacao": "nada"})),
    ] {
        let r = chamar(&app, nome, args).await;
        assert_eq!(r["isError"], true, "{nome}: {r}");
    }
}
