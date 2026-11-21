// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O erro unico das operacoes: cada variante sabe seu codigo de saida e seu codigo de envelope. O status HTTP de cada
//! uma mora no binario, que e' quem serve HTTP.

use std::fmt;

use rustnf::danfse::ErroDanfse;
use rustnf::nfse::leitura::ErroLeitura;
use rustnf::nfse::ErroNfse;
use rustnf::reforma::previa::ErroPrevia;

/// Por que um comando ou uma chamada nao deu certo. A mensagem e' para o usuario: nunca leva caminho de arquivo,
/// senha, chave nem XML assinado.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Erro {
    /// Opcao ou combinacao de opcoes invalida (saida 2).
    Uso(String),
    /// Entrada que nao serve: JSON invalido, arquivo ilegivel, certificado que nao abre (saida 2).
    Entrada(String),
    /// A biblioteca recusou por regra fiscal ou de leiaute; `campo` e' o caminho no XML, quando ha (saida 3).
    Recusa {
        /// Caminho do campo recusado (pode ser vazio).
        campo: String,
        /// O motivo.
        mensagem: String,
    },
    /// Falha de rede ou resposta de erro da Sefin Nacional (saida 4).
    Rede(String),
    /// A Sefin Nacional nao conhece a nota (saida 4).
    NaoEncontrada(String),
    /// Token ausente ou errado (HTTP 401).
    NaoAutorizado,
    /// Corpo maior que o limite, em bytes (HTTP 413).
    Grande(usize),
    /// Falha do proprio programa (saida 1); o detalhe nao sai daqui.
    Interno,
}

impl Erro {
    /// Codigo de saida do processo: 0 ok (nao passa por aqui), 1 o resto, 2 uso/entrada, 3 recusa de regra,
    /// 4 rede/Sefin.
    #[must_use]
    pub fn codigo_saida(&self) -> u8 {
        match self {
            Self::Uso(_) | Self::Entrada(_) | Self::NaoAutorizado | Self::Grande(_) => 2,
            Self::Recusa { .. } => 3,
            Self::Rede(_) | Self::NaoEncontrada(_) => 4,
            Self::Interno => 1,
        }
    }

    /// Codigo estavel do envelope `{"erro": {"codigo": ...}}`.
    #[must_use]
    pub fn codigo(&self) -> &'static str {
        match self {
            Self::Uso(_) => "uso",
            Self::Entrada(_) => "entrada_invalida",
            Self::Recusa { .. } => "recusa",
            Self::Rede(_) => "sefin",
            Self::NaoEncontrada(_) => "nao_encontrada",
            Self::NaoAutorizado => "nao_autorizado",
            Self::Grande(_) => "corpo_grande",
            Self::Interno => "erro_interno",
        }
    }

    /// Recusa com campo.
    #[must_use]
    pub fn recusa(campo: impl Into<String>, mensagem: impl Into<String>) -> Self {
        Self::Recusa {
            campo: campo.into(),
            mensagem: mensagem.into(),
        }
    }

    /// JSON do envelope unico de erro.
    #[must_use]
    pub fn envelope(&self) -> serde_json::Value {
        let mut e = serde_json::json!({"codigo": self.codigo(), "mensagem": self.mensagem()});
        if let Self::Recusa { campo, .. } = self {
            if !campo.is_empty() {
                e["campo"] = serde_json::Value::String(campo.clone());
            }
        }
        serde_json::json!({ "erro": e })
    }

    /// Apenas a mensagem, sem o campo.
    #[must_use]
    pub fn mensagem(&self) -> String {
        match self {
            Self::Uso(m) | Self::Entrada(m) | Self::Rede(m) | Self::NaoEncontrada(m) => m.clone(),
            Self::Recusa { mensagem, .. } => mensagem.clone(),
            Self::NaoAutorizado => "token ausente ou invalido".into(),
            Self::Grande(limite) => format!("corpo maior que o limite de {limite} bytes"),
            Self::Interno => "falha interna do programa".into(),
        }
    }
}

impl fmt::Display for Erro {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Recusa { campo, mensagem } if !campo.is_empty() => {
                write!(f, "{campo}: {mensagem}")
            }
            outro => f.write_str(&outro.mensagem()),
        }
    }
}

impl std::error::Error for Erro {}

impl From<ErroNfse> for Erro {
    fn from(e: ErroNfse) -> Self {
        Self::Recusa {
            campo: e.campo,
            mensagem: e.mensagem,
        }
    }
}

impl From<ErroPrevia> for Erro {
    fn from(e: ErroPrevia) -> Self {
        match e {
            ErroPrevia::Entrada { campo, motivo } => Self::recusa(campo, motivo),
            outro => Self::recusa("", outro.to_string()),
        }
    }
}

impl From<ErroDanfse> for Erro {
    fn from(e: ErroDanfse) -> Self {
        match e {
            ErroDanfse::Pdf(_) => Self::Interno,
            outro => Self::Entrada(outro.to_string()),
        }
    }
}

impl From<ErroLeitura> for Erro {
    fn from(e: ErroLeitura) -> Self {
        Self::Entrada(e.to_string())
    }
}

impl From<serde_json::Error> for Erro {
    fn from(e: serde_json::Error) -> Self {
        Self::Entrada(format!("JSON invalido: {e}"))
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn codigos_de_saida() {
        assert_eq!(Erro::Uso("x".into()).codigo_saida(), 2);
        assert_eq!(Erro::recusa("a", "b").codigo_saida(), 3);
        assert_eq!(Erro::Rede("x".into()).codigo_saida(), 4);
        assert_eq!(Erro::Interno.codigo_saida(), 1);
    }

    #[test]
    fn envelope_tem_codigo_e_mensagem() {
        let v = Erro::recusa("infDPS/serie", "serie invalida").envelope();
        assert_eq!(v["erro"]["codigo"], "recusa");
        assert_eq!(v["erro"]["campo"], "infDPS/serie");
        assert_eq!(v["erro"]["mensagem"], "serie invalida");
    }
}
