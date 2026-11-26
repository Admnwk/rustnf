// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Os grupos do item que nao sao o `gIBSCBS` comum: tributacao regular, credito presumido, `gTribSN` do Simples Nacional
//! e a composicao da compra governamental.

use serde_json::Value;

use super::super::aliquotas::Aliquotas;
use super::super::{credpres, tabela};
use super::super::{fmt2, fmt4, ErroReforma, Resultado};
use super::entrada::os;
use super::*;

pub(crate) fn trib_regular(vbc: i64, t: &TribRegular, al: &Aliquotas) -> Resultado<Vec<No>> {
    let c = tabela::cst(&t.cst).map_err(|_| {
        ErroReforma::novo(
            "CSTReg inexistente na tabela oficial -- NT 2025.002 UB69-10, rejeicao 1066",
            "UB69-10/1066",
        )
    })?;
    let k = tabela::classificacao(&t.cclass_trib).map_err(|_| {
        ErroReforma::novo(
            "cClassTribReg inexistente -- NT 2025.002 UB70-10, rejeicao 1067",
            "UB70-10/1067",
        )
    })?;
    if k.cst != c.cst {
        return Err(ErroReforma::novo(
            format!(
                "cClassTribReg {} pertence ao CST {}, nao ao CSTReg {}",
                k.codigo, k.cst, c.cst
            ),
            "UB70-10",
        ));
    }
    let mut ps = Vec::new();
    for (inf, base, trib) in [
        (&t.ibs_uf, al.ibs_uf, "ibs"),
        (&t.ibs_mun, al.ibs_mun, "ibs"),
        (&t.cbs, al.cbs, "cbs"),
    ] {
        let p = match inf {
            Some(x) => p4(x, "pAliqEfetReg")?,
            None => efetiva(base, if c.ind_g_red { k.p_red(trib) } else { 0 }),
        };
        ps.push(p);
    }
    let vs: Vec<i64> = ps.iter().map(|p| aplica(vbc, *p)).collect();
    Ok(vec![
        f("CSTReg", c.cst.clone()),
        f("cClassTribReg", k.codigo.clone()),
        f("pAliqEfetRegIBSUF", fmt4(ps[0])),
        f("vTribRegIBSUF", fmt2(vs[0])),
        f("pAliqEfetRegIBSMun", fmt4(ps[1])),
        f("vTribRegIBSMun", fmt2(vs[1])),
        f("pAliqEfetRegCBS", fmt4(ps[2])),
        f("vTribRegCBS", fmt2(vs[2])),
    ])
}

pub(crate) struct ValCp {
    pub(crate) ibs: i64,
    pub(crate) cbs: i64,
    pub(crate) ibs_cs: i64,
    pub(crate) cbs_cs: i64,
}

pub(crate) fn cred_pres(vbc: i64, cp: &CredPres) -> Resultado<(Vec<No>, ValCp)> {
    let ccp = cp.c_cred_pres.trim();
    if !(ccp.bytes().all(|b| b.is_ascii_digit()) && (1..=2).contains(&ccp.len())) {
        return Err(ErroReforma::novo(
            "credito presumido sem cCredPres valido (2 digitos, tabela cCredPres do Anexo IV da NT)",
            "cCredPres",
        ));
    }
    // Tabela cCredPres: UB122-10/1055, UB123-10/1053, UB123-20/1054, UB127-10/1050 e UB127-20/1058 (NT 2025.002 v1.53).
    let codigo = format!("{ccp:0>2}");
    if let Some(v) = credpres::conferir_grupos(&codigo, cp.ibs.is_some(), cp.cbs.is_some()) {
        return Err(ErroReforma::novo(
            format!(
                "{} -- NT 2025.002 {}, rejeicao {}",
                v.mensagem, v.regra_item, v.rejeicao
            ),
            format!("{}/{}", v.regra_item, v.rejeicao),
        ));
    }
    let base = match &cp.vbc {
        Some(x) => c2(x, "vBCCredPres")?,
        None => vbc,
    };
    let mut nos = vec![f("vBCCredPres", fmt2(base)), f("cCredPres", codigo)];
    let mut val = ValCp {
        ibs: 0,
        cbs: 0,
        ibs_cs: 0,
        cbs_cs: 0,
    };
    for (trib, tag, gt) in [
        ("ibs", "gIBSCredPres", &cp.ibs),
        ("cbs", "gCBSCredPres", &cp.cbs),
    ] {
        let Some(t) = gt else { continue };
        let p = p4(&t.p, "pCredPres")?;
        let v = aplica(base, p);
        match (trib, t.cond_sus) {
            ("ibs", false) => val.ibs = v,
            ("ibs", true) => val.ibs_cs = v,
            (_, false) => val.cbs = v,
            (_, true) => val.cbs_cs = v,
        }
        let tv = if t.cond_sus {
            "vCredPresCondSus"
        } else {
            "vCredPres"
        };
        nos.push(g(tag, vec![f("pCredPres", fmt4(p)), f(tv, fmt2(v))]));
    }
    Ok((nos, val))
}

/// (CRT x tpRegimeIBSCBS) quem confere e' a montagem da NF-e (`nfe::montagem`).
/// gTribSN do item (NT 2026.005 UB115a-g; XSD PL_010h `TTribSN`: gIBSSN 0-1, gCBSSN 1-1). Qual emitente leva o grupo
pub(crate) fn trib_sn(it: &mut ItemIbsCbs, sn: &TribSn, grupo: Option<&str>) -> Resultado<Vec<No>> {
    if grupo == Some("gIBSCBSMono") {
        return Err(ErroReforma::novo(
            "monofasia (CST 620, gIBSCBSMono) nao leva o grupo gTribSN -- NT 2026.005 secao 2.3",
            "gTribSN_mono",
        ));
    }
    let Some(cbs) = &sn.cbs else {
        return Err(ErroReforma::novo(
            "gTribSN exige o gCBSSN (pCBSSN, vCBSSN) -- XSD TTribSN, NT 2026.005 UB115e (1-1)",
            "gTribSN/gCBSSN",
        ));
    };
    let par = |nome: &str, d: &PValor| -> Resultado<(i64, i64)> {
        let (Some(p), Some(v)) = (&d.p, &d.v) else {
            return Err(ErroReforma::novo(
                format!(
                    "gTribSN/g{nome}SN: informe p (aliquota efetiva do Simples, %) e v (valor)"
                ),
                format!("gTribSN/g{nome}SN"),
            ));
        };
        let (p, v) = (p4(p, &format!("p{nome}SN"))?, c2(v, &format!("v{nome}SN"))?);
        if p < 0 || v < 0 {
            return Err(ErroReforma::novo(
                format!("p{nome}SN/v{nome}SN negativo"),
                format!("gTribSN/g{nome}SN"),
            ));
        }
        Ok((p, v))
    };
    let mut nos = vec![];
    it.tem_trib_sn = true;
    if let Some(ibs) = &sn.ibs {
        let (p, v) = par("IBS", ibs)?;
        (it.tem_ibs_sn, it.v_ibs_sn) = (true, v);
        nos.push(g(
            "gIBSSN",
            vec![f("pIBSSN", fmt4(p)), f("vIBSSN", fmt2(v))],
        ));
    }
    let (p, v) = par("CBS", cbs)?;
    it.v_cbs_sn = v;
    nos.push(g(
        "gCBSSN",
        vec![f("pCBSSN", fmt4(p)), f("vCBSSN", fmt2(v))],
    ));
    Ok(nos)
}

/// gTribCompraGov: [(p x10000, v centavos)] de IBS UF, IBS municipal e CBS.
pub type TribCompraGov = [(i64, i64); 3];

/// UB82a-20: tolerancia de 0,04 (centavos).
pub(crate) const TOL_COMPRA_GOV: i64 = 4;

pub(crate) fn compra_gov_de(cg: &Value) -> Resultado<(i64, Option<TribCompraGov>)> {
    let forma = || {
        ErroReforma::novo(
            "compra_gov: use {p_redutor, trib: {ibs_uf: {p, v}, ibs_mun: {p, v}, cbs: {p, v}}}",
            "compra_gov",
        )
    };
    let o = cg.as_object().ok_or_else(forma)?;
    if o.keys().any(|k| k != "p_redutor" && k != "trib") {
        return Err(forma());
    }
    let red = p4(&os(cg, "p_redutor").ok_or_else(forma)?, "pRedutor")?;
    if !(0..1_000_000).contains(&red) {
        return Err(ErroReforma::novo(
            format!(
                "pRedutor {} fora de 0 <= pRedutor < 100 (art. 472 LC 214/2025)",
                fmt4(red)
            ),
            "pRedutor",
        ));
    }
    let Some(t) = cg.get("trib").filter(|x| !x.is_null()) else {
        return Ok((red, None));
    };
    let tr = || {
        ErroReforma::novo("gTribCompraGov: informe ibs_uf, ibs_mun e cbs, cada um {p, v} (XSD TTribCompraGov, os 6 campos 1-1)",
                          "gTribCompraGov")
    };
    let to = t.as_object().ok_or_else(tr)?;
    if to.len() != 3
        || !["ibs_uf", "ibs_mun", "cbs"]
            .iter()
            .all(|k| to.contains_key(*k))
    {
        return Err(tr());
    }
    let mut out = [(0i64, 0i64); 3];
    for (i, nome) in ["ibs_uf", "ibs_mun", "cbs"].iter().enumerate() {
        let d = &to[*nome];
        let ok = d
            .as_object()
            .is_some_and(|m| m.len() == 2 && m.contains_key("p") && m.contains_key("v"));
        if !ok {
            return Err(ErroReforma::novo(
                format!("gTribCompraGov/{nome}: informe p (aliquota, %) e v (valor)"),
                "gTribCompraGov",
            ));
        }
        let p = p4(&os(d, "p").unwrap_or_default(), &format!("pAliq {nome}"))?;
        let v = c2(&os(d, "v").unwrap_or_default(), &format!("vTrib {nome}"))?;
        if p < 0 || v < 0 {
            return Err(ErroReforma::novo(
                format!("gTribCompraGov/{nome}: aliquota ou valor negativo"),
                "gTribCompraGov",
            ));
        }
        out[i] = (p, v);
    }
    Ok((red, Some(out)))
}

/// No Simples sem regime regular o item nao leva o `gIBSCBS` (NT 2026.005, secao 2.3): o que so' existe dentro dele
/// (diferimento, devolucao, tributacao regular, credito presumido, compra governamental) nao pode vir no pedido.
pub(crate) fn sem_g_ibscbs(e: &EntradaItem) -> Resultado<()> {
    let pedidos = [
        (e.p_dif.is_some(), "p_dif"),
        (e.p_dev_cbs.is_some(), "p_dev_cbs"),
        (e.trib_regular.is_some(), "trib_regular"),
        (e.cred_pres.is_some(), "cred_pres"),
        (e.compra_gov.is_some(), "compra_gov"),
    ];
    match pedidos.iter().find(|p| p.0) {
        Some((_, nome)) => Err(ErroReforma::novo(
            format!(
                "{nome} so' existe dentro do gIBSCBS e o Simples sem regime regular do IBS/CBS nao o informa (NT 2026.005, \n                 secao 2.3): tire o campo do item"
            ),
            "gIBSCBS",
        )),
        None => Ok(()),
    }
}
