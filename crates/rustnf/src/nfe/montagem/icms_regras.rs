// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! As regras do grupo de ICMS do item: ST propria, diferimento, desoneracao, partilha, repasse, FCP, ICMS efetivo e
//! codigo de beneficio fiscal. Cada funcao confere o que o MOC 7.00 (Anexo I) manda antes de o XML ser escrito.

use std::collections::BTreeMap;

use super::formato::*;
use super::modelo::*;

/// Grupos em que a ST propria e' obrigatoria (N18 a N23d); o CST 90 e o CSOSN 900 a levam se informada.
pub(crate) const GRUPOS_ST: [&str; 6] = [
    "ICMS10",
    "ICMS30",
    "ICMS70",
    "ICMSSN201",
    "ICMSSN202",
    "ICMSPart",
];
pub(crate) const ST_OPCIONAL: [&str; 2] = ["ICMS90", "ICMSSN900"];

pub(crate) fn tem_st(c: &Icms) -> bool {
    c.mod_bc_st.is_some()
        || [
            &c.vbc_st,
            &c.p_icms_st,
            &c.v_icms_st,
            &c.p_mva_st,
            &c.p_red_bc_st,
            &c.vbc_fcp_st,
            &c.p_fcp_st,
            &c.v_fcp_st,
        ]
        .iter()
        .any(|v| v.is_some())
}

/// Regras da ST propria: `modBCST` de 0 a 6 (N18), `pMVAST` so' com `modBCST` 4 (932 e 933), CEST obrigatorio
/// na NF-e com ICMS-ST (806; a partilha dispensa, excecao 1) e `pFCPST` diferente de zero (881).
pub(crate) fn conferir_st(c: &Icms, grupo: &str, cest: Option<&str>, k: &str) -> Res<()> {
    if ST_OPCIONAL.contains(&grupo) && !tem_st(c) {
        return Ok(());
    }
    let m = match c.mod_bc_st {
        Some(m) if (0..=6).contains(&m) => m,
        o => {
            let v = o.map_or_else(|| "nao informado".to_string(), |m| m.to_string());
            return Err(erro(
                format!("{k}modBCST obrigatorio, 0..6 (N18): {v}"),
                "modBCST",
            ));
        }
    };
    let mva = c.p_mva_st.is_some() && !dec(&c.p_mva_st, "pMVAST")?.eh_zero();
    if m == 4 && !mva {
        return Err(erro_r(
            format!("{k}modBCST 4 (MVA) sem pMVAST"),
            "pMVAST",
            "932",
        ));
    }
    if m != 4 && mva {
        return Err(erro_r(
            format!("{k}pMVAST com modBCST {m} (so' com 4 = MVA)"),
            "pMVAST",
            "933",
        ));
    }
    let v_st = dec(&c.v_icms_st, "vICMSST")?;
    if cest.is_none_or(|s| s.trim().is_empty())
        && (GRUPOS_ST.contains(&grupo) || !v_st.eh_zero())
        && grupo != "ICMSPart"
    {
        return Err(erro_r(
            format!("{k}operacao com ICMS-ST sem o CEST -- N23-10"),
            "CEST",
            "806",
        ));
    }
    if c.p_fcp_st.is_some() && dec(&c.p_fcp_st, "pFCPST")?.eh_zero() {
        return Err(erro_r(
            format!("{k}pFCPST igual a zero: nao informe o FCP ST de produto sem incidencia"),
            "pFCPST",
            "881",
        ));
    }
    Ok(())
}

/// UF em que o diferimento (CST 51) leva os campos opcionais mesmo zerados (RJ, SP e PR); nas demais, zero nao sai.
pub(crate) const UF_51_ZERADO: [&str; 3] = ["RJ", "SP", "PR"];

pub(crate) fn dif_informado(c: &Icms) -> bool {
    [
        &c.v_icms_op,
        &c.p_dif,
        &c.v_icms_dif,
        &c.p_fcp_dif,
        &c.v_fcp_dif,
        &c.v_fcp_efet,
    ]
    .iter()
    .any(|v| v.is_some())
}

/// Diferimento (CST 51): valores nao negativos, `pDif` ate 100 e as contas 351 (`vICMSOp`), 352 (`vICMSDif`) e 353
/// (`vICMS`), com a tolerancia de 0,01 do MOC.
pub(crate) fn conferir_51(c: &Icms, k: &str) -> Res<()> {
    let pares = [
        ("vBC", &c.vbc),
        ("pICMS", &c.p_icms),
        ("vICMSOp", &c.v_icms_op),
        ("pDif", &c.p_dif),
        ("vICMSDif", &c.v_icms_dif),
        ("vICMS", &c.v_icms),
    ];
    let mut z = BTreeMap::new();
    for (t, v) in pares {
        let d = dec(v, t)?;
        if d.negativo() {
            return Err(erro(format!("{k}CST 51 com {t} negativo"), t));
        }
        z.insert(t, d);
    }
    let pct = Dec::centavos(1); // x / 100
    let tol = Dec::centavos(1);
    if z["pDif"].maior(Dec::de_int(100)) {
        return Err(erro(
            format!("{k}pDif acima de 100 (XSD TDec_0302a04Max100)"),
            "pDif",
        ));
    }
    let op = z["vBC"].vezes(z["pICMS"]).vezes(pct);
    if z["vICMSOp"].menos(op).abs().maior(tol) {
        return Err(erro_r(
            format!(
                "{k}vICMSOp {} difere de vBC x pICMS ({})",
                z["vICMSOp"].fixo(2),
                op.fixo(2)
            ),
            "vICMSOp",
            "351",
        ));
    }
    let dif = z["vICMSOp"].vezes(z["pDif"]).vezes(pct);
    if z["vICMSDif"].menos(dif).abs().maior(tol) {
        return Err(erro_r(
            format!(
                "{k}vICMSDif {} difere de vICMSOp x pDif ({})",
                z["vICMSDif"].fixo(2),
                dif.fixo(2)
            ),
            "vICMSDif",
            "352",
        ));
    }
    let devido = z["vICMSOp"].menos(z["vICMSDif"]);
    if (c.v_icms_dif.is_some() || c.v_icms.is_some()) && z["vICMS"].menos(devido).abs().maior(tol) {
        return Err(erro_r(
            format!(
                "{k}vICMS {} difere de vICMSOp - vICMSDif ({})",
                z["vICMS"].fixo(2),
                devido.fixo(2)
            ),
            "vICMS",
            "353",
        ));
    }
    Ok(())
}

/// Motivos de desoneracao (`motDesICMS`) aceitos em cada grupo, como enumera o XSD PL_010h.
pub(crate) const MOTIVOS_DESON: [(&str, &[&str]); 6] = [
    ("ICMSPart", &["9", "10", "11"]),
    ("ICMS20", &["3", "9", "10", "11", "12"]),
    ("ICMS30", &["6", "7", "9"]),
    (
        "ICMS40",
        &[
            "1", "3", "4", "5", "6", "7", "8", "9", "10", "11", "16", "90",
        ],
    ),
    ("ICMS70", &["3", "9", "12"]),
    ("ICMS90", &["3", "9", "12"]),
];
/// Motivos de desoneracao da ST (`motDesICMSST`) por grupo.
pub(crate) const MOTIVOS_DESON_ST: [(&str, &[&str]); 3] = [
    ("ICMS10", &["3", "9", "12"]),
    ("ICMS70", &["3", "9", "12"]),
    ("ICMS90", &["3", "9", "12"]),
];
/// Rejeicao 626: CFOP da venda isenta para a Zona Franca de Manaus (`motDesICMS` 7), na ordem do MOC.
pub(crate) const CFOP_ZFM: [&str; 36] = [
    "1203", "1204", "1208", "1209", "2203", "2204", "2208", "2209", "5109", "5110", "5120", "5151",
    "5152", "5651", "5652", "5654", "5655", "5658", "5659", "5910", "6905", "6109", "6110", "6120",
    "6122", "6123", "6151", "6152", "6651", "6652", "6654", "6655", "6658", "6659", "6910", "6923",
];

/// CST que levam a partilha (`ICMSPart`) e o repasse (`ICMSST`).
pub(crate) const CST_PARTILHA: [&str; 2] = ["10", "90"];
pub(crate) const CST_REPASSE: [&str; 2] = ["41", "60"];
/// `UFST` da partilha: sigla da UF ou `EX` (exterior).
pub(crate) const UF_ST: [&str; 28] = [
    "AC", "AL", "AM", "AP", "BA", "CE", "DF", "ES", "GO", "MA", "MG", "MS", "MT", "PA", "PB", "PE",
    "PI", "PR", "RJ", "RN", "RO", "RR", "RS", "SC", "SE", "SP", "TO", "EX",
];
/// Levam FCP retido (N27a-N27d) e ICMS efetivo (N34-N37).
pub(crate) const GRUPOS_RET: [&str; 3] = ["ICMS60", "ICMSSN500", "ICMSST"];

pub(crate) fn tem_fcp_ret(c: &Icms) -> bool {
    c.vbc_fcp_st_ret.is_some() || c.p_fcp_st_ret.is_some() || c.v_fcp_st_ret.is_some()
}

pub(crate) fn tem_efet(c: &Icms) -> bool {
    c.p_red_bc_efet.is_some()
        || c.vbc_efet.is_some()
        || c.p_icms_efet.is_some()
        || c.v_icms_efet.is_some()
}

/// Troca o grupo pela partilha (ICMSPart) ou pelo repasse (ICMSST) e recusa o campo fora do grupo dele. `nao_contrib`
/// = NF-e de saida a nao contribuinte SEM veiculo novo na nota: a partilha so' passa na 508 com o veicProd (N12-70 exc. 3).
#[allow(clippy::too_many_arguments)]
pub(crate) fn grupo_especial<'a>(
    c: &Icms,
    grupo: &'a str,
    cod: &str,
    simples: bool,
    k: &str,
    nao_contrib: bool,
) -> Res<&'a str> {
    let tipo = if simples { "CSOSN" } else { "CST" };
    let mut g = grupo;
    if c.partilha && c.repasse {
        return Err(erro(
            format!("{k}partilha e repasse no mesmo item (ICMSPart x ICMSST)"),
            "CST",
        ));
    }
    if c.partilha {
        if simples || !CST_PARTILHA.contains(&cod) {
            return Err(erro(
                format!("{k}partilha (ICMSPart) so' com CST 10 ou 90 (informado {tipo} {cod})"),
                "CST",
            ));
        }
        if nao_contrib {
            return Err(erro_r(
                format!(
                    "{k}partilha (CST {cod}) a nao contribuinte so' com o grupo veicProd (N12-70 \
                                       exc. 3): informe o veiculo do item (veicProd)"
                ),
                "CST",
                "508",
            ));
        }
        g = "ICMSPart";
    } else if c.p_bc_op.is_some() || c.uf_st.is_some() {
        return Err(erro(
            format!("{k}pBCOp/UFST so' na partilha (CST 10 ou 90 com partilha)"),
            "pBCOp",
        ));
    }
    if c.repasse {
        if simples || !CST_REPASSE.contains(&cod) {
            return Err(erro(
                format!("{k}repasse (ICMSST) so' com CST 41 ou 60 (informado {tipo} {cod})"),
                "CST",
            ));
        }
        g = "ICMSST";
    } else if c.vbc_st_dest.is_some() || c.v_icms_st_dest.is_some() {
        return Err(erro(
            format!("{k}vBCSTDest/vICMSSTDest so' no repasse (CST 41 ou 60 com repasse)"),
            "vBCSTDest",
        ));
    }
    if !GRUPOS_RET.contains(&g) && (tem_fcp_ret(c) || tem_efet(c)) {
        return Err(erro(
            format!(
                "{k}{tipo} {cod} nao leva FCP retido nem ICMS efetivo (so' CST 60, CSOSN 500 e o \
                                 repasse)"
            ),
            if tem_fcp_ret(c) {
                "vBCFCPSTRet"
            } else {
                "vBCEfet"
            },
        ));
    }
    Ok(g)
}

/// A partilha confere modBC 0-3, sem FCP proprio nem ST desonerado, pBCOp 0..100 e UFST; devolve a UF em maiuscula.
pub(crate) fn conferir_part(c: &Icms, k: &str) -> Res<String> {
    if !(0..=3).contains(&c.mod_bc) {
        return Err(erro(
            format!("{k}modBC da partilha 0..3 (XSD ICMSPart): {}", c.mod_bc),
            "modBC",
        ));
    }
    if c.vbc_fcp.is_some() || c.p_fcp.is_some() || c.v_fcp.is_some() {
        return Err(erro(
            format!("{k}a partilha (ICMSPart) nao tem FCP proprio no leiaute"),
            "vFCP",
        ));
    }
    if c.v_icms_st_deson.is_some() || c.mot_des_icms_st.is_some() {
        return Err(erro(
            format!("{k}a partilha (ICMSPart) nao tem ICMS-ST desonerado no leiaute"),
            "vICMSSTDeson",
        ));
    }
    let p = dec(exige(&c.p_bc_op, &format!("{k}pBCOp"))?, "pBCOp")?;
    if p.negativo() || p.maior(Dec::de_int(100)) {
        return Err(erro(
            format!(
                "{k}pBCOp fora de 0..100: {}",
                c.p_bc_op.as_deref().unwrap_or("").trim()
            ),
            "pBCOp",
        ));
    }
    let Some(uf) = c.uf_st.as_deref() else {
        return Err(erro(format!("{k}UFST: obrigatorio"), &format!("{k}UFST")));
    };
    let u = uf.trim().to_uppercase();
    if !UF_ST.contains(&u.as_str()) {
        return Err(erro(
            format!("{k}UFST invalida: '{}' (sigla da UF ou EX)", uf.trim()),
            "UFST",
        ));
    }
    Ok(u)
}

pub(crate) fn cod_benef(v: &str, campo: &str, sem: bool) -> Res<String> {
    let s = v.trim();
    if sem && s == "SEM CBENEF" {
        return Ok(s.to_string());
    }
    let n = s.chars().count();
    if (n != 8 && n != 10) || s.chars().any(|ch| !('!'..='\u{ff}').contains(&ch)) {
        return Err(erro(
            format!(
                "{campo}: '{s}' fora do formato do XSD (8 ou 10 caracteres, sem espaco{})",
                if sem { ", ou SEM CBENEF" } else { "" }
            ),
            campo,
        ));
    }
    Ok(s.to_string())
}

pub(crate) fn motivos<'a>(tabela: &'a [(&str, &'a [&str])], grupo: &str) -> Option<&'a [&'a str]> {
    tabela.iter().find(|(g, _)| *g == grupo).map(|(_, m)| *m)
}

pub(crate) fn motivo(v: &Option<String>, campo: &str, k: &str) -> Res<Option<String>> {
    let Some(v) = v else { return Ok(None) };
    let s = v.trim();
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return Err(erro(
            format!("{k}{campo} deve ser o codigo numerico do motivo: '{s}'"),
            campo,
        ));
    }
    let n: u64 = s.parse().map_err(|_| {
        erro(
            format!("{k}{campo} deve ser o codigo numerico do motivo: '{s}'"),
            campo,
        )
    })?;
    Ok(Some(n.to_string()))
}

/// Desoneracao conferida ANTES de sair (nunca XML que a SEFAZ rejeita).
pub(crate) struct Deson {
    pub(crate) v: Dec,
    pub(crate) mot: Option<String>,
    pub(crate) ind: Option<i64>,
    pub(crate) v_st: Dec,
    pub(crate) mot_st: Option<String>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn conferir_deson(
    c: &Icms,
    grupo: &str,
    tipo: &str,
    cfop: &str,
    isuf_dest: bool,
    k: &str,
) -> Res<Deson> {
    let mot = motivo(&c.mot_des_icms, "motDesICMS", k)?;
    let mot_st = motivo(&c.mot_des_icms_st, "motDesICMSST", k)?;
    let v = dec(&c.v_icms_deson, "vICMSDeson")?;
    let v_st = dec(&c.v_icms_st_deson, "vICMSSTDeson")?;
    let ind = c.ind_deduz_deson;
    let tem = c.v_icms_deson.is_some() || mot.is_some() || ind.is_some();
    let tem_st = c.v_icms_st_deson.is_some() || mot_st.is_some();
    let lista = motivos(&MOTIVOS_DESON, grupo);
    let lista_st = motivos(&MOTIVOS_DESON_ST, grupo);
    if tem && lista.is_none() {
        return Err(erro(
            format!(
                "{k}{tipo} {} nao leva ICMS desonerado (vICMSDeson: CST 20/30/40/41/50/70/90)",
                c.codigo
            ),
            "vICMSDeson",
        ));
    }
    if tem_st && lista_st.is_none() {
        return Err(erro(
            format!(
                "{k}{tipo} {} nao leva ICMS-ST desonerado (vICMSSTDeson: CST 10/70/90)",
                c.codigo
            ),
            "vICMSSTDeson",
        ));
    }
    for (t, x) in [("vICMSDeson", v), ("vICMSSTDeson", v_st)] {
        if x.negativo() {
            return Err(erro(format!("{k}{t} negativo"), t));
        }
    }
    if let Some(i) = ind {
        if i != 0 && i != 1 {
            return Err(erro(
                format!("{k}indDeduzDeson 0 ou 1 (N28b): {i}"),
                "indDeduzDeson",
            ));
        }
    }
    if let Some(lista) = lista.filter(|_| tem) {
        if let Some(m) = mot.as_deref().filter(|_| v.eh_zero()) {
            return Err(erro_r(
                format!("{k}motDesICMS {m} com vICMSDeson zerado ou ausente"),
                "vICMSDeson",
                "627",
            ));
        }
        if !v.eh_zero() && mot.is_none() {
            return Err(erro(
                format!("{k}vICMSDeson sem o motDesICMS (N28, obrigatorio no XSD)"),
                "motDesICMS",
            ));
        }
        if v.eh_zero() && ind.is_some() {
            return Err(erro(
                format!("{k}indDeduzDeson sem vICMSDeson"),
                "indDeduzDeson",
            ));
        }
        if let Some(m) = mot.as_deref().filter(|m| !lista.contains(m)) {
            return Err(erro(
                format!(
                    "{k}motDesICMS {m} nao vale no CST {} (XSD {grupo}: {})",
                    c.codigo,
                    lista.join(", ")
                ),
                "motDesICMS",
            ));
        }
        if mot.as_deref() == Some("7") {
            let cfop = cfop.trim();
            if ind != Some(1) {
                return Err(erro_r(
                    format!("{k}motDesICMS 7 (SUFRAMA) exige indDeduzDeson 1 -- N28-12"),
                    "indDeduzDeson",
                    "652",
                ));
            }
            if !isuf_dest && !cfop.starts_with(['1', '2']) {
                return Err(erro_r(format!(
                    "{k}motDesICMS 7 (SUFRAMA) sem a inscricao SUFRAMA do destinatario (dest/ISUF)"), "ISUF", "625"));
            }
            if !CFOP_ZFM.contains(&cfop) {
                return Err(erro_r(
                    format!("{k}motDesICMS 7 (SUFRAMA) com CFOP {cfop} fora da lista da ZFM"),
                    "CFOP",
                    "626",
                ));
            }
        }
    }
    if let Some(lista) = lista_st.filter(|_| tem_st) {
        if let Some(m) = mot_st.as_deref().filter(|_| v_st.eh_zero()) {
            return Err(erro(
                format!("{k}motDesICMSST {m} com vICMSSTDeson zerado ou ausente"),
                "vICMSSTDeson",
            ));
        }
        if !v_st.eh_zero() && mot_st.is_none() {
            return Err(erro(
                format!("{k}vICMSSTDeson sem o motDesICMSST (N33b, obrigatorio no XSD)"),
                "motDesICMSST",
            ));
        }
        if let Some(m) = mot_st.as_deref().filter(|m| !lista.contains(m)) {
            return Err(erro(
                format!(
                    "{k}motDesICMSST {m} nao vale no CST {} (XSD {grupo}: {})",
                    c.codigo,
                    lista.join(", ")
                ),
                "motDesICMSST",
            ));
        }
    }
    Ok(Deson {
        v,
        mot,
        ind,
        v_st,
        mot_st,
    })
}
