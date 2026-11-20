// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Anexo VIII da NFS-e Nacional (RTC): item LC 116 x NBS -> cIndOp, local de incidencia do IBS, cClassTrib (+ CST).
//! Dado: `dados/nfse_correlacao.json` (planilha oficial v1.02.00 de 26/10/2026, portal da NFS-e, secao RTC),
//! embutido com `include_str!`; contrato `tests/casos/nfse_correlacao.json`.
//! Politica: nunca escolher calado (`sugerir` so' com UMA combinacao cIndOp + cClassTrib); CST pela tabela
//! cClassTrib deste crate; `avisos` = o que o Anexo VII e a tabela de classificacao dizem que nao e' de NFS-e.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::Deserialize;

use super::base::{ErroNfse, R};
use crate::reforma::{indop, tabela};

pub const CAMPO: &str = "infDPS/valores/trib/IBSCBS";
const DADOS: &str = include_str!("../../dados/nfse_correlacao.json");

#[derive(Deserialize)]
struct Linha {
    linha: u32,
    item: String,
    nbs: Option<String>,
    cod_ativ_sn: Option<String>,
    cind_op: Option<String>,
    local: Option<String>,
    cclasstrib: Option<String>,
}

#[derive(Deserialize)]
struct Regra {
    onerosa_exterior: Option<String>,
    nao_onerosa: Option<String>,
    nao_onerosa_exterior: Option<String>,
}

#[derive(Deserialize)]
struct Arquivo {
    versao: String,
    itens: BTreeMap<String, String>,
    nbs: BTreeMap<String, String>,
    regra_nao_onerosa_exterior: BTreeMap<String, Regra>,
    linhas: Vec<Linha>,
}

/// Uma linha da aba "tabela geral" (celulas mescladas preenchidas).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Correlacao {
    pub linha: u32,
    pub item: String,
    pub nbs: Option<String>,
    pub cod_ativ_sn: Option<String>,
    pub cind_op: Option<String>,
    pub local: Option<String>,
    pub cclasstrib: Option<String>,
    pub cst: Option<String>,
    pub avisos: Vec<String>,
}

struct Base {
    arq: Arquivo,
    por_item: BTreeMap<String, Vec<Correlacao>>,
}

fn avisos(ci: Option<&str>, cc: Option<&str>) -> Vec<String> {
    let mut out = Vec::new();
    match ci {
        None => out.push("o Anexo VIII nao traz cIndOp nesta linha".to_string()),
        Some(ci) => {
            if let Some(r) = indop::registro(ci) {
                if !r.ind_nfse {
                    out.push(format!(
                        "cIndOp {ci} nao e' de NFS-e no Anexo VII ({}): confirme com o contador",
                        r.tipo_operacao
                    ));
                }
            }
        }
    }
    match cc {
        None => out.push("o Anexo VIII nao traz cClassTrib nesta linha".to_string()),
        Some(cc) => {
            let vale = tabela::classificacao(cc)
                .map(|c| c.vale_em("nfse"))
                .unwrap_or(false);
            if !vale {
                out.push(format!(
                    "cClassTrib {cc} nao lista NFS-e na tabela CFF: confirme com o contador"
                ));
            }
        }
    }
    out
}

fn base() -> &'static Base {
    static B: OnceLock<Base> = OnceLock::new();
    B.get_or_init(|| {
        let arq: Arquivo =
            serde_json::from_str(DADOS).expect("dados/nfse_correlacao.json embutido e' valido");
        let mut por_item: BTreeMap<String, Vec<Correlacao>> = BTreeMap::new();
        for x in &arq.linhas {
            let cst = x.cclasstrib.as_deref().map(|c| {
                tabela::classificacao(c)
                    .expect("cClassTrib do anexo esta na tabela (o extrator confere)")
                    .cst
                    .clone()
            });
            por_item
                .entry(x.item.clone())
                .or_default()
                .push(Correlacao {
                    linha: x.linha,
                    item: x.item.clone(),
                    nbs: x.nbs.clone(),
                    cod_ativ_sn: x.cod_ativ_sn.clone(),
                    cind_op: x.cind_op.clone(),
                    local: x.local.clone(),
                    cclasstrib: x.cclasstrib.clone(),
                    cst,
                    avisos: avisos(x.cind_op.as_deref(), x.cclasstrib.as_deref()),
                });
        }
        Base { arq, por_item }
    })
}

pub fn versao() -> &'static str {
    &base().arq.versao
}

fn digitos(s: &str) -> String {
    s.chars().filter(char::is_ascii_digit).collect()
}

fn erro(m: String) -> ErroNfse {
    ErroNfse::novo(CAMPO, m)
}

/// Item do Anexo VIII a partir do cTribNac ('250101'), do subitem ('25.01.01') ou do item ('25.01', '2501').
/// Os itens 99 (fora do ISS) vao pelo subitem inteiro: '990201' -> '99.02.01'.
pub fn item_de(codigo: &str) -> R<String> {
    let d = digitos(codigo);
    let k = if d.starts_with("99") {
        if d.len() != 6 {
            return Err(erro(format!(
                "{codigo:?}: no Anexo VIII o item 99 vai pelos 6 digitos (ex.: 990201)"
            )));
        }
        format!("{}.{}.{}", &d[..2], &d[2..4], &d[4..])
    } else if d.len() == 4 || d.len() == 6 {
        format!("{}.{}", &d[..2], &d[2..4])
    } else {
        return Err(erro(format!(
            "{codigo:?} nao e' item LC 116 (4 digitos) nem cTribNac (6 digitos)"
        )));
    };
    if !base().por_item.contains_key(&k) {
        return Err(erro(format!(
            "o item {k} nao esta no Anexo VIII v{}; confirme a classificacao com o contador",
            versao()
        )));
    }
    Ok(k)
}

pub fn descricao_item(codigo: &str) -> R<&'static str> {
    let k = item_de(codigo)?;
    Ok(base().arq.itens.get(&k).map_or("", String::as_str))
}

pub fn descricao_nbs(nbs: &str) -> &'static str {
    base().arq.nbs.get(&digitos(nbs)).map_or("", String::as_str)
}

fn nbs9(nbs: &str) -> R<String> {
    let d = digitos(nbs);
    if d.len() != 9 {
        let curto: String = nbs.chars().take(20).collect();
        return Err(erro(format!(
            "NBS {curto:?} nao tem 9 digitos (ex.: 1.2603.00.00 = 126030000)"
        )));
    }
    Ok(d)
}

/// Filtros opcionais de `candidatos`/`sugerir`.
#[derive(Debug, Clone, Copy, Default)]
pub struct Filtro<'a> {
    pub nbs: Option<&'a str>,
    pub cclasstrib: Option<&'a str>,
    pub cod_ativ_sn: Option<&'a str>,
}

fn opcoes<'a>(v: &[&'a Correlacao], f: impl Fn(&'a Correlacao) -> Option<&'a str>) -> String {
    let mut o: Vec<&str> = v.iter().filter_map(|c| f(c)).collect();
    o.sort_unstable();
    o.dedup();
    o.join(", ")
}

/// Linhas do item, filtradas. Filtro que zera recusa dizendo o que o item aceita (nunca lista vazia calada).
pub fn candidatos(codigo: &str, f: Filtro<'_>) -> R<Vec<&'static Correlacao>> {
    let k = item_de(codigo)?;
    let mut v: Vec<&'static Correlacao> = base().por_item[&k].iter().collect();
    if let Some(n) = f.nbs {
        let n = nbs9(n)?;
        let x: Vec<_> = v
            .iter()
            .copied()
            .filter(|c| c.nbs.as_deref() == Some(n.as_str()))
            .collect();
        if x.is_empty() {
            let o = opcoes(&v, |c| c.nbs.as_deref());
            return Err(erro(format!(
                "NBS {n} nao esta no item {k} do Anexo VIII (o item traz: {})",
                if o.is_empty() { "nenhum" } else { &o }
            )));
        }
        v = x;
    }
    if let Some(cc) = f.cclasstrib {
        let cc = cc.trim();
        let x: Vec<_> = v
            .iter()
            .copied()
            .filter(|c| c.cclasstrib.as_deref() == Some(cc))
            .collect();
        if x.is_empty() {
            let o = opcoes(&v, |c| c.cclasstrib.as_deref());
            let curto: String = cc.chars().take(8).collect();
            return Err(erro(format!(
                "cClassTrib {curto} nao esta no item {k} do Anexo VIII para esse filtro (opcoes: {})",
                if o.is_empty() { "nenhuma" } else { &o }
            )));
        }
        v = x;
    }
    if let Some(a) = f.cod_ativ_sn {
        let a = a.trim();
        let x: Vec<_> = v
            .iter()
            .copied()
            .filter(|c| c.cod_ativ_sn.as_deref() == Some(a))
            .collect();
        if x.is_empty() {
            let o = opcoes(&v, |c| c.cod_ativ_sn.as_deref());
            let curto: String = a.chars().take(5).collect();
            return Err(erro(format!(
                "COD_ATIV_SN {curto} nao esta no item {k} do Anexo VIII para esse filtro (opcoes: {})",
                if o.is_empty() { "nenhuma" } else { &o }
            )));
        }
        v = x;
    }
    Ok(v)
}

fn pares(v: &[&'static Correlacao]) -> Vec<(Option<&'static str>, Option<&'static str>)> {
    let mut out = Vec::new();
    for c in v {
        let p = (c.cind_op.as_deref(), c.cclasstrib.as_deref());
        if !out.contains(&p) {
            out.push(p);
        }
    }
    out
}

fn fmt_pares(p: &[(Option<&str>, Option<&str>)]) -> String {
    p.iter()
        .map(|(a, b)| {
            format!(
                "cIndOp {} + cClassTrib {}",
                a.unwrap_or("None"),
                b.unwrap_or("None")
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// A UNICA combinacao (cIndOp, cClassTrib) que o filtro deixa; a primeira linha dela. Mais de uma: erro com as
/// opcoes. Linha sem cIndOp ou sem cClassTrib (99.01.01): erro.
pub fn sugerir(codigo: &str, f: Filtro<'_>) -> R<&'static Correlacao> {
    let v = candidatos(codigo, f)?;
    let p = pares(&v);
    if p.len() > 1 {
        return Err(erro(format!(
            "o item {} tem {} classificacoes no Anexo VIII para esse filtro ({}): informe o NBS ou o cClassTrib -- \
             escolher por voce seria chute fiscal",
            v[0].item,
            p.len(),
            fmt_pares(&p)
        )));
    }
    let c = v[0];
    if c.cind_op.is_none() || c.cclasstrib.is_none() {
        return Err(erro(format!(
            "o Anexo VIII nao traz cIndOp/cClassTrib para o item {} (linha {}); confirme com o contador",
            c.item, c.linha
        )));
    }
    Ok(c)
}

/// cIndOp da tabela (onerosa no pais) -> o da operacao real, pela aba "REGRA OP NAO ONE_EXT" do anexo.
/// So' 100101, 100301 e 100501 tem variante; nao onerosa ao exterior = celula vazia -> erro.
pub fn cind_op_da_operacao(cind_op: &str, onerosa: bool, exterior: bool) -> R<String> {
    let ci = cind_op.trim();
    if onerosa && !exterior {
        return Ok(ci.to_string());
    }
    let regras = &base().arq.regra_nao_onerosa_exterior;
    let Some(r) = regras.get(ci) else {
        let curto: String = ci.chars().take(8).collect();
        return Err(erro(format!(
            "o Anexo VIII so' traz cIndOp de operacao nao onerosa/exterior para {}; para {curto} confirme com o \
             contador",
            regras.keys().cloned().collect::<Vec<_>>().join(", ")
        )));
    };
    let alvo = if onerosa {
        &r.onerosa_exterior
    } else if exterior {
        &r.nao_onerosa_exterior
    } else {
        &r.nao_onerosa
    };
    alvo.clone().ok_or_else(|| {
        erro(format!(
            "o Anexo VIII nao traz cIndOp para operacao nao onerosa com adquirente no exterior (base {ci}); confirme \
             com o contador"
        ))
    })
}

/// Avisos da classificacao informada contra o anexo (vazio = a combinacao esta no Anexo VIII). Nao recusa.
pub fn conferir(codigo: &str, nbs: &str, cind_op: &str, cclasstrib: &str) -> Vec<String> {
    let v = match candidatos(
        codigo,
        Filtro {
            nbs: Some(nbs),
            ..Filtro::default()
        },
    ) {
        Ok(v) => v,
        Err(e) => return vec![e.mensagem],
    };
    let (ci, cc) = (cind_op.trim(), cclasstrib.trim());
    if let Some(c) = v
        .iter()
        .find(|c| c.cind_op.as_deref() == Some(ci) && c.cclasstrib.as_deref() == Some(cc))
    {
        return c.avisos.clone();
    }
    let (ci8, cc8): (String, String) = (ci.chars().take(8).collect(), cc.chars().take(8).collect());
    vec![format!(
        "cIndOp {ci8} + cClassTrib {cc8} nao esta no Anexo VIII v{} para o item {} e o NBS {} (o anexo traz: {})",
        versao(),
        v[0].item,
        v[0].nbs.as_deref().unwrap_or("None"),
        fmt_pares(&pares(&v))
    )]
}
