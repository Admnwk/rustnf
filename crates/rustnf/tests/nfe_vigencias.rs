// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Calendario das Notas Tecnicas: `tests/casos/nfe_vigencias.json`. As datas do caso foram lidas a mao no
//! "Historico de Alteracoes / Cronograma" de cada NT, nao copiadas da tabela da biblioteca.

use chrono::NaiveDate;
use rustnf::nfe::vigencias::{buscar, inicio, listar, vigente, Ambiente};
use serde_json::Value;

fn casos() -> Value {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/casos/nfe_vigencias.json");
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn dia(v: &Value) -> NaiveDate {
    NaiveDate::parse_from_str(v.as_str().unwrap(), "%Y-%m-%d").unwrap()
}

#[test]
fn datas_do_cronograma_de_cada_nt() {
    let c = casos();
    let linhas = c["linhas"].as_array().unwrap();
    assert_eq!(
        linhas.len(),
        listar().len(),
        "toda linha da tabela tem caso"
    );
    for x in linhas {
        let chave = x["chave"].as_str().unwrap();
        let v = buscar(chave).unwrap_or_else(|| panic!("{chave} fora da tabela"));
        assert_eq!(v.nt, x["nt"].as_str().unwrap(), "{chave}");
        assert_eq!(
            inicio(chave, Ambiente::Producao),
            Some(dia(&x["producao"])),
            "{chave}: producao"
        );
        let homologacao = if x["homologacao"].is_null() {
            dia(&x["producao"])
        } else {
            dia(&x["homologacao"])
        };
        assert_eq!(
            inicio(chave, Ambiente::Homologacao),
            Some(homologacao),
            "{chave}: homologacao"
        );
    }
}

#[test]
fn vigente_inclui_o_primeiro_dia() {
    let d = |s: &str| NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap();
    let k = "nt2025_002.v1_53";
    assert_eq!(vigente(k, d("2026-11-15"), Ambiente::Producao), Some(false));
    assert_eq!(vigente(k, d("2026-11-16"), Ambiente::Producao), Some(true));
    assert_eq!(
        vigente(k, d("2026-11-09"), Ambiente::Homologacao),
        Some(true)
    );
    assert_eq!(
        vigente(k, d("2026-11-08"), Ambiente::Homologacao),
        Some(false)
    );
    // sem data de homologacao propria vale a de producao
    assert_eq!(
        inicio("nt2026_010.danfe_rtc", Ambiente::Homologacao),
        Some(d("2026-12-01"))
    );
}

#[test]
fn chave_desconhecida_nao_inventa_data() {
    let d = NaiveDate::from_ymd_opt(2030, 1, 1).unwrap();
    assert_eq!(inicio("nao_existe", Ambiente::Producao), None);
    assert_eq!(vigente("nao_existe", d, Ambiente::Producao), None);
    assert!(buscar("nao_existe").is_none());
}

#[test]
fn listar_vem_em_ordem_de_producao() {
    let l = listar();
    assert!(l
        .windows(2)
        .all(|p| (p[0].producao, p[0].chave) <= (p[1].producao, p[1].chave)));
    assert_eq!(l.first().map(|v| v.chave), Some("nt2021_003.grupo_i"));
    assert_eq!(l.last().map(|v| v.chave), Some("nt2021_003.grupo_v"));
}
