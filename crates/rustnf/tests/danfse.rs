// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Contrato do DANFSe: `tests/casos/danfse.json`. O `dados()` tem de bater com a foto guardada -- conteudo, leiaute
//! e cada operacao da pagina --, menos o desenho do QR (confere-se o texto). Os casos tambem sao impressos em PDF de
//! verdade.

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

/// NT 008 v1.02, item 2.4.5, nota 4: operacao sem incidencia do ISSQN (tribISSQN 4) = bloco so' com a frase; imunidade
/// e exportacao seguem com o bloco inteiro.
#[test]
fn nao_incidencia_reduz_o_bloco_municipal() {
    let xml = xml_do_caso("minima");
    assert!(
        xml.contains("<tribISSQN>1</tribISSQN>"),
        "caso base tributavel"
    );
    for (trib, visivel) in [("1", true), ("2", true), ("3", true), ("4", false)] {
        let x = xml.replace(
            "<tribISSQN>1</tribISSQN>",
            &format!("<tribISSQN>{trib}</tribISSQN>"),
        );
        let d = danfse::dados(&x, &Opcoes::default()).expect("dados");
        assert_eq!(d["municipal"]["visivel"], visivel, "tribISSQN {trib}");
        if !visivel {
            assert_eq!(
                d["municipal"]["mensagem"],
                "TRIBUTAÇÃO MUNICIPAL (ISSQN) - OPERAÇÃO NÃO SUJEITA AO ISSQN"
            );
        }
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

/// O PDF sem o corpo dos fluxos (binarios compactados): so' dicionarios, metadados e estrutura.
fn sem_fluxos(pdf: &[u8]) -> String {
    let mut resto = pdf;
    let mut saida = Vec::new();
    while let Some(i) = find(
        resto, b"stream
",
    ) {
        saida.extend_from_slice(&resto[..i]);
        let Some(fim) = find(&resto[i..], b"endstream") else {
            break;
        };
        resto = &resto[i + fim + b"endstream".len()..];
    }
    saida.extend_from_slice(resto);
    String::from_utf8_lossy(&saida).into_owned()
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
    // letra de unidade seguida de barra invertida, so' nos trechos de texto: fonte e imagem sao binarios e casam
    // por acaso (os pixels do logotipo, por exemplo)
    let unidade = regex::Regex::new(r"(?i)[a-z]:\\[a-z_]").expect("padrao valido");
    let textos = std::iter::once(sem_fluxos(&pdf)).chain(
        fluxos(&pdf)
            .into_iter()
            .filter(|b| {
                b.iter()
                    .all(|c| c.is_ascii_graphic() || c.is_ascii_whitespace())
            })
            .filter_map(|b| String::from_utf8(b).ok()),
    );
    for t in textos {
        if let Some(m) = unidade.find(&t) {
            panic!("caminho de maquina no PDF: {:?}", &t[m.start()..m.end()]);
        }
    }
    // o logotipo vai como imagem, com a transparencia dele
    assert!(contem(&pdf, b"/Subtype/Image"), "logotipo ausente");
    assert!(contem(&pdf, b"/SMask"), "logotipo sem a transparencia");
    // o titulo do documento leva o numero da NFS-e
    assert!(contem(&pdf, b"DANFSe 1234"), "titulo ausente");
}

fn xml_do_caso(nome: &str) -> String {
    let g = golden();
    let c = g["casos"]
        .as_array()
        .expect("casos")
        .iter()
        .find(|c| texto(c, "nome") == nome)
        .unwrap_or_else(|| panic!("caso `{nome}` ausente"));
    texto(c, "xml").to_string()
}

fn pdf_com(nome: &str, situacao: Situacao, canhoto: bool) -> Vec<u8> {
    let op = Opcoes {
        situacao,
        canhoto,
        municipio: None,
    };
    danfse::pdf(&xml_do_caso(nome), &op).expect("pdf")
}

#[test]
fn mesma_entrada_gera_os_mesmos_bytes() {
    let a = pdf_com("rtc_2027_completa", Situacao::Valida, true);
    let b = pdf_com("rtc_2027_completa", Situacao::Valida, true);
    assert_eq!(a, b, "o PDF nao e' deterministico");
    let c = pdf_com("rtc_2027_completa", Situacao::Cancelada, true);
    assert_eq!(
        c,
        pdf_com("rtc_2027_completa", Situacao::Cancelada, true),
        "o PDF da cancelada nao e' deterministico"
    );
}

#[test]
fn os_casos_do_contrato_geram_pdf() {
    let g = golden();
    for c in g["casos"].as_array().expect("casos") {
        let nome = texto(c, "nome");
        let pdf = danfse::pdf(texto(c, "xml"), &op(c).expect("opcoes")).expect("pdf");
        assert!(pdf.starts_with(b"%PDF-"), "{nome}: nao e' PDF");
        assert!(pdf.len() > 10_000, "{nome}: PDF pequeno demais");
        assert!(contem(&pdf, b"/Count 1"), "{nome}: nao declara uma pagina");
    }
}

#[test]
fn a_marca_dagua_aumenta_o_pdf_e_o_canhoto_pode_sair() {
    let valida = pdf_com("rtc_2027_completa", Situacao::Valida, true);
    let cancelada = pdf_com("rtc_2027_completa", Situacao::Cancelada, true);
    let substituida = pdf_com("rtc_2027_completa", Situacao::Substituida, true);
    assert!(cancelada.len() > valida.len(), "cancelada sem marca");
    assert!(substituida.len() > valida.len(), "substituida sem marca");
    let sem_canhoto = pdf_com("rtc_2027_completa", Situacao::Valida, false);
    assert!(sem_canhoto.starts_with(b"%PDF-"));
    assert_ne!(sem_canhoto, valida, "o canhoto nao muda o PDF");
}

#[test]
fn as_duas_fontes_vao_embutidas_como_subconjunto() {
    let pdf = pdf_com("rtc_2027_completa", Situacao::Valida, true);
    let nomes = regex::bytes::Regex::new(r"/BaseFont/([A-Z]{6})\+(LiberationSans(?:-Bold)?)/")
        .expect("padrao valido");
    let mut achados: Vec<String> = nomes
        .captures_iter(&pdf)
        .map(|c| String::from_utf8_lossy(&c[2]).into_owned())
        .collect();
    achados.sort();
    achados.dedup();
    assert_eq!(achados, ["LiberationSans", "LiberationSans-Bold"]);
    assert!(
        !contem(&pdf, b"/BaseFont/LiberationSans"),
        "fonte inteira, sem subconjunto"
    );
    let embutidas = regex::bytes::Regex::new(r"/FontFile2 \d+ 0 R")
        .expect("padrao valido")
        .find_iter(&pdf)
        .count();
    assert_eq!(embutidas, 2, "esperadas 2 fontes embutidas");
}

// ---- texto extraivel: o conteudo e' lido de volta pelo mapa ToUnicode de cada fonte

/// O corpo do objeto `n` (do fim da linha `N 0 obj` ate `endobj`).
fn objeto(pdf: &[u8], n: u32) -> &[u8] {
    let marca = format!("\n{n} 0 obj\n");
    let ini = find(pdf, marca.as_bytes()).unwrap_or_else(|| panic!("objeto {n} ausente"));
    let corpo = &pdf[ini + marca.len()..];
    &corpo[..find(corpo, b"endobj").expect("fim do objeto")]
}

/// O fluxo do objeto `n`, descomprimido.
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

/// Mapa codigo -> caractere de um ToUnicode (`beginbfchar`: `<0001> <0044>`).
fn mapa_tounicode(cmap: &str) -> std::collections::HashMap<u16, char> {
    let par = regex::Regex::new(r"<([0-9A-Fa-f]{4})>\s*<([0-9A-Fa-f]{4})>").expect("padrao valido");
    par.captures_iter(cmap)
        .filter_map(|c| {
            let cod = u16::from_str_radix(&c[1], 16).ok()?;
            let car = char::from_u32(u32::from_str_radix(&c[2], 16).ok()?)?;
            Some((cod, car))
        })
        .collect()
}

/// Desfaz os escapes de uma cadeia literal do PDF (`\000`, `\n`, `\(` ...).
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

/// Todo o texto desenhado na pagina, um trecho por operacao, lido pelos mapas ToUnicode.
fn texto_do_pdf(pdf: &[u8]) -> Vec<String> {
    let numero = |t: &str| t.parse::<u32>().expect("numero de objeto");
    let recursos = regex::Regex::new(r"/(f\d+) (\d+) 0 R").expect("padrao valido");
    let ini = find(pdf, b"/Font<<").expect("recursos de fonte");
    let bloco = String::from_utf8_lossy(&pdf[ini..ini + 80]).into_owned();
    let to_unicode = regex::Regex::new(r"/ToUnicode (\d+) 0 R").expect("padrao valido");
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
    let trecho = regex::bytes::Regex::new(r"(?s)/(f\d+) [\d.]+ Tf [^\[]*\[\((.*?)\)\]TJ")
        .expect("padrao valido");
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

#[test]
fn o_texto_do_pdf_e_extraivel() {
    let pdf = pdf_com("rtc_2027_completa", Situacao::Valida, true);
    let linhas = texto_do_pdf(&pdf);
    for esperado in [
        "1234",
        "MARIA DE TESTE",
        "CLIMA SERVICOS LTDA",
        "DANFSe v2.0",
    ] {
        assert!(
            linhas.iter().any(|l| l == esperado),
            "texto `{esperado}` nao extraivel; ha {} trechos",
            linhas.len()
        );
    }
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

#[test]
fn consulta_publica_de_municipio_pela_tabela_embutida() {
    use rustnf::danfse::municipio::buscar;
    assert_eq!(
        buscar("4106902"),
        Some(("CURITIBA".to_string(), "PR".to_string()))
    );
    assert_eq!(
        buscar("3550308"),
        Some(("SÃO PAULO".to_string(), "SP".to_string()))
    );
    // codigo que nao consta, curto demais ou com sobra: sem resposta
    assert_eq!(buscar("0000000"), None);
    assert_eq!(buscar("35503"), None);
    assert_eq!(buscar(" 3550308"), None);
    assert_eq!(buscar(""), None);
}
