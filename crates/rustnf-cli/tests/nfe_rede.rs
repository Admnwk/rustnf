// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! `rustnf nfe` nos comandos que falam com a SEFAZ, por dentro do processo e com transporte falso: o codigo de saida de cada
//! desfecho e o que fica na pasta de notas. O DANFE da nota emitida sai pelo binario de verdade. Nenhum teste chama a
//! SEFAZ.

mod comum;

use std::process::Command;

use chrono::{Duration, TimeZone, Utc};
use clap::Parser;
use comum::nfe_falso::*;
use comum::*;
use rustnf::nfe::sefaz::ClienteSefaz;
use rustnf_app::config::Config;
use rustnf_app::nfe::{config_sefaz, Contexto};
use rustnf_app::notas::nfe::{NotasNfe, Situacao};
use rustnf_cli::cli::{nfe_com, AcaoNfe, Cli, Comando};
use serde_json::json;

struct Cenario {
    pasta: Pasta,
    config: Config,
    notas: NotasNfe,
    cliente: ClienteSefaz<FalsoNfe>,
    falso: FalsoNfe,
    pedido: String,
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
    let pedido = json!({
        "emissao": "2026-10-03T09:00:00-03:00",
        "nat_op": "VENDA",
        "dest": {"documento": "52998224725", "nome": "CLIENTE TESTE", "ind_ie_dest": 9,
                 "endereco": {"logradouro": "Rua A", "numero": "1", "bairro": "Centro",
                              "cod_municipio": "3550308", "municipio": "Sao Paulo", "uf": "SP"}},
        "ind_final": 1, "ind_pres": 1,
        "itens": [{"codigo": "1", "descricao": "Cafe torrado 500g", "ncm": "09012100", "cfop": "5102",
                   "unidade": "UN", "quantidade": "2", "valor_unitario": "15.90",
                   "icms": {"codigo": "102"}, "pis": {"cst": "49"}, "cofins": {"cst": "49"}}],
        "pagamentos": [{"t_pag": "01", "valor": "31.80"}]
    });
    let pedido = pasta.escrever("pedido.json", pedido.to_string().as_bytes());
    Cenario {
        pasta,
        config,
        notas,
        cliente,
        falso,
        pedido,
    }
}

impl Cenario {
    /// Roda `rustnf nfe <args>` por dentro do processo; o relogio e' o do momento do envio mais `depois` segundos.
    async fn rodar(&self, args: &[&str], depois: i64) -> Result<(), rustnf_cli::erro::Erro> {
        let cert = a1();
        let ctx = Contexto::novo(&self.cliente, &cert, &self.config, Some(&self.notas)).com_agora(
            Utc.with_ymd_and_hms(2026, 10, 3, 12, 0, 0).unwrap() + Duration::seconds(depois),
        );
        nfe_com(&ctx, acao(args)).await
    }
}

fn acao(args: &[&str]) -> AcaoNfe {
    let mut v = vec!["rustnf", "nfe"];
    v.extend(args);
    match Cli::try_parse_from(v).unwrap().comando {
        Comando::Nfe { acao } => acao,
        outro => panic!("nao e' nfe: {outro:?}"),
    }
}

#[test]
fn a_definicao_da_linha_de_comando_e_coerente() {
    use clap::CommandFactory;
    Cli::command().debug_assert();
    // a senha nunca vem por argumento, e as opcoes que nao existem sao recusadas
    assert!(Cli::try_parse_from(["rustnf", "nfe", "status", "--senha", "x"]).is_err());
    assert!(Cli::try_parse_from(["rustnf", "nfe", "emitir"]).is_err());
    assert!(Cli::try_parse_from(["rustnf", "nfe", "danfe"]).is_err());
    assert!(Cli::try_parse_from(["rustnf", "nfe", "danfe", "a.xml", "--chave", "1"]).is_err());
    assert!(Cli::try_parse_from([
        "rustnf",
        "nfe",
        "emitir",
        "p.json",
        "--ambiente",
        "producao"
    ])
    .is_ok());
    assert!(Cli::try_parse_from([
        "rustnf",
        "nfe",
        "emitir",
        "p.json",
        "--ambiente",
        "restrita"
    ])
    .is_err());
}

#[tokio::test]
async fn emitir_autorizada_sai_0_e_a_nota_fica_na_pasta() {
    let c = cenario(vec![(
        "NFeAutorizacao4",
        envio("100", "Autorizado o uso da NF-e", NPROT),
    )]);
    c.rodar(&["emitir", &c.pedido, "--json"], 0).await.unwrap();
    let lista = c.notas.listar().unwrap();
    assert_eq!(lista.len(), 1);
    assert_eq!(
        (lista[0].situacao, lista[0].protocolo.as_str()),
        (Situacao::Autorizada, NPROT)
    );
    assert_eq!(c.falso.restantes(), 0);
}

#[tokio::test]
async fn emitir_rejeitada_sai_3_e_nao_deixa_nota_nem_gasta_numero() {
    let c = cenario(vec![(
        "NFeAutorizacao4",
        envio("225", "Rejeicao: Falha no Schema XML do lote de NFe", ""),
    )]);
    let erro = c.rodar(&["emitir", &c.pedido], 0).await.unwrap_err();
    assert_eq!(erro.codigo_saida(), 3);
    assert!(erro.to_string().contains("continua livre"), "{erro}");
    assert!(c.notas.listar().unwrap().is_empty());
    assert_eq!(c.notas.proximo_numero("1").unwrap(), 1);
}

#[tokio::test]
async fn emitir_denegada_sai_3_e_gasta_o_numero() {
    let c = cenario(vec![(
        "NFeAutorizacao4",
        envio(
            "302",
            "Uso Denegado: Irregularidade fiscal do destinatario",
            NPROT,
        ),
    )]);
    let erro = c.rodar(&["emitir", &c.pedido], 0).await.unwrap_err();
    assert_eq!(erro.codigo_saida(), 3);
    assert_eq!(c.notas.listar().unwrap()[0].situacao, Situacao::Denegada);
    assert_eq!(c.notas.proximo_numero("1").unwrap(), 2);
}

#[tokio::test]
async fn sem_resposta_sai_4_a_nota_fica_pendente_e_o_retomar_a_fecha() {
    let c = cenario(vec![
        ("NFeAutorizacao4", perdida()),
        ("NFeConsultaProtocolo4", perdida()),
    ]);
    let erro = c.rodar(&["emitir", &c.pedido], 0).await.unwrap_err();
    assert_eq!(erro.codigo_saida(), 4);
    assert!(erro.to_string().contains("PENDENTE"), "{erro}");
    let chave = c.notas.listar().unwrap()[0].chave.clone();
    assert_eq!(c.notas.listar().unwrap()[0].situacao, Situacao::Pendente);

    // retomar sem resposta de novo: continua pendente, saida 4
    c.falso.empilhar("NFeConsultaProtocolo4", perdida());
    let erro = c.rodar(&["pendentes", "--retomar"], 10).await.unwrap_err();
    assert_eq!(erro.codigo_saida(), 4, "{erro}");
    assert_eq!(c.notas.registro(&chave).unwrap().tentativas, 1);

    // a SEFAZ responde: a mesma chave fica autorizada, uma nota so'
    c.falso.empilhar(
        "NFeConsultaProtocolo4",
        consulta("100", "Autorizado o uso da NF-e", NPROT),
    );
    c.rodar(&["pendentes", "--retomar", "--json"], 20)
        .await
        .unwrap();
    let lista = c.notas.listar().unwrap();
    assert_eq!(
        (lista.len(), lista[0].chave.as_str(), lista[0].situacao),
        (1, chave.as_str(), Situacao::Autorizada)
    );
    // nada mais a retomar
    c.rodar(&["pendentes", "--retomar"], 30).await.unwrap();
}

#[tokio::test]
async fn cancelar_corrigir_e_inutilizar_registram_e_guardam() {
    let c = cenario(vec![
        (
            "NFeAutorizacao4",
            envio("100", "Autorizado o uso da NF-e", NPROT),
        ),
        (
            "NFeRecepcaoEvento4",
            evento(
                "135",
                "Evento registrado e vinculado a NF-e",
                "135260000000050",
            ),
        ),
        (
            "NFeRecepcaoEvento4",
            evento(
                "135",
                "Evento registrado e vinculado a NF-e",
                "135260000000051",
            ),
        ),
        (
            "NFeRecepcaoEvento4",
            evento(
                "135",
                "Evento registrado e vinculado a NF-e",
                "135260000000052",
            ),
        ),
        ("NFeInutilizacao4", inutilizacao("135260000000300")),
    ]);
    c.rodar(&["emitir", &c.pedido], 0).await.unwrap();
    let chave = c.notas.listar().unwrap()[0].chave.clone();
    for texto in [
        "Corrigir o complemento do endereco",
        "Corrigir o complemento do bairro",
    ] {
        c.rodar(&["corrigir", "--chave", &chave, "--texto", texto], 0)
            .await
            .unwrap();
    }
    assert_eq!(c.notas.proxima_sequencia(&chave, "110110").unwrap(), 3);
    c.rodar(
        &[
            "cancelar",
            "--chave",
            &chave,
            "--justificativa",
            "Cancelamento por erro de digitacao",
        ],
        0,
    )
    .await
    .unwrap();
    assert_eq!(
        c.notas.registro(&chave).unwrap().situacao,
        Situacao::Cancelada
    );
    c.rodar(
        &[
            "inutilizar",
            "--inicio",
            "2",
            "--fim",
            "3",
            "--justificativa",
            "Numeros pulados por falha do sistema",
        ],
        0,
    )
    .await
    .unwrap();
    assert_eq!(c.notas.inutilizacoes().unwrap().len(), 1);
    assert_eq!(c.notas.proximo_numero("1").unwrap(), 4);
    assert_eq!(c.falso.restantes(), 0);
}

#[tokio::test]
async fn cancelar_fora_do_prazo_sai_3_e_justificativa_curta_sai_3_sem_rede() {
    let c = cenario(vec![
        (
            "NFeAutorizacao4",
            envio("100", "Autorizado o uso da NF-e", NPROT),
        ),
        (
            "NFeRecepcaoEvento4",
            evento(
                "501",
                "Rejeicao: Prazo de Cancelamento superior ao previsto na Legislacao",
                "",
            ),
        ),
    ]);
    c.rodar(&["emitir", &c.pedido], 0).await.unwrap();
    let chave = c.notas.listar().unwrap()[0].chave.clone();
    let curta = c
        .rodar(
            &["cancelar", "--chave", &chave, "--justificativa", "curta"],
            0,
        )
        .await
        .unwrap_err();
    assert_eq!(curta.codigo_saida(), 3, "{curta}");
    let erro = c
        .rodar(
            &[
                "cancelar",
                "--chave",
                &chave,
                "--justificativa",
                "Cancelamento depois do prazo legal",
            ],
            0,
        )
        .await
        .unwrap_err();
    assert_eq!(erro.codigo_saida(), 3, "{erro}");
    assert_eq!(
        c.notas.registro(&chave).unwrap().situacao,
        Situacao::Autorizada
    );
}

#[tokio::test]
async fn consultar_e_status_com_os_codigos_de_saida() {
    let c = cenario(vec![
        ("NFeAutorizacao4", perdida()),
        ("NFeConsultaProtocolo4", perdida()),
        (
            "NFeConsultaProtocolo4",
            consulta("100", "Autorizado o uso da NF-e", NPROT),
        ),
        ("NFeStatusServico4", status("107", "Servico em Operacao")),
        (
            "NFeStatusServico4",
            status("108", "Servico Paralisado Momentaneamente"),
        ),
        ("NFeStatusServico4", perdida()),
    ]);
    c.rodar(&["emitir", &c.pedido], 0).await.unwrap_err();
    let chave = c.notas.listar().unwrap()[0].chave.clone();
    c.rodar(&["consultar", "--chave", &chave], 0).await.unwrap();
    assert_eq!(
        c.notas.registro(&chave).unwrap().situacao,
        Situacao::Autorizada
    );
    c.rodar(&["status"], 0).await.unwrap();
    assert_eq!(c.rodar(&["status"], 0).await.unwrap_err().codigo_saida(), 4);
    assert_eq!(
        c.rodar(&["status", "--json"], 0)
            .await
            .unwrap_err()
            .codigo_saida(),
        4
    );
}

#[tokio::test]
async fn o_danfe_da_nota_emitida_sai_pelo_binario_com_e_sem_saida() {
    let c = cenario(vec![(
        "NFeAutorizacao4",
        envio("100", "Autorizado o uso da NF-e", NPROT),
    )]);
    c.rodar(&["emitir", &c.pedido], 0).await.unwrap();
    let chave = c.notas.listar().unwrap()[0].chave.clone();
    let cfg = c.pasta.texto("rustnf.toml");
    let rodar = |args: &[&str]| {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_rustnf"));
        isolar(&mut cmd, &c.pasta.0);
        let s = cmd
            .args(["--config", &cfg, "nfe", "danfe"])
            .args(args)
            .output()
            .unwrap();
        (
            s.status.code().unwrap(),
            String::from_utf8_lossy(&s.stdout).into_owned(),
            String::from_utf8_lossy(&s.stderr).into_owned(),
        )
    };
    // sem -o: o PDF fica ao lado do XML e o caminho sai no stdout
    let (codigo, out, err) = rodar(&["--chave", &chave]);
    assert_eq!(codigo, 0, "{err}");
    let caminho = std::path::PathBuf::from(out.trim());
    assert!(
        caminho
            .to_string_lossy()
            .ends_with(&format!("{chave}-procNFe.pdf")),
        "{out}"
    );
    assert!(std::fs::read(&caminho).unwrap().starts_with(b"%PDF"));
    // com -o e sem canhoto
    let saida = c.pasta.texto("danfe.pdf");
    let (codigo, _, err) = rodar(&["--chave", &chave, "-o", &saida, "--sem-canhoto"]);
    assert_eq!(codigo, 0, "{err}");
    let sem_canhoto = std::fs::read(&saida).unwrap();
    assert!(sem_canhoto.starts_with(b"%PDF"));
    assert_ne!(sem_canhoto, std::fs::read(&caminho).unwrap());
    // pelo XML solto
    let xml = c.notas.caminho_do_proc(&chave).unwrap();
    let saida2 = c.pasta.texto("danfe2.pdf");
    let (codigo, _, err) = rodar(&[xml.to_str().unwrap(), "-o", &saida2]);
    assert_eq!(codigo, 0, "{err}");
    assert_eq!(
        std::fs::read(&saida2).unwrap(),
        std::fs::read(&caminho).unwrap()
    );
    // chave que nao esta na pasta, e XML que nao e' NF-e
    let (codigo, _, err) = rodar(&["--chave", &"1".repeat(44)]);
    assert_eq!(codigo, 2, "{err}");
    let lixo = c.pasta.escrever("lixo.xml", b"<x/>");
    let (codigo, _, _) = rodar(&[&lixo, "-o", &saida2]);
    assert_eq!(codigo, 2);
}
