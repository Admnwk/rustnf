// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Testes da janela da Nova nota, no ambiente de teste do GPUI, com o emissor de demonstracao (nunca a Sefin), cofre
//! em memoria e pasta temporaria. Nenhum toca o cofre real, a pasta de configuracao do usuario nem a rede.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    px, size, AppContext as _, Bounds, Entity, Point, TestAppContext, WindowBounds, WindowHandle,
    WindowOptions,
};
use rustnf_app::cofre::{Cofre, CofreEmMemoria};
use rustnf_app::config::Config;
use rustnf_app::notas::Notas;
use rustnf_app::segredo::Segredo;

use super::nota::{Fase, NovaNota};
use super::{iniciar, Raiz, Tela};
use crate::demonstracao::{
    Cancelamento, Desfecho, EmissorDemonstracao, Ensaio, FabricaDemonstracao,
};
use crate::modelo::nota::CampoNota;
use crate::servicos::{Conexao, FuturoConexao, Preparo, Servicos};
use crate::testes_comuns::{AbridorDeTeste, PastaTemporaria};

pub(super) const SENHA: &str = "senha-do-teste-123";
pub(super) const CNPJ: &str = "11222333000181";

pub(super) struct SemConexao;

impl Conexao for SemConexao {
    fn testar(&self, _preparo: Preparo) -> FuturoConexao {
        Box::pin(async { (false, String::new()) })
    }
}

/// O que cada teste precisa para olhar e conferir.
pub(super) struct Cenario {
    pub(super) janela: WindowHandle<gpui_kit::base::Root>,
    pub(super) raiz: Entity<Raiz>,
    pub(super) nova: Entity<NovaNota>,
    pub(super) emissor: Arc<EmissorDemonstracao>,
    pub(super) abridor: Arc<AbridorDeTeste>,
    pub(super) cofre: Arc<CofreEmMemoria>,
    pub(super) notas: PathBuf,
    pub(super) arquivo: PathBuf,
    pub(super) _pasta: PastaTemporaria,
}

pub(super) fn arquivo_de_ensaio(
    pasta: &PastaTemporaria,
    pfx: &Path,
    ambiente: &str,
    completo: bool,
) -> PathBuf {
    let caminho = pasta.caminho().join("rustnf.toml");
    let mut c = Config::default();
    if completo {
        c.emitente.cnpj = CNPJ.into();
        c.emitente.nome = "OFICINA DE TESTE LTDA".into();
        c.emitente.inscricao_municipal = "123456".into();
        c.emitente.municipio = "3550308".into();
        c.emitente.op_simples = 3;
        c.emitente.regime_apuracao = 1;
        c.sefin.pfx = pfx.to_string_lossy().into_owned();
        c.notas.pasta = pasta.caminho().join("notas").to_string_lossy().into_owned();
    }
    c.sefin.ambiente = ambiente.into();
    c.salvar(&caminho).unwrap();
    caminho
}

pub(super) fn abrir_janela(
    cx: &mut TestAppContext,
    servicos: Arc<Servicos>,
    largura: f32,
    altura: f32,
) -> (WindowHandle<gpui_kit::base::Root>, Entity<Raiz>) {
    cx.update(iniciar);
    cx.update(|cx| {
        let (janela, raiz) = gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: Point::default(),
                    size: size(px(largura), px(altura)),
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

/// Alto o bastante para o formulario inteiro aparecer sem rolar.
pub(super) fn cenario(
    cx: &mut TestAppContext,
    ambiente: &str,
    roteiro: &[Desfecho],
    completo: bool,
    largura: f32,
) -> Cenario {
    cenario_com_altura(cx, ambiente, roteiro, completo, largura, 5200.)
}

pub(super) fn cenario_com_altura(
    cx: &mut TestAppContext,
    ambiente: &str,
    roteiro: &[Desfecho],
    completo: bool,
    largura: f32,
    altura: f32,
) -> Cenario {
    cenario_de_cancelamento(cx, ambiente, roteiro, &[], completo, largura, altura)
}

/// Como `cenario_com_altura`, com o roteiro do cancelamento da demonstracao (vazio: registrado).
pub(super) fn cenario_de_cancelamento(
    cx: &mut TestAppContext,
    ambiente: &str,
    roteiro: &[Desfecho],
    cancelamento: &[Cancelamento],
    completo: bool,
    largura: f32,
    altura: f32,
) -> Cenario {
    // a rede roda em outra linha (o runtime proprio): o agendador do teste precisa saber
    cx.executor().allow_parking();
    let pasta = PastaTemporaria::nova("janela-nota");
    let pfx = pasta.certificado("a.pfx", CNPJ, SENHA, 365);
    let arquivo = arquivo_de_ensaio(&pasta, &pfx, ambiente, completo);
    let cofre = Arc::new(CofreEmMemoria::novo());
    cofre.guardar(CNPJ, &Segredo::novo(SENHA.into())).unwrap();
    let emissor = EmissorDemonstracao::com_ensaio(
        if ambiente == "producao" { 1 } else { 2 },
        roteiro,
        Ensaio {
            cancelamento: cancelamento.to_vec(),
            pasta: Some(pasta.caminho().join("notas")),
        },
        Duration::from_millis(120),
    );
    let abridor = Arc::new(AbridorDeTeste::default());
    let cofre_dyn: Arc<dyn Cofre> = cofre.clone();
    let servicos = Arc::new(
        Servicos::com(
            Some(arquivo.clone()),
            Arc::new(|_| None),
            cofre_dyn,
            false,
            Arc::new(SemConexao),
            Arc::new(chrono::Utc::now),
        )
        .com_fabrica(FabricaDemonstracao::nova(emissor.clone()))
        .com_abridor(abridor.clone()),
    );
    let (janela, raiz) = abrir_janela(cx, servicos, largura, altura);
    cx.run_until_parked();
    cx.update(|cx| {
        raiz.update(cx, |r, cx| {
            r.tela = Tela::NovaNota;
            cx.notify();
        });
    });
    cx.run_until_parked();
    let nova = cx.update(|cx| raiz.read(cx).nova.clone());
    Cenario {
        notas: pasta.caminho().join("notas"),
        janela,
        raiz,
        nova,
        emissor,
        abridor,
        cofre,
        arquivo,
        _pasta: pasta,
    }
}

pub(super) fn digitar(cx: &mut TestAppContext, c: &Cenario, id: &'static str, texto: &str) {
    cx.update_window(c.janela.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click(id, cx);
        window.press("ctrl-a", cx);
        window.input(texto, cx);
    })
    .unwrap();
    cx.run_until_parked();
}

pub(super) fn clicar(cx: &mut TestAppContext, c: &Cenario, id: &'static str) {
    cx.update_window(c.janela.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click(id, cx);
    })
    .unwrap();
    cx.run_until_parked();
}

pub(super) fn existe(cx: &mut TestAppContext, c: &Cenario, id: &'static str) -> bool {
    cx.update_window(c.janela.into(), |_, window, cx| {
        window.render_frame(cx);
        window.try_find(id).is_some()
    })
    .unwrap()
}

/// A descricao e' um Textarea sem id: o foco vai pelo modelo e o texto, pela janela.
pub(super) fn escrever_a_descricao(cx: &mut TestAppContext, c: &Cenario, texto: &str) {
    let nova = c.nova.clone();
    cx.update_window(c.janela.into(), |_, window, cx| {
        window.render_frame(cx);
        let descricao = nova.read(cx).descricao.clone();
        descricao.update(cx, |e, cx| e.focus(window, cx));
        window.input(texto, cx);
    })
    .unwrap();
    cx.run_until_parked();
}

/// Deixa os eventos de foco e de digitacao correrem.
pub(super) fn assentar(cx: &mut TestAppContext, c: &Cenario) {
    cx.update_window(c.janela.into(), |_, window, cx| window.render_frame(cx))
        .unwrap();
    cx.run_until_parked();
}

/// Preenche a nota do caso comum pela janela, campo a campo.
pub(super) fn preencher(cx: &mut TestAppContext, c: &Cenario) {
    digitar(cx, c, "nota-tomador-documento", "52998224725");
    digitar(cx, c, "nota-tomador-nome", "Maria de Teste");
    digitar(cx, c, "nota-ctribnac", "140101");
    escrever_a_descricao(cx, c, "Revisão geral do veículo");
    digitar(cx, c, "nota-nbs", "120011000");
    digitar(cx, c, "nota-valor-servico", "1234,56");
    digitar(cx, c, "nota-percentual-sn", "4,5");
    // o emitente do ensaio e' do Simples: a nota nasce sem o grupo IBS/CBS, e o caso comum o liga
    cx.update(|cx| c.nova.update(cx, |n, cx| n.escolher_ibscbs(true, cx)));
    cx.run_until_parked();
    digitar(cx, c, "nota-cind-op", "050101");
    digitar(cx, c, "nota-class-trib", "000001");
}

pub(super) fn esperar(
    cx: &mut TestAppContext,
    c: &Cenario,
    o_que: &str,
    pronto: impl Fn(&NovaNota) -> bool,
) {
    cx.executor().allow_parking();
    let t0 = std::time::Instant::now();
    loop {
        if cx.update(|cx| pronto(c.nova.read(cx))) {
            return;
        }
        assert!(
            t0.elapsed() < Duration::from_secs(20),
            "demorou demais: {o_que}"
        );
        std::thread::sleep(Duration::from_millis(20));
        cx.run_until_parked();
    }
}

pub(super) fn pendencias(cx: &mut TestAppContext, c: &Cenario) -> Vec<CampoNota> {
    cx.update(|cx| {
        c.nova
            .read(cx)
            .conferencia
            .pendencias
            .iter()
            .filter_map(|p| p.campo)
            .collect()
    })
}

/// Guarda na pasta de notas do ensaio a NFS-e de exemplo (tomador com endereco), com chave de digito certo e o tomador
/// trocado por um CNPJ que confere (o do exemplo e' ficticio e nao passa no digito verificador).
fn guardar_nota_com_endereco(c: &Cenario) {
    let original = include_str!("../../../rustnf-cli/exemplos/nfse.xml");
    let lida = rustnf::nfse::leitura::ler_nfse(original).unwrap();
    let chave = rustnf::chave_dfe::montar_nfse(3_550_308, CNPJ, 7, "2610", Some(1), 2).unwrap();
    let xml = original
        .replace(&lida.chave, &chave)
        .replace("<CNPJ>33444555000172</CNPJ>", "<CNPJ>11222333000181</CNPJ>");
    let n = rustnf::nfse::leitura::ler_nfse(&xml).unwrap();
    assert_eq!(n.tomador_documento, "11222333000181");
    assert_eq!(n.tomador_cep, "05422030");
    Notas::abrir(&c.notas)
        .unwrap()
        .guardar(&rustnf_app::notas::Registro::da_nfse(&n), &xml)
        .unwrap();
}

#[gpui_kit::test]
fn tomador_ja_usado_completa_os_campos_vazios(cx: &mut TestAppContext) {
    let c = cenario(cx, "restrita", &[Desfecho::Autorizada], true, 1120.);
    guardar_nota_com_endereco(&c);
    // o nome digitado antes do documento fica; o resto vem da nota anterior
    digitar(cx, &c, "nota-tomador-nome", "Nome digitado");
    digitar(cx, &c, "nota-tomador-documento", "11222333000181");
    esperar(cx, &c, "o tomador da pasta", |n| {
        n.tomador_preenchido.is_some()
    });
    cx.update(|cx| {
        let n = c.nova.read(cx);
        assert_eq!(n.dados.tomador_nome, "Nome digitado");
        assert_eq!(n.dados.end_cep, "05422-030");
        assert_eq!(n.dados.end_logradouro, "Rua");
        assert_eq!(n.dados.end_numero, "2176");
        assert_eq!(n.dados.end_bairro, "Centro");
        assert_eq!(n.dados.end_municipio, "3503208");
        let aviso = n.tomador_preenchido.clone().unwrap();
        assert!(aviso.contains("NFS-e nº 25"), "{aviso}");
    });
    // os campos da tela mostram o que veio, com a mascara
    cx.update_window(c.janela.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("nota-end-cep").value(), Some("05422-030"));
        assert_eq!(
            window.find("nota-tomador-nome").value(),
            Some("Nome digitado")
        );
    })
    .unwrap();
    assert!(!pendencias(cx, &c).contains(&CampoNota::EnderecoCep));

    // outro documento, que nao esta na pasta: nada vem, e o aviso sai
    digitar(cx, &c, "nota-tomador-documento", "52998224725");
    cx.run_until_parked();
    std::thread::sleep(Duration::from_millis(200));
    cx.run_until_parked();
    cx.update(|cx| {
        let n = c.nova.read(cx);
        assert!(n.tomador_preenchido.is_none());
        // o que ja estava no formulario fica: so' o documento mudou
        assert_eq!(n.dados.end_cep, "05422-030");
    });

    // "Limpar" e documento pela metade: nada e' procurado
    cx.update_window(c.janela.into(), |_, window, cx| {
        c.nova.update(cx, |n, cx| n.limpar(window, cx));
    })
    .unwrap();
    cx.run_until_parked();
    digitar(cx, &c, "nota-tomador-documento", "11222333");
    std::thread::sleep(Duration::from_millis(200));
    cx.run_until_parked();
    cx.update(|cx| {
        let n = c.nova.read(cx);
        assert!(n.tomador_preenchido.is_none());
        assert_eq!(n.dados.end_cep, "");
    });
}

#[gpui_kit::test]
fn preencher_ver_a_pendencia_sumir_emitir_e_a_nota_fica_na_pasta(cx: &mut TestAppContext) {
    let c = cenario(cx, "restrita", &[Desfecho::Autorizada], true, 1120.);
    // formulario vazio: o painel lista o que falta
    let vazias = pendencias(cx, &c);
    assert!(vazias.contains(&CampoNota::TomadorDocumento));
    assert!(vazias.contains(&CampoNota::ValorServico));
    cx.update(|cx| assert!(!c.nova.read(cx).conferencia.limpa()));
    // digitando o documento (com mascara) a pendencia dele some
    digitar(cx, &c, "nota-tomador-documento", "52998224725");
    cx.update_window(c.janela.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("nota-tomador-documento").value(),
            Some("529.982.247-25")
        );
    })
    .unwrap();
    assert!(!pendencias(cx, &c).contains(&CampoNota::TomadorDocumento));
    assert!(pendencias(cx, &c).contains(&CampoNota::TomadorNome));
    // com o formulario incompleto o botao esta desligado e diz o motivo
    cx.update(|cx| {
        assert!(c
            .nova
            .read(cx)
            .conferencia
            .motivo()
            .unwrap()
            .contains("(e mais"))
    });
    preencher(cx, &c);
    cx.update(|cx| {
        let n = c.nova.read(cx);
        assert!(n.conferencia.limpa(), "{:?}", n.conferencia.pendencias);
        assert!(n.conferencia.da_biblioteca);
    });
    // Emitir abre a confirmacao; confirmar envia fora da tela
    clicar(cx, &c, "emitir");
    assert!(existe(cx, &c, "confirmar-emissao"));
    assert_eq!(c.emissor.total_de_chamadas(), 0, "so' a confirmacao envia");
    clicar(cx, &c, "confirmar-emissao");
    cx.update(|cx| assert!(matches!(c.nova.read(cx).fase, Fase::Enviando { .. })));
    esperar(cx, &c, "autorizada", |n| {
        matches!(n.fase, Fase::Concluida(_))
    });
    // a tela de resultado
    let Fase::Concluida(r) = cx.update(|cx| c.nova.read(cx).fase.clone()) else {
        panic!("esperava concluida");
    };
    assert_eq!(r.numero, "1");
    assert_eq!(r.chave.len(), 50);
    assert!(r.guardada);
    assert!(existe(cx, &c, "copiar-chave"));
    assert!(existe(cx, &c, "nova-nota-limpa"));
    // a nota esta na pasta, o numero avancou e o rascunho sumiu
    let pasta = Notas::abrir(&c.notas).unwrap();
    assert_eq!(pasta.listar().unwrap().len(), 1);
    assert!(pasta.ler_xml(&r.chave).is_ok());
    assert_eq!(pasta.proximo_numero("1").unwrap(), 2);
    assert!(!c.notas.join("rascunho.json").exists());
    assert_eq!(c.emissor.total_de_chamadas(), 1);
    // "Nova nota" limpa o formulario
    clicar(cx, &c, "nova-nota-limpa");
    cx.update(|cx| {
        let n = c.nova.read(cx);
        assert!(matches!(n.fase, Fase::Editando));
        assert!(n.dados.vazio(&n.ctx.hoje()));
    });
    cx.executor().allow_parking();
    esperar(cx, &c, "proximo numero", |n| n.numero_provisorio == 2);
}

#[gpui_kit::test]
fn em_producao_a_confirmacao_e_explicita_e_sem_marcar_nada_e_enviado(cx: &mut TestAppContext) {
    let c = cenario(cx, "producao", &[Desfecho::Autorizada], true, 1120.);
    preencher(cx, &c);
    clicar(cx, &c, "emitir");
    assert!(existe(cx, &c, "confirmar-emissao"));
    assert!(existe(cx, &c, "entendo-valor-fiscal"));
    cx.update(|cx| assert!(!c.nova.read(cx).pode_confirmar()));
    // clicar em confirmar sem marcar nao envia nada
    clicar(cx, &c, "confirmar-emissao");
    cx.executor().allow_parking();
    std::thread::sleep(Duration::from_millis(400));
    cx.run_until_parked();
    assert_eq!(
        c.emissor.total_de_chamadas(),
        0,
        "sem a marcacao nada pode ser enviado"
    );
    cx.update(|cx| assert!(matches!(c.nova.read(cx).fase, Fase::Editando)));
    // marca a confirmacao: so' entao o botao vale
    clicar(cx, &c, "entendo-valor-fiscal");
    cx.update(|cx| assert!(c.nova.read(cx).pode_confirmar()));
    clicar(cx, &c, "confirmar-emissao");
    esperar(cx, &c, "autorizada", |n| {
        matches!(n.fase, Fase::Concluida(_))
    });
    assert_eq!(c.emissor.total_de_chamadas(), 1);
}

#[gpui_kit::test]
fn em_producao_restrita_a_confirmacao_nao_pede_a_marcacao(cx: &mut TestAppContext) {
    let c = cenario(cx, "restrita", &[Desfecho::Autorizada], true, 1120.);
    preencher(cx, &c);
    clicar(cx, &c, "emitir");
    assert!(existe(cx, &c, "confirmar-emissao"));
    assert!(!existe(cx, &c, "entendo-valor-fiscal"));
    cx.update(|cx| assert!(c.nova.read(cx).pode_confirmar()));
    // Esc fecha o dialogo sem enviar nada
    assert!(ha_dialogo(cx, &c));
    cx.update_window(c.janela.into(), |_, window, cx| {
        window.press("escape", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert!(!ha_dialogo(cx, &c));
    assert_eq!(c.emissor.total_de_chamadas(), 0);
}

#[gpui_kit::test]
fn rejeicao_mantem_os_dados_digitados_e_marca_o_campo(cx: &mut TestAppContext) {
    let c = cenario(cx, "restrita", &[Desfecho::Rejeitada], true, 1120.);
    preencher(cx, &c);
    clicar(cx, &c, "emitir");
    clicar(cx, &c, "confirmar-emissao");
    esperar(cx, &c, "rejeicao", |n| n.rejeicao.is_some());
    cx.update(|cx| {
        let n = c.nova.read(cx);
        assert!(matches!(n.fase, Fase::Editando));
        // os dados continuam la
        assert_eq!(n.dados.tomador_nome, "Maria de Teste");
        assert_eq!(n.dados.valor_servico, "1.234,56");
        assert_eq!(n.dados.ctribnac, "14.01.01");
        // a Sefin apontou o codigo de tributacao (E0312): o campo fica marcado
        assert!(n.erro_do(CampoNota::Ctribnac).is_some());
        assert_eq!(n.rejeicao.as_ref().unwrap().len(), 2);
    });
    cx.update_window(c.janela.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("nota-tomador-nome").value(),
            Some("Maria de Teste")
        );
    })
    .unwrap();
    // nada foi guardado e o numero nao foi gasto
    let pasta = Notas::abrir(&c.notas).unwrap();
    assert!(pasta.listar().unwrap().is_empty());
    assert_eq!(pasta.proximo_numero("1").unwrap(), 1);
    // mexer no campo marcado tira a marca
    digitar(cx, &c, "nota-ctribnac", "140101");
    cx.update(|cx| assert!(c.nova.read(cx).erro_do(CampoNota::Ctribnac).is_none()));
}

fn ha_dialogo(cx: &mut TestAppContext, c: &Cenario) -> bool {
    cx.update_window(c.janela.into(), |_, window, cx| {
        use gpui_kit::component::WindowExt as _;
        window.render_frame(cx);
        window.has_active_dialog(cx)
    })
    .unwrap()
}

#[gpui_kit::test]
fn clique_duplo_em_emitir_envia_uma_vez_so(cx: &mut TestAppContext) {
    let c = cenario(cx, "restrita", &[Desfecho::Autorizada], true, 1120.);
    preencher(cx, &c);
    // Emitir abre a confirmacao; ela e' unica (o segundo clique cairia no fundo do dialogo e o fecharia)
    clicar(cx, &c, "emitir");
    assert!(ha_dialogo(cx, &c));
    clicar(cx, &c, "cancelar-emissao");
    assert!(!ha_dialogo(cx, &c));
    assert_eq!(c.emissor.total_de_chamadas(), 0, "cancelar nao envia");
    // confirmar: o envio comeca na hora e um segundo clique (ou Ctrl+Enter) enquanto envia nao faz nada
    clicar(cx, &c, "emitir");
    clicar(cx, &c, "confirmar-emissao");
    cx.update(|cx| assert!(matches!(c.nova.read(cx).fase, Fase::Enviando { .. })));
    cx.update_window(c.janela.into(), |_, window, cx| {
        c.nova.update(cx, |n, cx| {
            n.confirmar_emissao(window, cx);
            n.pedir_emissao(window, cx);
        });
        window.render_frame(cx);
        window.click("emitir", cx);
    })
    .unwrap();
    esperar(cx, &c, "autorizada", |n| {
        matches!(n.fase, Fase::Concluida(_))
    });
    assert_eq!(
        c.emissor.total_de_chamadas(),
        1,
        "o clique duplo enviou duas vezes"
    );
    assert!(!ha_dialogo(cx, &c));
    assert_eq!(Notas::abrir(&c.notas).unwrap().listar().unwrap().len(), 1);
}

#[gpui_kit::test]
fn sem_resposta_trava_o_formulario_e_tentar_de_novo_reenvia_o_mesmo_pedido(
    cx: &mut TestAppContext,
) {
    let c = cenario(
        cx,
        "restrita",
        &[Desfecho::SemResposta, Desfecho::Recuperada],
        true,
        1120.,
    );
    preencher(cx, &c);
    clicar(cx, &c, "emitir");
    clicar(cx, &c, "confirmar-emissao");
    esperar(cx, &c, "sem resposta", |n| {
        matches!(n.fase, Fase::Pendente { .. })
    });
    assert!(existe(cx, &c, "tentar-de-novo-emissao"));
    assert!(existe(cx, &c, "desistir-da-tentativa"));
    // o formulario esta travado: digitar nao muda nada
    let antes = cx.update(|cx| c.nova.read(cx).dados.clone());
    digitar(cx, &c, "nota-tomador-nome", "Outro Nome");
    assert_eq!(cx.update(|cx| c.nova.read(cx).dados.clone()), antes);
    // e Emitir esta desligado
    clicar(cx, &c, "emitir");
    assert_eq!(c.emissor.total_de_chamadas(), 1);
    // tentar de novo: o mesmo pedido, e a nota e' recuperada
    clicar(cx, &c, "tentar-de-novo-emissao");
    esperar(cx, &c, "recuperada", |n| {
        matches!(n.fase, Fase::Concluida(_))
    });
    let chamadas = c.emissor.chamadas();
    assert_eq!(chamadas.len(), 2);
    assert_eq!(chamadas[0], chamadas[1]);
    let Fase::Concluida(r) = cx.update(|cx| c.nova.read(cx).fase.clone()) else {
        panic!("esperava concluida");
    };
    assert!(r.recuperada);
    assert_eq!(Notas::abrir(&c.notas).unwrap().listar().unwrap().len(), 1);
}

#[gpui_kit::test]
fn desistir_volta_a_editar_com_o_aviso_de_que_a_nota_pode_ter_sido_autorizada(
    cx: &mut TestAppContext,
) {
    let c = cenario(cx, "restrita", &[Desfecho::SemResposta], true, 1120.);
    preencher(cx, &c);
    clicar(cx, &c, "emitir");
    clicar(cx, &c, "confirmar-emissao");
    esperar(cx, &c, "sem resposta", |n| {
        matches!(n.fase, Fase::Pendente { .. })
    });
    clicar(cx, &c, "desistir-da-tentativa");
    // o dialogo avisa e pede confirmacao; ainda esta pendente
    cx.update(|cx| assert!(matches!(c.nova.read(cx).fase, Fase::Pendente { .. })));
    cx.update(|cx| c.nova.update(cx, |n, cx| n.desistir(cx)));
    cx.update(|cx| {
        let n = c.nova.read(cx);
        assert!(matches!(n.fase, Fase::Editando));
        let aviso = n.desistencia.clone().unwrap();
        assert!(aviso.contains("PODE ter sido autorizada"), "{aviso}");
        assert_eq!(n.dados.tomador_nome, "Maria de Teste");
    });
    assert!(!existe(cx, &c, "tentar-de-novo-emissao"));
}

#[gpui_kit::test]
fn depois_de_desistir_a_nota_antiga_que_volta_e_avisada_e_o_formulario_fica(
    cx: &mut TestAppContext,
) {
    let c = cenario(
        cx,
        "restrita",
        &[Desfecho::SemResposta, Desfecho::Recuperada],
        true,
        1120.,
    );
    preencher(cx, &c);
    clicar(cx, &c, "emitir");
    clicar(cx, &c, "confirmar-emissao");
    esperar(cx, &c, "sem resposta", |n| {
        matches!(n.fase, Fase::Pendente { .. })
    });
    cx.update(|cx| c.nova.update(cx, |n, cx| n.desistir(cx)));
    // outra nota no mesmo formulario: muda o valor e emite de novo, com o mesmo numero de DPS
    digitar(cx, &c, "nota-valor-servico", "999,00");
    clicar(cx, &c, "emitir");
    clicar(cx, &c, "confirmar-emissao");
    esperar(cx, &c, "a nota antiga", |n| {
        matches!(n.fase, Fase::Concluida(_))
    });
    let Fase::Concluida(r) = cx.update(|cx| c.nova.read(cx).fase.clone()) else {
        panic!("esperava concluida");
    };
    assert!(r.recuperada);
    assert_eq!(r.diferencas, ["valor do serviço"]);
    assert!(existe(cx, &c, "voltar-ao-formulario"));
    assert!(!existe(cx, &c, "nova-nota-limpa"));
    // a nota que existe e' a da primeira tentativa: foi guardada e gastou o numero
    let chamadas = c.emissor.chamadas();
    assert_eq!(chamadas.len(), 2);
    assert_eq!(chamadas[0].numero, chamadas[1].numero);
    assert_ne!(chamadas[0].valor, chamadas[1].valor);
    let notas = Notas::abrir(&c.notas).unwrap();
    let lista = notas.listar().unwrap();
    assert_eq!(lista.len(), 1);
    assert_eq!(lista[0].valor_servico, chamadas[0].valor);
    assert_eq!(notas.proximo_numero("1").unwrap(), 2);
    // o formulario volta como estava, para emitir a nota que faltou
    clicar(cx, &c, "voltar-ao-formulario");
    cx.update(|cx| {
        let n = c.nova.read(cx);
        assert!(matches!(n.fase, Fase::Editando));
        assert_eq!(n.dados.valor_servico, "999,00");
        assert_eq!(n.dados.tomador_nome, "Maria de Teste");
    });
}

#[gpui_kit::test]
fn sem_senha_guardada_a_tela_pede_a_senha_e_nao_a_guarda(cx: &mut TestAppContext) {
    let c = cenario(cx, "restrita", &[Desfecho::Autorizada], true, 1120.);
    c.cofre.apagar(CNPJ).unwrap();
    preencher(cx, &c);
    clicar(cx, &c, "emitir");
    clicar(cx, &c, "confirmar-emissao");
    // o certificado nao abre sem senha: aparece o dialogo, nada e' enviado
    esperar(cx, &c, "pedido de senha", |n| n.senha_pedida.is_some());
    assert!(existe(cx, &c, "senha-da-emissao"));
    assert_eq!(c.emissor.total_de_chamadas(), 0);
    cx.update(|cx| assert!(matches!(c.nova.read(cx).fase, Fase::Editando)));
    // senha errada: o dialogo continua, com o erro
    digitar(cx, &c, "senha-da-emissao", "errada");
    clicar(cx, &c, "usar-senha");
    let t0 = std::time::Instant::now();
    loop {
        let erro = cx.update(|cx| {
            c.nova
                .read(cx)
                .senha_pedida
                .as_ref()
                .and_then(|(d, _)| d.read(cx).erro.clone())
        });
        if erro.as_deref() == Some("Senha do certificado incorreta.") {
            break;
        }
        assert!(
            t0.elapsed() < Duration::from_secs(20),
            "demorou demais: senha incorreta"
        );
        std::thread::sleep(Duration::from_millis(20));
        cx.run_until_parked();
    }
    assert_eq!(c.emissor.total_de_chamadas(), 0);
    // senha certa: emite, e o cofre continua sem ela
    digitar(cx, &c, "senha-da-emissao", SENHA);
    clicar(cx, &c, "usar-senha");
    esperar(cx, &c, "autorizada", |n| {
        matches!(n.fase, Fase::Concluida(_))
    });
    assert_eq!(c.emissor.total_de_chamadas(), 1);
    assert!(
        c.cofre.ler(CNPJ).unwrap().is_none(),
        "a senha digitada nao pode ser guardada"
    );
}

#[gpui_kit::test]
fn configuracao_incompleta_mostra_o_que_falta_e_nenhum_formulario(cx: &mut TestAppContext) {
    let c = cenario(cx, "restrita", &[Desfecho::Autorizada], false, 1120.);
    assert!(existe(cx, &c, "ir-para-o-inicio"));
    assert!(existe(cx, &c, "ir-para-as-configuracoes"));
    assert!(!existe(cx, &c, "emitir"));
    assert!(!existe(cx, &c, "nota-tomador-documento"));
    clicar(cx, &c, "ir-para-o-inicio");
    cx.update(|cx| assert_eq!(c.raiz.read(cx).tela, Tela::Inicio));
}

#[gpui_kit::test]
fn inicio_com_a_configuracao_completa_leva_a_nova_nota(cx: &mut TestAppContext) {
    let c = cenario(cx, "restrita", &[Desfecho::Autorizada], true, 1120.);
    cx.update(|cx| {
        c.raiz.update(cx, |r, cx| {
            r.tela = Tela::Inicio;
            cx.notify();
        });
    });
    cx.run_until_parked();
    assert!(existe(cx, &c, "emitir-uma-nota"));
    clicar(cx, &c, "emitir-uma-nota");
    cx.update(|cx| assert_eq!(c.raiz.read(cx).tela, Tela::NovaNota));
    assert!(existe(cx, &c, "emitir"));
}

#[gpui_kit::test]
fn ctrl_n_abre_a_nova_nota_e_ctrl_enter_leva_ao_primeiro_campo_pendente(cx: &mut TestAppContext) {
    let c = cenario(cx, "restrita", &[Desfecho::Autorizada], true, 1120.);
    cx.update(|cx| {
        c.raiz.update(cx, |r, cx| {
            r.tela = Tela::Configuracoes;
            cx.notify();
        });
    });
    cx.run_until_parked();
    // o atalho vale com o foco dentro da janela (aqui, num campo das Configuracoes)
    clicar(cx, &c, "nome");
    cx.update_window(c.janela.into(), |_, window, cx| {
        window.render_frame(cx);
        window.press("ctrl-n", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| assert_eq!(c.raiz.read(cx).tela, Tela::NovaNota));
    // com pendencia, Ctrl+Enter mostra os erros e poe o foco no primeiro campo pendente
    clicar(cx, &c, "nota-tomador-nome");
    cx.update_window(c.janela.into(), |_, window, cx| {
        window.render_frame(cx);
        window.press("ctrl-enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let n = c.nova.read(cx);
        assert!(n.mostrar_tudo);
        assert!(n.erro_do(CampoNota::TomadorDocumento).is_some());
    });
    cx.update_window(c.janela.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("nota-tomador-documento").focused(), Some(true));
    })
    .unwrap();
    assert_eq!(c.emissor.total_de_chamadas(), 0);
}

#[gpui_kit::test]
fn clicar_numa_pendencia_poe_o_foco_no_campo(cx: &mut TestAppContext) {
    let c = cenario(cx, "restrita", &[Desfecho::Autorizada], true, 1120.);
    // o clique numa pendencia chama isto com o campo dela
    let primeiro = cx.update(|cx| {
        c.nova
            .read(cx)
            .conferencia
            .primeira()
            .unwrap()
            .campo
            .unwrap()
    });
    assert_eq!(primeiro, CampoNota::TomadorDocumento);
    let nova = c.nova.clone();
    cx.update_window(c.janela.into(), |_, window, cx| {
        window.render_frame(cx);
        nova.update(cx, |n, cx| n.ir_ao_campo(primeiro, window, cx));
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| assert!(c.nova.read(cx).mostrar_tudo));
    cx.update_window(c.janela.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("nota-tomador-documento").focused(), Some(true));
    })
    .unwrap();
}

#[gpui_kit::test]
fn erro_so_aparece_depois_de_tocar_no_campo(cx: &mut TestAppContext) {
    let c = cenario(cx, "restrita", &[Desfecho::Autorizada], true, 1120.);
    cx.update(|cx| {
        let n = c.nova.read(cx);
        // o formulario vazio nao nasce vermelho
        assert!(n.erro_do(CampoNota::TomadorDocumento).is_none());
        assert!(!n.conferencia.limpa());
    });
    // passar pelo campo (foco e saida) o marca como tocado
    clicar(cx, &c, "nota-tomador-documento");
    clicar(cx, &c, "nota-tomador-nome");
    assentar(cx, &c);
    // o evento de saida do campo (que o GPUI emite ao perder o foco) marca o campo como tocado
    cx.update(|cx| {
        c.nova
            .update(cx, |n, cx| n.ao_sair(CampoNota::TomadorDocumento, cx))
    });
    cx.update(|cx| {
        assert!(c
            .nova
            .read(cx)
            .erro_do(CampoNota::TomadorDocumento)
            .is_some());
        assert!(c.nova.read(cx).erro_do(CampoNota::ValorServico).is_none());
    });
}

#[gpui_kit::test]
fn rascunho_e_salvo_sozinho_e_volta_ao_reabrir(cx: &mut TestAppContext) {
    let c = cenario(cx, "restrita", &[Desfecho::Autorizada], true, 1120.);
    digitar(cx, &c, "nota-tomador-nome", "Maria de Teste");
    digitar(cx, &c, "nota-valor-servico", "99,9");
    // o rascunho espera uns instantes depois da ultima mudanca (o relogio do teste e' do executor)
    assert!(!c.notas.join("rascunho.json").exists());
    cx.executor().advance_clock(Duration::from_millis(900));
    cx.executor().allow_parking();
    let t0 = std::time::Instant::now();
    while !c.notas.join("rascunho.json").exists() {
        assert!(
            t0.elapsed() < Duration::from_secs(10),
            "o rascunho nunca foi salvo"
        );
        std::thread::sleep(Duration::from_millis(50));
        cx.run_until_parked();
    }
    esperar(cx, &c, "indicador", |n| {
        n.rascunho == super::nota::EstadoDoRascunho::Salvo
    });
    let texto = std::fs::read_to_string(c.notas.join("rascunho.json")).unwrap();
    assert!(texto.contains("Maria de Teste"));
    assert!(texto.contains("\"versao\": 1"));
    // reabre a janela com o mesmo arquivo: o rascunho volta
    let cofre: Arc<dyn Cofre> = c.cofre.clone();
    let servicos = Arc::new(
        Servicos::com(
            Some(c.arquivo.clone()),
            Arc::new(|_| None),
            cofre,
            false,
            Arc::new(SemConexao),
            Arc::new(chrono::Utc::now),
        )
        .com_fabrica(FabricaDemonstracao::nova(c.emissor.clone())),
    );
    let (janela2, raiz2) = abrir_janela(cx, servicos, 1120., 5200.);
    cx.run_until_parked();
    let nova2 = cx.update(|cx| raiz2.read(cx).nova.clone());
    esperar_em(cx, &nova2, "rascunho recuperado", |n| n.recuperado);
    cx.update(|cx| {
        let n = nova2.read(cx);
        assert_eq!(n.dados.tomador_nome, "Maria de Teste");
        assert_eq!(n.dados.valor_servico, "99,9");
    });
    // limpar o formulario apaga o rascunho
    cx.update_window(janela2.into(), |_, window, cx| {
        nova2.update(cx, |n, cx| n.limpar(window, cx));
    })
    .unwrap();
    esperar_em(cx, &nova2, "rascunho apagado", |n| {
        n.rascunho == super::nota::EstadoDoRascunho::Nenhum
    });
    let t0 = std::time::Instant::now();
    while c.notas.join("rascunho.json").exists() {
        assert!(
            t0.elapsed() < Duration::from_secs(10),
            "o rascunho nao foi apagado"
        );
        std::thread::sleep(Duration::from_millis(50));
        cx.run_until_parked();
    }
}

fn esperar_em(
    cx: &mut TestAppContext,
    nova: &Entity<NovaNota>,
    o_que: &str,
    pronto: impl Fn(&NovaNota) -> bool,
) {
    cx.executor().allow_parking();
    let t0 = std::time::Instant::now();
    loop {
        if cx.update(|cx| pronto(nova.read(cx))) {
            return;
        }
        assert!(
            t0.elapsed() < Duration::from_secs(20),
            "demorou demais: {o_que}"
        );
        std::thread::sleep(Duration::from_millis(20));
        cx.run_until_parked();
    }
}

#[gpui_kit::test]
fn limpar_pede_confirmacao_e_esvazia_o_formulario(cx: &mut TestAppContext) {
    let c = cenario(cx, "restrita", &[Desfecho::Autorizada], true, 1120.);
    digitar(cx, &c, "nota-tomador-nome", "Maria de Teste");
    clicar(cx, &c, "limpar");
    // o dialogo de confirmacao esta aberto e nada foi limpo ainda
    cx.update_window(c.janela.into(), |_, window, cx| {
        use gpui_kit::component::WindowExt as _;
        assert!(window.has_active_dialog(cx));
    })
    .unwrap();
    cx.update(|cx| assert_eq!(c.nova.read(cx).dados.tomador_nome, "Maria de Teste"));
    // confirmar limpa tudo
    cx.update_window(c.janela.into(), |_, window, cx| {
        c.nova.update(cx, |n, cx| n.limpar(window, cx));
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let n = c.nova.read(cx);
        assert!(n.dados.vazio(&n.ctx.hoje()));
    });
    cx.update_window(c.janela.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("nota-tomador-nome").value(), Some(""));
    })
    .unwrap();
}

/// A posicao do botao Emitir, que mora no painel.
fn posicao_do_emitir(cx: &mut TestAppContext, c: &Cenario) -> Bounds<gpui_kit::Pixels> {
    cx.update_window(c.janela.into(), |_, window, cx| {
        window.render_frame(cx);
        window.find("emitir").bounds()
    })
    .unwrap()
}

#[gpui_kit::test]
fn painel_fixo_ao_lado_nao_rola_junto_e_em_janela_estreita_vai_para_baixo(cx: &mut TestAppContext) {
    use gpui_kit::ScrollDelta;
    let larga = cenario_com_altura(cx, "restrita", &[Desfecho::Autorizada], true, 1120., 900.);
    let antes = posicao_do_emitir(cx, &larga);
    // ao lado: o botao fica na faixa da direita
    assert!(antes.origin.x > px(760.), "{antes:?}");
    // rolar o formulario nao move o painel
    cx.update_window(larga.janela.into(), |_, window, cx| {
        window.render_frame(cx);
        window.scroll(
            "nota-tomador-nome",
            ScrollDelta::Pixels(gpui_kit::point(px(0.), px(-600.))),
            cx,
        );
    })
    .unwrap();
    cx.run_until_parked();
    let depois = posicao_do_emitir(cx, &larga);
    assert_eq!(antes.origin.y, depois.origin.y, "o painel rolou junto");
    // estreita: o painel vai para baixo do formulario, na mesma coluna
    let estreita = cenario_com_altura(cx, "restrita", &[Desfecho::Autorizada], true, 940., 900.);
    let b = posicao_do_emitir(cx, &estreita);
    assert!(b.origin.x < px(700.), "{b:?}");
}
