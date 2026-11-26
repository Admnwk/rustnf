// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A retomada do que ficou sem resultado: a varredura do limbo da biblioteca sobre a pasta de notas.
//!
//! A nota pendente e' consultada pela MESMA chave; se a SEFAZ diz que nao a recebeu, a MESMA NF-e assinada, que esta na pasta,
//! e' reenviada. Em nenhum caminho nasce outra nota. A nota em EPEC e' transmitida a SEFAZ da UF (o MOC 7.00, Visao Geral,
//! 3.3.6, da 168 horas depois do EPEC).

use rustnf::nfe::sefaz::limbo::{self, Acao, Estado, NotaLocal, Politica};
use rustnf::nfe::sefaz::Transporte;
use serde_json::{json, Value};

use super::emissao::aplicar_resultado;
use super::resposta::{json_do_resultado, nome_do_desfecho};
use super::Contexto;
use crate::erro::Erro;
use crate::notas::nfe::{Registro, Situacao};

/// O que a retomada fez com uma nota.
#[derive(Debug, Clone)]
pub struct Retomada {
    /// Chave da nota.
    pub chave: String,
    /// Serie.
    pub serie: String,
    /// Numero.
    pub numero: String,
    /// `consultou`, `reenviou`, `transmitiu` (a nota em EPEC) ou `nada` (nem chegou a rede).
    pub acao: &'static str,
    /// O resultado da SEFAZ, em JSON (`status`, `frase`, `cstat`...); `Null` se nao chegou a rede.
    pub resultado: Value,
    /// A situacao da nota na pasta depois da retomada; `None` = foi rejeitada e saiu da pasta.
    pub situacao: Option<Situacao>,
    /// Aviso para a pessoa, se houver.
    pub aviso: Option<String>,
    /// O nome do desfecho (`autorizada`, `sem_resposta`...), vazio se nao chegou a rede.
    pub status: &'static str,
}

impl Retomada {
    /// O JSON: `chave`, `serie`, `numero`, `acao`, `status`, `situacao`, `aviso` e `resultado`.
    #[must_use]
    pub fn para_json(&self) -> Value {
        json!({
            "chave": self.chave,
            "serie": self.serie,
            "numero": self.numero,
            "acao": self.acao,
            "status": self.status,
            "situacao": self.situacao.map(Situacao::texto),
            "aviso": self.aviso,
            "resultado": self.resultado,
        })
    }
}

fn local(r: &Registro, nfe_assinada: String) -> NotaLocal {
    NotaLocal {
        chave: r.chave.clone(),
        estado: Estado::Pendente,
        desde: r.desde,
        tentativas: r.tentativas,
        ultima: r.ultima,
        nfe_assinada,
    }
}

/// Retoma TODAS as notas pendentes e em EPEC da pasta, uma a uma: a pendente e' consultada (e reenviada, se a SEFAZ diz que
/// nao a recebeu); a em EPEC e' transmitida. A pessoa pede a retomada, entao a espera entre as consultas e' so' a de 1
/// segundo, o minimo que a biblioteca admite.
///
/// # Errors
///
/// [`Erro::Uso`] sem pasta de notas e erro de gravacao na pasta. Falha de uma nota (rede, erro de uso) nao interrompe as outras: volta no
/// `aviso` dela.
pub async fn retomar<T: Transporte>(ctx: &Contexto<'_, T>) -> Result<Vec<Retomada>, Erro> {
    let notas = ctx.pasta()?;
    let todas = notas.a_resolver()?;
    let agora = ctx.segundos();
    let pendentes: Vec<NotaLocal> = todas
        .iter()
        .filter(|r| r.situacao == Situacao::Pendente)
        .map(|r| {
            local(
                r,
                notas
                    .ler_assinada(&r.chave)
                    .ok()
                    .flatten()
                    .unwrap_or_default(),
            )
        })
        .collect();
    let politica = Politica {
        primeira_apos: 1,
        espera_inicial: 1,
        fator: 1,
        espera_max: 1,
        aviso_apos: 1800,
        max_por_rodada: pendentes.len().max(1),
    };
    let mut saida = Vec::new();
    for passo in limbo::varrer(ctx.cliente, &pendentes, agora, &politica).await {
        let Some(r) = todas.iter().find(|r| r.chave == passo.chave) else {
            continue;
        };
        let (situacao, resultado, status) = match &passo.resultado {
            Some(res) => (
                aplicar_resultado(notas, r, res)?,
                json_do_resultado(res),
                nome_do_desfecho(&res.desfecho),
            ),
            None => (Some(r.situacao), Value::Null, ""),
        };
        if situacao == Some(Situacao::Pendente) {
            notas.marcar_tentativa(&r.chave, agora)?;
        }
        saida.push(Retomada {
            chave: r.chave.clone(),
            serie: r.serie.clone(),
            numero: r.numero.clone(),
            acao: match passo.acao {
                Acao::Consultou => "consultou",
                Acao::Reenviou => "reenviou",
                _ => "nada",
            },
            resultado,
            situacao,
            aviso: passo.aviso,
            status,
        });
    }
    for r in todas.iter().filter(|r| r.situacao == Situacao::Epec) {
        saida.push(transmitir(ctx, r).await?);
    }
    Ok(saida)
}

/// Transmite a NF-e em EPEC a SEFAZ da UF, a MESMA que o EPEC registrou.
async fn transmitir<T: Transporte>(ctx: &Contexto<'_, T>, r: &Registro) -> Result<Retomada, Erro> {
    let notas = ctx.pasta()?;
    let base = Retomada {
        chave: r.chave.clone(),
        serie: r.serie.clone(),
        numero: r.numero.clone(),
        acao: "transmitiu",
        resultado: Value::Null,
        situacao: Some(r.situacao),
        aviso: None,
        status: "",
    };
    let Some(assinada) = notas.ler_assinada(&r.chave)? else {
        return Ok(Retomada {
            aviso: Some(
                "a NF-e assinada do EPEC nao esta na pasta: nao ha o que transmitir".into(),
            ),
            ..base
        });
    };
    match ctx.cliente.autorizar(&assinada).await {
        Ok(res) => Ok(Retomada {
            situacao: aplicar_resultado(notas, r, &res)?,
            resultado: json_do_resultado(&res),
            status: nome_do_desfecho(&res.desfecho),
            ..base
        }),
        Err(e) => Ok(Retomada {
            aviso: Some(e.to_string()),
            acao: "nada",
            ..base
        }),
    }
}

/// O erro do lote retomado, ou `None` se tudo se resolveu: notas que seguem sem resultado sao falha de rede (nao emita outra
/// no lugar); notas que a SEFAZ recusou ou denegou sao recusa de regra.
#[must_use]
pub fn erro_da_retomada(feitos: &[Retomada]) -> Option<Erro> {
    let sem = feitos
        .iter()
        .filter(|f| matches!(f.situacao, Some(Situacao::Pendente | Situacao::Epec)))
        .count();
    if sem > 0 {
        return Some(Erro::Rede(format!(
            "{sem} nota(s) seguem sem resultado: tente de novo mais tarde e nao emita outra no lugar"
        )));
    }
    let recusadas = feitos
        .iter()
        .filter(|f| matches!(f.status, "rejeitada" | "denegada" | "numero_ja_usado"))
        .count();
    (recusadas > 0).then(|| {
        Erro::recusa(
            "",
            format!("{recusadas} nota(s) foram recusadas ou denegadas pela SEFAZ"),
        )
    })
}
