// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Testes do emissor de demonstracao: a resposta autorizada e' uma NFS-e que a camada de aplicacao le, o roteiro da
//! emissao, da consulta (pela chave e pela DPS), do cancelamento e dos eventos.

use rustnf::nfse::sefin::Status;
use rustnf_app::notas::Registro;

use super::*;

fn rodar<T>(futuro: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(futuro)
}

fn dps() -> Dps {
    serde_json::from_value(serde_json::json!({
        "prestador": {"documento": "11222333000181", "nome": "OFICINA DE TESTE", "im": "123456", "op_simples": 1},
        "cmun_emissor": "3550308",
        "numero": 7,
        "serie": 1,
        "emissao": "2026-11-21T10:00:00-03:00",
        "competencia": "2026-11-21",
        "tomador": {"documento": "52998224725", "nome": "Maria & Filhos <ME>"},
        "servico": {"ctribnac": "140101", "descricao": "Revisão \"geral\""},
        "valores": {"servico": "100.50"}
    }))
    .unwrap()
}

#[test]
fn a_resposta_autorizada_e_uma_nfse_que_a_camada_de_aplicacao_le() {
    rodar(async {
        let e = EmissorDemonstracao::novo(2, &[Desfecho::Autorizada], Duration::ZERO);
        let r = e.emitir(&dps()).await;
        assert_eq!(r.status, Status::Autorizada);
        assert_eq!(r.chave.len(), 50);
        chave_dfe::conferir(&r.chave).unwrap();
        let nfse = r.nfse.as_ref().expect("nfse lida");
        assert_eq!(nfse.numero, "7");
        assert_eq!(nfse.tomador_nome, "Maria & Filhos <ME>");
        assert_eq!(nfse.valor_servico, "100.50");
        let registro = Registro::da_nfse(nfse);
        assert_eq!(registro.chave, r.chave);
        assert_eq!(registro.numero_dps, "7");
        assert!(registro.emissao.starts_with("2026-11-21"));
        assert_eq!(e.total_de_chamadas(), 1);
        assert_eq!(e.chamadas()[0].numero, "7");
    });
}

#[test]
fn o_roteiro_segue_a_ordem_e_repete_o_ultimo() {
    rodar(async {
        let e = EmissorDemonstracao::novo(
            2,
            &[Desfecho::SemResposta, Desfecho::Recuperada],
            Duration::ZERO,
        );
        assert_eq!(e.emitir(&dps()).await.status, Status::Pendente);
        let r = e.emitir(&dps()).await;
        assert_eq!(r.status, Status::Autorizada);
        assert!(r.recuperada);
        assert_eq!(e.emitir(&dps()).await.status, Status::Autorizada);
        assert_eq!(e.total_de_chamadas(), 3);
    });
}

#[test]
fn rejeitada_traz_duas_mensagens_e_nenhuma_nfse() {
    rodar(async {
        let e = EmissorDemonstracao::novo(2, &[Desfecho::Rejeitada], Duration::ZERO);
        let r = e.emitir(&dps()).await;
        assert_eq!(r.status, Status::Rejeitada);
        assert_eq!(r.erros.len(), 2);
        assert!(r.nfse.is_none());
        assert!(r.xml_retorno.is_empty());
    });
}

#[test]
fn os_nomes_do_roteiro() {
    assert_eq!(
        roteiro_do_nome("autorizada"),
        Some(vec![Desfecho::Autorizada])
    );
    assert_eq!(
        roteiro_do_nome("rejeitada"),
        Some(vec![Desfecho::Rejeitada])
    );
    assert_eq!(
        roteiro_do_nome("sem-resposta"),
        Some(vec![Desfecho::SemResposta, Desfecho::Recuperada])
    );
    assert_eq!(roteiro_do_nome("outra"), None);
}

#[test]
fn os_nomes_do_roteiro_de_cancelamento() {
    assert_eq!(
        cancelamento_do_nome("registrado"),
        Some(vec![Cancelamento::Registrado])
    );
    assert_eq!(
        cancelamento_do_nome("rejeitado"),
        Some(vec![Cancelamento::Rejeitado])
    );
    assert_eq!(
        cancelamento_do_nome("sem-resposta"),
        Some(vec![Cancelamento::SemResposta, Cancelamento::Registrado])
    );
    assert_eq!(cancelamento_do_nome("outro"), None);
}

#[test]
fn consulta_devolve_a_nota_autorizada_nesta_execucao_e_nao_acha_o_resto() {
    rodar(async {
        let e = EmissorDemonstracao::novo(2, &[Desfecho::Autorizada], Duration::ZERO);
        let nota = e.emitir(&dps()).await;
        let chave = nota.chave.clone();
        // a chave pode vir com espaco e em minuscula
        let r = e.consultar(&chave.to_lowercase()).await;
        assert_eq!(r.status, Status::Autorizada);
        assert_eq!(r.chave, chave);
        assert_eq!(r.nfse.as_ref().unwrap().numero, "7");
        assert!(r.xml_retorno.contains("<NFSe"));
        let outra =
            chave_dfe::montar_nfse(3_550_308, "11222333000181", 99, "2611", Some(1), 2).unwrap();
        assert_eq!(e.consultar(&outra).await.status, Status::NaoEncontrada);
        assert_eq!(e.chaves_consultadas().len(), 2);
    });
}

#[test]
fn consulta_tambem_le_a_pasta_de_notas_do_ensaio() {
    rodar(async {
        let pasta = std::env::temp_dir().join(format!("rustnf-gui-demo-{}", std::process::id()));
        let notas = Notas::abrir(&pasta).unwrap();
        let autora = EmissorDemonstracao::novo(2, &[Desfecho::Autorizada], Duration::ZERO);
        let nota = autora.emitir(&dps()).await;
        let n = nota.nfse.as_ref().unwrap();
        notas
            .guardar(&Registro::da_nfse(n), &nota.xml_retorno)
            .unwrap();
        let e = EmissorDemonstracao::com_ensaio(
            2,
            &[Desfecho::Autorizada],
            Ensaio {
                pasta: Some(pasta.clone()),
                ..Ensaio::default()
            },
            Duration::ZERO,
        );
        assert_eq!(e.consultar(&nota.chave).await.status, Status::Autorizada);
        assert_eq!(e.cancelada(&nota.chave).await.status, Status::NaoEncontrada);
        notas.marcar_cancelada(&nota.chave).unwrap();
        assert_eq!(e.cancelada(&nota.chave).await.status, Status::Ok);
        let _ = std::fs::remove_dir_all(&pasta);
    });
}

#[test]
fn o_cancelamento_segue_o_roteiro_e_so_registra_nota_que_conhece() {
    rodar(async {
        let e = EmissorDemonstracao::com_ensaio(
            2,
            &[Desfecho::Autorizada],
            Ensaio {
                cancelamento: cancelamento_do_nome("sem-resposta").unwrap(),
                ..Ensaio::default()
            },
            Duration::ZERO,
        );
        let chave = e.emitir(&dps()).await.chave;
        let r = e.cancelar(&chave, "1", "justificativa de teste").await;
        assert_eq!(r.status, Status::Pendente);
        assert_eq!(e.cancelada(&chave).await.status, Status::NaoEncontrada);
        // o reenvio e' o mesmo pedido
        let r = e.cancelar(&chave, "1", "justificativa de teste").await;
        assert_eq!(r.status, Status::Registrado);
        assert_eq!(e.cancelada(&chave).await.status, Status::Ok);
        let pedidos = e.pedidos_de_cancelamento();
        assert_eq!(pedidos.len(), 2);
        assert_eq!(pedidos[0], pedidos[1]);
        // chave desconhecida: a demonstracao recusa
        let outra =
            chave_dfe::montar_nfse(3_550_308, "11222333000181", 98, "2611", Some(1), 2).unwrap();
        let r = e.cancelar(&outra, "1", "justificativa de teste").await;
        assert_eq!(r.status, Status::Rejeitada);
        assert_eq!(r.erros.len(), 1);
        // rejeitado: duas mensagens
        let e = EmissorDemonstracao::com_ensaio(
            2,
            &[Desfecho::Autorizada],
            Ensaio {
                cancelamento: cancelamento_do_nome("rejeitado").unwrap(),
                ..Ensaio::default()
            },
            Duration::ZERO,
        );
        let chave = e.emitir(&dps()).await.chave;
        let r = e.cancelar(&chave, "9", "justificativa de teste").await;
        assert_eq!(r.status, Status::Rejeitada);
        // a recusa por prazo do Anexo II, a que a tela responde oferecendo a analise fiscal
        assert_eq!(r.erros.len(), 1);
        assert_eq!(r.erros[0].codigo, "E0822");
        assert_eq!(e.cancelada(&chave).await.status, Status::NaoEncontrada);
    });
}

#[test]
fn os_nomes_do_roteiro_de_evento() {
    use crate::demonstracao_eventos::{evento_do_nome, RegistroDeEvento};
    assert_eq!(
        evento_do_nome("registrado"),
        Some(vec![RegistroDeEvento::Registrado])
    );
    assert_eq!(
        evento_do_nome("rejeitado"),
        Some(vec![RegistroDeEvento::Rejeitado])
    );
    assert_eq!(
        evento_do_nome("sem-resposta"),
        Some(vec![
            RegistroDeEvento::SemResposta,
            RegistroDeEvento::Registrado
        ])
    );
    assert_eq!(evento_do_nome("outro"), None);
}

#[test]
fn o_evento_registrado_volta_na_consulta_de_evento_como_a_sefin_o_devolveria() {
    use crate::demonstracao_eventos::RegistroDeEvento;
    rodar(async {
        let e = EmissorDemonstracao::com_ensaio(
            2,
            &[Desfecho::Autorizada],
            Ensaio {
                evento: vec![RegistroDeEvento::Registrado],
                recebida_para: Some("11222333000181".into()),
                ..Ensaio::default()
            },
            Duration::ZERO,
        );
        let chave = e.emitir(&dps()).await.chave;
        // antes do pedido nao ha evento
        assert_eq!(
            e.consultar_evento(&chave, "101103", 1).await.status,
            Status::NaoEncontrada
        );
        let r = e
            .registrar_evento("101103", &chave, "1", "Valor lancado errado na nota")
            .await;
        assert_eq!(r.status, Status::Registrado);
        let lido = e.consultar_evento(&chave.to_lowercase(), "101103", 1).await;
        assert_eq!(lido.status, Status::Ok);
        let ev = rustnf::nfse::leitura::ler_evento(&lido.xml_retorno).unwrap();
        assert_eq!(ev.tipo, "101103");
        assert_eq!(ev.chave, chave);
        assert_eq!(ev.sequencia, "1");
        assert_eq!(ev.motivo, "Valor lancado errado na nota");
        assert!(!ev.processado_em.is_empty() && !ev.data_evento.is_empty());
        // o segundo pedido do mesmo tipo e' a sequencia 2
        e.registrar_evento("101103", &chave, "1", "Outro pedido de analise fiscal")
            .await;
        assert_eq!(
            e.consultar_evento(&chave, "101103", 2).await.status,
            Status::Ok
        );
        assert_eq!(e.pedidos_de_evento().len(), 2);
    });
}

#[test]
fn evento_rejeitado_sem_resposta_e_nota_desconhecida() {
    use crate::demonstracao_eventos::{evento_do_nome, RegistroDeEvento};
    rodar(async {
        let e = EmissorDemonstracao::com_ensaio(
            2,
            &[Desfecho::Autorizada],
            Ensaio {
                evento: evento_do_nome("sem-resposta").unwrap(),
                ..Ensaio::default()
            },
            Duration::ZERO,
        );
        let chave = e.emitir(&dps()).await.chave;
        let r = e.registrar_evento("203202", &chave, "", "").await;
        assert_eq!(r.status, Status::Pendente);
        // sem resposta nao e' registrado
        assert_eq!(
            e.consultar_evento(&chave, "203202", 1).await.status,
            Status::NaoEncontrada
        );
        let r = e.registrar_evento("203202", &chave, "", "").await;
        assert_eq!(r.status, Status::Registrado);
        // chave que a demonstracao nao conhece
        let outra =
            chave_dfe::montar_nfse(3_550_308, "11222333000181", 97, "2611", Some(1), 2).unwrap();
        let r = e.registrar_evento("203202", &outra, "", "").await;
        assert_eq!(r.status, Status::Rejeitada);
        // rejeitado: duas mensagens
        let e = EmissorDemonstracao::com_ensaio(
            2,
            &[Desfecho::Autorizada],
            Ensaio {
                evento: vec![RegistroDeEvento::Rejeitado],
                ..Ensaio::default()
            },
            Duration::ZERO,
        );
        let chave = e.emitir(&dps()).await.chave;
        let r = e
            .registrar_evento("101103", &chave, "1", "justificativa de teste")
            .await;
        assert_eq!(r.status, Status::Rejeitada);
        assert_eq!(r.erros.len(), 2);
        assert_eq!(
            e.consultar_evento(&chave, "101103", 1).await.status,
            Status::NaoEncontrada
        );
    });
}

#[test]
fn a_nota_recebida_e_de_outro_emitente_e_tem_o_emitente_da_configuracao_como_tomador() {
    rodar(async {
        let e = EmissorDemonstracao::com_ensaio(
            2,
            &[Desfecho::Autorizada],
            Ensaio {
                recebida_para: Some("11222333000181".into()),
                ..Ensaio::default()
            },
            Duration::ZERO,
        );
        let chave = e.chave_da_nota_recebida().expect("ha nota recebida");
        chave_dfe::conferir(&chave).unwrap();
        let r = e.consultar(&chave).await;
        assert_eq!(r.status, Status::Autorizada);
        let n = r.nfse.unwrap();
        assert_eq!(n.tomador_documento, "11222333000181");
        assert_eq!(n.emitente_documento, "12345678909");
        // sem o documento da configuracao nao ha nota recebida
        let sem = EmissorDemonstracao::novo(2, &[Desfecho::Autorizada], Duration::ZERO);
        assert!(sem.chave_da_nota_recebida().is_none());
    });
}
