// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::ffi::OsString;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::Duration as Dias;
use rustnf::nfse::sefin::{ClienteNfse, FalhaRede, Transporte};
use rustnf_app::cofre::{Cofre, CofreEmMemoria};

use super::*;
use crate::testes_comuns::{servicos_de_teste, PastaTemporaria};

const SENHA: &str = "senha-do-teste-123";
const CNPJ: &str = "11222333000181";

type Resposta = Result<(u16, Vec<u8>), FalhaRede>;

/// Transporte falso: uma resposta fixa para qualquer pedido, e a lista do que foi pedido.
#[derive(Clone)]
struct Falso {
    resposta: Arc<Mutex<Resposta>>,
    pedidos: Arc<Mutex<Vec<String>>>,
}

#[async_trait]
impl Transporte for Falso {
    async fn enviar(
        &self,
        metodo: &str,
        url: &str,
        _corpo: Option<Vec<u8>>,
        _aceita: &str,
    ) -> Result<(u16, Vec<u8>), FalhaRede> {
        self.pedidos.lock().unwrap().push(format!("{metodo} {url}"));
        self.resposta.lock().unwrap().clone()
    }
}

/// A conexao dos testes: o mesmo `ClienteNfse::testar` da biblioteca, com o transporte falso.
struct ConexaoFalsa(Falso);

impl Conexao for ConexaoFalsa {
    fn testar(&self, preparo: Preparo) -> FuturoConexao {
        let transporte = self.0.clone();
        Box::pin(async move {
            let ambiente = preparo.ambiente;
            let cliente =
                ClienteNfse::com_transporte(preparo.into_certificado(), ambiente, None, transporte)
                    .unwrap();
            cliente.testar().await
        })
    }
}

fn conexao_falsa(resposta: Result<(u16, Vec<u8>), FalhaRede>) -> (Arc<ConexaoFalsa>, Falso) {
    let falso = Falso {
        resposta: Arc::new(Mutex::new(resposta)),
        pedidos: Arc::default(),
    };
    (Arc::new(ConexaoFalsa(falso.clone())), falso)
}

fn config_com_certificado(pfx: &std::path::Path) -> Config {
    let mut c = Config::default();
    c.emitente.cnpj = CNPJ.into();
    c.sefin.pfx = pfx.to_string_lossy().into_owned();
    c
}

fn rodar<T>(futuro: impl Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(futuro)
}

#[test]
fn arquivo_indicado_que_nao_existe_comeca_vazio_e_e_criado_ao_salvar() {
    let pasta = PastaTemporaria::nova("carga-novo");
    let arquivo = pasta.caminho().join("novo").join("rustnf.toml");
    let (s, _) = servicos_de_teste(&arquivo, Arc::new(ConexaoReal));
    let carga = s.carregar().unwrap();
    assert!(carga.arquivo_novo);
    assert_eq!(carga.arquivo, None);
    assert_eq!(carga.config, Config::default());
    assert_eq!(carga.ambiente, Ambiente::Restrita);
    let mut c = carga.config;
    c.emitente.nome = "EMPRESA".into();
    s.salvar(&c).unwrap();
    let lido = s.carregar().unwrap();
    assert!(!lido.arquivo_novo);
    assert_eq!(lido.config.emitente.nome, "EMPRESA");
    assert_eq!(lido.arquivo.as_deref(), Some(arquivo.as_path()));
}

#[test]
fn a_configuracao_vem_do_mesmo_arquivo_da_linha_de_comando_e_o_ambiente_em_vigor_vem_da_variavel() {
    let pasta = PastaTemporaria::nova("carga-env");
    let arquivo = pasta.caminho().join("rustnf.toml");
    std::fs::write(
        &arquivo,
        "[sefin]\nambiente = \"restrita\"\n[servidor]\nporta = 9001\n",
    )
    .unwrap();
    let cofre: Arc<dyn Cofre> = Arc::new(CofreEmMemoria::novo());
    let env: LerAmbiente =
        Arc::new(|nome| (nome == "RUSTNF_AMBIENTE").then(|| OsString::from("producao")));
    let s = Servicos::com(
        Some(arquivo.clone()),
        env,
        cofre,
        false,
        Arc::new(ConexaoReal),
        Arc::new(Utc::now),
    );
    let carga = s.carregar().unwrap();
    // o que a tela edita e grava e' o arquivo, nao o ambiente
    assert_eq!(carga.config.sefin.ambiente, "restrita");
    assert_eq!(carga.ambiente, Ambiente::Producao);
    assert_eq!(carga.variaveis, vec!["RUSTNF_AMBIENTE"]);
    assert_eq!(carga.config.servidor.porta, 9001);
    // e a gravacao nao leva o valor da variavel para o arquivo
    s.salvar(&carga.config).unwrap();
    assert_eq!(Config::ler(&arquivo).unwrap().sefin.ambiente, "restrita");
}

#[test]
fn arquivo_com_sintaxe_errada_e_erro_sem_o_valor_e_sem_o_caminho() {
    let pasta = PastaTemporaria::nova("carga-ruim");
    let arquivo = pasta.caminho().join("rustnf.toml");
    std::fs::write(&arquivo, "[sefin]\nsenha = \"abc-segredo\"\n").unwrap();
    let (s, _) = servicos_de_teste(&arquivo, Arc::new(ConexaoReal));
    let erro = s.carregar().unwrap_err();
    let texto = erro.texto();
    assert!(texto.contains("linha 2"), "{texto}");
    assert!(!texto.contains("abc-segredo"));
    assert!(!texto.contains("rustnf.toml"));
}

#[test]
fn arquivo_com_valor_invalido_ainda_abre_para_a_tela_mostrar_o_erro_no_campo() {
    let pasta = PastaTemporaria::nova("carga-invalido");
    let arquivo = pasta.caminho().join("rustnf.toml");
    std::fs::write(&arquivo, "[emitente]\nmunicipio = \"12\"\n").unwrap();
    let (s, _) = servicos_de_teste(&arquivo, Arc::new(ConexaoReal));
    let carga = s.carregar().unwrap();
    assert_eq!(carga.config.emitente.municipio, "12");
    // salvar recusa o valor e nao toca no arquivo
    let erro = s.salvar(&carga.config).unwrap_err();
    assert!(matches!(erro, ErroTela::Valor(_)), "{erro:?}");
    assert!(std::fs::read_to_string(&arquivo)
        .unwrap()
        .contains("\"12\""));
}

#[test]
fn sem_local_de_configuracao_salvar_diz_o_que_fazer() {
    let cofre: Arc<dyn Cofre> = Arc::new(CofreEmMemoria::novo());
    let s = Servicos::com(
        None,
        Arc::new(|_| None),
        cofre,
        false,
        Arc::new(ConexaoReal),
        Arc::new(Utc::now),
    );
    let erro = s.salvar(&Config::default()).unwrap_err();
    assert_eq!(erro, ErroTela::SemLocalDeConfiguracao);
    assert!(erro.texto().contains("--config"));
}

#[test]
fn guardar_senha_confere_que_abre_o_certificado_antes_de_gravar_no_cofre() {
    let pasta = PastaTemporaria::nova("guardar");
    let pfx = pasta.certificado("a.pfx", CNPJ, SENHA, 365);
    let (s, cofre) = servicos_de_teste(&pasta.caminho().join("c.toml"), Arc::new(ConexaoReal));
    let c = config_com_certificado(&pfx);

    let erro = s
        .guardar_senha(&c, &Segredo::novo("outra-senha".into()))
        .unwrap_err();
    assert_eq!(erro, ErroTela::SenhaIncorreta);
    assert_eq!(erro.campo(), Some(Campo::Senha));
    assert!(!erro.texto().contains("outra-senha"));
    assert!(
        cofre.ler(CNPJ).unwrap().is_none(),
        "senha errada nao pode ficar no cofre"
    );
    assert_eq!(s.senha_guardada(&c), Some(false));

    s.guardar_senha(&c, &Segredo::novo(SENHA.into())).unwrap();
    assert_eq!(cofre.ler(CNPJ).unwrap().unwrap().expor(), SENHA);
    assert_eq!(s.senha_guardada(&c), Some(true));

    s.apagar_senha(&c).unwrap();
    assert!(cofre.ler(CNPJ).unwrap().is_none());
    assert_eq!(s.senha_guardada(&c), Some(false));
    // apagar de novo nao e' erro
    s.apagar_senha(&c).unwrap();
}

#[test]
fn cofre_desligado_recusa_guardar_e_diz_que_a_senha_nao_sera_guardada() {
    let pasta = PastaTemporaria::nova("desligado");
    let pfx = pasta.certificado("a.pfx", CNPJ, SENHA, 365);
    let s = Servicos::com(
        None,
        Arc::new(|_| None),
        Arc::new(rustnf_app::cofre::CofreDesligado),
        true,
        Arc::new(ConexaoReal),
        Arc::new(Utc::now),
    );
    assert!(s.cofre_desligado());
    let c = config_com_certificado(&pfx);
    let erro = s
        .guardar_senha(&c, &Segredo::novo(SENHA.into()))
        .unwrap_err();
    assert_eq!(erro, ErroTela::Cofre(ErroCofre::Desligado));
    assert!(erro.texto().contains("não será guardada"));
    // com a senha digitada o teste do certificado funciona sem cofre
    assert!(s
        .testar_certificado(&c, Some(&Segredo::novo(SENHA.into())))
        .is_ok());
    // sem senha digitada e sem cofre: pede a senha
    assert_eq!(
        s.testar_certificado(&c, None).unwrap_err(),
        ErroTela::SemSenha
    );
    assert_eq!(s.senha_guardada(&c), None);
    assert_eq!(
        s.estado_do_certificado(&c),
        EstadoCertificado::SenhaNaoGuardada
    );
}

#[test]
fn guardar_precisa_de_chave_e_de_senha() {
    let pasta = PastaTemporaria::nova("chave");
    let (s, _) = servicos_de_teste(&pasta.caminho().join("c.toml"), Arc::new(ConexaoReal));
    let erro = s
        .guardar_senha(&Config::default(), &Segredo::novo(SENHA.into()))
        .unwrap_err();
    assert_eq!(erro, ErroTela::SemChaveDoCofre);
    let pfx = pasta.certificado("a.pfx", CNPJ, SENHA, 365);
    let c = config_com_certificado(&pfx);
    assert_eq!(
        s.guardar_senha(&c, &Segredo::novo(String::new()))
            .unwrap_err(),
        ErroTela::SemSenha
    );
}

#[test]
fn o_estado_do_rodape_percorre_todos_os_casos() {
    let pasta = PastaTemporaria::nova("estado");
    let (s, cofre) = servicos_de_teste(&pasta.caminho().join("c.toml"), Arc::new(ConexaoReal));
    // sem certificado
    assert_eq!(
        s.estado_do_certificado(&Config::default()),
        EstadoCertificado::SemCertificado
    );
    // arquivo que nao existe
    let mut c = Config::default();
    c.emitente.cnpj = CNPJ.into();
    c.sefin.pfx = pasta
        .caminho()
        .join("falta.pfx")
        .to_string_lossy()
        .into_owned();
    assert_eq!(
        s.estado_do_certificado(&c),
        EstadoCertificado::ArquivoNaoEncontrado
    );
    // existe, mas sem senha no cofre
    let pfx = pasta.certificado("a.pfx", CNPJ, SENHA, 365);
    let c = config_com_certificado(&pfx);
    assert_eq!(
        s.estado_do_certificado(&c),
        EstadoCertificado::SenhaNaoGuardada
    );
    // senha errada no cofre
    cofre
        .guardar(CNPJ, &Segredo::novo("errada".into()))
        .unwrap();
    assert_eq!(
        s.estado_do_certificado(&c),
        EstadoCertificado::SenhaNaoConfere
    );
    // senha certa: valido
    cofre.guardar(CNPJ, &Segredo::novo(SENHA.into())).unwrap();
    match s.estado_do_certificado(&c) {
        EstadoCertificado::Valido { dias, .. } => assert!((363..=365).contains(&dias)),
        outro => panic!("esperava Valido, veio {outro:?}"),
    }
    // vence em breve
    let curto = pasta.certificado("b.pfx", CNPJ, SENHA, 12);
    let c = config_com_certificado(&curto);
    assert!(matches!(
        s.estado_do_certificado(&c),
        EstadoCertificado::VenceEmBreve { .. }
    ));
    // vencido, com o relogio adiantado
    let cofre2: Arc<dyn Cofre> = cofre.clone();
    let adiante = Servicos::com(
        None,
        Arc::new(|_| None),
        cofre2,
        false,
        Arc::new(ConexaoReal),
        Arc::new(|| Utc::now() + Dias::days(400)),
    );
    let pfx = pasta.certificado("a.pfx", CNPJ, SENHA, 365);
    assert!(matches!(
        adiante.estado_do_certificado(&config_com_certificado(&pfx)),
        EstadoCertificado::Vencido { .. }
    ));
    // arquivo que nao e' certificado
    let lixo = pasta.caminho().join("lixo.pfx");
    std::fs::write(&lixo, b"isto nao e' um certificado").unwrap();
    assert_eq!(
        s.estado_do_certificado(&config_com_certificado(&lixo)),
        EstadoCertificado::Ilegivel
    );
}

#[test]
fn testar_certificado_mostra_titular_documento_e_validade_sem_caminho_nem_senha() {
    let pasta = PastaTemporaria::nova("testar");
    let pfx = pasta.certificado("a.pfx", CNPJ, SENHA, 365);
    let (s, cofre) = servicos_de_teste(&pasta.caminho().join("c.toml"), Arc::new(ConexaoReal));
    let c = config_com_certificado(&pfx);
    // sem senha digitada nem no cofre
    let erro = s.testar_certificado(&c, None).unwrap_err();
    assert_eq!(erro, ErroTela::SemSenha);
    // com a senha digitada
    let info = s
        .testar_certificado(&c, Some(&Segredo::novo(SENHA.into())))
        .unwrap();
    assert!(info.apto);
    assert!(info.titular.contains("EMPRESA DE TESTE"));
    assert_eq!(info.documento, "11.222.333/0001-81");
    assert_eq!(info.validade.len(), 10);
    let tudo = format!("{info:?}");
    assert!(!tudo.contains(SENHA));
    assert!(!tudo.contains("a.pfx"));
    // com a senha do cofre
    cofre.guardar(CNPJ, &Segredo::novo(SENHA.into())).unwrap();
    assert!(s.testar_certificado(&c, None).unwrap().apto);
    // senha digitada errada
    let erro = s
        .testar_certificado(&c, Some(&Segredo::novo("x".into())))
        .unwrap_err();
    assert_eq!(erro, ErroTela::SenhaIncorreta);
    // arquivo ausente
    let mut ausente = c.clone();
    ausente.sefin.pfx = pasta
        .caminho()
        .join("falta.pfx")
        .to_string_lossy()
        .into_owned();
    let erro = s
        .testar_certificado(&ausente, Some(&Segredo::novo(SENHA.into())))
        .unwrap_err();
    assert_eq!(erro.campo(), Some(Campo::Pfx));
    assert!(erro.texto().contains("não encontrado") || erro.texto().contains("nao encontrado"));
    assert!(!erro.texto().contains("falta.pfx"));
    // sem certificado escolhido
    assert_eq!(
        s.testar_certificado(&Config::default(), None).unwrap_err(),
        ErroTela::SemCertificado
    );
}

#[test]
fn testar_conexao_so_fala_com_o_transporte_dado_e_diz_ok_para_resposta_de_negocio() {
    let pasta = PastaTemporaria::nova("conexao");
    let pfx = pasta.certificado("a.pfx", CNPJ, SENHA, 365);
    let (conexao, falso) = conexao_falsa(Ok((
        404,
        br#"{"erro":{"codigo":"E404","descricao":"nao achado"}}"#.to_vec(),
    )));
    let (s, _) = servicos_de_teste(&pasta.caminho().join("c.toml"), conexao);
    let mut c = config_com_certificado(&pfx);
    c.sefin.ambiente = "producao".into();
    let preparo = s
        .preparar_conexao(&c, Some(&Segredo::novo(SENHA.into())))
        .unwrap();
    assert_eq!(preparo.ambiente, 1);
    assert_eq!(preparo.algoritmo, Algoritmo::Sha256);
    assert_eq!(preparo.tempo_limite, Duration::from_secs(60));
    let (ok, texto) = rodar(s.conexao().testar(preparo));
    assert!(ok, "{texto}");
    assert!(texto.contains("Conexao com o SEFIN ok"), "{texto}");
    let pedidos = falso.pedidos.lock().unwrap().clone();
    assert_eq!(pedidos.len(), 1, "{pedidos:?}");
    assert!(pedidos[0].starts_with("GET https://sefin.nfse.gov.br/sefinnacional/nfse/"));
    assert!(!texto.contains(SENHA));
}

#[test]
fn testar_conexao_acusa_recusa_do_certificado_e_falta_de_rede() {
    let pasta = PastaTemporaria::nova("conexao-ruim");
    let pfx = pasta.certificado("a.pfx", CNPJ, SENHA, 365);
    let c = config_com_certificado(&pfx);
    let senha = Segredo::novo(SENHA.into());
    for (resposta, esperado) in [
        (Ok((401, b"{}".to_vec())), "recusou o certificado"),
        (Ok((502, b"<html>".to_vec())), "Resposta inesperada"),
        (Err(FalhaRede::Rede("sem rota".into())), "Sem conexao"),
    ] {
        let (conexao, _) = conexao_falsa(resposta);
        let (s, _) = servicos_de_teste(&pasta.caminho().join("c.toml"), conexao);
        let preparo = s.preparar_conexao(&c, Some(&senha)).unwrap();
        let (ok, texto) = rodar(s.conexao().testar(preparo));
        assert!(!ok);
        assert!(texto.contains(esperado), "{texto}");
    }
}

#[test]
fn preparar_conexao_sem_certificado_ou_com_senha_errada_nao_chega_a_rede() {
    let pasta = PastaTemporaria::nova("preparo");
    let (conexao, falso) = conexao_falsa(Ok((200, b"{}".to_vec())));
    let (s, _) = servicos_de_teste(&pasta.caminho().join("c.toml"), conexao);
    assert!(matches!(
        s.preparar_conexao(&Config::default(), None),
        Err(ErroTela::SemCertificado)
    ));
    let pfx = pasta.certificado("a.pfx", CNPJ, SENHA, 365);
    let c = config_com_certificado(&pfx);
    assert!(matches!(
        s.preparar_conexao(&c, Some(&Segredo::novo("errada".into()))),
        Err(ErroTela::SenhaIncorreta)
    ));
    assert!(falso.pedidos.lock().unwrap().is_empty());
}

#[test]
fn pasta_de_notas_e_criada_ou_recusada() {
    let pasta = PastaTemporaria::nova("notas");
    let (s, _) = servicos_de_teste(&pasta.caminho().join("c.toml"), Arc::new(ConexaoReal));
    let nova = pasta.caminho().join("a").join("b");
    s.conferir_pasta(&nova.to_string_lossy()).unwrap();
    assert!(nova.is_dir());
    // um arquivo no lugar da pasta
    let arquivo = pasta.caminho().join("arquivo.txt");
    std::fs::write(&arquivo, b"x").unwrap();
    let erro = s.conferir_pasta(&arquivo.to_string_lossy()).unwrap_err();
    assert_eq!(erro, ErroTela::Pasta);
    assert_eq!(erro.campo(), Some(Campo::NotasPasta));
    assert!(s.conferir_pasta("").is_err());
}

#[test]
fn nenhuma_mensagem_de_erro_leva_senha_nem_caminho() {
    let erros = [
        ErroTela::Configuracao("x".into()),
        ErroTela::SemLocalDeConfiguracao,
        ErroTela::Gravacao,
        ErroTela::SemCertificado,
        ErroTela::SemSenha,
        ErroTela::SenhaIncorreta,
        ErroTela::SemChaveDoCofre,
        ErroTela::Pasta,
        ErroTela::Cofre(ErroCofre::Indisponivel),
        ErroTela::Cofre(ErroCofre::Desligado),
        ErroTela::Cofre(ErroCofre::NaoSuportado),
        ErroTela::Cofre(ErroCofre::ChaveInvalida),
    ];
    for e in erros {
        let t = e.texto();
        assert!(!t.contains(":\\") && !t.contains(".pfx"), "{t}");
        assert!(!t.is_empty());
    }
}
