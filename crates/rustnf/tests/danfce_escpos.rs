// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! DANFC-e em ESC/POS: bytes estaveis, comando do QR Code com o texto exato do `qrCode` do XML (nivel M), colunas e
//! pagina de codigo. Um interpretador de teste converte os bytes em linhas de texto e comandos.

use rustnf::danfce::{self, ErroDanfce, Largura, Opcoes, PaginaDeCodigo};
use serde_json::Value;

fn casos() -> Value {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/casos/danfce.json");
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn opcoes(o: &Value) -> Opcoes {
    let mut op = Opcoes::default();
    if o.get("largura").is_some_and(|l| l == "mm58") {
        op.largura = Largura::Mm58;
    }
    let b = |k: &str| o.get(k).and_then(Value::as_bool);
    op.resumido = b("resumido").unwrap_or(false);
    op.cancelada = b("cancelada").unwrap_or(false);
    op.via_estabelecimento = b("via_estabelecimento").unwrap_or(false);
    op.consumidor_com_nome_e_endereco = b("consumidor_com_nome_e_endereco").unwrap_or(true);
    op
}

fn xml(c: &Value) -> &[u8] {
    c["xml"].as_str().unwrap().as_bytes()
}

/// Letras acentuadas do PC850 que o cupom usa (byte -> caractere).
const PC850: [(u8, char); 21] = [
    (0x80, 'Ç'),
    (0x82, 'é'),
    (0x83, 'â'),
    (0x87, 'ç'),
    (0x88, 'ê'),
    (0x93, 'ô'),
    (0x90, 'É'),
    (0xa0, 'á'),
    (0xa2, 'ó'),
    (0xb5, 'Á'),
    (0xc7, 'Ã'),
    (0xe0, 'Ó'),
    (0xe2, 'Ô'),
    (0xe4, 'õ'),
    (0xe9, 'Ú'),
    (0xa1, 'í'),
    (0xa3, 'ú'),
    (0xa7, 'º'),
    (0xc6, 'ã'),
    (0xd2, 'Ê'),
    (0xd6, 'Í'),
];

#[derive(Debug, Default)]
struct Impresso {
    linhas: Vec<String>,
    larguras_duplas: Vec<bool>,
    qrs: Vec<String>,
    modulo_do_qr: Vec<u8>,
    nivel_do_qr: Vec<u8>,
    modelo_do_qr: Vec<u8>,
    pagina: Option<u8>,
    inicializou: bool,
    cortou: bool,
}

/// Interpreta os comandos que o cupom usa: `ESC @`, `ESC t n`, `ESC a n`, `ESC E n`, `GS ! n`, `GS ( k` e `GS V 1`.
fn interpretar(b: &[u8]) -> Impresso {
    let mut o = Impresso::default();
    let (mut i, mut atual, mut duplo) = (0usize, Vec::<u8>::new(), false);
    let mut carga: Option<String> = None;
    while i < b.len() {
        match b[i] {
            0x1b if b[i + 1] == b'@' => {
                o.inicializou = true;
                i += 2;
            }
            0x1b if b[i + 1] == b't' => {
                o.pagina = Some(b[i + 2]);
                i += 3;
            }
            0x1b if b[i + 1] == b'a' || b[i + 1] == b'E' => i += 3,
            0x1d if b[i + 1] == b'!' => {
                duplo = b[i + 2] == 0x10;
                i += 3;
            }
            0x1d if b[i + 1] == b'V' => {
                assert_eq!(b[i + 2], 1, "corte parcial");
                o.cortou = true;
                i += 3;
            }
            0x1d if b[i + 1] == b'(' && b[i + 2] == b'k' => {
                let n = usize::from(b[i + 3]) + 256 * usize::from(b[i + 4]);
                let corpo = &b[i + 5..i + 5 + n];
                assert_eq!(corpo[0], 0x31, "cn do QR Code");
                match corpo[1] {
                    0x41 => o.modelo_do_qr.push(corpo[2]),
                    0x43 => o.modulo_do_qr.push(corpo[2]),
                    0x45 => o.nivel_do_qr.push(corpo[2]),
                    0x50 => {
                        assert_eq!(corpo[2], 0x30);
                        carga = Some(String::from_utf8(corpo[3..].to_vec()).unwrap());
                    }
                    0x51 => o
                        .qrs
                        .push(carga.take().expect("QR impresso sem dados guardados")),
                    f => panic!("funcao {f:#x} do QR inesperada"),
                }
                i += 5 + n;
            }
            0x0a => {
                if !atual.is_empty() || o.cortou {
                    o.larguras_duplas.push(duplo);
                    o.linhas.push(decodificar(&atual));
                }
                atual.clear();
                i += 1;
            }
            c => {
                atual.push(c);
                i += 1;
            }
        }
    }
    o
}

fn decodificar(b: &[u8]) -> String {
    b.iter()
        .map(|&c| {
            if c < 0x80 {
                c as char
            } else {
                PC850
                    .iter()
                    .find(|(x, _)| *x == c)
                    .map_or('?', |(_, ch)| *ch)
            }
        })
        .collect()
}

#[test]
fn os_bytes_sao_estaveis_e_comecam_e_terminam_como_manda_o_protocolo() {
    let c = casos();
    for x in c["casos"].as_array().unwrap() {
        let nome = x["nome"].as_str().unwrap();
        let op = opcoes(&x["opcoes"]);
        let b = danfce::escpos(xml(x), &op).unwrap_or_else(|e| panic!("{nome}: {e}"));
        assert_eq!(
            b,
            danfce::escpos(xml(x), &op).unwrap(),
            "{nome}: bytes estaveis"
        );
        assert!(
            b.starts_with(&[0x1b, b'@', 0x1b, b't', 2]),
            "{nome}: ESC @ e ESC t 2"
        );
        assert!(
            b.ends_with(&[0x1d, b'V', 1]),
            "{nome}: corte parcial no fim"
        );
        let o = interpretar(&b);
        assert!(o.inicializou && o.cortou && o.pagina == Some(2), "{nome}");
    }
}

#[test]
fn o_qr_code_sai_pelo_comando_nativo_com_o_texto_exato_do_xml_e_nivel_m() {
    let c = casos();
    for x in c["casos"].as_array().unwrap() {
        let nome = x["nome"].as_str().unwrap();
        let qr_xml = x["dados"]
            .get("/qr_code")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| {
                let d = danfce::dados(xml(x), &opcoes(&x["opcoes"])).unwrap();
                d.qr_code
            });
        let b = danfce::escpos(xml(x), &opcoes(&x["opcoes"])).unwrap();
        let o = interpretar(&b);
        assert_eq!(o.qrs, vec![qr_xml.clone()], "{nome}: texto do QR");
        assert_eq!(o.nivel_do_qr, vec![0x31], "{nome}: nivel M (49)");
        assert_eq!(o.modelo_do_qr, vec![0x32], "{nome}: modelo 2");
        // o comando de guardar os dados com o tamanho certo: pL pH = tamanho + 3
        let n = qr_xml.len() + 3;
        let cabeca = [
            0x1d,
            b'(',
            b'k',
            (n & 255) as u8,
            (n >> 8) as u8,
            0x31,
            0x50,
            0x30,
        ];
        assert!(
            b.windows(cabeca.len()).any(|w| w == cabeca),
            "{nome}: GS ( k com o tamanho dos dados"
        );
        // 22 mm a 203 dpi sao 176 pontos: o modulo nunca deixa o conteudo menor que isso, e cabe na bobina
        let modulos = qrcode_modulos(&qr_xml);
        let m = usize::from(o.modulo_do_qr[0]);
        assert!(
            m * modulos >= 176 || m == 16,
            "{nome}: modulo {m} x {modulos}"
        );
        let colunas = if opcoes(&x["opcoes"]).largura == Largura::Mm58 {
            32
        } else {
            48
        };
        assert!(m * modulos <= colunas * 12, "{nome}: o QR passa da largura");
    }
}

/// Lado do QR de nivel M para o texto: o do menor tamanho que o comporta (tabela do ISO/IEC 18004, 8 bits, nivel M).
fn qrcode_modulos(texto: &str) -> usize {
    const CAPACIDADE_M: [usize; 20] = [
        14, 26, 42, 62, 84, 106, 122, 152, 180, 213, 251, 287, 331, 362, 412, 450, 504, 560, 624,
        666,
    ];
    let v = CAPACIDADE_M
        .iter()
        .position(|&c| texto.len() <= c)
        .expect("QR grande demais para o teste")
        + 1;
    17 + 4 * v
}

#[test]
fn as_linhas_do_cupom_cabem_nas_colunas_e_trazem_o_que_o_caso_manda() {
    let c = casos();
    for x in c["casos"].as_array().unwrap() {
        let nome = x["nome"].as_str().unwrap();
        let op = opcoes(&x["opcoes"]);
        let colunas = if op.largura == Largura::Mm58 { 32 } else { 48 };
        let o = interpretar(&danfce::escpos(xml(x), &op).unwrap());
        for (l, duplo) in o.linhas.iter().zip(&o.larguras_duplas) {
            let n = l.chars().count();
            assert!(
                n <= if *duplo { colunas / 2 } else { colunas },
                "{nome}: `{l}` tem {n} colunas"
            );
            assert!(
                !l.contains('?'),
                "{nome}: caractere fora da pagina em `{l}`"
            );
        }
        for esperada in x["escpos"].as_array().unwrap() {
            let e = esperada.as_str().unwrap();
            assert!(
                o.linhas.iter().any(|l| l.trim_end() == e.trim_end()),
                "{nome}: linha `{e}` ausente em {:#?}",
                o.linhas
            );
        }
    }
}

#[test]
fn o_aviso_da_contingencia_sai_em_duas_linhas_e_em_dois_lugares() {
    let c = casos();
    let x = c["casos"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["nome"] == "contingencia_off_line_pendente_via_consumidor")
        .unwrap();
    let o = interpretar(&danfce::escpos(xml(x), &opcoes(&x["opcoes"])).unwrap());
    let onde: Vec<usize> = o
        .linhas
        .iter()
        .enumerate()
        .filter(|(_, l)| l.trim() == "EMITIDA EM CONTINGÊNCIA")
        .map(|(i, _)| i)
        .collect();
    assert_eq!(onde.len(), 2, "{:#?}", o.linhas);
    for i in onde {
        assert_eq!(o.linhas[i + 1].trim(), "Pendente de autorização");
        assert!(
            o.larguras_duplas[i],
            "a primeira linha vai em largura dupla"
        );
    }
    // sem protocolo na contingencia
    assert!(!o
        .linhas
        .iter()
        .any(|l| l.contains("Protocolo de autorização")));
}

#[test]
fn colunas_e_paginas_de_codigo() {
    let c = casos();
    let x = &c["casos"].as_array().unwrap()[1];
    for cols in [48u8, 42, 32] {
        let op = Opcoes {
            colunas: Some(cols),
            ..Opcoes::default()
        };
        let o = interpretar(&danfce::escpos(xml(x), &op).unwrap());
        assert!(
            o.linhas
                .iter()
                .zip(&o.larguras_duplas)
                .all(|(l, d)| l.chars().count()
                    <= if *d {
                        usize::from(cols) / 2
                    } else {
                        usize::from(cols)
                    }),
            "{cols} colunas"
        );
    }
    let op = Opcoes {
        colunas: Some(40),
        ..Opcoes::default()
    };
    assert_eq!(
        danfce::escpos(xml(x), &op).unwrap_err(),
        ErroDanfce::Colunas(40)
    );
    // o `é` de "Cartão de Débito": 0x82 no PC850 e no PC860, 0xe9 no Windows-1252 (e a pagina sai no `ESC t`)
    for (pagina, esc_t, e_agudo, a_til) in [
        (PaginaDeCodigo::Pc850, 2u8, 0x82u8, 0xc6u8),
        (PaginaDeCodigo::Pc860, 3, 0x82, 0x84),
        (PaginaDeCodigo::Wpc1252, 16, 0xe9, 0xe3),
    ] {
        let op = Opcoes {
            pagina_de_codigo: pagina,
            ..Opcoes::default()
        };
        let b = danfce::escpos(xml(x), &op).unwrap();
        assert_eq!(&b[2..5], &[0x1b, b't', esc_t]);
        let alvo: Vec<u8> = [b"Cart".as_slice(), &[a_til], b"o de D", &[e_agudo], b"bito"].concat();
        assert!(
            b.windows(alvo.len()).any(|w| w == alvo),
            "{pagina:?}: `Cartão de Débito`"
        );
    }
}

#[test]
fn sem_corte_o_cupom_termina_so_com_avanco() {
    let c = casos();
    let x = &c["casos"].as_array().unwrap()[0];
    let op = Opcoes {
        cortar: false,
        ..Opcoes::default()
    };
    let b = danfce::escpos(xml(x), &op).unwrap();
    assert!(!interpretar(&b).cortou);
    assert!(b.ends_with(&[0x0a, 0x0a]));
}
