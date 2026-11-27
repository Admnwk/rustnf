// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Cancelamento, carta de correcao, EPEC e inutilizacao: o evento sai pelo cliente da biblioteca e, registrado, e'
//! guardado na pasta ao lado da nota.

use chrono::Datelike;
use rustnf::chave_dfe;
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

/// O EPEC de [`epec_substituindo`]: o que o evento registrou e a NF-e pendente que ele substituiu.
#[derive(Debug, Clone)]
pub struct EpecRegistrado {
    /// O resultado do EPEC e o que ficou na pasta.
    pub registrado: Registrado,
    /// A chave da NF-e pendente que o EPEC substituiu e ficou ligada a ele; vazio se o pedido nao substituia nenhuma ou se o
    /// EPEC foi recusado (nesse caso nao ha vinculo).
    pub substitui: String,
}

impl EpecRegistrado {
    /// O JSON do EPEC ([`Registrado::para_json`]) mais `substitui`, se houve vinculo.
    #[must_use]
    pub fn para_json(&self) -> Value {
        let mut v = self.registrado.para_json();
        if !self.substitui.is_empty() {
            v["substitui"] = json!(self.substitui);
        }
        v
    }

    /// O erro do resultado (`None` = registrado).
    #[must_use]
    pub fn erro(&self) -> Option<Erro> {
        self.registrado.erro()
    }
}

/// O resultado da inutilizacao e o que ficou na pasta.
#[derive(Debug, Clone)]
pub struct Inutilizada {
    /// O resultado da biblioteca (com o `ProcInutNFe`, se inutilizada).
    pub resultado: ResultadoInutilizacao,
    /// A faixa foi guardada na pasta de notas.
    pub guardada: bool,
    /// As chaves das NF-e substituidas que sairam da pasta com a faixa (a NF-e assinada de cada uma fica guardada ao lado
    /// da faixa, em `inutilizacao/<ano>/`).
    pub substituidas: Vec<String>,
}

impl Inutilizada {
    /// O JSON da inutilizacao: o do resultado mais `guardada`.
    #[must_use]
    pub fn para_json(&self) -> Value {
        let mut v = json_da_inutilizacao(&self.resultado);
        v["guardada"] = json!(self.guardada);
        if !self.substituidas.is_empty() {
            v["substituidas"] = json!(self.substituidas);
        }
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
    Ok(epec_substituindo(ctx, pedido, estrito, None)
        .await?
        .registrado)
}

/// Como [`epec`], para a venda de uma NF-e que ficou SEM RESPOSTA: com `substitui` (a chave dessa NF-e pendente), o EPEC
/// sai com numero NOVO, o proximo livre, como manda o Manual de Contingencia da NF-e (MOC 7.00, Anexo III, item 2.3.3: a
/// pendente de retorno recebe nova numeracao para a contingencia nao ser rejeitada se a original saiu autorizada), e a
/// pendente fica ligada a ele. Dai em diante a retomada so' CONSULTA a pendente, nunca a reenvia, e diz o que a norma manda
/// fazer com ela: cancelar, se saiu autorizada, ou inutilizar o numero, se a SEFAZ nao a tem.
///
/// `substitui` e' conferido antes de qualquer rede: a chave tem de ser de uma NF-e (55) da pasta, ainda pendente e nao
/// substituida, do mesmo emitente e do mesmo ambiente da configuracao. O vinculo so' e' gravado quando o EPEC fica na pasta
/// (registrado, ou sem resposta e pendente); EPEC recusado nao deixa vinculo.
///
/// # Errors
///
/// Os de [`epec`] e [`Erro::Recusa`] para a `substitui` que nao serve.
pub async fn epec_substituindo<T: Transporte>(
    ctx: &Contexto<'_, T>,
    pedido: Value,
    estrito: bool,
    substitui: Option<&str>,
) -> Result<EpecRegistrado, Erro> {
    let substituida = substitui
        .map(|c| conferir_a_substituida(ctx, &chave_limpa(c)))
        .transpose()?;
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
        return Ok(EpecRegistrado {
            registrado: Registrado {
                chave: registro.chave,
                resultado: r,
                guardado: false,
                situacao: None,
                avisos: montagem::avisos(&p.nota),
            },
            substitui: String::new(),
        });
    }
    let (guardado, situacao) = guardar(ctx, &registro.chave, &r)?;
    let mut ligada = String::new();
    if let (Some(pendente), Some(Situacao::Epec | Situacao::Pendente)) = (substituida, situacao) {
        notas.vincular_substituicao(&pendente.chave, &registro.chave)?;
        ligada = pendente.chave;
    }
    Ok(EpecRegistrado {
        registrado: Registrado {
            chave: registro.chave,
            resultado: r,
            guardado,
            situacao,
            avisos: montagem::avisos(&p.nota),
        },
        substitui: ligada,
    })
}

/// Tira do pedido o campo `substitui` (a chave da NF-e pendente que o EPEC substitui) e o devolve: ele nao e' campo do
/// leiaute, entao o pedido que o leva nao passaria na conferencia estrita. E' o que o HTTP e o MCP usam para receber a
/// substituicao dentro do corpo do pedido do EPEC.
///
/// # Errors
///
/// [`Erro::Entrada`] se `substitui` nao e' um texto com a chave.
pub fn separar_substitui(pedido: &mut Value) -> Result<Option<String>, Erro> {
    let Some(raiz) = pedido.as_object_mut() else {
        return Ok(None);
    };
    match raiz.remove("substitui") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(c)) if !chave_limpa(&c).is_empty() => Ok(Some(c)),
        Some(_) => Err(Erro::Entrada(
            "substitui: informe a chave de acesso (44 digitos) da NF-e pendente que o EPEC substitui".into(),
        )),
    }
}

/// A NF-e que o EPEC substitui, conferida contra a pasta e a configuracao: recusa em palavras, sem rede.
fn conferir_a_substituida<T: Transporte>(
    ctx: &Contexto<'_, T>,
    chave: &str,
) -> Result<Registro, Erro> {
    let recusa = |m: String| Erro::recusa("substitui", m);
    let notas = ctx.pasta()?;
    let nota = match notas.registro(chave) {
        Ok(r) => r,
        Err(_) => {
            return Err(recusa(format!(
                "a chave {chave} nao e' de uma NF-e (55) desta pasta de notas: so' se substitui nota pendente que esta nela"
            )))
        }
    };
    if nota.situacao != Situacao::Pendente {
        return Err(recusa(format!(
            "so' se substitui NF-e pendente (sem resposta da SEFAZ), e esta esta {}",
            nota.situacao.texto()
        )));
    }
    if !nota.substituida_por.is_empty() {
        return Err(recusa(format!(
            "esta NF-e ja foi substituida pelo EPEC {}: retome os pendentes para saber o que fazer com ela",
            nota.substituida_por
        )));
    }
    let e = &ctx.config.emitente;
    let documento = if e.cnpj.is_empty() { &e.cpf } else { &e.cnpj };
    let esperado = format!("{documento:0>14}");
    if !chave_dfe::decompor(chave, true).is_ok_and(|c| c.emitente == esperado) {
        return Err(recusa(
            "esta NF-e e' de outro emitente que o da configuracao".into(),
        ));
    }
    if nota.ambiente != ctx.config.nfe.ambiente {
        return Err(recusa(format!(
            "esta NF-e e' de {} e a configuracao esta em {}",
            nota.ambiente, ctx.config.nfe.ambiente
        )));
    }
    Ok(nota)
}

/// Inutiliza uma faixa de numeracao da serie (`serie` vazia = a de `[nfe].serie`) do emitente da configuracao. A faixa
/// que tem nota da pasta e' recusada antes da rede (a SEFAZ a recusaria com 241). A unica excecao e' a NF-e pendente
/// substituida por um EPEC que a SEFAZ confirmou nao ter (a retomada anotou o `cStat` 217): a norma manda inutilizar o
/// numero dela (MOC 7.00, Anexo III, item 2.3.3), e ela sai da pasta junto com a faixa, com a NF-e assinada guardada.
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
    let mut substituidas = Vec::new();
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
            substituidas = notas.guardar_inutilizacao(&f, proc)?;
            true
        }
        _ => false,
    };
    Ok(Inutilizada {
        resultado: r,
        guardada,
        substituidas,
    })
}
