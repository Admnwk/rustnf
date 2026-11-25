// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Erro da leitura do XML da NF-e.

use std::fmt;

/// Que tipo de valor o campo devia ter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum TipoCampo {
    /// Decimal do leiaute (`.` como separador, sem sinal).
    Decimal,
    /// Inteiro sem sinal.
    Inteiro,
    /// Data `AAAA-MM-DD`.
    Data,
    /// Data e hora `AAAA-MM-DDThh:mm:ss` com fuso (`Z` ou `+hh:mm`).
    DataHora,
}

impl fmt::Display for TipoCampo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Decimal => "valor",
            Self::Inteiro => "numero",
            Self::Data => "data",
            Self::DataHora => "data/hora (com fuso)",
        })
    }
}

/// Por que o XML nao virou uma [`Nfe`](super::Nfe).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErroLeitura {
    /// Passa do limite de tamanho (`LIMITE_BYTES`).
    Grande,
    /// `encoding=` do prologo que a leitura nao entende (aceita UTF-8, UTF-16, ISO-8859-1 e Windows-1252).
    Codificacao(String),
    /// Os bytes nao estao na codificacao que o arquivo declara.
    CodificacaoIncorreta,
    /// O XML traz DTD ou entidade (a NF-e nao tem; recusado para evitar expansao de entidade).
    Dtd,
    /// XML mal formado.
    MalFormado,
    /// O elemento raiz e' de outro espaco de nomes que nao o da NF-e.
    OutroLeiaute,
    /// O XML e' de outro documento (`raiz` e' o nome do elemento raiz, cortado em 40 caracteres).
    OutroDocumento {
        /// Nome do elemento raiz.
        raiz: String,
    },
    /// Falta um grupo obrigatorio.
    GrupoAusente(&'static str),
    /// `infNFe` sem o atributo `Id` ou `versao`.
    SemIdOuVersao,
    /// Versao do leiaute que a leitura nao cobre (cobre 3.10 e 4.00).
    Leiaute(String),
    /// Modelo diferente de 55 (a NFC-e, modelo 65, ainda nao e' lida).
    Modelo(String),
    /// `ide` sem `serie` ou `nNF`.
    IdeIncompleto,
    /// O item `n` (posicao no XML) nao tem o grupo `prod`.
    ItemSemProd(u64),
    /// Campo com conteudo fora do formato do leiaute.
    Campo {
        /// Que tipo de valor era esperado.
        tipo: TipoCampo,
        /// Onde: `grupo/campo`.
        onde: String,
        /// O que veio, cortado em 30 caracteres.
        lido: String,
    },
}

impl fmt::Display for ErroLeitura {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Grande => f.write_str("arquivo grande demais para ser uma NF-e"),
            Self::Codificacao(c) => write!(
                f,
                "codificacao {c} nao suportada (UTF-8, UTF-16, ISO-8859-1, Windows-1252)"
            ),
            Self::CodificacaoIncorreta => {
                f.write_str("o arquivo nao esta na codificacao que declara")
            }
            Self::Dtd => f.write_str("XML com DTD/entidade recusado (NF-e nao tem DTD)"),
            Self::MalFormado => f.write_str("XML mal formado"),
            Self::OutroLeiaute => {
                f.write_str("o XML nao e do leiaute da NF-e (espaco de nomes de outro documento)")
            }
            Self::OutroDocumento { raiz } => {
                write!(f, "o XML nao e uma NF-e: elemento raiz {raiz}")
            }
            Self::GrupoAusente(g) => write!(f, "NF-e sem o grupo {g}"),
            Self::SemIdOuVersao => f.write_str("infNFe sem o atributo Id ou versao"),
            Self::Leiaute(v) => write!(f, "leiaute {v} nao suportado (3.10 e 4.00)"),
            Self::Modelo(m) => write!(f, "modelo {m} nao e NF-e (55)"),
            Self::IdeIncompleto => f.write_str("ide sem serie ou numero"),
            Self::ItemSemProd(n) => write!(f, "item {n} sem o grupo prod"),
            Self::Campo { tipo, onde, lido } => write!(f, "{tipo} invalido em {onde}: '{lido}'"),
        }
    }
}

impl std::error::Error for ErroLeitura {}
