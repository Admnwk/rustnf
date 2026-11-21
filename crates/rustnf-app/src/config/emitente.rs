// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Completa o JSON do pedido da DPS com os dados do `[emitente]`.

use serde_json::{json, Map, Value};

use super::modelo::Emitente;

/// Campo ausente: nao existe, e' `null` ou e' texto vazio.
fn ausente(objeto: &Map<String, Value>, chave: &str) -> bool {
    match objeto.get(chave) {
        None | Some(Value::Null) => true,
        Some(Value::String(s)) => s.trim().is_empty(),
        Some(_) => false,
    }
}

fn completar(objeto: &mut Map<String, Value>, chave: &str, valor: Value) {
    if ausente(objeto, chave) {
        objeto.insert(chave.into(), valor);
    }
}

fn texto(s: &str) -> Option<Value> {
    (!s.is_empty()).then(|| json!(s))
}

/// Completa, no JSON do pedido da DPS, so' os campos AUSENTES com os dados do emitente. Nunca sobrescreve o que o
/// pedido traz. Pedido que nao e' um objeto, ou `prestador` que nao e' um objeto, ficam como estao (a leitura da DPS
/// recusa depois, com a mensagem de sempre).
pub fn aplicar_emitente(pedido: &mut Value, emitente: &Emitente) {
    let Some(raiz) = pedido.as_object_mut() else {
        return;
    };
    if let Some(v) = texto(&emitente.municipio) {
        completar(raiz, "cmun_emissor", v);
    }
    if let Some(v) = texto(&emitente.serie) {
        completar(raiz, "serie", v);
    }
    let prestador = raiz
        .entry("prestador")
        .or_insert_with(|| Value::Object(Map::new()));
    if prestador.is_null() {
        *prestador = Value::Object(Map::new());
    }
    let Some(p) = prestador.as_object_mut() else {
        return;
    };
    let documento = if emitente.cnpj.is_empty() {
        &emitente.cpf
    } else {
        &emitente.cnpj
    };
    for (chave, valor) in [
        ("documento", texto(documento)),
        ("nome", texto(&emitente.nome)),
        ("im", texto(&emitente.inscricao_municipal)),
    ] {
        if let Some(v) = valor {
            completar(p, chave, v);
        }
    }
    completar(p, "op_simples", json!(emitente.op_simples));
    if emitente.regime_apuracao != 0 {
        completar(p, "apuracao_sn", json!(emitente.regime_apuracao));
    }
    if emitente.regime_especial != 0 {
        completar(p, "regime_especial", json!(emitente.regime_especial));
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn emitente() -> Emitente {
        Emitente {
            cnpj: "11222333000181".into(),
            nome: "OFICINA TESTE".into(),
            inscricao_municipal: "123456".into(),
            municipio: "3550308".into(),
            op_simples: 3,
            regime_apuracao: 1,
            serie: "7".into(),
            ..Emitente::default()
        }
    }

    #[test]
    fn completa_o_que_falta() {
        let mut p = json!({"servico": {"descricao": "x"}});
        aplicar_emitente(&mut p, &emitente());
        assert_eq!(p["prestador"]["documento"], "11222333000181");
        assert_eq!(p["prestador"]["nome"], "OFICINA TESTE");
        assert_eq!(p["prestador"]["im"], "123456");
        assert_eq!(p["prestador"]["op_simples"], 3);
        assert_eq!(p["prestador"]["apuracao_sn"], 1);
        assert_eq!(p["cmun_emissor"], "3550308");
        assert_eq!(p["serie"], "7");
        assert_eq!(p["servico"]["descricao"], "x");
        assert!(p["prestador"].get("regime_especial").is_none());
    }

    #[test]
    fn nunca_sobrescreve_o_que_o_pedido_traz() {
        let mut p = json!({
            "prestador": {"documento": "52998224725", "op_simples": 1, "nome": ""},
            "cmun_emissor": 4106902,
            "serie": 2
        });
        aplicar_emitente(&mut p, &emitente());
        assert_eq!(p["prestador"]["documento"], "52998224725");
        assert_eq!(p["prestador"]["op_simples"], 1);
        assert_eq!(p["cmun_emissor"], 4106902);
        assert_eq!(p["serie"], 2);
        // texto vazio conta como ausente
        assert_eq!(p["prestador"]["nome"], "OFICINA TESTE");
    }

    #[test]
    fn emitente_vazio_so_poe_o_regime() {
        let mut p = json!({});
        aplicar_emitente(&mut p, &Emitente::default());
        assert_eq!(p, json!({"prestador": {"op_simples": 1}}));
    }

    #[test]
    fn pedido_fora_do_formato_fica_como_esta() {
        let mut p = json!([1, 2]);
        aplicar_emitente(&mut p, &emitente());
        assert_eq!(p, json!([1, 2]));
        let mut p = json!({"prestador": "texto"});
        aplicar_emitente(&mut p, &emitente());
        assert_eq!(p["prestador"], "texto");
    }
}
