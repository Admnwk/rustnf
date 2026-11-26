// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Contingencia SVC: a UF -> SVC-AN ou SVC-RS pelo Ato COTEPE/ICMS 39/12 (redacao do Ato 110/22), o `tpEmis` 6 ou 7 da
//! nota, o envio, a consulta e os eventos no endereco da SVC, e o que a SVC nao lista.

mod nfe_sefaz_comum;

use nfe_sefaz_comum::eventos::*;
use nfe_sefaz_comum::*;
use rustnf::nfe::sefaz::svc::{svc_da_uf, Svc, UF_DA_SVC_AN, UF_DA_SVC_RS};
use rustnf::nfe::sefaz::{tabela, Desfecho, DesfechoEvento, Servico};
use rustnf::nfe::vigencias::Ambiente::{Homologacao, Producao};

const AUT_SVC_AN: &str =
    "https://hom.sefazvirtual.fazenda.gov.br/NFeAutorizacao4/NFeAutorizacao4.asmx";
const AUT_SVC_RS: &str =
    "https://nfe-homologacao.svrs.rs.gov.br/ws/NfeAutorizacao/NFeAutorizacao4.asmx";

// ------------------------------------------------------------------------------------------------ o Ato

#[test]
fn cada_uf_vai_para_a_svc_que_o_ato_cotepe_39_12_diz() {
    // art. 1o, incisos I e II, na redacao do Ato COTEPE/ICMS 110/22 (Confaz, lido em 25/11/2026)
    let an = [
        "AC", "AL", "AP", "CE", "ES", "MG", "PA", "PB", "PI", "RJ", "RS", "RN", "RO", "RR", "SC",
        "SE", "SP", "TO", "DF",
    ];
    let rs = ["AM", "BA", "GO", "MA", "MT", "MS", "PE", "PR"];
    assert_eq!(an.len() + rs.len(), 27);
    for uf in an {
        assert_eq!(svc_da_uf(uf).unwrap(), Svc::An, "{uf}");
    }
    for uf in rs {
        assert_eq!(svc_da_uf(uf).unwrap(), Svc::Rs, "{uf}");
    }
    // as listas publicas da crate sao as mesmas
    let mut a: Vec<&str> = UF_DA_SVC_AN.to_vec();
    a.sort_unstable();
    let mut b: Vec<&str> = an.to_vec();
    b.sort_unstable();
    assert_eq!(a, b);
    assert_eq!(UF_DA_SVC_RS.len(), 8);
    // por codigo IBGE tambem; UF desconhecida e' erro, nao cai em outra SVC
    assert_eq!(svc_da_uf("35").unwrap(), Svc::An);
    assert_eq!(svc_da_uf("41").unwrap(), Svc::Rs);
    assert!(svc_da_uf("XX").is_err() && svc_da_uf("").is_err());
}

#[test]
fn o_manual_limita_a_svc_e_a_inutilizacao_nao_e_oferecida_mesmo_listada_no_portal() {
    assert_eq!((Svc::An.tp_emis(), Svc::Rs.tp_emis()), (6, 7));
    assert_eq!(
        (Svc::An.autorizador(), Svc::Rs.autorizador()),
        ("SVC-AN", "SVC-RS")
    );
    assert_eq!(
        (Svc::do_tp_emis(6), Svc::do_tp_emis(7), Svc::do_tp_emis(1)),
        (Some(Svc::An), Some(Svc::Rs), None)
    );
    for amb in [Homologacao, Producao] {
        for s in Servico::TODOS {
            assert!(Svc::An.oferece(amb, s), "SVC-AN {s:?} {amb:?}");
            assert!(Svc::Rs.oferece(amb, s), "SVC-RS {s:?} {amb:?}");
        }
        assert!(
            Svc::An.oferece(amb, Servico::RecepcaoEvento)
                && Svc::Rs.oferece(amb, Servico::RecepcaoEvento)
        );
        // MOC 7.00, Anexo III, 2.1.3.4, item e: a SVC nao oferece a inutilizacao; o Portal lista NfeInutilizacao so' na SVC-AN
        assert!(tabela::url_do_autorizador("SVC-AN", amb, Servico::Inutilizacao).is_ok());
        assert!(!Svc::An.oferece(amb, Servico::Inutilizacao));
        assert!(!Svc::Rs.oferece(amb, Servico::Inutilizacao));
    }
    // a SVC-RS nem endereco de inutilizacao tem: nada se inventa
    let e = tabela::url_do_autorizador("SVC-RS", Homologacao, Servico::Inutilizacao).unwrap_err();
    assert!(
        e.mensagem.contains("NfeInutilizacao") && e.mensagem.contains("SVC-RS"),
        "{e}"
    );
    assert_eq!(
        tabela::url_do_autorizador("SVC-AN", Producao, Servico::RecepcaoEvento).unwrap(),
        "https://www.sefazvirtual.fazenda.gov.br/NFeRecepcaoEvento4/NFeRecepcaoEvento4.asmx"
    );
    // dos eventos, so' o cancelamento (item c); carta e os outros nao (item d)
    assert!(Svc::atende_evento("110111"));
    for outro in ["110110", "110140", "112110", "110001", "211110"] {
        assert!(!Svc::atende_evento(outro), "{outro}");
    }
}

#[test]
fn a_montagem_exige_dhcont_e_xjust_na_svc() {
    use rustnf::nfe::montagem::{montar, nota_de_json};
    for tp in [6, 7] {
        let mut p = pedido(80, 2);
        p["tp_emis"] = serde_json::json!(tp);
        let e = montar(&nota_de_json(&p).unwrap()).unwrap_err();
        assert_eq!(e.campo, "dhCont", "{tp}");
        // o Anexo I nao tem rejeicao para isso nos tpEmis 6 e 7 (557 vale so' para 2, 4, 5 e 9)
        assert_eq!(e.regra, "", "{tp}");
        assert!(e.mensagem.contains("Anexo III"), "{e}");
        // so' um dos dois tambem nao serve
        p["dh_cont"] = serde_json::json!("2026-10-03T08:30:00-03:00");
        assert!(montar(&nota_de_json(&p).unwrap()).is_err());
        p["x_just"] = serde_json::json!("Falha na comunicacao com a SEFAZ de origem");
        assert!(montar(&nota_de_json(&p).unwrap()).is_ok());
    }
}

// ------------------------------------------------------------------------------------------------ a nota na SVC

#[tokio::test]
async fn nota_com_tpemis_6_de_sp_vai_ao_endereco_da_svc_an() {
    let (xml, chave) = nfe_em_contingencia(60, 6);
    assert_eq!(&chave[34..35], "6");
    let prot = prot_ok(&chave);
    let f = Falso::novo(vec![(AUT, ok(AUT, &ret_envi_sinc(&prot)))]);
    let (c, _) = cliente(f.clone());
    let r = c.autorizar(&xml).await.unwrap();
    assert!(r.autorizada(), "{r:?}");
    assert_eq!(f.chamadas()[0].url, AUT_SVC_AN);
    assert!(r.nfe_proc().unwrap().contains(&xml));
}

#[tokio::test]
async fn nota_com_tpemis_7_do_parana_vai_ao_endereco_da_svc_rs() {
    let (xml, chave) = nfe_do_parana(61, 7);
    assert_eq!(&chave[34..35], "7");
    let prot = prot_ok(&chave);
    let f = Falso::novo(vec![(AUT, ok(AUT, &ret_envi_sinc(&prot)))]);
    let (c, _) = cliente_em("PR", Homologacao, (2026, 10, 3), f.clone());
    let r = c.autorizar(&xml).await.unwrap();
    assert!(r.autorizada(), "{r:?}");
    assert_eq!(f.chamadas()[0].url, AUT_SVC_RS);
}

#[tokio::test]
async fn tpemis_da_svc_errada_para_a_uf_e_recusado_antes_da_rede_713() {
    // SP e' da SVC-AN (6): a nota de SP com 7 seria rejeitada com 713
    let (sp7, _) = nfe_em_contingencia(62, 7);
    let f = Falso::novo(vec![]);
    let (c, _) = cliente(f.clone());
    let e = c.autorizar(&sp7).await.unwrap_err();
    assert_eq!(e.campo, "tpEmis");
    assert!(
        e.mensagem.contains("713")
            && e.mensagem.contains("SVC-AN")
            && e.mensagem.contains("Ato COTEPE/ICMS 39/12"),
        "{e}"
    );
    // PR e' da SVC-RS (7): a nota de PR com 6 idem
    let (pr6, _) = nfe_do_parana(63, 6);
    let (c, _) = cliente_em("PR", Homologacao, (2026, 10, 3), f.clone());
    let e = c.autorizar(&pr6).await.unwrap_err();
    assert!(
        e.mensagem.contains("713") && e.mensagem.contains("SVC-RS"),
        "{e}"
    );
    assert!(f.chamadas().is_empty());
}

#[tokio::test]
async fn resposta_perdida_na_svc_consulta_a_chave_na_svc_nao_na_uf() {
    let (xml, chave) = nfe_em_contingencia(64, 6);
    let prot = prot_ok(&chave);
    let f = Falso::novo(vec![
        (AUT, tempo()),
        (
            CON,
            ok(
                CON,
                &ret_sit(&chave, "100", "Autorizado o uso da NF-e", &prot),
            ),
        ),
    ]);
    let (c, _) = cliente(f.clone());
    let r = c.autorizar(&xml).await.unwrap();
    assert!(r.autorizada() && r.recuperada, "{r:?}");
    let ch = f.chamadas();
    assert_eq!(ch[0].url, AUT_SVC_AN);
    assert_eq!(
        ch[1].url,
        "https://hom.sefazvirtual.fazenda.gov.br/NFeConsultaProtocolo4/NFeConsultaProtocolo4.asmx"
    );
    // a consulta solta, so' pela chave, tambem vai a SVC
    let f = Falso::novo(vec![(
        CON,
        ok(
            CON,
            &ret_sit(&chave, "100", "Autorizado o uso da NF-e", &prot),
        ),
    )]);
    let (c, _) = cliente(f.clone());
    let r = c.consultar(&chave, Some(&xml)).await.unwrap();
    assert!(matches!(r.desfecho, Desfecho::Autorizada { .. }));
    assert!(f.chamadas()[0].url.contains("sefazvirtual"));
}

#[tokio::test]
async fn o_recibo_da_nota_da_svc_tambem_e_consultado_na_svc() {
    let (xml, chave) = nfe_em_contingencia(65, 6);
    let prot = prot_ok(&chave);
    let f = Falso::novo(vec![
        (AUT, ok(AUT, &ret_envi_recibo("351000000000042", 1))),
        (
            RET,
            ok(
                RET,
                &ret_reci("351000000000042", "104", "Lote processado", &prot),
            ),
        ),
    ]);
    let (c, _) = cliente(f.clone());
    let r = c.autorizar_assincrono(&xml).await.unwrap();
    assert!(r.autorizada(), "{r:?}");
    assert_eq!(
        f.chamadas()[1].url,
        "https://hom.sefazvirtual.fazenda.gov.br/NFeRetAutorizacao4/NFeRetAutorizacao4.asmx"
    );
}

// ------------------------------------------------------------------------------------------------ eventos da nota na SVC

#[tokio::test]
async fn cancelamento_da_nota_da_svc_vai_a_svc_e_a_carta_vai_a_sefaz_de_origem() {
    let (_, chave) = nfe_em_contingencia(66, 6);
    let f = Falso::novo(vec![
        (
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
        ),
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
                    NPROT_EVT,
                ),
            ),
        ),
    ]);
    let (c, _) = cliente(f.clone());
    let a = c
        .cancelar(
            &certificado(),
            &chave,
            NPROT,
            "Cancelamento da nota de contingencia",
        )
        .await
        .unwrap();
    let b = c
        .corrigir(
            &certificado(),
            &chave,
            "Corrigir o endereco de entrega da nota",
            1,
        )
        .await
        .unwrap();
    assert!(a.registrado() && b.registrado());
    // MOC 7.00, Anexo III, 2.1.3.4: cancelamento so' da nota autorizada na SVC; carta nao e' servico da SVC
    assert_eq!(
        (a.autorizador.as_str(), b.autorizador.as_str()),
        ("SVC-AN", "SP")
    );
    let ch = f.chamadas();
    assert_eq!(
        ch[0].url,
        "https://hom.sefazvirtual.fazenda.gov.br/NFeRecepcaoEvento4/NFeRecepcaoEvento4.asmx"
    );
    assert_eq!(
        ch[1].url,
        "https://homologacao.nfe.fazenda.sp.gov.br/ws/nferecepcaoevento4.asmx"
    );
    // a nota normal da mesma UF: os dois vao a UF
    let (_, normal) = nfe(67, 2);
    let f = Falso::novo(vec![(
        EVT,
        ok(EVT, &ret_lote(&normal, "110111", 1, "135", "ok", NPROT_EVT)),
    )]);
    let (c, _) = cliente(f.clone());
    let r = c
        .cancelar(
            &certificado(),
            &normal,
            NPROT,
            "Cancelamento da nota normal",
        )
        .await
        .unwrap();
    assert_eq!(r.autorizador, "SP");
    assert!(f.chamadas()[0].url.contains("fazenda.sp.gov.br"));
}

#[tokio::test]
async fn carta_da_nota_da_svc_sem_resposta_consulta_na_sefaz_de_origem_onde_ela_foi() {
    let (_, chave) = nfe_em_contingencia(69, 6);
    let miolo = format!(
        "{}{}",
        prot_ok(&chave),
        proc_na_consulta(&chave, "110110", 1, "135", NPROT_EVT)
    );
    let f = Falso::novo(vec![
        (EVT, tempo()),
        (
            CON,
            ok(
                CON,
                &ret_sit(&chave, "100", "Autorizado o uso da NF-e", &miolo),
            ),
        ),
    ]);
    let (c, _) = cliente(f.clone());
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
    let ch = f.chamadas();
    assert!(
        ch[0].url.contains("fazenda.sp.gov.br") && ch[1].url.contains("fazenda.sp.gov.br"),
        "{ch:?}"
    );
}

#[tokio::test]
async fn status_da_svc_vai_ao_endereco_da_svc_da_uf() {
    let f = Falso::novo(vec![(
        STA,
        ok(STA, &ret_status("107", "Servico SVC em Operacao", "")),
    )]);
    let (c, _) = cliente(f.clone());
    let s = c.status_svc().await.unwrap();
    assert!(s.em_operacao());
    assert_eq!(
        f.chamadas()[0].url,
        "https://hom.sefazvirtual.fazenda.gov.br/NFeStatusServico4/NFeStatusServico4.asmx"
    );
    // 114: a SEFAZ de origem desabilitou a SVC
    let f = Falso::novo(vec![(
        STA,
        ok(
            STA,
            &ret_status("114", "SVC desabilitada pela SEFAZ Origem", ""),
        ),
    )]);
    let (c, _) = cliente(f);
    let s = c.status_svc().await.unwrap();
    assert!(!s.em_operacao());
    assert!(s.frase.contains("114"), "{}", s.frase);
    // o status comum continua indo a UF
    let f = Falso::novo(vec![(
        STA,
        ok(STA, &ret_status("107", "Servico em Operacao", "")),
    )]);
    let (c, _) = cliente(f.clone());
    c.status().await.unwrap();
    assert!(f.chamadas()[0].url.contains("fazenda.sp.gov.br"));
}

#[tokio::test]
async fn cancelamento_da_nota_da_svc_sem_resposta_consulta_na_svc_e_acha_o_evento() {
    let (_, chave) = nfe_do_parana(68, 7);
    let miolo = format!(
        "{}{}",
        prot_ok(&chave),
        proc_na_consulta(&chave, "110111", 1, "135", NPROT_EVT)
    );
    let f = Falso::novo(vec![
        (EVT, tempo()),
        (
            CON,
            ok(
                CON,
                &ret_sit(&chave, "101", "Cancelamento de NF-e homologado", &miolo),
            ),
        ),
    ]);
    let (c, _) = cliente_em("PR", Homologacao, (2026, 10, 3), f.clone());
    let r = c
        .cancelar(
            &certificado(),
            &chave,
            NPROT,
            "Cancelamento da nota de contingencia",
        )
        .await
        .unwrap();
    assert!(matches!(r.desfecho, DesfechoEvento::Registrado { .. }) && r.recuperado);
    assert_eq!(r.autorizador, "SVC-RS");
    let ch = f.chamadas();
    assert!(ch[0]
        .url
        .contains("nfe-homologacao.svrs.rs.gov.br/ws/recepcaoevento"));
    assert!(ch[1]
        .url
        .contains("nfe-homologacao.svrs.rs.gov.br/ws/NfeConsulta"));
}

#[test]
fn a_pagina_de_homologacao_poe_o_pi_na_svc_rs_e_o_ato_na_svc_an() {
    use rustnf::nfe::sefaz::svc::homologacao_diverge_do_ato;
    // o Ato (e a pagina de producao) mandam o PI para a SVC-AN; a pagina de homologacao lista o PI na SVC-RS
    assert_eq!(svc_da_uf("PI").unwrap(), Svc::An);
    assert!(homologacao_diverge_do_ato("PI", Homologacao, 7));
    assert!(!homologacao_diverge_do_ato("PI", Producao, 7));
    assert!(!homologacao_diverge_do_ato("PI", Homologacao, 6));
    assert!(!homologacao_diverge_do_ato("CE", Homologacao, 7));
}
