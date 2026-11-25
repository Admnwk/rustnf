// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Chave de acesso da NFS-e Nacional (50 posicoes) e da NF-e/NFC-e (44 posicoes, modelos 55 e 65): conferencia de
//! formato e digito verificador, montagem, decomposicao, busca em texto e a validacao de CPF e CNPJ (numerico e
//! alfanumerico).
//!
//! O DV e' modulo 11 com pesos 2..9 da direita e `Ord(ch) - 48`, de modo que a letra do CNPJ alfanumerico pesa
//! como valor ASCII menos 48 (Nota Tecnica conjunta do CNPJ alfanumerico, Receita Federal:
//! <https://www.gov.br/receitafederal/pt-br/acesso-a-informacao/acoes-e-programas/programas-e-atividades/cnpj-alfanumerico>).
//! A estrutura da chave vem do XSD publicado no portal da NFS-e (tipo `TSChaveNFSe`): municipio (7), ambiente
//! gerador (1), tipo de inscricao (1), inscricao (14), numero (13), AAMM (4), codigo (9) e DV (1).
//!
//! A chave da NF-e e da NFC-e tem 44 posicoes: UF (2), AAMM (4), CNPJ ou `000` + CPF do emitente (14), modelo (2),
//! serie (3), numero (9), forma de emissao `tpEmis` (1), codigo numerico `cNF` (8) e DV (1) -- Anexo I do MOC 7.00,
//! campos B02 a B25 e regra B09-30 (`cNF`); a serie 910 a 969 e' a do emitente CPF (campo B07, NT 2018.001). O DV e'
//! o mesmo modulo 11 (MOC, Visao Geral, item 5.4).

use std::fmt;

/// Por que uma chave de acesso foi recusada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErroChave {
    /// Nao tem 50 posicoes depois de tirar a pontuacao e o prefixo de letras.
    Tamanho,
    /// Posicao com caractere que o XSD nao aceita (letra so' no CNPJ alfanumerico).
    Formato,
    /// O ultimo digito nao e' o modulo 11 dos 49 anteriores.
    DigitoVerificador,
    /// Os 2 primeiros digitos do municipio nao sao um codigo de UF do IBGE.
    UfInexistente,
    /// Ambiente gerador diferente de 1 (prefeitura) e de 2 (Sefin Nacional).
    AmbienteGerador,
    /// Mes do AAMM fora de 01..12.
    Mes,
    /// Emitente que nao e' CPF (11 digitos) nem CNPJ (14 posicoes).
    Emitente,
    /// A chave da NF-e/NFC-e nao tem 44 posicoes depois de tirar a pontuacao e o prefixo de letras.
    TamanhoDfe,
    /// Posicao que o leiaute da NF-e nao aceita (6 digitos, 12 posicoes do CNPJ, 26 digitos).
    FormatoDfe,
    /// Os 2 primeiros digitos nao sao um codigo de UF do IBGE.
    Uf,
    /// Modelo diferente de 55 (NF-e) e 65 (NFC-e).
    Modelo,
    /// CPF do emitente com digito verificador invalido.
    CpfEmitente,
    /// CNPJ do emitente com digito verificador invalido.
    CnpjEmitente,
    /// Numero da nota zero (o numero comeca em 1).
    NumeroZero,
    /// `cNF` que a regra B09-30 recusa (sequencia conhecida ou igual ao numero da nota).
    CodigoNumerico,
    /// Numero que nao cabe no campo (`nome`, com `tamanho` digitos).
    NaoCabe {
        /// Nome do campo no leiaute.
        nome: &'static str,
        /// Quantos digitos o campo comporta.
        tamanho: usize,
    },
}

impl fmt::Display for ErroChave {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tamanho => f.write_str("a chave da NFS-e tem 50 posicoes"),
            Self::Formato => f.write_str("formato da NFS-e (XSD): 50 posicoes, letra so' no CNPJ"),
            Self::DigitoVerificador => f.write_str("digito verificador nao confere"),
            Self::UfInexistente => f.write_str("municipio de UF inexistente"),
            Self::AmbienteGerador => f.write_str("ambiente gerador invalido (1 ou 2)"),
            Self::Mes => f.write_str("mes (AAMM) invalido"),
            Self::Emitente => {
                f.write_str("emitente = CPF (11 digitos) ou CNPJ (14, alfanumerico aceito)")
            }
            Self::TamanhoDfe => f.write_str("a chave da NF-e e da NFC-e tem 44 posicoes"),
            Self::FormatoDfe => f.write_str(
                "formato da chave: 6 digitos, 12 posicoes do CNPJ [0-9A-Z] e 26 digitos",
            ),
            Self::Uf => f.write_str("codigo de UF inexistente"),
            Self::NumeroZero => f.write_str("o numero da nota comeca em 1"),
            Self::Modelo => f.write_str("modelo diferente de 55 (NF-e) e 65 (NFC-e)"),
            Self::CpfEmitente => f.write_str("CPF do emitente invalido"),
            Self::CnpjEmitente => f.write_str("CNPJ do emitente invalido"),
            Self::CodigoNumerico => f.write_str(
                "cNF recusado pela regra B09-30 (sequencia conhecida ou igual ao numero)",
            ),
            Self::NaoCabe { nome, tamanho } => {
                write!(f, "{nome} precisa de ate {tamanho} digitos")
            }
        }
    }
}

impl std::error::Error for ErroChave {}

/// Codigos IBGE de UF (`TCodUfIBGE` do XSD).
const UFS: [(u32, &str); 27] = [
    (11, "RO"),
    (12, "AC"),
    (13, "AM"),
    (14, "RR"),
    (15, "PA"),
    (16, "AP"),
    (17, "TO"),
    (21, "MA"),
    (22, "PI"),
    (23, "CE"),
    (24, "RN"),
    (25, "PB"),
    (26, "PE"),
    (27, "AL"),
    (28, "SE"),
    (29, "BA"),
    (31, "MG"),
    (32, "ES"),
    (33, "RJ"),
    (35, "SP"),
    (41, "PR"),
    (42, "SC"),
    (43, "RS"),
    (50, "MS"),
    (51, "MT"),
    (52, "GO"),
    (53, "DF"),
];

/// Sigla da UF do codigo IBGE (`None` se o codigo nao e' de UF).
fn sigla_uf(cod: u32) -> Option<&'static str> {
    UFS.iter().find(|(c, _)| *c == cod).map(|(_, s)| *s)
}

fn alnum_up(c: char) -> bool {
    c.is_ascii_digit() || c.is_ascii_uppercase()
}

fn digitos(s: &str) -> bool {
    s.bytes().all(|b| b.is_ascii_digit())
}

fn n(s: &str) -> u32 {
    s.parse().unwrap_or(0)
}

/// So' `[0-9A-Z]`, em maiusculas.
fn so_alnum(valor: &str) -> String {
    valor
        .chars()
        .map(|c| c.to_ascii_uppercase())
        .filter(|c| alnum_up(*c))
        .collect()
}

/// A chave "crua": em maiusculas, so' `[0-9A-Z]` e a partir do primeiro par de digitos (o `Id` da NFS-e e o da NF-e
/// trazem letras na frente: `NFS`, `NFe`).
#[must_use]
pub fn limpar(valor: &str) -> String {
    let v = so_alnum(valor);
    let b = v.as_bytes();
    (0..b.len().saturating_sub(1))
        .find(|&i| b[i].is_ascii_digit() && b[i + 1].is_ascii_digit())
        .map_or_else(String::new, |i| v[i..].to_string())
}

/// DV das 43 (NF-e, NFC-e) ou 49 (NFS-e) primeiras posicoes da chave: modulo 11, pesos 2..9 da direita,
/// `Ord(ch) - 48`.
pub(crate) fn dv(base: &str) -> Result<u32, ErroChave> {
    if !matches!(base.len(), 43 | 49) {
        return Err(ErroChave::Tamanho);
    }
    if !base.chars().all(alnum_up) {
        return Err(ErroChave::Formato);
    }
    let soma: u32 = base
        .bytes()
        .rev()
        .zip((2..=9).cycle())
        .map(|(ch, peso)| (u32::from(ch) - 48) * peso)
        .sum();
    let resto = soma % 11;
    Ok(if resto < 2 { 0 } else { 11 - resto })
}

fn formato(c: &str) -> bool {
    if c.len() != 50 || !c.is_ascii() || !digitos(&c[..8]) || !digitos(&c[23..]) {
        return false;
    }
    match &c[8..9] {
        "1" => digitos(&c[9..23]),
        "2" => c[9..20].chars().all(alnum_up) && digitos(&c[20..23]),
        _ => false,
    }
}

/// CPF com DV (11 digitos, nao repetidos).
#[must_use]
pub fn cpf_valido(d: &str) -> bool {
    if d.len() != 11 || !digitos(d) || d.bytes().all(|b| b == d.as_bytes()[0]) {
        return false;
    }
    let v: Vec<u32> = d.bytes().map(|b| u32::from(b - b'0')).collect();
    [9usize, 10].into_iter().all(|t| {
        let s: u32 = v[..t]
            .iter()
            .zip((2..=t as u32 + 1).rev())
            .map(|(d, p)| d * p)
            .sum();
        (s * 10 % 11) % 10 == v[t]
    })
}

/// CNPJ numerico ou alfanumerico com DV.
#[must_use]
pub fn cnpj_valido(c: &str) -> bool {
    const PESOS: [u32; 13] = [6, 5, 4, 3, 2, 9, 8, 7, 6, 5, 4, 3, 2];
    if c.len() != 14
        || !c.is_ascii()
        || !c[..12].chars().all(alnum_up)
        || !digitos(&c[12..])
        || c.bytes().all(|b| b == b'0')
    {
        return false;
    }
    let v: Vec<u32> = c.bytes().map(|b| u32::from(b) - 48).collect();
    [12usize, 13].into_iter().all(|k| {
        let s: u32 = v[..k]
            .iter()
            .zip(&PESOS[13 - k..])
            .map(|(d, p)| d * p)
            .sum();
        let d = if s % 11 < 2 { 0 } else { 11 - s % 11 };
        d == v[k]
    })
}

fn num(valor: u64, tamanho: usize, nome: &'static str) -> Result<String, ErroChave> {
    let s = valor.to_string();
    if s.len() > tamanho {
        return Err(ErroChave::NaoCabe { nome, tamanho });
    }
    Ok(format!("{s:0>tamanho$}"))
}

/// `2609`, `2026-09`, `202609` -> `2609` (mes 01..12).
fn aamm(v: &str) -> Result<String, ErroChave> {
    let d: String = v.chars().filter(char::is_ascii_digit).collect();
    let s = if d.len() == 6 { d[2..].to_string() } else { d };
    if s.len() != 4 || !(1..=12).contains(&n(&s[2..])) {
        return Err(ErroChave::Mes);
    }
    Ok(s)
}

/// (inscricao em 14 posicoes, tipo de inscricao da chave: `1` CPF, `2` CNPJ).
fn emitente(doc: &str) -> Result<(String, char), ErroChave> {
    let d = so_alnum(doc);
    if d.len() == 11 && digitos(&d) {
        return Ok((format!("000{d}"), '1'));
    }
    if d.len() == 14 && digitos(&d[12..]) {
        return Ok((d, '2'));
    }
    Err(ErroChave::Emitente)
}

/// Monta a chave da NFS-e Nacional (50 posicoes). `ambiente_gerador`: 1 prefeitura, 2 Sefin Nacional.
/// `codigo: None` sorteia o codigo numerico de 9 digitos.
pub fn montar_nfse(
    cmun: u64,
    inscricao: &str,
    numero: u64,
    competencia: &str,
    codigo: Option<u64>,
    ambiente_gerador: u32,
) -> Result<String, ErroChave> {
    let cm = num(cmun, 7, "cMun")?;
    if sigla_uf(n(&cm[..2])).is_none() {
        return Err(ErroChave::UfInexistente);
    }
    if !matches!(ambiente_gerador, 1 | 2) {
        return Err(ErroChave::AmbienteGerador);
    }
    let (insc, tipo) = emitente(inscricao)?;
    let codigo = codigo
        .unwrap_or_else(|| u64::from_le_bytes(crate::aleatorio::bytes::<8>()) % 1_000_000_000);
    let base = format!(
        "{cm}{ambiente_gerador}{tipo}{insc}{}{}{}",
        num(numero, 13, "nNFSe")?,
        aamm(competencia)?,
        num(codigo, 9, "cNum")?
    );
    if !formato(&format!("{base}0")) {
        return Err(ErroChave::Formato);
    }
    let d = dv(&base)?;
    Ok(format!("{base}{d}"))
}

/// Confere a chave da NFS-e Nacional: tamanho, formato do XSD, digito verificador, UF, ambiente gerador e mes.
/// Pontuacao, espacos e o prefixo de letras do `Id` sao ignorados.
pub fn conferir(chave: &str) -> Result<(), ErroChave> {
    let c = limpar(chave);
    if c.len() != 50 {
        return Err(ErroChave::Tamanho);
    }
    if !formato(&c) {
        return Err(ErroChave::Formato);
    }
    if dv(&c[..49])? != n(&c[49..]) {
        return Err(ErroChave::DigitoVerificador);
    }
    if sigla_uf(n(&c[..2])).is_none() {
        return Err(ErroChave::UfInexistente);
    }
    if !matches!(&c[7..8], "1" | "2") {
        return Err(ErroChave::AmbienteGerador);
    }
    if !(1..=12).contains(&n(&c[38..40])) {
        return Err(ErroChave::Mes);
    }
    Ok(())
}

// ---------------------------------------------------------------- NF-e e NFC-e (44 posicoes)

/// Modelos da chave de 44 posicoes que a biblioteca trata: 55 (NF-e) e 65 (NFC-e).
const MODELOS: [&str; 2] = ["55", "65"];

/// Valores que a regra B09-30 do Anexo I do MOC 7.00 recusa como `cNF` (alem de `cNF` igual ao numero da nota).
const CNF_RECUSADOS: [u64; 20] = [
    0, 11_111_111, 22_222_222, 33_333_333, 44_444_444, 55_555_555, 66_666_666, 77_777_777,
    88_888_888, 99_999_999, 12_345_678, 23_456_789, 34_567_890, 45_678_901, 56_789_012, 67_890_123,
    78_901_234, 89_012_345, 90_123_456, 1_234_567,
];

fn formato_dfe(c: &str) -> bool {
    c.len() == 44
        && c.is_ascii()
        && digitos(&c[..6])
        && c[6..18].chars().all(alnum_up)
        && digitos(&c[18..])
}

/// O `cNF` (8 digitos) e' aceito pela regra B09-30: fora das sequencias recusadas e diferente do numero da nota.
#[must_use]
pub fn codigo_valido(codigo: u64, numero: u64) -> bool {
    codigo < 100_000_000 && codigo != numero && !CNF_RECUSADOS.contains(&codigo)
}

/// `cNF` aleatorio (do sistema) que passa em [`codigo_valido`].
#[must_use]
pub fn gerar_codigo(numero: u64) -> u64 {
    loop {
        let c = u64::from_le_bytes(crate::aleatorio::bytes::<8>()) % 100_000_000;
        if codigo_valido(c, numero) {
            return c;
        }
    }
}

/// Campos de [`montar`]. `codigo: None` sorteia um `cNF` valido.
#[derive(Debug, Clone)]
pub struct Montar<'a> {
    /// Codigo IBGE da UF do emitente.
    pub cuf: u32,
    /// Mes da emissao: `2610`, `2026-10` ou `202610`.
    pub aamm: &'a str,
    /// CNPJ (numerico ou alfanumerico) ou CPF do emitente, com ou sem pontuacao.
    pub emitente: &'a str,
    /// 55 (NF-e) ou 65 (NFC-e).
    pub modelo: u32,
    /// Serie, ate 3 digitos.
    pub serie: u64,
    /// Numero da nota, de 1 a 999999999.
    pub numero: u64,
    /// Forma de emissao (`tpEmis`), 1 digito.
    pub tp_emis: u32,
    /// `cNF`, 8 digitos.
    pub codigo: Option<u64>,
}

impl Default for Montar<'_> {
    fn default() -> Self {
        Self {
            cuf: 0,
            aamm: "",
            emitente: "",
            modelo: 55,
            serie: 1,
            numero: 0,
            tp_emis: 1,
            codigo: None,
        }
    }
}

/// Monta a chave de acesso da NF-e ou da NFC-e (44 posicoes) com o digito verificador. Campo que nao cabe, UF ou
/// modelo desconhecido, emitente com DV invalido e `cNF` que a regra B09-30 recusa sao erro: nada e' cortado nem
/// corrigido em silencio.
pub fn montar(p: &Montar) -> Result<String, ErroChave> {
    if sigla_uf(p.cuf).is_none() {
        return Err(ErroChave::Uf);
    }
    if !matches!(p.modelo, 55 | 65) {
        return Err(ErroChave::Modelo);
    }
    if p.numero == 0 {
        return Err(ErroChave::NumeroZero);
    }
    let (emit, tipo) = emitente(p.emitente)?;
    if tipo == '1' && !cpf_valido(&emit[3..]) {
        return Err(ErroChave::CpfEmitente);
    }
    if tipo == '2' && !cnpj_valido(&emit) {
        return Err(ErroChave::CnpjEmitente);
    }
    let codigo = p.codigo.unwrap_or_else(|| gerar_codigo(p.numero));
    let cnf = num(codigo, 8, "cNF")?;
    if !codigo_valido(codigo, p.numero) {
        return Err(ErroChave::CodigoNumerico);
    }
    let base = format!(
        "{}{}{emit}{}{}{}{}{cnf}",
        num(u64::from(p.cuf), 2, "cUF")?,
        aamm(p.aamm)?,
        num(u64::from(p.modelo), 2, "modelo")?,
        num(p.serie, 3, "serie")?,
        num(p.numero, 9, "numero")?,
        num(u64::from(p.tp_emis), 1, "tpEmis")?,
    );
    let d = dv(&base)?;
    Ok(format!("{base}{d}"))
}

/// Emitente da chave de 44 posicoes: (documento, e' CPF). A serie 910 a 969 e' a do emitente CPF; na faixa 890 a 899
/// (emissao no site do Fisco) o emitente pode ser CNPJ ou CPF, e o CPF vem como `000` + 11 digitos (Anexo I do MOC
/// 7.00, campo B07).
fn emitente_dfe(c: &str) -> (&str, bool) {
    let campo = &c[6..20];
    let serie = if digitos(&c[22..25]) {
        n(&c[22..25])
    } else {
        0
    };
    let cpf = match serie {
        910..=969 => true,
        890..=899 => !cnpj_valido(campo) && campo.starts_with("000") && cpf_valido(&campo[3..]),
        _ => false,
    };
    if cpf {
        (&campo[3..], true)
    } else {
        (campo, false)
    }
}

/// Primeiro defeito da chave de 44 posicoes ja limpa, na ordem: formato, DV, UF, mes, modelo e emitente.
fn erro_dfe(c: &str, conferir_emitente: bool) -> Result<(), ErroChave> {
    if !formato_dfe(c) {
        return Err(ErroChave::FormatoDfe);
    }
    if dv(&c[..43])? != n(&c[43..]) {
        return Err(ErroChave::DigitoVerificador);
    }
    if sigla_uf(n(&c[..2])).is_none() {
        return Err(ErroChave::Uf);
    }
    if !(1..=12).contains(&n(&c[4..6])) {
        return Err(ErroChave::Mes);
    }
    if !MODELOS.contains(&&c[20..22]) {
        return Err(ErroChave::Modelo);
    }
    if conferir_emitente {
        match emitente_dfe(c) {
            (doc, true) if !cpf_valido(doc) => return Err(ErroChave::CpfEmitente),
            (doc, false) if !cnpj_valido(doc) => return Err(ErroChave::CnpjEmitente),
            _ => {}
        }
    }
    Ok(())
}

/// Confere a chave da NF-e ou da NFC-e (44 posicoes; pontuacao, espacos e o prefixo `NFe` do `Id` sao ignorados).
/// `conferir_emitente` confere tambem o DV do CPF ou CNPJ que a chave carrega. A chave da NFS-e (50) usa [`conferir`].
pub fn validar(chave: &str, conferir_emitente: bool) -> Result<(), ErroChave> {
    let c = limpar(chave);
    if c.len() != 44 {
        return Err(ErroChave::TamanhoDfe);
    }
    erro_dfe(&c, conferir_emitente)
}

/// O que a chave de 44 posicoes diz de si mesma.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chave {
    /// A chave limpa (44 posicoes).
    pub chave: String,
    /// Primeiro defeito achado; `None` = chave valida (com a conferencia do emitente pedida a [`decompor`]).
    pub erro: Option<ErroChave>,
    /// Codigo IBGE da UF do emitente.
    pub cuf: u32,
    /// Sigla da UF, vazia se o codigo nao e' de UF.
    pub uf: &'static str,
    /// `AAMM` como esta na chave.
    pub aamm: String,
    /// `AAAA-MM`, no seculo XXI.
    pub competencia: String,
    /// CNPJ ou CPF do emitente, sem pontuacao.
    pub emitente: String,
    /// O emitente e' CPF (serie 910 a 969, ou 890 a 899 com `000` + CPF valido).
    pub emitente_cpf: bool,
    /// Modelo (`55` ou `65`, ou o que a chave trouxer).
    pub modelo: String,
    /// Serie.
    pub serie: u32,
    /// Numero da nota.
    pub numero: u64,
    /// Forma de emissao (`tpEmis`).
    pub tp_emis: u32,
    /// `cNF`.
    pub codigo: u64,
    /// Digito verificador da chave.
    pub dv: u32,
}

impl Chave {
    /// A chave passa em [`validar`] com a conferencia do emitente.
    #[must_use]
    pub fn valida(&self) -> bool {
        self.erro.is_none()
    }

    /// Chave em grupos de 4 posicoes.
    #[must_use]
    pub fn formatada(&self) -> String {
        formatar(&self.chave)
    }
}

/// Decompoe a chave de 44 posicoes. Tamanho ou formato impossivel e' erro; DV, UF, mes, modelo ou emitente
/// invalidos voltam em [`Chave::erro`], com os campos lidos (e' o que a tela precisa para dizer o que esta errado).
pub fn decompor(chave: &str, conferir_emitente: bool) -> Result<Chave, ErroChave> {
    let c = limpar(chave);
    if c.len() != 44 {
        return Err(ErroChave::TamanhoDfe);
    }
    if !formato_dfe(&c) {
        return Err(ErroChave::FormatoDfe);
    }
    let (emit, cpf) = emitente_dfe(&c);
    let cuf = n(&c[..2]);
    Ok(Chave {
        erro: erro_dfe(&c, conferir_emitente).err(),
        cuf,
        uf: sigla_uf(cuf).unwrap_or(""),
        aamm: c[2..6].to_string(),
        competencia: format!("20{}-{}", &c[2..4], &c[4..6]),
        emitente: emit.to_string(),
        emitente_cpf: cpf,
        modelo: c[20..22].to_string(),
        serie: n(&c[22..25]),
        numero: c[25..34].parse().unwrap_or(0),
        tp_emis: n(&c[34..35]),
        codigo: c[35..43].parse().unwrap_or(0),
        dv: n(&c[43..]),
        chave: c,
    })
}

/// A chave em grupos de 4 posicoes separados por espaco (como o DANFE a imprime).
#[must_use]
pub fn formatar(chave: &str) -> String {
    limpar(chave)
        .as_bytes()
        .chunks(4)
        .map(|g| String::from_utf8_lossy(g).into_owned())
        .collect::<Vec<_>>()
        .join(" ")
}

// ---------------------------------------------------------------- achar no texto

/// Parametros de URL que carregam a chave (QR Code e consulta publica).
const URL_PARAMS: [&str; 3] = ["p", "chave", "chnfe"];

/// `c` (44 ou 50 posicoes, limpa) e' uma chave valida de NF-e/NFC-e ou de NFS-e.
fn chave_ok(c: &str, conferir_emitente: bool) -> bool {
    match c.len() {
        44 => erro_dfe(c, conferir_emitente).is_ok(),
        50 => conferir(c).is_ok(),
        _ => false,
    }
}

fn registrar(achadas: &mut Vec<(usize, String)>, pos: usize, chave: &str) {
    if !achadas.iter().any(|(_, x)| x == chave) {
        achadas.push((pos, chave.to_string()));
    }
}

/// Chaves em `?p=...`, `&chave=...` e `&chNFe=...`.
fn achar_em_urls(cs: &[char], conferir_emitente: bool, achadas: &mut Vec<(usize, String)>) {
    for (i, _) in cs
        .iter()
        .enumerate()
        .filter(|(_, c)| matches!(c, '?' | '&'))
    {
        for nome in URL_PARAMS {
            let fim = i + 1 + nome.len();
            if fim >= cs.len() || cs[fim] != '=' {
                continue;
            }
            let lido: String = cs[i + 1..fim]
                .iter()
                .collect::<String>()
                .to_ascii_lowercase();
            if lido != nome {
                continue;
            }
            let corrida: String = cs[fim + 1..]
                .iter()
                .take_while(|c| c.is_ascii_alphanumeric())
                .take(50)
                .collect::<String>()
                .to_ascii_uppercase();
            if let Some(t) = [50, 44]
                .into_iter()
                .find(|&t| corrida.len() >= t && chave_ok(&corrida[..t], conferir_emitente))
            {
                registrar(achadas, i, &corrida[..t]);
            }
        }
    }
}

/// Chaves no meio de uma corrida de `[0-9A-Z]` com, no maximo, um separador (espaco, quebra ou ponto) entre as
/// posicoes.
fn achar_em_corridas(cs: &[char], conferir_emitente: bool, achadas: &mut Vec<(usize, String)>) {
    let up: Vec<char> = cs.iter().map(char::to_ascii_uppercase).collect();
    let sep = |c: char| c.is_whitespace() || c == '.';
    let mut i = 0;
    while i < up.len() {
        if !alnum_up(up[i]) {
            i += 1;
            continue;
        }
        let mut corrida = String::new();
        let mut pos = Vec::new();
        let mut j = i;
        loop {
            corrida.push(up[j]);
            pos.push(j);
            if j + 1 < up.len() && alnum_up(up[j + 1]) {
                j += 1;
            } else if j + 2 < up.len() && sep(up[j + 1]) && alnum_up(up[j + 2]) {
                j += 2;
            } else {
                break;
            }
        }
        achar_na_corrida(&corrida, &pos, conferir_emitente, achadas);
        i = j + 1;
    }
}

fn achar_na_corrida(
    corrida: &str,
    pos: &[usize],
    conferir_emitente: bool,
    achadas: &mut Vec<(usize, String)>,
) {
    let b = corrida.as_bytes();
    let mut k = 0;
    while k + 44 <= b.len() {
        let comeca_em_digitos = b[k].is_ascii_digit() && b[k + 1].is_ascii_digit();
        let achou = comeca_em_digitos
            .then(|| {
                [50, 44]
                    .into_iter()
                    .find(|&t| k + t <= b.len() && chave_ok(&corrida[k..k + t], conferir_emitente))
            })
            .flatten();
        match achou {
            Some(t) => {
                registrar(achadas, pos[k], &corrida[k..k + t]);
                k += t;
            }
            None => k += 1,
        }
    }
}

/// Chaves VALIDAS (NF-e, NFC-e e NFS-e) no texto, inclusive em URL de QR Code, na ordem em que aparecem e sem
/// repetir. Serve para texto de OCR: a chave pode vir em grupos de 4 ou quebrada em linhas.
#[must_use]
pub fn achar(texto: &str, conferir_emitente: bool) -> Vec<String> {
    let cs: Vec<char> = texto.chars().collect();
    let mut achadas: Vec<(usize, String)> = Vec::new();
    achar_em_urls(&cs, conferir_emitente, &mut achadas);
    achar_em_corridas(&cs, conferir_emitente, &mut achadas);
    achadas.sort_by_key(|(p, _)| *p);
    achadas.into_iter().map(|(_, c)| c).collect()
}

/// Chave no conteudo lido de um codigo de barras (a chave pura) ou de um QR Code (URL). A mesma regra de [`achar`].
#[must_use]
pub fn achar_no_codigo(conteudo: &str, conferir_emitente: bool) -> Vec<String> {
    let t = conteudo.trim();
    let c = limpar(t);
    if chave_ok(&c, conferir_emitente) {
        return vec![c];
    }
    achar(t, conferir_emitente)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod testes {
    use super::*;

    #[test]
    fn cpf_e_cnpj() {
        assert!(cpf_valido("52998224725"));
        assert!(!cpf_valido("11111111111"));
        assert!(!cpf_valido("52998224724"));
        assert!(cnpj_valido("11222333000181"));
        assert!(!cnpj_valido("11222333000180"));
        assert!(!cnpj_valido("00000000000000"));
        // alfanumerico: exemplo da nota tecnica da Receita Federal
        assert!(cnpj_valido("12ABC34501DE35"));
        assert!(!cnpj_valido("12ABC34501DE36"));
    }

    #[test]
    fn conferir_aponta_o_motivo() {
        let base = concat!(
            "3550308",
            "1",
            "2",
            "11222333000181",
            "0000000000001",
            "2601",
            "123456789"
        );
        assert_eq!(base.len(), 49);
        let d = dv(base).unwrap();
        let chave = format!("{base}{d}");
        assert_eq!(conferir(&chave), Ok(()));
        assert_eq!(conferir(&format!("NFS{chave}")), Ok(()), "prefixo do Id");
        let errada = format!("{base}{}", (d + 1) % 10);
        assert_eq!(conferir(&errada), Err(ErroChave::DigitoVerificador));
        assert_eq!(conferir("123"), Err(ErroChave::Tamanho));
        // tipo de inscricao 3 nao existe
        assert_eq!(
            conferir(&chave.replacen("355030812", "355030813", 1)),
            Err(ErroChave::Formato)
        );
        assert_eq!(
            ErroChave::DigitoVerificador.to_string(),
            "digito verificador nao confere"
        );
    }

    #[test]
    fn montar_nfse_fecha_com_o_dv() {
        let c = montar_nfse(3550308, "11222333000181", 42, "2610", Some(1), 2).unwrap();
        assert_eq!(c.len(), 50);
        assert_eq!(conferir(&c), Ok(()));
        assert_eq!(
            montar_nfse(9900000, "11222333000181", 42, "2610", Some(1), 2),
            Err(ErroChave::UfInexistente)
        );
        assert_eq!(
            montar_nfse(3550308, "11222333000181", 42, "2613", Some(1), 2),
            Err(ErroChave::Mes)
        );
        assert_eq!(
            montar_nfse(3550308, "123", 42, "2610", Some(1), 2),
            Err(ErroChave::Emitente)
        );
        let cpf = montar_nfse(3550308, "529.982.247-25", 7, "2026-10", Some(9), 1).unwrap();
        assert_eq!(&cpf[7..23], "1100052998224725");
        assert_eq!(conferir(&cpf), Ok(()));
    }
}
