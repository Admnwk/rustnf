// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Leitura das respostas dos eventos (`retEnvEvento`) e da inutilizacao (`retInutNFe`), e a montagem do
//! `procEventoNFe` e do `ProcInutNFe` com o texto que a SEFAZ mandou. Parte pura: nada aqui faz rede.
//!
//! Como o `nfeProc`, os dois documentos de distribuicao saem do TEXTO CRU: o evento (ou a inutilizacao) como foi
//! enviado, assinado, e o `retEvento` (ou o `retInutNFe`) como a SEFAZ o devolveu, sem reserializar. Fonte: MOC 7.00,
//! Visao Geral, 5.8.2 (tabela 5-33: `retEnvEvento` e `retEvento`), 5.8.6 (tabela 5-36: `procEventoNFe`) e 5.3.2
//! (tabela 5-10: `retInutNFe`).

use roxmltree::Node;

use super::pedidos::{sem_declaracao, VERSAO};
use super::resposta::{abrir, filhos, fim_da_abertura, recortar, texto_filho, ErroResposta};
use crate::nfe::NS;

/// Versao do leiaute do evento.
const VERSAO_EVENTO: &str = "1.00";

/// O resultado de UM evento do lote (`retEvento/infEvento`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InfRetEvento {
    /// `tpAmb`.
    pub tp_amb: String,
    /// `verAplic`: o aplicativo da SEFAZ que registrou.
    pub ver_aplic: String,
    /// `cOrgao`.
    pub c_orgao: String,
    /// `cStat` do evento: 135, 136, 155 ou uma rejeicao.
    pub cstat: String,
    /// `xMotivo`.
    pub motivo: String,
    /// `chNFe` (vazio se a SEFAZ nao o mandou).
    pub chave: String,
    /// `tpEvento`.
    pub tp_evento: String,
    /// `xEvento`.
    pub x_evento: String,
    /// `nSeqEvento`.
    pub seq: Option<u32>,
    /// `dhRegEvento`, como veio.
    pub data: String,
    /// `nProt` do evento (vazio na rejeicao).
    pub protocolo: String,
    /// `chNFePend`: os EPEC pendentes de conciliacao (rejeicao 142).
    pub pendentes: Vec<String>,
    /// O `retEvento` como a SEFAZ o mandou.
    pub cru: String,
}

/// `retEnvEvento`: o lote de eventos processado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetEnvEvento {
    /// `tpAmb`.
    pub tp_amb: String,
    /// `verAplic`.
    pub ver_aplic: String,
    /// `cOrgao`.
    pub c_orgao: String,
    /// `cStat` do lote: 128 (lote processado) ou a rejeicao do lote.
    pub cstat: String,
    /// `xMotivo` do lote.
    pub motivo: String,
    /// `idLote`.
    pub id_lote: String,
    /// Os `retEvento`, na ordem em que vieram.
    pub eventos: Vec<InfRetEvento>,
}

fn evento_de(no: Node<'_, '_>, texto: &str) -> Result<InfRetEvento, ErroResposta> {
    let inf = no
        .children()
        .find(|c| {
            c.is_element()
                && c.tag_name().name() == "infEvento"
                && c.tag_name().namespace() == Some(NS)
        })
        .ok_or_else(|| ErroResposta::Invalida("retEvento sem infEvento".into()))?;
    Ok(InfRetEvento {
        tp_amb: texto_filho(inf, "tpAmb"),
        ver_aplic: texto_filho(inf, "verAplic"),
        c_orgao: texto_filho(inf, "cOrgao"),
        cstat: texto_filho(inf, "cStat"),
        motivo: texto_filho(inf, "xMotivo"),
        chave: texto_filho(inf, "chNFe"),
        tp_evento: texto_filho(inf, "tpEvento"),
        x_evento: texto_filho(inf, "xEvento"),
        seq: texto_filho(inf, "nSeqEvento").parse().ok(),
        data: texto_filho(inf, "dhRegEvento"),
        protocolo: texto_filho(inf, "nProt"),
        pendentes: filhos(inf, "chNFePend")
            .filter_map(|n| n.text())
            .map(|t| t.trim().to_string())
            .collect(),
        cru: recortar(no, texto)?,
    })
}

/// Le o `retEnvEvento`.
///
/// # Errors
///
/// [`ErroResposta`] se o texto nao e' esse retorno.
pub fn ler_evento(texto: &str) -> Result<RetEnvEvento, ErroResposta> {
    let doc = abrir(texto, "retEnvEvento")?;
    let r = doc.root_element();
    let eventos = filhos(r, "retEvento")
        .map(|e| evento_de(e, texto))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(RetEnvEvento {
        tp_amb: texto_filho(r, "tpAmb"),
        ver_aplic: texto_filho(r, "verAplic"),
        c_orgao: texto_filho(r, "cOrgao"),
        cstat: texto_filho(r, "cStat"),
        motivo: texto_filho(r, "xMotivo"),
        id_lote: texto_filho(r, "idLote"),
        eventos,
    })
}

/// `retInutNFe`: o resultado da inutilizacao.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetInutilizacao {
    /// `tpAmb`.
    pub tp_amb: String,
    /// `verAplic`.
    pub ver_aplic: String,
    /// `cStat`: 102 (homologada), 563 (a faixa ja foi pedida), 998 (servico desativado) ou uma rejeicao.
    pub cstat: String,
    /// `xMotivo`.
    pub motivo: String,
    /// `cUF`.
    pub cuf: String,
    /// `ano` (2 digitos).
    pub ano: String,
    /// `CNPJ`.
    pub cnpj: String,
    /// `mod`.
    pub modelo: String,
    /// `serie`.
    pub serie: String,
    /// `nNFIni`.
    pub inicio: String,
    /// `nNFFin`.
    pub fim: String,
    /// `dhRecbto`, como veio.
    pub data: String,
    /// `nProt` (vazio na rejeicao; na 563 e' o da inutilizacao anterior).
    pub protocolo: String,
    /// O `retInutNFe` como a SEFAZ o mandou.
    pub cru: String,
}

/// Le o `retInutNFe`.
///
/// # Errors
///
/// [`ErroResposta`] se o texto nao e' esse retorno.
pub fn ler_inutilizacao(texto: &str) -> Result<RetInutilizacao, ErroResposta> {
    let doc = abrir(texto, "retInutNFe")?;
    let r = doc.root_element();
    let inf = r
        .children()
        .find(|c| {
            c.is_element()
                && c.tag_name().name() == "infInut"
                && c.tag_name().namespace() == Some(NS)
        })
        .ok_or_else(|| ErroResposta::Invalida("retInutNFe sem infInut".into()))?;
    Ok(RetInutilizacao {
        tp_amb: texto_filho(inf, "tpAmb"),
        ver_aplic: texto_filho(inf, "verAplic"),
        cstat: texto_filho(inf, "cStat"),
        motivo: texto_filho(inf, "xMotivo"),
        cuf: texto_filho(inf, "cUF"),
        ano: texto_filho(inf, "ano"),
        cnpj: texto_filho(inf, "CNPJ"),
        modelo: texto_filho(inf, "mod"),
        serie: texto_filho(inf, "serie"),
        inicio: texto_filho(inf, "nNFIni"),
        fim: texto_filho(inf, "nNFFin"),
        data: texto_filho(inf, "dhRecbto"),
        protocolo: texto_filho(inf, "nProt"),
        cru: recortar(r, texto)?,
    })
}

/// O `procEventoNFe`: o evento como foi enviado (assinado) mais o `retEvento` como a SEFAZ o mandou.
#[must_use]
pub fn proc_evento(evento_assinado: &str, ret_evento_cru: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><procEventoNFe xmlns=\"{NS}\" versao=\"{VERSAO_EVENTO}\">{}{ret_evento_cru}</procEventoNFe>",
        sem_declaracao(evento_assinado)
    )
}

/// O `ProcInutNFe` (o `P` maiusculo e' do XSD `procInutNFe_v4.00`): o `inutNFe` como foi enviado (assinado) mais o `retInutNFe` como a SEFAZ o mandou.
#[must_use]
pub fn proc_inutilizacao(inut_assinado: &str, ret_inut_cru: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><ProcInutNFe xmlns=\"{NS}\" versao=\"{VERSAO}\">{}{ret_inut_cru}</ProcInutNFe>",
        sem_declaracao(inut_assinado)
    )
}

/// O que se le de volta de um `procEventoNFe`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcEvento {
    /// `evento/infEvento/tpEvento`.
    pub tp_evento: String,
    /// `evento/infEvento/nSeqEvento`.
    pub seq: Option<u32>,
    /// `evento/infEvento/chNFe`.
    pub chave: String,
    /// `evento/infEvento/Id`.
    pub id: String,
    /// O registro da SEFAZ.
    pub registro: InfRetEvento,
    /// O `procEventoNFe` como esta no texto lido, com o `xmlns` da NF-e na abertura (pronto para guardar).
    pub cru: String,
}

/// Poe o `xmlns` da NF-e na abertura do elemento recortado, se ele o herdava de fora.
fn com_espaco_de_nomes(cru: String) -> String {
    let Some(fim) = fim_da_abertura(&cru, 0) else {
        return cru;
    };
    if cru[..=fim].contains(" xmlns=") {
        return cru;
    }
    let nome = cru[1..]
        .find(|c: char| c.is_whitespace() || c == '>' || c == '/')
        .map_or(cru.len(), |i| i + 1);
    format!("{} xmlns=\"{NS}\"{}", &cru[..nome], &cru[nome..])
}

fn proc_de(p: Node<'_, '_>, texto: &str) -> Result<ProcEvento, ErroResposta> {
    let ev = filhos(p, "evento")
        .next()
        .ok_or_else(|| ErroResposta::Invalida("procEventoNFe sem evento".into()))?;
    let inf = filhos(ev, "infEvento")
        .next()
        .ok_or_else(|| ErroResposta::Invalida("evento sem infEvento".into()))?;
    let ret = filhos(p, "retEvento")
        .next()
        .ok_or_else(|| ErroResposta::Invalida("procEventoNFe sem retEvento".into()))?;
    Ok(ProcEvento {
        tp_evento: texto_filho(inf, "tpEvento"),
        seq: texto_filho(inf, "nSeqEvento").parse().ok(),
        chave: texto_filho(inf, "chNFe"),
        id: inf.attribute("Id").unwrap_or("").to_string(),
        registro: evento_de(ret, texto)?,
        cru: com_espaco_de_nomes(recortar(p, texto)?),
    })
}

/// Le de volta o `procEventoNFe` (o que se guarda).
///
/// # Errors
///
/// [`ErroResposta`] se o texto nao e' um `procEventoNFe` com o `evento` e o `retEvento`.
pub fn ler_proc_evento(texto: &str) -> Result<ProcEvento, ErroResposta> {
    let doc = abrir(texto, "procEventoNFe")?;
    proc_de(doc.root_element(), texto)
}

/// Os `procEventoNFe` de um `retConsSitNFe` (a consulta devolve os eventos da NF-e), cada um lido e com o texto pronto
/// para guardar.
///
/// # Errors
///
/// [`ErroResposta`] se o texto nao e' o `retConsSitNFe` ou um `procEventoNFe` esta fora do leiaute.
pub fn eventos_da_consulta(texto: &str) -> Result<Vec<ProcEvento>, ErroResposta> {
    let doc = abrir(texto, "retConsSitNFe")?;
    filhos(doc.root_element(), "procEventoNFe")
        .map(|p| proc_de(p, texto))
        .collect()
}
