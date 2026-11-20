// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Decimal exato de ponto fixo para a conta do IBS/CBS.
//!
//! Soma, subtracao e multiplicacao nao arredondam; cada numero publicado e' arredondado uma unica vez, no fim,
//! com "meio para o par" (ABNT NBR 5891), como a calculadora oficial da Receita Federal
//! (<https://github.com/ReceitaFederal/calculadora-rtc-regime-regular>, registro de decisao ADR003).
//! Toda operacao que estouraria devolve `None`: quem chama transforma em erro.

use std::cmp::Ordering;

/// Maximo de digitos antes da virgula aceito na entrada.
const MAX_INTEIROS: usize = 15;
/// Maximo de casas decimais aceito na entrada.
const MAX_CASAS: usize = 12;

/// `mantissa / 10^casas`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Dec {
    mantissa: i128,
    casas: u32,
}

impl Dec {
    pub(crate) const ZERO: Dec = Dec {
        mantissa: 0,
        casas: 0,
    };

    pub(crate) const fn novo(mantissa: i128, casas: u32) -> Dec {
        Dec { mantissa, casas }
    }

    /// `"10,5"`, `"-1.313"`, `"7"`; vazio vale zero. `None` se nao for numero ou passar dos limites de entrada.
    pub(crate) fn de_texto(texto: &str) -> Option<Dec> {
        let t = texto.trim().replace(',', ".");
        if t.is_empty() {
            return Some(Dec::ZERO);
        }
        let (negativo, corpo) = match t.strip_prefix('-') {
            Some(resto) => (true, resto),
            None => (false, t.strip_prefix('+').unwrap_or(&t)),
        };
        let (inteiros, fracao) = corpo.split_once('.').unwrap_or((corpo, ""));
        let so_digitos = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
        if (inteiros.is_empty() && fracao.is_empty())
            || !so_digitos(inteiros)
            || !so_digitos(fracao)
            || inteiros.len() > MAX_INTEIROS
            || fracao.len() > MAX_CASAS
        {
            return None;
        }
        let mantissa: i128 = format!("{inteiros}{fracao}").parse().ok()?;
        Some(Dec {
            mantissa: if negativo { -mantissa } else { mantissa },
            casas: fracao.len() as u32,
        })
    }

    fn potencia(casas: u32) -> Option<i128> {
        10i128.checked_pow(casas)
    }

    /// A mantissa com `casas` casas (so' para `casas >= self.casas`).
    fn mantissa_em(self, casas: u32) -> Option<i128> {
        self.mantissa
            .checked_mul(Self::potencia(casas.checked_sub(self.casas)?)?)
    }

    pub(crate) fn somar(self, outro: Dec) -> Option<Dec> {
        let casas = self.casas.max(outro.casas);
        Some(Dec {
            mantissa: self
                .mantissa_em(casas)?
                .checked_add(outro.mantissa_em(casas)?)?,
            casas,
        })
    }

    pub(crate) fn subtrair(self, outro: Dec) -> Option<Dec> {
        self.somar(Dec {
            mantissa: outro.mantissa.checked_neg()?,
            casas: outro.casas,
        })
    }

    pub(crate) fn multiplicar(self, outro: Dec) -> Option<Dec> {
        Some(Dec {
            mantissa: self.mantissa.checked_mul(outro.mantissa)?,
            casas: self.casas.checked_add(outro.casas)?,
        })
    }

    /// Divide por 100 sem arredondar (pontos percentuais -> fracao).
    pub(crate) fn por_cento(self) -> Option<Dec> {
        Some(Dec {
            mantissa: self.mantissa,
            casas: self.casas.checked_add(2)?,
        })
    }

    /// Arredonda a `casas` casas com "meio para o par".
    pub(crate) fn arredondar(self, casas: u32) -> Option<Dec> {
        if self.casas <= casas {
            return Some(Dec {
                mantissa: self.mantissa_em(casas)?,
                casas,
            });
        }
        let divisor = Self::potencia(self.casas - casas)?;
        let quociente = self.mantissa / divisor;
        let resto = (self.mantissa % divisor).abs();
        let sobe = match resto.checked_mul(2)?.cmp(&divisor) {
            Ordering::Greater => true,
            Ordering::Equal => quociente % 2 != 0,
            Ordering::Less => false,
        };
        Some(Dec {
            mantissa: if sobe {
                quociente + self.mantissa.signum()
            } else {
                quociente
            },
            casas,
        })
    }

    /// A mantissa como `i64` (para um numero ja arredondado nas casas que se quer guardar).
    pub(crate) fn mantissa_i64(self) -> Option<i64> {
        i64::try_from(self.mantissa).ok()
    }

    pub(crate) fn negativo(self) -> bool {
        self.mantissa < 0
    }

    pub(crate) fn positivo(self) -> bool {
        self.mantissa > 0
    }

    /// `self >= outro`.
    pub(crate) fn maior_ou_igual(self, outro: Dec) -> Option<bool> {
        Some(!self.subtrair(outro)?.negativo())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod testes {
    use super::*;

    fn d(s: &str) -> Dec {
        Dec::de_texto(s).unwrap()
    }

    fn arred(s: &str, casas: u32) -> i128 {
        d(s).arredondar(casas).unwrap().mantissa
    }

    #[test]
    fn le_texto_decimal() {
        assert_eq!(d("10,5"), Dec::novo(105, 1));
        assert_eq!(d(" -1.313 "), Dec::novo(-1313, 3));
        assert_eq!(d("+7"), Dec::novo(7, 0));
        assert_eq!(d(""), Dec::ZERO);
        assert_eq!(d(".5"), Dec::novo(5, 1));
        for ruim in [
            "1a",
            "1.2.3",
            "-",
            ".",
            "1e3",
            "1234567890123456",
            "0.1234567890123",
        ] {
            assert!(Dec::de_texto(ruim).is_none(), "{ruim}");
        }
    }

    #[test]
    fn meio_vai_para_o_par() {
        assert_eq!(arred("0.225", 2), 22);
        assert_eq!(arred("0.235", 2), 24);
        assert_eq!(arred("0.025", 2), 2);
        assert_eq!(arred("0.2251", 2), 23);
        assert_eq!(arred("0.2249", 2), 22);
        assert_eq!(arred("-1.313", 2), -131);
        assert_eq!(arred("-0.125", 2), -12);
        assert_eq!(arred("-0.135", 2), -14);
        assert_eq!(arred("3.52492", 4), 35249);
        assert_eq!(arred("7", 2), 700);
    }

    #[test]
    fn conta_exata() {
        let p = d("33.33")
            .por_cento()
            .unwrap()
            .multiplicar(d("1000.05"))
            .unwrap();
        assert_eq!(p, Dec::novo(333_316_665, 6));
        assert_eq!(p.arredondar(2).unwrap().mantissa, 33_332);
        assert_eq!(d("100").subtrair(d("5.5")).unwrap(), Dec::novo(945, 1));
        assert_eq!(d("0.1").somar(d("0.02")).unwrap(), Dec::novo(12, 2));
        assert!(d("5").maior_ou_igual(d("5.00")).unwrap());
        assert!(!d("4.99").maior_ou_igual(d("5")).unwrap());
        assert!(d("-0.01").negativo() && d("0.01").positivo() && !Dec::ZERO.positivo());
    }

    #[test]
    fn estouro_vira_none() {
        let grande = Dec::novo(i128::MAX, 0);
        assert!(grande.somar(Dec::novo(1, 0)).is_none());
        assert!(grande.multiplicar(Dec::novo(2, 0)).is_none());
        assert!(Dec::novo(1, 0).mantissa_em(60).is_none());
        assert!(Dec::novo(i128::from(i64::MAX) + 1, 0)
            .mantissa_i64()
            .is_none());
    }
}
