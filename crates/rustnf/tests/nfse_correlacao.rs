// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! nfse::correlacao (Anexo VIII v1.02.00): `tests/casos/nfse_correlacao.json` (esperado escrito a mao; erro/aviso
//! conferem por TRECHO da mensagem).

use rustnf::nfse::correlacao::{self as c, Filtro};
use serde_json::{json, Value};

fn casos() -> Value {
    let p =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/casos/nfse_correlacao.json");
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn s<'a>(a: &'a Value, k: &str) -> Option<&'a str> {
    a.get(k).and_then(Value::as_str)
}

fn rodar(op: &str, a: &Value) -> Value {
    let f = Filtro {
        nbs: s(a, "nbs"),
        cclasstrib: s(a, "cclasstrib"),
        cod_ativ_sn: s(a, "cod_ativ_sn"),
    };
    let codigo = s(a, "codigo").unwrap_or("");
    let r = match op {
        "sugerir" => c::sugerir(codigo, f).map(|x| {
            json!({"linha": x.linha, "cind_op": x.cind_op, "cclasstrib": x.cclasstrib, "cst": x.cst,
                   "avisos": x.avisos})
        }),
        "candidatos" => {
            c::candidatos(codigo, f).map(|v| json!(v.iter().map(|x| x.linha).collect::<Vec<_>>()))
        }
        "cind_op" => c::cind_op_da_operacao(
            s(a, "cind_op").unwrap(),
            a["onerosa"].as_bool().unwrap(),
            a["exterior"].as_bool().unwrap(),
        )
        .map(Value::from),
        "conferir" => Ok(json!(c::conferir(
            codigo,
            s(a, "nbs").unwrap(),
            s(a, "cind_op").unwrap(),
            s(a, "cclasstrib").unwrap()
        ))),
        o => panic!("operacao {o}"),
    };
    r.unwrap_or_else(|e| json!({"erro": e.mensagem}))
}

fn contem(o: &Value, t: &Value) -> bool {
    o.as_str()
        .zip(t.as_str())
        .is_some_and(|(o, t)| o.contains(t))
}

fn trechos(o: &Value, t: &Value) -> bool {
    let (o, t) = (o.as_array().unwrap(), t.as_array().unwrap());
    o.len() == t.len() && o.iter().zip(t).all(|(o, t)| contem(o, t))
}

fn bate(o: &Value, e: &Value) -> bool {
    if let Some(t) = e.get("erro") {
        return o.get("erro").is_some_and(|m| contem(m, t));
    }
    if let (Some(eo), Some(oo)) = (e.as_object(), o.as_object()) {
        let sem = |m: &serde_json::Map<String, Value>| {
            m.iter()
                .filter(|(k, _)| *k != "avisos")
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<Vec<_>>()
        };
        let (mut a, mut b) = (sem(oo), sem(eo));
        a.sort_by(|x, y| x.0.cmp(&y.0));
        b.sort_by(|x, y| x.0.cmp(&y.0));
        return a == b && trechos(&oo["avisos"], &eo["avisos"]);
    }
    if e.as_array()
        .is_some_and(|v| v.first().is_some_and(Value::is_string))
    {
        return trechos(o, e);
    }
    o == e
}

#[test]
fn contrato() {
    let cs = casos();
    assert_eq!(cs["versao_anexo"], c::versao());
    let mut ruins = Vec::new();
    for k in cs["casos"].as_array().unwrap() {
        let o = rodar(k["operacao"].as_str().unwrap(), &k["args"]);
        if !bate(&o, &k["esperado"]) {
            ruins.push(format!("{}: {o}", k["nome"]));
        }
    }
    assert!(ruins.is_empty(), "{ruins:#?}");
    assert!(cs["casos"].as_array().unwrap().len() >= 45);
}

#[test]
fn contraprova_do_comparador() {
    let e = json!({"linha": 2, "cind_op": "100301", "cclasstrib": "000001", "cst": "000", "avisos": []});
    assert!(!bate(
        &json!({"linha": 1, "cind_op": "100301", "cclasstrib": "000001", "cst": "000", "avisos": []}),
        &e
    ));
    assert!(!bate(
        &json!({"linha": 2, "cind_op": "100301", "cclasstrib": "000001", "cst": "000", "avisos": ["x"]}),
        &e
    ));
    assert!(!bate(&json!({"erro": "outra"}), &json!({"erro": "tem 2"})));
    assert!(!bate(&json!([1360]), &json!([1360, 1361])));
}

#[test]
fn cst_da_tabela_e_campo_do_erro() {
    let x = c::sugerir(
        "250101",
        Filtro {
            nbs: Some("126030000"),
            ..Filtro::default()
        },
    )
    .unwrap();
    assert_eq!(x.cst.as_deref(), Some("200"));
    let e = c::sugerir("2501", Filtro::default()).unwrap_err();
    assert_eq!(e.campo, c::CAMPO);
    assert!(!c::descricao_nbs("1.2603.00.00").is_empty());
}
