// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O que o `prod` leva alem do produto comum: a rastreabilidade (`rastro`, I80 a I85), o medicamento (`med`, K01a a K06)
//! e a regra do XSD de que o item leva um grupo especifico so': `veicProd`, `med`, `arma` ou `comb`. Fontes: MOC 7.00,
//! Anexo I, grupos I08 (rejeicoes 870 e 877) e K (rejeicao 873, redacao da NT 2026.002 v1.11); XSD PL_010h v1.03.

use super::formato::*;
use super::modelo::{Item, Nota};
use super::{ind_pres, tp_imp, uf_nota, X};

/// Detalhamento de produto sujeito a rastreabilidade (I80), de 0 a 500 por item.
#[derive(Debug, Clone, Default)]
pub struct Rastro {
    /// I81 `nLote`: de 1 a 20 caracteres.
    pub n_lote: String,
    /// I82 `qLote`: ate 8 inteiros e 3 casas.
    pub q_lote: Option<String>,
    /// I83 `dFab`: `AAAA-MM-DD`; nao pode ser depois da emissao (rejeicao 877).
    pub d_fab: String,
    /// I84 `dVal`: `AAAA-MM-DD`; nao pode ser antes da fabricacao (rejeicao 870).
    pub d_val: String,
    /// I85 `cAgreg`: de 1 a 20 caracteres.
    pub c_agreg: Option<String>,
}

/// Medicamento e materia-prima farmaceutica (grupo K, `med`).
#[derive(Debug, Clone, Default)]
pub struct Medicamento {
    /// K01a `cProdANVISA`: numero do registro (11 ou 13 digitos) ou o literal `ISENTO`.
    pub c_prod_anvisa: String,
    /// K01b `xMotivoIsencao`: numero da decisao que isenta o registro (de 1 a 255 caracteres).
    pub x_motivo_isencao: Option<String>,
    /// K06 `vPMC`: preco maximo ao consumidor (2 casas).
    pub v_pmc: Option<String>,
}

/// O item leva so' um dos grupos especificos do produto (choice do XSD): veiculo novo, medicamento ou combustivel.
pub(crate) fn um_grupo_so(it: &Item, k: &str) -> Res<()> {
    let tem = [
        (it.veiculo.is_some(), "veicProd"),
        (it.med.is_some(), "med"),
        (it.combustivel.is_some(), "comb"),
    ];
    let nomes: Vec<&str> = tem.iter().filter(|t| t.0).map(|t| t.1).collect();
    if nomes.len() > 1 {
        return Err(erro(
            format!(
                "{k}{} no mesmo item: o XSD aceita um grupo so' entre veicProd, med, arma e comb",
                nomes.join(" e ")
            ),
            nomes[1],
        ));
    }
    Ok(())
}

/// `rastro` do item, na ordem do XSD (depois de `nItemPed`, antes do grupo especifico).
pub(crate) fn rastros(x: &mut X, n: &Nota, it: &Item, k: &str, ra: bool) -> Res<()> {
    if it.rastro.len() > 500 {
        return Err(erro(
            format!("{k}rastro: {} grupos (maximo 500)", it.rastro.len()),
            &format!("{k}rastro"),
        ));
    }
    if it.rastro.is_empty() {
        return Ok(());
    }
    let emissao = na_uf(&n.emissao, &uf_nota(n)?).date_naive();
    for r in &it.rastro {
        let fab = data_xml(&r.d_fab, &format!("{k}dFab"))?;
        let val = data_xml(&r.d_val, &format!("{k}dVal"))?;
        // a data de processamento nao existe na montagem; a emissao nunca e' posterior a ela
        if fab > emissao {
            return Err(erro_r(
                format!(
                    "{k}dFab {fab} e' depois da emissao ({emissao}) -- I83-10: a fabricacao nao pode passar da data de processamento"
                ),
                "dFab",
                "877",
            ));
        }
        if val < fab {
            return Err(erro_r(
                format!("{k}dVal {val} e' antes da fabricacao ({fab}) -- I84-10"),
                "dVal",
                "870",
            ));
        }
        let q = format!("{k}qLote");
        let qlote = Dec::parse(
            r.q_lote
                .as_deref()
                .ok_or_else(|| erro(format!("{q}: obrigatorio"), &q))?,
            &q,
        )?;
        x.abre("rastro");
        x.t("nLote", &txt(&r.n_lote, &format!("{k}nLote"), 20, 1, ra)?);
        x.t("qLote", &num_lim(qlote, 8, 3, &q)?);
        x.t("dFab", &fab.format("%Y-%m-%d").to_string());
        x.t("dVal", &val.format("%Y-%m-%d").to_string());
        x.o(
            "cAgreg",
            &txt_ou_none(&r.c_agreg, &format!("{k}cAgreg"), 20, 1, ra)?,
        );
        x.fecha("rastro");
    }
    Ok(())
}

/// K01-20 (rejeicao 873): o medicamento exige o grupo `rastro`, salvo devolucao (`finNFe` 4), venda nao presencial
/// (`indPres` 2 ou 3), venda para entrega futura ou a ordem, nota de entrada e DANFE simplificado (`tpImp` 6).
fn exige_rastro(n: &Nota, it: &Item) -> bool {
    const CFOP_DISPENSA: [&str; 8] = [
        "5922", "6922", "5118", "6118", "5119", "6119", "5120", "6120",
    ];
    let cfop = it.cfop.replace('.', "");
    !(n.fin_nfe == 4
        || matches!(ind_pres(n), 2 | 3)
        || CFOP_DISPENSA.contains(&cfop.trim())
        || n.tp_nf == 0
        || tp_imp(n) == 6)
}

/// `med` do item: registro na ANVISA, motivo da isencao e preco maximo ao consumidor.
pub(crate) fn medicamento(
    x: &mut X,
    n: &Nota,
    it: &Item,
    m: &Medicamento,
    k: &str,
    ra: bool,
) -> Res<()> {
    let campo = format!("{k}cProdANVISA");
    // o XSD (PL_010h) aceita 11 ou 13 digitos; o MOC 7.00 ainda descreve 6 ou 13
    let reg = m.c_prod_anvisa.trim().to_uppercase();
    let digitos_ok = matches!(reg.len(), 11 | 13) && reg.bytes().all(|b| b.is_ascii_digit());
    if !digitos_ok && reg != "ISENTO" {
        return Err(erro(
            format!(
                "{campo}: '{}' deve ter 11 ou 13 digitos ou ser ISENTO",
                corta(&reg, 20)
            ),
            &campo,
        ));
    }
    if it.rastro.is_empty() && exige_rastro(n, it) {
        return Err(erro_r(
            format!("{k}medicamento sem o grupo rastro (lote, fabricacao e validade) -- K01-20"),
            "rastro",
            "873",
        ));
    }
    let v = format!("{k}vPMC");
    let pmc = Dec::parse(
        m.v_pmc
            .as_deref()
            .ok_or_else(|| erro(format!("{v}: obrigatorio"), &v))?,
        &v,
    )?;
    x.abre("med");
    x.t("cProdANVISA", &reg);
    x.o(
        "xMotivoIsencao",
        &txt_ou_none(
            &m.x_motivo_isencao,
            &format!("{k}xMotivoIsencao"),
            255,
            1,
            ra,
        )?,
    );
    x.t("vPMC", &num_lim(pmc, 13, 2, &v)?);
    x.fecha("med");
    Ok(())
}
