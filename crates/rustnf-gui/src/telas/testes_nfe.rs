// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Testes da janela da NF-e, no ambiente de teste do GPUI, com a SEFAZ de mentira (nunca a rede), cofre em memoria,
//! pasta temporaria e um abridor de arquivos que so' anota o pedido: configurar e testar a conexao, alternar a lista
//! para NF-e, abrir o detalhe e o DANFE, cancelar (registrado, rejeitado, sem resposta), carta de correcao, inutilizar,
//! retomar a nota sem resposta e, em producao, nenhuma chamada sem a marcacao. Nenhum toca o cofre real, a pasta do
//! usuario, a rede nem abre programa externo.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use gpui_kit::component::WindowExt as _;
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AppContext as _, Entity, TestAppContext, WindowHandle};
use rustnf_app::cofre::{Cofre, CofreEmMemoria};
use rustnf_app::config::Config;
use rustnf_app::notas::nfe::{NotasNfe, Situacao};
use rustnf_app::segredo::Segredo;

use super::nfe::TelaNfe;
use super::nfe_dialogo::{DialogoNfe, Fase, Resposta};
use super::notas::Carga;
use super::testes_nota::{abrir_janela, SemConexao, CNPJ, SENHA};
use super::testes_notas::esperar_ate;
use super::{Raiz, Secao, Tela};
use crate::demonstracao_nfe::{
    semear, FabricaNfeDemonstracao, NotasDoEnsaio, RoteiroNfe, TransporteNfeDemonstracao,
};
use crate::modelo::nfe_eventos::DesfechoNfe;
use crate::modelo::nfe_lista::EscolhaDeNotas;
use crate::servicos::{Destino, Servicos};
use crate::testes_comuns::{AbridorDeTeste, PastaTemporaria};

const EVT: &str = "NFeRecepcaoEvento4";
const STA: &str = "NFeStatusServico4";

struct CenarioNfe {
    janela: WindowHandle<gpui_kit::base::Root>,
    raiz: Entity<Raiz>,
    nfe: Entity<TelaNfe>,
    dialogo: Entity<DialogoNfe>,
    transporte: TransporteNfeDemonstracao,
    abridor: Arc<AbridorDeTeste>,
    notas: PathBuf,
    arquivo: PathBuf,
    ensaio: Option<NotasDoEnsaio>,
    servicos: Arc<Servicos>,
    _pasta: PastaTemporaria,
}

fn arquivo_nfe(pasta: &PastaTemporaria, pfx: &std::path::Path, ambiente: &str, uf: &str) -> Config {
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

fn cenario_nfe(
    cx: &mut TestAppContext,
    ambiente: &str,
    roteiro: RoteiroNfe,
    semeado: bool,
    uf: &str,
    largura: f32,
) -> CenarioNfe {
    cx.executor().allow_parking();
    let pasta = PastaTemporaria::nova("janela-nfe");
    let pfx = pasta.certificado("a.pfx", CNPJ, SENHA, 365);
    let config = arquivo_nfe(&pasta, &pfx, ambiente, uf);
    let arquivo = pasta.caminho().join("rustnf.toml");
    config.salvar(&arquivo).unwrap();
    let transporte = TransporteNfeDemonstracao::novo(roteiro, Duration::from_millis(30));
    let ensaio = semeado.then(|| semear(&config, &transporte).unwrap());
    let cofre = Arc::new(CofreEmMemoria::novo());
    cofre.guardar(CNPJ, &Segredo::novo(SENHA.into())).unwrap();
    let abridor = Arc::new(AbridorDeTeste::default());
    let cofre_dyn: Arc<dyn Cofre> = cofre;
    let servicos = Arc::new(
        Servicos::com(
            Some(arquivo.clone()),
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
    let (janela, raiz) = abrir_janela(cx, Arc::clone(&servicos), largura, 2600.);
    cx.run_until_parked();
    let (nfe, dialogo) = cx.update(|cx| {
        let r = raiz.read(cx);
        (r.nfe.clone(), r.dialogo_nfe.clone())
    });
    CenarioNfe {
        notas: pasta.caminho().join("notas"),
        janela,
        raiz,
        nfe,
        dialogo,
        transporte,
        abridor,
        arquivo,
        ensaio,
        servicos,
        _pasta: pasta,
    }
}

fn mostrar_a_nfe(cx: &mut TestAppContext, c: &CenarioNfe, total: usize) {
    cx.update(|cx| {
        c.raiz.update(cx, |r, cx| {
            r.ir_para(Tela::Notas, cx);
            r.escolher_notas(EscolhaDeNotas::Nfe, cx);
        })
    });
    cx.run_until_parked();
    let nfe = c.nfe.clone();
    esperar_ate(cx, "a lista da NF-e", move |cx| {
        cx.update(|cx| {
            let n = nfe.read(cx);
            n.carga == Carga::Pronta && n.lista.total() == total
        })
    });
}

fn clicar(cx: &mut TestAppContext, c: &CenarioNfe, id: &'static str) {
    cx.update_window(c.janela.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click(id, cx);
    })
    .unwrap();
    cx.run_until_parked();
}

fn digitar(cx: &mut TestAppContext, c: &CenarioNfe, id: &'static str, texto: &str) {
    cx.update_window(c.janela.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click(id, cx);
        window.press("ctrl-a", cx);
        window.input(texto, cx);
    })
    .unwrap();
    cx.run_until_parked();
}

fn existe(cx: &mut TestAppContext, c: &CenarioNfe, id: &'static str) -> bool {
    cx.update_window(c.janela.into(), |_, window, cx| {
        window.render_frame(cx);
        window.try_find(id).is_some()
    })
    .unwrap()
}

/// O texto do dialogo (justificativa, correcao) e' um Textarea sem id: o foco vai pelo modelo.
fn escrever_no_dialogo(cx: &mut TestAppContext, c: &CenarioNfe, texto: &str) {
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

fn abrir_a_nota(cx: &mut TestAppContext, c: &CenarioNfe, chave: &str) {
    let nfe = c.nfe.clone();
    let alvo = chave.to_string();
    let indice = cx.update(|cx| {
        let n = nfe.read(cx);
        (0..n.lista.quantas()).find(|i| n.lista.linha(*i).is_some_and(|l| l.chave == alvo))
    });
    let i = indice.expect("a nota esta na lista");
    cx.update_window(c.janela.into(), |_, window, cx| {
        window.render_frame(cx);
        nfe.update(cx, |n, cx| n.abrir(i, window, cx));
    })
    .unwrap();
    cx.run_until_parked();
    let nfe = c.nfe.clone();
    esperar_ate(cx, "o detalhe da NF-e", move |cx| {
        cx.update(|cx| nfe.read(cx).detalhe_pronto())
    });
}

fn fase(cx: &mut TestAppContext, c: &CenarioNfe) -> Fase {
    cx.update(|cx| c.dialogo.read(cx).fase.clone())
}

fn desfecho(cx: &mut TestAppContext, c: &CenarioNfe) -> Option<Resposta> {
    match fase(cx, c) {
        Fase::Desfecho(r) => Some(r),
        _ => None,
    }
}

fn esperar_o_desfecho(cx: &mut TestAppContext, c: &CenarioNfe) -> Resposta {
    let (dialogo, _) = (c.dialogo.clone(), ());
    esperar_ate(cx, "o desfecho do dialogo", move |cx| {
        cx.update(|cx| matches!(dialogo.read(cx).fase, Fase::Desfecho(_)))
    });
    desfecho(cx, c).unwrap()
}

fn situacoes(c: &CenarioNfe) -> Vec<Situacao> {
    NotasNfe::abrir(&c.notas)
        .unwrap()
        .listar()
        .unwrap()
        .iter()
        .map(|r| r.situacao)
        .collect()
}

fn situacao_de(c: &CenarioNfe, chave: &str) -> Situacao {
    NotasNfe::abrir(&c.notas)
        .unwrap()
        .registro(chave)
        .unwrap()
        .situacao
}

const JUSTIFICATIVA: &str = "Cancelamento pedido pelo cliente antes da entrega";

// ---------------------------------------------------------------- a lista

#[gpui_kit::test]
fn a_lista_alterna_para_a_nfe_com_autorizada_cancelada_e_pendente(cx: &mut TestAppContext) {
    let c = cenario_nfe(cx, "homologacao", RoteiroNfe::Registrado, true, "SP", 1400.);
    let e = c.ensaio.clone().unwrap();
    // antes de escolher, a tela Notas e' a da NFS-e
    cx.update(|cx| c.raiz.update(cx, |r, cx| r.ir_para(Tela::Notas, cx)));
    cx.run_until_parked();
    assert!(
        existe(cx, &c, "atualizar-notas"),
        "a NFS-e segue sendo o padrao"
    );
    assert!(
        existe(cx, &c, "notas-documento-1"),
        "o seletor NFS-e | NF-e esta no alto"
    );
    mostrar_a_nfe(cx, &c, 3);
    assert!(existe(cx, &c, "nfe-atualizar"));
    assert!(!existe(cx, &c, "atualizar-notas"));
    let mut s = situacoes(&c);
    s.sort_by_key(|s| s.texto());
    assert_eq!(
        s,
        vec![
            Situacao::Autorizada,
            Situacao::Cancelada,
            Situacao::Pendente
        ]
    );
    // o destaque das notas sem resposta, com o texto e o botao Retomar
    assert!(existe(cx, &c, "nfe-retomar"));
    let n = c.nfe.clone();
    let linhas = cx.update(|cx| {
        let l = &n.read(cx).lista;
        (0..l.quantas())
            .filter_map(|i| l.linha(i).cloned())
            .collect::<Vec<_>>()
    });
    let pendente = linhas.iter().find(|l| l.chave == e.pendente).unwrap();
    assert!(pendente.aviso.as_deref().unwrap().contains("Retomar"));
    assert!(linhas
        .iter()
        .find(|l| l.chave == e.autorizada)
        .unwrap()
        .documento
        .starts_with("***."));
    // nada saiu para a SEFAZ so' de olhar
    assert!(c.transporte.chamadas().is_empty());
}

#[gpui_kit::test]
fn a_escolha_de_notas_e_lembrada_na_pasta(cx: &mut TestAppContext) {
    let c = cenario_nfe(cx, "homologacao", RoteiroNfe::Registrado, true, "SP", 1400.);
    mostrar_a_nfe(cx, &c, 3);
    let config = c
        .servicos
        .efetiva(&Config::carregar_com(Some(&c.arquivo), &|_| None).unwrap().0);
    esperar_ate(cx, "a escolha gravada", |_| {
        c.servicos.ler_escolha_de_notas(&config) == EscolhaDeNotas::Nfe
    });
    // de volta a NFS-e, a escolha muda e tambem e' lembrada
    cx.update(|cx| {
        c.raiz
            .update(cx, |r, cx| r.escolher_notas(EscolhaDeNotas::Nfse, cx))
    });
    esperar_ate(cx, "a escolha da NFS-e gravada", |_| {
        c.servicos.ler_escolha_de_notas(&config) == EscolhaDeNotas::Nfse
    });
}

#[gpui_kit::test]
fn sem_a_uf_a_nfe_explica_o_que_falta_e_leva_as_configuracoes(cx: &mut TestAppContext) {
    let c = cenario_nfe(cx, "homologacao", RoteiroNfe::Registrado, false, "", 1400.);
    cx.update(|cx| {
        c.raiz.update(cx, |r, cx| {
            r.ir_para(Tela::Notas, cx);
            r.escolher_notas(EscolhaDeNotas::Nfe, cx);
        })
    });
    cx.run_until_parked();
    assert!(existe(cx, &c, "nfe-ir-para-configuracoes"));
    clicar(cx, &c, "nfe-ir-para-configuracoes");
    let (tela, secao) = cx.update(|cx| {
        let r = c.raiz.read(cx);
        (r.tela, r.secao)
    });
    assert_eq!((tela, secao), (Tela::Configuracoes, Secao::Nfe));
    assert!(c.transporte.chamadas().is_empty());
}

// ---------------------------------------------------------------- detalhe e DANFE

#[gpui_kit::test]
fn o_detalhe_abre_e_o_danfe_vai_para_a_pasta_e_para_o_sistema(cx: &mut TestAppContext) {
    let c = cenario_nfe(cx, "homologacao", RoteiroNfe::Registrado, true, "SP", 1400.);
    let e = c.ensaio.clone().unwrap();
    mostrar_a_nfe(cx, &c, 3);
    abrir_a_nota(cx, &c, &e.autorizada);
    assert!(existe(cx, &c, "nfe-cancelar") && existe(cx, &c, "nfe-corrigir"));
    clicar(cx, &c, "nfe-abrir-danfe");
    let abridor = c.abridor.clone();
    esperar_ate(cx, "o pedido de abrir o DANFE", move |_| {
        !abridor.pedidos().is_empty()
    });
    let (destino, caminho) = c.abridor.pedidos()[0].clone();
    assert_eq!(destino, Destino::Abrir);
    assert_eq!(caminho.extension().and_then(|e| e.to_str()), Some("pdf"));
    assert!(std::fs::read(&caminho).unwrap().starts_with(b"%PDF"));
    // a nota cancelada tambem abre (com a marca de cancelada) e nao oferece cancelar de novo
    abrir_a_nota(cx, &c, &e.cancelada);
    assert!(!existe(cx, &c, "nfe-cancelar"));
    clicar(cx, &c, "nfe-abrir-danfe");
    let abridor = c.abridor.clone();
    esperar_ate(cx, "o DANFE da cancelada", move |_| {
        abridor.pedidos().len() == 2
    });
    assert!(c.transporte.chamadas().is_empty());
}

// ---------------------------------------------------------------- cancelar

#[gpui_kit::test]
fn cancelar_registrado_marca_a_pasta_e_o_botao_so_vale_com_a_justificativa(
    cx: &mut TestAppContext,
) {
    let c = cenario_nfe(cx, "homologacao", RoteiroNfe::Registrado, true, "SP", 1400.);
    let e = c.ensaio.clone().unwrap();
    mostrar_a_nfe(cx, &c, 3);
    abrir_a_nota(cx, &c, &e.autorizada);
    clicar(cx, &c, "nfe-cancelar");
    assert!(existe(cx, &c, "nfe-dialogo-enviar"));
    clicar(cx, &c, "nfe-dialogo-enviar");
    assert_eq!(
        c.transporte.chamadas_de(EVT),
        0,
        "sem justificativa nada sai"
    );
    escrever_no_dialogo(cx, &c, "curta");
    clicar(cx, &c, "nfe-dialogo-enviar");
    assert_eq!(
        c.transporte.chamadas_de(EVT),
        0,
        "abaixo do minimo da biblioteca"
    );
    escrever_no_dialogo(cx, &c, " demais para valer o cancelamento");
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
    let r = esperar_o_desfecho(cx, &c);
    assert!(
        matches!(r, Resposta::Pedido(DesfechoNfe::Registrado { .. })),
        "{r:?}"
    );
    assert_eq!(c.transporte.chamadas_de(EVT), 1);
    assert_eq!(situacao_de(&c, &e.autorizada), Situacao::Cancelada);
    // a lista atualiza sozinha
    let nfe = c.nfe.clone();
    esperar_ate(cx, "a lista com a nota cancelada", move |cx| {
        cx.update(|cx| {
            let l = &nfe.read(cx).lista;
            (0..l.quantas())
                .filter_map(|i| l.linha(i))
                .filter(|l| l.situacao == Situacao::Cancelada)
                .count()
                == 2
        })
    });
}

#[gpui_kit::test]
fn cancelar_rejeitado_por_prazo_explica_sem_inventar_prazo(cx: &mut TestAppContext) {
    let c = cenario_nfe(cx, "homologacao", RoteiroNfe::Rejeitado, true, "SP", 1400.);
    let e = c.ensaio.clone().unwrap();
    mostrar_a_nfe(cx, &c, 3);
    abrir_a_nota(cx, &c, &e.autorizada);
    clicar(cx, &c, "nfe-cancelar");
    escrever_no_dialogo(cx, &c, JUSTIFICATIVA);
    clicar(cx, &c, "nfe-dialogo-enviar");
    let Resposta::Pedido(d) = esperar_o_desfecho(cx, &c) else {
        panic!("era um pedido");
    };
    assert!(d.recusado_por_prazo(), "{d:?}");
    let DesfechoNfe::Rejeitado { orientacao, .. } = &d else {
        panic!("era uma rejeicao");
    };
    assert!(orientacao.contains("legislação de cada estado"));
    assert_eq!(
        situacao_de(&c, &e.autorizada),
        Situacao::Autorizada,
        "a nota nao mudou"
    );
}

#[gpui_kit::test]
fn cancelar_sem_resposta_confere_e_tenta_de_novo(cx: &mut TestAppContext) {
    let c = cenario_nfe(
        cx,
        "homologacao",
        RoteiroNfe::SemResposta,
        true,
        "SP",
        1400.,
    );
    let e = c.ensaio.clone().unwrap();
    mostrar_a_nfe(cx, &c, 3);
    abrir_a_nota(cx, &c, &e.autorizada);
    clicar(cx, &c, "nfe-cancelar");
    escrever_no_dialogo(cx, &c, JUSTIFICATIVA);
    clicar(cx, &c, "nfe-dialogo-enviar");
    let r = esperar_o_desfecho(cx, &c);
    // a consulta da nota respondeu sem o evento: nada foi registrado e e' seguro pedir de novo
    assert!(
        matches!(
            r,
            Resposta::Pedido(DesfechoNfe::NaoRegistrado | DesfechoNfe::SemResposta { .. })
        ),
        "{r:?}"
    );
    assert_eq!(situacao_de(&c, &e.autorizada), Situacao::Autorizada);
    clicar(cx, &c, "nfe-tentar-de-novo");
    let dialogo = c.dialogo.clone();
    esperar_ate(cx, "o segundo envio", move |cx| {
        cx.update(|cx| {
            matches!(
                &dialogo.read(cx).fase,
                Fase::Desfecho(Resposta::Pedido(DesfechoNfe::Registrado { .. }))
            )
        })
    });
    assert_eq!(situacao_de(&c, &e.autorizada), Situacao::Cancelada);
}

// ---------------------------------------------------------------- carta de correcao

#[gpui_kit::test]
fn carta_de_correcao_registra_e_conta_as_cartas(cx: &mut TestAppContext) {
    let c = cenario_nfe(cx, "homologacao", RoteiroNfe::Registrado, true, "SP", 1400.);
    let e = c.ensaio.clone().unwrap();
    mostrar_a_nfe(cx, &c, 3);
    abrir_a_nota(cx, &c, &e.autorizada);
    clicar(cx, &c, "nfe-corrigir");
    assert!(existe(cx, &c, "nfe-ver-texto-oficial"));
    let aviso = cx.update(|cx| c.dialogo.read(cx).corrigir.aviso_das_cartas());
    assert!(aviso.contains("1ª carta"), "{aviso}");
    clicar(cx, &c, "nfe-ver-texto-oficial");
    escrever_no_dialogo(cx, &c, "Corrigir o endereço de entrega informado");
    clicar(cx, &c, "nfe-dialogo-enviar");
    let r = esperar_o_desfecho(cx, &c);
    assert!(
        matches!(r, Resposta::Pedido(DesfechoNfe::Registrado { .. })),
        "{r:?}"
    );
    let reg = NotasNfe::abrir(&c.notas)
        .unwrap()
        .registro(&e.autorizada)
        .unwrap();
    assert_eq!(reg.situacao, Situacao::Autorizada);
    assert_eq!(reg.eventos.iter().filter(|x| x.tipo == "110110").count(), 1);
    // a segunda carta avisa que substitui a primeira
    let nfe = c.nfe.clone();
    esperar_ate(cx, "o detalhe lido de novo", move |cx| {
        cx.update(|cx| nfe.read(cx).detalhe_pronto())
    });
    cx.update_window(c.janela.into(), |_, window, cx| window.close_dialog(cx))
        .unwrap();
    cx.run_until_parked();
    clicar(cx, &c, "nfe-corrigir");
    let aviso = cx.update(|cx| c.dialogo.read(cx).corrigir.aviso_das_cartas());
    assert!(
        aviso.contains("SUBSTITUI") && aviso.contains("1 carta"),
        "{aviso}"
    );
}

// ---------------------------------------------------------------- inutilizar

#[gpui_kit::test]
fn inutilizar_uma_faixa_registra_e_guarda_na_pasta(cx: &mut TestAppContext) {
    let c = cenario_nfe(
        cx,
        "homologacao",
        RoteiroNfe::Registrado,
        false,
        "MG",
        1400.,
    );
    mostrar_a_nfe(cx, &c, 0);
    clicar(cx, &c, "nfe-inutilizar");
    assert!(existe(cx, &c, "nfe-inutilizar-inicio"));
    clicar(cx, &c, "nfe-dialogo-enviar");
    assert_eq!(c.transporte.chamadas().len(), 0);
    digitar(cx, &c, "nfe-inutilizar-inicio", "50");
    digitar(cx, &c, "nfe-inutilizar-fim", "52");
    escrever_no_dialogo(cx, &c, "Numeração pulada por falha na impressão");
    clicar(cx, &c, "nfe-dialogo-enviar");
    let r = esperar_o_desfecho(cx, &c);
    assert!(
        matches!(r, Resposta::Pedido(DesfechoNfe::Registrado { .. })),
        "{r:?}"
    );
    let faixas = NotasNfe::abrir(&c.notas).unwrap().inutilizacoes().unwrap();
    assert_eq!(faixas.len(), 1);
    assert_eq!((faixas[0].inicio, faixas[0].fim), (50, 52));
}

// SP, AC e BA perderam o servico de inutilizacao (NT 2023.002 v1.02): a biblioteca recusa antes da rede e a tela diz isso
#[gpui_kit::test]
fn em_sp_a_inutilizacao_e_recusada_antes_da_rede_com_a_explicacao(cx: &mut TestAppContext) {
    let c = cenario_nfe(
        cx,
        "homologacao",
        RoteiroNfe::Registrado,
        false,
        "SP",
        1400.,
    );
    mostrar_a_nfe(cx, &c, 0);
    clicar(cx, &c, "nfe-inutilizar");
    digitar(cx, &c, "nfe-inutilizar-inicio", "50");
    escrever_no_dialogo(cx, &c, "Numeração pulada por falha na impressão");
    clicar(cx, &c, "nfe-dialogo-enviar");
    let r = esperar_o_desfecho(cx, &c);
    assert!(
        matches!(&r, Resposta::Pedido(DesfechoNfe::Recusado { texto }) if texto.contains("nao tem mais o servico de inutilizacao")),
        "{r:?}"
    );
    assert!(c.transporte.chamadas().is_empty());
}

#[gpui_kit::test]
fn inutilizar_numero_de_nota_existente_nem_chega_a_sefaz(cx: &mut TestAppContext) {
    let c = cenario_nfe(cx, "homologacao", RoteiroNfe::Registrado, true, "SP", 1400.);
    mostrar_a_nfe(cx, &c, 3);
    clicar(cx, &c, "nfe-inutilizar");
    digitar(cx, &c, "nfe-inutilizar-inicio", "1");
    escrever_no_dialogo(cx, &c, "Tentando inutilizar um numero ja usado");
    clicar(cx, &c, "nfe-dialogo-enviar");
    let r = esperar_o_desfecho(cx, &c);
    assert!(
        matches!(&r, Resposta::Pedido(DesfechoNfe::Recusado { texto }) if texto.contains("ja esta")),
        "{r:?}"
    );
    assert!(
        c.transporte.chamadas().is_empty(),
        "a recusa e' antes da rede"
    );
}

// ---------------------------------------------------------------- retomar

#[gpui_kit::test]
fn a_pendente_retomada_vira_autorizada(cx: &mut TestAppContext) {
    let c = cenario_nfe(cx, "homologacao", RoteiroNfe::Registrado, true, "SP", 1400.);
    let e = c.ensaio.clone().unwrap();
    mostrar_a_nfe(cx, &c, 3);
    assert_eq!(situacao_de(&c, &e.pendente), Situacao::Pendente);
    clicar(cx, &c, "nfe-retomar");
    assert!(existe(cx, &c, "nfe-dialogo-enviar"));
    clicar(cx, &c, "nfe-dialogo-enviar");
    let r = esperar_o_desfecho(cx, &c);
    let Resposta::Retomada(resumo) = r else {
        panic!("era uma retomada: {r:?}");
    };
    assert!(resumo.texto.contains("resolvidas"), "{}", resumo.texto);
    assert_eq!(situacao_de(&c, &e.pendente), Situacao::Autorizada);
    // nenhuma nota nova nasceu: continuam tres, e a faixa dos pendentes saiu
    assert_eq!(situacoes(&c).len(), 3);
    let nfe = c.nfe.clone();
    esperar_ate(cx, "a lista sem pendentes", move |cx| {
        cx.update(|cx| nfe.read(cx).lista.a_resolver() == 0)
    });
    assert!(!existe(cx, &c, "nfe-pendentes"));
    assert_eq!(
        c.transporte.chamadas_de("NFeAutorizacao4"),
        0,
        "so' consultou"
    );
}

#[gpui_kit::test]
fn retomar_sem_resposta_deixa_a_nota_pendente_e_nao_emite_outra(cx: &mut TestAppContext) {
    let c = cenario_nfe(
        cx,
        "homologacao",
        RoteiroNfe::SemResposta,
        true,
        "SP",
        1400.,
    );
    let e = c.ensaio.clone().unwrap();
    mostrar_a_nfe(cx, &c, 3);
    clicar(cx, &c, "nfe-retomar");
    clicar(cx, &c, "nfe-dialogo-enviar");
    let r = esperar_o_desfecho(cx, &c);
    let Resposta::Retomada(resumo) = r else {
        panic!("era uma retomada");
    };
    assert!(
        resumo.texto.contains("não emita outra nota"),
        "{}",
        resumo.texto
    );
    assert_eq!(situacao_de(&c, &e.pendente), Situacao::Pendente);
    assert_eq!(situacoes(&c).len(), 3);
}

// ---------------------------------------------------------------- producao

#[gpui_kit::test]
fn em_producao_sem_a_marcacao_nada_sai_para_a_sefaz(cx: &mut TestAppContext) {
    let c = cenario_nfe(cx, "producao", RoteiroNfe::Registrado, true, "SP", 1400.);
    let e = c.ensaio.clone().unwrap();
    mostrar_a_nfe(cx, &c, 3);
    abrir_a_nota(cx, &c, &e.autorizada);
    clicar(cx, &c, "nfe-cancelar");
    assert!(existe(cx, &c, "nfe-entendo-efeito-fiscal"));
    escrever_no_dialogo(cx, &c, JUSTIFICATIVA);
    // justificativa boa, producao, sem a marcacao: o botao esta desligado e o envio direto tambem recusa
    clicar(cx, &c, "nfe-dialogo-enviar");
    let d = c.dialogo.clone();
    cx.update_window(c.janela.into(), |_, window, cx| {
        d.update(cx, |d, cx| d.enviar(window, cx));
    })
    .unwrap();
    cx.run_until_parked();
    assert!(
        c.transporte.chamadas().is_empty(),
        "ZERO chamadas sem a marcacao"
    );
    assert!(matches!(fase(cx, &c), Fase::Preenchendo));
    // a retomada e a inutilizacao tambem exigem a marcacao
    cx.update_window(c.janela.into(), |_, window, cx| window.close_dialog(cx))
        .unwrap();
    cx.run_until_parked();
    clicar(cx, &c, "nfe-retomar");
    clicar(cx, &c, "nfe-dialogo-enviar");
    assert!(
        c.transporte.chamadas().is_empty(),
        "retomar em producao tambem pede a marcacao"
    );
    clicar(cx, &c, "nfe-entendo-efeito-fiscal");
    clicar(cx, &c, "nfe-dialogo-enviar");
    let r = esperar_o_desfecho(cx, &c);
    assert!(matches!(r, Resposta::Retomada(_)));
    assert!(!c.transporte.chamadas().is_empty(), "com a marcacao, sai");
}

#[gpui_kit::test]
fn em_producao_com_a_marcacao_o_cancelamento_sai(cx: &mut TestAppContext) {
    let c = cenario_nfe(cx, "producao", RoteiroNfe::Registrado, true, "SP", 1400.);
    let e = c.ensaio.clone().unwrap();
    mostrar_a_nfe(cx, &c, 3);
    abrir_a_nota(cx, &c, &e.autorizada);
    clicar(cx, &c, "nfe-cancelar");
    escrever_no_dialogo(cx, &c, JUSTIFICATIVA);
    clicar(cx, &c, "nfe-entendo-efeito-fiscal");
    clicar(cx, &c, "nfe-dialogo-enviar");
    let r = esperar_o_desfecho(cx, &c);
    assert!(
        matches!(r, Resposta::Pedido(DesfechoNfe::Registrado { .. })),
        "{r:?}"
    );
    assert_eq!(c.transporte.chamadas_de(EVT), 1);
}

// ---------------------------------------------------------------- configuracoes

#[gpui_kit::test]
fn configurar_a_nfe_valida_testa_a_conexao_e_grava_no_mesmo_arquivo(cx: &mut TestAppContext) {
    let c = cenario_nfe(
        cx,
        "homologacao",
        RoteiroNfe::Registrado,
        false,
        "SP",
        1400.,
    );
    cx.update(|cx| {
        c.raiz.update(cx, |r, cx| {
            r.tela = Tela::Configuracoes;
            r.secao = Secao::Nfe;
            cx.notify();
        })
    });
    cx.run_until_parked();
    assert!(existe(cx, &c, "nfe-testar-conexao"));
    // erro de campo: CEP incompleto
    digitar(cx, &c, "nfe-cep", "0100");
    let erro = cx.update(|cx| c.raiz.read(cx).form_nfe.erros().values().next().cloned());
    assert!(erro.unwrap().contains("CEP"));
    let (alterado, motivo) = cx.update(|cx| {
        let r = c.raiz.read(cx);
        (r.alterado(), r.motivo_sem_salvar())
    });
    assert!(alterado);
    assert!(motivo.unwrap().contains("CEP"));
    // corrige, muda a UF e a serie, e salva pelo mesmo Salvar da NFS-e
    digitar(cx, &c, "nfe-cep", "20040020");
    digitar(cx, &c, "nfe-uf", "rj");
    digitar(cx, &c, "nfe-serie", "2");
    // Testar conexao: so' pergunta o status do servico
    clicar(cx, &c, "nfe-testar-conexao");
    let raiz = c.raiz.clone();
    esperar_ate(cx, "o teste de conexao", move |cx| {
        cx.update(|cx| raiz.read(cx).resultado_sefaz.is_some())
    });
    assert_eq!(c.transporte.chamadas(), vec![STA]);
    let ok = cx.update(|cx| c.raiz.read(cx).resultado_sefaz.clone().unwrap().ok());
    assert!(ok);
    cx.update_window(c.janela.into(), |_, window, cx| {
        c.raiz.update(cx, |r, cx| r.salvar(window, cx));
    })
    .unwrap();
    esperar_ate(cx, "o arquivo gravado", |_| {
        Config::carregar_com(Some(&c.arquivo), &|_| None).is_ok_and(|(cfg, _)| cfg.nfe.uf == "RJ")
    });
    let (gravada, _) = Config::carregar_com(Some(&c.arquivo), &|_| None).unwrap();
    assert_eq!(gravada.nfe.serie, "2");
    assert_eq!(gravada.nfe.endereco.cep, "20040020");
    assert_eq!(
        gravada.emitente.cnpj, CNPJ,
        "o resto do arquivo nao se perdeu"
    );
    assert_eq!(c.transporte.chamadas_de(EVT), 0);
}

#[gpui_kit::test]
fn a_contingencia_svc_exige_o_motivo_antes_de_gravar(cx: &mut TestAppContext) {
    let c = cenario_nfe(
        cx,
        "homologacao",
        RoteiroNfe::Registrado,
        false,
        "SP",
        1400.,
    );
    cx.update(|cx| {
        c.raiz.update(cx, |r, cx| {
            r.tela = Tela::Configuracoes;
            r.secao = Secao::Nfe;
            r.form_nfe.dados.svc = true;
            cx.notify();
        })
    });
    cx.run_until_parked();
    assert!(existe(cx, &c, "nfe-motivo-contingencia"));
    let motivo = cx.update(|cx| c.raiz.read(cx).motivo_sem_salvar());
    let m = motivo.unwrap();
    assert!(m.contains("contingência"), "{m}");
    digitar(
        cx,
        &c,
        "nfe-motivo-contingencia",
        "SEFAZ do estado fora do ar desde as nove horas",
    );
    let motivo = cx.update(|cx| c.raiz.read(cx).motivo_sem_salvar());
    assert_eq!(motivo, None);
}
