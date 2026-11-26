// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A NFC-e no cliente da SEFAZ, com transporte falso: o endereco vem da tabela da NFC-e (outra que a da NF-e), o envio
//! e' sincrono, a consulta e o cancelamento servem a chave do modelo 65, e a contingencia off-line diz o passo seguinte.
//! Nenhuma chamada real: o transporte e' um roteiro em memoria.

mod nfce_comum;
mod nfe_sefaz_comum;

use chrono::DateTime;
use nfe_sefaz_comum::eventos::{ret_lote, EVT, NPROT_EVT};
use nfe_sefaz_comum::{
    cliente_em, ok, prot_ok, ret_envi_sinc, ret_sit, ret_status, Falso, AUT, CON, NPROT, STA,
};
use rustnf::nfe::montagem::{assinar, montar, nota_de_json, prazo_de_transmissao};
use rustnf::nfe::sefaz::contingencia::{passo_offline, PassoOffline};
use rustnf::nfe::sefaz::{tabela, Desfecho, Servico, SituacaoServico};
use rustnf::nfe::vigencias::Ambiente::{Homologacao, Producao};
use serde_json::json;

/// A NFC-e assinada (QR Code v3) do emitente ficticio de SP e a chave.
fn nfce(altera: serde_json::Value) -> (String, String) {
    let a = nfce_comum::arquivo();
    let m = montar(&nota_de_json(&nfce_comum::pedido(&a, &altera)).unwrap()).unwrap();
    (assinar(&m, &nfce_comum::certificado()).unwrap(), m.chave)
}

#[tokio::test]
async fn a_nfce_autoriza_no_endereco_da_nfce_e_em_lote_sincrono() {
    let (xml, chave) = nfce(json!({}));
    let f = Falso::novo(vec![(AUT, ok(AUT, &ret_envi_sinc(&prot_ok(&chave))))]);
    let (c, _) = cliente_em("SP", Homologacao, (2026, 10, 3), f.clone());
    let r = c.autorizar(&xml).await.unwrap();
    assert!(r.autorizada(), "{r:?}");
    assert_eq!(r.frase, format!("NFC-e autorizada: protocolo {NPROT}."));
    let Desfecho::Autorizada {
        nfe_proc: Some(proc_),
        ..
    } = &r.desfecho
    else {
        panic!("{r:?}");
    };
    // o documento guardado leva a NFC-e como foi assinada, com o QR Code
    assert!(proc_.contains("<infNFeSupl>") && proc_.contains(&xml));
    let ch = f.chamadas();
    assert_eq!(ch.len(), 1);
    // a NFC-e de SP tem endereco proprio: nao e' o da NF-e (`nfe.fazenda...`)
    assert_eq!(
        ch[0].url,
        "https://homologacao.nfce.fazenda.sp.gov.br/ws/NFeAutorizacao4.asmx"
    );
    assert!(
        ch[0].corpo.contains("<indSinc>1</indSinc>"),
        "{}",
        ch[0].corpo
    );
    assert!(ch[0].corpo.contains("<mod>65</mod>"));
}

#[tokio::test]
async fn a_nfce_nao_tem_lote_assincrono() {
    let (xml, _) = nfce(json!({}));
    let (c, _) = cliente_em("SP", Homologacao, (2026, 10, 3), Falso::default());
    let e = c.autorizar_assincrono(&xml).await.unwrap_err();
    assert_eq!(e.campo, "indSinc");
    assert!(e.mensagem.contains("NFC-e"), "{e}");
}

#[tokio::test]
async fn consulta_a_nfce_pela_chave_no_autorizador_da_nfce() {
    let (xml, chave) = nfce(json!({}));
    let f = Falso::novo(vec![(
        CON,
        ok(
            CON,
            &ret_sit(&chave, "100", "Autorizado o uso da NF-e", &prot_ok(&chave)),
        ),
    )]);
    let (c, _) = cliente_em("SP", Homologacao, (2026, 10, 3), f.clone());
    let r = c.consultar(&chave, Some(&xml)).await.unwrap();
    assert!(r.autorizada(), "{r:?}");
    assert_eq!(
        f.chamadas()[0].url,
        "https://homologacao.nfce.fazenda.sp.gov.br/ws/NFeConsultaProtocolo4.asmx"
    );
}

#[tokio::test]
async fn cancela_a_nfce_com_o_evento_110111_no_endereco_da_nfce() {
    let (_, chave) = nfce(json!({}));
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
    let (c, _) = cliente_em("SP", Homologacao, (2026, 10, 3), f.clone());
    let r = c
        .cancelar(
            &nfce_comum::certificado(),
            &chave,
            NPROT,
            "Cancelamento de teste da NFC-e",
        )
        .await
        .unwrap();
    assert!(r.registrado(), "{r:?}");
    assert_eq!(r.autorizador, "SP");
    let ch = f.chamadas();
    assert_eq!(
        ch[0].url,
        "https://homologacao.nfce.fazenda.sp.gov.br/ws/NFeRecepcaoEvento4.asmx"
    );
    assert!(ch[0].corpo.contains("<tpEvento>110111</tpEvento>"));
    assert!(ch[0].corpo.contains(&format!("<chNFe>{chave}</chNFe>")));
}

#[tokio::test]
async fn o_status_da_nfce_vai_ao_servico_da_nfce() {
    let f = Falso::novo(vec![(
        STA,
        ok(STA, &ret_status("107", "Servico em Operacao", "")),
    )]);
    let (c, _) = cliente_em("SP", Homologacao, (2026, 10, 3), f.clone());
    let s = c.status_nfce().await.unwrap();
    assert_eq!(s.situacao, SituacaoServico::EmOperacao);
    assert_eq!(
        f.chamadas()[0].url,
        "https://homologacao.nfce.fazenda.sp.gov.br/ws/NFeStatusServico4.asmx"
    );
}

#[test]
fn a_tabela_da_nfce_tem_autorizador_para_as_27_uf_e_endereco_de_cada_servico() {
    let ufs = [
        "AC", "AL", "AM", "AP", "BA", "CE", "DF", "ES", "GO", "MA", "MG", "MS", "MT", "PA", "PB",
        "PE", "PI", "PR", "RJ", "RN", "RO", "RR", "RS", "SC", "SE", "SP", "TO",
    ];
    for uf in ufs {
        for amb in [Producao, Homologacao] {
            for s in [
                Servico::StatusServico,
                Servico::Autorizacao,
                Servico::ConsultaProtocolo,
                Servico::RecepcaoEvento,
            ] {
                let u = tabela::url_nfce(uf, amb, s).unwrap_or_else(|e| panic!("{uf}: {e}"));
                assert!(
                    u.starts_with("https://") && !u.ends_with("?wsdl"),
                    "{uf}: {u}"
                );
            }
        }
    }
    // o mapa de autorizadores da NFC-e: 7 UF com servico proprio, o resto na SVRS (e o RS tambem)
    let proprias: Vec<&str> = ufs
        .iter()
        .copied()
        .filter(|u| tabela::autorizador_nfce_da_uf(u).unwrap() != "SVRS")
        .collect();
    assert_eq!(proprias, ["AM", "GO", "MG", "MS", "MT", "PR", "SP"]);
    assert_eq!(tabela::autorizador_nfce_da_uf("RS").unwrap(), "SVRS");
    // a NF-e continua com a tabela dela
    assert_eq!(
        tabela::url("SP", Homologacao, Servico::Autorizacao).unwrap(),
        "https://homologacao.nfe.fazenda.sp.gov.br/ws/nfeautorizacao4.asmx"
    );
}

#[test]
fn o_prazo_da_contingencia_off_line_e_o_fim_do_primeiro_dia_util_seguinte() {
    let em = |s: &str| DateTime::parse_from_rfc3339(s).unwrap();
    // sexta-feira 2026-10-02 as 18h: o prazo e' a segunda-feira 2026-10-05, 23:59:59
    let p = prazo_de_transmissao(&em("2026-10-02T18:00:00-03:00"), "SP").unwrap();
    assert_eq!(p.to_rfc3339(), "2026-10-05T23:59:59-03:00");
    // terca-feira: a quarta
    let p = prazo_de_transmissao(&em("2026-10-06T09:00:00-03:00"), "SP").unwrap();
    assert_eq!(p.to_rfc3339(), "2026-10-07T23:59:59-03:00");
    // sabado: a segunda
    let p = prazo_de_transmissao(&em("2026-10-03T09:00:00-03:00"), "SP").unwrap();
    assert_eq!(p.to_rfc3339(), "2026-10-05T23:59:59-03:00");
    // o fuso e' o da UF (AM, -04:00)
    let p = prazo_de_transmissao(&em("2026-10-06T09:00:00-03:00"), "AM").unwrap();
    assert_eq!(p.to_rfc3339(), "2026-10-07T23:59:59-04:00");
    assert!(prazo_de_transmissao(&em("2026-10-06T09:00:00-03:00"), "XX").is_err());
}

#[tokio::test]
async fn a_nfce_off_line_transmitida_diz_o_passo_seguinte() {
    let off = json!({"tp_emis": 9, "dh_cont": "2026-10-03T08:40:00-03:00",
                     "x_just": "Falha de comunicacao com a SEFAZ"});
    let (xml, chave) = nfce(off);
    // autorizada dentro do prazo: acabou
    let f = Falso::novo(vec![(AUT, ok(AUT, &ret_envi_sinc(&prot_ok(&chave))))]);
    let (c, _) = cliente_em("SP", Homologacao, (2026, 10, 3), f);
    let r = c.autorizar(&xml).await.unwrap();
    assert_eq!(passo_offline(&r), PassoOffline::Concluida);
    // 150: vale, mas fora do prazo
    let p150 = nfe_sefaz_comum::prot(
        &chave,
        "150",
        "Autorizado o uso da NF-e, autorizacao fora de prazo",
        NPROT,
    );
    let f = Falso::novo(vec![(AUT, ok(AUT, &ret_envi_sinc(&p150)))]);
    let (c, _) = cliente_em("SP", Homologacao, (2026, 10, 3), f);
    let r = c.autorizar(&xml).await.unwrap();
    assert_eq!(passo_offline(&r), PassoOffline::ConcluidaForaDoPrazo);
    // rejeitada: corrigir e reenviar com a mesma chave
    let rej = nfe_sefaz_comum::prot(
        &chave,
        "704",
        "Rejeicao: NFC-e com Data-Hora de emissao atrasada",
        "",
    );
    let f = Falso::novo(vec![(AUT, ok(AUT, &ret_envi_sinc(&rej)))]);
    let (c, _) = cliente_em("SP", Homologacao, (2026, 10, 3), f);
    let r = c.autorizar(&xml).await.unwrap();
    assert_eq!(
        passo_offline(&r),
        PassoOffline::CorrigirEReenviar {
            cstat: "704".into(),
            motivo: "Rejeicao: NFC-e com Data-Hora de emissao atrasada".into()
        }
    );
}
