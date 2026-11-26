// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A linha do indice dos documentos recebidos e a situacao dela.

use rustnf::nfe::sefaz::dfe::{ResumoNfe, SituacaoResumo};
use rustnf::nfe::sefaz::evento::{CIENCIA, CONFIRMACAO, DESCONHECIMENTO, NAO_REALIZADA};
use rustnf::nfe::Nfe;
use serde::{Deserialize, Serialize};

use super::super::EventoGuardado;

/// `tpEvento` do cancelamento de NF-e: quem a emitiu a cancelou.
pub(super) const CANCELAMENTO: &str = "110111";

/// A situacao de uma NF-e recebida. Uma so' palavra, na ordem do que pesa mais: cancelada pelo emitente, denegada, a
/// ultima manifestacao do destinatario e, sem ela, o que se tem do documento.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SituacaoRecebida {
    /// So' o resumo (`resNFe`): o destinatario ainda nao manifestou, entao o Ambiente Nacional nao entregou a nota.
    #[default]
    Resumo,
    /// A NF-e completa (`procNFe`) esta na pasta, sem manifestacao.
    Completa,
    /// Ciencia da operacao (210210).
    Ciencia,
    /// Confirmacao da operacao (210200).
    Confirmada,
    /// Desconhecimento da operacao (210220).
    Desconhecida,
    /// Operacao nao realizada (210240).
    NaoRealizada,
    /// O emitente cancelou a NF-e (evento 110111 ou `cSitNFe` 3).
    Cancelada,
    /// A NF-e foi denegada (`cSitNFe` 2 ou `cStat` 110, 301, 302 ou 303).
    Denegada,
}

impl SituacaoRecebida {
    /// O texto da situacao, o mesmo do JSON e da linha de comando.
    #[must_use]
    pub fn texto(self) -> &'static str {
        match self {
            Self::Resumo => "resumo",
            Self::Completa => "completa",
            Self::Ciencia => "ciencia",
            Self::Confirmada => "confirmada",
            Self::Desconhecida => "desconhecida",
            Self::NaoRealizada => "nao_realizada",
            Self::Cancelada => "cancelada",
            Self::Denegada => "denegada",
        }
    }

    /// A situacao em palavras, para a tela.
    #[must_use]
    pub fn em_palavras(self) -> &'static str {
        match self {
            Self::Resumo => "so' o resumo (ainda nao manifestada)",
            Self::Completa => "nota completa baixada",
            Self::Ciencia => "ciencia da operacao registrada",
            Self::Confirmada => "operacao confirmada",
            Self::Desconhecida => "operacao desconhecida",
            Self::NaoRealizada => "operacao nao realizada",
            Self::Cancelada => "cancelada pelo emitente",
            Self::Denegada => "denegada",
        }
    }
}

/// Uma NF-e recebida no indice (`nfe/recebidas/indice.json`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Recebida {
    /// Chave de acesso, 44 posicoes.
    pub chave: String,
    /// CNPJ ou CPF do emitente.
    pub emitente: String,
    /// Nome do emitente.
    pub emitente_nome: String,
    /// Data de emissao (`dhEmi`).
    pub emissao: String,
    /// `vNF`, como a nota ou o resumo traz.
    pub valor: String,
    /// `homologacao` ou `producao`.
    pub ambiente: String,
    /// A situacao (veja [`SituacaoRecebida`]).
    pub situacao: SituacaoRecebida,
    /// A NF-e completa esta na pasta.
    pub completa: bool,
    /// O emitente cancelou a NF-e.
    pub cancelada: bool,
    /// A NF-e foi denegada.
    pub denegada: bool,
    /// `tpEvento` da ultima manifestacao do destinatario (vazio = nenhuma).
    pub manifestacao: String,
    /// `nProt` da autorizacao.
    pub protocolo: String,
    /// O ultimo NSU em que algo desta nota chegou.
    pub nsu: String,
    /// Os eventos da nota guardados na pasta (o do emitente, de terceiros, e a manifestacao do destinatario).
    pub eventos: Vec<EventoGuardado>,
}

impl Recebida {
    pub(super) fn nova(chave: &str, ambiente: &str) -> Self {
        Self {
            chave: chave.to_string(),
            ambiente: ambiente.to_string(),
            ..Self::default()
        }
    }

    /// Calcula a situacao a partir do que a linha sabe.
    pub(super) fn recalcular(&mut self) {
        self.situacao = if self.cancelada {
            SituacaoRecebida::Cancelada
        } else if self.denegada {
            SituacaoRecebida::Denegada
        } else {
            match self.manifestacao.as_str() {
                CONFIRMACAO => SituacaoRecebida::Confirmada,
                DESCONHECIMENTO => SituacaoRecebida::Desconhecida,
                NAO_REALIZADA => SituacaoRecebida::NaoRealizada,
                CIENCIA => SituacaoRecebida::Ciencia,
                _ if self.completa => SituacaoRecebida::Completa,
                _ => SituacaoRecebida::Resumo,
            }
        };
    }

    /// Guarda o evento (uma vez so': o mesmo tipo e sequencia nao se repete) e deixa a linha em dia com ele.
    pub(super) fn anotar_evento(&mut self, e: EventoGuardado) {
        if e.tipo == CANCELAMENTO {
            self.cancelada = true;
        }
        if matches!(
            e.tipo.as_str(),
            CIENCIA | CONFIRMACAO | DESCONHECIMENTO | NAO_REALIZADA
        ) {
            self.manifestacao.clone_from(&e.tipo);
        }
        match self
            .eventos
            .iter_mut()
            .find(|x| x.tipo == e.tipo && x.sequencia == e.sequencia)
        {
            Some(x) => *x = e,
            None => self.eventos.push(e),
        }
        self.recalcular();
    }

    /// O resumo (`resNFe`) preenche so' o que a linha ainda nao sabe: a nota completa, se ja veio, e' a que vale.
    pub(super) fn do_resumo(&mut self, r: &ResumoNfe, nsu: &str) {
        if !self.completa {
            self.emitente.clone_from(&r.emitente);
            self.emitente_nome.clone_from(&r.nome);
            self.emissao = r
                .emissao
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_default();
            self.valor = r
                .valor
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_default();
        }
        if self.protocolo.is_empty() {
            self.protocolo.clone_from(&r.protocolo);
        }
        match r.situacao {
            SituacaoResumo::Cancelada => self.cancelada = true,
            SituacaoResumo::Denegada => self.denegada = true,
            _ => {}
        }
        self.nsu = nsu.to_string();
        self.recalcular();
    }

    /// A NF-e completa (`procNFe`).
    pub(super) fn da_nota(&mut self, n: &Nfe, nsu: &str) {
        self.completa = true;
        self.emitente.clone_from(&n.emit.documento);
        self.emitente_nome.clone_from(&n.emit.nome);
        self.emissao = n
            .ide
            .emissao
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_default();
        self.valor = n
            .total
            .icms_tot
            .as_ref()
            .map(|g| g.valor("vNF").to_string())
            .unwrap_or_default();
        if let Some(p) = &n.protocolo {
            self.protocolo.clone_from(&p.numero);
            if matches!(p.cstat.as_str(), "110" | "301" | "302" | "303") {
                self.denegada = true;
            }
        }
        self.nsu = nsu.to_string();
        self.recalcular();
    }
}
