// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! As `[[aliquotas]]` do arquivo de configuracao alimentam a previa do IBS/CBS.

use rustnf_app::config::Config;
use rustnf_app::nucleo::{calcular, calcular_com, ler_operacao};

const PEDIDO_2027: &str = r#"{"tributos": {"data": "2027-06-01", "cst": "000", "cclass_trib": "000001", "base": "100.00"}}"#;

const ARQUIVO: &str = r#"
[[aliquotas]]
ano = 2027
cbs_referencia = "8.8"
fonte = "exemplo: norma ficticia de aliquotas"
"#;

#[test]
fn o_arquivo_vira_as_publicadas_da_previa() {
    let c = Config::de_texto(ARQUIVO).unwrap();
    c.validar().unwrap();
    let p = c.publicadas();
    assert_eq!(p.len(), 1);
    let a = &p[&2027];
    assert_eq!(a.cbs_referencia.as_deref(), Some("8.8"));
    assert_eq!(a.cbs, None);
    assert_eq!(a.ibs_uf, None);
    assert_eq!(
        a.fonte.as_deref(),
        Some("exemplo: norma ficticia de aliquotas")
    );
    assert!(Config::default().publicadas().is_empty());
}

#[test]
fn ano_de_2027_sem_o_arquivo_nao_tem_o_numero_publicado_e_com_ele_calcula() {
    let op = ler_operacao(PEDIDO_2027).unwrap();
    let erro = calcular(&op).unwrap_err();
    let publicadas = Config::de_texto(ARQUIVO).unwrap().publicadas();
    let com = calcular_com(&op, &publicadas).unwrap();

    // sem o arquivo a previa recusa: o numero publicado da CBS de 2027 nao foi informado
    assert_eq!(erro.codigo_saida(), 3, "{erro}");
    assert!(erro.to_string().contains("2027"), "{erro}");
    // com ele, 8,8 - 0,1 = 8,7
    assert_eq!(com["tributos"]["cbs"]["aliquota"], "8.70", "{com}");
    assert_eq!(com["tributos"]["cbs"]["devido"], "8.70", "{com}");
    assert_eq!(com["recusado"], false);
}

#[test]
fn o_que_o_pedido_traz_vale_sobre_o_arquivo() {
    let op = ler_operacao(
        r#"{"tributos": {"data": "2027-06-01", "cst": "000", "cclass_trib": "000001", "base": "100.00",
            "publicadas": {"2027": {"cbs_referencia": "10.0"}}}}"#,
    )
    .unwrap();
    let publicadas = Config::de_texto(ARQUIVO).unwrap().publicadas();
    let v = calcular_com(&op, &publicadas).unwrap();
    assert_eq!(v["tributos"]["cbs"]["aliquota"], "9.90", "{v}");
}
