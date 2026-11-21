// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A linha de comando por processo real (`CARGO_BIN_EXE_rustnf`): saidas, codigos de saida e a regra de que a
//! senha do certificado nunca vem por argumento. Nenhum teste chama a Sefin Nacional: os que usam certificado
//! param numa recusa local antes de qualquer envio.

mod comum;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

use chrono::Utc;
use comum::*;
use rustnf::assinatura::cert_a1;
use serde_json::{json, Value};

struct Saida {
    codigo: i32,
    out: Vec<u8>,
    err: String,
}

fn pasta() -> PathBuf {
    let p = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("cli");
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn gravar(nome: &str, conteudo: &[u8]) -> String {
    let p = pasta().join(nome);
    std::fs::write(&p, conteudo).unwrap();
    p.to_string_lossy().into_owned()
}

fn rodar(args: &[&str], stdin: Option<&str>, env: &[(&str, &str)]) -> Saida {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_rustnf"));
    let vazia = pasta().join("vazia");
    std::fs::create_dir_all(&vazia).unwrap();
    isolar(&mut cmd, &vazia);
    cmd.args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in env {
        cmd.env(k, v);
    }
    let mut filho = cmd.spawn().unwrap();
    if let Some(t) = stdin {
        filho.stdin.take().unwrap().write_all(t.as_bytes()).unwrap();
    } else {
        drop(filho.stdin.take());
    }
    let saida = filho.wait_with_output().unwrap();
    Saida {
        codigo: saida.status.code().unwrap(),
        out: saida.stdout,
        err: String::from_utf8_lossy(&saida.stderr).into_owned(),
    }
}

/// Um .pfx de teste (senha "teste"), gerado uma vez e compartilhado pelos testes.
fn pfx_de_teste(senha: &str) -> String {
    static PFX: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    assert_eq!(senha, "teste");
    PFX.get_or_init(|| {
        let bytes = cert_a1::gerar_teste(
            "11222333000181",
            "OFICINA TESTE",
            senha,
            365,
            true,
            false,
            Utc::now(),
        )
        .unwrap();
        gravar("teste.pfx", &bytes)
    })
    .clone()
}

#[test]
fn dps_montar_escreve_o_xml_no_stdout() {
    let pedido = gravar("dps_valida.json", dps_valida().to_string().as_bytes());
    let s = rodar(&["dps", "montar", &pedido], None, &[]);
    assert_eq!(s.codigo, 0, "{}", s.err);
    assert_eq!(
        String::from_utf8(s.out).unwrap(),
        format!("{}\n", xml_da_dps_valida())
    );
    assert!(
        s.err.is_empty(),
        "diagnostico no stderr, dado no stdout: {}",
        s.err
    );
}

#[test]
fn dps_montar_le_do_stdin_com_traco() {
    let s = rodar(
        &["dps", "montar", "-"],
        Some(&dps_valida().to_string()),
        &[],
    );
    assert_eq!(s.codigo, 0, "{}", s.err);
    assert_eq!(
        String::from_utf8(s.out).unwrap(),
        format!("{}\n", xml_da_dps_valida())
    );
}

#[test]
fn dps_conferir_diz_ok_ou_a_recusa_com_saida_3() {
    let boa = gravar("dps_boa.json", dps_valida().to_string().as_bytes());
    let s = rodar(&["dps", "conferir", &boa], None, &[]);
    assert_eq!(
        (s.codigo, String::from_utf8(s.out).unwrap().as_str()),
        (0, "ok\n")
    );

    let (ruim, campo) = dps_recusada();
    let ruim = gravar("dps_ruim.json", ruim.to_string().as_bytes());
    let s = rodar(&["dps", "conferir", &ruim], None, &[]);
    assert_eq!(s.codigo, 3);
    assert!(s.out.is_empty());
    assert!(s.err.contains(&campo), "{}", s.err);

    let s = rodar(&["dps", "conferir", &ruim, "--json"], None, &[]);
    assert_eq!(s.codigo, 3);
    let v: Value = serde_json::from_slice(&s.out).unwrap();
    assert_eq!(v["ok"], false);
    assert_eq!(v["campo"], campo);
}

#[test]
fn dps_com_json_invalido_ou_arquivo_ausente_e_saida_2() {
    let s = rodar(&["dps", "conferir", "-"], Some("{ nao e json"), &[]);
    assert_eq!(s.codigo, 2);
    let s = rodar(&["dps", "montar", "nao-existe.json"], None, &[]);
    assert_eq!(s.codigo, 2);
    assert!(
        !s.err.contains("nao-existe"),
        "a mensagem nao cita o caminho: {}",
        s.err
    );
}

#[test]
fn calcular_usa_um_caso_da_previa() {
    let previa = casos("reforma_previa.json");
    let op = json!({"base": previa["bases"][0]["entrada"].clone()});
    let arq = gravar("calc_base.json", op.to_string().as_bytes());
    let s = rodar(&["calcular", &arq], None, &[]);
    assert_eq!(s.codigo, 0, "{}", s.err);
    let v: Value = serde_json::from_slice(&s.out).unwrap();
    assert_eq!(
        v["bases"]["regime_regular"],
        previa["bases"][0]["regime_regular"]
    );

    let op = json!({"tributos": previa["tributos"][0]["entrada"].clone()});
    let s = rodar(&["calcular", "-"], Some(&op.to_string()), &[]);
    assert_eq!(s.codigo, 0, "{}", s.err);
    let v: Value = serde_json::from_slice(&s.out).unwrap();
    assert_eq!(
        v["tributos"]["ibs"],
        previa["tributos"][0]["esperado"]["ibs"]
    );
}

#[test]
fn calcular_recusado_sai_com_3_e_vazio_sai_com_2() {
    let s = rodar(
        &["calcular", "-"],
        Some(r#"{"tributos":{"data":"2026-10-20","cst":"000","cclass_trib":"000001"}}"#),
        &[],
    );
    assert_eq!(s.codigo, 3, "{}", s.err);
    let s = rodar(&["calcular", "-"], Some("{}"), &[]);
    assert_eq!(s.codigo, 2);
}

#[test]
fn danfse_grava_um_pdf() {
    let xml = gravar("nfse.xml", nfse_xml_para_danfse().as_bytes());
    let saida = pasta().join("danfse.pdf");
    let _ = std::fs::remove_file(&saida);
    let saida = saida.to_string_lossy().into_owned();
    let s = rodar(
        &[
            "danfse",
            &xml,
            "-o",
            &saida,
            "--situacao",
            "cancelada",
            "--sem-canhoto",
        ],
        None,
        &[],
    );
    assert_eq!(s.codigo, 0, "{}", s.err);
    assert!(std::fs::read(&saida).unwrap().starts_with(b"%PDF-"));

    let s = rodar(
        &["danfse", "-", "-o", "-"],
        Some(&nfse_xml_para_danfse()),
        &[],
    );
    assert_eq!(s.codigo, 0, "{}", s.err);
    assert!(s.out.starts_with(b"%PDF-"));

    let s = rodar(&["danfse", "-", "-o", &saida], Some("<nao-e-nfse/>"), &[]);
    assert_eq!(s.codigo, 2);
}

#[test]
fn a_senha_nao_pode_ir_por_argumento() {
    let pedido = gravar("dps_para_senha.json", dps_valida().to_string().as_bytes());
    let s = rodar(
        &[
            "emitir",
            &pedido,
            "--pfx",
            "x.pfx",
            "--ambiente",
            "restrita",
            "--senha",
            "segredo123",
        ],
        None,
        &[],
    );
    assert_eq!(s.codigo, 2);
    assert!(s.err.contains("--senha"), "{}", s.err);
    // nem na ajuda existe uma opcao `--senha` pura: so' --senha-env e --senha-stdin
    for sub in ["emitir", "consultar", "cancelar", "serve"] {
        let ajuda = rodar(&[sub, "--help"], None, &[]);
        assert_eq!(ajuda.codigo, 0);
        let texto = String::from_utf8(ajuda.out).unwrap();
        assert!(
            texto.contains("--senha-env") && texto.contains("--senha-stdin"),
            "{sub}"
        );
        assert!(
            !texto.contains("--senha <") && !texto.contains("--senha,"),
            "{sub}: {texto}"
        );
    }
}

#[test]
fn emitir_sem_origem_de_senha_ou_com_variavel_ausente_e_saida_2() {
    let pedido = gravar("dps_sem_senha.json", dps_valida().to_string().as_bytes());
    let pfx = pfx_de_teste("teste");
    let base = [
        "emitir",
        pedido.as_str(),
        "--pfx",
        pfx.as_str(),
        "--ambiente",
        "restrita",
    ];
    let s = rodar(&base, None, &[]);
    assert_eq!(s.codigo, 2);
    assert!(s.err.contains("--senha-env"), "{}", s.err);

    let mut args = base.to_vec();
    args.extend(["--senha-env", "RUSTNF_VARIAVEL_QUE_NAO_EXISTE"]);
    let s = rodar(&args, None, &[]);
    assert_eq!(s.codigo, 2);
    assert!(s.err.contains("RUSTNF_VARIAVEL_QUE_NAO_EXISTE"));

    let mut args = base.to_vec();
    args.extend(["--senha-env", "X", "--senha-stdin"]);
    assert_eq!(rodar(&args, None, &[("X", "teste")]).codigo, 2);
}

#[test]
fn senha_errada_do_certificado_e_saida_2_sem_ecoar_a_senha() {
    let pedido = gravar("dps_senha_errada.json", dps_valida().to_string().as_bytes());
    let pfx = pfx_de_teste("teste");
    let s = rodar(
        &[
            "emitir",
            &pedido,
            "--pfx",
            &pfx,
            "--ambiente",
            "restrita",
            "--senha-env",
            "RUSTNF_TESTE_SENHA",
        ],
        None,
        &[("RUSTNF_TESTE_SENHA", "senha-errada-ultra-secreta")],
    );
    assert_eq!(s.codigo, 2, "{}", s.err);
    assert!(!s.err.contains("ultra-secreta") && String::from_utf8_lossy(&s.out).is_empty());
}

#[test]
fn emitir_com_pedido_recusado_para_antes_da_rede_com_a_senha_do_ambiente_ou_do_stdin() {
    let (ruim, campo) = dps_recusada();
    let pedido = gravar("dps_recusada_emitir.json", ruim.to_string().as_bytes());
    let pfx = pfx_de_teste("teste");
    let s = rodar(
        &[
            "emitir",
            &pedido,
            "--pfx",
            &pfx,
            "--ambiente",
            "restrita",
            "--senha-env",
            "RUSTNF_TESTE_SENHA",
        ],
        None,
        &[("RUSTNF_TESTE_SENHA", "teste")],
    );
    assert_eq!(s.codigo, 3, "{}", s.err);
    assert!(s.err.contains(&campo), "{}", s.err);

    let s = rodar(
        &[
            "emitir",
            &pedido,
            "--pfx",
            &pfx,
            "--ambiente",
            "restrita",
            "--senha-stdin",
        ],
        Some("teste\n"),
        &[],
    );
    assert_eq!(s.codigo, 3, "{}", s.err);

    // pedido e senha pelo stdin ao mesmo tempo: conflito de uso
    let s = rodar(
        &[
            "emitir",
            "-",
            "--pfx",
            &pfx,
            "--ambiente",
            "restrita",
            "--senha-stdin",
        ],
        Some("{}"),
        &[],
    );
    assert_eq!(s.codigo, 2);
}

#[test]
fn consultar_e_cancelar_com_chave_ruim_param_antes_da_rede() {
    let pfx = pfx_de_teste("teste");
    let env = [("RUSTNF_TESTE_SENHA", "teste")];
    let s = rodar(
        &[
            "consultar",
            "--chave",
            "123",
            "--pfx",
            &pfx,
            "--ambiente",
            "restrita",
            "--senha-env",
            "RUSTNF_TESTE_SENHA",
        ],
        None,
        &env,
    );
    assert_eq!(s.codigo, 3, "{}", s.err);
    let s = rodar(
        &[
            "cancelar",
            "--chave",
            "123",
            "--motivo",
            "1",
            "--justificativa",
            "Valor digitado errado na nota",
            "--pfx",
            &pfx,
            "--ambiente",
            "restrita",
            "--senha-env",
            "RUSTNF_TESTE_SENHA",
        ],
        None,
        &env,
    );
    assert_eq!(s.codigo, 3, "{}", s.err);
}

#[test]
fn serve_recusa_subir_com_emissao_sem_token() {
    let pfx = pfx_de_teste("teste");
    // sem --token-env: o clap recusa
    let s = rodar(
        &[
            "serve",
            "--porta",
            "0",
            "--permitir-emissao",
            "--pfx",
            &pfx,
            "--ambiente",
            "restrita",
            "--senha-env",
            "S",
        ],
        None,
        &[("S", "teste")],
    );
    assert_eq!(s.codigo, 2, "{}", s.err);
    assert!(s.err.contains("token-env"), "{}", s.err);
    // variavel do token ausente
    let s = rodar(
        &[
            "serve",
            "--porta",
            "0",
            "--permitir-emissao",
            "--pfx",
            &pfx,
            "--ambiente",
            "restrita",
            "--senha-env",
            "S",
            "--token-env",
            "RUSTNF_TOKEN_AUSENTE",
        ],
        None,
        &[("S", "teste")],
    );
    assert_eq!(s.codigo, 2, "{}", s.err);
    assert!(s.err.contains("sem token configurado"), "{}", s.err);
    // token curto
    let s = rodar(
        &[
            "serve",
            "--porta",
            "0",
            "--permitir-emissao",
            "--pfx",
            &pfx,
            "--ambiente",
            "restrita",
            "--senha-env",
            "S",
            "--token-env",
            "T",
        ],
        None,
        &[("S", "teste"), ("T", "curto")],
    );
    assert_eq!(s.codigo, 2, "{}", s.err);
    assert!(s.err.contains("32"), "{}", s.err);
}

struct Servidor(Child);

impl Drop for Servidor {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn http_get(endereco: &str, rota: &str) -> (u16, String) {
    let mut s = TcpStream::connect(endereco).unwrap();
    write!(
        s,
        "GET {rota} HTTP/1.1\r\nHost: {endereco}\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    let mut resposta = String::new();
    s.read_to_string(&mut resposta).unwrap();
    let status = resposta.split(' ').nth(1).unwrap().parse().unwrap();
    (
        status,
        resposta.split("\r\n\r\n").nth(1).unwrap_or("").to_string(),
    )
}

#[test]
fn serve_sobe_em_porta_livre_e_responde_health() {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_rustnf"));
    isolar(&mut cmd, &pasta().join("vazia"));
    let filho = cmd
        .args(["serve", "--porta", "0"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut srv = Servidor(filho);
    let mut linhas = BufReader::new(srv.0.stderr.take().unwrap());
    let mut linha = String::new();
    linhas.read_line(&mut linha).unwrap();
    let endereco = linha
        .trim()
        .strip_prefix("rustnf: escutando em http://")
        .unwrap_or_else(|| panic!("primeira linha inesperada: {linha}"))
        .to_string();
    assert!(endereco.starts_with("127.0.0.1:"), "{endereco}");
    let (st, corpo) = http_get(&endereco, "/health");
    assert_eq!(st, 200);
    assert_eq!(
        serde_json::from_str::<Value>(&corpo).unwrap()["status"],
        "ok"
    );
    let (st, _) = http_get(&endereco, "/emitir");
    assert_eq!(st, 404);
}

#[test]
fn versao_e_ajuda() {
    let s = rodar(&["--version"], None, &[]);
    assert_eq!(s.codigo, 0);
    assert!(String::from_utf8(s.out).unwrap().starts_with("rustnf "));
    assert_eq!(rodar(&["subcomando-que-nao-existe"], None, &[]).codigo, 2);
}

#[test]
fn os_exemplos_do_readme_funcionam() {
    let ex = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("exemplos");
    let ex = |n: &str| ex.join(n).to_string_lossy().into_owned();
    let s = rodar(&["dps", "montar", &ex("dps.json")], None, &[]);
    assert_eq!(s.codigo, 0, "{}", s.err);
    assert_eq!(
        String::from_utf8(s.out).unwrap(),
        format!("{}\n", xml_da_dps_valida())
    );
    let s = rodar(&["calcular", &ex("calculo.json")], None, &[]);
    assert_eq!(s.codigo, 0, "{}", s.err);
    let saida = pasta().join("exemplo.pdf").to_string_lossy().into_owned();
    let s = rodar(&["danfse", &ex("nfse.xml"), "-o", &saida], None, &[]);
    assert_eq!(s.codigo, 0, "{}", s.err);
    assert!(std::fs::read(&saida).unwrap().starts_with(b"%PDF-"));
}
