// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Tabela de `cStat` da NF-e: `tests/casos/nfe_cstat.json`. As mensagens do caso foram conferidas a mao contra o
//! texto do MOC 7.00 (Anexo I, secao 4.4) e da NT 2023.002 v1.02.

use rustnf::nfe::cstat::{classe, consultar, mensagem, normalizar, todos, Classe};
use serde_json::Value;

fn casos() -> Value {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/casos/nfe_cstat.json");
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn da_classe(nome: &str) -> Classe {
    match nome {
        "autorizada" => Classe::Autorizada,
        "cancelada" => Classe::Cancelada,
        "denegada" => Classe::Denegada,
        "rejeitada" => Classe::Rejeitada,
        "processamento" => Classe::Processamento,
        "desconhecida" => Classe::Desconhecida,
        outra => panic!("classe {outra}"),
    }
}

#[test]
fn mensagem_e_origem_conferidas_contra_o_documento() {
    for x in casos()["conferidos"].as_array().unwrap() {
        let cod = x["cstat"].as_str().unwrap();
        let e = consultar(cod).unwrap_or_else(|| panic!("{cod} fora da tabela"));
        assert_eq!(e.mensagem, x["mensagem"].as_str().unwrap(), "{cod}");
        assert_eq!(mensagem(cod), Some(e.mensagem.as_str()));
        assert_eq!(e.origem.as_deref(), x["origem"].as_str(), "{cod}");
    }
}

#[test]
fn classe_de_cada_familia() {
    for x in casos()["classes"].as_array().unwrap() {
        let esperada = da_classe(x["classe"].as_str().unwrap());
        for cod in x["codigos"].as_array().unwrap() {
            assert_eq!(classe(cod.as_str().unwrap()), esperada, "{cod}");
        }
    }
    assert!(classe("100").autorizada() && !classe("110").autorizada());
}

#[test]
fn codigo_fora_da_tabela_e_desconhecido() {
    assert!(consultar("1555").is_none());
    assert_eq!(mensagem("0"), None);
    assert_eq!(classe("123456"), Classe::Desconhecida);
    assert_eq!(classe(""), Classe::Desconhecida);
}

#[test]
fn normalizar_aceita_o_codigo_como_a_sefaz_o_escreve() {
    assert_eq!(normalizar(" 0100 "), "100");
    assert_eq!(classe("0100"), Classe::Autorizada);
    assert_eq!(mensagem("cStat 539"), mensagem("539"));
}

#[test]
fn tamanho_e_contagem_por_classe() {
    let c = casos();
    let tabela: Vec<_> = todos().collect();
    assert_eq!(tabela.len() as u64, c["total"].as_u64().unwrap());
    for (nome, n) in c["contagem"].as_object().unwrap() {
        let achados = tabela
            .iter()
            .filter(|e| e.classe == da_classe(nome))
            .count();
        assert_eq!(achados as u64, n.as_u64().unwrap(), "{nome}");
    }
    let ordenados: Vec<u32> = tabela.iter().map(|e| e.cstat.parse().unwrap()).collect();
    assert!(
        ordenados.windows(2).all(|p| p[0] < p[1]),
        "em ordem numerica, sem repetir"
    );
}

#[test]
fn codigo_escrito_de_dois_jeitos_guarda_as_duas_mensagens() {
    let e = consultar("489").unwrap();
    assert_eq!(
        e.mensagem,
        "Rejeição: CNPJ informado inválido (DV ou zeros)"
    );
    assert_eq!(
        e.outras,
        vec!["Rejeição: CNPJ informado inválido (DV ou zeros) [nItem: 999]".to_string()]
    );
}
