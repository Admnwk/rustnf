// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Camada de aplicacao do `rustnf`: o que a linha de comando, a API HTTP e uma interface de desktop compartilham.
//!
//! Sem tela, sem servidor HTTP e sem analise de argumentos. Nenhuma regra fiscal mora aqui: cada operacao le a
//! entrada ja em memoria, chama a biblioteca `rustnf` e devolve o resultado.
//!
//! * [`nucleo`] e [`emissao`] -- calculo, montagem da DPS, DANFSe, emissao, consulta e cancelamento;
//! * [`erro`] -- o erro unico, com codigo de saida e envelope JSON;
//! * [`segredo`] -- senha em memoria, com `Debug` redigido;
//! * [`config`] -- o arquivo de configuracao (TOML), comum a todas as interfaces;
//! * [`cofre`] -- onde a senha do certificado fica guardada;
//! * [`notas`] -- a pasta de notas autorizadas e a numeracao da DPS.
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod emissao;
pub mod erro;
pub mod nucleo;
pub mod segredo;
