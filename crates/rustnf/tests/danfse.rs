// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Contrato do DANFSe: `tests/casos/danfse.json`. O `dados()` tem de bater com a foto guardada -- conteudo, leiaute
//! e cada operacao da pagina --, menos o desenho do QR (confere-se o texto). Um caso e' impresso em PDF de verdade.

use flate2::read::ZlibDecoder;
use rustnf::danfse::{self, ErroDanfse, Opcoes, Situacao};
use serde_json::Value;
use std::io::Read;

fn golden() -> Value {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/casos/danfse.json");
    serde_json::from_str(&std::fs::read_to_string(p).expect("ler o contrato"))
        .expect("contrato e' JSON")
}

fn texto<'a>(v: &'a Value, campo: &str) -> &'a str {
    v[campo]
        .as_str()
        .unwrap_or_else(|| panic!("campo `{campo}` ausente"))
}

fn op(c: &Value) -> Result<Opcoes<'static>, ErroDanfse> {
    let o = &c["opcoes"];
    Ok(Opcoes {
        situacao: match o.get("situacao").and_then(Value::as_str) {
            Some(s) => s.parse::<Situacao>()?,
            None => Situacao::Valida,
        },
        canhoto: o.get("canhoto").and_then(Value::as_bool).unwrap_or(true),
        municipio: None,
    })
}

fn sem_desenho_do_qr(mut d: Value) -> Value {
    let t = d["qr"]["texto"].clone();
    d["qr"] = serde_json::json!({ "texto": t });
    d
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
    for c in golden()["casos"].as_array().expect("casos") {
        let nome = texto(c, "nome");
        let obtido = sem_desenho_do_qr(
            danfse::dados(texto(c, "xml"), &op(c).expect("opcoes")).expect("dados"),
        );
        let guardado = sem_desenho_do_qr(c["dados"].clone());
        let mut dif = Vec::new();
        diferencas(&obtido, &guardado, nome, &mut dif);
        assert!(
            dif.is_empty(),
            "{} diferencas, as primeiras:\n{}",
            dif.len(),
            dif[..dif.len().min(15)].join("\n")
        );
    }
}

#[test]
fn esperado_a_mao() {
    for c in golden()["casos"].as_array().expect("casos") {
        let d = danfse::dados(texto(c, "xml"), &op(c).expect("opcoes")).expect("dados");
        for (k, v) in c["esperado"].as_object().expect("esperado") {
            let obtido = k.split('.').fold(&d, |acc, p| &acc[p]);
            assert_eq!(obtido, v, "{} {k}", texto(c, "nome"));
        }
    }
}

#[test]
fn recusas() {
    for r in golden()["recusas"].as_array().expect("recusas") {
        let erro = op(r)
            .and_then(|o| danfse::dados(texto(r, "xml"), &o))
            .expect_err(texto(r, "nome"))
            .to_string();
        assert!(
            erro.contains(texto(r, "erro")),
            "{}: {erro}",
            texto(r, "nome")
        );
    }
}

#[test]
fn municipio_resolvido_por_quem_chama() {
    let c = &golden()["casos"][0];
    let f = |ibge: &str| (ibge == "3503208").then(|| ("Araraquara".to_string(), "SP".to_string()));
    let d = danfse::dados(
        texto(c, "xml"),
        &Opcoes {
            municipio: Some(&f),
            ..Opcoes::default()
        },
    )
    .expect("dados");
    assert_eq!(d["partes"]["tomador"]["municipio_uf"], "Araraquara / SP");
}

#[test]
fn qr_e_a_consulta_publica() {
    let c = &golden()["casos"][1];
    let d = danfse::dados(texto(c, "xml"), &Opcoes::default()).expect("dados");
    assert_eq!(
        texto(&d["qr"], "texto"),
        format!("{}{}", danfse::CONSULTA, texto(&d["acesso"], "chave"))
    );
    assert!(d["qr"]["n"].as_u64().expect("n") >= 25);
}

#[test]
fn situacao_desconhecida_e_recusada() {
    assert_eq!(
        "anulada".parse::<Situacao>(),
        Err(ErroDanfse::Situacao("anulada".into()))
    );
    assert_eq!("cancelada".parse::<Situacao>(), Ok(Situacao::Cancelada));
}

// ---------------------------------------------------------------- PDF

/// Os fluxos do PDF, descomprimidos quando sao zlib; o resto fica como esta.
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

fn find(palheiro: &[u8], agulha: &[u8]) -> Option<usize> {
    palheiro.windows(agulha.len()).position(|w| w == agulha)
}

fn contem(pdf: &[u8], agulha: &[u8]) -> bool {
    find(pdf, agulha).is_some() || fluxos(pdf).iter().any(|f| find(f, agulha).is_some())
}

#[test]
fn pdf_de_verdade_com_uma_pagina_e_sem_rastro_de_maquina() {
    let g = golden();
    let c = g["casos"]
        .as_array()
        .expect("casos")
        .iter()
        .find(|c| texto(c, "nome") == "rtc_2027_completa")
        .expect("caso completo");
    let pdf = danfse::pdf(texto(c, "xml"), &op(c).expect("opcoes")).expect("pdf");
    if let Some(destino) = std::env::var_os("RUSTNF_DANFSE_EXEMPLO") {
        std::fs::write(destino, &pdf).expect("gravar o exemplar");
    }
    assert!(pdf.starts_with(b"%PDF-"), "nao e' PDF");
    assert!(
        pdf.len() > 20_000,
        "PDF pequeno demais: {} bytes",
        pdf.len()
    );

    // uma pagina: o no `/Pages` declara `/Count 1` e so' ha um objeto `/Type/Page`
    assert!(contem(&pdf, b"/Count 1"), "o PDF nao declara uma pagina");
    let paginas = pdf
        .windows(b"/Type/Page/".len())
        .filter(|w| w.starts_with(b"/Type/Page") && w[b"/Type/Page".len()] != b's')
        .count();
    assert_eq!(paginas, 1, "esperada 1 pagina");

    // nenhum caminho de maquina nem nome de usuario nos metadados e nos fluxos
    for rastro in [&b"\\Users\\"[..], b"/Users/", b"/home/", b"/tmp/"] {
        assert!(
            !contem(&pdf, rastro),
            "rastro de maquina no PDF: {}",
            String::from_utf8_lossy(rastro)
        );
    }
    // letra de unidade seguida de barra invertida, so' nos trechos de texto (fonte binaria casa por acaso)
    let unidade = regex::Regex::new(r"(?i)[a-z]:\\[a-z_]").expect("padrao valido");
    let textos = std::iter::once(String::from_utf8_lossy(&pdf).into_owned()).chain(
        fluxos(&pdf)
            .into_iter()
            .filter_map(|b| String::from_utf8(b).ok()),
    );
    for t in textos {
        assert!(!unidade.is_match(&t), "caminho de maquina no PDF");
    }
    // o titulo do documento leva o numero da NFS-e
    assert!(contem(&pdf, b"DANFSe 1234"), "titulo ausente");
}

#[test]
fn pdf_recusa_o_que_nao_e_nfse() {
    let e = danfse::pdf(
        "<DPS xmlns=\"http://www.sped.fazenda.gov.br/nfse\"/>",
        &Opcoes::default(),
    )
    .unwrap_err();
    assert!(matches!(e, ErroDanfse::NaoENfse(_)), "{e}");
}
