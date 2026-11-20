// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Tabela embarcada das aliquotas: `tests/casos/reforma_embarcadas.json`.

use rustnf::reforma::aliquotas::{aliquotas, aliquotas_com, embarcadas, publicadas_de_json};
use serde_json::Value;

#[test]
fn embarcadas_contrato() {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/casos/reforma_embarcadas.json");
    let j: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
    for c in j["casos"].as_array().unwrap() {
        let (pubs, emb) = (publicadas_de_json(&c["pub"]), publicadas_de_json(&c["emb"]));
        let r = aliquotas_com(c["data"].as_str().unwrap(), Some(&pubs), &emb, false);
        match &c["e"] {
            Value::String(regra) => assert_eq!(&r.unwrap_err().regra, regra, "{}", c["nome"]),
            e => {
                let a = r.unwrap_or_else(|x| panic!("{}: {x}", c["nome"]));
                let mut d = a.para_json();
                d.as_object_mut().unwrap().remove("motivo");
                assert_eq!(&d, e, "{}", c["nome"]);
                assert_eq!(
                    a.avisos.len() as u64,
                    c["n_avisos"].as_u64().unwrap(),
                    "{}: {:?}",
                    c["nome"],
                    a.avisos
                );
                if let Some(m) = c.get("aviso").and_then(Value::as_str) {
                    assert!(
                        a.avisos.iter().any(|x| x.contains(m)),
                        "{}: {:?}",
                        c["nome"],
                        a.avisos
                    );
                }
                if let Some(f) = c.get("fonte").and_then(Value::as_str) {
                    assert!(a.fonte.contains(f), "{}: {}", c["nome"], a.fonte);
                }
            }
        }
    }
}

/// Ratchet do embarque: toda entrada leva a fonte e passa pela regra do ano.
#[test]
fn tabela_embarcada_tem_fonte_e_e_valida() {
    for (ano, p) in embarcadas() {
        assert!(
            p.fonte.as_deref().is_some_and(|f| !f.trim().is_empty()),
            "embarcadas[{ano}] sem fonte"
        );
        let a = aliquotas(&format!("{ano}-12-31"), None, false).unwrap();
        assert!(a.confirmado, "{ano}: {}", a.motivo);
    }
}
