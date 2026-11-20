// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Metrica da Liberation Sans (a mesma da Arial, NT 008 item 2.4): largura de texto, corte com reticencias e quebra
//! de linha, para o leiaute decidir as linhas antes de desenhar.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde_json::Value;

const LARGURAS: &str = include_str!("../../dados/fontes/LiberationSans-larguras.json");

struct Larguras {
    em: f64,
    regular: BTreeMap<u32, i64>,
    bold: BTreeMap<u32, i64>,
}

fn larguras() -> &'static Larguras {
    static L: OnceLock<Larguras> = OnceLock::new();
    L.get_or_init(|| {
        let v: Value =
            serde_json::from_str(LARGURAS).expect("LiberationSans-larguras.json embutido invalido");
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
    })
}

fn avanco(c: char, negrito: bool) -> i64 {
    let l = larguras();
    let t = if negrito { &l.bold } else { &l.regular };
    t.get(&(c as u32))
        .copied()
        .unwrap_or_else(|| t[&('M' as u32)])
}

/// Largura em pt do texto na Liberation Sans (sem kerning); caractere fora da tabela = largura do 'M'.
#[must_use]
pub(super) fn largura(texto: &str, tamanho: f64, negrito: bool) -> f64 {
    let soma: i64 = texto.chars().map(|c| avanco(c, negrito)).sum();
    soma as f64 * tamanho / larguras().em
}

fn prefixo(texto: &[char], tamanho: f64, w: f64) -> usize {
    let mut soma: i64 = 0;
    for (i, c) in texto.iter().enumerate() {
        soma += avanco(*c, false);
        if soma as f64 * tamanho / larguras().em > w {
            return i;
        }
    }
    texto.len()
}

/// O texto se cabe em `w` pt; senao o maior prefixo + "..." que cabe ("" se nem "..." cabe).
#[must_use]
pub(super) fn caber(texto: &str, tamanho: f64, w: f64, negrito: bool) -> String {
    if texto.is_empty() || largura(texto, tamanho, negrito) <= w {
        return texto.into();
    }
    let c: Vec<char> = texto.chars().collect();
    let (mut lo, mut hi, mut melhor) = (0i64, c.len() as i64 - 1, String::new());
    while lo <= hi {
        let meio = (lo + hi) / 2;
        let p: String = c[..meio as usize].iter().collect();
        let cand = format!("{}...", p.trim_end_matches(' '));
        if largura(&cand, tamanho, negrito) <= w {
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

fn proxima(texto: &[char], tamanho: f64, w: f64) -> (String, usize) {
    let n = prefixo(texto, tamanho, w);
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
    let mut out = Vec::new();
    for par in texto.split('\n') {
        let todos: Vec<char> = par.chars().collect();
        let mut resto: Vec<char> = strip(&todos).to_vec();
        while !resto.is_empty() {
            let (parte, consumido) = proxima(&resto, tamanho, w);
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
