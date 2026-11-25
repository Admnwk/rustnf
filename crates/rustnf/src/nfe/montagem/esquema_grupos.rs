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
