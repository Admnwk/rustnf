// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Contrato de `rustnf::reforma::previa`: `tests/casos/reforma_previa.json`. Os casos oficiais sao os testes da
//! calculadora da Receita Federal; o resultado tem de bater centavo a centavo.

use rustnf::reforma::previa::{bases, tributos, EntradaBase, EntradaTributos, ErroPrevia};
use serde_json::{json, Value};

fn casos() -> Value {
    let p =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/casos/reforma_previa.json");
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn variante(e: &ErroPrevia) -> &'static str {
    match e {
        ErroPrevia::Entrada { .. } => "Entrada",
        ErroPrevia::Classificacao(_) => "Classificacao",
        ErroPrevia::AliquotaNaoPublicada(_) => "AliquotaNaoPublicada",
        ErroPrevia::AliquotaNaoInformada { .. } => "AliquotaNaoInformada",
        ErroPrevia::BaseNegativa => "BaseNegativa",
        ErroPrevia::NaoCoberto(_) => "NaoCoberto",
        ErroPrevia::Estouro => "Estouro",
        _ => "outra",
    }
}

/// Tudo o que o esperado diz tem de estar igual no obtido; o que ele nao cita nao e' conferido.
fn confere(esperado: &Value, obtido: &Value, onde: &str) {
    match esperado {
        Value::Object(m) => {
            for (k, v) in m {
                confere(v, &obtido[k], &format!("{onde}/{k}"));
            }
        }
        _ => assert_eq!(esperado, obtido, "{onde}"),
    }
}

fn tem_aviso(caso: &Value, avisos: &[String]) {
    if let Some(trecho) = caso.get("aviso").and_then(Value::as_str) {
        assert!(
            avisos.iter().any(|a| a.contains(trecho)),
            "{}: nenhum aviso com '{trecho}' em {avisos:?}",
            caso["nome"]
        );
    }
}

#[test]
fn contrato_das_bases() {
    let cs = casos();
    let lista = cs["bases"].as_array().unwrap();
    assert!(lista.len() >= 20, "contrato encolheu: {}", lista.len());
    for c in lista {
        let nome = &c["nome"];
        let entrada: EntradaBase = serde_json::from_value(c["entrada"].clone())
            .unwrap_or_else(|e| panic!("{nome}: entrada invalida: {e}"));
        match bases(&entrada) {
            Err(e) => assert_eq!(json!(variante(&e)), c["recusa"], "{nome}: {e}"),
            Ok(b) => {
                assert!(c.get("recusa").is_none(), "{nome}: esperava recusa");
                let v = serde_json::to_value(&b).unwrap();
                assert_eq!(v["regime_regular"], c["regime_regular"], "{nome}: regular");
                assert_eq!(v["simples"], c["simples"], "{nome}: simples");
                let codigos: Vec<&str> = b.erros.iter().map(|o| o.codigo).collect();
                assert_eq!(json!(codigos), c["erros"], "{nome}: erros {:?}", b.erros);
                tem_aviso(c, &b.avisos);
            }
        }
    }
}

#[test]
fn contrato_dos_tributos() {
    let cs = casos();
    let lista = cs["tributos"].as_array().unwrap();
    assert!(lista.len() >= 12, "contrato encolheu: {}", lista.len());
    for c in lista {
        let nome = &c["nome"];
        let entrada: EntradaTributos = serde_json::from_value(c["entrada"].clone())
            .unwrap_or_else(|e| panic!("{nome}: entrada invalida: {e}"));
        match tributos(&entrada) {
            Err(e) => assert_eq!(json!(variante(&e)), c["recusa"], "{nome}: {e}"),
            Ok(t) => {
                assert!(c.get("recusa").is_none(), "{nome}: esperava recusa");
                confere(
                    &c["esperado"],
                    &serde_json::to_value(&t).unwrap(),
                    &nome.to_string(),
                );
                tem_aviso(c, &t.avisos);
            }
        }
    }
}

/// O IBS total e' a soma do estadual com o municipal ja arredondados, e o valor nunca passa de base x aliquota.
#[test]
fn o_resultado_fecha_consigo_mesmo() {
    for centavos in [1u32, 7, 33, 2_500, 1_234_567, 99_999_999] {
        let entrada: EntradaTributos = serde_json::from_value(json!({
            "data": "2027-06-01", "cst": "200", "cclass_trib": "200028",
            "base": format!("{}.{:02}", centavos / 100, centavos % 100),
            "aliquotas": {"cbs": "8.7654", "ibs_uf": "0.0512", "ibs_mun": "0.0488"}
        }))
        .unwrap();
        let t = tributos(&entrada).unwrap();
        assert_eq!(t.ibs.0, t.ibs_uf.devido.0 + t.ibs_mun.devido.0);
        for x in [&t.ibs_uf, &t.ibs_mun, &t.cbs] {
            // refazendo a conta com os numeros publicados chega-se ao mesmo centavo (tolerancia de meio centavo)
            let exato = t.base.0 as i128 * x.aliquota_efetiva.0 as i128; // centavos x 10^6
            let publicado = x.calculado.0 as i128 * 1_000_000;
            assert!(
                (exato - publicado).abs() <= 500_000,
                "base {} aliquota {}",
                t.base,
                x.aliquota_efetiva
            );
            assert_eq!(x.devido, x.calculado);
        }
    }
}

#[test]
fn entrada_com_campo_desconhecido_ou_codigo_invalido_e_recusada() {
    let ruim = |v: Value| serde_json::from_value::<EntradaBase>(v).is_err();
    assert!(ruim(
        json!({"ano": 2026, "op_simples": 1, "servico": "1", "vServ": "1"})
    ));
    assert!(ruim(json!({"ano": 2026, "op_simples": 9, "servico": "1"})));
    assert!(ruim(
        json!({"ano": 2026, "op_simples": 3, "regime_apuracao": 4})
    ));
    assert!(ruim(
        json!({"ano": 2026, "op_simples": 1, "situacao": "outra"})
    ));
    assert!(serde_json::from_value::<EntradaTributos>(
        json!({"data": "2026-01-01", "cst": "000", "cClassTrib": "000001", "base": "1"})
    )
    .is_err());
}
