// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Os documentos da distribuicao: cada `docZip` e' aberto (base64, gzip com LIMITE de tamanho) e lido pelo que o seu
//! esquema diz ser.
//!
//! Fonte: NT 2014.002 v1.02d, itens 3.2 e 3.3 (cada documento vai compactado em gzip, em base64, com o NSU e o nome do
//! esquema que o descreve) e itens 3.11.1 e 3.11.2 (os dois resumos); esquemas `resNFe_v1.01.xsd` e
//! `resEvento_v1.01.xsd` do pacote de liberacao da distribuicao v1.04.
//!
//! Nada se descarta: o que a biblioteca nao le (esquema desconhecido, XML que nao passa na leitura, `docZip` que nao
//! abre) volta no mesmo [`Doc`], com o texto cru e o motivo, para a aplicacao guardar o NSU e reler depois.

use std::io::Read;

use base64::engine::{general_purpose::GeneralPurpose, DecodePaddingMode, GeneralPurposeConfig};
use base64::Engine;

use super::super::pedidos::sem_declaracao;
use super::super::resposta_evento::{ler_proc_evento, ProcEvento};
use super::cursor::Nsu;
use crate::chave_dfe;
use crate::nfe::decimal::{DataHora, Decimal};
use crate::nfe::erro::ErroLeitura;
use crate::nfe::xml::{
    abrir, corta, data_hora, decimal, inteiro, nome, t, texto_xml, LIMITE_BYTES,
};
use crate::nfe::{ler, Nfe};

/// A situacao da NF-e no resumo (`cSitNFe`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SituacaoResumo {
    /// 1: uso autorizado.
    Autorizada,
    /// 2: uso denegado.
    Denegada,
    /// 3: NF-e cancelada.
    Cancelada,
    /// Outro valor (o texto como veio).
    Outra(String),
}

impl SituacaoResumo {
    fn do_codigo(c: &str) -> SituacaoResumo {
        match c {
            "1" => SituacaoResumo::Autorizada,
            "2" => SituacaoResumo::Denegada,
            "3" => SituacaoResumo::Cancelada,
            outro => SituacaoResumo::Outra(corta(outro, 10)),
        }
    }
}

/// `resNFe`: o resumo que o destinatario recebe ANTES de manifestar (NT 2014.002 v1.02d, item 3.11.1).
#[derive(Debug, Clone, PartialEq)]
pub struct ResumoNfe {
    /// `chNFe`.
    pub chave: String,
    /// `CNPJ` ou `CPF` do emitente.
    pub emitente: String,
    /// `xNome` do emitente.
    pub nome: String,
    /// `IE` do emitente (vazio, `ISENTO` ou a inscricao).
    pub ie: String,
    /// `dhEmi`.
    pub emissao: Option<DataHora>,
    /// `tpNF`: 0 entrada, 1 saida.
    pub tipo_nf: String,
    /// `vNF`.
    pub valor: Option<Decimal>,
    /// `digVal`.
    pub digest: String,
    /// `dhRecbto`: a autorizacao.
    pub recebido_em: Option<DataHora>,
    /// `nProt`.
    pub protocolo: String,
    /// `cSitNFe`.
    pub situacao: SituacaoResumo,
}

/// `resEvento`: o resumo de um evento (NT 2014.002 v1.02d, item 3.11.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumoEvento {
    /// `cOrgao` (91 = Ambiente Nacional).
    pub orgao: String,
    /// `CNPJ` ou `CPF` do emitente da NF-e.
    pub emitente: String,
    /// `chNFe`.
    pub chave: String,
    /// `dhEvento`.
    pub emitido_em: Option<DataHora>,
    /// `tpEvento`.
    pub tp_evento: String,
    /// `nSeqEvento`.
    pub seq: Option<u64>,
    /// `xEvento`.
    pub descricao: String,
    /// `dhRecbto`: o registro do evento.
    pub recebido_em: Option<DataHora>,
    /// `nProt` do evento.
    pub protocolo: String,
}

/// O que um documento da distribuicao e', depois de aberto e lido.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Conteudo {
    /// `resNFe`: o destinatario ainda nao manifestou, so' recebe o resumo.
    ResumoNfe(ResumoNfe),
    /// `procNFe`: a NF-e completa, lida pelo leitor da NF-e.
    Nfe(Box<Nfe>),
    /// `resEvento`.
    ResumoEvento(ResumoEvento),
    /// `procEventoNFe`: o evento completo, com o registro da SEFAZ.
    Evento(Box<ProcEvento>),
    /// O esquema nao e' um dos quatro que a biblioteca le: o XML esta em [`Doc::xml`], cru.
    OutroEsquema,
    /// O documento nao abriu ou nao passou na leitura; o motivo e' para o registro. O NSU e o texto (se houve) estao no
    /// [`Doc`] e `consNSU` rele.
    Ilegivel {
        /// Por que.
        motivo: String,
    },
}

/// Um documento da distribuicao.
#[derive(Debug, Clone, PartialEq)]
pub struct Doc {
    /// O NSU do documento.
    pub nsu: Nsu,
    /// O atributo `schema` do `docZip` (por exemplo `procNFe_v4.00.xsd`).
    pub esquema: String,
    /// O XML INTEIRO como o Ambiente Nacional mandou (nunca cortado). Vazio so' se o `docZip` nao abriu.
    pub xml: String,
    /// O que o documento e'.
    pub conteudo: Conteudo,
    /// O `docZip` em base64 como veio, SO' quando ele nao abriu (para nada se perder).
    pub doczip_original: Option<String>,
}

impl Doc {
    fn ilegivel(
        nsu: Nsu,
        esquema: &str,
        xml: String,
        motivo: String,
        base64: Option<String>,
    ) -> Doc {
        Doc {
            nsu,
            esquema: esquema.to_string(),
            xml,
            conteudo: Conteudo::Ilegivel { motivo },
            doczip_original: base64,
        }
    }

    /// A chave da NF-e do documento ("" se ele nao foi lido ou nao tem uma so').
    #[must_use]
    pub fn chave(&self) -> &str {
        match &self.conteudo {
            Conteudo::ResumoNfe(r) => &r.chave,
            Conteudo::Nfe(n) => &n.chave,
            Conteudo::ResumoEvento(r) => &r.chave,
            Conteudo::Evento(e) => &e.chave,
            Conteudo::OutroEsquema | Conteudo::Ilegivel { .. } => "",
        }
    }

    /// O documento nao abriu ou nao foi lido.
    #[must_use]
    pub fn e_ilegivel(&self) -> bool {
        matches!(self.conteudo, Conteudo::Ilegivel { .. })
    }
}

/// Gunzip com teto na SAIDA (bomba de descompressao): passou do `limite`, e' erro, e o excesso nunca vai a memoria.
pub(crate) fn gunzip_limitado(dados: &[u8], limite: usize) -> Result<Vec<u8>, String> {
    let mut saida = Vec::new();
    let mut d = flate2::read::GzDecoder::new(dados).take(limite as u64 + 1);
    d.read_to_end(&mut saida)
        .map_err(|e| format!("gzip ilegivel: {e}"))?;
    if saida.len() > limite {
        return Err(format!("o docZip descomprimido passa de {limite} bytes"));
    }
    Ok(saida)
}

fn base64_do_docszip(s: &str) -> Result<Vec<u8>, String> {
    let limpo: String = s.split_whitespace().collect();
    let motor = GeneralPurpose::new(
        &base64::alphabet::STANDARD,
        GeneralPurposeConfig::new()
            .with_decode_padding_mode(DecodePaddingMode::RequireCanonical)
            .with_decode_allow_trailing_bits(true),
    );
    motor
        .decode(limpo)
        .map_err(|e| format!("base64 ilegivel: {e}"))
}

/// Abre o `docZip` (base64 -> gzip -> XML) e o le conforme o esquema. Nunca falha: o que nao abre ou nao le vira
/// [`Conteudo::Ilegivel`], no mesmo documento.
pub(crate) fn abrir_doc(nsu: Nsu, esquema: &str, base64: &str) -> Doc {
    let bytes = match base64_do_docszip(base64).and_then(|b| gunzip_limitado(&b, LIMITE_BYTES)) {
        Ok(b) => b,
        Err(m) => {
            return Doc::ilegivel(
                nsu,
                esquema,
                String::new(),
                m,
                Some(base64.trim().to_string()),
            );
        }
    };
    let xml = match String::from_utf8(bytes) {
        Ok(x) => x,
        Err(_) => {
            return Doc::ilegivel(
                nsu,
                esquema,
                String::new(),
                "o documento nao esta em UTF-8".into(),
                Some(base64.trim().to_string()),
            );
        }
    };
    let baixo = esquema.to_lowercase();
    let lido = if baixo.starts_with("resnfe") {
        Some(ler_resumo_nfe(&xml).map(Conteudo::ResumoNfe))
    } else if baixo.starts_with("procnfe") || baixo.starts_with("nfeproc") {
        Some(ler(sem_declaracao(&xml).as_bytes()).map(|n| Conteudo::Nfe(Box::new(n))))
    } else if baixo.starts_with("resevento") {
        Some(ler_resumo_evento(&xml).map(Conteudo::ResumoEvento))
    } else if baixo.starts_with("proceventonfe") {
        return Doc {
            nsu,
            esquema: esquema.to_string(),
            conteudo: match ler_proc_evento(sem_declaracao(&xml)) {
                Ok(e) => Conteudo::Evento(Box::new(e)),
                Err(e) => Conteudo::Ilegivel {
                    motivo: format!("{esquema}: {e}"),
                },
            },
            xml,
            doczip_original: None,
        };
    } else {
        None
    };
    let conteudo = match lido {
        None => Conteudo::OutroEsquema,
        Some(Ok(c)) => c,
        Some(Err(e)) => Conteudo::Ilegivel {
            motivo: format!("{esquema}: {e}"),
        },
    };
    Doc {
        nsu,
        esquema: esquema.to_string(),
        xml,
        conteudo,
        doczip_original: None,
    }
}

fn emitente_do(n: Option<roxmltree::Node<'_, '_>>) -> String {
    ["CNPJ", "CPF"]
        .iter()
        .map(|tag| t(n, tag))
        .find(|v| !v.is_empty())
        .unwrap_or_default()
        .to_uppercase()
}

fn raiz_de<'a>(texto: &'a str, esperada: &str) -> Result<roxmltree::Document<'a>, ErroLeitura> {
    let d = abrir(texto)?;
    let raiz = d.root_element();
    if nome(&raiz) != esperada {
        return Err(ErroLeitura::OutroDocumento {
            raiz: corta(nome(&raiz), 40),
        });
    }
    Ok(d)
}

/// Le o `resNFe`.
///
/// # Errors
///
/// [`ErroLeitura`] se o texto nao e' um `resNFe` ou um campo esta fora do formato do leiaute.
pub(crate) fn ler_resumo_nfe(xml: &str) -> Result<ResumoNfe, ErroLeitura> {
    let texto = texto_xml(xml.as_bytes())?;
    let d = raiz_de(&texto, "resNFe")?;
    let o = Some(d.root_element());
    let chave = chave_dfe::limpar(&t(o, "chNFe"));
    if chave.is_empty() {
        return Err(ErroLeitura::GrupoAusente("chNFe"));
    }
    Ok(ResumoNfe {
        chave,
        emitente: emitente_do(o),
        nome: t(o, "xNome"),
        ie: t(o, "IE"),
        emissao: data_hora(o, "dhEmi")?,
        tipo_nf: t(o, "tpNF"),
        valor: decimal(o, "vNF")?,
        digest: t(o, "digVal"),
        recebido_em: data_hora(o, "dhRecbto")?,
        protocolo: t(o, "nProt"),
        situacao: SituacaoResumo::do_codigo(&t(o, "cSitNFe")),
    })
}

/// Le o `resEvento`.
///
/// # Errors
///
/// [`ErroLeitura`] se o texto nao e' um `resEvento` ou um campo esta fora do formato do leiaute.
pub(crate) fn ler_resumo_evento(xml: &str) -> Result<ResumoEvento, ErroLeitura> {
    let texto = texto_xml(xml.as_bytes())?;
    let d = raiz_de(&texto, "resEvento")?;
    let o = Some(d.root_element());
    let chave = chave_dfe::limpar(&t(o, "chNFe"));
    if chave.is_empty() {
        return Err(ErroLeitura::GrupoAusente("chNFe"));
    }
    Ok(ResumoEvento {
        orgao: t(o, "cOrgao"),
        emitente: emitente_do(o),
        chave,
        emitido_em: data_hora(o, "dhEvento")?,
        tp_evento: t(o, "tpEvento"),
        seq: inteiro(o, "nSeqEvento")?,
        descricao: t(o, "xEvento"),
        recebido_em: data_hora(o, "dhRecbto")?,
        protocolo: t(o, "nProt"),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod testes {
    use super::*;
    use flate2::{write::GzEncoder, Compression};
    use std::io::Write;

    fn gz(dados: &[u8]) -> Vec<u8> {
        let mut e = GzEncoder::new(Vec::new(), Compression::default());
        e.write_all(dados).unwrap();
        e.finish().unwrap()
    }

    #[test]
    fn a_bomba_de_descompressao_para_no_limite() {
        let grande = vec![b'a'; 5000];
        assert_eq!(gunzip_limitado(&gz(&grande), 5000).unwrap().len(), 5000);
        let e = gunzip_limitado(&gz(&grande), 4999).unwrap_err();
        assert!(e.contains("passa de 4999"), "{e}");
        assert!(gunzip_limitado(b"isto nao e gzip", 100).is_err());
    }

    #[test]
    fn docs_que_nao_abrem_voltam_com_o_texto_original() {
        let d = abrir_doc(Nsu::ler("7").unwrap(), "resNFe_v1.01.xsd", "@@@");
        assert!(d.e_ilegivel());
        assert_eq!(d.doczip_original.as_deref(), Some("@@@"));
        assert!(d.xml.is_empty());
    }
}
