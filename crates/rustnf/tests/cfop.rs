// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Tabela de CFOP do Portal da NF-e (`dados/cfop.json`, indicadores da IT 2023.002 v2.00): contagem, registros
//! conferidos a mao contra a planilha oficial, vigencia e o par venda -> devolucao pela descricao oficial.

use chrono::NaiveDate;
use rustnf::cfop;

fn d(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
}

#[test]
fn a_tabela_tem_os_619_cfop_e_105_de_devolucao() {
    assert_eq!(cfop::tabela().len(), 619);
    assert_eq!(
        cfop::tabela().values().filter(|r| r.ind_devol == 1).count(),
        105
    );
    assert_eq!(cfop::versao(), "2.00");
}

#[test]
fn registros_conferidos_a_mao_contra_a_planilha() {
    let r = cfop::info("1202").unwrap();
    assert_eq!((r.ind_nfe, r.ind_devol, r.ind_retor), (1, 1, 0));
    assert!(r
        .titulo
        .starts_with("Devolução de venda de mercadoria adquirida"));
    assert_eq!(r.inicio.as_deref(), Some("2006-01-01"));
    assert_eq!(r.fim, None);
    let r = cfop::info("5405").unwrap();
    assert_eq!((r.ind_nfe, r.ind_devol), (1, 0));
    // CFOP de combustivel: indComb 1 e 2 como vieram
    assert_eq!(cfop::info("5667").unwrap().ind_comb, 1);
    assert_eq!(cfop::info("5651").unwrap().ind_comb, 2);
    // transporte: indTransp 1
    assert_eq!(cfop::info("5353").unwrap().ind_transp, 1);
    assert_eq!(cfop::info("5102").unwrap().ind_transp, 0);
    // vigencia que comecou depois de 2006
    assert_eq!(
        cfop::info("5131").unwrap().inicio.as_deref(),
        Some("2018-01-01")
    );
}

#[test]
fn aceita_o_codigo_com_ponto_e_o_inexistente_e_nada() {
    assert_eq!(cfop::info("5.102"), cfop::info("5102"));
    assert!(cfop::info("5102").is_some());
    assert!(cfop::info("9999").is_none());
    assert!(cfop::info("").is_none());
}

#[test]
fn devolucao_e_vigencia() {
    assert!(cfop::e_devolucao("1202") && cfop::e_devolucao("1.202"));
    assert!(!cfop::e_devolucao("1949") && !cfop::e_devolucao("5102") && !cfop::e_devolucao("9999"));
    assert!(cfop::CFOP_DEVOLUCAO_MEI
        .iter()
        .all(|c| cfop::e_devolucao(c)));
    assert!(cfop::vigente("5102", d("2026-11-26")));
    assert!(!cfop::vigente("5102", d("2005-12-31")));
    assert!(!cfop::vigente("5131", d("2017-12-31")) && cfop::vigente("5131", d("2018-01-01")));
    assert!(!cfop::vigente("9999", d("2026-11-26")));
}

#[test]
fn pares_de_devolucao_pela_descricao_oficial() {
    assert_eq!(cfop::devolucoes_de("5405"), ["1411"]);
    assert_eq!(cfop::devolucoes_de("5.102"), ["1202"]);
    assert_eq!(cfop::origens("1202"), ["5102", "5104", "5106"]);
    assert_eq!(cfop::origens("1411"), ["5403", "5405"]);
    assert_eq!(cfop::origens("1553"), ["5551"]);
    // os codigos de exclusao da descricao do 1202 (5501, 5502 ...) NAO sao origem
    assert!(!cfop::origens("1202").contains(&"5501".to_string()));
    // a descricao do CFOP de devolucao que nao cita origem: lista vazia
    assert!(cfop::origens("1918").is_empty() && cfop::origens("5921").is_empty());
    // CFOP que nao e de devolucao nao tem origem
    assert!(cfop::origens("5102").is_empty());
    assert!(cfop::devolucoes_de("1949").is_empty());
}
