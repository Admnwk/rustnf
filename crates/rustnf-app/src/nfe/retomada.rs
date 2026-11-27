// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A retomada do que ficou sem resultado: a varredura do limbo da biblioteca sobre a pasta de notas.
//!
//! A nota pendente e' consultada pela MESMA chave; se a SEFAZ diz que nao a recebeu, a MESMA NF-e assinada, que esta na pasta,
//! e' reenviada. Em nenhum caminho nasce outra nota. A nota em EPEC e' transmitida a SEFAZ da UF (o MOC 7.00, Visao Geral,
//! 3.3.6, da 168 horas depois do EPEC).
//!
//! A pendente SUBSTITUIDA (a venda dela saiu em outro EPEC, [`super::epec_substituindo`]) e' so' CONSULTADA, nunca reenviada:
//! reenvia-la poderia deixar duas notas autorizadas para uma venda. O Manual de Contingencia da NF-e (MOC 7.00, Anexo III,
//! item 2.3.3) manda, quando a operacao normal volta, CANCELAR a pendente que saiu autorizada e INUTILIZAR a numeracao da que
//! nao foi autorizada nem denegada; a retomada diz qual dos dois e' o caso.

use rustnf::nfe::sefaz::limbo::{self, Acao, Estado, NotaLocal, Politica};
use rustnf::nfe::sefaz::{Desfecho, Transporte};
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
    /// A chave do EPEC que substituiu esta nota (vazio = nao foi substituida). Vai no `resultado` da consulta.
    #[must_use]
    pub fn substituida_por(&self) -> &str {
        self.resultado
            .get("substituida_por")
            .and_then(Value::as_str)
            .unwrap_or_default()
    }

    /// O que a pessoa tem de fazer com a nota substituida, pela norma (MOC 7.00, Anexo III, item 2.3.3): `cancelar` (a SEFAZ
    /// a autorizou) ou `inutilizar` (a SEFAZ nao a tem); vazio = nada. Vai no `resultado` da consulta.
    #[must_use]
    pub fn a_fazer(&self) -> &str {
        self.resultado
            .get("a_fazer")
            .and_then(Value::as_str)
            .unwrap_or_default()
    }

    /// O JSON: `chave`, `serie`, `numero`, `acao`, `status`, `situacao`, `aviso` e `resultado`; e, na nota substituida,
    /// `substituida_por` e `a_fazer`.
    #[must_use]
    pub fn para_json(&self) -> Value {
        let mut v = json!({
            "chave": self.chave,
            "serie": self.serie,
            "numero": self.numero,
            "acao": self.acao,
            "status": self.status,
            "situacao": self.situacao.map(Situacao::texto),
            "aviso": self.aviso,
            "resultado": self.resultado,
        });
        if !self.substituida_por().is_empty() {
            v["substituida_por"] = json!(self.substituida_por());
        }
        if !self.a_fazer().is_empty() {
            v["a_fazer"] = json!(self.a_fazer());
        }
        v
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
    // a substituida so' e' consultada (abaixo): fora desta lista a biblioteca nunca a recebe, e so' ela reenvia
    let (substituidas, comuns): (Vec<&Registro>, Vec<&Registro>) = todas
        .iter()
        .filter(|r| r.situacao == Situacao::Pendente)
        .partition(|r| !r.substituida_por.is_empty());
    let pendentes: Vec<NotaLocal> = comuns
        .iter()
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
    for r in substituidas {
        saida.extend(consultar_substituida(ctx, r, agora, &politica).await?);
    }
    for r in todas.iter().filter(|r| r.situacao == Situacao::Epec) {
        saida.push(transmitir(ctx, r).await?);
    }
    Ok(saida)
}

/// Consulta a pendente substituida pela chave, SEM reenvia-la, e diz o que a norma manda fazer com ela. A NF-e assinada so'
/// vai para a consulta montar o `nfeProc` se a SEFAZ a autorizou. `None` = ainda nao e' a hora da proxima consulta.
async fn consultar_substituida<T: Transporte>(
    ctx: &Contexto<'_, T>,
    r: &Registro,
    agora: i64,
    politica: &Politica,
) -> Result<Option<Retomada>, Erro> {
    let notas = ctx.pasta()?;
    if limbo::devidas(&[local(r, String::new())], agora, politica).is_empty() {
        return Ok(None);
    }
    let base = Retomada {
        chave: r.chave.clone(),
        serie: r.serie.clone(),
        numero: r.numero.clone(),
        acao: "consultou",
        resultado: Value::Null,
        situacao: Some(Situacao::Pendente),
        aviso: None,
        status: "",
    };
    let assinada = notas.ler_assinada(&r.chave).ok().flatten();
    let res = match ctx.cliente.consultar(&r.chave, assinada.as_deref()).await {
        Ok(res) => res,
        Err(e) => {
            notas.marcar_tentativa(&r.chave, agora)?;
            return Ok(Some(Retomada {
                acao: "nada",
                aviso: Some(format!("Nota {}: {e}", r.chave)),
                ..base
            }));
        }
    };
    let epec = notas
        .registro(&r.substituida_por)
        .map_or_else(|_| r.substituida_por.clone(), |e| e.numero);
    let nao_consta = res.desfecho == Desfecho::NaoRecebida;
    let situacao = if nao_consta {
        Some(Situacao::Pendente)
    } else {
        aplicar_resultado(notas, r, &res)?
    };
    if situacao == Some(Situacao::Pendente) {
        // o 217 anotado e' o que libera a inutilizacao; qualquer outra resposta o desfaz
        let cstat = if nao_consta {
            Registro::CSTAT_NAO_CONSTA
        } else {
            ""
        };
        notas.anotar_consulta(&r.chave, cstat)?;
        notas.marcar_tentativa(&r.chave, agora)?;
    }
    let (a_fazer, aviso) = match (&res.desfecho, situacao) {
        (_, Some(Situacao::Autorizada)) => (
            "cancelar",
            Some(format!(
                "a venda ja esta na NF-e {epec}: cancele esta nota (MOC 7.00, Anexo III, item 2.3.3)"
            )),
        ),
        (_, Some(Situacao::Denegada)) => (
            "",
            Some(format!(
                "a SEFAZ denegou esta nota: o numero {} fica consumido e a venda esta na NF-e {epec}",
                r.numero
            )),
        ),
        (Desfecho::NaoRecebida, _) => (
            "inutilizar",
            Some(format!(
                "a SEFAZ nao tem esta nota: inutilize o numero {} (MOC 7.00, Anexo III, item 2.3.3)",
                r.numero
            )),
        ),
        (Desfecho::Rejeitada { cstat, motivo }, _) => (
            "",
            Some(format!("Nota {}: recusada ({cstat}): {motivo}", r.chave)),
        ),
        _ => ("", None),
    };
    let mut resultado = json_do_resultado(&res);
    resultado["substituida_por"] = json!(r.substituida_por);
    if !a_fazer.is_empty() {
        resultado["a_fazer"] = json!(a_fazer);
    }
    Ok(Some(Retomada {
        resultado,
        situacao,
        aviso,
        status: nome_do_desfecho(&res.desfecho),
        ..base
    }))
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

/// A lista de pendentes em JSON: a linha do indice de cada nota e, na pendente substituida, o que falta fazer (`a_fazer`).
///
/// # Errors
///
/// Erro de serializacao da linha do indice.
pub fn pendentes_json(lista: &[Registro]) -> Result<Value, Erro> {
    let mut saida = Vec::with_capacity(lista.len());
    for r in lista {
        let mut v = serde_json::to_value(r)?;
        if !r.a_fazer().is_empty() {
            v["a_fazer"] = json!(r.a_fazer());
        }
        saida.push(v);
    }
    Ok(Value::Array(saida))
}

/// O erro do lote retomado, ou `None` se tudo se resolveu: notas que seguem sem resultado sao falha de rede (nao emita outra
/// no lugar); notas que a SEFAZ recusou ou denegou sao recusa de regra.
#[must_use]
pub fn erro_da_retomada(feitos: &[Retomada]) -> Option<Erro> {
    // a substituida que a SEFAZ nao tem espera a inutilizacao, nao outra tentativa
    let sem = feitos
        .iter()
        .filter(|f| matches!(f.situacao, Some(Situacao::Pendente | Situacao::Epec)))
        .filter(|f| f.a_fazer() != "inutilizar")
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
