// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Apoio dos testes da NFC-e: os casos de `casos/nfce_montar.json` e o certificado de teste.

#![allow(dead_code)]

use chrono::Utc;
use rustnf::assinatura::cert_a1::{self, Certificado};
use serde_json::Value;

/// O arquivo de casos inteiro.
pub fn arquivo() -> Value {
    serde_json::from_str(include_str!("../casos/nfce_montar.json")).unwrap()
}

/// Troca em `v` o que `altera` manda: chave com pontos, numero indexa lista, o valor entra inteiro.
pub fn aplicar(v: &mut Value, altera: &Value) {
    for (caminho, novo) in altera.as_object().unwrap() {
        let mut no = &mut *v;
        let partes: Vec<&str> = caminho.split('.').collect();
        for (i, p) in partes.iter().enumerate() {
            let ultimo = i + 1 == partes.len();
            no = match p.parse::<usize>() {
                Ok(ix) => {
                    if ultimo {
                        no[ix] = novo.clone();
                        break;
                    }
                    &mut no[ix]
                }
                Err(_) => {
                    if ultimo {
                        no[*p] = novo.clone();
                        break;
                    }
                    &mut no[*p]
                }
            };
        }
    }
}

/// O pedido de um caso ou de uma regra: a base com o `altera` aplicado.
pub fn pedido(a: &Value, altera: &Value) -> Value {
    let mut p = a["base"].clone();
    aplicar(&mut p, altera);
    p
}

/// O certificado A1 de teste do emitente ficticio.
pub fn certificado() -> Certificado {
    let pfx = cert_a1::gerar_teste(
        "11222333000181",
        "LOJA DE TESTE",
        "teste",
        365,
        true,
        false,
        Utc::now(),
    )
    .unwrap();
    cert_a1::carregar(&pfx, "teste").unwrap()
}

/// Guarda o XML numa pasta se `RUSTNF_XML_DE_TESTE` aponta uma: e' como o contrato de XSD fora do repositorio os le.
pub fn guardar(nome: &str, xml: &str) {
    if let Ok(pasta) = std::env::var("RUSTNF_XML_DE_TESTE") {
        let _ = std::fs::create_dir_all(&pasta);
        let _ = std::fs::write(format!("{pasta}/{nome}.xml"), xml);
    }
}
