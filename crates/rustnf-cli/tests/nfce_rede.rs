// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! `rustnf nfce` nos comandos que falam com a SEFAZ (por dentro do processo, com transporte falso) e nos que usam so' a
//! configuracao, o cofre e a pasta de notas (pelo binario de verdade, e pelo cofre em memoria). Nenhum teste chama a SEFAZ,
//! abre impressora ou toca o cofre do sistema.

mod comum;

use std::process::Command;

use chrono::{Duration, TimeZone, Utc};
use clap::Parser;
use comum::nfe_falso::*;
use comum::*;
use rustnf::nfe::sefaz::ClienteSefaz;
use rustnf_app::cofre::{Cofre, CofreEmMemoria};
use rustnf_app::config::Config;
use rustnf_app::nfe::{config_sefaz, Contexto};
use rustnf_app::notas::nfe::{NotasNfe, Situacao};
use rustnf_cli::cli::{executar_com, nfce_com, AcaoNfce, Cli, Comando};
use serde_json::{json, Value};

/// O valor do CSC dos testes: ficticio, so' para procurar vazamento.
const CSC_TESTE: &str = "ZZTESTECSC0123456789ABCDEF000000";

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
         municipio = \"Sao Paulo\"\ncep = \"01001000\"\n\
         [nfce]\nid_csc = \"1\"\n",
        pasta.caminho("notas").to_string_lossy()
    );
    pasta.escrever("rustnf.toml", texto.as_bytes());
    // a configuracao que a NFC-e enxerga: o ambiente e a serie de [nfce]
    let config = Config::de_texto(&texto).unwrap().como_nfce();
    let notas = NotasNfe::abrir_nfce(pasta.caminho("notas")).unwrap();
    let falso = FalsoNfe::novo(roteiro);
    let cliente = ClienteSefaz::com_transporte(config_sefaz(&config).unwrap(), falso.clone())
        .unwrap()
        .com_relogio(|| Utc.with_ymd_and_hms(2026, 10, 3, 12, 0, 0).unwrap())
        .com_dormir(|_| Box::pin(async {}));
    let pedido = json!({
        "emissao": "2026-10-03T09:00:00-03:00",
        "nat_op": "VENDA",
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
    /// Roda `rustnf nfce <args>` por dentro do processo; o relogio e' o do momento do envio mais `depois` segundos.
    async fn rodar(&self, args: &[&str], depois: i64) -> Result<(), rustnf_cli::erro::Erro> {
        let cert = a1();
        let ctx = Contexto::novo(&self.cliente, &cert, &self.config, Some(&self.notas)).com_agora(
            Utc.with_ymd_and_hms(2026, 10, 3, 12, 0, 0).unwrap() + Duration::seconds(depois),
        );
        nfce_com(&ctx, acao(args)).await
    }

    /// Roda `rustnf nfce <args>` pelo binario de verdade (so' os comandos sem rede).
    fn binario(&self, args: &[&str]) -> (i32, String, String) {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_rustnf"));
        isolar(&mut cmd, &self.pasta.0);
        let s = cmd
            .args(["--config", &self.pasta.texto("rustnf.toml"), "nfce"])
            .args(args)
            .output()
            .unwrap();
        (
            s.status.code().unwrap(),
            String::from_utf8_lossy(&s.stdout).into_owned(),
            String::from_utf8_lossy(&s.stderr).into_owned(),
        )
    }
}

fn acao(args: &[&str]) -> AcaoNfce {
    let mut v = vec!["rustnf", "nfce"];
    v.extend(args);
    match Cli::try_parse_from(v).unwrap().comando {
        Comando::Nfce { acao } => acao,
        outro => panic!("nao e' nfce: {outro:?}"),
    }
}

fn saida_de(r: Result<(), rustnf_cli::erro::Erro>) -> u8 {
    r.map_or_else(|e| e.codigo_saida(), |()| 0)
}

#[test]
fn a_definicao_da_linha_de_comando_e_coerente() {
    use clap::CommandFactory;
    Cli::command().debug_assert();
    let ok = |a: &[&str]| {
        let mut v = vec!["rustnf", "nfce"];
        v.extend(a);
        Cli::try_parse_from(v).is_ok()
    };
    // o CSC nunca vem por argumento; a senha tambem nao
    assert!(!ok(&["csc", "guardar", "--csc", "x"]));
    assert!(!ok(&["status", "--senha", "x"]));
    assert!(ok(&["csc", "guardar", "--csc-env", "VAR"]));
    // as combinacoes que nao fazem sentido
    assert!(!ok(&[
        "emitir",
        "p.json",
        "--offline",
        "--offline-se-necessario"
    ]));
    assert!(!ok(&["danfce", "a.xml"]));
    assert!(!ok(&["danfce", "a.xml", "--pdf", "a", "--escpos", "b"]));
    assert!(!ok(&["danfce", "a.xml", "--pdf", "a", "--largura", "70"]));
    assert!(ok(&[
        "danfce",
        "a.xml",
        "--escpos",
        "a.bin",
        "--largura",
        "58",
        "--colunas",
        "32"
    ]));
    assert!(ok(&[
        "pendentes",
        "--retomar",
        "--fechar-caixa",
        "--ambiente",
        "producao"
    ]));
}

#[tokio::test]
async fn emitir_autorizada_sai_0_a_nota_fica_na_pasta_da_nfce_e_o_danfce_vai_ao_lado() {
    let c = cenario(vec![(
        "NFeAutorizacao4",
        envio("100", "Autorizado o uso da NF-e", NPROT),
    )]);
    c.rodar(&["emitir", &c.pedido, "--json", "--danfce"], 0)
        .await
        .unwrap();
    let lista = c.notas.listar().unwrap();
    assert_eq!(lista.len(), 1);
    assert_eq!(
        (lista[0].situacao, lista[0].protocolo.as_str()),
        (Situacao::Autorizada, NPROT)
    );
    assert_eq!(&lista[0].chave[20..22], "65");
    assert_eq!(c.falso.restantes(), 0);
    // a pasta do 55 nem foi criada
    assert!(!c.pasta.caminho("notas/nfe").exists());
    let pdf = c.pasta.caminho(&format!(
        "notas/nfce/2026/10/{}-procNFe.pdf",
        lista[0].chave
    ));
    assert!(std::fs::read(pdf).unwrap().starts_with(b"%PDF"));
}

#[tokio::test]
async fn rejeitada_sai_3_sem_numero_gasto_e_sem_resposta_sai_4_e_o_retomar_fecha() {
    let c = cenario(vec![(
        "NFeAutorizacao4",
        envio("225", "Rejeicao: Falha no Schema XML do lote de NFe", ""),
    )]);
    let r = c.rodar(&["emitir", &c.pedido], 0).await;
    assert_eq!(saida_de(r), 3);
    assert!(c.notas.listar().unwrap().is_empty());
    assert_eq!(c.notas.proximo_numero("1").unwrap(), 1);

    c.falso.empilhar("NFeAutorizacao4", perdida());
    c.falso.empilhar("NFeConsultaProtocolo4", perdida());
    let r = c.rodar(&["emitir", &c.pedido], 0).await;
    assert_eq!(saida_de(r), 4);
    let chave = c.notas.listar().unwrap()[0].chave.clone();
    assert_eq!(c.notas.listar().unwrap()[0].situacao, Situacao::Pendente);

    c.falso.empilhar(
        "NFeConsultaProtocolo4",
        consulta("100", "Autorizado o uso da NF-e", NPROT),
    );
    c.rodar(&["pendentes", "--retomar", "--fechar-caixa"], 5)
        .await
        .unwrap();
    let lista = c.notas.listar().unwrap();
    assert_eq!((lista.len(), lista[0].chave.as_str()), (1, chave.as_str()));
    assert_eq!(lista[0].situacao, Situacao::Autorizada);
}

#[tokio::test]
async fn off_line_grava_pendente_o_fechar_caixa_sai_4_e_a_retomada_zera() {
    let c = cenario(vec![]);
    // sem pendente, fechar o caixa sai 0
    let (codigo, out, err) = c.binario(&["pendentes", "--fechar-caixa"]);
    assert_eq!(codigo, 0, "{err}");
    assert!(out.contains("nenhuma NFC-e pendente"));

    c.rodar(&["emitir", &c.pedido, "--offline", "--danfce"], 0)
        .await
        .unwrap();
    let r = c.notas.listar().unwrap();
    assert!(r[0].offline && r[0].prazo.is_some());
    assert_eq!(c.falso.restantes(), 0);

    // com pendente: sai 4, diz a acao e o prazo; sem a opcao, so' lista (0)
    let (codigo, out, err) = c.binario(&["pendentes", "--fechar-caixa"]);
    assert_eq!(codigo, 4, "{out}{err}");
    assert!(
        out.contains("!! NFC-e 1") && out.contains("VENCEU"),
        "{out}"
    );
    assert!(err.contains("rustnf nfce pendentes --retomar"), "{err}");
    let (codigo, _, _) = c.binario(&["pendentes"]);
    assert_eq!(codigo, 0);
    let (_, json, _) = c.binario(&["pendentes", "--json"]);
    let v: Value = serde_json::from_str(&json).unwrap();
    assert_eq!(
        (v[0]["offline"].clone(), v[0]["prazo"].clone()),
        (json!(true), json!("vencido"))
    );

    // a retomada transmite a MESMA nota; depois o caixa fecha em 0
    c.falso.empilhar(
        "NFeConsultaProtocolo4",
        consulta(
            "217",
            "Rejeicao: NF-e nao consta na base de dados da SEFAZ",
            "",
        ),
    );
    c.falso.empilhar(
        "NFeAutorizacao4",
        envio("100", "Autorizado o uso da NF-e", NPROT),
    );
    c.rodar(&["pendentes", "--retomar", "--fechar-caixa"], 5)
        .await
        .unwrap();
    assert_eq!(c.notas.listar().unwrap()[0].situacao, Situacao::Autorizada);
    let (codigo, _, err) = c.binario(&["pendentes", "--fechar-caixa"]);
    assert_eq!(codigo, 0, "{err}");
}

#[tokio::test]
async fn danfce_pelo_binario_gera_pdf_e_escpos_com_bytes_estaveis() {
    let c = cenario(vec![(
        "NFeAutorizacao4",
        envio("100", "Autorizado o uso da NF-e", NPROT),
    )]);
    c.rodar(&["emitir", &c.pedido], 0).await.unwrap();
    let chave = c.notas.listar().unwrap()[0].chave.clone();
    let pdf = c.pasta.texto("cupom.pdf");
    let (codigo, _, err) = c.binario(&["danfce", &chave, "--pdf", &pdf]);
    assert_eq!(codigo, 0, "{err}");
    let a = std::fs::read(&pdf).unwrap();
    assert!(a.starts_with(b"%PDF"));
    let (codigo, _, err) = c.binario(&["danfce", &chave, "--pdf", &pdf]);
    assert_eq!(codigo, 0, "{err}");
    assert_eq!(
        a,
        std::fs::read(&pdf).unwrap(),
        "o mesmo cupom, os mesmos bytes"
    );
    // ESC/POS, da chave e do XML da pasta, e na bobina de 58 mm
    let bin = c.pasta.texto("cupom.bin");
    let (codigo, _, err) = c.binario(&[
        "danfce",
        &chave,
        "--escpos",
        &bin,
        "--colunas",
        "32",
        "--largura",
        "58",
    ]);
    assert_eq!(codigo, 0, "{err}");
    let escpos = std::fs::read(&bin).unwrap();
    assert!(escpos.starts_with(&[0x1b, 0x40]), "ESC @ no comeco");
    let xml = c
        .pasta
        .texto(&format!("notas/nfce/2026/10/{chave}-procNFe.xml"));
    let bin2 = c.pasta.texto("cupom2.bin");
    let (codigo, _, err) = c.binario(&[
        "danfce",
        &xml,
        "--escpos",
        &bin2,
        "--colunas",
        "32",
        "--largura",
        "58",
    ]);
    assert_eq!(codigo, 0, "{err}");
    assert_eq!(escpos, std::fs::read(&bin2).unwrap());
    // chave que nao esta na pasta, e NF-e (55) que nao tem cupom
    let (codigo, _, _) = c.binario(&["danfce", &"1".repeat(44), "--pdf", &pdf]);
    assert_ne!(codigo, 0);
}

#[tokio::test]
async fn cancelar_registra_o_evento_e_o_status_sai_com_o_codigo_certo() {
    let c = cenario(vec![(
        "NFeAutorizacao4",
        envio("100", "Autorizado o uso da NF-e", NPROT),
    )]);
    c.rodar(&["emitir", &c.pedido], 0).await.unwrap();
    let chave = c.notas.listar().unwrap()[0].chave.clone();
    c.falso.empilhar(
        "NFeRecepcaoEvento4",
        evento(
            "135",
            "Evento registrado e vinculado a NF-e",
            "135260000000099",
        ),
    );
    // 30 horas depois: o aviso do prazo nacional sai no stderr e o pedido segue
    c.rodar(
        &[
            "cancelar",
            "--chave",
            &chave,
            "--justificativa",
            "Cupom emitido em duplicidade",
        ],
        30 * 3600,
    )
    .await
    .unwrap();
    assert_eq!(c.notas.listar().unwrap()[0].situacao, Situacao::Cancelada);

    c.falso
        .empilhar("NFeStatusServico4", status("107", "Servico em Operacao"));
    assert_eq!(saida_de(c.rodar(&["status"], 0).await), 0);
    c.falso.empilhar(
        "NFeStatusServico4",
        status("108", "Servico Paralisado Momentaneamente"),
    );
    assert_eq!(saida_de(c.rodar(&["status"], 0).await), 4);
}

async fn csc_cli(
    cfg: &str,
    cofre: &CofreEmMemoria,
    args: &[&str],
) -> Result<(), rustnf_cli::erro::Erro> {
    let mut v = vec!["rustnf", "--config", cfg, "nfce", "csc"];
    v.extend(args);
    executar_com(Cli::try_parse_from(v).unwrap(), cofre).await
}

#[tokio::test]
async fn csc_guardar_testar_apagar_com_o_cofre_em_memoria_sem_mostrar_o_valor() {
    let p = Pasta::nova();
    let texto = "[emitente]
cnpj = \"11222333000181\"
[nfce]
qr_versao = 2
id_csc = \"1\"
";
    let cfg = p.escrever("rustnf.toml", texto.as_bytes());
    let cofre = CofreEmMemoria::novo();
    std::env::set_var("RUSTNF_TESTE_CSC", CSC_TESTE);
    // sem CSC: testar recusa (3) e diz como guardar
    let erro = csc_cli(&cfg, &cofre, &["testar"]).await.unwrap_err();
    assert_eq!(erro.codigo_saida(), 3);
    assert!(erro.to_string().contains("nfce csc guardar"), "{erro}");
    csc_cli(&cfg, &cofre, &["guardar", "--csc-env", "RUSTNF_TESTE_CSC"])
        .await
        .unwrap();
    csc_cli(&cfg, &cofre, &["testar"]).await.unwrap();
    // o item do cofre esta sob a chave propria e o valor so' esta la
    let chave = "csc:11222333000181:homologacao:1";
    assert_eq!(cofre.ler(chave).unwrap().unwrap().expor(), CSC_TESTE);
    // CSC torto e' recusado sem ser repetido
    std::env::set_var("RUSTNF_TESTE_CSC_RUIM", "curto-demais!");
    let erro = csc_cli(
        &cfg,
        &cofre,
        &["guardar", "--csc-env", "RUSTNF_TESTE_CSC_RUIM"],
    )
    .await
    .unwrap_err();
    assert!(!erro.to_string().contains("curto-demais"), "{erro}");
    // outro ambiente nao enxerga o CSC deste
    let outro = csc_cli(&cfg, &cofre, &["testar", "--ambiente", "producao"]).await;
    assert_eq!(outro.unwrap_err().codigo_saida(), 3);
    csc_cli(&cfg, &cofre, &["apagar"]).await.unwrap();
    assert!(cofre.ler(chave).unwrap().is_none());
}

#[test]
fn csc_pelo_binario_com_o_cofre_desligado_diz_o_que_fazer_e_nao_vaza_o_valor() {
    let c = cenario(vec![]);
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_rustnf"));
    isolar(&mut cmd, &c.pasta.0);
    let s = cmd
        .env("RUSTNF_TESTE_CSC", CSC_TESTE)
        .args([
            "--config",
            &c.pasta.texto("rustnf.toml"),
            "nfce",
            "csc",
            "guardar",
            "--csc-env",
            "RUSTNF_TESTE_CSC",
        ])
        .output()
        .unwrap();
    assert_eq!(s.status.code(), Some(2));
    let tudo = format!(
        "{}{}",
        String::from_utf8_lossy(&s.stdout),
        String::from_utf8_lossy(&s.stderr)
    );
    assert!(tudo.contains("cofre"), "{tudo}");
    assert!(!tudo.contains(CSC_TESTE));
}
