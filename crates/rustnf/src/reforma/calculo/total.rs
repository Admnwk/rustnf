// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O total do IBS/CBS da nota (`IBSCBSTot`) e o `vNFTot`.

use super::super::aliquotas::tributos_somam_no_total;
use super::super::{ano_de, fmt2, Resultado};
use super::*;

// ----------------------------------------------------------------------------------- total

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TotalIbsCbs {
    pub vbc: i64,
    pub v_dif_uf: i64,
    pub v_dev_uf: i64,
    pub v_ibs_uf: i64,
    pub v_dif_mun: i64,
    pub v_dev_mun: i64,
    pub v_ibs_mun: i64,
    pub v_ibs: i64,
    pub v_cred_pres_ibs: i64,
    pub v_cred_pres_cond_sus_ibs: i64,
    pub v_dif_cbs: i64,
    pub v_dev_cbs: i64,
    pub v_cbs: i64,
    pub v_cred_pres_cbs: i64,
    pub v_cred_pres_cond_sus_cbs: i64,
    pub mono: MonoTot,
    pub v_estorno_ibs: i64,
    pub v_estorno_cbs: i64,
    pub tem_trib_sn: bool,
    pub v_ibs_sn: i64,
    pub v_cbs_sn: i64,
    pub arvore: Vec<No>,
}

impl TotalIbsCbs {
    pub fn tags(&self) -> Vec<(String, String)> {
        let mut out = Vec::new();
        achatar(&self.arvore, "", &mut out);
        out
    }

    pub fn xml(&self) -> String {
        let mut s = String::new();
        xml(&self.arvore, &mut s);
        s
    }
}

/// IBSCBSTot = SOMA dos itens ja arredondados (W35-10..W59g-10). None = nenhum item (W34-10).
pub fn totalizar(itens: &[&ItemIbsCbs]) -> Option<TotalIbsCbs> {
    if itens.is_empty() {
        return None;
    }
    let mut t = TotalIbsCbs::default();
    for i in itens {
        if i.grupo == Some("gIBSCBS") {
            t.vbc += i.vbc;
        }
        t.v_dif_uf += i.ibs_uf.v_dif;
        t.v_dev_uf += i.ibs_uf.v_dev;
        t.v_ibs_uf += i.ibs_uf.v;
        t.v_dif_mun += i.ibs_mun.v_dif;
        t.v_dev_mun += i.ibs_mun.v_dev;
        t.v_ibs_mun += i.ibs_mun.v;
        t.v_ibs += i.v_ibs;
        t.v_cred_pres_ibs += i.v_cred_pres_ibs;
        t.v_cred_pres_cond_sus_ibs += i.v_cred_pres_cond_sus_ibs;
        t.v_dif_cbs += i.cbs.v_dif;
        t.v_dev_cbs += i.cbs.v_dev;
        t.v_cbs += i.v_cbs;
        t.v_cred_pres_cbs += i.v_cred_pres_cbs;
        t.v_cred_pres_cond_sus_cbs += i.v_cred_pres_cond_sus_cbs;
        t.mono.v_ibs_mono += i.mono.v_ibs_mono;
        t.mono.v_cbs_mono += i.mono.v_cbs_mono;
        t.mono.v_ibs_mono_reten += i.mono.v_ibs_mono_reten;
        t.mono.v_cbs_mono_reten += i.mono.v_cbs_mono_reten;
        t.mono.v_ibs_mono_ret += i.mono.v_ibs_mono_ret;
        t.mono.v_cbs_mono_ret += i.mono.v_cbs_mono_ret;
        t.mono.vtot_ibs += i.mono.vtot_ibs;
        t.mono.vtot_cbs += i.mono.vtot_cbs;
        t.v_estorno_ibs += i.v_estorno_ibs;
        t.v_estorno_cbs += i.v_estorno_cbs;
        t.tem_trib_sn |= i.tem_trib_sn;
        t.v_ibs_sn += i.v_ibs_sn;
        t.v_cbs_sn += i.v_cbs_sn;
    }
    let mut nos = vec![f("vBCIBSCBS", fmt2(t.vbc))];
    if [
        t.v_ibs,
        t.v_dif_uf,
        t.v_dif_mun,
        t.v_dev_uf,
        t.v_dev_mun,
        t.v_cred_pres_ibs,
        t.v_cred_pres_cond_sus_ibs,
    ]
    .iter()
    .any(|v| *v > 0)
    {
        nos.push(g(
            "gIBS",
            vec![
                g(
                    "gIBSUF",
                    vec![
                        f("vDif", fmt2(t.v_dif_uf)),
                        f("vDevTrib", fmt2(t.v_dev_uf)),
                        f("vIBSUF", fmt2(t.v_ibs_uf)),
                    ],
                ),
                g(
                    "gIBSMun",
                    vec![
                        f("vDif", fmt2(t.v_dif_mun)),
                        f("vDevTrib", fmt2(t.v_dev_mun)),
                        f("vIBSMun", fmt2(t.v_ibs_mun)),
                    ],
                ),
                f("vIBS", fmt2(t.v_ibs)),
                f("vCredPres", fmt2(t.v_cred_pres_ibs)),
                f("vCredPresCondSus", fmt2(t.v_cred_pres_cond_sus_ibs)),
            ],
        ));
    }
    if [
        t.v_cbs,
        t.v_dif_cbs,
        t.v_dev_cbs,
        t.v_cred_pres_cbs,
        t.v_cred_pres_cond_sus_cbs,
    ]
    .iter()
    .any(|v| *v > 0)
    {
        nos.push(g(
            "gCBS",
            vec![
                f("vDif", fmt2(t.v_dif_cbs)),
                f("vDevTrib", fmt2(t.v_dev_cbs)),
                f("vCBS", fmt2(t.v_cbs)),
                f("vCredPres", fmt2(t.v_cred_pres_cbs)),
                f("vCredPresCondSus", fmt2(t.v_cred_pres_cond_sus_cbs)),
            ],
        ));
    }
    let m = &t.mono;
    if [
        m.v_ibs_mono,
        m.v_cbs_mono,
        m.v_ibs_mono_reten,
        m.v_cbs_mono_reten,
        m.v_ibs_mono_ret,
        m.v_cbs_mono_ret,
    ]
    .iter()
    .any(|v| *v > 0)
    {
        nos.push(g(
            "gMono",
            vec![
                f("vIBSMono", fmt2(m.v_ibs_mono)),
                f("vCBSMono", fmt2(m.v_cbs_mono)),
                f("vIBSMonoReten", fmt2(m.v_ibs_mono_reten)),
                f("vCBSMonoReten", fmt2(m.v_cbs_mono_reten)),
                f("vIBSMonoRet", fmt2(m.v_ibs_mono_ret)),
                f("vCBSMonoRet", fmt2(m.v_cbs_mono_ret)),
            ],
        ));
    }
    if t.v_estorno_ibs > 0 || t.v_estorno_cbs > 0 {
        nos.push(g(
            "gEstornoCred",
            vec![
                f("vIBSEstCred", fmt2(t.v_estorno_ibs)),
                f("vCBSEstCred", fmt2(t.v_estorno_cbs)),
            ],
        ));
    }
    // NT 2026.005 W59h-10/1275 (item com gTribSN -> total obrigatorio, mesmo zerado), W59h-20/1276 (sem item -> nao vai);
    // W59i-10/1277 e W59j-10/1284 = soma dos itens. Ultimo filho do IBSCBSTot (XSD PL_010h TIBSCBSMonoTot).
    if t.tem_trib_sn {
        nos.push(g(
            "gTribSN",
            vec![f("vIBSSN", fmt2(t.v_ibs_sn)), f("vCBSSN", fmt2(t.v_cbs_sn))],
        ));
    }
    t.arvore = vec![g("IBSCBSTot", nos)];
    Some(t)
}

/// vNFTot (centavos) = vNF + vIBS + vCBS + vIS + vTotMono -- so' de 2027 em diante (NT 2025.002 VB01-10, excecao 1).
pub fn vnftot(vnf: &str, total: Option<&TotalIbsCbs>, data: &str, v_is: &str) -> Resultado<i64> {
    let base = c2(vnf, "vNF")?;
    match total {
        Some(t) if tributos_somam_no_total(ano_de(data)?) => {
            Ok(base + t.v_ibs + t.v_cbs + c2(v_is, "vIS")? + t.mono.vtot_ibs + t.mono.vtot_cbs)
        }
        _ => Ok(base),
    }
}
