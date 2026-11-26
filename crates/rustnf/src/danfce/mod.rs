// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! DANFC-e: o cupom da NFC-e (modelo 65), a partir do `nfeProc` da nota autorizada.
//!
//! * [`dados`] devolve o conteudo por divisao do Manual (I a IX), ja formatado, sem I/O e com JSON estavel;
//! * [`pdf`] escreve o cupom num PDF de bobina de 80 mm (padrao) ou de 58 mm, com altura pelo conteudo, o QR Code no
//!   tamanho do Manual e texto selecionavel (escritor e fonte do DANFSe);
//! * [`escpos`] devolve os bytes para impressora termica ESC/POS (QR Code pelo comando nativo, sem abrir impressora).
//!
//! A mesma entrada gera sempre os mesmos bytes. O cupom so' sai de nota autorizada, cancelada ou emitida em
//! contingencia off-line ainda pendente; nota sem protocolo fora da contingencia, denegada ou rejeitada e' recusada
//! com o motivo.
//!
//! ```ignore
//! let pdf = rustnf::danfce::pdf(proc_nfce.as_bytes(), &rustnf::danfce::Opcoes::default())?;
//! let bytes = rustnf::danfce::escpos(proc_nfce.as_bytes(), &rustnf::danfce::Opcoes::default())?;
//! ```
//!
//! Norma: Manual de Padroes Tecnicos do DANFE NFC-e e QR Code, versao 6.0 (marco de 2025), do Portal Nacional da NF-e:
//! leiaute das divisoes (3.1), papel e margens (3.3), tamanho do QR Code (3.4) e nivel de correcao M (4.5.2). Texto e
//! corpo da letra nao sao regulados pelo Manual; valores com virgula nos centavos e ponto no milhar.

mod conteudo;
mod escpos;
mod modelo;
mod pdf;
mod tabelas_escpos;

use std::fmt;

use crate::nfe::Nfe;

pub use modelo::*;

/// O que impede de gerar o DANFC-e.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErroDanfce {
    /// O XML nao e' uma NFC-e legivel (o motivo da leitura).
    Leitura(String),
    /// Nao e' modelo 65 (o modelo encontrado); a NF-e 55 tem o DANFE A4.
    NaoENfce(String),
    /// A chave de acesso nao confere (o motivo).
    Chave(String),
    /// Sem `infNFeSupl/qrCode`: o QR Code e' obrigatorio no cupom (Manual, 4).
    SemQr,
    /// Sem `infNFeSupl/urlChave`: a divisao IV exige a URL de consulta (Manual, 3.1.4).
    SemUrlChave,
    /// A nota nao tem `dhEmi`.
    SemEmissao,
    /// `tpEmis` que a NFC-e nao tem (so' 1 e 9).
    EmissaoInexistente(String),
    /// Sem protocolo de autorizacao e fora da contingencia off-line: a nota nao vale.
    SemProtocolo,
    /// Nota denegada.
    Denegada,
    /// Protocolo de rejeicao (o `cStat`).
    Rejeitada(String),
    /// O protocolo e' de outra chave: o `nfeProc` foi montado errado.
    ProtocoloDeOutraChave,
    /// Pediu o cupom de nota cancelada, mas a nota nao tem protocolo de autorizacao.
    CanceladaSemAutorizacao,
    /// A via do estabelecimento so' existe na contingencia off-line (Manual, 3.1.8).
    ViaSemContingencia,
    /// Colunas do ESC/POS fora de 48, 42 e 32.
    Colunas(u8),
    /// O QR Code nao coube (o motivo).
    Qr(String),
    /// A montagem do PDF falhou.
    Pdf(String),
}

impl fmt::Display for ErroDanfce {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Leitura(m) => write!(f, "XML da NFC-e ilegivel: {m}"),
            Self::NaoENfce(m) => write!(
                f,
                "o DANFC-e e' da NFC-e modelo 65; o documento e' do modelo '{m}'"
            ),
            Self::Chave(m) => write!(f, "chave de acesso invalida: {m}"),
            Self::SemQr => f.write_str("a NFC-e nao tem infNFeSupl/qrCode: o QR Code e' obrigatorio no cupom"),
            Self::SemUrlChave => f.write_str("a NFC-e nao tem infNFeSupl/urlChave: a consulta pela chave e' obrigatoria no cupom"),
            Self::SemEmissao => f.write_str("a nota nao tem dhEmi"),
            Self::EmissaoInexistente(t) => write!(f, "tpEmis '{t}' nao existe na NFC-e (so' 1 e 9)"),
            Self::SemProtocolo => f.write_str(
                "a NFC-e nao tem protocolo de autorizacao e nao esta em contingencia off-line: nao ha cupom",
            ),
            Self::Denegada => f.write_str("a NFC-e foi denegada: nao ha cupom"),
            Self::Rejeitada(c) => write!(f, "o protocolo e' de rejeicao (cStat {c}): nao ha cupom"),
            Self::ProtocoloDeOutraChave => {
                f.write_str("o protocolo e' de outra chave: o nfeProc esta montado errado")
            }
            Self::CanceladaSemAutorizacao => {
                f.write_str("cupom de nota cancelada pede o protocolo de autorizacao da nota")
            }
            Self::ViaSemContingencia => {
                f.write_str("a via do estabelecimento so' existe na contingencia off-line")
            }
            Self::Colunas(c) => write!(f, "{c} colunas: use 48, 42 ou 32"),
            Self::Qr(m) => write!(f, "QR Code: {m}"),
            Self::Pdf(m) => write!(f, "PDF: {m}"),
        }
    }
}

impl std::error::Error for ErroDanfce {}

/// Largura da bobina.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Largura {
    /// 80 mm (padrao): 48 colunas no ESC/POS.
    #[default]
    Mm80,
    /// 58 mm: 32 colunas no ESC/POS.
    Mm58,
}

/// Pagina de codigo que o ESC/POS declara (`ESC t`) para os acentos.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PaginaDeCodigo {
    /// PC850 (multilingue latino), `ESC t 2`.
    #[default]
    Pc850,
    /// PC860 (portugues), `ESC t 3`.
    Pc860,
    /// Windows-1252 (WPC1252), `ESC t 16`.
    Wpc1252,
}

/// Opcoes de impressao.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opcoes {
    /// Largura da bobina.
    pub largura: Largura,
    /// Cupom resumido, sem a divisao II (so' onde a UF permite e o consumidor pede; Manual, 2 e 3.1.2).
    pub resumido: bool,
    /// A nota foi cancelada por evento (o `nfeProc` guardado segue com o protocolo de autorizacao). O protocolo de
    /// cancelamento (101 ou 151) tambem marca.
    pub cancelada: bool,
    /// Contingencia: a segunda via, "Via do Estabelecimento" (Manual, 3.1.8).
    pub via_estabelecimento: bool,
    /// Imprime o nome e o endereco do consumidor identificado (opcionais no Manual, 3.1.6; na entrega a domicilio
    /// saem sempre). O documento do consumidor sai como esta no XML: o Manual nao permite mascara.
    pub consumidor_com_nome_e_endereco: bool,
    /// Tributos totais da Lei 12.741/2012 (`vTotTrib`), na divisao IX (Manual, 3.1.9.1).
    pub tributos: bool,
    /// Mensagem institucional depois da divisao IX (nao consta do XML; Manual, 3.1.9).
    pub mensagem_institucional: Option<String>,
    /// So' no ESC/POS: colunas (48, 42 ou 32); sem valor, 48 na bobina de 80 mm e 32 na de 58 mm.
    pub colunas: Option<u8>,
    /// So' no ESC/POS: pagina de codigo dos acentos.
    pub pagina_de_codigo: PaginaDeCodigo,
    /// So' no ESC/POS: corte parcial do papel no fim.
    pub cortar: bool,
}

impl Default for Opcoes {
    fn default() -> Self {
        Opcoes {
            largura: Largura::Mm80,
            resumido: false,
            cancelada: false,
            via_estabelecimento: false,
            consumidor_com_nome_e_endereco: true,
            tributos: true,
            mensagem_institucional: None,
            colunas: None,
            pagina_de_codigo: PaginaDeCodigo::Pc850,
            cortar: true,
        }
    }
}

/// O conteudo do DANFC-e a partir do XML (`nfeProc` ou `procNFe` da NFC-e).
///
/// # Errors
///
/// [`ErroDanfce`] quando o XML nao e' uma NFC-e legivel ou a nota nao tem cupom (sem protocolo fora da contingencia,
/// denegada, rejeitada, NF-e 55).
pub fn dados(xml: &[u8], op: &Opcoes) -> Result<Danfce, ErroDanfce> {
    let nfe = crate::nfe::ler(xml).map_err(|e| ErroDanfce::Leitura(e.to_string()))?;
    dados_da_nfe(&nfe, op)
}

/// [`dados`] para uma NFC-e que quem chama ja leu.
///
/// # Errors
///
/// Como em [`dados`], menos a leitura do XML.
pub fn dados_da_nfe(nfe: &Nfe, op: &Opcoes) -> Result<Danfce, ErroDanfce> {
    conteudo::conteudo(nfe, op)
}

/// O DANFC-e em PDF de bobina (uma pagina, altura pelo conteudo). Sincrono e pesado em CPU.
///
/// # Errors
///
/// [`ErroDanfce`] como em [`dados`], ou [`ErroDanfce::Qr`] e [`ErroDanfce::Pdf`] na escrita.
pub fn pdf(xml: &[u8], op: &Opcoes) -> Result<Vec<u8>, ErroDanfce> {
    pdf_de(&dados(xml, op)?, op)
}

/// O PDF a partir do conteudo de [`dados`].
///
/// # Errors
///
/// [`ErroDanfce::Qr`] ou [`ErroDanfce::Pdf`].
pub fn pdf_de(d: &Danfce, op: &Opcoes) -> Result<Vec<u8>, ErroDanfce> {
    pdf::pdf(d, op.largura)
}

/// Os bytes ESC/POS do cupom, para uma impressora termica. Nao abre impressora, porta nem socket.
///
/// # Errors
///
/// [`ErroDanfce`] como em [`dados`], ou [`ErroDanfce::Colunas`] e [`ErroDanfce::Qr`].
pub fn escpos(xml: &[u8], op: &Opcoes) -> Result<Vec<u8>, ErroDanfce> {
    escpos_de(&dados(xml, op)?, op)
}

/// Os bytes ESC/POS a partir do conteudo de [`dados`].
///
/// # Errors
///
/// [`ErroDanfce::Colunas`] ou [`ErroDanfce::Qr`].
pub fn escpos_de(d: &Danfce, op: &Opcoes) -> Result<Vec<u8>, ErroDanfce> {
    escpos::bytes(d, op)
}
