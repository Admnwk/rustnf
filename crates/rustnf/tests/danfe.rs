// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Contrato do DANFE da NF-e: `tests/casos/danfe.json`. O `dados()` tem de bater com a foto guardada (conteudo e
//! plano das folhas; das operacoes de desenho guarda-se o SHA-256 de cada folha), com as conferencias feitas a mao, e
//! o PDF tem de sair com as folhas do plano e sempre com os mesmos bytes. As medidas do Anexo II (margens, codigo de
//! barras, corpos de fonte) sao conferidas nas operacoes de desenho.

use rustnf::danfe::{self, barras, ErroDanfe, Leiaute, Opcoes};
use serde_json::Value;
use sha2::{Digest, Sha256};

fn golden() -> Value {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/casos/danfe.json");
    serde_json::from_str(&std::fs::read_to_string(p).expect("ler o contrato"))
        .expect("contrato e' JSON")
}

fn texto<'a>(v: &'a Value, campo: &str) -> &'a str {
    v[campo]
        .as_str()
        .unwrap_or_else(|| panic!("campo `{campo}` ausente"))
}

fn op<'a>(o: &'a Value, logo: Option<&'a [u8]>) -> Opcoes<'a> {
    Opcoes {
        canhoto: o.get("canhoto").and_then(Value::as_bool).unwrap_or(true),
        logo,
        protocolo_epec: o.get("protocolo_epec").and_then(Value::as_str),
        cancelada: o.get("cancelada").and_then(Value::as_bool).unwrap_or(false),
        leiaute: o
            .get("leiaute")
            .and_then(Value::as_str)
            .map_or(Leiaute::Auto, |s| s.parse().expect("leiaute")),
    }
}

fn caso<'a>(g: &'a Value, nome: &str) -> &'a Value {
    g["casos"]
        .as_array()
        .expect("casos")
        .iter()
        .find(|c| texto(c, "nome") == nome)
        .unwrap_or_else(|| panic!("caso `{nome}` ausente"))
}

fn gerar(c: &Value) -> Value {
    danfe::dados(texto(c, "xml").as_bytes(), &op(&c["opcoes"], None)).expect("dados")
}

fn sha(v: &Value) -> String {
    Sha256::digest(serde_json::to_string(v).expect("json").as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn operacoes(d: &Value, folha: usize) -> &[Value] {
    d["paginas"][folha]["ops"].as_array().expect("ops")
}

fn diferencas(a: &Value, b: &Value, caminho: &str, out: &mut Vec<String>) {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            for k in x.keys().chain(y.keys().filter(|k| !x.contains_key(*k))) {
                diferencas(
                    x.get(k).unwrap_or(&Value::Null),
                    y.get(k).unwrap_or(&Value::Null),
                    &format!("{caminho}.{k}"),
                    out,
                );
            }
        }
        (Value::Array(x), Value::Array(y)) if x.len() == y.len() => {
            for (i, (p, q)) in x.iter().zip(y).enumerate() {
                diferencas(p, q, &format!("{caminho}[{i}]"), out);
            }
        }
        _ if a != b => out.push(format!("{caminho}: obtido {a} != guardado {b}")),
        _ => {}
    }
}

#[test]
fn foto_guardada_campo_a_campo() {
    let g = golden();
    for c in g["casos"].as_array().expect("casos") {
        let nome = texto(c, "nome");
        let mut d = gerar(c);
        let paginas = d["paginas"].take();
        let mut dif = Vec::new();
        diferencas(&d, &c["dados"], nome, &mut dif);
        assert!(
            dif.is_empty(),
            "{} diferencas, as primeiras:\n{}",
            dif.len(),
            dif[..dif.len().min(15)].join("\n")
        );
        let shas: Vec<String> = paginas
            .as_array()
            .expect("paginas")
            .iter()
            .map(|p| sha(&p["ops"]))
            .collect();
        assert_eq!(
            serde_json::to_value(&shas).expect("json"),
            c["ops_sha256"],
            "{nome}: o desenho de alguma folha mudou"
        );
    }
}

#[test]
fn esperado_a_mao() {
    let g = golden();
    for c in g["casos"].as_array().expect("casos") {
        let d = gerar(c);
        for (ponteiro, v) in c["esperado"].as_object().expect("esperado") {
            let obtido = d.pointer(ponteiro).unwrap_or(&Value::Null);
            assert_eq!(obtido, v, "{} {ponteiro}", texto(c, "nome"));
        }
    }
}

#[test]
fn recusas() {
    let g = golden();
    for r in g["recusas"].as_array().expect("recusas") {
        let erro = danfe::dados(texto(r, "xml").as_bytes(), &op(&r["opcoes"], None))
            .expect_err(texto(r, "nome"));
        assert_eq!(erro.to_string(), texto(r, "erro"), "{}", texto(r, "nome"));
    }
    let epec = caso(&g, "epec");
    let e = danfe::dados(texto(epec, "xml").as_bytes(), &Opcoes::default()).unwrap_err();
    assert_eq!(e, ErroDanfe::EpecSemProtocolo);
    let e = danfe::dados(
        texto(epec, "xml").as_bytes(),
        &Opcoes {
            protocolo_epec: Some("123"),
            ..Opcoes::default()
        },
    )
    .unwrap_err();
    assert_eq!(e, ErroDanfe::EpecProtocoloInvalido);
}

#[test]
fn o_leiaute_automatico_segue_a_vigencia_da_nt() {
    let g = golden();
    assert_eq!(gerar(caso(&g, "ibscbs_auto_rtc"))["leiaute"], "rtc");
    assert_eq!(
        gerar(caso(&g, "ibscbs_antes_da_vigencia_moc"))["leiaute"],
        "moc"
    );
    assert_eq!("moc".parse::<Leiaute>(), Ok(Leiaute::Moc));
    assert!("outro".parse::<Leiaute>().is_err());
}

/// Barras de uma operacao: (larguras, modulo em mm, altura em mm, x e largura da caixa nao se sabe: so' o total).
fn barras_da_folha(d: &Value) -> Vec<(String, f64, f64)> {
    operacoes(d, 0)
        .iter()
        .filter(|o| o["t"] == "barras")
        .map(|o| {
            (
                texto(o, "p").to_string(),
                o["m"].as_f64().expect("m") * 10.0,
                o["h"].as_f64().expect("h") * 10.0,
            )
        })
        .collect()
}

#[test]
fn as_barras_decodificam_na_chave_e_cumprem_o_item_2_do_anexo_ii() {
    let g = golden();
    for c in g["casos"].as_array().expect("casos") {
        let nome = texto(c, "nome");
        let d = gerar(c);
        let chave: String = texto(&d, "chave")
            .chars()
            .filter(char::is_ascii_digit)
            .collect();
        let b = barras_da_folha(&d);
        assert_eq!(b[0].0, texto(&d, "chave_barras"));
        assert_eq!(
            barras::decodificar(&b[0].0).expect("decodifica"),
            chave,
            "{nome}"
        );
        for (p, modulo, altura) in &b {
            assert!(
                *modulo >= 0.2,
                "{nome}: modulo de {modulo} mm abaixo de 0,2 mm"
            );
            assert!(
                *altura >= 8.0,
                "{nome}: barras de {altura} mm abaixo de 0,8 cm"
            );
            let largura = barras::modulos(p) as f64 * modulo;
            assert!(
                largura >= 60.0 - 20.0 * modulo,
                "{nome}: codigo de {largura} mm"
            );
        }
        if chave.len() == 44 && d["campo1"]["barras"].is_null() {
            // 22 simbolos de 11 modulos, o Start C, o DV e o Stop (Anexo II, item 2)
            assert_eq!(barras::modulos(&b[0].0), 22 * 11 + 11 + 11 + 13, "{nome}");
        }
    }
}

/// DV do modulo 11 da chave, calculado aqui de forma independente da biblioteca.
fn dv11(base: &str) -> u32 {
    let soma: u32 = base
        .bytes()
        .rev()
        .enumerate()
        .map(|(i, c)| u32::from(c - b'0') * (2 + (i as u32) % 8))
        .sum();
    let r = soma % 11;
    if r < 2 {
        0
    } else {
        11 - r
    }
}

#[test]
fn o_codigo_de_dados_da_contingencia_fs_da_tem_os_campos_do_item_3_9_2() {
    let g = golden();
    let d = gerar(caso(&g, "fs_da"));
    let b = barras_da_folha(&d);
    assert_eq!(b.len(), 2, "chave e dados");
    let dados = barras::decodificar(&b[1].0).expect("decodifica");
    assert_eq!(dados.len(), 36);
    assert_eq!(&dados[0..2], "35", "cUF do destinatario");
    assert_eq!(&dados[2..3], "5", "tpEmis");
    assert_eq!(
        &dados[3..17],
        "00052998224725",
        "CPF do destinatario com zeros a esquerda"
    );
    assert_eq!(&dados[17..31], "00000000003180", "vNF em centavos");
    assert_eq!(&dados[31..32], "2", "sem destaque de ICMS proprio");
    assert_eq!(&dados[32..33], "2", "sem destaque de ICMS ST");
    assert_eq!(&dados[33..35], "03", "dia da emissao");
    assert_eq!(dados[35..].parse::<u32>().expect("dv"), dv11(&dados[..35]));
    let numerico: String = d["campo2"][1]
        .as_str()
        .expect("campo 2")
        .split(' ')
        .collect();
    assert_eq!(numerico, dados, "o campo 2 e' a representacao numerica");
}

#[test]
fn medidas_do_anexo_ii_nas_operacoes() {
    let g = golden();
    for c in g["casos"].as_array().expect("casos") {
        let nome = texto(c, "nome");
        let d = gerar(c);
        let n = d["paginas"].as_array().expect("paginas").len();
        assert_eq!(
            n as u64,
            d["plano"]["folhas"].as_u64().expect("folhas"),
            "{nome}"
        );
        for f in 0..n {
            let (mut min_x, mut max_x, mut max_y) = (f64::MAX, 0.0_f64, 0.0_f64);
            for o in operacoes(&d, f) {
                match texto(o, "t") {
                    "ret" => {
                        min_x = min_x.min(o["x"].as_f64().expect("x"));
                        max_x =
                            max_x.max(o["x"].as_f64().expect("x") + o["w"].as_f64().expect("w"));
                        max_y =
                            max_y.max(o["y"].as_f64().expect("y") + o["h"].as_f64().expect("h"));
                    }
                    "lin" => {
                        min_x = min_x
                            .min(o["x1"].as_f64().expect("x1"))
                            .min(o["x2"].as_f64().expect("x2"));
                        max_x = max_x
                            .max(o["x1"].as_f64().expect("x1"))
                            .max(o["x2"].as_f64().expect("x2"));
                        max_y = max_y
                            .max(o["y1"].as_f64().expect("y1"))
                            .max(o["y2"].as_f64().expect("y2"));
                    }
                    "txt" => assert!(
                        o["s"].as_f64().expect("s") >= 5.0,
                        "{nome}: texto de {} pt (minimo do Anexo II: 5 pt)",
                        o["s"]
                    ),
                    _ => {}
                }
            }
            // item 3.6.2: margens de 0,2 a 0,8 cm nas laterais, em cima e embaixo (A4: 21,0 x 29,7 cm)
            assert!(
                (0.2..=0.8).contains(&min_x),
                "{nome} folha {f}: margem esquerda {min_x} cm"
            );
            assert!(
                (0.2..=0.8).contains(&(21.0 - max_x)),
                "{nome} folha {f}: margem direita {} cm",
                21.0 - max_x
            );
            assert!(
                29.7 - max_y >= 0.2,
                "{nome} folha {f}: margem inferior {} cm",
                29.7 - max_y
            );
        }
    }
}

#[test]
fn corpos_minimos_do_item_3_7_nos_blocos_principais() {
    let g = golden();
    let d = gerar(caso(&g, "simples"));
    let tam = |t: &str| -> Vec<f64> {
        operacoes(&d, 0)
            .iter()
            .filter(|o| o["t"] == "txt" && o["txt"] == t)
            .map(|o| o["s"].as_f64().expect("s"))
            .collect()
    };
    assert!(tam("DANFE")[0] >= 12.0, "3.7.4: DANFE em 12 pt ou mais");
    assert!(
        tam("LOJA DE TESTE LTDA")[0] >= 12.0,
        "3.7.6: razao social em 12 pt ou mais"
    );
    assert!(
        tam("VENDA")[0] >= 10.0,
        "3.7.9: demais campos em 10 pt ou mais"
    );
    assert!(tam("CLIENTE TESTE")[0] >= 10.0);
    assert!(
        tam("NATUREZA DA OPERAÇÃO")[0] >= 6.0,
        "3.7.3: rotulo em 6 pt ou mais"
    );
    assert!(
        tam("Cafe torrado 500g")[0] >= 6.0,
        "3.7.7: item em 6 pt ou mais"
    );
    for o in operacoes(&d, 0).iter().filter(|o| o["t"] == "txt") {
        let t = texto(o, "txt");
        if t.starts_with("3526 ") {
            assert!(o["b"].as_bool().expect("b"), "3.7.5: chave em negrito");
        }
    }
}

// ---------------------------------------------------------------- PDF

fn find(palheiro: &[u8], agulha: &[u8]) -> Option<usize> {
    palheiro.windows(agulha.len()).position(|w| w == agulha)
}

fn paginas_do_pdf(pdf: &[u8]) -> usize {
    pdf.windows(b"/Type/Page".len() + 1)
        .filter(|w| w.starts_with(b"/Type/Page") && w[b"/Type/Page".len()] != b's')
        .count()
}

#[test]
fn pdf_de_verdade_com_as_folhas_do_plano() {
    let g = golden();
    for c in g["casos"].as_array().expect("casos") {
        let nome = texto(c, "nome");
        let d = gerar(c);
        let pdf = danfe::pdf(texto(c, "xml").as_bytes(), &op(&c["opcoes"], None)).expect("pdf");
        assert!(pdf.starts_with(b"%PDF-"), "{nome}");
        assert_eq!(
            paginas_do_pdf(&pdf) as u64,
            d["plano"]["folhas"].as_u64().expect("folhas"),
            "{nome}: folhas do PDF"
        );
        let titulo = format!("DANFE {}", texto(&d, "titulo").trim_start_matches("DANFE "));
        assert!(
            find(&pdf, titulo.as_bytes()).is_some() || find(&pdf, b"/Title").is_some(),
            "{nome}"
        );
        assert!(
            find(&pdf, b"/ToUnicode").is_some(),
            "{nome}: sem texto extraivel"
        );
        for rastro in [&b"\\Users\\"[..], b"/Users/", b"/home/", b"/tmp/"] {
            assert!(find(&pdf, rastro).is_none(), "{nome}: rastro de maquina");
        }
    }
}

#[test]
fn mesma_entrada_gera_os_mesmos_bytes() {
    let g = golden();
    for nome in ["varias_folhas", "ibscbs_forcado_rtc", "fs_da"] {
        let c = caso(&g, nome);
        let a = danfe::pdf(texto(c, "xml").as_bytes(), &op(&c["opcoes"], None)).expect("pdf");
        let b = danfe::pdf(texto(c, "xml").as_bytes(), &op(&c["opcoes"], None)).expect("pdf");
        assert_eq!(a, b, "{nome}: o PDF nao e' deterministico");
    }
}

#[test]
fn a_fonte_e_a_liberation_serif_em_subconjunto() {
    let g = golden();
    let c = caso(&g, "simples");
    let pdf = danfe::pdf(texto(c, "xml").as_bytes(), &Opcoes::default()).expect("pdf");
    let nomes = regex::bytes::Regex::new(r"/BaseFont/([A-Z]{6})\+(LiberationSerif(?:-Bold)?)/")
        .expect("padrao valido");
    let mut achados: Vec<String> = nomes
        .captures_iter(&pdf)
        .map(|c| String::from_utf8_lossy(&c[2]).into_owned())
        .collect();
    achados.sort();
    achados.dedup();
    assert_eq!(achados, ["LiberationSerif", "LiberationSerif-Bold"]);
    assert!(find(&pdf, b"LiberationSans").is_none());
}

#[test]
fn logotipo_do_emitente_opcional() {
    let g = golden();
    let c = caso(&g, "simples");
    let png = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("dados/danfse_logo_nfse.png"),
    )
    .expect("logo");
    let sem = danfe::pdf(texto(c, "xml").as_bytes(), &Opcoes::default()).expect("pdf");
    let com = danfe::pdf(
        texto(c, "xml").as_bytes(),
        &Opcoes {
            logo: Some(&png),
            ..Opcoes::default()
        },
    )
    .expect("pdf");
    assert!(find(&sem, b"/Subtype/Image").is_none());
    assert!(find(&com, b"/Subtype/Image").is_some());
    let d = danfe::dados(
        texto(c, "xml").as_bytes(),
        &Opcoes {
            logo: Some(&png),
            ..Opcoes::default()
        },
    )
    .expect("dados");
    assert!(operacoes(&d, 0).iter().any(|o| o["t"] == "logo"));
}

#[test]
fn sem_canhoto_o_quadro_dos_itens_cresce() {
    let g = golden();
    let c = caso(&g, "simples");
    let com = danfe::dados(texto(c, "xml").as_bytes(), &Opcoes::default()).expect("dados");
    let sem = danfe::dados(
        texto(c, "xml").as_bytes(),
        &Opcoes {
            canhoto: false,
            ..Opcoes::default()
        },
    )
    .expect("dados");
    let topo = |d: &Value| d["plano"]["topo_itens"].as_f64().expect("topo");
    assert!(
        (topo(&com) - topo(&sem) - 21.2).abs() < 1e-6,
        "{} {}",
        topo(&com),
        topo(&sem)
    );
}

#[test]
fn nota_cancelada_por_evento_ganha_a_marca() {
    let g = golden();
    let c = caso(&g, "simples");
    let d = danfe::dados(
        texto(c, "xml").as_bytes(),
        &Opcoes {
            cancelada: true,
            ..Opcoes::default()
        },
    )
    .expect("dados");
    assert_eq!(d["marca"][0][0], "NOTA CANCELADA");
}

#[test]
fn issqn_so_aparece_quando_a_nota_tem_o_grupo() {
    let g = golden();
    let c = caso(&g, "simples");
    assert!(gerar(c)["issqn"].is_null());
    // a montagem nao produz ISSQNtot: o grupo entra a mao no XML
    let xml = texto(c, "xml").replace(
        "</ICMSTot>",
        "</ICMSTot><ISSQNtot><vServ>31.80</vServ><vBC>31.80</vBC><vISS>1.59</vISS></ISSQNtot>",
    );
    let d = danfe::dados(xml.as_bytes(), &Opcoes::default()).expect("dados");
    assert_eq!(d["issqn"]["vISS"], "1,59");
    assert_eq!(d["issqn"]["vServ"], "31,80");
    let pdf = danfe::pdf_de(&d, &Opcoes::default()).expect("pdf");
    assert!(pdf.starts_with(b"%PDF-"));
}
