// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O resultado da biblioteca em JSON e em codigo de saida. Nunca leva o XML assinado, o corpo da resposta nem o detalhe de
//! rede (`detalhe`): so' o que a pessoa precisa para decidir.

use rustnf::nfe::sefaz::{
    Desfecho, DesfechoEvento, DesfechoInutilizacao, Resultado, ResultadoEvento,
};
use serde_json::{json, Value};

use crate::erro::Erro;

/// Nome estavel do desfecho do envio ou da consulta, para o JSON.
pub(crate) fn nome_do_desfecho(d: &Desfecho) -> &'static str {
    match d {
        Desfecho::Autorizada { .. } => "autorizada",
        Desfecho::Denegada { .. } => "denegada",
        Desfecho::Cancelada { .. } => "cancelada",
        Desfecho::Rejeitada { .. } => "rejeitada",
        Desfecho::NumeroJaUsado { .. } => "numero_ja_usado",
        Desfecho::EmProcessamento { .. } => "em_processamento",
        Desfecho::NaoRecebida => "nao_recebida",
        Desfecho::SoEpec { .. } => "so_epec",
        Desfecho::Indisponivel { .. } => "indisponivel",
        Desfecho::ConsultaRecusada { .. } => "consulta_recusada",
        Desfecho::SemResposta => "sem_resposta",
        // desfecho novo da biblioteca: a frase dela diz o que houve
        _ => "outro",
    }
}

/// `cStat`, motivo e protocolo do desfecho (vazios quando ele nao os tem).
fn dados_do(d: &Desfecho) -> (String, String, String) {
    match d {
        Desfecho::Autorizada {
            cstat,
            motivo,
            protocolo,
            ..
        }
        | Desfecho::Denegada {
            cstat,
            motivo,
            protocolo,
            ..
        }
        | Desfecho::Cancelada {
            cstat,
            motivo,
            protocolo,
            ..
        } => (cstat.clone(), motivo.clone(), protocolo.clone()),
        Desfecho::Rejeitada { cstat, motivo }
        | Desfecho::Indisponivel { cstat, motivo }
        | Desfecho::ConsultaRecusada { cstat, motivo } => {
            (cstat.clone(), motivo.clone(), String::new())
        }
        Desfecho::NumeroJaUsado { motivo, .. } => ("539".into(), motivo.clone(), String::new()),
        Desfecho::SoEpec { motivo } => ("124".into(), motivo.clone(), String::new()),
        _ => (String::new(), String::new(), String::new()),
    }
}

/// O JSON do resultado de um envio ou de uma consulta: `status`, `frase`, `chave`, `cstat`, `motivo`, `protocolo`,
/// `recuperada` (a resposta se perdeu e a consulta achou a nota) e `recibo`.
#[must_use]
pub fn json_do_resultado(r: &Resultado) -> Value {
    let (cstat, motivo, protocolo) = dados_do(&r.desfecho);
    let mut v = json!({
        "status": nome_do_desfecho(&r.desfecho),
        "frase": r.frase,
        "chave": r.chave,
        "cstat": cstat,
        "motivo": motivo,
        "protocolo": protocolo,
        "recuperada": r.recuperada,
    });
    if let Some(recibo) = &r.recibo {
        v["recibo"] = json!(recibo);
    }
    if let Desfecho::NumeroJaUsado {
        chave_existente: Some(c),
        ..
    } = &r.desfecho
    {
        v["chave_existente"] = json!(c);
    }
    v
}

/// O erro (e o codigo de saida) do resultado: `None` se a nota esta autorizada. Rejeitada, denegada e numero ja usado
/// sao recusa de regra (saida 3); o que ficou sem resposta ou com o servico parado e' falha de rede (saida 4).
#[must_use]
pub fn erro_do_resultado(r: &Resultado) -> Option<Erro> {
    match &r.desfecho {
        Desfecho::Autorizada { .. } => None,
        Desfecho::Rejeitada { .. } | Desfecho::NumeroJaUsado { .. } | Desfecho::Denegada { .. } => {
            Some(Erro::recusa("", r.frase.clone()))
        }
        _ => Some(Erro::Rede(r.frase.clone())),
    }
}

fn nome_do_evento(d: &DesfechoEvento) -> &'static str {
    match d {
        DesfechoEvento::Registrado { .. } => "registrado",
        DesfechoEvento::SemVinculo { .. } => "sem_vinculo",
        DesfechoEvento::PrazoVencido { .. } => "prazo_vencido",
        DesfechoEvento::EpecBloqueado { .. } => "epec_bloqueado",
        DesfechoEvento::Rejeitado { .. } => "rejeitado",
        DesfechoEvento::Indisponivel { .. } => "indisponivel",
        DesfechoEvento::NaoRegistrado => "nao_registrado",
        DesfechoEvento::SemResposta => "sem_resposta",
        _ => "outro",
    }
}

/// O JSON do resultado de um evento: `status`, `frase`, `chave`, `tipo`, `sequencia`, `cstat`, `protocolo`.
#[must_use]
pub(crate) fn json_do_evento(r: &ResultadoEvento) -> Value {
    let (cstat, protocolo) = match &r.desfecho {
        DesfechoEvento::Registrado {
            cstat, protocolo, ..
        }
        | DesfechoEvento::SemVinculo {
            cstat, protocolo, ..
        } => (cstat.clone(), protocolo.clone()),
        DesfechoEvento::PrazoVencido { cstat, .. }
        | DesfechoEvento::Rejeitado { cstat, .. }
        | DesfechoEvento::Indisponivel { cstat, .. } => (cstat.clone(), String::new()),
        _ => (String::new(), String::new()),
    };
    json!({
        "status": nome_do_evento(&r.desfecho),
        "frase": r.frase,
        "chave": r.chave,
        "tipo": r.tp_evento,
        "sequencia": r.seq,
        "cstat": cstat,
        "protocolo": protocolo,
        "recuperado": r.recuperado,
    })
}

/// O erro do resultado de um evento: `None` se esta registrado.
#[must_use]
pub fn erro_do_evento(r: &ResultadoEvento) -> Option<Erro> {
    match &r.desfecho {
        DesfechoEvento::Registrado { .. } => None,
        DesfechoEvento::SemVinculo { .. }
        | DesfechoEvento::PrazoVencido { .. }
        | DesfechoEvento::EpecBloqueado { .. }
        | DesfechoEvento::Rejeitado { .. } => Some(Erro::recusa("", r.frase.clone())),
        _ => Some(Erro::Rede(r.frase.clone())),
    }
}

/// O JSON da inutilizacao: `status`, `frase`, `id`, `cstat`, `protocolo`.
pub(crate) fn json_da_inutilizacao(r: &rustnf::nfe::sefaz::ResultadoInutilizacao) -> Value {
    let (nome, cstat, protocolo) = match &r.desfecho {
        DesfechoInutilizacao::Inutilizada {
            cstat, protocolo, ..
        } => ("inutilizada", cstat.clone(), protocolo.clone()),
        DesfechoInutilizacao::JaInutilizada { protocolo, .. } => {
            ("ja_inutilizada", "563".to_string(), protocolo.clone())
        }
        DesfechoInutilizacao::ServicoDesativado { .. } => {
            ("servico_desativado", String::new(), String::new())
        }
        DesfechoInutilizacao::Rejeitada { cstat, .. } => {
            ("rejeitada", cstat.clone(), String::new())
        }
        DesfechoInutilizacao::Indisponivel { cstat, .. } => {
            ("indisponivel", cstat.clone(), String::new())
        }
        DesfechoInutilizacao::SemResposta => ("sem_resposta", String::new(), String::new()),
        _ => ("outro", String::new(), String::new()),
    };
    json!({"status": nome, "frase": r.frase, "id": r.id, "cstat": cstat, "protocolo": protocolo})
}

/// O erro da inutilizacao: `None` se a faixa esta inutilizada.
pub(crate) fn erro_da_inutilizacao(r: &rustnf::nfe::sefaz::ResultadoInutilizacao) -> Option<Erro> {
    match &r.desfecho {
        DesfechoInutilizacao::Inutilizada { .. } | DesfechoInutilizacao::JaInutilizada { .. } => {
            None
        }
        DesfechoInutilizacao::Rejeitada { .. } | DesfechoInutilizacao::ServicoDesativado { .. } => {
            Some(Erro::recusa("", r.frase.clone()))
        }
        _ => Some(Erro::Rede(r.frase.clone())),
    }
}
