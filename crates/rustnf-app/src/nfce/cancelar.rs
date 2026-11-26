// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O aviso de prazo do cancelamento (110111) da NFC-e, dado ANTES de enviar o pedido.
//!
//! A fonte e' o MOC 7.00, Visao Geral: a regra 2P12-14 manda rejeitar (501) o cancelamento de nota autorizada ha mais de 1 dia
//! (24 horas), "considerando a excecao de prazo definida em legislacao estadual", e o item 5.9.4 diz que, para a NFC-e, o
//! pedido fora do prazo e' rejeitado com o 501. A lista do prazo de CADA UF nao esta nas fontes oficiais que o projeto
//! usa: por isso o aviso diz o prazo NACIONAL e nunca bloqueia o envio (quem decide e' a SEFAZ).

use chrono::{DateTime, Duration, Utc};

use crate::nfe::chave_limpa;
use crate::notas::nfe::NotasNfe;

/// Prazo nacional do cancelamento normal, em horas.
pub const PRAZO_NACIONAL_HORAS: i64 = 24;

/// Quando a nota foi autorizada: o `dhRecbto` do protocolo guardado, ou, sem ele, quando foi enviada.
fn autorizada_em(notas: &NotasNfe, chave: &str) -> Option<DateTime<Utc>> {
    let r = notas.registro(chave).ok()?;
    let do_protocolo = notas
        .ler_proc(chave)
        .ok()
        .and_then(|x| rustnf::nfe::ler(x.as_bytes()).ok())
        .and_then(|n| n.protocolo)
        .and_then(|p| p.recebido_em)
        .map(|d| d.0.with_timezone(&Utc));
    do_protocolo.or_else(|| DateTime::from_timestamp(r.desde, 0).filter(|_| r.desde > 0))
}

/// O aviso do prazo nacional se ele ja acabou para a nota da pasta; `None` se esta dentro do prazo ou se a nota nao e' da
/// pasta (sem data de autorizacao nao ha o que dizer).
#[must_use]
pub fn aviso_de_prazo(notas: &NotasNfe, chave: &str, agora: DateTime<Utc>) -> Option<String> {
    let chave = chave_limpa(chave);
    let fim = autorizada_em(notas, &chave)? + Duration::hours(PRAZO_NACIONAL_HORAS);
    if agora <= fim {
        return None;
    }
    let fuso = chrono::FixedOffset::west_opt(3 * 3600)?;
    Some(format!(
        "o prazo nacional para cancelar a NFC-e ({PRAZO_NACIONAL_HORAS} horas depois da autorizacao) acabou em {} (horario de \
         Brasilia; MOC 7.00, Visao Geral, regra 2P12-14). A SEFAZ da sua UF pode ter outro prazo (legislacao estadual) e e' ela \
         quem decide: o pedido vai ser enviado, e a SEFAZ responde 501 se recusar",
        fim.with_timezone(&fuso).format("%d/%m/%Y %H:%M")
    ))
}
