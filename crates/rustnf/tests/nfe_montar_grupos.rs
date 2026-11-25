// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Montagem da NF-e (modelo 55): o DIFAL com a entrega e o combustivel, e os grupos que completam a nota (cobranca,
//! transporte, exportacao, compra), mais o canal de avisos. Os casos de XML e de recusa por regra ficam em
//! `tests/casos/nfe_montar_*.json`; aqui estao as coisas que um caso de contrato nao diz: a regra que depende do
//! contexto da nota inteira, a leitura de volta e a lista de avisos.

use rustnf::nfe::montagem::{montar, nota_de_json, ErroMontagem, Montada};
use serde_json::{json, Value};

fn montar_json(p: &Value) -> Result<Montada, ErroMontagem> {
    montar(&nota_de_json(p)?)
}

/// Venda interestadual de SP para a BA a consumidor final nao contribuinte, regime normal, com o DIFAL calculado.
fn pedido_difal() -> Value {
    json!({
        "modelo": 55, "serie": 1, "numero": 7001, "emissao": "2026-11-16T10:00:00-03:00",
        "nat_op": "VENDA A CONSUMIDOR", "cmun_fg": "3550308", "codigo": 87654321,
        "emit": {"documento": "11222333000181", "nome": "INDUSTRIA TESTE S/A", "crt": 3, "ie": "111111111111",
                 "endereco": {"logradouro": "Rua das Flores", "numero": "100", "bairro": "Centro",
                              "cod_municipio": "3550308", "municipio": "Sao Paulo", "uf": "SP", "cep": "01001000"}},
        "id_dest": 2, "ind_final": 1, "ind_pres": 1, "tp_nf": 1, "fin_nfe": 1, "ver_proc": "rustnf teste",
        "dest": {"documento": "12345678909", "nome": "MARIA CONSUMIDORA", "ind_ie_dest": 9,
                 "endereco": {"logradouro": "Rua Chile", "numero": "10", "bairro": "Centro",
                              "cod_municipio": "2927408", "municipio": "Salvador", "uf": "BA", "cep": "40020000"}},
        "itens": [{"codigo": "P1", "descricao": "Cadeira de escritorio", "ncm": "94013000", "cfop": "6108",
                   "unidade": "UN", "quantidade": "1", "valor_unitario": "1000",
                   "icms": {"codigo": "00", "orig": 0, "vbc": "1000", "p_icms": "7", "v_icms": "70.00"},
                   "pis": {"cst": "49"}, "cofins": {"cst": "49"},
                   "icms_uf_dest": {"vbc": 100000, "aliquota_interna": "18", "aliquota_inter": "7", "ano": 2026,
                                    "fcp": "2"}}],
        "pagamentos": [{"t_pag": "01", "valor": "1000.00"}]
    })
}

fn sem_difal(mut p: Value) -> Value {
    p["itens"][0]
        .as_object_mut()
        .unwrap()
        .remove("icms_uf_dest");
    p
}

fn com_entrega(mut p: Value, uf: &str) -> Value {
    p["entrega"] = json!({
        "documento": "52998224725",
        "endereco": {"logradouro": "Rua B", "numero": "2", "bairro": "Centro", "cod_municipio": "3550308",
                     "municipio": "Sao Paulo", "uf": uf}
    });
    p
}

fn com_combustivel(mut p: Value, anp: &str, descricao: &str) -> Value {
    p["itens"][0]["combustivel"] =
        json!({"c_prod_anp": anp, "desc_anp": descricao, "uf_cons": "BA"});
    p
}

fn regra(r: Result<Montada, ErroMontagem>) -> String {
    match r {
        Ok(_) => "montou".into(),
        Err(e) => format!("{} ({})", e.regra, e.campo),
    }
}

// ----------------------------------------------------------------------------------------------- DIFAL (694 e 695)

/// Controle: sem o grupo e sem excecao, a venda interestadual a nao contribuinte e' a rejeicao 694.
#[test]
fn difal_controle_sem_o_grupo_e_a_694() {
    assert_eq!(
        regra(montar_json(&sem_difal(pedido_difal()))),
        "694 (ICMSUFDest)"
    );
    assert_eq!(regra(montar_json(&pedido_difal())), "montou");
}

/// 694, excecao 7: o local de entrega na UF do emitente dispensa o grupo.
#[test]
fn difal_entrega_na_uf_do_emitente_dispensa_o_grupo() {
    let p = com_entrega(sem_difal(pedido_difal()), "SP");
    assert_eq!(regra(montar_json(&p)), "montou");
}

/// 694 continua valendo com a entrega em outra UF que nao a do emitente (a excecao 7 e' so' da UF do emitente).
#[test]
fn difal_entrega_em_outra_uf_continua_exigindo_o_grupo() {
    let p = com_entrega(sem_difal(pedido_difal()), "BA");
    assert_eq!(regra(montar_json(&p)), "694 (ICMSUFDest)");
}

/// 695: fora da venda interestadual o grupo e' indevido; a excecao 2 libera quando a entrega e' em UF diferente da do
/// emitente.
#[test]
fn difal_entrega_em_uf_diferente_libera_o_grupo_na_operacao_interna() {
    let mut p = pedido_difal();
    p["id_dest"] = json!(1);
    p["dest"]["endereco"]["uf"] = json!("SP");
    p["itens"][0]["cfop"] = json!("5108");
    assert_eq!(regra(montar_json(&p)), "695 (ICMSUFDest)");
    let com = com_entrega(p, "BA");
    assert_eq!(regra(montar_json(&com)), "montou");
}

/// 694, excecao 6: combustivel derivado de petroleo fora da lista de codigos da ANP nao leva o grupo; o da lista leva.
#[test]
fn difal_combustivel_derivado_dispensa_o_grupo() {
    let gasolina = com_combustivel(sem_difal(pedido_difal()), "320102001", "GASOLINA C COMUM");
    assert_eq!(regra(montar_json(&gasolina)), "montou");
    let etanol = com_combustivel(
        sem_difal(pedido_difal()),
        "810101001",
        "ETANOL HIDRATADO COMUM",
    );
    assert_eq!(regra(montar_json(&etanol)), "694 (ICMSUFDest)");
}

/// 695: o grupo e' indevido no combustivel derivado de petroleo fora da lista; no da lista e' permitido.
#[test]
fn difal_combustivel_derivado_proibe_o_grupo() {
    let gasolina = com_combustivel(pedido_difal(), "320102001", "GASOLINA C COMUM");
    assert_eq!(regra(montar_json(&gasolina)), "695 (ICMSUFDest)");
    let etanol = com_combustivel(pedido_difal(), "810101001", "ETANOL HIDRATADO COMUM");
    assert_eq!(regra(montar_json(&etanol)), "montou");
}

// ------------------------------------------------------------------------------------------ ida e volta pela leitura

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

fn lida(arquivo: &str, nome: &str) -> rustnf::nfe::Nfe {
    let m = montar_json(&caso_de(arquivo, nome)).unwrap();
    rustnf::nfe::ler(m.xml.as_bytes()).unwrap()
}

#[test]
fn combustivel_volta_pela_leitura() {
    let l = lida("nfe_montar_comb.json", "gasolina_encerrante_cide_origens");
    let c = &l.itens[0].combustivel;
    assert_eq!(c["cProdANP"], "320102001");
    assert_eq!(c["UFCons"], "SP");
    assert_eq!(c["CIDE/vCIDE"], "5.00");
    assert_eq!(c["encerrante/vEncIni"], "1000.500");
    assert_eq!(c["encerrante/vEncFin"], "1050.500");
    assert_eq!(c["origComb/cUFOrig"], "35");
    assert_eq!(c["origComb/cUFOrig[2]"], "41");
    assert_eq!(c["origComb/indImport[2]"], "1");
}

#[test]
fn medicamento_e_rastro_voltam_pela_leitura() {
    let l = lida("nfe_montar_produto.json", "med_com_dois_lotes");
    let item = &l.itens[0];
    assert_eq!(item.medicamentos.len(), 1);
    assert_eq!(item.medicamentos[0]["cProdANVISA"], "1234567890123");
    assert_eq!(item.medicamentos[0]["vPMC"], "19.90");
    assert_eq!(item.rastros.len(), 2);
    assert_eq!(item.rastros[0].lote, "L001");
    assert_eq!(item.rastros[0].agregacao, "AG1");
    assert_eq!(
        item.rastros[0].fabricacao.unwrap().to_string(),
        "2026-01-10"
    );
    assert_eq!(item.rastros[1].validade.unwrap().to_string(), "2028-01-31");
    assert_eq!(item.rastros[1].quantidade.unwrap().to_string(), "1.500");
}

#[test]
fn transporte_volta_pela_leitura() {
    let l = lida("nfe_montar_transp.json", "transp_completo_por_veiculo");
    let t = l.transporte.unwrap();
    assert_eq!(t.modalidade_frete, "0");
    let tr = t.transportadora.unwrap();
    assert_eq!(tr.documento, "11222333000181");
    assert_eq!(tr.ie, "ISENTO");
    assert_eq!(t.retencao_icms["CFOP"], "5353");
    assert_eq!(t.veiculo["placa"], "ABC1234");
    assert_eq!(t.reboques.len(), 2);
    assert_eq!(t.reboques[1]["placa"], "GHI1J23");
    assert_eq!(t.volumes.len(), 2);
    assert_eq!(
        t.volumes[0].lacres,
        vec!["L1".to_string(), "22".to_string()]
    );
    assert_eq!(t.volumes[0].peso_liquido.unwrap().to_string(), "100.500");
}

#[test]
fn cobranca_exportacao_e_compra_voltam_pela_leitura() {
    let l = lida(
        "nfe_montar_grupos.json",
        "transporte_cobranca_pagamento_exportacao_e_compra",
    );
    let c = l.cobranca.as_ref().unwrap();
    assert_eq!(c.fatura_numero, "F-9");
    assert_eq!(c.fatura_liquido.unwrap().to_string(), "100.00");
    assert_eq!(c.duplicatas.len(), 1);
    assert_eq!(c.duplicatas[0].numero, "001");
    assert_eq!(
        c.duplicatas[0].vencimento.unwrap().to_string(),
        "2026-11-03"
    );
    assert_eq!(l.exportacao["UFSaidaPais"], "SP");
    assert_eq!(l.exportacao["xLocDespacho"], "ARMAZEM 4");
    assert_eq!(l.compra["xNEmp"], "NE-1");
    // a conferencia interna da leitura (soma das duplicatas contra a fatura, totais) nao acha divergencia
    let mut d = rustnf::nfe::conferir(&l);
    d.retain(|x| !x.starts_with("nota de homologacao"));
    assert!(d.is_empty(), "{d:?}");
}

#[test]
fn a_mesma_entrada_dos_grupos_novos_da_o_mesmo_xml() {
    for (arq, nome) in [
        ("nfe_montar_comb.json", "glp_com_gas_natural_e_origem"),
        ("nfe_montar_produto.json", "med_com_dois_lotes"),
        ("nfe_montar_transp.json", "transp_completo_por_veiculo"),
        ("nfe_montar_cobr.json", "cobr_tres_parcelas"),
        (
            "nfe_montar_grupos.json",
            "transporte_cobranca_pagamento_exportacao_e_compra",
        ),
    ] {
        let p = caso_de(arq, nome);
        assert_eq!(
            montar_json(&p).unwrap().xml,
            montar_json(&p).unwrap().xml,
            "{nome}"
        );
    }
}

// ------------------------------------------------------------------------------- limites e excecoes das regras

/// O pedido de venda interna de um medicamento sem rastro, para variar as excecoes da regra 873.
fn med_sem_rastro() -> Value {
    let mut p = caso_de("nfe_montar_produto.json", "med_com_dois_lotes");
    p["itens"][0].as_object_mut().unwrap().remove("rastro");
    p
}

#[test]
fn medicamento_sem_rastro_e_a_873_e_as_excecoes_da_regra_a_dispensam() {
    assert_eq!(regra(montar_json(&med_sem_rastro())), "873 (rastro)");
    let mut devolucao = med_sem_rastro();
    devolucao["fin_nfe"] = json!(4);
    let mut nao_presencial = med_sem_rastro();
    nao_presencial["ind_pres"] = json!(2);
    let mut simplificado = med_sem_rastro();
    simplificado["tp_imp"] = json!(6);
    for (nome, p) in [
        ("finNFe 4", devolucao),
        ("indPres 2", nao_presencial),
        ("tpImp 6", simplificado),
    ] {
        let r = regra(montar_json(&p));
        assert!(!r.starts_with("873"), "{nome}: {r}");
    }
    for cfop in [
        "5922", "6922", "5118", "6118", "5119", "6119", "5120", "6120",
    ] {
        let mut p = med_sem_rastro();
        p["itens"][0]["cfop"] = json!(cfop);
        let r = regra(montar_json(&p));
        assert!(!r.starts_with("873"), "CFOP {cfop}: {r}");
    }
    let mut entrada = med_sem_rastro();
    entrada["tp_nf"] = json!(0);
    assert!(!regra(montar_json(&entrada)).starts_with("873"));
}

#[test]
fn os_limites_de_quantidade_do_xsd_valem() {
    let mut p = caso_de("nfe_montar_transp.json", "transp_completo_por_veiculo");
    p["transp"]["vol"] = json!(vec![json!({"q_vol": "1"}); 5001]);
    assert_eq!(regra(montar_json(&p)), " (vol)");
    p["transp"]["vol"] = json!(vec![json!({"q_vol": "1"}); 5000]);
    assert_eq!(regra(montar_json(&p)), "montou");
    p["transp"]["vol"] = json!([{"lacres": vec!["L"; 5001]}]);
    assert_eq!(regra(montar_json(&p)), " (lacres)");
    p["transp"]["vol"] = json!([{"lacres": vec!["L"; 5000]}]);
    assert_eq!(regra(montar_json(&p)), "montou");
    let mut c = caso_de("nfe_montar_cobr.json", "cobr_tres_parcelas");
    c["itens"][0]["quantidade"] = json!("120");
    c["itens"][0]["valor_unitario"] = json!("0.01");
    c["pagamentos"][0]["valor"] = json!("1.20");
    c["cobr"] = json!({
        "fat": {"n_fat": "F", "v_orig": "1.20", "v_liq": "1.20"},
        "dup": (1..=120).map(|i| json!({"n_dup": format!("{i:03}"), "d_venc": "2026-11-03", "v_dup": "0.01"}))
            .collect::<Vec<_>>()
    });
    assert_eq!(regra(montar_json(&c)), "montou");
}
