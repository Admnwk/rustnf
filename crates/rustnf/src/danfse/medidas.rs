// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Medidas da pagina A4 do DANFSe (em cm, a partir do canto superior esquerdo) e tamanhos de fonte (em pt).

use serde_json::Value;

pub(super) const CM: f64 = 72.0 / 2.54;
pub(super) const X0: f64 = 0.30;
pub(super) const X2: f64 = 5.41;
pub(super) const X3: f64 = 10.51;
pub(super) const X4: f64 = 15.62;
pub(super) const XF: f64 = 20.70;
pub(super) const COLS: [f64; 5] = [X0, X2, X3, X4, XF];
pub(super) const RECUO: f64 = 0.08;
pub(super) const T_ROT: f64 = 6.0;
pub(super) const T_VAL: f64 = 7.0;
pub(super) const T_BLOCO: f64 = 7.0;
pub(super) const T_COMPL: f64 = 6.5;
pub(super) const Y_CANHOTO: f64 = 28.10;
pub(super) const Y_CANHOTO_FIM: f64 = 28.77;
pub(super) const FOLGA_INFO: f64 = 0.10;
pub(super) const MIN_LINHAS_COMPL: usize = 12;
pub(super) const ASCENDENTE: f64 = 0.718 + 0.19;

pub(super) fn passo_linha() -> f64 {
    (T_VAL * 1.2) / CM
}

pub(super) fn passo_compl() -> f64 {
    (T_COMPL * 1.2) / CM
}

pub(super) fn larg_texto() -> f64 {
    (XF - X0 - 2.0 * RECUO) * CM
}

pub(super) fn r4(v: f64) -> f64 {
    (v * 10000.0 + 0.5).floor() / 10000.0
}

pub(super) fn larg(ini: usize, qtd: usize) -> f64 {
    COLS[ini + qtd] - COLS[ini]
}

pub(super) fn s(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}

pub(super) fn f(v: &Value) -> f64 {
    v.as_f64().unwrap_or(0.0)
}
