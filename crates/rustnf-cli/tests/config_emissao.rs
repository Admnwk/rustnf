// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A emissao com a configuracao (completa o pedido com o `[emitente]`, numera e guarda a nota pela pasta de notas) e o
//! cofre de senhas, por dentro do processo, com transporte falso e cofre em memoria. Nenhum teste chama a Sefin
//! Nacional, toca o cofre real do sistema ou le a pasta de configuracao do usuario.

mod comum;

use std::path::Path;

use clap::Parser;
use comum::*;
use rustnf::chave_dfe;
use rustnf::nfse::{compactar, descompactar};
use rustnf_app::cofre::{Cofre, CofreEmMemoria};
use rustnf_app::config::{Config, Efetiva};
use rustnf_app::notas::Notas;
use rustnf_cli::cli::{emitir_valor, executar_com, Cli};
use serde_json::{json, Value};
fn id_dps(numero: u64) -> String {
    format!(
        "DPS{}{}{}{:05}{:015}",
        "3550308", "2", "11222333000181", 1, numero
    )
}

/// Resposta de emissao autorizada com a nota inteira (data de processamento e do tomador).
fn autorizada_completa(numero: u64) -> (String, Resp) {
    let ch =
        chave_dfe::montar_nfse(3550308, "11222333000181", 70 + numero, "2610", Some(1), 2).unwrap();
    let xml = format!(
        "<NFSe xmlns=\"http://www.sped.fazenda.gov.br/nfse\" versao=\"1.01\"><infNFSe Id=\"NFS{ch}\">\
         <nNFSe>{}</nNFSe><dhProc>2026-10-02T10:00:01-03:00</dhProc><cStat>100</cStat><ambGer>2</ambGer>\
         <valores><vLiq>150.00</vLiq></valores><DPS versao=\"1.01\"><infDPS Id=\"{}\"><tpAmb>2</tpAmb>\
         <serie>1</serie><nDPS>{numero}</nDPS><dCompet>2026-10-01</dCompet>\
         <toma><CNPJ>11222333000181</CNPJ><xNome>Cliente</xNome></toma>\
         <valores><vServPrest><vServ>150.00</vServ></vServPrest></valores></infDPS></DPS></infNFSe></NFSe>",
        70 + numero,
        id_dps(numero)
    );
    let resp = json_resp(
        201,
        json!({"chaveAcesso": ch, "idDps": id_dps(numero), "nfseXmlGZipB64": compactar(&xml)}),
    );
    (ch, resp)
}

/// O pedido valido sem nada do que o `[emitente]` completa.
fn pedido_sem_emitente() -> Value {
    let mut v = dps_valida();
    v["prestador"] = json!({});
    for campo in ["cmun_emissor", "numero"] {
        v.as_object_mut().unwrap().remove(campo);
    }
    v
}

fn efetiva_com_notas(pasta: &Path) -> Efetiva {
    let mut c = Config::ler(&exemplo()).unwrap();
    c.notas.pasta = pasta.to_string_lossy().into_owned();
    Efetiva::de(c, None, &|_| None).unwrap()
}

#[tokio::test]
async fn emitir_completa_o_emitente_numera_e_guarda_a_nota() {
    let p = Pasta::nova();
    let pasta = p.caminho("notas");
    let efetiva = efetiva_com_notas(&pasta);
    // a pasta ja tem as notas ate a 41: a proxima DPS e' a 42
    Notas::abrir(&pasta)
        .unwrap()
        .confirmar_numero("1", 41)
        .unwrap();
    let (ch1, r1) = autorizada_completa(42);
    let (ch2, r2) = autorizada_completa(43);
    let falso = Falso::novo(vec![
        ("POST", "/nfse".into(), r1),
        ("POST", "/nfse".into(), r2),
    ]);
    let emissor = emissor_falso(falso.clone());

    let v = emitir_valor(emissor.as_ref(), pedido_sem_emitente(), &efetiva, None)
        .await
        .unwrap();
    assert_eq!(v["status"], "autorizada", "{v}");
    assert_eq!(v["guardada"], true, "{v}");
    assert_eq!(v["numero_dps"], 42);

    // o que foi enviado a Sefin tem os dados do emitente e o numero da pasta
    let corpo: Value = serde_json::from_slice(&falso.corpos.lock().unwrap()[0]).unwrap();
    let xml = descompactar(corpo["dpsXmlGZipB64"].as_str().unwrap()).unwrap();
    for trecho in [
        "<CNPJ>11222333000181</CNPJ>",
        "<nDPS>42</nDPS>",
        "<cLocEmi>3550308</cLocEmi>",
        "<IM>123456</IM>",
        "<serie>1</serie>",
    ] {
        assert!(xml.contains(trecho), "faltou {trecho} em {xml}");
    }

    // a nota ficou na pasta, e a numeracao avancou
    let notas = Notas::abrir(&pasta).unwrap();
    assert!(pasta
        .join("2026")
        .join("10")
        .join(format!("{ch1}.xml"))
        .is_file());
    assert!(notas.ler_xml(&ch1).unwrap().contains("<nNFSe>112</nNFSe>"));
    assert_eq!(notas.proximo_numero("1").unwrap(), 43);
    let lista = notas.listar().unwrap();
    assert_eq!(lista.len(), 1);
    assert_eq!(lista[0].valor_servico, "150.00");
    assert_eq!(lista[0].tomador_nome, "Cliente");

    // a segunda emissao pega o 43
    let v = emitir_valor(emissor.as_ref(), pedido_sem_emitente(), &efetiva, None)
        .await
        .unwrap();
    assert_eq!(v["numero_dps"], 43, "{v}");
    assert_eq!(notas.proximo_numero("1").unwrap(), 44);
    assert_eq!(notas.listar().unwrap().len(), 2);
    assert!(notas.ler_xml(&ch2).is_ok());
}

#[tokio::test]
async fn emitir_com_o_numero_no_pedido_nao_usa_a_pasta_e_rejeitada_nao_avanca() {
    let p = Pasta::nova();
    let pasta = p.caminho("notas");
    let efetiva = efetiva_com_notas(&pasta);
    let falso = Falso::novo(vec![(
        "POST",
        "/nfse".into(),
        json_resp(
            400,
            json!({"erro": [{"codigo": "E0001", "descricao": "recusada de proposito"}]}),
        ),
    )]);
    let emissor = emissor_falso(falso);
    let mut pedido = pedido_sem_emitente();
    pedido["numero"] = json!(42);
    let erro = emitir_valor(emissor.as_ref(), pedido, &efetiva, None)
        .await
        .unwrap_err();
    assert_eq!(erro.codigo_saida(), 3, "{erro}");
    let notas = Notas::abrir(&pasta).unwrap();
    assert!(notas.listar().unwrap().is_empty());
    assert_eq!(
        notas.proximo_numero("1").unwrap(),
        1,
        "rejeitada nao avanca o numero"
    );
}

#[tokio::test]
async fn emitir_sem_pasta_de_notas_e_sem_numero_no_pedido_recusa_como_sempre() {
    let efetiva = Efetiva::de(Config::default(), None, &|_| None).unwrap();
    let emissor = emissor_falso(Falso::novo(vec![]));
    // sem [emitente] e sem numero: o pedido e' recusado pela montagem da DPS, sem chegar a rede
    let mut pedido = dps_valida();
    pedido.as_object_mut().unwrap().remove("numero");
    let erro = emitir_valor(emissor.as_ref(), pedido, &efetiva, None)
        .await
        .unwrap_err();
    assert_eq!(erro.codigo_saida(), 3, "{erro}");
}

fn cli(args: &[&str]) -> Cli {
    let mut todos = vec!["rustnf"];
    todos.extend(args);
    Cli::try_parse_from(todos).unwrap()
}

#[tokio::test]
async fn senha_guardar_confere_a_senha_grava_no_cofre_e_o_emitir_a_usa() {
    let p = Pasta::nova();
    let pfx = pfx_de_teste(&p);
    let cfg = p.escrever(
        "c.toml",
        toml_com_pfx(&pfx, "[emitente]\ncnpj = \"11222333000181\"\n").as_bytes(),
    );
    let cofre = CofreEmMemoria::novo();
    // variaveis proprias deste teste: nenhum outro teste as le
    std::env::set_var("RUSTNF_TESTE_SENHA_CERTA", "teste");
    std::env::set_var("RUSTNF_TESTE_SENHA_ERRADA", "errada-ultra-secreta");

    // senha errada: nao grava
    let erro = executar_com(
        cli(&[
            "senha",
            "guardar",
            "--config",
            &cfg,
            "--senha-env",
            "RUSTNF_TESTE_SENHA_ERRADA",
        ]),
        &cofre,
    )
    .await
    .unwrap_err();
    assert_eq!(erro.codigo_saida(), 2);
    assert!(!erro.to_string().contains("ultra-secreta"));
    assert!(cofre.ler("11222333000181").unwrap().is_none());

    // senha certa: grava com a chave do emitente
    executar_com(
        cli(&[
            "senha",
            "guardar",
            "--config",
            &cfg,
            "--senha-env",
            "RUSTNF_TESTE_SENHA_CERTA",
        ]),
        &cofre,
    )
    .await
    .unwrap();
    assert_eq!(
        cofre.ler("11222333000181").unwrap().unwrap().expor(),
        "teste"
    );

    // emitir sem --senha-env: a senha vem do cofre (o pedido recusado para na regra, saida 3, depois de abrir o
    // certificado com ela)
    let (ruim, campo) = dps_recusada();
    let pedido = p.escrever("pedido.json", ruim.to_string().as_bytes());
    let erro = executar_com(cli(&["emitir", &pedido, "--config", &cfg]), &cofre)
        .await
        .unwrap_err();
    assert_eq!(erro.codigo_saida(), 3, "{erro}");
    assert!(erro.to_string().contains(&campo));

    // apagar: o emitir volta a pedir a senha
    executar_com(cli(&["senha", "apagar", "--config", &cfg]), &cofre)
        .await
        .unwrap();
    assert!(cofre.ler("11222333000181").unwrap().is_none());
    let erro = executar_com(cli(&["emitir", &pedido, "--config", &cfg]), &cofre)
        .await
        .unwrap_err();
    assert_eq!(erro.codigo_saida(), 2);
    assert!(erro.to_string().contains("--senha-env"));
}
