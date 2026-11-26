// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Formatos de impressao do DANFE e do DANFC-e: decimal em ponto fixo (nunca ponto flutuante), documento, CEP, telefone, data e hora no fuso da
//! UF do emitente.

use super::montagem::na_uf;
use super::{DataHora, Decimal};

/// `valor` com `casas` casas, arredondado meio-para-cima (afastando do zero), em inteiros: o texto do decimal e'
/// quebrado em inteiro e fracao e nunca passa por `f64`.
fn escalar(v: Decimal, casas: u32) -> (bool, u128) {
    let texto = v.to_string();
    let (negativo, abs) = texto
        .strip_prefix('-')
        .map_or((false, texto.as_str()), |r| (true, r));
    let (inteiro, fracao) = abs.split_once('.').unwrap_or((abs, ""));
    let casas = casas as usize;
    let mut digitos: String = inteiro.to_string();
    let mut fr: String = fracao.chars().take(casas).collect();
    while fr.len() < casas {
        fr.push('0');
    }
    digitos.push_str(&fr);
    let mut n: u128 = digitos.parse().unwrap_or(0);
    if fracao.as_bytes().get(casas).is_some_and(|c| *c >= b'5') {
        n += 1;
    }
    (negativo, n)
}

/// `1.234,56`: milhar com ponto, decimal com virgula, `casas` casas. Ausente = zero.
#[must_use]
pub(crate) fn brl(v: Option<Decimal>, casas: u32) -> String {
    let (negativo, n) = escalar(v.unwrap_or(Decimal::ZERO), casas);
    let base = 10u128.pow(casas);
    let inteiro = (n / base).to_string();
    let mut agrupado = String::new();
    for (i, c) in inteiro.chars().enumerate() {
        if i > 0 && (inteiro.len() - i).is_multiple_of(3) {
            agrupado.push('.');
        }
        agrupado.push(c);
    }
    let sinal = if negativo && n != 0 { "-" } else { "" };
    if casas == 0 {
        format!("{sinal}{agrupado}")
    } else {
        format!(
            "{sinal}{agrupado},{:0>largura$}",
            n % base,
            largura = casas as usize
        )
    }
}

/// Quantidade sem os zeros a direita: `2` vira `2`, `1.5000` vira `1,5`.
#[must_use]
pub(crate) fn qtd(v: Option<Decimal>) -> String {
    let s = v.unwrap_or(Decimal::ZERO).to_string();
    let s = if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    };
    s.replace('.', ",")
}

/// Casas do valor unitario: as significativas de `vUnCom`, entre 2 e 4 (o combustivel tem 3; o leiaute vai a 10).
#[must_use]
pub(crate) fn casas_unitario(v: Option<Decimal>) -> u32 {
    let Some(v) = v else { return 2 };
    let s = v.to_string();
    let n = s
        .split_once('.')
        .map_or(0, |(_, f)| f.trim_end_matches('0').len());
    n.clamp(2, 4) as u32
}

#[cfg(feature = "danfe")]
/// Aliquota com no minimo 2 e no maximo 4 casas, e `%`: `0,00%` a `0,0000%`.
#[must_use]
pub(crate) fn pct(v: Option<Decimal>) -> String {
    let s = brl(v, 4);
    let (inteiro, dec) = s.split_once(',').unwrap_or((s.as_str(), ""));
    let mut dec = dec.trim_end_matches('0').to_string();
    while dec.len() < 2 {
        dec.push('0');
    }
    format!("{inteiro},{dec}%")
}

fn alfanumerico(valor: &str) -> String {
    valor
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

/// CNPJ (`99.999.999/9999-99`, com letras no CNPJ alfanumerico) ou CPF (`999.999.999-99`) com a mascara; documento
/// de outro tipo ou de tamanho errado sai como veio.
#[must_use]
pub(crate) fn documento(tipo: &str, valor: &str) -> String {
    let v = alfanumerico(valor);
    if tipo == "cnpj" && v.len() == 14 {
        return format!(
            "{}.{}.{}/{}-{}",
            &v[..2],
            &v[2..5],
            &v[5..8],
            &v[8..12],
            &v[12..]
        );
    }
    if tipo == "cpf" && v.len() == 11 {
        return format!("{}.{}.{}-{}", &v[..3], &v[3..6], &v[6..9], &v[9..]);
    }
    valor.to_string()
}

#[cfg(feature = "danfe")]
/// `99999-999`; fora de 8 algarismos, so' os algarismos.
#[must_use]
pub(crate) fn cep(c: &str) -> String {
    let c: String = c.chars().filter(char::is_ascii_digit).collect();
    if c.len() == 8 {
        format!("{}-{}", &c[..5], &c[5..])
    } else {
        c
    }
}

#[cfg(feature = "danfe")]
/// `(99) 9999-9999` ou `(99) 99999-9999`; de outro tamanho, so' os algarismos.
#[must_use]
pub(crate) fn fone(f: &str) -> String {
    let f: String = f.chars().filter(char::is_ascii_digit).collect();
    if f.len() == 10 || f.len() == 11 {
        return format!("({}) {}-{}", &f[..2], &f[2..f.len() - 4], &f[f.len() - 4..]);
    }
    f
}

#[cfg(feature = "danfe")]
/// Numero da nota em tres grupos de tres algarismos: `000.000.123`.
#[must_use]
pub(crate) fn num9(n: u64) -> String {
    let s = format!("{n:09}");
    format!("{}.{}.{}", &s[..3], &s[3..6], &s[6..])
}

/// `dd/mm/aaaa hh:mm:ss` no horario da UF; sem data, vazio.
#[must_use]
pub(crate) fn local(d: Option<&DataHora>, uf: &str) -> String {
    d.map_or_else(String::new, |d| {
        na_uf(&d.0, uf).format("%d/%m/%Y %H:%M:%S").to_string()
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod testes {
    use super::*;

    fn d(s: &str) -> Option<Decimal> {
        Decimal::ler(s)
    }

    #[test]
    fn dinheiro_agrupa_e_arredonda_para_cima() {
        assert_eq!(brl(d("1234.565"), 2), "1.234,57");
        assert_eq!(brl(d("1234567.8"), 2), "1.234.567,80");
        assert_eq!(brl(d("0"), 2), "0,00");
        assert_eq!(brl(None, 2), "0,00");
        assert_eq!(brl(d("999.995"), 2), "1.000,00");
        assert_eq!(brl(d("12.5"), 0), "13");
        assert_eq!(brl(d("0.001"), 2), "0,00");
        assert_eq!(brl(d("15.9000"), 4), "15,9000");
    }

    #[test]
    fn quantidade_e_casas_do_unitario() {
        assert_eq!(qtd(d("2")), "2");
        assert_eq!(qtd(d("1.5000")), "1,5");
        assert_eq!(qtd(d("100.0000")), "100");
        assert_eq!(casas_unitario(d("15.90")), 2);
        assert_eq!(casas_unitario(d("5.499")), 3);
        assert_eq!(casas_unitario(d("0.1234567")), 4);
    }

    #[test]
    fn aliquota_tem_ao_menos_duas_casas() {
        assert_eq!(pct(d("0.9")), "0,90%");
        assert_eq!(pct(d("0.0500")), "0,05%");
        assert_eq!(pct(d("12.3456")), "12,3456%");
        assert_eq!(pct(None), "0,00%");
    }

    #[test]
    fn mascaras() {
        assert_eq!(documento("cnpj", "11222333000181"), "11.222.333/0001-81");
        assert_eq!(documento("cpf", "52998224725"), "529.982.247-25");
        assert_eq!(documento("estrangeiro", "AB123"), "AB123");
        assert_eq!(cep("01001000"), "01001-000");
        assert_eq!(fone("1133334444"), "(11) 3333-4444");
        assert_eq!(fone("11933334444"), "(11) 93333-4444");
        assert_eq!(num9(123), "000.000.123");
    }
}
