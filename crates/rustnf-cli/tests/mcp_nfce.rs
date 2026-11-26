// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! As ferramentas MCP da NFC-e (`POST /mcp`, revisao 2026-07-28), sobre o transporte falso da SEFAZ: `tools/list` com e sem o
//! Bearer, uma chamada por ferramenta e desfecho, recusa fiscal como `isError`, campo desconhecido e CSC recusados sem envio,
//! o CSC que nunca aparece e o `inputSchema` de cada ferramenta. Nenhum teste chama a SEFAZ.

mod comum;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use base64::Engine;
use comum::nfce_falso::*;
use comum::nfe_falso::*;
use comum::*;
use serde_json::{json, Value};

const MODERNA: &str = "2026-07-28";
const OK: &str = "Autorizado o uso da NF-e";
const REGISTRADO: &str = "Evento registrado e vinculado a NF-e";

const LEITURA: [&str; 1] = ["gerar_danfce"];
const EFEITO: [&str; 6] = [
    "emitir_nfce",
    "consultar_nfce",
    "cancelar_nfce",
    "status_sefaz_nfce",
    "pendentes_nfce",
    "retomar_pendentes_nfce",
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

fn blob(r: &Value) -> Vec<u8> {
    base64::engine::general_purpose::STANDARD
        .decode(r["content"][1]["resource"]["blob"].as_str().unwrap())
        .unwrap()
}

#[tokio::test]
async fn tools_list_sem_bearer_so_mostra_a_leitura_e_com_bearer_mostra_tudo() {
    let p = Pasta::nova();
    let (app, _, _) = app_nfce(&p, vec![]);
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
    for t in v["result"]["tools"].as_array().unwrap() {
        assert_eq!(t["inputSchema"]["type"], "object", "{}", t["name"]);
        let nome = t["name"].as_str().unwrap();
        // as de efeito fiscal dizem isso na descricao
        if ["emitir_nfce", "cancelar_nfce", "retomar_pendentes_nfce"].contains(&nome) {
            let d = t["description"].as_str().unwrap();
            assert!(d.contains("Operacao com efeito fiscal"), "{nome}: {d}");
        }
    }
}

#[tokio::test]
async fn sem_permitir_emissao_so_a_leitura_existe_e_a_de_efeito_e_desconhecida() {
    let p = Pasta::nova();
    let app = app_nfce_leitura(&p);
    let (_, _, corpo) = enviar(&app, requisicao_mcp("tools/list", json!({}), &[])).await;
    let lista = nomes(&json_do_corpo(&corpo));
    assert!(lista.iter().any(|n| n == "gerar_danfce"));
    assert!(!lista
        .iter()
        .any(|n| n.ends_with("_nfce") && n != "gerar_danfce"));
    let r = enviar(
        &app,
        requisicao_mcp(
            "tools/call",
            json!({"name": "emitir_nfce", "arguments": pedido_nfce()}),
            &[],
        ),
    )
    .await;
    assert!(
        String::from_utf8_lossy(&r.2).contains("Unknown tool"),
        "{}",
        String::from_utf8_lossy(&r.2)
    );
}

#[tokio::test]
async fn ferramenta_de_efeito_sem_o_bearer_e_401_e_nada_e_enviado() {
    let p = Pasta::nova();
    let (app, falso, notas) = app_nfce(&p, vec![("NFeAutorizacao4", envio("100", OK, NPROT))]);
    for nome in EFEITO {
        let req = requisicao_mcp(
            "tools/call",
            json!({"name": nome, "arguments": pedido_nfce()}),
            &[],
        );
        let (st, h, _) = enviar(&app, req).await;
        assert_eq!(st, StatusCode::UNAUTHORIZED, "{nome}");
        assert_eq!(h["www-authenticate"], "Bearer");
    }
    assert_eq!(falso.restantes(), 1, "nada foi enviado");
    assert!(notas.listar().unwrap().is_empty());
}

#[tokio::test]
async fn gerar_danfce_devolve_o_pdf_e_o_escpos_em_base64() {
    let p = Pasta::nova();
    let (app, _, notas) = app_nfce(&p, vec![("NFeAutorizacao4", envio("100", OK, NPROT))]);
    let r = chamar(&app, "emitir_nfce", pedido_nfce()).await;
    assert_eq!(r["isError"], false, "{r}");
    let xml = notas
        .ler_xml(r["structuredContent"]["chave"].as_str().unwrap())
        .unwrap();
    let r = chamar(&app, "gerar_danfce", json!({"xml": xml})).await;
    assert_eq!(r["isError"], false, "{r}");
    assert_eq!(r["content"][1]["resource"]["mimeType"], "application/pdf");
    assert!(blob(&r).starts_with(b"%PDF"));
    let r = chamar(
        &app,
        "gerar_danfce",
        json!({"xml": xml, "formato": "escpos", "largura": 58, "colunas": 32}),
    )
    .await;
    assert_eq!(r["isError"], false, "{r}");
    assert_eq!(
        r["content"][1]["resource"]["mimeType"],
        "application/octet-stream"
    );
    assert!(blob(&r).starts_with(&[0x1b, 0x40]));

    for ruim in [
        json!({"xml": "<a/>"}),
        json!({"xml": xml, "formato": "png"}),
        json!({"xml": xml, "largura": 70}),
        json!({"xml": xml, "csc": "x"}),
    ] {
        let r = chamar(&app, "gerar_danfce", ruim).await;
        assert_eq!(r["isError"], true, "{r}");
    }
}

#[tokio::test]
async fn emitir_autorizada_rejeitada_e_sem_resposta() {
    let p = Pasta::nova();
    let (app, falso, notas) = app_nfce(
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
    let r = chamar(&app, "emitir_nfce", pedido_nfce()).await;
    assert_eq!(r["isError"], true, "{r}");
    assert_eq!(r["structuredContent"]["cstat"], "225");
    assert!(notas.listar().unwrap().is_empty());

    let r = chamar(&app, "emitir_nfce", pedido_nfce()).await;
    assert_eq!(r["isError"], false, "{r}");
    assert_eq!(r["structuredContent"]["status"], "autorizada");
    assert_eq!(r["structuredContent"]["numero"], 1);
    assert_eq!(
        &r["structuredContent"]["chave"].as_str().unwrap()[20..22],
        "65"
    );

    // sem resposta: isError, a chave volta, a nota fica pendente
    let r = chamar(&app, "emitir_nfce", pedido_nfce()).await;
    assert_eq!(r["isError"], true, "{r}");
    assert_eq!(r["structuredContent"]["situacao"], "pendente");
    assert_eq!(r["structuredContent"]["chave"].as_str().unwrap().len(), 44);
    assert_eq!(notas.listar().unwrap().len(), 2);
    assert_eq!(falso.restantes(), 0);
}

#[tokio::test]
async fn off_line_pendentes_e_retomada_da_mesma_chave() {
    let p = Pasta::nova();
    let (app, falso, notas) = app_nfce(&p, vec![]);
    let mut pedido = pedido_nfce();
    pedido["offline"] = json!(true);
    let r = chamar(&app, "emitir_nfce", pedido).await;
    assert_eq!(r["isError"], false, "{r}");
    assert_eq!(r["structuredContent"]["status"], "offline_pendente");
    assert_eq!(falso.restantes(), 0, "off-line nao fala com a SEFAZ");
    let chave = r["structuredContent"]["chave"]
        .as_str()
        .unwrap()
        .to_string();

    let r = chamar(&app, "pendentes_nfce", json!({})).await;
    assert_eq!(r["isError"], false, "{r}");
    let s = &r["structuredContent"];
    assert_eq!(
        (&s["notas"][0]["chave"], &s["pode_fechar_caixa"]),
        (&json!(chave), &json!(false))
    );
    assert!(s["aviso"].as_str().unwrap().contains("--retomar"), "{r}");

    falso.empilhar(
        "NFeConsultaProtocolo4",
        consulta(
            "217",
            "Rejeicao: NF-e nao consta na base de dados da SEFAZ",
            "",
        ),
    );
    falso.empilhar("NFeAutorizacao4", envio("100", OK, NPROT));
    tokio::time::sleep(std::time::Duration::from_millis(2100)).await;
    let r = chamar(&app, "retomar_pendentes_nfce", json!({})).await;
    assert_eq!(r["isError"], false, "{r}");
    assert_eq!(
        r["structuredContent"]["retomadas"][0]["chave"],
        chave.as_str()
    );
    assert_eq!(r["structuredContent"]["pode_fechar_caixa"], true);
    assert_eq!(
        notas.registro(&chave).unwrap().situacao.texto(),
        "autorizada"
    );
    assert_eq!(notas.listar().unwrap().len(), 1);
    assert_eq!(falso.restantes(), 0);
}

#[tokio::test]
async fn consultar_cancelar_com_aviso_de_prazo_e_status() {
    let p = Pasta::nova();
    let (app, falso, notas) = app_nfce(
        &p,
        vec![
            ("NFeAutorizacao4", perdida()),
            ("NFeConsultaProtocolo4", perdida()),
            ("NFeConsultaProtocolo4", consulta("100", OK, NPROT)),
            (
                "NFeRecepcaoEvento4",
                evento("135", REGISTRADO, "135260000000099"),
            ),
            ("NFeStatusServico4", status("107", "Servico em Operacao")),
            (
                "NFeStatusServico4",
                status("108", "Servico Paralisado Momentaneamente"),
            ),
        ],
    );
    let r = chamar(&app, "emitir_nfce", pedido_nfce()).await;
    let chave = r["structuredContent"]["chave"]
        .as_str()
        .unwrap()
        .to_string();

    let r = chamar(&app, "consultar_nfce", json!({"chave": chave})).await;
    assert_eq!(r["isError"], false, "{r}");
    assert_eq!(r["structuredContent"]["status"], "autorizada");

    let r = chamar(
        &app,
        "cancelar_nfce",
        json!({"chave": chave, "justificativa": "Cupom emitido em duplicidade"}),
    )
    .await;
    assert_eq!(r["isError"], false, "{r}");
    assert_eq!(r["structuredContent"]["situacao"], "cancelada");
    assert!(
        r["structuredContent"]["aviso_prazo"]
            .as_str()
            .unwrap()
            .contains("24 horas"),
        "{r}"
    );
    assert_eq!(
        notas.registro(&chave).unwrap().situacao.texto(),
        "cancelada"
    );

    let r = chamar(&app, "status_sefaz_nfce", json!({})).await;
    assert_eq!(
        (&r["isError"], &r["structuredContent"]["status"]),
        (&json!(false), &json!("em_operacao"))
    );
    let r = chamar(&app, "status_sefaz_nfce", json!({})).await;
    assert_eq!(
        (&r["isError"], &r["structuredContent"]["status"]),
        (&json!(true), &json!("paralisado"))
    );
    assert_eq!(falso.restantes(), 0);
}

#[tokio::test]
async fn campo_desconhecido_e_csc_recusam_sem_envio_e_argumento_a_mais_tambem() {
    let p = Pasta::nova();
    let (app, falso, notas) = app_nfce(&p, vec![("NFeAutorizacao4", envio("100", OK, NPROT))]);
    let mut errado = pedido_nfce();
    errado["natop"] = json!("VENDA");
    let r = chamar(&app, "emitir_nfce", errado).await;
    assert_eq!(r["isError"], true, "{r}");
    assert!(
        r["content"][0]["text"].as_str().unwrap().contains("natop"),
        "{r}"
    );

    for campo in ["csc", "id_csc", "idCSC"] {
        let mut com_csc = pedido_nfce();
        com_csc[campo] = json!(CSC_TESTE);
        let r = chamar(&app, "emitir_nfce", com_csc).await;
        let texto = r.to_string();
        assert_eq!(r["isError"], true, "{campo}: {texto}");
        assert!(
            texto.contains("nfce csc guardar") && !texto.contains(CSC_TESTE),
            "{texto}"
        );
    }
    for (nome, args) in [
        ("consultar_nfce", json!({"chave": "1", "csc": CSC_TESTE})),
        (
            "cancelar_nfce",
            json!({"chave": "1", "justificativa": "x", "csc": CSC_TESTE}),
        ),
        ("status_sefaz_nfce", json!({"csc": CSC_TESTE})),
        ("pendentes_nfce", json!({"csc": CSC_TESTE})),
        ("retomar_pendentes_nfce", json!({"csc": CSC_TESTE})),
    ] {
        let r = chamar(&app, nome, args).await;
        assert_eq!(r["isError"], true, "{nome}: {r}");
        assert!(!r.to_string().contains(CSC_TESTE), "{nome}");
    }
    assert_eq!(falso.restantes(), 1, "nada foi enviado");
    assert!(notas.listar().unwrap().is_empty());
    assert_eq!(notas.proximo_numero("1").unwrap(), 1);
}

#[tokio::test]
async fn o_csc_nunca_aparece_na_resposta_no_schema_nem_no_que_o_servidor_grava() {
    let p = Pasta::nova();
    let (app, _, notas) = app_nfce_com_csc(&p, vec![("NFeAutorizacao4", envio("100", OK, NPROT))]);
    let mut todas = String::new();
    let r = chamar(&app, "emitir_nfce", pedido_nfce()).await;
    assert_eq!(r["isError"], false, "{r}");
    todas += &r.to_string();
    let mut off = pedido_nfce();
    off["offline"] = json!(true);
    todas += &chamar(&app, "emitir_nfce", off).await.to_string();
    todas += &chamar(&app, "pendentes_nfce", json!({})).await.to_string();
    for n in notas.listar().unwrap() {
        let xml = notas.ler_xml(&n.chave).unwrap();
        todas += &chamar(&app, "gerar_danfce", json!({"xml": xml}))
            .await
            .to_string();
    }
    let a = auth();
    let (_, _, lista) = enviar(
        &app,
        requisicao_mcp("tools/list", json!({}), &[("authorization", a.as_str())]),
    )
    .await;
    let lista = String::from_utf8_lossy(&lista).into_owned();
    // nenhuma ferramenta tem campo de CSC no schema
    for t in json_do_corpo(lista.as_bytes())["result"]["tools"]
        .as_array()
        .unwrap()
    {
        if t["name"].as_str().unwrap().ends_with("nfce") {
            let campos = t["inputSchema"]["properties"].as_object().unwrap();
            assert!(
                !campos.keys().any(|k| k.to_lowercase().contains("csc")),
                "{}: {campos:?}",
                t["name"]
            );
        }
    }
    todas += &lista;
    assert!(!todas.contains(CSC_TESTE), "o CSC vazou numa resposta");
    assert!(
        !tudo_que_foi_gravado(&p).contains(CSC_TESTE),
        "o CSC foi gravado na pasta"
    );
}

#[tokio::test]
async fn duas_emissoes_mcp_ao_mesmo_tempo_nao_gastam_o_mesmo_numero() {
    let p = Pasta::nova();
    let (app, falso, notas) = app_nfce(
        &p,
        vec![
            ("NFeAutorizacao4", envio("100", OK, NPROT)),
            ("NFeAutorizacao4", envio("100", OK, "135260000000002")),
        ],
    );
    let (a, b) = tokio::join!(
        chamar(&app, "emitir_nfce", pedido_nfce()),
        chamar(&app, "emitir_nfce", pedido_nfce()),
    );
    let mut numeros = [
        a["structuredContent"]["numero"].as_u64().unwrap(),
        b["structuredContent"]["numero"].as_u64().unwrap(),
    ];
    numeros.sort_unstable();
    assert_eq!(numeros, [1, 2]);
    assert_eq!(notas.listar().unwrap().len(), 2);
    assert_eq!(falso.restantes(), 0);
}

#[tokio::test]
async fn nfce_sem_configuracao_e_resultado_com_erro_em_palavras() {
    let app = app_nfce_sem_configuracao();
    for (nome, args) in [
        ("status_sefaz_nfce", json!({})),
        ("emitir_nfce", pedido_nfce()),
        ("retomar_pendentes_nfce", json!({})),
    ] {
        let r = chamar(&app, nome, args).await;
        assert_eq!(r["isError"], true, "{nome}: {r}");
        let texto = r["content"][0]["text"].as_str().unwrap();
        assert!(
            texto.contains("NFC-e") && texto.contains("[nfe]"),
            "{nome}: {texto}"
        );
    }
    // a leitura segue no ar
    let r = chamar(&app, "gerar_danfce", json!({"xml": "<a/>"})).await;
    assert_eq!(r["isError"], true);
    assert!(r["content"][0]["text"].is_string());
}
