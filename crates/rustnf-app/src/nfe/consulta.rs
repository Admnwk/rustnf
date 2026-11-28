// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Status do servico, consulta pela chave e DANFE.

use rustnf::danfe::{self, Opcoes};
use rustnf::nfe::sefaz::{ClienteSefaz, Resultado, SituacaoServico, StatusServico, Transporte};
use serde_json::{json, Value};

use super::emissao::aplicar_resultado;
use super::resposta::{erro_do_resultado, json_do_resultado};
use super::{chave_limpa, Contexto};
use crate::config::Config;
use crate::erro::Erro;
use crate::notas::nfe::{NotasNfe, Situacao};

/// O estado do servico da SEFAZ (ou da SVC, com `svc`), com `ambiente` e `uf` do cliente.
///
/// # Errors
///
/// [`Erro::Recusa`] se o pedido nao monta (UF desconhecida).
pub async fn status<T: Transporte>(
    cliente: &ClienteSefaz<T>,
    svc: bool,
) -> Result<StatusServico, Erro> {
    Ok(if svc {
        cliente.status_svc().await?
    } else {
        cliente.status().await?
    })
}

/// O JSON do status: `status` (`em_operacao`, `paralisado`, `sem_resposta`, `outra`), `frase`, `cstat`, `motivo`,
/// `tempo_medio`, `uf` e `ambiente`.
#[must_use]
pub fn estado_do_servico<T: Transporte>(cliente: &ClienteSefaz<T>, s: &StatusServico) -> Value {
    let nome = match s.situacao {
        SituacaoServico::EmOperacao => "em_operacao",
        SituacaoServico::Paralisado => "paralisado",
        SituacaoServico::SemResposta => "sem_resposta",
        _ => "outra",
    };
    json!({
        "status": nome,
        "frase": s.frase,
        "cstat": s.cstat,
        "motivo": s.motivo,
        "tempo_medio": s.tempo_medio,
        "retorno_previsto": s.retorno_previsto,
        "uf": cliente.uf(),
        "ambiente": match cliente.ambiente() {
            rustnf::nfe::vigencias::Ambiente::Producao => "producao",
            _ => "homologacao",
        },
    })
}

/// O que a consulta achou e o que se fez com a nota da pasta.
#[derive(Debug, Clone)]
pub struct Consultada {
    /// O resultado da biblioteca.
    pub resultado: Resultado,
    /// A situacao da nota na pasta depois da consulta; `None` = a nota nao esta na pasta.
    pub situacao: Option<Situacao>,
}

impl Consultada {
    /// O JSON: o do resultado mais `situacao`.
    #[must_use]
    pub fn para_json(&self) -> Value {
        let mut v = json_do_resultado(&self.resultado);
        v["situacao"] = json!(self.situacao.map(Situacao::texto));
        v
    }

    /// O erro da consulta: `None` = autorizada; denegada e cancelada sao recusa (saida 3); a sem resposta e a consulta
    /// recusada pela SEFAZ, falha de rede (saida 4).
    #[must_use]
    pub fn erro(&self) -> Option<Erro> {
        erro_do_resultado(&self.resultado)
    }
}

/// Consulta a NF-e pela chave. Se a nota esta na pasta, a situacao dela e' posta em dia com o que a SEFAZ diz (a pendente
/// vira autorizada ou denegada; a autorizada vira cancelada). A consulta que a SEFAZ recusa (o consumo indevido 656, por
/// exemplo) e' recusa do pedido, nao da nota: a pasta fica como estava.
///
/// # Errors
///
/// [`Erro::Recusa`] se a chave nao e' de uma NF-e valida; erro de gravacao na pasta.
pub async fn consultar<T: Transporte>(
    ctx: &Contexto<'_, T>,
    chave: &str,
) -> Result<Consultada, Erro> {
    let chave = chave_limpa(chave);
    let guardada = ctx.notas.and_then(|n| n.registro(&chave).ok());
    let assinada = match (ctx.notas, &guardada) {
        (Some(n), Some(_)) => n.ler_assinada(&chave)?,
        _ => None,
    };
    let resultado = ctx.cliente.consultar(&chave, assinada.as_deref()).await?;
    let situacao = match (ctx.notas, &guardada) {
        (Some(n), Some(r)) => aplicar_resultado(n, r, &resultado)?,
        _ => None,
    };
    Ok(Consultada {
        resultado,
        situacao,
    })
}

/// O DANFE em PDF a partir do XML (`nfeProc`, ou `NFe` solta como previa). Sincrono e pesado: dentro de um servidor, em
/// tarefa propria.
///
/// # Errors
///
/// [`Erro::Entrada`] para XML que nao e' NF-e modelo 55 ou nota rejeitada; [`Erro::Interno`] se o PDF nao se escreve.
pub fn danfe_do_xml(
    xml: &str,
    canhoto: bool,
    cancelada: bool,
    protocolo_epec: Option<&str>,
) -> Result<Vec<u8>, Erro> {
    let opcoes = Opcoes {
        canhoto,
        cancelada,
        protocolo_epec,
        ..Opcoes::default()
    };
    Ok(danfe::pdf(xml.as_bytes(), &opcoes)?)
}

/// O DANFE de uma nota da pasta, com a marca de cancelada e o protocolo do EPEC que a pasta guardou. A nota so' com a NF-e
/// assinada (pendente) sai como previa, sem protocolo.
///
/// # Errors
///
/// [`Erro::NaoEncontrada`] se a nota nao esta na pasta; os de [`danfe_do_xml`].
pub fn danfe_da_pasta(notas: &NotasNfe, config: &Config, chave: &str) -> Result<Vec<u8>, Erro> {
    let chave = chave_limpa(chave);
    let registro = notas.registro(&chave)?;
    let xml = notas.ler_xml(&chave)?;
    let epec = Some(registro.protocolo_epec.as_str()).filter(|p| !p.is_empty());
    danfe_do_xml(
        &xml,
        config.nfe.canhoto,
        registro.situacao == Situacao::Cancelada,
        epec,
    )
}
