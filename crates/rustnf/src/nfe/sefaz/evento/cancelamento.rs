// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Cancelamento (110111) e Carta de Correcao (110110).
//!
//! * **Cancelamento.** MOC 7.00, Visao Geral, 5.9 (tabelas 5-37 e 5-38): `nProt` do protocolo de autorizacao e
//!   justificativa de 15 a 255 caracteres, `nSeqEvento` 1. O XSD (`e110111_v1.00`) fixa `descEvento` "Cancelamento".
//!   **Prazo:** a regra 2P12-14 pede a rejeicao 501 ("Prazo de cancelamento superior ao previsto na Legislacao") se a
//!   NF-e foi autorizada ha mais de 1 dia (24 horas) e ressalva a excecao de prazo da legislacao estadual; e o item
//!   5.9.4 deixa a SEFAZ aceitar fora do prazo com o codigo 155 ("Cancelamento homologado fora de prazo"). Como o prazo
//!   efetivo e' o da UF, o cliente NAO o confere: envia, e o resultado diz em palavras o que a SEFAZ decidiu (101/135,
//!   155 ou 501).
//! * **Carta de correcao.** MOC 7.00, Visao Geral, 5.10 (tabelas 5-39 e 5-40): texto de 15 a 1000 caracteres, sequencia
//!   de 1 a 20 (rejeicao 594), so' para NF-e autorizada (580) e nunca para NFC-e (784). **A carta nova substitui a
//!   anterior**: ela tem de trazer TODAS as correcoes que ainda valem. `xCondUso` e' o texto fixo do XSD
//!   (`e110110_v1.00`), que aceita duas formas, com e sem acento; este cliente usa a sem acento, igual a `descEvento`
//!   "Carta de Correcao" que o mesmo XSD aceita.

use chrono::{DateTime, FixedOffset};

use super::super::erro::ErroSefaz;
use super::super::pedidos::chave_valida;
use super::comum::{montar, protocolo, texto_livre, Cabecalho, Montado};
use crate::nfe::vigencias::Ambiente;

/// `tpEvento` do cancelamento.
pub const CANCELAMENTO: &str = "110111";
/// `tpEvento` da carta de correcao.
pub const CARTA_CORRECAO: &str = "110110";

/// Maior `nSeqEvento` da carta de correcao (MOC 7.00, Visao Geral, 5.10.3, regra GA03).
pub const MAX_SEQ_CARTA: u32 = 20;

/// O texto fixo de `xCondUso`, sem acento: um dos dois que o `e110110_v1.00.xsd` aceita.
pub const COND_USO: &str = "A Carta de Correcao e disciplinada pelo paragrafo 1o-A do art. 7o do Convenio S/N, de 15 de dezembro de 1970 e pode ser utilizada para regularizacao de erro ocorrido na emissao de documento fiscal, desde que o erro nao esteja relacionado com: I - as variaveis que determinam o valor do imposto tais como: base de calculo, aliquota, diferenca de preco, quantidade, valor da operacao ou da prestacao; II - a correcao de dados cadastrais que implique mudanca do remetente ou do destinatario; III - a data de emissao ou de saida.";

/// Os dados de um pedido de cancelamento.
#[derive(Debug, Clone)]
pub struct Cancelamento<'a> {
    /// Chave da NF-e a cancelar.
    pub chave: &'a str,
    /// `nProt` do protocolo de AUTORIZACAO da NF-e (15 ou 17 digitos).
    pub protocolo: &'a str,
    /// Justificativa, de 15 a 255 caracteres.
    pub justificativa: &'a str,
    /// Ambiente.
    pub ambiente: Ambiente,
    /// Data e hora do evento.
    pub quando: DateTime<FixedOffset>,
}

/// Os dados de uma carta de correcao.
#[derive(Debug, Clone)]
pub struct CartaCorrecao<'a> {
    /// Chave da NF-e a corrigir.
    pub chave: &'a str,
    /// A correcao, de 15 a 1000 caracteres. Substitui as cartas anteriores.
    pub correcao: &'a str,
    /// `nSeqEvento`: 1 na primeira carta e 1 a mais a cada nova, ate 20.
    pub seq: u32,
    /// Ambiente.
    pub ambiente: Ambiente,
    /// Data e hora do evento.
    pub quando: DateTime<FixedOffset>,
}

fn autor_da_chave(c: &crate::chave_dfe::Chave) -> &'static str {
    if c.emitente_cpf {
        "CPF"
    } else {
        "CNPJ"
    }
}

/// Monta o evento de cancelamento (sem assinatura).
///
/// # Errors
///
/// [`ErroSefaz`] se a chave nao e' de NF-e valida, o protocolo nao tem 15 ou 17 digitos ou a justificativa esta fora
/// de 15 a 255 caracteres.
pub fn cancelamento(p: &Cancelamento<'_>) -> Result<Montado, ErroSefaz> {
    let c = chave_valida(p.chave)?;
    let n_prot = protocolo(p.protocolo, "nProt")?;
    let det = format!(
        "<descEvento>Cancelamento</descEvento><nProt>{n_prot}</nProt><xJust>{}</xJust>",
        texto_livre(p.justificativa, "xJust", 15, 255)?
    );
    Ok(montar(
        &Cabecalho {
            c_orgao: c.cuf,
            ambiente: p.ambiente,
            autor_tag: autor_da_chave(&c),
            autor: &c.emitente,
            chave: &c.chave,
            tp_evento: CANCELAMENTO,
            seq: 1,
            uf_fuso: c.uf,
            quando: &p.quando,
        },
        &det,
    ))
}

/// Monta a carta de correcao (sem assinatura).
///
/// # Errors
///
/// [`ErroSefaz`] se a chave nao e' de NF-e valida, a sequencia nao esta entre 1 e 20 ou a correcao esta fora de 15 a
/// 1000 caracteres.
pub fn carta_correcao(p: &CartaCorrecao<'_>) -> Result<Montado, ErroSefaz> {
    let c = chave_valida(p.chave)?;
    if !(1..=MAX_SEQ_CARTA).contains(&p.seq) {
        return Err(ErroSefaz::novo(
            "nSeqEvento da carta de correcao: de 1 a 20 (rejeicao 594)",
            "nSeqEvento",
        ));
    }
    let det = format!(
        "<descEvento>Carta de Correcao</descEvento><xCorrecao>{}</xCorrecao><xCondUso>{COND_USO}</xCondUso>",
        texto_livre(p.correcao, "xCorrecao", 15, 1000)?
    );
    Ok(montar(
        &Cabecalho {
            c_orgao: c.cuf,
            ambiente: p.ambiente,
            autor_tag: autor_da_chave(&c),
            autor: &c.emitente,
            chave: &c.chave,
            tp_evento: CARTA_CORRECAO,
            seq: p.seq,
            uf_fuso: c.uf,
            quando: &p.quando,
        },
        &det,
    ))
}
