// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A NFC-e (modelo 65) na camada de aplicacao. Nenhuma regra fiscal mora aqui: a biblioteca monta, assina, envia, entende a
//! resposta e desenha o DANFC-e; esta camada completa o pedido com a configuracao, numera pela pasta, guarda e diz o resultado
//! em palavras. O que a NFC-e compartilha com a NF-e (envio, consulta, retomada, cancelamento) usa o MESMO caminho de
//! [`crate::nfe`], com o contexto da NFC-e:
//!
//! * a configuracao de [`crate::config::Config::como_nfce`] (ambiente e serie de `[nfce]`, o resto de `[nfe]`/`[emitente]`);
//! * a pasta [`NotasNfe::abrir_nfce`] (`nfce/` dentro da `[notas].pasta`), com indice e numeracao PROPRIOS;
//! * as opcoes do QR Code de [`opcoes_qr`] (versao 3 sem CSC, ou versao 2 com o CSC do cofre).
//!
//! * [`emitir`] -- a emissao, normal ou em contingencia off-line;
//! * [`csc`], [`opcoes_qr`] -- o CSC no cofre;
//! * [`listar_pendentes`] e [`aviso_de_fechamento`] -- o que falta transmitir ("ao fechar o caixa");
//! * [`aviso_de_prazo`] -- o prazo do cancelamento;
//! * [`danfce`] -- o cupom em PDF ou ESC/POS.

mod cancelar;
pub mod csc;
pub mod danfce;
mod emissao;
mod pendentes;
#[cfg(test)]
mod testes;

use crate::notas::nfe::NotasNfe;

pub use cancelar::{aviso_de_prazo, PRAZO_NACIONAL_HORAS};
pub use csc::opcoes_qr;
pub use emissao::{emitir, Emissao, Gravada, Modo};
pub use pendentes::{
    aviso_de_fechamento, erro_de_fechamento, listar as listar_pendentes,
    listar_agora as listar_pendentes_agora, Pendente, Prazo,
};

/// A pasta da NFC-e dentro da `[notas].pasta`.
///
/// # Errors
///
/// [`crate::erro::Erro`] se a pasta nao abre.
pub fn abrir_pasta(raiz: impl AsRef<std::path::Path>) -> Result<NotasNfe, crate::erro::Erro> {
    Ok(NotasNfe::abrir_nfce(raiz)?)
}
