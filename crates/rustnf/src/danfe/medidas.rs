// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Medidas do DANFE A4 retrato, folhas soltas (MOC 7.00, Anexo II, item 3.8.1 e Anexo III.02), em milimetros a partir
//! do canto superior esquerdo, e os corpos de fonte minimos do item 3.7.
//!
//! A tabela 3.8.1 da' alturas e larguras MINIMAS e posicoes sugeridas: a largura util soma 20,57 cm, que com a posicao
//! esquerda de 0,25 cm deixaria 0,18 cm a direita, abaixo do minimo de 0,2 cm do item 3.6.2. Aqui as larguras da
//! tabela ficam inteiras e o corpo impresso comeca em 0,21 cm: sobra 0,22 cm a direita, e as duas margens ficam
//! entre 0,2 cm e 0,8 cm. Quando o conteudo nao cabe no campo, o campo cresce na altura (a tabela e' de minimos) em
//! vez de o texto descer abaixo do corpo exigido.

/// Margem esquerda (item 3.6.2: entre 0,2 e 0,8 cm).
pub(super) const X0: f64 = 2.1;
/// Largura util: 20,57 cm (item 3.8.1).
pub(super) const LARG: f64 = 205.7;
/// Borda direita do corpo impresso.
pub(super) const X1: f64 = X0 + LARG;
/// Topo do canhoto: 0,42 cm (item 3.8.1).
pub(super) const TOPO: f64 = 4.2;
/// Fim do quadro das informacoes complementares: 26,33 cm + 3,07 cm (item 3.8.1); sobra 0,3 cm embaixo da folha.
pub(super) const FUNDO: f64 = 294.0;
/// Altura minima do campo: 0,85 cm.
pub(super) const H: f64 = 8.5;
/// Altura do titulo de bloco: 0,42 cm.
pub(super) const HT: f64 = 4.2;
/// Altura minima do quadro do emitente e do bloco do codigo de barras: 3,92 cm.
pub(super) const H_CABECA: f64 = 39.2;
/// Altura do quadro das informacoes complementares e do reservado ao fisco: 3,07 cm.
pub(super) const H_COMPL: f64 = 30.7;
/// Altura do cabecalho da tabela dos itens (duas linhas de 5 pt).
pub(super) const H_COLUNAS: f64 = 6.0;
/// Distancia entre as linhas de texto dos itens e das informacoes complementares, em mm (6,5 pt).
pub(super) const LH: f64 = 2.75;

/// Titulo de bloco: negrito e caixa alta, no minimo 5 pt (item 3.7.1).
pub(super) const T_BLOCO: f64 = 6.0;
/// Descritivo (rotulo) dos campos: caixa alta, no minimo 6 pt (item 3.7.3).
pub(super) const T_ROTULO: f64 = 6.0;
/// Descritivo das colunas dos produtos: caixa alta, no minimo 5 pt (item 3.7.2).
pub(super) const T_COLUNA: f64 = 5.0;
/// Conteudo dos produtos e das informacoes complementares: no minimo 6 pt (itens 3.7.7 e 3.7.8).
pub(super) const T_ITEM: f64 = 6.5;
/// Conteudo dos demais campos: no minimo 10 pt (item 3.7.9).
pub(super) const T_VALOR: f64 = 10.0;
/// "DANFE": negrito, no minimo 12 pt (item 3.7.4).
pub(super) const T_DANFE: f64 = 14.0;
/// Numero, serie, folha e tipo de operacao: negrito, no minimo 10 pt (item 3.7.4).
pub(super) const T_NUMERO: f64 = 10.0;
/// "Documento auxiliar..." e as descricoes de entrada e saida: no minimo 8 pt (item 3.7.4).
pub(super) const T_AUXILIAR: f64 = 8.0;
/// Razao social do emitente: negrito, no minimo 12 pt (item 3.7.6).
pub(super) const T_EMITENTE: f64 = 12.0;
/// Endereco, municipio, CEP e fone do emitente: negrito, no minimo 8 pt (item 3.7.6).
pub(super) const T_ENDERECO: f64 = 8.0;
/// Chave de acesso: negrito (item 3.7.5, que nao fixa o corpo); 44 algarismos e 10 espacos em 8 cm pedem 9 pt.
pub(super) const T_CHAVE: f64 = 9.0;
/// Menor corpo da chave quando a largura aperta.
pub(super) const T_CHAVE_MIN: f64 = 7.0;

/// Altura do codigo de barras: o minimo e' 0,8 cm (item 2 do Anexo II).
pub(super) const H_BARRAS: f64 = 10.8;
/// Largura maxima do modulo do codigo de barras, em mm.
pub(super) const MODULO_MAX: f64 = 0.33;
/// Largura do quadro do codigo de barras e da chave: 12,54 cm a partir da esquerda ate a borda direita.
pub(super) const X_BARRAS: f64 = X0 + 125.4;
/// Largura do quadro do emitente.
pub(super) const W_EMITENTE: f64 = 100.0;
/// Largura do quadro "DANFE".
pub(super) const W_DANFE: f64 = 25.4;
