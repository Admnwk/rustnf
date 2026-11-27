// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! As NF-e recebidas fora da tela, com a distribuicao de mentira e uma pasta temporaria: buscar notas novas (e a espera que a
//! biblioteca impoe), baixar a nota completa, responder a nota, o DANFE e a leitura da pasta. Nenhuma chamada ao Ambiente
//! Nacional, nenhum cofre real.

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use rustnf_app::cofre::{Cofre, CofreEmMemoria};
use rustnf_app::config::Config;
use rustnf_app::notas::nfe::SituacaoRecebida;
use rustnf_app::segredo::Segredo;

use super::*;
use crate::demonstracao_nfe::{FabricaNfeDemonstracao, RoteiroNfe, TransporteNfeDemonstracao};
use crate::demonstracao_recebidas::{semear_recebidas, RecebidasDoEnsaio, RoteiroRecebidas, DIST};
use crate::modelo::nfe_detalhe::LeituraNfe;
use crate::modelo::nfe_eventos::DesfechoNfe;
use crate::modelo::nfe_recebidas::RespostaDada;
use crate::modelo::nfe_recebidas_manifestar::Resposta;
use crate::modelo::nfe_recebidas_resultados::{DesfechoBaixa, DesfechoBusca};
use crate::testes_comuns::PastaTemporaria;

const SENHA: &str = "senha-do-teste-123";
const CNPJ: &str = "11222333000181";
const EVT: &str = "NFeRecepcaoEvento4";

fn rodar<T>(futuro: impl Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(futuro)
}

struct Ensaio {
    _pasta: PastaTemporaria,
    servicos: Servicos,
    config: Config,
    transporte: TransporteNfeDemonstracao,
    chaves: RecebidasDoEnsaio,
}

fn ensaio(recebidas: RoteiroRecebidas, nfe: RoteiroNfe) -> Ensaio {
    let pasta = PastaTemporaria::nova("servico-recebidas");
    let pfx = pasta.certificado("a.pfx", CNPJ, SENHA, 365);
    let texto = format!(
        "[emitente]\ncnpj = \"{CNPJ}\"\nnome = \"LOJA DE TESTE LTDA\"\ninscricao_municipal = \"123456\"\nmunicipio = \"3550308\"\nop_simples = 3\nregime_apuracao = 1\n\
         [sefin]\npfx = '{}'\n[notas]\npasta = '{}'\n\
         [nfe]\nambiente = \"homologacao\"\nuf = \"SP\"\nie = \"111111111111\"\ncrt = 1\nserie = \"1\"\n\
         [nfe.endereco]\nlogradouro = \"Rua das Flores\"\nnumero = \"100\"\nbairro = \"Centro\"\nmunicipio = \"Sao Paulo\"\ncep = \"01001000\"\n",
        pfx.to_string_lossy(),
        pasta.caminho().join("notas").to_string_lossy()
    );
    let config = Config::de_texto(&texto).unwrap();
    let transporte = TransporteNfeDemonstracao::novo(nfe, Duration::ZERO).com_recebidas(recebidas);
    let chaves = semear_recebidas(&config, &transporte).unwrap();
    let cofre = Arc::new(CofreEmMemoria::novo());
    cofre.guardar(CNPJ, &Segredo::novo(SENHA.into())).unwrap();
    let cofre_dyn: Arc<dyn Cofre> = cofre;
    let servicos = Servicos::com(
        Some(pasta.caminho().join("rustnf.toml")),
        Arc::new(|_| None),
        cofre_dyn,
        false,
        Arc::new(SemConexao),
        Arc::new(chrono::Utc::now),
    )
    .com_fabrica_nfe(FabricaNfeDemonstracao::nova(transporte.clone()));
    Ensaio {
        _pasta: pasta,
        servicos,
        config,
        transporte,
        chaves,
    }
}

struct SemConexao;

impl Conexao for SemConexao {
    fn testar(&self, _preparo: Preparo) -> FuturoConexao {
        Box::pin(async { (false, String::new()) })
    }
}

fn buscar(e: &Ensaio) -> DesfechoBusca {
    let preparo = e.servicos.preparar_recebidas(&e.config, None).unwrap();
    rodar(buscar_notas(
        e.servicos.fabrica_nfe(),
        preparo,
        e.config.clone(),
    ))
}

fn baixar(e: &Ensaio, chave: &str, ciencia: bool) -> DesfechoBaixa {
    let preparo = e.servicos.preparar_recebidas(&e.config, None).unwrap();
    rodar(baixar_nota(
        e.servicos.fabrica_nfe(),
        preparo,
        e.config.clone(),
        chave.to_string(),
        ciencia,
    ))
}

fn responder(e: &Ensaio, chave: &str, r: Resposta, justificativa: Option<&str>) -> DesfechoNfe {
    let preparo = e.servicos.preparar_recebidas(&e.config, None).unwrap();
    rodar(responder_nota(
        e.servicos.fabrica_nfe(),
        preparo,
        e.config.clone(),
        chave.to_string(),
        r,
        justificativa.map(str::to_string),
    ))
}

fn situacao(e: &Ensaio, chave: &str) -> SituacaoRecebida {
    e.servicos
        .listar_recebidas(&e.config)
        .unwrap()
        .into_iter()
        .find(|r| r.chave == chave)
        .unwrap()
        .situacao
}

#[test]
fn a_semente_poe_tres_notas_na_pasta_e_nada_sai_para_a_rede_depois() {
    let e = ensaio(RoteiroRecebidas::Normal, RoteiroNfe::Registrado);
    let lista = e.servicos.listar_recebidas(&e.config).unwrap();
    assert_eq!(lista.len(), 3);
    assert_eq!(situacao(&e, &e.chaves.resumo), SituacaoRecebida::Resumo);
    assert_eq!(situacao(&e, &e.chaves.completa), SituacaoRecebida::Ciencia);
    assert_eq!(
        situacao(&e, &e.chaves.cancelada),
        SituacaoRecebida::Cancelada
    );
    assert!(
        e.transporte.chamadas().is_empty(),
        "o que se contou semeando nao e' do uso"
    );
    // a semente nao deixa espera: a primeira busca da janela sai
    let fila = e.servicos.fila_das_recebidas(&e.config).unwrap();
    assert!(fila.espera_ate.is_none(), "{fila:?}");
    assert!(fila.ultima_busca.is_some());
}

#[test]
fn buscar_traz_tres_notas_novas_e_a_segunda_busca_espera_sem_ir_a_rede() {
    let e = ensaio(RoteiroRecebidas::Normal, RoteiroNfe::Registrado);
    let d = buscar(&e);
    let DesfechoBusca::Novas {
        novas,
        mais_na_fila,
        proxima,
        ..
    } = &d
    else {
        panic!("esperava notas novas: {d:?}");
    };
    assert_eq!((*novas, *mais_na_fila), (3, false));
    assert!(proxima.is_some(), "a fila em dia manda esperar");
    assert_eq!(d.titulo(), "3 notas novas");
    assert_eq!(e.servicos.listar_recebidas(&e.config).unwrap().len(), 6);
    let chamadas = e.transporte.chamadas_de(DIST);
    assert!(chamadas >= 1);
    // a espera esta na pasta e a biblioteca a respeita: nenhuma chamada nova
    let fila = e.servicos.fila_das_recebidas(&e.config).unwrap();
    assert!(fila.espera_ate.is_some());
    let de_novo = buscar(&e);
    assert!(
        matches!(de_novo, DesfechoBusca::Espera { .. }),
        "{de_novo:?}"
    );
    assert_eq!(
        e.transporte.chamadas_de(DIST),
        chamadas,
        "ZERO chamadas antes da hora"
    );
}

#[test]
fn o_roteiro_espera_bloqueia_com_o_horario_e_a_proxima_busca_nem_sai() {
    let e = ensaio(RoteiroRecebidas::Espera, RoteiroNfe::Registrado);
    let d = buscar(&e);
    assert!(
        matches!(d, DesfechoBusca::Bloqueada { ate: Some(_), .. }),
        "{d:?}"
    );
    assert!(d.texto().contains("depois das"), "{}", d.texto());
    let chamadas = e.transporte.chamadas_de(DIST);
    assert!(e
        .servicos
        .fila_das_recebidas(&e.config)
        .unwrap()
        .espera_ate
        .is_some());
    assert!(matches!(buscar(&e), DesfechoBusca::Espera { .. }));
    assert_eq!(e.transporte.chamadas_de(DIST), chamadas);
    assert_eq!(
        e.servicos.listar_recebidas(&e.config).unwrap().len(),
        3,
        "nada chegou"
    );
}

#[test]
fn o_roteiro_vazio_diz_que_nao_ha_nada_novo() {
    let e = ensaio(RoteiroRecebidas::Vazio, RoteiroNfe::Registrado);
    let d = buscar(&e);
    assert!(matches!(d, DesfechoBusca::NadaNovo { .. }), "{d:?}");
    assert_eq!(d.titulo(), "Nada novo");
    assert_eq!(e.servicos.listar_recebidas(&e.config).unwrap().len(), 3);
}

#[test]
fn baixar_sem_ciencia_so_traz_o_resumo_e_nada_e_registrado() {
    let e = ensaio(RoteiroRecebidas::Normal, RoteiroNfe::Registrado);
    let d = baixar(&e, &e.chaves.resumo, false);
    assert!(matches!(d, DesfechoBaixa::SoResumo { .. }), "{d:?}");
    assert_eq!(
        e.transporte.chamadas_de(EVT),
        0,
        "sem a ciencia nenhum evento sai"
    );
    assert_eq!(situacao(&e, &e.chaves.resumo), SituacaoRecebida::Resumo);
}

#[test]
fn baixar_com_ciencia_registra_o_evento_e_traz_a_nota_completa_e_o_danfe() {
    let e = ensaio(RoteiroRecebidas::Normal, RoteiroNfe::Registrado);
    let d = baixar(&e, &e.chaves.resumo, true);
    assert_eq!(
        d,
        DesfechoBaixa::Completa {
            ciencia_registrada: true
        }
    );
    assert_eq!(e.transporte.chamadas_de(EVT), 1);
    assert_eq!(situacao(&e, &e.chaves.resumo), SituacaoRecebida::Ciencia);
    let lida = e
        .servicos
        .ler_recebida(&e.config, &e.chaves.resumo)
        .unwrap();
    let LeituraNfe::Lida(detalhe) = &lida.leitura else {
        panic!("a nota completa devia estar legivel");
    };
    assert_eq!(detalhe.total, "1.250,00");
    assert!(!detalhe.itens.is_empty());
    let pdf = e
        .servicos
        .danfe_da_recebida(&e.config, &e.chaves.resumo)
        .unwrap();
    assert!(pdf.is_file());
    assert!(std::fs::read(&pdf).unwrap().starts_with(b"%PDF"));
    assert!(e
        .servicos
        .caminho_do_xml_recebida(&e.config, &e.chaves.resumo)
        .is_ok());
}

#[test]
fn o_resumo_nao_tem_danfe_nem_xml_para_mostrar_e_a_leitura_so_traz_o_indice() {
    let e = ensaio(RoteiroRecebidas::Normal, RoteiroNfe::Registrado);
    let lida = e
        .servicos
        .ler_recebida(&e.config, &e.chaves.resumo)
        .unwrap();
    assert!(matches!(lida.leitura, LeituraNfe::Ausente(_)));
    assert!(matches!(
        e.servicos.danfe_da_recebida(&e.config, &e.chaves.resumo),
        Err(ErroTela::Danfse(_))
    ));
    assert_eq!(
        e.servicos
            .caminho_do_xml_recebida(&e.config, &e.chaves.resumo),
        Err(ErroTela::XmlAusente)
    );
    assert_eq!(
        e.servicos.ler_recebida(&e.config, &"9".repeat(44)),
        Err(ErroTela::NotaAusente)
    );
}

#[test]
fn as_quatro_respostas_registram_e_a_lista_acompanha() {
    for (resposta, esperada, dada) in [
        (
            Resposta::Confirmar,
            SituacaoRecebida::Confirmada,
            RespostaDada::Confirmada,
        ),
        (
            Resposta::Ciencia,
            SituacaoRecebida::Ciencia,
            RespostaDada::Ciencia,
        ),
        (
            Resposta::Desconhecer,
            SituacaoRecebida::Desconhecida,
            RespostaDada::Desconhecida,
        ),
        (
            Resposta::NaoRealizada,
            SituacaoRecebida::NaoRealizada,
            RespostaDada::NaoRealizada,
        ),
    ] {
        let e = ensaio(RoteiroRecebidas::Normal, RoteiroNfe::Registrado);
        let justificativa = resposta
            .pede_justificativa()
            .then_some("A mercadoria nao chegou ao deposito");
        let d = responder(&e, &e.chaves.resumo, resposta, justificativa);
        assert!(d.registrado(), "{resposta:?}: {d:?}");
        assert_eq!(situacao(&e, &e.chaves.resumo), esperada, "{resposta:?}");
        assert_eq!(e.transporte.chamadas_de(EVT), 1);
        let linha = crate::modelo::nfe_recebidas::LinhaRecebida::do_registro(
            &e.servicos
                .listar_recebidas(&e.config)
                .unwrap()
                .into_iter()
                .find(|r| r.chave == e.chaves.resumo)
                .unwrap(),
        );
        assert_eq!(linha.resposta, dada);
    }
}

#[test]
fn o_que_a_aplicacao_recusa_nem_sai_da_maquina() {
    let e = ensaio(RoteiroRecebidas::Normal, RoteiroNfe::Registrado);
    // nao realizada sem justificativa; ciencia de nota cancelada (650)
    let sem = responder(&e, &e.chaves.resumo, Resposta::NaoRealizada, None);
    assert!(matches!(sem, DesfechoNfe::Recusado { .. }), "{sem:?}");
    let cancelada = responder(&e, &e.chaves.cancelada, Resposta::Ciencia, None);
    assert!(
        matches!(cancelada, DesfechoNfe::Recusado { .. }),
        "{cancelada:?}"
    );
    // a mesma ciencia duas vezes (573): a segunda e' recusada antes
    let repetida = responder(&e, &e.chaves.completa, Resposta::Ciencia, None);
    assert!(
        matches!(repetida, DesfechoNfe::Recusado { .. }),
        "{repetida:?}"
    );
    assert_eq!(
        e.transporte.chamadas_de(EVT),
        0,
        "nenhuma das tres foi enviada"
    );
}

#[test]
fn a_recusa_do_ambiente_nacional_nao_muda_a_pasta_e_a_orientacao_nao_inventa_prazo() {
    let e = ensaio(RoteiroRecebidas::Normal, RoteiroNfe::Rejeitado);
    let d = responder(&e, &e.chaves.resumo, Resposta::Confirmar, None);
    let DesfechoNfe::Rejeitado {
        codigo, orientacao, ..
    } = &d
    else {
        panic!("esperava recusa: {d:?}");
    };
    assert_eq!(codigo, "596");
    assert!(orientacao.contains("não o calcula"), "{orientacao}");
    assert_eq!(situacao(&e, &e.chaves.resumo), SituacaoRecebida::Resumo);
}

#[test]
fn sem_resposta_na_primeira_vez_e_a_segunda_registra_a_mesma_resposta() {
    let e = ensaio(RoteiroRecebidas::Normal, RoteiroNfe::SemResposta);
    let primeira = responder(&e, &e.chaves.resumo, Resposta::Confirmar, None);
    assert!(primeira.deixa_em_aberto(), "{primeira:?}");
    assert_eq!(situacao(&e, &e.chaves.resumo), SituacaoRecebida::Resumo);
    let segunda = responder(&e, &e.chaves.resumo, Resposta::Confirmar, None);
    assert!(segunda.registrado(), "{segunda:?}");
    assert_eq!(situacao(&e, &e.chaves.resumo), SituacaoRecebida::Confirmada);
}
