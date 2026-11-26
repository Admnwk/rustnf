// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Metrica da Liberation Sans (a mesma da Arial, NT 008 item 2.4): largura de texto, corte com reticencias e quebra
//! de linha, para o leiaute decidir as linhas antes de desenhar. Com a feature `danfe`, a mesma conta serve a
//! Liberation Serif (metrica da Times New Roman, a fonte do DANFE): as funcoes `*_em` recebem a [`Familia`] e as de
//! nome simples usam a Sans.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde_json::Value;

const LARGURAS: &str = include_str!("../../dados/fontes/LiberationSans-larguras.json");
#[cfg(feature = "danfe")]
const LARGURAS_SERIF: &str = include_str!("../../dados/fontes/LiberationSerif-larguras.json");

/// A familia da fonte que decide a largura: a Sans (metrica da Arial, DANFSe) ou a Serif (metrica da Times New Roman,
/// DANFE).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Familia {
    Sans,
    #[cfg(feature = "danfe")]
    Serif,
}

struct Larguras {
    em: f64,
    regular: BTreeMap<u32, i64>,
    bold: BTreeMap<u32, i64>,
}

fn carregar(texto: &str) -> Larguras {
    let v: Value = serde_json::from_str(texto).expect("tabela de larguras embutida invalida");
    let m = |k: &str| -> BTreeMap<u32, i64> {
        v[k].as_object()
            .expect("tabela de larguras")
            .iter()
            .map(|(c, w)| (c.parse().expect("codigo"), w.as_i64().expect("largura")))
            .collect()
    };
    Larguras {
        em: v["unidades_em"].as_f64().expect("unidades_em"),
        regular: m("regular"),
        bold: m("bold"),
    }
}

fn larguras(fam: Familia) -> &'static Larguras {
    static SANS: OnceLock<Larguras> = OnceLock::new();
    match fam {
        Familia::Sans => SANS.get_or_init(|| carregar(LARGURAS)),
        #[cfg(feature = "danfe")]
        Familia::Serif => {
            static SERIF: OnceLock<Larguras> = OnceLock::new();
            SERIF.get_or_init(|| carregar(LARGURAS_SERIF))
        }
    }
}

fn avanco(fam: Familia, c: char, negrito: bool) -> i64 {
    let l = larguras(fam);
    let t = if negrito { &l.bold } else { &l.regular };
    t.get(&(c as u32))
        .copied()
        .unwrap_or_else(|| t[&('M' as u32)])
}

/// Largura em pt do texto na Liberation Sans (sem kerning); caractere fora da tabela = largura do 'M'.
#[must_use]
pub(super) fn largura(texto: &str, tamanho: f64, negrito: bool) -> f64 {
    largura_em(Familia::Sans, texto, tamanho, negrito)
}

/// [`largura`] na familia dada.
#[must_use]
pub(crate) fn largura_em(fam: Familia, texto: &str, tamanho: f64, negrito: bool) -> f64 {
    let soma: i64 = texto.chars().map(|c| avanco(fam, c, negrito)).sum();
    soma as f64 * tamanho / larguras(fam).em
}

fn prefixo(fam: Familia, texto: &[char], tamanho: f64, w: f64, negrito: bool) -> usize {
    let mut soma: i64 = 0;
    for (i, c) in texto.iter().enumerate() {
        soma += avanco(fam, *c, negrito);
        if soma as f64 * tamanho / larguras(fam).em > w {
            return i;
        }
    }
    texto.len()
}

/// O texto se cabe em `w` pt; senao o maior prefixo + "..." que cabe ("" se nem "..." cabe).
#[must_use]
pub(super) fn caber(texto: &str, tamanho: f64, w: f64, negrito: bool) -> String {
    caber_em(Familia::Sans, texto, tamanho, w, negrito)
}

/// [`caber`] na familia dada.
#[must_use]
pub(crate) fn caber_em(fam: Familia, texto: &str, tamanho: f64, w: f64, negrito: bool) -> String {
    if texto.is_empty() || largura_em(fam, texto, tamanho, negrito) <= w {
        return texto.into();
    }
    let c: Vec<char> = texto.chars().collect();
    let (mut lo, mut hi, mut melhor) = (0i64, c.len() as i64 - 1, String::new());
    while lo <= hi {
        let meio = (lo + hi) / 2;
        let p: String = c[..meio as usize].iter().collect();
        let cand = format!("{}...", p.trim_end_matches(' '));
        if largura_em(fam, &cand, tamanho, negrito) <= w {
            melhor = cand;
            lo = meio + 1;
        } else {
            hi = meio - 1;
        }
    }
    if melhor == "..." {
        String::new()
    } else {
        melhor
    }
}

fn strip(c: &[char]) -> &[char] {
    let mut a = 0;
    let mut b = c.len();
    while a < b && c[a].is_whitespace() {
        a += 1;
    }
    while b > a && c[b - 1].is_whitespace() {
        b -= 1;
    }
    &c[a..b]
}

fn proxima(fam: Familia, texto: &[char], tamanho: f64, w: f64, negrito: bool) -> (String, usize) {
    let n = prefixo(fam, texto, tamanho, w, negrito);
    if n == 0 {
        return (String::new(), 0);
    }
    let mut parte: Vec<char> = strip(&texto[..n]).to_vec();
    let mut consumido = n;
    if n < texto.len() {
        let k = parte.iter().rposition(|&c| c == ' ' || c == '\n');
        if let Some(k) = k.filter(|&k| k > 0) {
            parte = strip(&parte[..k]).to_vec();
            let mut f = k;
            while f > 0 && (texto[f - 1] == ' ' || texto[f - 1] == '\n') {
                f -= 1;
            }
            consumido = f;
        }
    }
    (parte.into_iter().collect(), consumido)
}

/// Linhas de no maximo `w` pt (quebra na ultima palavra; palavra maior que a linha quebra por caractere); '\n' no
/// texto = quebra forcada.
#[must_use]
pub(super) fn quebrar(texto: &str, tamanho: f64, w: f64) -> Vec<String> {
    quebrar_em(Familia::Sans, texto, tamanho, w, false)
}

/// [`quebrar`] na familia dada, medindo em regular ou em negrito.
#[must_use]
pub(crate) fn quebrar_em(
    fam: Familia,
    texto: &str,
    tamanho: f64,
    w: f64,
    negrito: bool,
) -> Vec<String> {
    let mut out = Vec::new();
    for par in texto.split('\n') {
        let todos: Vec<char> = par.chars().collect();
        let mut resto: Vec<char> = strip(&todos).to_vec();
        while !resto.is_empty() {
            let (parte, consumido) = proxima(fam, &resto, tamanho, w, negrito);
            if parte.is_empty() || consumido == 0 {
                break;
            }
            out.push(parte);
            resto = strip(&resto[consumido..]).to_vec();
        }
    }
    out
}

pub(super) fn limitar(linhas: Vec<String>, n: usize, tamanho: f64, w: f64) -> Vec<String> {
    if linhas.len() <= n {
        return linhas;
    }
    let mut c: Vec<String> = linhas.into_iter().take(n).collect();
    let ult = c.len() - 1;
    c[ult] = caber(&format!("{}...", c[ult]), tamanho, w, false);
    c
}
