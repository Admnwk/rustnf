// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Cancelamento, carta de correcao, inutilizacao e EPEC: um teste por desfecho e as regras de numeracao da carta.

use super::testes_comuns::*;
use super::{cancelar, corrigir, emitir, epec, inutilizar};
use crate::erro::Erro;
use crate::notas::nfe::{EventoGuardado, Situacao};

const NPROT_EVT: &str = "135260000000077";

/// Uma pasta com a nota 1 autorizada, mais o roteiro seguinte.
async fn com_nota(rotulo: &str, depois: Vec<(&'static str, Resp)>) -> (Cenario, String) {
    let c = Cenario::novo(
        rotulo,
        vec![(AUT, envio("100", "Autorizado o uso da NF-e", NPROT))],
    );
    let e = emitir(&c.ctx(), pedido(), true).await.unwrap();
    c.roteiro(depois);
    (c, e.chave)
}

#[tokio::test]
async fn cancelar_guarda_o_evento_marca_cancelada_e_usa_o_protocolo_da_pasta() {
    let (c, chave) = com_nota(
        "nfe-cancela",
        vec![(
            EVT,
            evento("135", "Evento registrado e vinculado a NF-e", NPROT_EVT),
        )],
    )
    .await;
    // a chave colada com pontuacao serve
    let colada = format!("{}.{}", &chave[..4], &chave[4..]);
    let r = cancelar(&c.ctx(), &colada, "Cancelamento por erro no pedido", None)
        .await
        .unwrap();
    assert!(r.erro().is_none(), "{:?}", r.resultado.frase);
    assert!(r.guardado);
    assert_eq!(r.situacao, Some(Situacao::Cancelada));
    let j = r.para_json();
    assert_eq!(
        (
            j["status"].as_str(),
            j["tipo"].as_str(),
            j["protocolo"].as_str()
        ),
        (Some("registrado"), Some("110111"), Some(NPROT_EVT))
    );
    // o nProt da autorizacao foi para a SEFAZ
    assert!(c
        .falso
        .corpos()
        .last()
        .unwrap()
        .contains(&format!("<nProt>{NPROT}</nProt>")));
    let dir = c.pasta.caminho().join("notas/nfe/2026/10");
    assert!(dir
        .join(format!("{chave}-110111-1-procEventoNFe.xml"))
        .is_file());
    let reg = c.notas.registro(&chave).unwrap();
    assert_eq!(reg.situacao, Situacao::Cancelada);
    assert_eq!(reg.eventos.len(), 1);
}

#[tokio::test]
async fn cancelar_so_nota_autorizada_e_a_de_fora_da_pasta_exige_o_protocolo() {
    let c = Cenario::novo("nfe-cancela-recusa", vec![(AUT, tempo()), (CON, tempo())]);
    let pendente = emitir(&c.ctx(), pedido(), true).await.unwrap();
    let erro = cancelar(&c.ctx(), &pendente.chave, "Cancelamento de teste", None)
        .await
        .unwrap_err();
    assert_eq!(erro.codigo_saida(), 3);
    assert!(
        erro.to_string().contains("so' se cancela nota autorizada"),
        "{erro}"
    );

    // a nota que nao esta na pasta: sem o protocolo, erro de uso; com ele, vai e nao guarda nada
    let outra = chave_de_outra_pasta();
    let erro = cancelar(&c.ctx(), &outra, "Cancelamento de teste", None)
        .await
        .unwrap_err();
    assert!(
        matches!(erro, Erro::Uso(ref m) if m.contains("nProt")),
        "{erro}"
    );
    c.roteiro(vec![(
        EVT,
        evento("135", "Evento registrado e vinculado a NF-e", NPROT_EVT),
    )]);
    let r = cancelar(&c.ctx(), &outra, "Cancelamento de teste", Some(NPROT))
        .await
        .unwrap();
    assert!(r.resultado.registrado());
    assert!(!r.guardado);
    assert_eq!(r.situacao, None);
}

fn chave_de_outra_pasta() -> String {
    use rustnf::chave_dfe::{montar, Montar};
    montar(&Montar {
        cuf: 35,
        aamm: "2610",
        emitente: "11222333000181",
        modelo: 55,
        serie: 1,
        numero: 900,
        tp_emis: 1,
        codigo: Some(12_345_670),
    })
    .unwrap()
}

#[tokio::test]
async fn cancelamento_fora_do_prazo_e_recusa_e_nao_marca_a_nota() {
    let (c, chave) = com_nota(
        "nfe-cancela-501",
        vec![(
            EVT,
            evento(
                "501",
                "Rejeicao: Prazo de Cancelamento superior ao previsto na Legislacao",
                "",
            ),
        )],
    )
    .await;
    let r = cancelar(&c.ctx(), &chave, "Cancelamento depois do prazo", None)
        .await
        .unwrap();
    assert_eq!(r.para_json()["status"], "prazo_vencido");
    assert_eq!(r.erro().unwrap().codigo_saida(), 3);
    assert!(!r.guardado);
    assert_eq!(
        c.notas.registro(&chave).unwrap().situacao,
        Situacao::Autorizada
    );
}

#[tokio::test]
async fn cancelar_sem_resposta_manda_consultar_e_nao_marca() {
    let (c, chave) = com_nota("nfe-cancela-sem", vec![(EVT, tempo()), (CON, tempo())]).await;
    let r = cancelar(&c.ctx(), &chave, "Cancelamento sem resposta", None)
        .await
        .unwrap();
    assert_eq!(r.para_json()["status"], "sem_resposta");
    assert_eq!(r.erro().unwrap().codigo_saida(), 4);
    assert_eq!(
        c.notas.registro(&chave).unwrap().situacao,
        Situacao::Autorizada
    );
}

#[tokio::test]
async fn a_carta_de_correcao_numera_1_2_3_pela_pasta_e_a_recusada_nao_gasta_a_sequencia() {
    let reg = || evento("135", "Evento registrado e vinculado a NF-e", NPROT_EVT);
    let (c, chave) = com_nota(
        "nfe-carta",
        vec![
            (EVT, reg()),
            (EVT, reg()),
            (EVT, reg()),
            (
                EVT,
                evento(
                    "594",
                    "Rejeicao: Numero de sequencia do evento maior que o permitido",
                    "",
                ),
            ),
            (EVT, reg()),
        ],
    )
    .await;
    for esperada in 1..=3 {
        let r = corrigir(&c.ctx(), &chave, "Corrigir o complemento do endereco", None)
            .await
            .unwrap();
        assert!(r.erro().is_none());
        assert_eq!(r.para_json()["sequencia"], esperada);
    }
    // a quarta e' recusada: nada guardado, a sequencia 4 continua livre
    let recusada = corrigir(&c.ctx(), &chave, "Corrigir o complemento do endereco", None)
        .await
        .unwrap();
    assert_eq!(recusada.erro().unwrap().codigo_saida(), 3);
    assert!(!recusada.guardado);
    let quarta = corrigir(&c.ctx(), &chave, "Corrigir o complemento do endereco", None)
        .await
        .unwrap();
    assert_eq!(quarta.para_json()["sequencia"], 4);
    let sequencias: Vec<String> = c
        .falso
        .corpos()
        .iter()
        .skip(1)
        .map(|b| crate::nfe::testes_comuns::entre(b, "<nSeqEvento>", "</nSeqEvento>").to_string())
        .collect();
    assert_eq!(sequencias, ["1", "2", "3", "4", "4"]);
    let reg = c.notas.registro(&chave).unwrap();
    assert_eq!(reg.eventos.iter().filter(|e| e.tipo == "110110").count(), 4);
    assert_eq!(
        reg.situacao,
        Situacao::Autorizada,
        "a carta nao muda a situacao"
    );
}

#[tokio::test]
async fn carta_alem_da_20a_e_de_nota_nao_autorizada_e_recusada_antes_da_rede() {
    let (c, chave) = com_nota("nfe-carta-limite", vec![]).await;
    for seq in 1..=20 {
        let ev = EventoGuardado {
            tipo: "110110".into(),
            sequencia: seq,
            protocolo: "1".into(),
        };
        c.notas
            .guardar_evento(&chave, &ev, "<procEventoNFe/>", "")
            .unwrap();
    }
    let erro = corrigir(&c.ctx(), &chave, "Corrigir o complemento do endereco", None)
        .await
        .unwrap_err();
    assert_eq!(erro.codigo_saida(), 3);
    assert!(erro.to_string().contains("20 cartas"), "{erro}");

    // a nota fora da pasta pede a sequencia
    let erro = corrigir(
        &c.ctx(),
        &chave_de_outra_pasta(),
        "Corrigir o complemento do endereco",
        None,
    )
    .await
    .unwrap_err();
    assert!(
        matches!(erro, Erro::Uso(ref m) if m.contains("sequencia")),
        "{erro}"
    );

    // nota cancelada nao se corrige
    let ev = EventoGuardado {
        tipo: "110111".into(),
        sequencia: 1,
        protocolo: "2".into(),
    };
    c.notas
        .guardar_evento(&chave, &ev, "<procEventoNFe/>", "")
        .unwrap();
    let erro = corrigir(&c.ctx(), &chave, "Corrigir o complemento do endereco", None)
        .await
        .unwrap_err();
    assert!(
        erro.to_string().contains("so' se corrige nota autorizada"),
        "{erro}"
    );
    assert!(c.falso.servicos().len() == 1, "so' a emissao foi a rede");
}

#[tokio::test]
async fn inutilizar_guarda_a_faixa_e_recusa_antes_da_rede_a_faixa_que_tem_nota() {
    let (c, chave) = com_nota(
        "nfe-inutiliza",
        vec![
            (
                INU,
                inutilizacao(
                    "102",
                    "Inutilizacao de numero homologado",
                    "135260000000300",
                ),
            ),
            (
                INU,
                inutilizacao(
                    "563",
                    "Rejeicao: Ja existe pedido de Inutilizacao com a mesma faixa",
                    "135260000000300",
                ),
            ),
        ],
    )
    .await;
    let r = inutilizar(&c.ctx(), None, 2, 3, "Numeros pulados por falha do sistema")
        .await
        .unwrap();
    assert!(r.erro().is_none());
    assert!(r.guardada);
    assert_eq!(r.para_json()["status"], "inutilizada");
    let faixas = c.notas.inutilizacoes().unwrap();
    assert_eq!(
        (faixas[0].inicio, faixas[0].fim, faixas[0].serie.as_str()),
        (2, 3, "1")
    );
    assert_eq!(faixas[0].protocolo, "135260000000300");
    assert_eq!(faixas[0].ano, "2026");
    assert_eq!(c.notas.proximo_numero("1").unwrap(), 4);
    assert!(c
        .pasta
        .caminho()
        .join("notas/nfe/inutilizacao/2026/1-2-3-procInutNFe.xml")
        .is_file());

    // a faixa que tem a nota 1 nem vai a rede
    let antes = c.falso.servicos().len();
    let erro = inutilizar(&c.ctx(), None, 1, 3, "Numeros pulados por falha do sistema")
        .await
        .unwrap_err();
    assert_eq!(erro.codigo_saida(), 3);
    assert!(erro.to_string().contains(&chave), "{erro}");
    assert_eq!(c.falso.servicos().len(), antes);

    // o 563 (a mesma faixa de novo) tambem e' faixa inutilizada
    let r = inutilizar(&c.ctx(), None, 4, 4, "Numero pulado por falha do sistema")
        .await
        .unwrap();
    assert_eq!(r.para_json()["status"], "ja_inutilizada");
    assert!(r.guardada);
}

#[tokio::test]
async fn inutilizar_rejeitada_nao_guarda_e_o_emitente_cpf_nao_inutiliza() {
    let c = Cenario::novo(
        "nfe-inutiliza-rej",
        vec![(
            INU,
            inutilizacao("241", "Rejeicao: Um numero da faixa ja foi utilizado", ""),
        )],
    );
    let r = inutilizar(
        &c.ctx(),
        Some("1"),
        1,
        2,
        "Numeros pulados por falha do sistema",
    )
    .await
    .unwrap();
    assert_eq!(r.erro().unwrap().codigo_saida(), 3);
    assert!(!r.guardada);
    assert!(c.notas.inutilizacoes().unwrap().is_empty());

    let mut config = c.config.clone();
    config.emitente.cnpj = String::new();
    config.emitente.cpf = "52998224725".into();
    let ctx = super::Contexto::novo(&c.cliente, &c.certificado, &config, Some(&c.notas))
        .com_agora(agora());
    let erro = inutilizar(&ctx, None, 1, 2, "Numeros pulados por falha do sistema")
        .await
        .unwrap_err();
    assert!(
        matches!(erro, Erro::Uso(ref m) if m.contains("CNPJ")),
        "{erro}"
    );
}

#[tokio::test]
async fn epec_registrado_deixa_a_nota_em_epec_e_o_recusado_nao_gasta_numero() {
    let c = Cenario::novo(
        "nfe-epec",
        vec![
            (
                EVT,
                evento("228", "Rejeicao: Data de emissao muito atrasada", ""),
            ),
            (
                EVT,
                evento(
                    "136",
                    "Evento registrado, mas nao vinculado a NF-e",
                    "891260000000001",
                ),
            ),
        ],
    );
    let mut p = pedido();
    p["x_just"] = serde_json::json!("Falha na comunicacao com a SEFAZ de origem");
    // recusado: a nota sai da pasta e o numero volta
    let r = epec(&c.ctx(), p.clone(), true).await.unwrap();
    assert_eq!(r.erro().unwrap().codigo_saida(), 3);
    assert_eq!(r.situacao, None);
    assert!(c.notas.listar().unwrap().is_empty());
    assert_eq!(c.notas.proximo_numero("1").unwrap(), 1);

    // registrado: a nota fica em EPEC, com o protocolo como o DANFE o imprime
    let r = epec(&c.ctx(), p, true).await.unwrap();
    assert!(r.erro().is_none(), "{}", r.resultado.frase);
    assert_eq!(r.situacao, Some(Situacao::Epec));
    assert!(r.guardado);
    let reg = c.notas.registro(&r.chave).unwrap();
    assert_eq!(reg.protocolo_epec, "891260000000001 03/10/2026 09:00:03");
    assert_eq!(reg.eventos[0].tipo, "110140");
    assert!(c
        .notas
        .ler_assinada(&r.chave)
        .unwrap()
        .unwrap()
        .contains("<tpEmis>4</tpEmis>"));
    assert_eq!(c.notas.proximo_numero("1").unwrap(), 2);
    // sem motivo da contingencia, nem monta
    let erro = epec(&c.ctx(), pedido(), true).await.unwrap_err();
    assert!(
        matches!(erro, Erro::Uso(ref m) if m.contains("motivo")),
        "{erro}"
    );
}
