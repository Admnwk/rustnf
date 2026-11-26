// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! DANFE da NF-e modelo 55 em PDF: A4 retrato, folhas soltas, a partir do `nfeProc` (ou da `NFe` sem protocolo, so'
//! como previa).
//!
//! * [`dados`] devolve o que sera impresso: o conteudo ja formatado, o plano das folhas e cada folha como uma lista
//!   de operacoes de desenho em cm. E' texto puro (JSON), sem I/O;
//! * [`pdf`] escreve essas folhas direto em um PDF (texto selecionavel), com a Liberation Serif embutida no binario
//!   e em subconjunto, e devolve os bytes. A mesma entrada gera sempre os mesmos bytes: nada no PDF vem do relogio ou
//!   da maquina.
//!
//! ```ignore
//! let pdf = rustnf::danfe::pdf(proc_nfe.as_bytes(), &rustnf::danfe::Opcoes::default())?;
//! ```
//!
//! O desenho segue o MOC 7.00, Anexo II (Manual de Especificacoes Tecnicas do DANFE e Codigo de Barras): blocos e
//! medidas do item 3.8.1, corpos de fonte minimos do item 3.7, itens em varias folhas (3.5) e os campos 1 e 2 do
//! quadro da chave conforme a forma de emissao (3.9). O leiaute da Reforma Tributaria (IBS, CBS e IS) segue a NT 2026.010
//! v1.00 e vale para a emissao a partir da vigencia que [`crate::nfe::vigencias`] guarda; antes dela, o do MOC.
//! A fonte e' a Liberation Serif (metrica da Times New Roman, item 3.7), licenca SIL OFL 1.1.
//!
//! O que muda com a situacao da nota: homologacao ("SEM VALOR FISCAL"), cancelada, denegada, nota sem protocolo (so'
//! previa), contingencia com formulario de seguranca, SVC e EPEC, cada uma com o texto e os campos que o Anexo II e o
//! Anexo III do MOC mandam. O DANFE so' imprime o que consta do XML (item 3.1).

pub mod barras;
mod blocos;
mod blocos_rtc;
mod conteudo;
mod desenho;
mod folhas;
mod formato;
mod medidas;
mod rtc;
mod tabela;

use std::fmt;
use std::str::FromStr;

use serde_json::{json, Value};

use crate::danfse::escritor::pdf_paginas;
use crate::danfse::metrica::Familia;
use crate::danfse::qr;
use crate::nfe::modelo::Nfe;

/// O que impede de gerar o DANFE.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErroDanfe {
    /// O XML nao e' uma NF-e legivel (o motivo da leitura).
    Leitura(String),
    /// Nao e' modelo 55 (o modelo encontrado); a NFC-e tem o DANFE NFC-e.
    NaoEModelo55(String),
    /// A chave de acesso nao confere (o motivo).
    Chave(String),
    /// O protocolo diz que a nota foi rejeitada: nao ha DANFE.
    Rejeitada,
    /// O protocolo e' de outra chave: o `nfeProc` foi montado errado.
    ProtocoloDeOutraChave,
    /// A nota nao tem `dhEmi`.
    SemEmissao,
    /// `tpEmis` 9 (contingencia off-line) e' so' da NFC-e.
    EmissaoOffline,
    /// Contingencia EPEC sem o protocolo do evento (numero, data e hora do registro).
    EpecSemProtocolo,
    /// O protocolo do EPEC nao esta na forma `NNNNNNNNNNNNNNN dd/mm/aaaa hh:mm:ss`.
    EpecProtocoloInvalido,
    /// Contingencia com formulario de seguranca: o destinatario nao tem UF valida para o codigo de barras dos dados.
    ContingenciaSemUf,
    /// O codigo de barras dos dados da NF-e saiu com tamanho errado (as posicoes encontradas).
    ContingenciaTamanho(usize),
    /// Codigo de barras que o CODE-128 nao representa.
    Barras(String),
    /// O QR Code nao coube (o motivo).
    Qr(String),
    /// Um item e' maior que uma folha inteira (o numero do item).
    ItemMaiorQueAFolha(usize),
    /// A montagem do PDF falhou (fonte, pagina, logotipo ou operacao de desenho recusada).
    Pdf(String),
}

impl fmt::Display for ErroDanfe {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Leitura(m) => write!(f, "XML da NF-e ilegivel: {m}"),
            Self::NaoEModelo55(m) => write!(
                f,
                "o DANFE A4 e' da NF-e modelo 55; o documento e' do modelo '{m}'"
            ),
            Self::Chave(m) => write!(f, "chave de acesso invalida: {m}"),
            Self::Rejeitada => f.write_str("a nota foi rejeitada pela SEFAZ: nao ha DANFE"),
            Self::ProtocoloDeOutraChave => {
                f.write_str("o protocolo e' de outra chave: o nfeProc esta montado errado")
            }
            Self::SemEmissao => f.write_str("a nota nao tem dhEmi"),
            Self::EmissaoOffline => f.write_str("tpEmis 9 (contingencia off-line) e' so' da NFC-e"),
            Self::EpecSemProtocolo => f.write_str(
                "contingencia EPEC: informe o protocolo do evento (numero, data e hora do registro)",
            ),
            Self::EpecProtocoloInvalido => f.write_str(
                "protocolo do EPEC fora da forma 'NNNNNNNNNNNNNNN dd/mm/aaaa hh:mm:ss'",
            ),
            Self::ContingenciaSemUf => f.write_str(
                "contingencia com formulario de seguranca: o destinatario nao tem UF valida para o codigo de barras dos dados",
            ),
            Self::ContingenciaTamanho(n) => write!(
                f,
                "o codigo de barras dos dados da NF-e saiu com {n} posicoes (esperado 35 mais o DV)"
            ),
            Self::Barras(m) => write!(f, "codigo de barras: {m}"),
            Self::Qr(m) => write!(f, "QR Code: {m}"),
            Self::ItemMaiorQueAFolha(n) => write!(f, "o item {n} e' maior que uma folha inteira"),
            Self::Pdf(m) => write!(f, "PDF: {m}"),
        }
    }
}

impl std::error::Error for ErroDanfe {}

/// Qual leiaute imprimir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Leiaute {
    /// O da Reforma Tributaria (NT 2026.010) para a emissao a partir da vigencia, o do MOC antes dela.
    #[default]
    Auto,
    /// O do MOC 7.00, Anexo II.
    Moc,
    /// O da NT 2026.010, mesmo antes da vigencia.
    Rtc,
}

impl FromStr for Leiaute {
    type Err = ErroDanfe;

    fn from_str(s: &str) -> Result<Self, ErroDanfe> {
        match s {
            "auto" => Ok(Self::Auto),
            "moc" => Ok(Self::Moc),
            "rtc" => Ok(Self::Rtc),
            outro => Err(ErroDanfe::Leitura(format!(
                "leiaute '{}': use auto, moc ou rtc",
                outro.chars().take(15).collect::<String>()
            ))),
        }
    }
}

/// Opcoes de impressao.
#[derive(Clone, Copy)]
pub struct Opcoes<'a> {
    /// Com o canhoto na primeira folha (item 3.3.1: o emitente pode suprimi-lo).
    pub canhoto: bool,
    /// Logotipo do emitente, em PNG ou JPEG (item 3.1.3); sem ele, o quadro do emitente so' leva texto.
    pub logo: Option<&'a [u8]>,
    /// Protocolo do EPEC, `NNNNNNNNNNNNNNN dd/mm/aaaa hh:mm:ss`: obrigatorio na contingencia EPEC (`tpEmis` 4), que o
    /// XML da nota nao traz (item 3.9.3). Sai do evento registrado: `nfe::sefaz::protocolo_epec_danfe`.
    pub protocolo_epec: Option<&'a str>,
    /// A nota foi cancelada por evento (o `nfeProc` guardado segue com o protocolo de autorizacao): marca d'agua de
    /// nota cancelada. O protocolo de cancelamento (101 ou 151) tambem marca.
    pub cancelada: bool,
    /// O leiaute.
    pub leiaute: Leiaute,
}

impl Default for Opcoes<'_> {
    fn default() -> Self {
        Opcoes {
            canhoto: true,
            logo: None,
            protocolo_epec: None,
            cancelada: false,
            leiaute: Leiaute::Auto,
        }
    }
}

impl fmt::Debug for Opcoes<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Opcoes")
            .field("canhoto", &self.canhoto)
            .field("logo", &self.logo.map(|l| l.len()))
            .field("protocolo_epec", &self.protocolo_epec)
            .field("cancelada", &self.cancelada)
            .field("leiaute", &self.leiaute)
            .finish()
    }
}

/// O JSON do DANFE a partir do XML da NF-e (`nfeProc` ou `NFe` solta): o conteudo, o plano das folhas (`plano`) e as
/// folhas (`paginas`, cada uma com as operacoes de desenho em cm).
///
/// # Errors
///
/// [`ErroDanfe`] quando o XML nao e' uma NF-e legivel ou a nota nao tem DANFE (rejeitada, NFC-e, contingencia sem os
/// dados que o papel exige).
pub fn dados(xml: &[u8], op: &Opcoes) -> Result<Value, ErroDanfe> {
    let nfe = crate::nfe::ler(xml).map_err(|e| ErroDanfe::Leitura(e.to_string()))?;
    // a leitura tambem le a NFC-e (modelo 65); o DANFE A4 continua so' do 55, com a recusa de sempre para o XML
    if nfe.modelo != "55" {
        return Err(ErroDanfe::Leitura(
            crate::nfe::ErroLeitura::Modelo(nfe.modelo).to_string(),
        ));
    }
    dados_da_nfe(&nfe, op)
}

/// [`dados`] para uma NF-e que quem chama ja leu.
///
/// # Errors
///
/// Como em [`dados`], menos a leitura do XML.
pub fn dados_da_nfe(nfe: &Nfe, op: &Opcoes) -> Result<Value, ErroDanfe> {
    let rtc = match op.leiaute {
        Leiaute::Auto => rtc::vigente(nfe),
        Leiaute::Moc => false,
        Leiaute::Rtc => true,
    };
    let mut d = conteudo::conteudo(nfe, op, medidas::LARG)?;
    d["leiaute"] = json!(if rtc { "rtc" } else { "moc" });
    if rtc {
        d["rtc"] = rtc::dados(nfe)?;
        if !nfe.qr_code.is_empty() {
            d["qr"] = qr::dados(&nfe.qr_code).map_err(|e| ErroDanfe::Qr(e.to_string()))?;
        }
    }
    let (folhas, plano) = folhas::montar(&d)?;
    d["plano"] = plano;
    d["paginas"] = Value::Array(
        folhas
            .into_iter()
            .enumerate()
            .map(|(i, ops)| json!({"folha": i + 1, "ops": ops}))
            .collect(),
    );
    Ok(d)
}

/// O DANFE em PDF (A4 retrato, uma ou mais folhas). Sincrono e pesado em CPU: dentro de um servidor assincrono, rode
/// em uma tarefa propria para bloqueio.
///
/// # Errors
///
/// [`ErroDanfe`] como em [`dados`], ou [`ErroDanfe::Pdf`] se a escrita do PDF falhar.
pub fn pdf(xml: &[u8], op: &Opcoes) -> Result<Vec<u8>, ErroDanfe> {
    pdf_de(&dados(xml, op)?, op)
}

/// O PDF a partir do JSON de [`dados`].
///
/// # Errors
///
/// [`ErroDanfe::Pdf`] se a escrita do PDF falhar.
pub fn pdf_de(d: &Value, op: &Opcoes) -> Result<Vec<u8>, ErroDanfe> {
    let paginas: Vec<&[Value]> = d["paginas"]
        .as_array()
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .map(|p| p["ops"].as_array().map_or(&[][..], Vec::as_slice))
        .collect();
    let titulo = d["titulo"].as_str().unwrap_or("DANFE");
    pdf_paginas(Familia::Serif, titulo, d, &paginas, op.logo).map_err(|e| match e {
        crate::danfse::ErroDanfse::Pdf(m) => ErroDanfe::Pdf(m),
        outro => ErroDanfe::Pdf(outro.to_string()),
    })
}
