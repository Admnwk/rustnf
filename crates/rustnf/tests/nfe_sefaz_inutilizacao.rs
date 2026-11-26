// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Inutilizacao: o pedido (Id, faixa, ano, justificativa), os desfechos (102, 563, 998, HTTP 404, rejeicao, sem
//! resposta) e o servico que acabou em AC, BA e SP em 11/11/2026 (a UF sem o servico nao chama a rede).

mod nfe_sefaz_comum;

use chrono::NaiveDate;
use nfe_sefaz_comum::eventos::*;
use nfe_sefaz_comum::*;
use rustnf::nfe::sefaz::inutilizacao::{self, montar, Inutilizacao};
use rustnf::nfe::sefaz::{DesfechoInutilizacao, PedidoInutilizar};
use rustnf::nfe::vigencias::Ambiente::{self, Homologacao, Producao};

const CNPJ: &str = "11222333000181";
const JUST: &str = "Numeracao pulada por falha no sistema";

fn dia(a: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(a, m, d).unwrap()
}

fn faixa<'a>(ini: u64, fim: u64) -> PedidoInutilizar<'a> {
    PedidoInutilizar {
        cnpj: CNPJ,
        serie: 1,
        inicio: ini,
        fim,
        justificativa: JUST,
        ano: None,
    }
}

fn pura<'a>(uf: &'a str, ini: u64, fim: u64) -> Inutilizacao<'a> {
    Inutilizacao {
        uf,
        ambiente: Homologacao,
        cnpj: CNPJ,
        serie: 1,
        inicio: ini,
        fim,
        justificativa: JUST,
        ano: 2026,
    }
}

// ------------------------------------------------------------------------------------------------ o pedido

#[test]
fn o_pedido_tem_o_id_de_43_posicoes_e_a_ordem_do_leiaute() {
    let m = montar(&pura("SP", 10, 25), dia(2026, 10, 3)).unwrap();
    assert_eq!(m.id, format!("ID3526{CNPJ}55001000000010000000025"));
    assert_eq!(m.id.len(), 43);
    assert_eq!(
        m.xml,
        format!(
            "<inutNFe xmlns=\"{NS}\" versao=\"4.00\"><infInut Id=\"{}\"><tpAmb>2</tpAmb><xServ>INUTILIZAR</xServ>\
             <cUF>35</cUF><ano>26</ano><CNPJ>{CNPJ}</CNPJ><mod>55</mod><serie>1</serie><nNFIni>10</nNFIni>\
             <nNFFin>25</nNFFin><xJust>{JUST}</xJust></infInut></inutNFe>",
            m.id
        )
    );
}

#[test]
fn o_pedido_assinado_confere_e_adulterar_a_faixa_quebra_a_assinatura() {
    let p = montar(&pura("SP", 1, 1), dia(2026, 10, 3))
        .unwrap()
        .assinar(&certificado())
        .unwrap();
    assert!(p.xml.contains("<Signature ") && p.xml.ends_with("</Signature></inutNFe>"));
    assert!(rustnf::assinatura::xmldsig::conferido(&p.xml));
    assert!(!rustnf::assinatura::xmldsig::conferido(
        &p.xml.replace("<nNFFin>1<", "<nNFFin>9<")
    ));
}

#[test]
fn as_regras_da_tabela_5_12_valem_antes_da_rede() {
    let hoje = dia(2026, 10, 3);
    // 224: faixa invertida; 201: mais de 10.000 numeros (10.000 passa)
    assert!(montar(&pura("SP", 9, 8), hoje)
        .unwrap_err()
        .mensagem
        .contains("224"));
    assert!(montar(&pura("SP", 1, 10_001), hoje)
        .unwrap_err()
        .mensagem
        .contains("201"));
    assert!(montar(&pura("SP", 1, 10_000), hoje).is_ok());
    assert!(montar(&pura("SP", 5_000, 14_999), hoje).is_ok());
    assert!(montar(&pura("SP", 5_000, 15_000), hoje).is_err());
    // 453 e 454: ano
    let futuro = Inutilizacao {
        ano: 2027,
        ..pura("SP", 1, 2)
    };
    assert!(montar(&futuro, hoje).unwrap_err().mensagem.contains("453"));
    let antigo = Inutilizacao {
        ano: 2005,
        ..pura("SP", 1, 2)
    };
    assert!(montar(&antigo, hoje).unwrap_err().mensagem.contains("454"));
    // o ano leva os 2 ultimos digitos
    let m = montar(
        &Inutilizacao {
            ano: 2006,
            ..pura("SP", 1, 2)
        },
        hoje,
    )
    .unwrap();
    assert!(m.xml.contains("<ano>06</ano>"));
    // numero zero, serie 1000, justificativa curta
    assert_eq!(montar(&pura("SP", 0, 2), hoje).unwrap_err().campo, "nNFIni");
    assert_eq!(
        montar(
            &Inutilizacao {
                serie: 1000,
                ..pura("SP", 1, 2)
            },
            hoje
        )
        .unwrap_err()
        .campo,
        "serie"
    );
    let curta = Inutilizacao {
        justificativa: "curta",
        ..pura("SP", 1, 2)
    };
    assert_eq!(montar(&curta, hoje).unwrap_err().campo, "xJust");
    assert_eq!(montar(&pura("XX", 1, 2), hoje).unwrap_err().campo, "uf");
}

#[test]
fn o_inutnfe_nacional_so_tem_cnpj_nao_ha_cpf() {
    let hoje = dia(2026, 10, 3);
    let e = montar(
        &Inutilizacao {
            cnpj: "52998224725",
            ..pura("MT", 1, 2)
        },
        hoje,
    )
    .unwrap_err();
    assert_eq!(e.campo, "CNPJ");
    assert!(e.mensagem.contains("CNPJ"), "{e}");
    let e = montar(
        &Inutilizacao {
            cnpj: "123",
            ..pura("SP", 1, 2)
        },
        hoje,
    )
    .unwrap_err();
    assert_eq!(e.campo, "CNPJ");
}

// ------------------------------------------------------------------------------------------------ AC, BA e SP

#[test]
fn ac_ba_e_sp_perderam_o_servico_em_11_11_2026_nos_dois_ambientes() {
    for uf in ["AC", "BA", "SP", "sp"] {
        for amb in [Producao, Homologacao] {
            assert!(
                inutilizacao::disponivel(uf, amb, dia(2026, 11, 10)).unwrap(),
                "{uf} {amb:?} antes"
            );
            assert!(
                !inutilizacao::disponivel(uf, amb, dia(2026, 11, 11)).unwrap(),
                "{uf} {amb:?} no dia"
            );
            assert!(
                !inutilizacao::disponivel(uf, amb, dia(2027, 6, 1)).unwrap(),
                "{uf} {amb:?} depois"
            );
            assert_eq!(inutilizacao::eliminada_em(uf, amb), Some(dia(2026, 11, 11)));
        }
    }
    // as outras 24 UF seguem com o servico, hoje e depois
    for uf in [
        "RO", "AM", "PA", "MA", "PI", "CE", "RN", "PB", "PE", "AL", "SE", "MG", "ES", "RJ", "PR",
        "SC", "RS", "MS", "MT", "GO", "DF", "TO", "AP", "RR", "35",
    ] {
        let esperado = uf != "35";
        assert_eq!(
            inutilizacao::disponivel(uf, Producao, dia(2027, 6, 1)).unwrap(),
            esperado,
            "{uf}"
        );
    }
    assert!(inutilizacao::eliminada_em("MG", Producao).is_none());
    assert!(inutilizacao::disponivel("XX", Producao, dia(2026, 1, 1)).is_err());
}

#[tokio::test]
async fn uf_sem_o_servico_devolve_erro_em_palavras_sem_chamar_a_rede() {
    for uf in ["SP", "AC", "BA"] {
        for amb in [Homologacao, Producao] {
            let f = Falso::novo(vec![]);
            let (c, _) = cliente_em(uf, amb, (2026, 11, 25), f.clone());
            let e = c
                .inutilizar(&certificado(), &faixa(1, 2))
                .await
                .unwrap_err();
            assert_eq!(e.campo, "uf");
            assert!(
                e.mensagem.contains("11/11/2026") && e.mensagem.contains("NT 2023.002 v1.02"),
                "{e}"
            );
            assert!(
                e.mensagem.contains(uf) && e.mensagem.contains("nao foi enviado"),
                "{e}"
            );
            assert!(f.chamadas().is_empty(), "{uf} {amb:?}: chamou a rede");
        }
    }
}

// ------------------------------------------------------------------------------------------------ desfechos

fn minas(f: Falso) -> rustnf::nfe::sefaz::ClienteSefaz<Falso> {
    // MG tem webservice proprio e ainda tem o servico
    cliente_em("MG", Homologacao, (2026, 11, 25), f).0
}

#[tokio::test]
async fn faixa_inutilizada_102_guarda_o_proc_inut_e_vai_assinada() {
    let f = Falso::novo(vec![(
        INU,
        ok(
            INU,
            &ret_inut(
                "102",
                "Inutilizacao de numero homologado",
                1,
                10,
                25,
                "135260000000501",
            ),
        ),
    )]);
    let c = minas(f.clone());
    let r = c.inutilizar(&certificado(), &faixa(10, 25)).await.unwrap();
    assert!(r.inutilizada());
    let DesfechoInutilizacao::Inutilizada {
        cstat, protocolo, ..
    } = &r.desfecho
    else {
        panic!("{r:?}")
    };
    assert_eq!(
        (cstat.as_str(), protocolo.as_str()),
        ("102", "135260000000501")
    );
    assert_eq!(r.frase, "Faixa inutilizada: protocolo 135260000000501.");
    assert_eq!(r.id.len(), 43);
    assert!(r.id.starts_with("ID3126"));
    let proc_ = r.proc_inut.as_deref().unwrap();
    assert!(proc_.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?><ProcInutNFe "));
    assert!(proc_.contains(&r.xml_enviado) && proc_.contains("<retInutNFe "));
    assert!(proc_.ends_with("</retInutNFe></ProcInutNFe>"));
    let ch = f.chamadas();
    assert_eq!(ch.len(), 1);
    assert_eq!(
        ch[0].content_type,
        "application/soap+xml; charset=utf-8; action=\"http://www.portalfiscal.inf.br/nfe/wsdl/NFeInutilizacao4/nfeInutilizacaoNF\""
    );
    assert!(rustnf::assinatura::xmldsig::conferido(&r.xml_enviado));
    assert!(ch[0].corpo.contains(&r.xml_enviado));
}

#[tokio::test]
async fn a_mesma_faixa_de_novo_563_e_ja_inutilizada_com_o_protocolo_anterior() {
    let f = Falso::novo(vec![(
        INU,
        ok(
            INU,
            &ret_inut(
                "563",
                "Rejeicao: Ja existe pedido de Inutilizacao com a mesma faixa de inutilizacao",
                1,
                10,
                25,
                "135260000000501",
            ),
        ),
    )]);
    let r = minas(f)
        .inutilizar(&certificado(), &faixa(10, 25))
        .await
        .unwrap();
    assert!(r.inutilizada());
    let DesfechoInutilizacao::JaInutilizada { protocolo, .. } = &r.desfecho else {
        panic!("{r:?}")
    };
    assert_eq!(protocolo, "135260000000501");
    assert!(r.frase.contains("563") && r.frase.contains("ja estava inutilizada"));
    assert!(r.proc_inut.is_some());
}

#[tokio::test]
async fn servico_desativado_998_e_http_404_dizem_para_nao_repetir() {
    let f = Falso::novo(vec![(
        INU,
        ok(
            INU,
            &ret_inut("998", "Rejeicao: Servico Desativado", 0, 0, 0, ""),
        ),
    )]);
    let r = minas(f)
        .inutilizar(&certificado(), &faixa(1, 2))
        .await
        .unwrap();
    assert!(matches!(
        r.desfecho,
        DesfechoInutilizacao::ServicoDesativado { .. }
    ));
    assert!(!r.inutilizada() && !r.pode_repetir());
    assert!(r.frase.contains("nao repita"), "{}", r.frase);

    let f = Falso::novo(vec![(
        INU,
        Resp::R(404, b"<html>Not found</html>".to_vec()),
    )]);
    let r = minas(f)
        .inutilizar(&certificado(), &faixa(1, 2))
        .await
        .unwrap();
    assert!(matches!(
        r.desfecho,
        DesfechoInutilizacao::ServicoDesativado { .. }
    ));
    assert!(r.detalhe.starts_with("HTTP 404"));
    assert!(r.frase.contains("nao repita"));
}

#[tokio::test]
async fn rejeicao_servico_parado_e_resposta_perdida_tem_cada_um_o_seu_desfecho() {
    let f = Falso::novo(vec![(
        INU,
        ok(
            INU,
            &ret_inut(
                "241",
                "Rejeicao: Um numero da faixa ja foi utilizado",
                0,
                0,
                0,
                "",
            ),
        ),
    )]);
    let r = minas(f)
        .inutilizar(&certificado(), &faixa(1, 2))
        .await
        .unwrap();
    let DesfechoInutilizacao::Rejeitada { cstat, motivo } = &r.desfecho else {
        panic!("{r:?}")
    };
    assert_eq!(cstat, "241");
    assert!(motivo.contains("faixa") && r.proc_inut.is_none());

    let f = Falso::novo(vec![(
        INU,
        ok(
            INU,
            &ret_inut("108", "Servico Paralisado Momentaneamente", 0, 0, 0, ""),
        ),
    )]);
    let r = minas(f)
        .inutilizar(&certificado(), &faixa(1, 2))
        .await
        .unwrap();
    assert!(matches!(
        r.desfecho,
        DesfechoInutilizacao::Indisponivel { .. }
    ));
    assert!(r.pode_repetir());

    for resp in [
        tempo(),
        fault("falha interna"),
        Resp::R(200, b"<html>ok</html>".to_vec()),
    ] {
        let r = minas(Falso::novo(vec![(INU, resp)]))
            .inutilizar(&certificado(), &faixa(1, 2))
            .await
            .unwrap();
        assert!(matches!(r.desfecho, DesfechoInutilizacao::SemResposta));
        assert!(r.pode_repetir());
        assert!(
            r.frase.contains("MESMA") && r.frase.contains("563"),
            "{}",
            r.frase
        );
    }
}

#[tokio::test]
async fn resposta_de_outra_faixa_nao_vale_como_inutilizada() {
    let f = Falso::novo(vec![(
        INU,
        ok(
            INU,
            &ret_inut(
                "102",
                "Inutilizacao de numero homologado",
                1,
                90,
                99,
                "135260000000501",
            ),
        ),
    )]);
    let r = minas(f)
        .inutilizar(&certificado(), &faixa(10, 25))
        .await
        .unwrap();
    assert!(matches!(r.desfecho, DesfechoInutilizacao::SemResposta));
    assert!(r.detalhe.contains("outra faixa"));
}

#[tokio::test]
async fn o_pedido_que_nao_monta_nao_chega_a_rede() {
    let f = Falso::novo(vec![]);
    let c = minas(f.clone());
    let e = c
        .inutilizar(&certificado(), &faixa(1, 20_000))
        .await
        .unwrap_err();
    assert!(e.mensagem.contains("201"));
    assert!(f.chamadas().is_empty());
    let _: Ambiente = Homologacao;
}
