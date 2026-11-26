// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! DANFC-e em PDF de bobina: as divisoes do Manual na ordem da figura 5 (QR Code centralizado), desenhadas com as
//! operacoes do escritor do DANFSe (Liberation Sans, texto selecionavel, QR em retangulos). A altura da pagina e' a do
//! conteudo.
//!
//! Medidas: papel de 80 mm ou 58 mm (o Manual pede no minimo 56 mm, item 3.3); margem lateral de 4 mm (o minimo do
//! Manual e' 2 mm); QR Code de 36 mm com 10% de zona quieta em cada lado, 28,8 mm de conteudo (o minimo do Manual e' 25
//! mm com 22 mm de conteudo, item 3.4); o QR denso (mais de 61 modulos, o da contingencia off-line) vai a 48 mm. O corpo da letra nao e' regulado pelo Manual: 7,5 pt (80 mm) e 6,5 pt (58 mm).

use serde_json::{json, Value};

use super::modelo::{
    Danfce, Destaque, CHAMADA_CONSULTA, COLUNAS_ITENS, ROTULO_ACRESCIMOS, ROTULO_AUTORIZACAO,
    ROTULO_DESCONTO, ROTULO_FORMA_PAGAMENTO, ROTULO_PROTOCOLO, ROTULO_QTDE_ITENS, ROTULO_TROCO,
    ROTULO_VALOR_A_PAGAR, ROTULO_VALOR_PAGO, ROTULO_VALOR_TOTAL, TRIBUTOS,
};
use super::{ErroDanfce, Largura};
use crate::danfse::escritor::pdf_paginas_com_tamanho;
use crate::danfse::metrica::{largura_em, quebrar_em, Familia};
use crate::danfse::qr;

/// mm por ponto.
const MM_PT: f64 = 25.4 / 72.0;
const MARGEM: f64 = 4.0;
/// Lado total do QR Code, em mm (zona quieta inclusa).
const LADO_QR: f64 = 36.0;
const ZONA_QUIETA: f64 = 0.10;
/// Espaco antes de cada divisao, em mm.
const ESPACO: f64 = 1.6;
const FAM: Familia = Familia::Sans;

struct Papel {
    largura: f64,
    corpo: f64,
    destaque: f64,
}

struct Pagina {
    p: Papel,
    y: f64,
    ops: Vec<Value>,
}

fn cm(mm: f64) -> f64 {
    ((mm / 10.0) * 10000.0 + 0.5).floor() / 10000.0
}

impl Pagina {
    fn util(&self) -> f64 {
        self.p.largura - 2.0 * MARGEM
    }

    fn larg(&self, t: &str, tam: f64, negrito: bool) -> f64 {
        largura_em(FAM, t, tam, negrito) * MM_PT
    }

    fn linha_de_texto(&mut self, x: f64, t: &str, tam: f64, negrito: bool) {
        let altura = tam * MM_PT * 1.2;
        self.ops.push(json!({
            "t": "txt", "x": cm(x), "y": cm(self.y + tam * MM_PT * 0.95),
            "s": tam, "b": negrito, "txt": t
        }));
        self.y += altura;
    }

    fn quebrado(&self, t: &str, tam: f64, negrito: bool) -> Vec<String> {
        quebrar_em(FAM, t, tam, self.util() / MM_PT, negrito)
    }

    fn centro_com(&mut self, t: &str, tam: f64, negrito: bool) {
        for l in self.quebrado(t, tam, negrito) {
            let x = MARGEM + (self.util() - self.larg(&l, tam, negrito)) / 2.0;
            self.linha_de_texto(x, &l, tam, negrito);
        }
    }

    fn centro(&mut self, t: &str, negrito: bool) {
        self.centro_com(t, self.p.corpo, negrito);
    }

    fn esq(&mut self, t: &str, negrito: bool) {
        for l in self.quebrado(t, self.p.corpo, negrito) {
            self.linha_de_texto(MARGEM, &l, self.p.corpo, negrito);
        }
    }

    /// Rotulo a esquerda e valor a direita; o rotulo quebra se nao couber e o valor fecha a ultima linha.
    fn par(&mut self, rotulo: &str, valor: &str, negrito: bool) {
        let tam = self.p.corpo;
        let wv = self.larg(valor, tam, negrito);
        let folga = self.larg("  ", tam, negrito);
        let mut ls = quebrar_em(FAM, rotulo, tam, self.util() / MM_PT, negrito);
        let ultima = ls.pop().unwrap_or_default();
        for l in ls {
            self.linha_de_texto(MARGEM, &l, tam, negrito);
        }
        let ya = self.y;
        if self.larg(&ultima, tam, negrito) + folga + wv <= self.util() {
            self.linha_de_texto(MARGEM, &ultima, tam, negrito);
            self.y = ya;
        } else {
            self.linha_de_texto(MARGEM, &ultima, tam, negrito);
        }
        let x = MARGEM + self.util() - wv;
        self.linha_de_texto(x, valor, tam, negrito);
    }

    fn destaque(&mut self, d: &Destaque) {
        self.y += 0.8;
        let g = self.p.destaque;
        self.centro_com(&d.linha1, g, true);
        if let Some(l2) = &d.linha2 {
            self.centro_com(l2, self.p.corpo * 1.15, true);
        }
        self.y += 0.8;
    }

    /// Traco fino entre as divisoes.
    fn divisor(&mut self) {
        self.y += ESPACO / 2.0;
        self.ops.push(json!({
            "t": "lin", "x1": cm(MARGEM), "y1": cm(self.y), "x2": cm(MARGEM + self.util()), "y2": cm(self.y),
            "esp": 0.3, "tr": 0.08
        }));
        self.y += ESPACO / 2.0;
    }
}

fn desenhar(d: &Danfce, largura: Largura, qr: &Value) -> (Vec<Value>, f64) {
    let (w, corpo, destaque) = match largura {
        Largura::Mm80 => (80.0, 7.5, 12.0),
        Largura::Mm58 => (58.0, 6.5, 10.0),
    };
    let mut p = Pagina {
        p: Papel {
            largura: w,
            corpo,
            destaque,
        },
        y: MARGEM,
        ops: Vec::new(),
    };
    // I: cabecalho
    p.centro(&d.cabecalho.nome, true);
    p.centro(
        &format!(
            "{}: {}",
            d.cabecalho.rotulo_documento, d.cabecalho.documento
        ),
        false,
    );
    p.centro(&d.cabecalho.endereco, false);
    p.centro(&d.cabecalho.titulo, true);
    if let Some(x) = &d.destaque {
        p.destaque(x);
    }
    // II: itens
    p.divisor();
    if let Some(itens) = &d.itens {
        p.esq(&format!("{} {}", COLUNAS_ITENS[0], COLUNAS_ITENS[1]), true);
        p.par(
            &format!(
                "{} {} X {}",
                COLUNAS_ITENS[2], COLUNAS_ITENS[3], COLUNAS_ITENS[4]
            ),
            COLUNAS_ITENS[5],
            true,
        );
        for i in itens {
            p.esq(&format!("{} {}", i.codigo, i.descricao), false);
            p.par(
                &format!("{} {} X {}", i.quantidade, i.unidade, i.valor_unitario),
                &i.valor_total,
                false,
            );
        }
        p.divisor();
    }
    // III: totais, pagamentos e troco
    let t = &d.totais;
    p.par(ROTULO_QTDE_ITENS, &t.quantidade_itens, false);
    p.par(ROTULO_VALOR_TOTAL, &t.valor_total, false);
    if let Some(v) = &t.acrescimos {
        p.par(ROTULO_ACRESCIMOS, v, false);
    }
    if let Some(v) = &t.desconto {
        p.par(ROTULO_DESCONTO, v, false);
    }
    if let Some(v) = &t.valor_a_pagar {
        p.par(ROTULO_VALOR_A_PAGAR, v, true);
    }
    p.y += 0.6;
    p.par(ROTULO_FORMA_PAGAMENTO, ROTULO_VALOR_PAGO, false);
    for g in &d.pagamentos {
        p.par(&g.forma, &g.valor, false);
    }
    if let Some(v) = &d.troco {
        p.par(ROTULO_TROCO, v, false);
    }
    p.divisor();
    // IV: consulta pela chave
    p.centro(CHAMADA_CONSULTA, true);
    p.centro(&d.consulta.url, false);
    p.centro(&d.consulta.chave, false);
    p.y += 1.0;
    // VI: consumidor
    let k = &d.consumidor;
    let rotulo = match &k.documento {
        Some(doc) => format!("{} {doc}", k.rotulo),
        None => k.rotulo.clone(),
    };
    p.centro(&rotulo, true);
    if let Some(n) = &k.nome {
        p.centro(n, false);
    }
    if let Some(e) = &k.endereco {
        p.centro(e, false);
    }
    p.y += 0.6;
    // VII: identificacao e protocolo
    let i = &d.identificacao;
    let id = format!("NFC-e nº {} Série {} {}", i.numero, i.serie, i.emissao);
    match &i.via {
        Some(v) if p.larg(&format!("{id} {v}"), p.p.corpo, true) <= p.util() => {
            p.centro(&format!("{id} {v}"), true);
        }
        via => {
            p.centro(&id, true);
            if let Some(v) = via {
                p.centro(v, true);
            }
        }
    }
    if let Some(n) = &i.protocolo {
        p.centro(&format!("{ROTULO_PROTOCOLO} {n}"), false);
    }
    if let Some(a) = &i.autorizacao {
        p.centro(&format!("{ROTULO_AUTORIZACAO} {a}"), false);
    }
    if let Some(x) = &d.destaque {
        p.destaque(x);
    }
    // V: QR Code (mais denso, mais largo: o modulo nao fica menor que o do QR de 61 modulos em 36 mm)
    p.y += 1.0;
    let modulos = qr["n"].as_f64().unwrap_or(0.0);
    let lado = if modulos > 61.0 {
        p.util().min(48.0)
    } else {
        LADO_QR.min(p.util())
    };
    let x = MARGEM + (p.util() - lado) / 2.0;
    let margem_qr = lado * ZONA_QUIETA;
    p.ops.push(json!({
        "t": "qr", "x": cm(x + margem_qr), "y": cm(p.y + margem_qr), "lado": cm(lado - 2.0 * margem_qr)
    }));
    p.y += lado + 1.0;
    // VIII: mensagem fiscal
    if let Some(m) = &d.mensagem_fiscal {
        p.centro(m, false);
    }
    if let Some(h) = &d.homologacao_texto {
        p.centro(h, true);
    }
    // IX: interesse do contribuinte
    if d.interesse_do_contribuinte.is_some() || d.tributos.is_some() {
        p.divisor();
    }
    if let Some(m) = &d.interesse_do_contribuinte {
        p.esq(m, false);
    }
    if let Some(v) = &d.tributos {
        p.esq(&format!("{TRIBUTOS} {v}"), false);
    }
    if let Some(m) = &d.mensagem_institucional {
        p.y += ESPACO;
        p.centro(m, false);
    }
    let altura = p.y + MARGEM;
    (p.ops, altura)
}

pub(super) fn pdf(d: &Danfce, largura: Largura) -> Result<Vec<u8>, ErroDanfce> {
    let qr = qr::dados(&d.qr_code).map_err(|e| ErroDanfce::Qr(e.to_string()))?;
    let (ops, altura) = desenhar(d, largura, &qr);
    let w = match largura {
        Largura::Mm80 => 80.0,
        Largura::Mm58 => 58.0,
    };
    let dados = json!({ "qr": qr });
    let pt = |mm: f64| (mm / MM_PT) as f32;
    pdf_paginas_com_tamanho(
        FAM,
        &format!("DANFC-e {}", d.identificacao.numero),
        &dados,
        &[(pt(w), pt(altura), &ops)],
        None,
    )
    .map_err(|e| match e {
        crate::danfse::ErroDanfse::Pdf(m) => ErroDanfce::Pdf(m),
        outro => ErroDanfce::Pdf(outro.to_string()),
    })
}
