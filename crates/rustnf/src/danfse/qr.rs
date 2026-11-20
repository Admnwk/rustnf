// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! QR da consulta publica, entregue ao modelo Typst como corridas de modulos escuros.

use serde_json::{json, Value};

use super::ErroDanfse;

/// `{"texto", "n", "corridas": [[linha, coluna, comprimento]...]}`, correcao de erro nivel M. Os modulos escuros
/// de cada linha viram corridas para o modelo desenhar menos retangulos.
pub(super) fn dados(texto: &str) -> Result<Value, ErroDanfse> {
    let qr = qrcode::QrCode::with_error_correction_level(texto.as_bytes(), qrcode::EcLevel::M)
        .map_err(|e| ErroDanfse::Qr(e.to_string()))?;
    let n = qr.width();
    let escuro: Vec<bool> = qr
        .to_colors()
        .into_iter()
        .map(|c| c == qrcode::Color::Dark)
        .collect();
    let mut corridas = Vec::new();
    for y in 0..n {
        let mut x = 0;
        while x < n {
            if !escuro[y * n + x] {
                x += 1;
                continue;
            }
            let ini = x;
            while x < n && escuro[y * n + x] {
                x += 1;
            }
            corridas.push(json!([y, ini, x - ini]));
        }
    }
    Ok(json!({ "texto": texto, "n": n, "corridas": corridas }))
}
