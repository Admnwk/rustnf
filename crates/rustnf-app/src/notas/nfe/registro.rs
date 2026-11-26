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
            destinatario_documento: documento,
            destinatario_nome: nome,
            ..Self::default()
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
