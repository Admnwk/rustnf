// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O que o cliente devolve nos eventos e na inutilizacao: o desfecho em palavras e o que fazer.

/// O desfecho do registro de um evento.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DesfechoEvento {
    /// O evento esta registrado: 135 (registrado e vinculado a NF-e), 155 (cancelamento homologado FORA DO PRAZO) ou 136
    /// no EPEC (que so' existe sem a NF-e: o MOC exclui o 135).
    Registrado {
        /// `cStat` do evento.
        cstat: String,
        /// `xMotivo` do evento.
        motivo: String,
        /// `nProt` do evento.
        protocolo: String,
        /// 155: a SEFAZ homologou depois do prazo.
        fora_do_prazo: bool,
    },
    /// 136: a SEFAZ guardou o evento, mas a NF-e nao existia no momento (a vinculacao ficou prejudicada). A NF-e NAO foi
    /// alterada.
    SemVinculo {
        /// `cStat` do evento.
        cstat: String,
        /// `xMotivo` do evento.
        motivo: String,
        /// `nProt` do evento.
        protocolo: String,
    },
    /// Cancelamento recusado por prazo (501): a NF-e passou do prazo da legislacao e a SEFAZ da UF nao aceitou fora
    /// dele.
    PrazoVencido {
        /// `cStat` (501).
        cstat: String,
        /// `xMotivo`.
        motivo: String,
    },
    /// EPEC recusado com 142: o Ambiente Nacional bloqueou novos EPEC do emitente enquanto houver EPEC sem a NF-e
    /// transmitida.
    EpecBloqueado {
        /// `xMotivo`.
        motivo: String,
        /// As chaves dos EPEC pendentes de conciliacao (`chNFePend`).
        pendentes: Vec<String>,
    },
    /// A SEFAZ recusou: o `cStat` e o motivo dizem por que. O evento NAO foi registrado.
    Rejeitado {
        /// O codigo da rejeicao (do lote ou do evento).
        cstat: String,
        /// O motivo, como a SEFAZ o escreveu.
        motivo: String,
    },
    /// Servico parado (108 ou 109): nada foi processado.
    Indisponivel {
        /// O codigo da SEFAZ.
        cstat: String,
        /// O motivo da SEFAZ.
        motivo: String,
    },
    /// Sem resposta do envio, e a consulta da NF-e respondeu SEM o evento: ele nao foi registrado. Reenviar o MESMO
    /// evento (mesmo tipo e mesmo `nSeqEvento`) e' seguro.
    NaoRegistrado,
    /// Sem resposta do envio nem da consulta: o evento PODE ter sido registrado. Consulte a NF-e de novo antes de
    /// qualquer reenvio.
    SemResposta,
}

/// O resultado de `cancelar`, `corrigir`, `registrar_evento_rtc` ou `registrar_epec`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultadoEvento {
    /// O que aconteceu.
    pub desfecho: DesfechoEvento,
    /// A chave da NF-e (44 posicoes).
    pub chave: String,
    /// `tpEvento`.
    pub tp_evento: String,
    /// `nSeqEvento`.
    pub seq: u32,
    /// O autorizador que recebeu o pedido (UF, `SVC-AN`, `SVC-RS`, `AN`, `SVRS`...).
    pub autorizador: String,
    /// O `envEvento` enviado.
    pub xml_enviado: String,
    /// O `retEnvEvento` cru da ultima resposta util (vazio sem resposta).
    pub xml_retorno: String,
    /// O `procEventoNFe` a guardar: o evento enviado mais o `retEvento` (registrado, ou achado pela consulta).
    pub proc_evento: Option<String>,
    /// A resposta do envio se perdeu e o desfecho veio da consulta pela chave.
    pub recuperado: bool,
    /// Frase para a tela: sem URL, sem corpo da resposta, com o que fazer.
    pub frase: String,
    /// So' para o registro (HTTP, tempo esgotado, o que a consulta respondeu...).
    pub detalhe: String,
}

impl ResultadoEvento {
    /// O evento esta registrado (135, 155 ou o 136 do EPEC).
    #[must_use]
    pub fn registrado(&self) -> bool {
        matches!(self.desfecho, DesfechoEvento::Registrado { .. })
    }

    /// Reenviar o MESMO evento e' seguro: a SEFAZ nao processou nada ou a consulta mostra que ele nao esta la.
    #[must_use]
    pub fn pode_reenviar(&self) -> bool {
        matches!(
            self.desfecho,
            DesfechoEvento::NaoRegistrado | DesfechoEvento::Indisponivel { .. }
        )
    }

    /// Falta saber: consultar a NF-e de novo. Enviar outro evento NAO e' opcao.
    #[must_use]
    pub fn deve_consultar(&self) -> bool {
        matches!(self.desfecho, DesfechoEvento::SemResposta)
    }
}

pub(super) fn nome_do_evento(tp_evento: &str) -> String {
    match tp_evento {
        "110111" => "Cancelamento".to_string(),
        "110110" => "Carta de correcao".to_string(),
        "110140" => "EPEC".to_string(),
        outro => format!("Evento {outro}"),
    }
}

pub(super) fn frase_do_evento(d: &DesfechoEvento, tp_evento: &str) -> String {
    let nome = nome_do_evento(tp_evento);
    match d {
        DesfechoEvento::Registrado {
            cstat,
            protocolo,
            fora_do_prazo,
            ..
        } => {
            if *fora_do_prazo {
                format!(
                    "{nome} registrado FORA DO PRAZO ({cstat}): protocolo {protocolo}. A SEFAZ da UF aceitou depois do prazo."
                )
            } else {
                format!("{nome} registrado: protocolo {protocolo}.")
            }
        }
        DesfechoEvento::SemVinculo { cstat, motivo, .. } => format!(
            "{nome} guardado SEM vinculo a NF-e ({cstat}): {motivo}. A NF-e nao foi alterada."
        ),
        DesfechoEvento::PrazoVencido { cstat, motivo } => format!(
            "{nome} recusado por prazo ({cstat}): {motivo}. A NF-e passou do prazo de cancelamento da legislacao e a SEFAZ da UF nao aceitou fora dele; procure a SEFAZ da UF."
        ),
        DesfechoEvento::EpecBloqueado { motivo, pendentes } => format!(
            "EPEC recusado (142): {motivo}. Ha {} EPEC sem a NF-e transmitida; transmita essas notas para liberar novos EPEC.",
            pendentes.len()
        ),
        DesfechoEvento::Rejeitado { cstat, motivo } => {
            format!("{nome} rejeitado ({cstat}): {motivo}")
        }
        DesfechoEvento::Indisponivel { cstat, motivo } => format!(
            "Servico de eventos da SEFAZ indisponivel ({cstat}): {motivo}. Nada foi processado; pode reenviar o MESMO evento mais tarde."
        ),
        DesfechoEvento::NaoRegistrado => format!(
            "Sem resposta da SEFAZ, e a consulta da NF-e mostra que o evento NAO foi registrado. Pode reenviar o MESMO {nome} (mesmo nSeqEvento); nao crie outro com sequencia nova."
        ),
        DesfechoEvento::SemResposta => format!(
            "Sem resposta da SEFAZ e sem como confirmar agora. O {nome} PODE ter sido registrado: consulte a NF-e de novo antes de qualquer reenvio e NAO envie outro com sequencia nova."
        ),
    }
}

/// O desfecho da inutilizacao.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DesfechoInutilizacao {
    /// Faixa inutilizada (102).
    Inutilizada {
        /// `cStat`.
        cstat: String,
        /// `xMotivo`.
        motivo: String,
        /// `nProt`.
        protocolo: String,
    },
    /// 563: a mesma faixa ja tinha sido inutilizada; o protocolo e' o do pedido anterior.
    JaInutilizada {
        /// `xMotivo`.
        motivo: String,
        /// `nProt` do pedido anterior.
        protocolo: String,
    },
    /// 998 ou HTTP 404: a UF eliminou o servico (NT 2023.002 v1.02). Nao repita.
    ServicoDesativado {
        /// `xMotivo` ou o HTTP, em palavras.
        motivo: String,
    },
    /// A SEFAZ recusou: o `cStat` e o motivo dizem por que. A faixa NAO foi inutilizada.
    Rejeitada {
        /// O codigo da rejeicao.
        cstat: String,
        /// O motivo, como a SEFAZ o escreveu.
        motivo: String,
    },
    /// Servico parado (108 ou 109): nada foi processado.
    Indisponivel {
        /// O codigo da SEFAZ.
        cstat: String,
        /// O motivo da SEFAZ.
        motivo: String,
    },
    /// Sem resposta. O servico nao tem consulta: repetir a MESMA inutilizacao e' seguro, porque a faixa repetida volta
    /// 563 (com o protocolo) se a primeira foi aceita.
    SemResposta,
}

/// O resultado de `inutilizar`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultadoInutilizacao {
    /// O que aconteceu.
    pub desfecho: DesfechoInutilizacao,
    /// `Id` do `infInut` (43 posicoes).
    pub id: String,
    /// O `inutNFe` enviado (vazio quando a UF nao tem mais o servico).
    pub xml_enviado: String,
    /// O `retInutNFe` cru da resposta (vazio sem resposta).
    pub xml_retorno: String,
    /// O `ProcInutNFe` a guardar (102 ou 563).
    pub proc_inut: Option<String>,
    /// Frase para a tela.
    pub frase: String,
    /// So' para o registro.
    pub detalhe: String,
}

impl ResultadoInutilizacao {
    /// A faixa esta inutilizada (102 ou 563).
    #[must_use]
    pub fn inutilizada(&self) -> bool {
        matches!(
            self.desfecho,
            DesfechoInutilizacao::Inutilizada { .. } | DesfechoInutilizacao::JaInutilizada { .. }
        )
    }

    /// Repetir a MESMA inutilizacao e' seguro (sem resposta, ou servico parado).
    #[must_use]
    pub fn pode_repetir(&self) -> bool {
        matches!(
            self.desfecho,
            DesfechoInutilizacao::SemResposta | DesfechoInutilizacao::Indisponivel { .. }
        )
    }
}

pub(super) fn frase_da_inutilizacao(d: &DesfechoInutilizacao) -> String {
    match d {
        DesfechoInutilizacao::Inutilizada { protocolo, .. } => {
            format!("Faixa inutilizada: protocolo {protocolo}.")
        }
        DesfechoInutilizacao::JaInutilizada { protocolo, .. } => format!(
            "A mesma faixa ja estava inutilizada (563): protocolo {protocolo} do pedido anterior."
        ),
        DesfechoInutilizacao::ServicoDesativado { motivo } => format!(
            "O servico de inutilizacao foi desativado nesta UF ({motivo}): nao repita o pedido (NT 2023.002 v1.02, item 2.5)."
        ),
        DesfechoInutilizacao::Rejeitada { cstat, motivo } => {
            format!("Inutilizacao rejeitada ({cstat}): {motivo}")
        }
        DesfechoInutilizacao::Indisponivel { cstat, motivo } => format!(
            "Servico de inutilizacao da SEFAZ indisponivel ({cstat}): {motivo}. Nada foi processado; pode repetir a MESMA inutilizacao mais tarde."
        ),
        DesfechoInutilizacao::SemResposta => "Sem resposta da SEFAZ. Repita a MESMA inutilizacao: se a primeira foi aceita, a SEFAZ responde 563 com o protocolo dela."
            .to_string(),
    }
}
