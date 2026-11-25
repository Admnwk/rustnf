// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! reforma (IBS/CBS): `tests/casos/reforma.json`. As aliquotas tem de sair texto a texto e a validacao de
//! CST/cClassTrib tem de recusar com a MESMA regra da nota tecnica.

use rustnf::reforma::aliquotas::{self, publicadas_de_json};
use rustnf::reforma::{tabela, ErroReforma};
use serde_json::{json, Value};

fn casos() -> Value {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/casos/reforma.json");
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn txt(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        o => o.to_string(),
    }
}

fn esperado(c: &Value) -> Value {
    match c.get("ok") {
        Some(ok) => json!({"ok": ok}),
        None => json!({"erro": c["erro"]}),
    }
}

fn erro(e: ErroReforma) -> Value {
    json!({"erro": if e.regra.is_empty() { "erro".to_string() } else { e.regra }})
}

#[test]
fn contrato_aliquotas() {
    for c in casos()["aliquotas"].as_array().unwrap() {
        let p = c.get("publicadas").map(publicadas_de_json);
        let simples = c
            .get("simples_nacional")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let r = match aliquotas::aliquotas(&txt(&c["data"]), p.as_ref(), simples) {
            Ok(a) => json!({"ok": a.para_json()}),
            Err(e) => erro(e),
        };
        assert_eq!(r, esperado(c), "{c}");
    }
}

#[test]
fn contrato_validar() {
    for c in casos()["validar"].as_array().unwrap() {
        let data = c.get("data").map(txt);
        let r = match tabela::validar(
            &txt(&c["cst"]),
            &txt(&c["cclass_trib"]),
            &txt(&c["documento"]),
            data.as_deref(),
        ) {
            Ok((cst, _)) => json!({"ok": cst.cst}),
            Err(e) => erro(e),
        };
        assert_eq!(r, esperado(c), "{c}");
    }
}

#[test]
fn tabela_oficial_e_procedencia() {
    let p = tabela::procedencia();
    assert_eq!((p.total_cst, p.total_cclasstrib), (18, 173));
    assert_eq!(
        json!(p.sha256_pagina),
        casos()["procedencia"]["sha256_pagina"]
    );
    // a tabela embutida ja' tem os codigos criados pelo informe tecnico mais recente
    for k in tabela::NOVOS_IT_V170 {
        assert_eq!(&tabela::classificacao(k).unwrap().cst, &k[..3], "{k}");
    }
    assert!(tabela::classificacao("999999")
        .unwrap_err()
        .mensagem
        .contains("inexistente"));
    assert!(!tabela::credito_adquirente("410001").unwrap().cbs);
}

#[test]
fn classificacoes_de_servico_existem_e_filtram_por_documento() {
    let nfse = tabela::classificacoes(None, Some("nfse"), None);
    assert!(
        !nfse.is_empty(),
        "a tabela precisa ter cClassTrib validos na NFS-e"
    );
    assert!(nfse.iter().all(|k| k.vale_em("nfse")));
    let todas = tabela::classificacoes(None, None, None);
    assert!(
        todas.len() > nfse.len(),
        "o filtro por documento tem de cortar"
    );
}

#[test]
fn optante_do_simples_e_avisado_da_opcao_semestral_pelo_regime_regular() {
    let a = aliquotas::aliquotas("2027-03-10", None, true).unwrap();
    let aviso = a
        .avisos
        .iter()
        .find(|x| x.contains("Simples"))
        .expect("aviso do optante");
    for trecho in [
        "art. 41 par. 3o",
        "art. 13 par. 9o a 11",
        "LC 227/2026",
        "1o/1/2027",
        "semestre iniciado em janeiro ou julho",
        "irretratavel",
        "setembro ou marco",
    ] {
        assert!(aviso.contains(trecho), "falta {trecho:?} em {aviso}");
    }
    let sem = aliquotas::aliquotas("2027-03-10", None, false).unwrap();
    assert!(!sem.avisos.iter().any(|x| x.contains("Simples")));
}
