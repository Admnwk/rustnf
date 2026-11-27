// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Testes da janela do Caixa (NFC-e), no ambiente de teste do GPUI, com a SEFAZ de mentira (nunca a rede), cofre em memoria e
//! pasta temporaria: a venda do inicio ao fim pelo teclado (autorizada, com o cupom gerado), a rejeitada que se corrige e sai
//! com o mesmo numero, a sem resposta que se retoma sem nota gemea, a SEFAZ fora antes de enviar (off-line, Fechar o caixa e
//! Transmitir com a MESMA chave), a producao sem a marcacao (zero chamadas), o cancelamento com o aviso do prazo, o CSC sem
//! cofre e o CSC guardado no cofre (nunca no arquivo) e o rascunho que sobrevive. Nenhum teste toca o cofre real, a pasta do
//! usuario nem a rede.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AppContext as _, Entity, TestAppContext, WindowHandle};
use rustnf_app::cofre::{Cofre, CofreEmMemoria};
use rustnf_app::config::Config;
use rustnf_app::notas::nfe::{NotasNfe, Situacao};
use rustnf_app::segredo::Segredo;

use super::caixa::{Fase as FaseCaixa, TelaCaixa, Vista};
use super::nfe::TelaNfe;
use super::nfe_dialogo::{DialogoNfe, Fase as FaseDialogo};
use super::notas::Carga;
use super::testes_nota::{abrir_janela, SemConexao, CNPJ, SENHA};
use super::testes_notas::esperar_ate;
use super::{Raiz, Secao, Tela};
use crate::demonstracao_nfce::{semear_nfce, NotasDoEnsaioNfce, RoteiroNfce};
use crate::demonstracao_nfe::{FabricaNfeDemonstracao, RoteiroNfe, TransporteNfeDemonstracao};
use crate::modelo::nfce_config::EstadoCsc;
use crate::modelo::nfce_desfecho::DesfechoCaixa;
use crate::modelo::nfe_lista::EscolhaDeNotas;
use crate::servicos::{Destino, Servicos};
use crate::testes_comuns::{AbridorDeTeste, PastaTemporaria};

const AUT: &str = "NFeAutorizacao4";
const STA: &str = "NFeStatusServico4";
const CSC_DE_TESTE: &str = "ZZTESTECSC0123456789ABCDEF000000";

struct Cenario {
    janela: WindowHandle<gpui_kit::base::Root>,
    raiz: Entity<Raiz>,
    caixa: Entity<TelaCaixa>,
    lista: Entity<TelaNfe>,
    dialogo: Entity<DialogoNfe>,
    transporte: TransporteNfeDemonstracao,
    abridor: Arc<AbridorDeTeste>,
    cofre: Arc<CofreEmMemoria>,
    notas: PathBuf,
    arquivo: PathBuf,
    ensaio: Option<NotasDoEnsaioNfce>,
    servicos: Arc<Servicos>,
    config: Config,
    _pasta: PastaTemporaria,
}

fn arquivo(pasta: &PastaTemporaria, pfx: &std::path::Path, nfce: &str) -> Config {
    let texto = format!(
        "[emitente]\ncnpj = \"{CNPJ}\"\nnome = \"LOJA DE TESTE LTDA\"\ninscricao_municipal = \"123456\"\nmunicipio = \"3550308\"\nop_simples = 3\nregime_apuracao = 1\n\
         [sefin]\npfx = '{}'\n[notas]\npasta = '{}'\n\
         [nfe]\nambiente = \"homologacao\"\nuf = \"SP\"\nie = \"111111111111\"\ncrt = 1\nserie = \"1\"\n\
         [nfe.endereco]\nlogradouro = \"Rua das Flores\"\nnumero = \"100\"\nbairro = \"Centro\"\nmunicipio = \"Sao Paulo\"\ncep = \"01001000\"\n\
         [nfce]\nserie = \"1\"\n{nfce}",
        pfx.to_string_lossy(),
        pasta.caminho().join("notas").to_string_lossy()
    );
    Config::de_texto(&texto).unwrap()
}

/// `nfce`: o que vai na `[nfce]` alem da serie; `semeado`: a pasta ganha as quatro NFC-e de ensaio e o cadastro.
fn cenario(
    cx: &mut TestAppContext,
    nfce: &str,
    roteiro: RoteiroNfce,
    semeado: bool,
    cofre_desligado: bool,
) -> Cenario {
    cx.executor().allow_parking();
    let pasta = PastaTemporaria::nova("janela-caixa");
    let pfx = pasta.certificado("a.pfx", CNPJ, SENHA, 365);
    let config = arquivo(&pasta, &pfx, nfce);
    let arquivo_toml = pasta.caminho().join("rustnf.toml");
    config.salvar(&arquivo_toml).unwrap();
    let transporte =
        TransporteNfeDemonstracao::novo(RoteiroNfe::Registrado, Duration::from_millis(30))
            .com_nfce(roteiro);
    let ensaio = semeado.then(|| semear_nfce(&config, &transporte).unwrap());
    if !semeado {
        crate::demonstracao_nfe::semear_produtos(&config);
    }
    let cofre = Arc::new(CofreEmMemoria::novo());
    cofre.guardar(CNPJ, &Segredo::novo(SENHA.into())).unwrap();
    let abridor = Arc::new(AbridorDeTeste::default());
    let cofre_dyn: Arc<dyn Cofre> = cofre.clone();
    let servicos = Arc::new(
        Servicos::com(
            Some(arquivo_toml.clone()),
            Arc::new(|_| None),
            cofre_dyn,
            cofre_desligado,
            Arc::new(SemConexao),
            Arc::new(chrono::Utc::now),
        )
        .com_fabrica_nfe(FabricaNfeDemonstracao::nova(transporte.clone()))
        .com_abridor(abridor.clone())
        .com_intervalo(Duration::ZERO),
    );
    let (janela, raiz) = abrir_janela(cx, Arc::clone(&servicos), 1300., 2400.);
    cx.run_until_parked();
    let (caixa, lista, dialogo) = cx.update(|cx| {
        let r = raiz.read(cx);
        (r.caixa.clone(), r.nfce.clone(), r.dialogo_nfce.clone())
    });
    Cenario {
        notas: pasta.caminho().join("notas"),
        arquivo: arquivo_toml,
        janela,
        raiz,
        caixa,
        lista,
        dialogo,
        transporte,
        abridor,
        cofre,
        ensaio,
        servicos,
        config,
        _pasta: pasta,
    }
}

fn mostrar_o_caixa(cx: &mut TestAppContext, c: &Cenario) {
    cx.update(|cx| c.raiz.update(cx, |r, cx| r.ir_para(Tela::Caixa, cx)));
    cx.run_until_parked();
    let caixa = c.caixa.clone();
    esperar_ate(cx, "o caixa pronto", move |cx| {
        cx.update(|cx| {
            let k = caixa.read(cx);
            k.pronta && !k.cadastro.produtos.is_empty() && k.pendentes_lidos
        })
    });
}

fn clicar(cx: &mut TestAppContext, c: &Cenario, id: &'static str) {
    cx.update_window(c.janela.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click(id, cx);
    })
    .unwrap();
    cx.run_until_parked();
}

fn digitar(cx: &mut TestAppContext, c: &Cenario, id: &'static str, texto: &str) {
    cx.update_window(c.janela.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click(id, cx);
        window.press("ctrl-a", cx);
        window.input(texto, cx);
    })
    .unwrap();
    cx.run_until_parked();
}

fn tecla(cx: &mut TestAppContext, c: &Cenario, tecla: &'static str) {
    cx.update_window(c.janela.into(), |_, window, cx| {
        window.render_frame(cx);
        window.press(tecla, cx);
    })
    .unwrap();
    cx.run_until_parked();
}

fn existe(cx: &mut TestAppContext, c: &Cenario, id: &'static str) -> bool {
    cx.update_window(c.janela.into(), |_, window, cx| {
        window.render_frame(cx);
        window.try_find(id).is_some()
    })
    .unwrap()
}

fn notas_da_pasta(c: &Cenario) -> Vec<(String, String, Situacao)> {
    NotasNfe::abrir_nfce(&c.notas)
        .unwrap()
        .listar()
        .unwrap()
        .iter()
        .map(|r| (r.numero.clone(), r.chave.clone(), r.situacao))
        .collect()
}

fn desfecho(cx: &mut TestAppContext, c: &Cenario) -> Option<DesfechoCaixa> {
    cx.update(|cx| c.caixa.read(cx).desfecho.clone())
}

fn esperar_o_desfecho(cx: &mut TestAppContext, c: &Cenario) -> DesfechoCaixa {
    let caixa = c.caixa.clone();
    esperar_ate(cx, "o desfecho da venda", move |cx| {
        cx.update(|cx| {
            let k = caixa.read(cx);
            k.desfecho.is_some() && matches!(k.fase, FaseCaixa::Vendendo)
        })
    });
    desfecho(cx, c).unwrap()
}

/// Uma venda de dois cafes (R$ 31,80), toda pelo teclado: busca, Enter, valor recebido, Enter.
fn vender_dois_cafes_com_cinquenta(cx: &mut TestAppContext, c: &Cenario) {
    digitar(cx, c, "caixa-quantidade", "2");
    digitar(cx, c, "caixa-busca", "C1");
    tecla(cx, c, "enter");
    digitar(cx, c, "caixa-recebido", "50");
    tecla(cx, c, "enter");
}

fn esperar_o_dialogo(cx: &mut TestAppContext, c: &Cenario) {
    let d = c.dialogo.clone();
    esperar_ate(cx, "o resultado do dialogo", move |cx| {
        cx.update(|cx| matches!(d.read(cx).fase, FaseDialogo::Desfecho(_)))
    });
}

// ---------------------------------------------------------------- a venda

#[gpui_kit::test]
fn venda_inteira_pelo_teclado_autorizada_e_o_cupom_sai(cx: &mut TestAppContext) {
    let c = cenario(cx, "", RoteiroNfce::Autorizada, false, false);
    mostrar_o_caixa(cx, &c);
    assert!(
        !existe(cx, &c, "caixa-entendo"),
        "em homologacao nao ha marcacao"
    );
    vender_dois_cafes_com_cinquenta(cx, &c);
    let (itens, total, troco, pode) = cx.update(|cx| {
        let k = c.caixa.read(cx);
        (
            k.cupom.itens.len(),
            k.cupom.totais().total,
            k.cupom.troco(),
            k.pode_emitir(),
        )
    });
    assert_eq!((itens, total, troco, pode), (1, 3180, 1820, true));
    // (o Div do troco nao tem interacao: o estado se confere pelo modelo, acima)
    tecla(cx, &c, "ctrl-enter");
    let d = esperar_o_desfecho(cx, &c);
    assert!(matches!(d, DesfechoCaixa::Autorizada { .. }), "{d:?}");
    // pronto para a proxima venda
    assert!(cx.update(|cx| c.caixa.read(cx).cupom.vazio()));
    let notas = notas_da_pasta(&c);
    assert_eq!(notas.len(), 1);
    assert_eq!(notas[0].2, Situacao::Autorizada);
    assert_eq!(c.transporte.chamadas_de(AUT), 1);
    // o cupom como arquivo
    clicar(cx, &c, "caixa-cupom-abrir");
    let abridor = c.abridor.clone();
    esperar_ate(cx, "o cupom", move |_| !abridor.pedidos().is_empty());
    let (destino, caminho) = c.abridor.pedidos()[0].clone();
    assert_eq!(destino, Destino::Abrir);
    assert_eq!(caminho.extension().and_then(|e| e.to_str()), Some("pdf"));
    assert!(std::fs::read(&caminho).unwrap().starts_with(b"%PDF"));
    // ESC/POS ao lado do PDF
    clicar(cx, &c, "caixa-cupom-escpos");
    let abridor = c.abridor.clone();
    esperar_ate(cx, "o ESC/POS", move |_| abridor.pedidos().len() == 2);
    let (destino, caminho) = c.abridor.pedidos()[1].clone();
    assert_eq!(destino, Destino::Mostrar);
    assert_eq!(caminho.extension().and_then(|e| e.to_str()), Some("escpos"));
    assert!(!std::fs::read(&caminho).unwrap().is_empty());
    // o desfecho da autorizada fica pronto para a proxima: uma nova venda apaga o aviso
    digitar(cx, &c, "caixa-busca", "C2");
    tecla(cx, &c, "enter");
    assert!(cx.update(|cx| c.caixa.read(cx).desfecho.is_none()));
}

#[gpui_kit::test]
fn rejeitada_volta_ao_cupom_e_a_seguinte_sai_com_o_mesmo_numero(cx: &mut TestAppContext) {
    let c = cenario(cx, "", RoteiroNfce::Rejeitada, false, false);
    mostrar_o_caixa(cx, &c);
    vender_dois_cafes_com_cinquenta(cx, &c);
    tecla(cx, &c, "ctrl-enter");
    let d = esperar_o_desfecho(cx, &c);
    let DesfechoCaixa::Rejeitada {
        cstat,
        motivo,
        numero,
    } = d
    else {
        panic!("{d:?}")
    };
    assert_eq!(cstat, "778");
    assert!(motivo.contains("NCM"), "{motivo}");
    assert!(notas_da_pasta(&c).is_empty(), "a rejeitada saiu da pasta");
    // o cupom continua para a correcao e a venda tenta de novo
    assert_eq!(cx.update(|cx| c.caixa.read(cx).cupom.itens.len()), 1);
    clicar(cx, &c, "caixa-fechar-aviso");
    tecla(cx, &c, "ctrl-enter");
    let d = esperar_o_desfecho(cx, &c);
    let DesfechoCaixa::Autorizada { numero: n2, .. } = d else {
        panic!("{d:?}")
    };
    assert_eq!(n2, numero, "o numero voltou livre");
    assert_eq!(notas_da_pasta(&c).len(), 1);
}

#[gpui_kit::test]
fn sem_resposta_se_retoma_com_a_mesma_chave_sem_nota_gemea(cx: &mut TestAppContext) {
    let c = cenario(cx, "", RoteiroNfce::SemResposta, false, false);
    mostrar_o_caixa(cx, &c);
    vender_dois_cafes_com_cinquenta(cx, &c);
    tecla(cx, &c, "ctrl-enter");
    let d = esperar_o_desfecho(cx, &c);
    let DesfechoCaixa::SemResposta { chave, .. } = d else {
        panic!("{d:?}")
    };
    assert!(
        !existe(cx, &c, "caixa-cupom-abrir"),
        "sem resposta nao tem cupom"
    );
    let notas = notas_da_pasta(&c);
    assert_eq!(notas.len(), 1);
    assert_eq!(
        (notas[0].1.as_str(), notas[0].2),
        (chave.as_str(), Situacao::Pendente)
    );
    // o caminho e' o Retomar, nunca emitir de novo
    clicar(cx, &c, "caixa-retomar");
    clicar(cx, &c, "nfe-dialogo-enviar");
    esperar_o_dialogo(cx, &c);
    let notas = notas_da_pasta(&c);
    assert_eq!(notas.len(), 1, "nenhuma nota gemea");
    assert_eq!(
        (notas[0].1.as_str(), notas[0].2),
        (chave.as_str(), Situacao::Autorizada)
    );
    assert_eq!(c.transporte.chamadas_de(AUT), 1, "so' a tentativa original");
}

#[gpui_kit::test]
fn sefaz_fora_antes_de_enviar_sai_off_line_e_o_fechamento_transmite_a_mesma_chave(
    cx: &mut TestAppContext,
) {
    let c = cenario(cx, "", RoteiroNfce::Fora, false, false);
    mostrar_o_caixa(cx, &c);
    vender_dois_cafes_com_cinquenta(cx, &c);
    tecla(cx, &c, "ctrl-enter");
    let d = esperar_o_desfecho(cx, &c);
    let DesfechoCaixa::Offline { chave, prazo, .. } = d else {
        panic!("{d:?}")
    };
    assert!(!prazo.is_empty());
    assert_eq!(c.transporte.chamadas_de(AUT), 0, "nada foi enviado");
    assert_eq!(c.transporte.chamadas_de(STA), 1);
    assert!(existe(cx, &c, "caixa-cupom-abrir"), "o cupom vale");
    let notas = notas_da_pasta(&c);
    assert_eq!(
        (notas[0].1.as_str(), notas[0].2),
        (chave.as_str(), Situacao::Pendente)
    );
    let caixa = c.caixa.clone();
    esperar_ate(cx, "o pendente no fechamento", move |cx| {
        cx.update(|cx| caixa.read(cx).pendentes.len() == 1)
    });
    // Fechar o caixa
    clicar(cx, &c, "caixa-fechar");
    assert_eq!(cx.update(|cx| c.caixa.read(cx).vista), Vista::Fechamento);
    assert!(existe(cx, &c, "caixa-transmitir"));
    assert_eq!(cx.update(|cx| c.caixa.read(cx).pendentes.len()), 1);
    // a retomada so' mexe em nota com mais de 1 segundo de vida (politica da biblioteca): a nota off-line real tem horas
    std::thread::sleep(Duration::from_millis(2100));
    clicar(cx, &c, "caixa-transmitir");
    clicar(cx, &c, "nfe-dialogo-enviar");
    esperar_o_dialogo(cx, &c);
    let notas = notas_da_pasta(&c);
    assert_eq!(notas.len(), 1);
    assert_eq!(
        (notas[0].1.as_str(), notas[0].2),
        (chave.as_str(), Situacao::Autorizada)
    );
    assert_eq!(
        c.transporte.chamadas_de(AUT),
        1,
        "a MESMA nota foi reenviada"
    );
    let caixa = c.caixa.clone();
    esperar_ate(cx, "o fechamento limpo", move |cx| {
        cx.update(|cx| caixa.read(cx).pendentes.is_empty())
    });
    assert!(existe(cx, &c, "caixa-voltar-a-venda"));
}

// ---------------------------------------------------------------- producao, CSC e rascunho

#[gpui_kit::test]
fn em_producao_sem_a_marcacao_nenhuma_chamada_sai(cx: &mut TestAppContext) {
    let c = cenario(
        cx,
        "ambiente = \"producao\"\n",
        RoteiroNfce::Autorizada,
        false,
        false,
    );
    mostrar_o_caixa(cx, &c);
    vender_dois_cafes_com_cinquenta(cx, &c);
    assert!(existe(cx, &c, "caixa-entendo"));
    let motivo = cx.update(|cx| c.caixa.read(cx).por_que_nao_emite().unwrap());
    assert!(motivo.contains("efeito fiscal"), "{motivo}");
    clicar(cx, &c, "caixa-emitir");
    tecla(cx, &c, "ctrl-enter");
    assert!(
        c.transporte.chamadas().is_empty(),
        "{:?}",
        c.transporte.chamadas()
    );
    assert!(desfecho(cx, &c).is_none());
    // com a marcacao a venda sai, e a marcacao vale para a proxima (a sessao do caixa)
    clicar(cx, &c, "caixa-entendo");
    tecla(cx, &c, "ctrl-enter");
    assert!(matches!(
        esperar_o_desfecho(cx, &c),
        DesfechoCaixa::Autorizada { .. }
    ));
    assert!(cx.update(|cx| c.caixa.read(cx).entendo));
}

#[gpui_kit::test]
fn csc_sem_cofre_explica_e_o_caixa_nao_emite(cx: &mut TestAppContext) {
    let c = cenario(
        cx,
        "qr_versao = 2\nid_csc = \"000001\"\n",
        RoteiroNfce::Autorizada,
        false,
        true,
    );
    mostrar_o_caixa(cx, &c);
    assert_eq!(cx.update(|cx| c.raiz.read(cx).csc), EstadoCsc::SemCofre);
    vender_dois_cafes_com_cinquenta(cx, &c);
    let motivo = cx.update(|cx| c.caixa.read(cx).por_que_nao_emite().unwrap());
    assert!(
        motivo.contains("cofre de senhas está desligado"),
        "{motivo}"
    );
    assert!(existe(cx, &c, "caixa-ir-para-csc"));
    clicar(cx, &c, "caixa-emitir");
    tecla(cx, &c, "ctrl-enter");
    assert!(c.transporte.chamadas().is_empty());
    assert!(notas_da_pasta(&c).is_empty());
    // a tela de Configuracoes diz o mesmo e nao oferece guardar
    cx.update(|cx| {
        c.raiz.update(cx, |r, cx| {
            r.ir_para(Tela::Configuracoes, cx);
            r.secao = Secao::Nfce;
        })
    });
    cx.run_until_parked();
    clicar(cx, &c, "nfce-guardar-csc");
    assert!(
        cx.update(|cx| c.raiz.read(cx).resultado_csc.is_none()),
        "o botao esta desligado"
    );
}

#[gpui_kit::test]
fn o_csc_vai_para_o_cofre_e_nunca_para_o_arquivo(cx: &mut TestAppContext) {
    let c = cenario(
        cx,
        "qr_versao = 2\nid_csc = \"000001\"\n",
        RoteiroNfce::Autorizada,
        false,
        false,
    );
    mostrar_o_caixa(cx, &c);
    assert_eq!(cx.update(|cx| c.raiz.read(cx).csc), EstadoCsc::NaoGuardado);
    cx.update(|cx| {
        c.raiz.update(cx, |r, cx| {
            r.ir_para(Tela::Configuracoes, cx);
            r.secao = Secao::Nfce;
        })
    });
    cx.run_until_parked();
    digitar(cx, &c, "nfce-csc", CSC_DE_TESTE);
    clicar(cx, &c, "nfce-guardar-csc");
    let raiz = c.raiz.clone();
    esperar_ate(cx, "o CSC guardado", move |cx| {
        cx.update(|cx| {
            let r = raiz.read(cx);
            !r.ocupado.csc && r.csc == EstadoCsc::Guardado
        })
    });
    let chave = format!("csc:{CNPJ}:homologacao:000001");
    assert_eq!(c.cofre.ler(&chave).unwrap().unwrap().expor(), CSC_DE_TESTE);
    let arquivo = std::fs::read_to_string(&c.arquivo).unwrap();
    assert!(
        !arquivo.contains(CSC_DE_TESTE),
        "o CSC nao vai para o arquivo"
    );
    // com o CSC guardado o caixa vende pela versao 2 do QR Code
    mostrar_o_caixa(cx, &c);
    vender_dois_cafes_com_cinquenta(cx, &c);
    tecla(cx, &c, "ctrl-enter");
    let d = esperar_o_desfecho(cx, &c);
    assert!(matches!(d, DesfechoCaixa::Autorizada { .. }), "{d:?}");
    let pasta = NotasNfe::abrir_nfce(&c.notas).unwrap();
    let chave = pasta.listar().unwrap()[0].chave.clone();
    let xml = pasta.ler_xml(&chave).unwrap();
    assert!(!xml.contains(CSC_DE_TESTE), "o CSC nao vai para a nota");
    // apagar
    cx.update(|cx| {
        c.raiz
            .update(cx, |r, cx| r.ir_para(Tela::Configuracoes, cx))
    });
    cx.run_until_parked();
    clicar(cx, &c, "nfce-apagar-csc");
    let raiz = c.raiz.clone();
    esperar_ate(cx, "o CSC apagado", move |cx| {
        cx.update(|cx| {
            let r = raiz.read(cx);
            !r.ocupado.csc && r.csc == EstadoCsc::NaoGuardado
        })
    });
    assert!(c.cofre.ler(&chave).unwrap().is_none());
}

#[gpui_kit::test]
fn o_rascunho_do_cupom_sobrevive_a_fechar_a_janela(cx: &mut TestAppContext) {
    let c = cenario(cx, "", RoteiroNfce::Autorizada, false, false);
    mostrar_o_caixa(cx, &c);
    digitar(cx, &c, "caixa-busca", "F103");
    tecla(cx, &c, "enter");
    // o rascunho espera uns instantes depois da ultima mudanca (o relogio do teste e' do executor)
    cx.executor().advance_clock(Duration::from_millis(900));
    let (servicos, config) = (Arc::clone(&c.servicos), c.config.clone());
    esperar_ate(cx, "o rascunho gravado", move |_| {
        servicos.ler_rascunho_caixa(&config).is_some()
    });
    // outra janela sobre a mesma pasta
    let (janela2, raiz2) = abrir_janela(cx, Arc::clone(&c.servicos), 1300., 2400.);
    cx.run_until_parked();
    let caixa2 = cx.update(|cx| raiz2.read(cx).caixa.clone());
    cx.update(|cx| raiz2.update(cx, |r, cx| r.ir_para(Tela::Caixa, cx)));
    let caixa = caixa2.clone();
    esperar_ate(cx, "o cupom restaurado", move |cx| {
        cx.update(|cx| caixa.read(cx).cupom.itens.len() == 1)
    });
    let _ = janela2;
    // o cupom emitido apaga o rascunho: esvaziar tambem
    cx.update_window(c.janela.into(), |_, window, cx| {
        c.caixa.update(cx, |k, cx| k.limpar(window, cx));
    })
    .unwrap();
    cx.executor().advance_clock(Duration::from_millis(900));
    let (servicos, config) = (Arc::clone(&c.servicos), c.config.clone());
    esperar_ate(cx, "o rascunho apagado", move |_| {
        servicos.ler_rascunho_caixa(&config).is_none()
    });
}

// ---------------------------------------------------------------- a lista e o cancelamento

fn mostrar_a_lista(cx: &mut TestAppContext, c: &Cenario, total: usize) {
    cx.update(|cx| {
        c.raiz.update(cx, |r, cx| {
            r.ir_para(Tela::Notas, cx);
            r.escolher_notas(EscolhaDeNotas::Nfce, cx);
        })
    });
    cx.run_until_parked();
    let lista = c.lista.clone();
    esperar_ate(cx, "a lista da NFC-e", move |cx| {
        cx.update(|cx| {
            let n = lista.read(cx);
            n.carga == Carga::Pronta && n.lista.total() == total
        })
    });
}

fn abrir_a_nota(cx: &mut TestAppContext, c: &Cenario, chave: &str) {
    let lista = c.lista.clone();
    let alvo = chave.to_string();
    let i = cx
        .update(|cx| {
            let n = lista.read(cx);
            (0..n.lista.quantas()).find(|i| n.lista.linha(*i).is_some_and(|l| l.chave == alvo))
        })
        .expect("a nota esta na lista");
    cx.update_window(c.janela.into(), |_, window, cx| {
        window.render_frame(cx);
        lista.update(cx, |n, cx| n.abrir(i, window, cx));
    })
    .unwrap();
    cx.run_until_parked();
    let lista = c.lista.clone();
    esperar_ate(cx, "o detalhe da NFC-e", move |cx| {
        cx.update(|cx| lista.read(cx).detalhe_pronto())
    });
}

#[gpui_kit::test]
fn a_lista_da_nfce_mostra_o_off_line_e_cancela_com_o_aviso_do_prazo(cx: &mut TestAppContext) {
    let c = cenario(cx, "", RoteiroNfce::Autorizada, true, false);
    let e = c.ensaio.clone().unwrap();
    mostrar_a_lista(cx, &c, 4);
    // a off-line aparece como "sem internet" com o Retomar
    let (off, a_resolver) = cx.update(|cx| {
        let l = &c.lista.read(cx).lista;
        let linha = (0..l.quantas())
            .filter_map(|i| l.linha(i))
            .find(|x| x.chave == e.offline);
        (linha.map(|x| (x.offline, x.rotulo)), l.a_resolver())
    });
    assert_eq!(off, Some((true, "Sem internet (off-line)")));
    assert_eq!(a_resolver, 2, "a off-line e a sem resposta");
    assert!(existe(cx, &c, "nfe-retomar"));
    // o detalhe da autorizada: cupom, sem carta de correcao nem modelo, e o cancelar com o aviso ANTES de enviar
    abrir_a_nota(cx, &c, &e.autorizada);
    assert!(existe(cx, &c, "nfe-abrir-danfe"));
    assert!(
        !existe(cx, &c, "nfe-corrigir"),
        "a NFC-e nao tem carta de correcao"
    );
    assert!(!existe(cx, &c, "nfe-usar-como-modelo"));
    assert!(
        !existe(cx, &c, "nfe-inutilizar"),
        "a aplicacao nao inutiliza numero da NFC-e"
    );
    clicar(cx, &c, "nfe-cancelar");
    let aviso = cx.update(|cx| c.dialogo.read(cx).aviso_de_prazo.clone());
    assert!(
        aviso.is_some_and(|a| a.contains("24 horas")),
        "o prazo nacional de 24 horas ja passou"
    );
    assert_eq!(c.transporte.chamadas().len(), 0, "o aviso nao envia nada");
    let d = c.dialogo.clone();
    cx.update_window(c.janela.into(), |_, window, cx| {
        window.render_frame(cx);
        let campo = d.read(cx).texto.clone();
        campo.update(cx, |e, cx| e.focus(window, cx));
        window.input("Cancelamento de teste do caixa", cx);
    })
    .unwrap();
    cx.run_until_parked();
    clicar(cx, &c, "nfe-dialogo-enviar");
    esperar_o_dialogo(cx, &c);
    let situacao = NotasNfe::abrir_nfce(&c.notas)
        .unwrap()
        .registro(&e.autorizada)
        .unwrap()
        .situacao;
    assert_eq!(situacao, Situacao::Cancelada, "o aviso nunca bloqueia");
}

#[gpui_kit::test]
fn conferir_na_sefaz_e_retomar_a_off_line_pela_lista(cx: &mut TestAppContext) {
    let c = cenario(cx, "", RoteiroNfce::Autorizada, true, false);
    let e = c.ensaio.clone().unwrap();
    mostrar_a_lista(cx, &c, 4);
    abrir_a_nota(cx, &c, &e.offline);
    assert!(existe(cx, &c, "nfe-retomar-esta"));
    clicar(cx, &c, "nfe-retomar-esta");
    clicar(cx, &c, "nfe-dialogo-enviar");
    esperar_o_dialogo(cx, &c);
    let reg = NotasNfe::abrir_nfce(&c.notas)
        .unwrap()
        .registro(&e.offline)
        .unwrap();
    assert_eq!(
        reg.situacao,
        Situacao::Autorizada,
        "a MESMA chave foi transmitida"
    );
    // conferir uma nota autorizada
    abrir_a_nota(cx, &c, &e.autorizada);
    clicar(cx, &c, "nfe-conferir");
    esperar_o_dialogo(cx, &c);
}
