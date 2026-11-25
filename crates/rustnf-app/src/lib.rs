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
//! * [`certificado`] -- abrir o certificado A1 e montar o cliente da Sefin Nacional;
//! * [`eventos`] -- o Id da DPS pelo `[emitente]` e o registro de eventos do contribuinte;
//! * [`notas`] -- a pasta de notas autorizadas e a numeracao da DPS;
//! * [`atualizacao`] -- a situacao das notas da pasta conferida contra a Sefin, em lote;
//! * [`atomico`] -- gravacao atomica de arquivo, para quem guarda algo na pasta de notas.
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod atomico;
pub mod atualizacao;
pub mod cep;
pub mod certificado;
pub mod cofre;
pub mod config;
pub mod emissao;
pub mod erro;
pub mod eventos;
pub mod notas;
pub mod nucleo;
pub mod pedido;
pub mod segredo;

#[cfg(test)]
mod testes_comuns;
#[cfg(test)]
mod testes_eventos;
#[cfg(test)]
mod testes_sefin;
