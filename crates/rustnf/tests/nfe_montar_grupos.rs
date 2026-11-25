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
    p["itens"][0].as_object_mut().unwrap().remove("icms_uf_dest");
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
    p["itens"][0]["combustivel"] = json!({"c_prod_anp": anp, "desc_anp": descricao, "uf_cons": "BA"});
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
    let etanol = com_combustivel(sem_difal(pedido_difal()), "810101001", "ETANOL HIDRATADO COMUM");
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
