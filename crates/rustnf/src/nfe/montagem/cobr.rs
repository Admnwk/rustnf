// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Grupo Y `cobr` (fatura e duplicatas, Y01 a Y10), na ordem do XSD PL_010h. Fontes: MOC 7.00, Anexo I, grupo Y: regras
//! 905 (campos da fatura), 901 (desconto maior que o valor original), 902 (liquido), 852 (numero da parcela), 900 e 850
//! (vencimento) e 851 (soma das parcelas contra o liquido da fatura).

use super::formato::*;
use super::modelo::Nota;
use super::{uf_nota, X};

/// Y02 `fat`: a fatura. Com `cobr` informado, `nFat`, `vOrig` e `vLiq` sao obrigatorios (rejeicao 905).
#[derive(Debug, Clone, Default)]
pub struct Fatura {
    /// Y03 `nFat`: de 1 a 60 caracteres.
    pub n_fat: Option<String>,
    /// Y04 `vOrig`: valor original (2 casas).
    pub v_orig: Option<String>,
    /// Y05 `vDesc`: desconto; ausente ou zero = a tag nao sai.
    pub v_desc: Option<String>,
    /// Y06 `vLiq`: valor liquido; igual ao original menos o desconto (rejeicao 902).
    pub v_liq: Option<String>,
}

/// Y07 `dup`: uma parcela.
#[derive(Debug, Clone, Default)]
pub struct Duplicata {
    /// Y08 `nDup`: tres algarismos sequenciais a partir de `001` (rejeicao 852).
    pub n_dup: Option<String>,
    /// Y09 `dVenc`: `AAAA-MM-DD`, na ordem crescente e nao antes da emissao (rejeicoes 900 e 850).
    pub d_venc: Option<String>,
    /// Y10 `vDup`: valor da parcela, maior que zero (2 casas).
    pub v_dup: Option<String>,
}

/// Y01 `cobr`: dados da cobranca.
#[derive(Debug, Clone, Default)]
pub struct Cobranca {
    /// Y02 `fat`.
    pub fat: Option<Fatura>,
    /// Y07 `dup`, de 0 a 120; a soma bate com o `vLiq` (rejeicao 851).
    pub dup: Vec<Duplicata>,
}

fn obrigatorio(v: &Option<String>, campo: &str) -> Res<String> {
    match v.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => Ok(s.to_string()),
        None => Err(erro_r(
            format!(
                "cobr: {campo} nao informado: com a cobranca, nFat, vOrig e vLiq sao obrigatorios"
            ),
            campo,
            "905",
        )),
    }
}

/// A fatura conferida (905, 901, 902) e escrita; devolve o valor liquido para a soma das parcelas.
fn fatura(x: &mut X, f: &Option<Fatura>, ra: bool) -> Res<Dec> {
    let Some(f) = f else {
        return Err(erro_r(
            "cobr sem a fatura: com a cobranca, nFat, vOrig e vLiq sao obrigatorios",
            "fat",
            "905",
        ));
    };
    let n_fat = obrigatorio(&f.n_fat, "nFat")?;
    let orig = Dec::parse(&obrigatorio(&f.v_orig, "vOrig")?, "vOrig")?;
    let liq = Dec::parse(&obrigatorio(&f.v_liq, "vLiq")?, "vLiq")?;
    let desc = dec(&f.v_desc, "vDesc")?;
    if desc.maior(orig) {
        return Err(erro_r("vDesc da fatura maior que o vOrig", "vDesc", "901"));
    }
    if liq.menos(orig.menos(desc)).m != 0 {
        return Err(erro_r(
            format!(
                "vLiq {} difere do vOrig menos o vDesc ({})",
                liq.fixo(2),
                orig.menos(desc).fixo(2)
            ),
            "vLiq",
            "902",
        ));
    }
    x.abre("fat");
    x.t("nFat", &txt(&n_fat, "nFat", 60, 1, ra)?);
    x.t("vOrig", &num_lim(orig, 13, 2, "vOrig")?);
    x.o("vDesc", &num_ou_none(&f.v_desc, 2, "vDesc")?);
    x.t("vLiq", &num_lim(liq, 13, 2, "vLiq")?);
    x.fecha("fat");
    Ok(liq)
}

/// Escreve o `cobr` da nota, se ela tem.
pub(crate) fn cobr(x: &mut X, n: &Nota) -> Res<()> {
    let Some(c) = &n.cobr else { return Ok(()) };
    if c.dup.len() > 120 {
        return Err(erro(
            format!("dup: {} parcelas (maximo 120)", c.dup.len()),
            "dup",
        ));
    }
    let emissao = na_uf(&n.emissao, &uf_nota(n)?).date_naive();
    x.abre("cobr");
    let liq = fatura(x, &c.fat, n.remover_acento)?;
    let mut soma = Dec::ZERO;
    let mut anterior = None;
    for (i, d) in c.dup.iter().enumerate() {
        let esperado = format!("{:03}", i + 1);
        if d.n_dup.as_deref().map(str::trim) != Some(esperado.as_str()) {
            return Err(erro_r(
                format!(
                    "parcela {}: nDup {:?} (as parcelas levam tres algarismos sequenciais: {esperado})",
                    i + 1,
                    d.n_dup.as_deref().unwrap_or("")
                ),
                "nDup",
                "852",
            ));
        }
        let venc = d
            .d_venc
            .as_deref()
            .ok_or_else(|| {
                erro_r(
                    format!("parcela {}: dVenc nao informado", i + 1),
                    "dVenc",
                    "900",
                )
            })
            .and_then(|v| data_xml(v, "dVenc"))?;
        if venc < emissao {
            return Err(erro_r(
                format!(
                    "parcela {}: dVenc {venc} e' antes da emissao ({emissao})",
                    i + 1
                ),
                "dVenc",
                "900",
            ));
        }
        if anterior.is_some_and(|a| venc < a) {
            return Err(erro_r(
                format!(
                    "parcela {}: dVenc {venc} e' antes do vencimento da parcela anterior",
                    i + 1
                ),
                "dVenc",
                "850",
            ));
        }
        anterior = Some(venc);
        let v = Dec::parse(
            d.v_dup
                .as_deref()
                .ok_or_else(|| erro(format!("parcela {}: vDup obrigatorio", i + 1), "vDup"))?,
            "vDup",
        )?;
        if v.cmp_zero() != std::cmp::Ordering::Greater {
            return Err(erro(
                format!("parcela {}: vDup deve ser maior que zero", i + 1),
                "vDup",
            ));
        }
        soma = soma.mais(v);
        x.abre("dup");
        x.t("nDup", &esperado);
        x.t("dVenc", &venc.format("%Y-%m-%d").to_string());
        x.t("vDup", &num_lim(v, 13, 2, "vDup")?);
        x.fecha("dup");
    }
    if !c.dup.is_empty() && soma.menos(liq).m != 0 {
        return Err(erro_r(
            format!(
                "a soma das parcelas ({}) difere do vLiq da fatura ({})",
                soma.fixo(2),
                liq.fixo(2)
            ),
            "vDup",
            "851",
        ));
    }
    x.fecha("cobr");
    Ok(())
}
