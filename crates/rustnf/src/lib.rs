// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! rustnf: emissor aberto de NFS-e nacional em Rust.
//!
//! * [`nfse`] -- DPS (leiaute 1.01 e NT 009) com as regras do Anexo I conferidas antes do envio, eventos,
//!   leitura do retorno, Lista Nacional de servicos e correlacao tributaria;
//! * [`reforma`] -- IBS/CBS da Reforma Tributaria (LC 214/2025) no que a NFS-e usa: tabela de CST/cClassTrib,
//!   cIndOp, atividades do Simples Nacional e aliquotas;
//! * [`assinatura`] -- XMLDSig (C14N) e certificado A1 (.pfx);
//! * `nfe` (feature `nfe`) -- NF-e modelo 55: leitura do XML, tabela de `cStat` e calendario das Notas Tecnicas; ainda
//!   NAO emite;
//! * `danfse` (feature `danfse`) -- DANFSe em PDF (NT 008) a partir do XML da NFS-e autorizada.
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used)]

pub mod aleatorio;
pub mod assinatura;
pub mod chave_dfe;
#[cfg(feature = "danfse")]
pub mod danfse;
#[cfg(feature = "nfe")]
pub mod nfe;
pub mod nfse;
pub mod reforma;
