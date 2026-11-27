// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Testes da janela das NF-e recebidas, no ambiente de teste do GPUI, com a SEFAZ de mentira (nunca a rede), cofre em memoria,
//! pasta temporaria e um abridor de arquivos que so' anota o pedido: o seletor, a lista, buscar notas novas (com a espera do
//! Ambiente Nacional), o detalhe do resumo e da nota completa, baixar com a ciencia, o DANFE e as quatro respostas a nota; e,
//! em producao, nenhuma chamada sem a marcacao. Nenhum toca o cofre real, a pasta do usuario, a rede nem abre programa
//! externo.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AppContext as _, Entity, TestAppContext, WindowHandle};
use rustnf_app::cofre::{Cofre, CofreEmMemoria};
use rustnf_app::config::Config;
use rustnf_app::notas::nfe::{NotasNfe, SituacaoRecebida};
use rustnf_app::segredo::Segredo;

use super::notas::Carga;
use super::recebidas::TelaRecebidas;
use super::recebidas_dialogo::{DialogoRecebidas, Fase, Resultado};
use super::testes_nota::{abrir_janela, SemConexao, CNPJ, SENHA};
use super::testes_notas::esperar_ate;
use super::{Raiz, Secao, Tela};
use crate::demonstracao_nfe::{FabricaNfeDemonstracao, RoteiroNfe, TransporteNfeDemonstracao};
use crate::demonstracao_recebidas::{semear_recebidas, RecebidasDoEnsaio, RoteiroRecebidas, DIST};
use crate::modelo::nfe_detalhe::LeituraNfe;
use crate::modelo::nfe_eventos::DesfechoNfe;
use crate::modelo::nfe_lista::EscolhaDeNotas;
use crate::modelo::nfe_recebidas::{RespostaDada, SituacaoDaNota};
use crate::modelo::nfe_recebidas_manifestar::Resposta;
use crate::modelo::nfe_recebidas_resultados::{DesfechoBaixa, DesfechoBusca};
use crate::servicos::{Destino, Servicos};
use crate::testes_comuns::{AbridorDeTeste, PastaTemporaria};

const EVT: &str = "NFeRecepcaoEvento4";

struct Cenario {
    janela: WindowHandle<gpui_kit::base::Root>,
    raiz: Entity<Raiz>,
    tela: Entity<TelaRecebidas>,
    dialogo: Entity<DialogoRecebidas>,
    transporte: TransporteNfeDemonstracao,
    abridor: Arc<AbridorDeTeste>,
    notas: PathBuf,
    chaves: RecebidasDoEnsaio,
    servicos: Arc<Servicos>,
    _pasta: PastaTemporaria,
}

fn arquivo(pasta: &PastaTemporaria, pfx: &std::path::Path, ambiente: &str, uf: &str) -> Config {
    let texto = format!(
        "[emitente]\ncnpj = \"{CNPJ}\"\nnome = \"LOJA DE TESTE LTDA\"\ninscricao_municipal = \"123456\"\nmunicipio = \"3550308\"\nop_simples = 3\nregime_apuracao = 1\n\
         [sefin]\npfx = '{}'\n[notas]\npasta = '{}'\n\
         [nfe]\nambiente = \"{ambiente}\"\nuf = \"{uf}\"\nie = \"111111111111\"\ncrt = 1\nserie = \"1\"\n\
         [nfe.endereco]\nlogradouro = \"Rua das Flores\"\nnumero = \"100\"\nbairro = \"Centro\"\nmunicipio = \"Sao Paulo\"\ncep = \"01001000\"\n",
        pfx.to_string_lossy(),
        pasta.caminho().join("notas").to_string_lossy()
    );
    Config::de_texto(&texto).unwrap()
}

fn cenario(
    cx: &mut TestAppContext,
    ambiente: &str,
    nfe: RoteiroNfe,
    recebidas: RoteiroRecebidas,
    uf: &str,
) -> Cenario {
    cx.executor().allow_parking();
    let pasta = PastaTemporaria::nova("janela-recebidas");
    let pfx = pasta.certificado("a.pfx", CNPJ, SENHA, 365);
    let config = arquivo(&pasta, &pfx, ambiente, uf);
    let arq = pasta.caminho().join("rustnf.toml");
    config.salvar(&arq).unwrap();
    let transporte =
        TransporteNfeDemonstracao::novo(nfe, Duration::from_millis(30)).com_recebidas(recebidas);
    // sem a UF nao ha NF-e configurada: nada para semear
    let chaves = if uf.is_empty() {
        RecebidasDoEnsaio {
            resumo: String::new(),
            completa: String::new(),
            cancelada: String::new(),
            novas: Vec::new(),
        }
    } else {
        semear_recebidas(&config, &transporte).unwrap()
    };
    let cofre = Arc::new(CofreEmMemoria::novo());
    cofre.guardar(CNPJ, &Segredo::novo(SENHA.into())).unwrap();
    let abridor = Arc::new(AbridorDeTeste::default());
    let cofre_dyn: Arc<dyn Cofre> = cofre;
    let servicos = Arc::new(
        Servicos::com(
            Some(arq),
            Arc::new(|_| None),
            cofre_dyn,
            false,
            Arc::new(SemConexao),
            Arc::new(chrono::Utc::now),
        )
        .com_fabrica_nfe(FabricaNfeDemonstracao::nova(transporte.clone()))
        .com_abridor(abridor.clone())
        .com_intervalo(Duration::ZERO),
    );
    let (janela, raiz) = abrir_janela(cx, Arc::clone(&servicos), 1400., 2600.);
    cx.run_until_parked();
    let (tela, dialogo) = cx.update(|cx| {
        let r = raiz.read(cx);
        (r.recebidas.lista.clone(), r.recebidas.dialogo.clone())
    });
    Cenario {
        notas: pasta.caminho().join("notas"),
        janela,
        raiz,
        tela,
        dialogo,
        transporte,
        abridor,
        chaves,
        servicos,
        _pasta: pasta,
    }
}

fn normal(cx: &mut TestAppContext, ambiente: &str) -> Cenario {
    cenario(
        cx,
        ambiente,
        RoteiroNfe::Registrado,
        RoteiroRecebidas::Normal,
        "SP",
    )
}

fn mostrar(cx: &mut TestAppContext, c: &Cenario, total: usize) {
    cx.update(|cx| {
        c.raiz.update(cx, |r, cx| {
            r.ir_para(Tela::Notas, cx);
            r.escolher_notas(EscolhaDeNotas::Recebidas, cx);
        })
    });
    cx.run_until_parked();
    let tela = c.tela.clone();
    esperar_ate(cx, "a lista das recebidas", move |cx| {
        cx.update(|cx| {
            let t = tela.read(cx);
            t.carga == Carga::Pronta && t.lista.total() == total
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

fn existe(cx: &mut TestAppContext, c: &Cenario, id: &'static str) -> bool {
    cx.update_window(c.janela.into(), |_, window, cx| {
        window.render_frame(cx);
        window.try_find(id).is_some()
    })
    .unwrap()
}

fn escrever(cx: &mut TestAppContext, c: &Cenario, texto: &str) {
    let d = c.dialogo.clone();
    cx.update_window(c.janela.into(), |_, window, cx| {
        window.render_frame(cx);
        let campo = d.read(cx).texto.clone();
        campo.update(cx, |e, cx| e.focus(window, cx));
        window.input(texto, cx);
    })
    .unwrap();
    cx.run_until_parked();
}

fn abrir_a_nota(cx: &mut TestAppContext, c: &Cenario, chave: &str) {
    let tela = c.tela.clone();
    let alvo = chave.to_string();
    let indice = cx.update(|cx| {
        let t = tela.read(cx);
        (0..t.lista.quantas()).find(|i| t.lista.linha(*i).is_some_and(|l| l.chave == alvo))
    });
    let i = indice.expect("a nota esta na lista");
    cx.update_window(c.janela.into(), |_, window, cx| {
        window.render_frame(cx);
        tela.update(cx, |t, cx| t.abrir(i, window, cx));
    })
    .unwrap();
    cx.run_until_parked();
    let tela = c.tela.clone();
    esperar_ate(cx, "o detalhe da nota recebida", move |cx| {
        cx.update(|cx| tela.read(cx).detalhe_pronto())
    });
}

fn esperar_o_resultado(cx: &mut TestAppContext, c: &Cenario) -> Resultado {
    let d = c.dialogo.clone();
    esperar_ate(cx, "o desfecho do dialogo", move |cx| {
        cx.update(|cx| matches!(d.read(cx).fase, Fase::Desfecho(_)))
    });
    cx.update(|cx| match c.dialogo.read(cx).fase.clone() {
        Fase::Desfecho(r) => r,
        _ => unreachable!("esperou o desfecho"),
    })
}

fn fase(cx: &mut TestAppContext, c: &Cenario) -> Fase {
    cx.update(|cx| c.dialogo.read(cx).fase.clone())
}

fn registro(c: &Cenario, chave: &str) -> rustnf_app::notas::nfe::Recebida {
    NotasNfe::abrir(&c.notas)
        .unwrap()
        .recebidas()
        .unwrap()
        .registro(chave)
        .unwrap()
}

fn escolher_resposta(cx: &mut TestAppContext, c: &Cenario, r: Resposta) {
    let indice = Resposta::TODAS.iter().position(|x| *x == r).unwrap();
    let id: &'static str = [
        "recebidas-resposta-0",
        "recebidas-resposta-1",
        "recebidas-resposta-2",
        "recebidas-resposta-3",
    ][indice];
    clicar(cx, c, id);
}

// ---------------------------------------------------------------- o seletor e a lista

#[gpui_kit::test]
fn a_opcao_recebidas_aparece_e_e_lembrada_e_a_nova_nota_nao_a_oferece(cx: &mut TestAppContext) {
    let c = normal(cx, "homologacao");
    cx.update(|cx| c.raiz.update(cx, |r, cx| r.ir_para(Tela::Notas, cx)));
    cx.run_until_parked();
    assert!(
        existe(cx, &c, "notas-documento-3"),
        "Recebidas e' a quarta opcao"
    );
    assert!(
        existe(cx, &c, "atualizar-notas"),
        "a NFS-e segue sendo o padrao"
    );
    mostrar(cx, &c, 3);
    assert!(existe(cx, &c, "recebidas-buscar"));
    assert!(!existe(cx, &c, "atualizar-notas"));
    let config = c.servicos.efetiva(
        &Config::carregar_com(Some(&c._pasta.caminho().join("rustnf.toml")), &|_| None)
            .unwrap()
            .0,
    );
    esperar_ate(cx, "a escolha gravada", |_| {
        c.servicos.ler_escolha_de_notas(&config) == EscolhaDeNotas::Recebidas
    });
    // a Nova nota so' oferece o que se emite
    cx.update(|cx| c.raiz.update(cx, |r, cx| r.ir_para(Tela::NovaNota, cx)));
    cx.run_until_parked();
    assert!(existe(cx, &c, "notas-documento-2"));
    assert!(!existe(cx, &c, "notas-documento-3"));
    assert!(c.transporte.chamadas().is_empty());
}

#[gpui_kit::test]
fn a_lista_separa_a_situacao_da_nota_da_resposta_da_empresa(cx: &mut TestAppContext) {
    let c = normal(cx, "homologacao");
    mostrar(cx, &c, 3);
    let linhas = cx.update(|cx| {
        let l = &c.tela.read(cx).lista;
        (0..l.quantas())
            .filter_map(|i| l.linha(i).cloned())
            .collect::<Vec<_>>()
    });
    let de = |chave: &str| linhas.iter().find(|l| l.chave == chave).unwrap();
    let a = de(&c.chaves.resumo);
    assert_eq!(
        (a.nota, a.resposta, a.completa),
        (SituacaoDaNota::Autorizada, RespostaDada::Nenhuma, false)
    );
    assert_eq!(a.valor, "1.250,00");
    let b = de(&c.chaves.completa);
    assert_eq!(
        (b.nota, b.resposta, b.completa),
        (SituacaoDaNota::Autorizada, RespostaDada::Ciencia, true)
    );
    let x = de(&c.chaves.cancelada);
    assert_eq!(
        (x.nota, x.resposta),
        (SituacaoDaNota::Cancelada, RespostaDada::Nenhuma)
    );
    // o destaque do que falta responder e a ultima busca: so' a primeira nota espera resposta (a cancelada nao)
    let (a_responder, fila) = cx.update(|cx| {
        let t = c.tela.read(cx);
        (t.lista.a_responder(), t.fila.clone())
    });
    assert_eq!(a_responder, 1);
    assert!(fila.ultima_busca.is_some());
    assert!(fila.espera_ate.is_none(), "a semente nao deixa espera");
    assert!(
        c.transporte.chamadas().is_empty(),
        "olhar nao chama a SEFAZ"
    );
}

#[gpui_kit::test]
fn sem_a_uf_as_recebidas_explicam_o_que_falta_e_levam_as_configuracoes(cx: &mut TestAppContext) {
    let c = cenario(
        cx,
        "homologacao",
        RoteiroNfe::Registrado,
        RoteiroRecebidas::Normal,
        "",
    );
    cx.update(|cx| {
        c.raiz.update(cx, |r, cx| {
            r.ir_para(Tela::Notas, cx);
            r.escolher_notas(EscolhaDeNotas::Recebidas, cx);
        })
    });
    cx.run_until_parked();
    assert!(existe(cx, &c, "recebidas-ir-para-configuracoes"));
    clicar(cx, &c, "recebidas-ir-para-configuracoes");
    let (tela, secao) = cx.update(|cx| {
        let r = c.raiz.read(cx);
        (r.tela, r.secao)
    });
    assert_eq!((tela, secao), (Tela::Configuracoes, Secao::Nfe));
    assert!(c.transporte.chamadas().is_empty());
}

// ---------------------------------------------------------------- buscar notas novas

#[gpui_kit::test]
fn buscar_traz_tres_notas_novas_para_a_lista_e_a_proxima_busca_espera(cx: &mut TestAppContext) {
    let c = normal(cx, "homologacao");
    mostrar(cx, &c, 3);
    clicar(cx, &c, "recebidas-buscar");
    let Resultado::Busca(d) = esperar_o_resultado(cx, &c) else {
        panic!("era uma busca");
    };
    assert!(matches!(d, DesfechoBusca::Novas { novas: 3, .. }), "{d:?}");
    assert_eq!(d.titulo(), "3 notas novas");
    let tela = c.tela.clone();
    esperar_ate(cx, "as notas novas na lista", move |cx| {
        cx.update(|cx| tela.read(cx).lista.total() == 6)
    });
    assert!(c.transporte.chamadas_de(DIST) >= 1);
    // fecha o dialogo: o resultado fica na tela e a espera desliga o botao
    clicar(cx, &c, "recebidas-dialogo-fechar");
    let guardado = cx.update(|cx| c.tela.read(cx).ultima_resposta.clone());
    assert!(
        matches!(guardado, Some(DesfechoBusca::Novas { novas: 3, .. })),
        "o resultado fica na tela"
    );
    let tela = c.tela.clone();
    esperar_ate(cx, "a espera lida da pasta", move |cx| {
        cx.update(|cx| tela.read(cx).fila.espera_ate.is_some())
    });
    let chamadas = c.transporte.chamadas_de(DIST);
    clicar(cx, &c, "recebidas-buscar");
    assert_eq!(
        c.transporte.chamadas_de(DIST),
        chamadas,
        "ZERO chamadas com o botao desligado"
    );
}

#[gpui_kit::test]
fn a_espera_do_ambiente_nacional_desliga_o_botao_com_o_horario_e_clicar_nao_chama(
    cx: &mut TestAppContext,
) {
    let c = cenario(
        cx,
        "homologacao",
        RoteiroNfe::Registrado,
        RoteiroRecebidas::Espera,
        "SP",
    );
    mostrar(cx, &c, 3);
    clicar(cx, &c, "recebidas-buscar");
    let Resultado::Busca(d) = esperar_o_resultado(cx, &c) else {
        panic!("era uma busca");
    };
    let DesfechoBusca::Bloqueada { ate: Some(ate) } = d else {
        panic!("esperava o bloqueio com o horario: {d:?}");
    };
    clicar(cx, &c, "recebidas-dialogo-fechar");
    let tela = c.tela.clone();
    esperar_ate(cx, "a espera lida da pasta", move |cx| {
        cx.update(|cx| tela.read(cx).fila.espera_ate.is_some())
    });
    let hora = crate::modelo::nfe_recebidas::hora_de_brasilia(ate);
    let texto = cx.update(|cx| c.tela.read(cx).fila.texto_da_espera(chrono::Utc::now()));
    assert!(texto.unwrap().contains(&hora));
    let chamadas = c.transporte.chamadas_de(DIST);
    assert_eq!(chamadas, 1);
    clicar(cx, &c, "recebidas-buscar");
    clicar(cx, &c, "recebidas-buscar");
    assert_eq!(
        c.transporte.chamadas_de(DIST),
        chamadas,
        "ZERO chamadas ao clicar"
    );
    assert_eq!(
        cx.update(|cx| c.tela.read(cx).lista.total()),
        3,
        "nada chegou"
    );
}

#[gpui_kit::test]
fn sem_nada_novo_o_resultado_diz_isso(cx: &mut TestAppContext) {
    let c = cenario(
        cx,
        "homologacao",
        RoteiroNfe::Registrado,
        RoteiroRecebidas::Vazio,
        "SP",
    );
    mostrar(cx, &c, 3);
    clicar(cx, &c, "recebidas-buscar");
    let Resultado::Busca(d) = esperar_o_resultado(cx, &c) else {
        panic!("era uma busca");
    };
    assert!(matches!(d, DesfechoBusca::NadaNovo { .. }), "{d:?}");
    assert_eq!(d.titulo(), "Nada novo");
    assert_eq!(cx.update(|cx| c.tela.read(cx).lista.total()), 3);
}

// ---------------------------------------------------------------- o detalhe

#[gpui_kit::test]
fn o_resumo_oferece_baixar_e_responder_e_a_cancelada_nao_oferece_nada(cx: &mut TestAppContext) {
    let c = normal(cx, "homologacao");
    mostrar(cx, &c, 3);
    abrir_a_nota(cx, &c, &c.chaves.resumo);
    assert!(existe(cx, &c, "recebidas-baixar") && existe(cx, &c, "recebidas-responder"));
    assert!(
        !existe(cx, &c, "recebidas-abrir-danfe"),
        "o resumo nao e' uma nota"
    );
    abrir_a_nota(cx, &c, &c.chaves.cancelada);
    assert!(!existe(cx, &c, "recebidas-baixar") && !existe(cx, &c, "recebidas-responder"));
    assert!(
        !existe(cx, &c, "recebidas-abrir-danfe"),
        "a cancelada so' tem o resumo"
    );
    // a completa (com ciencia): DANFE e o mesmo detalhe da NF-e emitida
    abrir_a_nota(cx, &c, &c.chaves.completa);
    assert!(existe(cx, &c, "recebidas-abrir-danfe"));
    let leitura = cx.update(|cx| c.tela.read(cx).detalhe_leitura());
    let Some(LeituraNfe::Lida(d)) = leitura else {
        panic!("a nota completa devia estar lida");
    };
    assert!(
        !d.itens.is_empty(),
        "o mesmo detalhe da NF-e emitida: itens, totais e emissao"
    );
    assert!(!existe(cx, &c, "recebidas-baixar"));
    assert!(c.transporte.chamadas().is_empty());
}

#[gpui_kit::test]
fn baixar_com_ciencia_traz_a_nota_completa_e_o_danfe_abre(cx: &mut TestAppContext) {
    let c = normal(cx, "homologacao");
    mostrar(cx, &c, 3);
    abrir_a_nota(cx, &c, &c.chaves.resumo);
    clicar(cx, &c, "recebidas-baixar");
    assert!(existe(cx, &c, "recebidas-dialogo-enviar"));
    assert_eq!(
        c.transporte.chamadas_de(EVT),
        0,
        "abrir o dialogo nao envia nada"
    );
    clicar(cx, &c, "recebidas-dialogo-enviar");
    let Resultado::Baixa(d) = esperar_o_resultado(cx, &c) else {
        panic!("era uma baixa");
    };
    assert_eq!(
        d,
        DesfechoBaixa::Completa {
            ciencia_registrada: true
        }
    );
    assert_eq!(c.transporte.chamadas_de(EVT), 1);
    assert_eq!(
        registro(&c, &c.chaves.resumo).situacao,
        SituacaoRecebida::Ciencia
    );
    clicar(cx, &c, "recebidas-dialogo-fechar");
    // o detalhe se le de novo sozinho: agora e' a nota completa
    let tela = c.tela.clone();
    esperar_ate(cx, "o detalhe completo", move |cx| {
        cx.update(|cx| matches!(&tela.read(cx).detalhe_leitura(), Some(LeituraNfe::Lida(_))))
    });
    assert!(existe(cx, &c, "recebidas-abrir-danfe"));
    clicar(cx, &c, "recebidas-abrir-danfe");
    let abridor = c.abridor.clone();
    esperar_ate(cx, "o pedido de abrir o DANFE", move |_| {
        !abridor.pedidos().is_empty()
    });
    let (destino, caminho) = c.abridor.pedidos()[0].clone();
    assert_eq!(destino, Destino::Abrir);
    assert_eq!(caminho.extension().and_then(|e| e.to_str()), Some("pdf"));
    assert!(std::fs::read(&caminho).unwrap().starts_with(b"%PDF"));
}

#[gpui_kit::test]
fn em_producao_a_baixa_com_ciencia_nao_sai_sem_a_marcacao(cx: &mut TestAppContext) {
    let c = normal(cx, "producao");
    mostrar(cx, &c, 3);
    abrir_a_nota(cx, &c, &c.chaves.resumo);
    clicar(cx, &c, "recebidas-baixar");
    assert!(existe(cx, &c, "recebidas-entendo-efeito-fiscal"));
    clicar(cx, &c, "recebidas-dialogo-enviar");
    clicar(cx, &c, "recebidas-dialogo-enviar");
    assert!(
        c.transporte.chamadas().is_empty(),
        "sem a marcacao nada sai"
    );
    assert!(matches!(fase(cx, &c), Fase::Preenchendo));
    clicar(cx, &c, "recebidas-entendo-efeito-fiscal");
    clicar(cx, &c, "recebidas-dialogo-enviar");
    let Resultado::Baixa(d) = esperar_o_resultado(cx, &c) else {
        panic!("era uma baixa");
    };
    assert!(matches!(d, DesfechoBaixa::Completa { .. }), "{d:?}");
}

// ---------------------------------------------------------------- responder

fn responder_pela_janela(cx: &mut TestAppContext, r: Resposta, esperada: SituacaoRecebida) {
    let c = normal(cx, "homologacao");
    mostrar(cx, &c, 3);
    abrir_a_nota(cx, &c, &c.chaves.resumo);
    clicar(cx, &c, "recebidas-responder");
    // sem escolher, nada sai
    clicar(cx, &c, "recebidas-dialogo-enviar");
    assert_eq!(c.transporte.chamadas_de(EVT), 0);
    escolher_resposta(cx, &c, r);
    if r.pede_justificativa() {
        clicar(cx, &c, "recebidas-dialogo-enviar");
        assert_eq!(
            c.transporte.chamadas_de(EVT),
            0,
            "sem justificativa nada sai"
        );
        escrever(cx, &c, "curta");
        clicar(cx, &c, "recebidas-dialogo-enviar");
        assert_eq!(c.transporte.chamadas_de(EVT), 0, "abaixo do minimo");
        escrever(cx, &c, " demais, agora vale a justificativa");
    }
    // clique duplo: o segundo pedido chega com o primeiro em curso e nao envia de novo
    let d = c.dialogo.clone();
    cx.update_window(c.janela.into(), |_, window, cx| {
        window.render_frame(cx);
        d.update(cx, |d, cx| {
            d.enviar(window, cx);
            d.enviar(window, cx);
        });
    })
    .unwrap();
    cx.run_until_parked();
    let Resultado::Resposta(des, pedida) = esperar_o_resultado(cx, &c) else {
        panic!("era uma resposta");
    };
    assert!(des.registrado(), "{des:?}");
    assert_eq!(pedida, r);
    assert_eq!(c.transporte.chamadas_de(EVT), 1);
    assert_eq!(registro(&c, &c.chaves.resumo).situacao, esperada);
    // a lista acompanha sozinha
    let tela = c.tela.clone();
    let chave = c.chaves.resumo.clone();
    esperar_ate(cx, "a lista com a resposta", move |cx| {
        cx.update(|cx| {
            tela.read(cx).lista.da_chave(&chave).is_some_and(|l| {
                l.resposta == crate::modelo::nfe_recebidas_resultados::resposta_registrada(r.tipo())
            })
        })
    });
}

#[gpui_kit::test]
fn responder_confirmando_a_operacao(cx: &mut TestAppContext) {
    responder_pela_janela(cx, Resposta::Confirmar, SituacaoRecebida::Confirmada);
}

#[gpui_kit::test]
fn responder_com_ciencia(cx: &mut TestAppContext) {
    responder_pela_janela(cx, Resposta::Ciencia, SituacaoRecebida::Ciencia);
}

#[gpui_kit::test]
fn responder_desconhecendo_a_operacao(cx: &mut TestAppContext) {
    responder_pela_janela(cx, Resposta::Desconhecer, SituacaoRecebida::Desconhecida);
}

#[gpui_kit::test]
fn responder_operacao_nao_realizada_com_justificativa(cx: &mut TestAppContext) {
    responder_pela_janela(cx, Resposta::NaoRealizada, SituacaoRecebida::NaoRealizada);
}

#[gpui_kit::test]
fn em_producao_a_resposta_sem_a_marcacao_nao_chama_a_sefaz(cx: &mut TestAppContext) {
    let c = normal(cx, "producao");
    mostrar(cx, &c, 3);
    abrir_a_nota(cx, &c, &c.chaves.resumo);
    clicar(cx, &c, "recebidas-responder");
    escolher_resposta(cx, &c, Resposta::Confirmar);
    assert!(existe(cx, &c, "recebidas-entendo-efeito-fiscal"));
    clicar(cx, &c, "recebidas-dialogo-enviar");
    clicar(cx, &c, "recebidas-dialogo-enviar");
    assert!(
        c.transporte.chamadas().is_empty(),
        "ZERO chamadas sem a marcacao"
    );
    assert!(matches!(fase(cx, &c), Fase::Preenchendo));
    clicar(cx, &c, "recebidas-entendo-efeito-fiscal");
    clicar(cx, &c, "recebidas-dialogo-enviar");
    let Resultado::Resposta(d, _) = esperar_o_resultado(cx, &c) else {
        panic!("era uma resposta");
    };
    assert!(d.registrado(), "{d:?}");
    assert_eq!(c.transporte.chamadas_de(EVT), 1);
}

#[gpui_kit::test]
fn o_dialogo_desliga_as_respostas_que_a_sefaz_recusaria_antes_de_enviar(cx: &mut TestAppContext) {
    let c = normal(cx, "homologacao");
    mostrar(cx, &c, 3);
    // a nota ja com ciencia: dar ciencia de novo e' recusado (573), as outras valem
    abrir_a_nota(cx, &c, &c.chaves.completa);
    clicar(cx, &c, "recebidas-responder");
    let alvo = cx.update(|cx| c.dialogo.read(cx).alvo.clone()).unwrap();
    assert!(alvo.recusada(Resposta::Ciencia).is_some());
    assert!(alvo.recusada(Resposta::Confirmar).is_none());
    escolher_resposta(cx, &c, Resposta::Ciencia);
    let escolha = cx.update(|cx| c.dialogo.read(cx).formulario.escolha);
    assert_eq!(escolha, None, "a opcao desligada nao se escolhe");
    escolher_resposta(cx, &c, Resposta::Confirmar);
    let escolha = cx.update(|cx| c.dialogo.read(cx).formulario.escolha);
    assert_eq!(escolha, Some(Resposta::Confirmar));
    assert!(c.transporte.chamadas().is_empty());
}

#[gpui_kit::test]
fn recusada_pelo_ambiente_nacional_explica_sem_inventar_prazo_e_nao_muda_a_pasta(
    cx: &mut TestAppContext,
) {
    let c = cenario(
        cx,
        "homologacao",
        RoteiroNfe::Rejeitado,
        RoteiroRecebidas::Normal,
        "SP",
    );
    mostrar(cx, &c, 3);
    abrir_a_nota(cx, &c, &c.chaves.resumo);
    clicar(cx, &c, "recebidas-responder");
    escolher_resposta(cx, &c, Resposta::Confirmar);
    clicar(cx, &c, "recebidas-dialogo-enviar");
    let Resultado::Resposta(d, _) = esperar_o_resultado(cx, &c) else {
        panic!("era uma resposta");
    };
    let DesfechoNfe::Rejeitado {
        codigo, orientacao, ..
    } = &d
    else {
        panic!("esperava a recusa: {d:?}");
    };
    assert_eq!(codigo, "596");
    assert!(orientacao.contains("não o calcula"), "{orientacao}");
    assert_eq!(
        registro(&c, &c.chaves.resumo).situacao,
        SituacaoRecebida::Resumo
    );
}

#[gpui_kit::test]
fn sem_resposta_tenta_de_novo_e_registra_a_mesma_resposta(cx: &mut TestAppContext) {
    let c = cenario(
        cx,
        "homologacao",
        RoteiroNfe::SemResposta,
        RoteiroRecebidas::Normal,
        "SP",
    );
    mostrar(cx, &c, 3);
    abrir_a_nota(cx, &c, &c.chaves.resumo);
    clicar(cx, &c, "recebidas-responder");
    escolher_resposta(cx, &c, Resposta::Confirmar);
    clicar(cx, &c, "recebidas-dialogo-enviar");
    let Resultado::Resposta(d, _) = esperar_o_resultado(cx, &c) else {
        panic!("era uma resposta");
    };
    assert!(d.deixa_em_aberto(), "{d:?}");
    assert_eq!(
        registro(&c, &c.chaves.resumo).situacao,
        SituacaoRecebida::Resumo
    );
    clicar(cx, &c, "recebidas-tentar-de-novo");
    let dialogo = c.dialogo.clone();
    esperar_ate(cx, "o segundo envio", move |cx| {
        cx.update(|cx| {
            matches!(
                &dialogo.read(cx).fase,
                Fase::Desfecho(Resultado::Resposta(d, _)) if d.registrado()
            )
        })
    });
    assert_eq!(
        registro(&c, &c.chaves.resumo).situacao,
        SituacaoRecebida::Confirmada
    );
}
