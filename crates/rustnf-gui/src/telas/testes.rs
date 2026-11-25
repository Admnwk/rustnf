// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Testes da janela, no ambiente de teste do GPUI: abrem a janela de verdade (sem renderizador de imagem), clicam e
//! digitam pelos elementos, e conferem o estado e o arquivo gravado numa pasta temporaria. Nenhum toca o cofre do
//! sistema, a pasta de configuracao do usuario nem a rede.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    px, size, AppContext as _, Bounds, Entity, Point, TestAppContext, WindowBounds, WindowHandle,
    WindowOptions,
};
use rustnf::nfse::sefin::{ClienteNfse, FalhaRede, Transporte};
use rustnf_app::cofre::{Cofre, CofreEmMemoria};
use rustnf_app::config::Config;

use super::{iniciar, Raiz, Tela};
use crate::servicos::{Conexao, FuturoConexao, Preparo, Servicos};
use crate::testes_comuns::PastaTemporaria;

const SENHA: &str = "senha-do-teste-123";
const CNPJ: &str = "11222333000181";

fn servicos(
    arquivo: &Path,
    cofre: Arc<CofreEmMemoria>,
    conexao: Arc<dyn Conexao>,
) -> Arc<Servicos> {
    let cofre: Arc<dyn Cofre> = cofre;
    Arc::new(Servicos::com(
        Some(arquivo.to_path_buf()),
        Arc::new(|_| None),
        cofre,
        false,
        conexao,
        Arc::new(chrono::Utc::now),
    ))
}

fn abrir(
    cx: &mut TestAppContext,
    servicos: Arc<Servicos>,
) -> (WindowHandle<gpui_kit::base::Root>, Entity<Raiz>) {
    cx.update(iniciar);
    cx.update(|cx| {
        let (janela, raiz) = gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: Point::default(),
                    // alto o bastante para o formulario inteiro aparecer sem rolar
                    size: size(px(1120.), px(1500.)),
                })),
                ..Default::default()
            },
            cx,
            move |window, cx| cx.new(|cx| Raiz::new(servicos, window, cx)),
        )
        .unwrap();
        (janela.downcast::<gpui_kit::base::Root>().unwrap(), raiz)
    })
}

type Resposta = Result<(u16, Vec<u8>), FalhaRede>;

/// Transporte falso: uma resposta fixa, e a lista do que foi pedido.
#[derive(Clone)]
struct Falso {
    resposta: Arc<Mutex<Resposta>>,
    pedidos: Arc<Mutex<Vec<String>>>,
}

#[async_trait::async_trait]
impl Transporte for Falso {
    async fn enviar(
        &self,
        metodo: &str,
        url: &str,
        _corpo: Option<Vec<u8>>,
        _aceita: &str,
    ) -> Result<(u16, Vec<u8>), FalhaRede> {
        self.pedidos.lock().unwrap().push(format!("{metodo} {url}"));
        // um pouco de demora de rede, para provar que a tela nao trava esperando
        tokio::time::sleep(Duration::from_millis(150)).await;
        self.resposta.lock().unwrap().clone()
    }
}

struct ConexaoFalsa(Falso);

impl Conexao for ConexaoFalsa {
    fn testar(&self, preparo: Preparo) -> FuturoConexao {
        let transporte = self.0.clone();
        Box::pin(async move {
            let ambiente = preparo.ambiente;
            ClienteNfse::com_transporte(preparo.into_certificado(), ambiente, None, transporte)
                .unwrap()
                .testar()
                .await
        })
    }
}

fn conexao_que_responde(codigo: u16, corpo: &str) -> (Arc<ConexaoFalsa>, Falso) {
    let falso = Falso {
        resposta: Arc::new(Mutex::new(Ok((codigo, corpo.as_bytes().to_vec())))),
        pedidos: Arc::default(),
    };
    (Arc::new(ConexaoFalsa(falso.clone())), falso)
}

fn sem_rede() -> Arc<dyn Conexao> {
    Arc::new(ConexaoFalsa(Falso {
        resposta: Arc::new(Mutex::new(Err(FalhaRede::Rede("teste sem rede".into())))),
        pedidos: Arc::default(),
    }))
}

fn arquivo_completo(pasta: &PastaTemporaria, pfx: &Path) -> PathBuf {
    let caminho = pasta.caminho().join("rustnf.toml");
    let mut c = Config::default();
    c.emitente.cnpj = CNPJ.into();
    c.emitente.nome = "OFICINA TESTE".into();
    c.emitente.inscricao_municipal = "123456".into();
    c.emitente.municipio = "3550308".into();
    c.sefin.pfx = pfx.to_string_lossy().into_owned();
    c.notas.pasta = pasta.caminho().join("notas").to_string_lossy().into_owned();
    c.servidor.porta = 9090;
    c.salvar(&caminho).unwrap();
    caminho
}

/// Clica no campo, (opcionalmente apaga o que ha) e digita; os eventos de digitacao correm depois do quadro, entao
/// espera-os antes de seguir.
fn digitar(
    cx: &mut TestAppContext,
    janela: WindowHandle<gpui_kit::base::Root>,
    id: &'static str,
    texto: &str,
    apagar: bool,
) {
    cx.update_window(janela.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click(id, cx);
        if apagar {
            window.press("ctrl-a", cx);
        }
        window.input(texto, cx);
    })
    .unwrap();
    cx.run_until_parked();
}

fn clicar(cx: &mut TestAppContext, janela: WindowHandle<gpui_kit::base::Root>, id: &'static str) {
    cx.update_window(janela.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click(id, cx);
    })
    .unwrap();
    cx.run_until_parked();
}

#[gpui_kit::test]
fn configuracoes_digita_salva_e_o_arquivo_fica_igual_ao_da_linha_de_comando(
    cx: &mut TestAppContext,
) {
    let pasta = PastaTemporaria::nova("janela-salvar");
    let pfx = pasta.certificado("a.pfx", CNPJ, SENHA, 365);
    let arquivo = arquivo_completo(&pasta, &pfx);
    let (janela, raiz) = abrir(
        cx,
        servicos(&arquivo, Arc::new(CofreEmMemoria::novo()), sem_rede()),
    );
    cx.run_until_parked();
    cx.update(|cx| {
        raiz.update(cx, |r, cx| {
            r.tela = Tela::Configuracoes;
            cx.notify();
        });
    });
    digitar(cx, janela, "nome", "OFICINA NOVA LTDA", true);
    cx.update_window(janela.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("nome").value(), Some("OFICINA NOVA LTDA"));
    })
    .unwrap();
    cx.update(|cx| {
        let r = raiz.read(cx);
        assert!(r.formulario.alterado());
        assert_eq!(r.formulario.motivo_sem_salvar(), None);
    });
    // Ctrl+S salva (o foco esta no campo, dentro da janela)
    cx.update_window(janela.into(), |_, window, cx| {
        window.press("ctrl-s", cx);
    })
    .unwrap();
    cx.run_until_parked();
    let lido = Config::ler(&arquivo).unwrap();
    assert_eq!(lido.emitente.nome, "OFICINA NOVA LTDA");
    // o que a tela nao edita ([servidor]) sobreviveu ao Salvar
    assert_eq!(lido.servidor.porta, 9090);
    cx.update(|cx| assert!(!raiz.read(cx).formulario.alterado()));

    // e o botao Salvar faz o mesmo
    digitar(cx, janela, "serie", "7", true);
    cx.update(|cx| assert!(raiz.read(cx).formulario.alterado()));
    clicar(cx, janela, "salvar");
    let lido = Config::ler(&arquivo).unwrap();
    assert_eq!(lido.emitente.serie, "7");
    assert_eq!(lido.servidor.porta, 9090);
    cx.update(|cx| assert!(!raiz.read(cx).formulario.alterado()));

    // Ctrl+Shift+D descarta o que foi digitado e nao salvo: o campo volta ao valor do arquivo
    digitar(cx, janela, "nome", "NOME QUE NAO FICA", true);
    cx.update(|cx| assert!(raiz.read(cx).formulario.alterado()));
    cx.update_window(janela.into(), |_, window, cx| {
        window.press("ctrl-shift-d", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| assert!(!raiz.read(cx).formulario.alterado()));
    cx.update_window(janela.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("nome").value(), Some("OFICINA NOVA LTDA"));
    })
    .unwrap();
    assert_eq!(
        Config::ler(&arquivo).unwrap().emitente.nome,
        "OFICINA NOVA LTDA"
    );
    // fora das Configuracoes o atalho nao faz nada
    cx.update(|cx| {
        raiz.update(cx, |r, cx| {
            r.tela = Tela::Inicio;
            cx.notify();
        });
    });
    cx.update_window(janela.into(), |_, window, cx| {
        window.render_frame(cx);
        window.press("ctrl-shift-d", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| assert_eq!(raiz.read(cx).tela, Tela::Inicio));
}

#[gpui_kit::test]
fn primeiro_uso_percorre_os_tres_passos_guarda_a_senha_e_grava_o_arquivo(cx: &mut TestAppContext) {
    use crate::modelo::campo::Campo;
    use crate::modelo::passos::Passo;
    let pasta = PastaTemporaria::nova("janela-primeiro-uso");
    let pfx = pasta.certificado("a.pfx", CNPJ, SENHA, 365);
    let arquivo = pasta.caminho().join("novo").join("rustnf.toml");
    let notas = pasta.caminho().join("minhas-notas");
    let cofre = Arc::new(CofreEmMemoria::novo());
    let (janela, raiz) = abrir(cx, servicos(&arquivo, cofre.clone(), sem_rede()));
    cx.run_until_parked();
    cx.update(|cx| {
        let r = raiz.read(cx);
        assert!(r.pergunta_o_primeiro_uso());
        assert!(r.info.as_ref().unwrap().novo);
    });

    // passo 1: com tudo vazio, Continuar mostra o que falta embaixo de cada campo e nao sai do passo
    clicar(cx, janela, "continuar");
    cx.update(|cx| {
        let r = raiz.read(cx);
        assert_eq!(r.passo, Passo::Emitente);
        assert!(
            r.tentou_continuar,
            "o clique em Continuar nao chegou ao botao"
        );
        let e = r.erros_visiveis();
        assert_eq!(
            e.do_campo(Campo::Documento),
            Some("Informe o CNPJ ou o CPF.")
        );
        assert_eq!(
            e.do_campo(Campo::Nome),
            Some("Informe o nome ou a razão social.")
        );
    });

    digitar(cx, janela, "documento", CNPJ, false);
    cx.update_window(janela.into(), |_, window, cx| {
        window.render_frame(cx);
        // a mascara aparece sozinha
        assert_eq!(window.find("documento").value(), Some("11.222.333/0001-81"));
    })
    .unwrap();
    digitar(cx, janela, "nome", "OFICINA TESTE", false);
    digitar(cx, janela, "inscricao", "123456", false);
    digitar(cx, janela, "municipio", "3550308", false);
    cx.update(|cx| {
        let r = raiz.read(cx);
        assert_eq!(r.formulario.dados.documento, "11.222.333/0001-81");
        assert_eq!(
            r.formulario.rotulo_do_municipio().texto().as_deref(),
            Some("São Paulo - SP")
        );
    });
    clicar(cx, janela, "continuar");
    cx.update(|cx| assert_eq!(raiz.read(cx).passo, Passo::Certificado));

    // passo 2: arquivo e senha digitados; a senha vai para o cofre so' depois de abrir o certificado
    digitar(cx, janela, "pfx", &pfx.to_string_lossy(), false);
    digitar(cx, janela, "senha", "senha-errada", false);
    clicar(cx, janela, "continuar");
    cx.update(|cx| {
        let r = raiz.read(cx);
        assert_eq!(r.passo, Passo::Certificado, "senha errada nao avanca");
        assert_eq!(
            r.erros_visiveis().do_campo(Campo::Senha),
            Some("Senha do certificado incorreta.")
        );
    });
    assert!(
        cofre.ler(CNPJ).unwrap().is_none(),
        "senha errada nao pode ir para o cofre"
    );
    digitar(cx, janela, "senha", SENHA, true);
    clicar(cx, janela, "continuar");
    cx.update(|cx| assert_eq!(raiz.read(cx).passo, Passo::Notas));
    assert_eq!(cofre.ler(CNPJ).unwrap().unwrap().expor(), SENHA);

    // passo 3: pasta das notas e Concluir
    digitar(
        cx,
        janela,
        "pasta-das-notas",
        &notas.to_string_lossy(),
        false,
    );
    clicar(cx, janela, "continuar");
    let lido = Config::ler(&arquivo).unwrap();
    assert_eq!(lido.emitente.cnpj, CNPJ);
    assert_eq!(lido.emitente.nome, "OFICINA TESTE");
    assert_eq!(lido.emitente.municipio, "3550308");
    assert_eq!(lido.sefin.pfx, pfx.to_string_lossy());
    assert_eq!(lido.notas.pasta, notas.to_string_lossy());
    assert!(notas.is_dir(), "a pasta das notas e' criada");
    // a senha nao esta em arquivo nenhum
    assert!(!std::fs::read_to_string(&arquivo).unwrap().contains(SENHA));
    cx.update(|cx| {
        let r = raiz.read(cx);
        assert!(
            !r.pergunta_o_primeiro_uso(),
            "configuracao completa: o Inicio mostra o resumo"
        );
        assert!(matches!(
            r.cert,
            crate::modelo::certificado::EstadoCertificado::Valido { .. }
        ));
    });
}

#[gpui_kit::test]
fn testar_conexao_nao_trava_a_tela_e_mostra_o_resultado(cx: &mut TestAppContext) {
    let pasta = PastaTemporaria::nova("janela-conexao");
    let pfx = pasta.certificado("a.pfx", CNPJ, SENHA, 365);
    let arquivo = arquivo_completo(&pasta, &pfx);
    let (conexao, falso) = conexao_que_responde(
        404,
        r#"{"erro":{"codigo":"E404","descricao":"nao achado"}}"#,
    );
    let cofre = Arc::new(CofreEmMemoria::novo());
    cofre
        .guardar(CNPJ, &rustnf_app::segredo::Segredo::novo(SENHA.into()))
        .unwrap();
    let (janela, raiz) = abrir(cx, servicos(&arquivo, cofre, conexao));
    cx.run_until_parked();
    cx.update(|cx| {
        raiz.update(cx, |r, cx| {
            r.tela = Tela::Configuracoes;
            r.secao = super::Secao::Sefin;
            cx.notify();
        });
    });
    cx.update_window(janela.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("testar-conexao", cx);
    })
    .unwrap();
    // a chamada de rede leva ~150 ms no runtime proprio; a tela continua respondendo e mostra que esta trabalhando
    cx.update(|cx| {
        assert!(raiz.read(cx).ocupado.conexao);
        assert!(raiz.read(cx).resultado_conexao.is_none());
    });
    cx.update_window(janela.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("resultado-da-conexao").is_none());
    })
    .unwrap();
    cx.executor().allow_parking();
    let t0 = std::time::Instant::now();
    loop {
        let pronto = cx.update(|cx| raiz.read(cx).resultado_conexao.is_some());
        if pronto {
            break;
        }
        assert!(
            t0.elapsed() < Duration::from_secs(20),
            "a conexao nunca respondeu"
        );
        std::thread::sleep(Duration::from_millis(20));
        cx.run_until_parked();
    }
    cx.update(|cx| {
        let r = raiz.read(cx);
        assert!(!r.ocupado.conexao);
        let (ok, texto) = r.resultado_conexao.clone().unwrap();
        assert!(ok, "{texto}");
        assert!(texto.contains("Conexao com o SEFIN ok"), "{texto}");
    });
    assert_eq!(falso.pedidos.lock().unwrap().len(), 1);
}

#[gpui_kit::test]
fn tab_percorre_os_campos_na_ordem_visual_e_enter_confirma_o_primeiro_uso(cx: &mut TestAppContext) {
    let pasta = PastaTemporaria::nova("janela-tab");
    let arquivo = pasta.caminho().join("rustnf.toml");
    let (janela, raiz) = abrir(
        cx,
        servicos(&arquivo, Arc::new(CofreEmMemoria::novo()), sem_rede()),
    );
    cx.run_until_parked();
    cx.update_window(janela.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("documento", cx);
        assert_eq!(window.find("documento").focused(), Some(true));
        // ordem visual do passo 1: documento, inscricao, nome, municipio
        for esperado in ["inscricao", "nome", "municipio"] {
            window.press("tab", cx);
            assert_eq!(
                window.find(esperado).focused(),
                Some(true),
                "o Tab devia ir para {esperado}"
            );
        }
        // Enter num campo do primeiro uso confirma o passo (aqui falta tudo: mostra o que falta)
        window.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let r = raiz.read(cx);
        assert!(r.tentou_continuar, "o Enter nao confirmou o passo");
        assert!(r
            .erros_visiveis()
            .do_campo(crate::modelo::campo::Campo::Documento)
            .is_some());
    });
}

#[gpui_kit::test]
fn quem_ja_emitia_continua_a_numeracao_pela_tela(cx: &mut TestAppContext) {
    let pasta = PastaTemporaria::nova("janela-numero-inicial");
    let pfx = pasta.certificado("a.pfx", CNPJ, SENHA, 365);
    let arquivo = arquivo_completo(&pasta, &pfx);
    let notas_dir = pasta.caminho().join("notas");
    let (janela, raiz) = abrir(
        cx,
        servicos(&arquivo, Arc::new(CofreEmMemoria::novo()), sem_rede()),
    );
    cx.run_until_parked();
    cx.update(|cx| {
        raiz.update(cx, |r, cx| {
            r.tela = Tela::Configuracoes;
            r.secao = super::Secao::Notas;
            cx.notify();
        });
    });
    cx.run_until_parked();
    cx.update(|cx| assert_eq!(raiz.read(cx).proximo_dps, Some(1)));

    // texto que nao e' numero: aviso no campo, sem perguntar nada
    digitar(cx, janela, "proximo-numero-dps", "15a", true);
    clicar(cx, janela, "continuar-deste-numero");
    cx.update(|cx| {
        assert!(raiz
            .read(cx)
            .erros_externos
            .contains_key(&crate::modelo::campo::Campo::ProximoDps));
    });

    // a ultima DPS do outro sistema foi a 1500: pergunta e, confirmado, a serie continua na 1501
    digitar(cx, janela, "proximo-numero-dps", "1501", true);
    clicar(cx, janela, "continuar-deste-numero");
    assert_eq!(
        rustnf_app::notas::Notas::abrir(&notas_dir)
            .unwrap()
            .proximo_numero("1")
            .unwrap(),
        1,
        "so' a confirmacao muda a numeracao"
    );

    clicar(cx, janela, "ok");
    cx.run_until_parked();

    assert_eq!(
        rustnf_app::notas::Notas::abrir(&notas_dir)
            .unwrap()
            .proximo_numero("1")
            .unwrap(),
        1501
    );
    cx.update(|cx| {
        let r = raiz.read(cx);
        assert_eq!(r.proximo_dps, Some(1501));
        assert!(!r
            .erros_externos
            .contains_key(&crate::modelo::campo::Campo::ProximoDps));
        // a Nova nota ja' mostra o numero novo
        assert_eq!(r.nova.read(cx).numero_provisorio, 1501);
    });

    // voltar atras: recusado com a frase do numero que vale, e nada muda
    digitar(cx, janela, "proximo-numero-dps", "100", true);
    clicar(cx, janela, "continuar-deste-numero");
    clicar(cx, janela, "ok");
    cx.run_until_parked();
    cx.update(|cx| {
        let r = raiz.read(cx);
        let erro = r
            .erros_externos
            .get(&crate::modelo::campo::Campo::ProximoDps)
            .cloned()
            .unwrap_or_default();
        assert!(
            erro.contains("já foi usado") && erro.contains("1501"),
            "{erro}"
        );
        assert_eq!(r.proximo_dps, Some(1501));
    });
}

/// Como [`abrir`], numa janela baixa: o formulario nao cabe e a tela precisa rolar.
fn abrir_baixa(
    cx: &mut TestAppContext,
    servicos: Arc<Servicos>,
) -> (WindowHandle<gpui_kit::base::Root>, Entity<Raiz>) {
    cx.update(iniciar);
    cx.update(|cx| {
        let (janela, raiz) = gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: Point::default(),
                    size: size(px(1120.), px(480.)),
                })),
                ..Default::default()
            },
            cx,
            move |window, cx| cx.new(|cx| Raiz::new(servicos, window, cx)),
        )
        .unwrap();
        (janela.downcast::<gpui_kit::base::Root>().unwrap(), raiz)
    })
}

fn deslocamento(cx: &mut TestAppContext, raiz: &Entity<Raiz>) -> f32 {
    cx.update(|cx| {
        let r = raiz.read(cx);
        f32::from(r.rolagem_da_tela(cx).unwrap().offset().y)
    })
}

fn apertar(cx: &mut TestAppContext, janela: WindowHandle<gpui_kit::base::Root>, tecla: &str) {
    cx.update_window(janela.into(), |_, window, cx| {
        window.render_frame(cx);
        window.press(tecla, cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(janela.into(), |_, window, cx| window.render_frame(cx))
        .unwrap();
}

#[gpui_kit::test]
fn o_teclado_rola_a_tela_e_o_tab_leva_a_tela_ate_o_campo(cx: &mut TestAppContext) {
    let pasta = PastaTemporaria::nova("janela-rolagem");
    let pfx = pasta.certificado("a.pfx", CNPJ, SENHA, 365);
    let arquivo = arquivo_completo(&pasta, &pfx);
    let (janela, raiz) = abrir_baixa(
        cx,
        servicos(&arquivo, Arc::new(CofreEmMemoria::novo()), sem_rede()),
    );
    cx.run_until_parked();
    cx.update(|cx| {
        raiz.update(cx, |r, cx| {
            r.tela = Tela::Configuracoes;
            cx.notify();
        });
    });
    cx.update_window(janela.into(), |_, window, cx| window.render_frame(cx))
        .unwrap();
    assert_eq!(deslocamento(cx, &raiz), 0.0);

    // com o foco num campo, PageDown rola a tela (o campo de uma linha nao usa a tecla)
    clicar(cx, janela, "documento");
    apertar(cx, janela, "pagedown");
    let depois_da_pagina = deslocamento(cx, &raiz);
    assert!(depois_da_pagina < -100.0, "PageDown: {depois_da_pagina}");
    // PageUp volta
    apertar(cx, janela, "pageup");
    assert_eq!(deslocamento(cx, &raiz), 0.0, "PageUp");
    cx.update(|cx| {
        raiz.update(cx, |r, cx| {
            r.rolagem.set_offset(Point::default());
            cx.notify();
        });
    });

    // Tab de campo em campo: o campo que ganha o foco aparece; a tela desce sozinha
    clicar(cx, janela, "documento");
    // documento -> inscricao -> nome -> municipio -> serie, que fica abaixo da dobra nesta janela
    for _ in 0..4 {
        apertar(cx, janela, "tab");
    }
    cx.update_window(janela.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("serie").focused(),
            Some(true),
            "o Tab devia chegar na serie"
        );
    })
    .unwrap();
    let depois_do_tab = deslocamento(cx, &raiz);
    assert!(depois_do_tab < -50.0, "Tab: {depois_do_tab}");
    // a tela vai ao fim; Shift+Tab ate o documento (primeiro campo, agora acima da area) traz a tela de volta
    apertar(cx, janela, "pagedown");
    apertar(cx, janela, "pagedown");
    let longe = deslocamento(cx, &raiz);
    assert!(longe < depois_do_tab, "PageDown: {longe}");
    for _ in 0..4 {
        apertar(cx, janela, "shift-tab");
    }
    cx.update_window(janela.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("documento").focused(), Some(true));
    })
    .unwrap();
    let de_volta = deslocamento(cx, &raiz);
    assert!(de_volta > longe, "Shift+Tab: {de_volta} x {longe}");
}
