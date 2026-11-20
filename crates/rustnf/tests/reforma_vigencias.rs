// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Vigencia dentro do ano das aliquotas: `tests/casos/reforma_vigencias.json`.

use rustnf::reforma::aliquotas::{aliquotas, exigir_aliquotas, publicadas_de_json};
use serde_json::{json, Value};

#[test]
fn vigencias_contrato() {
    let p =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/casos/reforma_vigencias.json");
    let j: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
    for c in j["casos"].as_array().unwrap() {
        let pubs = publicadas_de_json(&c["pub"]);
        let r = aliquotas(c["data"].as_str().unwrap(), Some(&pubs), false);
        match &c["e"] {
            Value::String(regra) => assert_eq!(&r.unwrap_err().regra, regra, "{}", c["nome"]),
            e => {
                let a = r.unwrap_or_else(|x| panic!("{}: {x}", c["nome"]));
                let mut d = a.para_json();
                d.as_object_mut().unwrap().remove("motivo");
                assert_eq!(&d, e, "{}", c["nome"]);
                if let Some(m) = c.get("motivo").and_then(Value::as_str) {
                    assert!(a.motivo.contains(m), "{}: {}", c["nome"], a.motivo);
                }
            }
        }
    }
}

#[test]
fn exigir_para_antes_da_troca() {
    let pubs =
        publicadas_de_json(&json!({"2027": {"trocas": [{"inicio": "2027-04-01", "cbs": "11"}]}}));
    let e = exigir_aliquotas("2027-03-31", Some(&pubs), false).unwrap_err();
    assert_eq!(e.regra, "aliquota_nao_publicada");
    assert!(e.mensagem.contains("2027-04-01"));
}
