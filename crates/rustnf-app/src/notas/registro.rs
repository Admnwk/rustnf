// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A linha do indice de notas.

use rustnf::nfse::leitura::Nfse;
use serde::{Deserialize, Serialize};

/// Situacao da nota na pasta.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Situacao {
    /// Autorizada pela Sefin Nacional.
    Autorizada,
    /// Cancelada (evento 101101 registrado).
    Cancelada,
}

impl Situacao {
    /// O texto da situacao.
    #[must_use]
    pub fn texto(self) -> &'static str {
        match self {
            Self::Autorizada => "autorizada",
            Self::Cancelada => "cancelada",
        }
    }
}

/// Uma nota no indice (`indice.json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Registro {
    /// Chave de acesso, 50 posicoes.
    pub chave: String,
    /// Numero da NFS-e.
    pub numero: String,
    /// Serie da DPS.
    pub serie: String,
    /// Numero da DPS (nDPS).
    pub numero_dps: String,
    /// Data de emissao, `AAAA-MM-DD` (pode seguir a hora).
    pub emissao: String,
    /// Valor do servico, como a nota traz.
    pub valor_servico: String,
    /// Situacao.
    pub situacao: Situacao,
    /// CPF ou CNPJ do tomador (vazio se nao houver).
    pub tomador_documento: String,
    /// Nome do tomador.
    pub tomador_nome: String,
}

impl Registro {
    /// O registro de uma NFS-e lida da resposta da Sefin. A data de emissao e' a do processamento; se faltar, a
    /// competencia.
    #[must_use]
    pub fn da_nfse(n: &Nfse) -> Self {
        let emissao = if n.processada_em.trim().is_empty() {
            n.competencia.clone()
        } else {
            n.processada_em.clone()
        };
        Self {
            chave: n.chave.clone(),
            numero: n.numero.clone(),
            serie: n.serie_dps.clone(),
            numero_dps: n.numero_dps.clone(),
            emissao,
            valor_servico: n.valor_servico.clone(),
            situacao: Situacao::Autorizada,
            tomador_documento: n.tomador_documento.clone(),
            tomador_nome: n.tomador_nome.clone(),
        }
    }
}
