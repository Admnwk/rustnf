// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! As linhas do indice da pasta de notas da NF-e.

use rustnf::nfe::Nfe;
use serde::{Deserialize, Serialize};

/// Situacao da NF-e (ou da faixa) na pasta.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Situacao {
    /// Uso autorizado (100 ou 150).
    Autorizada,
    /// Uso denegado (110, 301, 302 ou 303): o numero esta consumido e a nota nao vale.
    Denegada,
    /// Cancelada por evento (110111).
    Cancelada,
    /// Assinada e gravada, enviada (ou a enviar) sem resultado: a SEFAZ pode ou nao ter autorizado.
    Pendente,
    /// Faixa de numeracao inutilizada (so' nas linhas de [`Inutilizacao`]).
    Inutilizada,
    /// EPEC registrado e a NF-e ainda por transmitir a SEFAZ da UF.
    Epec,
}

impl Situacao {
    /// O texto da situacao.
    #[must_use]
    pub fn texto(self) -> &'static str {
        match self {
            Self::Autorizada => "autorizada",
            Self::Denegada => "denegada",
            Self::Cancelada => "cancelada",
            Self::Pendente => "pendente",
            Self::Inutilizada => "inutilizada",
            Self::Epec => "epec",
        }
    }

    /// A nota gastou o numero: a SEFAZ o tem na base (autorizada, denegada, cancelada, EPEC) ou pode te-lo (pendente).
    #[must_use]
    pub fn consome_numero(self) -> bool {
        !matches!(self, Self::Inutilizada)
    }
}

/// Um evento registrado da nota (o XML esta em `<chave>-<tipo>-<sequencia>-procEventoNFe.xml`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventoGuardado {
    /// `tpEvento`: 110111 cancelamento, 110110 carta de correcao, 110140 EPEC.
    pub tipo: String,
    /// `nSeqEvento`.
    pub sequencia: u32,
    /// `nProt` do evento.
    pub protocolo: String,
}

/// Uma NF-e no indice (`nfe/indice.json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Registro {
    /// Chave de acesso, 44 posicoes.
    pub chave: String,
    /// Serie.
    pub serie: String,
    /// Numero da nota (`nNF`).
    pub numero: String,
    /// Data de emissao (`dhEmi`, `AAAA-MM-DD` e o resto).
    pub emissao: String,
    /// `vNF`, como a nota traz.
    pub valor: String,
    /// `homologacao` ou `producao`.
    pub ambiente: String,
    /// Situacao.
    pub situacao: Situacao,
    /// `nProt` da autorizacao (vazio enquanto pendente).
    pub protocolo: String,
    /// `cStat` do ultimo resultado da SEFAZ (vazio sem resultado).
    pub cstat: String,
    /// CPF ou CNPJ do destinatario (vazio se nao houver).
    pub destinatario_documento: String,
    /// Nome do destinatario.
    pub destinatario_nome: String,
    /// Protocolo do EPEC como o DANFE o imprime (so' na nota em EPEC).
    pub protocolo_epec: String,
    /// Quando foi enviada pela primeira vez (segundos Unix; 0 = nao se sabe).
    pub desde: i64,
    /// Quantas consultas a retomada ja fez.
    pub tentativas: i64,
    /// Quando foi a ultima (segundos Unix).
    pub ultima: Option<i64>,
    /// Os eventos registrados.
    pub eventos: Vec<EventoGuardado>,
    /// NFC-e emitida em contingencia off-line (`tpEmis` 9): ainda precisa ser TRANSMITIDA a SEFAZ, com a mesma chave.
    #[serde(skip_serializing_if = "is_false")]
    pub offline: bool,
    /// Fim do prazo para transmitir a off-line (segundos Unix); `None` fora da off-line.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prazo: Option<i64>,
    /// O que a SEFAZ recusou na ultima transmissao da off-line (`cStat` e motivo): a nota segue pendente ate ser
    /// corrigida; vazio = nada recusado.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub recusa: String,
    /// Na NF-e pendente: a chave do EPEC que registrou a MESMA operacao (a nota ficou sem resposta e a venda saiu em
    /// contingencia). Vazio = nao foi substituida.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub substituida_por: String,
    /// No EPEC: a chave da NF-e pendente que ele substitui. Vazio = nao substitui nenhuma.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub substitui: String,
}

fn is_false(b: &bool) -> bool {
    !*b
}

impl Default for Registro {
    fn default() -> Self {
        Self {
            chave: String::new(),
            serie: String::new(),
            numero: String::new(),
            emissao: String::new(),
            valor: String::new(),
            ambiente: String::new(),
            situacao: Situacao::Pendente,
            protocolo: String::new(),
            cstat: String::new(),
            destinatario_documento: String::new(),
            destinatario_nome: String::new(),
            protocolo_epec: String::new(),
            desde: 0,
            tentativas: 0,
            ultima: None,
            eventos: Vec::new(),
            offline: false,
            prazo: None,
            recusa: String::new(),
            substituida_por: String::new(),
            substitui: String::new(),
        }
    }
}

impl Registro {
    /// O registro de uma NF-e lida do XML, na `situacao` dada (os campos do resultado ficam vazios).
    #[must_use]
    pub fn da_nfe(n: &Nfe, situacao: Situacao) -> Self {
        let (documento, nome) = n
            .dest
            .as_ref()
            .map(|d| (d.documento.clone(), d.nome.clone()))
            .unwrap_or_default();
        Self {
            chave: n.chave.clone(),
            serie: n.ide.serie.to_string(),
            numero: n.ide.numero.to_string(),
            emissao: n
                .ide
                .emissao
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_default(),
            valor: n.total.v("vNF").to_string(),
            ambiente: if n.ide.ambiente == "1" {
                "producao".into()
            } else {
                "homologacao".into()
            },
            situacao,
            offline: n.ide.tipo_emissao == "9",
            destinatario_documento: documento,
            destinatario_nome: nome,
            ..Self::default()
        }
    }

    /// A SEFAZ confirmou, na ultima consulta, que nao tem esta nota (`cStat` 217, MOC 7.00, Anexo I, "NF-e nao consta na base
    /// de dados da SEFAZ").
    pub const CSTAT_NAO_CONSTA: &'static str = "217";

    /// A nota e' uma pendente cuja venda ja saiu em EPEC e que a SEFAZ nao tem: o numero dela se inutiliza (MOC 7.00,
    /// Anexo III, Manual de Contingencia, item 2.3.3). Qualquer outra pendente pode ter sido autorizada e NAO se inutiliza.
    #[must_use]
    pub fn inutilizavel_por_substituicao(&self) -> bool {
        self.situacao == Situacao::Pendente
            && !self.substituida_por.is_empty()
            && self.cstat == Self::CSTAT_NAO_CONSTA
    }

    /// O que falta fazer com a nota que um EPEC substituiu: `consultar` (a retomada ainda nao confirmou o que a SEFAZ fez com
    /// ela), `inutilizar` (a SEFAZ confirmou que nao a tem) ou `cancelar` (a SEFAZ a autorizou e ela segue autorizada). Vazio
    /// nas demais notas.
    #[must_use]
    pub fn a_fazer(&self) -> &'static str {
        match self.situacao {
            _ if self.substituida_por.is_empty() => "",
            Situacao::Autorizada => "cancelar",
            Situacao::Pendente if self.inutilizavel_por_substituicao() => "inutilizar",
            Situacao::Pendente => "consultar",
            _ => "",
        }
    }

    /// O numero e' um inteiro (0 se o indice o trouxer torto).
    #[must_use]
    pub fn numero_inteiro(&self) -> u64 {
        self.numero.parse().unwrap_or(0)
    }
}

/// Uma faixa inutilizada (`nfe/indice.json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Inutilizacao {
    /// `Id` do `infInut` (43 posicoes).
    pub id: String,
    /// Serie.
    pub serie: String,
    /// Primeiro numero da faixa.
    pub inicio: u64,
    /// Ultimo numero da faixa.
    pub fim: u64,
    /// Ano do pedido, 4 digitos.
    pub ano: String,
    /// `nProt` da inutilizacao.
    pub protocolo: String,
    /// Sempre [`Situacao::Inutilizada`].
    pub situacao: Situacao,
}
