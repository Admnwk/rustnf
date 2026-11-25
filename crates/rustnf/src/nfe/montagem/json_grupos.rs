// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O pedido em JSON dos grupos especificos do item (combustivel, medicamento e rastreabilidade): cada campo vira o da
//! struct de mesmo nome. Valor monetario e percentual vao em texto ou inteiro; ponto flutuante e' recusado.

use serde_json::Value;

use super::comb::{Cide, Combustivel, Encerrante, OrigemComb};
use super::formato::Res;
use super::json::{opt_obj, s_, s_num};
use super::produto::{Medicamento, Rastro};

/// `itens[].combustivel` do pedido.
pub(crate) fn combustivel_de(item: &Value) -> Res<Option<Combustivel>> {
    let Some(co) = opt_obj(item, "combustivel") else {
        return Ok(None);
    };
    let cide = match opt_obj(co, "cide") {
        None => None,
        Some(ci) => Some(Cide {
            q_bc_prod: s_num(ci, "q_bc_prod", "qBCProd")?,
            v_aliq_prod: s_num(ci, "v_aliq_prod", "vAliqProd")?,
            v_cide: s_num(ci, "v_cide", "vCIDE")?,
        }),
    };
    let encerrante = match opt_obj(co, "encerrante") {
        None => None,
        Some(e) => Some(Encerrante {
            n_bico: s_(e, "n_bico").unwrap_or_default(),
            n_tanque: s_(e, "n_tanque").unwrap_or_default(),
            v_enc_ini: s_num(e, "v_enc_ini", "vEncIni")?,
            v_enc_fin: s_num(e, "v_enc_fin", "vEncFin")?,
            n_bomba: s_(e, "n_bomba"),
        }),
    };
    let mut origens = Vec::new();
    for o in co
        .get("origens")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        origens.push(OrigemComb {
            ind_import: s_(o, "ind_import").unwrap_or_default(),
            c_uf_orig: s_(o, "c_uf_orig").unwrap_or_default(),
            p_orig: s_num(o, "p_orig", "pOrig")?,
        });
    }
    Ok(Some(Combustivel {
        c_prod_anp: s_(co, "c_prod_anp").unwrap_or_default(),
        desc_anp: s_(co, "desc_anp").unwrap_or_default(),
        uf_cons: s_(co, "uf_cons").unwrap_or_default(),
        p_glp: s_num(co, "p_glp", "pGLP")?,
        p_gn_n: s_num(co, "p_gn_n", "pGNn")?,
        p_gn_i: s_num(co, "p_gn_i", "pGNi")?,
        v_part: s_num(co, "v_part", "vPart")?,
        codif: s_(co, "codif"),
        q_temp: s_num(co, "q_temp", "qTemp")?,
        cide,
        encerrante,
        p_bio: s_num(co, "p_bio", "pBio")?,
        origens,
    }))
}

/// `itens[].med` do pedido.
pub(crate) fn medicamento_de(item: &Value) -> Res<Option<Medicamento>> {
    let Some(m) = opt_obj(item, "med") else {
        return Ok(None);
    };
    Ok(Some(Medicamento {
        c_prod_anvisa: s_(m, "c_prod_anvisa").unwrap_or_default(),
        x_motivo_isencao: s_(m, "x_motivo_isencao"),
        v_pmc: s_num(m, "v_pmc", "vPMC")?,
    }))
}

/// `itens[].rastro[]` do pedido.
pub(crate) fn rastros_de(item: &Value) -> Res<Vec<Rastro>> {
    let mut v = Vec::new();
    for r in item
        .get("rastro")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        v.push(Rastro {
            n_lote: s_(r, "n_lote").unwrap_or_default(),
            q_lote: s_num(r, "q_lote", "qLote")?,
            d_fab: s_(r, "d_fab").unwrap_or_default(),
            d_val: s_(r, "d_val").unwrap_or_default(),
            c_agreg: s_(r, "c_agreg"),
        });
    }
    Ok(v)
}
