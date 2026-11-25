// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Consulta da DPS por serie e numero e registro de eventos do contribuinte com a configuracao: o Id da DPS sai do
//! `[emitente]`, e o pedido de analise fiscal registrado marca a nota na pasta de notas.

use rustnf::nfse::eventos;
use rustnf::nfse::sefin::Resultado;
use serde_json::{json, Value};

use crate::config::Emitente;
use crate::emissao::{self, Emissor};
use crate::erro::Erro;
use crate::notas::{Notas, Situacao};

/// O Id da DPS (45 posicoes) que o emitente da configuracao enviou com esta `serie` e este `numero`: o municipio e o
/// CNPJ/CPF vem do `[emitente]`; sem `serie`, vale a do `[emitente]` ou 1.
///
/// # Errors
///
/// [`Erro::Entrada`] se o `[emitente]` nao tem municipio e CNPJ/CPF; [`Erro::Recusa`] se serie ou numero fogem do
/// leiaute.
pub fn id_da_dps(emitente: &Emitente, serie: Option<&str>, numero: &str) -> Result<String, Erro> {
    let documento = if emitente.cnpj.trim().is_empty() {
        emitente.cpf.trim()
    } else {
        emitente.cnpj.trim()
    };
    if emitente.municipio.trim().is_empty() || documento.is_empty() {
        return Err(Erro::Entrada(
            "para achar a DPS por serie e numero, o [emitente] precisa de municipio e cnpj (ou cpf); ou informe o Id da DPS"
                .into(),
        ));
    }
    let serie = serie
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .or_else(|| Some(emitente.serie.trim()).filter(|s| !s.is_empty()))
        .unwrap_or("1");
    Ok(rustnf::nfse::id_dps(
        emitente.municipio.trim(),
        documento,
        serie,
        numero,
    )?)
}

/// O que `registrar_evento_completo` devolve: o resultado do cliente e o que se fez com a pasta de notas.
#[derive(Debug, Clone)]
pub struct Registrada {
    /// O resultado do cliente da Sefin (registrado, rejeitado com as mensagens, ou sem confirmacao).
    pub resultado: Resultado,
    /// Pedido de analise fiscal (101103) registrado e pasta de notas em uso: a nota ficou marcada como em analise
    /// fiscal? `Some(false)` quando a nota nao esta na pasta, ou ja saiu de autorizada, ou a pasta nao pode ser
    /// atualizada; `None` nos outros casos.
    pub marcada: Option<bool>,
}

/// Registra um evento que o contribuinte pode pedir (`tipo` = `tpEvento`; o cancelamento tem [`crate::pedido::cancelar_pedido`]).
/// Antes de qualquer envio recusa o cancelamento e os eventos do fisco e do sistema. So' o 101103 registrado muda a
/// pasta de notas: a nota autorizada que esta nela passa a em analise fiscal.
///
/// # Errors
///
/// [`Erro::Recusa`] do tipo que o contribuinte nao registra por aqui; o que a Sefin responde vem em
/// [`Registrada::resultado`].
pub async fn registrar_evento_completo(
    emissor: &dyn Emissor,
    tipo: &str,
    chave: &str,
    motivo: &str,
    justificativa: &str,
    notas: Option<&Notas>,
) -> Result<Registrada, Erro> {
    emissao::conferir_tipo_de_registro(tipo)?;
    let resultado = emissor
        .registrar_evento(tipo, chave, motivo, justificativa)
        .await;
    let marcada = match notas {
        Some(notas) if resultado.ok() && tipo == eventos::ANALISE_FISCAL => {
            let da_nota = if resultado.chave.is_empty() {
                chave
            } else {
                &resultado.chave
            };
            Some(marcar_analise_fiscal(notas, da_nota))
        }
        _ => None,
    };
    Ok(Registrada { resultado, marcada })
}

/// Como [`registrar_evento_completo`], em JSON: o resultado de [`emissao::resultado_do_evento`] e, com a pasta de
/// notas, `marcada` quando o evento mexe nela.
///
/// # Errors
///
/// Como [`registrar_evento_completo`] e [`emissao::resultado_json`].
pub async fn registrar_evento_pedido(
    emissor: &dyn Emissor,
    tipo: &str,
    chave: &str,
    motivo: &str,
    justificativa: &str,
    notas: Option<&Notas>,
) -> Result<Value, Erro> {
    let r = registrar_evento_completo(emissor, tipo, chave, motivo, justificativa, notas).await?;
    let mut saida = emissao::resultado_do_evento(&r.resultado)?;
    if let Some(marcada) = r.marcada {
        saida["marcada"] = json!(marcada);
    }
    Ok(saida)
}

/// A nota autorizada da pasta passa a em analise fiscal. Nota ja em analise conta como marcada; a cancelada ou a
/// substituida nao volta atras por um pedido novo.
fn marcar_analise_fiscal(notas: &Notas, chave: &str) -> bool {
    match notas.registro(chave).map(|r| r.situacao) {
        Ok(Situacao::AnaliseFiscal) => true,
        Ok(Situacao::Autorizada) => notas
            .definir_situacao(chave, Situacao::AnaliseFiscal)
            .is_ok(),
        _ => false,
    }
}
