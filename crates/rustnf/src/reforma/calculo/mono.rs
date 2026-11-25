// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Monofasia (`gIBSCBSMono`, CST 620): subgrupos por tributo e modalidade do ano.

use super::super::aliquotas::{mono_modalidade, Modalidade};
use super::super::tabela::ClassTrib;
use super::super::{fmt2, fmt4, ErroReforma, Resultado};
use super::*;

pub(crate) fn q4m(s: &str, campo: &str) -> Resultado<i64> {
    p4(s, campo)
}

/// (valor do subgrupo em centavos) -- ad rem: q (x10000) x ad_rem (x10000) -> /1e6 centavos.
pub(crate) fn ad_rem_v(q: i64, ar: i64) -> i64 {
    div_arred(q as i128 * ar as i128, 1_000_000) as i64
}

pub(crate) struct ValMono {
    mono: i64,
    reten: i64,
    ret: i64,
}

#[must_use]
pub fn regra_mono(trib: &str, modal: Modalidade, sub: Option<&str>, sufixo: &str) -> String {
    let (grupo, n) = match (trib, modal) {
        ("ibs", Modalidade::AdRem) => ("UB85a", 86),
        ("ibs", Modalidade::AdValorem) => ("UB90", 91),
        (_, Modalidade::AdRem) => ("UB95a", 96),
        _ => ("UB100", 101),
    };
    match sub {
        None => format!("{grupo}-{sufixo}"),
        Some(s) => {
            let i = ["padrao", "reten", "ret", "pbio"]
                .iter()
                .position(|x| *x == s)
                .unwrap_or(0);
            format!("UB{}-{sufixo}", n + i)
        }
    }
}

pub(crate) fn mono_trib(
    trib: &str,
    modal: Modalidade,
    m: &MonoTrib,
    k: &ClassTrib,
) -> Resultado<(Vec<No>, ValMono)> {
    let tt = if trib == "ibs" { "IBS" } else { "CBS" };
    let grupo = match (trib, modal) {
        ("ibs", Modalidade::AdRem) => "gIBSMonoAdRem",
        ("ibs", Modalidade::AdValorem) => "gIBSMonoAdValorem",
        (_, Modalidade::AdRem) => "gCBSMonoAdRem",
        _ => "gCBSMonoAdValorem",
    };
    for (sub, exige, tem) in [
        ("padrao", k.ind_g_mono_padrao, m.padrao.is_some()),
        ("reten", k.ind_g_mono_reten, m.reten.is_some()),
        ("ret", k.ind_g_mono_ret, m.ret.is_some()),
        ("pbio", k.ind_gp_bio_diferenca, m.pbio.is_some()),
    ] {
        if exige && !tem {
            let r = regra_mono(trib, modal, Some(sub), "10");
            return Err(ErroReforma::novo(format!("cClassTrib {} exige o subgrupo de monofasia '{sub}' do {tt} -- NT 2025.002 {r}", k.codigo), r));
        }
        if !exige && tem {
            let r = regra_mono(trib, modal, Some(sub), "20");
            return Err(ErroReforma::novo(format!("cClassTrib {} NAO permite o subgrupo de monofasia '{sub}' do {tt} -- NT 2025.002 {r}", k.codigo), r));
        }
    }
    let mut nos = Vec::new();
    let mut v = ValMono {
        mono: 0,
        reten: 0,
        ret: 0,
    };
    for (sub, d) in [("padrao", &m.padrao), ("reten", &m.reten)] {
        let Some(d) = d else { continue };
        let suf = if sub == "padrao" { "" } else { "Reten" };
        let gnome = if sub == "padrao" {
            "gMonoPadrao"
        } else {
            "gMonoReten"
        };
        let val;
        if modal == Modalidade::AdRem {
            let q = q4m(d.q.as_deref().unwrap_or(""), "qBCMono")?;
            let ar = p4(d.ad_rem.as_deref().unwrap_or(""), "adRem")?;
            val = ad_rem_v(q, ar);
            nos.push(g(
                gnome,
                vec![
                    f(&format!("qBCMono{suf}"), fmt4(q)),
                    f(&format!("adRem{tt}{suf}"), fmt4(ar)),
                    f(&format!("v{tt}Mono{suf}"), fmt2(val)),
                ],
            ));
        } else if trib == "ibs" && sub == "padrao" {
            let vb = c2(d.vbc.as_deref().unwrap_or(""), "vBCMono")?;
            let puf = p4(d.p_uf.as_deref().unwrap_or(""), "pAliqMonoUF")?;
            let pmun = p4(d.p_mun.as_deref().unwrap_or(""), "pAliqMonoMun")?;
            let (vuf, vmun) = (aplica(vb, puf), aplica(vb, pmun));
            val = vuf + vmun;
            nos.push(g(
                gnome,
                vec![
                    f("vBCMono", fmt2(vb)),
                    f("pAliqMonoUF", fmt4(puf)),
                    f("vIBSMonoUF", fmt2(vuf)),
                    f("pAliqMonoMun", fmt4(pmun)),
                    f("vIBSMonoMun", fmt2(vmun)),
                    f("vIBSMono", fmt2(val)),
                ],
            ));
        } else {
            let vb = c2(d.vbc.as_deref().unwrap_or(""), "vBCMono")?;
            let p = p4(d.p.as_deref().unwrap_or(""), "pAliqMono")?;
            val = aplica(vb, p);
            let tag_p = if trib == "cbs" && sub == "padrao" {
                "pAliqMonoCBS"
            } else {
                "pAliqMonoReten"
            };
            nos.push(g(
                gnome,
                vec![
                    f(&format!("vBCMono{suf}"), fmt2(vb)),
                    f(tag_p, fmt4(p)),
                    f(&format!("v{tt}Mono{suf}"), fmt2(val)),
                ],
            ));
        }
        if sub == "padrao" {
            v.mono = val;
        } else {
            v.reten = val;
        }
    }
    if let Some(r) = &m.ret {
        v.ret = c2(r.v.as_deref().unwrap_or(""), "vMonoRet")?;
        nos.push(g(
            "gMonoRet",
            vec![f(&format!("v{tt}MonoRet"), fmt2(v.ret))],
        ));
    }
    if let Some(d) = &m.pbio {
        let q = q4m(d.q.as_deref().unwrap_or(""), "qBCBioComb")?;
        let val = match &d.ad_rem {
            Some(a) => ad_rem_v(q, p4(a, "adRem")?),
            None => c2(d.v.as_deref().unwrap_or(""), "vDiferenca")?,
        };
        nos.push(g(
            "gpBioDiferenca",
            vec![
                f("qBCBioComb", fmt4(q)),
                f(&format!("v{tt}Diferenca"), fmt2(val)),
            ],
        ));
    }
    Ok((
        if nos.is_empty() {
            vec![]
        } else {
            vec![g(grupo, nos)]
        },
        v,
    ))
}

pub(crate) fn mono(m: &Mono, ano: i32, k: &ClassTrib) -> Resultado<(Vec<No>, MonoTot)> {
    let (mi, mc) = mono_modalidade(ano);
    let mut nos = Vec::new();
    let mut tot = MonoTot::default();
    for (trib, modal, mt) in [("ibs", mi, &m.ibs), ("cbs", mc, &m.cbs)] {
        let v = if let Some(mt) = mt.as_ref().filter(|m| !m.vazio()) {
            for d in [&mt.padrao, &mt.reten].into_iter().flatten() {
                if d.q.is_some() != (modal == Modalidade::AdRem) {
                    let errado = if modal == Modalidade::AdRem {
                        Modalidade::AdValorem
                    } else {
                        Modalidade::AdRem
                    };
                    let r = regra_mono(trib, errado, None, "10");
                    return Err(ErroReforma::novo(
                        format!(
                            "monofasia do {} em {ano} tem outra modalidade (NT 2025.002 {r})",
                            trib.to_uppercase()
                        ),
                        r,
                    ));
                }
            }
            let (n, v) = mono_trib(trib, modal, mt, k)?;
            nos.extend(n);
            v
        } else {
            if k.ind_g_mono_padrao
                || k.ind_g_mono_reten
                || k.ind_g_mono_ret
                || k.ind_gp_bio_diferenca
            {
                let r = regra_mono(trib, modal, None, "20");
                return Err(ErroReforma::novo(
                    format!(
                        "cClassTrib {} exige a monofasia do {} -- NT 2025.002 {r}",
                        k.codigo,
                        trib.to_uppercase()
                    ),
                    r,
                ));
            }
            ValMono {
                mono: 0,
                reten: 0,
                ret: 0,
            }
        };
        if trib == "ibs" {
            (tot.v_ibs_mono, tot.v_ibs_mono_reten, tot.v_ibs_mono_ret) = (v.mono, v.reten, v.ret);
        } else {
            (tot.v_cbs_mono, tot.v_cbs_mono_reten, tot.v_cbs_mono_ret) = (v.mono, v.reten, v.ret);
        }
    }
    tot.vtot_ibs = tot.v_ibs_mono + tot.v_ibs_mono_reten;
    tot.vtot_cbs = tot.v_cbs_mono + tot.v_cbs_mono_reten;
    nos.push(f("vTotIBSMonoItem", fmt2(tot.vtot_ibs)));
    nos.push(f("vTotCBSMonoItem", fmt2(tot.vtot_cbs)));
    Ok((nos, tot))
}
