// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Entrada segura do XML da NF-e: bytes -> texto (codificacao declarada), documento sem DTD e os acessos por
//! elemento filho direto, com o valor ja no tipo do leiaute.
//!
//! O `roxmltree` recusa qualquer `<!DOCTYPE` por padrao (`ParsingOptions::allow_dtd` falso;
//! <https://docs.rs/roxmltree/0.21.2/roxmltree/struct.ParsingOptions.html>), vazio ou nao, entao nao ha entidade
//! externa nem expansao de entidade. A busca de elemento e' sempre por filho DIRETO, como o caminho do XSD.

use chrono::{FixedOffset, NaiveDate, NaiveDateTime};

use super::decimal::{DataHora, Decimal};
use super::erro::{ErroLeitura, TipoCampo};

/// Espaco de nomes do leiaute da NF-e.
pub const NS: &str = "http://www.portalfiscal.inf.br/nfe";

/// Tamanho maximo aceito. Uma NF-e de 990 itens tem de 1 a 2 MB; este limite corta o arquivo absurdo.
pub const LIMITE_BYTES: usize = 20 * 1024 * 1024;

pub(super) type R<T> = Result<T, ErroLeitura>;
pub(super) type No<'a, 'i> = roxmltree::Node<'a, 'i>;

/// Os primeiros `n` caracteres de `s`.
pub(super) fn corta(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

// ---------------------------------------------------------------- bytes -> texto

fn codificacao(nome: &str) -> Option<&'static str> {
    Some(match nome {
        "utf-8" | "utf8" => "utf-8",
        "iso-8859-1" | "iso8859-1" | "latin1" | "latin-1" => "latin-1",
        "windows-1252" | "cp1252" => "cp1252",
        "us-ascii" | "ascii" => "ascii",
        _ => return None,
    })
}

/// O valor de `encoding=` no prologo `<?xml ...?>`, se houver.
fn declaracao_de_codificacao(b: &[u8]) -> Option<String> {
    let inicio = b
        .iter()
        .position(|c| !matches!(c, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c))?;
    let resto = &b[inicio..];
    if !resto.starts_with(b"<?xml") {
        return None;
    }
    let fim = resto.iter().position(|&c| c == b'>').unwrap_or(resto.len());
    let cabecalho = &resto[5..fim];
    let mut k = 0;
    while let Some(p) = cabecalho[k..].windows(8).position(|w| w == b"encoding") {
        if let Some(nome) = valor_depois_do_igual(&cabecalho[k + p + 8..]) {
            return Some(nome);
        }
        k += p + 1;
    }
    None
}

/// `  = "nome"` -> `nome` (letras, digitos, `.`, `_` e `-`, entre aspas).
fn valor_depois_do_igual(b: &[u8]) -> Option<String> {
    let b = b.trim_ascii_start();
    let b = b.strip_prefix(b"=")?.trim_ascii_start();
    let aspas = *b.first().filter(|c| matches!(c, b'"' | b'\''))?;
    let corpo = &b[1..];
    let tam = corpo
        .iter()
        .take_while(|c| c.is_ascii_alphanumeric() || b"._-".contains(c))
        .count();
    (tam > 0 && corpo.get(tam) == Some(&aspas))
        .then(|| String::from_utf8_lossy(&corpo[..tam]).to_string())
}

/// Windows-1252, 0x80 a 0x9F (as cinco posicoes que a tabela deixa indefinidas -- 81, 8D, 8F, 90 e 9D -- sao erro).
const CP1252: [u16; 32] = [
    0x20AC, 0, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0160, 0x2039,
    0x0152, 0, 0x017D, 0, 0, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014, 0x02DC,
    0x2122, 0x0161, 0x203A, 0x0153, 0, 0x017E, 0x0178,
];

fn decodificar(b: &[u8], cod: &str) -> Option<String> {
    match cod {
        "utf-8" => String::from_utf8(b.to_vec()).ok(),
        "latin-1" => Some(b.iter().map(|&c| char::from(c)).collect()),
        "ascii" => b.is_ascii().then(|| String::from_utf8_lossy(b).to_string()),
        "cp1252" => b
            .iter()
            .map(|&c| match c {
                0x80..=0x9F => match CP1252[usize::from(c - 0x80)] {
                    0 => None,
                    u => char::from_u32(u32::from(u)),
                },
                _ => Some(char::from(c)),
            })
            .collect(),
        _ => None,
    }
}

fn utf16(b: &[u8]) -> Option<String> {
    let little = b.starts_with(b"\xff\xfe");
    let corpo = b.get(2..)?;
    if corpo.len() % 2 != 0 {
        return None;
    }
    let (pares, _) = corpo.as_chunks::<2>();
    let unidades: Vec<u16> = pares
        .iter()
        .map(|&c| {
            if little {
                u16::from_le_bytes(c)
            } else {
                u16::from_be_bytes(c)
            }
        })
        .collect();
    char::decode_utf16(unidades)
        .collect::<Result<String, _>>()
        .ok()
}

/// Tira o prologo `<?xml ... ?>` (o texto ja esta decodificado; o `encoding=` nao vale mais).
fn sem_prologo(t: &str) -> String {
    let r = t.trim_start();
    if let Some(resto) = r.strip_prefix("<?xml") {
        if let Some(p) = resto.find('>') {
            if p > 0 && resto.as_bytes()[p - 1] == b'?' {
                return resto[p + 1..].to_string();
            }
        }
    }
    t.to_string()
}

/// Bytes do arquivo -> texto XML: UTF-8 (com ou sem BOM), UTF-16 com BOM, ISO-8859-1 e Windows-1252 declarados.
pub(super) fn texto_xml(dados: &[u8]) -> R<String> {
    if dados.len() > LIMITE_BYTES {
        return Err(ErroLeitura::Grande);
    }
    let texto = if let Some(r) = dados.strip_prefix(b"\xef\xbb\xbf") {
        decodificar(r, "utf-8")
    } else if dados.starts_with(b"\xff\xfe") || dados.starts_with(b"\xfe\xff") {
        utf16(dados)
    } else {
        let nome = declaracao_de_codificacao(dados)
            .map_or_else(|| "utf-8".to_string(), |n| n.to_lowercase());
        let Some(cod) = codificacao(&nome) else {
            return Err(ErroLeitura::Codificacao(corta(&nome, 20)));
        };
        decodificar(dados, cod)
    };
    let Some(texto) = texto else {
        return Err(ErroLeitura::CodificacaoIncorreta);
    };
    let texto = texto.strip_prefix('\u{feff}').unwrap_or(&texto);
    if texto.chars().count() > LIMITE_BYTES {
        return Err(ErroLeitura::Grande);
    }
    Ok(sem_prologo(texto))
}

/// Texto -> documento, sem DTD, e do espaco de nomes da NF-e (ou sem espaco de nomes).
pub(super) fn abrir(texto: &str) -> R<roxmltree::Document<'_>> {
    let opcoes = roxmltree::ParsingOptions {
        allow_dtd: false,
        ..roxmltree::ParsingOptions::default()
    };
    let d = roxmltree::Document::parse_with_options(texto, opcoes).map_err(|e| match e {
        roxmltree::Error::DtdDetected => ErroLeitura::Dtd,
        _ => ErroLeitura::MalFormado,
    })?;
    if !matches!(d.root_element().tag_name().namespace(), None | Some(NS)) {
        return Err(ErroLeitura::OutroLeiaute);
    }
    Ok(d)
}

// ---------------------------------------------------------------- elementos

fn do_leiaute(n: &No) -> bool {
    n.is_element() && matches!(n.tag_name().namespace(), None | Some(NS))
}

/// Nome local do elemento.
pub(super) fn nome<'a>(n: &No<'a, '_>) -> &'a str {
    n.tag_name().name()
}

/// Filhos diretos (todos, ou so' os de um nome).
pub(super) fn filhos<'a, 'i>(n: Option<No<'a, 'i>>, so: Option<&str>) -> Vec<No<'a, 'i>> {
    n.map(|n| {
        n.children()
            .filter(|c| do_leiaute(c) && so.is_none_or(|x| nome(c) == x))
            .collect()
    })
    .unwrap_or_default()
}

/// Filhos diretos de um nome.
pub(super) fn todos<'a, 'i>(n: No<'a, 'i>, so: &str) -> Vec<No<'a, 'i>> {
    filhos(Some(n), Some(so))
}

/// O primeiro filho direto de um nome.
pub(super) fn filho<'a, 'i>(n: Option<No<'a, 'i>>, so: &str) -> Option<No<'a, 'i>> {
    n?.children().find(|c| do_leiaute(c) && nome(c) == so)
}

/// O elemento tem filho elemento (e' um grupo, nao uma folha).
pub(super) fn tem_filhos(n: No) -> bool {
    n.children().any(|c| do_leiaute(&c))
}

/// Texto do elemento, sem espacos nas pontas.
pub(super) fn texto_de(n: No) -> String {
    n.children()
        .filter(|c| c.is_text())
        .filter_map(|c| c.text())
        .collect::<String>()
        .trim()
        .to_string()
}

/// Texto do filho `so` (vazio se nao existe).
pub(super) fn t(n: Option<No>, so: &str) -> String {
    filho(n, so).map(texto_de).unwrap_or_default()
}

fn onde(n: Option<No>, campo: &str) -> String {
    n.map_or_else(|| campo.to_string(), |n| format!("{}/{campo}", nome(&n)))
}

fn invalido(tipo: TipoCampo, n: Option<No>, campo: &str, lido: &str) -> ErroLeitura {
    ErroLeitura::Campo {
        tipo,
        onde: onde(n, campo),
        lido: corta(lido, 30),
    }
}

// ---------------------------------------------------------------- valores tipados

/// Decimal do filho `campo`; `None` se nao ha o campo.
pub(super) fn decimal(n: Option<No>, campo: &str) -> R<Option<Decimal>> {
    let s = t(n, campo);
    if s.is_empty() {
        return Ok(None);
    }
    Decimal::ler(&s)
        .map(Some)
        .ok_or_else(|| invalido(TipoCampo::Decimal, n, campo, &s))
}

/// Inteiro sem sinal do filho `campo`.
pub(super) fn inteiro(n: Option<No>, campo: &str) -> R<Option<u64>> {
    let s = t(n, campo);
    if s.is_empty() {
        return Ok(None);
    }
    match (s.bytes().all(|c| c.is_ascii_digit()), s.parse::<u64>()) {
        (true, Ok(v)) => Ok(Some(v)),
        _ => Err(invalido(TipoCampo::Inteiro, n, campo, &s)),
    }
}

/// `AAAA-MM-DD` (so' o formato do leiaute, sem outro separador).
pub(super) fn data(n: Option<No>, campo: &str) -> R<Option<NaiveDate>> {
    let s = t(n, campo);
    if s.is_empty() {
        return Ok(None);
    }
    let b = s.as_bytes();
    let forma = b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b.iter()
            .enumerate()
            .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit());
    match (forma, NaiveDate::parse_from_str(&s, "%Y-%m-%d")) {
        (true, Ok(d)) => Ok(Some(d)),
        _ => Err(invalido(TipoCampo::Data, n, campo, &s)),
    }
}

/// O fuso do fim de `AAAA-MM-DDThh:mm:ss[.f]`: `Z` ou `+hh:mm` / `-hh:mm`, em segundos.
fn fuso_em_segundos(resto: &str) -> Option<i32> {
    if resto == "Z" {
        return Some(0);
    }
    let b = resto.as_bytes();
    let forma = b.len() == 6
        && matches!(b[0], b'+' | b'-')
        && b[3] == b':'
        && b[1..3].iter().chain(&b[4..6]).all(u8::is_ascii_digit);
    if !forma {
        return None;
    }
    let h: i32 = resto[1..3].parse().ok()?;
    let m: i32 = resto[4..6].parse().ok()?;
    if h > 23 || m > 59 {
        return None;
    }
    let seg = h * 3600 + m * 60;
    Some(if b[0] == b'-' { -seg } else { seg })
}

/// `AAAA-MM-DDThh:mm:ss[.f](Z|+hh:mm)`. Sem fuso e' erro: o leiaute exige.
pub(super) fn data_hora(n: Option<No>, campo: &str) -> R<Option<DataHora>> {
    let s = t(n, campo);
    if s.is_empty() {
        return Ok(None);
    }
    let ruim = || invalido(TipoCampo::DataHora, n, campo, &s);
    let b = s.as_bytes();
    let forma = s.is_ascii()
        && b.len() >= 20
        && b[4] == b'-'
        && b[7] == b'-'
        && b[10] == b'T'
        && b[13] == b':'
        && b[16] == b':'
        && b[..19]
            .iter()
            .enumerate()
            .all(|(i, c)| [4, 7, 10, 13, 16].contains(&i) || c.is_ascii_digit());
    if !forma {
        return Err(ruim());
    }
    let mut resto = &s[19..];
    if let Some(r) = resto.strip_prefix('.') {
        let casas = r.bytes().take_while(u8::is_ascii_digit).count();
        if casas == 0 {
            return Err(ruim());
        }
        resto = &r[casas..];
    }
    let segundos = fuso_em_segundos(resto).ok_or_else(ruim)?;
    let local = NaiveDateTime::parse_from_str(&s[..19], "%Y-%m-%dT%H:%M:%S").map_err(|_| ruim())?;
    if &s[17..19] > "59" {
        return Err(ruim());
    }
    let fuso = FixedOffset::east_opt(segundos).ok_or_else(ruim)?;
    let dh = local.and_local_timezone(fuso).single().ok_or_else(ruim)?;
    Ok(Some(DataHora(dh)))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod testes {
    use super::*;

    #[test]
    fn dtd_e_recusado_mesmo_vazio() {
        for doctype in ["<!DOCTYPE NFe>", "<!DOCTYPE NFe []>"] {
            let xml = format!("{doctype}<NFe/>");
            assert_eq!(abrir(&xml).unwrap_err(), ErroLeitura::Dtd, "{doctype}");
        }
        assert!(abrir("<NFe/>").is_ok());
    }

    #[test]
    fn codificacoes() {
        let latin1 = b"<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?><a>Ca\xe7ador</a>";
        assert!(texto_xml(latin1).unwrap().contains("Ca\u{e7}ador"));
        let cp = b"<?xml version=\"1.0\" encoding=\"windows-1252\"?><a>\x80 \x93x\x94</a>";
        assert!(texto_xml(cp).unwrap().contains('\u{20ac}'));
        let indefinido = b"<?xml version=\"1.0\" encoding=\"windows-1252\"?><a>\x81</a>";
        assert_eq!(
            texto_xml(indefinido).unwrap_err(),
            ErroLeitura::CodificacaoIncorreta
        );
        let outra = b"<?xml version=\"1.0\" encoding=\"koi8-r\"?><a/>";
        assert_eq!(
            texto_xml(outra).unwrap_err(),
            ErroLeitura::Codificacao("koi8-r".into())
        );
        let mut u16le = vec![0xff, 0xfe];
        for u in "<a>\u{e7}</a>".encode_utf16() {
            u16le.extend_from_slice(&u.to_le_bytes());
        }
        assert_eq!(texto_xml(&u16le).unwrap(), "<a>\u{e7}</a>");
        let mut bom = vec![0xef, 0xbb, 0xbf];
        bom.extend_from_slice(b"<a/>");
        assert_eq!(texto_xml(&bom).unwrap(), "<a/>");
        assert_eq!(
            texto_xml(b"<a>\xff</a>").unwrap_err(),
            ErroLeitura::CodificacaoIncorreta
        );
    }
}
