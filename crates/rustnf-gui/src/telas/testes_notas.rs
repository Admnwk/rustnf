// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Testes da janela das telas Notas, Consultar, Previa e do cancelamento, no ambiente de teste do GPUI, com o emissor de
//! demonstracao (nunca a Sefin), cofre em memoria, pasta temporaria e um abridor de arquivos que so' anota o pedido.
//! Nenhum toca o cofre real, a pasta de configuracao do usuario, a rede nem abre programa externo.

use std::time::{Duration, Instant};

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AppContext as _, TestAppContext};
use rustnf::nfse::Dps;
use rustnf_app::cofre::Cofre as _;
use rustnf_app::notas::{Notas, Registro, Situacao};
use serde_json::json;

use super::cancelar::Fase as FaseDoCancelamento;
use super::consultar::Fase as FaseDaConsulta;
use super::notas::Carga;
use super::testes_nota::{
    cenario, cenario_de_cancelamento, clicar, digitar, existe, preencher, Cenario, CNPJ,
};
use super::Tela;
use crate::demonstracao::{Cancelamento, Desfecho};
use crate::modelo::cancelamento::DesfechoDoCancelamento;
use crate::modelo::chave;
use crate::modelo::consulta::DesfechoDaConsulta;
use crate::modelo::previa::PreviaIbsCbs;
use crate::servicos::Destino;

pub(super) fn esperar_ate(
    cx: &mut TestAppContext,
    o_que: &str,
    pronto: impl Fn(&mut TestAppContext) -> bool,
) {
    cx.executor().allow_parking();
    let t0 = Instant::now();
    loop {
        if pronto(cx) {
            return;
        }
        assert!(
            t0.elapsed() < Duration::from_secs(30),
            "demorou demais: {o_que}"
        );
        std::thread::sleep(Duration::from_millis(20));
        cx.run_until_parked();
    }
}

pub(super) fn ir_para(cx: &mut TestAppContext, c: &Cenario, tela: Tela) {
    cx.update(|cx| c.raiz.update(cx, |r, cx| r.ir_para(tela, cx)));
    cx.run_until_parked();
}

pub(super) fn dps(numero: u64, documento: &str, nome: &str, valor: &str, prestador: &str) -> Dps {
    serde_json::from_value(json!({
        "prestador": {"documento": prestador, "nome": "OFICINA DE TESTE LTDA", "im": "123456", "op_simples": 3},
        "cmun_emissor": "3550308",
        "numero": numero,
        "serie": 1,
        "emissao": "2026-11-21T10:00:00-03:00",
        "competencia": "2026-11-21",
        "tomador": {"documento": documento, "nome": nome},
        "servico": {"ctribnac": "140101", "descricao": "Revisão geral do veículo"},
        "valores": {"servico": valor}
    }))
    .unwrap()
}

pub(super) fn rodar<T>(futuro: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(futuro)
}

/// Faz o emissor de demonstracao "autorizar" uma nota (sem guarda-la na pasta) e devolve a chave e o XML.
pub(super) fn autorizar(c: &Cenario, d: &Dps) -> (String, String) {
    let r = rodar(rustnf_app::emissao::Emissor::emitir(&*c.emissor, d));
    (r.chave, r.xml_retorno)
}

/// Poe uma nota na pasta de notas do ensaio e devolve a chave.
pub(super) fn semear(c: &Cenario, numero: u64, documento: &str, nome: &str, valor: &str) -> String {
    let (chave, xml) = autorizar(c, &dps(numero, documento, nome, valor, CNPJ));
    let nfse = rustnf::nfse::leitura::ler_nfse(&xml).unwrap();
    let notas = Notas::abrir(&c.notas).unwrap();
    notas.guardar(&Registro::da_nfse(&nfse), &xml).unwrap();
    chave
}

pub(super) fn tela_notas(
    cx: &mut TestAppContext,
    c: &Cenario,
) -> gpui_kit::Entity<super::notas::TelaNotas> {
    cx.update(|cx| c.raiz.read(cx).notas.clone())
}

pub(super) fn esperar_a_lista(cx: &mut TestAppContext, c: &Cenario, total: usize) {
    let notas = tela_notas(cx, c);
    esperar_ate(cx, "a lista", move |cx| {
        cx.update(|cx| {
            let n = notas.read(cx);
            n.carga == Carga::Pronta && n.lista.total() == total
        })
    });
}

pub(super) fn situacoes(c: &Cenario) -> Vec<Situacao> {
    Notas::abrir(&c.notas)
        .unwrap()
        .listar()
        .unwrap()
        .iter()
        .map(|r| r.situacao)
        .collect()
}

fn pedidos(c: &Cenario) -> usize {
    c.emissor.pedidos_de_cancelamento().len()
}

/// A lista esta na tela: o botao "Atualizar" e' dela. (As linhas sao `div`s com clique, que o suporte de teste da
/// janela nao acha; o clique numa linha chama `abrir`, que os testes chamam direto.)
fn lista_na_tela(cx: &mut TestAppContext, c: &Cenario) -> bool {
    existe(cx, c, "atualizar-notas")
}

/// Abre o detalhe da nota `i` e espera a leitura do XML.
pub(super) fn abrir_o_detalhe(cx: &mut TestAppContext, c: &Cenario, i: usize, chave: &str) {
    let notas = tela_notas(cx, c);
    let a_abrir = notas.clone();
    cx.update_window(c.janela.into(), |_, window, cx| {
        window.render_frame(cx);
        a_abrir.update(cx, |n, cx| n.abrir(i, window, cx));
    })
    .unwrap();
    cx.run_until_parked();
    let chave = chave.to_string();
    esperar_ate(cx, "o detalhe", move |cx| {
        cx.update(|cx| {
            notas
                .read(cx)
                .detalhe
                .read(cx)
                .chave()
                .is_some_and(|k| k == chave)
        })
    });
}

pub(super) fn escrever_a_justificativa(cx: &mut TestAppContext, c: &Cenario, texto: &str) {
    let canc = cx.update(|cx| c.raiz.read(cx).cancelamento.clone());
    cx.update_window(c.janela.into(), |_, window, cx| {
        window.render_frame(cx);
        let campo = canc.read(cx).justificativa.clone();
        campo.update(cx, |e, cx| e.focus(window, cx));
        window.input(texto, cx);
    })
    .unwrap();
    cx.run_until_parked();
}

pub(super) fn fase_do_cancelamento(cx: &mut TestAppContext, c: &Cenario) -> FaseDoCancelamento {
    cx.update(|cx| c.raiz.read(cx).cancelamento.read(cx).fase.clone())
}

pub(super) fn desfecho_do_cancelamento(
    cx: &mut TestAppContext,
    c: &Cenario,
) -> Option<DesfechoDoCancelamento> {
    match fase_do_cancelamento(cx, c) {
        FaseDoCancelamento::Desfecho(d) => Some(d),
        _ => None,
    }
}

/// Abre o dialogo de cancelar a nota 0 e preenche motivo e justificativa.
pub(super) fn preencher_o_cancelamento(cx: &mut TestAppContext, c: &Cenario) {
    clicar(cx, c, "cancelar-nota");
    assert!(existe(cx, c, "cancelamento-enviar"));
    clicar(cx, c, "motivo-cancelamento-1");
    escrever_a_justificativa(cx, c, "Nota emitida com o valor errado");
}

// ---------------------------------------------------------------- notas: lista, busca, filtro, detalhe

#[gpui_kit::test]
fn emitir_pela_nova_nota_e_ver_a_nota_na_lista(cx: &mut TestAppContext) {
    let c = cenario(cx, "restrita", &[Desfecho::Autorizada], true, 1120.);
    preencher(cx, &c);
    clicar(cx, &c, "emitir");
    clicar(cx, &c, "confirmar-emissao");
    esperar_ate(cx, "autorizada", |cx| {
        cx.update(|cx| matches!(c.nova.read(cx).fase, super::nota::Fase::Concluida(_)))
    });
    // a lista se atualiza sozinha quando a nota e' autorizada, sem a pessoa abrir a tela
    esperar_a_lista(cx, &c, 1);
    ir_para(cx, &c, Tela::Notas);
    assert!(lista_na_tela(cx, &c));
    let notas = tela_notas(cx, &c);
    cx.update(|cx| {
        let n = notas.read(cx);
        let l = n.lista.linha(0).unwrap();
        assert_eq!(l.tomador, "Maria de Teste");
        assert_eq!(l.documento, "***.***.***-25");
        assert_eq!(l.valor, "1.234,56");
        assert_eq!(l.situacao, Situacao::Autorizada);
    });
}

#[gpui_kit::test]
fn abrir_o_detalhe_e_voltar_para_a_lista_em_janela_estreita(cx: &mut TestAppContext) {
    let c = cenario(cx, "restrita", &[Desfecho::Autorizada], true, 1120.);
    let chave = semear(&c, 1, "52998224725", "Maria de Teste", "100.50");
    ir_para(cx, &c, Tela::Notas);
    esperar_a_lista(cx, &c, 1);
    abrir_o_detalhe(cx, &c, 0, &chave);
    // em janela estreita o detalhe e' uma pagina, com a chave, o DANFSe e o cancelamento
    assert!(existe(cx, &c, "voltar-a-lista"));
    assert!(existe(cx, &c, "copiar-chave-detalhe"));
    assert!(existe(cx, &c, "abrir-danfse"));
    assert!(existe(cx, &c, "mostrar-na-pasta"));
    assert!(existe(cx, &c, "usar-como-modelo"));
    assert!(existe(cx, &c, "cancelar-nota"));
    assert!(!lista_na_tela(cx, &c), "a lista saiu da frente");
    clicar(cx, &c, "voltar-a-lista");
    assert!(lista_na_tela(cx, &c));
    assert!(!existe(cx, &c, "voltar-a-lista"));
}

#[gpui_kit::test]
fn em_janela_larga_o_detalhe_fica_ao_lado_da_lista(cx: &mut TestAppContext) {
    let c = cenario(cx, "restrita", &[Desfecho::Autorizada], true, 1500.);
    let chave = semear(&c, 1, "52998224725", "Maria de Teste", "100.50");
    ir_para(cx, &c, Tela::Notas);
    esperar_a_lista(cx, &c, 1);
    abrir_o_detalhe(cx, &c, 0, &chave);
    assert!(lista_na_tela(cx, &c), "a lista continua na tela");
    assert!(existe(cx, &c, "copiar-chave-detalhe"));
    assert!(!existe(cx, &c, "voltar-a-lista"));
}

#[gpui_kit::test]
fn busca_e_filtro_na_janela(cx: &mut TestAppContext) {
    let c = cenario(cx, "restrita", &[Desfecho::Autorizada], true, 1120.);
    semear(&c, 1, "52998224725", "Maria de Teste", "100.00");
    semear(&c, 2, "11222333000181", "Oficina Exemplo Ltda", "200.00");
    let cancelada = semear(&c, 3, "12345678909", "João da Silva", "300.00");
    Notas::abrir(&c.notas)
        .unwrap()
        .marcar_cancelada(&cancelada)
        .unwrap();
    ir_para(cx, &c, Tela::Notas);
    esperar_a_lista(cx, &c, 3);
    let notas = tela_notas(cx, &c);
    digitar(cx, &c, "busca-de-notas", "joao");
    cx.update(|cx| {
        let n = notas.read(cx);
        assert_eq!(n.lista.quantas(), 1);
        assert_eq!(n.lista.linha(0).unwrap().situacao, Situacao::Cancelada);
    });
    digitar(cx, &c, "busca-de-notas", "zzz");
    assert!(
        existe(cx, &c, "notas-limpar-busca"),
        "o estado de busca sem resultado"
    );
    clicar(cx, &c, "notas-limpar-busca");
    cx.update(|cx| assert_eq!(notas.read(cx).lista.quantas(), 3));
    // filtro por situacao: "Canceladas" e' o terceiro
    clicar(cx, &c, "filtro-notas-2");
    cx.update(|cx| assert_eq!(notas.read(cx).lista.quantas(), 1));
    clicar(cx, &c, "filtro-notas-1");
    cx.update(|cx| assert_eq!(notas.read(cx).lista.quantas(), 2));
}

#[gpui_kit::test]
fn pasta_vazia_orienta_e_leva_a_nova_nota(cx: &mut TestAppContext) {
    let c = cenario(cx, "restrita", &[Desfecho::Autorizada], true, 1120.);
    ir_para(cx, &c, Tela::Notas);
    esperar_a_lista(cx, &c, 0);
    assert!(existe(cx, &c, "notas-ir-para-nova"));
    clicar(cx, &c, "notas-ir-para-nova");
    cx.update(|cx| assert_eq!(c.raiz.read(cx).tela, Tela::NovaNota));
}

#[gpui_kit::test]
fn indice_ilegivel_mostra_o_erro_e_tentar_de_novo_le_outra_vez(cx: &mut TestAppContext) {
    let c = cenario(cx, "restrita", &[Desfecho::Autorizada], true, 1120.);
    semear(&c, 1, "52998224725", "Maria de Teste", "100.00");
    let indice = c.notas.join("indice.json");
    let bom = std::fs::read(&indice).unwrap();
    std::fs::write(&indice, b"isto nao e' json").unwrap();
    ir_para(cx, &c, Tela::Notas);
    let notas = tela_notas(cx, &c);
    esperar_ate(cx, "o erro de leitura", |cx| {
        cx.update(|cx| matches!(notas.read(cx).carga, Carga::Falha(_)))
    });
    assert!(existe(cx, &c, "notas-tentar-de-novo"));
    std::fs::write(&indice, bom).unwrap();
    clicar(cx, &c, "notas-tentar-de-novo");
    esperar_a_lista(cx, &c, 1);
}

#[gpui_kit::test]
fn teclado_seta_e_enter_abrem_o_detalhe_e_esc_volta(cx: &mut TestAppContext) {
    let c = cenario(cx, "restrita", &[Desfecho::Autorizada], true, 1120.);
    let a = semear(&c, 1, "52998224725", "Maria de Teste", "100.00");
    let b = semear(&c, 2, "11222333000181", "Oficina Exemplo Ltda", "200.00");
    ir_para(cx, &c, Tela::Notas);
    esperar_a_lista(cx, &c, 2);
    // a lista vem da mais recente para a mais antiga; abrir a primeira poe o foco na lista
    let primeira_chave = {
        let notas = tela_notas(cx, &c);
        cx.update(|cx| notas.read(cx).lista.linha(0).unwrap().chave.clone())
    };
    let outra = if primeira_chave == a { b } else { a };
    abrir_o_detalhe(cx, &c, 0, &primeira_chave);
    cx.update_window(c.janela.into(), |_, window, cx| {
        window.render_frame(cx);
        window.press("escape", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert!(lista_na_tela(cx, &c), "Esc voltou para a lista");
    cx.update_window(c.janela.into(), |_, window, cx| {
        window.render_frame(cx);
        window.press("down", cx);
        window.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
    let notas = tela_notas(cx, &c);
    esperar_ate(cx, "o detalhe da segunda", move |cx| {
        cx.update(|cx| {
            notas
                .read(cx)
                .detalhe
                .read(cx)
                .chave()
                .is_some_and(|k| k == outra)
        })
    });
}

#[gpui_kit::test]
fn abrir_a_pasta_de_notas_e_copiar_o_xml_da_nota(cx: &mut TestAppContext) {
    let c = cenario(cx, "restrita", &[Desfecho::Autorizada], true, 1120.);
    let chave = semear(&c, 1, "52998224725", "Maria de Teste", "100.50");
    ir_para(cx, &c, Tela::Notas);
    esperar_a_lista(cx, &c, 1);
    // "Abrir a pasta" pede ao sistema a pasta de notas inteira
    clicar(cx, &c, "abrir-pasta-de-notas");
    assert_eq!(c.abridor.pedidos(), vec![(Destino::Abrir, c.notas.clone())]);
    // no detalhe, "Copiar o XML" poe na area de transferencia o XML guardado, inteiro
    abrir_o_detalhe(cx, &c, 0, &chave);
    let xml = Notas::abrir(&c.notas).unwrap().ler_xml(&chave).unwrap();
    assert!(existe(cx, &c, "copiar-xml"));
    clicar(cx, &c, "copiar-xml");
    esperar_ate(cx, "o XML na area de transferencia", |cx| {
        cx.run_until_parked();
        cx.read_from_clipboard().and_then(|item| item.text()) == Some(xml.clone())
    });
    // o botao diz que copiou; escolher outra nota (ou a mesma de novo) volta ao texto de antes
    let notas = tela_notas(cx, &c);
    cx.update(|cx| assert!(notas.read(cx).detalhe.read(cx).xml_copiado()));
    abrir_o_detalhe(cx, &c, 0, &chave);
    cx.update(|cx| assert!(!notas.read(cx).detalhe.read(cx).xml_copiado()));
}

/// Hoje em Brasilia, `AAAA-MM-DD` (a mesma conta do contexto da tela).
fn hoje_iso() -> String {
    (chrono::Utc::now() - chrono::Duration::hours(3))
        .format("%Y-%m-%d")
        .to_string()
}

/// Como `semear`, com a nota emitida hoje (o periodo da lista conta a partir de hoje).
fn semear_hoje(c: &Cenario, numero: u64, documento: &str, nome: &str) -> String {
    let mut d = dps(numero, documento, nome, "100.00", CNPJ);
    d.emissao = Some(format!("{}T10:00:00-03:00", hoje_iso()));
    let (chave, xml) = autorizar(c, &d);
    let nfse = rustnf::nfse::leitura::ler_nfse(&xml).unwrap();
    Notas::abrir(&c.notas)
        .unwrap()
        .guardar(&Registro::da_nfse(&nfse), &xml)
        .unwrap();
    chave
}

#[gpui_kit::test]
fn periodo_da_lista_e_exportar_o_que_esta_nela(cx: &mut TestAppContext) {
    let c = cenario(cx, "restrita", &[Desfecho::Autorizada], true, 1120.);
    let maria = semear_hoje(&c, 1, "52998224725", "Maria de Teste");
    let oficina = semear_hoje(&c, 2, "11222333000181", "Oficina Exemplo Ltda");
    ir_para(cx, &c, Tela::Notas);
    esperar_a_lista(cx, &c, 2);
    let notas = tela_notas(cx, &c);
    // "Mes passado" (o terceiro): nenhuma, e nada a exportar; "Este mes" (o segundo): as duas
    clicar(cx, &c, "periodo-notas-2");
    cx.update(|cx| assert_eq!(notas.read(cx).lista.quantas(), 0));
    // o estado "nenhuma nota encontrada" (o suporte de teste acha o botao dele, nao o painel)
    assert!(existe(cx, &c, "notas-limpar-busca"));
    clicar(cx, &c, "periodo-notas-1");
    cx.update(|cx| assert_eq!(notas.read(cx).lista.quantas(), 2));
    // a busca vale junto: so' a Maria vai para a exportacao
    digitar(cx, &c, "busca-de-notas", "maria");
    cx.update(|cx| assert_eq!(notas.read(cx).lista.quantas(), 1));

    let destino = c._pasta.caminho().join("para o contador");
    clicar(cx, &c, "exportar-notas");
    assert!(cx.did_prompt_for_paths(), "pergunta a pasta de destino");
    let escolhida = destino.clone();
    cx.simulate_path_prompt_response(move |_| Some(vec![escolhida]));
    esperar_ate(cx, "a exportacao", |cx| {
        cx.run_until_parked();
        cx.update(|cx| notas.read(cx).exportacao.is_some())
    });
    cx.update(|cx| {
        let (grav, texto) = notas.read(cx).exportacao.clone().unwrap();
        assert_eq!(grav, crate::modelo::certificado::Gravidade::Ok, "{texto}");
        assert!(
            texto.contains("1 nota") && texto.contains("para o contador"),
            "{texto}"
        );
    });
    assert!(destino.join(format!("{maria}.xml")).is_file());
    assert!(!destino.join(format!("{oficina}.xml")).exists());
    let lista = std::fs::read_to_string(destino.join("notas.csv")).unwrap();
    assert_eq!(lista.lines().count(), 2, "titulos e a Maria: {lista}");
    assert!(
        lista.contains("Maria de Teste") && lista.contains(&maria),
        "{lista}"
    );
    // depois de gravar, a pasta exportada e' aberta; a pasta de notas fica como estava
    assert!(c
        .abridor
        .pedidos()
        .contains(&(Destino::Abrir, destino.clone())));
    assert_eq!(Notas::abrir(&c.notas).unwrap().listar().unwrap().len(), 2);

    // lista vazia: o botao nao pergunta nada
    clicar(cx, &c, "periodo-notas-2");
    clicar(cx, &c, "exportar-notas");
    assert!(!cx.did_prompt_for_paths());
}

// ---------------------------------------------------------------- DANFSe

#[gpui_kit::test]
fn o_danfse_e_gravado_ao_lado_do_xml_aberto_e_a_lista_nao_muda(cx: &mut TestAppContext) {
    let c = cenario(cx, "restrita", &[Desfecho::Autorizada], true, 1120.);
    let chave = semear(&c, 1, "52998224725", "Maria de Teste", "100.50");
    ir_para(cx, &c, Tela::Notas);
    esperar_a_lista(cx, &c, 1);
    abrir_o_detalhe(cx, &c, 0, &chave);
    let xml = Notas::abrir(&c.notas)
        .unwrap()
        .caminho_do_xml_guardado(&chave)
        .unwrap();
    let pdf = xml.with_extension("pdf");
    assert!(!pdf.exists());
    clicar(cx, &c, "abrir-danfse");
    esperar_ate(cx, "o PDF", |cx| {
        cx.run_until_parked();
        pdf.exists() && !c.abridor.pedidos().is_empty()
    });
    let bytes = std::fs::read(&pdf).unwrap();
    assert!(bytes.starts_with(b"%PDF"), "o arquivo e' um PDF");
    assert_eq!(c.abridor.pedidos(), vec![(Destino::Abrir, pdf.clone())]);
    // a lista, o indice e a numeracao nao mudam com o PDF na pasta
    let notas = Notas::abrir(&c.notas).unwrap();
    assert_eq!(notas.listar().unwrap().len(), 1);
    assert_eq!(notas.proximo_numero("1").unwrap(), 2);
    // "Mostrar na pasta" pede ao sistema o XML guardado
    clicar(cx, &c, "mostrar-na-pasta");
    esperar_ate(cx, "mostrar na pasta", |cx| {
        cx.run_until_parked();
        c.abridor.pedidos().len() == 2
    });
    assert_eq!(c.abridor.pedidos()[1], (Destino::Mostrar, xml));
}

#[gpui_kit::test]
fn o_danfse_de_nota_cancelada_leva_a_marca_e_a_pasta_nao_muda_de_numeracao(
    cx: &mut TestAppContext,
) {
    let c = cenario(cx, "restrita", &[Desfecho::Autorizada], true, 1120.);
    let chave = semear(&c, 1, "52998224725", "Maria de Teste", "100.50");
    Notas::abrir(&c.notas)
        .unwrap()
        .marcar_cancelada(&chave)
        .unwrap();
    ir_para(cx, &c, Tela::Notas);
    esperar_a_lista(cx, &c, 1);
    abrir_o_detalhe(cx, &c, 0, &chave);
    // nota cancelada nao oferece cancelar de novo
    assert!(!existe(cx, &c, "cancelar-nota"));
    assert!(existe(cx, &c, "abrir-danfse"));
}

// ---------------------------------------------------------------- cancelar

#[gpui_kit::test]
fn cancelar_registrado_marca_a_pasta_e_a_lista_atualiza(cx: &mut TestAppContext) {
    let c = cenario(cx, "restrita", &[Desfecho::Autorizada], true, 1120.);
    let chave = semear(&c, 1, "52998224725", "Maria de Teste", "100.50");
    ir_para(cx, &c, Tela::Notas);
    esperar_a_lista(cx, &c, 1);
    abrir_o_detalhe(cx, &c, 0, &chave);
    clicar(cx, &c, "cancelar-nota");
    // sem motivo nem justificativa o botao esta desligado e nada sai
    assert!(existe(cx, &c, "cancelamento-enviar"));
    clicar(cx, &c, "cancelamento-enviar");
    assert_eq!(pedidos(&c), 0);
    clicar(cx, &c, "motivo-cancelamento-1");
    escrever_a_justificativa(cx, &c, "curta");
    clicar(cx, &c, "cancelamento-enviar");
    assert_eq!(
        pedidos(&c),
        0,
        "justificativa abaixo do minimo da biblioteca"
    );
    escrever_a_justificativa(cx, &c, " demais para valer o cancelamento");
    // clique duplo: o segundo pedido chega com o primeiro ainda em curso e nao envia de novo (depois do primeiro
    // clique o botao sai do dialogo, entao o segundo clique e' o mesmo comando chamado de novo)
    let canc = cx.update(|cx| c.raiz.read(cx).cancelamento.clone());
    cx.update_window(c.janela.into(), |_, window, cx| {
        window.render_frame(cx);
        canc.update(cx, |c, cx| {
            c.enviar(window, cx);
            c.enviar(window, cx);
        });
    })
    .unwrap();
    cx.run_until_parked();
    esperar_ate(cx, "cancelamento registrado", |cx| {
        desfecho_do_cancelamento(cx, &c).is_some_and(|d| d.registrado())
    });
    assert_eq!(situacoes(&c), vec![Situacao::Cancelada]);
    let pedidos_feitos = c.emissor.pedidos_de_cancelamento();
    assert_eq!(pedidos_feitos.len(), 1);
    assert_eq!(pedidos_feitos[0].chave, chave);
    assert_eq!(pedidos_feitos[0].motivo, "1");
    assert_eq!(
        pedidos_feitos[0].justificativa,
        "curta demais para valer o cancelamento"
    );
    // a lista atualizou sozinha: a nota aparece como cancelada
    let notas = tela_notas(cx, &c);
    esperar_ate(cx, "a lista cancelada", |cx| {
        cx.update(|cx| {
            notas
                .read(cx)
                .lista
                .linha(0)
                .is_some_and(|l| l.situacao == Situacao::Cancelada)
        })
    });
    // o botao de enviar saiu do dialogo e houve um unico pedido
    assert!(!existe(cx, &c, "cancelamento-enviar"));
    assert_eq!(pedidos(&c), 1);
}

#[gpui_kit::test]
fn em_producao_cancelar_exige_a_marcacao_e_sem_ela_nada_e_enviado(cx: &mut TestAppContext) {
    let c = cenario(cx, "producao", &[Desfecho::Autorizada], true, 1120.);
    let chave = semear(&c, 1, "52998224725", "Maria de Teste", "100.50");
    ir_para(cx, &c, Tela::Notas);
    esperar_a_lista(cx, &c, 1);
    abrir_o_detalhe(cx, &c, 0, &chave);
    preencher_o_cancelamento(cx, &c);
    assert!(existe(cx, &c, "entendo-efeito-fiscal"));
    // tudo preenchido, mas sem a marcacao o botao nao envia
    clicar(cx, &c, "cancelamento-enviar");
    assert_eq!(pedidos(&c), 0);
    assert_eq!(
        c.emissor.total_de_chamadas(),
        1,
        "so' a semeadura chamou o falso"
    );
    assert_eq!(situacoes(&c), vec![Situacao::Autorizada]);
    clicar(cx, &c, "entendo-efeito-fiscal");
    clicar(cx, &c, "cancelamento-enviar");
    esperar_ate(cx, "cancelamento registrado", |cx| {
        desfecho_do_cancelamento(cx, &c).is_some_and(|d| d.registrado())
    });
    assert_eq!(pedidos(&c), 1);
    assert_eq!(situacoes(&c), vec![Situacao::Cancelada]);
}

#[gpui_kit::test]
fn cancelamento_rejeitado_mostra_a_recusa_e_nao_marca_a_pasta(cx: &mut TestAppContext) {
    let c = cenario_de_cancelamento(
        cx,
        "restrita",
        &[Desfecho::Autorizada],
        &[Cancelamento::Rejeitado],
        true,
        1120.,
        5200.,
    );
    let chave = semear(&c, 1, "52998224725", "Maria de Teste", "100.50");
    ir_para(cx, &c, Tela::Notas);
    esperar_a_lista(cx, &c, 1);
    abrir_o_detalhe(cx, &c, 0, &chave);
    preencher_o_cancelamento(cx, &c);
    clicar(cx, &c, "cancelamento-enviar");
    esperar_ate(cx, "rejeicao", |cx| {
        matches!(
            desfecho_do_cancelamento(cx, &c),
            Some(DesfechoDoCancelamento::Rejeitado { .. })
        )
    });
    assert!(existe(cx, &c, "cancelamento-voltar"));
    assert_eq!(situacoes(&c), vec![Situacao::Autorizada]);
    // voltar ao formulario mantem o que foi digitado
    clicar(cx, &c, "cancelamento-voltar");
    assert!(existe(cx, &c, "cancelamento-enviar"));
    let canc = cx.update(|cx| c.raiz.read(cx).cancelamento.clone());
    cx.update(|cx| {
        let f = &canc.read(cx).form;
        assert_eq!(f.motivo, Some("1"));
        assert_eq!(f.justificativa, "Nota emitida com o valor errado");
    });
}

#[gpui_kit::test]
fn sem_resposta_o_reenvio_e_o_mesmo_pedido_e_conferir_diz_que_nao_foi_registrado(
    cx: &mut TestAppContext,
) {
    let c = cenario_de_cancelamento(
        cx,
        "restrita",
        &[Desfecho::Autorizada],
        &[Cancelamento::SemResposta, Cancelamento::Registrado],
        true,
        1120.,
        5200.,
    );
    let chave = semear(&c, 1, "52998224725", "Maria de Teste", "100.50");
    ir_para(cx, &c, Tela::Notas);
    esperar_a_lista(cx, &c, 1);
    abrir_o_detalhe(cx, &c, 0, &chave);
    preencher_o_cancelamento(cx, &c);
    clicar(cx, &c, "cancelamento-enviar");
    esperar_ate(cx, "sem resposta", |cx| {
        desfecho_do_cancelamento(cx, &c).is_some_and(|d| d.deixa_em_aberto())
    });
    assert!(existe(cx, &c, "cancelamento-tentar-de-novo"));
    assert!(existe(cx, &c, "cancelamento-conferir"));
    assert_eq!(
        situacoes(&c),
        vec![Situacao::Autorizada],
        "sem resposta nao marca"
    );
    // conferir na Sefin: nao ha cancelamento registrado
    clicar(cx, &c, "cancelamento-conferir");
    esperar_ate(cx, "conferencia", |cx| {
        desfecho_do_cancelamento(cx, &c) == Some(DesfechoDoCancelamento::NaoRegistrado)
    });
    assert_eq!(situacoes(&c), vec![Situacao::Autorizada]);
    // tentar de novo reenvia o MESMO pedido e agora registra
    clicar(cx, &c, "cancelamento-tentar-de-novo");
    esperar_ate(cx, "registrado", |cx| {
        desfecho_do_cancelamento(cx, &c).is_some_and(|d| d.registrado())
    });
    let pedidos_feitos = c.emissor.pedidos_de_cancelamento();
    assert_eq!(pedidos_feitos.len(), 2);
    assert_eq!(pedidos_feitos[0], pedidos_feitos[1]);
    assert_eq!(situacoes(&c), vec![Situacao::Cancelada]);
}

#[gpui_kit::test]
fn sem_senha_guardada_o_cancelamento_pede_a_senha_e_volta_ao_dialogo(cx: &mut TestAppContext) {
    let c = cenario(cx, "restrita", &[Desfecho::Autorizada], true, 1120.);
    c.cofre.apagar(CNPJ).unwrap();
    let chave = semear(&c, 1, "52998224725", "Maria de Teste", "100.50");
    ir_para(cx, &c, Tela::Notas);
    esperar_a_lista(cx, &c, 1);
    abrir_o_detalhe(cx, &c, 0, &chave);
    preencher_o_cancelamento(cx, &c);
    clicar(cx, &c, "cancelamento-enviar");
    esperar_ate(cx, "pedido de senha", |cx| {
        cx.run_until_parked();
        existe(cx, &c, "senha-da-emissao")
    });
    assert_eq!(pedidos(&c), 0);
    digitar(cx, &c, "senha-da-emissao", super::testes_nota::SENHA);
    clicar(cx, &c, "usar-senha");
    esperar_ate(cx, "registrado", |cx| {
        desfecho_do_cancelamento(cx, &c).is_some_and(|d| d.registrado())
    });
    assert!(
        c.cofre.ler(CNPJ).unwrap().is_none(),
        "a senha digitada nao e' guardada"
    );
    assert_eq!(situacoes(&c), vec![Situacao::Cancelada]);
}

// ---------------------------------------------------------------- usar como modelo

#[gpui_kit::test]
fn usar_como_modelo_leva_a_nota_para_a_nova_nota_com_a_competencia_de_hoje(
    cx: &mut TestAppContext,
) {
    let c = cenario(cx, "restrita", &[Desfecho::Autorizada], true, 1120.);
    let chave = semear(&c, 1, "52998224725", "Maria de Teste", "100.50");
    ir_para(cx, &c, Tela::Notas);
    esperar_a_lista(cx, &c, 1);
    abrir_o_detalhe(cx, &c, 0, &chave);
    clicar(cx, &c, "usar-como-modelo");
    cx.update(|cx| assert_eq!(c.raiz.read(cx).tela, Tela::NovaNota));
    cx.update(|cx| {
        let n = c.nova.read(cx);
        assert_eq!(n.dados.tomador_nome, "Maria de Teste");
        assert_eq!(n.dados.tomador_documento, "529.982.247-25");
        assert_eq!(n.dados.valor_servico, "100,50");
        assert_eq!(n.dados.ctribnac, "14.01.01");
        assert_eq!(n.dados.descricao, "Revisão geral do veículo");
        assert_eq!(n.dados.competencia, n.ctx.hoje());
    });
}

#[gpui_kit::test]
fn com_algo_digitado_pergunta_antes_de_substituir(cx: &mut TestAppContext) {
    let c = cenario(cx, "restrita", &[Desfecho::Autorizada], true, 1120.);
    let chave = semear(&c, 1, "52998224725", "Maria de Teste", "100.50");
    digitar(cx, &c, "nota-tomador-nome", "Digitado antes");
    ir_para(cx, &c, Tela::Notas);
    esperar_a_lista(cx, &c, 1);
    abrir_o_detalhe(cx, &c, 0, &chave);
    clicar(cx, &c, "usar-como-modelo");
    // a pergunta aparece e nada foi trocado
    assert!(existe(cx, &c, "ok"), "o dialogo de substituir");
    cx.update(|cx| {
        assert_eq!(c.raiz.read(cx).tela, Tela::Notas);
        assert_eq!(c.nova.read(cx).dados.tomador_nome, "Digitado antes");
    });
    clicar(cx, &c, "cancel");
    cx.update(|cx| assert_eq!(c.nova.read(cx).dados.tomador_nome, "Digitado antes"));
    // confirmando, o modelo substitui
    clicar(cx, &c, "usar-como-modelo");
    clicar(cx, &c, "ok");
    cx.update(|cx| {
        assert_eq!(c.raiz.read(cx).tela, Tela::NovaNota);
        assert_eq!(c.nova.read(cx).dados.tomador_nome, "Maria de Teste");
    });
}

#[gpui_kit::test]
fn com_uma_tentativa_pendente_o_modelo_nao_substitui(cx: &mut TestAppContext) {
    // a primeira chamada ao falso (a semeadura) autoriza; a emissao pela tela fica sem resposta
    let c = cenario(
        cx,
        "restrita",
        &[Desfecho::Autorizada, Desfecho::SemResposta],
        true,
        1120.,
    );
    let chave = semear(&c, 1, "52998224725", "Maria de Teste", "100.50");
    preencher(cx, &c);
    clicar(cx, &c, "emitir");
    clicar(cx, &c, "confirmar-emissao");
    esperar_ate(cx, "sem resposta", |cx| {
        cx.update(|cx| matches!(c.nova.read(cx).fase, super::nota::Fase::Pendente { .. }))
    });
    ir_para(cx, &c, Tela::Notas);
    esperar_a_lista(cx, &c, 1);
    abrir_o_detalhe(cx, &c, 0, &chave);
    clicar(cx, &c, "usar-como-modelo");
    cx.update(|cx| {
        assert_eq!(c.raiz.read(cx).tela, Tela::Notas, "nao saiu da tela");
        assert_eq!(c.nova.read(cx).dados.tomador_nome, "Maria de Teste");
        assert!(matches!(
            c.nova.read(cx).fase,
            super::nota::Fase::Pendente { .. }
        ));
    });
}

// ---------------------------------------------------------------- consultar

pub(super) fn consulta_fase(cx: &mut TestAppContext, c: &Cenario) -> FaseDaConsulta {
    cx.update(|cx| c.raiz.read(cx).consultar.read(cx).fase.clone())
}

pub(super) fn esperar_a_consulta(cx: &mut TestAppContext, c: &Cenario) -> DesfechoDaConsulta {
    esperar_ate(cx, "a consulta", |cx| {
        matches!(consulta_fase(cx, c), FaseDaConsulta::Desfecho(_))
    });
    match consulta_fase(cx, c) {
        FaseDaConsulta::Desfecho(d) => d,
        _ => unreachable!(),
    }
}

#[gpui_kit::test]
fn consultar_pela_chave_acha_a_nota_e_guarda_na_pasta(cx: &mut TestAppContext) {
    let c = cenario(cx, "restrita", &[Desfecho::Autorizada], true, 1120.);
    let (chave_da_nota, _) =
        autorizar(&c, &dps(7, "52998224725", "Maria de Teste", "100.50", CNPJ));
    ir_para(cx, &c, Tela::Consultar);
    // chave que nao confere: o botao desliga e nada sai
    let mut errada = chave_da_nota.clone();
    errada.replace_range(
        30..31,
        if &chave_da_nota[30..31] == "1" {
            "2"
        } else {
            "1"
        },
    );
    digitar(cx, &c, "consulta-chave", &errada);
    let consultar = cx.update(|cx| c.raiz.read(cx).consultar.clone());
    cx.update(|cx| {
        let t = consultar.read(cx);
        assert!(!t.estado_da_chave().valida());
        assert!(t
            .estado_da_chave()
            .mensagem()
            .unwrap()
            .contains("dígito verificador"));
    });
    clicar(cx, &c, "consultar");
    assert!(c.emissor.chaves_consultadas().is_empty());
    // a chave certa, colada com espacos, e' mostrada em grupos e consulta
    digitar(
        cx,
        &c,
        "consulta-chave",
        &format!("  {}  ", chave::em_grupos(&chave_da_nota)),
    );
    cx.update(|cx| {
        let t = consultar.read(cx);
        assert!(t.estado_da_chave().valida());
        assert_eq!(t.texto, chave::em_grupos(&chave_da_nota));
    });
    clicar(cx, &c, "consultar");
    let desfecho = esperar_a_consulta(cx, &c);
    let DesfechoDaConsulta::Encontrada(nota) = desfecho else {
        panic!("esperava encontrada");
    };
    assert_eq!(nota.detalhe.chave, chave_da_nota);
    assert!(chave::sao_a_mesma(&nota.detalhe.chave, &chave_da_nota));
    assert_eq!(c.emissor.chaves_consultadas(), vec![chave_da_nota.clone()]);
    // e' nota do emitente e nao esta na pasta: oferece guardar
    esperar_ate(cx, "o botao de guardar", |cx| {
        cx.run_until_parked();
        existe(cx, &c, "guardar-na-pasta")
    });
    assert!(existe(cx, &c, "abrir-danfse"));
    assert!(existe(cx, &c, "cancelar-nota"));
    assert!(Notas::abrir(&c.notas).unwrap().listar().unwrap().is_empty());
    clicar(cx, &c, "guardar-na-pasta");
    esperar_ate(cx, "a nota guardada", |cx| {
        cx.run_until_parked();
        !Notas::abrir(&c.notas).unwrap().listar().unwrap().is_empty()
    });
    let notas = Notas::abrir(&c.notas).unwrap();
    assert!(notas.ler_xml(&chave_da_nota).is_ok());
    assert_eq!(
        notas.proximo_numero("1").unwrap(),
        8,
        "o numero da DPS da nota guardada conta"
    );
    esperar_ate(cx, "o botao sumir", |cx| {
        cx.run_until_parked();
        !existe(cx, &c, "guardar-na-pasta")
    });
}

#[gpui_kit::test]
fn nota_de_outro_emitente_nao_e_guardada(cx: &mut TestAppContext) {
    let c = cenario(cx, "restrita", &[Desfecho::Autorizada], true, 1120.);
    // o prestador desta nota nao e' o emitente da configuracao
    let (chave_da_nota, _) = autorizar(
        &c,
        &dps(9, "52998224725", "Maria de Teste", "50.00", "12345678909"),
    );
    ir_para(cx, &c, Tela::Consultar);
    digitar(cx, &c, "consulta-chave", &chave_da_nota);
    clicar(cx, &c, "consultar");
    let DesfechoDaConsulta::Encontrada(_) = esperar_a_consulta(cx, &c) else {
        panic!("esperava encontrada");
    };
    assert!(existe(cx, &c, "abrir-danfse"));
    assert!(!existe(cx, &c, "guardar-na-pasta"));
    assert!(
        !existe(cx, &c, "cancelar-nota"),
        "so' nota do emitente se cancela"
    );
    assert!(Notas::abrir(&c.notas).unwrap().listar().unwrap().is_empty());
}

#[gpui_kit::test]
fn consultar_nota_da_pasta_diz_que_ja_esta_la_e_a_cancelada_vem_cancelada(cx: &mut TestAppContext) {
    let c = cenario(cx, "restrita", &[Desfecho::Autorizada], true, 1120.);
    let chave_da_nota = semear(&c, 1, "52998224725", "Maria de Teste", "100.50");
    Notas::abrir(&c.notas)
        .unwrap()
        .marcar_cancelada(&chave_da_nota)
        .unwrap();
    ir_para(cx, &c, Tela::Consultar);
    digitar(cx, &c, "consulta-chave", &chave_da_nota);
    clicar(cx, &c, "consultar");
    let DesfechoDaConsulta::Encontrada(nota) = esperar_a_consulta(cx, &c) else {
        panic!("esperava encontrada");
    };
    assert_eq!(nota.detalhe.situacao, Situacao::Cancelada);
    esperar_ate(cx, "o detalhe", |cx| {
        cx.run_until_parked();
        existe(cx, &c, "abrir-danfse")
    });
    assert!(!existe(cx, &c, "guardar-na-pasta"), "ja esta na pasta");
    assert!(!existe(cx, &c, "cancelar-nota"), "ja esta cancelada");
}

#[gpui_kit::test]
fn consulta_de_chave_que_a_sefin_nao_tem_diz_nao_encontrada(cx: &mut TestAppContext) {
    let c = cenario(cx, "restrita", &[Desfecho::Autorizada], true, 1120.);
    let desconhecida =
        rustnf::chave_dfe::montar_nfse(3_550_308, "11222333000181", 123, "2611", Some(5), 2)
            .unwrap();
    ir_para(cx, &c, Tela::Consultar);
    digitar(cx, &c, "consulta-chave", &desconhecida);
    // Enter no campo consulta
    cx.update_window(c.janela.into(), |_, window, cx| {
        window.render_frame(cx);
        window.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(
        esperar_a_consulta(cx, &c),
        DesfechoDaConsulta::NaoEncontrada
    );
    assert!(!existe(cx, &c, "guardar-na-pasta"));
    // mudar a chave descarta o resultado antigo
    digitar(cx, &c, "consulta-chave", &desconhecida[..40]);
    assert_eq!(consulta_fase(cx, &c), FaseDaConsulta::Parada);
}

#[gpui_kit::test]
fn sem_senha_guardada_a_consulta_pede_a_senha(cx: &mut TestAppContext) {
    let c = cenario(cx, "restrita", &[Desfecho::Autorizada], true, 1120.);
    c.cofre.apagar(CNPJ).unwrap();
    let (chave_da_nota, _) =
        autorizar(&c, &dps(7, "52998224725", "Maria de Teste", "100.50", CNPJ));
    ir_para(cx, &c, Tela::Consultar);
    digitar(cx, &c, "consulta-chave", &chave_da_nota);
    clicar(cx, &c, "consultar");
    esperar_ate(cx, "pedido de senha", |cx| {
        cx.run_until_parked();
        existe(cx, &c, "senha-da-emissao")
    });
    assert!(c.emissor.chaves_consultadas().is_empty());
    digitar(cx, &c, "senha-da-emissao", super::testes_nota::SENHA);
    clicar(cx, &c, "usar-senha");
    let DesfechoDaConsulta::Encontrada(_) = esperar_a_consulta(cx, &c) else {
        panic!("esperava encontrada");
    };
    assert!(c.cofre.ler(CNPJ).unwrap().is_none());
}

// ---------------------------------------------------------------- previa

#[gpui_kit::test]
fn a_previa_calcula_ao_digitar_e_funciona_sem_configuracao(cx: &mut TestAppContext) {
    // configuracao incompleta: sem certificado nem pasta, a calculadora funciona
    let c = cenario(cx, "restrita", &[Desfecho::Autorizada], false, 1120.);
    ir_para(cx, &c, Tela::Previa);
    let previa = cx.update(|cx| c.raiz.read(cx).previa.clone());
    cx.update(|cx| {
        assert!(matches!(
            previa.read(cx).previa,
            PreviaIbsCbs::Incompleta(_)
        ))
    });
    digitar(cx, &c, "previa-valor", "1000");
    digitar(cx, &c, "previa-class-trib", "000001");
    cx.update(|cx| {
        let PreviaIbsCbs::Calculada(v) = &previa.read(cx).previa else {
            panic!("esperava calculada: {:?}", previa.read(cx).previa);
        };
        assert_eq!(v.base, "1.000,00");
        assert_eq!(v.cbs.valor, "9,00");
        assert_eq!(v.total, "10,00");
    });
    // o desconto reduz a base
    digitar(cx, &c, "previa-desconto", "100");
    cx.update(|cx| {
        let PreviaIbsCbs::Calculada(v) = &previa.read(cx).previa else {
            panic!("calculada");
        };
        assert_eq!(v.base, "900,00");
    });
    // sem aliquota para o ano: diz isso e leva a aba Aliquotas
    digitar(cx, &c, "previa-data", "21/03/2029");
    cx.update(|cx| {
        assert_eq!(
            previa.read(cx).previa,
            PreviaIbsCbs::SemAliquota { ano: 2029 }
        )
    });
    assert!(existe(cx, &c, "previa-abrir-aliquotas"));
    clicar(cx, &c, "previa-abrir-aliquotas");
    cx.update(|cx| {
        let r = c.raiz.read(cx);
        assert_eq!(r.tela, Tela::Configuracoes);
        assert_eq!(r.secao, super::Secao::Aliquotas);
    });
}

#[gpui_kit::test]
fn a_previa_busca_a_classificacao_e_mostra_a_recusa_da_biblioteca(cx: &mut TestAppContext) {
    let c = cenario(cx, "restrita", &[Desfecho::Autorizada], true, 1120.);
    ir_para(cx, &c, Tela::Previa);
    let previa = cx.update(|cx| c.raiz.read(cx).previa.clone());
    digitar(cx, &c, "previa-busca-classe", "integralmente");
    cx.update(|cx| assert!(!previa.read(cx).resultados.is_empty()));
    // a linha e um `div` com clique, que o suporte de teste nao acha: o clique chama `escolher_classificacao`
    let codigo = cx.update(|cx| previa.read(cx).resultados[0].codigo.clone());
    cx.update_window(c.janela.into(), |_, window, cx| {
        window.render_frame(cx);
        previa.update(cx, |p, cx| p.escolher_classificacao(&codigo, window, cx));
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let p = previa.read(cx);
        assert_eq!(p.dados.cclass_trib.len(), 6);
        assert!(p.resultados.is_empty());
    });
    // desconto maior que o servico: a base ficaria negativa e a biblioteca recusa
    digitar(cx, &c, "previa-valor", "100");
    digitar(cx, &c, "previa-desconto", "500");
    cx.update(|cx| assert!(matches!(previa.read(cx).previa, PreviaIbsCbs::Recusada(_))));
}

// ---------------------------------------------------------------- configuracao incompleta e atalhos

#[gpui_kit::test]
fn notas_e_consultar_com_a_configuracao_incompleta_explicam_o_que_falta(cx: &mut TestAppContext) {
    let c = cenario(cx, "restrita", &[Desfecho::Autorizada], false, 1120.);
    for tela in [Tela::Notas, Tela::Consultar] {
        ir_para(cx, &c, tela);
        assert!(existe(cx, &c, "ir-para-o-inicio"), "{tela:?}");
        assert!(existe(cx, &c, "ir-para-as-configuracoes"));
        assert!(!existe(cx, &c, "atualizar-notas"));
        assert!(!existe(cx, &c, "consultar"));
    }
}

#[gpui_kit::test]
fn os_atalhos_levam_as_telas(cx: &mut TestAppContext) {
    let c = cenario(cx, "restrita", &[Desfecho::Autorizada], true, 1120.);
    for (tecla, tela) in [
        ("ctrl-l", Tela::Notas),
        ("ctrl-k", Tela::Consultar),
        ("ctrl-p", Tela::Previa),
        ("ctrl-n", Tela::NovaNota),
    ] {
        // o atalho vale com o foco dentro da janela (aqui, num campo das Configuracoes)
        ir_para(cx, &c, Tela::Configuracoes);
        clicar(cx, &c, "nome");
        cx.update_window(c.janela.into(), |_, window, cx| {
            window.render_frame(cx);
            window.press(tecla, cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update(|cx| assert_eq!(c.raiz.read(cx).tela, tela, "{tecla}"));
    }
}
