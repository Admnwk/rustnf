// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Cliente da SEFAZ com transporte falso: autorizada, denegada, rejeitada, lote em processamento e retorno do recibo,
//! resposta perdida (a consulta acha a nota, ou diz que nao consta), duplicidade 204 e 539, SOAP Fault, HTTP de erro,
//! assincrono depois do corte da NT 2023.002, e as conferencias que recusam o envio antes da rede.

mod nfe_sefaz_comum;

use std::time::Duration;

use chrono::{TimeZone, Utc};
use nfe_sefaz_comum::*;
use rustnf::chave_dfe;
use rustnf::nfe::sefaz::resposta::nfe_proc;
use rustnf::nfe::sefaz::{tabela, ClienteSefaz, Config, Desfecho, Servico, SituacaoServico};
use rustnf::nfe::vigencias::Ambiente::{Homologacao, Producao};
use serde_json::json;

// ------------------------------------------------------------------------------------------------ autorizacao

#[tokio::test]
async fn autorizada_no_sincrono_guarda_a_nfe_como_foi_enviada_com_o_protocolo_da_sefaz() {
    let (xml, chave) = nfe(1, 2);
    let prot_cru = prot_ok(&chave);
    let f = Falso::novo(vec![(AUT, ok(AUT, &ret_envi_sinc(&prot_cru)))]);
    let (c, _) = cliente(f.clone());
    let r = c.autorizar(&xml).await.unwrap();

    let Desfecho::Autorizada {
        cstat,
        protocolo,
        nfe_proc: Some(proc_),
        ..
    } = &r.desfecho
    else {
        panic!("{r:?}");
    };
    assert_eq!((cstat.as_str(), protocolo.as_str()), ("100", NPROT));
    assert!(r.autorizada());
    assert_eq!(r.chave, chave);
    assert!(!r.recuperada);
    assert_eq!(r.frase, format!("NF-e autorizada: protocolo {NPROT}."));
    // o documento guardado: a NFe como veio assinada e o protNFe como a SEFAZ mandou, sem tocar num byte
    assert_eq!(*proc_, nfe_proc(&xml, &prot_cru));
    assert!(proc_.contains(&xml) && proc_.contains(&prot_cru));
    assert_eq!(r.nfe_proc(), Some(proc_.as_str()));
    // ida e volta: a leitura da NF-e acha o protocolo
    let lida = rustnf::nfe::ler(proc_.as_bytes()).unwrap();
    let p = lida.protocolo.unwrap();
    assert_eq!((p.numero.as_str(), p.cstat.as_str()), (NPROT, "100"));
    assert_eq!(p.chave, chave);

    // a chamada: uma so', no endereco de homologacao de SP, SOAP 1.2 com a NFe crua dentro do enviNFe
    let ch = f.chamadas();
    assert_eq!(ch.len(), 1);
    assert_eq!(
        ch[0].url,
        "https://homologacao.nfe.fazenda.sp.gov.br/ws/nfeautorizacao4.asmx"
    );
    assert_eq!(
        ch[0].content_type,
        "application/soap+xml; charset=utf-8; action=\"http://www.portalfiscal.inf.br/nfe/wsdl/NFeAutorizacao4/nfeAutorizacaoLote\""
    );
    assert!(
        ch[0].corpo.contains(&xml),
        "a NFe tem de ir como foi assinada"
    );
    assert!(ch[0].corpo.contains("<indSinc>1</indSinc>"));
    assert_eq!(r.xml_enviado.matches("<NFe ").count(), 1);
    assert_eq!(f.restantes(), 0);
}

#[tokio::test]
async fn denegada_110_301_302_o_numero_esta_consumido() {
    for cs in ["110", "301", "302", "303"] {
        let (xml, chave) = nfe(2, 2);
        let p = prot(
            &chave,
            cs,
            "Uso Denegado: Irregularidade fiscal do emitente",
            NPROT,
        );
        let f = Falso::novo(vec![(AUT, ok(AUT, &ret_envi_sinc(&p)))]);
        let (c, _) = cliente(f);
        let r = c.autorizar(&xml).await.unwrap();
        let Desfecho::Denegada {
            cstat,
            protocolo,
            nfe_proc: Some(proc_),
            ..
        } = &r.desfecho
        else {
            panic!("{cs}: {r:?}");
        };
        assert_eq!((cstat.as_str(), protocolo.as_str()), (cs, NPROT));
        assert_eq!(*proc_, nfe_proc(&xml, &p));
        assert!(!r.autorizada() && !r.pode_reenviar() && !r.deve_consultar());
        assert!(
            r.frase.contains("denegado") && r.frase.contains("consumido"),
            "{}",
            r.frase
        );
    }
}

#[tokio::test]
async fn rejeitada_diz_o_codigo_e_o_motivo_e_nao_manda_reenviar() {
    let (xml, chave) = nfe(3, 2);
    // rejeicao da NF-e dentro do lote processado
    let p = prot(
        &chave,
        "225",
        "Rejeicao: Falha no Schema XML do lote de NFe",
        "",
    );
    let (c, _) = cliente(Falso::novo(vec![(AUT, ok(AUT, &ret_envi_sinc(&p)))]));
    let r = c.autorizar(&xml).await.unwrap();
    assert_eq!(
        r.desfecho,
        Desfecho::Rejeitada {
            cstat: "225".into(),
            motivo: "Rejeicao: Falha no Schema XML do lote de NFe".into()
        }
    );
    assert_eq!(
        r.frase,
        "Rejeitada (225): Rejeicao: Falha no Schema XML do lote de NFe"
    );
    assert!(!r.pode_reenviar() && !r.deve_consultar() && r.nfe_proc().is_none());
    assert!(r.xml_retorno.contains("<cStat>225</cStat>"));

    // rejeicao do lote inteiro (sem protNFe)
    let ret = ret_envi("215", "Rejeicao: Falha no schema XML", "");
    let (c, _) = cliente(Falso::novo(vec![(AUT, ok(AUT, &ret))]));
    let r = c.autorizar(&xml).await.unwrap();
    assert!(matches!(&r.desfecho, Desfecho::Rejeitada { cstat, .. } if cstat == "215"));
}

#[tokio::test]
async fn servico_paralisado_108_e_109_nada_foi_processado_e_pode_reenviar() {
    for cs in ["108", "109"] {
        let (xml, _) = nfe(4, 2);
        let ret = ret_envi(cs, "Servico Paralisado Momentaneamente (curto prazo)", "");
        let f = Falso::novo(vec![(AUT, ok(AUT, &ret))]);
        let (c, _) = cliente(f.clone());
        let r = c.autorizar(&xml).await.unwrap();
        assert!(matches!(&r.desfecho, Desfecho::Indisponivel { cstat, .. } if cstat == cs));
        assert!(r.pode_reenviar() && !r.autorizada());
        assert_eq!(
            f.chamadas().len(),
            1,
            "o 108 e' resposta: nao ha o que consultar"
        );
    }
}

// ------------------------------------------------------------------------------------------------ assincrono

#[tokio::test]
async fn assincrono_recibo_105_105_e_depois_o_protocolo() {
    let (xml, chave) = nfe(5, 2);
    let p = prot_ok(&chave);
    let f = Falso::novo(vec![
        (AUT, ok(AUT, &ret_envi_recibo("351000000000042", 4))),
        (
            RET,
            ok(
                RET,
                &ret_reci("351000000000042", "105", "Lote em processamento", ""),
            ),
        ),
        (
            RET,
            ok(
                RET,
                &ret_reci("351000000000042", "105", "Lote em processamento", ""),
            ),
        ),
        (
            RET,
            ok(
                RET,
                &ret_reci("351000000000042", "104", "Lote processado", &p),
            ),
        ),
    ]);
    let (c, pausas) = cliente(f.clone());
    let r = c.autorizar_assincrono(&xml).await.unwrap();
    assert!(r.autorizada(), "{r:?}");
    assert_eq!(r.recibo.as_deref(), Some("351000000000042"));
    assert_eq!(r.nfe_proc(), Some(nfe_proc(&xml, &p).as_str()));
    let ch = f.chamadas();
    assert!(ch[0].corpo.contains("<indSinc>0</indSinc>"));
    assert!(
        ch[1].corpo.contains("<consReciNFe")
            && ch[1].corpo.contains("<nRec>351000000000042</nRec>")
    );
    assert_eq!(
        ch[1].url,
        "https://homologacao.nfe.fazenda.sp.gov.br/ws/nferetautorizacao4.asmx"
    );
    // espera o tempo medio do recibo (4 s) e depois o intervalo entre as consultas (3 s)
    assert_eq!(
        *pausas.lock().unwrap(),
        [
            Duration::from_secs(4),
            Duration::from_secs(3),
            Duration::from_secs(3)
        ]
    );
}

#[tokio::test]
async fn lote_que_nao_sai_do_105_fica_em_processamento_e_o_retorno_acha_depois() {
    let (xml, chave) = nfe(6, 2);
    let em_proc = || {
        ok(
            RET,
            &ret_reci("351000000000043", "105", "Lote em processamento", ""),
        )
    };
    let f = Falso::novo(vec![
        (AUT, ok(AUT, &ret_envi_recibo("351000000000043", 1))),
        (RET, em_proc()),
        (RET, em_proc()),
        (RET, em_proc()),
    ]);
    let mut cfg = Config::novo("SP", Homologacao);
    cfg.tentativas = 3;
    cfg.intervalo = Duration::from_secs(2);
    let c = ClienteSefaz::com_transporte(cfg, f.clone())
        .unwrap()
        .com_relogio(|| Utc.with_ymd_and_hms(2026, 10, 3, 12, 0, 0).unwrap())
        .com_dormir(|_| Box::pin(async {}));
    let r = c.autorizar_assincrono(&xml).await.unwrap();
    assert_eq!(
        r.desfecho,
        Desfecho::EmProcessamento {
            recibo: "351000000000043".into()
        }
    );
    assert!(r.deve_consultar() && !r.pode_reenviar() && !r.autorizada());
    assert!(r.frase.contains("nao reenvie"), "{}", r.frase);
    assert_eq!(f.chamadas().len(), 4);

    // mais tarde: o retorno do mesmo recibo acha a nota
    let f2 = Falso::novo(vec![(
        RET,
        ok(
            RET,
            &ret_reci(
                "351000000000043",
                "104",
                "Lote processado",
                &prot_ok(&chave),
            ),
        ),
    )]);
    let (c2, pausas) = cliente(f2);
    let r2 = c2.retorno("351000000000043", &xml).await.unwrap();
    assert!(r2.autorizada());
    assert!(
        pausas.lock().unwrap().is_empty(),
        "sem tMed nao ha espera inicial"
    );
}

#[tokio::test]
async fn recibo_nao_localizado_106_cai_na_consulta_pela_chave() {
    let (xml, chave) = nfe(7, 2);
    let f = Falso::novo(vec![
        (
            RET,
            ok(
                RET,
                &ret_reci("351000000000044", "106", "Lote nao localizado", ""),
            ),
        ),
        (
            CON,
            ok(
                CON,
                &ret_sit(&chave, "100", "Autorizado o uso da NF-e", &prot_ok(&chave)),
            ),
        ),
    ]);
    let (c, _) = cliente(f);
    let r = c.retorno("351000000000044", &xml).await.unwrap();
    assert!(r.autorizada() && r.recuperada, "{r:?}");
}

#[tokio::test]
async fn assincrono_so_ate_o_corte_da_nt_2023_002_por_ambiente_e_depois_nao_chama_a_rede() {
    // homologacao: corte em 01/02/2027; um dia antes ainda sai
    let (xml, chave) = nfe(8, 2);
    let f = Falso::novo(vec![
        (AUT, ok(AUT, &ret_envi_recibo("351000000000045", 1))),
        (
            RET,
            ok(
                RET,
                &ret_reci(
                    "351000000000045",
                    "104",
                    "Lote processado",
                    &prot_ok(&chave),
                ),
            ),
        ),
    ]);
    let (c, _) = cliente_em("SP", Homologacao, (2027, 1, 31), f.clone());
    assert!(c.autorizar_assincrono(&xml).await.unwrap().autorizada());
    assert!(f.chamadas()[0].corpo.contains("<indSinc>0</indSinc>"));

    for (amb, dia) in [
        (Homologacao, (2027, 2, 1)),
        (Homologacao, (2027, 6, 1)),
        (Producao, (2027, 4, 5)),
    ] {
        let (x, _) = nfe(8, if amb == Producao { 1 } else { 2 });
        let f = Falso::novo(vec![]);
        let (c, _) = cliente_em("SP", amb, dia, f.clone());
        let e = c.autorizar_assincrono(&x).await.unwrap_err();
        assert_eq!(e.campo, "indSinc");
        assert!(
            e.mensagem.contains("129") && e.mensagem.contains("NT 2023.002 v1.02"),
            "{e}"
        );
        assert!(f.chamadas().is_empty(), "depois do corte nao ha chamada");
    }
    // a data do corte aparece na mensagem
    let (x, _) = nfe(8, 1);
    let (c, _) = cliente_em("SP", Producao, (2027, 4, 5), Falso::novo(vec![]));
    assert!(c
        .autorizar_assincrono(&x)
        .await
        .unwrap_err()
        .mensagem
        .contains("05/04/2027"));
    let (x, _) = nfe(8, 2);
    let (c, _) = cliente_em("SP", Homologacao, (2027, 2, 1), Falso::novo(vec![]));
    assert!(c
        .autorizar_assincrono(&x)
        .await
        .unwrap_err()
        .mensagem
        .contains("01/02/2027"));
}

// ------------------------------------------------------------------------------------------------ consulta

#[tokio::test]
async fn consulta_por_situacao() {
    let (xml, chave) = nfe(23, 2);
    let p = prot_ok(&chave);
    // 100 com a NF-e: nfeProc montado; sem a NF-e: autorizada, sem o nfeProc
    let (c, _) = cliente(Falso::novo(vec![(
        CON,
        ok(CON, &ret_sit(&chave, "100", "Autorizado", &p)),
    )]));
    let r = c.consultar(&chave, Some(&xml)).await.unwrap();
    assert_eq!(r.nfe_proc(), Some(nfe_proc(&xml, &p).as_str()));
    assert!(!r.recuperada);
    let (c, _) = cliente(Falso::novo(vec![(
        CON,
        ok(
            CON,
            &ret_sit(
                &chave,
                "150",
                "Autorizado fora do prazo",
                &p.replace("<cStat>100", "<cStat>150"),
            ),
        ),
    )]));
    let r = c.consultar(&chave, None).await.unwrap();
    assert!(r.autorizada() && r.nfe_proc().is_none());

    // 101 e 151: cancelada
    for cs in ["101", "151"] {
        let (c, _) = cliente(Falso::novo(vec![(
            CON,
            ok(
                CON,
                &ret_sit(&chave, cs, "Cancelamento de NF-e homologado", &p),
            ),
        )]));
        let r = c.consultar(&chave, Some(&xml)).await.unwrap();
        assert!(
            matches!(&r.desfecho, Desfecho::Cancelada { cstat, .. } if cstat == cs),
            "{r:?}"
        );
    }
    // denegada
    let pd = prot(&chave, "110", "Uso Denegado", NPROT);
    let (c, _) = cliente(Falso::novo(vec![(
        CON,
        ok(CON, &ret_sit(&chave, "110", "Uso Denegado", &pd)),
    )]));
    let r = c.consultar(&chave, Some(&xml)).await.unwrap();
    assert!(matches!(&r.desfecho, Desfecho::Denegada { protocolo, .. } if protocolo == NPROT));
    // 217, 124, 108 e uma rejeicao qualquer
    let casos = [
        ("217", Desfecho::NaoRecebida),
        (
            "124",
            Desfecho::SoEpec {
                motivo: "EPEC Autorizado".into(),
            },
        ),
        (
            "108",
            Desfecho::Indisponivel {
                cstat: "108".into(),
                motivo: "EPEC Autorizado".into(),
            },
        ),
        (
            "236",
            Desfecho::ConsultaRecusada {
                cstat: "236".into(),
                motivo: "EPEC Autorizado".into(),
            },
        ),
    ];
    for (cs, esperado) in casos {
        let (c, _) = cliente(Falso::novo(vec![(
            CON,
            ok(CON, &ret_sit(&chave, cs, "EPEC Autorizado", "")),
        )]));
        assert_eq!(
            c.consultar(&chave, None).await.unwrap().desfecho,
            esperado,
            "{cs}"
        );
    }
}

#[tokio::test]
async fn consulta_vai_ao_autorizador_da_uf_da_chave_e_confere_a_nfe_dada() {
    // chave do Rio Grande do Sul: a consulta vai ao RS, mesmo com o cliente de SP
    let rs = chave_dfe::montar(&chave_dfe::Montar {
        cuf: 43,
        aamm: "2610",
        emitente: "11222333000181",
        numero: 5,
        ..Default::default()
    })
    .unwrap();
    let f = Falso::novo(vec![(CON, ok(CON, &ret_sit(&rs, "217", "x", "")))]);
    let (c, _) = cliente(f.clone());
    let r = c.consultar(&rs, None).await.unwrap();
    assert_eq!(r.desfecho, Desfecho::NaoRecebida);
    assert_eq!(
        f.chamadas()[0].url,
        tabela::url("RS", Homologacao, Servico::ConsultaProtocolo).unwrap()
    );
    assert!(
        f.chamadas()[0].url.contains("sefazrs.rs.gov.br"),
        "{}",
        f.chamadas()[0].url
    );

    // NF-e dada de outra chave
    let (xml, _) = nfe(24, 2);
    let (c, _) = cliente(Falso::novo(vec![]));
    let e = c.consultar(&rs, Some(&xml)).await.unwrap_err();
    assert_eq!(e.campo, "chNFe");
    assert_eq!(c.consultar("123", None).await.unwrap_err().campo, "chNFe");
}

// ------------------------------------------------------------------------------------------------ status

#[tokio::test]
async fn status_do_servico() {
    let f = Falso::novo(vec![(
        STA,
        ok(
            STA,
            &ret_status("107", "Servico em Operacao", "<tMed>2</tMed>"),
        ),
    )]);
    let (c, _) = cliente(f.clone());
    let s = c.status().await.unwrap();
    assert!(s.em_operacao());
    assert_eq!((s.cstat.as_str(), s.tempo_medio), ("107", Some(2)));
    assert_eq!(s.frase, "Servico da SEFAZ em operacao.");
    let ch = f.chamadas();
    assert_eq!(
        ch[0].url,
        "https://homologacao.nfe.fazenda.sp.gov.br/ws/nfestatusservico4.asmx"
    );
    assert!(ch[0].corpo.contains("<cUF>35</cUF><xServ>STATUS</xServ>"));

    let (c, _) = cliente(Falso::novo(vec![(
        STA,
        ok(
            STA,
            &ret_status(
                "108",
                "Servico Paralisado Momentaneamente (curto prazo)",
                "<dhRetorno>2026-10-03T13:00:00</dhRetorno>",
            ),
        ),
    )]));
    let s = c.status().await.unwrap();
    assert_eq!(s.situacao, SituacaoServico::Paralisado);
    assert_eq!(s.retorno_previsto, "2026-10-03T13:00:00");
    assert!(!s.em_operacao());

    for (resp, marca) in [
        (fault("Falha"), "falha SOAP"),
        (tempo(), "tempo esgotado"),
        (Resp::R(503, vec![]), "503: servico indisponivel"),
    ] {
        let (c, _) = cliente(Falso::novo(vec![(STA, resp)]));
        let s = c.status().await.unwrap();
        assert_eq!(s.situacao, SituacaoServico::SemResposta);
        assert!(s.detalhe.contains(marca), "{marca}: {}", s.detalhe);
    }
}

// ------------------------------------------------------------------------------------------------ conferencias antes da rede

#[tokio::test]
async fn a_nfe_que_nao_serve_nao_chega_a_rede() {
    let (ok_xml, _) = nfe(31, 2);
    let sem_ass = {
        let i = ok_xml.find("<Signature").unwrap();
        format!("{}</NFe>", &ok_xml[..i])
    };
    let (prod, _) = nfe(31, 1);
    let mut p = pedido(31, 2);
    // SP e' da SVC-AN (tpEmis 6): o tpEmis 7 e' da SVC-RS e a SVC-AN o rejeitaria com 713
    p["tp_emis"] = json!(7);
    p["dh_cont"] = json!("2026-10-03T08:30:00-03:00");
    p["x_just"] = json!("Falha na comunicacao com a SEFAZ de origem");
    let (svc, _) = nfe_de(&p);
    let casos: Vec<(&str, String, &str)> = vec![
        ("sem assinatura", sem_ass, "Signature"),
        ("ambiente", prod, "tpAmb"),
        ("lixo", "nao e xml".to_string(), "NFe"),
    ];
    for (nome, x, campo) in casos {
        let f = Falso::novo(vec![]);
        let (c, _) = cliente(f.clone());
        let e = c.autorizar(&x).await.unwrap_err();
        assert_eq!(e.campo, campo, "{nome}: {e}");
        assert!(f.chamadas().is_empty(), "{nome}");
    }
    // UF diferente: nota de SP em cliente de MG
    let f = Falso::novo(vec![]);
    let (c, _) = cliente_em("MG", Homologacao, (2026, 10, 3), f.clone());
    let e = c.autorizar(&ok_xml).await.unwrap_err();
    assert_eq!(e.campo, "cUF");
    assert!(
        e.mensagem.contains("SP") && e.mensagem.contains("MG"),
        "{e}"
    );
    assert!(f.chamadas().is_empty());
    // contingencia SVC com o tpEmis da SVC errada para a UF: recusada antes da rede (a SVC certa esta em nfe_sefaz_svc)
    let f = Falso::novo(vec![]);
    let (c, _) = cliente(f.clone());
    let e = c.autorizar(&svc).await.unwrap_err();
    assert_eq!(e.campo, "tpEmis");
    assert!(f.chamadas().is_empty());
    // recibo fora da forma
    let (c, _) = cliente(Falso::novo(vec![]));
    assert_eq!(c.retorno("123", &ok_xml).await.unwrap_err().campo, "nRec");
}

#[test]
fn endereco_que_sobrepoe_a_tabela_so_com_https_ou_http_no_proprio_computador() {
    let mut cfg = Config::novo("SP", Homologacao);
    cfg.urls
        .insert(Servico::Autorizacao, "http://127.0.0.1:4050/aut".into());
    assert!(ClienteSefaz::com_transporte(cfg, Falso::novo(vec![])).is_ok());
    for ruim in [
        "http://exemplo.gov.br/aut",
        format!("{}://localhost@exemplo.gov.br/", "http").as_str(),
        "ftp://127.0.0.1/",
        "aut",
    ] {
        let mut cfg = Config::novo("SP", Homologacao);
        cfg.urls.insert(Servico::Autorizacao, ruim.into());
        let e = ClienteSefaz::com_transporte(cfg, Falso::novo(vec![]))
            .err()
            .unwrap();
        assert_eq!(e.campo, "urls", "{ruim}");
    }
    assert_eq!(
        ClienteSefaz::com_transporte(Config::novo("XX", Homologacao), Falso::novo(vec![]))
            .err()
            .unwrap()
            .campo,
        "uf"
    );
}

#[tokio::test]
async fn o_endereco_de_teste_vale_so_para_o_servico_dado() {
    let (xml, chave) = nfe(32, 2);
    let mut cfg = Config::novo("SP", Homologacao);
    cfg.urls
        .insert(Servico::Autorizacao, "http://127.0.0.1:4050/aut".into());
    let f = Falso::novo(vec![
        (AUT, ok(AUT, &ret_envi_sinc(&prot_ok(&chave)))),
        (STA, ok(STA, &ret_status("107", "Servico em Operacao", ""))),
    ]);
    let c = ClienteSefaz::com_transporte(cfg, f.clone()).unwrap();
    assert!(c.autorizar(&xml).await.unwrap().autorizada());
    c.status().await.unwrap();
    let ch = f.chamadas();
    assert_eq!(ch[0].url, "http://127.0.0.1:4050/aut");
    assert!(ch[1]
        .url
        .starts_with("https://homologacao.nfe.fazenda.sp.gov.br/"));
}

#[tokio::test]
async fn retorno_do_lote_sem_cstat_cai_na_consulta() {
    let (xml, chave) = nfe(33, 2);
    let f = Falso::novo(vec![
        (AUT, ok(AUT, "<retEnviNFe xmlns=\"http://www.portalfiscal.inf.br/nfe\" versao=\"4.00\"></retEnviNFe>")),
        (CON, ok(CON, &ret_sit(&chave, "100", "Autorizado", &prot_ok(&chave)))),
    ]);
    let (c, _) = cliente(f);
    let r = c.autorizar(&xml).await.unwrap();
    assert!(r.autorizada() && r.recuperada);
}
