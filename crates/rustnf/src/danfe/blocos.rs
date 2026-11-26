// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Os blocos do DANFE retrato, folhas soltas (MOC 7.00, Anexo II, item 3.8.1 e Anexo III.02). Cada funcao desenha o
//! bloco a partir de `y` e devolve a altura que ele ocupou; para saber so' a altura (o plano das folhas), desenha-se
//! numa caneta que se joga fora.

use serde_json::{json, Value};

use super::desenho::{ajustar, altura_campo, altura_linha, cm, Al, Caneta};
use super::medidas::*;

pub(super) fn s(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}

/// Um campo de uma linha de campos: rotulo, valor, largura (mm), alinhamento e negrito.
pub(super) struct Cp<'a> {
    pub(super) rotulo: &'a str,
    pub(super) valor: &'a str,
    pub(super) w: f64,
    pub(super) al: Al,
    pub(super) negrito: bool,
}

pub(super) fn cp<'a>(rotulo: &'a str, valor: &'a str, w: f64) -> Cp<'a> {
    Cp {
        rotulo,
        valor,
        w,
        al: Al::E,
        negrito: false,
    }
}

impl<'a> Cp<'a> {
    pub(super) fn centro(mut self) -> Self {
        self.al = Al::C;
        self
    }

    pub(super) fn direita(mut self) -> Self {
        self.al = Al::D;
        self
    }

    pub(super) fn negrito(mut self) -> Self {
        self.negrito = true;
        self
    }
}

/// Uma linha de campos lado a lado: todos com a mesma altura (a do que mais pede). Devolve a altura.
pub(super) fn linha_campos(c: &mut Caneta, y: f64, campos: &[Cp]) -> f64 {
    let h = campos
        .iter()
        .map(|p| altura_campo(p.w, p.rotulo, p.valor, p.negrito, T_VALOR))
        .fold(H, f64::max);
    let mut x = X0;
    for p in campos {
        c.campo(
            x, y, p.w, h, p.rotulo, p.valor, p.al, p.negrito, T_VALOR, T_VALOR,
        );
        x += p.w;
    }
    h
}

/// Varias linhas de campos, uma sob a outra, depois do titulo `titulo`. Devolve a altura do bloco.
pub(super) fn bloco(c: &mut Caneta, y: f64, titulo: &str, linhas: &[Vec<Cp>]) -> f64 {
    c.titulo(X0, y, LARG, titulo);
    let mut yy = y + HT;
    for l in linhas {
        yy += linha_campos(c, yy, l);
    }
    yy - y
}

/// Canhoto (item 3.8.1): a frase de recebimento, data e assinatura e o numero da nota, com a linha de picote.
pub(super) fn canhoto(c: &mut Caneta, y: f64, d: &Value) -> f64 {
    c.ret(X0, y, 161.0, H);
    c.escrever(
        X0 + 0.6,
        y + 0.5,
        159.8,
        H - 1.0,
        s(&d["canhoto_texto"]),
        6.5,
        false,
        Al::E,
        Some(6.0),
        true,
    );
    let w1 = 41.0;
    for (x, w, rotulo) in [
        (X0, w1, "Data de recebimento"),
        (X0 + w1, 120.0, "Identificação e assinatura do recebedor"),
    ] {
        let h = altura_campo(w, rotulo, "", false, T_VALOR);
        c.campo(x, y + H, w, h, rotulo, "", Al::E, false, T_VALOR, T_VALOR);
    }
    c.ret(X0 + 161.0, y, LARG - 161.0, 2.0 * H);
    let nota = format!("NF-e\nNº {}\nSÉRIE {}", s(&d["numero"]), s(&d["serie"]));
    c.escrever(
        X0 + 161.0,
        y + 1.5,
        LARG - 161.0,
        14.0,
        &nota,
        T_NUMERO,
        true,
        Al::C,
        Some(T_NUMERO),
        true,
    );
    c.linha(X0, y + 2.0 * H + 2.0, X1, y + 2.0 * H + 2.0, Some(1.0));
    2.0 * H + 4.2
}

/// Altura do quadro do emitente: o minimo da tabela (3,92 cm) ou o que o nome (12 pt) e o endereco (8 pt) pedirem.
fn altura_emitente(d: &Value) -> f64 {
    let (_, w) = area_emitente(d);
    let e = &d["emitente"];
    let (_, lh_n, nome) = ajustar(s(&e["nome"]), w, 1e6, T_EMITENTE, true, None);
    let (_, lh_e, ender) = ajustar(s(&e["linhas"]), w, 1e6, T_ENDERECO, true, None);
    (4.0 + nome.len() as f64 * lh_n + 3.0 + ender.len() as f64 * lh_e).max(H_CABECA)
}

/// Onde vai o texto do emitente: `x` e largura, com ou sem o logotipo (item 3.1.3).
fn area_emitente(d: &Value) -> (f64, f64) {
    if d["logo"].as_bool().unwrap_or(false) {
        (X0 + 29.0, 70.0)
    } else {
        (X0 + 1.0, 98.0)
    }
}

/// Cabecalho de toda folha (item 3.5): emitente, "DANFE", numero/serie/folha, codigo de barras, chave, campos 1 e 2,
/// natureza, protocolo e os dados de identificacao do emitente. Devolve a altura.
pub(super) fn cabecalho(c: &mut Caneta, y: f64, d: &Value, folha: usize, total: usize) -> f64 {
    let h = altura_emitente(d);
    emitente(c, y, h, d);
    danfe(c, y, h, d, folha, total);
    let x = X_BARRAS;
    let w = X1 - x;
    c.ret(x, y, w, 14.8);
    c.barras(s(&d["chave_barras"]), x, y + 2.0, w, H_BARRAS, MODULO_MAX);
    let hc = altura_campo(w, "Chave de acesso", s(&d["chave"]), true, T_CHAVE).max(H);
    c.campo(
        x,
        y + 14.8,
        w,
        hc,
        "Chave de acesso",
        s(&d["chave"]),
        Al::C,
        true,
        T_CHAVE,
        T_CHAVE_MIN,
    );
    let y1 = y + 14.8 + hc;
    let h1 = (h - 14.8 - hc).max(0.0);
    c.ret(x, y1, w, h1);
    if let Some(p) = d["campo1"]["barras"].as_str() {
        c.barras(p, x, y1 + 2.0, w, h1 - 4.0, MODULO_MAX);
    } else {
        let consulta = s(&d["campo1"]["texto"]);
        c.escrever(
            x + 1.0,
            y1 + 1.0,
            w - 2.0,
            h1 - 2.0,
            consulta,
            T_VALOR,
            false,
            Al::C,
            Some(T_VALOR),
            true,
        );
    }
    let e = &d["emitente"];
    let c2 = &d["campo2"];
    let mut yy = y + h;
    yy += linha_campos(
        c,
        yy,
        &[
            cp("Natureza da operação", s(&d["natureza"]), 125.4),
            cp(s(&c2[0]), s(&c2[1]), LARG - 125.4).centro().negrito(),
        ],
    );
    yy += linha_campos(
        c,
        yy,
        &[
            cp("Inscrição estadual", s(&e["ie"]), 68.6),
            cp("Inscrição estadual do subst. trib.", s(&e["iest"]), 68.6),
            cp("CNPJ / CPF", s(&e["cnpj"]), LARG - 137.2),
        ],
    );
    if let Some(crt) = d["rtc"]["crt"].as_str() {
        let w1 = LARG * 0.54;
        yy += linha_campos(
            c,
            yy,
            &[
                cp("Código do regime tributário", crt, w1),
                cp("Tipo de regime de apuração do IBS e da CBS", "", LARG - w1),
            ],
        );
    }
    yy - y
}

/// Quadro do emitente: nome em 12 pt e endereco em 8 pt, os dois em negrito (item 3.7.6); logotipo opcional (3.1.3).
fn emitente(c: &mut Caneta, y: f64, h: f64, d: &Value) {
    let e = &d["emitente"];
    c.ret(X0, y, W_EMITENTE, h);
    let (x, w) = area_emitente(d);
    if d["logo"].as_bool().unwrap_or(false) {
        c.ops.push(json!({
            "t": "logo", "x": cm(X0 + 1.5), "y": cm(y + 2.0), "w": cm(26.0), "h": cm(26.0)
        }));
    }
    let (_, lh_n, nome) = ajustar(s(&e["nome"]), w, 1e6, T_EMITENTE, true, None);
    let (_, lh_e, ender) = ajustar(s(&e["linhas"]), w, 1e6, T_ENDERECO, true, None);
    let hn = nome.len() as f64 * lh_n;
    let he = ender.len() as f64 * lh_e;
    let topo = y + (h - (hn + 3.0 + he)) / 2.0;
    for (i, l) in nome.iter().enumerate() {
        c.celula(
            x,
            topo + i as f64 * lh_n,
            w,
            lh_n,
            l,
            T_EMITENTE,
            true,
            Al::C,
        );
    }
    for (i, l) in ender.iter().enumerate() {
        c.celula(
            x,
            topo + hn + 3.0 + i as f64 * lh_e,
            w,
            lh_e,
            l,
            T_ENDERECO,
            true,
            Al::C,
        );
    }
}

/// Quadro "DANFE" (item 3.7.4): titulo, descricao, entrada ou saida com o tipo de operacao e numero, serie e folha.
fn danfe(c: &mut Caneta, y: f64, h: f64, d: &Value, folha: usize, total: usize) {
    let x = X0 + W_EMITENTE;
    c.ret(x, y, W_DANFE, h);
    c.celula(x, y + 1.0, W_DANFE, 5.5, "DANFE", T_DANFE, true, Al::C);
    c.escrever(
        x + 0.8,
        y + 6.8,
        W_DANFE - 1.6,
        11.0,
        "DOCUMENTO AUXILIAR DA NOTA FISCAL ELETRÔNICA",
        T_AUXILIAR,
        false,
        Al::C,
        Some(T_AUXILIAR),
        false,
    );
    c.celula(
        x + 1.0,
        y + 18.0,
        15.0,
        3.3,
        "0 - ENTRADA",
        T_AUXILIAR,
        false,
        Al::E,
    );
    c.celula(
        x + 1.0,
        y + 21.3,
        15.0,
        3.3,
        "1 - SAÍDA",
        T_AUXILIAR,
        false,
        Al::E,
    );
    c.ret(x + 19.0, y + 18.2, 5.0, 5.5);
    c.celula(
        x + 19.0,
        y + 18.2,
        5.0,
        5.5,
        s(&d["tipo_nf"]),
        12.0,
        true,
        Al::C,
    );
    let id = format!(
        "Nº {}\nSÉRIE {}\nFOLHA {folha:02}/{total:02}",
        s(&d["numero"]),
        s(&d["serie"])
    );
    c.escrever(
        x + 0.5,
        y + 25.5,
        W_DANFE - 1.0,
        13.0,
        &id,
        T_NUMERO,
        true,
        Al::C,
        Some(T_NUMERO),
        false,
    );
}

fn participante_linhas<'a>(p: &'a Value, completo: bool) -> Vec<Vec<Cp<'a>>> {
    let n = |k: &str| s(&p[k]);
    if completo {
        vec![
            vec![
                cp("Nome / razão social", n("nome"), 123.2),
                cp("CNPJ / CPF", n("doc"), 53.3).centro().negrito(),
                cp("Data da emissão", "", LARG - 176.5).centro(),
            ],
            vec![
                cp("Endereço", n("endereco"), 101.6),
                cp("Bairro / distrito", n("bairro"), 48.3),
                cp("CEP", n("cep"), 26.6).centro(),
                cp("Data da saída/entrada", "", LARG - 176.5)
                    .centro()
                    .negrito(),
            ],
            vec![
                cp("Município", n("municipio"), 71.1),
                cp("Fone / fax", n("fone"), 40.6),
                cp("UF", n("uf"), 11.5).centro(),
                cp("Inscrição estadual", n("ie"), 53.3).centro(),
                cp("Hora da saída/entrada", "", LARG - 176.5)
                    .centro()
                    .negrito(),
            ],
        ]
    } else {
        vec![
            vec![
                cp("Nome / razão social", n("nome"), 123.2),
                cp("CNPJ / CPF", n("doc"), 53.3).centro(),
                cp("Inscrição estadual", n("ie"), LARG - 176.5).centro(),
            ],
            vec![
                cp("Endereço", n("endereco"), 101.6),
                cp("Bairro / distrito", n("bairro"), 48.3),
                cp("Município", n("municipio"), 44.4),
                cp("UF", n("uf"), LARG - 194.3).centro(),
            ],
        ]
    }
}

/// Destinatario/remetente com as datas e a hora da emissao e da saida (item 3.8.1).
pub(super) fn destinatario(c: &mut Caneta, y: f64, d: &Value) -> f64 {
    let vazio = json!({});
    let p = if d["destinatario"].is_null() {
        &vazio
    } else {
        &d["destinatario"]
    };
    let mut linhas = participante_linhas(p, true);
    linhas[0][2].valor = s(&d["emissao"]);
    linhas[1][3].valor = s(&d["saida_data"]);
    linhas[2][4].valor = s(&d["saida_hora"]);
    bloco(c, y, "DESTINATÁRIO / REMETENTE", &linhas)
}

/// Local de retirada ou de entrega (itens 3.1.4 e 3.1.5).
pub(super) fn local(c: &mut Caneta, y: f64, titulo: &str, p: &Value) -> f64 {
    bloco(c, y, titulo, &participante_linhas(p, false))
}

/// Fatura e duplicatas (item 3.1.6): o texto da fatura e, em seis colunas, as duplicatas.
pub(super) fn fatura(c: &mut Caneta, y: f64, f: &Value) -> f64 {
    c.titulo(X0, y, LARG, "FATURA / DUPLICATAS");
    let mut yy = y + HT;
    if !s(&f["texto"]).is_empty() {
        let h = altura_linha(T_VALOR) + 2.0;
        c.ret(X0, yy, LARG, h);
        c.escrever(
            X0 + 0.6,
            yy + 1.0,
            LARG - 1.2,
            h - 1.0,
            s(&f["texto"]),
            T_VALOR,
            false,
            Al::E,
            Some(T_VALOR),
            true,
        );
        yy += h;
    }
    let w = LARG / 6.0;
    let dups = f["duplicatas"].as_array().map_or(&[][..], Vec::as_slice);
    for linha in dups.chunks(6) {
        let h = linha
            .iter()
            .map(|d| {
                let n = super::desenho::quebrar(s(d), w - 1.2, T_VALOR, false).len() as f64;
                n * altura_linha(T_VALOR) + 1.0
            })
            .fold(H, f64::max);
        for (k, dup) in linha.iter().enumerate() {
            let x = X0 + k as f64 * w;
            c.ret(x, yy, w, h);
            c.escrever(
                x + 0.6,
                yy + 0.5,
                w - 1.2,
                h - 1.0,
                s(dup),
                T_VALOR,
                false,
                Al::E,
                Some(T_VALOR),
                true,
            );
        }
        yy += h;
    }
    yy - y
}

/// Calculo do imposto (item 3.8.1).
pub(super) fn imposto(c: &mut Caneta, y: f64, d: &Value) -> f64 {
    let t = &d["imposto"];
    let v = |k: &str| s(&t[k]);
    let u = 40.6;
    let g = 33.0;
    bloco(
        c,
        y,
        "CÁLCULO DO IMPOSTO",
        &[
            vec![
                cp("Base de cálculo do ICMS", v("vBC"), u).direita(),
                cp("Valor do ICMS", v("vICMS"), u).direita(),
                cp("Base de cálculo do ICMS ST", v("vBCST"), u).direita(),
                cp("Valor do ICMS ST", v("vST"), u).direita(),
                cp("Valor total dos produtos", v("vProd"), LARG - 4.0 * u).direita(),
            ],
            vec![
                cp("Valor do frete", v("vFrete"), g).direita(),
                cp("Valor do seguro", v("vSeg"), g).direita(),
                cp("Desconto", v("vDesc"), g).direita(),
                cp("Outras despesas acessórias", v("vOutro"), g).direita(),
                cp("Valor do IPI", v("vIPI"), g).direita(),
                cp("Valor total da nota", v("vNF"), LARG - 5.0 * g)
                    .direita()
                    .negrito(),
            ],
        ],
    )
}

/// Transportador e volumes (itens 3.1.10 e 3.8.1).
pub(super) fn transporte(c: &mut Caneta, y: f64, d: &Value) -> f64 {
    let t = &d["transporte"];
    let v = |k: &str| s(&t[k]);
    bloco(
        c,
        y,
        "TRANSPORTADOR / VOLUMES TRANSPORTADOS",
        &[
            vec![
                cp("Nome / razão social", v("nome"), 90.2),
                cp("Frete por conta", v("frete"), 27.9).centro(),
                cp("Código ANTT", v("antt"), 17.8).centro(),
                cp("Placa do veículo", v("placa"), 22.9).centro(),
                cp("UF", v("uf_veiculo"), 7.6).centro(),
                cp("CNPJ / CPF", v("doc"), LARG - 166.4).centro(),
            ],
            vec![
                cp("Endereço", v("endereco"), 90.2),
                cp("Município", v("municipio"), 68.6),
                cp("UF", v("uf"), 7.6).centro(),
                cp("Inscrição estadual", v("ie"), LARG - 166.4).centro(),
            ],
            vec![
                cp("Quantidade", v("quantidade"), 29.2).direita(),
                cp("Espécie", v("especie"), 30.5),
                cp("Marca", v("marca"), 30.5),
                cp("Numeração", v("numeracao"), 48.3),
                cp("Peso bruto", v("peso_bruto"), 34.3).direita(),
                cp("Peso líquido", v("peso_liquido"), LARG - 172.8).direita(),
            ],
        ],
    )
}

/// Calculo do ISSQN (item 3.3.3: so' quando a nota tem o grupo).
pub(super) fn issqn(c: &mut Caneta, y: f64, g: &Value) -> f64 {
    let w = LARG / 4.0;
    bloco(
        c,
        y,
        "CÁLCULO DO ISSQN",
        &[vec![
            cp("Inscrição municipal", s(&g["im"]), w),
            cp("Valor total dos serviços", s(&g["vServ"]), w).direita(),
            cp("Base de cálculo do ISSQN", s(&g["vBC"]), w).direita(),
            cp("Valor do ISSQN", s(&g["vISS"]), w).direita(),
        ]],
    )
}

/// Dados adicionais da primeira folha: informacoes complementares (corpo de 6,5 pt, item 3.7.8) e o quadro do fisco.
/// `y` e' o topo do titulo; `w_compl` e a largura do quadro das informacoes (o resto e' o reservado ao fisco, menos o
/// quadro do QR Code, se houver).
pub(super) fn dados_adicionais(
    c: &mut Caneta,
    y: f64,
    linhas: &[String],
    w_compl: f64,
    w_fisco: f64,
) {
    c.titulo(X0, y, LARG, "DADOS ADICIONAIS");
    let yt = y + HT;
    c.ret(X0, yt, w_compl, H_COMPL);
    c.celula(
        X0 + 0.6,
        yt + 0.4,
        100.0,
        2.2,
        "INFORMAÇÕES COMPLEMENTARES",
        T_ROTULO,
        false,
        Al::E,
    );
    for (k, l) in linhas.iter().enumerate() {
        c.celula(
            X0 + 0.6,
            yt + 3.0 + k as f64 * LH,
            w_compl - 1.2,
            LH,
            l,
            T_ITEM,
            false,
            Al::E,
        );
    }
    c.ret(X0 + w_compl, yt, w_fisco, H_COMPL);
    c.celula(
        X0 + w_compl + 0.6,
        yt + 0.4,
        60.0,
        2.2,
        "RESERVADO AO FISCO",
        T_ROTULO,
        false,
        Al::E,
    );
}

/// Continuacao das informacoes complementares nas folhas seguintes (item 3.5).
pub(super) fn continuacao(c: &mut Caneta, y: f64, linhas: &[String]) -> f64 {
    c.titulo(X0, y, LARG, "DADOS ADICIONAIS (CONTINUAÇÃO)");
    let yt = y + HT;
    let h = linhas.len() as f64 * LH + 2.0;
    c.ret(X0, yt, LARG, h);
    for (k, l) in linhas.iter().enumerate() {
        c.celula(
            X0 + 0.6,
            yt + 1.0 + k as f64 * LH,
            LARG - 1.2,
            LH,
            l,
            T_ITEM,
            false,
            Al::E,
        );
    }
    HT + h
}

/// Marca d'agua: as linhas, em cinza, no meio da area dos itens (item 3.10.1: nao prejudica a leitura).
pub(super) fn marca(c: &mut Caneta, linhas: &[Value]) {
    let mut y = 150.0;
    for l in linhas {
        let tam = l[1].as_f64().unwrap_or(40.0);
        c.marca(X0, y, LARG, s(&l[0]), tam);
        y += tam * 0.45;
    }
}
