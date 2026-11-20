// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! `rustnf`: linha de comando, API HTTP e servidor MCP do emissor de NFS-e nacional.
//!
//! Este crate so' LE a entrada, chama a biblioteca `rustnf` e escreve a saida: nenhuma regra fiscal mora aqui.
//!
//! * [`cli`] -- subcomandos e a execucao de cada um;
//! * [`http`] -- API JSON (`rustnf serve`);
//! * [`mcp`] -- servidor MCP em `POST /mcp`, no mesmo servidor;
//! * [`nucleo`] e [`emissao`] -- as operacoes que os tres compartilham;
//! * [`erro`] -- o erro unico, com codigo de saida e envelope HTTP.
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod cli;
pub mod emissao;
pub mod entrada;
pub mod erro;
pub mod http;
pub mod mcp;
pub mod nucleo;
pub mod segredo;
