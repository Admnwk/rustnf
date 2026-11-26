// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! As rotas `/nfe/recebidas/` em memoria, sobre o transporte falso e uma pasta temporaria: o que exige o Bearer e a
//! emissao, a sincronizacao, o bloqueio 656, a lista, o cursor e a manifestacao. Nenhum teste chama o Ambiente Nacional.

mod comum;

use axum::http::StatusCode;
use comum::dfe_falso::*;
use comum::nfe_falso::*;
use comum::*;
use serde_json::{json, Value};

const REGISTRADO: &str = "Evento registrado e vinculado a NF-e";

fn get(rota: &str, token: bool) -> axum::http::Request<axum::body::Body> {
    let auth = format!("Bearer {TOKEN}");
    let cab: Vec<(&str, &str)> = if token {
        vec![("authorization", auth.as_str())]
    } else {
        vec![]
    };
    requisicao("GET", rota, &cab, vec![])
}

fn post_vazio(rota: &str) -> axum::http::Request<axum::body::Body> {
    let auth = format!("Bearer {TOKEN}");
    requisicao("POST", rota, &[("authorization", auth.as_str())], vec![])
}

#[tokio::test]
async fn sem_emissao_as_rotas_nao_existem_e_sem_o_bearer_sao_401() {
    let p = Pasta::nova();
    let leitura = app_nfe_leitura(&p);
    for (metodo, rota) in [
        ("GET", "/nfe/recebidas"),
        ("GET", "/nfe/recebidas/cursor"),
        ("POST", "/nfe/recebidas/sincronizar"),
        ("POST", "/nfe/recebidas/baixar"),
        ("POST", "/nfe/recebidas/manifestar"),
    ] {
        let (st, _, _) = enviar(&leitura, requisicao(metodo, rota, &[], vec![])).await;
        assert_eq!(st, StatusCode::NOT_FOUND, "{metodo} {rota}");
    }
    let (app, falso, _) = app_nfe(&p, vec![]);
    for (metodo, rota) in [
        ("GET", "/nfe/recebidas"),
        ("GET", "/nfe/recebidas/cursor"),
        ("POST", "/nfe/recebidas/sincronizar"),
        ("POST", "/nfe/recebidas/baixar"),
        ("POST", "/nfe/recebidas/manifestar"),
    ] {
        let (st, _, _) = enviar(&app, requisicao(metodo, rota, &[], vec![])).await;
        assert_eq!(st, StatusCode::UNAUTHORIZED, "{metodo} {rota}");
    }
    assert_eq!(falso.restantes(), 0);
}

#[tokio::test]
async fn sincronizar_sem_corpo_baixa_lista_e_mostra_o_cursor() {
    let p = Pasta::nova();
    let (proc, chave) = nota_recebida(&config_nfe(&p), 1);
    let (app, falso, _) = app_nfe(
        &p,
        vec![(
            DIST,
            lote(2, 2, &[doc_resumo(1, &chave), doc_completa(2, &proc)]),
        )],
    );
    let (st, _, corpo) = enviar(&app, post_vazio("/nfe/recebidas/sincronizar")).await;
    let v = json_do_corpo(&corpo);
    assert_eq!(st, StatusCode::OK, "{v}");
    assert_eq!(
        (v["status"].as_str(), v["documentos"].as_u64()),
        (Some("em_dia"), Some(2))
    );
    assert_eq!(v["tipos"]["resumos"], 1);
    assert_eq!(v["tipos"]["notas"], 1);
    assert_eq!(falso.restantes(), 0);

    let (st, _, corpo) = enviar(&app, get("/nfe/recebidas", true)).await;
    let v = json_do_corpo(&corpo);
    assert_eq!(st, StatusCode::OK, "{v}");
    assert_eq!(v["notas"].as_array().unwrap().len(), 1);
    assert_eq!(v["notas"][0]["situacao"], "completa");
    assert_eq!(v["notas"][0]["chave"], chave);

    let (_, _, corpo) = enviar(&app, get("/nfe/recebidas?situacao=resumo", true)).await;
    assert!(json_do_corpo(&corpo)["notas"]
        .as_array()
        .unwrap()
        .is_empty());
    let (_, _, corpo) = enviar(&app, get("/nfe/recebidas?busca=loja%20de+teste", true)).await;
    assert_eq!(json_do_corpo(&corpo)["notas"].as_array().unwrap().len(), 1);
    let (st, _, corpo) = enviar(&app, get("/nfe/recebidas?situacao=nada", true)).await;
    assert_eq!(
        st,
        StatusCode::BAD_REQUEST,
        "{}",
        String::from_utf8_lossy(&corpo)
    );

    let (st, _, corpo) = enviar(&app, get("/nfe/recebidas/cursor", true)).await;
    let v = json_do_corpo(&corpo);
    assert_eq!(st, StatusCode::OK, "{v}");
    assert_eq!(v["ult_nsu"], "000000000000002");
    assert!(v["espera_ate"].is_string());
}

#[tokio::test]
async fn o_656_volta_422_com_o_resultado_e_o_corpo_com_campo_desconhecido_nao_sai() {
    let p = Pasta::nova();
    let (app, falso, _) = app_nfe(&p, vec![(DIST, bloqueio(0))]);
    let (st, _, corpo) = enviar(&app, post_vazio("/nfe/recebidas/sincronizar")).await;
    let v = json_do_corpo(&corpo);
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{v}");
    assert_eq!(v["status"], "consumo_indevido");
    assert_eq!(v["cstat"], "656");
    assert_eq!(v["erro"]["codigo"], "recusa");
    assert_eq!(falso.restantes(), 0);

    // campo que o pedido nao conhece: recusado antes de qualquer envio (o roteiro esta vazio e derrubaria o teste)
    let (st, _, _) = enviar(
        &app,
        post_json_token("/nfe/recebidas/sincronizar", &json!({"lote": 3})),
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    let (st, _, _) = enviar(
        &app,
        post_json_token(
            "/nfe/recebidas/manifestar",
            &json!({"chave": "1", "tipo": "talvez"}),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn manifestar_e_baixar_pela_rota() {
    let p = Pasta::nova();
    let (proc, chave) = nota_recebida(&config_nfe(&p), 1);
    let (app, _, _) = app_nfe(
        &p,
        vec![
            (DIST, lote(1, 1, &[doc_resumo(1, &chave)])),
            ("NFeRecepcaoEvento4", evento("135", REGISTRADO, NPROT)),
            (DIST, lote(2, 2, &[doc_completa(2, &proc)])),
        ],
    );
    enviar(&app, post_vazio("/nfe/recebidas/sincronizar")).await;

    let (st, _, corpo) = enviar(
        &app,
        post_json_token(
            "/nfe/recebidas/manifestar",
            &json!({"chave": chave, "tipo": "ciencia"}),
        ),
    )
    .await;
    let v = json_do_corpo(&corpo);
    assert_eq!(st, StatusCode::OK, "{v}");
    assert_eq!(
        (v["status"].as_str(), v["tipo"].as_str()),
        (Some("registrado"), Some("210210"))
    );
    assert_eq!(v["situacao"], "ciencia");

    // a nota ja tem ciencia: so' a consulta traz a nota completa (sem segunda ciencia)
    let (st, _, corpo) = enviar(
        &app,
        post_json_token(
            "/nfe/recebidas/baixar",
            &json!({"chave": chave, "ciencia": true}),
        ),
    )
    .await;
    let v: Value = json_do_corpo(&corpo);
    assert_eq!(st, StatusCode::OK, "{v}");
    assert_eq!(v["completa"], true);
    assert!(v.get("ciencia").is_none(), "{v}");
}
