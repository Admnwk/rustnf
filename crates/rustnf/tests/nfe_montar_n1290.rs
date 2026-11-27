// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Regra N12-90 (rejeicao 934) da NT 2019.001 v1.70: em GO e RJ (quadro 3.6.1) o CST 20, 30, 40, 41, 50, 70 ou 90
//! exige o valor e o motivo da desoneracao; fora ficam a devolucao (E2), o ajuste (E3) e o CST 90 sem reducao de base
//! (excecao 5). Os pedidos saem dos casos de `tests/casos/nfe_montar_deson.json` (emitente em SP), trocando so' a UF
//! do emitente, a finalidade ou o ICMS do item.

use std::path::PathBuf;

use rustnf::nfe::montagem::{montar, nota_de_json, ErroMontagem};
use serde_json::{json, Value};

fn deson() -> Value {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/casos/nfe_montar_deson.json");
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn pedido(nome: &str) -> Value {
    deson()["casos"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["nome"] == nome)
        .unwrap_or_else(|| panic!("caso {nome}"))["entrada"]
        .clone()
}

/// Emitente em Goiania (GO); o destinatario do caso (RJ) segue interestadual.
fn em_go(mut p: Value) -> Value {
    let e = &mut p["emit"]["endereco"];
    e["uf"] = json!("GO");
    e["cod_municipio"] = json!("5208707");
    e["municipio"] = json!("Goiania");
    e["cep"] = json!("74000000");
    p["cmun_fg"] = json!("5208707");
    p
}

/// Emitente no Rio de Janeiro (RJ); o destinatario vai para Sao Paulo para a operacao seguir interestadual.
fn em_rj(mut p: Value) -> Value {
    let e = &mut p["emit"]["endereco"];
    e["uf"] = json!("RJ");
    e["cod_municipio"] = json!("3304557");
    e["municipio"] = json!("Rio de Janeiro");
    e["cep"] = json!("20040002");
    p["cmun_fg"] = json!("3304557");
    let d = &mut p["dest"]["endereco"];
    d["uf"] = json!("SP");
    d["cod_municipio"] = json!("3550308");
    d["municipio"] = json!("Sao Paulo");
    d["cep"] = json!("01001000");
    p
}

fn sem_deson(mut p: Value) -> Value {
    for it in p["itens"].as_array_mut().unwrap() {
        let c = it["icms"].as_object_mut().unwrap();
        c.remove("v_icms_deson");
        c.remove("mot_des_icms");
        c.remove("ind_deduz_deson");
    }
    p
}

fn montar_json(p: &Value) -> Result<(), ErroMontagem> {
    montar(&nota_de_json(p)?).map(|_| ())
}

fn exige_934(p: &Value, nome: &str) {
    let e = montar_json(p).expect_err(nome);
    assert_eq!(
        (e.campo.as_str(), e.regra.as_str()),
        ("vICMSDeson", "934"),
        "{nome}: {e}"
    );
}

fn nao_da_934(p: &Value, nome: &str) {
    if let Err(e) = montar_json(p) {
        assert_ne!(e.regra, "934", "{nome}: {e}");
    }
}

#[test]
fn go_e_rj_exigem_a_desoneracao_nos_cst_da_regra() {
    for caso in [
        "cst40_zero_nao_sai",
        "cst40_deduz",
        "cst41_sem_ind",
        "cst20_reducao",
        "cst30_st",
        "cst70_os_dois",
    ] {
        let p = sem_deson(pedido(caso));
        exige_934(&em_go(p.clone()), &format!("GO {caso}"));
        exige_934(&em_rj(p), &format!("RJ {caso}"));
    }
}

#[test]
fn com_a_desoneracao_informada_monta() {
    for caso in [
        "cst40_deduz",
        "cst41_sem_ind",
        "cst20_reducao",
        "cst70_os_dois",
        "cst90_so_deson",
    ] {
        montar_json(&em_go(pedido(caso))).unwrap_or_else(|e| panic!("GO {caso}: {e}"));
        montar_json(&em_rj(pedido(caso))).unwrap_or_else(|e| panic!("RJ {caso}: {e}"));
    }
}

#[test]
fn outras_uf_nao_aplicam_a_regra() {
    montar_json(&sem_deson(pedido("cst40_deduz"))).unwrap();
    montar_json(&pedido("cst40_zero_nao_sai")).unwrap();
}

#[test]
fn devolucao_e_ajuste_ficam_fora_e_a_complementar_nao() {
    let base = em_go(sem_deson(pedido("cst40_deduz")));
    for fin in [3, 4] {
        let mut p = base.clone();
        p["fin_nfe"] = json!(fin);
        if fin == 4 {
            p["itens"][0]["cfop"] = json!("6202"); // devolucao de compra (CFOP de devolucao, regra 327)
        }
        montar_json(&p).unwrap_or_else(|e| panic!("finNFe {fin}: {e}"));
    }
    let mut p = base;
    p["fin_nfe"] = json!(2);
    exige_934(&p, "finNFe 2");
}

#[test]
fn cst_90_sem_reducao_de_base_fica_fora() {
    let p = em_go(sem_deson(pedido("cst90_so_deson")));
    montar_json(&p).unwrap_or_else(|e| panic!("CST 90 sem reducao: {e}"));
    let mut q = p.clone();
    q["itens"][0]["icms"]["p_red_bc"] = json!("10.00");
    exige_934(&q, "pRedBC");
    // pRedBCST so' vem com o grupo da ST (mesmos campos do caso cst30_st)
    let mut q = p;
    q["itens"][0]["cest"] = json!("0300700");
    q["itens"][0]["cfop"] = json!("6401");
    let c = &mut q["itens"][0]["icms"];
    c["mod_bc_st"] = json!(5);
    c["vbc_st"] = json!("600.00");
    c["p_icms_st"] = json!("20");
    c["v_icms_st"] = json!("120.00");
    montar_json(&q).unwrap_or_else(|e| panic!("CST 90 com ST sem reducao: {e}"));
    q["itens"][0]["icms"]["p_red_bc_st"] = json!("10.00");
    exige_934(&q, "pRedBCST");
}

#[test]
fn cst_fora_da_regra_nao_e_cobrado() {
    nao_da_934(&em_go(pedido("cst10_st_deson")), "CST 10");
}
