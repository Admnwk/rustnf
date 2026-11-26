// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Cancelamento, carta de correcao, EPEC e inutilizacao: o evento sai pelo cliente da biblioteca e, registrado, e'
//! guardado na pasta ao lado da nota.

use chrono::Datelike;
use rustnf::nfe::montagem::{self, Aviso};
use rustnf::nfe::sefaz::evento::{CARTA_CORRECAO, EPEC, MAX_SEQ_CARTA};
use rustnf::nfe::sefaz::{
    DesfechoEvento, DesfechoInutilizacao, PedidoInutilizar, ResultadoEvento, ResultadoInutilizacao,
    Transporte,
};
use serde_json::{json, Value};

use super::emissao::preparar_e_gravar;
use super::pedido::avisos_json;
use super::resposta::{erro_da_inutilizacao, erro_do_evento, json_da_inutilizacao, json_do_evento};
use super::{chave_limpa, Contexto};
use crate::erro::Erro;
use crate::notas::nfe::{EventoGuardado, Inutilizacao, Ocupante, Registro, Situacao};

/// O resultado de um evento e o que ficou na pasta.
#[derive(Debug, Clone)]
pub struct Registrado {
    /// A chave da nota.
    pub chave: String,
    /// O resultado da biblioteca (com o `procEventoNFe`, se registrado).
    pub resultado: ResultadoEvento,
    /// O `procEventoNFe` foi guardado na pasta de notas.
    pub guardado: bool,
    /// A situacao da nota na pasta depois do evento; `None` se a nota nao esta na pasta.
    pub situacao: Option<Situacao>,
    /// Os avisos da montagem (so' no EPEC).
    pub avisos: Vec<Aviso>,
}

impl Registrado {
    /// O JSON do evento: o do resultado mais `guardado`, `situacao` e `avisos`.
    #[must_use]
    pub fn para_json(&self) -> Value {
        let mut v = json_do_evento(&self.resultado);
        v["guardado"] = json!(self.guardado);
        v["situacao"] = json!(self.situacao.map(Situacao::texto));
        if !self.avisos.is_empty() {
            v["avisos"] = avisos_json(&self.avisos);
        }
        v
    }

    /// O erro do resultado (`None` = registrado).
    #[must_use]
    pub fn erro(&self) -> Option<Erro> {
        erro_do_evento(&self.resultado)
    }
}

/// O resultado da inutilizacao e o que ficou na pasta.
#[derive(Debug, Clone)]
pub struct Inutilizada {
    /// O resultado da biblioteca (com o `ProcInutNFe`, se inutilizada).
    pub resultado: ResultadoInutilizacao,
    /// A faixa foi guardada na pasta de notas.
    pub guardada: bool,
}

impl Inutilizada {
    /// O JSON da inutilizacao: o do resultado mais `guardada`.
    #[must_use]
    pub fn para_json(&self) -> Value {
        let mut v = json_da_inutilizacao(&self.resultado);
        v["guardada"] = json!(self.guardada);
        v
    }

    /// O erro do resultado (`None` = faixa inutilizada).
    #[must_use]
    pub fn erro(&self) -> Option<Erro> {
        erro_da_inutilizacao(&self.resultado)
    }
}

/// A nota da pasta, se a pasta existe e a nota consta dela.
fn local<T: Transporte>(ctx: &Contexto<'_, T>, chave: &str) -> Option<Registro> {
    ctx.notas.and_then(|n| n.registro(chave).ok())
}

/// Guarda o evento registrado ao lado da nota (se ela esta na pasta) e devolve a situacao dela.
fn guardar<T: Transporte>(
    ctx: &Contexto<'_, T>,
    chave: &str,
    r: &ResultadoEvento,
) -> Result<(bool, Option<Situacao>), Erro> {
    let (Some(notas), Some(proc), DesfechoEvento::Registrado { protocolo, .. }) =
        (ctx.notas, r.proc_evento.as_deref(), &r.desfecho)
    else {
        return Ok((false, local(ctx, chave).map(|x| x.situacao)));
    };
    if notas.registro(chave).is_err() {
        return Ok((false, None));
    }
    let epec = if r.tp_evento == EPEC {
        crate::notas::nfe::protocolo_epec(proc).unwrap_or_default()
    } else {
        String::new()
    };
    let evento = EventoGuardado {
        tipo: r.tp_evento.clone(),
        sequencia: r.seq,
        protocolo: protocolo.clone(),
    };
    notas.guardar_evento(chave, &evento, proc, &epec)?;
    Ok((true, notas.registro(chave).ok().map(|x| x.situacao)))
}

/// Cancela a NF-e (evento 110111). O protocolo da AUTORIZACAO vem da pasta; para a nota que nao esta nela,
/// `protocolo` e' obrigatorio.
///
/// # Errors
///
/// [`Erro::Uso`] sem o protocolo; [`Erro::Recusa`] se a nota da pasta nao esta autorizada ou o pedido nao monta
/// (justificativa de 15 a 255 caracteres). O que a SEFAZ responde, inclusive o prazo vencido (501), volta em
/// [`Registrado`].
pub async fn cancelar<T: Transporte>(
    ctx: &Contexto<'_, T>,
    chave: &str,
    justificativa: &str,
    protocolo: Option<&str>,
) -> Result<Registrado, Erro> {
    let chave = chave_limpa(chave);
    let nprot = match local(ctx, &chave) {
        Some(r) if r.situacao == Situacao::Autorizada => r.protocolo,
        Some(r) => {
            return Err(Erro::recusa(
                "chNFe",
                format!(
                    "so' se cancela nota autorizada, e esta esta {}",
                    r.situacao.texto()
                ),
            ))
        }
        None => protocolo.map(str::to_string).ok_or_else(|| {
            Erro::Uso(
                "a nota nao esta na pasta de notas: informe o protocolo da autorizacao (nProt)"
                    .into(),
            )
        })?,
    };
    let r = ctx
        .cliente
        .cancelar(ctx.certificado, &chave, &nprot, justificativa)
        .await?;
    let (guardado, situacao) = guardar(ctx, &chave, &r)?;
    Ok(Registrado {
        chave,
        resultado: r,
        guardado,
        situacao,
        avisos: Vec::new(),
    })
}

/// Registra uma carta de correcao (evento 110110). A sequencia (1, 2, 3...) sai da pasta; `sequencia` so' e' preciso
/// para a nota que nao esta nela. **A carta nova substitui as anteriores**: o texto leva TODAS as correcoes que ainda
/// valem.
///
/// # Errors
///
/// [`Erro::Uso`] se a nota nao esta na pasta e nao veio a sequencia; [`Erro::Recusa`] se a nota nao esta autorizada, ja
/// tem as 20 cartas ou o texto nao serve (15 a 1000 caracteres).
pub async fn corrigir<T: Transporte>(
    ctx: &Contexto<'_, T>,
    chave: &str,
    correcao: &str,
    sequencia: Option<u32>,
) -> Result<Registrado, Erro> {
    let chave = chave_limpa(chave);
    let seq = match (local(ctx, &chave), ctx.notas, sequencia) {
        (Some(r), Some(notas), dada) if r.situacao == Situacao::Autorizada => {
            dada.map_or_else(|| notas.proxima_sequencia(&chave, CARTA_CORRECAO), Ok)?
        }
        (Some(r), _, _) => {
            return Err(Erro::recusa(
                "chNFe",
                format!(
                    "so' se corrige nota autorizada, e esta esta {}",
                    r.situacao.texto()
                ),
            ))
        }
        (None, _, Some(s)) => s,
        (None, _, None) => return Err(Erro::Uso(
            "a nota nao esta na pasta de notas: informe a sequencia da carta (1 para a primeira)"
                .into(),
        )),
    };
    if seq > MAX_SEQ_CARTA {
        return Err(Erro::recusa(
            "nSeqEvento",
            format!("a nota ja tem as {MAX_SEQ_CARTA} cartas de correcao que o MOC admite"),
        ));
    }
    let r = ctx
        .cliente
        .corrigir(ctx.certificado, &chave, correcao, seq)
        .await?;
    let (guardado, situacao) = guardar(ctx, &chave, &r)?;
    Ok(Registrado {
        chave,
        resultado: r,
        guardado,
        situacao,
        avisos: Vec::new(),
    })
}

/// Emite em contingencia EPEC: monta e assina a NF-e com `tpEmis` 4 (o motivo vem de `x_just` do pedido ou de
/// `[nfe].motivo_contingencia`), grava como pendente e registra o EPEC no Ambiente Nacional. Registrado, a nota fica
/// em EPEC na pasta; a MESMA NF-e tem de ser transmitida a SEFAZ da UF quando ela voltar (a retomada faz isso).
///
/// # Errors
///
/// Os de [`super::emitir`] antes da rede.
pub async fn epec<T: Transporte>(
    ctx: &Contexto<'_, T>,
    pedido: Value,
    estrito: bool,
) -> Result<Registrado, Erro> {
    let (p, assinada, registro) = preparar_e_gravar(ctx, pedido, estrito, true)?;
    let notas = ctx.pasta()?;
    let ver = concat!("rustnf ", env!("CARGO_PKG_VERSION"));
    let r = ctx
        .cliente
        .registrar_epec(ctx.certificado, &assinada, ver)
        .await?;
    // EPEC recusado: nada foi gasto e a NF-e nao saiu; sem resposta, ela pode ter sido registrada e fica pendente
    if matches!(
        r.desfecho,
        DesfechoEvento::Rejeitado { .. }
            | DesfechoEvento::EpecBloqueado { .. }
            | DesfechoEvento::PrazoVencido { .. }
    ) {
        notas.descartar_pendente(&registro.chave)?;
        return Ok(Registrado {
            chave: registro.chave,
            resultado: r,
            guardado: false,
            situacao: None,
            avisos: montagem::avisos(&p.nota),
        });
    }
    let (guardado, situacao) = guardar(ctx, &registro.chave, &r)?;
    Ok(Registrado {
        chave: registro.chave,
        resultado: r,
        guardado,
        situacao,
        avisos: montagem::avisos(&p.nota),
    })
}

/// Inutiliza uma faixa de numeracao da serie (`serie` vazia = a de `[nfe].serie`) do emitente da configuracao. A faixa
/// que tem nota da pasta e' recusada antes da rede (a SEFAZ a recusaria com 241).
///
/// # Errors
///
/// [`Erro::Uso`] sem CNPJ (o `inutNFe` nao tem CPF); [`Erro::Recusa`] para faixa com nota da pasta ou pedido que nao
/// monta (justificativa de 15 a 255 caracteres, faixa de ate 10.000 numeros, UF que perdeu o servico).
pub async fn inutilizar<T: Transporte>(
    ctx: &Contexto<'_, T>,
    serie: Option<&str>,
    inicio: u64,
    fim: u64,
    justificativa: &str,
) -> Result<Inutilizada, Erro> {
    let cnpj = ctx.config.emitente.cnpj.as_str();
    if cnpj.is_empty() {
        return Err(Erro::Uso(
            "a inutilizacao e' do CNPJ do emitente: informe [emitente].cnpj no arquivo de configuracao".into(),
        ));
    }
    let serie_txt = serie
        .filter(|s| !s.trim().is_empty())
        .map_or_else(|| ctx.config.nfe.serie.clone(), |s| s.trim().to_string());
    let serie_n: u32 = serie_txt
        .parse()
        .map_err(|_| Erro::Uso("a serie vai de 0 a 999".into()))?;
    let ocupados = match ctx.notas {
        Some(n) => n.ocupados_na_faixa(&serie_txt, inicio, fim)?,
        None => Vec::new(),
    };
    if let Some((numero, ocupante)) = ocupados.first() {
        let quem = match ocupante {
            Ocupante::Nota(chave, s) => format!("a nota {chave} ({})", s.texto()),
            Ocupante::Inutilizada => "uma faixa ja inutilizada".to_string(),
        };
        return Err(Erro::recusa(
            "nNFIni",
            format!("o numero {numero} da serie {serie_n} ja esta com {quem}: nao se inutiliza numero usado"),
        ));
    }
    let ano = ctx.agora.year();
    let faixa = PedidoInutilizar {
        cnpj,
        serie: serie_n,
        inicio,
        fim,
        justificativa,
        ano: Some(ano),
    };
    let r = ctx.cliente.inutilizar(ctx.certificado, &faixa).await?;
    let guardada = match (ctx.notas, &r.desfecho, r.proc_inut.as_deref()) {
        (
            Some(notas),
            DesfechoInutilizacao::Inutilizada { protocolo, .. }
            | DesfechoInutilizacao::JaInutilizada { protocolo, .. },
            Some(proc),
        ) => {
            let f = Inutilizacao {
                id: r.id.clone(),
                serie: serie_n.to_string(),
                inicio,
                fim,
                ano: ano.to_string(),
                protocolo: protocolo.clone(),
                situacao: Situacao::Inutilizada,
            };
            notas.guardar_inutilizacao(&f, proc)?;
            true
        }
        _ => false,
    };
    Ok(Inutilizada {
        resultado: r,
        guardada,
    })
}
