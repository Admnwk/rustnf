// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A emissao com o emissor de demonstracao e uma pasta temporaria: nenhuma chamada a Sefin, nenhum cofre real.

use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use rustnf_app::cofre::{Cofre, CofreEmMemoria};
use rustnf_app::config::Config;
use rustnf_app::notas::Notas;
use rustnf_app::segredo::Segredo;

use super::*;
use crate::demonstracao::{Desfecho, EmissorDemonstracao, FabricaDemonstracao};
use crate::modelo::emissao::DesfechoEmissao;
use crate::modelo::nota::CampoNota;
use crate::modelo::pedido::testes::nota_do_caso;
use crate::modelo::pedido::PedidoCongelado;
use crate::testes_comuns::PastaTemporaria;

const SENHA: &str = "senha-do-teste-123";
const CNPJ: &str = "11222333000181";

fn rodar<T>(futuro: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(futuro)
}

/// Tudo para emitir: pasta de ensaio, certificado de teste, senha no cofre em memoria e o emissor falso.
struct Ensaio {
    _pasta: PastaTemporaria,
    servicos: Servicos,
    config: Config,
    emissor: Arc<EmissorDemonstracao>,
}

fn agora_fixo() -> DateTime<Utc> {
    "2026-11-21T15:00:00Z".parse().unwrap()
}

fn ensaio(roteiro: &[Desfecho]) -> Ensaio {
    let pasta = PastaTemporaria::nova("emissao");
    let pfx = pasta.certificado("a.pfx", CNPJ, SENHA, 365);
    let mut config = Config::default();
    config.emitente.cnpj = CNPJ.into();
    config.emitente.nome = "OFICINA DE TESTE LTDA".into();
    config.emitente.inscricao_municipal = "123456".into();
    config.emitente.municipio = "3550308".into();
    config.emitente.op_simples = 3;
    config.emitente.regime_apuracao = 1;
    config.sefin.pfx = pfx.to_string_lossy().into_owned();
    config.notas.pasta = pasta.caminho().join("notas").to_string_lossy().into_owned();
    let cofre = Arc::new(CofreEmMemoria::novo());
    cofre.guardar(CNPJ, &Segredo::novo(SENHA.into())).unwrap();
    let emissor = EmissorDemonstracao::novo(2, roteiro, Duration::ZERO);
    let servicos = Servicos::com(
        Some(pasta.caminho().join("rustnf.toml")),
        Arc::new(|_| None),
        cofre,
        false,
        Arc::new(SemConexao),
        Arc::new(agora_fixo),
    )
    .com_fabrica(FabricaDemonstracao::nova(emissor.clone()));
    Ensaio {
        _pasta: pasta,
        servicos,
        config,
        emissor,
    }
}

struct SemConexao;

impl Conexao for SemConexao {
    fn testar(&self, _preparo: Preparo) -> FuturoConexao {
        Box::pin(async { (false, String::new()) })
    }
}

fn emitir_pedido(e: &Ensaio, tentativa: &PedidoCongelado) -> DesfechoEmissao {
    let preparo = e.servicos.preparar_emissao(&e.config, None).unwrap();
    rodar(emitir(
        e.servicos.fabrica(),
        preparo,
        e.config.clone(),
        tentativa.clone(),
    ))
}

fn notas(e: &Ensaio) -> Notas {
    Notas::abrir(&e.config.notas.pasta).unwrap()
}

#[test]
fn autorizada_guarda_o_xml_entra_no_indice_e_o_numero_avanca() {
    let e = ensaio(&[Desfecho::Autorizada]);
    let t = e
        .servicos
        .congelar_pedido(&e.config, &nota_do_caso())
        .unwrap();
    assert_eq!(t.numero, 1);
    assert_eq!(t.emissao, "2026-11-21T11:59:00-03:00");
    let DesfechoEmissao::Autorizada {
        numero,
        chave,
        guardada,
        aviso,
        recuperada,
        ..
    } = emitir_pedido(&e, &t)
    else {
        panic!("esperava autorizada");
    };
    assert_eq!(numero, "1");
    assert!(guardada);
    assert_eq!(aviso, None);
    assert!(!recuperada);
    let pasta = notas(&e);
    let lista = pasta.listar().unwrap();
    assert_eq!(lista.len(), 1);
    assert_eq!(lista[0].chave, chave);
    assert_eq!(lista[0].numero_dps, "1");
    assert!(pasta.ler_xml(&chave).unwrap().contains("<NFSe"));
    assert_eq!(pasta.proximo_numero("1").unwrap(), 2);
}

#[test]
fn rejeitada_nao_gasta_numero_e_marca_o_campo_que_a_sefin_apontou() {
    let e = ensaio(&[Desfecho::Rejeitada]);
    let t = e
        .servicos
        .congelar_pedido(&e.config, &nota_do_caso())
        .unwrap();
    let DesfechoEmissao::Rejeitada { mensagens } = emitir_pedido(&e, &t) else {
        panic!("esperava rejeitada");
    };
    assert_eq!(mensagens.len(), 2);
    assert_eq!(mensagens[0].codigo, "E0312");
    assert_eq!(mensagens[0].campo, Some(CampoNota::Ctribnac));
    assert_eq!(mensagens[1].campo, None);
    assert_eq!(notas(&e).proximo_numero("1").unwrap(), 1);
    assert!(notas(&e).listar().unwrap().is_empty());
    // o proximo congelamento usa o MESMO numero
    let outra = e
        .servicos
        .congelar_pedido(&e.config, &nota_do_caso())
        .unwrap();
    assert_eq!(outra.numero, 1);
}

#[test]
fn sem_resposta_e_tentar_de_novo_reenvia_o_mesmo_pedido_e_recupera_a_nota() {
    let e = ensaio(&[Desfecho::SemResposta, Desfecho::Recuperada]);
    let t = e
        .servicos
        .congelar_pedido(&e.config, &nota_do_caso())
        .unwrap();
    let primeira = emitir_pedido(&e, &t);
    assert!(primeira.deixa_pendente(), "{primeira:?}");
    assert!(notas(&e).listar().unwrap().is_empty());
    assert_eq!(
        notas(&e).proximo_numero("1").unwrap(),
        1,
        "sem resposta nao gasta numero"
    );
    // "Tentar de novo": o MESMO pedido congelado
    let DesfechoEmissao::Autorizada {
        recuperada,
        guardada,
        ..
    } = emitir_pedido(&e, &t)
    else {
        panic!("esperava autorizada");
    };
    assert!(recuperada);
    assert!(guardada);
    let chamadas = e.emissor.chamadas();
    assert_eq!(chamadas.len(), 2);
    assert_eq!(chamadas[0], chamadas[1], "o reenvio tem de ser identico");
    assert_eq!(chamadas[0].numero, "1");
    assert_eq!(
        chamadas[0].emissao.as_deref(),
        Some("2026-11-21T11:59:00-03:00")
    );
    assert_eq!(notas(&e).proximo_numero("1").unwrap(), 2);
}

#[test]
fn a_segunda_emissao_usa_o_numero_seguinte() {
    let e = ensaio(&[Desfecho::Autorizada]);
    for esperado in 1..=2u64 {
        let t = e
            .servicos
            .congelar_pedido(&e.config, &nota_do_caso())
            .unwrap();
        assert_eq!(t.numero, esperado);
        let d = emitir_pedido(&e, &t);
        assert!(matches!(d, DesfechoEmissao::Autorizada { .. }), "{d:?}");
    }
    let chamadas = e.emissor.chamadas();
    assert_eq!(chamadas[0].numero, "1");
    assert_eq!(chamadas[1].numero, "2");
    assert_eq!(notas(&e).listar().unwrap().len(), 2);
}

#[test]
fn sem_senha_guardada_pede_a_senha_e_a_digitada_vale_so_para_a_emissao() {
    let e = ensaio(&[Desfecho::Autorizada]);
    // apaga a senha do cofre: a emissao precisa da digitada
    e.servicos.apagar_senha(&e.config).unwrap();
    assert!(matches!(
        e.servicos.preparar_emissao(&e.config, None),
        Err(ErroTela::SemSenha)
    ));
    assert!(matches!(
        e.servicos
            .preparar_emissao(&e.config, Some(&Segredo::novo("errada".into()))),
        Err(ErroTela::SenhaIncorreta)
    ));
    assert!(e
        .servicos
        .preparar_emissao(&e.config, Some(&Segredo::novo(SENHA.into())))
        .is_ok());
    // e nao ficou guardada
    assert_eq!(e.servicos.senha_guardada(&e.config), Some(false));
}

#[test]
fn recusa_da_biblioteca_nao_envia_nada() {
    let e = ensaio(&[Desfecho::Autorizada]);
    let mut dados = nota_do_caso();
    dados.definir(CampoNota::Competencia, "21/12/2099");
    let t = e.servicos.congelar_pedido(&e.config, &dados).unwrap();
    let DesfechoEmissao::Recusada { pendencia } = emitir_pedido(&e, &t) else {
        panic!("esperava recusada");
    };
    assert_eq!(pendencia.campo, Some(CampoNota::Competencia));
    assert_eq!(e.emissor.total_de_chamadas(), 0);
}

#[test]
fn rascunho_ida_e_volta_na_pasta_de_notas_sem_confundir_a_lista() {
    let e = ensaio(&[Desfecho::Autorizada]);
    assert_eq!(e.servicos.ler_rascunho(&e.config), None);
    let dados = nota_do_caso();
    e.servicos.gravar_rascunho(&e.config, &dados).unwrap();
    assert_eq!(e.servicos.ler_rascunho(&e.config), Some(dados));
    // a pasta de notas continua legivel e o rascunho nao entra na lista
    assert!(notas(&e).listar().unwrap().is_empty());
    assert_eq!(notas(&e).proximo_numero("1").unwrap(), 1);
    // arquivo ilegivel ou de outra versao: ignorado, sem erro
    let arquivo =
        std::path::Path::new(&e.config.notas.pasta).join(crate::modelo::rascunho::ARQUIVO);
    std::fs::write(&arquivo, b"{ nao e json").unwrap();
    assert_eq!(e.servicos.ler_rascunho(&e.config), None);
    std::fs::write(&arquivo, b"{\"versao\":99,\"dados\":{}}").unwrap();
    assert_eq!(e.servicos.ler_rascunho(&e.config), None);
    e.servicos
        .gravar_rascunho(&e.config, &nota_do_caso())
        .unwrap();
    e.servicos.apagar_rascunho(&e.config);
    assert!(!arquivo.exists());
    // apagar o que nao existe nao e' erro
    e.servicos.apagar_rascunho(&e.config);
    // sem pasta configurada nao ha rascunho
    let mut sem = e.config.clone();
    sem.notas.pasta.clear();
    assert_eq!(e.servicos.ler_rascunho(&sem), None);
    assert!(e.servicos.gravar_rascunho(&sem, &nota_do_caso()).is_err());
}

#[test]
fn servicos_sem_fabrica_nao_enviam_nada_e_nao_dizem_que_e_demonstracao() {
    let pasta = PastaTemporaria::nova("sem-fabrica");
    let s = crate::testes_comuns::servicos_de_teste(
        &pasta.caminho().join("x.toml"),
        Arc::new(SemConexao),
    )
    .0;
    assert!(!s.demonstracao());
    let pfx = pasta.certificado("a.pfx", CNPJ, SENHA, 365);
    let mut config = Config::default();
    config.emitente.cnpj = CNPJ.into();
    config.sefin.pfx = pfx.to_string_lossy().into_owned();
    let preparo = s
        .preparar_emissao(&config, Some(&Segredo::novo(SENHA.into())))
        .unwrap();
    assert!(s.fabrica().criar(preparo).is_err());
}

#[test]
fn o_emissor_de_demonstracao_diz_que_e_demonstracao() {
    let e = ensaio(&[Desfecho::Autorizada]);
    assert!(e.servicos.demonstracao());
}
