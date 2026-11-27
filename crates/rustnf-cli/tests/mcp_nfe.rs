// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! As ferramentas MCP da NF-e (`POST /mcp`, revisao 2026-07-28), sobre o transporte falso da SEFAZ: `tools/list` com e sem
//! o Bearer, uma chamada por ferramenta e desfecho, recusa fiscal como `isError`, campo desconhecido recusado sem envio
//! e o `inputSchema` de cada ferramenta. Nenhum teste chama a SEFAZ.

mod comum;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use base64::Engine;
use comum::nfe_falso::*;
use comum::*;
use rustnf_cli::mcp::schemas_nfe::campos_do_pedido;
use serde_json::{json, Value};

const MODERNA: &str = "2026-07-28";
const OK: &str = "Autorizado o uso da NF-e";
const REGISTRADO: &str = "Evento registrado e vinculado a NF-e";

const LEITURA: [&str; 3] = ["conferir_nfe", "montar_nfe", "gerar_danfe"];
const EFEITO: [&str; 8] = [
    "emitir_nfe",
    "consultar_nfe",
    "cancelar_nfe",
    "corrigir_nfe",
    "inutilizar_nfe",
    "status_sefaz",
    "registrar_epec",
    "retomar_pendentes_nfe",
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

fn nomes(v: &Value) -> Vec<String> {
    v["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect()
}

/// `tools/call` com o Bearer; devolve o `result` (o protocolo sempre responde 200 quando a ferramenta existe).
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

#[tokio::test]
async fn tools_list_sem_bearer_so_mostra_a_leitura_e_com_bearer_mostra_tudo() {
    let p = Pasta::nova();
    let (app, _, _) = app_nfe(&p, vec![]);
    let (st, _, corpo) = enviar(&app, requisicao_mcp("tools/list", json!({}), &[])).await;
    assert_eq!(st, StatusCode::OK);
    let sem = nomes(&json_do_corpo(&corpo));
    for n in LEITURA {
        assert!(sem.iter().any(|x| x == n), "falta {n}");
    }
    for n in EFEITO {
        assert!(!sem.iter().any(|x| x == n), "{n} apareceu sem o Bearer");
    }

    let a = auth();
    let (_, _, corpo) = enviar(
        &app,
        requisicao_mcp("tools/list", json!({}), &[("authorization", a.as_str())]),
    )
    .await;
    let v = json_do_corpo(&corpo);
    let com = nomes(&v);
    for n in LEITURA.iter().chain(EFEITO.iter()) {
        assert!(com.iter().any(|x| x == n), "falta {n} com o Bearer");
    }
    // cada ferramenta tem inputSchema de objeto; as de efeito fiscal dizem isso na descricao
    for t in v["result"]["tools"].as_array().unwrap() {
        assert_eq!(t["inputSchema"]["type"], "object", "{}", t["name"]);
        let nome = t["name"].as_str().unwrap();
        if [
            "emitir_nfe",
            "cancelar_nfe",
            "corrigir_nfe",
            "inutilizar_nfe",
            "registrar_epec",
            "retomar_pendentes_nfe",
        ]
        .contains(&nome)
        {
            let d = t["description"].as_str().unwrap();
            assert!(d.contains("Operacao com efeito fiscal"), "{nome}: {d}");
        }
    }
}

#[tokio::test]
async fn sem_permitir_emissao_so_a_leitura_existe_e_a_de_efeito_e_desconhecida() {
    let p = Pasta::nova();
    let app = app_nfe_leitura(&p);
    let a = auth();
    let (_, _, corpo) = enviar(
        &app,
        requisicao_mcp("tools/list", json!({}), &[("authorization", a.as_str())]),
    )
    .await;
    let n = nomes(&json_do_corpo(&corpo));
    assert!(LEITURA.iter().all(|l| n.iter().any(|x| x == l)));
    assert!(EFEITO.iter().all(|e| !n.iter().any(|x| x == e)));
    let (st, _, corpo) = enviar(
        &app,
        requisicao_mcp(
            "tools/call",
            json!({"name": "emitir_nfe", "arguments": pedido_nfe()}),
            &[("authorization", a.as_str())],
        ),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(json_do_corpo(&corpo)["error"]["code"], -32602);
}

#[tokio::test]
async fn ferramenta_de_efeito_sem_bearer_e_401_e_nada_e_enviado() {
    let p = Pasta::nova();
    let (app, falso, notas) = app_nfe(&p, vec![("NFeAutorizacao4", envio("100", OK, NPROT))]);
    for nome in EFEITO {
        let (st, h, _) = enviar(
            &app,
            requisicao_mcp(
                "tools/call",
                json!({"name": nome, "arguments": pedido_nfe()}),
                &[],
            ),
        )
        .await;
        assert_eq!(st, StatusCode::UNAUTHORIZED, "{nome}");
        assert_eq!(h["www-authenticate"], "Bearer");
    }
    assert_eq!(falso.restantes(), 1);
    assert!(notas.listar().unwrap().is_empty());
}

#[tokio::test]
async fn conferir_e_montar_sao_leitura_e_recusa_e_is_error() {
    let p = Pasta::nova();
    let app = app_nfe_leitura(&p);
    let r = chamar(&app, "conferir_nfe", pedido_nfe()).await;
    assert_eq!(r["isError"], false, "{r}");
    assert_eq!(r["structuredContent"]["ok"], true);
    assert_eq!(r["structuredContent"]["chave"].as_str().unwrap().len(), 44);

    let mut ruim = pedido_nfe();
    ruim["itens"][0]["ncm"] = json!("1");
    let r = chamar(&app, "conferir_nfe", ruim).await;
    assert_eq!(r["isError"], true, "{r}");
    assert_eq!(r["structuredContent"]["ok"], false);
    assert!(r["structuredContent"]["erro"]["mensagem"]
        .as_str()
        .unwrap()
        .contains("NCM"));

    let r = chamar(&app, "montar_nfe", pedido_nfe()).await;
    assert_eq!(r["isError"], false, "{r}");
    let xml = r["structuredContent"]["xml"].as_str().unwrap();
    assert!(xml.contains("<NFe") && !xml.contains("<Signature"));
    assert_eq!(r["structuredContent"]["assinada"], false);
}

#[tokio::test]
async fn gerar_danfe_devolve_o_pdf_em_base64() {
    let p = Pasta::nova();
    let (app, _, notas) = app_nfe(&p, vec![("NFeAutorizacao4", envio("100", OK, NPROT))]);
    let r = chamar(&app, "emitir_nfe", pedido_nfe()).await;
    assert_eq!(r["isError"], false, "{r}");
    let chave = r["structuredContent"]["chave"].as_str().unwrap();
    let xml = notas.ler_xml(chave).unwrap();
    let r = chamar(&app, "gerar_danfe", json!({"xml": xml, "canhoto": false})).await;
    assert_eq!(r["isError"], false, "{r}");
    let recurso = &r["content"][1]["resource"];
    assert_eq!(recurso["mimeType"], "application/pdf");
    let pdf = base64::engine::general_purpose::STANDARD
        .decode(recurso["blob"].as_str().unwrap())
        .unwrap();
    assert!(pdf.starts_with(b"%PDF"));

    let r = chamar(&app, "gerar_danfe", json!({"xml": "<a/>"})).await;
    assert_eq!(r["isError"], true);
}

#[tokio::test]
async fn emitir_autorizada_rejeitada_e_pendente() {
    let p = Pasta::nova();
    let (app, falso, notas) = app_nfe(
        &p,
        vec![
            (
                "NFeAutorizacao4",
                envio("225", "Rejeicao: Falha no Schema XML do lote de NFe", ""),
            ),
            ("NFeAutorizacao4", envio("100", OK, NPROT)),
            ("NFeAutorizacao4", perdida()),
            ("NFeConsultaProtocolo4", perdida()),
        ],
    );
    // rejeitada: recusa fiscal e' isError com o JSON da SEFAZ, e o numero segue livre
    let r = chamar(&app, "emitir_nfe", pedido_nfe()).await;
    assert_eq!(r["isError"], true, "{r}");
    assert_eq!(r["structuredContent"]["cstat"], "225");
    assert!(notas.listar().unwrap().is_empty());

    let r = chamar(&app, "emitir_nfe", pedido_nfe()).await;
    assert_eq!(r["isError"], false, "{r}");
    assert_eq!(r["structuredContent"]["status"], "autorizada");
    assert_eq!(r["structuredContent"]["numero"], 1);

    let r = chamar(&app, "emitir_nfe", pedido_nfe()).await;
    assert_eq!(r["isError"], true, "{r}");
    assert_eq!(r["structuredContent"]["situacao"], "pendente");
    assert_eq!(notas.listar().unwrap().len(), 2);
    assert_eq!(falso.restantes(), 0);
}

#[tokio::test]
async fn campo_desconhecido_recusa_sem_envio_e_argumento_a_mais_tambem() {
    let p = Pasta::nova();
    let (app, falso, notas) = app_nfe(&p, vec![("NFeAutorizacao4", envio("100", OK, NPROT))]);
    let mut errado = pedido_nfe();
    errado["natop"] = json!("VENDA");
    errado["x_just"] = json!("Sem acesso a SEFAZ por queda do link de internet");
    for nome in ["emitir_nfe", "registrar_epec", "conferir_nfe", "montar_nfe"] {
        let r = chamar(&app, nome, errado.clone()).await;
        assert_eq!(r["isError"], true, "{nome}: {r}");
        assert!(
            r["content"][0]["text"].as_str().unwrap().contains("natop"),
            "{nome}: {r}"
        );
    }
    let r = chamar(
        &app,
        "cancelar_nfe",
        json!({"chave": "1", "justificativa": "x", "extra": 1}),
    )
    .await;
    assert_eq!(r["isError"], true, "{r}");
    let r = chamar(&app, "retomar_pendentes_nfe", json!({"extra": 1})).await;
    assert_eq!(r["isError"], true, "{r}");
    assert_eq!(falso.restantes(), 1, "nada foi enviado");
    assert!(notas.listar().unwrap().is_empty());
}

#[tokio::test]
async fn consultar_cancelar_corrigir_inutilizar_status_epec_e_retomar() {
    let p = Pasta::nova();
    let evt = |n: &'static str| ("NFeRecepcaoEvento4", evento("135", REGISTRADO, n));
    let (app, falso, notas) = app_nfe(
        &p,
        vec![
            ("NFeAutorizacao4", envio("100", OK, NPROT)),
            ("NFeConsultaProtocolo4", consulta("100", OK, NPROT)),
            evt("135260000000050"),
            evt("135260000000051"),
            ("NFeInutilizacao4", inutilizacao("135260000000300")),
            ("NFeStatusServico4", status("107", "Servico em Operacao")),
            (
                "NFeRecepcaoEvento4",
                evento("136", REGISTRADO, "135260000000070"),
            ),
        ],
    );
    let r = chamar(&app, "emitir_nfe", pedido_nfe()).await;
    let chave = r["structuredContent"]["chave"]
        .as_str()
        .unwrap()
        .to_string();

    let r = chamar(&app, "consultar_nfe", json!({"chave": chave})).await;
    assert_eq!(
        (
            r["isError"].as_bool(),
            r["structuredContent"]["status"].as_str()
        ),
        (Some(false), Some("autorizada"))
    );

    let r = chamar(
        &app,
        "corrigir_nfe",
        json!({"chave": chave, "texto": "Corrigir o complemento do endereco"}),
    )
    .await;
    assert_eq!(
        (
            r["isError"].as_bool(),
            r["structuredContent"]["sequencia"].as_u64()
        ),
        (Some(false), Some(1))
    );

    let r = chamar(
        &app,
        "cancelar_nfe",
        json!({"chave": chave, "justificativa": "Cancelamento por erro de digitacao"}),
    )
    .await;
    assert_eq!(
        (
            r["isError"].as_bool(),
            r["structuredContent"]["situacao"].as_str()
        ),
        (Some(false), Some("cancelada"))
    );

    let r = chamar(
        &app,
        "inutilizar_nfe",
        json!({"inicio": 2, "fim": 3, "justificativa": "Numeros pulados por falha do sistema"}),
    )
    .await;
    assert_eq!(r["isError"], false, "{r}");
    assert_eq!(notas.inutilizacoes().unwrap().len(), 1);

    let r = chamar(&app, "status_sefaz", json!({})).await;
    assert_eq!(
        (
            r["isError"].as_bool(),
            r["structuredContent"]["status"].as_str()
        ),
        (Some(false), Some("em_operacao"))
    );

    let mut epec = pedido_nfe();
    epec["x_just"] = json!("Sem acesso a SEFAZ por queda do link de internet");
    let r = chamar(&app, "registrar_epec", epec).await;
    assert_eq!(
        (
            r["isError"].as_bool(),
            r["structuredContent"]["situacao"].as_str()
        ),
        (Some(false), Some("epec")),
        "{r}"
    );

    assert_eq!(falso.restantes(), 0);
}

#[tokio::test]
async fn registrar_epec_com_substitui_declara_o_campo_e_liga_a_nota_sem_resposta() {
    let p = Pasta::nova();
    let (app, falso, notas) = app_nfe(
        &p,
        vec![
            ("NFeAutorizacao4", perdida()),
            ("NFeConsultaProtocolo4", perdida()),
        ],
    );
    // o schema da ferramenta declara `substitui`, com a descricao; as outras ferramentas do pedido nao
    let a = auth();
    let (_, _, corpo) = enviar(
        &app,
        requisicao_mcp("tools/list", json!({}), &[("authorization", a.as_str())]),
    )
    .await;
    let v = json_do_corpo(&corpo);
    let ferramenta = |nome: &str| {
        v["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["name"] == nome)
            .unwrap()
            .clone()
    };
    let d = ferramenta("registrar_epec")["inputSchema"]["properties"]["substitui"]["description"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(d.contains("SEM RESPOSTA") && d.contains("inutilize"), "{d}");
    assert!(ferramenta("emitir_nfe")["inputSchema"]["properties"]["substitui"].is_null());

    let r = chamar(&app, "emitir_nfe", pedido_nfe()).await;
    assert_eq!(r["isError"], true, "{r}");
    let pendente = r["structuredContent"]["chave"]
        .as_str()
        .unwrap()
        .to_string();

    let mut epec = pedido_nfe();
    epec["x_just"] = json!("Sem acesso a SEFAZ por queda do link de internet");
    epec["substitui"] = json!("123");
    let r = chamar(&app, "registrar_epec", epec.clone()).await;
    assert_eq!(r["isError"], true, "{r}");
    assert!(
        r["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("substitui"),
        "{r}"
    );
    assert_eq!(falso.restantes(), 0, "nada foi enviado");

    falso.empilhar(
        "NFeRecepcaoEvento4",
        evento(
            "136",
            "Evento registrado e vinculado a NF-e",
            "135260000000070",
        ),
    );
    epec["substitui"] = json!(pendente);
    let r = chamar(&app, "registrar_epec", epec).await;
    assert_eq!(r["isError"], false, "{r}");
    assert_eq!(r["structuredContent"]["substitui"], pendente.as_str());
    assert_eq!(notas.registro(&pendente).unwrap().substituida_por.len(), 44);
}

#[tokio::test]
async fn retomar_pendentes_fecha_a_mesma_chave() {
    let p = Pasta::nova();
    let (app, falso, notas) = app_nfe(
        &p,
        vec![
            ("NFeAutorizacao4", perdida()),
            ("NFeConsultaProtocolo4", perdida()),
        ],
    );
    let r = chamar(&app, "emitir_nfe", pedido_nfe()).await;
    assert_eq!(r["isError"], true, "{r}");
    let chave = r["structuredContent"]["chave"]
        .as_str()
        .unwrap()
        .to_string();
    falso.empilhar("NFeConsultaProtocolo4", consulta("100", OK, NPROT));
    // a retomada so' consulta nota com mais de 1 segundo
    tokio::time::sleep(std::time::Duration::from_millis(2100)).await;
    let r = chamar(&app, "retomar_pendentes_nfe", json!({})).await;
    assert_eq!(r["isError"], false, "{r}");
    assert_eq!(r["structuredContent"]["notas"][0]["chave"], chave.as_str());
    assert_eq!(r["structuredContent"]["notas"][0]["situacao"], "autorizada");
    assert_eq!(notas.listar().unwrap().len(), 1);
}

#[tokio::test]
async fn nfe_sem_configuracao_e_resultado_com_erro_em_palavras() {
    let app = app_nfe_sem_configuracao();
    let r = chamar(&app, "status_sefaz", json!({})).await;
    assert_eq!(r["isError"], true, "{r}");
    let texto = r["content"][0]["text"].as_str().unwrap();
    assert!(
        texto.contains("[nfe]") && texto.contains("[notas]"),
        "{texto}"
    );
}

#[test]
fn os_campos_do_schema_do_pedido_existem_no_leiaute() {
    // um campo do schema que o leiaute nao conhece seria recusado: confere cada um
    let mut pedido = serde_json::Map::new();
    for c in campos_do_pedido() {
        pedido.insert(c.to_string(), json!(1));
    }
    pedido.insert("emit".into(), json!({}));
    let desconhecidos = rustnf::nfe::montagem::campos_desconhecidos(&Value::Object(pedido));
    assert!(
        desconhecidos.is_empty(),
        "o schema declara campos que o leiaute nao conhece: {desconhecidos:?}"
    );
}
