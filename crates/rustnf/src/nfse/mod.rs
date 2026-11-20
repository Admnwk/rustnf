// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! NFS-e Padrao Nacional (leiaute 1.01): DPS completa com as regras do Anexo I conferidas antes do envio, eventos,
//! leitura do retorno e Lista Nacional de servicos.
//!
//! Fontes publicas: portal da NFS-e (<https://www.gov.br/nfse>): XSD, Anexo I (regras de validacao) e Anexo II.

pub mod base;
pub mod correlacao;
pub mod ctribnac;
pub mod dps;
pub mod eventos;
pub mod leitura;
#[cfg(feature = "sefin")]
pub mod sefin;

pub use base::{ErroNfse, Valor};
pub use dps::{
    aliquota_iss, id_dps, montar, AjusteLocImovel, AtividadeEvento, BemMovel, ComercioExterior,
    Deducao, DocAjusteBC, DocDeducao, DocReembolso, Dps, Endereco, EnderecoSimples, IbsCbs, Imovel,
    InfoComplementar, LocacaoImovel, Obra, Pessoa, PisCofins, Prestador, Servico, Substituicao,
    TotalTributos, TributacaoFederal, TributacaoMunicipal, UnidadeImobiliaria, Valores, LEIAUTES,
    LEIAUTE_NT009, LEIAUTE_PRODUCAO,
};
pub use leitura::{
    compactar, descompactar, ler_evento, ler_nfse, mensagens, Evento, Mensagem, Nfse,
};
