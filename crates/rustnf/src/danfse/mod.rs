// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! DANFSe A4 retrato (NFS-e Padrao Nacional) a partir do XML da NFS-e autorizada.
//!
//! * [`dados`] devolve o que sera impresso: o conteudo ja formatado, o leiaute (topo de cada bloco, linhas
//!   quebradas) e a pagina como uma lista de operacoes de desenho em cm. E' texto puro (JSON), sem I/O;
//! * [`pdf`] compila o modelo Typst embutido sobre esse JSON, com a Liberation Sans e o logotipo da NFS-e tambem
//!   embutidos, e devolve os bytes do PDF.
//!
//! ```ignore
//! let pdf = rustnf::danfse::pdf(&xml_nfse, &rustnf::danfse::Opcoes::default())?;
//! ```
//!
//! Norma: Nota Tecnica 008 do Comite Gestor da NFS-e (DANFSe v2.0), publicada em <https://www.gov.br/nfse>. A
//! tabela de larguras da Liberation Sans (metrica da Arial, NT 008 item 2.4) decide a quebra de linha e o corte
//! com "..." antes do desenho. Onde o XSD da NFS-e e a NT divergem de outro gerador, vale o XSD/NT.
//! Valores em inteiros de ponto fixo, arredondamento meio-para-cima; nada de ponto flutuante nas contas de dinheiro.

mod compilar;
mod conteudo;
mod formato;
mod leiaute;
mod medidas;
mod metrica;
pub mod municipio;
mod pagina;
mod qr;
mod tabelas;
mod xml;

use std::fmt;
use std::str::FromStr;

use serde_json::{json, Value};

/// Consulta publica da NFS-e: o QR do DANFSe leva este endereco seguido da chave de acesso.
pub const CONSULTA: &str = "https://www.nfse.gov.br/ConsultaPublica/?tpc=1&chave=";
const NS: &str = "http://www.sped.fazenda.gov.br/nfse";

/// O que impede de gerar o DANFSe.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErroDanfse {
    /// Situacao fora de `valida`, `cancelada` ou `substituida` (o texto, cortado em 15 caracteres).
    Situacao(String),
    /// XML mal formado, com DTD ou de outro leiaute.
    XmlIlegivel(&'static str),
    /// A raiz do XML nao e' `NFSe` (o nome encontrado, cortado em 20 caracteres).
    NaoENfse(String),
    /// `infNFSe` sem o atributo `Id` (chave de acesso).
    SemChave,
    /// A chave nao coube no QR.
    Qr(String),
    /// O Typst recusou o modelo ou a montagem do PDF falhou.
    Pdf(String),
}

impl fmt::Display for ErroDanfse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Situacao(s) => write!(f, "situacao '{s}': use valida, cancelada ou substituida"),
            Self::XmlIlegivel(motivo) => write!(f, "XML da NFS-e ilegivel: {motivo}"),
            Self::NaoENfse(raiz) => write!(f, "o XML nao e' NFS-e (raiz '{raiz}')"),
            Self::SemChave => f.write_str("infNFSe sem Id (chave de acesso)"),
            Self::Qr(motivo) => write!(f, "QR: {motivo}"),
            Self::Pdf(motivo) => write!(f, "PDF: {motivo}"),
        }
    }
}

impl std::error::Error for ErroDanfse {}

/// Situacao da NFS-e no momento da impressao; as duas ultimas levam marca d'agua (NT 008 itens 2.5.1 e 2.5.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Situacao {
    #[default]
    Valida,
    Cancelada,
    Substituida,
}

impl FromStr for Situacao {
    type Err = ErroDanfse;

    fn from_str(s: &str) -> Result<Self, ErroDanfse> {
        match s {
            "valida" => Ok(Self::Valida),
            "cancelada" => Ok(Self::Cancelada),
            "substituida" => Ok(Self::Substituida),
            outra => Err(ErroDanfse::Situacao(outra.chars().take(15).collect())),
        }
    }
}

/// Resolvedor de municipio de quem chama: codigo IBGE (7 digitos) -> (nome, UF).
pub type Municipio<'a> = &'a dyn Fn(&str) -> Option<(String, String)>;

/// Opcoes de impressao.
#[derive(Clone, Copy)]
pub struct Opcoes<'a> {
    /// Situacao da NFS-e (marca d'agua de cancelada ou substituida).
    pub situacao: Situacao,
    /// Com o canhoto de cientificacao no rodape (NT 008 item 2.1.13).
    pub canhoto: bool,
    /// Resolvedor de municipio; sem ele vale a tabela embutida.
    pub municipio: Option<Municipio<'a>>,
}

impl Default for Opcoes<'_> {
    fn default() -> Self {
        Opcoes {
            situacao: Situacao::Valida,
            canhoto: true,
            municipio: None,
        }
    }
}

impl fmt::Debug for Opcoes<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Opcoes")
            .field("situacao", &self.situacao)
            .field("canhoto", &self.canhoto)
            .field("municipio", &self.municipio.map(|_| "<resolvedor>"))
            .finish()
    }
}

/// O JSON do DANFSe: o conteudo, o QR (`qr`), o leiaute (topos e linhas) e a pagina (operacoes de desenho em cm).
///
/// # Errors
///
/// [`ErroDanfse`] quando o XML nao e' uma NFS-e legivel.
pub fn dados(xml_nfse: &str, op: &Opcoes) -> Result<Value, ErroDanfse> {
    let mut c = conteudo::conteudo(xml_nfse, op)?;
    c["canhoto"] = json!(op.canhoto);
    c["qr"] = qr::dados(medidas::s(&c["acesso"]["qr_url"]))?;
    let l = leiaute::leiaute(&c, op.canhoto);
    c["pagina"] = Value::Array(pagina::operacoes(&c, &l, op.canhoto));
    c["leiaute"] = l;
    Ok(c)
}

/// O DANFSe em PDF (A4 retrato, uma pagina). Sincrono e pesado em CPU: dentro de um servidor assincrono, rode em
/// uma tarefa propria para bloqueio.
///
/// # Errors
///
/// [`ErroDanfse`] como em [`dados`], ou [`ErroDanfse::Pdf`] se o Typst falhar.
pub fn pdf(xml_nfse: &str, op: &Opcoes) -> Result<Vec<u8>, ErroDanfse> {
    compilar::pdf(&dados(xml_nfse, op)?)
}
