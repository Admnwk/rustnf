// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Consulta de DPS, consulta de evento e registro de eventos, com o cliente real sobre a "Sefin" falsa.

use rustnf::nfse::sefin::Status;
use serde_json::json;

use crate::config::Emitente;
use crate::emissao;
use crate::erro::Erro;
use crate::notas::Situacao;
use crate::pedido::{id_da_dps, registrar_evento_completo, registrar_evento_pedido};
use crate::testes_sefin::{chave, emissor, id_dps, pasta_com, registro, situacao_de, Falso};

const JUSTIFICATIVA: &str = "Valor digitado errado na nota";

fn emitente() -> Emitente {
    Emitente {
        cnpj: "11222333000181".into(),
        municipio: "3550308".into(),
        serie: "1".into(),
        ..Emitente::default()
    }
}

#[tokio::test]
async fn consulta_de_dps_achou_nao_achou_e_sem_resposta() {
    let falso = Falso::novo().com_dps(42).com_nota(42).muda_dps(44);
    let c = emissor(&falso);
    let v = emissao::consultar_dps(&c, &id_dps(42), false)
        .await
        .unwrap();
    assert_eq!(v["status"], "autorizada");
    assert_eq!(v["chave"], chave(42));
    assert_eq!(v["id_dps"], id_dps(42));
    assert!(v["nfse"].is_null(), "sem a opcao, so' a chave");
    // com a nota
    let v = emissao::consultar_dps(&c, &id_dps(42), true).await.unwrap();
    assert_eq!(v["chave"], chave(42));
    assert_eq!(v["nfse"]["numero"], "77");
    // a DPS nao gerou nota
    let e = emissao::consultar_dps(&c, &id_dps(43), false)
        .await
        .unwrap_err();
    assert!(matches!(e, Erro::NaoEncontrada(_)), "{e:?}");
    // sem resposta
    let e = emissao::consultar_dps(&c, &id_dps(44), false)
        .await
        .unwrap_err();
    assert!(matches!(e, Erro::Rede(_)), "{e:?}");
    // Id fora da forma: nem sai
    let antes = falso.urls().len();
    let e = emissao::consultar_dps(&c, "DPS123", false)
        .await
        .unwrap_err();
    assert!(matches!(e, Erro::Recusa { .. }), "{e:?}");
    assert_eq!(falso.urls().len(), antes);
}

#[test]
fn o_id_da_dps_sai_do_emitente_a_serie_e_o_numero() {
    assert_eq!(id_da_dps(&emitente(), None, "42").unwrap(), id_dps(42));
    assert_eq!(
        id_da_dps(&emitente(), Some("7"), "42").unwrap(),
        "DPS355030821122233300018100007000000000000042"
    );
    let sem_municipio = Emitente {
        municipio: String::new(),
        ..emitente()
    };
    assert!(matches!(
        id_da_dps(&sem_municipio, None, "42").unwrap_err(),
        Erro::Entrada(_)
    ));
    assert!(matches!(
        id_da_dps(&emitente(), None, "0").unwrap_err(),
        Erro::Recusa { .. }
    ));
}

#[tokio::test]
async fn consulta_de_evento_confere_o_evento_devolvido() {
    let falso = Falso::novo()
        .com_evento(1, "101103", 1)
        .com_evento_trocado(2, "101103", 1);
    let c = emissor(&falso);
    let v = emissao::consultar_evento(&c, &chave(1), "101103", 1)
        .await
        .unwrap();
    assert_eq!(v["status"], "ok");
    assert_eq!(v["evento"]["tipo"], "101103");
    assert_eq!(v["evento"]["chave"], chave(1));
    assert_eq!(v["evento"]["sequencia"], "1");
    assert_eq!(
        v["evento"]["descricao"],
        "Solicitação de Análise Fiscal para Cancelamento de NFS-e"
    );
    // nao existe
    let e = emissao::consultar_evento(&c, &chave(1), "101103", 2)
        .await
        .unwrap_err();
    assert!(matches!(e, Erro::NaoEncontrada(_)), "{e:?}");
    // a Sefin respondeu com o evento de outra nota: nao e' o pedido
    let e = emissao::consultar_evento(&c, &chave(2), "101103", 1)
        .await
        .unwrap_err();
    assert!(matches!(e, Erro::Rede(_)), "{e:?}");
    // tipo fora da forma: nem sai
    let antes = falso.urls().len();
    let e = emissao::consultar_evento(&c, &chave(1), "../x", 1)
        .await
        .unwrap_err();
    assert!(matches!(e, Erro::Recusa { .. }), "{e:?}");
    assert_eq!(falso.urls().len(), antes);
}

#[tokio::test]
async fn analise_fiscal_registrada_marca_a_nota_da_pasta() {
    let (_p, notas) = pasta_com(
        "eventos-marca",
        &[registro(1, Situacao::Autorizada, "Maria")],
    );
    let falso = Falso::novo().evento_registrado(1, "101103");
    let v = registrar_evento_pedido(
        &emissor(&falso),
        "101103",
        &chave(1),
        "1",
        JUSTIFICATIVA,
        Some(&notas),
    )
    .await
    .unwrap();
    assert_eq!(v["status"], "registrado");
    assert_eq!(v["evento"]["tipo"], "101103");
    assert_eq!(v["marcada"], true);
    assert_eq!(situacao_de(&notas, 1), Situacao::AnaliseFiscal);
}

#[tokio::test]
async fn analise_fiscal_rejeitada_nao_marca() {
    let (_p, notas) = pasta_com(
        "eventos-rejeitada",
        &[registro(1, Situacao::Autorizada, "Maria")],
    );
    let rejeicao = json!({"erros": [{"codigo": "E1802", "descricao": "prazo"}]});
    let falso = Falso::novo().com_post(Ok((400, rejeicao.to_string().into_bytes())));
    let c = emissor(&falso);
    let r = registrar_evento_completo(&c, "101103", &chave(1), "1", JUSTIFICATIVA, Some(&notas))
        .await
        .unwrap();
    assert_eq!(r.resultado.status, Status::Rejeitada);
    assert_eq!(r.marcada, None);
    assert_eq!(situacao_de(&notas, 1), Situacao::Autorizada);
}

#[tokio::test]
async fn analise_fiscal_fora_da_pasta_ou_ja_cancelada_nao_mexe_na_pasta() {
    let (_p, notas) = pasta_com(
        "eventos-fora",
        &[registro(1, Situacao::Cancelada, "Cancelada")],
    );
    // a nota 2 nao esta na pasta; a 1 ja saiu de autorizada
    for n in [1, 2] {
        let falso = Falso::novo().evento_registrado(n, "101103");
        let r = registrar_evento_completo(
            &emissor(&falso),
            "101103",
            &chave(n),
            "1",
            JUSTIFICATIVA,
            Some(&notas),
        )
        .await
        .unwrap();
        assert_eq!(r.resultado.status, Status::Registrado, "nota {n}");
        assert_eq!(r.marcada, Some(false), "nota {n}");
    }
    assert_eq!(situacao_de(&notas, 1), Situacao::Cancelada);
}

#[tokio::test]
async fn outros_eventos_registrados_nao_mexem_na_pasta() {
    let (_p, notas) = pasta_com(
        "eventos-outros",
        &[registro(1, Situacao::Autorizada, "Maria")],
    );
    let falso = Falso::novo().evento_registrado(1, "202201");
    let r = registrar_evento_completo(&emissor(&falso), "202201", &chave(1), "", "", Some(&notas))
        .await
        .unwrap();
    assert_eq!(r.resultado.status, Status::Registrado);
    assert_eq!(r.marcada, None);
    assert_eq!(situacao_de(&notas, 1), Situacao::Autorizada);
}

#[tokio::test]
async fn cancelamento_e_eventos_de_outro_autor_sao_recusados_antes_da_rede() {
    let falso = Falso::novo();
    let c = emissor(&falso);
    for tipo in [
        "101101", "105102", "105104", "105105", "305101", "205204", "999999",
    ] {
        let e = registrar_evento_pedido(&c, tipo, &chave(1), "1", JUSTIFICATIVA, None)
            .await
            .unwrap_err();
        assert!(matches!(e, Erro::Recusa { .. }), "{tipo}: {e:?}");
        if tipo == "101101" {
            assert!(e.to_string().contains("cancelar"), "{e}");
        }
        if tipo == "105104" {
            assert!(e.to_string().contains("fisco"), "{e}");
        }
    }
    assert!(
        falso.urls().is_empty(),
        "nenhuma chamada: {:?}",
        falso.urls()
    );
}
