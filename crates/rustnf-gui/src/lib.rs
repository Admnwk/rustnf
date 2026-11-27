// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Interface de desktop do `rustnf`, em GPUI.
//!
//! * [`modelo`] -- estado dos formularios, mascaras, conferencia campo a campo, estado do certificado e passos do
//!   primeiro uso, sem janela;
//! * [`servicos`] -- carregar e gravar a configuracao, abrir o certificado, cofre de senhas e teste de conexao, sem
//!   janela;
//! * [`fora`] -- o runtime proprio para a rede;
//! * `demonstracao` -- o emissor sem rede, so' com a feature `demonstracao` (e nos testes);
//! * `telas` -- as janelas, so' no Windows (a versao atual do GPUI usada aqui so' tem janela no Windows).
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

#[cfg(any(test, feature = "demonstracao"))]
pub mod demonstracao;
#[cfg(any(test, feature = "demonstracao"))]
pub mod demonstracao_eventos;
#[cfg(any(test, feature = "demonstracao"))]
pub mod demonstracao_nfce;
#[cfg(any(test, feature = "demonstracao"))]
pub mod demonstracao_nfe;
#[cfg(any(test, feature = "demonstracao"))]
pub mod demonstracao_recebidas;
pub mod fora;
pub mod modelo;
pub mod servicos;
#[cfg(windows)]
pub mod telas;

#[cfg(test)]
mod testes_comuns;
