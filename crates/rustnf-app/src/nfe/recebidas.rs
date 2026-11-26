// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Os documentos RECEBIDOS: a distribuicao de DF-e do Ambiente Nacional (as NF-e em que o CNPJ da empresa e' o
//! destinatario, e os eventos delas) e a manifestacao do destinatario. A biblioteca decide o proximo NSU e quando esperar;
//! esta camada grava os documentos e o cursor na pasta `nfe/recebidas` e diz o resultado em palavras.
//!
//! O que sai da maquina: [`sincronizar`], [`baixar`] e [`manifestar`] falam com o Ambiente Nacional; [`listar`] e
//! [`cursor`] so' leem a pasta.
//!
//! A regra do Ambiente Nacional (NT 2014.002 v1.02d, item 3, texto do servico e nota 1 da tabela de documentos): o destinatario so' recebe a NF-e COMPLETA depois de
//! manifestar (ciencia, confirmacao ou operacao nao realizada); antes disso recebe o resumo. O desconhecimento nao libera
//! a nota. Por isso [`baixar`] tem a opcao de dar a ciencia antes.

use chrono::{DateTime, FixedOffset, Utc};
use rustnf::nfe::sefaz::dfe::SituacaoResumo;
use rustnf::nfe::sefaz::dfe::{
    Conteudo, Cursor, DesfechoDfe, Doc, ErroSincronizacao, ResultadoDfe,
};
use rustnf::nfe::sefaz::evento::{Manifestacao, TipoManifestacao};
use rustnf::nfe::sefaz::{ResultadoEvento, Transporte};
use rustnf::nfe::vigencias::Ambiente;
use serde_json::{json, Value};

use super::resposta::{erro_do_evento, json_do_evento};
use super::{chave_limpa, Contexto};
use crate::erro::Erro;
use crate::notas::nfe::{NotasNfe, NotasRecebidas, Recebida, SituacaoRecebida};
use crate::notas::sem_acento;

/// Quantos lotes (de ate 50 documentos) uma sincronizacao le por chamada, se a pessoa nao disser.
pub const LOTES_PADRAO: usize = 20;
/// O maximo de lotes que uma chamada aceita.
pub const LOTES_MAXIMO: usize = 200;
/// Quantos documentos ilegiveis (que nao abriram) cada sincronizacao tenta reler.
const RELEITURAS: usize = 10;

/// O CNPJ (ou CPF) da empresa, que e' o interessado da distribuicao e o autor da manifestacao.
fn interessado(config: &crate::config::Config) -> Result<String, Erro> {
    let e = &config.emitente;
    let doc: String = [&e.cnpj, &e.cpf]
        .into_iter()
        .find(|d| !d.trim().is_empty())
        .map(|d| d.chars().filter(char::is_ascii_digit).collect())
        .unwrap_or_default();
    if doc.is_empty() {
        return Err(Erro::Uso(
            "informe o CNPJ da empresa em [emitente].cnpj do arquivo de configuracao: e' ele quem recebe os documentos"
                .into(),
        ));
    }
    Ok(doc)
}

fn ambiente_do(config: &crate::config::Config) -> (&'static str, Ambiente) {
    if config.nfe.ambiente == "producao" {
        ("producao", Ambiente::Producao)
    } else {
        ("homologacao", Ambiente::Homologacao)
    }
}

fn pasta_de<'a, T: Transporte>(ctx: &Contexto<'a, T>) -> Result<NotasRecebidas, Erro> {
    Ok(ctx.pasta()?.recebidas()?)
}

/// Quantos documentos de cada tipo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Contagem {
    /// `resNFe`.
    pub resumos: usize,
    /// `procNFe`.
    pub notas: usize,
    /// `resEvento` e `procEventoNFe`.
    pub eventos: usize,
    /// Outro esquema.
    pub outros: usize,
    /// Nao abriram: ficam para reler.
    pub ilegiveis: usize,
}

impl Contagem {
    fn de(docs: &[Doc]) -> Self {
        let mut c = Self::default();
        for d in docs {
            match d.conteudo {
                Conteudo::ResumoNfe(_) => c.resumos += 1,
                Conteudo::Nfe(_) => c.notas += 1,
                Conteudo::ResumoEvento(_) | Conteudo::Evento(_) => c.eventos += 1,
                Conteudo::Ilegivel { .. } => c.ilegiveis += 1,
                _ => c.outros += 1,
            }
        }
        c
    }

    /// O total.
    #[must_use]
    pub fn total(&self) -> usize {
        self.resumos + self.notas + self.eventos + self.outros + self.ilegiveis
    }

    fn em_palavras(&self) -> String {
        let partes: Vec<String> = [
            (self.resumos, "resumo(s) de NF-e"),
            (self.notas, "NF-e completa(s)"),
            (self.eventos, "evento(s)"),
            (self.outros, "de outro tipo"),
            (self.ilegiveis, "que nao abriram (serao relidos)"),
        ]
        .into_iter()
        .filter(|(n, _)| *n > 0)
        .map(|(n, o)| format!("{n} {o}"))
        .collect();
        if partes.is_empty() {
            String::new()
        } else {
            format!(" ({})", partes.join(", "))
        }
    }

    fn para_json(&self) -> Value {
        json!({
            "resumos": self.resumos, "notas": self.notas, "eventos": self.eventos,
            "outros": self.outros, "ilegiveis": self.ilegiveis,
        })
    }
}

fn nome_do_desfecho(d: &DesfechoDfe) -> &'static str {
    match d {
        DesfechoDfe::Parcial => "parcial",
        DesfechoDfe::EmDia => "em_dia",
        DesfechoDfe::Localizado => "localizado",
        DesfechoDfe::NaoLocalizado => "nao_localizado",
        DesfechoDfe::Aguarde { .. } => "aguarde",
        DesfechoDfe::ConsumoIndevido { .. } => "consumo_indevido",
        DesfechoDfe::Rejeitada { .. } => "rejeitada",
        DesfechoDfe::Indisponivel => "indisponivel",
        // desfecho novo da biblioteca: a frase dela diz o que houve
        _ => "outro",
    }
}

/// A hora de Brasilia (`HH:MM`) de um instante: o Ambiente Nacional conta em horas, e a frase e' para quem trabalha no
/// horario do pais.
fn hora_de_brasilia(t: DateTime<Utc>) -> String {
    FixedOffset::west_opt(3 * 3600).map_or_else(
        || t.format("%H:%M UTC").to_string(),
        |f| t.with_timezone(&f).format("%H:%M").to_string(),
    )
}

/// O erro do resultado da distribuicao: `None` se deu certo (inclusive "ainda nao e' hora", que nao e' falha).
fn erro_da_distribuicao(r: &ResultadoDfe, frase: &str) -> Option<Erro> {
    match &r.desfecho {
        DesfechoDfe::Parcial
        | DesfechoDfe::EmDia
        | DesfechoDfe::Localizado
        | DesfechoDfe::NaoLocalizado
        | DesfechoDfe::Aguarde { .. } => None,
        DesfechoDfe::ConsumoIndevido { .. } | DesfechoDfe::Rejeitada { .. } => {
            Some(Erro::recusa("", frase))
        }
        _ => Some(Erro::Rede(frase.to_string())),
    }
}

fn json_do_cursor(c: &Cursor, ilegiveis: &[String]) -> Value {
    json!({
        "ult_nsu": c.ult_nsu.to_string(),
        "max_nsu": c.max_nsu.map(|n| n.to_string()),
        "espera_ate": c.espera.map(|e| e.ate.to_rfc3339()),
        "lacunas": c.lacunas.iter().map(|f| json!([f.inicio.to_string(), f.fim.to_string()])).collect::<Vec<_>>(),
        "ilegiveis": ilegiveis,
    })
}

/// O resultado de [`sincronizar`] e o que ficou na pasta.
#[derive(Debug, Clone)]
pub struct Sincronizada {
    /// O resultado da biblioteca (com o cursor final).
    pub resultado: ResultadoDfe,
    /// Quantos documentos de cada tipo vieram (incluindo os relidos).
    pub contagem: Contagem,
    /// Quantos documentos que nao tinham aberto foram lidos agora.
    pub relidos: usize,
    /// A frase para a tela.
    pub frase: String,
    ilegiveis: Vec<String>,
}

impl Sincronizada {
    /// O JSON: `status`, `frase`, `cstat`, `lotes`, `documentos`, `tipos`, o cursor e as faixas fora do alcance.
    #[must_use]
    pub fn para_json(&self) -> Value {
        let r = &self.resultado;
        let mut v = json!({
            "status": nome_do_desfecho(&r.desfecho),
            "frase": self.frase,
            "cstat": r.cstat,
            "motivo": r.motivo,
            "lotes": r.lotes,
            "documentos": self.contagem.total(),
            "tipos": self.contagem.para_json(),
            "relidos": self.relidos,
            "cursor": json_do_cursor(&r.cursor, &self.ilegiveis),
        });
        if !r.irrecuperaveis.is_empty() {
            v["fora_do_alcance"] = json!(r
                .irrecuperaveis
                .iter()
                .map(|f| json!([f.inicio.to_string(), f.fim.to_string()]))
                .collect::<Vec<_>>());
        }
        v
    }

    /// O erro do resultado (`None` = deu certo, ou ainda nao e' hora de consultar).
    #[must_use]
    pub fn erro(&self) -> Option<Erro> {
        erro_da_distribuicao(&self.resultado, &self.frase)
    }
}

fn frase_da_sincronizacao(r: &ResultadoDfe, c: &Contagem) -> String {
    let total = c.total();
    let baixados = format!(
        "{total} documento(s) baixado(s) em {} lote(s){}",
        r.lotes,
        c.em_palavras()
    );
    let aviso_irrecuperavel = if r.irrecuperaveis.is_empty() {
        String::new()
    } else {
        " Houve NSU que o Ambiente Nacional ja nao guarda (so' guarda 90 dias): o que estava neles nao pode mais ser baixado."
            .to_string()
    };
    let corpo = match &r.desfecho {
        DesfechoDfe::Parcial => {
            format!("{baixados}; ainda ha mais na fila: rode de novo para continuar.")
        }
        DesfechoDfe::EmDia => {
            let quando = r
                .cursor
                .espera
                .map_or_else(String::new, |e| hora_de_brasilia(e.ate));
            format!(
                "{baixados}; a fila esta em dia. Consulte de novo so' depois das {quando} (horario de Brasilia): o Ambiente Nacional manda esperar uma hora."
            )
        }
        _ if total > 0 => format!("Antes de parar, {baixados}. {}", r.frase),
        _ => r.frase.clone(),
    };
    format!("{corpo}{aviso_irrecuperavel}")
}

/// Baixa o que ha de novo: do ultimo NSU ate o fim da fila, respeitando a espera de uma hora e o bloqueio (656). Cada lote
/// e' gravado na pasta ANTES de o cursor andar; uma queda no meio refaz o mesmo lote, nunca pula um NSU. Os documentos que
/// nao abriram numa vez anterior sao relidos (`consNSU`).
///
/// `lotes`: no maximo quantos lotes ler nesta chamada (1 a [`LOTES_MAXIMO`]; sem valor, [`LOTES_PADRAO`]).
///
/// # Errors
///
/// [`Erro::Uso`] sem `[emitente].cnpj` ou sem pasta de notas; [`Erro::Recusa`] se o CNPJ nao e' valido; [`Erro::Entrada`] se a
/// pasta nao grava ou o cursor esta ilegivel. O que o Ambiente Nacional responde (espera, bloqueio, rejeicao) volta em
/// [`Sincronizada`].
pub async fn sincronizar<T: Transporte>(
    ctx: &Contexto<'_, T>,
    lotes: Option<usize>,
) -> Result<Sincronizada, Erro> {
    let lotes = lotes.unwrap_or(LOTES_PADRAO);
    if !(1..=LOTES_MAXIMO).contains(&lotes) {
        return Err(Erro::Uso(format!(
            "lotes: use de 1 a {LOTES_MAXIMO} (cada lote traz ate 50 documentos)"
        )));
    }
    let (documento, (ambiente, _)) = (interessado(ctx.config)?, ambiente_do(ctx.config));
    let pasta = pasta_de(ctx)?;
    let cursor = pasta.cursor(ambiente, &documento)?;
    let r = ctx
        .cliente
        .sincronizar(
            &documento,
            &cursor,
            |docs: &[Doc], novo: &Cursor| pasta.gravar_lote(ambiente, &documento, docs, novo),
            lotes,
        )
        .await
        .map_err(|e| match e {
            ErroSincronizacao::Gravar(e) => Erro::from(e),
            ErroSincronizacao::Sefaz(e) => Erro::from(e),
            _ => Erro::Interno,
        })?;
    let mut contagem = Contagem::de(&r.documentos);
    let relidos = reler_ilegiveis(ctx, &pasta, &documento, ambiente, &r).await?;
    contagem.resumos += relidos.resumos;
    contagem.notas += relidos.notas;
    contagem.eventos += relidos.eventos;
    contagem.outros += relidos.outros;
    let frase = frase_da_sincronizacao(&r, &contagem);
    Ok(Sincronizada {
        ilegiveis: pasta.ilegiveis()?,
        relidos: relidos.total(),
        resultado: r,
        contagem,
        frase,
    })
}

/// Tenta reler (`consNSU`) os documentos que nao abriram numa sincronizacao anterior. Nao toca no cursor.
async fn reler_ilegiveis<T: Transporte>(
    ctx: &Contexto<'_, T>,
    pasta: &NotasRecebidas,
    documento: &str,
    ambiente: &str,
    feito: &ResultadoDfe,
) -> Result<Contagem, Erro> {
    let parado = matches!(
        feito.desfecho,
        DesfechoDfe::ConsumoIndevido { .. }
            | DesfechoDfe::Rejeitada { .. }
            | DesfechoDfe::Indisponivel
    );
    let mut lidos = Contagem::default();
    if parado {
        return Ok(lidos);
    }
    // os que chegaram NESTA sincronizacao ja foram tentados agora: ficam para a proxima
    let desta_vez: Vec<String> = feito
        .documentos
        .iter()
        .filter(|d| d.e_ilegivel())
        .map(|d| d.nsu.to_string())
        .collect();
    for texto in pasta
        .ilegiveis()?
        .into_iter()
        .filter(|n| !desta_vez.contains(n))
        .take(RELEITURAS)
    {
        let Ok(nsu) = rustnf::nfe::sefaz::dfe::Nsu::ler(&texto) else {
            continue;
        };
        let r = ctx
            .cliente
            .distribuir_nsu(documento, &feito.cursor, nsu)
            .await?;
        if !matches!(r.desfecho, DesfechoDfe::Localizado) {
            // espera, bloqueio ou falha: o resto fica para a proxima vez
            break;
        }
        let abertos: Vec<Doc> = r
            .documentos
            .into_iter()
            .filter(|d| !d.e_ilegivel())
            .collect();
        pasta.gravar_documentos(ambiente, &abertos)?;
        let c = Contagem::de(&abertos);
        lidos.resumos += c.resumos;
        lidos.notas += c.notas;
        lidos.eventos += c.eventos;
        lidos.outros += c.outros;
    }
    Ok(lidos)
}

/// O resultado de [`baixar`].
#[derive(Debug, Clone)]
pub struct Baixada {
    /// A chave pedida.
    pub chave: String,
    /// O resultado da biblioteca na ultima consulta.
    pub resultado: ResultadoDfe,
    /// A NF-e completa esta na pasta.
    pub completa: bool,
    /// A situacao da nota na pasta; `None` se ela nao esta.
    pub situacao: Option<SituacaoRecebida>,
    /// A ciencia da operacao registrada por este pedido (so' com `ciencia`).
    pub ciencia: Option<Manifestada>,
    /// A frase para a tela.
    pub frase: String,
}

impl Baixada {
    /// O JSON: `status`, `frase`, `chave`, `completa`, `situacao`, e a `ciencia` se foi dada.
    #[must_use]
    pub fn para_json(&self) -> Value {
        let mut v = json!({
            "status": nome_do_desfecho(&self.resultado.desfecho),
            "frase": self.frase,
            "chave": self.chave,
            "cstat": self.resultado.cstat,
            "completa": self.completa,
            "situacao": self.situacao.map(SituacaoRecebida::texto),
        });
        if let Some(c) = &self.ciencia {
            v["ciencia"] = c.para_json();
        }
        v
    }

    /// O erro do resultado: nao localizada, sem a nota completa ou falha na consulta.
    #[must_use]
    pub fn erro(&self) -> Option<Erro> {
        if let Some(e) = self.ciencia.as_ref().and_then(Manifestada::erro) {
            return Some(e);
        }
        match &self.resultado.desfecho {
            DesfechoDfe::Localizado if self.completa => None,
            DesfechoDfe::Localizado | DesfechoDfe::NaoLocalizado => {
                Some(Erro::recusa("chNFe", self.frase.clone()))
            }
            _ => erro_da_distribuicao(&self.resultado, &self.frase),
        }
    }
}

/// Uma consulta pela chave; grava o que veio e o cursor (que pode ter ganho a espera do 656).
async fn consultar_chave<T: Transporte>(
    ctx: &Contexto<'_, T>,
    pasta: &NotasRecebidas,
    documento: &str,
    ambiente: &str,
    chave: &str,
) -> Result<ResultadoDfe, Erro> {
    let cursor = pasta.cursor(ambiente, documento)?;
    let r = ctx
        .cliente
        .distribuir_chave(documento, &cursor, chave)
        .await?;
    if matches!(
        r.desfecho,
        DesfechoDfe::Localizado | DesfechoDfe::ConsumoIndevido { .. }
    ) {
        pasta.gravar_lote(ambiente, documento, &r.documentos, &r.cursor)?;
    }
    Ok(r)
}

/// Baixa a NF-e de UMA chave (`consChNFe`). O Ambiente Nacional so' entrega a nota completa ao destinatario que ja
/// manifestou; antes disso vem so' o resumo. Com `ciencia`, e se a nota ainda nao tem manifestacao nesta pasta, registra a
/// ciencia da operacao (210210, um evento fiscal) e consulta de novo.
///
/// # Errors
///
/// [`Erro::Uso`] sem `[emitente].cnpj` ou sem pasta de notas; [`Erro::Recusa`] se a chave nao e' de NF-e; os da ciencia.
pub async fn baixar<T: Transporte>(
    ctx: &Contexto<'_, T>,
    chave: &str,
    ciencia: bool,
) -> Result<Baixada, Erro> {
    let chave = chave_limpa(chave);
    let (documento, (ambiente, _)) = (interessado(ctx.config)?, ambiente_do(ctx.config));
    let pasta = pasta_de(ctx)?;
    let mut r = consultar_chave(ctx, &pasta, &documento, ambiente, &chave).await?;
    let mut dada = None;
    let resumo_so = |p: &NotasRecebidas| p.registro(&chave).ok().filter(|x| !x.completa);
    if ciencia && matches!(r.desfecho, DesfechoDfe::Localizado) {
        if let Some(x) = resumo_so(&pasta).filter(|x| x.manifestacao.is_empty()) {
            let m = manifestar(ctx, &x.chave, TipoManifestacao::Ciencia, None).await?;
            let registrada = m.resultado.registrado();
            dada = Some(m);
            if registrada {
                r = consultar_chave(ctx, &pasta, &documento, ambiente, &chave).await?;
            }
        }
    }
    let linha = pasta.registro(&chave).ok();
    let completa = linha.as_ref().is_some_and(|x| x.completa);
    let frase = frase_da_baixa(&r, completa, linha.as_ref(), dada.as_ref());
    Ok(Baixada {
        chave,
        completa,
        situacao: linha.map(|x| x.situacao),
        ciencia: dada,
        resultado: r,
        frase,
    })
}

fn frase_da_baixa(
    r: &ResultadoDfe,
    completa: bool,
    linha: Option<&Recebida>,
    ciencia: Option<&Manifestada>,
) -> String {
    match (&r.desfecho, completa, linha) {
        (DesfechoDfe::Localizado, true, _) => {
            "NF-e completa baixada e guardada na pasta de notas recebidas.".to_string()
        }
        (DesfechoDfe::Localizado, false, _) if ciencia.is_some_and(|c| !c.resultado.registrado()) => {
            format!(
                "Veio so' o resumo e a ciencia da operacao NAO foi registrada: {}",
                ciencia.map(|c| c.resultado.frase.as_str()).unwrap_or_default()
            )
        }
        (DesfechoDfe::Localizado, false, Some(x)) if x.manifestacao.is_empty() => {
            "Veio so' o resumo: o Ambiente Nacional so' entrega a NF-e completa a quem ja manifestou. Repita com --ciencia (registra a ciencia da operacao, um evento fiscal) ou manifeste antes."
                .to_string()
        }
        (DesfechoDfe::Localizado, false, Some(x)) if x.manifestacao == "210220" => {
            "Veio so' o resumo: o desconhecimento da operacao nao libera a NF-e completa (so' ciencia, confirmacao ou operacao nao realizada)."
                .to_string()
        }
        (DesfechoDfe::Localizado, false, _) => {
            "Veio so' o resumo mesmo depois da manifestacao: a nota completa chega na proxima sincronizacao (pode levar alguns minutos)."
                .to_string()
        }
        _ => r.frase.clone(),
    }
}

/// O resultado de [`manifestar`] e o que ficou na pasta.
#[derive(Debug, Clone)]
pub struct Manifestada {
    /// A chave da NF-e.
    pub chave: String,
    /// O resultado da biblioteca (com o `procEventoNFe`, se registrada).
    pub resultado: ResultadoEvento,
    /// O `procEventoNFe` foi guardado na pasta dos recebidos.
    pub guardado: bool,
    /// A situacao da nota na pasta depois da manifestacao.
    pub situacao: Option<SituacaoRecebida>,
}

impl Manifestada {
    /// O JSON: o do evento mais `guardado` e `situacao`.
    #[must_use]
    pub fn para_json(&self) -> Value {
        let mut v = json_do_evento(&self.resultado);
        v["guardado"] = json!(self.guardado);
        v["situacao"] = json!(self.situacao.map(SituacaoRecebida::texto));
        v
    }

    /// O erro do resultado (`None` = registrada).
    #[must_use]
    pub fn erro(&self) -> Option<Erro> {
        erro_do_evento(&self.resultado)
    }
}

/// Registra a manifestacao do destinatario (um EVENTO FISCAL no Ambiente Nacional): ciencia (210210), confirmacao
/// (210200), desconhecimento (210220) ou operacao nao realizada (210240, com justificativa de 15 a 255 caracteres). O
/// evento e' assinado com o certificado da empresa e vale a ultima manifestacao conclusiva (nao ha desfazer). O evento
/// registrado e' guardado ao lado da nota. A sequencia e' sempre 1 (MOC 7.00, 5.11); o que a pasta sabe da nota (cancelada,
/// denegada, a ultima manifestacao) confere o pedido antes da rede.
///
/// # Errors
///
/// [`Erro::Uso`] sem `[emitente].cnpj`, sem `[nfe].uf` ou sem pasta; [`Erro::Recusa`] se o pedido nao serve (justificativa,
/// combinacao que o Ambiente Nacional recusaria, certificado de outro CNPJ). O que o Ambiente Nacional responde volta em
/// [`Manifestada`].
pub async fn manifestar<T: Transporte>(
    ctx: &Contexto<'_, T>,
    chave: &str,
    tipo: TipoManifestacao,
    justificativa: Option<&str>,
) -> Result<Manifestada, Erro> {
    let chave = chave_limpa(chave);
    let (documento, (ambiente, amb)) = (interessado(ctx.config)?, ambiente_do(ctx.config));
    let pasta = pasta_de(ctx)?;
    let linha = pasta.registro(&chave).ok();
    let situacao_nfe = linha.as_ref().and_then(|x| match x.situacao {
        SituacaoRecebida::Cancelada => Some(SituacaoResumo::Cancelada),
        SituacaoRecebida::Denegada => Some(SituacaoResumo::Denegada),
        _ => None,
    });
    let pedido = Manifestacao {
        documento: &documento,
        chave: &chave,
        tipo,
        justificativa,
        situacao_nfe,
        anterior: linha
            .as_ref()
            .and_then(|x| TipoManifestacao::de_codigo(&x.manifestacao)),
        uf_autor: &ctx.config.nfe.uf,
        ambiente: amb,
        quando: ctx.agora.fixed_offset(),
    };
    let r = ctx.cliente.manifestar(ctx.certificado, &pedido).await?;
    let (guardado, situacao) = match (&r.desfecho, r.proc_evento.as_deref()) {
        (rustnf::nfe::sefaz::DesfechoEvento::Registrado { protocolo, .. }, Some(proc)) => {
            pasta.garantir_linha(&chave, ambiente)?;
            let nova = pasta.guardar_manifestacao(&chave, tipo.codigo(), protocolo, proc)?;
            (true, Some(nova.situacao))
        }
        _ => (false, linha.map(|x| x.situacao)),
    };
    Ok(Manifestada {
        chave,
        resultado: r,
        guardado,
        situacao,
    })
}

/// O filtro de [`listar`].
#[derive(Debug, Clone, Default)]
pub struct FiltroRecebidas {
    /// So' as notas nesta situacao.
    pub situacao: Option<SituacaoRecebida>,
    /// So' as que casam com o texto: nome do emitente (sem acento), CNPJ/CPF dele ou chave.
    pub busca: String,
}

/// As NF-e recebidas da pasta, da mais recente para a mais antiga. So' le a pasta: nao usa a rede.
///
/// # Errors
///
/// [`Erro::Entrada`] se o indice esta ilegivel.
pub fn listar(notas: &NotasNfe, filtro: &FiltroRecebidas) -> Result<Vec<Recebida>, Erro> {
    let busca = sem_acento(filtro.busca.trim());
    let digitos: String = busca.chars().filter(char::is_ascii_digit).collect();
    Ok(notas
        .recebidas()?
        .listar()?
        .into_iter()
        .filter(|r| filtro.situacao.is_none_or(|s| r.situacao == s))
        .filter(|r| {
            busca.is_empty()
                || sem_acento(&r.emitente_nome).contains(&busca)
                || (!digitos.is_empty()
                    && (r.emitente.contains(&digitos) || r.chave.contains(&digitos)))
        })
        .collect())
}

/// O cursor da distribuicao e os documentos que nao abriram, como estao na pasta. So' le a pasta.
///
/// # Errors
///
/// [`Erro::Uso`] sem `[emitente].cnpj`; [`Erro::Entrada`] se o cursor esta ilegivel.
pub fn cursor(notas: &NotasNfe, config: &crate::config::Config) -> Result<Value, Erro> {
    let documento = interessado(config)?;
    let (ambiente, _) = ambiente_do(config);
    let pasta = notas.recebidas()?;
    let mut v = json_do_cursor(&pasta.cursor(ambiente, &documento)?, &pasta.ilegiveis()?);
    v["ambiente"] = json!(ambiente);
    v["documento"] = json!(documento);
    Ok(v)
}
