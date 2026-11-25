// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! cIndOp -- Codigo Indicador do Local da Operacao de fornecimento.
//!
//! Dado: `dados/indop.json` (Anexo VII RTC da NFS-e Nacional v1.03.00), embutido com `include_str!`. A tabela diz
//! em que documento cada codigo vale; a DPS usa a coluna da NFS-e.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::Deserialize;

const DADOS: &str = include_str!("../../dados/indop.json");

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct IndOp {
    pub codigo: String,
    pub tipo_operacao: String,
    pub caracteristica: String,
    pub local: String,
    pub dispositivo: String,
    pub observacao: String,
    pub ind_nfe: bool,
    pub ind_nfse: bool,
    pub ind_nfse_via: bool,
    pub nfse_loc_incid_ibs: String,
}

#[derive(Deserialize)]
struct Bruto {
    versao: String,
    codigos: Vec<IndOp>,
}

fn dados() -> &'static (String, BTreeMap<String, IndOp>) {
    static D: OnceLock<(String, BTreeMap<String, IndOp>)> = OnceLock::new();
    D.get_or_init(|| {
        let b: Bruto = serde_json::from_str(DADOS).expect("indop.json embutido invalido");
        (
            b.versao,
            b.codigos
                .into_iter()
                .map(|c| (c.codigo.clone(), c))
                .collect(),
        )
    })
}

/// A tabela inteira, por codigo.
#[must_use]
pub fn tabela() -> &'static BTreeMap<String, IndOp> {
    &dados().1
}

/// Versao do Anexo VII embutido (ex.: "1.03.00").
#[must_use]
pub fn versao() -> &'static str {
    &dados().0
}

#[must_use]
pub fn registro(codigo: &str) -> Option<&'static IndOp> {
    tabela().get(codigo.trim())
}

/// O codigo existe e vale no documento (`55`/`nfe`, `nfse`, `nfsvia`); NFC-e (65) nunca leva cIndOp.
/// Documento desconhecido = `Err`.
pub fn permitido(codigo: &str, documento: &str) -> Result<bool, String> {
    let d = documento.trim().to_ascii_lowercase();
    let r = registro(codigo);
    match d.as_str() {
        "55" | "nfe" => Ok(r.is_some_and(|x| x.ind_nfe)),
        "nfse" => Ok(r.is_some_and(|x| x.ind_nfse)),
        "nfsvia" | "nfse_via" => Ok(r.is_some_and(|x| x.ind_nfse_via)),
        "65" | "nfce" => Ok(false),
        _ => Err(format!(
            "documento {documento:?}: use 55/nfe, 65/nfce, nfse ou nfsvia"
        )),
    }
}

/// `cIndOp` que exigem o grupo `retirada` na NF-e (regra B25d-30).
pub const EXIGE_RETIRADA_NFE: [&str; 2] = ["010104", "010105"];

/// Regras B25d-10, B25d-20 e B25d-30 da NF-e (NT 2025.002): `Some((cStat, mensagem))` da rejeicao, ou `None` se passa.
/// Codigo vazio = nota sem `cIndOp`.
#[must_use]
pub fn erro_nfe(codigo: &str, modelo: i64, tem_retirada: bool) -> Option<(&'static str, String)> {
    let c = codigo.trim();
    if c.is_empty() {
        return None;
    }
    if modelo == 65 {
        return Some((
            "1099",
            "cIndOp nao pode ir na NFC-e (modelo 65) -- B25d-10".into(),
        ));
    }
    if !permitido(c, "nfe").unwrap_or(false) {
        let na_nfe: Vec<&str> = tabela()
            .values()
            .filter(|x| x.ind_nfe)
            .map(|x| x.codigo.as_str())
            .collect();
        return Some((
            "1102",
            format!(
                "cIndOp {c} nao permitido na NF-e (indNFe vazio no Anexo VII v{}; na NF-e so' {}) -- B25d-20",
                versao(),
                na_nfe.join(", ")
            ),
        ));
    }
    if EXIGE_RETIRADA_NFE.contains(&c) && !tem_retirada {
        return Some((
            "1110",
            format!("cIndOp {c} exige o local de retirada (grupo retirada) -- B25d-30"),
        ));
    }
    None
}
