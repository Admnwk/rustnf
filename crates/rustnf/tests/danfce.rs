// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! DANFC-e: o conteudo por divisao do Manual (`dados`), as recusas e o PDF de bobina. Os casos estao em
//! `tests/casos/danfce.json` (XML de NFC-e que a montagem e a assinatura do `rustnf` produzem, protocolo ficticio).
//! O ESC/POS esta em `tests/danfce_escpos.rs`.

use std::io::Read;

use flate2::read::ZlibDecoder;
use rustnf::danfce::{self, ErroDanfce, Largura, Opcoes};
use serde_json::{to_value, Value};

fn casos() -> Value {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/casos/danfce.json");
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn opcoes(o: &Value) -> Opcoes {
    let mut op = Opcoes::default();
    if let Some(l) = o.get("largura") {
        op.largura = if l == "mm58" {
            Largura::Mm58
        } else {
            Largura::Mm80
        };
    }
    let b = |k: &str| o.get(k).and_then(Value::as_bool);
    op.resumido = b("resumido").unwrap_or(op.resumido);
    op.cancelada = b("cancelada").unwrap_or(op.cancelada);
    op.via_estabelecimento = b("via_estabelecimento").unwrap_or(op.via_estabelecimento);
    op.consumidor_com_nome_e_endereco =
        b("consumidor_com_nome_e_endereco").unwrap_or(op.consumidor_com_nome_e_endereco);
    op.tributos = b("tributos").unwrap_or(op.tributos);
    op.colunas = o.get("colunas").and_then(Value::as_u64).map(|c| c as u8);
    op
}

fn xml(c: &Value) -> &[u8] {
    c["xml"].as_str().unwrap().as_bytes()
}

// ---------------------------------------------------------------- conteudo

#[test]
fn o_conteudo_de_cada_caso_confere_com_o_manual() {
    let c = casos();
    let todos = c["casos"].as_array().unwrap();
    assert_eq!(todos.len(), 12);
    for x in todos {
        let nome = x["nome"].as_str().unwrap();
        let d =
            danfce::dados(xml(x), &opcoes(&x["opcoes"])).unwrap_or_else(|e| panic!("{nome}: {e}"));
        let json = to_value(&d).unwrap();
        for (ponteiro, esperado) in x["dados"].as_object().unwrap() {
            let achado = json.pointer(ponteiro).unwrap_or(&Value::Null);
            assert_eq!(achado, esperado, "{nome}: {ponteiro}");
        }
        // mesma entrada, mesmo conteudo e mesmo JSON
        let outra = danfce::dados(xml(x), &opcoes(&x["opcoes"])).unwrap();
        assert_eq!(d, outra, "{nome}: determinismo");
        assert_eq!(to_value(&outra).unwrap(), json, "{nome}: json estavel");
    }
}

#[test]
fn as_recusas_dizem_o_motivo_em_palavras() {
    let c = casos();
    let todas = c["recusas"].as_array().unwrap();
    assert_eq!(todas.len(), 10);
    for x in todas {
        let nome = x["nome"].as_str().unwrap();
        let e = danfce::dados(xml(x), &opcoes(&x["opcoes"])).unwrap_err();
        assert_eq!(e.to_string(), x["erro"].as_str().unwrap(), "{nome}");
        // PDF e ESC/POS recusam do mesmo jeito
        assert_eq!(
            danfce::pdf(xml(x), &opcoes(&x["opcoes"])).unwrap_err(),
            e,
            "{nome}: pdf"
        );
        assert_eq!(
            danfce::escpos(xml(x), &opcoes(&x["opcoes"])).unwrap_err(),
            e,
            "{nome}: escpos"
        );
    }
}

#[test]
fn a_recusa_e_tipada() {
    let c = casos();
    let por_nome = |n: &str| {
        let x = c["recusas"]
            .as_array()
            .unwrap()
            .iter()
            .find(|x| x["nome"] == n)
            .unwrap();
        danfce::dados(xml(x), &opcoes(&x["opcoes"])).unwrap_err()
    };
    assert_eq!(
        por_nome("nota_sem_protocolo_fora_da_contingencia"),
        ErroDanfce::SemProtocolo
    );
    assert_eq!(
        por_nome("rejeitada_539"),
        ErroDanfce::Rejeitada("539".into())
    );
    assert_eq!(por_nome("denegada_110"), ErroDanfce::Denegada);
    assert_eq!(
        por_nome("nfe_modelo_55_continua_recusada"),
        ErroDanfce::NaoENfce("55".into())
    );
    assert_eq!(por_nome("sem_qr_code"), ErroDanfce::SemQr);
    assert_eq!(
        por_nome("via_do_estabelecimento_fora_da_contingencia"),
        ErroDanfce::ViaSemContingencia
    );
}

#[test]
fn o_cupom_so_mostra_valor_a_pagar_com_acrescimo_ou_desconto() {
    let c = casos();
    for x in c["casos"].as_array().unwrap() {
        let d = danfce::dados(xml(x), &opcoes(&x["opcoes"])).unwrap();
        let ajustes = d.totais.acrescimos.is_some() || d.totais.desconto.is_some();
        assert_eq!(d.totais.valor_a_pagar.is_some(), ajustes, "{}", x["nome"]);
    }
}

// ---------------------------------------------------------------- PDF

fn find(palheiro: &[u8], agulha: &[u8]) -> Option<usize> {
    palheiro.windows(agulha.len()).position(|w| w == agulha)
}

fn fluxos(pdf: &[u8]) -> Vec<Vec<u8>> {
    let mut saida = Vec::new();
    let mut i = 0;
    while let Some(p) = find(&pdf[i..], b"stream") {
        let ini = i + p + b"stream".len();
        let ini = if pdf[ini..].starts_with(b"\r\n") {
            ini + 2
        } else if pdf[ini..].starts_with(b"\n") {
            ini + 1
        } else {
            i = ini;
            continue;
        };
        let Some(fim) = find(&pdf[ini..], b"endstream") else {
            break;
        };
        let bruto = &pdf[ini..ini + fim];
        let mut aberto = Vec::new();
        if ZlibDecoder::new(bruto).read_to_end(&mut aberto).is_ok() {
            saida.push(aberto);
        } else {
            saida.push(bruto.to_vec());
        }
        i = ini + fim + b"endstream".len();
    }
    saida
}

fn contem(pdf: &[u8], agulha: &[u8]) -> bool {
    find(pdf, agulha).is_some() || fluxos(pdf).iter().any(|f| find(f, agulha).is_some())
}

fn objeto(pdf: &[u8], n: u32) -> &[u8] {
    let marca = format!("\n{n} 0 obj\n");
    let ini = find(pdf, marca.as_bytes()).unwrap_or_else(|| panic!("objeto {n} ausente"));
    let corpo = &pdf[ini + marca.len()..];
    &corpo[..find(corpo, b"endobj").expect("fim do objeto")]
}

fn fluxo_do_objeto(pdf: &[u8], n: u32) -> Vec<u8> {
    let corpo = objeto(pdf, n);
    let ini = find(corpo, b"stream\n").expect("fluxo") + b"stream\n".len();
    let fim = find(corpo, b"endstream").expect("fim do fluxo");
    let mut aberto = Vec::new();
    ZlibDecoder::new(&corpo[ini..fim])
        .read_to_end(&mut aberto)
        .expect("fluxo zlib");
    aberto
}

fn mapa_tounicode(cmap: &str) -> std::collections::HashMap<u16, char> {
    let par = regex::Regex::new(r"<([0-9A-Fa-f]{4})>\s*<([0-9A-Fa-f]{4})>").unwrap();
    par.captures_iter(cmap)
        .filter_map(|c| {
            let cod = u16::from_str_radix(&c[1], 16).ok()?;
            let car = char::from_u32(u32::from_str_radix(&c[2], 16).ok()?)?;
            Some((cod, car))
        })
        .collect()
}

fn cadeia_literal(bytes: &[u8]) -> Vec<u8> {
    let mut saida = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'\\' {
            saida.push(bytes[i]);
            i += 1;
        } else if bytes[i + 1].is_ascii_digit() {
            i += 1;
            let mut v = 0u32;
            for _ in 0..3 {
                if i < bytes.len() && bytes[i].is_ascii_digit() {
                    v = v * 8 + u32::from(bytes[i] - b'0');
                    i += 1;
                }
            }
            saida.push(v as u8);
        } else {
            saida.push(match bytes[i + 1] {
                b'n' => b'\n',
                b'r' => b'\r',
                b't' => b'\t',
                b'b' => 8,
                b'f' => 12,
                outro => outro,
            });
            i += 2;
        }
    }
    saida
}

/// Todo o texto desenhado na pagina, um trecho por operacao, lido pelos mapas ToUnicode de cada fonte.
fn texto_do_pdf(pdf: &[u8]) -> Vec<String> {
    let numero = |t: &str| t.parse::<u32>().expect("numero de objeto");
    let recursos = regex::Regex::new(r"/(f\d+) (\d+) 0 R").unwrap();
    let ini = find(pdf, b"/Font<<").expect("recursos de fonte");
    let bloco = String::from_utf8_lossy(&pdf[ini..ini + 80]).into_owned();
    let to_unicode = regex::Regex::new(r"/ToUnicode (\d+) 0 R").unwrap();
    let mapas: std::collections::HashMap<String, std::collections::HashMap<u16, char>> = recursos
        .captures_iter(&bloco)
        .map(|c| {
            let fonte = String::from_utf8_lossy(objeto(pdf, numero(&c[2]))).into_owned();
            let cmap = fluxo_do_objeto(
                pdf,
                numero(&to_unicode.captures(&fonte).expect("ToUnicode")[1]),
            );
            (
                c[1].to_string(),
                mapa_tounicode(&String::from_utf8_lossy(&cmap)),
            )
        })
        .collect();
    let conteudo = fluxos(pdf)
        .into_iter()
        .find(|f| find(f, b" Tf ").is_some())
        .expect("conteudo da pagina");
    let trecho = regex::bytes::Regex::new(r"(?s)/(f\d+) [\d.]+ Tf [^\[]*\[\((.*?)\)\]TJ").unwrap();
    trecho
        .captures_iter(&conteudo)
        .map(|c| {
            let mapa = &mapas[std::str::from_utf8(&c[1]).expect("nome da fonte")];
            cadeia_literal(&c[2])
                .chunks(2)
                .map(|p| mapa[&u16::from_be_bytes([p[0], p[1]])])
                .collect()
        })
        .collect()
}

/// (largura, altura) do `/MediaBox`, em mm.
fn tamanho_mm(pdf: &[u8]) -> (f64, f64) {
    let r = regex::bytes::Regex::new(r"/MediaBox\s*\[\s*0\s+0\s+([\d.]+)\s+([\d.]+)\s*\]").unwrap();
    let c = r.captures(pdf).expect("MediaBox");
    let n = |i: usize| std::str::from_utf8(&c[i]).unwrap().parse::<f64>().unwrap() / 72.0 * 25.4;
    (n(1), n(2))
}

#[test]
fn o_pdf_e_de_bobina_com_uma_pagina_e_os_mesmos_bytes_sempre() {
    let c = casos();
    for x in c["casos"].as_array().unwrap() {
        let nome = x["nome"].as_str().unwrap();
        let op = opcoes(&x["opcoes"]);
        let pdf = danfce::pdf(xml(x), &op).unwrap_or_else(|e| panic!("{nome}: {e}"));
        assert!(pdf.starts_with(b"%PDF-"), "{nome}");
        assert_eq!(
            pdf,
            danfce::pdf(xml(x), &op).unwrap(),
            "{nome}: bytes estaveis"
        );
        assert!(contem(&pdf, b"/Count 1"), "{nome}: uma pagina");
        let (w, h) = tamanho_mm(&pdf);
        let esperado = if op.largura == Largura::Mm58 {
            58.0
        } else {
            80.0
        };
        assert!((w - esperado).abs() < 0.05, "{nome}: largura {w}");
        // a altura vem do conteudo: cupom com itens e QR tem de passar de 8 cm e nao de 40
        assert!((80.0..400.0).contains(&h), "{nome}: altura {h}");
        for rastro in [&b"\\Users\\"[..], b"/Users/", b"/home/", b"/tmp/"] {
            assert!(!contem(&pdf, rastro), "{nome}: rastro de maquina");
        }
        assert!(contem(&pdf, b"DANFC-e 000000100"), "{nome}: titulo");
    }
}

#[test]
fn a_altura_do_pdf_acompanha_o_conteudo() {
    let c = casos();
    let por_nome = |n: &str| {
        let x = c["casos"]
            .as_array()
            .unwrap()
            .iter()
            .find(|x| x["nome"] == n)
            .unwrap();
        tamanho_mm(&danfce::pdf(xml(x), &opcoes(&x["opcoes"])).unwrap()).1
    };
    // o resumido (sem a divisao II) e' mais curto que o completo da mesma nota
    assert!(
        por_nome("resumido_sem_a_divisao_ii") < por_nome("homologacao_nao_identificado_com_troco")
    );
}

#[test]
fn o_texto_do_pdf_e_extraivel() {
    let c = casos();
    for x in c["casos"].as_array().unwrap() {
        let nome = x["nome"].as_str().unwrap();
        let pdf = danfce::pdf(xml(x), &opcoes(&x["opcoes"])).unwrap();
        let linhas = texto_do_pdf(&pdf);
        for esperado in x["pdf_texto"].as_array().unwrap() {
            let e = esperado.as_str().unwrap();
            // o texto que quebra em varias linhas do cupom sai em varios trechos: confere por trecho ou por palavras
            let achou = linhas.iter().any(|l| l == e)
                || e.split(' ').all(|p| linhas.iter().any(|l| l.contains(p)));
            assert!(
                achou,
                "{nome}: texto `{e}` nao extraivel; ha {} trechos: {linhas:?}",
                linhas.len()
            );
        }
    }
}

#[test]
fn o_pdf_de_58_mm_cabe_na_largura_do_papel() {
    // o texto mais comprido do caso de 58 mm quebra em linhas: nenhum trecho passa de 50 mm de largura util
    let c = casos();
    let x = c["casos"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["nome"] == "bobina_de_58_mm")
        .unwrap();
    let pdf = danfce::pdf(xml(x), &opcoes(&x["opcoes"])).unwrap();
    let linhas = texto_do_pdf(&pdf);
    assert!(linhas.iter().any(|l| l.contains("Acréscimos")));
    assert!(linhas.iter().all(|l| l.chars().count() < 60), "{linhas:?}");
}
