// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O pedido em JSON vira a [`Nota`]. Valor monetario e percentual vao em texto (`"10.50"`) ou inteiro; numero
//! com ponto flutuante e' recusado. O formato esta em `docs/NFE_PEDIDO.md`.

use chrono::{DateTime, FixedOffset};
use serde_json::Value;

use crate::reforma::calculo::{ibscbs_do_item, EntradaItem};

use super::difal;
use super::formato::*;
use super::json_grupos::{
    cobr_de, combustivel_de, compra_de, exporta_de, medicamento_de, rastros_de,
    regime_pis_cofins_de, transp_de,
};
use super::modelo::*;
use super::VER_PROC;

/// Texto de um campo (numero vira texto); ausente ou de outro tipo, `None`.
pub(crate) fn s_(v: &Value, k: &str) -> Option<String> {
    match v.get(k)? {
        Value::Null => None,
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

pub(crate) fn s_num(v: &Value, k: &str, campo: &str) -> Res<Option<String>> {
    match v.get(k) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(Value::Number(n)) if n.is_i64() || n.is_u64() => Ok(Some(n.to_string())),
        Some(Value::Number(n)) => Err(erro(
            format!("{campo}: use Decimal ou texto, nao float ({n})"),
            campo,
        )),
        _ => Err(erro(format!("{campo}: esperado numero em texto"), campo)),
    }
}

pub(crate) fn i_(v: &Value, k: &str) -> Option<i64> {
    match v.get(k)? {
        Value::Number(n) => n.as_i64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

pub(crate) fn endereco_de(v: &Value) -> Endereco {
    Endereco {
        logradouro: s_(v, "logradouro").unwrap_or_default(),
        numero: s_(v, "numero").unwrap_or_default(),
        bairro: s_(v, "bairro").unwrap_or_default(),
        cod_municipio: s_(v, "cod_municipio").unwrap_or_default(),
        municipio: s_(v, "municipio").unwrap_or_default(),
        uf: s_(v, "uf").unwrap_or_default(),
        cep: s_(v, "cep"),
        complemento: s_(v, "complemento"),
        cod_pais: s_(v, "cod_pais"),
        pais: s_(v, "pais"),
        fone: s_(v, "fone"),
    }
}

pub(crate) fn opt_obj<'a>(v: &'a Value, k: &str) -> Option<&'a Value> {
    v.get(k).filter(|x| !x.is_null())
}

/// Le o pedido em JSON. Campo que o leiaute nao conhece e' IGNORADO aqui: [`super::campos_desconhecidos`] os lista
/// (aviso) e [`super::nota_de_json_estrito`] os recusa.
///
/// # Errors
///
/// [`ErroMontagem`] quando falta um bloco obrigatorio (`emit`, `emissao`), um numero vem como ponto flutuante, a data
/// vem sem fuso ou o IBS/CBS ou o DIFAL do item nao calculam.
pub fn nota_de_json(v: &Value) -> Res<Nota> {
    let sem_g_ibscbs = {
        let e = v.get("emit").unwrap_or(&Value::Null);
        i_(e, "crt") == Some(1) && i_(e, "tp_regime_ibscbs").is_none()
    };
    let local_de = |k: &str| -> Res<Option<Local>> {
        let Some(l) = opt_obj(v, k) else {
            return Ok(None);
        };
        Ok(Some(Local {
            documento: s_(l, "documento").unwrap_or_default(),
            endereco: endereco_de(l.get("endereco").unwrap_or(&Value::Null)),
            nome: s_(l, "nome"),
            email: s_(l, "email"),
            ie: s_(l, "ie"),
        }))
    };
    let (retirada, entrega) = (local_de("retirada")?, local_de("entrega")?);
    let e = v
        .get("emit")
        .ok_or_else(|| erro("emit: obrigatorio", "emit"))?;
    let emit = Emitente {
        documento: s_(e, "documento").unwrap_or_default(),
        nome: s_(e, "nome").unwrap_or_default(),
        endereco: endereco_de(e.get("endereco").unwrap_or(&Value::Null)),
        crt: i_(e, "crt").unwrap_or(0),
        ie: s_(e, "ie"),
        fantasia: s_(e, "fantasia"),
        iest: s_(e, "iest"),
        im: s_(e, "im"),
        cnae: s_(e, "cnae"),
        tp_regime_ibscbs: i_(e, "tp_regime_ibscbs"),
        isuf: s_(e, "isuf"),
    };
    let dest = opt_obj(v, "dest").map(|d| Destinatario {
        documento: s_(d, "documento"),
        id_estrangeiro: s_(d, "id_estrangeiro"),
        nome: s_(d, "nome"),
        endereco: opt_obj(d, "endereco").map(endereco_de),
        ind_ie_dest: i_(d, "ind_ie_dest").unwrap_or(9),
        ie: s_(d, "ie"),
        isuf: s_(d, "isuf"),
        im: s_(d, "im"),
        email: s_(d, "email"),
    });
    let mut itens = Vec::new();
    for (i, it) in v
        .get("itens")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .iter()
        .enumerate()
    {
        let k = format!("item {}", i + 1);
        let veic = match opt_obj(it, "veiculo") {
            None => None,
            Some(vo) => {
                let g = |c: &str| s_(vo, c).unwrap_or_default();
                Some(Veiculo {
                    tp_op: g("tp_op"),
                    chassi: g("chassi"),
                    c_cor: g("c_cor"),
                    x_cor: g("x_cor"),
                    pot: g("pot"),
                    cilin: g("cilin"),
                    peso_l: g("peso_l"),
                    peso_b: g("peso_b"),
                    n_serie: g("n_serie"),
                    tp_comb: g("tp_comb"),
                    n_motor: g("n_motor"),
                    cmt: g("cmt"),
                    dist: g("dist"),
                    ano_mod: g("ano_mod"),
                    ano_fab: g("ano_fab"),
                    tp_pint: g("tp_pint"),
                    tp_veic: g("tp_veic"),
                    esp_veic: g("esp_veic"),
                    vin: g("vin"),
                    cond_veic: g("cond_veic"),
                    c_mod: g("c_mod"),
                    c_cor_denatran: g("c_cor_denatran"),
                    lota: g("lota"),
                    tp_rest: g("tp_rest"),
                })
            }
        };
        let n2 = |o: &Value, c: &str| s_num(o, c, c);
        let icms = match opt_obj(it, "icms") {
            None => None,
            Some(ic) => {
                // motivo: texto ou inteiro; outro tipo segue como texto e o `conferir_deson` recusa (nunca some calado)
                let mot = |c: &str| match ic.get(c) {
                    None | Some(Value::Null) => None,
                    Some(Value::String(s)) => Some(s.clone()),
                    Some(x) => Some(x.to_string()),
                };
                let ind = match ic.get("ind_deduz_deson") {
                    None | Some(Value::Null) => None,
                    Some(Value::Number(n)) if n.is_i64() => n.as_i64(),
                    Some(x) => {
                        return Err(erro(
                            format!("{k}: indDeduzDeson 0 ou 1 (N28b): {x}"),
                            "indDeduzDeson",
                        ))
                    }
                };
                Some(Icms {
                    codigo: s_(ic, "codigo").unwrap_or_default(),
                    orig: i_(ic, "orig").unwrap_or(0),
                    mod_bc: i_(ic, "mod_bc").unwrap_or(3),
                    vbc: n2(ic, "vbc")?,
                    p_red_bc: n2(ic, "p_red_bc")?,
                    p_icms: n2(ic, "p_icms")?,
                    v_icms: n2(ic, "v_icms")?,
                    vbc_fcp: n2(ic, "vbc_fcp")?,
                    p_fcp: n2(ic, "p_fcp")?,
                    v_fcp: n2(ic, "v_fcp")?,
                    vbc_st_ret: n2(ic, "vbc_st_ret")?,
                    p_st: n2(ic, "p_st")?,
                    v_icms_substituto: n2(ic, "v_icms_substituto")?,
                    v_icms_st_ret: n2(ic, "v_icms_st_ret")?,
                    p_cred_sn: n2(ic, "p_cred_sn")?,
                    v_cred_icms_sn: n2(ic, "v_cred_icms_sn")?,
                    mod_bc_st: i_(ic, "mod_bc_st"),
                    p_mva_st: n2(ic, "p_mva_st")?,
                    p_red_bc_st: n2(ic, "p_red_bc_st")?,
                    vbc_st: n2(ic, "vbc_st")?,
                    p_icms_st: n2(ic, "p_icms_st")?,
                    v_icms_st: n2(ic, "v_icms_st")?,
                    vbc_fcp_st: n2(ic, "vbc_fcp_st")?,
                    p_fcp_st: n2(ic, "p_fcp_st")?,
                    v_fcp_st: n2(ic, "v_fcp_st")?,
                    v_icms_op: n2(ic, "v_icms_op")?,
                    p_dif: n2(ic, "p_dif")?,
                    v_icms_dif: n2(ic, "v_icms_dif")?,
                    p_fcp_dif: n2(ic, "p_fcp_dif")?,
                    v_fcp_dif: n2(ic, "v_fcp_dif")?,
                    v_fcp_efet: n2(ic, "v_fcp_efet")?,
                    v_icms_deson: n2(ic, "v_icms_deson")?,
                    mot_des_icms: mot("mot_des_icms"),
                    ind_deduz_deson: ind,
                    v_icms_st_deson: n2(ic, "v_icms_st_deson")?,
                    mot_des_icms_st: mot("mot_des_icms_st"),
                    partilha: ic.get("partilha").and_then(Value::as_bool).unwrap_or(false),
                    p_bc_op: n2(ic, "p_bc_op")?,
                    uf_st: s_(ic, "uf_st"),
                    repasse: ic.get("repasse").and_then(Value::as_bool).unwrap_or(false),
                    vbc_st_dest: n2(ic, "vbc_st_dest")?,
                    v_icms_st_dest: n2(ic, "v_icms_st_dest")?,
                    vbc_fcp_st_ret: n2(ic, "vbc_fcp_st_ret")?,
                    p_fcp_st_ret: n2(ic, "p_fcp_st_ret")?,
                    v_fcp_st_ret: n2(ic, "v_fcp_st_ret")?,
                    p_red_bc_efet: n2(ic, "p_red_bc_efet")?,
                    vbc_efet: n2(ic, "vbc_efet")?,
                    p_icms_efet: n2(ic, "p_icms_efet")?,
                    v_icms_efet: n2(ic, "v_icms_efet")?,
                    c_benef_rbc: s_(ic, "c_benef_rbc"),
                })
            }
        };
        let pc = |nome: &str| -> Res<Option<PisCofins>> {
            let Some(o) = opt_obj(it, nome) else {
                return Ok(None);
            };
            Ok(Some(PisCofins {
                cst: s_(o, "cst").unwrap_or_default(),
                vbc: n2(o, "vbc")?,
                aliquota: n2(o, "aliquota")?,
                valor: n2(o, "valor")?,
            }))
        };
        let ipi = match opt_obj(it, "ipi") {
            None => None,
            Some(o) => Some(Ipi {
                cst: s_(o, "cst").unwrap_or_default(),
                c_enq: s_(o, "c_enq").unwrap_or_else(|| "999".into()),
                vbc: n2(o, "vbc")?,
                aliquota: n2(o, "aliquota")?,
                valor: n2(o, "valor")?,
            }),
        };
        let ibscbs = match opt_obj(it, "ibscbs") {
            None => None,
            Some(o) => Some(
                EntradaItem::de_json(o)
                    .and_then(|mut e| {
                        if o.get("fin_nfe").is_none() {
                            e.fin_nfe = s_(v, "fin_nfe");
                        }
                        if o.get("tp_nf_credito").is_none() {
                            e.tp_nf_credito = s_(v, "tp_nf_credito");
                        }
                        // Simples sem regime regular: a tabela da NT 2026.005 (secao 2.3) manda nao informar o gIBSCBS
                        e.sem_g_ibscbs = sem_g_ibscbs;
                        ibscbs_do_item(&e)
                    })
                    .map_err(|e| erro(e.to_string(), "IBSCBS"))?,
            ),
        };
        let dfe_ref = opt_obj(it, "dfe_ref").and_then(Value::as_array).map(|a| {
            (
                a.first()
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                a.get(1).and_then(Value::as_i64),
            )
        });
        itens.push(Item {
            codigo: s_(it, "codigo").unwrap_or_default(),
            descricao: s_(it, "descricao").unwrap_or_default(),
            ncm: s_(it, "ncm").unwrap_or_default(),
            cfop: s_(it, "cfop").unwrap_or_default(),
            unidade: s_(it, "unidade").unwrap_or_default(),
            quantidade: n2(it, "quantidade")?.unwrap_or_default(),
            valor_unitario: n2(it, "valor_unitario")?.unwrap_or_default(),
            icms,
            pis: pc("pis")?,
            cofins: pc("cofins")?,
            valor_produto: n2(it, "valor_produto")?,
            gtin: s_(it, "gtin"),
            gtin_trib: s_(it, "gtin_trib"),
            unidade_trib: s_(it, "unidade_trib"),
            quantidade_trib: n2(it, "quantidade_trib")?,
            valor_unitario_trib: n2(it, "valor_unitario_trib")?,
            cest: s_(it, "cest"),
            cbenef: s_(it, "cbenef"),
            extipi: s_(it, "extipi"),
            frete: n2(it, "frete")?,
            seguro: n2(it, "seguro")?,
            desconto: n2(it, "desconto")?,
            outros: n2(it, "outros")?,
            ind_tot: i_(it, "ind_tot").unwrap_or(1),
            ipi,
            ibscbs,
            v_tot_trib: n2(it, "v_tot_trib")?,
            inf_ad_prod: s_(it, "inf_ad_prod"),
            x_ped: s_(it, "x_ped"),
            n_item_ped: s_(it, "n_item_ped"),
            dfe_ref,
            valor_unitario_liquido: n2(it, "valor_unitario_liquido")?,
            valor_produto_liquido: n2(it, "valor_produto_liquido")?,
            c_ativ_sn: s_(it, "c_ativ_sn"),
            v_icms_previsto: n2(it, "v_icms_previsto")?,
            icms_uf_dest: match opt_obj(it, "icms_uf_dest") {
                None => None,
                Some(o) => Some(difal_de_json(o).map_err(|e| erro(e, "ICMSUFDest"))?),
            },
            difal_dispensado: s_(it, "difal_dispensado"),
            veiculo: veic,
            combustivel: combustivel_de(it)?,
            med: medicamento_de(it)?,
            rastro: rastros_de(it)?,
        });
    }
    let mut pagamentos = Vec::new();
    for p in v
        .get("pagamentos")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
    {
        let cartao = opt_obj(&p, "cartao").map(|c| Cartao {
            tp_integra: i_(c, "tp_integra").unwrap_or(0),
            cnpj: s_(c, "cnpj"),
            t_band: s_(c, "t_band"),
            c_aut: s_(c, "c_aut"),
        });
        pagamentos.push(Pagamento {
            t_pag: s_(&p, "t_pag").unwrap_or_default(),
            valor: s_num(&p, "valor", "vPag")?.unwrap_or_default(),
            ind_pag: i_(&p, "ind_pag"),
            x_pag: s_(&p, "x_pag"),
            cartao,
        });
    }
    let resp_tec = opt_obj(v, "resp_tec").map(|r| RespTec {
        cnpj: s_(r, "cnpj").unwrap_or_default(),
        contato: s_(r, "contato").unwrap_or_default(),
        email: s_(r, "email").unwrap_or_default(),
        fone: s_(r, "fone").unwrap_or_default(),
        id_csrt: i_(r, "id_csrt"),
        csrt: s_(r, "csrt"),
    });
    let dt = |k: &str| -> Res<Option<DateTime<FixedOffset>>> {
        match s_(v, k) {
            None => Ok(None),
            Some(s) => {
                let fuso = match k {
                    "emissao" => "dhEmi",
                    "saida" => "dhSaiEnt",
                    _ => "dhCont",
                };
                data_hora(&s, k, fuso).map(Some)
            }
        }
    };
    Ok(Nota {
        modelo: i_(v, "modelo").unwrap_or(0),
        serie: i_(v, "serie").unwrap_or(0),
        numero: i_(v, "numero").unwrap_or(0),
        emissao: dt("emissao")?.ok_or_else(|| erro("emissao: obrigatoria", "emissao"))?,
        nat_op: s_(v, "nat_op").unwrap_or_default(),
        cmun_fg: s_(v, "cmun_fg").unwrap_or_default(),
        emit,
        itens,
        pagamentos,
        dest,
        tp_amb: i_(v, "tp_amb").unwrap_or(2),
        tp_emis: i_(v, "tp_emis").unwrap_or(1),
        tp_nf: i_(v, "tp_nf").unwrap_or(1),
        id_dest: i_(v, "id_dest").unwrap_or(1),
        fin_nfe: i_(v, "fin_nfe").unwrap_or(1),
        ind_final: i_(v, "ind_final"),
        ind_pres: i_(v, "ind_pres"),
        ind_intermed: i_(v, "ind_intermed"),
        tp_imp: i_(v, "tp_imp"),
        proc_emi: i_(v, "proc_emi").unwrap_or(0),
        ver_proc: s_(v, "ver_proc").unwrap_or_else(|| VER_PROC.into()),
        codigo: i_(v, "codigo"),
        saida: dt("saida")?,
        cmun_fg_ibs: s_(v, "cmun_fg_ibs"),
        dh_cont: dt("dh_cont")?,
        x_just: s_(v, "x_just"),
        refs_nfe: v
            .get("refs_nfe")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default(),
        troco: s_num(v, "troco", "vTroco")?,
        mod_frete: i_(v, "mod_frete").unwrap_or(9),
        inf_ad_fisco: s_(v, "inf_ad_fisco"),
        inf_cpl: s_(v, "inf_cpl"),
        resp_tec,
        remover_acento: v
            .get("remover_acento")
            .and_then(Value::as_bool)
            .unwrap_or(true),
        tp_nf_debito: s_(v, "tp_nf_debito"),
        tp_nf_credito: s_(v, "tp_nf_credito"),
        pag_antecipado: v
            .get("pag_antecipado")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default(),
        compra_gov: match opt_obj(v, "compra_gov") {
            None => None,
            Some(c) => Some(CompraGov {
                tp_ente: i_(c, "tp_ente").unwrap_or(0),
                p_redutor: s_num(c, "p_redutor", "pRedutor")?.unwrap_or_default(),
                tp_oper: i_(c, "tp_oper").unwrap_or(0),
                refs: c
                    .get("refs")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .filter_map(|x| x.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default(),
            }),
        },
        c_ind_op: s_(v, "c_ind_op"),
        retirada,
        entrega,
        transp: transp_de(v)?,
        cobr: cobr_de(v)?,
        exporta: exporta_de(v),
        compra: compra_de(v),
        regime_pis_cofins: regime_pis_cofins_de(v)?,
    })
}

/// Os argumentos de [`difal::calcular`] em JSON.
fn difal_de_json(o: &Value) -> Result<difal::Grupo, String> {
    let txt = |k: &str, d: &str| o.get(k).and_then(Value::as_str).unwrap_or(d).to_string();
    difal::calcular(&difal::Entrada {
        vbc: o
            .get("vbc")
            .and_then(Value::as_i64)
            .ok_or("vbc deve ser inteiro (centavos) >= 0")?,
        aliquota_interna: txt("aliquota_interna", ""),
        aliquota_inter: txt("aliquota_inter", ""),
        ano: o
            .get("ano")
            .and_then(Value::as_i64)
            .and_then(|a| i32::try_from(a).ok())
            .unwrap_or(0),
        fcp: txt("fcp", "0"),
        vbc_uf_dest: o.get("vbc_uf_dest").and_then(Value::as_i64),
    })
}
