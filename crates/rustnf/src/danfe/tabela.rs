// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Quadro "Dados dos Produtos / Servicos" (MOC 7.00, Anexo II, itens 3.1.7 e 3.2) nos dois leiautes: as 14 colunas do
//! MOC e as 8 da NT 2026.010 (item 4.3). Cada item vira o que se desenha (linhas por coluna) e a altura que ocupa;
//! o plano das folhas usa a altura para decidir onde a folha termina. Itens de varias linhas levam o destaque
//! divisorio tracejado (item 3.1.7).

use serde_json::Value;

use super::blocos::s;
use super::desenho::{larg, quebrar, Al, Caneta};
use super::medidas::*;

/// Corpo das linhas de tributos do leiaute RTC: o piso de 6 pt do conteudo dos produtos (item 3.7.7 do Anexo II).
const T_TRIBUTO: f64 = 6.0;
const LH_TRIBUTO: f64 = T_TRIBUTO * 0.352_78 * 1.15;
/// Largura de cada coluna do leiaute RTC em fracao da largura util; a ultima (valor dos tributos) leva o resto.
const FRACOES_RTC: [f64; 7] = [0.17, 0.055, 0.065, 0.07, 0.075, 0.14, 0.18];
const TITULOS_RTC: [&str; 8] = [
    "DESCRIÇÃO DO PRODUTO / SERVIÇO",
    "CST / CFOP",
    "QTD / UN",
    "VLR UNIT",
    "VLR TOTAL",
    "BASES DE CÁLCULO",
    "ALÍQUOTAS",
    "VALOR DOS TRIBUTOS",
];

/// Uma coluna do quadro: titulo, largura em mm, alinhamento.
pub(super) struct Coluna {
    pub(super) titulo: String,
    pub(super) w: f64,
    pub(super) al: Al,
}

/// Um item pronto para o desenho: as linhas de cada celula e a altura do item.
pub(super) struct ItemPronto {
    pub(super) linhas: Vec<Vec<String>>,
    pub(super) h: f64,
    /// So' no leiaute RTC: o item como veio, para os tributos.
    pub(super) rtc: Option<Value>,
}

fn alinhamento(a: &str) -> Al {
    match a {
        "C" => Al::C,
        "D" => Al::D,
        _ => Al::E,
    }
}

pub(super) fn colunas_moc(d: &Value) -> Vec<Coluna> {
    d["colunas"]
        .as_array()
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .map(|c| Coluna {
            titulo: s(&c[0]).to_string(),
            w: c[1].as_f64().unwrap_or(0.0),
            al: alinhamento(s(&c[2])),
        })
        .collect()
}

pub(super) fn colunas_rtc() -> Vec<Coluna> {
    let mut ws: Vec<f64> = FRACOES_RTC.iter().map(|f| LARG * f).collect();
    ws.push(LARG - ws.iter().sum::<f64>());
    TITULOS_RTC
        .iter()
        .zip(ws)
        .map(|(t, w)| Coluna {
            titulo: (*t).to_string(),
            w,
            al: Al::C,
        })
        .collect()
}

/// As linhas de uma celula: cada paragrafo cabe inteiro (numero a direita) ou quebra pela metrica da fonte; o
/// texto nunca e' cortado.
fn linhas_da_celula(texto: &str, w: f64, al: Al) -> Vec<String> {
    let mut out = Vec::new();
    for p in texto.split('\n') {
        if p.is_empty() {
            continue;
        }
        if al == Al::D && larg(p, T_ITEM, false) <= w {
            out.push(p.to_string());
        } else {
            out.extend(quebrar(p, w, T_ITEM, false));
        }
    }
    out
}

/// O item do leiaute MOC: as celulas ja formatadas, em linhas por coluna.
pub(super) fn preparar_moc(cols: &[Coluna], celulas: &Value) -> ItemPronto {
    let linhas: Vec<Vec<String>> = cols
        .iter()
        .enumerate()
        .map(|(j, c)| linhas_da_celula(s(&celulas[j]), c.w - 1.0, c.al))
        .collect();
    let n = linhas.iter().map(Vec::len).max().unwrap_or(1).max(1);
    ItemPronto {
        linhas,
        h: n as f64 * LH + 1.0,
        rtc: None,
    }
}

/// O item do leiaute RTC: descricao, informacao adicional e classificacao na primeira coluna.
pub(super) fn preparar_rtc(cols: &[Coluna], it: &Value) -> ItemPronto {
    let w0 = cols[0].w - 1.0;
    let desc = quebrar(s(&it["desc"]), w0, T_ITEM, false);
    let adic = quebrar(s(&it["adic"]), w0, T_ITEM - 0.5, false);
    let clas = quebrar(s(&it["trib"]["classificacao"]), w0, T_ITEM - 0.5, false);
    let n = desc.len() + adic.len() + clas.len();
    let h = (n as f64 * LH).max(2.0 * LH).max(4.0 * LH_TRIBUTO) + 1.0;
    ItemPronto {
        linhas: vec![desc, adic, clas],
        h,
        rtc: Some(it.clone()),
    }
}

/// `rotulo....valor` com pontos ate ocupar `w` mm (o pontilhado do modelo da NT 2026.010).
fn pontilhado(rotulo: &str, valor: &str, w: f64, tam: f64) -> String {
    let mut pontos = String::new();
    while larg(&format!("{rotulo}{pontos}.{valor}"), tam, false) <= w && pontos.len() < 60 {
        pontos.push('.');
    }
    format!("{rotulo}{pontos}{valor}")
}

fn item_moc(c: &mut Caneta, y: f64, cols: &[Coluna], it: &ItemPronto) {
    let mut x = X0;
    for (j, col) in cols.iter().enumerate() {
        for (q, l) in it.linhas[j].iter().enumerate() {
            c.celula(
                x + 0.5,
                y + 0.5 + q as f64 * LH,
                col.w - 1.0,
                LH,
                l,
                T_ITEM,
                false,
                col.al,
            );
        }
        x += col.w;
    }
}

fn item_rtc(c: &mut Caneta, y: f64, cols: &[Coluna], it: &ItemPronto) {
    let Some(v) = &it.rtc else { return };
    let xs: Vec<f64> = cols
        .iter()
        .scan(X0, |x, col| {
            let atual = *x;
            *x += col.w;
            Some(atual)
        })
        .collect();
    let w = |j: usize| cols[j].w - 1.0;
    let mut yy = y + 0.5;
    for (grupo, tam, al) in [
        (0usize, T_ITEM, Al::E),
        (1, T_ITEM - 0.5, Al::E),
        (2, T_ITEM - 0.5, Al::D),
    ] {
        for l in &it.linhas[grupo] {
            c.celula(xs[0] + 0.5, yy, w(0), LH, l, tam, false, al);
            yy += LH;
        }
    }
    let pt = y + 0.5;
    let pilha = |x: f64, w: f64, valor: &str, c: &mut Caneta, dy: f64, al: Al| {
        c.celula(x + 0.5, pt + dy, w, LH, valor, T_ITEM, false, al);
    };
    let cst = pontilhado("CST", s(&v["cst"]), w(1), T_ITEM);
    let cfop = pontilhado("CFOP", s(&v["cfop"]), w(1), T_ITEM);
    pilha(xs[1], w(1), &cst, c, 0.0, Al::E);
    pilha(xs[1], w(1), &cfop, c, LH, Al::E);
    pilha(xs[2], w(2), s(&v["qtd"]), c, 0.0, Al::D);
    if !s(&v["un"]).is_empty() {
        pilha(xs[2], w(2), &format!("[{}]", s(&v["un"])), c, LH, Al::C);
    }
    pilha(xs[3], w(3), s(&v["vun"]), c, 0.0, Al::D);
    pilha(xs[4], w(4), s(&v["vtot"]), c, 0.0, Al::D);
    let trib = &v["trib"];
    for (j, b) in trib["bases"]
        .as_array()
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .enumerate()
    {
        let texto = pontilhado(s(&b[0]), s(&b[1]), w(5), T_TRIBUTO);
        c.celula(
            xs[5] + 0.5,
            pt + j as f64 * LH_TRIBUTO,
            w(5),
            LH_TRIBUTO,
            &texto,
            T_TRIBUTO,
            false,
            Al::E,
        );
    }
    for (col, chave) in [(6usize, "aliquotas"), (7, "valores")] {
        let largura = w(col);
        let por_metade = (largura - larg(" / ", T_TRIBUTO, false)) / 2.0;
        for (j, p) in trib[chave]
            .as_array()
            .map_or(&[][..], Vec::as_slice)
            .iter()
            .enumerate()
        {
            let a = pontilhado(s(&p[0][0]), s(&p[0][1]), por_metade, T_TRIBUTO);
            let b = pontilhado(s(&p[1][0]), s(&p[1][1]), por_metade, T_TRIBUTO);
            c.celula(
                xs[col] + 0.5,
                pt + j as f64 * LH_TRIBUTO,
                largura,
                LH_TRIBUTO,
                &format!("{a} / {b}"),
                T_TRIBUTO,
                false,
                Al::E,
            );
        }
    }
}

/// Desenha o quadro: cabecalho de colunas (caixa alta, 5 pt, item 3.7.2), os itens separados por linha tracejada e as
/// colunas ate `fim` (ou ate o ultimo item, se `fim` for `None`). `y` e' o topo do cabecalho de colunas.
pub(super) fn tabela(
    c: &mut Caneta,
    y: f64,
    cols: &[Coluna],
    itens: &[ItemPronto],
    fim: Option<f64>,
) {
    let mut x = X0;
    for col in cols {
        c.ret(x, y, col.w, H_COLUNAS);
        c.escrever(
            x + 0.3,
            y + 0.3,
            col.w - 0.6,
            H_COLUNAS - 0.6,
            &col.titulo,
            T_COLUNA,
            true,
            Al::C,
            Some(T_COLUNA),
            true,
        );
        x += col.w;
    }
    let topo = y + H_COLUNAS;
    let mut yy = topo;
    for (k, it) in itens.iter().enumerate() {
        if it.rtc.is_some() {
            item_rtc(c, yy, cols, it);
        } else {
            item_moc(c, yy, cols, it);
        }
        yy += it.h;
        if k + 1 < itens.len() {
            c.linha(X0, yy, X1, yy, Some(0.6));
        }
    }
    let base = fim.unwrap_or(yy);
    let mut x = X0;
    for col in cols {
        c.linha(x, topo, x, base, None);
        x += col.w;
    }
    c.linha(X1, topo, X1, base, None);
    c.linha(X0, base, X1, base, None);
}
