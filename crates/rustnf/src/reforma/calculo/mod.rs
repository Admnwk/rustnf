// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! IBS/CBS do ITEM de mercadoria (grupo `IBSCBS` da NF-e) e o total (`IBSCBSTot`, `vNFTot`).
//!
//! Formulas da NT 2025.002: `pAliqEfet` = p x (1 - pRedAliq/100) com 4 casas (UB28-10); `vDif` = vBC x pEfet x pDif
//! (UB23-10); `vIBSUF`, `vIBSMun` e `vCBS` = vBC x pEfet - vDif - vDevTrib (UB35-10, UB54-10 e UB67-10); `vIBS` =
//! vIBSUF + vIBSMun - vCredPres, se o credito presumido deduz (UB54a-10); na tributacao regular a aliquota do item e' zero
//! (excecao 1 da UB18-10) e o `gTribRegular` leva a aliquota regular (UB72-10); `vTotIBSMonoItem` = vIBSMono +
//! vIBSMonoReten (UB105a-10 e UB105b-10). O total e' a soma dos itens (W35-10 a W59g-10) e cada grupo do total so' sai
//! com valor maior que zero. A ordem das tags e' a do XSD PL_010h (`TCIBS_NFe`, `TTribNFe`, `TMonofasia`).
//!
//! O Simples Nacional (NT 2026.005) leva o `gTribSN` com os valores prontos do chamador; no Simples sem regime regular
//! o item NAO leva o `gIBSCBS` (veja [`EntradaItem::sem_g_ibscbs`]). Compra governamental leva o `gTribCompraGov` com a
//! composicao informada: o calculo do art. 473 da LC 214/2025 nao e' feito aqui.

use super::aliquotas::{self, Aliquotas};
use super::tabela;
use super::{ano_de, div_arred, escala, fmt2, fmt4, ErroReforma, Resultado};

mod entrada;
mod especiais;
mod mono;
mod total;

pub use entrada::{
    AjusteCompet, AliqInformada, CredPres, CredPresTrib, CredPresZfm, EntradaItem, Mono, MonoSub,
    MonoTrib, PDif, PValor, ParTxt, TribRegular, TribSn,
};
pub use especiais::TribCompraGov;
use especiais::{compra_gov_de, cred_pres, sem_g_ibscbs, trib_regular, trib_sn, TOL_COMPRA_GOV};
use mono::mono;
pub use mono::regra_mono;
pub use total::{totalizar, vnftot, TotalIbsCbs};

/// No da arvore de tags: folha (tag, texto) ou grupo (tag, filhos).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum No {
    Folha(String, String),
    Grupo(String, Vec<No>),
}

fn f(t: &str, v: String) -> No {
    No::Folha(t.into(), v)
}

fn g(t: &str, v: Vec<No>) -> No {
    No::Grupo(t.into(), v)
}

fn achatar(nos: &[No], prefixo: &str, out: &mut Vec<(String, String)>) {
    for n in nos {
        match n {
            No::Folha(t, v) => out.push((format!("{prefixo}{t}"), v.clone())),
            No::Grupo(t, filhos) => achatar(filhos, &format!("{prefixo}{t}/"), out),
        }
    }
}

fn xml(nos: &[No], out: &mut String) {
    for n in nos {
        match n {
            No::Folha(t, v) => out.push_str(&format!("<{t}>{v}</{t}>")),
            No::Grupo(t, filhos) => {
                out.push_str(&format!("<{t}>"));
                xml(filhos, out);
                out.push_str(&format!("</{t}>"));
            }
        }
    }
}

/// IBS UF, IBS municipal ou CBS do item (centavos e x10000).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Esfera {
    pub p: i64,
    pub p_red: i64,
    pub p_efet: i64,
    pub tem_red: bool,
    pub p_dif: i64,
    pub v_dif: i64,
    pub p_dev: i64,
    pub v_dev: i64,
    pub v: i64,
}

/// Totais da monofasia do item (centavos).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MonoTot {
    pub v_ibs_mono: i64,
    pub v_cbs_mono: i64,
    pub v_ibs_mono_reten: i64,
    pub v_cbs_mono_reten: i64,
    pub v_ibs_mono_ret: i64,
    pub v_cbs_mono_ret: i64,
    pub vtot_ibs: i64,
    pub vtot_cbs: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemIbsCbs {
    pub cst: String,
    pub cclass_trib: String,
    pub grupo: Option<&'static str>,
    pub ano: i32,
    pub vbc: i64,
    pub ibs_uf: Esfera,
    pub ibs_mun: Esfera,
    pub cbs: Esfera,
    pub v_ibs: i64,
    pub v_cbs: i64,
    pub v_cred_pres_ibs: i64,
    pub v_cred_pres_cbs: i64,
    pub v_cred_pres_cond_sus_ibs: i64,
    pub v_cred_pres_cond_sus_cbs: i64,
    pub v_estorno_ibs: i64,
    pub v_estorno_cbs: i64,
    /// gTribSN (NT 2026.005 UB115a); `tem_ibs_sn` = veio o gIBSSN (0-1 no XSD TTribSN; o gCBSSN e' 1-1).
    pub tem_trib_sn: bool,
    pub tem_ibs_sn: bool,
    pub v_ibs_sn: i64,
    pub v_cbs_sn: i64,
    pub mono: MonoTot,
    /// gCompraGov/pRedutor usado no pAliqEfet (x10000); None = sem compra governamental.
    pub p_redutor: Option<i64>,
    /// gTribCompraGov: [(p x10000, v centavos)] de IBS UF, IBS municipal e CBS; None = sem o grupo.
    pub trib_compra_gov: Option<TribCompraGov>,
    pub arvore: Vec<No>,
    pub avisos: Vec<String>,
}

impl ItemIbsCbs {
    /// [(caminho, texto)] na ordem do documento -- 'IBSCBS/gIBSCBS/gIBSUF/pIBSUF' -> '0.1000'.
    pub fn tags(&self) -> Vec<(String, String)> {
        let mut out = Vec::new();
        achatar(&self.arvore, "", &mut out);
        out
    }

    /// O grupo IBSCBS em XML, sem namespace (filho de `imposto`).
    pub fn xml(&self) -> String {
        let mut s = String::new();
        xml(&self.arvore, &mut s);
        s
    }
}

// ----------------------------------------------------------------------------------- calculo

fn c2(s: &str, campo: &str) -> Resultado<i64> {
    Ok(escala(s, 2, campo)? as i64)
}

fn p4(s: &str, campo: &str) -> Resultado<i64> {
    Ok(escala(s, 4, campo)? as i64)
}

/// pEfet = p x (1 - red/100), 4 casas (p e red em x10000).
fn efetiva(p: i64, red: i64) -> i64 {
    div_arred(p as i128 * (1_000_000 - red as i128), 1_000_000) as i64
}

/// vBC (centavos) x p (x10000) / 100 -> centavos.
fn aplica(vbc: i64, p: i64) -> i64 {
    div_arred(vbc as i128 * p as i128, 1_000_000) as i64
}

/// UB28-10 com gCompraGov: p x (1 - red/100) x (1 - redutor/100), arredondado so' no fim (4 casas).
fn efetiva_gov(p: i64, red: i64, redutor: i64) -> i64 {
    div_arred(
        p as i128 * (1_000_000 - red as i128) * (1_000_000 - redutor as i128),
        1_000_000_000_000,
    ) as i64
}

fn esfera(
    vbc: i64,
    p: i64,
    red: Option<i64>,
    p_dif: Option<i64>,
    p_dev: Option<i64>,
    redutor: Option<i64>,
) -> Esfera {
    let mut e = Esfera {
        p,
        ..Esfera::default()
    };
    match red {
        Some(r) => {
            e.tem_red = true;
            e.p_red = r;
            e.p_efet = match redutor {
                Some(rg) => efetiva_gov(p, r, rg),
                None => efetiva(p, r),
            };
        }
        None => e.p_efet = p,
    }
    let bruto = vbc as i128 * e.p_efet as i128; // centavos x1e6
    if let Some(d) = p_dif {
        e.p_dif = d;
        e.v_dif = div_arred(bruto * d as i128, 1_000_000 * 1_000_000) as i64;
    }
    if let Some(d) = p_dev {
        e.p_dev = d;
        e.v_dev = div_arred(bruto * d as i128, 1_000_000 * 1_000_000) as i64;
    }
    e.v = div_arred(bruto - (e.v_dif + e.v_dev) as i128 * 1_000_000, 1_000_000) as i64;
    e
}

fn nos_esfera(tag_p: &str, tag_v: &str, e: &Esfera, tem_dif: bool) -> Vec<No> {
    let mut v = vec![f(tag_p, fmt4(e.p))];
    if tem_dif {
        v.push(g(
            "gDif",
            vec![f("pDif", fmt4(e.p_dif)), f("vDif", fmt2(e.v_dif))],
        ));
    }
    if e.v_dev > 0 || e.p_dev > 0 {
        v.push(g(
            "gDevTrib",
            vec![f("pDevTrib", fmt4(e.p_dev)), f("vDevTrib", fmt2(e.v_dev))],
        ));
    }
    if e.tem_red {
        v.push(g(
            "gRed",
            vec![f("pRedAliq", fmt4(e.p_red)), f("pAliqEfet", fmt4(e.p_efet))],
        ));
    }
    v.push(f(tag_v, fmt2(e.v)));
    v
}

fn aliq_do_item(e: &EntradaItem, ano: i32) -> Resultado<Aliquotas> {
    let a = match &e.aliquotas {
        None => aliquotas::exigir_aliquotas(&e.data, e.publicadas.as_ref(), e.simples_nacional)?,
        Some(x) => Aliquotas {
            ano,
            ibs_uf: p4(&x.ibs_uf, "ibs_uf")?,
            ibs_mun: p4(&x.ibs_mun, "ibs_mun")?,
            cbs: p4(&x.cbs, "cbs")?,
            confirmado: true,
            aplica: !(e.simples_nacional && ano == 2026),
            motivo: if e.simples_nacional && ano == 2026 {
                "as aliquotas de 2026 nao se aplicam as operacoes do optante do Simples Nacional (LC 214/2025 art. 348, III, c)".into()
            } else {
                String::new()
            },
            fonte: "informada pelo chamador".into(),
            avisos: vec![],
        },
    };
    if !a.aplica {
        return Err(ErroReforma::novo(
            format!(
                "o grupo gIBSCBS nao se aplica: {} -- omita o IBS/CBS do item ou use o CST certo",
                a.motivo
            ),
            "nao_se_aplica",
        ));
    }
    Ok(a)
}

pub const TP_NF_CREDITO_SEM_ALIQ_DA_LEI: [&str; 3] = ["03", "04", "06"];

fn conferir_da_lei(e: &EntradaItem, a: &Aliquotas, ano: i32) -> Resultado<()> {
    if ano < 2026 || (e.compra_gov.is_some() && (2027..=2032).contains(&ano)) {
        return Ok(()); // compra governamental 2027-2032: BB02-10 (art. 473) -- a montagem da NF-e confere
    }
    let fin = e
        .fin_nfe
        .as_deref()
        .map(str::trim)
        .filter(|x| !x.is_empty());
    let cred = e
        .tp_nf_credito
        .as_deref()
        .map(|x| format!("{:0>2}", x.trim()));
    let exc2 = fin.is_some_and(|x| x.parse::<i64>().ok() == Some(4))
        || cred
            .as_deref()
            .is_some_and(|c| TP_NF_CREDITO_SEM_ALIQ_DA_LEI.contains(&c));
    let lei = aliquotas::aliquotas(&e.data, e.publicadas.as_ref(), false)?;
    if ano <= 2028 && !exc2 {
        for (inf, fix, tag, regra, rej) in [
            (a.ibs_uf, lei.ibs_uf, "pIBSUF", "UB18-10", "1026"),
            (a.ibs_mun, lei.ibs_mun, "pIBSMun", "UB37-10", "1036"),
        ] {
            if inf != fix {
                return Err(ErroReforma::novo(
                    format!("{tag} {}% em {ano}: a lei fixa {}% (LC 214/2025 arts. 343/344) -- NT 2025.002 {regra}, rejeicao {rej}", fmt4(inf), fmt4(fix)),
                    format!("{regra}/{rej}"),
                ));
            }
        }
    }
    let (regra, dispensa) = if ano == 2026 {
        ("UB56-10", cred.as_deref() == Some("04"))
    } else {
        ("UB56-20", exc2 || !lei.confirmado)
    };
    let zero_ok = e.cbs_zero_alc_zfm || e.trib_sn.is_some();
    if !dispensa && a.cbs != lei.cbs && !(zero_ok && a.cbs == 0) {
        return Err(ErroReforma::novo(
            format!("pCBS {}% em {ano}: a aliquota vigente e' {}% ({}) -- NT 2025.002 {regra}, rejeicao 1037", fmt4(a.cbs), fmt4(lei.cbs), lei.fonte),
            format!("{regra}/1037"),
        ));
    }
    Ok(())
}

/// IBS/CBS de UM item de NF-e: os valores e a arvore de tags do grupo `IBSCBS`.
///
/// # Errors
///
/// [`ErroReforma`] com a regra da NT 2025.002 ou da NT 2026.005 que o item descumpre (CST ou cClassTrib fora da
/// tabela, aliquota da lei, diferimento, monofasia, gTribSN...).
pub fn ibscbs_do_item(e: &EntradaItem) -> Resultado<ItemIbsCbs> {
    item(e)
}

fn item(e: &EntradaItem) -> Resultado<ItemIbsCbs> {
    let doc = "nfe";
    let (redutor, trib_gov) = match &e.compra_gov {
        None => (None, None),
        Some(cg) => {
            let (r, t) = compra_gov_de(cg)?;
            (Some(r), t)
        }
    };
    let (c, k) = tabela::validar(
        &e.cst,
        &e.cclass_trib,
        doc,
        if e.validar_vigencia {
            Some(e.data.as_str())
        } else {
            None
        },
    )?;
    let grupo = tabela::grupo_exigido(&c.cst, doc, e.tp_nf_debito.as_deref())?;
    let grupo = if e.sem_g_ibscbs && grupo == Some("gIBSCBS") {
        sem_g_ibscbs(e)?;
        None
    } else {
        grupo
    };
    let ano = ano_de(&e.data)?;
    let base = c2(&e.vbc, "vBC")?;
    if base < 0 {
        return Err(ErroReforma::novo(
            "base do IBS/CBS negativa: os tributos do item passam do valor dele -- confira as aliquotas",
            "vbc_negativo",
        ));
    }
    let mut it = ItemIbsCbs {
        cst: c.cst.clone(),
        cclass_trib: k.codigo.clone(),
        grupo,
        ano,
        vbc: 0,
        ibs_uf: Esfera::default(),
        ibs_mun: Esfera::default(),
        cbs: Esfera::default(),
        v_ibs: 0,
        v_cbs: 0,
        v_cred_pres_ibs: 0,
        v_cred_pres_cbs: 0,
        v_cred_pres_cond_sus_ibs: 0,
        v_cred_pres_cond_sus_cbs: 0,
        v_estorno_ibs: 0,
        v_estorno_cbs: 0,
        tem_trib_sn: false,
        tem_ibs_sn: false,
        v_ibs_sn: 0,
        v_cbs_sn: 0,
        mono: MonoTot::default(),
        p_redutor: None,
        trib_compra_gov: None,
        arvore: vec![],
        avisos: vec![],
    };
    let mut raiz = vec![f("CST", c.cst.clone()), f("cClassTrib", k.codigo.clone())];
    if e.ind_doacao {
        raiz.push(f("indDoacao", "1".into()));
    }
    let pedidos = [
        (
            "gIBSCBS",
            e.p_dif.is_some() || e.p_dev_cbs.is_some() || e.trib_regular.is_some(),
        ),
        ("gIBSCBSMono", e.mono.is_some()),
        ("gTransfCred", e.transf_cred.is_some()),
        ("gAjusteCompet", e.ajuste_compet.is_some()),
    ];
    for (gr, pediu) in pedidos {
        if pediu && Some(gr) != grupo {
            return Err(ErroReforma::novo(
                format!(
                    "CST {} nao leva o grupo {gr} -- NT 2025.002 UB13-20/UB13-39/UB13-44",
                    c.cst
                ),
                "UB13",
            ));
        }
    }
    let mut cp_nos: Vec<No> = vec![];
    match grupo {
        Some("gIBSCBS") => {
            let al = aliq_do_item(e, ano)?;
            it.avisos.extend(al.avisos.iter().cloned());
            let treg = k.ind_g_trib_regular;
            if e.aliquotas.is_some() && !treg {
                conferir_da_lei(e, &al, ano)?; // tributacao regular: p = 0 (excecao 1), nao ha o que conferir
            }
            if treg && e.trib_regular.is_none() {
                return Err(ErroReforma::novo(
                    format!("cClassTrib {} exige o grupo de Tributacao Regular (gTribRegular) -- NT 2025.002 UB68-10, rejeicao 1065", k.codigo),
                    "UB68-10/1065",
                ));
            }
            if !treg && e.trib_regular.is_some() {
                return Err(ErroReforma::novo(
                    format!("cClassTrib {} NAO permite gTribRegular -- NT 2025.002 UB68-11, rejeicao 1114", k.codigo),
                    "UB68-11/1114",
                ));
            }
            let (puf, pmun, pcbs) = if treg {
                (0, 0, 0)
            } else {
                (al.ibs_uf, al.ibs_mun, al.cbs)
            };
            let (mut ri, mut rc) = if c.ind_g_red {
                (Some(k.p_red("ibs")), Some(k.p_red("cbs")))
            } else {
                (None, None)
            };
            if redutor.is_some() {
                // UB26-20/UB45/UB64: gRed sempre; UB27-10 obs 2: pRedAliq 0 se o CST veda a reducao
                (ri, rc) = (Some(ri.unwrap_or(0)), Some(rc.unwrap_or(0)));
            }
            let mut difs: Vec<Option<i64>> = vec![];
            for (nome, d) in [
                ("ibs_uf", e.p_dif.as_ref().and_then(|x| x.ibs_uf.clone())),
                ("ibs_mun", e.p_dif.as_ref().and_then(|x| x.ibs_mun.clone())),
                ("cbs", e.p_dif.as_ref().and_then(|x| x.cbs.clone())),
            ]
            .into_iter()
            {
                let d = match d {
                    Some(x) => Some(p4(&x, &format!("pDif ({nome})"))?),
                    None => None,
                };
                if c.ind_g_dif && !d.is_some_and(|x| x > 0 && x <= 1_000_000) {
                    return Err(ErroReforma::novo(
                        format!("CST {} exige diferimento: informe o pDif ({nome}, 0 < pDif <= 100) -- NT 2025.002 UB22-20", c.cst),
                        "UB22-20/1030",
                    ));
                }
                if !c.ind_g_dif && d.is_some() {
                    return Err(ErroReforma::novo(
                        format!(
                            "CST {} NAO permite diferimento -- NT 2025.002 UB22-10/UB40-20/UB59-20",
                            c.cst
                        ),
                        "UB22-10/1029",
                    ));
                }
                difs.push(d);
            }
            let pdev = match &e.p_dev_cbs {
                Some(x) => Some(p4(x, "pDevTrib")?),
                None => None,
            };
            it.vbc = base;
            it.ibs_uf = esfera(base, puf, ri, difs[0], None, redutor);
            it.ibs_mun = esfera(base, pmun, ri, difs[1], None, redutor);
            it.cbs = esfera(base, pcbs, rc, difs[2], pdev, redutor);
            if let Some(cp) = &e.cred_pres {
                if !k.ind_g_cred_pres_oper {
                    return Err(ErroReforma::novo(
                        format!("cClassTrib {} nao permite credito presumido da operacao (ind_gCredPresOper) -- NT 2025.002 UB120", k.codigo),
                        "UB120",
                    ));
                }
                let (n, v) = cred_pres(base, cp)?;
                cp_nos = n;
                (it.v_cred_pres_ibs, it.v_cred_pres_cbs) = (v.ibs, v.cbs);
                (it.v_cred_pres_cond_sus_ibs, it.v_cred_pres_cond_sus_cbs) = (v.ibs_cs, v.cbs_cs);
            }
            it.v_ibs = it.ibs_uf.v + it.ibs_mun.v
                - if e.deduz_cred_pres {
                    it.v_cred_pres_ibs
                } else {
                    0
                };
            it.v_cbs = it.cbs.v
                - if e.deduz_cred_pres {
                    it.v_cred_pres_cbs
                } else {
                    0
                };
            let mut gr = vec![
                f("vBC", fmt2(base)),
                g(
                    "gIBSUF",
                    nos_esfera("pIBSUF", "vIBSUF", &it.ibs_uf, c.ind_g_dif),
                ),
                g(
                    "gIBSMun",
                    nos_esfera("pIBSMun", "vIBSMun", &it.ibs_mun, c.ind_g_dif),
                ),
                f("vIBS", fmt2(it.v_ibs)),
                g("gCBS", nos_esfera("pCBS", "vCBS", &it.cbs, c.ind_g_dif)),
            ];
            if let (true, Some(t)) = (treg, e.trib_regular.as_ref()) {
                gr.push(g("gTribRegular", trib_regular(base, t, &al)?));
            }
            if redutor.is_some() {
                let Some(tg) = trib_gov else {
                    return Err(ErroReforma::novo(
                        "compra governamental com gIBSCBS exige o gTribCompraGov (composicao sem o art. 473: \
                         compra_gov['trib']) -- NT 2025.002 UB82a-10, rejeicao 1141",
                        "UB82a-10/1141",
                    ));
                };
                let soma: i64 = tg.iter().map(|x| x.1).sum();
                let item = it.ibs_uf.v + it.ibs_mun.v + it.cbs.v;
                if (soma - item).abs() > TOL_COMPRA_GOV {
                    return Err(ErroReforma::novo(
                        format!("gTribCompraGov soma {} e o item tem vIBSUF + vIBSMun + vCBS = {} (tolerancia 0,04) -- NT \
                                 2025.002 UB82a-20, rejeicao 1142", fmt2(soma), fmt2(item)),
                        "UB82a-20/1142",
                    ));
                }
                it.trib_compra_gov = Some(tg);
                let mut nos = vec![];
                for ((tp, tv), (p, v)) in [
                    ("pAliqIBSUF", "vTribIBSUF"),
                    ("pAliqIBSMun", "vTribIBSMun"),
                    ("pAliqCBS", "vTribCBS"),
                ]
                .iter()
                .zip(tg.iter())
                {
                    nos.push(f(tp, fmt4(*p)));
                    nos.push(f(tv, fmt2(*v)));
                }
                gr.push(g("gTribCompraGov", nos));
            }
            raiz.push(g("gIBSCBS", gr));
        }
        Some("gIBSCBSMono") => {
            let Some(m) = &e.mono else {
                return Err(ErroReforma::novo(
                    format!("CST {} exige o grupo gIBSCBSMono (monofasia) -- NT 2025.002 UB13-40, rejeicao 1116", c.cst),
                    "UB13-40/1116",
                ));
            };
            let (n, tot) = mono(m, ano, k)?;
            it.mono = tot;
            raiz.push(g("gIBSCBSMono", n));
        }
        Some("gTransfCred") => {
            let Some(t) = &e.transf_cred else {
                return Err(ErroReforma::novo(
                    format!(
                        "CST {} exige o grupo gTransfCred -- NT 2025.002 UB13-45, rejeicao 1132",
                        c.cst
                    ),
                    "UB13-45/1132",
                ));
            };
            raiz.push(g(
                "gTransfCred",
                vec![
                    f("vIBS", fmt2(c2(&t.ibs, "vIBS")?)),
                    f("vCBS", fmt2(c2(&t.cbs, "vCBS")?)),
                ],
            ));
        }
        Some(_) => {
            let Some(a) = &e.ajuste_compet else {
                return Err(ErroReforma::novo(
                    format!(
                        "CST {} exige o grupo gAjusteCompet (competApur, vIBS, vCBS)",
                        c.cst
                    ),
                    "gAjusteCompet",
                ));
            };
            let comp = a.compet_apur.as_str();
            let ok = comp.len() == 7
                && comp.as_bytes()[4] == b'-'
                && comp[..4].bytes().all(|b| b.is_ascii_digit())
                && comp[5..].bytes().all(|b| b.is_ascii_digit())
                && (1..=12).contains(&comp[5..].parse::<u32>().unwrap_or(0));
            if !ok {
                return Err(ErroReforma::novo(
                    "competApur do ajuste invalida (AAAA-MM)",
                    "competApur",
                ));
            }
            raiz.push(g(
                "gAjusteCompet",
                vec![
                    f("competApur", comp.into()),
                    f("vIBS", fmt2(c2(&a.ibs, "vIBS")?)),
                    f("vCBS", fmt2(c2(&a.cbs, "vCBS")?)),
                ],
            ));
        }
        None => {
            if e.cred_pres.is_some() || e.p_dif.is_some() {
                return Err(ErroReforma::novo(
                    format!("CST {} nao leva grupo de valores neste documento -- o item vai so' com CST e cClassTrib", c.cst),
                    "UB13-20/1021",
                ));
            }
        }
    }
    // XSD TTribNFe/TTribNFCe (PL_010h): depois do choice de valores, antes do gEstornoCred
    if trib_gov.is_some() && grupo != Some("gIBSCBS") {
        return Err(ErroReforma::novo(
            format!("gTribCompraGov e' filho do gIBSCBS e o CST {} nao leva o grupo (UB82a-10: sem gIBSCBS a composicao \
                     nao se informa)", c.cst),
            "gTribCompraGov",
        ));
    }
    it.p_redutor = redutor;
    if let Some(sn) = &e.trib_sn {
        raiz.push(g("gTribSN", trib_sn(&mut it, sn, grupo)?));
    }
    let perda = matches!(e.tp_nf_debito.as_deref(), Some("07") | Some("7"));
    if e.estorno.is_none() && (k.ind_g_estorno_cred || perda) {
        return Err(ErroReforma::novo(
            format!(
                "cClassTrib {} exige o grupo gEstornoCred (ind_gEstornoCred ou nota de debito 07) -- NT 2025.002 UB116-20, rejeicao 1173",
                k.codigo
            ),
            "UB116-20",
        ));
    }
    if let Some(est) = &e.estorno {
        if !k.ind_g_estorno_cred && !perda {
            return Err(ErroReforma::novo(
                format!("cClassTrib {} nao permite estorno de credito (ind_gEstornoCred) -- NT 2025.002 UB116", k.codigo),
                "UB116",
            ));
        }
        it.v_estorno_ibs = c2(&est.ibs, "vIBSEstCred")?;
        it.v_estorno_cbs = c2(&est.cbs, "vCBSEstCred")?;
        if !perda && it.v_estorno_ibs <= 0 && it.v_estorno_cbs <= 0 {
            return Err(ErroReforma::novo(
                "estorno de credito com vIBS e vCBS zerados -- NT 2025.002 UB116-30, rejeicao 1174",
                "UB116-30",
            ));
        }
        raiz.push(g(
            "gEstornoCred",
            vec![
                f("vIBSEstCred", fmt2(it.v_estorno_ibs)),
                f("vCBSEstCred", fmt2(it.v_estorno_cbs)),
            ],
        ));
    }
    if grupo == Some("gIBSCBS") && e.cred_pres.is_some() {
        raiz.push(g("gCredPresOper", cp_nos));
    }
    if let Some(z) = &e.cred_pres_zfm {
        if !c.ind_g_cred_pres_ibszfm {
            return Err(ErroReforma::novo(
                format!(
                    "CST {} nao permite gCredPresIBSZFM (ind_gCredPresIBSZFM) -- NT 2025.002 UB131",
                    c.cst
                ),
                "UB131",
            ));
        }
        if e.cred_pres.is_some() {
            return Err(ErroReforma::novo(
                "gCredPresOper e gCredPresIBSZFM sao excludentes (xs:choice do TTribNFe)",
                "TTribNFe",
            ));
        }
        if !matches!(z.tp.as_str(), "0" | "1" | "2" | "3" | "4") {
            return Err(ErroReforma::novo(
                "tpCredPresIBSZFM invalido (0 a 4, XSD TTpCredPresIBSZFM)",
                "tpCredPresIBSZFM",
            ));
        }
        raiz.push(g(
            "gCredPresIBSZFM",
            vec![
                f("competApur", z.compet_apur.clone()),
                f("tpCredPresIBSZFM", z.tp.clone()),
                f("vCredPresIBSZFM", fmt2(c2(&z.v, "vCredPresIBSZFM")?)),
            ],
        ));
    }
    it.arvore = vec![g("IBSCBS", raiz)];
    Ok(it)
}
