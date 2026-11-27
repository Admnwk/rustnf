// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! As rotas `/nfe/` em memoria, sobre o transporte falso da SEFAZ e uma pasta temporaria: um teste por rota e desfecho,
//! os 404 sem `--permitir-emissao`, os 401 sem o Bearer, o campo desconhecido recusado sem envio, duas emissoes
//! paralelas com numeros diferentes e a NF-e sem `[nfe]`. Nenhum teste chama a SEFAZ.

mod comum;

use axum::http::StatusCode;
use comum::nfe_falso::*;
use comum::*;
use serde_json::{json, Value};

const OK: &str = "Autorizado o uso da NF-e";
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

async fn emitir_uma(app: &axum::Router) -> Value {
    let (st, _, corpo) = enviar(app, post_json_token("/nfe/emitir", &pedido_nfe())).await;
    let v = json_do_corpo(&corpo);
    assert_eq!(st, StatusCode::OK, "{v}");
    v
}

#[tokio::test]
async fn health_diz_se_a_nfe_esta_pronta() {
    let p = Pasta::nova();
    let (st, _, corpo) = enviar(&app_nfe_leitura(&p), get("/health", false)).await;
    assert_eq!(st, StatusCode::OK);
    let v = json_do_corpo(&corpo);
    assert_eq!(
        (v["status"].as_str(), v["emissao"].as_bool()),
        (Some("ok"), Some(false))
    );
    assert_eq!(v["nfe"], false);
    let (app, _, _) = app_nfe(&p, vec![]);
    let (_, _, corpo) = enviar(&app, get("/health", false)).await;
    let v = json_do_corpo(&corpo);
    assert_eq!(
        (v["emissao"].as_bool(), v["nfe"].as_bool()),
        (Some(true), Some(true))
    );
    let (_, _, corpo) = enviar(&app_nfe_sem_configuracao(), get("/health", false)).await;
    assert_eq!(json_do_corpo(&corpo)["nfe"], false);
}

#[tokio::test]
async fn conferir_diz_ok_ou_recusa_com_o_campo() {
    let p = Pasta::nova();
    let app = app_nfe_leitura(&p);
    let (st, _, corpo) = enviar(&app, post_json("/nfe/conferir", &pedido_nfe())).await;
    let v = json_do_corpo(&corpo);
    assert_eq!(st, StatusCode::OK, "{v}");
    assert_eq!(
        (v["ok"].as_bool(), v["numero"].as_u64()),
        (Some(true), Some(1))
    );
    assert_eq!(v["chave"].as_str().unwrap().len(), 44);
    assert!(v["avisos"].is_array());

    let mut ruim = pedido_nfe();
    ruim["itens"][0]["ncm"] = json!("1");
    let (st, _, corpo) = enviar(&app, post_json("/nfe/conferir", &ruim)).await;
    let v = json_do_corpo(&corpo);
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{v}");
    assert_eq!(v["ok"], false);
    assert_eq!(v["erro"]["codigo"], "recusa");
    assert!(
        v["erro"]["mensagem"].as_str().unwrap().contains("NCM"),
        "{v}"
    );
}

#[tokio::test]
async fn campo_desconhecido_recusa_por_padrao_e_o_estrito_falso_o_ignora() {
    let p = Pasta::nova();
    let app = app_nfe_leitura(&p);
    let mut errado = pedido_nfe();
    errado["natop"] = json!("VENDA");
    let (st, _, corpo) = enviar(&app, post_json("/nfe/conferir", &errado)).await;
    let v = json_do_corpo(&corpo);
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{v}");
    assert_eq!(v["erro"]["campo"], "natop");
    let (st, _, corpo) = enviar(&app, post_json("/nfe/conferir?estrito=false", &errado)).await;
    let v = json_do_corpo(&corpo);
    assert_eq!(st, StatusCode::OK, "{v}");
    assert_eq!(v["campos_desconhecidos"], json!(["natop"]));
    let (st, _, _) = enviar(&app, post_json("/nfe/conferir?estrito=talvez", &errado)).await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn montar_devolve_o_xml_sem_assinatura() {
    let p = Pasta::nova();
    let (st, _, corpo) = enviar(
        &app_nfe_leitura(&p),
        post_json("/nfe/montar", &pedido_nfe()),
    )
    .await;
    let v = json_do_corpo(&corpo);
    assert_eq!(st, StatusCode::OK, "{v}");
    let xml = v["xml"].as_str().unwrap();
    assert!(xml.contains("<NFe") && !xml.contains("<Signature"), "{xml}");
    assert_eq!(v["assinada"], false);
    assert_eq!(v["chave"].as_str().unwrap().len(), 44);
    // montar nao gasta numero nem grava nota
    let notas = rustnf_app::notas::nfe::NotasNfe::abrir(p.caminho("notas")).unwrap();
    assert!(notas.listar().unwrap().is_empty());
}

#[tokio::test]
async fn emitir_autorizada_guarda_a_nota_e_o_danfe_sai_do_xml_guardado() {
    let p = Pasta::nova();
    let (app, falso, notas) = app_nfe(&p, vec![("NFeAutorizacao4", envio("100", OK, NPROT))]);
    let v = emitir_uma(&app).await;
    assert_eq!(
        (v["status"].as_str(), v["situacao"].as_str()),
        (Some("autorizada"), Some("autorizada"))
    );
    assert_eq!(v["protocolo"], NPROT);
    assert_eq!(falso.restantes(), 0);
    let lista = notas.listar().unwrap();
    assert_eq!(lista.len(), 1);
    assert_eq!(lista[0].chave, v["chave"].as_str().unwrap());

    let xml = notas.ler_xml(&lista[0].chave).unwrap();
    let req = requisicao("POST", "/nfe/danfe?canhoto=false", &[], xml.into_bytes());
    let (st, cab, corpo) = enviar(&app, req).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(cab["content-type"], "application/pdf");
    assert!(corpo.starts_with(b"%PDF"), "nao e' um PDF");
}

#[tokio::test]
async fn danfe_de_xml_que_nao_e_nfe_e_400() {
    let p = Pasta::nova();
    let req = requisicao("POST", "/nfe/danfe", &[], b"<a/>".to_vec());
    let (st, _, corpo) = enviar(&app_nfe_leitura(&p), req).await;
    assert_eq!(
        st,
        StatusCode::BAD_REQUEST,
        "{}",
        String::from_utf8_lossy(&corpo)
    );
    assert_eq!(json_do_corpo(&corpo)["erro"]["codigo"], "entrada_invalida");
}

#[tokio::test]
async fn emitir_rejeitada_e_422_com_o_cstat_e_o_numero_continua_livre() {
    let p = Pasta::nova();
    let (app, _, notas) = app_nfe(
        &p,
        vec![(
            "NFeAutorizacao4",
            envio("225", "Rejeicao: Falha no Schema XML do lote de NFe", ""),
        )],
    );
    let (st, _, corpo) = enviar(&app, post_json_token("/nfe/emitir", &pedido_nfe())).await;
    let v = json_do_corpo(&corpo);
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{v}");
    assert_eq!(v["status"], "rejeitada");
    assert_eq!(v["cstat"], "225");
    assert_eq!(v["erro"]["codigo"], "recusa");
    let msg = v["erro"]["mensagem"].as_str().unwrap();
    assert!(
        msg.contains("225") && msg.contains("continua livre"),
        "{msg}"
    );
    assert!(notas.listar().unwrap().is_empty());
}

#[tokio::test]
async fn emitir_sem_resposta_e_502_a_nota_fica_pendente_e_o_retomar_a_fecha() {
    let p = Pasta::nova();
    let (app, falso, notas) = app_nfe(
        &p,
        vec![
            ("NFeAutorizacao4", perdida()),
            ("NFeConsultaProtocolo4", perdida()),
        ],
    );
    let (st, _, corpo) = enviar(&app, post_json_token("/nfe/emitir", &pedido_nfe())).await;
    let v = json_do_corpo(&corpo);
    assert_eq!(st, StatusCode::BAD_GATEWAY, "{v}");
    assert_eq!(v["situacao"], "pendente");
    assert_eq!(v["erro"]["codigo"], "sefin");
    let chave = v["chave"].as_str().unwrap().to_string();

    // a lista dos pendentes tem a chave, sem rede
    let (st, _, corpo) = enviar(&app, get("/nfe/pendentes", true)).await;
    let lista = json_do_corpo(&corpo);
    assert_eq!(st, StatusCode::OK, "{lista}");
    assert_eq!(lista["notas"][0]["chave"], chave.as_str());

    // a SEFAZ responde: a MESMA chave fica autorizada, uma nota so'
    falso.empilhar("NFeConsultaProtocolo4", consulta("100", OK, NPROT));
    // a retomada so' consulta nota com mais de 1 segundo
    tokio::time::sleep(std::time::Duration::from_millis(2100)).await;
    let (st, _, corpo) = enviar(&app, post_vazio("/nfe/pendentes/retomar")).await;
    let v = json_do_corpo(&corpo);
    assert_eq!(st, StatusCode::OK, "{v}");
    assert_eq!(v["notas"][0]["chave"], chave.as_str());
    assert_eq!(v["notas"][0]["situacao"], "autorizada");
    assert_eq!(notas.listar().unwrap().len(), 1);
}

#[tokio::test]
async fn consultar_atualiza_a_pasta_e_chave_ruim_e_422() {
    let p = Pasta::nova();
    let (app, _, notas) = app_nfe(
        &p,
        vec![
            ("NFeAutorizacao4", perdida()),
            ("NFeConsultaProtocolo4", perdida()),
            ("NFeConsultaProtocolo4", consulta("100", OK, NPROT)),
        ],
    );
    let (_, _, corpo) = enviar(&app, post_json_token("/nfe/emitir", &pedido_nfe())).await;
    let chave = json_do_corpo(&corpo)["chave"].as_str().unwrap().to_string();
    let (st, _, corpo) = enviar(&app, get(&format!("/nfe/{chave}"), true)).await;
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

    let (st, _, corpo) = enviar(&app, get("/nfe/123", true)).await;
    assert_eq!(
        st,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{}",
        String::from_utf8_lossy(&corpo)
    );
}

#[tokio::test]
async fn cancelar_corrigir_e_inutilizar_registram_e_guardam() {
    let p = Pasta::nova();
    let evt = |n: &'static str| ("NFeRecepcaoEvento4", evento("135", REGISTRADO, n));
    let (app, falso, notas) = app_nfe(
        &p,
        vec![
            ("NFeAutorizacao4", envio("100", OK, NPROT)),
            evt("135260000000050"),
            evt("135260000000051"),
            ("NFeInutilizacao4", inutilizacao("135260000000300")),
        ],
    );
    let chave = emitir_uma(&app).await["chave"]
        .as_str()
        .unwrap()
        .to_string();

    let corpo = json!({"chave": chave, "texto": "Corrigir o complemento do endereco"});
    let (st, _, resp) = enviar(&app, post_json_token("/nfe/corrigir", &corpo)).await;
    let v = json_do_corpo(&resp);
    assert_eq!(st, StatusCode::OK, "{v}");
    assert_eq!(
        (
            v["status"].as_str(),
            v["tipo"].as_str(),
            v["sequencia"].as_u64()
        ),
        (Some("registrado"), Some("110110"), Some(1))
    );

    let corpo = json!({"chave": chave, "justificativa": "Cancelamento por erro de digitacao"});
    let (st, _, resp) = enviar(&app, post_json_token("/nfe/cancelar", &corpo)).await;
    let v = json_do_corpo(&resp);
    assert_eq!(st, StatusCode::OK, "{v}");
    assert_eq!(v["situacao"], "cancelada");
    assert_eq!(
        notas.registro(&chave).unwrap().situacao.texto(),
        "cancelada"
    );

    let corpo =
        json!({"inicio": 2, "fim": 3, "justificativa": "Numeros pulados por falha do sistema"});
    let (st, _, resp) = enviar(&app, post_json_token("/nfe/inutilizar", &corpo)).await;
    let v = json_do_corpo(&resp);
    assert_eq!(st, StatusCode::OK, "{v}");
    assert_eq!(notas.inutilizacoes().unwrap().len(), 1);
    assert_eq!(falso.restantes(), 0);
}

#[tokio::test]
async fn justificativa_curta_e_422_sem_rede_e_campo_desconhecido_e_400() {
    let p = Pasta::nova();
    let (app, falso, _) = app_nfe(&p, vec![("NFeAutorizacao4", envio("100", OK, NPROT))]);
    let chave = emitir_uma(&app).await["chave"]
        .as_str()
        .unwrap()
        .to_string();
    let corpo = json!({"chave": chave, "justificativa": "curta"});
    let (st, _, resp) = enviar(&app, post_json_token("/nfe/cancelar", &corpo)).await;
    assert_eq!(
        st,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{}",
        String::from_utf8_lossy(&resp)
    );
    let corpo =
        json!({"chave": chave, "justificativa": "Cancelamento por erro de digitacao", "extra": 1});
    let (st, _, _) = enviar(&app, post_json_token("/nfe/cancelar", &corpo)).await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    assert_eq!(falso.restantes(), 0, "nada foi enviado");
}

#[tokio::test]
async fn status_e_epec() {
    let p = Pasta::nova();
    let (app, falso, notas) = app_nfe(
        &p,
        vec![
            ("NFeStatusServico4", status("107", "Servico em Operacao")),
            (
                "NFeStatusServico4",
                status("108", "Servico Paralisado Momentaneamente"),
            ),
            (
                "NFeRecepcaoEvento4",
                evento("136", REGISTRADO, "135260000000070"),
            ),
        ],
    );
    let (st, _, corpo) = enviar(&app, get("/nfe/status", true)).await;
    let v = json_do_corpo(&corpo);
    assert_eq!(st, StatusCode::OK, "{v}");
    assert_eq!(
        (v["status"].as_str(), v["uf"].as_str(), v["svc"].as_bool()),
        (Some("em_operacao"), Some("SP"), Some(false))
    );
    let (st, _, corpo) = enviar(&app, get("/nfe/status", true)).await;
    let v = json_do_corpo(&corpo);
    assert_eq!(st, StatusCode::BAD_GATEWAY, "{v}");
    assert_eq!(v["status"], "paralisado");
    let (st, _, _) = enviar(&app, get("/nfe/status?svc=sim", true)).await;
    assert_eq!(st, StatusCode::BAD_REQUEST);

    let mut epec = pedido_nfe();
    epec["x_just"] = json!("Sem acesso a SEFAZ por queda do link de internet");
    let (st, _, corpo) = enviar(&app, post_json_token("/nfe/epec", &epec)).await;
    let v = json_do_corpo(&corpo);
    assert_eq!(st, StatusCode::OK, "{v}");
    assert_eq!(v["situacao"], "epec");
    assert_eq!(notas.listar().unwrap().len(), 1);
    assert_eq!(falso.restantes(), 0);
}

#[tokio::test]
async fn epec_com_substitui_liga_a_nota_sem_resposta_e_a_retomada_so_a_consulta() {
    let p = Pasta::nova();
    let (app, falso, notas) = app_nfe(
        &p,
        vec![
            ("NFeAutorizacao4", perdida()),
            ("NFeConsultaProtocolo4", perdida()),
        ],
    );
    let (st, _, corpo) = enviar(&app, post_json_token("/nfe/emitir", &pedido_nfe())).await;
    let v = json_do_corpo(&corpo);
    assert_eq!(st, StatusCode::BAD_GATEWAY, "{v}");
    let pendente = v["chave"].as_str().unwrap().to_string();

    let mut epec = pedido_nfe();
    epec["x_just"] = json!("Sem acesso a SEFAZ por queda do link de internet");
    // chave que nao serve: 422 com o campo, antes de qualquer envio
    epec["substitui"] = json!("123");
    let (st, _, corpo) = enviar(&app, post_json_token("/nfe/epec", &epec)).await;
    let v = json_do_corpo(&corpo);
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{v}");
    assert_eq!(v["erro"]["campo"], "substitui");
    assert_eq!(falso.restantes(), 0, "nada foi enviado");
    // o campo so' existe no EPEC: na emissao normal o leiaute o recusa
    let (st, _, corpo) = enviar(&app, post_json_token("/nfe/emitir", &epec)).await;
    assert_eq!(
        st,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{}",
        json_do_corpo(&corpo)
    );

    // a substituicao vale: o EPEC sai com o numero 2, ligado a nota sem resposta
    falso.empilhar(
        "NFeRecepcaoEvento4",
        evento("136", REGISTRADO, "135260000000070"),
    );
    epec["substitui"] = json!(pendente);
    let (st, _, corpo) = enviar(&app, post_json_token("/nfe/epec", &epec)).await;
    let v = json_do_corpo(&corpo);
    assert_eq!(st, StatusCode::OK, "{v}");
    assert_eq!(
        (v["situacao"].as_str(), v["substitui"].as_str()),
        (Some("epec"), Some(pendente.as_str()))
    );
    let chave_epec = v["chave"].as_str().unwrap().to_string();

    let (_, _, corpo) = enviar(&app, get("/nfe/pendentes", true)).await;
    let lista = json_do_corpo(&corpo);
    let da_pendente = lista["notas"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["chave"] == pendente.as_str())
        .unwrap();
    assert_eq!(
        (
            da_pendente["substituida_por"].as_str(),
            da_pendente["a_fazer"].as_str()
        ),
        (Some(chave_epec.as_str()), Some("consultar"))
    );

    // a SEFAZ nao tem a nota sem resposta: a retomada so' a consulta (nenhum reenvio) e manda inutilizar o numero
    falso.empilhar(
        "NFeConsultaProtocolo4",
        consulta(
            "217",
            "Rejeicao: NF-e nao consta na base de dados da SEFAZ",
            "",
        ),
    );
    falso.empilhar("NFeAutorizacao4", envio("100", OK, NPROT));
    // a retomada so' consulta nota com mais de 1 segundo
    tokio::time::sleep(std::time::Duration::from_millis(2100)).await;
    let (st, _, corpo) = enviar(&app, post_vazio("/nfe/pendentes/retomar")).await;
    let v = json_do_corpo(&corpo);
    assert_eq!(st, StatusCode::OK, "{v}");
    let da_pendente = v["notas"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["chave"] == pendente.as_str())
        .unwrap();
    assert_eq!(
        (
            da_pendente["acao"].as_str(),
            da_pendente["a_fazer"].as_str(),
            da_pendente["substituida_por"].as_str()
        ),
        (
            Some("consultou"),
            Some("inutilizar"),
            Some(chave_epec.as_str())
        )
    );
    assert!(
        da_pendente["aviso"]
            .as_str()
            .unwrap()
            .contains("inutilize o numero 1"),
        "{da_pendente}"
    );
    assert_eq!(falso.restantes(), 0, "so' a consulta e o envio do EPEC");
    assert_eq!(notas.registro(&pendente).unwrap().cstat, "217");
}

#[tokio::test]
async fn sem_permitir_emissao_as_rotas_de_efeito_fiscal_nem_existem() {
    let p = Pasta::nova();
    let app = app_nfe_leitura(&p);
    for (metodo, rota) in [
        ("POST", "/nfe/emitir"),
        ("POST", "/nfe/epec"),
        ("POST", "/nfe/cancelar"),
        ("POST", "/nfe/corrigir"),
        ("POST", "/nfe/inutilizar"),
        ("GET", "/nfe/status"),
        ("GET", "/nfe/pendentes"),
        ("POST", "/nfe/pendentes/retomar"),
        ("GET", "/nfe/35260111222333000181550010000000011234567890"),
    ] {
        let auth = format!("Bearer {TOKEN}");
        let req = requisicao(metodo, rota, &[("authorization", auth.as_str())], vec![]);
        let (st, _, corpo) = enviar(&app, req).await;
        assert_eq!(st, StatusCode::NOT_FOUND, "{metodo} {rota}");
        assert_eq!(json_do_corpo(&corpo)["erro"]["codigo"], "nao_encontrada");
    }
}

#[tokio::test]
async fn sem_o_bearer_ou_com_bearer_errado_e_401_e_nada_e_enviado() {
    let p = Pasta::nova();
    let (app, falso, notas) = app_nfe(&p, vec![("NFeAutorizacao4", envio("100", OK, NPROT))]);
    for (metodo, rota) in [
        ("POST", "/nfe/emitir"),
        ("POST", "/nfe/epec"),
        ("POST", "/nfe/cancelar"),
        ("POST", "/nfe/corrigir"),
        ("POST", "/nfe/inutilizar"),
        ("GET", "/nfe/status"),
        ("GET", "/nfe/pendentes"),
        ("POST", "/nfe/pendentes/retomar"),
        ("GET", "/nfe/35260111222333000181550010000000011234567890"),
    ] {
        for cab in [vec![], vec![("authorization", "Bearer errado")]] {
            let corpo = pedido_nfe().to_string().into_bytes();
            let (st, h, _) = enviar(&app, requisicao(metodo, rota, &cab, corpo)).await;
            assert_eq!(st, StatusCode::UNAUTHORIZED, "{metodo} {rota}");
            assert_eq!(h["www-authenticate"], "Bearer");
        }
    }
    assert_eq!(falso.restantes(), 1, "nada foi enviado");
    assert!(notas.listar().unwrap().is_empty());
}

#[tokio::test]
async fn emitir_com_campo_desconhecido_recusa_antes_de_qualquer_envio() {
    let p = Pasta::nova();
    let (app, falso, notas) = app_nfe(&p, vec![("NFeAutorizacao4", envio("100", OK, NPROT))]);
    let mut errado = pedido_nfe();
    errado["natop"] = json!("VENDA");
    let (st, _, corpo) = enviar(&app, post_json_token("/nfe/emitir", &errado)).await;
    let v = json_do_corpo(&corpo);
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{v}");
    assert_eq!(v["erro"]["campo"], "natop");
    assert_eq!(falso.restantes(), 1, "nada foi enviado");
    assert!(notas.listar().unwrap().is_empty());
    assert_eq!(notas.proximo_numero("1").unwrap(), 1);
}

#[tokio::test]
async fn duas_emissoes_ao_mesmo_tempo_na_mesma_serie_nao_gastam_o_mesmo_numero() {
    let p = Pasta::nova();
    let (app, falso, notas) = app_nfe(
        &p,
        vec![
            ("NFeAutorizacao4", envio("100", OK, NPROT)),
            ("NFeAutorizacao4", envio("100", OK, "135260000000002")),
        ],
    );
    let (a, b) = tokio::join!(
        enviar(&app, post_json_token("/nfe/emitir", &pedido_nfe())),
        enviar(&app, post_json_token("/nfe/emitir", &pedido_nfe())),
    );
    for (st, _, corpo) in [&a, &b] {
        assert_eq!(*st, StatusCode::OK, "{}", String::from_utf8_lossy(corpo));
    }
    let (va, vb) = (json_do_corpo(&a.2), json_do_corpo(&b.2));
    let mut numeros = [
        va["numero"].as_u64().unwrap(),
        vb["numero"].as_u64().unwrap(),
    ];
    numeros.sort_unstable();
    assert_eq!(numeros, [1, 2]);
    assert_ne!(va["chave"], vb["chave"]);
    assert_eq!(notas.listar().unwrap().len(), 2);
    assert_eq!(falso.restantes(), 0);
}

#[tokio::test]
async fn nfe_sem_configuracao_diz_o_que_configurar_e_a_leitura_segue_no_ar() {
    let app = app_nfe_sem_configuracao();
    let (st, _, corpo) = enviar(&app, post_json_token("/nfe/emitir", &pedido_nfe())).await;
    let v = json_do_corpo(&corpo);
    assert_eq!(st, StatusCode::BAD_REQUEST, "{v}");
    assert_eq!(v["erro"]["codigo"], "uso");
    let msg = v["erro"]["mensagem"].as_str().unwrap();
    assert!(msg.contains("[nfe]") && msg.contains("[notas]"), "{msg}");
    let (st, _, corpo) = enviar(&app, get("/nfe/status", true)).await;
    assert_eq!(
        st,
        StatusCode::BAD_REQUEST,
        "{}",
        String::from_utf8_lossy(&corpo)
    );
    let (st, _, corpo) = enviar(&app, get("/nfe/pendentes", true)).await;
    assert_eq!(
        st,
        StatusCode::BAD_REQUEST,
        "{}",
        String::from_utf8_lossy(&corpo)
    );
    // a leitura (sem rede) nao cai: diz em palavras o que falta
    let (st, _, corpo) = enviar(&app, post_json("/nfe/conferir", &pedido_nfe())).await;
    assert_ne!(
        st,
        StatusCode::INTERNAL_SERVER_ERROR,
        "{}",
        String::from_utf8_lossy(&corpo)
    );
    assert!(json_do_corpo(&corpo)["erro"]["mensagem"].is_string());
}
