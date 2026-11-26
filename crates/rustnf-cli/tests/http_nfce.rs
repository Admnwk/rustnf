// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! As rotas `/nfce/` em memoria, sobre o transporte falso da SEFAZ e uma pasta temporaria: um teste por rota e desfecho
//! (autorizada, rejeitada, sem resposta e retomada, off-line e transmissao, cancelamento com aviso de prazo), os 404 sem
//! `--permitir-emissao`, os 401 sem o Bearer, o campo desconhecido e o CSC recusados sem envio, o CSC que nunca aparece,
//! as emissoes paralelas e a NFC-e sem configuracao utilizavel. Nenhum teste chama a SEFAZ.

mod comum;

use axum::http::StatusCode;
use comum::nfce_falso::*;
use comum::nfe_falso::*;
use comum::*;
use serde_json::{json, Value};

const OK: &str = "Autorizado o uso da NF-e";
const REGISTRADO: &str = "Evento registrado e vinculado a NF-e";
const CHAVE: &str = "35260111222333000181650010000000011234567890";

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

async fn emitir_com(app: &axum::Router, rota: &str, corpo: &Value) -> (StatusCode, Value) {
    let (st, _, resp) = enviar(app, post_json_token(rota, corpo)).await;
    (st, json_do_corpo(&resp))
}

async fn emitir_uma(app: &axum::Router) -> Value {
    let (st, v) = emitir_com(app, "/nfce/emitir", &pedido_nfce()).await;
    assert_eq!(st, StatusCode::OK, "{v}");
    v
}

#[tokio::test]
async fn health_diz_se_a_nfce_esta_pronta_e_se_o_csc_foi_lido_sem_mostra_lo() {
    let p = Pasta::nova();
    let (_, _, corpo) = enviar(&app_nfce_leitura(&p), get("/health", false)).await;
    let v = json_do_corpo(&corpo);
    assert_eq!(
        (v["status"].as_str(), v["emissao"].as_bool()),
        (Some("ok"), Some(false))
    );
    assert_eq!(
        (&v["nfce"], &v["nfce_csc_guardado"]),
        (&json!(false), &json!(false))
    );
    let (app, _, _) = app_nfce(&p, vec![]);
    let (_, _, corpo) = enviar(&app, get("/health", false)).await;
    let v = json_do_corpo(&corpo);
    assert_eq!(
        (&v["nfce"], &v["nfce_csc_guardado"]),
        (&json!(true), &json!(false))
    );
    let (app, _, _) = app_nfce_com_csc(&p, vec![]);
    let (_, _, corpo) = enviar(&app, get("/health", false)).await;
    let v = json_do_corpo(&corpo);
    assert_eq!(
        (&v["nfce"], &v["nfce_csc_guardado"]),
        (&json!(true), &json!(true))
    );
    assert!(!String::from_utf8_lossy(&corpo).contains(CSC_TESTE));
    let (_, _, corpo) = enviar(&app_nfce_sem_configuracao(), get("/health", false)).await;
    assert_eq!(json_do_corpo(&corpo)["nfce"], false);
}

#[tokio::test]
async fn emitir_autorizada_guarda_na_pasta_da_nfce_e_o_danfce_sai_em_pdf_e_em_escpos() {
    let p = Pasta::nova();
    let (app, falso, notas) = app_nfce(&p, vec![("NFeAutorizacao4", envio("100", OK, NPROT))]);
    let v = emitir_uma(&app).await;
    assert_eq!(
        (
            v["status"].as_str(),
            v["situacao"].as_str(),
            v["offline"].as_bool()
        ),
        (Some("autorizada"), Some("autorizada"), Some(false))
    );
    assert_eq!(v["protocolo"], NPROT);
    assert_eq!(&v["chave"].as_str().unwrap()[20..22], "65");
    assert_eq!(falso.restantes(), 0);
    let lista = notas.listar().unwrap();
    assert_eq!(lista.len(), 1);
    assert!(p.caminho("notas/nfce").exists() && !p.caminho("notas/nfe").exists());

    // a leitura nao precisa do Bearer nem da rede
    let xml = notas.ler_xml(&lista[0].chave).unwrap();
    let req = requisicao("POST", "/nfce/danfce", &[], xml.clone().into_bytes());
    let (st, cab, corpo) = enviar(&app, req).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(cab["content-type"], "application/pdf");
    assert!(corpo.starts_with(b"%PDF"), "nao e' um PDF");
    let rota = "/nfce/danfce?formato=escpos&largura=58&colunas=32";
    let (st, cab, corpo) = enviar(&app, requisicao("POST", rota, &[], xml.into_bytes())).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(cab["content-type"], "application/octet-stream");
    assert!(corpo.starts_with(&[0x1b, 0x40]), "ESC @ no comeco");
}

#[tokio::test]
async fn danfce_com_pedido_torto_e_400_sem_derrubar_o_servidor() {
    let p = Pasta::nova();
    let app = app_nfce_leitura(&p);
    for (rota, corpo) in [
        ("/nfce/danfce", &b"<a/>"[..]),
        ("/nfce/danfce?formato=png", b"<a/>"),
        ("/nfce/danfce?largura=70", b"<a/>"),
        ("/nfce/danfce?colunas=abc", b"<a/>"),
        ("/nfce/danfce?resumido=talvez", b"<a/>"),
    ] {
        let (st, _, resp) = enviar(&app, requisicao("POST", rota, &[], corpo.to_vec())).await;
        assert_eq!(
            st,
            StatusCode::BAD_REQUEST,
            "{rota}: {}",
            String::from_utf8_lossy(&resp)
        );
        assert!(json_do_corpo(&resp)["erro"]["mensagem"].is_string());
    }
}

#[tokio::test]
async fn emitir_rejeitada_e_422_com_o_cstat_e_o_numero_continua_livre() {
    let p = Pasta::nova();
    let (app, _, notas) = app_nfce(
        &p,
        vec![(
            "NFeAutorizacao4",
            envio("225", "Rejeicao: Falha no Schema XML do lote de NFe", ""),
        )],
    );
    let (st, v) = emitir_com(&app, "/nfce/emitir", &pedido_nfce()).await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{v}");
    assert_eq!(
        (v["status"].as_str(), v["cstat"].as_str()),
        (Some("rejeitada"), Some("225"))
    );
    assert_eq!(v["erro"]["codigo"], "recusa");
    assert!(notas.listar().unwrap().is_empty());
    assert_eq!(notas.proximo_numero("1").unwrap(), 1);
}

#[tokio::test]
async fn sem_resposta_e_502_a_nota_fica_pendente_e_a_retomada_fecha_a_mesma_chave() {
    let p = Pasta::nova();
    let (app, falso, notas) = app_nfce(
        &p,
        vec![
            ("NFeAutorizacao4", perdida()),
            ("NFeConsultaProtocolo4", perdida()),
        ],
    );
    let (st, v) = emitir_com(&app, "/nfce/emitir", &pedido_nfce()).await;
    assert_eq!(st, StatusCode::BAD_GATEWAY, "{v}");
    // a chave volta junto do erro: a pendente nunca se perde
    assert_eq!(
        (v["situacao"].as_str(), v["erro"]["codigo"].as_str()),
        (Some("pendente"), Some("sefin"))
    );
    let chave = v["chave"].as_str().unwrap().to_string();

    let (st, _, corpo) = enviar(&app, get("/nfce/pendentes", true)).await;
    let lista = json_do_corpo(&corpo);
    assert_eq!(st, StatusCode::OK, "{lista}");
    assert_eq!(lista["notas"][0]["chave"], chave.as_str());
    assert_eq!(lista["pode_fechar_caixa"], false);
    assert!(
        lista["aviso"]
            .as_str()
            .unwrap()
            .contains("pendentes --retomar"),
        "{lista}"
    );

    falso.empilhar("NFeConsultaProtocolo4", consulta("100", OK, NPROT));
    // a retomada so' consulta nota com mais de 1 segundo
    tokio::time::sleep(std::time::Duration::from_millis(2100)).await;
    let (st, _, corpo) = enviar(&app, post_vazio("/nfce/pendentes/retomar")).await;
    let v = json_do_corpo(&corpo);
    assert_eq!(st, StatusCode::OK, "{v}");
    assert_eq!(v["retomadas"][0]["chave"], chave.as_str());
    assert_eq!(v["retomadas"][0]["situacao"], "autorizada");
    assert_eq!(v["pode_fechar_caixa"], true);
    assert_eq!(notas.listar().unwrap().len(), 1);
    let (_, _, corpo) = enviar(&app, get("/nfce/pendentes", true)).await;
    assert_eq!(json_do_corpo(&corpo)["pode_fechar_caixa"], true);
}

#[tokio::test]
async fn off_line_grava_pendente_sem_rede_e_a_retomada_transmite_a_mesma_chave() {
    let p = Pasta::nova();
    let (app, falso, notas) = app_nfce(&p, vec![]);
    let mut pedido = pedido_nfce();
    pedido["offline"] = json!(true);
    let (st, v) = emitir_com(&app, "/nfce/emitir", &pedido).await;
    assert_eq!(st, StatusCode::OK, "{v}");
    assert_eq!(
        (v["status"].as_str(), v["offline"].as_bool()),
        (Some("offline_pendente"), Some(true))
    );
    assert!(v["prazo"].as_str().unwrap().contains('/'), "{v}");
    assert_eq!(falso.restantes(), 0, "off-line nao fala com a SEFAZ");
    let chave = v["chave"].as_str().unwrap().to_string();
    assert!(notas.registro(&chave).unwrap().offline);

    let (_, _, corpo) = enviar(&app, get("/nfce/pendentes", true)).await;
    let lista = json_do_corpo(&corpo);
    assert_eq!(
        (&lista["notas"][0]["offline"], &lista["pode_fechar_caixa"]),
        (&json!(true), &json!(false))
    );
    assert_eq!(lista["notas"][0]["prazo"], "vencido");

    // a retomada consulta (a SEFAZ nao conhece a nota) e transmite a MESMA nota
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
    let (st, _, corpo) = enviar(&app, post_vazio("/nfce/pendentes/retomar")).await;
    let v = json_do_corpo(&corpo);
    assert_eq!(st, StatusCode::OK, "{v}");
    assert_eq!(v["retomadas"][0]["chave"], chave.as_str());
    assert_eq!(notas.listar().unwrap().len(), 1);
    assert_eq!(
        notas.registro(&chave).unwrap().situacao.texto(),
        "autorizada"
    );
    assert_eq!(falso.restantes(), 0);
}

#[tokio::test]
async fn off_line_se_necessario_pergunta_o_status_e_so_cai_para_off_line_se_a_sefaz_nao_serve() {
    let p = Pasta::nova();
    let (app, falso, notas) = app_nfce(
        &p,
        vec![
            (
                "NFeStatusServico4",
                status("108", "Servico Paralisado Momentaneamente"),
            ),
            ("NFeStatusServico4", status("107", "Servico em Operacao")),
            ("NFeAutorizacao4", envio("100", OK, NPROT)),
        ],
    );
    let mut pedido = pedido_nfce();
    pedido["offline_se_necessario"] = json!(true);
    let (st, v) = emitir_com(&app, "/nfce/emitir", &pedido).await;
    assert_eq!(
        (st, v["offline"].as_bool()),
        (StatusCode::OK, Some(true)),
        "{v}"
    );
    assert!(
        v["motivo"].as_str().unwrap().contains("nao respondeu"),
        "{v}"
    );
    let (st, v) = emitir_com(&app, "/nfce/emitir", &pedido).await;
    assert_eq!(
        (st, v["status"].as_str()),
        (StatusCode::OK, Some("autorizada")),
        "{v}"
    );
    assert_eq!(notas.listar().unwrap().len(), 2);
    assert_eq!(falso.restantes(), 0);

    // os dois modos juntos, ou um valor que nao e' booleano, recusam sem envio
    pedido["offline"] = json!(true);
    let (st, _) = emitir_com(&app, "/nfce/emitir", &pedido).await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    let (st, _) = emitir_com(&app, "/nfce/emitir", &json!({"offline": "sim"})).await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn consultar_e_status_dizem_o_que_a_sefaz_respondeu() {
    let p = Pasta::nova();
    let (app, falso, notas) = app_nfce(
        &p,
        vec![
            ("NFeAutorizacao4", perdida()),
            ("NFeConsultaProtocolo4", perdida()),
            ("NFeConsultaProtocolo4", consulta("100", OK, NPROT)),
            ("NFeStatusServico4", status("107", "Servico em Operacao")),
            (
                "NFeStatusServico4",
                status("108", "Servico Paralisado Momentaneamente"),
            ),
        ],
    );
    let (_, v) = emitir_com(&app, "/nfce/emitir", &pedido_nfce()).await;
    let chave = v["chave"].as_str().unwrap().to_string();
    let (st, _, corpo) = enviar(&app, get(&format!("/nfce/{chave}"), true)).await;
    let v = json_do_corpo(&corpo);
    assert_eq!(st, StatusCode::OK, "{v}");
    assert_eq!(
        (v["status"].as_str(), v["cstat"].as_str()),
        (Some("autorizada"), Some("100"))
    );
    assert_eq!(
        notas.registro(&chave).unwrap().situacao.texto(),
        "autorizada"
    );

    let (st, _, corpo) = enviar(&app, get("/nfce/status", true)).await;
    let v = json_do_corpo(&corpo);
    assert_eq!(
        (st, v["status"].as_str(), v["uf"].as_str()),
        (StatusCode::OK, Some("em_operacao"), Some("SP")),
        "{v}"
    );
    let (st, _, corpo) = enviar(&app, get("/nfce/status", true)).await;
    assert_eq!(st, StatusCode::BAD_GATEWAY);
    assert_eq!(json_do_corpo(&corpo)["status"], "paralisado");
    assert_eq!(falso.restantes(), 0);

    let (st, _, _) = enviar(&app, get("/nfce/123", true)).await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn cancelar_registra_o_evento_e_o_prazo_vencido_vai_no_json_sem_bloquear() {
    let p = Pasta::nova();
    let (app, falso, notas) = app_nfce(
        &p,
        vec![
            ("NFeAutorizacao4", envio("100", OK, NPROT)),
            (
                "NFeRecepcaoEvento4",
                evento("135", REGISTRADO, "135260000000099"),
            ),
        ],
    );
    let chave = emitir_uma(&app).await["chave"]
        .as_str()
        .unwrap()
        .to_string();
    // a nota foi autorizada em 03/10/2026: hoje o prazo nacional de 24 horas ja acabou
    let corpo = json!({"chave": chave, "justificativa": "Cupom emitido em duplicidade"});
    let (st, v) = emitir_com(&app, "/nfce/cancelar", &corpo).await;
    assert_eq!(st, StatusCode::OK, "{v}");
    assert_eq!(v["situacao"], "cancelada");
    let aviso = v["aviso_prazo"].as_str().unwrap();
    assert!(
        aviso.contains("24 horas") && aviso.contains("a SEFAZ"),
        "{aviso}"
    );
    assert_eq!(
        notas.registro(&chave).unwrap().situacao.texto(),
        "cancelada"
    );
    assert_eq!(falso.restantes(), 0);

    // justificativa curta: recusa sem rede; campo a mais: 400
    let (st, _) = emitir_com(
        &app,
        "/nfce/cancelar",
        &json!({"chave": chave, "justificativa": "curta"}),
    )
    .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    let extra =
        json!({"chave": chave, "justificativa": "Cupom emitido em duplicidade", "extra": 1});
    let (st, _) = emitir_com(&app, "/nfce/cancelar", &extra).await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
}

const EFEITO: [(&str, &str); 6] = [
    ("POST", "/nfce/emitir"),
    ("POST", "/nfce/cancelar"),
    ("GET", "/nfce/status"),
    ("GET", "/nfce/pendentes"),
    ("POST", "/nfce/pendentes/retomar"),
    ("GET", CHAVE),
];

fn rota_de(r: &str) -> String {
    if r.starts_with('/') {
        r.to_string()
    } else {
        format!("/nfce/{r}")
    }
}

#[tokio::test]
async fn sem_permitir_emissao_as_rotas_de_efeito_fiscal_nem_existem_mas_o_danfce_sim() {
    let p = Pasta::nova();
    let app = app_nfce_leitura(&p);
    for (metodo, rota) in EFEITO {
        let auth = format!("Bearer {TOKEN}");
        let req = requisicao(
            metodo,
            &rota_de(rota),
            &[("authorization", auth.as_str())],
            vec![],
        );
        let (st, _, corpo) = enviar(&app, req).await;
        assert_eq!(st, StatusCode::NOT_FOUND, "{metodo} {rota}");
        assert_eq!(json_do_corpo(&corpo)["erro"]["codigo"], "nao_encontrada");
    }
    let (st, _, _) = enviar(
        &app,
        requisicao("POST", "/nfce/danfce", &[], b"<a/>".to_vec()),
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST, "a rota de leitura existe");
}

#[tokio::test]
async fn sem_o_bearer_ou_com_bearer_errado_e_401_e_nada_e_enviado() {
    let p = Pasta::nova();
    let (app, falso, notas) = app_nfce(&p, vec![("NFeAutorizacao4", envio("100", OK, NPROT))]);
    for (metodo, rota) in EFEITO {
        for cab in [vec![], vec![("authorization", "Bearer errado")]] {
            let corpo = pedido_nfce().to_string().into_bytes();
            let (st, h, _) = enviar(&app, requisicao(metodo, &rota_de(rota), &cab, corpo)).await;
            assert_eq!(st, StatusCode::UNAUTHORIZED, "{metodo} {rota}");
            assert_eq!(h["www-authenticate"], "Bearer");
        }
    }
    assert_eq!(falso.restantes(), 1, "nada foi enviado");
    assert!(notas.listar().unwrap().is_empty());
}

#[tokio::test]
async fn campo_desconhecido_e_csc_recusam_antes_de_qualquer_envio() {
    let p = Pasta::nova();
    let (app, falso, notas) = app_nfce(&p, vec![("NFeAutorizacao4", envio("100", OK, NPROT))]);
    let mut errado = pedido_nfce();
    errado["natop"] = json!("VENDA");
    let (st, v) = emitir_com(&app, "/nfce/emitir", &errado).await;
    assert_eq!(
        (st, &v["erro"]["campo"]),
        (StatusCode::UNPROCESSABLE_ENTITY, &json!("natop")),
        "{v}"
    );

    // CSC no pedido: recusado, mesmo com o estrito desligado, e o valor nunca volta na resposta
    for campo in ["csc", "id_csc", "idCSC"] {
        let mut com_csc = pedido_nfce();
        com_csc[campo] = json!(CSC_TESTE);
        for rota in ["/nfce/emitir", "/nfce/emitir?estrito=false"] {
            let (st, _, corpo) = enviar(&app, post_json_token(rota, &com_csc)).await;
            let texto = String::from_utf8_lossy(&corpo).into_owned();
            assert_eq!(
                st,
                StatusCode::UNPROCESSABLE_ENTITY,
                "{campo} {rota}: {texto}"
            );
            assert!(
                texto.contains("nfce csc guardar") && !texto.contains(CSC_TESTE),
                "{texto}"
            );
        }
    }
    assert_eq!(falso.restantes(), 1, "nada foi enviado");
    assert!(notas.listar().unwrap().is_empty());
    assert_eq!(notas.proximo_numero("1").unwrap(), 1);
}

#[tokio::test]
async fn o_csc_nunca_aparece_na_resposta_nem_no_que_o_servidor_grava() {
    let p = Pasta::nova();
    let (app, _, notas) = app_nfce_com_csc(
        &p,
        vec![
            ("NFeAutorizacao4", envio("100", OK, NPROT)),
            ("NFeAutorizacao4", perdida()),
            ("NFeConsultaProtocolo4", perdida()),
        ],
    );
    let mut todas = String::new();
    let (_, v) = emitir_com(&app, "/nfce/emitir", &pedido_nfce()).await;
    assert_eq!(v["status"], "autorizada", "{v}");
    todas += &v.to_string();
    let (_, v) = emitir_com(&app, "/nfce/emitir", &pedido_nfce()).await;
    todas += &v.to_string();
    let mut off = pedido_nfce();
    off["offline"] = json!(true);
    let (_, v) = emitir_com(&app, "/nfce/emitir", &off).await;
    todas += &v.to_string();
    for rota in ["/health", "/nfce/pendentes"] {
        let (_, _, corpo) = enviar(&app, get(rota, true)).await;
        todas += &String::from_utf8_lossy(&corpo);
    }
    let lista = notas.listar().unwrap();
    assert_eq!(lista.len(), 3);
    for n in &lista {
        let xml = notas.ler_xml(&n.chave).unwrap();
        assert!(
            xml.contains("<qrCode>") && xml.contains("p="),
            "o QR Code v2 esta na nota"
        );
        let (_, _, pdf) = enviar(
            &app,
            requisicao("POST", "/nfce/danfce", &[], xml.into_bytes()),
        )
        .await;
        todas += &String::from_utf8_lossy(&pdf);
    }
    assert!(!todas.contains(CSC_TESTE), "o CSC vazou numa resposta");
    assert!(
        !tudo_que_foi_gravado(&p).contains(CSC_TESTE),
        "o CSC foi gravado na pasta"
    );
}

#[tokio::test]
async fn duas_nfce_ao_mesmo_tempo_na_mesma_serie_nao_gastam_o_mesmo_numero() {
    let p = Pasta::nova();
    let (app, falso, notas) = app_nfce(
        &p,
        vec![
            ("NFeAutorizacao4", envio("100", OK, NPROT)),
            ("NFeAutorizacao4", envio("100", OK, "135260000000002")),
        ],
    );
    let pedido = pedido_nfce();
    let (a, b) = tokio::join!(
        emitir_com(&app, "/nfce/emitir", &pedido),
        emitir_com(&app, "/nfce/emitir", &pedido),
    );
    for (st, v) in [&a, &b] {
        assert_eq!(*st, StatusCode::OK, "{v}");
    }
    let mut numeros = [
        a.1["numero"].as_u64().unwrap(),
        b.1["numero"].as_u64().unwrap(),
    ];
    numeros.sort_unstable();
    assert_eq!(numeros, [1, 2]);
    assert_ne!(a.1["chave"], b.1["chave"]);
    assert_eq!(notas.listar().unwrap().len(), 2);
    assert_eq!(falso.restantes(), 0);
}

#[tokio::test]
async fn uma_nfce_e_uma_nfe_ao_mesmo_tempo_seguem_cada_uma_na_sua_numeracao() {
    let p = Pasta::nova();
    let (app, falso, notas_nfce, notas_nfe) = app_nfce_e_nfe(
        &p,
        vec![
            ("NFeAutorizacao4", envio("100", OK, NPROT)),
            ("NFeAutorizacao4", envio("100", OK, "135260000000002")),
        ],
    );
    let (pedido65, pedido55) = (pedido_nfce(), pedido_nfe());
    let (a, b) = tokio::join!(
        emitir_com(&app, "/nfce/emitir", &pedido65),
        emitir_com(&app, "/nfe/emitir", &pedido55),
    );
    for (st, v) in [&a, &b] {
        assert_eq!(*st, StatusCode::OK, "{v}");
    }
    // as duas na serie 1, e as duas com o numero 1: cada modelo conta a sua
    assert_eq!(
        (a.1["numero"].as_u64(), b.1["numero"].as_u64()),
        (Some(1), Some(1))
    );
    assert_eq!(&a.1["chave"].as_str().unwrap()[20..22], "65");
    assert_eq!(&b.1["chave"].as_str().unwrap()[20..22], "55");
    assert_eq!(
        (
            notas_nfce.listar().unwrap().len(),
            notas_nfe.listar().unwrap().len()
        ),
        (1, 1)
    );
    assert_eq!(falso.restantes(), 0);
}

#[tokio::test]
async fn sem_configuracao_utilizavel_a_nfce_diz_o_que_fazer_e_a_leitura_e_a_nfe_seguem_no_ar() {
    // sem [nfe] nem [notas]
    let app = app_nfce_sem_configuracao();
    for (metodo, rota) in EFEITO {
        let corpo = if rota.ends_with("cancelar") {
            json!({"chave": CHAVE, "justificativa": "Cupom emitido em duplicidade"})
        } else {
            pedido_nfce()
        };
        let auth = format!("Bearer {TOKEN}");
        let corpo = corpo.to_string().into_bytes();
        let req = requisicao(
            metodo,
            &rota_de(rota),
            &[("authorization", auth.as_str())],
            corpo,
        );
        let (st, _, resp) = enviar(&app, req).await;
        let v = json_do_corpo(&resp);
        assert_eq!(st, StatusCode::BAD_REQUEST, "{metodo} {rota}: {v}");
        assert_eq!(v["erro"]["codigo"], "uso");
        // em palavras: a NFC-e diz o que falta ([nfe]) e a lista de pendentes, sem rede, diz que falta a pasta de notas
        let msg = v["erro"]["mensagem"].as_str().unwrap();
        assert!(
            msg.contains("NFC-e") || msg.contains("[notas].pasta"),
            "{v}"
        );
    }
    let (st, _, _) = enviar(
        &app,
        requisicao("POST", "/nfce/danfce", &[], b"<a/>".to_vec()),
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);

    // QR Code versao 2 sem o CSC guardado: a frase diz como guardar, e a NF-e emite normalmente
    let p = Pasta::nova();
    let (app, falso) = app_nfce_sem_csc(&p, vec![("NFeAutorizacao4", envio("100", OK, NPROT))]);
    let (st, v) = emitir_com(&app, "/nfce/emitir", &pedido_nfce()).await;
    assert_eq!(st, StatusCode::BAD_REQUEST, "{v}");
    assert!(
        v["erro"]["mensagem"]
            .as_str()
            .unwrap()
            .contains("nfce csc guardar"),
        "{v}"
    );
    let (st, _, _) = enviar(&app, get("/nfce/status", true)).await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    assert_eq!(falso.restantes(), 1, "a NFC-e nao enviou nada");
    let (st, v) = emitir_com(&app, "/nfe/emitir", &pedido_nfe()).await;
    assert_eq!(st, StatusCode::OK, "a NF-e nao e' afetada: {v}");
    assert_eq!(falso.restantes(), 0);
}
