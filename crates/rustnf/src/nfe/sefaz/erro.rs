// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Erro de USO do cliente da SEFAZ: o pedido que nao sai da maquina.

use std::fmt;

/// Pedido recusado ANTES de chegar a rede (UF desconhecida, chave invalida, NF-e sem assinatura, assincrono depois do
/// corte da NT 2023.002...). A mensagem e' para a tela; `campo` e' a tag ou o parametro de que ela fala.
///
/// Falha de rede e resposta ruim NAO sao erro: voltam em [`Resultado`](super::Resultado), porque a nota pode ter sido
/// recebida.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErroSefaz {
    /// A tag do leiaute ou o parametro.
    pub campo: String,
    /// O que esta errado, em palavras.
    pub mensagem: String,
}

impl ErroSefaz {
    pub(crate) fn novo(mensagem: impl Into<String>, campo: &str) -> Self {
        ErroSefaz {
            campo: campo.into(),
            mensagem: mensagem.into(),
        }
    }
}

impl fmt::Display for ErroSefaz {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.mensagem)
    }
}

impl std::error::Error for ErroSefaz {}
