// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Contrato de `rustnf::reforma::atividades_sn`: `tests/casos/reforma_atividades_sn.json`.
//! Esperado = testes Java da calculadora RFB + carga Flyway (ver o JSON).

use rustnf::reforma::atividades_sn as a;
use serde_json::Value;

fn casos() -> Value {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/casos/reforma_atividades_sn.json");
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn s(v: &Value, k: &str) -> Option<String> {
    v.get(k).and_then(Value::as_str).map(String::from)
}

#[test]
fn dado_embarcado() {
    let c = casos();
    let p = a::procedencia();
    assert_eq!(
        p.ultima_migracao,
        c["procedencia"]["ultima_migracao"].as_str().unwrap()
    );
    assert_eq!(
        p.total,
        c["procedencia"]["total"].as_u64().unwrap() as usize
    );
    assert_eq!(p.commit.len(), 40);
    let ordem: Vec<&str> = a::tabela().iter().map(|x| x.cativsn.as_str()).collect();
    let esperado: Vec<&str> = c["ordem"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap())
        .collect();
    assert_eq!(ordem, esperado);
    let filtra = |f: fn(&a::AtividadeSN) -> bool| -> Vec<&str> {
        a::tabela()
            .iter()
            .filter(|x| f(x))
            .map(|x| x.cativsn.as_str())
            .collect()
    };
    assert_eq!(filtra(|x| x.nao_tributavel), ["90"]);
    assert_eq!(filtra(a::AtividadeSN::sujeita_fator_r), ["09"]);
    assert_eq!(filtra(|x| x.admite_beneficio_municipal), ["12", "13"]);
    assert_eq!(
        filtra(|x| x.iss_no_simples),
        ["07", "09", "10", "12", "13", "14"]
    );
    for m in ["55", "65"] {
        let na: Vec<&str> = a::tabela()
            .iter()
            .filter(|x| x.admite(m, None).unwrap())
            .map(|x| x.cativsn.as_str())
            .collect();
        assert_eq!(
            na,
            ["01", "02", "03", "04", "90"],
            "atividades que a calculadora oficial admite na NF-e e na NFC-e"
        );
    }
}

#[test]
fn anexo() {
    for c in casos()["anexo"].as_array().unwrap() {
        let cod = s(c, "cativsn").unwrap();
        let r = a::anexo(&cod, c["fator_r"].as_bool(), s(c, "data").as_deref());
        match s(c, "erro") {
            Some(e) => assert_eq!(r.unwrap_err().regra, e, "{c}"),
            None => assert_eq!(r.unwrap().map(String::from), s(c, "esperado"), "{c}"),
        }
    }
}

#[test]
fn modelos() {
    for c in casos()["modelos"].as_array().unwrap() {
        let m = a::atividade(&s(c, "cativsn").unwrap(), None)
            .unwrap()
            .unwrap()
            .modelos(None)
            .unwrap();
        let e: Vec<String> = c["esperado"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect();
        assert_eq!(m, e, "{c}");
    }
}

#[test]
fn motivo() {
    for c in casos()["motivo"].as_array().unwrap() {
        let r = a::motivo(
            s(c, "cativsn").as_deref(),
            &s(c, "documento").unwrap(),
            s(c, "cclasstrib").as_deref(),
            s(c, "data").as_deref(),
        );
        match s(c, "erro") {
            Some(e) => assert_eq!(r.unwrap_err().regra, e, "{c}"),
            None => {
                let r = r.unwrap();
                assert_eq!(r.as_ref().map(|x| x.0.to_string()), s(c, "esperado"), "{c}");
                if let Some((_, m)) = r {
                    assert!(!m.is_empty() && !m.contains('{'), "{c}");
                }
            }
        }
    }
}

#[test]
fn derivar_anexos() {
    for c in casos()["derivar"].as_array().unwrap() {
        let atv = a::atividade(&s(c, "cativsn").unwrap(), None)
            .unwrap()
            .unwrap();
        let lista: Option<Vec<&str>> = c["anexos"]
            .as_array()
            .map(|v| v.iter().map(|x| x.as_str().unwrap()).collect());
        let r = a::derivar_anexos(atv, lista.as_deref());
        match s(c, "erro") {
            Some(e) => assert_eq!(r.unwrap_err().regra, e, "{c}"),
            None => {
                let (disp, falt, modo) = r.unwrap();
                let ef: Vec<&str> = c["faltantes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|x| x.as_str().unwrap())
                    .collect();
                assert_eq!(
                    (disp, falt, modo),
                    (
                        c["disponivel"].as_bool().unwrap(),
                        ef,
                        c["modo"].as_str().unwrap()
                    ),
                    "{c}"
                );
            }
        }
    }
}

#[test]
fn opcoes() {
    for c in casos()["opcoes"].as_array().unwrap() {
        let lista: Option<Vec<&str>> = c
            .get("anexos")
            .and_then(Value::as_array)
            .map(|v| v.iter().map(|x| x.as_str().unwrap()).collect());
        let ops = a::opcoes(
            &s(c, "documento").unwrap(),
            s(c, "cclasstrib").as_deref(),
            lista.as_deref(),
            s(c, "data").as_deref(),
        )
        .unwrap();
        let disp: Vec<&str> = ops
            .iter()
            .filter(|o| o.disponivel())
            .map(|o| o.atividade.cativsn.as_str())
            .collect();
        let e: Vec<&str> = c["disponiveis"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap())
            .collect();
        assert_eq!(disp, e, "{c}");
        assert_eq!(
            ops.len() as u64,
            c.get("total").and_then(Value::as_u64).unwrap_or(15),
            "{c}"
        );
    }
}

#[test]
fn opcoes_motivos_distinguiveis() {
    let ops = a::opcoes("91", None, Some(&["II"]), Some("2027-01-01")).unwrap();
    let pega = |k: &str| ops.iter().find(|o| o.atividade.cativsn == k).unwrap();
    assert_eq!(pega("01").motivo.as_ref().unwrap().0, a::INADMISSIVEL);
    assert_eq!(pega("01").anexos_faltantes, ["I"]);
    assert!(
        pega("13").motivo.is_none()
            && pega("13").anexos_faltantes == ["IV"]
            && !pega("13").disponivel()
    );
}

#[test]
fn vedado_nao_tributavel() {
    let v: Vec<i64> = [0, 1, 2, 3, 4, 5, 9]
        .into_iter()
        .filter(|t| a::vedado_nao_tributavel(*t, "nfe", None).unwrap())
        .collect();
    assert_eq!(v, [1, 2, 3, 4, 5]);
    assert!(
        a::vedado_nao_tributavel(3, "nfse", None).unwrap()
            && !a::vedado_nao_tributavel(9, "nfse", None).unwrap()
    );
    assert_eq!(
        a::vedado_nao_tributavel(7, "nfe", None).unwrap_err().regra,
        "VSN-026"
    );
}
