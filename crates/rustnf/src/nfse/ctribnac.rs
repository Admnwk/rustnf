// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Lista Nacional de Servicos (cTribNac): `dados/ctribnac.json` (338 codigos do Anexo B oficial v1.01-20260122,
//! portal da NFS-e, <https://www.gov.br/nfse>), embutido com `include_str!`.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::Deserialize;

use super::base::{ErroNfse, R};

pub const VIGENCIA: &str = "2025-11-24";
const DADOS: &str = include_str!("../../dados/ctribnac.json");
const CAMPO: &str = "infDPS/serv/cServ/cTribNac";

#[derive(Debug, Clone, Deserialize)]
pub struct Codigo {
    pub ctribnac: String,
    pub item_lc116: String,
    pub descricao: String,
}

#[derive(Deserialize)]
struct Arquivo {
    codigos: Vec<Codigo>,
}

fn lista() -> &'static (Vec<Codigo>, BTreeMap<String, usize>) {
    static L: OnceLock<(Vec<Codigo>, BTreeMap<String, usize>)> = OnceLock::new();
    L.get_or_init(|| {
        let a: Arquivo =
            serde_json::from_str(DADOS).expect("dados/ctribnac.json embutido e' valido");
        let idx = a
            .codigos
            .iter()
            .enumerate()
            .map(|(i, c)| (c.ctribnac.clone(), i))
            .collect();
        (a.codigos, idx)
    })
}

/// Todos, na ordem oficial (seletor da tela).
pub fn codigos() -> &'static [Codigo] {
    &lista().0
}

pub fn achar(codigo: &str) -> Option<&'static Codigo> {
    let (v, idx) = lista();
    idx.get(codigo).map(|&i| &v[i])
}

pub fn existe(codigo: &str) -> bool {
    achar(codigo).is_some()
}

pub fn descricao(codigo: &str) -> &'static str {
    achar(codigo).map_or("", |c| c.descricao.as_str())
}

pub fn desdobros(item4: &str) -> Vec<&'static str> {
    let p: String = item4.chars().filter(char::is_ascii_digit).collect();
    if p.len() != 4 {
        return Vec::new();
    }
    codigos()
        .iter()
        .filter(|c| c.ctribnac.starts_with(&p))
        .map(|c| c.ctribnac.as_str())
        .collect()
}

fn sem_acento(s: &str) -> String {
    use unicode_normalization::char::canonical_combining_class;
    use unicode_normalization::UnicodeNormalization;
    s.to_lowercase()
        .nfkd()
        .filter(|c| canonical_combining_class(*c) == 0)
        .collect()
}

/// Codigos cuja descricao contem todas as palavras do termo (sem acento, sem caixa).
pub fn buscar(termo: &str, limite: usize) -> Vec<&'static Codigo> {
    let palavras: Vec<String> = sem_acento(termo)
        .split_whitespace()
        .map(String::from)
        .collect();
    codigos()
        .iter()
        .filter(|c| {
            let d = sem_acento(&c.descricao);
            palavras
                .iter()
                .all(|p| d.contains(p.as_str()) || c.ctribnac.contains(p.as_str()))
        })
        .take(limite)
        .collect()
}

/// cTribNac de 6 digitos conferido contra a lista ('14.01.01', '140101' ou o item '14.01' com desdobro unico).
/// `estrito = false` deixa passar 6 digitos fora da lista (lista nova ainda nao embarcada).
pub fn normalizar(raw: &str, estrito: bool) -> R<String> {
    let d: String = raw.chars().filter(char::is_ascii_digit).collect();
    if d.len() == 4 {
        let op = desdobros(&d);
        return match op.len() {
            1 => Ok(op[0].to_string()),
            0 => Err(ErroNfse::novo(CAMPO, format!(
                "o item LC 116 {}.{} nao tem codigo na Lista Nacional (vigente desde 24/11/2025); confirme com o \
                 contador", &d[..2], &d[2..]))),
            n => Err(ErroNfse::novo(CAMPO, format!(
                "o item {}.{} tem {n} desdobros na lista oficial ({}): informe os 6 digitos -- escolher por voce \
                 seria chute fiscal", &d[..2], &d[2..], op.join(", ")))),
        };
    }
    if d.len() != 6 {
        return Err(ErroNfse::novo(
            CAMPO,
            format!("{raw:?} nao e' um codigo de 6 digitos (item + subitem + desdobro da LC 116)"),
        ));
    }
    if d.ends_with("00") {
        let op = desdobros(&d[..4]);
        let dica = if op.is_empty() {
            String::new()
        } else {
            format!(
                " Codigos do item {}.{}: {}.",
                &d[..2],
                &d[2..4],
                op.join(", ")
            )
        };
        return Err(ErroNfse::novo(CAMPO, format!(
            "{d} nao existe: nenhum codigo da Lista Nacional termina em 00 (o desdobro comeca em 01).{dica}")));
    }
    if estrito && !existe(&d) {
        return Err(ErroNfse::novo(CAMPO, format!(
            "{d} nao esta na Lista Nacional de Servicos (vigente desde 24/11/2025); confirme o codigo com o contador")));
    }
    Ok(d)
}
