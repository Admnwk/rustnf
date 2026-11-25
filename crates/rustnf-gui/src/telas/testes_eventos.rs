// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Testes da janela dos eventos na tela: conferir a situacao das notas na Sefin, o historico de eventos de uma nota,
//! o pedido de analise fiscal do cancelamento, a consulta pela DPS e a nota recebida (confirmar e recusar). Tudo com o
//! emissor de demonstracao (nunca a Sefin), cofre em memoria e pasta temporaria.

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AppContext as _, Entity, TestAppContext};
use rustnf::chave_dfe;
use rustnf::nfse::sefin::Status;
use rustnf_app::emissao::Emissor;
use rustnf_app::notas::{Registro, Situacao};

use super::conferir::{ConferirNaSefin, Fase as FaseDaConferencia};
use super::historico::Fase as FaseDoHistorico;
use super::testes_nota::{cenario_de_eventos, clicar, digitar, existe, Cenario, CNPJ};
use super::testes_notas::{
    abrir_o_detalhe, autorizar, desfecho_do_cancelamento, dps, escrever_a_justificativa,
    esperar_a_consulta, esperar_a_lista, esperar_ate, ir_para, rodar, semear, situacoes,
    tela_notas,
};
use super::Tela;
use crate::demonstracao::{Cancelamento, Desfecho};
use crate::demonstracao_eventos::RegistroDeEvento;
use crate::modelo::cancelamento::DesfechoDoCancelamento;
use crate::modelo::chave;
use crate::modelo::conferir::DesfechoDoLote;
use crate::modelo::consulta::DesfechoDaConsulta;
use crate::modelo::consulta_dps::ModoDeConsulta;
use crate::modelo::evento::TipoDeEvento;
use crate::modelo::historico::DesfechoDoHistorico;

const JUSTIFICATIVA: &str = "O servico foi lancado com o valor errado";

fn cenario(
    cx: &mut TestAppContext,
    ambiente: &str,
    cancelamento: &[Cancelamento],
    evento: &[RegistroDeEvento],
    recebida: bool,
) -> Cenario {
    cenario_de_eventos(
        cx,
        ambiente,
        &[Desfecho::Autorizada],
        cancelamento,
        evento,
        recebida,
        true,
        1120.,
        5200.,
    )
}

fn conferencia(cx: &mut TestAppContext, c: &Cenario) -> Entity<ConferirNaSefin> {
    let notas = tela_notas(cx, c);
    cx.update(|cx| notas.read(cx).conferir.clone())
}

fn fase_da_conferencia(cx: &mut TestAppContext, c: &Cenario) -> FaseDaConferencia {
    let conf = conferencia(cx, c);
    cx.update(|cx| conf.read(cx).fase.clone())
}

fn esperar_o_lote(cx: &mut TestAppContext, c: &Cenario) -> DesfechoDoLote {
    esperar_ate(cx, "o resultado do lote", |cx| {
        matches!(fase_da_conferencia(cx, c), FaseDaConferencia::Resultado(_))
    });
    match fase_da_conferencia(cx, c) {
        FaseDaConferencia::Resultado(d) => d,
        outra => panic!("esperava o resultado, veio {outra:?}"),
    }
}

/// Registra, pelo emissor de demonstracao, um evento que a Sefin "ja tem" da nota (a pasta nao sabe).
fn registrar_na_sefin(c: &Cenario, tipo: &str, chave: &str) {
    let r = rodar(Emissor::registrar_evento(
        &*c.emissor,
        tipo,
        chave,
        "1",
        JUSTIFICATIVA,
    ));
    assert_eq!(r.status, Status::Registrado, "{}", r.frase);
}

fn abrir_a_nota_da_lista(cx: &mut TestAppContext, c: &Cenario, total: usize, chave: &str) {
    ir_para(cx, c, Tela::Notas);
    esperar_a_lista(cx, c, total);
    abrir_o_detalhe(cx, c, 0, chave);
}

fn dialogo(cx: &mut TestAppContext, c: &Cenario) -> Entity<super::cancelar::Cancelamento> {
    cx.update(|cx| c.raiz.read(cx).cancelamento.clone())
}

fn consultar_tela(cx: &mut TestAppContext, c: &Cenario) -> Entity<super::consultar::TelaConsultar> {
    cx.update(|cx| c.raiz.read(cx).consultar.clone())
}

// ---------------------------------------------------------------- conferir na Sefin

#[gpui_kit::test]
fn conferir_na_sefin_muda_a_situacao_e_a_lista_atualiza_sozinha(cx: &mut TestAppContext) {
    let c = cenario(cx, "restrita", &[], &[], false);
    let a = semear(&c, 1, "52998224725", "Maria de Teste", "100.50");
    let _b = semear(&c, 2, "12345678909", "Joao de Teste", "200.00");
    ir_para(cx, &c, Tela::Notas);
    esperar_a_lista(cx, &c, 2);
    // a Sefin tem um pedido de analise fiscal da nota 1 que a pasta nao sabe
    registrar_na_sefin(&c, "101103", &a);
    assert_eq!(situacoes(&c), vec![Situacao::Autorizada; 2]);
    clicar(cx, &c, "conferir-na-sefin");
    let DesfechoDoLote::Feito(r) = esperar_o_lote(cx, &c) else {
        panic!("esperava o lote feito");
    };
    assert_eq!((r.total, r.conferidas, r.parado), (2, 2, false));
    assert_eq!(r.mudaram.len(), 1);
    assert_eq!(r.mudaram[0].chave, a);
    assert_eq!(
        (r.mudaram[0].de, r.mudaram[0].para),
        (Situacao::Autorizada, Situacao::AnaliseFiscal)
    );
    assert_eq!(r.iguais, 1);
    assert!(r.falhas.is_empty());
    assert!(r.resumo().starts_with("Conferi 2 notas."), "{}", r.resumo());
    // a pasta gravou, e a lista da tela mostra a nota em analise fiscal sem ninguem apertar Atualizar
    let notas = tela_notas(cx, &c);
    esperar_ate(cx, "a lista atualizada", |cx| {
        cx.update(|cx| {
            let n = notas.read(cx);
            (0..2).any(|i| {
                n.lista
                    .linha(i)
                    .is_some_and(|l| l.situacao == Situacao::AnaliseFiscal)
            })
        })
    });
    assert_eq!(
        situacoes(&c)
            .iter()
            .filter(|s| **s == Situacao::AnaliseFiscal)
            .count(),
        1
    );
    // o botao volta a funcionar e o resultado pode ser fechado
    assert!(existe(cx, &c, "conferencia-fechar"));
    clicar(cx, &c, "conferencia-fechar");
    assert!(!existe(cx, &c, "conferencia-fechar"));
}

#[gpui_kit::test]
fn parar_interrompe_o_lote_e_o_que_ja_foi_conferido_fica_gravado(cx: &mut TestAppContext) {
    let c = cenario(cx, "restrita", &[], &[], false);
    let mut chaves = Vec::new();
    for n in 1..=4 {
        chaves.push(semear(
            &c,
            n,
            "52998224725",
            &format!("Cliente {n}"),
            "10.00",
        ));
    }
    ir_para(cx, &c, Tela::Notas);
    esperar_a_lista(cx, &c, 4);
    for chave in &chaves {
        registrar_na_sefin(&c, "101103", chave);
    }
    clicar(cx, &c, "conferir-na-sefin");
    // a primeira nota termina e a tela mostra o andamento; ai a pessoa aperta Parar
    let conf = conferencia(cx, &c);
    esperar_ate(cx, "a primeira nota conferida", |cx| {
        cx.update(|cx| {
            let t = conf.read(cx);
            t.andamento.feitas >= 1 && matches!(t.fase, FaseDaConferencia::Trabalhando(_))
        })
    });
    assert!(existe(cx, &c, "conferencia-parar"));
    let texto = cx.update(|cx| conf.read(cx).andamento.texto());
    assert!(
        texto.starts_with("Conferindo ") && texto.ends_with(" de 4..."),
        "{texto}"
    );
    clicar(cx, &c, "conferencia-parar");
    let DesfechoDoLote::Feito(r) = esperar_o_lote(cx, &c) else {
        panic!("esperava o lote feito");
    };
    assert!(r.parado, "{r:?}");
    assert!(r.conferidas >= 1 && r.conferidas < 4, "{r:?}");
    assert!(r.resumo().starts_with("Parei depois de"), "{}", r.resumo());
    // o que ficou conferido esta gravado; o resto nem foi tocado
    let em_analise = situacoes(&c)
        .iter()
        .filter(|s| **s == Situacao::AnaliseFiscal)
        .count();
    assert_eq!(em_analise, r.conferidas);
    assert_eq!(r.mudaram.len(), r.conferidas);
}

#[gpui_kit::test]
fn lote_grande_diz_o_tempo_e_so_comeca_depois_da_confirmacao(cx: &mut TestAppContext) {
    let c = cenario(cx, "restrita", &[], &[], false);
    ir_para(cx, &c, Tela::Notas);
    let registros: Vec<Registro> = (1..=51)
        .map(|n| Registro {
            chave: chave_dfe::montar_nfse(3_550_308, CNPJ, n, "2611", Some(1), 2).unwrap(),
            numero: n.to_string(),
            serie: "1".into(),
            numero_dps: n.to_string(),
            emissao: "2026-11-20".into(),
            valor_servico: "10.00".into(),
            situacao: Situacao::Autorizada,
            tomador_documento: String::new(),
            tomador_nome: "Cliente".into(),
        })
        .collect();
    let conf = conferencia(cx, &c);
    cx.update_window(c.janela.into(), |_, window, cx| {
        window.render_frame(cx);
        conf.update(cx, |t, cx| t.iniciar(registros, window, cx));
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(
        fase_da_conferencia(cx, &c),
        FaseDaConferencia::Confirmando(51)
    );
    assert!(existe(cx, &c, "conferencia-confirmar-sim"));
    assert_eq!(c.emissor.chaves_consultadas().len(), 0);
    assert_eq!(c.emissor.total_de_chamadas(), 0);
    clicar(cx, &c, "conferencia-confirmar-nao");
    assert_eq!(fase_da_conferencia(cx, &c), FaseDaConferencia::Parada);
    assert_eq!(c.emissor.chaves_consultadas().len(), 0);
}

#[gpui_kit::test]
fn sem_nota_na_lista_o_lote_diz_isso_e_nao_consulta(cx: &mut TestAppContext) {
    let c = cenario(cx, "restrita", &[], &[], false);
    ir_para(cx, &c, Tela::Notas);
    esperar_a_lista(cx, &c, 0);
    clicar(cx, &c, "conferir-na-sefin");
    assert_eq!(fase_da_conferencia(cx, &c), FaseDaConferencia::SemNotas);
}

// ---------------------------------------------------------------- historico

#[gpui_kit::test]
fn o_historico_mostra_os_eventos_da_nota_em_palavras(cx: &mut TestAppContext) {
    let c = cenario(cx, "restrita", &[], &[], false);
    let chave = semear(&c, 1, "52998224725", "Maria de Teste", "100.50");
    registrar_na_sefin(&c, "101103", &chave);
    abrir_a_nota_da_lista(cx, &c, 1, &chave);
    assert!(existe(cx, &c, "consultar-historico"));
    clicar(cx, &c, "consultar-historico");
    let notas = tela_notas(cx, &c);
    esperar_ate(cx, "o historico", |cx| {
        cx.update(|cx| {
            matches!(
                notas.read(cx).detalhe.read(cx).fase_do_historico(cx),
                FaseDoHistorico::Desfecho(_)
            )
        })
    });
    let FaseDoHistorico::Desfecho(DesfechoDoHistorico::Eventos(itens)) =
        cx.update(|cx| notas.read(cx).detalhe.read(cx).fase_do_historico(cx))
    else {
        panic!("esperava os eventos");
    };
    assert_eq!(itens.len(), 1);
    assert_eq!(itens[0].nome, "Pedido de análise fiscal do cancelamento");
    assert_eq!(itens[0].motivo, JUSTIFICATIVA);
    assert!(itens[0].quando.contains('/'), "{}", itens[0].quando);
}

// ---------------------------------------------------------------- analise fiscal

fn preencher_o_dialogo_de_evento(cx: &mut TestAppContext, c: &Cenario) {
    clicar(cx, c, "motivo-cancelamento-1");
    escrever_a_justificativa(cx, c, JUSTIFICATIVA);
}

#[gpui_kit::test]
fn depois_do_cancelamento_recusado_pedir_a_analise_fiscal_marca_a_nota(cx: &mut TestAppContext) {
    let c = cenario(cx, "restrita", &[Cancelamento::Rejeitado], &[], false);
    let chave = semear(&c, 1, "52998224725", "Maria de Teste", "100.50");
    abrir_a_nota_da_lista(cx, &c, 1, &chave);
    // o cancelamento direto e' recusado (a demonstracao diz que o prazo passou)
    clicar(cx, &c, "cancelar-nota");
    preencher_o_dialogo_de_evento(cx, &c);
    clicar(cx, &c, "cancelamento-enviar");
    esperar_ate(cx, "a rejeicao", |cx| {
        matches!(
            desfecho_do_cancelamento(cx, &c),
            Some(DesfechoDoCancelamento::Rejeitado { .. })
        )
    });
    assert_eq!(situacoes(&c), vec![Situacao::Autorizada]);
    // a recusa e' a do prazo (E0822): o proprio desfecho oferece a analise
    assert!(existe(cx, &c, "cancelamento-pedir-analise"));
    clicar(cx, &c, "cancelamento-fechar");
    // a acao tambem esta no detalhe da nota autorizada do emitente
    assert!(existe(cx, &c, "pedir-analise-fiscal"));
    assert!(!existe(cx, &c, "confirmar-nota-recebida"));
    assert!(!existe(cx, &c, "recusar-nota-recebida"));
    clicar(cx, &c, "pedir-analise-fiscal");
    assert!(existe(cx, &c, "cancelamento-enviar"));
    // sem motivo nem justificativa nada sai
    clicar(cx, &c, "cancelamento-enviar");
    assert!(c.emissor.pedidos_de_evento().is_empty());
    preencher_o_dialogo_de_evento(cx, &c);
    clicar(cx, &c, "cancelamento-enviar");
    esperar_ate(cx, "o pedido registrado", |cx| {
        desfecho_do_cancelamento(cx, &c).is_some_and(|d| d.registrado())
    });
    let pedidos = c.emissor.pedidos_de_evento();
    assert_eq!(pedidos.len(), 1);
    assert_eq!(
        (pedidos[0].tipo.as_str(), pedidos[0].chave.as_str()),
        ("101103", chave.as_str())
    );
    assert_eq!(pedidos[0].motivo, "1");
    assert_eq!(pedidos[0].justificativa, JUSTIFICATIVA);
    assert_eq!(situacoes(&c), vec![Situacao::AnaliseFiscal]);
    // nada de cancelamento: so' o pedido de analise
    assert_eq!(
        c.emissor.pedidos_de_cancelamento().len(),
        1,
        "so' o recusado"
    );
    // a lista atualiza e o detalhe perde a acao (a nota ja nao esta autorizada)
    let notas = tela_notas(cx, &c);
    esperar_ate(cx, "a lista em analise fiscal", |cx| {
        cx.update(|cx| {
            notas
                .read(cx)
                .lista
                .linha(0)
                .is_some_and(|l| l.situacao == Situacao::AnaliseFiscal)
        })
    });
    clicar(cx, &c, "cancelamento-fechar");
    assert!(!existe(cx, &c, "pedir-analise-fiscal"));
    assert!(!existe(cx, &c, "cancelar-nota"));
}

#[gpui_kit::test]
fn a_recusa_por_prazo_leva_ao_pedido_de_analise_com_o_que_ja_foi_escrito(cx: &mut TestAppContext) {
    let c = cenario(cx, "restrita", &[Cancelamento::Rejeitado], &[], false);
    let chave = semear(&c, 1, "52998224725", "Maria de Teste", "100.50");
    abrir_a_nota_da_lista(cx, &c, 1, &chave);
    clicar(cx, &c, "cancelar-nota");
    preencher_o_dialogo_de_evento(cx, &c);
    clicar(cx, &c, "cancelamento-enviar");
    esperar_ate(cx, "a recusa por prazo", |cx| {
        desfecho_do_cancelamento(cx, &c).is_some_and(|d| d.recusado_por_prazo())
    });
    clicar(cx, &c, "cancelamento-pedir-analise");
    // o mesmo dialogo, agora para a analise, com o motivo e a justificativa do cancelamento recusado
    let d = dialogo(cx, &c);
    cx.update(|cx| {
        let d = d.read(cx);
        assert_eq!(d.form.tipo, TipoDeEvento::AnaliseFiscal);
        assert_eq!(d.form.motivo, Some("1"));
        assert_eq!(d.form.justificativa, JUSTIFICATIVA);
    });
    assert!(c.emissor.pedidos_de_evento().is_empty());
    clicar(cx, &c, "cancelamento-enviar");
    esperar_ate(cx, "o pedido registrado", |cx| {
        desfecho_do_cancelamento(cx, &c).is_some_and(|d| d.registrado())
    });
    let pedidos = c.emissor.pedidos_de_evento();
    assert_eq!(pedidos.len(), 1);
    assert_eq!(
        (pedidos[0].tipo.as_str(), pedidos[0].motivo.as_str()),
        ("101103", "1")
    );
    assert_eq!(pedidos[0].justificativa, JUSTIFICATIVA);
    assert_eq!(
        c.emissor.pedidos_de_cancelamento().len(),
        1,
        "so' o recusado"
    );
    assert_eq!(situacoes(&c), vec![Situacao::AnaliseFiscal]);
}

#[gpui_kit::test]
fn em_producao_a_oferta_de_analise_pede_a_marcacao_de_novo(cx: &mut TestAppContext) {
    let c = cenario(cx, "producao", &[Cancelamento::Rejeitado], &[], false);
    let chave = semear(&c, 1, "52998224725", "Maria de Teste", "100.50");
    abrir_a_nota_da_lista(cx, &c, 1, &chave);
    clicar(cx, &c, "cancelar-nota");
    preencher_o_dialogo_de_evento(cx, &c);
    clicar(cx, &c, "entendo-efeito-fiscal");
    clicar(cx, &c, "cancelamento-enviar");
    esperar_ate(cx, "a recusa por prazo", |cx| {
        desfecho_do_cancelamento(cx, &c).is_some_and(|d| d.recusado_por_prazo())
    });
    clicar(cx, &c, "cancelamento-pedir-analise");
    let chamadas = c.emissor.total_de_chamadas();
    clicar(cx, &c, "cancelamento-enviar");
    assert!(c.emissor.pedidos_de_evento().is_empty());
    assert_eq!(c.emissor.total_de_chamadas(), chamadas);
    assert_eq!(situacoes(&c), vec![Situacao::Autorizada]);
}

#[gpui_kit::test]
fn em_producao_o_pedido_de_analise_exige_a_marcacao_e_sem_ela_nada_sai(cx: &mut TestAppContext) {
    let c = cenario(cx, "producao", &[], &[], false);
    let chave = semear(&c, 1, "52998224725", "Maria de Teste", "100.50");
    abrir_a_nota_da_lista(cx, &c, 1, &chave);
    clicar(cx, &c, "pedir-analise-fiscal");
    preencher_o_dialogo_de_evento(cx, &c);
    assert!(existe(cx, &c, "entendo-efeito-fiscal"));
    let chamadas = c.emissor.total_de_chamadas();
    clicar(cx, &c, "cancelamento-enviar");
    assert!(c.emissor.pedidos_de_evento().is_empty());
    assert_eq!(c.emissor.total_de_chamadas(), chamadas);
    assert_eq!(situacoes(&c), vec![Situacao::Autorizada]);
    clicar(cx, &c, "entendo-efeito-fiscal");
    clicar(cx, &c, "cancelamento-enviar");
    esperar_ate(cx, "o pedido registrado", |cx| {
        desfecho_do_cancelamento(cx, &c).is_some_and(|d| d.registrado())
    });
    assert_eq!(c.emissor.pedidos_de_evento().len(), 1);
    assert_eq!(situacoes(&c), vec![Situacao::AnaliseFiscal]);
}

#[gpui_kit::test]
fn pedido_sem_resposta_nao_e_reenviado_as_cegas_confere_primeiro(cx: &mut TestAppContext) {
    let c = cenario(
        cx,
        "restrita",
        &[],
        &[RegistroDeEvento::SemResposta, RegistroDeEvento::Registrado],
        false,
    );
    let chave = semear(&c, 1, "52998224725", "Maria de Teste", "100.50");
    abrir_a_nota_da_lista(cx, &c, 1, &chave);
    clicar(cx, &c, "pedir-analise-fiscal");
    preencher_o_dialogo_de_evento(cx, &c);
    clicar(cx, &c, "cancelamento-enviar");
    esperar_ate(cx, "sem resposta", |cx| {
        matches!(
            desfecho_do_cancelamento(cx, &c),
            Some(DesfechoDoCancelamento::SemResposta { .. })
        )
    });
    // conferir e' o caminho; o reenvio as cegas nao e' oferecido
    assert!(existe(cx, &c, "cancelamento-conferir"));
    assert!(!existe(cx, &c, "cancelamento-tentar-de-novo"));
    clicar(cx, &c, "cancelamento-conferir");
    esperar_ate(cx, "a conferencia", |cx| {
        matches!(
            desfecho_do_cancelamento(cx, &c),
            Some(DesfechoDoCancelamento::NaoRegistrado)
        )
    });
    assert_eq!(
        c.emissor.pedidos_de_evento().len(),
        1,
        "conferir so' consulta"
    );
    assert_eq!(situacoes(&c), vec![Situacao::Autorizada]);
    // a Sefin disse que nao ha registro: agora pedir de novo e' seguro
    assert!(existe(cx, &c, "cancelamento-tentar-de-novo"));
    clicar(cx, &c, "cancelamento-tentar-de-novo");
    esperar_ate(cx, "o pedido registrado", |cx| {
        desfecho_do_cancelamento(cx, &c).is_some_and(|d| d.registrado())
    });
    assert_eq!(c.emissor.pedidos_de_evento().len(), 2);
    assert_eq!(situacoes(&c), vec![Situacao::AnaliseFiscal]);
}

#[gpui_kit::test]
fn pedido_rejeitado_mostra_a_recusa_e_nao_marca_a_pasta(cx: &mut TestAppContext) {
    let c = cenario(cx, "restrita", &[], &[RegistroDeEvento::Rejeitado], false);
    let chave = semear(&c, 1, "52998224725", "Maria de Teste", "100.50");
    abrir_a_nota_da_lista(cx, &c, 1, &chave);
    clicar(cx, &c, "pedir-analise-fiscal");
    preencher_o_dialogo_de_evento(cx, &c);
    clicar(cx, &c, "cancelamento-enviar");
    esperar_ate(cx, "a rejeicao", |cx| {
        matches!(
            desfecho_do_cancelamento(cx, &c),
            Some(DesfechoDoCancelamento::Rejeitado { .. })
        )
    });
    assert_eq!(situacoes(&c), vec![Situacao::Autorizada]);
    assert!(existe(cx, &c, "cancelamento-voltar"));
}

// ---------------------------------------------------------------- consultar pela DPS

#[gpui_kit::test]
fn consultar_pela_serie_e_numero_da_dps_acha_a_nota(cx: &mut TestAppContext) {
    let c = cenario(cx, "restrita", &[], &[], false);
    let (chave_da_nota, _) =
        autorizar(&c, &dps(7, "52998224725", "Maria de Teste", "100.50", CNPJ));
    ir_para(cx, &c, Tela::Consultar);
    let tela = consultar_tela(cx, &c);
    cx.update(|cx| tela.update(cx, |t, cx| t.escolher_modo(ModoDeConsulta::Dps, cx)));
    cx.run_until_parked();
    assert!(existe(cx, &c, "consulta-numero-dps"));
    // numero com zero a esquerda: a biblioteca nao o aceita, e nada sai
    digitar(cx, &c, "consulta-numero-dps", "07");
    clicar(cx, &c, "consultar");
    assert!(c.emissor.chaves_consultadas().is_empty());
    cx.update(|cx| {
        let t = tela.read(cx);
        assert!(!t.estado_da_dps().valido());
        assert!(t.estado_da_dps().mensagem().unwrap().contains("zero"));
    });
    // serie vazia vale a da configuracao (1)
    digitar(cx, &c, "consulta-numero-dps", "7");
    cx.update(|cx| assert!(tela.read(cx).estado_da_dps().valido()));
    clicar(cx, &c, "consultar");
    let DesfechoDaConsulta::Encontrada(nota) = esperar_a_consulta(cx, &c) else {
        panic!("esperava encontrada");
    };
    assert_eq!(nota.detalhe.chave, chave_da_nota);
    assert_eq!(c.emissor.chaves_consultadas(), vec![chave_da_nota.clone()]);
    // numero que a Sefin nao conhece
    digitar(cx, &c, "consulta-numero-dps", "99");
    clicar(cx, &c, "consultar");
    esperar_ate(cx, "nao encontrada", |cx| {
        matches!(
            cx.update(|cx| tela.read(cx).fase.clone()),
            super::consultar::Fase::Desfecho(DesfechoDaConsulta::NaoEncontrada)
        )
    });
}

// ---------------------------------------------------------------- nota recebida

fn consultar_a_chave(cx: &mut TestAppContext, c: &Cenario, chave: &str) -> DesfechoDaConsulta {
    ir_para(cx, c, Tela::Consultar);
    digitar(cx, c, "consulta-chave", &chave::em_grupos(chave));
    clicar(cx, c, "consultar");
    esperar_a_consulta(cx, c)
}

#[gpui_kit::test]
fn nota_recebida_oferece_confirmar_e_recusar_e_nao_oferece_o_que_e_do_emitente(
    cx: &mut TestAppContext,
) {
    let c = cenario(cx, "restrita", &[], &[], true);
    let chave = c.emissor.chave_da_nota_recebida().unwrap();
    let DesfechoDaConsulta::Encontrada(nota) = consultar_a_chave(cx, &c, &chave) else {
        panic!("esperava encontrada");
    };
    // de outro emitente, com o tomador da configuracao: nota recebida
    assert!(!nota.detalhe.e_do_emitente(CNPJ));
    assert!(nota.detalhe.e_tomador(CNPJ));
    assert!(existe(cx, &c, "confirmar-nota-recebida"));
    assert!(existe(cx, &c, "recusar-nota-recebida"));
    assert!(!existe(cx, &c, "pedir-analise-fiscal"));
    assert!(!existe(cx, &c, "cancelar-nota"));
    // confirmar: sem motivo nem justificativa, em ambiente de testes, envia
    clicar(cx, &c, "confirmar-nota-recebida");
    assert!(existe(cx, &c, "cancelamento-enviar"));
    let d = dialogo(cx, &c);
    cx.update(|cx| {
        let f = &d.read(cx).form;
        assert_eq!(f.tipo, TipoDeEvento::Confirmar);
        assert!(!f.tipo.tem_justificativa() && f.tipo.motivos().is_empty());
    });
    clicar(cx, &c, "cancelamento-enviar");
    esperar_ate(cx, "a confirmacao", |cx| {
        desfecho_do_cancelamento(cx, &c).is_some_and(|d| d.registrado())
    });
    let pedidos = c.emissor.pedidos_de_evento();
    assert_eq!(pedidos.len(), 1);
    assert_eq!(pedidos[0].tipo, "203202");
    assert_eq!(pedidos[0].chave, chave);
    assert_eq!(pedidos[0].motivo, "");
    // a nota nao esta na pasta e nao mudou de situacao: nada na pasta
    assert!(situacoes(&c).is_empty());
}

#[gpui_kit::test]
fn recusar_a_nota_recebida_pede_o_motivo_da_lista_da_biblioteca(cx: &mut TestAppContext) {
    let c = cenario(cx, "restrita", &[], &[], true);
    let chave = c.emissor.chave_da_nota_recebida().unwrap();
    consultar_a_chave(cx, &c, &chave);
    clicar(cx, &c, "recusar-nota-recebida");
    // sem motivo, nada sai
    clicar(cx, &c, "cancelamento-enviar");
    assert!(c.emissor.pedidos_de_evento().is_empty());
    let d = dialogo(cx, &c);
    cx.update(|cx| {
        d.update(cx, |d, cx| {
            assert_eq!(d.form.tipo, TipoDeEvento::Recusar);
            // o codigo de motivo que a lista de recusa nao tem e' ignorado
            d.form.escolher_motivo("7");
            assert_eq!(d.form.motivo, None);
            d.form.escolher_motivo("3");
            cx.notify();
        });
    });
    cx.run_until_parked();
    // motivo que nao e' "Outros": a justificativa e' opcional
    clicar(cx, &c, "cancelamento-enviar");
    esperar_ate(cx, "a recusa", |cx| {
        desfecho_do_cancelamento(cx, &c).is_some_and(|d| d.registrado())
    });
    let pedidos = c.emissor.pedidos_de_evento();
    assert_eq!(pedidos.len(), 1);
    assert_eq!(
        (pedidos[0].tipo.as_str(), pedidos[0].motivo.as_str()),
        ("203206", "3")
    );
}

#[gpui_kit::test]
fn nota_de_outro_emitente_que_nao_e_do_tomador_nao_tem_acao_de_evento(cx: &mut TestAppContext) {
    let c = cenario(cx, "restrita", &[], &[], false);
    // prestador e tomador que nao sao o emitente da configuracao
    let (chave, _) = autorizar(
        &c,
        &dps(9, "52998224725", "Maria de Teste", "50.00", "12345678909"),
    );
    let DesfechoDaConsulta::Encontrada(nota) = consultar_a_chave(cx, &c, &chave) else {
        panic!("esperava encontrada");
    };
    assert!(!nota.detalhe.e_do_emitente(CNPJ));
    assert!(!nota.detalhe.e_tomador(CNPJ));
    for id in [
        "confirmar-nota-recebida",
        "recusar-nota-recebida",
        "pedir-analise-fiscal",
        "cancelar-nota",
    ] {
        assert!(!existe(cx, &c, id), "{id} nao devia aparecer");
    }
    assert!(c.emissor.pedidos_de_evento().is_empty());
}

#[gpui_kit::test]
fn em_producao_confirmar_exige_a_marcacao_e_sem_ela_o_falso_nao_e_chamado(cx: &mut TestAppContext) {
    let c = cenario(cx, "producao", &[], &[], true);
    let chave = c.emissor.chave_da_nota_recebida().unwrap();
    consultar_a_chave(cx, &c, &chave);
    clicar(cx, &c, "confirmar-nota-recebida");
    assert!(existe(cx, &c, "entendo-efeito-fiscal"));
    let consultas = c.emissor.chaves_consultadas().len();
    clicar(cx, &c, "cancelamento-enviar");
    assert!(c.emissor.pedidos_de_evento().is_empty());
    assert_eq!(c.emissor.chaves_consultadas().len(), consultas);
    clicar(cx, &c, "entendo-efeito-fiscal");
    clicar(cx, &c, "cancelamento-enviar");
    esperar_ate(cx, "a confirmacao", |cx| {
        desfecho_do_cancelamento(cx, &c).is_some_and(|d| d.registrado())
    });
    assert_eq!(c.emissor.pedidos_de_evento().len(), 1);
}
