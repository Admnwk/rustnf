// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O fluxo de emissao e de cancelamento com a configuracao: completa o pedido com o `[emitente]`, numera a DPS pela
//! pasta de notas, emite e guarda a nota autorizada.

use rustnf::nfse::sefin::Status;
use rustnf::nfse::Dps;
use serde_json::{json, Value};

use crate::config::{aplicar_emitente, Emitente};
use crate::emissao::{self, Emissor};
use crate::erro::Erro;
use crate::notas::{Notas, Registro};

/// Campo ausente do pedido: nao existe, e' `null` ou e' texto vazio.
fn ausente(pedido: &Value, chave: &str) -> bool {
    match pedido.get(chave) {
        None | Some(Value::Null) => true,
        Some(Value::String(s)) => s.trim().is_empty(),
        Some(_) => false,
    }
}

/// A serie do pedido como texto; sem ela, 1 (o padrao da DPS).
fn serie_do_pedido(pedido: &Value) -> String {
    match pedido.get("serie") {
        Some(Value::String(s)) if !s.trim().is_empty() => s.trim().to_string(),
        Some(Value::Number(n)) => n.to_string(),
        _ => "1".to_string(),
    }
}

/// Emite a DPS do `pedido` (JSON).
///
/// 1. completa so' os campos ausentes com o `[emitente]`;
/// 2. sem `numero` no pedido e com `notas`, usa o proximo numero da serie;
/// 3. emite (veja [`emissao::emitir`]);
/// 4. nota autorizada ou recuperada: guarda o XML na pasta de notas e confirma o numero da DPS. Se a guarda falhar,
///    a emissao vale (a nota existe na Sefin): a resposta traz `guardada: false` e o `aviso`.
///
/// A resposta e' a de [`emissao::emitir`]; com `notas`, ganha `guardada` (e `numero_dps`, quando o numero veio da
/// pasta).
///
/// # Errors
///
/// Como [`emissao::emitir`], mais o erro de ler o numero da pasta de notas ou da leitura do pedido.
pub async fn emitir_pedido(
    emissor: &dyn Emissor,
    mut pedido: Value,
    emitente: &Emitente,
    folga_segundos: u32,
    notas: Option<&Notas>,
) -> Result<Value, Erro> {
    aplicar_emitente(&mut pedido, emitente);
    let mut numero_da_pasta = None;
    if let (Some(notas), true) = (notas, pedido.is_object() && ausente(&pedido, "numero")) {
        let numero = notas.proximo_numero(&serie_do_pedido(&pedido))?;
        pedido["numero"] = json!(numero);
        numero_da_pasta = Some(numero);
    }
    let dps: Dps = serde_json::from_value(pedido)?;
    let serie = dps.serie.as_str().to_string();
    let numero = dps.numero.as_str().to_string();
    let resultado = emissao::enviar(emissor, dps, folga_segundos).await?;
    let mut saida = emissao::resultado_json(&resultado)?;
    let Some(notas) = notas else {
        return Ok(saida);
    };
    if resultado.status == Status::Autorizada {
        // o numero foi gasto na Sefin mesmo que a guarda do XML falhe
        let confirmado = confirmar(notas, &serie, &numero);
        let guardada = guardar(notas, &resultado, &serie, &numero);
        saida["guardada"] = json!(guardada);
        if !(guardada && confirmado) {
            saida["aviso"] = json!(
                "a nota foi autorizada, mas a pasta de notas nao pode ser atualizada por completo; consulte a nota pela chave e confira a numeracao"
            );
        }
    }
    if let Some(n) = numero_da_pasta {
        saida["numero_dps"] = json!(n);
    }
    Ok(saida)
}

fn guardar(notas: &Notas, r: &rustnf::nfse::sefin::Resultado, serie: &str, numero: &str) -> bool {
    let Some(nfse) = &r.nfse else {
        return false;
    };
    if r.xml_retorno.is_empty() {
        return false;
    }
    let mut registro = Registro::da_nfse(nfse);
    if registro.chave.is_empty() {
        registro.chave.clone_from(&r.chave);
    }
    if registro.serie.is_empty() {
        registro.serie = serie.to_string();
    }
    if registro.numero_dps.is_empty() {
        registro.numero_dps = numero.to_string();
    }
    notas.guardar(&registro, &r.xml_retorno).is_ok()
}

fn confirmar(notas: &Notas, serie: &str, numero: &str) -> bool {
    numero
        .parse::<u64>()
        .is_ok_and(|n| notas.confirmar_numero(serie, n).is_ok())
}

/// Cancela a NFS-e (veja [`emissao::cancelar`]) e, com `notas`, marca a nota como cancelada na pasta. Nota que nao
/// esta na pasta nao e' erro.
///
/// # Errors
///
/// Como [`emissao::cancelar`].
pub async fn cancelar_pedido(
    emissor: &dyn Emissor,
    chave: &str,
    motivo: &str,
    justificativa: &str,
    notas: Option<&Notas>,
) -> Result<Value, Erro> {
    let saida = emissao::cancelar(emissor, chave, motivo, justificativa).await?;
    if let Some(notas) = notas {
        // uma nota que nao esta na pasta (emitida em outro lugar) continua cancelada na Sefin
        let _ = notas.marcar_cancelada(chave);
    }
    Ok(saida)
}
