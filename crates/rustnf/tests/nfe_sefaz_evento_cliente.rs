// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Os eventos no cliente com transporte falso: cancelamento homologado (135) e fora do prazo (155 e 501), carta de
//! correcao, EPEC (136 e 142), evento da apuracao na SVRS, o 136 sem vinculo, evento sem resposta (a consulta acha, nao
//! acha, ou tambem nao responde), duplicidade 573, e o retorno com o XML escapado dentro do `nfeResultMsg`.

mod nfe_sefaz_comum;

use nfe_sefaz_comum::eventos::*;
use nfe_sefaz_comum::*;
use rustnf::nfe::sefaz::evento::{EventoRtc, ItemRtc};
use rustnf::nfe::sefaz::resposta_evento::ler_proc_evento;
use rustnf::nfe::sefaz::{protocolo_epec_danfe, DesfechoEvento, SituacaoServico};

const URL_EVENTO_SP: &str = "https://homologacao.nfe.fazenda.sp.gov.br/ws/nferecepcaoevento4.asmx";
const JUST: &str = "Cancelamento por erro no pedido";

fn protocolo_evt(r: &rustnf::nfe::sefaz::ResultadoEvento) -> &str {
    match &r.desfecho {
        DesfechoEvento::Registrado { protocolo, .. }
        | DesfechoEvento::SemVinculo { protocolo, .. } => protocolo,
        _ => "",
    }
}

// ------------------------------------------------------------------------------------------------ cancelamento

#[tokio::test]
async fn cancelamento_homologado_135_guarda_o_proc_evento_e_vai_assinado_a_uf() {
    let (_, chave) = nfe(1, 2);
    let f = Falso::novo(vec![(
        EVT,
        ok(
            EVT,
            &ret_lote(
                &chave,
                "110111",
                1,
                "135",
                "Evento registrado e vinculado a NF-e",
                NPROT_EVT,
            ),
        ),
    )]);
    let (c, _) = cliente(f.clone());
    let r = c
        .cancelar(&certificado(), &chave, NPROT, JUST)
        .await
        .unwrap();

    assert!(r.registrado());
    assert_eq!(protocolo_evt(&r), NPROT_EVT);
    assert_eq!(
        r.frase,
        format!("Cancelamento registrado: protocolo {NPROT_EVT}.")
    );
    assert!(!r.recuperado);
    assert_eq!(
        (r.tp_evento.as_str(), r.seq, r.autorizador.as_str()),
        ("110111", 1, "SP")
    );
    // o procEventoNFe: o evento assinado como foi enviado + o retEvento como a SEFAZ o mandou; a leitura acha tudo
    let proc_ = r.proc_evento.as_deref().unwrap();
    let lido = ler_proc_evento(proc_).unwrap();
    assert_eq!(
        (lido.tp_evento.as_str(), lido.seq, lido.chave.as_str()),
        ("110111", Some(1), chave.as_str())
    );
    assert_eq!(lido.registro.protocolo, NPROT_EVT);
    assert_eq!(lido.id, format!("ID110111{chave}01"));
    assert!(proc_.contains("<Signature "));
    assert!(proc_.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?><procEventoNFe "));
    // a chamada: uma so', RecepcaoEvento de SP, com o envEvento assinado
    let ch = f.chamadas();
    assert_eq!(ch.len(), 1);
    assert_eq!(ch[0].url, URL_EVENTO_SP);
    assert_eq!(
        ch[0].content_type,
        "application/soap+xml; charset=utf-8; action=\"http://www.portalfiscal.inf.br/nfe/wsdl/NFeRecepcaoEvento4/nfeRecepcaoEvento\""
    );
    assert!(ch[0].corpo.contains(&r.xml_enviado));
    assert!(rustnf::assinatura::xmldsig::conferido(&r.xml_enviado));
    assert_eq!(f.restantes(), 0);
}

#[tokio::test]
async fn cancelamento_155_homologado_fora_do_prazo_diz_isso_em_palavras() {
    let (_, chave) = nfe(2, 2);
    let f = Falso::novo(vec![(
        EVT,
        ok(
            EVT,
            &ret_lote(
                &chave,
                "110111",
                1,
                "155",
                "Cancelamento homologado fora de prazo",
                NPROT_EVT,
            ),
        ),
    )]);
    let (c, _) = cliente(f);
    let r = c
        .cancelar(&certificado(), &chave, NPROT, JUST)
        .await
        .unwrap();
    assert!(r.registrado());
    let DesfechoEvento::Registrado {
        cstat,
        fora_do_prazo,
        ..
    } = &r.desfecho
    else {
        panic!("{r:?}")
    };
    assert_eq!((cstat.as_str(), *fora_do_prazo), ("155", true));
    assert!(r.frase.contains("FORA DO PRAZO"), "{}", r.frase);
    assert!(r.proc_evento.is_some());
}

#[tokio::test]
async fn cancelamento_501_fora_do_prazo_recusado_diz_o_motivo_e_nao_consulta() {
    let (_, chave) = nfe(3, 2);
    let motivo = "Rejeicao: Prazo de cancelamento superior ao previsto na Legislacao";
    let f = Falso::novo(vec![(
        EVT,
        ok(EVT, &ret_lote(&chave, "110111", 1, "501", motivo, "")),
    )]);
    let (c, _) = cliente(f.clone());
    let r = c
        .cancelar(&certificado(), &chave, NPROT, JUST)
        .await
        .unwrap();
    assert!(!r.registrado() && !r.pode_reenviar() && !r.deve_consultar());
    let DesfechoEvento::PrazoVencido { cstat, motivo: m } = &r.desfecho else {
        panic!("{r:?}")
    };
    assert_eq!((cstat.as_str(), m.as_str()), ("501", motivo));
    assert!(
        r.frase.contains("prazo") && r.frase.contains("501"),
        "{}",
        r.frase
    );
    assert!(r.proc_evento.is_none());
    assert_eq!(f.chamadas().len(), 1);
}

#[tokio::test]
async fn cancelamento_rejeitado_traz_o_cstat_e_o_motivo_da_sefaz() {
    let (_, chave) = nfe(4, 2);
    let f = Falso::novo(vec![(
        EVT,
        ok(
            EVT,
            &ret_lote(
                &chave,
                "110111",
                1,
                "222",
                "Rejeicao: Protocolo de Autorizacao de Uso difere do cadastrado",
                "",
            ),
        ),
    )]);
    let (c, _) = cliente(f);
    let r = c
        .cancelar(&certificado(), &chave, NPROT, JUST)
        .await
        .unwrap();
    let DesfechoEvento::Rejeitado { cstat, motivo } = &r.desfecho else {
        panic!("{r:?}")
    };
    assert_eq!(cstat, "222");
    assert!(motivo.contains("Protocolo"));
    assert!(r
        .frase
        .starts_with("Cancelamento rejeitado (222): Rejeicao"));
}

#[tokio::test]
async fn evento_136_nao_vincula_e_diz_que_a_nota_nao_mudou() {
    let (_, chave) = nfe(5, 2);
    let f = Falso::novo(vec![(
        EVT,
        ok(
            EVT,
            &ret_lote(
                &chave,
                "110111",
                1,
                "136",
                "Evento registrado, mas nao vinculado a NF-e",
                NPROT_EVT,
            ),
        ),
    )]);
    let (c, _) = cliente(f);
    let r = c
        .cancelar(&certificado(), &chave, NPROT, JUST)
        .await
        .unwrap();
    assert!(matches!(r.desfecho, DesfechoEvento::SemVinculo { .. }));
    assert!(!r.registrado());
    assert!(r.frase.contains("SEM vinculo") && r.frase.contains("nao foi alterada"));
    assert!(r.proc_evento.is_some());
}

#[tokio::test]
async fn lote_rejeitado_e_servico_parado_nao_viram_registro() {
    let (_, chave) = nfe(6, 2);
    let lote = ret_env_evento("215", "Rejeicao: Falha no schema XML", "");
    let f = Falso::novo(vec![(EVT, ok(EVT, &lote))]);
    let (c, _) = cliente(f);
    let r = c
        .cancelar(&certificado(), &chave, NPROT, JUST)
        .await
        .unwrap();
    let DesfechoEvento::Rejeitado { cstat, .. } = &r.desfecho else {
        panic!("{r:?}")
    };
    assert_eq!(cstat, "215");
    assert!(r.xml_retorno.contains("<retEnvEvento"));

    let parado = ret_env_evento("108", "Servico Paralisado Momentaneamente", "");
    let f = Falso::novo(vec![(EVT, ok(EVT, &parado))]);
    let (c, _) = cliente(f);
    let r = c
        .cancelar(&certificado(), &chave, NPROT, JUST)
        .await
        .unwrap();
    assert!(matches!(r.desfecho, DesfechoEvento::Indisponivel { .. }));
    assert!(r.pode_reenviar());
}

// ------------------------------------------------------------------------------------------------ carta de correcao

#[tokio::test]
async fn carta_de_correcao_sequencias_1_e_2_e_a_segunda_substitui_a_primeira() {
    let (_, chave) = nfe(10, 2);
    let f = Falso::novo(vec![
        (
            EVT,
            ok(
                EVT,
                &ret_lote(
                    &chave,
                    "110110",
                    1,
                    "135",
                    "Evento registrado e vinculado a NF-e",
                    "135260000000011",
                ),
            ),
        ),
        (
            EVT,
            ok(
                EVT,
                &ret_lote(
                    &chave,
                    "110110",
                    2,
                    "135",
                    "Evento registrado e vinculado a NF-e",
                    "135260000000012",
                ),
            ),
        ),
    ]);
    let (c, _) = cliente(f.clone());
    let a = c
        .corrigir(
            &certificado(),
            &chave,
            "Corrigir o numero do pedido para 1234",
            1,
        )
        .await
        .unwrap();
    let b = c
        .corrigir(
            &certificado(),
            &chave,
            "Corrigir o numero do pedido para 1234 e o endereco para Rua B",
            2,
        )
        .await
        .unwrap();
    assert!(a.registrado() && b.registrado());
    assert_eq!((a.seq, b.seq), (1, 2));
    assert!(a.xml_enviado.contains(&format!("ID110110{chave}01")));
    assert!(b.xml_enviado.contains(&format!("ID110110{chave}02")));
    assert_eq!(
        a.frase,
        "Carta de correcao registrado: protocolo 135260000000011."
    );
    // cada uma e' um evento novo, com o texto inteiro dela (a nova substitui a anterior)
    let ch = f.chamadas();
    assert!(ch[1].corpo.contains("o endereco para Rua B") && !ch[0].corpo.contains("Rua B"));
    // sequencia 21 e' recusada antes da rede
    let e = c
        .corrigir(&certificado(), &chave, "Texto da correcao valido", 21)
        .await
        .unwrap_err();
    assert_eq!(e.campo, "nSeqEvento");
    assert_eq!(f.chamadas().len(), 2);
}

// ------------------------------------------------------------------------------------------------ EPEC

#[tokio::test]
async fn epec_136_vai_ao_ambiente_nacional_e_o_protocolo_sai_para_o_danfe() {
    let (xml, chave) = nfe_em_contingencia(20, 4);
    let ret = ret_lote(
        &chave,
        "110140",
        1,
        "136",
        "EPEC autorizado",
        "891260000000001",
    );
    let f = Falso::novo(vec![(EVT, ok(EVT, &ret))]);
    let (c, _) = cliente(f.clone());
    let r = c
        .registrar_epec(&certificado(), &xml, "rustnf 0.4")
        .await
        .unwrap();
    assert!(r.registrado());
    let DesfechoEvento::Registrado { cstat, .. } = &r.desfecho else {
        panic!("{r:?}")
    };
    assert_eq!(cstat, "136");
    assert_eq!(r.autorizador, "AN");
    assert_eq!(
        f.chamadas()[0].url,
        "https://hom1.nfe.fazenda.gov.br/NFeRecepcaoEvento4/NFeRecepcaoEvento4.asmx"
    );
    assert!(f.chamadas()[0].corpo.contains("<cOrgao>91</cOrgao>"));
    assert_eq!(
        protocolo_epec_danfe(&r).unwrap(),
        "891260000000001 03/10/2026 09:00:03"
    );
    // o que nao e' EPEC registrado nao tem protocolo de DANFE
    let (_, chave2) = nfe(21, 2);
    let f = Falso::novo(vec![(
        EVT,
        ok(EVT, &ret_lote(&chave2, "110111", 1, "135", "ok", NPROT_EVT)),
    )]);
    let (c, _) = cliente(f);
    let canc = c
        .cancelar(&certificado(), &chave2, NPROT, JUST)
        .await
        .unwrap();
    assert!(protocolo_epec_danfe(&canc).is_err());
}

#[tokio::test]
async fn epec_142_ambiente_bloqueado_traz_as_chaves_pendentes() {
    let (xml, chave) = nfe_em_contingencia(22, 4);
    let (_, p1) = nfe(23, 2);
    let (_, p2) = nfe(24, 2);
    let ev = ret_evento(
        &chave,
        "110140",
        1,
        "142",
        "Rejeicao: Ambiente de Contingencia EPEC bloqueado para o Emitente",
        "",
        &format!("<chNFePend>{p1}</chNFePend><chNFePend>{p2}</chNFePend>"),
    );
    let f = Falso::novo(vec![(
        EVT,
        ok(
            EVT,
            &ret_env_evento("128", "Lote de Evento Processado", &ev),
        ),
    )]);
    let (c, _) = cliente(f);
    let r = c
        .registrar_epec(&certificado(), &xml, "rustnf 0.4")
        .await
        .unwrap();
    let DesfechoEvento::EpecBloqueado { pendentes, .. } = &r.desfecho else {
        panic!("{r:?}")
    };
    assert_eq!(pendentes, &[p1, p2]);
    assert!(
        r.frase.contains("2 EPEC sem a NF-e transmitida"),
        "{}",
        r.frase
    );
    assert!(r.proc_evento.is_none());
}

// ------------------------------------------------------------------------------------------------ apuracao na SVRS

#[tokio::test]
async fn evento_de_apuracao_vai_a_svrs_mesmo_com_o_emitente_em_sp() {
    let (_, chave) = nfe(30, 2);
    let it = ItemRtc {
        item: 1,
        v_ibs: Some("1".into()),
        v_cbs: Some("2".into()),
        quantidade: Some("3".into()),
        unidade: Some("UN".into()),
        ..ItemRtc::default()
    };
    let base = EventoRtc {
        itens: vec![it],
        ..EventoRtc::novo(
            &chave,
            "112120",
            1,
            rustnf::nfe::vigencias::Ambiente::Producao,
            chrono::Utc::now().fixed_offset(),
            "rustnf 0.4",
        )
    };
    let f = Falso::novo(vec![(
        EVT,
        ok(
            EVT,
            &ret_lote(
                &chave,
                "112120",
                1,
                "135",
                "Evento registrado e vinculado a NF-e",
                NPROT_EVT,
            ),
        ),
    )]);
    let (c, _) = cliente(f.clone());
    let r = c.registrar_evento_rtc(&certificado(), &base).await.unwrap();
    assert!(r.registrado());
    assert_eq!(r.autorizador, "SVRS");
    let ch = f.chamadas();
    assert_eq!(
        ch[0].url,
        "https://nfe-homologacao.svrs.rs.gov.br/ws/recepcaoevento/recepcaoevento4.asmx"
    );
    // o ambiente e a hora sao os do cliente, nao os do pedido (que era Producao)
    assert!(ch[0].corpo.contains("<tpAmb>2</tpAmb>"));
    assert!(ch[0]
        .corpo
        .contains("<dhEvento>2026-10-03T09:00:00-03:00</dhEvento>"));
}

// ------------------------------------------------------------------------------------------------ sem resposta

fn consulta_com_evento(chave: &str, tp: &str, seq: u32, prot_evt: &str) -> String {
    let miolo = format!(
        "{}{}",
        prot_ok(chave),
        proc_na_consulta(chave, tp, seq, "135", prot_evt)
    );
    ret_sit(chave, "101", "Cancelamento de NF-e homologado", &miolo)
}

#[tokio::test]
async fn cancelamento_sem_resposta_a_consulta_acha_o_evento_e_devolve_o_registro() {
    let (_, chave) = nfe(40, 2);
    let f = Falso::novo(vec![
        (EVT, tempo()),
        (
            CON,
            ok(CON, &consulta_com_evento(&chave, "110111", 1, NPROT_EVT)),
        ),
    ]);
    let (c, _) = cliente(f.clone());
    let r = c
        .cancelar(&certificado(), &chave, NPROT, JUST)
        .await
        .unwrap();
    assert!(r.registrado() && r.recuperado);
    assert_eq!(protocolo_evt(&r), NPROT_EVT);
    let lido = ler_proc_evento(r.proc_evento.as_deref().unwrap()).unwrap();
    assert_eq!(lido.registro.protocolo, NPROT_EVT);
    assert!(r.detalhe.contains("sem resposta"), "{}", r.detalhe);
    // evento + consulta, nada mais: nenhum segundo cancelamento
    assert_eq!(f.chamadas().len(), 2);
    assert_eq!(f.restantes(), 0);
}

#[tokio::test]
async fn evento_sem_resposta_e_a_consulta_sem_o_evento_diz_para_reenviar_o_mesmo() {
    let (_, chave) = nfe(41, 2);
    let sem_evento = ret_sit(&chave, "100", "Autorizado o uso da NF-e", &prot_ok(&chave));
    let f = Falso::novo(vec![(EVT, tempo()), (CON, ok(CON, &sem_evento))]);
    let (c, _) = cliente(f);
    let r = c
        .corrigir(
            &certificado(),
            &chave,
            "Corrigir o endereco de entrega da nota",
            3,
        )
        .await
        .unwrap();
    assert!(matches!(r.desfecho, DesfechoEvento::NaoRegistrado));
    assert!(r.pode_reenviar() && !r.deve_consultar());
    assert_eq!(r.seq, 3);
    assert!(
        r.frase.contains("MESMO") && r.frase.contains("nSeqEvento"),
        "{}",
        r.frase
    );
}

#[tokio::test]
async fn evento_sem_resposta_e_a_consulta_tambem_sem_resposta_manda_consultar_de_novo() {
    let (_, chave) = nfe(42, 2);
    let f = Falso::novo(vec![(EVT, tempo()), (CON, tempo())]);
    let (c, _) = cliente(f.clone());
    let r = c
        .cancelar(&certificado(), &chave, NPROT, JUST)
        .await
        .unwrap();
    assert!(matches!(r.desfecho, DesfechoEvento::SemResposta));
    assert!(r.deve_consultar() && !r.pode_reenviar() && !r.registrado());
    assert!(
        r.frase.contains("PODE ter sido registrado") && r.frase.contains("NAO envie outro"),
        "{}",
        r.frase
    );
    assert_eq!(f.chamadas().len(), 2);
}

#[tokio::test]
async fn cancelamento_confirmado_pelo_estado_da_nota_mesmo_sem_o_evento_na_lista() {
    let (_, chave) = nfe(43, 2);
    let cancelada = ret_sit(
        &chave,
        "101",
        "Cancelamento de NF-e homologado",
        &prot_ok(&chave),
    );
    let f = Falso::novo(vec![
        (EVT, fault("falha interna")),
        (CON, ok(CON, &cancelada)),
    ]);
    let (c, _) = cliente(f);
    let r = c
        .cancelar(&certificado(), &chave, NPROT, JUST)
        .await
        .unwrap();
    assert!(r.registrado() && r.recuperado);
    assert!(r.proc_evento.is_none());
}

#[tokio::test]
async fn duplicidade_573_a_consulta_devolve_o_registro_que_ja_existe() {
    let (_, chave) = nfe(44, 2);
    let dup = ret_lote(
        &chave,
        "110110",
        1,
        "573",
        "Rejeicao: Duplicidade de evento",
        "",
    );
    let f = Falso::novo(vec![
        (EVT, ok(EVT, &dup)),
        (
            CON,
            ok(
                CON,
                &ret_sit(
                    &chave,
                    "100",
                    "Autorizado o uso da NF-e",
                    &format!(
                        "{}{}",
                        prot_ok(&chave),
                        proc_na_consulta(&chave, "110110", 1, "135", NPROT_EVT)
                    ),
                ),
            ),
        ),
    ]);
    let (c, _) = cliente(f);
    let r = c
        .corrigir(
            &certificado(),
            &chave,
            "Corrigir o endereco de entrega da nota",
            1,
        )
        .await
        .unwrap();
    assert!(r.registrado() && r.recuperado);
    // a consulta que ainda nao mostra o evento (sincronismo) NAO vira "nao registrado": manda consultar de novo
    let f = Falso::novo(vec![
        (EVT, ok(EVT, &dup)),
        (
            CON,
            ok(
                CON,
                &ret_sit(&chave, "100", "Autorizado o uso da NF-e", &prot_ok(&chave)),
            ),
        ),
    ]);
    let (c, _) = cliente(f);
    let r = c
        .corrigir(
            &certificado(),
            &chave,
            "Corrigir o endereco de entrega da nota",
            1,
        )
        .await
        .unwrap();
    assert!(r.deve_consultar());
}

#[tokio::test]
async fn resposta_ilegivel_do_evento_tambem_vai_para_a_consulta() {
    let (_, chave) = nfe(45, 2);
    let f = Falso::novo(vec![
        (
            EVT,
            Resp::R(200, b"<html>pagina de manutencao</html>".to_vec()),
        ),
        (
            CON,
            ok(CON, &consulta_com_evento(&chave, "110111", 1, NPROT_EVT)),
        ),
    ]);
    let (c, _) = cliente(f);
    let r = c
        .cancelar(&certificado(), &chave, NPROT, JUST)
        .await
        .unwrap();
    assert!(r.registrado() && r.recuperado);
}

// ------------------------------------------------------------------------------------------------ XML escapado

fn escapado(ns: &str, ret: &str) -> Vec<u8> {
    let txt = ret
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?><soap:Envelope xmlns:soap=\"http://www.w3.org/2003/05/soap-envelope\">\
         <soap:Body><nfeResultMsg xmlns=\"http://www.portalfiscal.inf.br/nfe/wsdl/{ns}\">{txt}</nfeResultMsg></soap:Body></soap:Envelope>"
    )
    .into_bytes()
}

#[tokio::test]
async fn retorno_com_o_xml_escapado_dentro_do_nferesultmsg_e_lido_no_evento() {
    let (_, chave) = nfe(50, 2);
    let ret = ret_lote(
        &chave,
        "110111",
        1,
        "135",
        "Evento registrado e vinculado a NF-e",
        NPROT_EVT,
    );
    let f = Falso::novo(vec![(EVT, Resp::R(200, escapado(EVT, &ret)))]);
    let (c, _) = cliente(f);
    let r = c
        .cancelar(&certificado(), &chave, NPROT, JUST)
        .await
        .unwrap();
    assert!(r.registrado() && !r.recuperado, "{r:?}");
    assert_eq!(protocolo_evt(&r), NPROT_EVT);
    assert!(ler_proc_evento(r.proc_evento.as_deref().unwrap()).is_ok());
}

#[tokio::test]
async fn retorno_com_o_xml_escapado_e_lido_nos_servicos_da_parte_1() {
    // status
    let f = Falso::novo(vec![(
        STA,
        Resp::R(
            200,
            escapado(
                STA,
                &ret_status("107", "Servico em Operacao", "<tMed>2</tMed>"),
            ),
        ),
    )]);
    let (c, _) = cliente(f);
    let s = c.status().await.unwrap();
    assert_eq!(s.situacao, SituacaoServico::EmOperacao);
    assert_eq!(s.tempo_medio, Some(2));
    // autorizacao + nfeProc
    let (xml, chave) = nfe(51, 2);
    let prot = prot_ok(&chave);
    let f = Falso::novo(vec![(
        AUT,
        Resp::R(200, escapado(AUT, &ret_envi_sinc(&prot))),
    )]);
    let (c, _) = cliente(f);
    let r = c.autorizar(&xml).await.unwrap();
    assert!(r.autorizada() && !r.recuperada, "{r:?}");
    assert!(r.nfe_proc().unwrap().contains(&prot));
    // consulta
    let f = Falso::novo(vec![(
        CON,
        Resp::R(
            200,
            escapado(
                CON,
                &ret_sit(&chave, "100", "Autorizado o uso da NF-e", &prot),
            ),
        ),
    )]);
    let (c, _) = cliente(f);
    let r = c.consultar(&chave, Some(&xml)).await.unwrap();
    assert!(r.autorizada());
    // texto que nao e' retorno da NF-e continua sendo erro
    let f = Falso::novo(vec![(
        STA,
        Resp::R(
            200,
            escapado(
                STA,
                "<outraCoisa xmlns=\"http://www.portalfiscal.inf.br/nfe\"/>",
            ),
        ),
    )]);
    let (c, _) = cliente(f);
    assert_eq!(
        c.status().await.unwrap().situacao,
        SituacaoServico::SemResposta
    );
}

#[tokio::test]
async fn consulta_com_so_o_epec_124_diz_que_a_nota_ainda_precisa_ser_transmitida() {
    let (_, chave) = nfe(52, 2);
    let f = Falso::novo(vec![(
        CON,
        ok(CON, &ret_sit(&chave, "124", "EPEC Autorizado", "")),
    )]);
    let (c, _) = cliente(f);
    let r = c.consultar(&chave, None).await.unwrap();
    assert!(matches!(
        r.desfecho,
        rustnf::nfe::sefaz::Desfecho::SoEpec { .. }
    ));
    assert_eq!(
        r.frase,
        "Na SEFAZ consta so' o EPEC (124), sem a NF-e: a nota ainda precisa ser transmitida."
    );
}
