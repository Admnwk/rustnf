// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! NF-e (modelo 55), leiaute 4.00: leitura, conferencia, montagem e assinatura. A NF-e ainda NAO e' enviada a SEFAZ.
//!
//! * [`leitura`]: o XML (`nfeProc` ou `NFe` solta) para a struct [`Nfe`]; [`conferencia`]: a situacao pelo protocolo e
//!   as divergencias entre as partes do proprio XML;
//! * [`cstat`]: a tabela dos codigos de retorno (`cStat`) do MOC, com a mensagem oficial e a classe;
//! * [`vigencias`]: o calendario das Notas Tecnicas por ambiente;
//! * [`montagem`]: o pedido em JSON vira o XML da NF-e, conferido pelas regras do MOC e das Notas Tecnicas, com o IBS/CBS, e
//!   o `infNFe` e' assinado com o A1;
//! * a chave de acesso de 44 posicoes esta em [`crate::chave_dfe`] (a mesma biblioteca que confere a da NFS-e).
//!
//! Fontes publicas: Manual de Orientacao do Contribuinte (MOC) 7.00, Anexo I (leiaute e regras de validacao), e as
//! Notas Tecnicas do Portal Nacional da NF-e (<https://www.nfe.fazenda.gov.br/portal>). A lista completa esta em
//! `docs/FONTES.md`.

pub mod conferencia;
pub mod cstat;
pub mod decimal;
pub mod erro;
pub mod leitura;
pub mod modelo;
pub mod montagem;
pub mod vigencias;
mod xml;

pub use conferencia::{conferir, Situacao};
pub use decimal::{DataHora, Decimal};
pub use erro::{ErroLeitura, TipoCampo};
pub use leitura::ler;
pub use modelo::{
    Cobranca, Duplicata, Endereco, Folhas, FormaPagamento, Grupo, Ide, Imposto, InfAdic, Item, Nfe,
    Obs, Participante, Protocolo, Rastro, Total, Transporte, Volume,
};
pub use xml::{LIMITE_BYTES, NS};
