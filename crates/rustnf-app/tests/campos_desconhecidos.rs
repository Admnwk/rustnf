// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O campo do JSON do pedido que a DPS nao le: o programa o aponta em vez de ignora-lo em silencio.

use std::path::PathBuf;

use rustnf_app::nucleo::{campos_desconhecidos, recusar_desconhecidos};
use serde_json::{json, Value};

fn contrato() -> Value {
    let p =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../rustnf/tests/casos/nfse_nacional.json");
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn exemplo() -> Value {
    serde_json::from_str(include_str!("../../rustnf-cli/exemplos/dps.json")).unwrap()
}

#[test]
fn nenhum_pedido_do_contrato_tem_campo_desconhecido() {
    let c = contrato();
    let mut conferidos = 0;
    for secao in ["dps", "dps_erro", "dps_nt009", "dps_nt009_erro"] {
        for caso in c[secao].as_array().unwrap() {
            let achados = campos_desconhecidos(&caso["entrada"]);
            assert!(
                achados.is_empty(),
                "{secao} '{}': {achados:?}",
                caso["nome"]
            );
            conferidos += 1;
        }
    }
    assert!(conferidos > 100, "{conferidos}");
    assert!(campos_desconhecidos(&exemplo()).is_empty());
    assert!(recusar_desconhecidos(&exemplo()).is_ok());
}

#[test]
fn nome_errado_e_apontado_com_o_caminho() {
    let mut p = exemplo();
    // na raiz, dentro de um grupo, no grupo achatado do prestador e num grupo opcional
    p["descontos"] = json!("10.00");
    p["valores"]["desconto_incondicional"] = json!("5.00");
    p["prestador"]["inscricao_municipal"] = json!("123");
    p["tomador"] = json!({"nome": "Cliente", "documento": "52998224725",
                          "endereco": {"logradouro": "Rua A", "cepp": "01001000"}});
    assert_eq!(
        campos_desconhecidos(&p),
        [
            "descontos",
            "prestador.inscricao_municipal",
            "tomador.endereco.cepp",
            "valores.desconto_incondicional",
        ]
    );
    // os nomes certos, nos mesmos lugares, passam
    let mut certo = exemplo();
    certo["valores"]["desconto_incondicionado"] = json!("5.00");
    certo["prestador"]["im"] = json!("123");
    certo["tomador"] = json!({"nome": "Cliente", "documento": "52998224725",
                              "endereco": {"logradouro": "Rua A", "cep": "01001000"}});
    assert!(campos_desconhecidos(&certo).is_empty());
}

#[test]
fn item_de_lista_leva_a_posicao() {
    let mut p = exemplo();
    p["valores"]["deducao"] = json!({"documentos": [
        {"emissao": "2026-10-01", "valor_dedutivel": "1.00", "valor_deducao": "1.00"},
        {"emissao": "2026-10-01", "valor_dedutivel": "1.00", "valor_deduzido": "1.00"},
    ]});
    assert_eq!(
        campos_desconhecidos(&p),
        ["valores.deducao.documentos[1].valor_deduzido"]
    );
}

#[test]
fn a_recusa_nomeia_o_primeiro_campo_e_conta_os_outros() {
    let mut p = exemplo();
    p["descontos"] = json!("10.00");
    let erro = recusar_desconhecidos(&p).unwrap_err();
    assert_eq!(erro.codigo_saida(), 3);
    assert_eq!(erro.envelope()["erro"]["campo"], "descontos");
    assert!(erro.to_string().contains("desconhecido"), "{erro}");

    p["valores"]["x"] = json!(1);
    let erro = recusar_desconhecidos(&p).unwrap_err();
    assert_eq!(erro.envelope()["erro"]["campo"], "descontos");
    assert!(erro.to_string().contains("valores.x"), "{erro}");
}

#[test]
fn o_que_nao_e_um_pedido_legivel_nao_e_acusado_aqui() {
    // tipo errado, lista e texto: quem diz o erro e' a leitura da DPS, com a mensagem de sempre
    for v in [
        json!([1, 2]),
        json!("texto"),
        json!({"valores": "nao e' objeto", "x": 1}),
    ] {
        assert!(campos_desconhecidos(&v).is_empty(), "{v}");
        assert!(recusar_desconhecidos(&v).is_ok(), "{v}");
    }
}
