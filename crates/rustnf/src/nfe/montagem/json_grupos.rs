// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O pedido em JSON dos grupos que completam a nota: os do item (combustivel, medicamento e rastreabilidade) e os da nota
//! (transporte, cobranca, exportacao e compra). Cada campo vira o da struct de mesmo nome. Valor monetario e percentual
//! vao em texto ou inteiro; ponto flutuante e' recusado.

use serde_json::Value;

use super::avisos::RegimePisCofins;
use super::cobr::{Cobranca, Duplicata, Fatura};
use super::comb::{Cide, Combustivel, Encerrante, OrigemComb};
use super::exporta::{Compra, Exportacao};
use super::formato::{corta, erro, Res};
use super::json::{opt_obj, s_, s_num};
use super::produto::{Medicamento, Rastro};
use super::transp::{RetencaoIcms, Transportadora, Transporte, VeiculoTransp, VolumeTransp};

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

/// Os textos de uma lista JSON (numero vira texto); o que nao e' texto nem numero e' ignorado.
fn textos(v: &Value, k: &str) -> Vec<String> {
    v.get(k)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|x| match x {
            Value::String(s) => Some(s.clone()),
            Value::Number(n) => Some(n.to_string()),
            _ => None,
        })
        .collect()
}

/// `cobr` do pedido.
pub(crate) fn cobr_de(pedido: &Value) -> Res<Option<Cobranca>> {
    let Some(c) = opt_obj(pedido, "cobr") else {
        return Ok(None);
    };
    let fat = match opt_obj(c, "fat") {
        None => None,
        Some(f) => Some(Fatura {
            n_fat: s_(f, "n_fat"),
            v_orig: s_num(f, "v_orig", "vOrig")?,
            v_desc: s_num(f, "v_desc", "vDesc")?,
            v_liq: s_num(f, "v_liq", "vLiq")?,
        }),
    };
    let mut dup = Vec::new();
    for d in c.get("dup").and_then(Value::as_array).into_iter().flatten() {
        dup.push(Duplicata {
            n_dup: s_(d, "n_dup"),
            d_venc: s_(d, "d_venc"),
            v_dup: s_num(d, "v_dup", "vDup")?,
        });
    }
    Ok(Some(Cobranca { fat, dup }))
}

fn veiculo_transp_de(v: &Value) -> VeiculoTransp {
    VeiculoTransp {
        placa: s_(v, "placa").unwrap_or_default(),
        uf: s_(v, "uf"),
        rntc: s_(v, "rntc"),
    }
}

/// `transp` do pedido (o `modFrete` fica em `mod_frete`, na raiz).
pub(crate) fn transp_de(pedido: &Value) -> Res<Option<Transporte>> {
    let Some(t) = opt_obj(pedido, "transp") else {
        return Ok(None);
    };
    let transporta = opt_obj(t, "transporta").map(|o| Transportadora {
        documento: s_(o, "documento"),
        nome: s_(o, "nome"),
        ie: s_(o, "ie"),
        endereco: s_(o, "endereco"),
        municipio: s_(o, "municipio"),
        uf: s_(o, "uf"),
    });
    let ret_transp = match opt_obj(t, "ret_transp") {
        None => None,
        Some(r) => Some(RetencaoIcms {
            v_serv: s_num(r, "v_serv", "vServ")?,
            v_bc_ret: s_num(r, "v_bc_ret", "vBCRet")?,
            p_icms_ret: s_num(r, "p_icms_ret", "pICMSRet")?,
            v_icms_ret: s_num(r, "v_icms_ret", "vICMSRet")?,
            cfop: s_(r, "cfop").unwrap_or_default(),
            c_mun_fg: s_(r, "c_mun_fg").unwrap_or_default(),
        }),
    };
    let mut vol = Vec::new();
    for v in t.get("vol").and_then(Value::as_array).into_iter().flatten() {
        vol.push(VolumeTransp {
            q_vol: s_(v, "q_vol"),
            esp: s_(v, "esp"),
            marca: s_(v, "marca"),
            n_vol: s_(v, "n_vol"),
            peso_l: s_num(v, "peso_l", "pesoL")?,
            peso_b: s_num(v, "peso_b", "pesoB")?,
            lacres: textos(v, "lacres"),
        });
    }
    Ok(Some(Transporte {
        transporta,
        ret_transp,
        veic: opt_obj(t, "veic").map(veiculo_transp_de),
        reboque: t
            .get("reboque")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(veiculo_transp_de)
            .collect(),
        vagao: s_(t, "vagao"),
        balsa: s_(t, "balsa"),
        vol,
    }))
}

/// `exporta` do pedido.
pub(crate) fn exporta_de(pedido: &Value) -> Option<Exportacao> {
    opt_obj(pedido, "exporta").map(|e| Exportacao {
        uf_saida_pais: s_(e, "uf_saida_pais").unwrap_or_default(),
        x_loc_exporta: s_(e, "x_loc_exporta").unwrap_or_default(),
        x_loc_despacho: s_(e, "x_loc_despacho"),
    })
}

/// `compra` do pedido.
pub(crate) fn compra_de(pedido: &Value) -> Option<Compra> {
    opt_obj(pedido, "compra").map(|c| Compra {
        x_n_emp: s_(c, "x_n_emp"),
        x_ped: s_(c, "x_ped"),
        x_cont: s_(c, "x_cont"),
    })
}

/// `regime_pis_cofins` do pedido: `cumulativo` ou `nao_cumulativo`; outro texto e' recusado.
pub(crate) fn regime_pis_cofins_de(pedido: &Value) -> Res<Option<RegimePisCofins>> {
    let Some(t) = s_(pedido, "regime_pis_cofins").filter(|t| !t.trim().is_empty()) else {
        return Ok(None);
    };
    RegimePisCofins::de_texto(&t).map(Some).ok_or_else(|| {
        erro(
            format!(
                "regime_pis_cofins: '{}' (use cumulativo ou nao_cumulativo)",
                corta(&t, 30)
            ),
            "regime_pis_cofins",
        )
    })
}
