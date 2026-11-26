// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Leitura da NFC-e (modelo 65): `tests/casos/nfe_leitura_nfce.json`, com os XML que a montagem e a assinatura do
//! `rustnf` produzem (QR Code versao 3) e o protocolo ficticio. A NF-e modelo 55 continua lida como antes
//! (`tests/nfe_leitura.rs`).

use rustnf::nfe::{conferir, ler, ErroLeitura};
use serde_json::{to_value, Value};

fn casos() -> Value {
    let p =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/casos/nfe_leitura_nfce.json");
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

#[test]
fn contrato_ler_a_nfce() {
    let c = casos();
    let todos = c["ler"].as_array().unwrap();
    assert_eq!(todos.len(), 10);
    for x in todos {
        let nome = x["nome"].as_str().unwrap();
        let nfe =
            ler(x["xml"].as_str().unwrap().as_bytes()).unwrap_or_else(|e| panic!("{nome}: {e}"));
        let json = to_value(&nfe).unwrap();
        for (ponteiro, esperado) in x["esperado"].as_object().unwrap() {
            let achado = json.pointer(ponteiro).unwrap_or(&Value::Null);
            assert_eq!(achado, esperado, "{nome}: {ponteiro}");
        }
        assert_eq!(
            nfe.situacao().como_texto(),
            x["situacao"],
            "{nome}: situacao"
        );
        assert_eq!(
            nfe.autorizada(),
            x["autorizada"].as_bool().unwrap(),
            "{nome}"
        );
        assert!(nfe.chave_valida(), "{nome}: chave");
        // a NFC-e sempre traz o QR Code e a URL de consulta pela chave do `infNFeSupl`
        assert!(nfe.qr_code.contains("qrcode?p="), "{nome}: qrCode");
        assert!(nfe.url_chave.ends_with("/consulta"), "{nome}: urlChave");
        assert_eq!(
            to_value(conferir(&nfe)).unwrap(),
            x["conferir"],
            "{nome}: conferir"
        );
    }
}

#[test]
fn contrato_erros_da_nfce() {
    let c = casos();
    let erros = c["erros"].as_array().unwrap();
    assert_eq!(erros.len(), 3);
    for x in erros {
        let nome = x["nome"].as_str().unwrap();
        let e = ler(x["xml"].as_str().unwrap().as_bytes()).unwrap_err();
        assert_eq!(e.to_string(), x["erro"].as_str().unwrap(), "{nome}");
    }
}

#[test]
fn o_erro_da_nfce_e_tipado() {
    let c = casos();
    let por_nome = |n: &str| {
        let x = c["erros"]
            .as_array()
            .unwrap()
            .iter()
            .find(|x| x["nome"] == n)
            .unwrap();
        ler(x["xml"].as_str().unwrap().as_bytes()).unwrap_err()
    };
    assert_eq!(
        por_nome("nfce_leiaute_310"),
        ErroLeitura::NfceLeiaute("3.10".into())
    );
    assert_eq!(por_nome("modelo_57"), ErroLeitura::Modelo("57".into()));
}
