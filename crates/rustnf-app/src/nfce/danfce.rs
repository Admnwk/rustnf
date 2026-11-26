// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O DANFC-e como ARQUIVO (PDF de bobina ou bytes ESC/POS), da nota da pasta ou de um XML. Nao abre impressora, porta nem
//! rede: quem imprime e' outro programa.

use rustnf::danfce::{self, Largura, Opcoes};

use crate::config::Config;
use crate::erro::Erro;
use crate::nfe::chave_limpa;
use crate::notas::nfe::{NotasNfe, Situacao};

/// O que a pessoa pediu do cupom; o que faltar vem de `[nfce]`.
#[derive(Debug, Clone, Copy, Default)]
pub struct Pedido {
    /// 80 ou 58 (mm).
    pub largura: Option<u8>,
    /// Colunas do ESC/POS (48, 42 ou 32).
    pub colunas: Option<u8>,
    /// Cupom resumido.
    pub resumido: bool,
    /// A via do estabelecimento (so' existe na contingencia off-line).
    pub via_estabelecimento: bool,
}

fn erro_do_cupom(e: &danfce::ErroDanfce) -> Erro {
    Erro::Entrada(e.to_string())
}

/// As opcoes da biblioteca: o pedido, senao `[nfce].largura_danfce` e `[nfce].colunas_escpos`.
///
/// # Errors
///
/// [`Erro::Uso`] para largura ou colunas que nao existem.
pub fn opcoes(config: &Config, p: Pedido, cancelada: bool) -> Result<Opcoes, Erro> {
    let largura = match p.largura.unwrap_or(config.nfce.largura_danfce) {
        80 => Largura::Mm80,
        58 => Largura::Mm58,
        _ => return Err(Erro::Uso("a largura da bobina e' 80 ou 58 (mm)".into())),
    };
    let colunas = p
        .colunas
        .or_else(|| Some(config.nfce.colunas_escpos).filter(|c| *c != 0));
    if colunas.is_some_and(|c| !matches!(c, 48 | 42 | 32)) {
        return Err(Erro::Uso("as colunas do ESC/POS sao 48, 42 ou 32".into()));
    }
    Ok(Opcoes {
        largura,
        resumido: p.resumido,
        cancelada,
        via_estabelecimento: p.via_estabelecimento,
        colunas,
        ..Opcoes::default()
    })
}

/// O PDF do cupom (sincrono e pesado: dentro de um servidor, em tarefa propria).
///
/// # Errors
///
/// [`Erro::Entrada`] se o XML nao e' uma NFC-e com cupom (NF-e 55, denegada, rejeitada, sem protocolo fora da off-line).
pub fn pdf(xml: &str, op: &Opcoes) -> Result<Vec<u8>, Erro> {
    danfce::pdf(xml.as_bytes(), op).map_err(|e| erro_do_cupom(&e))
}

/// Os bytes ESC/POS do cupom.
///
/// # Errors
///
/// Como em [`pdf`], mais as colunas que a impressora nao tem.
pub fn escpos(xml: &str, op: &Opcoes) -> Result<Vec<u8>, Erro> {
    danfce::escpos(xml.as_bytes(), op).map_err(|e| erro_do_cupom(&e))
}

/// O XML de uma nota da pasta e se ela esta cancelada (o cupom leva a marca).
///
/// # Errors
///
/// [`Erro::NaoEncontrada`] se a chave nao esta na pasta; [`Erro::Entrada`] para a chave torta.
pub fn xml_da_pasta(notas: &NotasNfe, chave: &str) -> Result<(String, bool), Erro> {
    let chave = chave_limpa(chave);
    let r = notas.registro(&chave)?;
    Ok((notas.ler_xml(&chave)?, r.situacao == Situacao::Cancelada))
}

/// Guarda o PDF ao lado do XML da nota (`<chave>-procNFe.pdf`, ou `<chave>-NFe.pdf` se a nota so' tem a assinada) e devolve
/// o caminho.
///
/// # Errors
///
/// [`Erro`] se a nota nao esta na pasta ou o arquivo nao se grava.
pub fn guardar(notas: &NotasNfe, chave: &str, pdf: &[u8]) -> Result<std::path::PathBuf, Erro> {
    Ok(notas.guardar_danfe(&chave_limpa(chave), pdf)?)
}
