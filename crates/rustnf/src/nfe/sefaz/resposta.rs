// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Leitura das RESPOSTAS dos webservices (SOAP 1.2) e montagem do `nfeProc` com o texto que a SEFAZ mandou. Parte
//! pura: nada aqui faz rede.
//!
//! O retorno (`retEnviNFe`, `retConsReciNFe`, `retConsSitNFe`, `retConsStatServ`) e o `protNFe` sao recortados do texto
//! da resposta pelas posicoes que o analisador informa, nao reescritos: reserializar o `protNFe` trocaria o prefixo
//! de espaco de nomes e `<X></X>` por `<X/>`, e o documento guardado deixaria de ser o que a SEFAZ assinou. So' o que
//! o recorte herdava do envelope (um `xmlns` ou `xmlns:prefixo` declarado fora dele) e' acrescentado na abertura do
//! elemento, e o escopo de nomes continua o mesmo.

use std::fmt;

use super::pedidos::{sem_declaracao, VERSAO};
use crate::nfe::NS;

const SOAP12: &str = "http://www.w3.org/2003/05/soap-envelope";
const SOAP11: &str = "http://schemas.xmlsoap.org/soap/envelope/";

type No<'a, 'i> = roxmltree::Node<'a, 'i>;

/// Por que a resposta nao e' um retorno da SEFAZ.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErroResposta {
    /// Nao e' XML (HTML de proxy, resposta vazia, XML quebrado). Traz o comeco do texto, so' para o registro.
    NaoEXml(String),
    /// A SEFAZ respondeu com uma falha SOAP; traz o motivo que ela deu.
    Fault(String),
    /// O `Body` nao traz nenhum retorno da NF-e (`ret*`).
    SemRetorno,
    /// Veio outro retorno que o esperado (por exemplo `retConsSitNFe` onde se esperava `retEnviNFe`).
    OutroRetorno {
        /// O elemento esperado.
        esperado: &'static str,
        /// O que veio.
        veio: String,
    },
    /// O retorno veio com uma estrutura que o leiaute nao permite.
    Invalida(String),
}

impl fmt::Display for ErroResposta {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NaoEXml(_) => {
                f.write_str("a resposta nao e' um XML da SEFAZ (pagina de erro ou resposta vazia)")
            }
            Self::Fault(m) => write!(f, "a SEFAZ respondeu com uma falha SOAP: {m}"),
            Self::SemRetorno => f.write_str("a resposta da SEFAZ nao traz o retorno da NF-e"),
            Self::OutroRetorno { esperado, veio } => {
                write!(f, "a SEFAZ devolveu {veio} onde se esperava {esperado}")
            }
            Self::Invalida(m) => write!(f, "retorno da SEFAZ fora do leiaute: {m}"),
        }
    }
}

impl std::error::Error for ErroResposta {}

pub(super) fn corta(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// O retorno de dentro do `Body`, no texto da SEFAZ.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Retorno {
    /// O nome do elemento (`retEnviNFe`, `retConsSitNFe`...).
    pub raiz: String,
    /// O elemento como a SEFAZ o mandou (com os `xmlns` herdados do envelope na abertura, se houver).
    pub texto: String,
}

/// Tira o retorno (`ret*` do espaco de nomes da NF-e) de dentro do `Body` SOAP.
///
/// # Errors
///
/// [`ErroResposta`]: nao e' XML, e' uma falha SOAP, ou nao tem retorno.
pub fn desembrulhar(corpo: &[u8]) -> Result<Retorno, ErroResposta> {
    let texto = String::from_utf8_lossy(corpo);
    // as posicoes dos elementos valem para o texto que foi analisado: o mesmo de onde sai o recorte
    let xml = sem_declaracao(&texto);
    let doc =
        roxmltree::Document::parse(xml).map_err(|_| ErroResposta::NaoEXml(corta(&texto, 120)))?;
    let e_soap = |n: &No<'_, '_>, nome: &str| {
        n.is_element()
            && n.tag_name().name() == nome
            && matches!(n.tag_name().namespace(), Some(SOAP12 | SOAP11))
    };
    if let Some(f) = doc.descendants().find(|n| e_soap(n, "Fault")) {
        let motivo = f
            .descendants()
            .filter(|n| n.is_element() && matches!(n.tag_name().name(), "Text" | "faultstring"))
            .filter_map(|n| n.text())
            .map(str::trim)
            .collect::<Vec<_>>()
            .join(" ");
        return Err(ErroResposta::Fault(corta(&motivo, 200)));
    }
    let body = doc
        .descendants()
        .find(|n| e_soap(n, "Body"))
        .ok_or(ErroResposta::SemRetorno)?;
    let alvo = body.descendants().skip(1).find(|n| {
        n.is_element()
            && n.tag_name().name().starts_with("ret")
            && n.tag_name().namespace() == Some(NS)
    });
    match alvo {
        Some(a) => Ok(Retorno {
            raiz: a.tag_name().name().to_string(),
            texto: recortar(a, xml)?,
        }),
        None => retorno_escapado(body),
    }
}

/// Algumas SEFAZ devolvem o retorno como TEXTO dentro do `nfeResultMsg` (o XML escapado, `&lt;retConsSitNFe ...`) e nao
/// como elemento. O texto, ja desescapado pelo analisador, e' o retorno: tem de ser um XML cuja raiz e' um `ret*` do
/// espaco de nomes da NF-e, e e' devolvido como veio.
fn retorno_escapado(body: No<'_, '_>) -> Result<Retorno, ErroResposta> {
    let msg = body
        .descendants()
        .find(|n| n.is_element() && n.tag_name().name() == "nfeResultMsg")
        .ok_or(ErroResposta::SemRetorno)?;
    let texto: String = msg
        .children()
        .filter(|c| c.is_text())
        .filter_map(|c| c.text())
        .collect();
    let interno = sem_declaracao(&texto);
    if interno.is_empty() {
        return Err(ErroResposta::SemRetorno);
    }
    let doc = roxmltree::Document::parse(interno)
        .map_err(|_| ErroResposta::NaoEXml(corta(interno, 120)))?;
    let raiz = doc.root_element();
    if !raiz.tag_name().name().starts_with("ret") || raiz.tag_name().namespace() != Some(NS) {
        return Err(ErroResposta::SemRetorno);
    }
    Ok(Retorno {
        raiz: raiz.tag_name().name().to_string(),
        texto: interno.to_string(),
    })
}

/// O fim da abertura do elemento (o `>` que fecha a tag, fora de aspas).
pub(super) fn fim_da_abertura(texto: &str, inicio: usize) -> Option<usize> {
    let mut aspas: Option<u8> = None;
    for (i, b) in texto.as_bytes()[inicio..].iter().enumerate() {
        match (aspas, *b) {
            (Some(a), c) if c == a => aspas = None,
            (Some(_), _) => {}
            (None, b'"' | b'\'') => aspas = Some(*b),
            (None, b'>') => return Some(inicio + i),
            _ => {}
        }
    }
    None
}

/// Os nomes `xmlns` e `xmlns:p` que a propria abertura declara.
fn declarados(abertura: &str) -> Vec<String> {
    let mut v = Vec::new();
    let b = abertura.as_bytes();
    let mut i = 1;
    // pula o nome do elemento
    while i < b.len() && !b[i].is_ascii_whitespace() && b[i] != b'>' && b[i] != b'/' {
        i += 1;
    }
    while i < b.len() {
        while i < b.len() && (b[i].is_ascii_whitespace() || b[i] == b'/') {
            i += 1;
        }
        if i >= b.len() || b[i] == b'>' {
            break;
        }
        let ini = i;
        while i < b.len() && !b[i].is_ascii_whitespace() && !matches!(b[i], b'=' | b'>' | b'/') {
            i += 1;
        }
        let nome = &abertura[ini..i];
        while i < b.len() && (b[i].is_ascii_whitespace() || b[i] == b'=') {
            i += 1;
        }
        if i < b.len() && matches!(b[i], b'"' | b'\'') {
            let q = b[i];
            i += 1;
            while i < b.len() && b[i] != q {
                i += 1;
            }
            i += 1;
        }
        if nome == "xmlns" || nome.starts_with("xmlns:") {
            v.push(nome.to_string());
        }
    }
    v
}

/// O recorte usa o prefixo `p` (em tag, atributo ou valor de tipo)? Declaracao herdada que ninguem usa nao se copia.
fn usa_prefixo(corpo: &str, p: &str) -> bool {
    let alvo = format!("{p}:");
    corpo.match_indices(&alvo).any(|(i, _)| {
        corpo[..i]
            .chars()
            .next_back()
            .is_some_and(|c| matches!(c, '<' | '/' | '"' | '\'') || c.is_whitespace())
    })
}

fn esc_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('"', "&quot;")
}

/// O elemento como esta no texto da resposta. Se ele herdava uma declaracao de espaco de nomes de fora dele, a
/// declaracao e' acrescentada na abertura; o padrao herdado que e' o da NF-e nao precisa ser repetido (o `nfeProc` e
/// o `ret*` tem o mesmo).
pub(crate) fn recortar(no: No<'_, '_>, texto: &str) -> Result<String, ErroResposta> {
    let r = no.range();
    let fim_ab = fim_da_abertura(texto, r.start)
        .ok_or_else(|| ErroResposta::Invalida("abertura do elemento sem fim".into()))?;
    let abertura = &texto[r.start..=fim_ab];
    let ja = declarados(abertura);
    let prefixado = abertura[1..]
        .split([' ', '\t', '\r', '\n', '>', '/'])
        .next()
        .is_some_and(|n| n.contains(':'));
    let mut extra = String::new();
    for ns in no.namespaces() {
        match ns.name() {
            Some("xml") => {}
            Some(p) => {
                if !ja.iter().any(|d| d.strip_prefix("xmlns:") == Some(p))
                    && usa_prefixo(&texto[r.clone()], p)
                {
                    extra.push_str(&format!(" xmlns:{p}=\"{}\"", esc_attr(ns.uri())));
                }
            }
            None => {
                if ja.iter().any(|d| d == "xmlns") || ns.uri() == NS {
                    continue;
                }
                if !prefixado {
                    return Err(ErroResposta::Invalida(
                        "elemento em espaco de nomes que nao e' o da NF-e".into(),
                    ));
                }
                extra.push_str(&format!(" xmlns=\"{}\"", esc_attr(ns.uri())));
            }
        }
    }
    let corpo = &texto[r];
    if extra.is_empty() {
        return Ok(corpo.to_string());
    }
    let fim_nome = corpo[1..]
        .find(|c: char| c.is_whitespace() || c == '>' || c == '/')
        .map_or(corpo.len(), |i| i + 1);
    Ok(format!(
        "{}{extra}{}",
        &corpo[..fim_nome],
        &corpo[fim_nome..]
    ))
}

// ------------------------------------------------------------------------------------------------ leitura tipada

pub(super) fn texto_filho(pai: No<'_, '_>, nome: &str) -> String {
    pai.children()
        .find(|c| {
            c.is_element() && c.tag_name().name() == nome && c.tag_name().namespace() == Some(NS)
        })
        .and_then(|c| c.text())
        .unwrap_or("")
        .trim()
        .to_string()
}

pub(super) fn filhos<'a, 'i>(pai: No<'a, 'i>, nome: &'a str) -> impl Iterator<Item = No<'a, 'i>> {
    pai.children().filter(move |c| {
        c.is_element() && c.tag_name().name() == nome && c.tag_name().namespace() == Some(NS)
    })
}

fn segundos(s: &str) -> Option<u32> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

pub(super) fn abrir<'t>(
    texto: &'t str,
    esperado: &'static str,
) -> Result<roxmltree::Document<'t>, ErroResposta> {
    let doc =
        roxmltree::Document::parse(texto).map_err(|_| ErroResposta::NaoEXml(corta(texto, 120)))?;
    let raiz = doc.root_element();
    if raiz.tag_name().name() != esperado || raiz.tag_name().namespace() != Some(NS) {
        return Err(ErroResposta::OutroRetorno {
            esperado,
            veio: raiz.tag_name().name().to_string(),
        });
    }
    Ok(doc)
}

/// A parte que todo retorno traz.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Cabecalho {
    /// `tpAmb`.
    pub tp_amb: String,
    /// `verAplic`: a versao do aplicativo da SEFAZ que respondeu.
    pub ver_aplic: String,
    /// `cStat`.
    pub cstat: String,
    /// `xMotivo`.
    pub motivo: String,
    /// `cUF`: a UF que atendeu.
    pub cuf: String,
    /// `dhRecbto`, como veio.
    pub data: String,
}

fn cabecalho(raiz: No<'_, '_>) -> Cabecalho {
    Cabecalho {
        tp_amb: texto_filho(raiz, "tpAmb"),
        ver_aplic: texto_filho(raiz, "verAplic"),
        cstat: texto_filho(raiz, "cStat"),
        motivo: texto_filho(raiz, "xMotivo"),
        cuf: texto_filho(raiz, "cUF"),
        data: texto_filho(raiz, "dhRecbto"),
    }
}

/// O protocolo (`protNFe`) de uma nota: o que se le dele e o elemento cru.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtNfe {
    /// `infProt/chNFe`.
    pub chave: String,
    /// `infProt/tpAmb`.
    pub tp_amb: String,
    /// `infProt/nProt`: o numero do protocolo (vazio na rejeicao).
    pub numero: String,
    /// `infProt/digVal`.
    pub digest: String,
    /// `infProt/cStat`.
    pub cstat: String,
    /// `infProt/xMotivo`.
    pub motivo: String,
    /// `infProt/dhRecbto`, como veio.
    pub data: String,
    /// O `protNFe` como a SEFAZ o mandou: e' o que vai dentro do `nfeProc`.
    pub cru: String,
}

fn protocolo(no: No<'_, '_>, texto: &str) -> Result<ProtNfe, ErroResposta> {
    let inf = no
        .children()
        .find(|c| {
            c.is_element()
                && c.tag_name().name() == "infProt"
                && c.tag_name().namespace() == Some(NS)
        })
        .ok_or_else(|| ErroResposta::Invalida("protNFe sem infProt".into()))?;
    Ok(ProtNfe {
        chave: texto_filho(inf, "chNFe"),
        tp_amb: texto_filho(inf, "tpAmb"),
        numero: texto_filho(inf, "nProt"),
        digest: texto_filho(inf, "digVal"),
        cstat: texto_filho(inf, "cStat"),
        motivo: texto_filho(inf, "xMotivo"),
        data: texto_filho(inf, "dhRecbto"),
        cru: recortar(no, texto)?,
    })
}

fn protocolos(raiz: No<'_, '_>, texto: &str) -> Result<Vec<ProtNfe>, ErroResposta> {
    filhos(raiz, "protNFe")
        .map(|p| protocolo(p, texto))
        .collect()
}

/// `retConsStatServ`: o status do servico.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetStatus {
    /// `tpAmb`, `verAplic`, `cStat`, `xMotivo`, `cUF` e `dhRecbto`.
    pub cab: Cabecalho,
    /// `tMed`: o tempo medio de resposta, em segundos.
    pub tempo_medio: Option<u32>,
    /// `dhRetorno`: quando o servico deve voltar.
    pub retorno_previsto: String,
    /// `xObs`.
    pub obs: String,
}

/// Le o `retConsStatServ`.
///
/// # Errors
///
/// [`ErroResposta`] se o texto nao e' esse retorno.
pub fn ler_status(texto: &str) -> Result<RetStatus, ErroResposta> {
    let doc = abrir(texto, "retConsStatServ")?;
    let r = doc.root_element();
    Ok(RetStatus {
        cab: cabecalho(r),
        tempo_medio: segundos(&texto_filho(r, "tMed")),
        retorno_previsto: texto_filho(r, "dhRetorno"),
        obs: texto_filho(r, "xObs"),
    })
}

/// O recibo do lote assincrono (`infRec`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recibo {
    /// `nRec`: 15 digitos.
    pub numero: String,
    /// `tMed`: o tempo medio de resposta, em segundos.
    pub tempo_medio: Option<u32>,
}

/// `retEnviNFe`: o resultado do envio do lote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetEnvio {
    /// O cabecalho do retorno (`cStat` do lote: 103, 104, 108, 109 ou uma rejeicao).
    pub cab: Cabecalho,
    /// `infRec`, so' no assincrono com o lote aceito.
    pub recibo: Option<Recibo>,
    /// Os `protNFe` do processamento sincrono.
    pub protocolos: Vec<ProtNfe>,
}

/// Le o `retEnviNFe`.
///
/// # Errors
///
/// [`ErroResposta`] se o texto nao e' esse retorno.
pub fn ler_envio(texto: &str) -> Result<RetEnvio, ErroResposta> {
    let doc = abrir(texto, "retEnviNFe")?;
    let r = doc.root_element();
    let recibo = filhos(r, "infRec").next().map(|i| Recibo {
        numero: texto_filho(i, "nRec"),
        tempo_medio: segundos(&texto_filho(i, "tMed")),
    });
    Ok(RetEnvio {
        cab: cabecalho(r),
        recibo,
        protocolos: protocolos(r, texto)?,
    })
}

/// `retConsReciNFe`: o resultado do lote pelo recibo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetRecibo {
    /// O cabecalho (`cStat` do lote: 104 processado, 105 em processamento, 106 nao localizado...).
    pub cab: Cabecalho,
    /// `nRec`: o recibo consultado.
    pub recibo: String,
    /// Os `protNFe` de cada nota do lote.
    pub protocolos: Vec<ProtNfe>,
}

/// Le o `retConsReciNFe`.
///
/// # Errors
///
/// [`ErroResposta`] se o texto nao e' esse retorno.
pub fn ler_recibo(texto: &str) -> Result<RetRecibo, ErroResposta> {
    let doc = abrir(texto, "retConsReciNFe")?;
    let r = doc.root_element();
    Ok(RetRecibo {
        cab: cabecalho(r),
        recibo: texto_filho(r, "nRec"),
        protocolos: protocolos(r, texto)?,
    })
}

/// `retConsSitNFe`: a situacao da nota pela chave.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetConsulta {
    /// O cabecalho (`cStat`: 100, 150, 110, 101, 151, 217...).
    pub cab: Cabecalho,
    /// `chNFe`: a chave consultada.
    pub chave: String,
    /// O `protNFe` da autorizacao ou da denegacao, se a SEFAZ o mandou.
    pub protocolo: Option<ProtNfe>,
    /// O `procEventoNFe` de cada evento da nota, crus (cancelamento, CC-e, EPEC).
    pub eventos: Vec<String>,
}

/// Le o `retConsSitNFe`.
///
/// # Errors
///
/// [`ErroResposta`] se o texto nao e' esse retorno.
pub fn ler_consulta(texto: &str) -> Result<RetConsulta, ErroResposta> {
    let doc = abrir(texto, "retConsSitNFe")?;
    let r = doc.root_element();
    let eventos = filhos(r, "procEventoNFe")
        .map(|e| recortar(e, texto))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(RetConsulta {
        cab: cabecalho(r),
        chave: texto_filho(r, "chNFe"),
        protocolo: protocolos(r, texto)?.into_iter().next(),
        eventos,
    })
}

/// O `nfeProc`: a NF-e como foi enviada, sem tocar num byte, mais o `protNFe` como a SEFAZ o mandou.
#[must_use]
pub fn nfe_proc(nfe_assinada: &str, prot_cru: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><nfeProc xmlns=\"{NS}\" versao=\"{VERSAO}\">{}{prot_cru}</nfeProc>",
        sem_declaracao(nfe_assinada)
    )
}
