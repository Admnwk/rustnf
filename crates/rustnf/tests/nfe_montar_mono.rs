// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! ICMS monofasico sobre combustiveis (CST 02, 15, 53 e 61) na NF-e modelo 55. Os casos de XML e de recusa estao em
//! `tests/casos/nfe_montar_mono.json` (rodados por `nfe_montar.rs`); aqui estao o texto exato de cada grupo, as
//! regras que dependem do contexto da nota (tolerancia, finalidade, destinatario), a leitura de volta e o pedido
//! estrito.

use std::path::PathBuf;

use rustnf::nfe::ler;
use rustnf::nfe::montagem::{
    campos_conhecidos, conferir, montar, nota_de_json, nota_de_json_estrito, ErroMontagem, Montada,
};
use serde_json::{json, Value};

fn arquivo() -> Value {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/casos/nfe_montar_mono.json");
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

/// Entrada do caso `nome` do arquivo de casos.
fn entrada(nome: &str) -> Value {
    arquivo()["casos"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["nome"] == nome)
        .unwrap_or_else(|| panic!("caso {nome} nao existe"))["entrada"]
        .clone()
}

fn montar_json(p: &Value) -> Result<Montada, ErroMontagem> {
    montar(&nota_de_json(p)?)
}

fn regra(r: Result<Montada, ErroMontagem>) -> String {
    match r {
        Ok(_) => "montou".into(),
        Err(e) if e.regra.is_empty() => format!("sem rejeicao ({})", e.campo),
        Err(e) => format!("{} ({})", e.regra, e.campo),
    }
}

/// O primeiro grupo `<ICMS>..</ICMS>` do XML.
fn icms(xml: &str) -> &str {
    let i = xml.find("<ICMS>").unwrap();
    let f = xml.find("</ICMS>").unwrap() + "</ICMS>".len();
    &xml[i..f]
}

fn icms_tot(xml: &str) -> &str {
    let i = xml.find("<ICMSTot>").unwrap();
    let f = xml.find("</ICMSTot>").unwrap();
    &xml[i..f]
}

// ------------------------------------------------------------------------------- o texto de cada grupo (XSD PL_010h)

#[test]
fn cst_02_sai_com_quantidade_aliquota_e_valor() {
    let m = montar_json(&entrada("mono_02_gasolina")).unwrap();
    assert_eq!(
        icms(&m.xml),
        "<ICMS><ICMS02><orig>0</orig><CST>02</CST><qBCMono>1000.0000</qBCMono><adRemICMS>1.3721</adRemICMS>\
         <vICMSMono>1372.10</vICMSMono></ICMS02></ICMS>"
    );
}

#[test]
fn cst_15_leva_a_retencao_e_a_reducao_na_ordem_do_xsd() {
    let m = montar_json(&entrada("mono_15_sem_bio_com_reducao")).unwrap();
    assert_eq!(
        icms(&m.xml),
        "<ICMS><ICMS15><orig>0</orig><CST>15</CST><qBCMono>900.0000</qBCMono><adRemICMS>1.3721</adRemICMS>\
         <vICMSMono>1234.89</vICMSMono><qBCMonoReten>100.0000</qBCMonoReten><adRemICMSReten>0.2000</adRemICMSReten>\
         <vICMSMonoReten>20.00</vICMSMonoReten><pRedAdRem>25.50</pRedAdRem><motRedAdRem>1</motRedAdRem></ICMS15></ICMS>"
    );
    // sem a reducao, os dois campos do N46 nao saem
    let m = montar_json(&entrada("mono_15_diesel_b_bio")).unwrap();
    assert!(!icms(&m.xml).contains("pRedAdRem") && !icms(&m.xml).contains("motRedAdRem"));
}

#[test]
fn cst_53_so_leva_o_que_tem_valor_e_o_diferimento_total_zera_o_devido() {
    let m = montar_json(&entrada("mono_53_diferimento_parcial")).unwrap();
    assert_eq!(
        icms(&m.xml),
        "<ICMS><ICMS53><orig>0</orig><CST>53</CST><qBCMono>1000.0000</qBCMono><adRemICMS>1.3721</adRemICMS>\
         <vICMSMonoOp>1372.10</vICMSMonoOp><pDif>40.5000</pDif><vICMSMonoDif>555.70</vICMSMonoDif>\
         <vICMSMono>816.40</vICMSMono></ICMS53></ICMS>"
    );
    // diferimento total: o devido e' zero e a tag nao sai; o total tambem nao
    let m = montar_json(&entrada("mono_53_diferimento_total")).unwrap();
    assert!(!icms(&m.xml).contains("<vICMSMono>"));
    assert!(icms(&m.xml).contains("<pDif>100.0000</pDif>"));
    assert!(!icms_tot(&m.xml).contains("vICMSMono"));
    assert!(!m.totais.contains_key("vICMSMono"));
}

#[test]
fn cst_61_leva_a_quantidade_retida_anteriormente() {
    let m = montar_json(&entrada("mono_61_gasolina")).unwrap();
    assert_eq!(
        icms(&m.xml),
        "<ICMS><ICMS61><orig>0</orig><CST>61</CST><qBCMonoRet>1000.0000</qBCMonoRet><adRemICMSRet>1.3721</adRemICMSRet>\
         <vICMSMonoRet>1372.10</vICMSMonoRet></ICMS61></ICMS>"
    );
}

// ------------------------------------------------------------------------------------------------------ totais e vNF

#[test]
fn os_totais_somam_os_itens_e_so_saem_com_valor_na_ordem_do_xsd() {
    let m = montar_json(&entrada("mono_soma_de_3_itens")).unwrap();
    let t = icms_tot(&m.xml);
    let ordem: Vec<usize> = [
        "<vFCPSTRet>",
        "<qBCMono>40.13</qBCMono>",
        "<vICMSMono>39.71</vICMSMono>",
        "<qBCMonoReten>10.00</qBCMonoReten>",
        "<vICMSMonoReten>3.15</vICMSMonoReten>",
        "<qBCMonoRet>20.50</qBCMonoRet>",
        "<vICMSMonoRet>22.96</vICMSMonoRet>",
        "<vProd>",
    ]
    .iter()
    .map(|s| t.find(s).unwrap_or_else(|| panic!("falta {s} em {t}")))
    .collect();
    assert!(ordem.windows(2).all(|w| w[0] < w[1]), "{t}");
    assert_eq!(m.totais["vICMSMonoRet"], "22.96");
    // sem item monofasico nenhum dos seis totais aparece
    let mut p = entrada("mono_02_gasolina");
    p["itens"][0]["icms"] =
        json!({"codigo": "00", "vbc": "5500", "p_icms": "18", "v_icms": "990.00"});
    let m = montar_json(&p).unwrap();
    for tag in [
        "qBCMono",
        "vICMSMono",
        "qBCMonoReten",
        "vICMSMonoReten",
        "qBCMonoRet",
        "vICMSMonoRet",
    ] {
        assert!(!icms_tot(&m.xml).contains(tag), "{tag}");
        assert!(!m.totais.contains_key(tag), "{tag}");
    }
}

/// W16-10: so' o `vICMSMonoReten` soma no `vNF`; o ICMS proprio (02) e o retido anteriormente (61) nao somam.
#[test]
fn so_o_icms_com_retencao_soma_no_vnf_e_no_vitem() {
    let m = montar_json(&entrada("mono_15_diesel_b_bio")).unwrap();
    assert_eq!(m.totais["vProd"], "5500.00");
    assert_eq!(m.totais["vNF"], "5547.28");
    for nome in [
        "mono_02_gasolina",
        "mono_53_diferimento_parcial",
        "mono_61_gasolina",
    ] {
        let m = montar_json(&entrada(nome)).unwrap();
        assert_eq!(m.totais["vNF"], "5500.00", "{nome}");
    }
    // VB01-10: o vItem do IBS/CBS soma a retencao
    let m = montar_json(&entrada("mono_15_com_ibscbs_vitem_soma_a_retencao")).unwrap();
    assert!(m.xml.contains("<vItem>5547.28</vItem>"), "{}", m.xml);
}

// ------------------------------------------------------------------------------------------- regras com contexto

/// (*4) da NT: o produto e' arredondado a 2 casas e a diferenca aceita e' de 0,01.
#[test]
fn a_tolerancia_da_962_conta_do_valor_arredondado() {
    let mut p = entrada("mono_02_gasolina");
    p["itens"][0]["quantidade"] = json!("10.1255");
    p["itens"][0]["valor_unitario"] = json!("5.50");
    p["pagamentos"] = json!([{"t_pag": "01", "valor": "55.69"}]);
    p["itens"][0]["icms"]["q_bc_mono"] = json!("10.1255");
    p["itens"][0]["icms"]["ad_rem_icms"] = json!("1.12");
    // 10,1255 x 1,12 = 11,34056 -> 11,34
    for (v, esperado) in [
        ("11.34", "montou"),
        ("11.33", "montou"),
        ("11.35", "montou"),
        ("11.32", "962 (vICMSMono)"),
        ("11.36", "962 (vICMSMono)"),
    ] {
        p["itens"][0]["icms"]["v_icms_mono"] = json!(v);
        assert_eq!(regra(montar_json(&p)), esperado, "{v}");
    }
}

/// N39-10 e N41-x so' conferem a conta na nota normal (`finNFe` 1): na de ajuste o valor vem do documento de origem.
#[test]
fn a_962_e_a_964_so_valem_na_nota_normal() {
    let mut p = entrada("mono_15_diesel_b_bio");
    p["itens"][0]["icms"]["v_icms_mono"] = json!("1000.00");
    p["itens"][0]["icms"]["v_icms_mono_reten"] = json!("40.00");
    assert_eq!(regra(montar_json(&p)), "962 (vICMSMono)");
    p["fin_nfe"] = json!(3);
    assert_eq!(regra(montar_json(&p)), "montou");
    // as obrigatorias (767, 768, 769) continuam valendo
    p["itens"][0]["icms"]
        .as_object_mut()
        .unwrap()
        .remove("q_bc_mono_reten");
    assert_eq!(regra(montar_json(&p)), "768 (qBCMonoReten)");
}

/// N12-70, excecao 1: a regra nao vale para a nota de entrada (`tpNF` 0).
#[test]
fn a_508_nao_vale_na_nota_de_entrada() {
    let mut p = entrada("mono_15_diesel_b_bio");
    p["dest"]["ind_ie_dest"] = json!(9);
    assert_eq!(regra(montar_json(&p)), "508 (CST)");
    p["tp_nf"] = json!(0);
    assert_eq!(regra(montar_json(&p)), "montou");
}

/// N12-20 (590), excecao da NT 2023.001: o CRT 1 informa o CST 02, 15, 53 e 61; os demais CST seguem recusados.
#[test]
fn o_simples_informa_so_o_cst_monofasico() {
    let mut p = entrada("mono_61_simples_crt1");
    assert_eq!(regra(montar_json(&p)), "montou");
    p["itens"][0]["icms"] =
        json!({"codigo": "00", "vbc": "5500", "p_icms": "18", "v_icms": "990.00"});
    assert_eq!(regra(montar_json(&p)), "sem rejeicao (CSOSN)");
}

/// Diferenca da origem: o XSD (`TDec_0302Max100`) e a NT aceitam o percentual de reducao zero, junto do motivo; nao
/// ha rejeicao para isso.
#[test]
fn percentual_de_reducao_zero_vale_como_no_xsd() {
    let m = montar_json(&entrada("mono_15_reducao_zero_motivo_9")).unwrap();
    assert!(icms(&m.xml).contains("<pRedAdRem>0.00</pRedAdRem><motRedAdRem>9</motRedAdRem>"));
}

#[test]
fn a_conferencia_da_nota_traz_a_recusa_da_regra_com_o_codigo() {
    let mut p = entrada("mono_02_gasolina");
    p["itens"][0]["icms"]["v_icms_mono"] = json!("1.00");
    let lista = conferir(&nota_de_json(&p).unwrap());
    assert_eq!(lista.len(), 1, "{lista:?}");
    assert!(
        lista[0].starts_with("entrada: ") && lista[0].contains("N39-10"),
        "{lista:?}"
    );
}

// --------------------------------------------------------------------------------------- determinismo e leitura

#[test]
fn a_mesma_entrada_da_o_mesmo_xml() {
    for c in arquivo()["casos"].as_array().unwrap() {
        let nota = nota_de_json(&c["entrada"]).unwrap();
        let (a, b) = (montar(&nota).unwrap(), montar(&nota).unwrap());
        assert_eq!(a.xml, b.xml, "{}", c["nome"]);
        assert_eq!(a.chave, b.chave, "{}", c["nome"]);
    }
}

#[test]
fn o_xml_do_monofasico_volta_pela_leitura() {
    let m = montar_json(&entrada("mono_soma_de_3_itens")).unwrap();
    let lida = ler(m.xml.as_bytes()).unwrap();
    let grupos: Vec<_> = lida
        .itens
        .iter()
        .map(|i| i.imposto.icms.as_ref().unwrap())
        .collect();
    assert_eq!(
        grupos.iter().map(|g| g.nome.as_str()).collect::<Vec<_>>(),
        ["ICMS02", "ICMS61", "ICMS15"]
    );
    assert_eq!(grupos[0].texto("CST"), "02");
    assert_eq!(grupos[0].valor("qBCMono").to_string(), "10.1255");
    assert_eq!(grupos[0].valor("vICMSMono").to_string(), "11.34");
    assert_eq!(grupos[1].valor("vICMSMonoRet").to_string(), "22.96");
    assert_eq!(grupos[2].valor("vICMSMonoReten").to_string(), "3.15");
    for (tag, v) in [
        ("qBCMono", "40.13"),
        ("vICMSMono", "39.71"),
        ("qBCMonoReten", "10.00"),
        ("vICMSMonoReten", "3.15"),
        ("qBCMonoRet", "20.50"),
        ("vICMSMonoRet", "22.96"),
        ("vNF", "391.59"),
    ] {
        assert_eq!(lida.total.v(tag).to_string(), v, "{tag}");
    }
}

// ------------------------------------------------------------------------------------------------- pedido estrito

#[test]
fn o_pedido_estrito_conhece_os_campos_novos_e_recusa_os_errados() {
    for c in arquivo()["casos"].as_array().unwrap() {
        assert!(nota_de_json_estrito(&c["entrada"]).is_ok(), "{}", c["nome"]);
    }
    let conhecidos = campos_conhecidos();
    for campo in [
        "q_bc_mono",
        "ad_rem_icms",
        "v_icms_mono",
        "q_bc_mono_reten",
        "ad_rem_icms_reten",
        "v_icms_mono_reten",
        "p_red_ad_rem",
        "mot_red_ad_rem",
        "v_icms_mono_op",
        "v_icms_mono_dif",
        "q_bc_mono_ret",
        "ad_rem_icms_ret",
        "v_icms_mono_ret",
    ] {
        assert!(
            conhecidos.contains(&format!("itens[].icms.{campo}")),
            "{campo}"
        );
    }
    let mut p = entrada("mono_02_gasolina");
    p["itens"][0]["icms"]["q_bc_monoo"] = json!("1");
    let e = nota_de_json_estrito(&p).unwrap_err();
    assert_eq!(e.campo, "itens[1].icms.q_bc_monoo");
}
