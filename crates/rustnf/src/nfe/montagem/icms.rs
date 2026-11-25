// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Grupo de ICMS do item (N): CST do regime normal e CSOSN do Simples, a ST propria, o diferimento (CST 51), a
//! desoneracao, a partilha, o repasse, o FCP, o ICMS efetivo e o codigo de beneficio fiscal. Cada grupo sai
//! na ordem do XSD PL_010h e as regras do MOC 7.00 (Anexo I) sao conferidas antes: nunca sai XML que a
//! SEFAZ rejeita por uma regra conhecida.

use super::formato::*;
use super::icms_regras::{
    cod_benef, conferir_51, conferir_deson, conferir_part, conferir_st, dif_informado,
    grupo_especial, tem_efet, tem_fcp_ret, tem_st, GRUPOS_ST, ST_OPCIONAL, UF_51_ZERADO,
};
use super::modelo::*;
use super::total::{Ac, DESON_DEDUZ};
use super::*;

/// Grupo do XSD para o CST (regime normal) ou o CSOSN (Simples) informado.
pub(crate) fn grupo_icms(cod: &str, simples: bool) -> Option<&'static str> {
    if simples {
        match cod {
            "101" => Some("ICMSSN101"),
            "102" | "103" | "300" | "400" => Some("ICMSSN102"),
            "201" => Some("ICMSSN201"),
            "202" | "203" => Some("ICMSSN202"),
            "500" => Some("ICMSSN500"),
            "900" => Some("ICMSSN900"),
            _ => None,
        }
    } else {
        match cod {
            "00" => Some("ICMS00"),
            "10" => Some("ICMS10"),
            "20" => Some("ICMS20"),
            "30" => Some("ICMS30"),
            "40" | "41" | "50" => Some("ICMS40"),
            "51" => Some("ICMS51"),
            "60" => Some("ICMS60"),
            "70" => Some("ICMS70"),
            "90" => Some("ICMS90"),
            _ => None,
        }
    }
}

#[allow(clippy::too_many_arguments)]
/// Escreve o grupo `ICMS` do item e soma os valores no acumulador dos totais.
pub(crate) fn icms(
    x: &mut X,
    c: &Icms,
    crt: i64,
    ac: &mut Ac,
    k: &str,
    it: &Item,
    uf: &str,
    isuf_dest: bool,
    fin: i64,
    nao_contrib: bool,
) -> Res<()> {
    let cest = it.cest.as_deref();
    let cod = c.codigo.trim();
    if !(0..=8).contains(&c.orig) {
        return Err(erro(format!("{k}orig invalida: {} (0..8)", c.orig), "orig"));
    }
    let simples = crt == 1 || crt == 4;
    let Some(grupo) = grupo_icms(cod, simples) else {
        let (tipo, lista) = if simples {
            ("CSOSN", "101, 102, 103, 201, 202, 203, 300, 400, 500, 900")
        } else {
            ("CST", "00, 10, 20, 30, 40, 41, 50, 51, 60, 70, 90")
        };
        return Err(erro(
            format!("{k}{tipo} '{cod}' fora do escopo desta montagem (CRT {crt}: {lista})"),
            tipo,
        ));
    };
    let grupo = grupo_especial(c, grupo, cod, simples, k, nao_contrib)?;
    let uf_st = if grupo == "ICMSPart" {
        conferir_part(c, k)?
    } else {
        String::new()
    };
    let com_st = GRUPOS_ST.contains(&grupo) || ST_OPCIONAL.contains(&grupo);
    if !com_st && tem_st(c) {
        return Err(erro(
            format!(
                "{k}{} {cod} nao leva ICMS-ST proprio (ST: CST 10/30/70/90, CSOSN 201/202/203/900)",
                if simples { "CSOSN" } else { "CST" }
            ),
            "modBCST",
        ));
    }
    if com_st {
        conferir_st(c, grupo, cest, k)?;
    }
    if grupo != "ICMS51" && dif_informado(c) {
        return Err(erro(
            format!(
                "{k}{} {cod} nao leva diferimento (vICMSOp/pDif/vICMSDif/FCP diferido: so' CST 51)",
                if simples { "CSOSN" } else { "CST" }
            ),
            "pDif",
        ));
    }
    if grupo == "ICMS51" {
        conferir_51(c, k)?;
    }
    let ds = conferir_deson(
        c,
        grupo,
        if simples { "CSOSN" } else { "CST" },
        &it.cfop,
        isuf_dest,
        k,
    )?;
    let cbr = match c.c_benef_rbc.as_deref().filter(|s| !s.trim().is_empty()) {
        None => None,
        Some(v) => {
            if grupo != "ICMS51" && grupo != "ICMS90" {
                return Err(erro(
                    format!(
                        "{k}{} {cod} nao leva cBenefRBC (so' CST 51 e 90)",
                        if simples { "CSOSN" } else { "CST" }
                    ),
                    "cBenefRBC",
                ));
            }
            if grupo == "ICMS90" && c.vbc.is_none() && c.v_icms.is_none() {
                return Err(erro(
                    format!("{k}cBenefRBC no CST 90 so' com o ICMS proprio (vBC/pICMS/vICMS)"),
                    "cBenefRBC",
                ));
            }
            Some(cod_benef(v, &format!("{k}cBenefRBC"), false)?)
        }
    };
    x.abre("ICMS");
    x.abre(grupo);
    x.t("orig", &c.orig.to_string());
    x.t(if simples { "CSOSN" } else { "CST" }, cod);

    let proprio = |x: &mut X, ac: &mut Ac, red: bool, obrig: bool| -> Res<()> {
        if !obrig && c.vbc.is_none() && c.v_icms.is_none() {
            return Ok(());
        }
        x.t("modBC", &c.mod_bc.to_string());
        let antes = grupo == "ICMS20" || grupo == "ICMS70"; // XSD: pRedBC obrigatorio ANTES do vBC
        if red && antes {
            x.t(
                "pRedBC",
                &num_txt(exige(&c.p_red_bc, &format!("{k}pRedBC"))?, 4, "pRedBC")?,
            );
        }
        x.t(
            "vBC",
            &num_txt(exige(&c.vbc, &format!("{k}vBC"))?, 2, "vBC")?,
        );
        if red && !antes {
            x.o("pRedBC", &num_ou_none(&c.p_red_bc, 4, "pRedBC")?);
        }
        if let Some(b) = &cbr {
            x.t("cBenefRBC", b); // N14a (so' o ICMS90 chega aqui)
        }
        x.t(
            "pICMS",
            &num_txt(exige(&c.p_icms, &format!("{k}pICMS"))?, 4, "pICMS")?,
        );
        x.t(
            "vICMS",
            &num_txt(exige(&c.v_icms, &format!("{k}vICMS"))?, 2, "vICMS")?,
        );
        ac.add("vBC", dec(&c.vbc, "vBC")?);
        ac.add("vICMS", dec(&c.v_icms, "vICMS")?);
        Ok(())
    };
    let fcp = |x: &mut X, ac: &mut Ac, com_base: bool| -> Res<()> {
        if c.v_fcp.is_none() || dec(&c.v_fcp, "vFCP")?.eh_zero() {
            return Ok(());
        }
        if com_base {
            x.t(
                "vBCFCP",
                &num_txt(exige(&c.vbc_fcp, &format!("{k}vBCFCP"))?, 2, "vBCFCP")?,
            );
        }
        x.t(
            "pFCP",
            &num_txt(exige(&c.p_fcp, &format!("{k}pFCP"))?, 4, "pFCP")?,
        );
        x.t("vFCP", &num_txt(&c.v_fcp, 2, "vFCP")?);
        ac.add("vFCP", dec(&c.v_fcp, "vFCP")?);
        Ok(())
    };
    let st_ret = |x: &mut X| -> Res<()> {
        if c.vbc_st_ret.is_none() && c.v_icms_st_ret.is_none() {
            return Ok(());
        }
        x.t(
            "vBCSTRet",
            &num_txt(
                exige(&c.vbc_st_ret, &format!("{k}vBCSTRet"))?,
                2,
                "vBCSTRet",
            )?,
        );
        x.t(
            "pST",
            &num_txt(exige(&c.p_st, &format!("{k}pST"))?, 4, "pST")?,
        );
        if c.v_icms_substituto.is_some() {
            x.t(
                "vICMSSubstituto",
                &num_txt(&c.v_icms_substituto, 2, "vICMSSubstituto")?,
            );
        }
        x.t(
            "vICMSSTRet",
            &num_txt(
                exige(&c.v_icms_st_ret, &format!("{k}vICMSSTRet"))?,
                2,
                "vICMSSTRet",
            )?,
        );
        Ok(())
    };
    // N27a-N27d: os 3 ou nenhum; 860 (vFCPSTRet = vBCFCPSTRet x pFCPSTRet na finNFe 1, tolerancia 0,01); W06b soma.
    let fcp_ret = |x: &mut X, ac: &mut Ac| -> Res<()> {
        if !tem_fcp_ret(c) {
            return Ok(());
        }
        let base = dec(
            exige(&c.vbc_fcp_st_ret, &format!("{k}vBCFCPSTRet"))?,
            "vBCFCPSTRet",
        )?;
        let p = dec(
            exige(&c.p_fcp_st_ret, &format!("{k}pFCPSTRet"))?,
            "pFCPSTRet",
        )?;
        let v = dec(
            exige(&c.v_fcp_st_ret, &format!("{k}vFCPSTRet"))?,
            "vFCPSTRet",
        )?;
        for (t, d) in [("vBCFCPSTRet", base), ("pFCPSTRet", p), ("vFCPSTRet", v)] {
            if d.negativo() {
                return Err(erro(format!("{k}{t} negativo"), t));
            }
        }
        let conta = base.vezes(p).vezes(Dec::centavos(1));
        if fin == 1 && v.menos(conta).abs().maior(Dec::centavos(1)) {
            return Err(erro_r(
                format!(
                    "{k}vFCPSTRet {} difere de vBCFCPSTRet x pFCPSTRet ({})",
                    v.fixo(2),
                    conta.fixo(2)
                ),
                "vFCPSTRet",
                "860",
            ));
        }
        x.t("vBCFCPSTRet", &num(base, 2, "vBCFCPSTRet")?);
        x.t("pFCPSTRet", &num(p, 4, "pFCPSTRet")?);
        x.t("vFCPSTRet", &num(v, 2, "vFCPSTRet")?);
        ac.add("vFCPSTRet", v);
        Ok(())
    };
    // N34-N37: os 4 ou nenhum (opcional a criterio da UF, 906 nao conferida).
    let efetivo = |x: &mut X| -> Res<()> {
        if !tem_efet(c) {
            return Ok(());
        }
        for (tag, v, casas) in [
            ("pRedBCEfet", &c.p_red_bc_efet, 4),
            ("vBCEfet", &c.vbc_efet, 2),
            ("pICMSEfet", &c.p_icms_efet, 4),
            ("vICMSEfet", &c.v_icms_efet, 2),
        ] {
            let d = dec(exige(v, &format!("{k}{tag}"))?, tag)?;
            if d.negativo() {
                return Err(erro(format!("{k}{tag} negativo"), tag));
            }
            x.t(tag, &num(d, casas, tag)?);
        }
        Ok(())
    };
    // N10b: vBCSTRet, pST?, vICMSSubstituto?, vICMSSTRet, [FCP retido], vBCSTDest, vICMSSTDest, [efetivo]. Nao soma no
    // vBCST/vST do total (N31/N32 nao sao N21/N23).
    let repasse = |x: &mut X, ac: &mut Ac| -> Res<()> {
        x.t(
            "vBCSTRet",
            &num_txt(
                exige(&c.vbc_st_ret, &format!("{k}vBCSTRet"))?,
                2,
                "vBCSTRet",
            )?,
        );
        x.o("pST", &num_ou_none(&c.p_st, 4, "pST")?);
        if c.v_icms_substituto.is_some() {
            x.t(
                "vICMSSubstituto",
                &num_txt(&c.v_icms_substituto, 2, "vICMSSubstituto")?,
            );
        }
        x.t(
            "vICMSSTRet",
            &num_txt(
                exige(&c.v_icms_st_ret, &format!("{k}vICMSSTRet"))?,
                2,
                "vICMSSTRet",
            )?,
        );
        fcp_ret(x, ac)?;
        x.t(
            "vBCSTDest",
            &num_txt(
                exige(&c.vbc_st_dest, &format!("{k}vBCSTDest"))?,
                2,
                "vBCSTDest",
            )?,
        );
        x.t(
            "vICMSSTDest",
            &num_txt(
                exige(&c.v_icms_st_dest, &format!("{k}vICMSSTDest"))?,
                2,
                "vICMSSTDest",
            )?,
        );
        efetivo(x)
    };
    let cred_sn = |x: &mut X, obrig: bool| -> Res<()> {
        if !obrig && c.p_cred_sn.is_none() && c.v_cred_icms_sn.is_none() {
            return Ok(());
        }
        x.t(
            "pCredSN",
            &num_txt(exige(&c.p_cred_sn, &format!("{k}pCredSN"))?, 4, "pCredSN")?,
        );
        x.t(
            "vCredICMSSN",
            &num_txt(
                exige(&c.v_cred_icms_sn, &format!("{k}vCredICMSSN"))?,
                2,
                "vCredICMSSN",
            )?,
        );
        Ok(())
    };
    let st = |x: &mut X, ac: &mut Ac| -> Res<()> {
        if ST_OPCIONAL.contains(&grupo) && !tem_st(c) {
            return Ok(());
        }
        x.t("modBCST", &c.mod_bc_st.unwrap_or_default().to_string());
        x.o("pMVAST", &num_ou_none(&c.p_mva_st, 4, "pMVAST")?);
        x.o("pRedBCST", &num_ou_none(&c.p_red_bc_st, 4, "pRedBCST")?);
        x.t(
            "vBCST",
            &num_txt(exige(&c.vbc_st, &format!("{k}vBCST"))?, 2, "vBCST")?,
        );
        x.t(
            "pICMSST",
            &num_txt(exige(&c.p_icms_st, &format!("{k}pICMSST"))?, 4, "pICMSST")?,
        );
        x.t(
            "vICMSST",
            &num_txt(exige(&c.v_icms_st, &format!("{k}vICMSST"))?, 2, "vICMSST")?,
        );
        // W05/W06/W06a: so' com item de ST (como o DIFAL)
        ac.add("vBCST", dec(&c.vbc_st, "vBCST")?);
        ac.add("vST", dec(&c.v_icms_st, "vICMSST")?);
        ac.add("vFCPST", Dec::ZERO);
        if c.vbc_fcp_st.is_none() && c.p_fcp_st.is_none() && c.v_fcp_st.is_none() {
            return Ok(());
        }
        x.t(
            "vBCFCPST",
            &num_txt(
                exige(&c.vbc_fcp_st, &format!("{k}vBCFCPST"))?,
                2,
                "vBCFCPST",
            )?,
        );
        x.t(
            "pFCPST",
            &num_txt(exige(&c.p_fcp_st, &format!("{k}pFCPST"))?, 4, "pFCPST")?,
        );
        x.t(
            "vFCPST",
            &num_txt(exige(&c.v_fcp_st, &format!("{k}vFCPST"))?, 2, "vFCPST")?,
        );
        ac.add("vFCPST", dec(&c.v_fcp_st, "vFCPST")?);
        Ok(())
    };
    let diferimento = |x: &mut X, ac: &mut Ac| -> Res<()> {
        let zerado = UF_51_ZERADO.contains(&uf);
        let o = |x: &mut X, tag: &str, v: &Option<String>, casas: u32| -> Res<()> {
            match num_ou_none(v, casas, tag)? {
                Some(s) => x.t(tag, &s),
                None if zerado => x.t(tag, &Dec::ZERO.fixo(casas)),
                None => {}
            }
            Ok(())
        };
        let mut base = false;
        for v in [&c.vbc, &c.p_red_bc, &c.p_icms, &c.v_icms_op] {
            base |= !dec(v, "vBC")?.eh_zero();
        }
        if zerado || base {
            x.t("modBC", &c.mod_bc.to_string());
        }
        o(x, "pRedBC", &c.p_red_bc, 4)?;
        if let Some(b) = &cbr {
            x.t("cBenefRBC", b); // N14a: depois do pRedBC (XSD ICMS51)
        }
        o(x, "vBC", &c.vbc, 2)?;
        o(x, "pICMS", &c.p_icms, 4)?;
        o(x, "vICMSOp", &c.v_icms_op, 2)?;
        o(x, "pDif", &c.p_dif, 4)?;
        o(x, "vICMSDif", &c.v_icms_dif, 2)?;
        o(x, "vICMS", &c.v_icms, 2)?;
        ac.add("vBC", dec(&c.vbc, "vBC")?);
        ac.add("vICMS", dec(&c.v_icms, "vICMS")?);
        fcp(x, ac, true)?;
        let mut tem = false;
        for v in [&c.p_fcp_dif, &c.v_fcp_dif, &c.v_fcp_efet] {
            tem |= !dec(v, "pFCPDif")?.eh_zero();
        }
        if !tem {
            return Ok(());
        }
        x.t(
            "pFCPDif",
            &num_txt(exige(&c.p_fcp_dif, &format!("{k}pFCPDif"))?, 4, "pFCPDif")?,
        );
        x.t(
            "vFCPDif",
            &num_txt(exige(&c.v_fcp_dif, &format!("{k}vFCPDif"))?, 2, "vFCPDif")?,
        );
        x.o("vFCPEfet", &num_ou_none(&c.v_fcp_efet, 2, "vFCPEfet")?);
        Ok(())
    };
    let deson = |x: &mut X, ac: &mut Ac| -> Res<()> {
        if ds.v.eh_zero() {
            return Ok(());
        }
        x.t("vICMSDeson", &num(ds.v, 2, "vICMSDeson")?);
        x.t("motDesICMS", ds.mot.as_deref().unwrap_or_default());
        if let Some(i) = ds.ind {
            x.t("indDeduzDeson", &i.to_string());
        }
        ac.add("vICMSDeson", ds.v);
        if ds.ind == Some(1) {
            ac.add(DESON_DEDUZ, ds.v);
        }
        Ok(())
    };
    // N33a/N33b: so' com valor > 0. Nao tem total proprio no ICMSTot nem entra no vNF.
    let deson_st = |x: &mut X| -> Res<()> {
        if !ds.v_st.eh_zero() {
            x.t("vICMSSTDeson", &num(ds.v_st, 2, "vICMSSTDeson")?);
            x.t("motDesICMSST", ds.mot_st.as_deref().unwrap_or_default());
        }
        Ok(())
    };
    match grupo {
        "ICMS00" => {
            proprio(x, ac, false, true)?;
            fcp(x, ac, false)?;
        }
        "ICMS10" => {
            proprio(x, ac, false, true)?;
            fcp(x, ac, true)?;
            st(x, ac)?;
            deson_st(x)?;
        }
        "ICMS20" => {
            proprio(x, ac, true, true)?;
            fcp(x, ac, true)?;
            deson(x, ac)?;
        }
        "ICMS30" => {
            st(x, ac)?;
            deson(x, ac)?;
        }
        "ICMS40" => deson(x, ac)?,
        "ICMS51" => diferimento(x, ac)?,
        "ICMS60" | "ICMSSN500" => {
            st_ret(x)?;
            fcp_ret(x, ac)?;
            efetivo(x)?;
        }
        "ICMSST" => repasse(x, ac)?,
        "ICMSPart" => {
            proprio(x, ac, true, true)?; // pRedBC depois do vBC (XSD ICMSPart)
            st(x, ac)?;
            x.t("pBCOp", &num_txt(&c.p_bc_op, 4, "pBCOp")?);
            x.t("UFST", &uf_st);
            deson(x, ac)?;
        }
        "ICMS70" => {
            proprio(x, ac, true, true)?;
            fcp(x, ac, true)?;
            st(x, ac)?;
            deson(x, ac)?;
            deson_st(x)?;
        }
        "ICMS90" => {
            proprio(x, ac, true, false)?;
            fcp(x, ac, true)?;
            st(x, ac)?;
            deson(x, ac)?;
            deson_st(x)?;
        }
        "ICMSSN101" => cred_sn(x, true)?,
        "ICMSSN201" => {
            st(x, ac)?;
            cred_sn(x, true)?;
        }
        "ICMSSN202" => st(x, ac)?,
        "ICMSSN900" => {
            proprio(x, ac, true, false)?;
            st(x, ac)?;
            cred_sn(x, false)?;
        }
        _ => {}
    }
    x.fecha(grupo);
    x.fecha("ICMS");
    Ok(())
}
