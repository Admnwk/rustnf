// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! `rustnf nfe recebidas`: a definicao da linha de comando, o codigo de saida de cada desfecho com transporte falso (por
//! dentro do processo) e `listar`/`cursor` pelo binario de verdade. Nenhum teste chama o Ambiente Nacional.

mod comum;

use std::process::Command;

use chrono::{TimeZone, Utc};
use clap::Parser;
use comum::dfe_falso::*;
use comum::nfe_falso::*;
use comum::*;
use rustnf::nfe::sefaz::ClienteSefaz;
use rustnf_app::config::Config;
use rustnf_app::nfe::{config_sefaz, Contexto};
use rustnf_app::notas::nfe::NotasNfe;
use rustnf_cli::cli::{nfe_com, Cli, Comando};
use rustnf_cli::erro::Erro;

struct Cenario {
    pasta: Pasta,
    config: Config,
    notas: NotasNfe,
    cliente: ClienteSefaz<FalsoNfe>,
    falso: FalsoNfe,
}

fn cenario(roteiro: Vec<(&'static str, Resposta)>) -> Cenario {
    let pasta = Pasta::nova();
    let texto = format!(
        "[emitente]\ncnpj = \"11222333000181\"\nnome = \"LOJA DE TESTE LTDA\"\nmunicipio = \"3550308\"\n\
         [notas]\npasta = '{}'\n[nfe]\nuf = \"SP\"\nie = \"111111111111\"\ncrt = 1\n\
         [nfe.endereco]\nlogradouro = \"Rua das Flores\"\nnumero = \"100\"\nbairro = \"Centro\"\n\
         municipio = \"Sao Paulo\"\ncep = \"01001000\"\n",
        pasta.caminho("notas").to_string_lossy()
    );
    pasta.escrever("rustnf.toml", texto.as_bytes());
    let config = Config::de_texto(&texto).unwrap();
    let notas = NotasNfe::abrir(pasta.caminho("notas")).unwrap();
    let falso = FalsoNfe::novo(roteiro);
    let cliente = ClienteSefaz::com_transporte(config_sefaz(&config).unwrap(), falso.clone())
        .unwrap()
        .com_relogio(|| Utc.with_ymd_and_hms(2026, 10, 3, 12, 0, 0).unwrap())
        .com_dormir(|_| Box::pin(async {}));
    Cenario {
        pasta,
        config,
        notas,
        cliente,
        falso,
    }
}

impl Cenario {
    /// Roda `rustnf nfe recebidas <args>` por dentro do processo.
    async fn rodar(&self, args: &[&str]) -> Result<(), Erro> {
        let cert = a1();
        let ctx = Contexto::novo(&self.cliente, &cert, &self.config, Some(&self.notas))
            .com_agora(Utc.with_ymd_and_hms(2026, 10, 3, 12, 0, 0).unwrap());
        let mut v = vec!["rustnf", "nfe", "recebidas"];
        v.extend(args);
        match Cli::try_parse_from(v).unwrap().comando {
            Comando::Nfe { acao } => nfe_com(&ctx, acao).await,
            outro => panic!("nao e' nfe: {outro:?}"),
        }
    }

    /// Roda o binario de verdade nos comandos que so' leem a pasta.
    fn binario(&self, args: &[&str]) -> (i32, String, String) {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_rustnf"));
        isolar(&mut cmd, &self.pasta.0);
        let cfg = self.pasta.texto("rustnf.toml");
        let s = cmd
            .args(["--config", &cfg, "nfe", "recebidas"])
            .args(args)
            .output()
            .unwrap();
        (
            s.status.code().unwrap(),
            String::from_utf8_lossy(&s.stdout).into_owned(),
            String::from_utf8_lossy(&s.stderr).into_owned(),
        )
    }

    fn codigo(r: &Result<(), Erro>) -> u8 {
        r.as_ref().err().map_or(0, Erro::codigo_saida)
    }
}

#[test]
fn a_definicao_da_linha_de_comando_e_coerente() {
    use clap::CommandFactory;
    Cli::command().debug_assert();
    let ok = |a: &[&str]| {
        let mut v = vec!["rustnf", "nfe", "recebidas"];
        v.extend(a);
        Cli::try_parse_from(v).is_ok()
    };
    assert!(ok(&["sincronizar", "--lotes", "3", "--json"]));
    assert!(ok(&[
        "listar",
        "--situacao",
        "nao-realizada",
        "--busca",
        "loja"
    ]));
    assert!(ok(&["baixar", "--chave", "1", "--ciencia"]));
    assert!(ok(&[
        "manifestar",
        "--chave",
        "1",
        "--tipo",
        "nao-realizada",
        "--justificativa",
        "x"
    ]));
    assert!(ok(&["cursor", "--json"]));
    // sem o tipo, ou com um que nao existe, ou com a senha por argumento, recusa
    assert!(!ok(&["manifestar", "--chave", "1"]));
    assert!(!ok(&["manifestar", "--chave", "1", "--tipo", "talvez"]));
    assert!(!ok(&["sincronizar", "--senha", "x"]));
    assert!(!ok(&["baixar"]));
}

#[test]
fn o_completar_cobre_os_comandos_novos() {
    let p = Pasta::nova();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_rustnf"));
    isolar(&mut cmd, &p.0);
    let s = cmd.args(["completar", "bash"]).output().unwrap();
    assert_eq!(s.status.code(), Some(0));
    let texto = String::from_utf8_lossy(&s.stdout);
    for palavra in [
        "recebidas",
        "sincronizar",
        "manifestar",
        "nao-realizada",
        "ciencia",
    ] {
        assert!(texto.contains(palavra), "falta {palavra} no completar");
    }
}

#[tokio::test]
async fn sincronizar_sai_0_grava_e_antes_da_hora_nao_chama_a_rede() {
    let c = cenario(vec![]);
    let (proc, chave) = nota_recebida(&c.config, 1);
    c.falso.empilhar(
        DIST,
        lote(2, 2, &[doc_resumo(1, &chave), doc_completa(2, &proc)]),
    );
    let r = c.rodar(&["sincronizar", "--json"]).await;
    assert_eq!(Cenario::codigo(&r), 0, "{r:?}");
    assert_eq!(c.falso.restantes(), 0);
    assert_eq!(c.notas.recebidas().unwrap().listar().unwrap().len(), 1);

    // dentro da hora de espera nada sai: se saisse, o transporte falso derrubaria o teste
    let r = c.rodar(&["sincronizar"]).await;
    assert_eq!(Cenario::codigo(&r), 0, "esperar nao e' falha: {r:?}");
}

#[tokio::test]
async fn o_656_sai_3_e_a_falta_de_resposta_sai_4() {
    let c = cenario(vec![(DIST, bloqueio(0))]);
    let r = c.rodar(&["sincronizar"]).await;
    assert_eq!(Cenario::codigo(&r), 3, "{r:?}");
    assert!(
        matches!(&r, Err(Erro::Recusa { mensagem, .. }) if mensagem.contains("656")),
        "{r:?}"
    );

    let c = cenario(vec![(DIST, perdida())]);
    let r = c.rodar(&["sincronizar"]).await;
    assert_eq!(Cenario::codigo(&r), 4, "{r:?}");
    assert_eq!(
        c.notas
            .recebidas()
            .unwrap()
            .cursor("homologacao", "11222333000181")
            .unwrap()
            .ult_nsu
            .valor(),
        0,
        "sem resposta nao anda o cursor"
    );
}

#[tokio::test]
async fn manifestar_registra_o_evento_e_a_recusa_antes_da_rede_sai_3() {
    let c = cenario(vec![]);
    let (_, chave) = nota_recebida(&c.config, 1);
    c.falso.empilhar(DIST, lote(1, 1, &[doc_resumo(1, &chave)]));
    c.rodar(&["sincronizar"]).await.unwrap();

    c.falso.empilhar(
        "NFeRecepcaoEvento4",
        evento("135", "Evento registrado e vinculado a NF-e", NPROT),
    );
    let r = c
        .rodar(&[
            "manifestar",
            "--chave",
            &chave,
            "--tipo",
            "ciencia",
            "--json",
        ])
        .await;
    assert_eq!(Cenario::codigo(&r), 0, "{r:?}");
    let linha = c.notas.recebidas().unwrap().registro(&chave).unwrap();
    assert_eq!(linha.situacao.texto(), "ciencia");

    // nao realizada sem justificativa: recusada antes de qualquer envio
    let r = c
        .rodar(&["manifestar", "--chave", &chave, "--tipo", "nao-realizada"])
        .await;
    assert_eq!(Cenario::codigo(&r), 3, "{r:?}");
    assert_eq!(c.falso.restantes(), 0);
}

#[tokio::test]
async fn baixar_so_com_resumo_sai_3_e_diz_o_que_fazer() {
    let c = cenario(vec![]);
    let (_, chave) = nota_recebida(&c.config, 1);
    c.falso.empilhar(DIST, lote(1, 1, &[doc_resumo(1, &chave)]));
    let r = c.rodar(&["baixar", "--chave", &chave]).await;
    assert_eq!(Cenario::codigo(&r), 3, "{r:?}");
    assert!(
        matches!(&r, Err(Erro::Recusa { mensagem, .. }) if mensagem.contains("--ciencia")),
        "{r:?}"
    );
}

#[tokio::test]
async fn listar_e_cursor_pelo_binario_so_leem_a_pasta() {
    let c = cenario(vec![]);
    let (proc, chave) = nota_recebida(&c.config, 1);
    c.falso
        .empilhar(DIST, lote(1, 1, &[doc_completa(1, &proc)]));
    c.rodar(&["sincronizar"]).await.unwrap();

    let (codigo, out, err) = c.binario(&["listar"]);
    assert_eq!(codigo, 0, "{err}");
    assert!(out.contains(&chave) && out.contains("completa"), "{out}");
    assert!(
        out.starts_with("2026-10-03\t31.80\tcompleta\tLOJA DE TESTE LTDA"),
        "{out}"
    );

    let (codigo, out, _) = c.binario(&["listar", "--json", "--situacao", "resumo"]);
    assert_eq!(codigo, 0);
    assert_eq!(out.trim(), "[]");

    let (codigo, out, err) = c.binario(&["cursor"]);
    assert_eq!(codigo, 0, "{err}");
    assert!(out.contains("ultimo NSU: 000000000000001"), "{out}");
    let (_, json, _) = c.binario(&["cursor", "--json"]);
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v["ult_nsu"], "000000000000001");
    assert_eq!(v["ambiente"], "homologacao");
}
