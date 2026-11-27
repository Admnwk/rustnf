// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Testes da janela da Nova NF-e, no ambiente de teste do GPUI, com a SEFAZ de mentira (nunca a rede), cofre em memoria e
//! pasta temporaria: o assistente do inicio ao fim (autorizada), o produto do cadastro e o produto novo, a rejeitada que se
//! corrige e sai com o mesmo numero, o caminho do Retomar para a nota sem resposta (sem nota gemea), a SEFAZ fora e a
//! contingencia EPEC (so' sem nota pendente), a producao sem a marcacao (zero chamadas), a nota anterior como modelo e a
//! tela de produtos. Nenhum teste toca o cofre real, a pasta do usuario nem a rede.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AppContext as _, Entity, TestAppContext, WindowHandle};
use rustnf_app::cofre::{Cofre, CofreEmMemoria};
use rustnf_app::config::Config;
use rustnf_app::notas::nfe::{NotasNfe, Situacao};
use rustnf_app::segredo::Segredo;

use super::nfe_nova::{Fase, NovaNfe};
use super::produtos::TelaProdutos;
use super::testes_nota::{abrir_janela, SemConexao, CNPJ, SENHA};
use super::testes_notas::esperar_ate;
use super::{Raiz, Tela};
use crate::demonstracao_nfe::{
    semear_com, FabricaNfeDemonstracao, NotasDoEnsaio, RoteiroEmissao, RoteiroNfe,
    TransporteNfeDemonstracao,
};
use crate::modelo::nfe_emissao::DesfechoNfeEmissao;
use crate::modelo::nfe_lista::EscolhaDeNotas;
use crate::modelo::nfe_nova::PassoNfe;
use crate::servicos::Servicos;
use crate::testes_comuns::PastaTemporaria;

const AUT: &str = "NFeAutorizacao4";
const EVT: &str = "NFeRecepcaoEvento4";
const STA: &str = "NFeStatusServico4";

struct Cenario {
    janela: WindowHandle<gpui_kit::base::Root>,
    raiz: Entity<Raiz>,
    nova: Entity<NovaNfe>,
    produtos: Entity<TelaProdutos>,
    transporte: TransporteNfeDemonstracao,
    notas: PathBuf,
    ensaio: Option<NotasDoEnsaio>,
    servicos: Arc<Servicos>,
    config: Config,
    _pasta: PastaTemporaria,
}

fn arquivo(pasta: &PastaTemporaria, pfx: &std::path::Path, ambiente: &str, crt: u8) -> Config {
    let texto = format!(
        "[emitente]\ncnpj = \"{CNPJ}\"\nnome = \"LOJA DE TESTE LTDA\"\ninscricao_municipal = \"123456\"\nmunicipio = \"3550308\"\nop_simples = 3\nregime_apuracao = 1\n\
         [sefin]\npfx = '{}'\n[notas]\npasta = '{}'\n\
         [nfe]\nambiente = \"{ambiente}\"\nuf = \"SP\"\nie = \"111111111111\"\ncrt = {crt}\nserie = \"1\"\n\
         [nfe.endereco]\nlogradouro = \"Rua das Flores\"\nnumero = \"100\"\nbairro = \"Centro\"\nmunicipio = \"Sao Paulo\"\ncep = \"01001000\"\n",
        pfx.to_string_lossy(),
        pasta.caminho().join("notas").to_string_lossy()
    );
    Config::de_texto(&texto).unwrap()
}

/// `semeado`: `Some(true)` a pasta ganha as tres NF-e (uma sem resposta), `Some(false)` as duas sem a pendente, `None` vazia.
fn cenario(
    cx: &mut TestAppContext,
    ambiente: &str,
    emissao: RoteiroEmissao,
    semeado: Option<bool>,
) -> Cenario {
    cx.executor().allow_parking();
    let pasta = PastaTemporaria::nova("janela-nfe-nova");
    let pfx = pasta.certificado("a.pfx", CNPJ, SENHA, 365);
    let config = arquivo(&pasta, &pfx, ambiente, 1);
    let arquivo_toml = pasta.caminho().join("rustnf.toml");
    config.salvar(&arquivo_toml).unwrap();
    let transporte =
        TransporteNfeDemonstracao::novo(RoteiroNfe::Registrado, Duration::from_millis(30))
            .com_emissao(emissao);
    let ensaio = semeado.map(|com| semear_com(&config, &transporte, com).unwrap());
    let cofre = Arc::new(CofreEmMemoria::novo());
    cofre.guardar(CNPJ, &Segredo::novo(SENHA.into())).unwrap();
    let cofre_dyn: Arc<dyn Cofre> = cofre;
    let servicos = Arc::new(
        Servicos::com(
            Some(arquivo_toml),
            Arc::new(|_| None),
            cofre_dyn,
            false,
            Arc::new(SemConexao),
            Arc::new(chrono::Utc::now),
        )
        .com_fabrica_nfe(FabricaNfeDemonstracao::nova(transporte.clone()))
        .com_intervalo(Duration::ZERO),
    );
    let (janela, raiz) = abrir_janela(cx, Arc::clone(&servicos), 1300., 3200.);
    cx.run_until_parked();
    let (nova, produtos) = cx.update(|cx| {
        let r = raiz.read(cx);
        (r.nova_nfe.clone(), r.produtos.clone())
    });
    Cenario {
        notas: pasta.caminho().join("notas"),
        janela,
        raiz,
        nova,
        produtos,
        transporte,
        ensaio,
        servicos,
        config,
        _pasta: pasta,
    }
}

fn mostrar_a_nova_nfe(cx: &mut TestAppContext, c: &Cenario) {
    cx.update(|cx| {
        c.raiz.update(cx, |r, cx| {
            r.ir_para(Tela::NovaNota, cx);
            r.escolher_notas(EscolhaDeNotas::Nfe, cx);
        })
    });
    cx.run_until_parked();
    let nova = c.nova.clone();
    esperar_ate(cx, "a Nova NF-e pronta", move |cx| {
        cx.update(|cx| nova.read(cx).pronta)
    });
    // o cadastro e o contexto chegam fora da tela
    let nova = c.nova.clone();
    esperar_ate(cx, "o cadastro lido", move |cx| {
        cx.update(|cx| !nova.read(cx).cadastro.produtos.is_empty() || nova.read(cx).dados.vazio())
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

fn digitar(cx: &mut TestAppContext, c: &Cenario, id: &str, texto: &str) {
    let id = id.to_string();
    cx.update_window(c.janela.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click(id.clone(), cx);
        window.press("ctrl-a", cx);
        window.input(texto, cx);
    })
    .unwrap();
    cx.run_until_parked();
}

fn existe(cx: &mut TestAppContext, c: &Cenario, id: &str) -> bool {
    let id = id.to_string();
    cx.update_window(c.janela.into(), |_, window, cx| {
        window.render_frame(cx);
        window.try_find(id).is_some()
    })
    .unwrap()
}

fn passo(cx: &mut TestAppContext, c: &Cenario) -> PassoNfe {
    cx.update(|cx| c.nova.read(cx).passo)
}

fn fase(cx: &mut TestAppContext, c: &Cenario) -> Fase {
    cx.update(|cx| c.nova.read(cx).fase.clone())
}

fn desfecho(cx: &mut TestAppContext, c: &Cenario) -> Option<DesfechoNfeEmissao> {
    match fase(cx, c) {
        Fase::Desfecho(d) => Some(*d),
        _ => None,
    }
}

fn esperar_o_desfecho(cx: &mut TestAppContext, c: &Cenario) -> DesfechoNfeEmissao {
    let nova = c.nova.clone();
    esperar_ate(cx, "o desfecho da NF-e", move |cx| {
        cx.update(|cx| matches!(nova.read(cx).fase, Fase::Desfecho(_)))
    });
    desfecho(cx, c).unwrap()
}

fn esperar_a_conferencia(cx: &mut TestAppContext, c: &Cenario) {
    let nova = c.nova.clone();
    esperar_ate(cx, "a conferência", move |cx| {
        cx.update(|cx| {
            let n = nova.read(cx);
            !n.conferindo && (n.conferencia.limpa() || !n.conferencia.erros.is_empty())
        })
    });
}

fn notas_da_pasta(c: &Cenario) -> Vec<(String, Situacao)> {
    NotasNfe::abrir(&c.notas)
        .unwrap()
        .listar()
        .unwrap()
        .iter()
        .map(|r| (r.numero.clone(), r.situacao))
        .collect()
}

/// Preenche o "Para quem" pela tela: cliente pessoa fisica de Sao Paulo, para uso proprio, sem inscricao estadual.
fn para_quem(cx: &mut TestAppContext, c: &Cenario) {
    digitar(cx, c, "nfe-Documento", "52998224725");
    digitar(cx, c, "nfe-Nome", "MARIA SOUZA DE TESTE");
    clicar(cx, c, "nfe-consumo-0");
    // CPF: a pessoa escolhe que nao tem inscricao estadual
    clicar(cx, c, "nfe-contribuinte-1");
    digitar(cx, c, "nfe-Logradouro", "Avenida Paulista");
    digitar(cx, c, "nfe-Numero", "1000");
    digitar(cx, c, "nfe-Bairro", "Bela Vista");
    digitar(cx, c, "nfe-CodMunicipio", "3550308");
}

/// Anda do "Para quem" ate o "Conferir" com um produto do cadastro (o primeiro que a busca achar).
fn ate_o_conferir_com_produto_do_cadastro(cx: &mut TestAppContext, c: &Cenario) {
    mostrar_a_nova_nfe(cx, c);
    para_quem(cx, c);
    clicar(cx, c, "nfe-continuar");
    assert_eq!(passo(cx, c), PassoNfe::OQueVendeu);
    digitar(cx, c, "nfe-busca-de-produto", "cafe torrado");
    clicar(cx, c, "nfe-adicionar-produto-0");
    clicar(cx, c, "nfe-continuar");
    assert_eq!(passo(cx, c), PassoNfe::ComoRecebeu);
    clicar(cx, c, "nfe-continuar");
    assert_eq!(passo(cx, c), PassoNfe::ComoEntrega);
    clicar(cx, c, "nfe-continuar");
    assert_eq!(passo(cx, c), PassoNfe::Conferir);
    esperar_a_conferencia(cx, c);
}

// ---------------------------------------------------------------- o assistente do inicio ao fim

#[gpui_kit::test]
fn o_assistente_do_inicio_ao_fim_autoriza_a_nota_e_guarda_o_produto_novo(cx: &mut TestAppContext) {
    let c = cenario(cx, "homologacao", RoteiroEmissao::Autorizada, Some(true));
    mostrar_a_nova_nfe(cx, &c);
    assert_eq!(passo(cx, &c), PassoNfe::ParaQuem);
    // continuar sem preencher: o passo nao anda e diz o que falta
    clicar(cx, &c, "nfe-continuar");
    assert_eq!(passo(cx, &c), PassoNfe::ParaQuem);
    assert!(
        cx.update(|cx| c.nova.read(cx).tentou),
        "a tela mostra o que falta"
    );
    para_quem(cx, &c);
    clicar(cx, &c, "nfe-continuar");
    assert_eq!(passo(cx, &c), PassoNfe::OQueVendeu);
    // produto novo, com NCM, descricao e valor
    clicar(cx, &c, "nfe-produto-novo");
    let id = cx.update(|cx| c.nova.read(cx).linhas[0].id);
    digitar(
        cx,
        &c,
        &format!("nfe-item-{id}-Descricao"),
        "Moedor de cafe manual",
    );
    digitar(cx, &c, &format!("nfe-item-{id}-Codigo"), "M7");
    digitar(cx, &c, &format!("nfe-item-{id}-Valor"), "50");
    digitar(cx, &c, &format!("nfe-item-{id}-Ncm"), "82100000");
    clicar(cx, &c, "nfe-continuar");
    assert_eq!(passo(cx, &c), PassoNfe::ComoRecebeu);
    // Pix
    clicar(cx, &c, "nfe-pagamento-1");
    clicar(cx, &c, "nfe-continuar");
    clicar(cx, &c, "nfe-continuar");
    assert_eq!(passo(cx, &c), PassoNfe::Conferir);
    esperar_a_conferencia(cx, &c);
    let (limpa, total, numero) = cx.update(|cx| {
        let n = c.nova.read(cx);
        (
            n.conferencia.limpa(),
            n.conferencia.total.clone(),
            n.conferencia.numero,
        )
    });
    assert!(
        limpa,
        "{:?}",
        cx.update(|cx| c.nova.read(cx).conferencia.erros.clone())
    );
    assert_eq!(total, "50,00");
    // a conferencia nao gasta numero nem fala com a SEFAZ
    assert_eq!(numero, 4, "as 3 notas do ensaio usam 1, 2 e 3");
    assert!(c.transporte.chamadas().is_empty());
    clicar(cx, &c, "nfe-enviar");
    let d = esperar_o_desfecho(cx, &c);
    match &d {
        DesfechoNfeEmissao::Autorizada {
            numero,
            guardada,
            protocolo,
            ..
        } => {
            assert_eq!(numero, "4");
            assert!(guardada);
            assert!(!protocolo.is_empty());
        }
        outro => panic!("{outro:?}"),
    }
    assert_eq!(c.transporte.chamadas_de(AUT), 1);
    assert!(notas_da_pasta(&c).contains(&("4".into(), Situacao::Autorizada)));
    assert!(existe(cx, &c, "nfe-copiar-chave"));
    assert!(existe(cx, &c, "nfe-ver-na-lista"));
    // o produto novo entrou no cadastro (os tres do ensaio mais ele)
    let servicos = Arc::clone(&c.servicos);
    let config = c.config.clone();
    esperar_ate(cx, "o produto guardado", move |_| {
        servicos
            .ler_produtos(&config)
            .cadastro
            .produtos
            .iter()
            .any(|p| p.codigo == "M7" && p.ncm == "82100000")
    });
    // o rascunho some com a nota feita
    let r = c.servicos.ler_rascunho_nfe(&c.config);
    assert!(r.is_none(), "o rascunho não serve mais");
    // Nova NF-e: o assistente volta vazio
    clicar(cx, &c, "nfe-nova-nota-limpa");
    assert_eq!(passo(cx, &c), PassoNfe::ParaQuem);
    assert!(cx.update(|cx| c.nova.read(cx).dados.vazio()));
}

#[gpui_kit::test]
fn o_produto_do_cadastro_vem_com_ncm_valor_e_a_conferencia_fecha(cx: &mut TestAppContext) {
    let c = cenario(cx, "homologacao", RoteiroEmissao::Autorizada, Some(true));
    ate_o_conferir_com_produto_do_cadastro(cx, &c);
    let (desc, ncm, valor, total) = cx.update(|cx| {
        let n = c.nova.read(cx);
        let i = &n.dados.itens[0];
        (
            i.descricao.clone(),
            i.ncm.clone(),
            i.valor.clone(),
            n.conferencia.total.clone(),
        )
    });
    assert_eq!(desc, "Cafe torrado 500g");
    assert_eq!(ncm, "09012100");
    assert_eq!(valor, "15,90");
    assert_eq!(total, "15,90");
    // o produto do cadastro nao entra de novo no cadastro quando nada mudou
    let sem_novos = cx.update(|cx| {
        let n = c.nova.read(cx);
        n.cadastro.novos_da_nota(&n.dados.itens).is_empty()
    });
    assert!(sem_novos);
    assert!(c.transporte.chamadas().is_empty());
}

#[gpui_kit::test]
fn rejeitada_volta_a_editar_e_a_correcao_autoriza_com_o_mesmo_numero(cx: &mut TestAppContext) {
    let c = cenario(cx, "homologacao", RoteiroEmissao::Rejeitada, Some(true));
    ate_o_conferir_com_produto_do_cadastro(cx, &c);
    clicar(cx, &c, "nfe-enviar");
    let nova = c.nova.clone();
    esperar_ate(cx, "a recusa da SEFAZ", move |cx| {
        cx.update(|cx| {
            matches!(nova.read(cx).fase, Fase::Editando) && nova.read(cx).rejeicao.is_some()
        })
    });
    let r = cx.update(|cx| c.nova.read(cx).rejeicao.clone()).unwrap();
    match r {
        DesfechoNfeEmissao::Rejeitada {
            cstat,
            motivo,
            o_que_fazer,
            numero,
            ..
        } => {
            assert_eq!(cstat, "778");
            assert!(motivo.contains("NCM"), "{motivo}");
            assert_eq!(numero, "4");
            assert!(o_que_fazer.contains("continua livre"));
        }
        outro => panic!("{outro:?}"),
    }
    // a SEFAZ recusou: nada ficou na pasta e o numero 4 continua livre
    assert!(!notas_da_pasta(&c).iter().any(|(n, _)| n == "4"));
    // os dados estao intactos; a pessoa volta ao passo, confere e envia de novo
    assert_eq!(cx.update(|cx| c.nova.read(cx).dados.itens.len()), 1);
    esperar_a_conferencia_depois_da_rejeicao(cx, &c);
    clicar(cx, &c, "nfe-enviar");
    let d = esperar_o_desfecho(cx, &c);
    assert!(
        matches!(&d, DesfechoNfeEmissao::Autorizada { numero, .. } if numero == "4"),
        "{d:?}"
    );
    assert_eq!(
        c.transporte.chamadas_de(AUT),
        2,
        "uma recusada e uma autorizada"
    );
    let autorizadas = notas_da_pasta(&c)
        .into_iter()
        .filter(|(n, s)| n == "4" && *s == Situacao::Autorizada)
        .count();
    assert_eq!(autorizadas, 1);
}

fn esperar_a_conferencia_depois_da_rejeicao(cx: &mut TestAppContext, c: &Cenario) {
    // a rejeicao leva ao passo do problema; a pessoa segue ate o Conferir
    for _ in 0..5 {
        if passo(cx, c) == PassoNfe::Conferir {
            break;
        }
        clicar(cx, c, "nfe-continuar");
    }
    assert_eq!(passo(cx, c), PassoNfe::Conferir);
    esperar_a_conferencia(cx, c);
}

// ---------------------------------------------------------------- sem resposta e o Retomar

#[gpui_kit::test]
fn sem_resposta_deixa_a_nota_pendente_e_o_retomar_acha_a_mesma_chave_sem_nota_gemea(
    cx: &mut TestAppContext,
) {
    let c = cenario(cx, "homologacao", RoteiroEmissao::SemResposta, Some(false));
    ate_o_conferir_com_produto_do_cadastro(cx, &c);
    clicar(cx, &c, "nfe-enviar");
    let d = esperar_o_desfecho(cx, &c);
    let chave = match &d {
        DesfechoNfeEmissao::SemResposta { chave, numero, .. } => {
            assert_eq!(numero, "3", "o ensaio sem a pendente usa 1 e 2");
            chave.clone()
        }
        outro => panic!("{outro:?}"),
    };
    // a nota ficou PENDENTE na pasta, com a chave que a tela mostra
    assert!(notas_da_pasta(&c).contains(&("3".into(), Situacao::Pendente)));
    // o texto manda retomar e nao emitir outra; nao ha "emitir de novo"
    assert!(existe(cx, &c, "nfe-retomar"));
    assert!(!existe(cx, &c, "nfe-enviar"));
    assert!(existe(cx, &c, "nfe-ver-na-lista"));
    // a retomada so' consulta a nota depois de 1 segundo na pasta (o minimo da biblioteca)
    std::thread::sleep(Duration::from_millis(1300));
    clicar(cx, &c, "nfe-retomar");
    let nova = c.nova.clone();
    esperar_ate(cx, "a nota autorizada pela retomada", move |cx| {
        cx.update(|cx| {
            matches!(
                &nova.read(cx).fase,
                Fase::Desfecho(d) if matches!(**d, DesfechoNfeEmissao::Autorizada { .. })
            )
        })
    });
    match desfecho(cx, &c).unwrap() {
        DesfechoNfeEmissao::Autorizada {
            chave: c2,
            recuperada,
            ..
        } => {
            assert_eq!(c2, chave, "é a MESMA nota");
            assert!(recuperada);
        }
        outro => panic!("{outro:?}"),
    }
    // um unico envio de autorizacao: nenhuma nota gemea
    assert_eq!(c.transporte.chamadas_de(AUT), 1);
    let na_pasta = notas_da_pasta(&c);
    assert_eq!(na_pasta.iter().filter(|(n, _)| n == "3").count(), 1);
    assert!(na_pasta.contains(&("3".into(), Situacao::Autorizada)));
    assert!(!na_pasta.iter().any(|(n, _)| n == "4"));
}

// ---------------------------------------------------------------- SEFAZ fora e a contingencia EPEC

#[gpui_kit::test]
fn sefaz_fora_oferece_a_contingencia_e_o_epec_registra_a_nota(cx: &mut TestAppContext) {
    let c = cenario(cx, "homologacao", RoteiroEmissao::Fora, Some(false));
    ate_o_conferir_com_produto_do_cadastro(cx, &c);
    // antes de verificar, a contingencia nao aparece
    assert!(!cx.update(|cx| c.nova.read(cx).epec_aberto));
    clicar(cx, &c, "nfe-verificar-sefaz");
    let nova = c.nova.clone();
    esperar_ate(cx, "a resposta da SEFAZ", move |cx| {
        cx.update(|cx| nova.read(cx).status.is_some())
    });
    assert!(cx.update(|cx| c.nova.read(cx).epec_aberto));
    assert_eq!(c.transporte.chamadas_de(STA), 1);
    assert_eq!(c.transporte.chamadas_de(AUT), 0, "verificar não emite nada");
    // o botao so' habilita com o motivo de 15 a 255 letras
    assert!(!cx.update(|cx| c.nova.read(cx).pode_epec(cx)));
    digitar(cx, &c, "nfe-motivo-da-contingencia", "curto");
    assert!(!cx.update(|cx| c.nova.read(cx).pode_epec(cx)));
    digitar(
        cx,
        &c,
        "nfe-motivo-da-contingencia",
        "SEFAZ da UF fora do ar desde as nove horas",
    );
    assert!(cx.update(|cx| c.nova.read(cx).pode_epec(cx)));
    clicar(cx, &c, "nfe-emitir-epec");
    let d = esperar_o_desfecho(cx, &c);
    match &d {
        DesfechoNfeEmissao::EpecRegistrado {
            numero, protocolo, ..
        } => {
            assert_eq!(numero, "3");
            assert!(!protocolo.is_empty());
        }
        outro => panic!("{outro:?}"),
    }
    assert_eq!(c.transporte.chamadas_de(EVT), 1);
    assert_eq!(
        c.transporte.chamadas_de(AUT),
        0,
        "a nota não foi enviada a SEFAZ da UF"
    );
    assert!(notas_da_pasta(&c).contains(&("3".into(), Situacao::Epec)));
    // a tela diz o que fazer depois: transmitir a mesma nota
}

#[gpui_kit::test]
fn com_nota_sem_resposta_a_contingencia_nao_e_oferecida_e_nada_sai(cx: &mut TestAppContext) {
    // a pasta do ensaio tem uma NF-e sem resposta: o EPEC seria OUTRA nota para a mesma venda
    let c = cenario(cx, "homologacao", RoteiroEmissao::Fora, Some(true));
    ate_o_conferir_com_produto_do_cadastro(cx, &c);
    let nova = c.nova.clone();
    esperar_ate(cx, "as pendentes lidas", move |cx| {
        cx.update(|cx| nova.read(cx).pendentes == 1)
    });
    clicar(cx, &c, "nfe-verificar-sefaz");
    let nova = c.nova.clone();
    esperar_ate(cx, "a resposta da SEFAZ", move |cx| {
        cx.update(|cx| nova.read(cx).status.is_some())
    });
    assert!(!cx.update(|cx| c.nova.read(cx).pode_epec(cx)));
    assert!(!existe(cx, &c, "nfe-emitir-epec"));
    // nem chamando o botao por baixo dos panos
    cx.update_window(c.janela.into(), |_, window, cx| {
        c.nova
            .update(cx, |n, cx| n.emitir_em_contingencia(window, cx));
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(c.transporte.chamadas_de(EVT), 0);
    assert_eq!(c.transporte.chamadas_de(AUT), 0);
    assert!(matches!(fase(cx, &c), Fase::Editando));
}

// ---------------------------------------------------------------- producao

#[gpui_kit::test]
fn em_producao_sem_a_marcacao_nao_ha_nenhuma_chamada(cx: &mut TestAppContext) {
    let c = cenario(cx, "producao", RoteiroEmissao::Autorizada, Some(false));
    ate_o_conferir_com_produto_do_cadastro(cx, &c);
    assert!(existe(cx, &c, "nfe-entendo-efeito-fiscal"));
    assert!(!cx.update(|cx| c.nova.read(cx).pode_enviar()));
    // clicar no botao desabilitado e chamar o envio por baixo dos panos: nada sai
    clicar(cx, &c, "nfe-enviar");
    cx.update_window(c.janela.into(), |_, window, cx| {
        c.nova.update(cx, |n, cx| n.enviar(window, cx));
    })
    .unwrap();
    cx.run_until_parked();
    assert!(matches!(fase(cx, &c), Fase::Editando));
    assert!(
        c.transporte.chamadas().is_empty(),
        "{:?}",
        c.transporte.chamadas()
    );
    // com a marcacao, o envio acontece (a SEFAZ aqui e' a de mentira)
    clicar(cx, &c, "nfe-entendo-efeito-fiscal");
    assert!(cx.update(|cx| c.nova.read(cx).pode_enviar()));
    clicar(cx, &c, "nfe-enviar");
    let d = esperar_o_desfecho(cx, &c);
    assert!(matches!(d, DesfechoNfeEmissao::Autorizada { .. }), "{d:?}");
    assert_eq!(c.transporte.chamadas_de(AUT), 1);
}

#[gpui_kit::test]
fn clique_duplo_no_enviar_envia_uma_vez(cx: &mut TestAppContext) {
    let c = cenario(cx, "homologacao", RoteiroEmissao::Autorizada, Some(false));
    ate_o_conferir_com_produto_do_cadastro(cx, &c);
    cx.update_window(c.janela.into(), |_, window, cx| {
        c.nova.update(cx, |n, cx| {
            n.enviar(window, cx);
            n.enviar(window, cx);
        });
    })
    .unwrap();
    let d = esperar_o_desfecho(cx, &c);
    assert!(matches!(d, DesfechoNfeEmissao::Autorizada { .. }));
    assert_eq!(c.transporte.chamadas_de(AUT), 1);
    assert_eq!(
        notas_da_pasta(&c)
            .iter()
            .filter(|(_, s)| *s == Situacao::Autorizada)
            .count(),
        2,
        "a do ensaio e a nova (a outra do ensaio esta cancelada)"
    );
}

// ---------------------------------------------------------------- nota como modelo e produtos

#[gpui_kit::test]
fn a_nota_anterior_vira_o_assistente_preenchido(cx: &mut TestAppContext) {
    let c = cenario(cx, "homologacao", RoteiroEmissao::Autorizada, Some(true));
    let chave = c.ensaio.clone().unwrap().autorizada;
    cx.update_window(c.janela.into(), |_, window, cx| {
        c.raiz.update(cx, |r, cx| {
            r.usar_nfe_como_modelo(chave.clone(), window, cx)
        });
    })
    .unwrap();
    let nova = c.nova.clone();
    esperar_ate(cx, "o modelo aplicado", move |cx| {
        cx.update(|cx| !nova.read(cx).dados.vazio())
    });
    let (tela, escolha, doc, itens, passo_atual, avisos) = cx.update(|cx| {
        let r = c.raiz.read(cx);
        let n = r.nova_nfe.read(cx);
        (
            r.tela,
            r.escolha_notas,
            n.dados.documento.clone(),
            n.dados.itens.clone(),
            n.passo,
            n.aviso_do_modelo.clone(),
        )
    });
    assert_eq!(tela, Tela::NovaNota);
    assert_eq!(escolha, EscolhaDeNotas::Nfe);
    assert_eq!(doc, "52998224725");
    assert_eq!(itens.len(), 1);
    assert_eq!(itens[0].descricao, "Cafe torrado 500g");
    assert_eq!(itens[0].quantidade, "2");
    assert_eq!(itens[0].valor, "15,90");
    assert_eq!(passo_atual, PassoNfe::ParaQuem);
    let _ = avisos;
    // o nome de teste da homologacao nao vem: a conferencia aponta
    let falta = cx.update(|cx| {
        let n = c.nova.read(cx);
        n.todas_as_pendencias()
    });
    assert!(
        falta.iter().any(|p| p.mensagem.contains("nome")),
        "{falta:?}"
    );
    // pedir outra nota como modelo com o assistente preenchido pergunta antes (nao substitui calado)
    let chave2 = c.ensaio.clone().unwrap().cancelada;
    cx.update_window(c.janela.into(), |_, window, cx| {
        c.raiz.update(cx, |r, cx| {
            r.usar_nfe_como_modelo(chave2.clone(), window, cx)
        });
    })
    .unwrap();
    cx.run_until_parked();
    std::thread::sleep(Duration::from_millis(200));
    cx.run_until_parked();
    let ainda = cx.update(|cx| c.nova.read(cx).dados.documento.clone());
    assert_eq!(
        ainda, "52998224725",
        "o assistente não foi substituido sem confirmar"
    );
    assert!(c.transporte.chamadas().is_empty());
}

#[gpui_kit::test]
fn a_tela_de_produtos_lista_edita_e_apaga(cx: &mut TestAppContext) {
    let c = cenario(cx, "homologacao", RoteiroEmissao::Autorizada, Some(true));
    cx.update(|cx| c.raiz.update(cx, |r, cx| r.ir_para(Tela::Produtos, cx)));
    let produtos = c.produtos.clone();
    esperar_ate(cx, "o cadastro na tela", move |cx| {
        cx.update(|cx| produtos.read(cx).total() == 3)
    });
    assert!(existe(cx, &c, "produto-editar-0"));
    clicar(cx, &c, "produto-editar-0");
    assert!(existe(cx, &c, "produto-salvar"));
    // NCM com poucos digitos: nao guarda e explica
    digitar(cx, &c, "produto-Ncm", "0901");
    clicar(cx, &c, "produto-salvar");
    assert!(cx.update(|cx| c.produtos.read(cx).erro_atual().is_some()));
    digitar(cx, &c, "produto-Ncm", "09012200");
    digitar(cx, &c, "produto-Valor", "17,50");
    clicar(cx, &c, "produto-salvar");
    let (servicos, config) = (Arc::clone(&c.servicos), c.config.clone());
    esperar_ate(cx, "o produto editado gravado", move |_| {
        servicos
            .ler_produtos(&config)
            .cadastro
            .produtos
            .iter()
            .any(|p| p.valor == "17,50" && p.ncm == "09012200")
    });
    // apagar
    let alvo = c.servicos.ler_produtos(&c.config).cadastro.produtos[1].clone();
    cx.update_window(c.janela.into(), |_, window, cx| {
        c.produtos
            .update(cx, |p, cx| p.apagar(alvo.clone(), window, cx));
    })
    .unwrap();
    let (servicos, config, codigo) = (
        Arc::clone(&c.servicos),
        c.config.clone(),
        alvo.codigo.clone(),
    );
    esperar_ate(cx, "o produto apagado", move |_| {
        !servicos
            .ler_produtos(&config)
            .cadastro
            .produtos
            .iter()
            .any(|p| p.codigo == codigo)
    });
    assert_eq!(
        c.servicos.ler_produtos(&c.config).cadastro.produtos.len(),
        2
    );
    assert!(c.transporte.chamadas().is_empty());
}

#[gpui_kit::test]
fn o_rascunho_da_nfe_sobrevive_a_fechar_a_janela(cx: &mut TestAppContext) {
    let c = cenario(cx, "homologacao", RoteiroEmissao::Autorizada, Some(true));
    mostrar_a_nova_nfe(cx, &c);
    digitar(cx, &c, "nfe-Nome", "Cliente do rascunho");
    // o rascunho espera uns instantes depois da ultima mudanca (o relogio do teste e' do executor)
    cx.executor().advance_clock(Duration::from_millis(900));
    cx.run_until_parked();
    let (servicos, config) = (Arc::clone(&c.servicos), c.config.clone());
    esperar_ate(cx, "o rascunho gravado", move |_| {
        servicos
            .ler_rascunho_nfe(&config)
            .is_some_and(|r| r.dados.nome == "Cliente do rascunho")
    });
    // o rascunho da NFS-e e' outro arquivo
    assert!(c.servicos.ler_rascunho(&c.config).is_none());
}
