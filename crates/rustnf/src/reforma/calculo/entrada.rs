// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A entrada do calculo do IBS/CBS do item ([`EntradaItem`]) e a leitura dela do JSON.

use serde_json::Value;

use super::super::aliquotas::{self, Publicadas};
use super::super::{ErroReforma, Resultado};

/// Aliquotas informadas pelo chamador (texto decimal), sobrepondo o calendario.
#[derive(Debug, Clone, Default)]
pub struct AliqInformada {
    pub ibs_uf: String,
    pub ibs_mun: String,
    pub cbs: String,
}

#[derive(Debug, Clone, Default)]
pub struct PDif {
    pub ibs_uf: Option<String>,
    pub ibs_mun: Option<String>,
    pub cbs: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct TribRegular {
    pub cst: String,
    pub cclass_trib: String,
    pub ibs_uf: Option<String>,
    pub ibs_mun: Option<String>,
    pub cbs: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct CredPresTrib {
    pub p: String,
    pub cond_sus: bool,
}

#[derive(Debug, Clone, Default)]
pub struct CredPres {
    pub c_cred_pres: String,
    pub vbc: Option<String>,
    pub ibs: Option<CredPresTrib>,
    pub cbs: Option<CredPresTrib>,
}

#[derive(Debug, Clone, Default)]
pub struct ParTxt {
    pub ibs: String,
    pub cbs: String,
}

#[derive(Debug, Clone, Default)]
pub struct AjusteCompet {
    pub compet_apur: String,
    pub ibs: String,
    pub cbs: String,
}

/// Aliquota efetiva do Simples (%, texto) e valor PRONTOS do chamador (a NT 2026.005 nao da a base; sai do PGDAS).
#[derive(Debug, Clone, Default)]
pub struct PValor {
    pub p: Option<String>,
    pub v: Option<String>,
}

/// gTribSN do item: `ibs` 0-1, `cbs` 1-1 (XSD PL_010h TTribSN).
#[derive(Debug, Clone, Default)]
pub struct TribSn {
    pub ibs: Option<PValor>,
    pub cbs: Option<PValor>,
}

#[derive(Debug, Clone, Default)]
pub struct CredPresZfm {
    pub compet_apur: String,
    pub tp: String,
    pub v: String,
}

/// Subgrupo da monofasia: ad rem (`q`, `ad_rem`), ad valorem (`vbc` + `p_uf`/`p_mun` no IBS padrao ou `p`), valor
/// pronto (`v`, no gMonoRet / gpBioDiferenca ad valorem).
#[derive(Debug, Clone, Default)]
pub struct MonoSub {
    pub q: Option<String>,
    pub ad_rem: Option<String>,
    pub vbc: Option<String>,
    pub p_uf: Option<String>,
    pub p_mun: Option<String>,
    pub p: Option<String>,
    pub v: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct MonoTrib {
    pub padrao: Option<MonoSub>,
    pub reten: Option<MonoSub>,
    pub ret: Option<MonoSub>,
    pub pbio: Option<MonoSub>,
}

impl MonoTrib {
    pub(super) fn vazio(&self) -> bool {
        self.padrao.is_none() && self.reten.is_none() && self.ret.is_none() && self.pbio.is_none()
    }
}

#[derive(Debug, Clone, Default)]
pub struct Mono {
    pub ibs: Option<MonoTrib>,
    pub cbs: Option<MonoTrib>,
}

#[derive(Debug, Clone)]
pub struct EntradaItem {
    pub cst: String,
    pub cclass_trib: String,
    pub vbc: String,
    pub data: String,
    pub aliquotas: Option<AliqInformada>,
    pub publicadas: Option<Publicadas>,
    pub simples_nacional: bool,
    pub p_dif: Option<PDif>,
    pub p_dev_cbs: Option<String>,
    pub trib_regular: Option<TribRegular>,
    pub cred_pres: Option<CredPres>,
    pub deduz_cred_pres: bool,
    pub estorno: Option<ParTxt>,
    pub mono: Option<Mono>,
    pub transf_cred: Option<ParTxt>,
    pub ajuste_compet: Option<AjusteCompet>,
    pub cred_pres_zfm: Option<CredPresZfm>,
    pub ind_doacao: bool,
    pub tp_nf_debito: Option<String>,
    pub compra_gov: Option<Value>,
    pub validar_vigencia: bool,
    pub trib_sn: Option<TribSn>,
    /// finNFe da nota: 4 (devolucao) dispensa a aliquota da lei (UB18-10/UB37-10/UB56-20 excecao 2).
    pub fin_nfe: Option<String>,
    /// tpNFCredito da nota: '03'/'04'/'06' dispensam a aliquota da lei (na CBS de 2026, so' a '04': UB56-10).
    pub tp_nf_credito: Option<String>,
    /// pCBS zero na mesma area incentivada ZFM/ALC (UB56-10 exc. 3) ou com gALCZFMCBS (UB56-20 exc. 3).
    pub cbs_zero_alc_zfm: bool,
    /// Simples Nacional sem regime regular do IBS/CBS (CRT 1 sem `tpRegimeIBSCBS`): a tabela da secao 2.3 da NT 2026.005
    /// manda NAO informar o `gIBSCBS`, mesmo que o CST o exija; o item leva so' CST, cClassTrib e `gTribSN`.
    pub sem_g_ibscbs: bool,
}

impl Default for EntradaItem {
    fn default() -> Self {
        EntradaItem {
            cst: String::new(),
            cclass_trib: String::new(),
            vbc: String::new(),
            data: String::new(),
            aliquotas: None,
            publicadas: None,
            simples_nacional: false,
            p_dif: None,
            p_dev_cbs: None,
            trib_regular: None,
            cred_pres: None,
            deduz_cred_pres: false,
            estorno: None,
            mono: None,
            transf_cred: None,
            ajuste_compet: None,
            cred_pres_zfm: None,
            ind_doacao: false,
            tp_nf_debito: None,
            compra_gov: None,
            validar_vigencia: true,
            trib_sn: None,
            fin_nfe: None,
            tp_nf_credito: None,
            cbs_zero_alc_zfm: false,
            sem_g_ibscbs: false,
        }
    }
}

fn s(v: &Value) -> String {
    match v {
        Value::String(x) => x.clone(),
        Value::Null => String::new(),
        o => o.to_string(),
    }
}

pub(super) fn os(v: &Value, k: &str) -> Option<String> {
    v.get(k).filter(|x| !x.is_null()).map(s)
}

fn sub_mono(v: Option<&Value>) -> Option<MonoSub> {
    let v = v.filter(|x| x.is_object())?;
    Some(MonoSub {
        q: os(v, "q"),
        ad_rem: os(v, "ad_rem"),
        vbc: os(v, "vbc"),
        p_uf: os(v, "p_uf"),
        p_mun: os(v, "p_mun"),
        p: os(v, "p"),
        v: os(v, "v"),
    })
}

fn trib_mono(v: Option<&Value>) -> Option<MonoTrib> {
    let v = v.filter(|x| x.is_object())?;
    Some(MonoTrib {
        padrao: sub_mono(v.get("padrao")),
        reten: sub_mono(v.get("reten")),
        ret: sub_mono(v.get("ret")),
        pbio: sub_mono(v.get("pbio")),
    })
}

fn par(v: Option<&Value>) -> Option<ParTxt> {
    let v = v.filter(|x| x.is_object())?;
    Some(ParTxt {
        ibs: os(v, "ibs").unwrap_or_default(),
        cbs: os(v, "cbs").unwrap_or_default(),
    })
}

fn cp_trib(v: Option<&Value>) -> Option<CredPresTrib> {
    let v = v.filter(|x| x.is_object())?;
    Some(CredPresTrib {
        p: os(v, "p").unwrap_or_default(),
        cond_sus: v.get("cond_sus").and_then(Value::as_bool).unwrap_or(false),
    })
}

fn trib_sn_de_json(v: &Value) -> Resultado<TribSn> {
    let forma = || ErroReforma::novo("trib_sn: use {ibs?: {p, v}, cbs: {p, v}}", "gTribSN");
    let o = v.as_object().ok_or_else(forma)?;
    if o.keys().any(|k| k != "ibs" && k != "cbs") {
        return Err(forma());
    }
    let pv = |nome: &str, x: Option<&Value>| -> Resultado<Option<PValor>> {
        match x {
            None | Some(Value::Null) => Ok(None),
            Some(Value::Object(m)) if m.is_empty() => Ok(None),
            Some(y @ Value::Object(m)) if m.keys().all(|k| k == "p" || k == "v") => {
                Ok(Some(PValor {
                    p: os(y, "p"),
                    v: os(y, "v"),
                }))
            }
            _ => Err(ErroReforma::novo(
                format!(
                    "gTribSN/g{nome}SN: informe p (aliquota efetiva do Simples, %) e v (valor)"
                ),
                format!("gTribSN/g{nome}SN"),
            )),
        }
    };
    Ok(TribSn {
        ibs: pv("IBS", o.get("ibs"))?,
        cbs: pv("CBS", o.get("cbs"))?,
    })
}

impl EntradaItem {
    pub fn de_json(v: &Value) -> Resultado<EntradaItem> {
        if !v.is_object() {
            return Err(ErroReforma::novo(
                "entrada do item tem de ser um objeto JSON",
                "entrada",
            ));
        }
        let b = |k: &str| v.get(k).and_then(Value::as_bool).unwrap_or(false);
        Ok(EntradaItem {
            cst: os(v, "cst").unwrap_or_default(),
            cclass_trib: os(v, "cclass_trib").unwrap_or_default(),
            vbc: os(v, "vbc").unwrap_or_default(),
            data: os(v, "data").unwrap_or_default(),
            aliquotas: v
                .get("aliquotas")
                .filter(|x| x.is_object())
                .map(|a| AliqInformada {
                    ibs_uf: os(a, "ibs_uf").unwrap_or_default(),
                    ibs_mun: os(a, "ibs_mun").unwrap_or_default(),
                    cbs: os(a, "cbs").unwrap_or_default(),
                }),
            publicadas: v.get("publicadas").map(aliquotas::publicadas_de_json),
            simples_nacional: b("simples_nacional"),
            p_dif: v.get("p_dif").filter(|x| !x.is_null()).map(|x| {
                if x.is_object() {
                    PDif {
                        ibs_uf: os(x, "ibs_uf"),
                        ibs_mun: os(x, "ibs_mun"),
                        cbs: os(x, "cbs"),
                    }
                } else {
                    let t = s(x);
                    PDif {
                        ibs_uf: Some(t.clone()),
                        ibs_mun: Some(t.clone()),
                        cbs: Some(t),
                    }
                }
            }),
            p_dev_cbs: os(v, "p_dev_cbs"),
            trib_regular: v
                .get("trib_regular")
                .filter(|x| x.is_object())
                .map(|t| TribRegular {
                    cst: os(t, "cst").unwrap_or_default(),
                    cclass_trib: os(t, "cclass_trib").unwrap_or_default(),
                    ibs_uf: os(t, "ibs_uf"),
                    ibs_mun: os(t, "ibs_mun"),
                    cbs: os(t, "cbs"),
                }),
            cred_pres: v
                .get("cred_pres")
                .filter(|x| x.is_object())
                .map(|c| CredPres {
                    c_cred_pres: os(c, "c_cred_pres").unwrap_or_default(),
                    vbc: os(c, "vbc"),
                    ibs: cp_trib(c.get("ibs")),
                    cbs: cp_trib(c.get("cbs")),
                }),
            deduz_cred_pres: b("deduz_cred_pres"),
            estorno: par(v.get("estorno")),
            mono: v.get("mono").filter(|x| x.is_object()).map(|m| Mono {
                ibs: trib_mono(m.get("ibs")),
                cbs: trib_mono(m.get("cbs")),
            }),
            transf_cred: par(v.get("transf_cred")),
            ajuste_compet: v
                .get("ajuste_compet")
                .filter(|x| x.is_object())
                .map(|a| AjusteCompet {
                    compet_apur: os(a, "compet_apur").unwrap_or_default(),
                    ibs: os(a, "ibs").unwrap_or_default(),
                    cbs: os(a, "cbs").unwrap_or_default(),
                }),
            cred_pres_zfm: v
                .get("cred_pres_zfm")
                .filter(|x| x.is_object())
                .map(|z| CredPresZfm {
                    compet_apur: os(z, "compet_apur").unwrap_or_default(),
                    tp: os(z, "tp").unwrap_or_else(|| "0".into()),
                    v: os(z, "v").unwrap_or_default(),
                }),
            ind_doacao: b("ind_doacao"),
            tp_nf_debito: os(v, "tp_nf_debito"),
            compra_gov: v.get("compra_gov").filter(|x| !x.is_null()).cloned(),
            validar_vigencia: v
                .get("validar_vigencia")
                .and_then(Value::as_bool)
                .unwrap_or(true),
            trib_sn: match v.get("trib_sn") {
                None | Some(Value::Null) => None,
                Some(x) => Some(trib_sn_de_json(x)?),
            },
            fin_nfe: os(v, "fin_nfe").filter(|x| !x.is_empty()),
            tp_nf_credito: os(v, "tp_nf_credito").filter(|x| !x.is_empty()),
            cbs_zero_alc_zfm: b("cbs_zero_alc_zfm"),
            sem_g_ibscbs: false,
        })
    }
}
