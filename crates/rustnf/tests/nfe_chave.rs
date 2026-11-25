// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Chave de acesso da NF-e e da NFC-e (44 posicoes): `tests/casos/nfe_chave.json`. O DV dos casos foi calculado a
//! parte (script de geracao em Python, regra do MOC), nao pela biblioteca.

use rustnf::chave_dfe::{
    achar, achar_no_codigo, codigo_valido, conferir, decompor, formatar, gerar_codigo, limpar,
    montar, validar, ErroChave, Montar,
};
use serde_json::Value;

fn casos() -> Value {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/casos/nfe_chave.json");
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn s(v: &Value) -> &str {
    v.as_str().unwrap()
}

fn u(v: &Value, k: &str) -> Option<u64> {
    v.get(k).and_then(Value::as_u64)
}

fn lista(v: &Value) -> Vec<String> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| s(x).to_string())
        .collect()
}

fn motivo(r: Result<(), ErroChave>) -> String {
    r.err().map(|e| e.to_string()).unwrap_or_default()
}

#[test]
fn chaves_validas() {
    for k in casos()["validos"].as_array().unwrap() {
        assert_eq!(motivo(validar(s(k), true)), "", "{k}");
        assert_eq!(
            motivo(validar(&formatar(s(k)), true)),
            "",
            "{k}: com grupos de 4"
        );
        assert_eq!(
            motivo(validar(&format!("NFe{}", s(k)), true)),
            "",
            "{k}: com o prefixo do Id"
        );
    }
}

#[test]
fn chaves_invalidas_dizem_o_motivo() {
    for x in casos()["invalidos"].as_array().unwrap() {
        assert_eq!(
            motivo(validar(s(&x[0]), x[1].as_bool().unwrap())),
            s(&x[2]),
            "{x}"
        );
    }
    // sem conferir o emitente, so' o DV do CPF ou do CNPJ do emitente deixa de ser olhado
    for k in casos()["validos_sem_conferir_emitente"].as_array().unwrap() {
        assert_eq!(motivo(validar(s(k), false)), "", "{k}");
    }
}

#[test]
fn dv_limpar_e_formatar() {
    let c = casos();
    for x in c["dv"].as_array().unwrap() {
        let chave = format!("{}{}", s(&x[0]), x[1]);
        let so_o_dv_confere = s(&x[0]) != "0".repeat(43);
        let r = validar(&chave, false);
        if so_o_dv_confere {
            assert_eq!(motivo(r), "", "{x}");
        } else {
            // 43 zeros e DV 0: o DV fecha, mas UF e mes nao existem
            assert_eq!(motivo(r), "codigo de UF inexistente", "{x}");
        }
    }
    for x in c["limpar"].as_array().unwrap() {
        assert_eq!(limpar(s(&x[0])), s(&x[1]), "{x}");
    }
    for x in c["formatar"].as_array().unwrap() {
        assert_eq!(formatar(s(&x[0])), s(&x[1]), "{x}");
    }
}

#[test]
fn decompor_le_cada_campo() {
    for x in casos()["decompor"].as_array().unwrap() {
        let d = decompor(s(&x["chave"]), true).unwrap();
        let e = &x["esperado"];
        let achado = serde_json::json!({
            "cuf": d.cuf, "uf": d.uf, "aamm": d.aamm, "competencia": d.competencia, "emitente": d.emitente,
            "emitente_cpf": d.emitente_cpf, "modelo": d.modelo, "serie": d.serie, "numero": d.numero,
            "tp_emis": d.tp_emis, "codigo": d.codigo, "dv": d.dv,
            "erro": d.erro.map(|e| e.to_string()),
        });
        for (campo, valor) in e.as_object().unwrap() {
            assert_eq!(&achado[campo], valor, "{campo} em {x}");
        }
        assert_eq!(d.valida(), e["erro"].is_null());
        assert_eq!(d.formatada(), formatar(&d.chave));
    }
    assert!(decompor("123", true).is_err());
}

fn parametros(kw: &Value) -> Montar<'_> {
    Montar {
        cuf: u(kw, "cuf").unwrap() as u32,
        aamm: s(&kw["aamm"]),
        emitente: s(&kw["emitente"]),
        modelo: u(kw, "modelo").unwrap_or(55) as u32,
        serie: u(kw, "serie").unwrap_or(1),
        numero: u(kw, "numero").unwrap(),
        tp_emis: u(kw, "tp_emis").unwrap_or(1) as u32,
        codigo: u(kw, "codigo"),
    }
}

#[test]
fn montar_fecha_com_o_dv_e_recusa_o_que_nao_cabe() {
    let c = casos();
    for x in c["montar"].as_array().unwrap() {
        assert_eq!(montar(&parametros(&x[0])).unwrap(), s(&x[1]), "{x}");
    }
    for x in c["montar_erro"].as_array().unwrap() {
        assert_eq!(
            montar(&parametros(&x[0])).unwrap_err().to_string(),
            s(&x[1]),
            "{x}"
        );
    }
}

#[test]
fn codigo_numerico_segue_a_regra_b09_30() {
    for x in casos()["codigo_valido"].as_array().unwrap() {
        let a = x.as_array().unwrap();
        assert_eq!(
            codigo_valido(a[0].as_u64().unwrap(), a[1].as_u64().unwrap()),
            a[2].as_bool().unwrap(),
            "{x}"
        );
    }
}

#[test]
fn montar_sem_codigo_sorteia_um_valido() {
    for numero in 1..300u64 {
        let m = Montar {
            cuf: 35,
            aamm: "2610",
            emitente: "11222333000181",
            numero,
            ..Montar::default()
        };
        let chave = montar(&m).unwrap();
        let d = decompor(&chave, true).unwrap();
        assert!(d.valida(), "{chave}");
        assert!(
            codigo_valido(d.codigo, numero) && d.numero == numero,
            "{chave}"
        );
        assert!(codigo_valido(gerar_codigo(numero), numero));
    }
}

#[test]
fn achar_no_texto_e_no_codigo() {
    let c = casos();
    for x in c["achar"].as_array().unwrap() {
        assert_eq!(achar(s(&x[0]), true), lista(&x[1]), "{x}");
    }
    for x in c["achar_no_codigo"].as_array().unwrap() {
        assert_eq!(achar_no_codigo(s(&x[0]), true), lista(&x[1]), "{x}");
    }
}

#[test]
fn achar_em_texto_grande_nao_trava() {
    let c = casos();
    let alvo = s(&c["validos"][0]).to_string();
    let miolo = "linha de extrato 1234567890 12/10/2026 R$ 1.234,56\n".repeat(4000);
    let t0 = std::time::Instant::now();
    assert_eq!(achar(&format!("{miolo}{alvo}{miolo}"), true), vec![alvo]);
    assert!(t0.elapsed().as_secs() < 10);
}

#[test]
fn a_chave_da_nfse_continua_com_o_conferir_de_antes() {
    let c = casos();
    let nfse = s(&c["nfse"]);
    assert_eq!(conferir(nfse), Ok(()));
    // a chave de 44 posicoes nao passa por `conferir` e a de 50 nao passa por `validar`
    assert!(conferir(s(&c["validos"][0])).is_err());
    assert_eq!(
        validar(nfse, false).unwrap_err().to_string(),
        "a chave da NF-e e da NFC-e tem 44 posicoes"
    );
}
