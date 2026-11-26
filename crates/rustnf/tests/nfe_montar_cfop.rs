// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! CFOP x finalidade da NF-e 55 pela Tabela de CFOP do Portal: as rejeicoes 327 (I08-140), 1179 (I08-141) e 328 (I08-144)
//! no `montar` e no `conferir`, o CFOP da retencao do transporte (722) e os avisos do CFOP (I08-04 e LA01-20). A base de
//! cada pedido vem dos casos de contrato de `tests/casos`; cada teste muda so' o que a regra olha.

use rustnf::nfe::montagem::{avisos, conferir, montar, nota_de_json, TipoAviso};
use serde_json::{json, Value};

fn caso_de(arquivo: &str, nome: &str) -> Value {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/casos");
    let v: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join(arquivo)).unwrap()).unwrap();
    v["casos"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["nome"] == nome)
        .unwrap_or_else(|| panic!("caso {nome} nao existe em {arquivo}"))["entrada"]
        .clone()
}

/// Devolucao (`finNFe` 4) de SP para o RJ, regime normal, item 1202.
fn devolucao() -> Value {
    caso_de("nfe_montar_fin.json", "devolucao_v153_ref_cabecalho")
}

/// Nota de credito de `tpNFCredito` 03 (retorno), regime normal, item 2202.
fn credito_03() -> Value {
    caso_de("nfe_montar_fin.json", "credito_03_ref_nfce")
}

/// Venda comum (`finNFe` 1), SP, item 5102.
fn venda() -> Value {
    let mut p = caso_de("nfe_montar_transp.json", "transp_completo_por_veiculo");
    p.as_object_mut().unwrap().remove("transp");
    p
}

fn com_cfop(mut p: Value, cfop: &str) -> Value {
    p["itens"][0]["cfop"] = json!(cfop);
    p
}

fn com_ncm(mut p: Value, ncm: &str) -> Value {
    p["itens"][0]["ncm"] = json!(ncm);
    p
}

/// Do `conferir`, so' as linhas das tres rejeicoes.
fn cfop_rej(p: &Value) -> Vec<String> {
    let n = nota_de_json(p).unwrap();
    conferir(&n)
        .into_iter()
        .filter(|l| ["327-", "1179-", "328-"].iter().any(|c| l.starts_with(c)))
        .collect()
}

/// A regra e o campo da recusa do `montar`; `None` = montou.
fn recusa(p: &Value) -> Option<(String, String)> {
    montar(&nota_de_json(p).unwrap())
        .err()
        .map(|e| (e.regra, e.campo))
}

fn codigos(p: &Value) -> Vec<String> {
    cfop_rej(p)
        .iter()
        .map(|l| l.split('-').next().unwrap().to_string())
        .collect()
}

// ------------------------------------------------------------------------------------------------------------ 327

#[test]
fn devolucao_com_cfop_de_devolucao_e_aceita() {
    let p = com_cfop(devolucao(), "1202");
    assert!(cfop_rej(&p).is_empty(), "{:?}", cfop_rej(&p));
    assert_eq!(recusa(&p), None);
}

#[test]
fn devolucao_com_cfop_de_venda_e_recusada_327() {
    let p = com_cfop(devolucao(), "5102");
    assert_eq!(codigos(&p), ["327"]);
    assert!(
        cfop_rej(&p)[0].ends_with("[nItem: 1]"),
        "{:?}",
        cfop_rej(&p)
    );
    assert_eq!(recusa(&p), Some(("327".into(), "CFOP".into())));
}

#[test]
fn cfop_inexistente_na_devolucao_e_327() {
    assert_eq!(codigos(&com_cfop(devolucao(), "1999")), ["327"]);
}

#[test]
fn cfop_1949_e_2949_so_na_devolucao_finnfe_4() {
    // NT 2026.009, Excecao 1: aceitos com finNFe 4 (sem condicao de destinatario)
    for c in ["1949", "2949"] {
        assert_eq!(
            codigos(&com_cfop(devolucao(), c)),
            Vec::<String>::new(),
            "{c}"
        );
        assert_eq!(recusa(&com_cfop(devolucao(), c)), None, "{c}");
    }
    // na nota de credito 03 o 1949 nao passa: a excecao e' so' do finNFe 4
    assert_eq!(codigos(&com_cfop(credito_03(), "1949")), ["327"]);
}

#[test]
fn gas_natural_aceita_5949_e_6949_so_com_o_ncm_27112100() {
    for c in ["5949", "6949"] {
        let ok = com_ncm(com_cfop(devolucao(), c), "27112100");
        assert!(cfop_rej(&ok).is_empty(), "{c}: {:?}", cfop_rej(&ok));
        let outro = com_ncm(com_cfop(devolucao(), c), "09012100");
        assert_eq!(codigos(&outro), ["327"], "{c}");
    }
}

#[test]
fn nota_de_credito_03_e_06_so_com_cfop_de_devolucao() {
    assert!(cfop_rej(&credito_03()).is_empty());
    assert_eq!(codigos(&com_cfop(credito_03(), "5102")), ["327"]);
    let mut p06 = credito_03();
    p06["tp_nf_credito"] = json!("06");
    assert_eq!(codigos(&com_cfop(p06, "5102")), ["327"]);
}

#[test]
fn nota_de_credito_04_nao_cai_na_327() {
    let mut p = credito_03();
    p["tp_nf_credito"] = json!("04");
    assert!(cfop_rej(&com_cfop(p, "5102")).is_empty());
}

// ----------------------------------------------------------------------------------------------------------- 1179

#[test]
fn mei_na_devolucao_so_com_os_seis_cfop_da_i08_141() {
    let mut p = devolucao();
    p["emit"]["crt"] = json!(4);
    // 1411 e' de devolucao (passa a 327) mas nao esta entre os seis
    assert_eq!(codigos(&com_cfop(p.clone(), "1411")), ["1179"]);
    assert_eq!(
        recusa(&com_cfop(p.clone(), "1411")),
        Some(("1179".into(), "CFOP".into()))
    );
    for c in ["1202", "1553", "2202", "2553", "5202", "6202"] {
        assert!(cfop_rej(&com_cfop(p.clone(), c)).is_empty(), "{c}");
    }
    // 1949 passa a 327 e cai na 1179
    assert_eq!(codigos(&com_cfop(p.clone(), "1949")), ["1179"]);
    // operacao com o exterior (idDest 3): a regra nao vale
    p["id_dest"] = json!(3);
    assert!(cfop_rej(&com_cfop(p, "1411")).is_empty());
}

#[test]
fn mei_fora_da_devolucao_nao_cai_na_1179() {
    let mut p = venda();
    p["emit"]["crt"] = json!(4);
    assert!(cfop_rej(&com_cfop(p, "5102")).is_empty());
}

// ------------------------------------------------------------------------------------------------------------ 328

#[test]
fn cfop_de_devolucao_em_venda_e_recusado_328() {
    let p = com_cfop(venda(), "5202");
    assert_eq!(codigos(&p), ["328"]);
    assert_eq!(recusa(&p), Some(("328".into(), "CFOP".into())));
    assert!(cfop_rej(&com_cfop(venda(), "5102")).is_empty());
}

#[test]
fn finnfe_2_e_credito_04_nao_caem_na_328() {
    let mut comp = venda();
    comp["fin_nfe"] = json!(2);
    assert!(!codigos(&com_cfop(comp, "5202")).contains(&"328".to_string()));
    let mut p = credito_03();
    p["tp_nf_credito"] = json!("04");
    assert!(cfop_rej(&com_cfop(p, "1202")).is_empty());
}

#[test]
fn credito_03_com_cfop_de_devolucao_e_aceito() {
    assert!(cfop_rej(&com_cfop(credito_03(), "1202")).is_empty());
}

#[test]
fn as_tres_regras_so_valem_no_modelo_55() {
    let mut p = com_cfop(venda(), "5202");
    p["modelo"] = json!(65);
    assert!(cfop_rej(&p).is_empty());
}

// ------------------------------------------------------------------------------------------------------------ 722

#[test]
fn cfop_da_retencao_do_transporte_so_com_indtransp_1() {
    let base = caso_de("nfe_montar_transp.json", "transp_completo_por_veiculo");
    assert_eq!(recusa(&base), None, "5353 e' indTransp 1");
    for c in ["5102", "5999"] {
        let mut p = base.clone();
        p["transp"]["ret_transp"]["cfop"] = json!(c);
        assert_eq!(
            recusa(&p),
            Some(("722".into(), "retTransp/CFOP".into())),
            "{c}"
        );
    }
}

// ----------------------------------------------------------------------------------------------------------- avisos

fn tipos(p: &Value) -> Vec<TipoAviso> {
    avisos(&nota_de_json(p).unwrap())
        .into_iter()
        .map(|a| a.tipo)
        .collect()
}

#[test]
fn cfop_fora_da_tabela_vira_aviso_e_nao_recusa() {
    let p = com_cfop(venda(), "5999");
    assert_eq!(tipos(&p), [TipoAviso::CfopForaDaTabela]);
    assert_eq!(recusa(&p), None);
    assert!(tipos(&venda()).is_empty());
}

#[test]
fn cfop_sem_vigencia_na_data_da_emissao_vira_aviso() {
    // 5131 comeca em 2018-01-01 na tabela
    let mut p = com_cfop(venda(), "5131");
    p["emissao"] = json!("2017-12-31T10:00:00-03:00");
    assert_eq!(tipos(&p), [TipoAviso::CfopForaDaTabela]);
    p["emissao"] = json!("2018-01-01T10:00:00-03:00");
    assert!(tipos(&p).is_empty());
}

#[test]
fn cfop_de_combustivel_sem_o_grupo_comb_vira_aviso_660() {
    let p = com_cfop(venda(), "5667");
    assert_eq!(tipos(&p), [TipoAviso::CfopCombustivelSemGrupo]);
}
