// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Grupo LA `comb` (combustivel liquido e lubrificante, LA01 a LA21) na ordem do XSD PL_010h, conferido campo a campo.
//! Fontes: MOC 7.00, Anexo I, grupo LA (regras 461, 855, 856, 379 e 380), regra I14-10 (817) e N12-20 (858); NT 2026.002
//! v1.11 (regra LA18-20, rejeicao 909). O codigo do produto na tabela da ANP (761) e o combustivel da tabela monofasica
//! (908 e LA18-10) dependem de tabelas publicadas a parte e nao sao conferidos aqui. O CFOP de combustivel (660,
//! LA01-20, facultativa) usa o `indComb` da Tabela de CFOP e sai como aviso da montagem (`TipoAviso::CfopCombustivelSemGrupo`),
//! nao como recusa.

use super::formato::*;
use super::item::{digitos_ate, int_faixa};
use super::modelo::{Item, Nota};
use super::{ind_final, X};

/// `cProdANP` do GLP: so' ele leva os percentuais `pGLP`, `pGNn` e `pGNi` e o valor de partida.
pub(crate) const GLP: &str = "210203001";

/// N12-20 (rejeicao 858): combustiveis sujeitos a repasse interestadual; o CST 60 deles vai no grupo `ICMSST`.
pub(crate) const ANP_REPASSE: [&str; 62] = [
    "210203001",
    "320101001",
    "320101002",
    "320102002",
    "320102001",
    "320102003",
    "320102005",
    "320201001",
    "320103001",
    "220102001",
    "320301001",
    "320103002",
    "820101032",
    "820101026",
    "820101027",
    "820101004",
    "820101005",
    "820101022",
    "820101031",
    "820101030",
    "820101014",
    "820101006",
    "820101016",
    "820101015",
    "820101025",
    "820101017",
    "820101018",
    "820101019",
    "820101020",
    "820101021",
    "420105001",
    "420101005",
    "420101004",
    "420102005",
    "420102004",
    "420104001",
    "820101033",
    "820101034",
    "420106001",
    "820101011",
    "820101003",
    "820101013",
    "820101012",
    "420106002",
    "830101001",
    "420301004",
    "420202001",
    "420301001",
    "420301002",
    "410103001",
    "410101001",
    "410102001",
    "430101004",
    "510101001",
    "510101002",
    "510102001",
    "510102002",
    "510201001",
    "510201003",
    "510301003",
    "510103001",
    "510301001",
];

/// LA07 a LA10: CIDE Combustiveis. Zero em todos os valores nao e' recusado: o XSD aceita.
#[derive(Debug, Clone, Default)]
pub struct Cide {
    /// LA08 `qBCProd`: base em quantidade (ate 12 inteiros e 4 casas).
    pub q_bc_prod: Option<String>,
    /// LA09 `vAliqProd`: aliquota em reais por unidade (4 casas).
    pub v_aliq_prod: Option<String>,
    /// LA10 `vCIDE` (2 casas).
    pub v_cide: Option<String>,
}

/// LA11 `encerrante` (NT 2015.002): leitura do contador da bomba. Na NF-e so' com o CFOP 5656 ou 5667 (rejeicao 379).
#[derive(Debug, Clone, Default)]
pub struct Encerrante {
    /// LA12 `nBico`: de 0 a 999.
    pub n_bico: String,
    /// LA14 `nTanque`: de 0 a 999.
    pub n_tanque: String,
    /// LA15 `vEncIni` (3 casas).
    pub v_enc_ini: Option<String>,
    /// LA16 `vEncFin` (3 casas); tem de ser maior que o `vEncIni` (rejeicao 380).
    pub v_enc_fin: Option<String>,
    /// LA13 `nBomba`: de 0 a 999; ausente = a tag nao sai.
    pub n_bomba: Option<String>,
}

/// LA18 `origComb`: UF de origem do combustivel e o percentual dela (de 0 a 30 grupos por item).
#[derive(Debug, Clone, Default)]
pub struct OrigemComb {
    /// LA19 `indImport`: 0 nacional, 1 importado.
    pub ind_import: String,
    /// LA20 `cUFOrig`: codigo IBGE da UF (`"35"`) ou a sigla (`"SP"`); sai o codigo.
    pub c_uf_orig: String,
    /// LA21 `pOrig`: maior que 0 e ate 100, ate 4 casas.
    pub p_orig: Option<String>,
}

/// Grupo LA `comb` do item. No GLP (`cProdANP` 210203001) `pGLP`, `pGNn` e `pGNi` somam 100 (rejeicao 855) e o valor de
/// partida e' obrigatorio (856); em outro produto os percentuais sao recusados (461).
#[derive(Debug, Clone, Default)]
pub struct Combustivel {
    /// LA02 `cProdANP`: 9 digitos (codificacao do SIMP, da ANP).
    pub c_prod_anp: String,
    /// LA03 `descANP`: de 2 a 95 caracteres.
    pub desc_anp: String,
    /// LA06 `UFCons`: sigla da UF de consumo, ou `EX`.
    pub uf_cons: String,
    /// LA03a `pGLP`: so' no GLP; ausente = 0.
    pub p_glp: Option<String>,
    /// LA03b `pGNn`: so' no GLP; ausente = 0.
    pub p_gn_n: Option<String>,
    /// LA03c `pGNi`: so' no GLP; ausente = 0.
    pub p_gn_i: Option<String>,
    /// LA03d `vPart`: so' no GLP, valor por quilograma sem ICMS.
    pub v_part: Option<String>,
    /// LA04 `CODIF`: de 1 a 21 digitos.
    pub codif: Option<String>,
    /// LA05 `qTemp`: 4 casas; zero ou ausente = a tag nao sai (o XSD nao aceita zero).
    pub q_temp: Option<String>,
    /// LA07 `CIDE`.
    pub cide: Option<Cide>,
    /// LA11 `encerrante`.
    pub encerrante: Option<Encerrante>,
    /// LA17 `pBio`: maior que 0 e ate 100; zero ou ausente = a tag nao sai.
    pub p_bio: Option<String>,
    /// LA18 `origComb`, de 0 a 30.
    pub origens: Vec<OrigemComb>,
}

/// CFOP da venda de combustivel a consumidor final, o unico que leva o encerrante na NF-e (rejeicao 379).
const CFOP_ENCERRANTE: [&str; 2] = ["5656", "5667"];
/// LA18-20, excecao 3: remessa para entrega futura dispensa o grupo de origem.
const CFOP_SEM_ORIGEM: [&str; 2] = ["5922", "6922"];

/// Percentual de 0 a 100 (`zero_ok`) ou de mais de 0 a 100, com ate 4 casas.
fn pct(v: &Option<String>, campo: &str, zero_ok: bool) -> Res<Dec> {
    let d = dec(v, campo)?;
    if d.negativo() || d.maior(Dec::de_int(100)) || (d.eh_zero() && !zero_ok) {
        let faixa = if zero_ok {
            "0 a 100"
        } else {
            "maior que 0 e ate 100"
        };
        let bruto = v.as_deref().unwrap_or("").trim();
        return Err(erro(
            format!("{campo}: '{bruto}' fora da faixa ({faixa})"),
            campo,
        ));
    }
    num(d, 4, campo)?;
    Ok(d)
}

/// O percentual no XML: 2 casas, ou 4 quando tem 3 ou 4 (`TDec_0302a04Max100`, `TDec_03v00a04Max100Opc`).
fn pct_txt(d: Dec, campo: &str) -> Res<String> {
    num(d, if d.casas() <= 2 { 2 } else { 4 }, campo)
}

/// Codigo IBGE da UF de origem: o codigo (`"35"`) ou a sigla (`"SP"`).
fn cod_uf(v: &str, campo: &str) -> Res<String> {
    let s = v.trim().to_uppercase();
    if !s.is_empty() && s.len() <= 2 && s.bytes().all(|b| b.is_ascii_digit()) {
        if let Ok(c) = s.parse::<u32>() {
            if uf_sigla(c).is_some() {
                return Ok(c.to_string());
            }
        }
    }
    if let Some(c) = uf_codigo(&s) {
        return Ok(c.to_string());
    }
    Err(erro(
        format!("{campo}: '{s}' nao e' codigo IBGE nem sigla de UF"),
        campo,
    ))
}

fn glp(x: &mut X, c: &Combustivel, it: &Item, k: &str) -> Res<()> {
    let unidade = it
        .unidade_trib
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or(&it.unidade);
    if unidade.trim().to_lowercase() != "kg" {
        return Err(erro_r(
            format!("{k}GLP (cProdANP {GLP}) com uTrib diferente de kg -- I14-10"),
            "uTrib",
            "817",
        ));
    }
    let cem = Dec::de_int(100);
    let mut soma = Dec::ZERO;
    for (tag, v) in [("pGLP", &c.p_glp), ("pGNn", &c.p_gn_n), ("pGNi", &c.p_gn_i)] {
        let campo = format!("{k}{tag}");
        let d = pct(v, &campo, true)?;
        soma = soma.mais(d);
        x.t(tag, &pct_txt(d, &campo)?);
    }
    if soma.menos(cem).m != 0 {
        return Err(erro_r(
            format!(
                "{k}pGLP + pGNn + pGNi = {} (deve ser 100) -- LA03c-20",
                soma.fixo(soma.e)
            ),
            "pGLP",
            "855",
        ));
    }
    if dec(&c.v_part, &format!("{k}vPart"))?.eh_zero() {
        return Err(erro_r(
            format!("{k}GLP sem o valor de partida (vPart, por kg sem ICMS) -- LA03d-10"),
            "vPart",
            "856",
        ));
    }
    let campo = format!("{k}vPart");
    x.t("vPart", &num_lim(dec(&c.v_part, &campo)?, 13, 2, &campo)?);
    Ok(())
}

fn cide(x: &mut X, ci: &Cide, k: &str) -> Res<()> {
    let q = format!("{k}qBCProd");
    let a = format!("{k}vAliqProd");
    let v = format!("{k}vCIDE");
    x.abre("CIDE");
    x.t("qBCProd", &num_lim(dec(&ci.q_bc_prod, &q)?, 12, 4, &q)?);
    x.t("vAliqProd", &num_lim(dec(&ci.v_aliq_prod, &a)?, 11, 4, &a)?);
    x.t("vCIDE", &num_lim(dec(&ci.v_cide, &v)?, 13, 2, &v)?);
    x.fecha("CIDE");
    Ok(())
}

fn encerrante(x: &mut X, e: &Encerrante, it: &Item, k: &str) -> Res<()> {
    let cfop = it.cfop.replace('.', "");
    if !CFOP_ENCERRANTE.contains(&cfop.trim()) {
        return Err(erro_r(
            format!(
                "{k}grupo encerrante na NF-e so' com o CFOP 5656 ou 5667 (venda de combustivel a consumidor final) -- LA11-20"
            ),
            "encerrante",
            "379",
        ));
    }
    let (ni, nf) = (format!("{k}vEncIni"), format!("{k}vEncFin"));
    let ini = dec(&e.v_enc_ini, &ni)?;
    let fim = dec(&e.v_enc_fin, &nf)?;
    let (ini_txt, fim_txt) = (num_lim(ini, 12, 3, &ni)?, num_lim(fim, 12, 3, &nf)?);
    if !fim.maior(ini) {
        return Err(erro_r(
            format!("{k}vEncFin {fim_txt} nao e' maior que o vEncIni {ini_txt} -- LA16-10"),
            "vEncFin",
            "380",
        ));
    }
    x.abre("encerrante");
    x.t(
        "nBico",
        &int_faixa(&e.n_bico, &format!("{k}nBico"), 0, 999, None)?,
    );
    if let Some(b) = e.n_bomba.as_deref().filter(|s| !s.trim().is_empty()) {
        x.t(
            "nBomba",
            &int_faixa(b, &format!("{k}nBomba"), 0, 999, None)?,
        );
    }
    x.t(
        "nTanque",
        &int_faixa(&e.n_tanque, &format!("{k}nTanque"), 0, 999, None)?,
    );
    x.t("vEncIni", &ini_txt);
    x.t("vEncFin", &fim_txt);
    x.fecha("encerrante");
    Ok(())
}

fn origens(x: &mut X, c: &Combustivel, k: &str) -> Res<()> {
    if c.origens.len() > 30 {
        return Err(erro(
            format!("{k}origComb: {} grupos (maximo 30)", c.origens.len()),
            &format!("{k}origComb"),
        ));
    }
    for o in &c.origens {
        let ind = o.ind_import.trim();
        if ind != "0" && ind != "1" {
            return Err(erro(
                format!("{k}indImport: '{ind}' deve ser 0 (nacional) ou 1 (importado)"),
                &format!("{k}indImport"),
            ));
        }
        let campo = format!("{k}pOrig");
        let p = pct(&o.p_orig, &campo, false)?;
        x.abre("origComb");
        x.t("indImport", ind);
        x.t("cUFOrig", &cod_uf(&o.c_uf_orig, &format!("{k}cUFOrig"))?);
        x.t("pOrig", &pct_txt(p, &campo)?);
        x.fecha("origComb");
    }
    Ok(())
}

/// LA18-20 (NT 2026.002 v1.11, rejeicao 909): gas natural no GLP (`pGNn` ou `pGNi` diferente de 0) exige o grupo de
/// origem, salvo consumidor final, nota complementar ou de devolucao, CFOP 5922 ou 6922 e DANFE simplificado.
fn exige_origem(c: &Combustivel, it: &Item, n: &Nota, k: &str) -> Res<()> {
    let tem_gn = !dec(&c.p_gn_n, "pGNn")?.eh_zero() || !dec(&c.p_gn_i, "pGNi")?.eh_zero();
    let cfop = it.cfop.replace('.', "");
    let dispensa = ind_final(n) == 1
        || n.fin_nfe == 2
        || n.fin_nfe == 4
        || CFOP_SEM_ORIGEM.contains(&cfop.trim())
        || super::nfce::varejo(n);
    if tem_gn && c.origens.is_empty() && !dispensa {
        return Err(erro_r(
            format!(
                "{k}GLP com gas natural (pGNn ou pGNi) sem o grupo de origem (origComb) -- LA18-20"
            ),
            "origComb",
            "909",
        ));
    }
    Ok(())
}

/// Escreve o `comb` do item (ultimo filho do `prod`, alternativa do choice `veicProd`, `med`, `arma`, `comb`).
pub(crate) fn combustivel(
    x: &mut X,
    c: &Combustivel,
    it: &Item,
    n: &Nota,
    k: &str,
    ra: bool,
) -> Res<()> {
    let anp = c.c_prod_anp.trim();
    if anp.len() != 9 || !anp.bytes().all(|b| b.is_ascii_digit()) {
        return Err(erro(
            format!("{k}cProdANP: '{anp}' deve ter 9 digitos (codigo do SIMP, da ANP)"),
            &format!("{k}cProdANP"),
        ));
    }
    let uf = c.uf_cons.trim().to_uppercase();
    if uf_codigo(&uf).is_none() && uf != "EX" {
        return Err(erro(
            format!("{k}UFCons: '{uf}' nao e' sigla de UF (nem EX)"),
            &format!("{k}UFCons"),
        ));
    }
    x.abre("comb");
    x.t("cProdANP", anp);
    x.t(
        "descANP",
        &txt(&c.desc_anp, &format!("{k}descANP"), 95, 2, ra)?,
    );
    if anp == GLP {
        glp(x, c, it, k)?;
    } else {
        if [&c.p_glp, &c.p_gn_n, &c.p_gn_i].iter().any(|v| v.is_some()) {
            return Err(erro_r(
                format!("{k}pGLP, pGNn e pGNi so' no GLP (cProdANP {GLP}) -- LA03c-10"),
                "pGLP",
                "461",
            ));
        }
        if c.v_part.is_some() {
            return Err(erro(
                format!("{k}vPart so' no GLP (cProdANP {GLP})"),
                &format!("{k}vPart"),
            ));
        }
    }
    if let Some(cod) = c.codif.as_deref().filter(|s| !s.trim().is_empty()) {
        x.t("CODIF", &digitos_ate(cod, &format!("{k}CODIF"), 21)?);
    }
    let qt = format!("{k}qTemp");
    x.o("qTemp", &num_lim_ou_none(&c.q_temp, 12, 4, &qt)?);
    x.t("UFCons", &uf);
    if let Some(ci) = &c.cide {
        cide(x, ci, k)?;
    }
    if let Some(e) = &c.encerrante {
        encerrante(x, e, it, k)?;
    }
    // zero = nao informado: o XSD (TDec_03v00a04Max100Opc) nao aceita zero
    if !dec(&c.p_bio, &format!("{k}pBio"))?.eh_zero() {
        let campo = format!("{k}pBio");
        x.t("pBio", &pct_txt(pct(&c.p_bio, &campo, false)?, &campo)?);
    }
    origens(x, c, k)?;
    exige_origem(c, it, n, k)?;
    x.fecha("comb");
    Ok(())
}

/// Numero opcional com limite de inteiros: ausente ou zero = a tag nao sai.
fn num_lim_ou_none(
    v: &Option<String>,
    inteiros: usize,
    casas: u32,
    campo: &str,
) -> Res<Option<String>> {
    let d = dec(v, campo)?;
    if d.eh_zero() {
        return Ok(None);
    }
    num_lim(d, inteiros, casas, campo).map(Some)
}

/// N12-20 (rejeicao 858): o CST 60 do combustivel sujeito a repasse interestadual vai no grupo `ICMSST` (`repasse`),
/// nao no `ICMS60`.
pub(crate) fn repasse_do_cst60(it: &Item, k: &str) -> Res<()> {
    let Some(c) = &it.combustivel else {
        return Ok(());
    };
    let Some(icms) = &it.icms else { return Ok(()) };
    if ANP_REPASSE.contains(&c.c_prod_anp.trim()) && icms.codigo.trim() == "60" && !icms.repasse {
        return Err(erro_r(
            format!(
                "{k}CST 60 de combustivel sujeito a repasse interestadual vai no grupo ICMSST (repasse), nao no ICMS60 -- N12-20"
            ),
            "CST",
            "858",
        ));
    }
    Ok(())
}
