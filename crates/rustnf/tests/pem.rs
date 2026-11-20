// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Contrato de `assinatura::pem::remontar`: `tests/casos/pem.json`.

use rustnf::assinatura::pem::{remontar, remontar_bytes};
use serde_json::Value;

fn golden() -> Value {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/casos/pem.json");
    serde_json::from_str(
        &std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display())),
    )
    .unwrap()
}

fn s(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}

#[test]
fn contrato_remontar() {
    let g = golden();
    let casos = g["casos"].as_array().unwrap();
    assert!(casos.len() >= 24, "golden encolheu");
    for c in casos {
        let (e, esp) = (s(&c["entrada"]), s(&c["esperado"]));
        assert_eq!(remontar(e), esp, "{}", s(&c["nome"]));
        // bytes = latin-1; o golden e' todo <= U+00FF
        let lat = |t: &str| {
            t.chars()
                .map(|ch| u8::try_from(u32::from(ch)).unwrap())
                .collect::<Vec<u8>>()
        };
        assert_eq!(remontar_bytes(&lat(e)), lat(esp), "bytes {}", s(&c["nome"]));
    }
}

#[test]
fn bytes_fora_do_ascii_voltam_iguais() {
    let t = b"\xff\xfe nao e pem \x80";
    assert_eq!(remontar_bytes(t), b"\xff\xfe nao e pem \x80".to_vec());
}
