// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A situacao das notas da pasta conferida contra a Sefin Nacional, em lote.
//!
//! Cada nota e' conferida pelos eventos que a Sefin tem dela, uma por vez e uma consulta por vez, com um intervalo
//! entre as consultas (cortesia com o servico: nenhuma consulta corre em paralelo). A regra, na ordem, parando no
//! primeiro evento que decide:
//!
//! 1. 101101 (cancelamento) ou 105104 (cancelamento deferido em analise fiscal): cancelada;
//! 2. 105102 (cancelamento por substituicao): substituida;
//! 3. 101103 (pedido de analise fiscal) sem 105104 e com mais pedidos do que indeferimentos (105105): em analise
//!    fiscal;
//! 4. senao, autorizada (a Sefin confirma que a nota existe).
//!
//! Nota que a pasta da como cancelada ou substituida e que a Sefin nao mostra assim NUNCA volta para autorizada por
//! aqui: vai para as falhas, como divergencia, para uma pessoa olhar.

use std::time::Duration;

use rustnf::nfse::eventos::{ANALISE_FISCAL, CANCELAMENTO, SUBSTITUICAO};
use rustnf::nfse::sefin::Status;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::emissao::Emissor;
use crate::erro::Erro;
use crate::notas::{Filtro, Notas, Registro, Situacao};

/// Cancelamento deferido pelo fisco depois do pedido de analise.
const CANCELAMENTO_DEFERIDO: &str = "105104";
/// Cancelamento indeferido pelo fisco depois do pedido de analise.
const CANCELAMENTO_INDEFERIDO: &str = "105105";
/// Quantas sequencias de um mesmo tipo se contam, no maximo.
const SEQUENCIAS_MAXIMAS: u32 = 20;

/// O intervalo padrao entre duas consultas a Sefin: 200 ms. E' cortesia com o servico, nao uma exigencia dele.
pub const INTERVALO_PADRAO: Duration = Duration::from_millis(200);

/// O que atualizar e com que intervalo, como o pedem a linha de comando, a API e o MCP. Sem nada preenchido: todas as
/// notas da pasta, com o intervalo padrao.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Pedido {
    /// So' as notas emitidas a partir deste dia, `AAAA-MM-DD` (inclusive).
    pub de: Option<String>,
    /// So' as notas emitidas ate este dia, `AAAA-MM-DD` (inclusive).
    pub ate: Option<String>,
    /// So' as notas que a pasta tem nesta situacao.
    pub situacao: Option<Situacao>,
    /// So' as notas que casam com o texto (nome do tomador, CPF/CNPJ, numero da nota ou da DPS, chave).
    pub busca: Option<String>,
    /// Intervalo entre duas consultas, em milissegundos; sem ele, [`INTERVALO_PADRAO`].
    pub intervalo_ms: Option<u64>,
}

/// Uma nota cuja situacao mudou.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Mudanca {
    /// Chave de acesso.
    pub chave: String,
    /// A situacao que a pasta tinha.
    pub de: Situacao,
    /// A situacao que a Sefin mostra e que a pasta passou a ter.
    pub para: Situacao,
}

/// Uma nota que nao deu para conferir, ou cuja conferencia contradiz a pasta. O indice dela nao muda.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Falha {
    /// Chave de acesso.
    pub chave: String,
    /// O que aconteceu.
    pub motivo: String,
}

/// O que `atualizar_situacoes` fez.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Relatorio {
    /// Quantas notas passaram no filtro e foram conferidas (mudaram + iguais + falhas).
    pub conferidas: usize,
    /// As que mudaram de situacao.
    pub mudaram: Vec<Mudanca>,
    /// Quantas ja estavam como a Sefin mostra.
    pub iguais: usize,
    /// As que nao deu para conferir (sem resposta, recusa, nota nao encontrada, divergencia).
    pub falhas: Vec<Falha>,
}

impl Relatorio {
    /// O relatorio em JSON.
    #[must_use]
    pub fn para_json(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }
}

/// O que a Sefin mostra de uma nota.
struct Vista {
    situacao: Situacao,
    /// Havia pedido de analise fiscal, ja decidido contra (105105): a nota segue autorizada.
    com_analise_indeferida: bool,
}

/// Marca o passo: espera `intervalo` antes de toda consulta, menos a primeira do lote.
struct Ritmo {
    intervalo: Duration,
    ja_consultou: bool,
}

impl Ritmo {
    async fn passo(&mut self) {
        if self.ja_consultou && !self.intervalo.is_zero() {
            tokio::time::sleep(self.intervalo).await;
        }
        self.ja_consultou = true;
    }
}

/// O evento existe? `Ok(true)` existe, `Ok(false)` a Sefin diz que nao ha, `Err` nao deu para saber (ou a resposta nao
/// e' o evento pedido: o cliente confere chave e tipo).
async fn existe(
    emissor: &dyn Emissor,
    ritmo: &mut Ritmo,
    chave: &str,
    tipo: &str,
    sequencia: u32,
) -> Result<bool, String> {
    ritmo.passo().await;
    let r = emissor.consultar_evento(chave, tipo, sequencia).await;
    match r.status {
        Status::Ok => Ok(true),
        Status::NaoEncontrada => Ok(false),
        _ => Err(format!("evento {tipo}: {}", r.frase)),
    }
}

/// Quantos eventos do tipo existem (sequencias 1, 2, 3... ate a primeira que nao existe).
async fn contar(
    emissor: &dyn Emissor,
    ritmo: &mut Ritmo,
    chave: &str,
    tipo: &str,
) -> Result<u32, String> {
    let mut total = 0;
    while total < SEQUENCIAS_MAXIMAS && existe(emissor, ritmo, chave, tipo, total + 1).await? {
        total += 1;
    }
    Ok(total)
}

/// A situacao da nota, pelos eventos que a Sefin tem dela.
async fn ver_na_sefin(
    emissor: &dyn Emissor,
    ritmo: &mut Ritmo,
    chave: &str,
) -> Result<Vista, String> {
    let vista = |situacao| Vista {
        situacao,
        com_analise_indeferida: false,
    };
    for tipo in [CANCELAMENTO, CANCELAMENTO_DEFERIDO] {
        if existe(emissor, ritmo, chave, tipo, 1).await? {
            return Ok(vista(Situacao::Cancelada));
        }
    }
    if existe(emissor, ritmo, chave, SUBSTITUICAO, 1).await? {
        return Ok(vista(Situacao::Substituida));
    }
    let pedidos = contar(emissor, ritmo, chave, ANALISE_FISCAL).await?;
    if pedidos > 0 {
        let indeferidos = contar(emissor, ritmo, chave, CANCELAMENTO_INDEFERIDO).await?;
        return Ok(if pedidos > indeferidos {
            vista(Situacao::AnaliseFiscal)
        } else {
            Vista {
                situacao: Situacao::Autorizada,
                com_analise_indeferida: true,
            }
        });
    }
    // sem evento algum: a Sefin precisa confirmar que a nota existe (evento inexistente e nota inexistente
    // respondem igual)
    ritmo.passo().await;
    let r = emissor.consultar(chave).await;
    match r.status {
        Status::Autorizada if r.chave.is_empty() || r.chave == chave => {
            Ok(vista(Situacao::Autorizada))
        }
        Status::Autorizada => Err("a Sefin respondeu com a nota de outra chave".into()),
        Status::NaoEncontrada => Err("a Sefin nao encontrou a nota".into()),
        _ => Err(format!("consulta da nota: {}", r.frase)),
    }
}

/// A pasta pode passar de `atual` para o que a Sefin mostra? `Err` e' a divergencia.
fn transicao(atual: Situacao, vista: &Vista) -> Result<Situacao, String> {
    let nova = vista.situacao;
    match (atual, nova) {
        (Situacao::Cancelada | Situacao::Substituida, Situacao::Autorizada | Situacao::AnaliseFiscal) => {
            Err(format!(
                "divergencia: a pasta diz {} e a Sefin nao mostra o cancelamento nem a substituicao; confira a nota",
                atual.texto()
            ))
        }
        (Situacao::AnaliseFiscal, Situacao::Autorizada) if !vista.com_analise_indeferida => Err(
            "divergencia: a pasta diz em analise fiscal e a Sefin nao mostra o pedido; confira a nota".into(),
        ),
        _ => Ok(nova),
    }
}

async fn conferir(
    emissor: &dyn Emissor,
    notas: &Notas,
    ritmo: &mut Ritmo,
    nota: &Registro,
) -> Result<Option<Mudanca>, String> {
    let vista = ver_na_sefin(emissor, ritmo, &nota.chave).await?;
    let nova = transicao(nota.situacao, &vista)?;
    if nova == nota.situacao {
        return Ok(None);
    }
    notas
        .definir_situacao(&nota.chave, nova)
        .map_err(|_| "nao foi possivel gravar a situacao na pasta de notas".to_string())?;
    Ok(Some(Mudanca {
        chave: nota.chave.clone(),
        de: nota.situacao,
        para: nova,
    }))
}

/// Confere na Sefin a situacao das notas da pasta que passam no `filtro` e atualiza o indice. Uma nota por vez, uma
/// consulta por vez, com `intervalo` entre as consultas ([`INTERVALO_PADRAO`]). A falha de uma nota (sem resposta,
/// recusa, nao encontrada, divergencia) vai para o relatorio e nao para as outras.
///
/// Precisa de um runtime do tokio com o relogio ligado (o intervalo e' um `tokio::time::sleep`).
///
/// # Errors
///
/// So' o erro de ler a pasta de notas.
pub async fn atualizar_situacoes(
    emissor: &dyn Emissor,
    notas: &Notas,
    filtro: &Filtro,
    intervalo: Duration,
) -> Result<Relatorio, Erro> {
    let lista = filtro.aplicar(notas.listar()?);
    Ok(atualizar_lista(emissor, notas, &lista, intervalo, &mut |_, _| true).await)
}

/// Como [`atualizar_situacoes`], para as notas de `lista` (as que uma interface mostra) e com acompanhamento: antes
/// de conferir cada nota, `acompanhar(feitas, total)` diz quantas ja foram conferidas e quantas ha; devolvendo `false`
/// o lote para ali. O que ja foi conferido fica gravado no indice e consta do relatorio.
pub async fn atualizar_lista(
    emissor: &dyn Emissor,
    notas: &Notas,
    lista: &[Registro],
    intervalo: Duration,
    acompanhar: &mut (dyn FnMut(usize, usize) -> bool + Send),
) -> Relatorio {
    let mut ritmo = Ritmo {
        intervalo,
        ja_consultou: false,
    };
    let mut relatorio = Relatorio::default();
    for (feitas, nota) in lista.iter().enumerate() {
        if !acompanhar(feitas, lista.len()) {
            break;
        }
        relatorio.conferidas += 1;
        match conferir(emissor, notas, &mut ritmo, nota).await {
            Ok(Some(mudanca)) => relatorio.mudaram.push(mudanca),
            Ok(None) => relatorio.iguais += 1,
            Err(motivo) => relatorio.falhas.push(Falha {
                chave: nota.chave.clone(),
                motivo,
            }),
        }
    }
    relatorio
}

/// Como [`atualizar_situacoes`], com o filtro e o intervalo de um [`Pedido`].
///
/// # Errors
///
/// [`Erro::Entrada`] se `de` ou `ate` nao for `AAAA-MM-DD`; os de [`atualizar_situacoes`].
pub async fn atualizar_pedido(
    emissor: &dyn Emissor,
    notas: &Notas,
    pedido: &Pedido,
) -> Result<Relatorio, Erro> {
    let filtro = Filtro::novo(
        pedido.de.as_deref(),
        pedido.ate.as_deref(),
        pedido.situacao,
        pedido.busca.as_deref().unwrap_or(""),
    )?;
    let intervalo = pedido
        .intervalo_ms
        .map_or(INTERVALO_PADRAO, Duration::from_millis);
    atualizar_situacoes(emissor, notas, &filtro, intervalo).await
}

#[cfg(test)]
#[path = "atualizacao_testes.rs"]
mod testes;
