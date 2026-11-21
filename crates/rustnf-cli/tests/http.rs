// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! API HTTP em memoria (`tower::ServiceExt::oneshot`): cada rota, teto de corpo, tempo limite, rotas de emissao
//! ausentes sem a opcao, Bearer, envelope de erro e ausencia de CORS. Nada abre porta nem fala com a rede.

mod comum;

use std::time::Duration;

use axum::http::StatusCode;
use comum::*;
use rustnf_app::notas::Notas;
use rustnf_cli::http::Config;
use serde_json::{json, Value};

#[tokio::test]
async fn health_diz_se_a_emissao_esta_ligada() {
    let (st, _, corpo) = enviar(&app_leitura(), requisicao("GET", "/health", &[], vec![])).await;
    assert_eq!(st, StatusCode::OK);
    let v = json_do_corpo(&corpo);
    assert_eq!(v["status"], "ok");
    assert_eq!(v["emissao"], false);
    let (_, _, corpo) = enviar(
        &app_emissao(Falso::default()),
        requisicao("GET", "/health", &[], vec![]),
    )
    .await;
    assert_eq!(json_do_corpo(&corpo)["emissao"], true);
}

#[tokio::test]
async fn calcular_base_tributos_e_os_dois_juntos() {
    let app = app_leitura();
    let previa = casos("reforma_previa.json");
    let base = previa["bases"][0]["entrada"].clone();
    let (st, _, corpo) = enviar(&app, post_json("/calcular", &json!({"base": base}))).await;
    assert_eq!(st, StatusCode::OK);
    let v = json_do_corpo(&corpo);
    assert_eq!(
        v["bases"]["regime_regular"],
        previa["bases"][0]["regime_regular"]
    );
    assert_eq!(v["recusado"], false);

    let tributos = previa["tributos"][0]["entrada"].clone();
    let (st, _, corpo) = enviar(&app, post_json("/calcular", &json!({"tributos": tributos}))).await;
    assert_eq!(st, StatusCode::OK);
    let v = json_do_corpo(&corpo);
    assert_eq!(
        v["tributos"]["ibs"],
        previa["tributos"][0]["esperado"]["ibs"]
    );

    // os dois juntos, sem tributos.base: vale a base do regime regular calculada
    let (st, _, corpo) = enviar(
        &app,
        post_json(
            "/calcular",
            &json!({"base": base, "tributos": {"data": "2026-10-20", "cst": "000", "cclass_trib": "000001"}}),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&corpo));
    let v = json_do_corpo(&corpo);
    assert_eq!(v["tributos"]["base"], v["bases"]["regime_regular"]);
}

#[tokio::test]
async fn calcular_recusa_entrada_ruim() {
    let app = app_leitura();
    let (st, _, corpo) = enviar(&app, post_json("/calcular", &json!({}))).await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    assert_eq!(json_do_corpo(&corpo)["erro"]["codigo"], "entrada_invalida");
    let (st, _, _) = enviar(
        &app,
        post_json(
            "/calcular",
            &json!({"base": {"ano": 2026, "op_simples": 9}}),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    let (st, _, corpo) = enviar(
        &app,
        post_json(
            "/calcular",
            &json!({"tributos": {"data": "2026-10-20", "cst": "000", "cclass_trib": "000001"}}),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(json_do_corpo(&corpo)["erro"]["campo"], "tributos.base");
}

#[tokio::test]
async fn dps_conferir_e_montar() {
    let app = app_leitura();
    let (st, _, corpo) = enviar(&app, post_json("/dps/conferir", &dps_valida())).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(json_do_corpo(&corpo), json!({"ok": true}));

    let (st, _, corpo) = enviar(&app, post_json("/dps/montar", &dps_valida())).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(json_do_corpo(&corpo)["xml"], xml_da_dps_valida());

    let (ruim, campo) = dps_recusada();
    let (st, _, corpo) = enviar(&app, post_json("/dps/conferir", &ruim)).await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    let v = json_do_corpo(&corpo);
    assert_eq!(v["erro"]["codigo"], "recusa");
    assert_eq!(v["erro"]["campo"], campo);
    let (st, _, _) = enviar(&app, post_json("/dps/montar", &ruim)).await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn danfse_devolve_pdf() {
    let app = app_leitura();
    let req = requisicao(
        "POST",
        "/danfse?situacao=cancelada&canhoto=false",
        &[("content-type", "application/xml")],
        nfse_xml_para_danfse().into_bytes(),
    );
    let (st, cab, corpo) = enviar(&app, req).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(cab["content-type"], "application/pdf");
    assert!(corpo.starts_with(b"%PDF-"));

    let (st, _, corpo) = enviar(
        &app,
        requisicao("POST", "/danfse", &[], b"<nao-e-nfse/>".to_vec()),
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    assert_eq!(json_do_corpo(&corpo)["erro"]["codigo"], "entrada_invalida");
    let (st, _, _) = enviar(
        &app,
        requisicao("POST", "/danfse?situacao=outra", &[], b"<x/>".to_vec()),
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn corpo_acima_do_teto_e_413_com_envelope() {
    let config = Config {
        limite_corpo: 100,
        ..Config::default()
    };
    let app = app_leitura_com(config);
    let grande = vec![b' '; 500];
    let (st, _, corpo) = enviar(&app, requisicao("POST", "/calcular", &[], grande.clone())).await;
    assert_eq!(st, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(json_do_corpo(&corpo)["erro"]["codigo"], "corpo_grande");
    // com o Content-Length declarado, a recusa vem antes de ler o corpo
    let (st, _, _) = enviar(
        &app,
        requisicao("POST", "/calcular", &[("content-length", "500")], grande),
    )
    .await;
    assert_eq!(st, StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn sem_a_opcao_as_rotas_de_emissao_nem_existem() {
    let app = app_leitura();
    let chave = chave();
    let auth = format!("Bearer {TOKEN}");
    for (metodo, rota) in [
        ("POST", "/emitir".to_string()),
        ("POST", "/cancelar".to_string()),
        ("GET", format!("/nfse/{chave}")),
    ] {
        for cab in [vec![], vec![("authorization", auth.as_str())]] {
            let (st, _, corpo) =
                enviar(&app, requisicao(metodo, &rota, &cab, b"{}".to_vec())).await;
            assert_eq!(st, StatusCode::NOT_FOUND, "{metodo} {rota}");
            assert_eq!(json_do_corpo(&corpo)["erro"]["codigo"], "nao_encontrada");
        }
    }
}

#[tokio::test]
async fn emissao_exige_bearer() {
    let app = app_emissao(Falso::default());
    let chave = chave();
    let errado = "Bearer errado-errado-errado-errado-errado-errado";
    let rotas = [
        ("POST", "/emitir".to_string()),
        ("POST", "/cancelar".to_string()),
        ("GET", format!("/nfse/{chave}")),
    ];
    for (metodo, rota) in &rotas {
        let (st, cab, corpo) = enviar(&app, requisicao(metodo, rota, &[], b"{}".to_vec())).await;
        assert_eq!(st, StatusCode::UNAUTHORIZED, "sem token: {rota}");
        assert_eq!(cab["www-authenticate"], "Bearer");
        assert_eq!(json_do_corpo(&corpo)["erro"]["codigo"], "nao_autorizado");
        let (st, _, _) = enviar(
            &app,
            requisicao(metodo, rota, &[("authorization", errado)], b"{}".to_vec()),
        )
        .await;
        assert_eq!(st, StatusCode::UNAUTHORIZED, "token errado: {rota}");
    }
}

#[tokio::test]
async fn emitir_consultar_e_cancelar_com_transporte_falso() {
    let ch = chave();
    let falso = Falso::novo(vec![
        ("POST", "/nfse".into(), autorizada(&ch)),
        ("GET", format!("/nfse/{ch}"), nota(&ch)),
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

    let (st, _, corpo) = enviar(&app, post_json_token("/emitir", &dps_valida())).await;
    assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&corpo));
    let v = json_do_corpo(&corpo);
    assert_eq!(v["status"], "autorizada");
    assert_eq!(v["chave"], ch);
    assert_eq!(v["nfse"]["numero"], "77");
    assert!(!String::from_utf8_lossy(&corpo).contains("Signature"));

    let auth = format!("Bearer {TOKEN}");
    let (st, _, corpo) = enviar(
        &app,
        requisicao(
            "GET",
            &format!("/nfse/{ch}"),
            &[("authorization", auth.as_str())],
            vec![],
        ),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(json_do_corpo(&corpo)["chave"], ch);

    let (st, _, corpo) = enviar(
        &app,
        post_json_token(
            "/cancelar",
            &json!({"chave": ch, "motivo": "1", "justificativa": "Valor digitado errado na nota"}),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&corpo));
    assert_eq!(json_do_corpo(&corpo)["status"], "registrado");
    assert_eq!(falso.chamadas().len(), 3);
}

#[tokio::test]
async fn emitir_e_cancelar_usam_o_emitente_e_a_pasta_de_notas_da_configuracao() {
    let p = Pasta::nova();
    let pasta = p.caminho("notas");
    let notas = Notas::abrir(&pasta).unwrap();
    // a pasta ja tem as notas ate a 41: a proxima DPS e' a 42
    notas.confirmar_numero("1", 41).unwrap();
    let (ch, autorizada) = autorizada_completa(42);
    let falso = Falso::novo(vec![
        ("POST", "/nfse".into(), autorizada),
        ("POST", "/eventos".into(), evento_registrado()),
    ]);
    let app = app_emissao_com_pasta(falso.clone(), &pasta);

    let (st, _, corpo) = enviar(&app, post_json_token("/emitir", &pedido_sem_emitente())).await;
    assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&corpo));
    let v = json_do_corpo(&corpo);
    assert_eq!(v["status"], "autorizada", "{v}");
    assert_eq!(v["guardada"], true, "{v}");
    assert_eq!(v["numero_dps"], 42, "{v}");

    // o que foi enviado a Sefin tem os dados do emitente e o numero da pasta
    let xml = dps_enviada(&falso, 0);
    for trecho in [
        "<CNPJ>11222333000181</CNPJ>",
        "<nDPS>42</nDPS>",
        "<cLocEmi>3550308</cLocEmi>",
        "<IM>123456</IM>",
    ] {
        assert!(xml.contains(trecho), "faltou {trecho} em {xml}");
    }

    // a nota ficou na pasta, e a numeracao avancou
    assert!(notas.ler_xml(&ch).is_ok());
    assert_eq!(notas.proximo_numero("1").unwrap(), 43);
    assert_eq!(notas.listar().unwrap()[0].situacao.texto(), "autorizada");

    // o cancelamento registrado marca a nota na pasta
    let (st, _, corpo) = enviar(
        &app,
        post_json_token(
            "/cancelar",
            &json!({"chave": ch, "motivo": "1", "justificativa": "Valor digitado errado na nota"}),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&corpo));
    assert_eq!(json_do_corpo(&corpo)["status"], "registrado");
    assert_eq!(notas.listar().unwrap()[0].situacao.texto(), "cancelada");
}

#[tokio::test]
async fn duas_emissoes_ao_mesmo_tempo_nao_pegam_o_mesmo_numero() {
    let p = Pasta::nova();
    let pasta = p.caminho("notas");
    let notas = Notas::abrir(&pasta).unwrap();
    notas.confirmar_numero("1", 41).unwrap();
    let (_, r1) = autorizada_completa(42);
    let (_, r2) = autorizada_completa(43);
    let falso = Falso::novo(vec![
        ("POST", "/nfse".into(), r1),
        ("POST", "/nfse".into(), r2),
    ])
    .com_atraso(Duration::from_millis(50));
    let app = app_emissao_com_pasta(falso.clone(), &pasta);

    let (a, b) = tokio::join!(
        enviar(&app, post_json_token("/emitir", &pedido_sem_emitente())),
        enviar(&app, post_json_token("/emitir", &pedido_sem_emitente())),
    );
    let mut numeros = Vec::new();
    for (st, _, corpo) in [a, b] {
        assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&corpo));
        numeros.push(json_do_corpo(&corpo)["numero_dps"].as_u64().unwrap());
    }
    numeros.sort_unstable();
    assert_eq!(numeros, [42, 43]);
    assert!(dps_enviada(&falso, 0).contains("<nDPS>42</nDPS>"));
    assert!(dps_enviada(&falso, 1).contains("<nDPS>43</nDPS>"));
    assert_eq!(notas.proximo_numero("1").unwrap(), 44);
}

#[tokio::test]
async fn sem_emitente_e_sem_pasta_o_emitir_segue_como_era() {
    // servidor sem `[emitente]` e sem pasta de notas: o pedido completo emite e a resposta nao fala em pasta
    let ch = chave();
    let app = app_emissao(Falso::novo(vec![("POST", "/nfse".into(), autorizada(&ch))]));
    let (st, _, corpo) = enviar(&app, post_json_token("/emitir", &dps_valida())).await;
    assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&corpo));
    let v = json_do_corpo(&corpo);
    assert!(
        v.get("guardada").is_none() && v.get("numero_dps").is_none(),
        "{v}"
    );

    // e o pedido sem numero continua recusado pela montagem, sem chegar a rede
    let falso = Falso::default();
    let app = app_emissao(falso.clone());
    let mut pedido = dps_valida();
    pedido.as_object_mut().unwrap().remove("numero");
    let (st, _, _) = enviar(&app, post_json_token("/emitir", &pedido)).await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(falso.chamadas().is_empty());
}

/// O pedido valido com um campo de nome errado na raiz e outro dentro de `valores`.
fn pedido_com_nome_errado() -> Value {
    let mut v = dps_valida();
    v["descontos"] = json!("10.00");
    v["valores"]["desconto_incondicional"] = json!("5.00");
    v
}

#[tokio::test]
async fn campo_desconhecido_volta_na_resposta_e_e_recusado_com_estrito() {
    let app = app_leitura();
    let pedido = pedido_com_nome_errado();
    let esperado = json!(["descontos", "valores.desconto_incondicional"]);

    let (st, _, corpo) = enviar(&app, post_json("/dps/conferir", &pedido)).await;
    assert_eq!(st, StatusCode::OK);
    let v = json_do_corpo(&corpo);
    assert_eq!(v["ok"], true);
    assert_eq!(v["campos_desconhecidos"], esperado);
    let (st, _, corpo) = enviar(&app, post_json("/dps/montar", &pedido)).await;
    assert_eq!(st, StatusCode::OK);
    let v = json_do_corpo(&corpo);
    assert_eq!(v["xml"], xml_da_dps_valida());
    assert_eq!(v["campos_desconhecidos"], esperado);

    // pedido limpo: a resposta e' a de sempre
    let (_, _, corpo) = enviar(&app, post_json("/dps/conferir", &dps_valida())).await;
    assert_eq!(json_do_corpo(&corpo), json!({"ok": true}));

    // ?estrito=true: 422 com o campo
    for rota in ["/dps/conferir?estrito=true", "/dps/montar?estrito=true"] {
        let (st, _, corpo) = enviar(&app, post_json(rota, &pedido)).await;
        assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{rota}");
        assert_eq!(
            json_do_corpo(&corpo)["erro"]["campo"],
            "descontos",
            "{rota}"
        );
    }
    let (st, _, _) = enviar(&app, post_json("/dps/conferir?estrito=true", &dps_valida())).await;
    assert_eq!(st, StatusCode::OK);
    let (st, _, _) = enviar(&app, post_json("/dps/conferir?estrito=talvez", &pedido)).await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn emitir_recusa_o_campo_desconhecido_e_com_estrito_false_emite_e_avisa() {
    let ch = chave();
    let falso = Falso::novo(vec![("POST", "/nfse".into(), autorizada(&ch))]);
    let app = app_emissao(falso.clone());
    let pedido = pedido_com_nome_errado();

    // por padrao (e com ?estrito=true): 422 e nada sai para a rede
    for rota in ["/emitir", "/emitir?estrito=true"] {
        let (st, _, corpo) = enviar(&app, post_json_token(rota, &pedido)).await;
        assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{rota}");
        assert_eq!(json_do_corpo(&corpo)["erro"]["campo"], "descontos");
    }
    assert!(falso.chamadas().is_empty());

    // com ?estrito=false: emite, e a resposta diz o que foi ignorado
    let (st, _, corpo) = enviar(&app, post_json_token("/emitir?estrito=false", &pedido)).await;
    assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&corpo));
    let v = json_do_corpo(&corpo);
    assert_eq!(v["status"], "autorizada");
    assert_eq!(
        v["campos_desconhecidos"],
        json!(["descontos", "valores.desconto_incondicional"])
    );
}

#[tokio::test]
async fn calcular_usa_as_aliquotas_da_configuracao() {
    let pedido = json!({"tributos": {"data": "2027-06-01", "cst": "000", "cclass_trib": "000001", "base": "100.00"}});
    // sem aliquotas na configuracao, a previa de 2027 recusa: falta o numero publicado da CBS
    let (st, _, _) = enviar(&app_leitura(), post_json("/calcular", &pedido)).await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    // com as do arquivo (cbs_referencia 8,8): 8,8 - 0,1 = 8,7
    let (st, _, corpo) = enviar(
        &app_leitura_com_aliquotas(),
        post_json("/calcular", &pedido),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&corpo));
    assert_eq!(json_do_corpo(&corpo)["tributos"]["cbs"]["aliquota"], "8.70");
}

#[tokio::test]
async fn recusa_de_regra_nao_chega_a_sefin() {
    let falso = Falso::default();
    let app = app_emissao(falso.clone());
    let (ruim, campo) = dps_recusada();
    let (st, _, corpo) = enviar(&app, post_json_token("/emitir", &ruim)).await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(json_do_corpo(&corpo)["erro"]["campo"], campo);
    assert!(falso.chamadas().is_empty());
}

#[tokio::test]
async fn rejeicao_da_sefin_vira_422_sem_xml_assinado() {
    let falso = Falso::novo(vec![(
        "POST",
        "/nfse".into(),
        json_resp(
            400,
            json!({"erros": [{"codigo": "E0310", "descricao": "cTribNac"}]}),
        ),
    )]);
    let app = app_emissao(falso);
    let (st, _, corpo) = enviar(&app, post_json_token("/emitir", &dps_valida())).await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    let texto = String::from_utf8_lossy(&corpo).to_string();
    assert!(texto.contains("E0310"), "{texto}");
    assert!(
        !texto.contains("Signature") && !texto.contains("<DPS"),
        "{texto}"
    );
}

#[tokio::test]
async fn falha_de_rede_vira_502_e_nao_encontrada_404() {
    let falso = Falso::novo(vec![(
        "POST",
        "/nfse".into(),
        Resp::F(rustnf::nfse::sefin::FalhaRede::Rede("sem conexao".into())),
    )]);
    let app = app_emissao(falso);
    let (st, _, corpo) = enviar(&app, post_json_token("/emitir", &dps_valida())).await;
    assert!(st == StatusCode::BAD_GATEWAY, "{st}");
    assert_eq!(json_do_corpo(&corpo)["erro"]["codigo"], "sefin");

    let auth = format!("Bearer {TOKEN}");
    let (st, _, _) = enviar(
        &app,
        requisicao(
            "GET",
            &format!("/nfse/{}", chave()),
            &[("authorization", auth.as_str())],
            vec![],
        ),
    )
    .await;
    assert_eq!(st, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn tempo_limite_vira_504() {
    let falso = Falso::default().com_atraso(Duration::from_millis(400));
    let config = Config {
        tempo_limite: Duration::from_millis(50),
        ..Config::default()
    };
    let app = app_emissao_com(falso, config);
    let (st, _, corpo) = enviar(&app, post_json_token("/emitir", &dps_valida())).await;
    assert_eq!(st, StatusCode::GATEWAY_TIMEOUT);
    assert_eq!(json_do_corpo(&corpo)["erro"]["codigo"], "tempo_esgotado");
}

#[tokio::test]
async fn nunca_ha_cabecalho_cors() {
    let app = app_emissao(Falso::default());
    let pedidos = [
        requisicao(
            "GET",
            "/health",
            &[("origin", "http://localhost:3000")],
            vec![],
        ),
        requisicao(
            "OPTIONS",
            "/calcular",
            &[
                ("origin", "http://localhost:3000"),
                ("access-control-request-method", "POST"),
            ],
            vec![],
        ),
        post_json("/calcular", &json!({})),
        requisicao("GET", "/nao-existe", &[], vec![]),
    ];
    for req in pedidos {
        let (_, cab, _) = enviar(&app, req).await;
        assert!(
            cab.keys()
                .all(|k| !k.as_str().starts_with("access-control-")),
            "cabecalhos: {cab:?}"
        );
    }
}

#[tokio::test]
async fn origin_de_fora_e_403_em_toda_rota() {
    let app = app_leitura();
    for rota in ["/health", "/calcular"] {
        let (st, _, corpo) = enviar(
            &app,
            requisicao(
                "POST",
                rota,
                &[("origin", "https://site.exemplo.com.br")],
                b"{}".to_vec(),
            ),
        )
        .await;
        assert_eq!(st, StatusCode::FORBIDDEN, "{rota}");
        assert_eq!(json_do_corpo(&corpo)["erro"]["codigo"], "origem_invalida");
    }
}

#[tokio::test]
async fn erro_de_rota_e_de_metodo_tem_o_envelope_unico() {
    let app = app_leitura();
    let (st, _, corpo) = enviar(&app, requisicao("GET", "/nao-existe", &[], vec![])).await;
    assert_eq!(st, StatusCode::NOT_FOUND);
    let v: Value = json_do_corpo(&corpo);
    assert!(v["erro"]["codigo"].is_string() && v["erro"]["mensagem"].is_string());
    let (st, _, corpo) = enviar(&app, requisicao("GET", "/calcular", &[], vec![])).await;
    assert_eq!(st, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(
        json_do_corpo(&corpo)["erro"]["codigo"],
        "metodo_nao_permitido"
    );
}

#[tokio::test]
async fn json_invalido_nao_ecoa_caminho_nem_segredo() {
    let app = app_leitura();
    let (st, _, corpo) = enviar(
        &app,
        requisicao("POST", "/dps/conferir", &[], b"{ nao e json".to_vec()),
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    let texto = String::from_utf8_lossy(&corpo).to_lowercase();
    assert!(
        !texto.contains("\\") && !texto.contains("c:/") && !texto.contains("senha"),
        "{texto}"
    );
}
