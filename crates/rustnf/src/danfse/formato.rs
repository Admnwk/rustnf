// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Formatos do DANFSe: decimal em ponto fixo (nunca ponto flutuante), moeda, percentual, documento, data.

use super::xml::{g, No};

/// Decimal com 6 casas (x1_000_000) -- o XSD da NFS-e tem no maximo 4; `None` = ausente ou ilegivel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Dec(pub(super) i128);

pub(super) const ESC: i128 = 1_000_000;

pub(super) fn dec_de(s: &str) -> Option<Dec> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let (neg, s) = match s.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, s.strip_prefix('+').unwrap_or(s)),
    };
    let (i, f) = s.split_once('.').unwrap_or((s, ""));
    if i.is_empty() && f.is_empty()
        || !i.chars().all(|c| c.is_ascii_digit())
        || !f.chars().all(|c| c.is_ascii_digit())
    {
        return None;
    }
    let mut v: i128 = if i.is_empty() { 0 } else { i.parse().ok()? };
    let mut casas = 0;
    for c in f.chars() {
        if casas == 6 {
            if c >= '5' {
                v += 1;
            }
            break;
        }
        v = v * 10 + i128::from(c as u8 - b'0');
        casas += 1;
    }
    for _ in casas..6 {
        v *= 10;
    }
    Some(Dec(if neg { -v } else { v }))
}

pub(super) fn d(r: Option<No>, c: &str) -> Option<Dec> {
    dec_de(&g(r, c))
}

pub(super) fn z(v: Option<Dec>) -> Dec {
    v.unwrap_or(Dec(0))
}

// ------------------------------------------------------------------------------------------------ formatacao
pub(super) fn traco(s: &str) -> String {
    let s = s.trim();
    if s.is_empty() {
        "-".into()
    } else {
        s.into()
    }
}

/// |v| com 2 casas meio-para-cima, milhar com ponto e decimal com virgula.
pub(super) fn agrupado(v: Dec) -> String {
    let a = v.0.abs();
    let c = (a + ESC / 200) / (ESC / 100);
    let inteiro = (c / 100).to_string();
    let mut out = String::new();
    for (i, ch) in inteiro.chars().enumerate() {
        if i > 0 && (inteiro.len() - i).is_multiple_of(3) {
            out.push('.');
        }
        out.push(ch);
    }
    format!("{out},{:02}", c % 100)
}

pub(super) fn moeda(v: Option<Dec>) -> String {
    let v = z(v);
    let a = agrupado(v);
    let sinal = if v.0 < 0 && a != "0,00" { "-" } else { "" };
    format!("R$ {sinal}{a}")
}

pub(super) fn moeda_ou_traco(v: Option<Dec>) -> String {
    match v {
        None | Some(Dec(0)) => "-".into(),
        x => moeda(x),
    }
}

pub(super) fn percentual(v: Option<Dec>, simbolo: bool) -> String {
    let v = z(v);
    let c = (v.0.abs() + ESC / 200) / (ESC / 100);
    let s = if v.0 < 0 && c != 0 { "-" } else { "" };
    format!(
        "{s}{},{:02}{}",
        c / 100,
        c % 100,
        if simbolo { "%" } else { "" }
    )
}

pub(super) fn percentual_ou_traco(v: Option<Dec>, simbolo: bool) -> String {
    match v {
        None | Some(Dec(0)) => "-".into(),
        x => percentual(x, simbolo),
    }
}

pub(super) fn documento(s: &str) -> String {
    let s = s.trim();
    let c: Vec<char> = s.chars().collect();
    let p = |a: usize, b: usize| c[a..b].iter().collect::<String>();
    match c.len() {
        11 => format!("{}.{}.{}-{}", p(0, 3), p(3, 6), p(6, 9), p(9, 11)),
        14 => format!(
            "{}.{}.{}/{}-{}",
            p(0, 2),
            p(2, 5),
            p(5, 8),
            p(8, 12),
            p(12, 14)
        ),
        _ => s.into(),
    }
}

pub(super) fn telefone(s: &str) -> String {
    let d: String = s.chars().filter(char::is_ascii_digit).collect();
    match d.len() {
        10 => format!("({}) {}-{}", &d[0..2], &d[2..6], &d[6..10]),
        11 => format!("({}) {}-{}", &d[0..2], &d[2..7], &d[7..11]),
        _ => d,
    }
}

pub(super) fn eh_data(s: &str) -> bool {
    let b = s.as_bytes();
    s.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && [0, 1, 2, 3, 5, 6, 8, 9]
            .iter()
            .all(|&i| b[i].is_ascii_digit())
}

pub(super) fn data(s: &str) -> String {
    let s = s.trim();
    if s.is_empty() {
        return "-".into();
    }
    if eh_data(s) {
        return format!("{}/{}/{}", &s[8..10], &s[5..7], &s[0..4]);
    }
    s.into()
}

pub(super) fn data_hora(s: &str) -> String {
    let s = s.trim();
    if s.is_empty() {
        return "-".into();
    }
    if s.len() == 10 {
        return data(s);
    }
    if s.len() < 19 || !s.is_char_boundary(19) || s.as_bytes()[10] != b'T' {
        return s.into();
    }
    let dt = data(&s[0..10]);
    if dt == s[0..10] {
        return s.into();
    }
    let h = &s.as_bytes()[11..19];
    if h[2] == b':' && h[5] == b':' && [0, 1, 3, 4, 6, 7].iter().all(|&i| h[i].is_ascii_digit()) {
        return format!("{dt} {}", &s[11..19]);
    }
    s.into()
}

pub(super) fn ctribnac(s: &str) -> String {
    let s: String = s.replace('.', "").trim().into();
    if s.len() == 6 && s.is_ascii() {
        format!("{}.{}.{}", &s[0..2], &s[2..4], &s[4..6])
    } else {
        s
    }
}

pub(super) fn nbs(s: &str) -> String {
    let d: String = s.chars().filter(char::is_ascii_digit).collect();
    if d.len() == 9 {
        format!("{}.{}.{}.{}", &d[0..1], &d[1..5], &d[5..7], &d[7..9])
    } else {
        s.into()
    }
}

pub(super) fn municipio_uf(m: &str, uf: &str) -> String {
    let (m, uf) = (m.trim(), uf.trim());
    match (m.is_empty(), uf.is_empty()) {
        (false, false) => format!("{m} / {uf}"),
        (false, true) => m.into(),
        (true, false) => uf.into(),
        _ => "-".into(),
    }
}

pub(super) fn ibge_cep(ibge: &str, cep: &str) -> String {
    let (ibge, mut cep) = (ibge.trim(), cep.trim().to_string());
    if cep.chars().count() == 8 && cep.is_ascii() {
        cep = format!("{}.{}-{}", &cep[0..2], &cep[2..5], &cep[5..8]);
    }
    match (ibge.is_empty(), cep.is_empty()) {
        (false, false) => format!("{ibge} / {cep}"),
        (false, true) => ibge.into(),
        (true, false) => cep,
        _ => "-".into(),
    }
}

pub(super) fn endereco(partes: &[String]) -> String {
    let v: Vec<&str> = partes
        .iter()
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .collect();
    if v.is_empty() {
        "-".into()
    } else {
        v.join(", ")
    }
}

pub(super) fn primeiro(v: &[String]) -> String {
    v.iter()
        .find(|x| !x.trim().is_empty())
        .cloned()
        .unwrap_or_default()
}

pub(super) fn com_barra(v: &[String]) -> String {
    let p: Vec<String> = v.iter().map(|x| traco(x)).collect();
    if p.iter().any(|x| x != "-") {
        p.join(" / ")
    } else {
        "-".into()
    }
}

pub(super) fn valores_com_barra(v: &[String]) -> String {
    let p: Vec<String> = v
        .iter()
        .map(|x| {
            if x.trim().is_empty() {
                "-".into()
            } else {
                x.clone()
            }
        })
        .collect();
    if p.iter().any(|x| x.trim() != "-") {
        p.join(" / ")
    } else {
        "-".into()
    }
}

pub(super) fn pais(s: &str) -> String {
    let s = s.trim();
    if s.is_empty() {
        "BR".into()
    } else {
        s.to_uppercase()
    }
}

pub(super) fn rstrip_esp_linha(s: &str) -> &str {
    s.trim_end_matches([' ', '\n'])
}

pub(super) fn truncar(t: &str, n: usize) -> String {
    let c: Vec<char> = t.chars().collect();
    if c.len() <= n {
        return t.into();
    }
    let p: String = c[..n].iter().collect();
    format!("{}...", rstrip_esp_linha(&p))
}
