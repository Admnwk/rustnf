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
fn a_tabela_do_portal_atende_as_duas_svc_e_a_svc_rs_nao_lista_a_inutilizacao() {
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
        assert!(Svc::An.oferece(amb, Servico::Inutilizacao));
        assert!(
            !Svc::Rs.oferece(amb, Servico::Inutilizacao),
            "a SVC-RS nao lista NfeInutilizacao"
        );
    }
    // o que a SVC nao lista nao tem endereco: nada se inventa
    let e = tabela::url_do_autorizador("SVC-RS", Homologacao, Servico::Inutilizacao).unwrap_err();
    assert!(
        e.mensagem.contains("NfeInutilizacao") && e.mensagem.contains("SVC-RS"),
        "{e}"
    );
    assert_eq!(
        tabela::url_do_autorizador("SVC-AN", Producao, Servico::RecepcaoEvento).unwrap(),
        "https://www.sefazvirtual.fazenda.gov.br/NFeRecepcaoEvento4/NFeRecepcaoEvento4.asmx"
    );
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
async fn cancelamento_e_carta_da_nota_da_svc_vao_a_svc_da_chave() {
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
    assert_eq!(
        (a.autorizador.as_str(), b.autorizador.as_str()),
        ("SVC-AN", "SVC-AN")
    );
    for ch in f.chamadas() {
        assert_eq!(
            ch.url,
            "https://hom.sefazvirtual.fazenda.gov.br/NFeRecepcaoEvento4/NFeRecepcaoEvento4.asmx"
        );
    }
    // a nota normal da mesma UF continua indo a UF
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
