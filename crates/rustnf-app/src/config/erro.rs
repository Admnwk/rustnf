// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Erro do arquivo de configuracao. As mensagens nunca citam o caminho do arquivo nem o valor lido.

use std::fmt;

use crate::erro::Erro;

/// De onde veio o caminho do arquivo que nao existe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Indicado {
    /// Opcao `--config` (ou o caminho passado por quem chama).
    Opcao,
    /// Variavel de ambiente `RUSTNF_CONFIG`.
    Variavel,
}

/// Por que a configuracao nao carregou, nao validou ou nao gravou.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErroConfig {
    /// O arquivo indicado explicitamente nao existe.
    NaoEncontrado(Indicado),
    /// O arquivo existe mas nao deu para le-lo (ou passa do teto de tamanho).
    Leitura,
    /// O arquivo nao e' TOML valido, ou tem campo desconhecido ou de tipo errado.
    Sintaxe {
        /// Linha (a partir de 1), quando conhecida.
        linha: Option<usize>,
        /// O que o analisador disse, sem o trecho do arquivo.
        mensagem: String,
    },
    /// Um valor que nao serve, apontando a secao e o campo.
    Invalido {
        /// Secao (`emitente`, `sefin`, `aliquotas[0]`...).
        secao: String,
        /// Campo dentro da secao.
        campo: String,
        /// O motivo.
        motivo: String,
    },
    /// Uma variavel de ambiente `RUSTNF_*` com valor que nao serve.
    Variavel {
        /// Nome da variavel.
        nome: String,
        /// O motivo.
        motivo: String,
    },
    /// Nao deu para gravar o arquivo.
    Gravacao,
}

impl ErroConfig {
    pub(crate) fn invalido(
        secao: impl Into<String>,
        campo: impl Into<String>,
        motivo: impl Into<String>,
    ) -> Self {
        Self::Invalido {
            secao: secao.into(),
            campo: campo.into(),
            motivo: motivo.into(),
        }
    }
}

impl fmt::Display for ErroConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NaoEncontrado(Indicado::Opcao) => {
                f.write_str("configuracao: o arquivo indicado em --config nao existe")
            }
            Self::NaoEncontrado(Indicado::Variavel) => {
                f.write_str("configuracao: o arquivo indicado em RUSTNF_CONFIG nao existe")
            }
            Self::Leitura => {
                f.write_str("configuracao: nao foi possivel ler o arquivo (ou ele passa de 1 MiB)")
            }
            Self::Sintaxe {
                linha: Some(l),
                mensagem,
            } => write!(f, "configuracao: linha {l}: {mensagem}"),
            Self::Sintaxe {
                linha: None,
                mensagem,
            } => write!(f, "configuracao: {mensagem}"),
            Self::Invalido {
                secao,
                campo,
                motivo,
            } => write!(f, "configuracao: [{secao}] {campo}: {motivo}"),
            Self::Variavel { nome, motivo } => write!(f, "variavel {nome}: {motivo}"),
            Self::Gravacao => f.write_str("configuracao: nao foi possivel gravar o arquivo"),
        }
    }
}

impl std::error::Error for ErroConfig {}

impl From<ErroConfig> for Erro {
    fn from(e: ErroConfig) -> Self {
        Self::Entrada(e.to_string())
    }
}
