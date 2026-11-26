// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Pecas de desenho do DANFE. O leiaute pensa em milimetros a partir do canto superior esquerdo da folha; cada peca
//! vira uma operacao em cm no formato que o escritor de PDF do DANFSe executa (`ret`, `lin`, `txt`, `cheio`,
//! `barras`). O texto e' medido com a metrica da Liberation Serif (a da Times New Roman, item 3.7 do Anexo II): o que
//! cabe ou quebra aqui e' o que o PDF desenha, sem kerning.

use serde_json::{json, Value};

use super::medidas::H;
use crate::danfse::metrica::{caber_em, largura_em, quebrar_em, Familia};

const FAMILIA: Familia = Familia::Serif;

/// pt -> mm.
pub(super) const MM_PT: f64 = 25.4 / 72.0;
/// Subida e descida da Liberation Serif em fracao do corpo (`hhea` da fonte: 1825 e 443 sobre 2048 unidades).
const ASCENDENTE: f64 = 0.891;
const DESCENDENTE: f64 = 0.216;
/// Espessura das linhas e das bordas: 0,2 mm, em pt.
const ESPESSURA: f64 = 0.2 / 25.4 * 72.0;
/// Cor do destaque que separa os itens e dos textos de marca d'agua.
const COR_MARCA: &str = "cinza";

/// mm -> cm, com 4 casas (o que as operacoes de desenho guardam).
pub(super) fn cm(mm: f64) -> f64 {
    ((mm / 10.0) * 10000.0 + 0.5).floor() / 10000.0
}

/// Largura do texto em mm.
#[must_use]
pub(super) fn larg(texto: &str, tam: f64, negrito: bool) -> f64 {
    largura_em(FAMILIA, texto, tam, negrito) * MM_PT
}

/// Linhas de no maximo `w` mm; palavra maior que a linha quebra por caractere.
#[must_use]
pub(super) fn quebrar(texto: &str, w: f64, tam: f64, negrito: bool) -> Vec<String> {
    quebrar_em(FAMILIA, texto, tam, w / MM_PT, negrito)
}

/// Altura da linha de um texto de `tam` pt, em mm.
#[must_use]
pub(super) fn altura_linha(tam: f64) -> f64 {
    tam * MM_PT * 1.12
}

/// (corpo, altura da linha, linhas) para caber em `w` x `h` mm: reduz o corpo de `tam` ate `minimo` sem quebrar
/// linha; no minimo, quebra e, se ainda nao couber na altura, reduz mais (ate 3 pt). Nunca corta o texto.
#[must_use]
pub(super) fn ajustar(
    texto: &str,
    w: f64,
    h: f64,
    tam: f64,
    negrito: bool,
    minimo: Option<f64>,
) -> (f64, f64, Vec<String>) {
    let minimo = minimo.unwrap_or(tam);
    let partes = texto.split('\n').count();
    let mut t = tam;
    loop {
        let lh = altura_linha(t);
        let linhas = quebrar(texto, w, t, negrito);
        if t > minimo {
            if linhas.len() == partes && partes as f64 * lh <= h {
                return (t, lh, linhas);
            }
        } else if linhas.len() as f64 * lh <= h || t <= 3.0 {
            return (t, lh, linhas);
        }
        t = ((t - 0.25) * 100.0).round() / 100.0;
    }
}

/// Altura de uma linha do rotulo (6 pt), em mm.
const H_ROTULO: f64 = 2.4;

/// O rotulo em caixa alta, em uma ou duas linhas (a segunda sai cortada com reticencias se ainda nao couber).
fn rotulo_em_linhas(rotulo: &str, w: f64) -> Vec<String> {
    let r = rotulo.to_uppercase();
    let mut l = quebrar(&r, w - 1.2, 6.0, false);
    if l.len() > 2 {
        l.truncate(2);
        l[1] = caber_em(
            FAMILIA,
            &format!("{}...", l[1]),
            6.0,
            (w - 1.2) / MM_PT,
            false,
        );
    }
    if l.is_empty() {
        l.push(String::new());
    }
    l
}

/// Altura do campo para o rotulo e o valor: o minimo da tabela (0,85 cm) ou o que o texto pedir em corpo `tam`.
#[must_use]
pub(super) fn altura_campo(w: f64, rotulo: &str, valor: &str, negrito: bool, tam: f64) -> f64 {
    let n_rotulo = rotulo_em_linhas(rotulo, w).len() as f64;
    let n_valor = quebrar(valor, w - 1.2, tam, negrito).len().max(1) as f64;
    (0.4 + n_rotulo * H_ROTULO + 0.2 + n_valor * altura_linha(tam) + 0.3).max(H)
}

/// Alinhamento horizontal de uma linha de texto na celula.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Al {
    E,
    C,
    D,
}

/// Uma folha em construcao: a lista de operacoes de desenho.
#[derive(Default)]
pub(super) struct Caneta {
    pub(super) ops: Vec<Value>,
}

impl Caneta {
    pub(super) fn ret(&mut self, x: f64, y: f64, w: f64, h: f64) {
        self.ops.push(json!({
            "t": "ret", "x": cm(x), "y": cm(y), "w": cm(w), "h": cm(h), "cinza": false, "borda": ESPESSURA
        }));
    }

    /// Linha; `tracejado` = comprimento (mm) dos tracos e dos vaos.
    pub(super) fn linha(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, tracejado: Option<f64>) {
        let mut op = json!({
            "t": "lin", "x1": cm(x1), "y1": cm(y1), "x2": cm(x2), "y2": cm(y2), "esp": ESPESSURA
        });
        if let Some(t) = tracejado {
            op["tr"] = json!(cm(t));
        }
        self.ops.push(op);
    }

    /// Uma linha de texto no retangulo (`x`, `y`, `w`, `h`): alinhada, centrada na altura, sem quebra.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn celula(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        texto: &str,
        tam: f64,
        negrito: bool,
        al: Al,
    ) {
        self.celula_cor(x, y, w, h, texto, tam, negrito, al, "preto");
    }

    #[allow(clippy::too_many_arguments)]
    fn celula_cor(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        texto: &str,
        tam: f64,
        negrito: bool,
        al: Al,
        cor: &str,
    ) {
        if texto.trim().is_empty() {
            return;
        }
        let tw = larg(texto, tam, negrito);
        let dx = match al {
            Al::E => 0.0,
            Al::C => (w - tw) / 2.0,
            Al::D => w - tw,
        };
        let altura_caixa = (ASCENDENTE + DESCENDENTE) * tam * MM_PT;
        let topo = y + (h - altura_caixa) / 2.0;
        let linha_de_base = topo + ASCENDENTE * tam * MM_PT;
        self.ops.push(json!({
            "t": "txt", "x": cm(x + dx), "y": cm(linha_de_base), "s": tam, "b": negrito, "cor": cor, "txt": texto
        }));
    }

    /// Texto ajustado ao retangulo (reduz ate `minimo`, depois quebra); `centro` = centrado na altura.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn escrever(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        texto: &str,
        tam: f64,
        negrito: bool,
        al: Al,
        minimo: Option<f64>,
        centro: bool,
    ) {
        if texto.is_empty() {
            return;
        }
        let (t, lh, linhas) = ajustar(texto, w, h, tam, negrito, minimo);
        let y0 = if centro {
            y + (h - linhas.len() as f64 * lh) / 2.0
        } else {
            y
        };
        for (i, s) in linhas.iter().enumerate() {
            self.celula(x, y0 + i as f64 * lh, w, lh, s, t, negrito, al);
        }
    }

    /// Campo do quadro: o retangulo, o rotulo em caixa alta no alto (6 pt, o minimo do item 3.7.3 do Anexo II; o que
    /// nao cabe numa linha quebra em ate duas, nunca em corpo menor) e o valor embaixo, que encolhe de `tam` ate
    /// `minimo` antes de quebrar linha. `h` vem de [`altura_campo`].
    #[allow(clippy::too_many_arguments)]
    pub(super) fn campo(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        rotulo: &str,
        valor: &str,
        al: Al,
        negrito: bool,
        tam: f64,
        minimo: f64,
    ) {
        self.ret(x, y, w, h);
        let linhas = rotulo_em_linhas(rotulo, w);
        for (i, r) in linhas.iter().enumerate() {
            self.celula(
                x + 0.6,
                y + 0.4 + i as f64 * H_ROTULO,
                w - 1.2,
                H_ROTULO,
                r,
                6.0,
                false,
                Al::E,
            );
        }
        let topo = y + 0.4 + linhas.len() as f64 * H_ROTULO + 0.2;
        self.escrever(
            x + 0.6,
            topo,
            w - 1.2,
            y + h - 0.3 - topo,
            valor,
            tam,
            negrito,
            al,
            Some(minimo),
            true,
        );
    }

    /// Titulo de bloco: caixa alta, negrito, 6 pt (acima dos 5 pt do item 3.7.1).
    pub(super) fn titulo(&mut self, x: f64, y: f64, w: f64, texto: &str) {
        self.celula(x, y + 0.8, w, 3.0, &texto.to_uppercase(), 6.0, true, Al::E);
    }

    /// Codigo de barras centrado em `x`..`x + w`: o modulo e' o maior (ate 0,33 mm) que deixa a margem clara de 10
    /// modulos em cada lado dentro de `w`. Devolve o modulo usado (mm).
    pub(super) fn barras(
        &mut self,
        padrao: &str,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        max_modulo: f64,
    ) -> f64 {
        let total = crate::danfe::barras::modulos(padrao) as f64;
        let m = (((w / (total + 20.0)) * 100.0).floor() / 100.0).min(max_modulo);
        let cx = x + (w - m * total) / 2.0;
        self.ops.push(json!({
            "t": "barras", "x": cm(cx), "y": cm(y), "m": m / 10.0, "h": cm(h), "p": padrao
        }));
        m
    }

    /// Linha de texto de marca d'agua (cinza, grande), centrada em `x`..`x + w`.
    pub(super) fn marca(&mut self, x: f64, y: f64, w: f64, texto: &str, tam: f64) {
        self.celula_cor(x, y, w, tam * 0.4, texto, tam, true, Al::C, COR_MARCA);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod testes {
    use super::*;

    #[test]
    fn texto_que_cabe_nao_encolhe_nem_quebra() {
        let (t, _, l) = ajustar("Rua das Flores", 60.0, 5.0, 8.0, false, Some(6.0));
        assert!((t - 8.0).abs() < 1e-9);
        assert_eq!(l, vec!["Rua das Flores".to_string()]);
    }

    #[test]
    fn texto_comprido_encolhe_ate_o_minimo_e_depois_quebra() {
        let longo = "CAFE TORRADO E MOIDO EM GRAOS SELECIONADOS DE ALTA QUALIDADE PACOTE 500G";
        let (t, _, l) = ajustar(longo, 30.0, 12.0, 10.0, false, Some(7.0));
        assert!(t <= 7.0 + 1e-9 && l.len() > 1, "{t} {l:?}");
        assert_eq!(l.join(" "), longo, "o texto nao pode perder palavra");
    }

    #[test]
    fn barras_deixam_a_margem_clara_de_dez_modulos() {
        let mut c = Caneta::default();
        let padrao = crate::danfe::barras::padrao(&"1".repeat(44)).unwrap();
        let m = c.barras(&padrao, 100.0, 0.0, 80.0, 10.0, 0.33);
        let total = crate::danfe::barras::modulos(&padrao) as f64;
        assert!(m >= 0.2, "modulo de {m} mm abaixo do minimo de 0,2 mm");
        assert!((80.0 - m * total) / 2.0 >= 10.0 * m, "margem clara curta");
    }
}
