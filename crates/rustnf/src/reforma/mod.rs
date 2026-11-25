// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Reforma tributaria do consumo (IBS/CBS, LC 214/2025) como dado e regra.
//!
//! * [`tabela`] -- CST e cClassTrib oficiais (portal da NFe/SVRS e Calculadora da RFB), embutidos de
//!   `dados/cclasstrib.json` com `include_str!`;
//! * [`atividades_sn`] -- cAtivSN: catalogo de atividades do Simples Nacional (base da Calculadora RTC da RFB),
//!   embutido de `dados/atividades_sn.json`: anexo/fator r, documento admitido, motivos e lista da calculadora;
//! * [`indop`] -- cIndOp (Anexo VII da Reforma Tributaria do Consumo, v1.03.00), embutido de `dados/indop.json`: em que
//!   documento fiscal cada codigo vale;
//! * [`aliquotas`] -- calendario 2026-2033 e a aliquota do ANO do fato gerador (LC 214/2025, arts. 343-349);
//! * [`previa`] -- a base de calculo e o IBS/CBS de um servico, como a Sefin Nacional vai calcular;
//! * `calculo` (feature `nfe`) -- o IBS/CBS de uma mercadoria por item e o total da NF-e (`IBSCBSTot`, `vNFTot`).
//!
//! A DPS nao leva valor de IBS/CBS calculado (quem calcula e' a Sefin Nacional): este modulo serve para conferir
//! o que vai na DPS e para consultar as tabelas e as aliquotas. Percentual em x10000 (4 casas, `i64`). Todo
//! arredondamento e' "meio para o par", como a calculadora oficial (registro de decisao ADR003): os VALORES da previa
//! (base, aliquota efetiva, IBS, CBS, veja [`previa`]) e a LEITURA de um texto de aliquota ou de reducao com mais de 4
//! casas. Os casos em `tests/casos/reforma.json` conferem texto a texto.
pub mod aliquotas;
pub mod atividades_sn;
#[cfg(feature = "nfe")]
pub mod calculo;
mod decimal;
pub mod indop;
pub mod previa;
pub mod tabela;

use std::fmt;

/// Erro de negocio com mensagem pronta para mostrar a quem emite; `regra` = regra/rejeicao da NT quando ha (ex.: `UB14-20/1024`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErroReforma {
    pub mensagem: String,
    pub regra: String,
}

impl ErroReforma {
    pub fn novo(mensagem: impl Into<String>, regra: impl Into<String>) -> Self {
        ErroReforma {
            mensagem: mensagem.into(),
            regra: regra.into(),
        }
    }
}

impl fmt::Display for ErroReforma {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.mensagem)
    }
}

impl std::error::Error for ErroReforma {}

pub type Resultado<T> = Result<T, ErroReforma>;

/// Divisao inteira com arredondamento meio-para-o-par: no empate exato fica o quociente par; fora dele, o mais
/// proximo. Negativo espelha.
pub(crate) fn div_arred(num: i128, den: i128) -> i128 {
    debug_assert!(den > 0);
    let q = num / den;
    let r2 = (num % den).abs() * 2;
    if r2 > den || (r2 == den && q % 2 != 0) {
        q + num.signum()
    } else {
        q
    }
}

/// Texto decimal ('10,5', '-1.25', '') -> inteiro com `casas` casas, meio-para-o-par no que sobrar. Vazio = 0.
pub(crate) fn escala(s: &str, casas: u32, campo: &str) -> Resultado<i128> {
    let t = s.trim().replace(',', ".");
    if t.is_empty() {
        return Ok(0);
    }
    let (neg, corpo) = match t.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, t.strip_prefix('+').unwrap_or(&t)),
    };
    let (i, f) = corpo.split_once('.').unwrap_or((corpo, ""));
    let ok = !(i.is_empty() && f.is_empty())
        && i.bytes().all(|b| b.is_ascii_digit())
        && f.bytes().all(|b| b.is_ascii_digit())
        && i.len() <= 20
        && f.len() <= 18;
    if !ok {
        let curto: String = s.chars().take(20).collect();
        return Err(ErroReforma::novo(
            format!("{campo}: '{curto}' nao e numero"),
            "",
        ));
    }
    let fe = f.len() as u32;
    let m: i128 = format!("{}{}", if i.is_empty() { "0" } else { i }, f)
        .parse()
        .unwrap_or(0);
    let v = if fe <= casas {
        m * 10i128.pow(casas - fe)
    } else {
        div_arred(m, 10i128.pow(fe - casas))
    };
    Ok(if neg { -v } else { v })
}

/// Centavos -> "123.45".
#[cfg(feature = "nfe")]
pub(crate) fn fmt2(v: i64) -> String {
    fmt_casas(v as i128, 2)
}

/// x10000 -> "0.9000".
pub(crate) fn fmt4(v: i64) -> String {
    fmt_casas(v as i128, 4)
}

pub(crate) fn fmt_casas(v: i128, casas: u32) -> String {
    let neg = v < 0;
    let d = v.unsigned_abs();
    let p = 10u128.pow(casas);
    format!(
        "{}{}.{:0w$}",
        if neg { "-" } else { "" },
        d / p,
        d % p,
        w = casas as usize
    )
}

/// x10000 -> texto normalizado ('8.75', '0.1', '0').
pub(crate) fn fmt_norm4(v: i64) -> String {
    let s = fmt4(v);
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    if s.is_empty() || s == "-" {
        "0".into()
    } else {
        s
    }
}

/// Ano do FATO GERADOR ('AAAA', 'AAAA-MM', 'AAAA-MM-DD...'). Nunca o relogio.
pub fn ano_de(data: &str) -> Resultado<i32> {
    let t = data.trim();
    let a = t.get(..4).filter(|x| x.bytes().all(|b| b.is_ascii_digit()));
    match a {
        Some(x) => Ok(x.parse().unwrap_or(0)),
        None => {
            let curto: String = data.chars().take(20).collect();
            Err(ErroReforma::novo(
                format!("data/competencia invalida: '{curto}' (use AAAA-MM-DD ou AAAA-MM)"),
                "",
            ))
        }
    }
}

/// 'AAAA-MM-DD' (dia 01 quando vem so' AAAA-MM; 01-01 quando vem so' o ano) -- para comparar com vigencia.
pub fn data_iso(data: &str) -> Resultado<String> {
    let t = data.trim();
    let ano = ano_de(t)?;
    let dois = |s: Option<&str>| {
        s.filter(|x| x.len() == 2 && x.bytes().all(|b| b.is_ascii_digit()))
            .map(String::from)
    };
    let mut mes = None;
    let mut dia = None;
    if t.as_bytes().get(4) == Some(&b'-') {
        mes = dois(t.get(5..7));
        if mes.is_some() && t.as_bytes().get(7) == Some(&b'-') {
            dia = dois(t.get(8..10));
        }
    }
    Ok(format!(
        "{ano:04}-{}-{}",
        mes.unwrap_or_else(|| "01".into()),
        dia.unwrap_or_else(|| "01".into())
    ))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod testes {
    use super::*;

    #[test]
    fn escala_e_formato() {
        assert_eq!(escala("5.555", 2, "x").unwrap(), 556);
        assert_eq!(escala("-0.005", 2, "x").unwrap(), 0); // empate: fica o par (0)
        assert_eq!(escala("-0.015", 2, "x").unwrap(), -2);
        assert_eq!(escala("0.025", 2, "x").unwrap(), 2);
        assert_eq!(escala("0.0250001", 2, "x").unwrap(), 3);
        assert_eq!(div_arred(225, 1000), 0);
        assert_eq!(div_arred(2250, 1000), 2);
        assert_eq!(div_arred(3500, 1000), 4);
        assert_eq!(div_arred(-2500, 1000), -2);
        assert_eq!(div_arred(-1500, 1000), -2);
        assert_eq!(div_arred(-1501, 1000), -2);
        assert_eq!(div_arred(2501, 1000), 3);
        assert_eq!(escala("10,5", 4, "x").unwrap(), 105_000);
        assert_eq!(escala("", 2, "x").unwrap(), 0);
        assert!(escala("1a", 2, "x").is_err());
        assert_eq!(fmt_casas(-5, 2), "-0.05");
        assert_eq!(fmt4(9000), "0.9000");
        assert_eq!(fmt_norm4(87_500), "8.75");
        assert_eq!(fmt_norm4(0), "0");
        assert_eq!(data_iso("2026-1").unwrap(), "2026-01-01");
        assert_eq!(data_iso("2026-10-03T10:00").unwrap(), "2026-10-03");
    }
}
