// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Codigo de barras CODE-128 da chave de acesso e dos dados da NF-e em contingencia (MOC 7.00, Anexo II, item 2 e
//! Anexo III.01).
//!
//! O Anexo II manda o conjunto C (dois algarismos por simbolo): a chave de 44 posicoes sao 22 simbolos, mais o
//! Start C, o digito verificador do modulo 103 e o Stop. Quando a chave leva letra (CNPJ alfanumerico) o conjunto C
//! nao a representa, e o codigo troca para o conjunto B (ASCII 32 a 126) so' onde precisa. A saida e' o texto das
//! larguras em modulos, barra e espaco alternados, comecando por barra: e' o que a tabela do Anexo III.01 traz
//! para cada simbolo.

use std::fmt;

/// Texto que o CODE-128 nao representa (a barra errada nunca e' desenhada em silencio) ou barras que nao decodificam.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErroBarras(pub String);

impl fmt::Display for ErroBarras {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ErroBarras {}

fn erro<T>(motivo: impl Into<String>) -> Result<T, ErroBarras> {
    Err(ErroBarras(motivo.into()))
}

/// Larguras de barra e espaco de cada um dos 107 simbolos (ISO/IEC 15417; Anexo III.01 do Anexo II). O ultimo e' o
/// Stop, com 7 larguras.
const SIMBOLOS: [&str; 107] = [
    "212222", "222122", "222221", "121223", "121322", "131222", "122213", "122312", "132212",
    "221213", "221312", "231212", "112232", "122132", "122231", "113222", "123122", "123221",
    "223211", "221132", "221231", "213212", "223112", "312131", "311222", "321122", "321221",
    "312212", "322112", "322211", "212123", "212321", "232121", "111323", "131123", "131321",
    "112313", "132113", "132311", "211313", "231113", "231311", "112133", "112331", "132131",
    "113123", "113321", "133121", "313121", "211331", "231131", "213113", "213311", "213131",
    "311123", "311321", "331121", "312113", "312311", "332111", "314111", "221411", "431111",
    "111224", "111422", "121124", "121421", "141122", "141221", "112214", "112412", "122114",
    "122411", "142112", "142211", "241211", "221114", "413111", "241112", "134111", "111242",
    "121142", "121241", "114212", "124112", "124211", "411212", "421112", "421211", "212141",
    "214121", "412121", "111143", "111341", "131141", "114113", "114311", "411113", "411311",
    "113141", "114131", "311141", "411131", "211412", "211214", "211232", "2331112",
];

const START_B: u8 = 104;
const START_C: u8 = 105;
const TROCA_B: u8 = 100;
const TROCA_C: u8 = 99;
const STOP: u8 = 106;

/// Quantos algarismos seguidos ha a partir de `i`.
fn algarismos(s: &[u8], i: usize) -> usize {
    s[i..].iter().take_while(|c| c.is_ascii_digit()).count()
}

/// Start + dados + digito verificador + Stop. Comeca em C com 4 ou mais algarismos (ou texto todo em pares) e, no
/// meio, vai para C com 6 ou mais algarismos (ou 4 ou mais em par fechando o texto); fora disso fica em B.
///
/// # Errors
///
/// [`ErroBarras`] se o texto esta vazio ou tem caractere fora do conjunto B (ASCII 32 a 126).
pub fn simbolos(texto: &str) -> Result<Vec<u8>, ErroBarras> {
    let t = texto.trim();
    if t.is_empty() {
        return erro("CODE-128 sem conteudo");
    }
    if let Some(c) = t.chars().find(|c| !(' '..='~').contains(c)) {
        return erro(format!("o CODE-128 B nao codifica o caractere {c:?}"));
    }
    let b = t.as_bytes();
    let inicio = algarismos(b, 0);
    let mut em_c = inicio >= 4 || (inicio == b.len() && inicio.is_multiple_of(2));
    let mut sim: Vec<u8> = vec![if em_c { START_C } else { START_B }];
    let mut i = 0;
    while i < b.len() {
        let n = algarismos(b, i);
        if em_c {
            if n >= 2 {
                for k in (i..i + n / 2 * 2).step_by(2) {
                    sim.push((b[k] - b'0') * 10 + (b[k + 1] - b'0'));
                }
                i += n / 2 * 2;
                continue;
            }
            sim.push(TROCA_B);
            em_c = false;
            continue;
        }
        if n >= 6 || (n >= 4 && n.is_multiple_of(2) && i + n == b.len()) {
            sim.push(TROCA_C);
            em_c = true;
            continue;
        }
        sim.push(b[i] - 32);
        i += 1;
    }
    sim.push(verificador(&sim));
    sim.push(STOP);
    Ok(sim)
}

/// Digito verificador: soma ponderada pela posicao (o Start pesa 1) modulo 103 (Anexo II, item 2.1).
fn verificador(sim: &[u8]) -> u8 {
    let soma: usize = usize::from(sim[0])
        + sim[1..]
            .iter()
            .enumerate()
            .map(|(k, v)| (k + 1) * usize::from(*v))
            .sum::<usize>();
    u8::try_from(soma % 103).unwrap_or(0)
}

/// As larguras em modulos de barra e espaco alternados, comecando por barra.
///
/// # Errors
///
/// [`ErroBarras`] como em [`simbolos`].
pub fn padrao(texto: &str) -> Result<String, ErroBarras> {
    Ok(simbolos(texto)?
        .into_iter()
        .map(|s| SIMBOLOS[usize::from(s)])
        .collect())
}

/// Total de modulos de um texto de larguras (sem a margem clara).
#[must_use]
pub fn modulos(padrao: &str) -> usize {
    padrao.bytes().map(|c| usize::from(c - b'0')).sum()
}

/// Le o texto de volta das larguras: separa os simbolos, confere o Start, o Stop e o digito verificador e desfaz as
/// trocas entre os conjuntos B e C.
///
/// # Errors
///
/// [`ErroBarras`] se as larguras nao formam simbolos do CODE-128 ou o digito verificador nao fecha.
pub fn decodificar(padrao: &str) -> Result<String, ErroBarras> {
    let mut sim: Vec<u8> = Vec::new();
    let mut resto = padrao;
    while !resto.is_empty() {
        let achado = SIMBOLOS
            .iter()
            .enumerate()
            .find(|(_, s)| resto.starts_with(**s))
            .map(|(i, s)| (i, s.len()));
        let Some((i, n)) = achado else {
            return erro("larguras que nao sao simbolo do CODE-128");
        };
        sim.push(i as u8);
        resto = &resto[n..];
    }
    if sim.len() < 4 || sim.last() != Some(&STOP) {
        return erro("falta o Stop");
    }
    sim.pop();
    let Some(dv) = sim.pop() else {
        return erro("falta o digito verificador");
    };
    if verificador(&sim) != dv {
        return erro("o digito verificador nao fecha");
    }
    let mut em_c = match sim[0] {
        START_C => true,
        START_B => false,
        _ => return erro("falta o Start"),
    };
    let mut texto = String::new();
    for s in &sim[1..] {
        match (*s, em_c) {
            (TROCA_B, true) => em_c = false,
            (TROCA_C, false) => em_c = true,
            (v, true) if v < 100 => texto.push_str(&format!("{v:02}")),
            (v, false) if v < 95 => texto.push(char::from(v + 32)),
            _ => return erro("simbolo fora do conjunto"),
        }
    }
    Ok(texto)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod testes {
    use super::*;

    #[test]
    fn exemplo_do_anexo_ii_item_2_1() {
        // chave de oito caracteres 09758364: Start C, 09 75 83 64, DV 48 (769 modulo 103), Stop
        assert_eq!(
            simbolos("09758364").unwrap(),
            vec![105, 9, 75, 83, 64, 48, 106]
        );
    }

    #[test]
    fn chave_de_44_posicoes_tem_277_modulos() {
        let p = padrao(&"1".repeat(44)).unwrap();
        assert_eq!(modulos(&p), 11 * 22 + 11 + 11 + 13);
    }

    #[test]
    fn ida_e_volta_com_troca_de_conjunto() {
        for texto in [
            "35261011222333000181550010000000011234567012",
            "AB12CD3456",
            "A",
            "12",
        ] {
            assert_eq!(
                decodificar(&padrao(texto).unwrap()).unwrap(),
                texto,
                "{texto}"
            );
        }
    }

    #[test]
    fn recusa_o_que_nao_representa() {
        assert!(simbolos("").is_err());
        assert!(simbolos("n\u{e3}o").is_err());
    }
}
