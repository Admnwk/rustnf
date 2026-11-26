// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Blocos que so' o leiaute da Reforma Tributaria tem (NT 2026.010 v1.00, itens 4.1, 4.4 e 4.5): os totais em tres
//! quadros (produtos e nota, ICMS/IPI, IBS/CBS/IS) com as linhas facultativas e o QR Code ao lado dos dados
//! adicionais.

use serde_json::{json, Value};

use super::blocos::{bloco, cp, s, Cp};
use super::desenho::{cm, Al, Caneta};
use super::medidas::*;

/// Largura do quadro do QR Code ao lado dos dados adicionais.
pub(super) const W_QR: f64 = 45.0;

/// Campos de largura igual dividindo a linha inteira, alinhados a direita; `destaque` poe um rotulo em negrito.
fn igual<'a>(campos: &'a [Value], destaque: Option<&str>) -> Vec<Cp<'a>> {
    let w = LARG / campos.len() as f64;
    campos
        .iter()
        .map(|c| {
            let p = cp(s(&c[0]), s(&c[1]), w).direita();
            if destaque == Some(s(&c[0])) {
                p.negrito()
            } else {
                p
            }
        })
        .collect()
}

fn lista(v: &Value) -> &[Value] {
    v.as_array().map_or(&[][..], Vec::as_slice)
}

/// Total dos produtos e total da nota (item 4.1: o novo bloco entra depois deste e do ICMS/IPI).
pub(super) fn totais(c: &mut Caneta, y: f64, d: &Value) -> f64 {
    let r = &d["rtc"];
    let nota = [json!(["Valor total da nota", r["vnf"]])];
    bloco(
        c,
        y,
        "TOTAL DOS PRODUTOS E TOTAL DA NOTA",
        &[
            igual(lista(&r["totais"]), None),
            igual(&nota, Some("Valor total da nota")),
        ],
    )
}

/// Total do ICMS e do IPI, com as linhas do FCP e da monofasia do ICMS so' quando o XML as traz (item 4.4).
pub(super) fn icms_ipi(c: &mut Caneta, y: f64, d: &Value) -> f64 {
    let r = &d["rtc"];
    let mut linhas = vec![igual(lista(&r["icmsipi"]), None)];
    for k in ["fcp", "mono_icms"] {
        if !r[k].is_null() {
            linhas.push(igual(lista(&r[k]), None));
        }
    }
    bloco(c, y, "TOTAL DO ICMS / IPI", &linhas)
}

/// Total do IBS, da CBS e do IS (item 4.1), com a linha da monofasia so' quando ha o grupo `gMono`.
pub(super) fn ibscbs(c: &mut Caneta, y: f64, d: &Value) -> f64 {
    let r = &d["rtc"];
    let mut linhas = vec![igual(lista(&r["ibscbs"]), None)];
    if !r["mono_ibs"].is_null() {
        linhas.push(igual(lista(&r["mono_ibs"]), None));
    }
    bloco(c, y, "TOTAL DO IBS / CBS / IS", &linhas)
}

/// Dados adicionais com o quadro do QR Code (item 4.5), que so' existe quando o XML traz o `qrCode`. Com QR, o
/// quadro das informacoes ocupa 60% do que sobra e o resto vai para o reservado ao fisco.
pub(super) fn dados_adicionais(
    c: &mut Caneta,
    y: f64,
    linhas: &[String],
    w_compl: f64,
    tem_qr: bool,
) {
    let w_total = if tem_qr { LARG - W_QR } else { LARG };
    super::blocos::dados_adicionais(c, y, linhas, w_compl, w_total - w_compl);
    if tem_qr {
        let x = X0 + w_total + 1.0;
        let w = LARG - w_total - 1.0;
        let yt = y + HT;
        c.celula(x, y + 0.8, w, 3.0, "QR CODE", T_BLOCO, true, Al::C);
        c.ret(x, yt, w, H_COMPL);
        let lado = (w - 6.0).min(H_COMPL - 6.0);
        c.ops.push(json!({
            "t": "qr", "x": cm(x + (w - lado) / 2.0), "y": cm(yt + 3.0), "lado": cm(lado)
        }));
    }
}

/// Largura do quadro das informacoes complementares no leiaute RTC.
pub(super) fn w_complementares(tem_qr: bool) -> f64 {
    (LARG - if tem_qr { W_QR } else { 0.0 }) * 0.6
}
