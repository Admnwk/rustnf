// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Leitura do XML da NFS-e: `roxmltree` sem DTD, com o namespace conferido, e acesso por caminho de elementos.

use super::ErroDanfse;

pub(super) type No<'a, 'i> = roxmltree::Node<'a, 'i>;

/// Documento com DTD/entidade recusados e o namespace conferido.
pub(super) fn documento<'i>(
    texto: &'i str,
    ns: &str,
) -> Result<roxmltree::Document<'i>, ErroDanfse> {
    let texto = texto.strip_prefix('\u{feff}').unwrap_or(texto);
    let opt = roxmltree::ParsingOptions {
        allow_dtd: false,
        ..roxmltree::ParsingOptions::default()
    };
    let d = roxmltree::Document::parse_with_options(texto, opt).map_err(|e| {
        ErroDanfse::XmlIlegivel(match e {
            roxmltree::Error::DtdDetected => "XML com DTD/entidade recusado",
            _ => "XML mal formado",
        })
    })?;
    if let Some(n) = d.root_element().tag_name().namespace() {
        if n != ns {
            return Err(ErroDanfse::XmlIlegivel(
                "o XML e' de outro leiaute (namespace diferente)",
            ));
        }
    }
    Ok(d)
}

pub(super) fn filho<'a, 'i>(el: Option<No<'a, 'i>>, nome: &str) -> Option<No<'a, 'i>> {
    el?.children()
        .find(|c| c.is_element() && c.tag_name().name() == nome)
}

pub(super) fn filhos<'a, 'i>(el: Option<No<'a, 'i>>, nome: &str) -> Vec<No<'a, 'i>> {
    el.map_or_else(Vec::new, |e| {
        e.children()
            .filter(|c| c.is_element() && c.tag_name().name() == nome)
            .collect()
    })
}

pub(super) fn caminho<'a, 'i>(el: Option<No<'a, 'i>>, nomes: &[&str]) -> Option<No<'a, 'i>> {
    nomes.iter().fold(el, |e, n| filho(e, n))
}

/// Texto do elemento, aparado: so' o que vem antes do primeiro filho.
pub(super) fn texto(e: No) -> String {
    e.children()
        .take_while(|c| !c.is_element())
        .filter_map(|c| c.text())
        .collect::<String>()
        .trim()
        .to_string()
}

pub(super) fn el<'a, 'i>(r: Option<No<'a, 'i>>, c: &str) -> Option<No<'a, 'i>> {
    let nomes: Vec<&str> = c.split('/').collect();
    caminho(r, &nomes)
}

pub(super) fn g(r: Option<No>, c: &str) -> String {
    el(r, c).map_or(String::new(), texto)
}

pub(super) fn int(r: Option<No>, c: &str) -> i64 {
    g(r, c).parse().unwrap_or(0)
}
