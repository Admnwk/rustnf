// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Pecas comuns: erro com o campo, formatos do leiaute (padroes do `tiposSimples_v1.01.xsd` do pacote de esquemas 20260727),
//! texto no conjunto que o XSD aceita e o escritor de XML ja na forma canonica. O contrato e'
//! `tests/casos/nfse_nacional.json`.

use std::fmt;

use chrono::{DateTime, FixedOffset, NaiveDate, NaiveDateTime, TimeZone, Utc};
use serde::de::{self, Deserializer, Visitor};
use serde::{Deserialize, Serialize};

pub const NS: &str = "http://www.sped.fazenda.gov.br/nfse";
pub const VERSAO: &str = "1.01";

/// Dado que o leiaute recusa. `campo` = caminho no XML; `mensagem` = frase de tela (nunca dado tecnico).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErroNfse {
    pub campo: String,
    pub mensagem: String,
}

impl ErroNfse {
    pub fn novo(campo: impl Into<String>, mensagem: impl Into<String>) -> Self {
        ErroNfse {
            campo: campo.into(),
            mensagem: mensagem.into(),
        }
    }
}

impl fmt::Display for ErroNfse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.campo.is_empty() {
            write!(f, "{}", self.mensagem)
        } else {
            write!(f, "{}: {}", self.campo, self.mensagem)
        }
    }
}

impl std::error::Error for ErroNfse {}

pub type R<T> = Result<T, ErroNfse>;

/// Regra do Anexo I conferida sem rede: recusa com o codigo.
pub fn regra(cond: bool, codigo: &str, campo: &str, msg: &str) -> R<()> {
    if cond {
        Err(ErroNfse::novo(
            campo,
            format!("{msg} (regra {codigo} do SEFIN)"),
        ))
    } else {
        Ok(())
    }
}

/// Valor que no JSON vem como numero ou texto (dinheiro, percentual, serie, numero, codigo). Guarda o texto.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Valor(pub String);

impl Valor {
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn vazio(&self) -> bool {
        self.0.trim().is_empty()
    }
}

impl fmt::Display for Valor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

macro_rules! valor_de {
    ($($t:ty),*) => {$(impl From<$t> for Valor { fn from(v: $t) -> Self { Valor(v.to_string()) } })*};
}
valor_de!(i32, i64, u8, u16, u32, u64, usize, f64, &str, String);

impl<'de> Deserialize<'de> for Valor {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl Visitor<'_> for V {
            type Value = Valor;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("numero ou texto")
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Valor, E> {
                Ok(Valor(v.to_string()))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Valor, E> {
                Ok(Valor(v.to_string()))
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Valor, E> {
                Ok(Valor(v.to_string()))
            }
            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Valor, E> {
                Ok(Valor(v.to_string()))
            }
        }
        d.deserialize_any(V)
    }
}

// -- padroes do XSD (sem crate de regex: verificadores pequenos) -------------------------------------------------
fn todos(s: &str, f: impl Fn(char) -> bool) -> bool {
    s.chars().all(f)
}
fn dig(c: char) -> bool {
    c.is_ascii_digit()
}
fn alnum_up(c: char) -> bool {
    c.is_ascii_digit() || c.is_ascii_uppercase()
}

/// Os tipos simples usados aqui. Devolve true se `v` casa o padrao inteiro.
pub fn casa(tipo: &str, v: &str) -> bool {
    let n = v.chars().count();
    match tipo {
        "TSCNPJ" => n == 14 && todos(v, alnum_up),
        "TSCPF" => n == 11 && todos(v, dig),
        "TSCodMunIBGE" => n == 7 && todos(v, dig),
        "TSCEP" => n == 8 && todos(v, dig),
        "TSSerieDPS" => todos(v, dig) && ((1..=4).contains(&n) || (n == 5 && !v.starts_with('9'))),
        "TSNumDPS" => (1..=15).contains(&n) && todos(v, dig) && !v.starts_with('0'),
        "TSCodTribNac" | "TSCodIndOp" | "TSCClassTrib" => n == 6 && todos(v, dig),
        "TSCodTribMun" | "TSCodMoeda" | "TSCST" => n == 3 && todos(v, dig),
        "TSCodNBS" => n == 9 && todos(v, dig),
        "TSCodPaisISO" => n == 2 && todos(v, |c| c.is_ascii_uppercase()),
        "TSTelefone" => (6..=20).contains(&n) && todos(v, dig),
        "TSChaveNFSe" => n == 50 && chave_xsd(v, 30),
        "TSChaveNFe" => n == 44 && chave_xsd(v, 24),
        "TSCAEPF" | "TSNumBM" => n == 14 && todos(v, dig),
        "TSNumProcesso" => n == 30 && todos(v, dig),
        "TSCIntContrib" => (1..=20).contains(&n) && todos(v, |c| c.is_ascii_alphanumeric()),
        "TSCCredPres" => n == 2 && todos(v, dig),
        // so' no XSD da Producao Restrita 20/10/2026 (leiaute nt009, bensMoveis)
        "TSNCM" => n == 8 && todos(v, dig),
        "TSNum3Dig" => (1..=3).contains(&n) && todos(v, dig),
        // so' no XSD da Producao Restrita 24/11/2026 (leiaute nt009, comExt do Release 2)
        "TSNumFatura" | "TSNumContCambio" => (1..=20).contains(&n) && todos(v, dig),
        _ => false,
    }
}

fn chave_xsd(v: &str, cauda: usize) -> bool {
    let c: Vec<char> = v.chars().collect();
    c.len() == 20 + cauda
        && c[..6].iter().all(|&x| dig(x))
        && c[6..20].iter().all(|&x| alnum_up(x))
        && c[20..].iter().all(|&x| dig(x))
}

/// TSIdDPS (`tiposSimples:54`): DPS[0-9]{7}(1[0-9]{14}|2[0-9A-Z]{14})[0-9]{20}.
pub fn casa_id_dps(v: &str) -> bool {
    let c: Vec<char> = v.chars().collect();
    c.len() == 45
        && v.starts_with("DPS")
        && c[3..10].iter().all(|&x| dig(x))
        && casa_insc(&c[10..25])
        && c[25..].iter().all(|&x| dig(x))
}

fn casa_insc(c: &[char]) -> bool {
    match c[0] {
        '1' => c[1..].iter().all(|&x| dig(x)),
        '2' => c[1..].iter().all(|&x| alnum_up(x)),
        _ => false,
    }
}

/// Chave que o pedRegEvento aceita: TSChaveNFSe x TSIdPedRegEvt = [0-9]{7}[0-9](1[0-9]{14}|2[0-9A-Z]{14})[0-9]{27}.
pub fn casa_chave_evento(v: &str) -> bool {
    let c: Vec<char> = v.chars().collect();
    c.len() == 50
        && c[..8].iter().all(|&x| dig(x))
        && casa_insc(&c[8..23])
        && c[23..].iter().all(|&x| dig(x))
}

pub fn confere(tipo: &str, valor: &str, campo: &str, explicacao: &str) -> R<String> {
    if casa(tipo, valor) {
        Ok(valor.to_string())
    } else {
        Err(ErroNfse::novo(
            campo,
            format!("valor {valor:?} fora do formato {tipo}. {explicacao}")
                .trim()
                .to_string(),
        ))
    }
}

pub fn so_alnum(v: &str) -> String {
    v.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

pub fn so_digitos(v: &str) -> String {
    v.chars().filter(char::is_ascii_digit).collect()
}

// -- texto --------------------------------------------------------------------------------------------------------
fn tipografia(c: char) -> Option<&'static str> {
    Some(match c as u32 {
        0x2010..=0x2015 | 0x2022 => "-",
        0x2018..=0x201a => "'",
        0x201c..=0x201e => "\"",
        0x2026 => "...",
        0x20ac => "EUR",
        0x2122 => "TM",
        0x2264 => "<=",
        0x2265 => ">=",
        _ => return None,
    })
}

/// TSString (`tiposSimples_v1.01.xsd:19`) so' aceita `[ -ÿ]`: tipografia comum vira ASCII, o resto se decompoe
/// (NFKD) e o que nao cabe sai. Contrato: secao `latin1` do golden.
pub fn latin1(s: &str) -> String {
    use unicode_normalization::char::canonical_combining_class;
    use unicode_normalization::UnicodeNormalization;
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if (c as u32) <= 0xFF {
            out.push(c);
        } else if let Some(t) = tipografia(c) {
            out.push_str(t);
        } else {
            let mut buf = [0u8; 4];
            for x in c.encode_utf8(&mut buf).nfkd() {
                if (x as u32) <= 0xFF && canonical_combining_class(x) == 0 {
                    out.push(x);
                }
            }
        }
    }
    out
}

/// Espaco no sentido do Unicode `White_Space` (o que o contrato espera).
fn espaco(c: char) -> bool {
    c.is_whitespace() || matches!(c as u32, 0x1c..=0x1f)
}

fn colapsa(s: &str) -> String {
    s.split(espaco)
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Opcoes do `texto`.
#[derive(Debug, Clone, Copy)]
pub struct Txt {
    pub minimo: usize,
    pub maximo: usize,
    pub quebra: bool,
    pub obrigatorio: bool,
    pub so_latin1: bool,
}

impl Txt {
    pub const fn max(maximo: usize) -> Self {
        Txt {
            minimo: 1,
            maximo,
            quebra: false,
            obrigatorio: true,
            so_latin1: true,
        }
    }
    pub const fn opc(maximo: usize) -> Self {
        Txt {
            minimo: 1,
            maximo,
            quebra: false,
            obrigatorio: false,
            so_latin1: true,
        }
    }
}

/// Texto do leiaute: sem controle, sem espaco nas pontas, CRLF -> LF (so' onde o tipo aceita quebra), cortado no
/// maximo, Latin-1 onde o tipo e' TSString. Vazio e opcional = "".
pub fn texto(v: &str, campo: &str, o: Txt) -> R<String> {
    let mut s = v.replace("\r\n", "\n").replace('\r', "\n");
    s = s
        .chars()
        .map(|c| {
            if matches!(c as u32, 0..=8 | 0x0b | 0x0c | 0x0e..=0x1f | 0x7f) {
                ' '
            } else {
                c
            }
        })
        .collect();
    if o.so_latin1 {
        s = latin1(&s);
    }
    let s = if o.quebra {
        s.split('\n')
            .map(colapsa)
            .collect::<Vec<_>>()
            .join("\n")
            .trim_matches(espaco)
            .to_string()
    } else {
        colapsa(&s)
    };
    if s.is_empty() {
        return if o.obrigatorio {
            Err(ErroNfse::novo(campo, "obrigatorio"))
        } else {
            Ok(String::new())
        };
    }
    let n = s.chars().count();
    if n < o.minimo {
        return Err(ErroNfse::novo(
            campo,
            format!("precisa de pelo menos {} caracteres (tem {n})", o.minimo),
        ));
    }
    let cortado: String = s.chars().take(o.maximo).collect();
    Ok(cortado.trim_end_matches(espaco).to_string())
}

// -- numeros e datas ----------------------------------------------------------------------------------------------
/// TSDec15V2 e afins: arredonda meio-para-cima em 2 casas, sem ponto flutuante. `inteiros` = digitos antes do ponto.
pub fn dinheiro(v: &str, campo: &str, inteiros: usize, permite_zero: bool) -> R<String> {
    let invalido = || ErroNfse::novo(campo, format!("valor invalido ({v:?})"));
    let t = v.trim();
    let (neg, t) = match t.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, t.strip_prefix('+').unwrap_or(t)),
    };
    let (int, frac) = match t.split_once('.') {
        Some((a, b)) => (a, b),
        None => (t, ""),
    };
    if (int.is_empty() && frac.is_empty()) || !todos(int, dig) || !todos(frac, dig) {
        return Err(invalido());
    }
    let mut cent: u128 = 0;
    for c in int
        .chars()
        .chain(frac.chars().chain(std::iter::repeat('0')).take(2))
    {
        cent = cent
            .checked_mul(10)
            .and_then(|x| x.checked_add(c as u128 - '0' as u128))
            .ok_or_else(invalido)?;
    }
    if frac.chars().nth(2).is_some_and(|c| c >= '5') {
        cent += 1;
    }
    if neg && cent > 0 {
        return Err(ErroNfse::novo(campo, "valor negativo"));
    }
    if cent == 0 && !permite_zero {
        return Err(ErroNfse::novo(campo, "precisa ser maior que zero"));
    }
    let s = format!("{}.{:02}", cent / 100, cent % 100);
    if (cent / 100).to_string().len() > inteiros && cent / 100 > 0 {
        return Err(ErroNfse::novo(
            campo,
            format!("passa de {inteiros} digitos inteiros"),
        ));
    }
    Ok(s)
}

/// TSDec15V2Neg (XSD da Producao Restrita 24/11/2026): como o `dinheiro`, mas aceita o sinal de menos. Zero sai sem
/// sinal. Devolve o texto e os centavos com sinal.
pub fn dinheiro_com_sinal(v: &str, campo: &str, inteiros: usize) -> R<(String, i128)> {
    let t = v.trim();
    let (neg, resto) = match t.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, t),
    };
    let s = dinheiro(resto, campo, inteiros, true)?;
    let c = centavos(&s)? as i128;
    if neg && c > 0 {
        Ok((format!("-{s}"), -c))
    } else {
        Ok((s, c))
    }
}

/// Valor decimal em centavos (para comparar). Erro com campo vazio.
pub fn centavos(v: &str) -> R<u128> {
    let s = dinheiro(v, "", 30, true)?;
    let (a, b) = s.split_once('.').unwrap_or((&s, "0"));
    Ok(a.parse::<u128>().unwrap_or(0) * 100 + b.parse::<u128>().unwrap_or(0))
}

/// TSData AAAA-MM-DD (aceita AAAA-MM -> dia 1, e texto mais longo cortado em 10). Ano 2000-2099.
pub fn data(v: &str, campo: &str) -> R<String> {
    let s = v.trim();
    let base = if s.len() >= 10 {
        s.get(..10).unwrap_or(s).to_string()
    } else if s.len() == 7 {
        format!("{s}-01")
    } else {
        s.to_string()
    };
    let d = NaiveDate::parse_from_str(&base, "%Y-%m-%d")
        .map_err(|_| ErroNfse::novo(campo, format!("data invalida ({v:?}); use AAAA-MM-DD")))?;
    if !(2000..=2099).contains(&chrono::Datelike::year(&d)) {
        return Err(ErroNfse::novo(campo, "ano fora de 2000-2099 (TSData)"));
    }
    Ok(d.format("%Y-%m-%d").to_string())
}

/// Brasilia (-03:00).
pub fn brasilia() -> FixedOffset {
    FixedOffset::west_opt(3 * 3600).expect("fuso fixo")
}

/// TSDateTimeUTC (`:146`): AAAA-MM-DDThh:mm:ss+hh:00 -- offset obrigatorio, hora cheia de -11 a +12, SEM fracao.
/// `v` em ISO 8601; sem fuso = Brasilia; `None` = agora em Brasilia.
pub fn data_hora(v: Option<&str>, campo: &str) -> R<String> {
    let dt: DateTime<FixedOffset> = match v.map(str::trim).filter(|s| !s.is_empty()) {
        None => Utc::now().with_timezone(&brasilia()),
        Some(s) => match DateTime::parse_from_rfc3339(s) {
            Ok(d) => d,
            Err(_) => {
                let n = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f")
                    .or_else(|_| NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S%.f"))
                    .map_err(|_| ErroNfse::novo(campo, format!("data e hora invalidas ({s:?})")))?;
                brasilia()
                    .from_local_datetime(&n)
                    .single()
                    .ok_or_else(|| ErroNfse::novo(campo, "hora inexistente"))?
            }
        },
    };
    de_datahora(&dt, campo)
}

/// `dhEmi` para a emissao de agora: o relogio local menos `folga_s` segundos, em Brasilia, no formato do leiaute.
///
/// A Sefin recusa a DPS cuja data de emissao seja POSTERIOR ao processamento (Anexo I v1.01, E0008); um relogio
/// local adiantado de poucos segundos basta para isso. A biblioteca recebe o `dhEmi` explicito (`Dps.emissao`); quem
/// emite "agora" (CLI, HTTP) usa esta funcao com a folga configurada (padrao 60 s).
pub fn agora_com_folga(folga_s: u32) -> String {
    let dt =
        (Utc::now() - chrono::Duration::seconds(i64::from(folga_s))).with_timezone(&brasilia());
    // o fuso de Brasilia e' fixo (-03:00): `de_datahora` nao recusa
    de_datahora(&dt, "infDPS/dhEmi").unwrap_or_default()
}

pub fn de_datahora(dt: &DateTime<FixedOffset>, campo: &str) -> R<String> {
    let seg = dt.offset().local_minus_utc();
    if seg % 3600 != 0 || !(-11 * 3600..=12 * 3600).contains(&seg) {
        return Err(ErroNfse::novo(
            campo,
            "fuso fora do leiaute (hora cheia entre -11:00 e +12:00)",
        ));
    }
    let sinal = if seg < 0 { '-' } else { '+' };
    Ok(format!(
        "{}{}{:02}:00",
        dt.format("%Y-%m-%dT%H:%M:%S"),
        sinal,
        seg.abs() / 3600
    ))
}

/// ("CNPJ"|"CPF", valor). CNPJ alfanumerico preservado; CPF so' digitos.
pub fn documento(v: &str) -> R<(&'static str, String)> {
    let d = so_alnum(v);
    if d.len() == 11 && todos(&d, dig) {
        return Ok(("CPF", d));
    }
    if d.len() == 14 && todos(&d[..12], alnum_up) && todos(&d[12..], dig) {
        return Ok(("CNPJ", d));
    }
    Err(ErroNfse::novo(
        "",
        format!("documento {v:?} nao e' CPF (11 digitos) nem CNPJ (14, os 2 ultimos digitos)"),
    ))
}

// -- XML ----------------------------------------------------------------------------------------------------------
/// Escape de TEXTO na forma que a C14N produz (so' & < >).
pub fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub fn esc_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('"', "&quot;")
        .replace('\t', "&#x9;")
        .replace('\n', "&#xA;")
}

/// Elemento minimo: sem prefixo, sem tag vazia, sem espaco entre elementos (a forma canonica).
#[derive(Debug, Clone, Default)]
pub struct No {
    pub tag: String,
    pub attrs: Vec<(String, String)>,
    pub filhos: Vec<No>,
    pub texto: Option<String>,
}

impl No {
    pub fn new(tag: &str) -> Self {
        No {
            tag: tag.into(),
            ..Default::default()
        }
    }
    pub fn attr(mut self, k: &str, v: &str) -> Self {
        self.attrs.push((k.into(), v.into()));
        self
    }
    /// Filho com texto.
    pub fn add(&mut self, tag: &str, texto: impl Into<String>) {
        self.filhos.push(No {
            tag: tag.into(),
            texto: Some(texto.into()),
            ..Default::default()
        });
    }
    /// Filho so' se houver texto.
    pub fn opc(&mut self, tag: &str, texto: &str) {
        if !texto.is_empty() {
            self.add(tag, texto);
        }
    }
    /// Filho grupo (devolve para preencher).
    pub fn grupo(&mut self, tag: &str) -> &mut No {
        self.filhos.push(No::new(tag));
        self.filhos.last_mut().expect("acabou de entrar")
    }
    pub fn anexar(&mut self, no: No) {
        self.filhos.push(no);
    }
    pub fn xml(&self, raiz_ns: Option<&str>) -> String {
        let mut s = String::new();
        self.escrever(&mut s, raiz_ns);
        s
    }
    fn escrever(&self, s: &mut String, raiz_ns: Option<&str>) {
        s.push('<');
        s.push_str(&self.tag);
        if let Some(ns) = raiz_ns {
            s.push_str(&format!(" xmlns=\"{ns}\""));
        }
        for (k, v) in &self.attrs {
            s.push_str(&format!(" {k}=\"{}\"", esc_attr(v)));
        }
        s.push('>');
        match &self.texto {
            Some(t) => s.push_str(&esc(t)),
            None => self.filhos.iter().for_each(|f| f.escrever(s, None)),
        }
        s.push_str("</");
        s.push_str(&self.tag);
        s.push('>');
    }
}
