// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! DANFC-e em ESC/POS: os bytes para uma impressora termica, sem abrir impressora, porta ou socket.
//!
//! Comandos do conjunto ESC/POS da Epson (o padrao de fato das termicas): `ESC @` (inicializa), `ESC t n` (pagina de
//! codigo), `ESC a n` (alinhamento), `ESC E n` (negrito), `GS ! n` (largura dupla), `LF`, `GS V 1` (corte parcial) e o
//! QR Code nativo `GS ( k` (modelo 2, tamanho do modulo, nivel de correcao M, dados, impressao). Nao ha perfil de
//! fabricante: so' o conjunto generico.
//!
//! O QR Code usa o nivel M (Manual do DANFE NFC-e 6.0, item 4.5.2) e o modulo com pontos suficientes para o conteudo
//! passar de 22 mm a 203 dpi (Manual, 3.4: 25 mm com 3 mm de zona quieta). Colunas: 48 (80 mm), 42 e 32 (58 mm).

use super::modelo::{
    Danfce, Destaque, CHAMADA_CONSULTA, COLUNAS_ITENS, ROTULO_ACRESCIMOS, ROTULO_AUTORIZACAO,
    ROTULO_DESCONTO, ROTULO_FORMA_PAGAMENTO, ROTULO_PROTOCOLO, ROTULO_QTDE_ITENS, ROTULO_TROCO,
    ROTULO_VALOR_A_PAGAR, ROTULO_VALOR_PAGO, ROTULO_VALOR_TOTAL, TRIBUTOS,
};
use super::tabelas_escpos::{codificar, numero};
use super::{ErroDanfce, Largura, Opcoes};

const ESC: u8 = 0x1b;
const GS: u8 = 0x1d;
const LF: u8 = 0x0a;
/// 22 mm de conteudo do QR a 203 dpi (8 pontos por mm), em pontos.
const PONTOS_MINIMOS_QR: usize = 176;
/// Pontos por coluna da fonte A.
const PONTOS_POR_COLUNA: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Alinha {
    Esq,
    Centro,
}

/// Uma linha do cupom ou o QR Code.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Linha {
    Texto {
        alinha: Alinha,
        negrito: bool,
        largo: bool,
        texto: String,
    },
    Qr(String),
}

fn colunas(op: &Opcoes) -> Result<usize, ErroDanfce> {
    match op.colunas {
        None => Ok(match op.largura {
            Largura::Mm80 => 48,
            Largura::Mm58 => 32,
        }),
        Some(c @ (48 | 42 | 32)) => Ok(usize::from(c)),
        Some(c) => Err(ErroDanfce::Colunas(c)),
    }
}

/// Quebra por palavra em linhas de ate `cols` caracteres; palavra maior que a linha e' cortada.
fn quebrar(texto: &str, cols: usize) -> Vec<String> {
    let mut linhas: Vec<String> = Vec::new();
    let mut atual = String::new();
    for palavra in texto.split_whitespace() {
        let mut p: Vec<char> = palavra.chars().collect();
        while p.len() > cols {
            if !atual.is_empty() {
                linhas.push(std::mem::take(&mut atual));
            }
            linhas.push(p.drain(..cols).collect());
        }
        let p: String = p.into_iter().collect();
        if atual.is_empty() {
            atual = p;
        } else if atual.chars().count() + 1 + p.chars().count() <= cols {
            atual.push(' ');
            atual.push_str(&p);
        } else {
            linhas.push(std::mem::replace(&mut atual, p));
        }
    }
    if !atual.is_empty() {
        linhas.push(atual);
    }
    linhas
}

struct Cupom {
    cols: usize,
    linhas: Vec<Linha>,
}

impl Cupom {
    fn texto(&mut self, alinha: Alinha, negrito: bool, largo: bool, t: &str) {
        let cols = if largo { self.cols / 2 } else { self.cols };
        for l in quebrar(t, cols) {
            self.linhas.push(Linha::Texto {
                alinha,
                negrito,
                largo,
                texto: l,
            });
        }
    }

    fn centro(&mut self, negrito: bool, t: &str) {
        self.texto(Alinha::Centro, negrito, false, t);
    }

    fn esq(&mut self, t: &str) {
        self.texto(Alinha::Esq, false, false, t);
    }

    /// `rotulo` a esquerda e `valor` a direita, na mesma linha se couber; senao o rotulo quebra e o valor fecha a ultima.
    fn par(&mut self, negrito: bool, rotulo: &str, valor: &str) {
        let mut ls = quebrar(rotulo, self.cols);
        let ultima = ls.pop().unwrap_or_default();
        for l in ls {
            self.texto(Alinha::Esq, negrito, false, &l);
        }
        let (a, b) = (ultima.chars().count(), valor.chars().count());
        let linha = if a + 1 + b <= self.cols {
            format!("{ultima}{}{valor}", " ".repeat(self.cols - a - b))
        } else {
            self.texto(Alinha::Esq, negrito, false, &ultima);
            format!("{}{valor}", " ".repeat(self.cols.saturating_sub(b)))
        };
        self.linhas.push(Linha::Texto {
            alinha: Alinha::Esq,
            negrito,
            largo: false,
            texto: linha,
        });
    }

    fn destaque(&mut self, d: &Destaque) {
        self.texto(Alinha::Centro, true, true, &d.linha1);
        if let Some(l2) = &d.linha2 {
            self.texto(Alinha::Centro, true, false, l2);
        }
    }
}

fn montar(d: &Danfce, cols: usize) -> Vec<Linha> {
    let mut c = Cupom {
        cols,
        linhas: Vec::new(),
    };
    // I: cabecalho
    c.centro(true, &d.cabecalho.nome);
    c.centro(
        false,
        &format!(
            "{}: {}",
            d.cabecalho.rotulo_documento, d.cabecalho.documento
        ),
    );
    c.centro(false, &d.cabecalho.endereco);
    c.centro(true, &d.cabecalho.titulo);
    if let Some(x) = &d.destaque {
        c.destaque(x);
    }
    // II: itens
    if let Some(itens) = &d.itens {
        c.texto(
            Alinha::Esq,
            true,
            false,
            &format!("{} {}", COLUNAS_ITENS[0], COLUNAS_ITENS[1]),
        );
        c.par(
            true,
            &format!(
                "{} {} X {}",
                COLUNAS_ITENS[2], COLUNAS_ITENS[3], COLUNAS_ITENS[4]
            ),
            COLUNAS_ITENS[5],
        );
        for i in itens {
            c.esq(&format!("{} {}", i.codigo, i.descricao));
            c.par(
                false,
                &format!("{} {} X {}", i.quantidade, i.unidade, i.valor_unitario),
                &i.valor_total,
            );
        }
    }
    // III: totais, pagamentos e troco
    let t = &d.totais;
    c.par(false, ROTULO_QTDE_ITENS, &t.quantidade_itens);
    c.par(false, ROTULO_VALOR_TOTAL, &t.valor_total);
    if let Some(v) = &t.acrescimos {
        c.par(false, ROTULO_ACRESCIMOS, v);
    }
    if let Some(v) = &t.desconto {
        c.par(false, ROTULO_DESCONTO, v);
    }
    if let Some(v) = &t.valor_a_pagar {
        c.par(true, ROTULO_VALOR_A_PAGAR, v);
    }
    c.par(false, ROTULO_FORMA_PAGAMENTO, ROTULO_VALOR_PAGO);
    for p in &d.pagamentos {
        c.par(false, &p.forma, &p.valor);
    }
    if let Some(v) = &d.troco {
        c.par(false, ROTULO_TROCO, v);
    }
    // IV: consulta pela chave
    c.centro(true, CHAMADA_CONSULTA);
    c.centro(false, &d.consulta.url);
    c.centro(false, &d.consulta.chave);
    // VI: consumidor
    let k = &d.consumidor;
    let rotulo = match &k.documento {
        Some(doc) => format!("{} {doc}", k.rotulo),
        None => k.rotulo.clone(),
    };
    c.centro(true, &rotulo);
    if let Some(n) = &k.nome {
        c.centro(false, n);
    }
    if let Some(e) = &k.endereco {
        c.centro(false, e);
    }
    // VII: identificacao e protocolo
    let i = &d.identificacao;
    let id = format!("NFC-e nº {} Série {} {}", i.numero, i.serie, i.emissao);
    match &i.via {
        Some(v) if id.chars().count() + 1 + v.chars().count() <= cols => {
            c.centro(true, &format!("{id} {v}"));
        }
        via => {
            c.centro(true, &id);
            if let Some(v) = via {
                c.centro(true, v);
            }
        }
    }
    if let Some(p) = &i.protocolo {
        c.centro(false, &format!("{ROTULO_PROTOCOLO} {p}"));
    }
    if let Some(a) = &i.autorizacao {
        c.centro(false, &format!("{ROTULO_AUTORIZACAO} {a}"));
    }
    if let Some(x) = &d.destaque {
        c.destaque(x);
    }
    // V: QR Code
    c.linhas.push(Linha::Qr(d.qr_code.clone()));
    // VIII: mensagem fiscal
    if let Some(m) = &d.mensagem_fiscal {
        c.centro(false, m);
    }
    if let Some(h) = &d.homologacao_texto {
        c.centro(true, h);
    }
    // IX: interesse do contribuinte
    if let Some(m) = &d.interesse_do_contribuinte {
        c.esq(m);
    }
    if let Some(v) = &d.tributos {
        c.esq(&format!("{TRIBUTOS} {v}"));
    }
    if let Some(m) = &d.mensagem_institucional {
        c.centro(false, m);
    }
    c.linhas
}

fn comando_qr(dados: &str, cols: usize) -> Result<Vec<u8>, ErroDanfce> {
    let qr = qrcode::QrCode::with_error_correction_level(dados.as_bytes(), qrcode::EcLevel::M)
        .map_err(|e| ErroDanfce::Qr(e.to_string()))?;
    let modulos = qr.width();
    let cabe = (cols * PONTOS_POR_COLUNA) / modulos;
    let modulo = PONTOS_MINIMOS_QR.div_ceil(modulos).min(cabe).clamp(1, 16);
    let n = dados.len() + 3;
    let mut b = vec![ESC, b'a', 1];
    // modelo 2
    b.extend_from_slice(&[GS, b'(', b'k', 4, 0, 0x31, 0x41, 0x32, 0x00]);
    // tamanho do modulo, em pontos
    b.extend_from_slice(&[GS, b'(', b'k', 3, 0, 0x31, 0x43, modulo as u8]);
    // nivel de correcao M
    b.extend_from_slice(&[GS, b'(', b'k', 3, 0, 0x31, 0x45, 0x31]);
    // dados (UTF-8, Manual 4.5.3)
    b.extend_from_slice(&[
        GS,
        b'(',
        b'k',
        (n & 0xff) as u8,
        (n >> 8) as u8,
        0x31,
        0x50,
        0x30,
    ]);
    b.extend_from_slice(dados.as_bytes());
    // imprime
    b.extend_from_slice(&[GS, b'(', b'k', 3, 0, 0x31, 0x51, 0x30, LF]);
    b.extend_from_slice(&[ESC, b'a', 0]);
    Ok(b)
}

pub(super) fn bytes(d: &Danfce, op: &Opcoes) -> Result<Vec<u8>, ErroDanfce> {
    let cols = colunas(op)?;
    let mut b = vec![ESC, b'@', ESC, b't', numero(op.pagina_de_codigo)];
    for l in montar(d, cols) {
        match l {
            Linha::Qr(dados) => b.extend(comando_qr(&dados, cols)?),
            Linha::Texto {
                alinha,
                negrito,
                largo,
                texto,
            } => {
                b.extend_from_slice(&[ESC, b'a', u8::from(alinha == Alinha::Centro)]);
                b.extend_from_slice(&[ESC, b'E', u8::from(negrito)]);
                b.extend_from_slice(&[GS, b'!', if largo { 0x10 } else { 0x00 }]);
                b.extend(codificar(&texto, op.pagina_de_codigo));
                b.push(LF);
            }
        }
    }
    b.extend_from_slice(&[ESC, b'a', 0, ESC, b'E', 0, GS, b'!', 0]);
    if op.cortar {
        b.extend_from_slice(&[LF, LF, LF, LF, GS, b'V', 1]);
    } else {
        b.extend_from_slice(&[LF, LF]);
    }
    Ok(b)
}
