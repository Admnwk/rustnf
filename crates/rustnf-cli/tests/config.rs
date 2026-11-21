// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A configuracao na linha de comando, por processo real: o arquivo de exemplo, `config`, `senha` e `notas` e a
//! precedencia opcao > ambiente > arquivo > padrao. Nenhum teste chama a Sefin Nacional, toca o cofre real do sistema
//! ou le a pasta de configuracao do usuario.

mod comum;

use std::io::Write;
use std::process::{Command, Stdio};

use comum::*;
use rustnf::chave_dfe;
use rustnf_app::config::Config;
use rustnf_app::notas::{Notas, Registro, Situacao};
use serde_json::{json, Value};
struct Saida {
    codigo: i32,
    out: String,
    err: String,
}

fn rodar(vazia: &Pasta, args: &[&str], stdin: Option<&str>, env: &[(&str, &str)]) -> Saida {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_rustnf"));
    isolar(&mut cmd, &vazia.0);
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
    let s = filho.wait_with_output().unwrap();
    Saida {
        codigo: s.status.code().unwrap(),
        out: String::from_utf8_lossy(&s.stdout).into_owned(),
        err: String::from_utf8_lossy(&s.stderr).into_owned(),
    }
}

#[test]
fn o_exemplo_carrega_e_valida() {
    let c = Config::ler(&exemplo()).unwrap();
    c.validar().unwrap();
    assert_eq!(c.emitente.cnpj, "11222333000181");
    assert_eq!(c.emitente.municipio, "3550308");
    assert_eq!(c.sefin.ambiente, "restrita");
    assert!(c.danfse.canhoto);
    assert_eq!(c.servidor.porta, 8080);
    assert!(c.publicadas().contains_key(&2027));
    // o exemplo diz o que o programa assume quando nao ha arquivo, fora os dados do emitente
    assert_eq!(c.servidor, Config::default().servidor);
    assert_eq!(
        c.sefin.tempo_limite_s,
        Config::default().sefin.tempo_limite_s
    );
}

#[test]
fn config_mostrar_sem_arquivo_mostra_o_padrao() {
    let vazia = Pasta::nova();
    let s = rodar(&vazia, &["config", "mostrar"], None, &[]);
    assert_eq!(s.codigo, 0, "{}", s.err);
    assert!(s.out.contains("# arquivo: nenhum"), "{}", s.out);
    assert!(s.out.contains("ambiente = \"restrita\""), "{}", s.out);
    assert!(s.out.contains("porta = 8080"), "{}", s.out);

    let s = rodar(&vazia, &["config", "mostrar", "--json"], None, &[]);
    assert_eq!(s.codigo, 0, "{}", s.err);
    let v: Value = serde_json::from_str(&s.out).unwrap();
    assert_eq!(v["arquivo"], Value::Null);
    assert_eq!(v["variaveis"], json!([]));
    assert_eq!(v["config"]["servidor"]["porta"], 8080);
}

#[test]
fn config_mostrar_com_o_exemplo_e_com_variavel_de_ambiente() {
    let vazia = Pasta::nova();
    let ex = exemplo().to_string_lossy().into_owned();
    let s = rodar(&vazia, &["config", "mostrar", "--config", &ex], None, &[]);
    assert_eq!(s.codigo, 0, "{}", s.err);
    assert!(s.out.contains("cnpj = \"11222333000181\""), "{}", s.out);
    assert!(s.out.contains("[[aliquotas]]"), "{}", s.out);
    assert!(s.out.contains("# arquivo: "), "{}", s.out);
    // nunca mostra segredo: nem existe campo para isso
    let minusculas = s.out.to_lowercase();
    assert!(!minusculas.contains("senha") && !minusculas.contains("token"));

    let s = rodar(
        &vazia,
        &["config", "mostrar", "--json", "--config", &ex],
        None,
        &[("RUSTNF_PORTA", "9191"), ("RUSTNF_AMBIENTE", "producao")],
    );
    assert_eq!(s.codigo, 0, "{}", s.err);
    let v: Value = serde_json::from_str(&s.out).unwrap();
    assert_eq!(v["config"]["servidor"]["porta"], 9191);
    assert_eq!(v["config"]["sefin"]["ambiente"], "producao");
    assert_eq!(v["variaveis"], json!(["RUSTNF_AMBIENTE", "RUSTNF_PORTA"]));
    assert_eq!(v["arquivo"], ex.as_str());
}

#[test]
fn config_caminho_segue_a_opcao_a_variavel_e_o_local_padrao() {
    let vazia = Pasta::nova();
    let s = rodar(
        &vazia,
        &["config", "caminho", "--config", "um.toml"],
        None,
        &[],
    );
    assert_eq!((s.codigo, s.out.trim()), (0, "um.toml"), "{}", s.err);
    let s = rodar(
        &vazia,
        &["config", "caminho"],
        None,
        &[("RUSTNF_CONFIG", "dois.toml")],
    );
    assert_eq!((s.codigo, s.out.trim()), (0, "dois.toml"), "{}", s.err);
    let s = rodar(&vazia, &["config", "caminho"], None, &[]);
    assert_eq!(s.codigo, 0, "{}", s.err);
    assert!(s.out.trim().ends_with("rustnf.toml"), "{}", s.out);
    assert!(s.out.contains("rustnf"), "{}", s.out);
}

#[test]
fn arquivo_explicito_ausente_e_erro_e_o_padrao_ausente_nao() {
    let vazia = Pasta::nova();
    let s = rodar(
        &vazia,
        &["config", "mostrar", "--config", "nao-existe.toml"],
        None,
        &[],
    );
    assert_eq!(s.codigo, 2);
    assert!(
        s.err.contains("--config") && !s.err.contains("nao-existe"),
        "{}",
        s.err
    );
    let s = rodar(
        &vazia,
        &["config", "mostrar"],
        None,
        &[("RUSTNF_CONFIG", "nao-existe.toml")],
    );
    assert_eq!(s.codigo, 2);
    assert!(s.err.contains("RUSTNF_CONFIG"), "{}", s.err);
    // sem opcao nem variavel e sem arquivo no local padrao: padrao, sem erro
    assert_eq!(rodar(&vazia, &["config", "mostrar"], None, &[]).codigo, 0);
}

#[test]
fn campo_desconhecido_senha_e_token_no_arquivo_sao_recusados() {
    let p = Pasta::nova();
    for (i, texto) in [
        "[sefin]\nsenha = \"abc123\"\n",
        "[servidor]\ntoken = \"abc123\"\n",
        "[sefin]\nambiente = \"restrita\"\nsenha_do_pfx = \"abc123\"\n",
    ]
    .iter()
    .enumerate()
    {
        let arq = p.escrever(&format!("ruim{i}.toml"), texto.as_bytes());
        let s = rodar(&p, &["config", "mostrar", "--config", &arq], None, &[]);
        assert_eq!(s.codigo, 2, "{texto}");
        assert!(s.err.contains("unknown field"), "{}", s.err);
        assert!(
            !s.err.contains("abc123"),
            "o valor nao pode ser ecoado: {}",
            s.err
        );
        assert!(s.out.is_empty());
    }
}

#[test]
fn valor_invalido_no_arquivo_aponta_secao_e_campo() {
    let p = Pasta::nova();
    let arq = p.escrever("c.toml", b"[sefin]\nambiente = \"homologacao\"\n");
    let s = rodar(&p, &["config", "mostrar", "--config", &arq], None, &[]);
    assert_eq!(s.codigo, 2);
    assert!(s.err.contains("[sefin] ambiente"), "{}", s.err);
}

#[test]
fn calcular_2027_so_calcula_com_as_aliquotas_do_arquivo() {
    let p = Pasta::nova();
    let op = json!({"tributos": {
        "data": "2027-06-01", "cst": "000", "cclass_trib": "000001", "base": "100.00"
    }})
    .to_string();
    let sem = rodar(&p, &["calcular", "-"], Some(&op), &[]);
    let com = rodar(
        &p,
        &["calcular", "-", "--config", &exemplo().to_string_lossy()],
        Some(&op),
        &[],
    );
    assert_eq!(com.codigo, 0, "{}", com.err);
    let v: Value = serde_json::from_str(&com.out).unwrap();
    assert_eq!(v["tributos"]["cbs"]["aliquota"], "8.70", "{}", com.out);
    // sem o arquivo a previa recusa (saida 3): o numero publicado da CBS de 2027 nao foi informado
    assert_eq!(sem.codigo, 3, "{}", sem.err);
    assert!(sem.err.contains("2027"), "{}", sem.err);

    // o que o pedido traz vale sobre o arquivo
    let op = json!({"tributos": {
        "data": "2027-06-01", "cst": "000", "cclass_trib": "000001", "base": "100.00",
        "publicadas": {"2027": {"cbs_referencia": "10.0"}}
    }})
    .to_string();
    let s = rodar(
        &p,
        &["calcular", "-", "--config", &exemplo().to_string_lossy()],
        Some(&op),
        &[],
    );
    assert_eq!(s.codigo, 0, "{}", s.err);
    let v: Value = serde_json::from_str(&s.out).unwrap();
    assert_eq!(v["tributos"]["cbs"]["aliquota"], "9.90", "{}", s.out);
}

#[test]
fn emitir_sem_arquivo_continua_exigindo_ambiente_e_certificado() {
    let p = Pasta::nova();
    let (ruim, _) = dps_recusada();
    let pedido = p.escrever("pedido.json", ruim.to_string().as_bytes());
    let s = rodar(
        &p,
        &["emitir", &pedido, "--senha-env", "S"],
        None,
        &[("S", "teste")],
    );
    assert_eq!(s.codigo, 2);
    assert!(s.err.contains("--pfx"), "{}", s.err);
    let pfx = pfx_de_teste(&p);
    let s = rodar(
        &p,
        &["emitir", &pedido, "--pfx", &pfx, "--senha-env", "S"],
        None,
        &[("S", "teste")],
    );
    assert_eq!(s.codigo, 2);
    assert!(s.err.contains("--ambiente"), "{}", s.err);
}

#[test]
fn emitir_usa_ambiente_e_certificado_do_arquivo_e_a_senha_do_ambiente() {
    let p = Pasta::nova();
    let (ruim, campo) = dps_recusada();
    let pedido = p.escrever("pedido.json", ruim.to_string().as_bytes());
    let pfx = pfx_de_teste(&p);
    let cfg = p.escrever("c.toml", toml_com_pfx(&pfx, "").as_bytes());
    // so' --senha-env: certificado e ambiente vem do arquivo; para na recusa de regra (saida 3), antes da rede
    let s = rodar(
        &p,
        &["emitir", &pedido, "--config", &cfg, "--senha-env", "S"],
        None,
        &[("S", "teste")],
    );
    assert_eq!(s.codigo, 3, "{}", s.err);
    assert!(s.err.contains(&campo), "{}", s.err);
}

#[test]
fn precedencia_do_certificado_opcao_ambiente_arquivo() {
    let p = Pasta::nova();
    let (ruim, _) = dps_recusada();
    let pedido = p.escrever("pedido.json", ruim.to_string().as_bytes());
    let bom = pfx_de_teste(&p);
    let inexistente = p.texto("nao-existe.pfx");
    // arquivo aponta para um .pfx que nao existe
    let cfg = p.escrever("c.toml", toml_com_pfx(&inexistente, "").as_bytes());
    let base = [
        "emitir",
        pedido.as_str(),
        "--config",
        cfg.as_str(),
        "--senha-env",
        "S",
    ];
    let env = [("S", "teste")];
    assert_eq!(
        rodar(&p, &base, None, &env).codigo,
        2,
        "so' o arquivo: nao abre"
    );

    // a variavel vence o arquivo
    let env_pfx = [("S", "teste"), ("RUSTNF_PFX", bom.as_str())];
    assert_eq!(rodar(&p, &base, None, &env_pfx).codigo, 3);

    // a opcao vence a variavel
    let mut args = base.to_vec();
    args.extend(["--pfx", inexistente.as_str()]);
    assert_eq!(rodar(&p, &args, None, &env_pfx).codigo, 2);
}

#[test]
fn precedencia_do_ambiente_opcao_ambiente_arquivo() {
    let p = Pasta::nova();
    let (ruim, _) = dps_recusada();
    let pedido = p.escrever("pedido.json", ruim.to_string().as_bytes());
    let pfx = pfx_de_teste(&p);
    // o arquivo traz um ambiente invalido para a conferencia: o programa recusa antes de usar
    let cfg = p.escrever(
        "c.toml",
        toml_com_pfx(&pfx, "")
            .replace("restrita", "producao")
            .as_bytes(),
    );
    let base = [
        "emitir",
        pedido.as_str(),
        "--config",
        cfg.as_str(),
        "--senha-env",
        "S",
    ];
    let env = [("S", "teste")];
    // arquivo: producao; variavel: restrita; opcao: producao. Todas passam na conferencia e chegam a recusa (3),
    // entao a ordem se prova pelo `config mostrar`
    assert_eq!(rodar(&p, &base, None, &env).codigo, 3);
    let s = rodar(
        &p,
        &["config", "mostrar", "--json", "--config", &cfg],
        None,
        &[("RUSTNF_AMBIENTE", "restrita")],
    );
    let v: Value = serde_json::from_str(&s.out).unwrap();
    assert_eq!(
        v["config"]["sefin"]["ambiente"], "restrita",
        "ambiente > arquivo"
    );
    let s = rodar(
        &p,
        &["config", "mostrar", "--json", "--config", &cfg],
        None,
        &[],
    );
    let v: Value = serde_json::from_str(&s.out).unwrap();
    assert_eq!(
        v["config"]["sefin"]["ambiente"], "producao",
        "arquivo > padrao"
    );
    // variavel com valor que nao serve: erro de uso apontando a variavel, sem o valor
    let s = rodar(
        &p,
        &["config", "mostrar"],
        None,
        &[("RUSTNF_PORTA", "oito-mil")],
    );
    assert_eq!(s.codigo, 2);
    assert!(
        s.err.contains("RUSTNF_PORTA") && !s.err.contains("oito-mil"),
        "{}",
        s.err
    );
}

#[test]
fn a_senha_nao_vai_por_argumento_nem_nos_comandos_de_senha() {
    let p = Pasta::nova();
    let s = rodar(
        &p,
        &["senha", "guardar", "--senha", "segredo123"],
        None,
        &[],
    );
    assert_eq!(s.codigo, 2);
    for sub in [["senha", "guardar"], ["senha", "apagar"]] {
        let mut args = sub.to_vec();
        args.push("--help");
        let ajuda = rodar(&p, &args, None, &[]);
        assert_eq!(ajuda.codigo, 0, "{}", ajuda.err);
        assert!(
            !ajuda.out.contains("--senha <") && !ajuda.out.contains("--senha,"),
            "{}",
            ajuda.out
        );
    }
    // sem origem de senha, com o cofre desligado: o mesmo aviso de sempre
    let (ruim, _) = dps_recusada();
    let pedido = p.escrever("pedido.json", ruim.to_string().as_bytes());
    let pfx = pfx_de_teste(&p);
    let s = rodar(
        &p,
        &["emitir", &pedido, "--pfx", &pfx, "--ambiente", "restrita"],
        None,
        &[],
    );
    assert_eq!(s.codigo, 2);
    assert!(s.err.contains("--senha-env"), "{}", s.err);
}

#[test]
fn notas_listar_mostra_a_pasta_configurada() {
    let p = Pasta::nova();
    let pasta = p.caminho("minhas-notas");
    let notas = Notas::abrir(&pasta).unwrap();
    let ch = chave_dfe::montar_nfse(3550308, "11222333000181", 5, "2610", Some(1), 2).unwrap();
    notas
        .guardar(
            &Registro {
                chave: ch.clone(),
                numero: "5".into(),
                serie: "1".into(),
                numero_dps: "5".into(),
                emissao: "2026-10-02T10:00:00-03:00".into(),
                valor_servico: "150.00".into(),
                situacao: Situacao::Autorizada,
                tomador_documento: "52998224725".into(),
                tomador_nome: "Cliente".into(),
            },
            "<NFSe/>",
        )
        .unwrap();
    let pasta_txt = pasta.to_string_lossy().into_owned();

    // sem pasta configurada: erro de uso
    let s = rodar(&p, &["notas", "listar"], None, &[]);
    assert_eq!(s.codigo, 2);
    assert!(s.err.contains("[notas].pasta"), "{}", s.err);

    let s = rodar(
        &p,
        &["notas", "listar", "--json"],
        None,
        &[("RUSTNF_NOTAS_PASTA", &pasta_txt)],
    );
    assert_eq!(s.codigo, 0, "{}", s.err);
    let v: Value = serde_json::from_str(&s.out).unwrap();
    assert_eq!(v[0]["chave"], ch.as_str());
    assert_eq!(v[0]["situacao"], "autorizada");
    assert_eq!(v[0]["tomador_nome"], "Cliente");

    let cfg = p.escrever(
        "c.toml",
        format!("[notas]\npasta = '{pasta_txt}'\n").as_bytes(),
    );
    let s = rodar(&p, &["notas", "listar", "--config", &cfg], None, &[]);
    assert_eq!(s.codigo, 0, "{}", s.err);
    assert!(
        s.out.contains("2026-10-02") && s.out.contains(&ch) && s.out.contains("150.00"),
        "{}",
        s.out
    );
}
