// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Release 2 da NT 009 no leiaute nt009 (XSD da Producao Restrita de 24/11/2026): `tests/casos/nfse_nt009_r2.json`.
//! Cada XML de montagem foi validado no esquema oficial fora do repositorio; cada recusa leva o campo e o codigo da
//! regra do Anexo I PRODREST de 24/11/2026.

use rustnf::nfse::{ler_nfse, montar, Dps};
use serde_json::Value;

fn casos() -> Value {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/casos/nfse_nt009_r2.json");
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn txt(v: &Value) -> String {
    v.as_str().unwrap_or_default().to_string()
}

#[test]
fn release2_mesmo_xml() {
    let cs = casos();
    let lista = cs["dps"].as_array().unwrap();
    assert!(lista.len() >= 3);
    for c in lista {
        let d: Dps = serde_json::from_value(c["entrada"].clone()).unwrap();
        let xml = montar(&d).unwrap_or_else(|e| panic!("{}: {e}", c["nome"]));
        assert!(
            !txt(&c["xml"]).is_empty(),
            "caso {} sem o XML esperado",
            c["nome"]
        );
        assert_eq!(xml, txt(&c["xml"]), "caso {}", c["nome"]);
    }
}

#[test]
fn release2_mesmo_erro() {
    let cs = casos();
    for c in cs["dps_erro"].as_array().unwrap() {
        let d: Dps = serde_json::from_value(c["entrada"].clone()).unwrap();
        let e = montar(&d).expect_err(&format!("caso {} deveria recusar", c["nome"]));
        assert_eq!(e.campo, txt(&c["campo"]), "caso {}: {e}", c["nome"]);
        let regra = txt(&c["regra"]);
        if !regra.is_empty() {
            assert!(e.mensagem.contains(&regra), "caso {}: {e}", c["nome"]);
        }
    }
}

/// indDest e' obrigatorio no infDPS do Release 2 mesmo sem o grupo IBSCBS, e sai depois do toma.
#[test]
fn o_inddest_vai_no_infdps_mesmo_sem_ibscbs() {
    let cs = casos();
    let c = cs["dps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["nome"] == "sem_ibscbs_indDest_zero")
        .unwrap();
    let xml = txt(&c["xml"]);
    assert!(xml.contains("</toma><indDest>0</indDest><serv>"), "{xml}");
    assert!(!xml.contains("<IBSCBS>"));
}

/// O ajuste negativo do comercio exterior sai com o sinal (TSDec15V2Neg).
#[test]
fn ajuste_do_comercio_exterior_sai_com_sinal() {
    let cs = casos();
    let c = cs["dps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["nome"] == "comext_exportacao_release2")
        .unwrap();
    assert!(txt(&c["xml"]).contains("<vAjusteBCIBSCBSComExt>-5.25</vAjusteBCIBSCBSComExt>"));
}

/// A NFS-e do Release 2 traz a versao da calculadora; a anterior a ele, nada.
#[test]
fn leitura_traz_a_versao_da_calculadora() {
    let antiga = include_str!("../../rustnf-cli/exemplos/nfse.xml");
    assert_eq!(ler_nfse(antiga).unwrap().versao_calculadora, "");
    let nova = antiga.replacen(
        "<tpEmis>",
        "<verCalcIBSCBS>1.7.2</verCalcIBSCBS><tpEmis>",
        1,
    );
    assert_ne!(nova, antiga, "o exemplo precisa ter tpEmis");
    assert_eq!(ler_nfse(&nova).unwrap().versao_calculadora, "1.7.2");
}
