// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Grupo `ICMSUFDest` (NA): o DIFAL da venda interestadual a consumidor final nao contribuinte. As regras 694,
//! 695, 697, 698, 699 e 793 sao conferidas antes de sair.

use chrono::Datelike;

use super::difal::{
    aliquota_interestadual, exige_grupo, partilha, pct, proibe_grupo, Contexto, ORIG_IMPORTADO,
};
use super::formato::*;
use super::modelo::*;
use super::total::Ac;
use super::*;

/// Percentual com 2 casas, ou 4 quando tem 3 ou 4 (`TDec_0302a04`).
fn pct_xml_txt(v: &str, campo: &str) -> Res<String> {
    let d = Dec::parse(v, campo)?;
    num(d, 4, campo)?;
    if d.casas() <= 2 {
        num(d, 2, campo)
    } else {
        num(d, 4, campo)
    }
}

/// Escreve o `ICMSUFDest` do item (se ele tem) ou confere que a nota nao o exige (694).
pub(crate) fn icms_uf_dest(x: &mut X, n: &Nota, it: &Item, ac: &mut Ac, k: &str) -> Res<()> {
    let g = it.icms_uf_dest.as_ref();
    let mut c = Contexto::novo(
        n.id_dest,
        ind_final(n),
        n.dest.as_ref().map_or(9, |d| d.ind_ie_dest),
    );
    if let Some(d) = it.difal_dispensado.as_deref() {
        if d != "cfop_retorno" && d != "cfop_remessa" {
            return Err(erro(
                format!("{k}difal_dispensado invalido: '{d}' (cfop_retorno | cfop_remessa)"),
                "difal_dispensado",
            ));
        }
    }
    let Some(g) = g else {
        c.tp_nf = n.tp_nf;
        c.fin_nfe = n.fin_nfe;
        c.crt = n.emit.crt;
        c.cst = it
            .icms
            .as_ref()
            .map(|i| i.codigo.clone())
            .unwrap_or_default();
        c.cfop = it.cfop.clone();
        // NA01-20, excecao 1: a partilha dispensa o DIFAL
        c.tem_icms_part = it.icms.as_ref().is_some_and(|i| i.partilha);
        if it.difal_dispensado.is_none() && exige_grupo(&c) {
            return Err(erro_r(format!("{k}venda interestadual a consumidor final nao contribuinte sem o grupo ICMSUFDest \
                                       (DIFAL) -- calcule-o com nfe::montagem::difal::calcular"), "ICMSUFDest", "694"));
        }
        return Ok(());
    };
    if proibe_grupo(&c) {
        return Err(erro_r(format!("{k}grupo ICMSUFDest fora de venda interestadual a consumidor final nao contribuinte -- NA01-30"),
                          "ICMSUFDest", "695"));
    }
    let p_inter = Dec::parse(&g.p_icms_inter, "pICMSInter")?;
    let p_txt = p_inter.fixo(2);
    let orig = it.icms.as_ref().map_or(0, |i| i.orig);
    if (p_txt == "4.00") != ORIG_IMPORTADO.contains(&orig) {
        return Err(erro_r(
            format!("{k}pICMSInter {p_txt} incompativel com orig {orig} -- NA09-20"),
            "pICMSInter",
            "697",
        ));
    }
    let uf_e = n.emit.endereco.uf.clone();
    let uf_d = n
        .dest
        .as_ref()
        .and_then(|d| d.endereco.as_ref())
        .map(|e| e.uf.clone())
        .unwrap_or_default();
    if p_txt != "4.00"
        && n.tp_nf == 1
        && n.fin_nfe == 1
        && !uf_d.is_empty()
        && !uf_d.eq_ignore_ascii_case(&uf_e)
    {
        let esp = aliquota_interestadual(&uf_e, &uf_d, 0).map_err(|e| erro(e, "pICMSInter"))?;
        if p_txt != esp {
            return Err(erro_r(format!("{k}pICMSInter {p_txt} incompativel com {uf_e}->{uf_d} (esperado {esp}) -- NA09-30"),
                              "pICMSInter", "698"));
        }
    }
    if n.fin_nfe == 1 {
        let ano = na_uf(&n.emissao, &uf_nota(n)?).year();
        let esp = partilha(ano).map_err(|e| erro(e, "pICMSInterPart"))?;
        if Dec::parse(&g.p_icms_inter_part, "pICMSInterPart")?.fixo(2) != esp {
            return Err(erro_r(
                format!("{k}pICMSInterPart difere do previsto para {ano} ({esp}) -- NA11-10"),
                "pICMSInterPart",
                "699",
            ));
        }
    }
    let fcp = match (g.vbc_fcp_uf_dest, g.p_fcp_uf_dest.as_ref(), g.v_fcp_uf_dest) {
        (Some(b), Some(p), Some(v)) => {
            let pf = pct(p, "pFCPUFDest").map_err(|e| erro(e, "pFCPUFDest"))?;
            // vBC (centavos) x p (x10000) / 1000000 = centavos; tolerancia de 1 centavo do MOC
            if (i128::from(b) * pf - i128::from(v) * 1_000_000).abs() > 1_000_000 {
                return Err(erro_r(
                    format!("{k}vFCPUFDest difere de vBCFCPUFDest x pFCPUFDest -- NA13-10"),
                    "vFCPUFDest",
                    "793",
                ));
            }
            Some((b, p.clone(), v))
        }
        _ => None,
    };
    let cent = |c: i64, campo: &str| -> Res<String> {
        if c < 0 {
            return Err(erro(
                format!("{campo} deve ser inteiro em centavos >= 0"),
                campo,
            ));
        }
        num(Dec::centavos(c), 2, campo)
    };
    x.abre("ICMSUFDest");
    x.t("vBCUFDest", &cent(g.vbc_uf_dest, "vBCUFDest")?);
    if let Some((b, p, _)) = &fcp {
        x.t("vBCFCPUFDest", &cent(*b, "vBCFCPUFDest")?);
        x.t("pFCPUFDest", &pct_xml_txt(p, "pFCPUFDest")?);
    }
    x.t(
        "pICMSUFDest",
        &pct_xml_txt(&g.p_icms_uf_dest, "pICMSUFDest")?,
    );
    x.t("pICMSInter", &p_txt);
    x.t(
        "pICMSInterPart",
        &pct_xml_txt(&g.p_icms_inter_part, "pICMSInterPart")?,
    );
    if let Some((_, _, v)) = &fcp {
        x.t("vFCPUFDest", &cent(*v, "vFCPUFDest")?);
    }
    x.t("vICMSUFDest", &cent(g.v_icms_uf_dest, "vICMSUFDest")?);
    x.t("vICMSUFRemet", &cent(g.v_icms_uf_remet, "vICMSUFRemet")?);
    x.fecha("ICMSUFDest");
    ac.add("vFCPUFDest", Dec::centavos(fcp.map_or(0, |f| f.2)));
    ac.add("vICMSUFDest", Dec::centavos(g.v_icms_uf_dest));
    ac.add("vICMSUFRemet", Dec::centavos(g.v_icms_uf_remet));
    Ok(())
}
