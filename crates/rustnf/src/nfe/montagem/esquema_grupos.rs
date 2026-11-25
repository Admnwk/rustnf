// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Os campos do pedido dos grupos que a montagem acrescentou depois do nucleo da nota (combustivel, medicamento e
//! rastreabilidade do item, cobranca, transporte, exportacao, compra): a mesma lista de `esquema.rs`, por caminho.

use super::esquema::{folha, lista, obj, Campo};

const CIDE: &[Campo] = &[folha("q_bc_prod"), folha("v_aliq_prod"), folha("v_cide")];

const ENCERRANTE: &[Campo] = &[
    folha("n_bico"),
    folha("n_tanque"),
    folha("v_enc_ini"),
    folha("v_enc_fin"),
    folha("n_bomba"),
];

const ORIGEM: &[Campo] = &[folha("ind_import"), folha("c_uf_orig"), folha("p_orig")];

pub(super) const COMBUSTIVEL: &[Campo] = &[
    folha("c_prod_anp"),
    folha("desc_anp"),
    folha("uf_cons"),
    folha("p_glp"),
    folha("p_gn_n"),
    folha("p_gn_i"),
    folha("v_part"),
    folha("codif"),
    folha("q_temp"),
    obj("cide", CIDE),
    obj("encerrante", ENCERRANTE),
    folha("p_bio"),
    lista("origens", ORIGEM),
];

pub(super) const MED: &[Campo] = &[
    folha("c_prod_anvisa"),
    folha("x_motivo_isencao"),
    folha("v_pmc"),
];

pub(super) const RASTRO: &[Campo] = &[
    folha("n_lote"),
    folha("q_lote"),
    folha("d_fab"),
    folha("d_val"),
    folha("c_agreg"),
];

const FAT: &[Campo] = &[
    folha("n_fat"),
    folha("v_orig"),
    folha("v_desc"),
    folha("v_liq"),
];

const DUP: &[Campo] = &[folha("n_dup"), folha("d_venc"), folha("v_dup")];

pub(super) const COBR: &[Campo] = &[obj("fat", FAT), lista("dup", DUP)];

const TRANSPORTA: &[Campo] = &[
    folha("documento"),
    folha("nome"),
    folha("ie"),
    folha("endereco"),
    folha("municipio"),
    folha("uf"),
];

const RET_TRANSP: &[Campo] = &[
    folha("v_serv"),
    folha("v_bc_ret"),
    folha("p_icms_ret"),
    folha("v_icms_ret"),
    folha("cfop"),
    folha("c_mun_fg"),
];

const VEICULO_TRANSP: &[Campo] = &[folha("placa"), folha("uf"), folha("rntc")];

const VOL: &[Campo] = &[
    folha("q_vol"),
    folha("esp"),
    folha("marca"),
    folha("n_vol"),
    folha("peso_l"),
    folha("peso_b"),
    folha("lacres"),
];

pub(super) const TRANSP: &[Campo] = &[
    obj("transporta", TRANSPORTA),
    obj("ret_transp", RET_TRANSP),
    obj("veic", VEICULO_TRANSP),
    lista("reboque", VEICULO_TRANSP),
    folha("vagao"),
    folha("balsa"),
    lista("vol", VOL),
];

pub(super) const EXPORTA: &[Campo] = &[
    folha("uf_saida_pais"),
    folha("x_loc_exporta"),
    folha("x_loc_despacho"),
];

pub(super) const COMPRA: &[Campo] = &[folha("x_n_emp"), folha("x_ped"), folha("x_cont")];
