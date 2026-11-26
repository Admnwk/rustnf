// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O que o cliente devolve: o desfecho da nota em palavras e o que fazer, e a situacao do servico.

/// O desfecho de um envio ou de uma consulta da nota.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Desfecho {
    /// Uso autorizado (`cStat` 100) ou autorizado fora do prazo (150). `nfe_proc` e' o documento a guardar: a NF-e como
    /// foi enviada mais o protocolo como a SEFAZ o mandou.
    Autorizada {
        /// `cStat` do protocolo.
        cstat: String,
        /// `xMotivo` do protocolo.
        motivo: String,
        /// `nProt`.
        protocolo: String,
        /// O `nfeProc`; `None` so' na consulta feita sem a NF-e (sem ela nao ha como monta-lo).
        nfe_proc: Option<String>,
    },
    /// Uso denegado (110, 301, 302 ou 303): o numero esta consumido e a nota nao vale.
    Denegada {
        /// `cStat` do protocolo.
        cstat: String,
        /// `xMotivo` do protocolo.
        motivo: String,
        /// `nProt` (vazio se a SEFAZ nao o mandou).
        protocolo: String,
        /// O `nfeProc`, quando ha protocolo.
        nfe_proc: Option<String>,
    },
    /// A consulta mostra a nota cancelada (101 ou 151). Nao ha `nfeProc`: o cancelamento e' evento.
    Cancelada {
        /// `cStat` da consulta.
        cstat: String,
        /// `xMotivo` da consulta.
        motivo: String,
        /// `nProt` do protocolo de autorizacao, se a SEFAZ o mandou.
        protocolo: String,
        /// O `nfeProc`, quando a consulta traz o protocolo da autorizacao.
        nfe_proc: Option<String>,
    },
    /// A SEFAZ recusou: o `cStat` e o motivo dizem por que. O numero NAO foi consumido.
    Rejeitada {
        /// O codigo da rejeicao.
        cstat: String,
        /// O motivo, como a SEFAZ o escreveu.
        motivo: String,
    },
    /// Rejeicao 539: o numero ja foi autorizado com OUTRA chave. A nota que vale e' a outra; esta, nao. A chave da outra
    /// vem na mensagem da SEFAZ e, se a consulta dela confere (mesmo emitente, modelo, serie e numero), vem aqui.
    NumeroJaUsado {
        /// O motivo, como a SEFAZ o escreveu.
        motivo: String,
        /// A chave da nota que tem o numero.
        chave_existente: Option<String>,
        /// `cStat` da consulta a essa chave (100 = autorizada).
        situacao_existente: Option<String>,
    },
    /// O lote assincrono segue em processamento (105) depois das tentativas: consulte o recibo de novo.
    EmProcessamento {
        /// `nRec`.
        recibo: String,
    },
    /// A SEFAZ diz que a nota nao consta (217): ela nao chegou. Reenviar a MESMA NF-e e' seguro.
    NaoRecebida,
    /// A consulta (124) mostra so' o EPEC da nota, sem a NF-e: a nota ainda tem de ser transmitida.
    SoEpec {
        /// O motivo, como a SEFAZ o escreveu.
        motivo: String,
    },
    /// Servico parado (108 ou 109): a SEFAZ nao processou nada. Reenviar a MESMA NF-e depois e' seguro.
    Indisponivel {
        /// O codigo da SEFAZ.
        cstat: String,
        /// O motivo da SEFAZ.
        motivo: String,
    },
    /// Nao deu para saber: sem resposta do envio e sem resposta da consulta. A nota PODE ter sido autorizada.
    SemResposta,
}

/// O resultado de `autorizar`, `retorno` ou `consultar`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resultado {
    /// O que aconteceu.
    pub desfecho: Desfecho,
    /// A chave da NF-e (44 posicoes).
    pub chave: String,
    /// O recibo do lote assincrono, quando houve.
    pub recibo: Option<String>,
    /// O `ret*` cru da ultima resposta util (vazio sem resposta).
    pub xml_retorno: String,
    /// O `enviNFe` enviado (vazio na consulta).
    pub xml_enviado: String,
    /// A resposta do envio se perdeu e o desfecho veio da consulta pela chave.
    pub recuperada: bool,
    /// Frase para a tela: sem URL, sem corpo da resposta, com o que fazer.
    pub frase: String,
    /// So' para o registro (HTTP, tempo esgotado, o que a consulta respondeu...).
    pub detalhe: String,
}

impl Resultado {
    pub(crate) fn novo(desfecho: Desfecho, chave: &str) -> Self {
        let frase = frase_do(&desfecho);
        Resultado {
            desfecho,
            chave: chave.to_string(),
            recibo: None,
            xml_retorno: String::new(),
            xml_enviado: String::new(),
            recuperada: false,
            frase,
            detalhe: String::new(),
        }
    }

    /// A nota esta autorizada.
    #[must_use]
    pub fn autorizada(&self) -> bool {
        matches!(self.desfecho, Desfecho::Autorizada { .. })
    }

    /// O `nfeProc` a guardar (autorizada, denegada ou cancelada com protocolo).
    #[must_use]
    pub fn nfe_proc(&self) -> Option<&str> {
        match &self.desfecho {
            Desfecho::Autorizada { nfe_proc, .. }
            | Desfecho::Denegada { nfe_proc, .. }
            | Desfecho::Cancelada { nfe_proc, .. } => nfe_proc.as_deref(),
            _ => None,
        }
    }

    /// Reenviar a MESMA NF-e assinada e' seguro: a SEFAZ disse que nao a recebeu ou nao processou nada. Reenviar e'
    /// chamar `autorizar` de novo com o mesmo XML (a chave nao muda, entao a SEFAZ nunca ve uma segunda nota).
    #[must_use]
    pub fn pode_reenviar(&self) -> bool {
        matches!(
            self.desfecho,
            Desfecho::NaoRecebida | Desfecho::Indisponivel { .. }
        )
    }

    /// Falta saber: consultar a chave (ou o recibo) de novo. Emitir outra nota NAO e' opcao.
    #[must_use]
    pub fn deve_consultar(&self) -> bool {
        matches!(
            self.desfecho,
            Desfecho::SemResposta | Desfecho::EmProcessamento { .. }
        )
    }
}

fn frase_do(d: &Desfecho) -> String {
    match d {
        Desfecho::Autorizada { protocolo, .. } => {
            format!("NF-e autorizada: protocolo {protocolo}.")
        }
        Desfecho::Denegada { cstat, motivo, .. } => format!(
            "Uso denegado ({cstat}): {motivo}. O numero da nota esta consumido e a nota nao vale."
        ),
        Desfecho::Cancelada { cstat, .. } => format!("A NF-e consta como cancelada ({cstat})."),
        Desfecho::Rejeitada { cstat, motivo } => format!("Rejeitada ({cstat}): {motivo}"),
        Desfecho::NumeroJaUsado {
            chave_existente, ..
        } => {
            let outra = chave_existente
                .as_ref()
                .map_or(String::new(), |c| format!(" (chave {c})"));
            format!(
                "Rejeitada (539): este numero ja foi autorizado com outra chave{outra}. A nota que vale e' a outra: \
                 confira-a antes de qualquer reenvio e nao emita uma terceira."
            )
        }
        Desfecho::SoEpec { .. } => "Na SEFAZ consta so' o EPEC (124), sem a NF-e: a nota ainda precisa ser transmitida."
            .to_string(),
        Desfecho::EmProcessamento { recibo } => format!(
            "A SEFAZ ainda esta processando o lote (recibo {recibo}). Consulte o recibo de novo mais tarde; \
             nao reenvie a nota."
        ),
        Desfecho::NaoRecebida => "A SEFAZ diz que a nota nao consta (217): ela nao chegou. Pode reenviar a \
                                  MESMA NF-e assinada."
            .to_string(),
        Desfecho::Indisponivel { cstat, motivo } => format!(
            "Servico da SEFAZ indisponivel ({cstat}): {motivo}. Nada foi processado; pode reenviar a MESMA \
             NF-e assinada mais tarde."
        ),
        Desfecho::SemResposta => "Sem resposta da SEFAZ e sem como confirmar agora. A nota PODE ter sido \
                                  autorizada: consulte a chave de novo e NAO emita outra nota."
            .to_string(),
    }
}

/// Como esta o servico da SEFAZ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SituacaoServico {
    /// 107: em operacao.
    EmOperacao,
    /// 108 ou 109: paralisado.
    Paralisado,
    /// Nao respondeu.
    SemResposta,
    /// Respondeu outro codigo.
    Outra,
}

/// O resultado de `status`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusServico {
    /// Como esta o servico.
    pub situacao: SituacaoServico,
    /// `cStat` da resposta (vazio sem resposta).
    pub cstat: String,
    /// `xMotivo` da resposta.
    pub motivo: String,
    /// `tMed`: o tempo medio de resposta, em segundos.
    pub tempo_medio: Option<u32>,
    /// `dhRetorno`: quando a SEFAZ preve a volta (vazio se nao disse).
    pub retorno_previsto: String,
    /// Frase para a tela.
    pub frase: String,
    /// So' para o registro.
    pub detalhe: String,
}

impl StatusServico {
    /// O servico esta em operacao.
    #[must_use]
    pub fn em_operacao(&self) -> bool {
        self.situacao == SituacaoServico::EmOperacao
    }
}
