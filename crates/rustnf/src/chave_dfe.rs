// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Chave de acesso da NFS-e Nacional (50 posicoes): conferencia de formato e digito verificador, montagem, e a
//! validacao de CPF e CNPJ (numerico e alfanumerico).
//!
//! O DV e' modulo 11 com pesos 2..9 da direita e `Ord(ch) - 48`, de modo que a letra do CNPJ alfanumerico pesa
//! como valor ASCII menos 48 (Nota Tecnica conjunta do CNPJ alfanumerico, Receita Federal:
//! <https://www.gov.br/receitafederal/pt-br/acesso-a-informacao/acoes-e-programas/programas-e-atividades/cnpj-alfanumerico>).
//! A estrutura da chave vem do XSD publicado no portal da NFS-e (tipo `TSChaveNFSe`): municipio (7), ambiente
//! gerador (1), tipo de inscricao (1), inscricao (14), numero (13), AAMM (4), codigo (9) e DV (1).

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
            Self::NaoCabe { nome, tamanho } => {
                write!(f, "{nome} precisa de ate {tamanho} digitos")
            }
        }
    }
}

impl std::error::Error for ErroChave {}

/// Codigos IBGE de UF (`TCodUfIBGE` do XSD).
const UFS: [u32; 27] = [
    11, 12, 13, 14, 15, 16, 17, 21, 22, 23, 24, 25, 26, 27, 28, 29, 31, 32, 33, 35, 41, 42, 43, 50,
    51, 52, 53,
];

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

/// A chave "crua": sem pontuacao e a partir do primeiro par de digitos (o `Id` da NFS-e traz letras na frente).
fn limpar(valor: &str) -> String {
    let v = so_alnum(valor);
    let b = v.as_bytes();
    (0..b.len().saturating_sub(1))
        .find(|&i| b[i].is_ascii_digit() && b[i + 1].is_ascii_digit())
        .map_or_else(String::new, |i| v[i..].to_string())
}

/// DV das 49 primeiras posicoes da chave: modulo 11, pesos 2..9 da direita, `Ord(ch) - 48`.
fn dv(base: &str) -> Result<u32, ErroChave> {
    if base.len() != 49 {
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
    if !UFS.contains(&n(&cm[..2])) {
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
    if !UFS.contains(&n(&c[..2])) {
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
