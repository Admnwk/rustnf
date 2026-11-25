// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Decimal e data/hora do leiaute da NF-e.
//!
//! [`Decimal`] guarda a mantissa inteira e o numero de casas, sem `f64`: o texto do XML volta igual (`10.50` continua
//! `10.50`) e a soma de valores da nota fecha no centavo. [`DataHora`] e' a data com hora e fuso do leiaute (`TDateTimeUTC`).

use std::fmt;

use chrono::{DateTime, FixedOffset};
use serde::{Serialize, Serializer};

/// Decimal do leiaute (`TDec_*`): mantissa inteira e casas. A soma e a subtracao alinham as casas.
#[derive(Debug, Clone, Copy, Default)]
pub struct Decimal {
    m: i128,
    e: u32,
}

impl Decimal {
    /// Zero, sem casas.
    pub const ZERO: Decimal = Decimal { m: 0, e: 0 };

    /// Le o formato do leiaute: `0` ou `[1-9][0-9]*`, com `.casas` opcional. Sem sinal, virgula, expoente nem zero a
    /// esquerda. Aceita ate 18 digitos inteiros e 12 casas (o leiaute vai a 13 inteiros e 10 casas).
    #[must_use]
    pub fn ler(s: &str) -> Option<Decimal> {
        let (inteiro, fracao) = s.split_once('.').unwrap_or((s, ""));
        let sem_zero_a_esquerda = inteiro
            .as_bytes()
            .first()
            .is_some_and(|c| (b'1'..=b'9').contains(c));
        let inteiro_ok =
            inteiro == "0" || (sem_zero_a_esquerda && inteiro.bytes().all(|c| c.is_ascii_digit()));
        let fracao_ok =
            !s.contains('.') || (!fracao.is_empty() && fracao.bytes().all(|c| c.is_ascii_digit()));
        if !inteiro_ok || !fracao_ok || inteiro.len() > 18 || fracao.len() > 12 {
            return None;
        }
        let m: i128 = format!("{inteiro}{fracao}").parse().ok()?;
        Some(Decimal {
            m,
            e: fracao.len() as u32,
        })
    }

    fn alinhar(a: Decimal, b: Decimal) -> (i128, i128, u32) {
        let e = a.e.max(b.e);
        (a.m * 10i128.pow(e - a.e), b.m * 10i128.pow(e - b.e), e)
    }

    /// Soma, com as casas do operando que tem mais.
    #[must_use]
    pub fn mais(self, o: Decimal) -> Decimal {
        let (a, b, e) = Self::alinhar(self, o);
        Decimal { m: a + b, e }
    }

    /// Diferenca, com as casas do operando que tem mais.
    #[must_use]
    pub fn menos(self, o: Decimal) -> Decimal {
        let (a, b, e) = Self::alinhar(self, o);
        Decimal { m: a - b, e }
    }
}

impl PartialEq for Decimal {
    fn eq(&self, o: &Decimal) -> bool {
        let (a, b, _) = Decimal::alinhar(*self, *o);
        a == b
    }
}

impl Eq for Decimal {}

impl fmt::Display for Decimal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sinal = if self.m < 0 { "-" } else { "" };
        let digitos = self.m.unsigned_abs().to_string();
        let casas = self.e as usize;
        if casas == 0 {
            write!(f, "{sinal}{digitos}")
        } else if digitos.len() > casas {
            let corte = digitos.len() - casas;
            write!(f, "{sinal}{}.{}", &digitos[..corte], &digitos[corte..])
        } else {
            write!(f, "{sinal}0.{}{digitos}", "0".repeat(casas - digitos.len()))
        }
    }
}

impl Serialize for Decimal {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl std::iter::Sum for Decimal {
    fn sum<I: Iterator<Item = Decimal>>(it: I) -> Decimal {
        it.fold(Decimal::ZERO, Decimal::mais)
    }
}

/// Data e hora com fuso. Serializa como `AAAA-MM-DDThh:mm:ss+hh:mm` (sem fracao de segundo).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DataHora(pub DateTime<FixedOffset>);

impl fmt::Display for DataHora {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.format("%Y-%m-%dT%H:%M:%S%:z"))
    }
}

impl Serialize for DataHora {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod testes {
    use super::*;

    #[test]
    fn soma_mantem_as_casas() {
        let v = |s| Decimal::ler(s).unwrap();
        assert_eq!(v("0.10").mais(v("0.20")).mais(v("0.30")), v("0.6"));
        assert_eq!(
            v("0.10").mais(v("0.20")).mais(v("0.30")).to_string(),
            "0.60"
        );
        assert_eq!(v("1000.00").menos(v("10.00")).to_string(), "990.00");
        assert_eq!(v("0.0000001000").to_string(), "0.0000001000");
        assert_eq!(v("10").menos(v("12.5")).to_string(), "-2.5");
    }

    #[test]
    fn recusa_o_que_o_leiaute_nao_tem() {
        for ruim in [
            "10,00", "-1", "+1", "01", "1e3", "1.", ".5", "", " 1", "1.5.2",
        ] {
            assert!(Decimal::ler(ruim).is_none(), "{ruim:?}");
        }
        assert!(Decimal::ler("1234567890123456789").is_none(), "19 inteiros");
        assert!(Decimal::ler("1.1234567890123").is_none(), "13 casas");
        assert!(Decimal::ler("999999999999999999.999999999999").is_some());
    }
}
