// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Erro da montagem, decimal exato, texto do XML (filtro e tamanhos do leiaute) e data e hora no fuso da UF.

use std::fmt;

use chrono::{DateTime, Datelike, FixedOffset, NaiveDate, NaiveDateTime, Offset, Utc};

// ------------------------------------------------------------------------------------------------------- erro

/// Por que o pedido nao virou XML: o campo do leiaute, a mensagem e, quando a regra tem codigo no MOC ou na NT, a
/// rejeicao que a SEFAZ devolveria (`regra`, vazio quando o defeito e' so' de formato).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErroMontagem {
    /// Texto para quem emite.
    pub mensagem: String,
    /// Campo ou grupo do leiaute (`natOp`, `enderEmit/UF`, `ICMS`...).
    pub campo: String,
    /// `cStat` da rejeicao correspondente, ou vazio.
    pub regra: String,
}

impl fmt::Display for ErroMontagem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.mensagem)
    }
}

impl std::error::Error for ErroMontagem {}

pub(crate) fn erro(msg: impl Into<String>, campo: &str) -> ErroMontagem {
    ErroMontagem {
        mensagem: msg.into(),
        campo: campo.into(),
        regra: String::new(),
    }
}

pub(crate) fn erro_r(msg: impl Into<String>, campo: &str, regra: &str) -> ErroMontagem {
    ErroMontagem {
        mensagem: msg.into(),
        campo: campo.into(),
        regra: regra.into(),
    }
}

/// Resultado da montagem.
pub type Res<T> = Result<T, ErroMontagem>;

// --------------------------------------------------------------------------------------------------- decimal

/// Decimal exato: mantissa x 10^-e.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Dec {
    pub(crate) m: i128,
    pub(crate) e: u32,
}

impl Dec {
    pub(crate) const ZERO: Dec = Dec { m: 0, e: 0 };

    /// `10.50`, `10,50`, `-1`, `7`. Sem expoente e sem espaco no meio; vazio vale zero.
    pub(crate) fn parse(s: &str, campo: &str) -> Res<Dec> {
        let t = s.trim().replace(',', ".");
        if t.is_empty() {
            return Ok(Dec::ZERO);
        }
        let (neg, corpo) = match t.strip_prefix('-') {
            Some(r) => (true, r),
            None => (false, t.as_str()),
        };
        let (int, frac) = match corpo.split_once('.') {
            Some((a, b)) => (a, b),
            None => (corpo, ""),
        };
        let ok = !int.is_empty()
            && int.bytes().all(|b| b.is_ascii_digit())
            && (!corpo.contains('.')
                || (!frac.is_empty() && frac.bytes().all(|b| b.is_ascii_digit())));
        if !ok || int.len() + frac.len() > 30 {
            return Err(erro(
                format!("{campo}: '{}' nao e numero", corta(s, 20)),
                campo,
            ));
        }
        let m: i128 = format!("{int}{frac}")
            .parse()
            .map_err(|_| erro(format!("{campo}: valor invalido"), campo))?;
        Ok(Dec {
            m: if neg { -m } else { m },
            e: frac.len() as u32,
        })
    }

    pub(crate) fn de_int(v: i64) -> Dec {
        Dec {
            m: i128::from(v),
            e: 0,
        }
    }

    pub(crate) fn centavos(v: i64) -> Dec {
        Dec {
            m: i128::from(v),
            e: 2,
        }
    }

    fn alinha(a: Dec, b: Dec) -> (i128, i128, u32) {
        let e = a.e.max(b.e);
        (a.m * 10i128.pow(e - a.e), b.m * 10i128.pow(e - b.e), e)
    }

    pub(crate) fn mais(self, o: Dec) -> Dec {
        let (a, b, e) = Dec::alinha(self, o);
        Dec { m: a + b, e }
    }

    pub(crate) fn menos(self, o: Dec) -> Dec {
        let (a, b, e) = Dec::alinha(self, o);
        Dec { m: a - b, e }
    }

    pub(crate) fn vezes(self, o: Dec) -> Dec {
        Dec {
            m: self.m * o.m,
            e: self.e + o.e,
        }
    }

    /// Arredonda para `casas` pelo criterio comercial (meio para cima, em modulo).
    pub(crate) fn arred(self, casas: u32) -> Dec {
        if self.e <= casas {
            return Dec {
                m: self.m * 10i128.pow(casas - self.e),
                e: casas,
            };
        }
        let d = 10i128.pow(self.e - casas);
        let q = self.m.abs() / d;
        let r = self.m.abs() % d;
        let q = if r * 2 >= d { q + 1 } else { q };
        Dec {
            m: if self.m < 0 { -q } else { q },
            e: casas,
        }
    }

    /// Quantas casas decimais o valor realmente usa (sem os zeros da direita).
    pub(crate) fn casas(self) -> u32 {
        let (mut m, mut e) = (self.m, self.e);
        while e > 0 && m % 10 == 0 {
            m /= 10;
            e -= 1;
        }
        e
    }

    pub(crate) fn eh_zero(self) -> bool {
        self.m == 0
    }

    pub(crate) fn negativo(self) -> bool {
        self.m < 0
    }

    pub(crate) fn cmp_zero(self) -> std::cmp::Ordering {
        self.m.cmp(&0)
    }

    pub(crate) fn abs(self) -> Dec {
        Dec {
            m: self.m.abs(),
            e: self.e,
        }
    }

    pub(crate) fn maior(self, o: Dec) -> bool {
        let (a, b, _) = Dec::alinha(self, o);
        a > b
    }

    /// Texto com `casas` casas fixas (arredonda; nao confere se o leiaute comporta, para isso veja [`num`]).
    pub(crate) fn fixo(self, casas: u32) -> String {
        let v = self.arred(casas);
        let neg = v.m < 0;
        let d = v.m.unsigned_abs().to_string();
        let s = if casas == 0 {
            d
        } else {
            let d = format!("{:0>w$}", d, w = casas as usize + 1);
            let (i, f) = d.split_at(d.len() - casas as usize);
            format!("{i}.{f}")
        };
        if neg {
            format!("-{s}")
        } else {
            s
        }
    }
}

pub(crate) fn corta(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// Numero no XML com `casas` casas fixas. Mais casas do que o leiaute comporta e valor negativo sao erro.
pub(crate) fn num(v: Dec, casas: u32, campo: &str) -> Res<String> {
    if v.negativo() {
        return Err(erro(
            format!("{campo}: valor negativo ({})", v.fixo(v.e)),
            campo,
        ));
    }
    if v.casas() > casas {
        return Err(erro(
            format!(
                "{campo}: {} tem mais de {casas} casas decimais",
                v.fixo(v.e)
            ),
            campo,
        ));
    }
    Ok(v.fixo(casas))
}

/// Como [`num`], e confere tambem as casas inteiras que o tipo do XSD comporta (`TDec_1302` leva ate 13 digitos
/// antes da virgula e 2 depois: `casas` = 2, `inteiros` = 13).
pub(crate) fn num_lim(v: Dec, inteiros: usize, casas: u32, campo: &str) -> Res<String> {
    let s = num(v, casas, campo)?;
    let int = s.split('.').next().map_or(0, str::len);
    if int > inteiros {
        return Err(erro(
            format!("{campo}: {s} tem mais de {inteiros} digitos antes da virgula"),
            campo,
        ));
    }
    Ok(s)
}

pub(crate) fn num_txt(v: &Option<String>, casas: u32, campo: &str) -> Res<String> {
    num(dec(v, campo)?, casas, campo)
}

/// Campo opcional: ausente ou zero = a tag nao sai.
pub(crate) fn num_ou_none(v: &Option<String>, casas: u32, campo: &str) -> Res<Option<String>> {
    match v {
        None => Ok(None),
        Some(s) => {
            let d = Dec::parse(s, campo)?;
            if d.eh_zero() {
                Ok(None)
            } else {
                num(d, casas, campo).map(Some)
            }
        }
    }
}

pub(crate) fn dec(v: &Option<String>, campo: &str) -> Res<Dec> {
    match v {
        None => Ok(Dec::ZERO),
        Some(s) => Dec::parse(s, campo),
    }
}

pub(crate) fn exige<'a>(v: &'a Option<String>, campo: &str) -> Res<&'a Option<String>> {
    if v.is_none() {
        return Err(erro(format!("{campo}: obrigatorio"), campo));
    }
    Ok(v)
}

// ----------------------------------------------------------------------------------------------------- texto

/// Troca a letra acentuada (ISO 8859-1) pela sem acento; fora da faixa do ASCII imprimivel vira espaco ou `?`.
pub(crate) fn tira_acento(ch: char) -> char {
    let o = ch as u32;
    if o < 0x80 {
        return if (32..=126).contains(&o) || o == 13 || o == 10 || o == 8 {
            ch
        } else {
            ' '
        };
    }
    const CP1252: &str = "€‚ƒ„…†‡ˆ‰Š‹ŒŽ‘’“”•–—˜™š›œžŸ";
    if CP1252.contains(ch) {
        return ' ';
    }
    if !(0xA0..=0xFF).contains(&o) {
        return '?';
    }
    match o {
        192..=198 => 'A',
        199 => 'C',
        200..=203 => 'E',
        204..=207 => 'I',
        208 => 'D',
        209 => 'N',
        210..=214 => 'O',
        215 => 'x',
        216 | 248 => '0',
        217..=220 => 'U',
        221 => 'Y',
        222 | 254 => 'b',
        223 => 'B',
        224..=230 => 'a',
        231 => 'c',
        232..=235 => 'e',
        236..=239 => 'i',
        240 | 242..=246 => 'o',
        247 => '/',
        241 => 'n',
        249..=252 => 'u',
        253 | 255 => 'y',
        _ => ' ',
    }
}

/// Texto como o leiaute o quer: com `remover_acento`, so' ASCII; espacos repetidos viram um; quebra de linha vira `;`;
/// sem espaco nas pontas.
pub(crate) fn filtrar(texto: &str, remover_acento: bool) -> String {
    let mut s: String = if remover_acento {
        texto.chars().map(tira_acento).collect()
    } else {
        texto.to_string()
    };
    while s.contains("  ") {
        s = s.replace("  ", " ");
    }
    if !s.trim().is_empty() {
        s = s
            .replace("\r\n", "\n")
            .replace('\r', "\n")
            .replace('\n', ";");
    }
    s.trim_matches(|c: char| (c as u32) <= 32).to_string()
}

/// Texto filtrado e conferido contra o tamanho minimo e maximo do campo.
pub(crate) fn txt(v: &str, campo: &str, max: usize, min: usize, ra: bool) -> Res<String> {
    let s = filtrar(v, ra);
    let n = s.chars().count();
    if n < min {
        return Err(erro(
            format!("{campo}: obrigatorio (minimo {min} caractere(s))"),
            campo,
        ));
    }
    if n > max {
        return Err(erro(
            format!("{campo}: {n} caracteres, maximo {max}"),
            campo,
        ));
    }
    Ok(s)
}

pub(crate) fn txt_ou_none(
    v: &Option<String>,
    campo: &str,
    max: usize,
    min: usize,
    ra: bool,
) -> Res<Option<String>> {
    match v {
        None => Ok(None),
        Some(x) => {
            let s = filtrar(x, ra);
            if s.is_empty() {
                Ok(None)
            } else {
                txt(&s, campo, max, min, false).map(Some)
            }
        }
    }
}

/// So' os digitos (sem espaco, ponto, hifen, barra e parenteses), com um dos `tamanhos` do leiaute.
pub(crate) fn digitos(v: &str, campo: &str, tamanhos: &[usize]) -> Res<String> {
    let s: String = v
        .chars()
        .filter(|c| !(c.is_whitespace() || ".-/()".contains(*c)))
        .collect();
    if s.is_empty()
        || !s.bytes().all(|b| b.is_ascii_digit())
        || !s.is_ascii()
        || !tamanhos.contains(&s.len())
    {
        let t: Vec<String> = tamanhos.iter().map(|x| x.to_string()).collect();
        return Err(erro(
            format!(
                "{campo}: '{}' precisa ter {} digitos",
                corta(v, 30),
                t.join(" ou ")
            ),
            campo,
        ));
    }
    Ok(s)
}

/// (`CNPJ` ou `CPF`, valor). O CNPJ alfanumerico vale; so' ASCII.
pub(crate) fn documento(v: &str, campo: &str) -> Res<(&'static str, String)> {
    let s: String = v
        .chars()
        .filter(|c| !(c.is_whitespace() || "./-".contains(*c)))
        .collect::<String>()
        .to_uppercase();
    let b = s.as_bytes();
    if b.len() == 11 && b.iter().all(u8::is_ascii_digit) {
        return Ok(("CPF", s));
    }
    if b.len() == 14
        && b[..12]
            .iter()
            .all(|c| c.is_ascii_digit() || c.is_ascii_uppercase())
        && b[12..].iter().all(u8::is_ascii_digit)
    {
        return Ok(("CNPJ", s));
    }
    Err(erro(
        format!("{campo}: '{}' nao e CPF (11) nem CNPJ (14)", corta(v, 30)),
        campo,
    ))
}

/// Escapa o texto de um elemento XML.
pub(crate) fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

// ------------------------------------------------------------------------------------------------ data e hora

/// Codigo IBGE e sigla de cada UF.
pub(crate) const UFS: [(u32, &str); 27] = [
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

pub(crate) fn uf_codigo(uf: &str) -> Option<u32> {
    UFS.iter().find(|(_, s)| *s == uf).map(|(c, _)| *c)
}

pub(crate) fn uf_sigla(c: u32) -> Option<&'static str> {
    UFS.iter().find(|(x, _)| *x == c).map(|(_, s)| *s)
}

/// Fuso da UF (horario de Brasilia, ou 1 a 2 horas a menos no Norte e no Centro-Oeste; sem horario de verao).
pub(crate) fn fuso_uf(uf: &str) -> FixedOffset {
    let h = if uf == "AC" {
        5
    } else if ["AM", "RR", "RO", "MT", "MS"].contains(&uf) {
        4
    } else {
        3
    };
    FixedOffset::west_opt(h * 3600).unwrap_or_else(|| Utc.fix())
}

pub(crate) fn na_uf(quando: &DateTime<FixedOffset>, uf: &str) -> DateTime<FixedOffset> {
    quando.with_timezone(&fuso_uf(uf))
}

/// `AAAA-MM-DDThh:mm:ss-03:00` no fuso da UF.
pub(crate) fn dh(quando: &DateTime<FixedOffset>, uf: &str) -> String {
    na_uf(quando, uf).format("%Y-%m-%dT%H:%M:%S%:z").to_string()
}

/// Data `AAAA-MM-DD` (tipo `TData` do XSD: ano de 2000 a 2099, dia que existe no mes).
pub(crate) fn data_xml(v: &str, campo: &str) -> Res<NaiveDate> {
    let s = v.trim();
    let d = (s.len() == 10)
        .then(|| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .flatten()
        .filter(|d| (2000..=2099).contains(&d.year()));
    d.ok_or_else(|| {
        erro(
            format!(
                "{campo}: '{}' nao e' data AAAA-MM-DD (ano de 2000 a 2099)",
                corta(v, 20)
            ),
            campo,
        )
    })
}

/// ISO 8601 COM fuso. Sem fuso e' erro em `campo_fuso`: nao se chuta o horario de Brasilia.
pub(crate) fn data_hora(s: &str, campo: &str, campo_fuso: &str) -> Res<DateTime<FixedOffset>> {
    if let Ok(d) = DateTime::parse_from_rfc3339(s.trim()) {
        return Ok(d);
    }
    if NaiveDateTime::parse_from_str(s.trim(), "%Y-%m-%dT%H:%M:%S").is_ok() {
        return Err(erro(
            format!("{campo_fuso}: data/hora sem fuso (nao se chuta -03:00)"),
            campo_fuso,
        ));
    }
    Err(erro(
        format!("{campo}: data/hora ISO 8601 com fuso ('2026-10-03T10:30:00-03:00')"),
        campo,
    ))
}
