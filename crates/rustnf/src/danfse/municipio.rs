// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Codigo IBGE de 7 digitos -> (nome, UF), pela tabela embutida em `dados/municipios.csv`.

use std::collections::BTreeMap;
use std::sync::OnceLock;

const TABELA: &str = include_str!("../../dados/municipios.csv");

type Tabela = BTreeMap<&'static str, (&'static str, &'static str)>;

/// codigo IBGE -> (nome como a ANATEL grava, UF)
fn tabela() -> &'static Tabela {
    static T: OnceLock<Tabela> = OnceLock::new();
    T.get_or_init(|| {
        let mut t = Tabela::new();
        for l in TABELA
            .lines()
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .skip(1)
        {
            let c: Vec<&'static str> = l.trim_end_matches('\r').split(';').collect();
            if c.len() >= 3 {
                t.insert(c[0], (c[2], c[1]));
            }
        }
        t
    })
}

/// Nome e UF do municipio de codigo IBGE `ibge`, se constar da tabela.
pub(super) fn buscar(ibge: &str) -> Option<(String, String)> {
    tabela()
        .get(ibge)
        .map(|(nome, uf)| ((*nome).to_string(), (*uf).to_string()))
}
