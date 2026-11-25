// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! DIFAL do ICMS na venda interestadual a consumidor final NAO contribuinte (grupo `ICMSUFDest`, NA01 a NA17): o
//! calculo da partilha e as condicoes em que o grupo e' obrigatorio (rejeicao 694) ou proibido (695). Fontes: MOC 7.00,
//! Anexo I, grupo NA e regras 694 a 699 e 793; Convenio ICMS 236/2021, clausula segunda; XSD PL_010h. Valores em
//! centavos; aliquotas em texto (`"18"`, `"17,5"`, ate 4 casas).

pub(crate) const SUL_SUDESTE_SEM_ES: [&str; 6] = ["PR", "SC", "RS", "SP", "RJ", "MG"];
pub(crate) const UFS: [&str; 27] = [
    "AC", "AL", "AM", "AP", "BA", "CE", "DF", "ES", "GO", "MA", "MG", "MS", "MT", "PA", "PB", "PE",
    "PI", "PR", "RJ", "RN", "RO", "RR", "RS", "SC", "SE", "SP", "TO",
];
pub(crate) const ORIG_IMPORTADO: [i64; 4] = [1, 2, 3, 8];
pub(crate) const CFOP_FORA: [&str; 3] = ["6552", "6922", "6929"];
pub(crate) const ANP_DERIVADO_OK: [&str; 16] = [
    "820101001",
    "820101010",
    "810102001",
    "810102004",
    "810102002",
    "810102003",
    "810101002",
    "810101001",
    "810101003",
    "220101003",
    "220101004",
    "220101002",
    "220101001",
    "220101005",
    "220101006",
    "560101001",
];
pub(crate) const CST_SEM_DIFAL: [&str; 5] = ["40", "41", "103", "300", "400"];

/// Percentual com ate 4 casas para inteiro x 10000 (`"18"` vira 180000).
pub(crate) fn pct(txt: &str, nome: &str) -> Result<i128, String> {
    let ruim = || format!("{nome} invalido (0 a 100, ate 4 casas)");
    let t = txt.trim().replace(',', ".");
    let (i, f) = t.split_once('.').unwrap_or((&t, ""));
    if i.is_empty()
        || !i.bytes().all(|b| b.is_ascii_digit())
        || !f.bytes().all(|b| b.is_ascii_digit())
        || t.ends_with('.')
    {
        return Err(if i.is_empty() && f.is_empty() {
            format!("{nome} invalido")
        } else {
            ruim()
        });
    }
    let f = f.trim_end_matches('0');
    if f.len() > 4 {
        return Err(ruim());
    }
    let x = i.parse::<i128>().map_err(|_| ruim())? * 10_000
        + if f.is_empty() {
            0
        } else {
            format!("{f:0<4}").parse::<i128>().map_err(|_| ruim())?
        };
    if x > 1_000_000 {
        return Err(ruim());
    }
    Ok(x)
}

/// x (% x 10_000) -> "18.00" ou "17.1250" (2 casas, ou 4 quando tem 3-4; TDec_0302a04).
#[must_use]
fn pct_xml(x: i128) -> String {
    if x % 100 == 0 {
        format!("{}.{:02}", x / 10_000, (x % 10_000) / 100)
    } else {
        format!("{}.{:04}", x / 10_000, x % 10_000)
    }
}

fn uf(v: &str, nome: &str) -> Result<String, String> {
    let u = v.trim().to_uppercase();
    if UFS.contains(&u.as_str()) {
        Ok(u)
    } else {
        Err(format!("{nome} invalida"))
    }
}

/// Aliquota interestadual: "4.00" (`orig` 1, 2, 3 ou 8, importado), "7.00" (de Sul e Sudeste, exceto ES, para as demais
/// UF) ou "12.00".
pub fn aliquota_interestadual(
    uf_origem: &str,
    uf_destino: &str,
    orig: i64,
) -> Result<String, String> {
    let (o, d) = (uf(uf_origem, "uf_origem")?, uf(uf_destino, "uf_destino")?);
    if o == d {
        return Err("mesma UF: nao ha aliquota interestadual".into());
    }
    if !(0..=8).contains(&orig) {
        return Err("orig invalida (0 a 8)".into());
    }
    if ORIG_IMPORTADO.contains(&orig) {
        return Ok("4.00".into());
    }
    if SUL_SUDESTE_SEM_ES.contains(&o.as_str()) && !SUL_SUDESTE_SEM_ES.contains(&d.as_str()) {
        return Ok("7.00".into());
    }
    Ok("12.00".into())
}

/// pICMSInterPart pelo ano (40/60/80 em 2016-2018, 100 depois).
pub fn partilha(ano: i32) -> Result<String, String> {
    match ano {
        ..=2015 => Err("ano anterior a 2016: nao ha grupo ICMSUFDest".into()),
        2016 => Ok("40.00".into()),
        2017 => Ok("60.00".into()),
        2018 => Ok("80.00".into()),
        _ => Ok("100.00".into()),
    }
}

/// O que a nota e o item dizem para decidir se o grupo e' obrigatorio ([`exige_grupo`]) ou proibido ([`proibe_grupo`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Contexto {
    pub(crate) id_dest: i64,
    pub ind_final: i64,
    pub ind_ie_dest: i64,
    pub tp_nf: i64,
    pub fin_nfe: i64,
    pub crt: i64,
    pub cst: String,
    pub cfop: String,
    pub tem_issqn: bool,
    pub cod_anp: String,
    pub uf_emitente: String,
    pub uf_entrega: String,
    pub cfop_retorno: bool,
    pub cfop_remessa: bool,
    pub tem_icms_part: bool,
}

impl Contexto {
    #[must_use]
    pub(crate) fn novo(id_dest: i64, ind_final: i64, ind_ie_dest: i64) -> Self {
        Contexto {
            id_dest,
            ind_final,
            ind_ie_dest,
            tp_nf: 1,
            fin_nfe: 1,
            crt: 3,
            cst: "00".into(),
            cfop: String::new(),
            tem_issqn: false,
            cod_anp: String::new(),
            uf_emitente: String::new(),
            uf_entrega: String::new(),
            cfop_retorno: false,
            cfop_remessa: false,
            tem_icms_part: false,
        }
    }
    fn base(&self) -> bool {
        self.id_dest == 2 && self.ind_final == 1 && self.ind_ie_dest == 9 && !self.tem_issqn
    }
    fn anp_fora(&self) -> bool {
        !self.cod_anp.is_empty() && !ANP_DERIVADO_OK.contains(&self.cod_anp.as_str())
    }
}

/// O grupo e' OBRIGATORIO (rejeicao 694)? Nao, nos retornos, remessas, CFOP e CST sem DIFAL, no Simples (CRT 1), na
/// partilha (ICMSPart), na entrada, nas finalidades 2 e 3 e quando a entrega e' na UF do emitente.
#[must_use]
pub(crate) fn exige_grupo(c: &Contexto) -> bool {
    if !c.base()
        || c.tem_icms_part
        || c.tp_nf == 0
        || c.cfop_retorno
        || c.cfop_remessa
        || matches!(c.fin_nfe, 2 | 3)
        || c.crt == 1
    {
        return false;
    }
    if c.anp_fora() {
        return false;
    }
    if !c.uf_entrega.is_empty()
        && !c.uf_emitente.is_empty()
        && c.uf_entrega.eq_ignore_ascii_case(&c.uf_emitente)
    {
        return false;
    }
    !(CFOP_FORA.contains(&c.cfop.replace('.', "").as_str())
        || CST_SEM_DIFAL.contains(&c.cst.as_str()))
}

/// Informar o grupo aqui da rejeicao 695? (entrega em UF diferente da do emitente libera).
#[must_use]
pub(crate) fn proibe_grupo(c: &Contexto) -> bool {
    if !c.uf_entrega.is_empty()
        && !c.uf_emitente.is_empty()
        && !c.uf_entrega.eq_ignore_ascii_case(&c.uf_emitente)
    {
        return false;
    }
    !c.base() || c.anp_fora()
}

/// Os campos do grupo `ICMSUFDest` (NA03-NA17): centavos e aliquotas em texto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grupo {
    pub vbc_uf_dest: i64,
    pub vbc_fcp_uf_dest: Option<i64>,
    pub p_fcp_uf_dest: Option<String>,
    pub p_icms_uf_dest: String,
    pub p_icms_inter: String,
    pub p_icms_inter_part: String,
    pub v_fcp_uf_dest: Option<i64>,
    pub v_icms_uf_dest: i64,
    pub v_icms_uf_remet: i64,
}

/// Entrada do [`calcular`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entrada {
    pub vbc: i64,
    pub aliquota_interna: String,
    pub aliquota_inter: String,
    pub ano: i32,
    pub fcp: String,
    pub vbc_uf_dest: Option<i64>,
}

fn arred(num: i128, den: i128) -> i128 {
    (2 * num + den) / (2 * den)
}

/// Calcula o grupo `ICMSUFDest` do item pela clausula segunda do Convenio ICMS 236/2021: base unica, ou a base do
/// destino quando informada (`vbc_uf_dest`). A partilha e' a do ano (`partilha`).
pub fn calcular(e: &Entrada) -> Result<Grupo, String> {
    if e.vbc < 0 {
        return Err("vbc deve ser inteiro (centavos) >= 0".into());
    }
    let bd = e.vbc_uf_dest.unwrap_or(e.vbc);
    if bd < e.vbc {
        return Err("vbc_uf_dest deve ser inteiro (centavos) >= vbc".into());
    }
    let p_int = pct(&e.aliquota_interna, "aliquota_interna")?;
    let p_inter = pct(&e.aliquota_inter, "aliquota_inter")?;
    let p_fcp = pct(&e.fcp, "fcp")?;
    if ![40_000, 70_000, 120_000].contains(&p_inter) {
        return Err("aliquota_inter deve ser 4, 7 ou 12 (pICMSInter)".into());
    }
    if p_fcp > 20_000 {
        return Err("fcp acima de 2 pontos (Conv. ICMS 236/21 cl. 2a par. 4o; MOC NA05)".into());
    }
    let part_txt = partilha(e.ano)?;
    let part = pct(&part_txt, "partilha")?;
    // dif em centavos x 1_000_000 (aliquota x 10_000 / 100%)
    let dif = (i128::from(bd) * p_int - i128::from(e.vbc) * p_inter).max(0);
    let total = arred(dif, 1_000_000);
    let dest = arred(dif * part, 1_000_000 * 1_000_000);
    let c = |x: i128| i64::try_from(x).map_err(|_| "valor fora do alcance".to_string());
    let mut g = Grupo {
        vbc_uf_dest: bd,
        vbc_fcp_uf_dest: None,
        p_fcp_uf_dest: None,
        p_icms_uf_dest: pct_xml(p_int),
        p_icms_inter: pct_xml(p_inter),
        p_icms_inter_part: pct_xml(part),
        v_fcp_uf_dest: None,
        v_icms_uf_dest: c(dest)?,
        v_icms_uf_remet: c(total - dest)?,
    };
    if p_fcp > 0 {
        g.vbc_fcp_uf_dest = Some(bd);
        g.p_fcp_uf_dest = Some(pct_xml(p_fcp));
        g.v_fcp_uf_dest = Some(c(arred(i128::from(bd) * p_fcp, 1_000_000))?);
    }
    Ok(g)
}
