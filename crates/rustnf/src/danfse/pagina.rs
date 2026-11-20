// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Pagina: as operacoes de desenho (retangulo, linha, texto, imagem, QR, marca d'agua) em cm, na ordem em que o
//! modelo Typst as executa.

use serde_json::{json, Value};

use super::formato::{municipio_uf, traco};
use super::medidas::*;
use super::metrica::{caber, largura, limitar, quebrar};

struct Pagina {
    ops: Vec<Value>,
}

impl Pagina {
    fn ret(&mut self, x: f64, y: f64, w: f64, h: f64, cinza: bool, borda: f64) {
        self.ops.push(json!({"t": "ret", "x": r4(x), "y": r4(y), "w": r4(w), "h": r4(h), "cinza": cinza, "borda": borda}));
    }

    fn lin(&mut self, x1: f64, y1: f64, x2: f64, y2: f64) {
        self.ops.push(
            json!({"t": "lin", "x1": r4(x1), "y1": r4(y1), "x2": r4(x2), "y2": r4(y2), "esp": 0.5}),
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn txt(&mut self, x: f64, topo: f64, t: &str, tam: f64, b: bool, cor: &str, w: Option<f64>) {
        if t.trim().is_empty() {
            return;
        }
        let y = topo + tam * ASCENDENTE / CM;
        let x = match w {
            Some(w) => x + (w - largura(t, tam, b) / CM) / 2.0,
            None => x,
        };
        self.ops.push(
            json!({"t": "txt", "x": r4(x), "y": r4(y), "s": tam, "b": b, "cor": cor, "txt": t}),
        );
    }

    fn t(&mut self, x: f64, topo: f64, t: &str, tam: f64) {
        self.txt(x, topo, t, tam, false, "preto", None);
    }

    #[allow(clippy::too_many_arguments)]
    fn celula(
        &mut self,
        x: f64,
        topo: f64,
        w: f64,
        rot: &str,
        val: &str,
        t_rot: f64,
        d_rot: f64,
        d_val: f64,
        sombra: Option<(f64, f64)>,
    ) {
        let lg = w * CM - 2.0 * RECUO * CM;
        if let Some((dy, h)) = sombra {
            self.ret(x, topo + dy, w, h, true, 0.0);
        }
        if !rot.is_empty() {
            let r = caber(rot, t_rot, lg, true);
            self.txt(x + RECUO, topo + d_rot, &r, t_rot, true, "preto", None);
        }
        let v = caber(&traco(val), T_VAL, lg, false);
        self.t(x + RECUO, topo + d_val, &v, T_VAL);
    }

    fn campo(&mut self, col: usize, topo: f64, qtd: usize, rot: &str, val: &str, sombra: bool) {
        self.celula(
            COLS[col],
            topo,
            larg(col, qtd),
            rot,
            val,
            T_ROT,
            0.0,
            0.30,
            if sombra {
                Some((0.015, 0.64 - 0.015))
            } else {
                None
            },
        );
    }

    fn secao(&mut self, topo: f64, titulo: &str, h: f64, sombra: bool) {
        if sombra {
            self.ret(X0, topo, X2 - X0, h, true, 0.0);
        }
        self.txt(
            X0 + RECUO,
            topo + 0.07,
            titulo,
            T_BLOCO,
            true,
            "preto",
            None,
        );
        self.lin(X0, topo, XF, topo);
    }

    fn suprimido(&mut self, topo: f64, msg: &str, h: f64) {
        self.lin(X0, topo, XF, topo);
        self.lin(X0, topo + h, XF, topo + h);
        let w = (XF - X0) * CM - 2.0;
        let t = caber(msg, 7.5, w, false);
        self.txt(
            X0 + 1.0 / CM,
            topo + (h - 7.5 / CM) / 2.0,
            &t,
            7.5,
            false,
            "preto",
            Some(w / CM),
        );
    }
}

fn parte_pagina(p: &mut Pagina, topo: f64, pt: &Value, variante: &str) {
    p.secao(topo, s(&pt["titulo"]), 0.63, true);
    p.campo(
        1,
        topo,
        if variante == "dest" { 2 } else { 1 },
        "CNPJ / CPF / NIF",
        s(&pt["documento"]),
        false,
    );
    if variante != "dest" {
        p.campo(
            2,
            topo,
            1,
            "Indicador Municipal (Inscrição)",
            s(&pt["im"]),
            false,
        );
    }
    p.campo(3, topo, 1, "Telefone", s(&pt["telefone"]), false);
    p.campo(
        0,
        topo + 0.64,
        2,
        "Nome / Nome Empresarial",
        s(&pt["nome"]),
        false,
    );
    p.campo(
        2,
        topo + 0.64,
        1,
        "Município / Sigla UF",
        s(&pt["municipio_uf"]),
        false,
    );
    p.campo(
        3,
        topo + 0.64,
        1,
        "Código IBGE / CEP",
        s(&pt["ibge_cep"]),
        false,
    );
    p.campo(0, topo + 1.28, 2, "Endereço", s(&pt["endereco"]), false);
    p.campo(2, topo + 1.28, 2, "E-mail", s(&pt["email"]), false);
    if variante == "prest" {
        p.campo(
            0,
            topo + 1.92,
            2,
            "Simples Nacional na Data de Competência",
            s(&pt["simples_nacional"]),
            false,
        );
        p.campo(
            2,
            topo + 1.92,
            2,
            "Regime de Apuração Tributária pelo SN",
            s(&pt["regime_sn"]),
            false,
        );
    }
}

pub(super) fn operacoes(c: &Value, l: &Value, canhoto: bool) -> Vec<Value> {
    let mut p = Pagina { ops: Vec::new() };
    let (cab, ac) = (&c["cabecalho"], &c["acesso"]);
    p.ret(X0, 0.30, 20.40, 1.16, true, 0.0);
    p.ops
        .push(json!({"t": "img", "nome": "logo", "x": 0.49, "y": 0.44, "w": 4.00, "h": 0.85}));
    p.txt(X2, 0.35, "DANFSe v2.0", 9.0, true, "preto", Some(10.19));
    p.txt(
        X2,
        0.65,
        "Documento Auxiliar da NFS-e",
        9.0,
        true,
        "preto",
        Some(10.19),
    );
    if cab["sem_validade"].as_bool().unwrap_or(false) {
        p.txt(
            X2,
            0.95,
            "NFS-e SEM VALIDADE JURÍDICA",
            9.0,
            true,
            "vermelho",
            Some(10.19),
        );
    }
    if !s(&cab["municipio"]).is_empty() || !s(&cab["uf"]).is_empty() {
        let m = format!(
            "Município: {}",
            municipio_uf(s(&cab["municipio"]), s(&cab["uf"]))
        );
        let m = caber(&m, 8.0, (XF - X4) * CM, false);
        p.t(X4, 0.45, &m, 8.0);
    }
    p.t(
        X4,
        0.97,
        &format!("Ambiente Gerador: {}", s(&cab["ambiente_gerador"])),
        6.0,
    );
    p.t(
        X4,
        1.22,
        &format!("Tipo de Ambiente: {}", s(&cab["tipo_ambiente"])),
        6.0,
    );
    p.lin(X0, 1.48, XF, 1.48);
    p.celula(
        X0,
        1.48,
        15.30,
        "CHAVE DE ACESSO DA NFS-E",
        s(&ac["chave"]),
        7.0,
        0.0,
        0.30,
        None,
    );
    p.ops
        .push(json!({"t": "qr", "x": 17.39, "y": 1.58, "lado": 1.90}));
    let q0 = 17.39 + (1.90 - 4.72) / 2.0;
    for (i, t) in [
        "A autenticidade desta NFS-e pode ser verificada",
        "pela leitura deste código QR ou pela consulta da",
        "chave de acesso no portal nacional da NFS-e",
    ]
    .iter()
    .enumerate()
    {
        p.t(q0, 3.53 + i as f64 * 0.21, t, 6.0);
    }
    for (col, topo, rot, k) in [
        (0, 2.27, "NÚMERO DA NFS-E", "numero"),
        (1, 2.27, "COMPETÊNCIA DA NFS-E", "competencia"),
        (2, 2.27, "DATA E HORA DA EMISSÃO DA NFS-E", "emissao_nfse"),
        (0, 2.96, "NÚMERO DA DPS", "numero_dps"),
        (1, 2.96, "SÉRIE DA DPS", "serie_dps"),
        (2, 2.96, "DATA E HORA DA EMISSÃO DA DPS", "emissao_dps"),
    ] {
        p.celula(
            COLS[col],
            topo,
            larg(col, 1),
            rot,
            s(&ac[k]),
            7.0,
            0.0,
            0.30,
            None,
        );
    }
    for (col, rot, k, sombra) in [
        (0, "EMITENTE DA NFS-e", "emitente", true),
        (1, "SITUAÇÃO DA NFS-e", "situacao", false),
        (2, "FINALIDADE", "finalidade", false),
    ] {
        p.celula(
            COLS[col],
            3.65,
            larg(col, 1),
            rot,
            s(&ac[k]),
            7.0,
            0.06,
            0.36,
            if sombra { Some((0.015, 0.675)) } else { None },
        );
    }
    let pt = &c["partes"];
    parte_pagina(&mut p, 4.34, &pt["prestador"], "prest");
    for (k, var) in [
        ("tomador", "toma"),
        ("destinatario", "dest"),
        ("intermediario", "interm"),
    ] {
        if pt[k]["visivel"].as_bool().unwrap_or(false) {
            parte_pagina(&mut p, f(&l[k]), &pt[k], var);
        } else {
            p.suprimido(f(&l[k]), s(&pt[k]["mensagem"]), 0.40);
        }
    }
    let (sv, y) = (&c["servico"], f(&l["servico"]));
    p.secao(y, "SERVIÇO PRESTADO", 0.63, true);
    p.campo(
        1,
        y,
        1,
        "Código de Tributação Nacional / Municipal",
        s(&sv["codigo_trib"]),
        false,
    );
    p.campo(2, y, 1, "Código da NBS", s(&sv["nbs"]), false);
    p.campo(
        3,
        y,
        1,
        "Local da Prestação / Sigla UF / País",
        s(&sv["local_prestacao"]),
        false,
    );
    let dt = caber(
        s(&sv["descricao_trib"]),
        T_VAL,
        (XF - X0) * CM - 0.15 * CM,
        false,
    );
    p.t(X0 + RECUO, y + 0.65, &dt, T_VAL);
    p.txt(
        X0 + RECUO,
        y + 1.05,
        "Descrição do Serviço",
        T_ROT,
        true,
        "preto",
        None,
    );
    for (i, t) in l["descricao"].as_array().into_iter().flatten().enumerate() {
        p.t(X0 + RECUO, y + 1.35 + i as f64 * passo_linha(), s(t), T_VAL);
    }
    let d = f(&l["desloc"]);
    let (mu, y) = (&c["municipal"], f(&l["municipal"]) + d);
    if !mu["visivel"].as_bool().unwrap_or(false) {
        p.suprimido(y, s(&mu["mensagem"]), 0.32);
    } else {
        p.secao(y, "TRIBUTAÇÃO MUNICIPAL (ISSQN)", 0.63, true);
        p.campo(
            1,
            y,
            1,
            "Tipo de Tributação do ISSQN",
            s(&mu["trib_issqn"]),
            false,
        );
        p.campo(
            2,
            y,
            2,
            "Município / Sigla UF / País de Incidência do ISSQN",
            s(&mu["local_incidencia"]),
            false,
        );
        let linhas: [(f64, [(&str, &str); 4]); 3] = [
            (
                1.0,
                [
                    ("Regime Especial de Tributação do ISSQN", "reg_esp"),
                    ("Tipo de Imunidade do ISSQN", "imunidade"),
                    ("Suspensão da Exigibilidade do ISSQN", "suspensao"),
                    ("Número Processo Suspensão", "processo"),
                ],
            ),
            (
                2.0,
                [
                    ("Benefício Municipal", "beneficio"),
                    ("Cálculo do BM", "calculo_bm"),
                    ("Total Deduções/Reduções", "deducoes"),
                    ("Desconto Incondicionado", "desconto_incond"),
                ],
            ),
            (
                3.0,
                [
                    ("BC ISSQN", "bc"),
                    ("Alíquota Aplicada", "aliquota"),
                    ("Retenção do ISSQN", "retencao"),
                    ("ISSQN Apurado", "issqn"),
                ],
            ),
        ];
        for (n, campos) in linhas {
            for (col, (rot, k)) in campos.iter().enumerate() {
                p.campo(col, y + n * 0.65, 1, rot, s(&mu[*k]), false);
            }
        }
    }
    let (fe, y) = (&c["federal"], f(&l["federal"]) + d);
    p.secao(y, "TRIBUTAÇÃO FEDERAL (EXCETO CBS)", 0.63, true);
    p.campo(1, y, 1, "IRRF", s(&fe["irrf"]), false);
    p.campo(
        2,
        y,
        1,
        "Contribuição Previdenciária - Retida",
        s(&fe["cp"]),
        false,
    );
    p.campo(
        3,
        y,
        1,
        "Contribuições Sociais - Retidas",
        s(&fe["csll_ret"]),
        false,
    );
    if fe["exibir_pis_cofins"].as_bool().unwrap_or(false) {
        p.campo(
            0,
            y + 0.65,
            1,
            "PIS - Débito Apuração Própria",
            s(&fe["pis"]),
            false,
        );
        p.campo(
            1,
            y + 0.65,
            1,
            "COFINS - Débito Apuração Própria",
            s(&fe["cofins"]),
            false,
        );
        p.campo(
            2,
            y + 0.65,
            2,
            "Descrição Contrib. Sociais - Retidas",
            s(&fe["desc_ret"]),
            false,
        );
    }
    let (ib, y) = (&c["ibscbs"], f(&l["ibscbs"]) + d);
    p.secao(y, "TRIBUTAÇÃO IBS / CBS", 0.63, true);
    p.campo(1, y, 1, "CST / cClassTrib", s(&ib["cst_cclasstrib"]), false);
    p.campo(
        2,
        y,
        2,
        "Indicador de Operação / Código IBGE Incidência / Município Incidência / Sigla UF",
        s(&ib["indop_local"]),
        false,
    );
    let linhas: [(f64, [(&str, &str); 4]); 3] = [
        (
            0.64,
            [
                ("Exclusões e Reduções da Base de Cálculo", "exclusoes"),
                ("Base de Cálculo Após Exclusões e Reduções", "bc"),
                ("Red. Alíquota IBS / Red. Alíquota CBS", "red_aliq"),
                ("Alíquota IBS UF / IBS Mun", "aliq_uf_mun"),
            ],
        ),
        (
            1.29,
            [
                ("Alíq. Efetiva Municipal - IBS", "aliq_efet_mun"),
                ("Valor Apurado Municipal - IBS", "valor_mun"),
                ("Alíq. Efetiva Estadual - IBS", "aliq_efet_uf"),
                ("Valor Apurado Estadual - IBS", "valor_uf"),
            ],
        ),
        (
            1.94,
            [
                ("Valor Total Apurado - IBS", "valor_ibs"),
                ("Alíquota - CBS", "aliq_cbs"),
                ("Alíquota Efetiva - CBS", "aliq_efet_cbs"),
                ("Valor Total Apurado - CBS", "valor_cbs"),
            ],
        ),
    ];
    for (dy, campos) in linhas {
        for (col, (rot, k)) in campos.iter().enumerate() {
            p.campo(col, y + dy, 1, rot, s(&ib[*k]), false);
        }
    }
    let (to, y) = (&c["totais"], f(&l["totais"]) + d);
    p.secao(y, "VALOR TOTAL DA NFS-E", 0.67, true);
    p.campo(
        1,
        y,
        1,
        "VALOR DA OPERAÇÃO / SERVIÇO",
        s(&to["valor_servico"]),
        false,
    );
    p.campo(
        2,
        y,
        1,
        "Desconto Incondicionado",
        s(&to["desconto_incond"]),
        false,
    );
    p.campo(
        3,
        y,
        1,
        "Desconto Condicionado",
        s(&to["desconto_cond"]),
        false,
    );
    p.campo(
        0,
        y + 0.69,
        1,
        "Total das Retenções (ISSQN / Federais)",
        s(&to["retencoes"]),
        false,
    );
    p.campo(
        1,
        y + 0.69,
        1,
        "VALOR LÍQUIDO DA NFS-e",
        s(&to["liquido"]),
        false,
    );
    p.campo(2, y + 0.69, 1, "Total do IBS/CBS", s(&to["ibscbs"]), false);
    p.campo(
        3,
        y + 0.69,
        1,
        "VALOR LÍQUIDO DA NFS-e + IBS/CBS",
        s(&to["liquido_ibscbs"]),
        true,
    );
    let y = f(&l["complementares"]) + d;
    p.secao(y, "INFORMAÇÕES COMPLEMENTARES", 0.39, false);
    for (i, t) in l["complementares_linhas"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
    {
        p.t(
            X0 + RECUO,
            y + 0.41 + i as f64 * passo_compl(),
            s(t),
            T_COMPL,
        );
    }
    if canhoto {
        p.lin(X0, Y_CANHOTO, XF, Y_CANHOTO);
        p.lin(X0, Y_CANHOTO_FIM, XF, Y_CANHOTO_FIM);
        for x in [X0, X2, X3, XF] {
            p.lin(x, Y_CANHOTO, x, Y_CANHOTO_FIM);
        }
        p.txt(
            X0 + 2.0 / CM,
            28.17,
            "DATA CIENTIFICAÇÃO:",
            6.0,
            true,
            "preto",
            None,
        );
        p.txt(
            X2 + 2.0 / CM,
            28.17,
            "IDENTIFICAÇÃO E ASSINATURA",
            6.0,
            true,
            "preto",
            None,
        );
        p.txt(
            X3 + 2.0 / CM,
            28.17,
            "Nº NFS-e / CHAVE NFS-e",
            6.0,
            true,
            "preto",
            None,
        );
        let w = (XF - X3) * CM;
        let linhas = limitar(
            quebrar(
                &format!("{} / {}", s(&ac["numero"]), s(&ac["chave"])),
                6.5,
                w,
            ),
            2,
            6.5,
            w,
        );
        for (i, t) in linhas.iter().enumerate() {
            p.t(X3 + 2.0 / CM, 28.47 + i as f64 * 6.5 * 1.2 / CM, t, 6.5);
        }
    }
    p.ret(0.15, 0.15, 20.70, 29.40, false, 1.0);
    if !s(&c["marca_dagua"]).is_empty() {
        p.ops.push(json!({"t": "marca", "txt": s(&c["marca_dagua"]), "s": 60.0, "ang": 45.0, "cinza": 0.35, "opacidade": 0.35}));
    }
    p.ops
}
