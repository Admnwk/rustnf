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

/// Uma nota do indice com outra data, outro tomador e outra situacao.
fn nota_da_pasta(n: u64, emissao: &str, nome: &str, situacao: Situacao) -> Registro {
    Registro {
        chave: chave_dfe::montar_nfse(3550308, "11222333000181", n, "2610", Some(1), 2).unwrap(),
        numero: n.to_string(),
        serie: "1".into(),
        numero_dps: n.to_string(),
        emissao: emissao.into(),
        valor_servico: "150.00".into(),
        situacao,
        tomador_documento: "52998224725".into(),
        tomador_nome: nome.into(),
    }
}

/// Pasta de notas com tres notas (setembro, outubro cancelada e novembro) e o arquivo de configuracao que a usa.
fn pasta_com_tres_notas(p: &Pasta) -> (Notas, String, Vec<Registro>) {
    let pasta = p.caminho("notas");
    let notas = Notas::abrir(&pasta).unwrap();
    let registros = vec![
        nota_da_pasta(
            1,
            "2026-09-30T10:00:00-03:00",
            "João da Silva",
            Situacao::Autorizada,
        ),
        nota_da_pasta(
            2,
            "2026-10-15T10:00:00-03:00",
            "Padaria Pão Quente",
            Situacao::Cancelada,
        ),
        nota_da_pasta(
            3,
            "2026-11-01T10:00:00-03:00",
            "Oficina; do Zé",
            Situacao::Autorizada,
        ),
    ];
    for r in &registros {
        notas
            .guardar(r, &format!("<NFSe>{}</NFSe>", r.numero))
            .unwrap();
    }
    notas.marcar_cancelada(&registros[1].chave).unwrap();
    let cfg = p.escrever(
        "notas.toml",
        format!("[notas]\npasta = '{}'\n", pasta.to_string_lossy()).as_bytes(),
    );
    (notas, cfg, registros)
}

#[test]
fn notas_listar_filtra_por_periodo_situacao_e_busca() {
    let p = Pasta::nova();
    let (_, cfg, reg) = pasta_com_tres_notas(&p);
    let numeros = |extra: &[&str]| -> Vec<String> {
        let mut args = vec!["notas", "listar", "--json", "--config", &cfg];
        args.extend(extra);
        let s = rodar(&p, &args, None, &[]);
        assert_eq!(s.codigo, 0, "{}", s.err);
        let v: Value = serde_json::from_str(&s.out).unwrap();
        v.as_array()
            .unwrap()
            .iter()
            .map(|n| n["numero"].as_str().unwrap().to_string())
            .collect()
    };
    assert_eq!(
        numeros(&[]),
        ["3", "2", "1"],
        "da mais recente para a mais antiga"
    );
    assert_eq!(numeros(&["--de", "2026-10-01"]), ["3", "2"]);
    assert_eq!(
        numeros(&["--de", "2026-10-01", "--ate", "2026-10-31"]),
        ["2"]
    );
    assert_eq!(numeros(&["--situacao", "cancelada"]), ["2"]);
    assert_eq!(
        numeros(&["--situacao", "autorizada", "--ate", "2026-10-31"]),
        ["1"]
    );
    assert_eq!(numeros(&["--busca", "pao quente"]), ["2"]);
    assert_eq!(numeros(&["--busca", &reg[0].chave]), ["1"]);
    assert!(numeros(&["--busca", "ninguem"]).is_empty());

    // data que nao e' um dia: erro de entrada, sem listar nada
    let s = rodar(
        &p,
        &["notas", "listar", "--config", &cfg, "--de", "01/10/2026"],
        None,
        &[],
    );
    assert_eq!(s.codigo, 2, "{}", s.err);
    assert!(
        s.err.contains("AAAA-MM-DD") && s.out.is_empty(),
        "{} {}",
        s.err,
        s.out
    );
    // --json e --csv nao vao juntos
    let s = rodar(
        &p,
        &["notas", "listar", "--config", &cfg, "--json", "--csv"],
        None,
        &[],
    );
    assert_eq!(s.codigo, 2, "{}", s.err);
}

#[test]
fn notas_listar_em_csv_para_planilha() {
    let p = Pasta::nova();
    let (_, cfg, reg) = pasta_com_tres_notas(&p);
    let s = rodar(
        &p,
        &[
            "notas",
            "listar",
            "--csv",
            "--config",
            &cfg,
            "--de",
            "2026-10-01",
        ],
        None,
        &[],
    );
    assert_eq!(s.codigo, 0, "{}", s.err);
    let linhas: Vec<&str> = s.out.split("\r\n").collect();
    assert_eq!(
        linhas[0],
        "emissao;numero;serie;numero_dps;valor_servico;situacao;tomador_documento;tomador_nome;chave"
    );
    assert_eq!(
        linhas[1],
        format!(
            "2026-11-01;3;1;3;150,00;autorizada;52998224725;\"Oficina; do Zé\";{}",
            reg[2].chave
        )
    );
    assert!(
        linhas[2].starts_with("2026-10-15;2;1;2;150,00;cancelada;"),
        "{}",
        linhas[2]
    );
    assert_eq!(linhas.len(), 4, "{linhas:?}");
}

#[test]
fn notas_exportar_copia_os_xml_do_mes_e_a_lista() {
    let p = Pasta::nova();
    let (notas, cfg, reg) = pasta_com_tres_notas(&p);
    let destino = p.caminho("para o contador");
    let destino_txt = destino.to_string_lossy().into_owned();

    let s = rodar(
        &p,
        &[
            "notas",
            "exportar",
            "--mes",
            "2026-10",
            "--para",
            &destino_txt,
            "--config",
            &cfg,
        ],
        None,
        &[],
    );
    assert_eq!(s.codigo, 0, "{}", s.err);
    assert!(
        s.out.contains("1 nota(s)") && s.out.contains("1 XML"),
        "{}",
        s.out
    );
    assert_eq!(
        std::fs::read_to_string(destino.join(format!("{}.xml", reg[1].chave))).unwrap(),
        "<NFSe>2</NFSe>"
    );
    let lista = std::fs::read_to_string(destino.join("notas.csv")).unwrap();
    assert!(
        lista.contains(";cancelada;") && lista.contains(&reg[1].chave),
        "{lista}"
    );
    assert_eq!(
        std::fs::read_dir(&destino).unwrap().count(),
        2,
        "so' a nota de outubro e a lista"
    );
    // a pasta de notas fica como estava
    assert_eq!(notas.listar().unwrap().len(), 3);

    // mes sem nota: nada e' criado
    let vazio = p.caminho("vazio");
    let vazio_txt = vazio.to_string_lossy().into_owned();
    let s = rodar(
        &p,
        &[
            "notas", "exportar", "--mes", "2025-01", "--para", &vazio_txt, "--config", &cfg,
        ],
        None,
        &[],
    );
    assert_eq!(s.codigo, 0, "{}", s.err);
    assert!(
        s.err.contains("nenhuma nota") && !vazio.exists(),
        "{}",
        s.err
    );
    // mes mal escrito: erro de entrada
    let s = rodar(
        &p,
        &[
            "notas",
            "exportar",
            "--mes",
            "10/2026",
            "--para",
            &destino_txt,
            "--config",
            &cfg,
        ],
        None,
        &[],
    );
    assert_eq!(s.codigo, 2, "{}", s.err);
}

#[test]
fn notas_numero_mostra_e_continua_a_numeracao_de_outro_sistema() {
    let p = Pasta::nova();
    let pasta = p.caminho("notas");
    let cfg = p.escrever(
        "notas.toml",
        format!(
            "[emitente]\nserie = '3'\n[notas]\npasta = '{}'\n",
            pasta.to_string_lossy()
        )
        .as_bytes(),
    );
    let s = rodar(&p, &["notas", "numero", "--config", &cfg], None, &[]);
    assert_eq!(s.codigo, 0, "{}", s.err);
    assert_eq!(s.out.trim(), "serie 3: proxima DPS 1");
    // a ultima DPS do outro sistema foi a 1500
    let s = rodar(
        &p,
        &["notas", "numero", "--proximo", "1501", "--config", &cfg],
        None,
        &[],
    );
    assert_eq!(s.codigo, 0, "{}", s.err);
    assert_eq!(s.out.trim(), "serie 3: proxima DPS 1501");
    let notas = Notas::abrir(&pasta).unwrap();
    assert_eq!(notas.proximo_numero("3").unwrap(), 1501);
    assert_eq!(notas.proximo_numero("1").unwrap(), 1);
    // outra serie, pela opcao
    let s = rodar(
        &p,
        &[
            "notas",
            "numero",
            "--serie",
            "1",
            "--proximo",
            "20",
            "--config",
            &cfg,
        ],
        None,
        &[],
    );
    assert_eq!(s.codigo, 0, "{}", s.err);
    assert_eq!(notas.proximo_numero("1").unwrap(), 20);
    // voltar atras e' erro de entrada, e nada muda
    let s = rodar(
        &p,
        &["notas", "numero", "--proximo", "100", "--config", &cfg],
        None,
        &[],
    );
    assert_eq!(s.codigo, 2, "{}", s.out);
    assert!(s.err.contains("ja foi usado"), "{}", s.err);
    assert_eq!(notas.proximo_numero("3").unwrap(), 1501);
}

#[test]
fn notas_reindexar_refaz_o_indice_ilegivel() {
    let p = Pasta::nova();
    let pasta = p.caminho("notas");
    let notas = Notas::abrir(&pasta).unwrap();
    let ch = guardar_nota_do_danfse(&notas);
    let antes = notas.listar().unwrap();
    let cfg = p.escrever(
        "notas.toml",
        format!("[notas]\npasta = '{}'\n", pasta.to_string_lossy()).as_bytes(),
    );
    std::fs::write(pasta.join("indice.json"), "{ quebrado").unwrap();
    let s = rodar(&p, &["notas", "listar", "--config", &cfg], None, &[]);
    assert_eq!(s.codigo, 2, "o indice ilegivel e' acusado: {}", s.err);

    let s = rodar(&p, &["notas", "reindexar", "--config", &cfg], None, &[]);
    assert_eq!(s.codigo, 0, "{}", s.err);
    assert!(s.out.contains("indice refeito: 1 nota(s)"), "{}", s.out);
    assert!(
        s.err.contains("ilegivel") && s.err.contains("indice.anterior.json"),
        "{}",
        s.err
    );
    assert_eq!(notas.listar().unwrap(), antes);
    assert_eq!(notas.listar().unwrap()[0].chave, ch);
    assert_eq!(
        std::fs::read_to_string(pasta.join("indice.anterior.json")).unwrap(),
        "{ quebrado"
    );
}

#[test]
fn danfse_pela_chave_sai_da_pasta_de_notas() {
    let p = Pasta::nova();
    let pasta = p.caminho("notas");
    let notas = Notas::abrir(&pasta).unwrap();
    let ch = guardar_nota_do_danfse(&notas);
    let cfg = p.escrever(
        "notas.toml",
        format!("[notas]\npasta = '{}'\n", pasta.to_string_lossy()).as_bytes(),
    );

    // sem -o: o PDF fica ao lado do XML e o caminho sai no stdout; a chave pode vir em grupos
    let em_grupos = ch
        .as_bytes()
        .chunks(5)
        .map(|g| String::from_utf8_lossy(g).into_owned())
        .collect::<Vec<_>>()
        .join(" ");
    let s = rodar(
        &p,
        &["danfse", "--chave", &em_grupos, "--config", &cfg],
        None,
        &[],
    );
    assert_eq!(s.codigo, 0, "{}", s.err);
    let pdf = notas
        .caminho_do_xml_guardado(&ch)
        .unwrap()
        .with_extension("pdf");
    assert_eq!(s.out.trim(), pdf.to_string_lossy());
    let valida = std::fs::read(&pdf).unwrap();
    assert!(valida.starts_with(b"%PDF-"));
    assert_eq!(
        notas.listar().unwrap().len(),
        1,
        "o PDF nao entra no indice"
    );

    // nota cancelada na pasta: o PDF sai com a marca, sem pedir --situacao; -o escolhe o arquivo
    notas.marcar_cancelada(&ch).unwrap();
    let fora = p.texto("cancelada.pdf");
    let s = rodar(
        &p,
        &["danfse", "--chave", &ch, "-o", &fora, "--config", &cfg],
        None,
        &[],
    );
    assert_eq!(s.codigo, 0, "{}", s.err);
    let cancelada = std::fs::read(&fora).unwrap();
    assert!(cancelada.starts_with(b"%PDF-") && cancelada != valida);
    // --situacao dita vale sobre a da pasta: sai o mesmo PDF da nota valida
    let de_novo = p.texto("valida.pdf");
    let s = rodar(
        &p,
        &[
            "danfse",
            "--chave",
            &ch,
            "-o",
            &de_novo,
            "--situacao",
            "valida",
            "--config",
            &cfg,
        ],
        None,
        &[],
    );
    assert_eq!(s.codigo, 0, "{}", s.err);
    assert_eq!(std::fs::read(&de_novo).unwrap(), valida);

    // chave que nao esta na pasta, chave invalida, sem pasta de notas, XML e chave juntos
    let outra = chave_dfe::montar_nfse(3550308, "11222333000181", 8, "2610", Some(1), 2).unwrap();
    let s = rodar(
        &p,
        &["danfse", "--chave", &outra, "--config", &cfg],
        None,
        &[],
    );
    assert_ne!(s.codigo, 0);
    assert!(s.err.contains("nao encontrada"), "{}", s.err);
    let s = rodar(
        &p,
        &["danfse", "--chave", "../../x", "--config", &cfg],
        None,
        &[],
    );
    assert_eq!(s.codigo, 2, "{}", s.err);
    let s = rodar(&p, &["danfse", "--chave", &ch], None, &[]);
    assert_eq!(s.codigo, 2, "{}", s.err);
    assert!(s.err.contains("[notas].pasta"), "{}", s.err);
    let s = rodar(
        &p,
        &["danfse", "nota.xml", "--chave", &ch, "--config", &cfg],
        None,
        &[],
    );
    assert_eq!(s.codigo, 2, "{}", s.err);
    // sem XML e sem chave, e XML sem -o: erro de uso
    assert_eq!(rodar(&p, &["danfse"], None, &[]).codigo, 2);
    assert_eq!(rodar(&p, &["danfse", "nota.xml"], None, &[]).codigo, 2);
}

#[test]
fn certificado_mostrar_diz_quem_e_e_ate_quando_vale() {
    let p = Pasta::nova();
    let pfx = pfx_de_teste(&p);
    let cfg = p.escrever("c.toml", toml_com_pfx(&pfx, "").as_bytes());
    let env = [("SENHA_A1", "teste")];

    let s = rodar(
        &p,
        &[
            "certificado",
            "mostrar",
            "--config",
            &cfg,
            "--senha-env",
            "SENHA_A1",
        ],
        None,
        &env,
    );
    assert_eq!(s.codigo, 0, "{}", s.err);
    for trecho in [
        "Certificado OK: OFICINA TESTE",
        "titular: OFICINA TESTE",
        "documento: 11222333000181",
        "(ICP-Brasil)",
        "validade: de 20",
        " dias)",
    ] {
        assert!(s.out.contains(trecho), "faltou '{trecho}' em {}", s.out);
    }
    assert!(!s.out.contains("teste.pfx") && !s.err.contains("teste.pfx"));

    // em JSON, e com o aviso quando o certificado e' de outro documento que o do emitente
    let outro = p.escrever(
        "outro.toml",
        toml_com_pfx(&pfx, "[emitente]\ncnpj = \"12ABC34501DE35\"\n").as_bytes(),
    );
    let s = rodar(
        &p,
        &[
            "certificado",
            "mostrar",
            "--json",
            "--config",
            &outro,
            "--senha-stdin",
        ],
        Some("teste\n"),
        &[],
    );
    assert_eq!(s.codigo, 0, "{}", s.err);
    let v: Value = serde_json::from_str(&s.out).unwrap();
    assert_eq!(v["ok"], true);
    assert_eq!(v["situacao"], "valido");
    assert_eq!(v["documento"], "11222333000181");
    assert!(v["dias_restantes"].as_i64().unwrap() >= 364, "{v}");
    assert!(v["avisos"].to_string().contains("E0718"), "{v}");

    // senha errada: erro de entrada, sem ecoar a senha
    let s = rodar(
        &p,
        &[
            "certificado",
            "mostrar",
            "--config",
            &cfg,
            "--senha-env",
            "SENHA_A1",
        ],
        None,
        &[("SENHA_A1", "senha-errada")],
    );
    assert_eq!(s.codigo, 2, "{}", s.err);
    assert!(!s.err.contains("senha-errada"), "{}", s.err);
    // sem certificado configurado: erro de uso
    let s = rodar(
        &p,
        &["certificado", "mostrar", "--senha-env", "SENHA_A1"],
        None,
        &env,
    );
    assert_eq!(s.codigo, 2, "{}", s.err);
    assert!(s.err.contains("--pfx"), "{}", s.err);
}

#[test]
fn certificado_vencido_sai_com_3_no_mostrar_e_no_teste_da_sefin_sem_ir_a_rede() {
    let p = Pasta::nova();
    let pfx = pfx_vencido(&p);
    let cfg = p.escrever("c.toml", toml_com_pfx(&pfx, "").as_bytes());
    let env = [("SENHA_A1", "teste")];

    let s = rodar(
        &p,
        &[
            "certificado",
            "mostrar",
            "--json",
            "--config",
            &cfg,
            "--senha-env",
            "SENHA_A1",
        ],
        None,
        &env,
    );
    assert_eq!(s.codigo, 3, "{}", s.err);
    let v: Value = serde_json::from_str(&s.out).unwrap();
    assert_eq!(v["ok"], false);
    assert_eq!(v["situacao"], "vencido");
    assert!(v["dias_restantes"].as_i64().unwrap() < 0, "{v}");
    assert!(s.err.contains("VENCIDO"), "{}", s.err);

    // o teste da Sefin para no certificado vencido: nenhuma chamada sai
    let s = rodar(
        &p,
        &[
            "sefin",
            "testar",
            "--config",
            &cfg,
            "--senha-env",
            "SENHA_A1",
        ],
        None,
        &env,
    );
    assert_eq!(s.codigo, 3, "{}", s.err);
    assert!(
        s.err.contains("VENCIDO") && s.out.is_empty(),
        "{} {}",
        s.err,
        s.out
    );
    // e sem ambiente definido nem chega a abrir o certificado
    let s = rodar(
        &p,
        &["sefin", "testar", "--pfx", &pfx, "--senha-env", "SENHA_A1"],
        None,
        &env,
    );
    assert_eq!(s.codigo, 2, "{}", s.err);
    assert!(s.err.contains("--ambiente"), "{}", s.err);
}
