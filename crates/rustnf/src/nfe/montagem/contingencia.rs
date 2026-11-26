// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Contingencia off-line da NFC-e (`tpEmis` 9): o prazo para transmitir a nota emitida sem autorizacao previa.
//!
//! O MOC 7.00, Anexo IV (itens 2 e 3), manda transmitir a NFC-e "ate o final do primeiro dia util subsequente
//! contado a partir de sua emissao em contingencia". A NT 2026.002 v1.11 (regra B09-40, excecao 2) acrescenta que a
//! nota transmitida depois de 24 horas volta com o `cStat` 150 (autorizada fora de prazo): a SEFAZ nao a recusa por
//! isso, mas o emitente fica fora do prazo.

use chrono::{DateTime, Datelike, FixedOffset, TimeZone, Weekday};

use super::formato::{erro, fuso_uf, na_uf, uf_codigo, Res};

/// O fim do prazo de transmissao da NFC-e emitida em `emissao`, no fuso da UF do emitente: as 23h59min59s do primeiro
/// dia util depois do dia da emissao.
///
/// So' sabado e domingo contam como dia nao util: feriado nacional, estadual ou municipal nao entra (a lista e' da
/// legislacao de cada UF); quem tem feriado no intervalo tem um dia a mais do que o resultado diz.
///
/// # Errors
///
/// [`super::ErroMontagem`] (campo `UF`) se a UF e' desconhecida.
pub fn prazo_de_transmissao(
    emissao: &DateTime<FixedOffset>,
    uf: &str,
) -> Res<DateTime<FixedOffset>> {
    let uf = uf.trim().to_uppercase();
    if uf_codigo(&uf).is_none() {
        return Err(erro(format!("UF invalida: {uf:?}"), "UF"));
    }
    let mut dia = na_uf(emissao, &uf).date_naive();
    loop {
        dia = dia
            .succ_opt()
            .ok_or_else(|| erro("data fora do calendario", "dhEmi"))?;
        if !matches!(dia.weekday(), Weekday::Sat | Weekday::Sun) {
            break;
        }
    }
    fuso_uf(&uf)
        .from_local_datetime(&dia.and_hms_opt(23, 59, 59).unwrap_or_default())
        .single()
        .ok_or_else(|| erro("fim do prazo fora do calendario", "dhEmi"))
}
