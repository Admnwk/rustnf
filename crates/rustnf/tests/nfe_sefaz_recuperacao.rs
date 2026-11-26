// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Cliente da SEFAZ com transporte falso, quando a resposta se perde ou a SEFAZ acusa duplicidade: a consulta acha a
//! nota ou diz que nao consta, 204 e 539, SOAP Fault, HTTP de erro, e o protocolo de outra chave que nao autoriza esta.

mod nfe_sefaz_comum;

use nfe_sefaz_comum::*;
use rustnf::nfe::sefaz::resposta::nfe_proc;
use rustnf::nfe::sefaz::Desfecho;

// ------------------------------------------------------------------------------------------------ resposta perdida

#[tokio::test]
async fn sem_resposta_a_consulta_acha_a_nota_autorizada_e_nao_ha_segundo_envio() {
    let (xml, chave) = nfe(9, 2);
    let p = prot_ok(&chave);
    let f = Falso::novo(vec![
        (AUT, tempo()),
        (
            CON,
            ok(CON, &ret_sit(&chave, "100", "Autorizado o uso da NF-e", &p)),
        ),
    ]);
    let (c, _) = cliente(f.clone());
    let r = c.autorizar(&xml).await.unwrap();
    assert!(r.autorizada() && r.recuperada, "{r:?}");
    assert_eq!(r.nfe_proc(), Some(nfe_proc(&xml, &p).as_str()));
    assert!(r.detalhe.contains("tempo esgotado"), "{}", r.detalhe);
    let ch = f.chamadas();
    assert_eq!(
        ch.iter().filter(|c| c.corpo.contains("<enviNFe")).count(),
        1
    );
    assert!(ch[1].corpo.contains(&format!("<chNFe>{chave}</chNFe>")));
}

#[tokio::test]
async fn sem_resposta_e_a_consulta_diz_217_a_nota_nao_chegou_e_reenviar_a_mesma_e_seguro() {
    let (xml, chave) = nfe(10, 2);
    let f = Falso::novo(vec![
        (AUT, tempo()),
        (
            CON,
            ok(
                CON,
                &ret_sit(
                    &chave,
                    "217",
                    "Rejeicao: NF-e nao consta na base de dados da SEFAZ",
                    "",
                ),
            ),
        ),
        // o reenvio: a MESMA nota
        (AUT, ok(AUT, &ret_envi_sinc(&prot_ok(&chave)))),
    ]);
    let (c, _) = cliente(f.clone());
    let r = c.autorizar(&xml).await.unwrap();
    assert_eq!(r.desfecho, Desfecho::NaoRecebida);
    assert!(r.pode_reenviar() && !r.recuperada && !r.deve_consultar());
    assert!(r.frase.contains("MESMA NF-e"), "{}", r.frase);
    let r2 = c.autorizar(&xml).await.unwrap();
    assert!(r2.autorizada());
    let ch = f.chamadas();
    let envios: Vec<&_> = ch.iter().filter(|c| c.corpo.contains("<enviNFe")).collect();
    assert_eq!(envios.len(), 2);
    for e in &envios {
        assert!(
            e.corpo.contains(&xml),
            "as duas vezes a mesma NFe, byte a byte"
        );
    }
    // o numero do lote muda, a nota nao
    let id = |c: &str| {
        c.split("<idLote>")
            .nth(1)
            .unwrap()
            .split('<')
            .next()
            .unwrap()
            .to_string()
    };
    assert_ne!(id(&envios[0].corpo), id(&envios[1].corpo));
    assert!(id(&envios[0].corpo).len() <= 15);
}

#[tokio::test]
async fn sem_resposta_nem_da_consulta_fica_sem_resposta_e_manda_consultar_e_nao_emitir_outra() {
    let (xml, _) = nfe(11, 2);
    let f = Falso::novo(vec![(AUT, tempo()), (CON, tempo())]);
    let (c, _) = cliente(f);
    let r = c.autorizar(&xml).await.unwrap();
    assert_eq!(r.desfecho, Desfecho::SemResposta);
    assert!(r.deve_consultar() && !r.pode_reenviar() && !r.autorizada());
    assert!(
        r.frase.contains("PODE ter sido autorizada") && r.frase.contains("NAO emita outra"),
        "{}",
        r.frase
    );
    assert!(r.detalhe.contains("sem resposta"), "{}", r.detalhe);
}

#[tokio::test]
async fn http_de_erro_html_de_proxy_e_fault_tambem_caem_na_consulta() {
    let casos: Vec<(&str, Resp)> = vec![
        (
            "HTTP 502",
            Resp::R(502, b"<html><body>Bad Gateway</body></html>".to_vec()),
        ),
        ("HTTP 503", Resp::R(503, b"".to_vec())),
        ("HTTP 404", Resp::R(404, b"nao achou".to_vec())),
        (
            "HTTP 200",
            Resp::R(200, b"<html><body>manutencao</body></html>".to_vec()),
        ),
        ("HTTP 500", Resp::R(500, b"Internal Server Error".to_vec())),
        ("falha SOAP", fault("Erro interno do servidor")),
    ];
    for (marca, resp) in casos {
        let (xml, chave) = nfe(12, 2);
        let f = Falso::novo(vec![
            (AUT, resp),
            (
                CON,
                ok(
                    CON,
                    &ret_sit(&chave, "100", "Autorizado o uso da NF-e", &prot_ok(&chave)),
                ),
            ),
        ]);
        let (c, _) = cliente(f);
        let r = c.autorizar(&xml).await.unwrap();
        assert!(r.autorizada() && r.recuperada, "{marca}: {r:?}");
        assert!(r.detalhe.contains(marca), "{marca}: {}", r.detalhe);
    }
}

#[tokio::test]
async fn http_de_erro_diz_o_que_e_em_palavras_sem_vazar_o_corpo() {
    let (xml, _) = nfe(13, 2);
    let f = Falso::novo(vec![
        (AUT, Resp::R(503, b"SEGREDO-NO-CORPO".to_vec())),
        (CON, Resp::R(502, b"SEGREDO-NO-CORPO".to_vec())),
    ]);
    let (c, _) = cliente(f);
    let r = c.autorizar(&xml).await.unwrap();
    assert!(
        r.detalhe.contains("503: servico indisponivel"),
        "{}",
        r.detalhe
    );
    assert!(
        r.detalhe.contains("502: servico indisponivel"),
        "{}",
        r.detalhe
    );
    assert!(!r.detalhe.contains("SEGREDO") && !r.frase.contains("SEGREDO"));
    assert!(
        !r.frase.contains("http"),
        "a frase da tela nao leva URL: {}",
        r.frase
    );
}

// ------------------------------------------------------------------------------------------------ duplicidade

#[tokio::test]
async fn duplicidade_204_a_consulta_devolve_a_nota_ja_autorizada_e_nao_gera_outra() {
    let (xml, chave) = nfe(14, 2);
    let motivo = "Rejeicao: Duplicidade de NF-e [nRec:351000000000046]";
    let p204 = prot(&chave, "204", motivo, "");
    let p_ok = prot_ok(&chave);
    let f = Falso::novo(vec![
        (AUT, ok(AUT, &ret_envi_sinc(&p204))),
        (
            CON,
            ok(
                CON,
                &ret_sit(&chave, "100", "Autorizado o uso da NF-e", &p_ok),
            ),
        ),
    ]);
    let (c, _) = cliente(f.clone());
    let r = c.autorizar(&xml).await.unwrap();
    assert!(r.autorizada() && r.recuperada, "{r:?}");
    assert_eq!(r.chave, chave);
    assert_eq!(
        r.nfe_proc(),
        Some(nfe_proc(&xml, &p_ok).as_str()),
        "o protocolo e' o da consulta"
    );
    assert!(r.detalhe.contains("204 duplicidade"), "{}", r.detalhe);
    let ch = f.chamadas();
    assert_eq!(
        ch.iter().filter(|c| c.corpo.contains("<enviNFe")).count(),
        1
    );
}

#[tokio::test]
async fn duplicidade_204_com_consulta_217_e_contradicao_e_nao_manda_reenviar() {
    let (xml, chave) = nfe(15, 2);
    let p204 = prot(
        &chave,
        "204",
        "Rejeicao: Duplicidade de NF-e [nRec:351000000000047]",
        "",
    );
    let f = Falso::novo(vec![
        (AUT, ok(AUT, &ret_envi_sinc(&p204))),
        (
            CON,
            ok(
                CON,
                &ret_sit(
                    &chave,
                    "217",
                    "Rejeicao: NF-e nao consta na base de dados da SEFAZ",
                    "",
                ),
            ),
        ),
    ]);
    let (c, _) = cliente(f);
    let r = c.autorizar(&xml).await.unwrap();
    assert_eq!(r.desfecho, Desfecho::SemResposta);
    assert!(!r.pode_reenviar());
}

#[tokio::test]
async fn duplicidade_539_diz_que_o_numero_e_de_outra_chave_e_qual() {
    let (xml, chave) = nfe(16, 2);
    let (_, outra) = nfe_outra_chave(16);
    assert_ne!(chave, outra);
    let motivo = format!(
        "Rejeicao: Duplicidade de NF-e com diferenca na Chave de Acesso [chNFe:{outra}][nRec:351000000000048]"
    );
    let p539 = prot(&chave, "539", &motivo, "");
    let f = Falso::novo(vec![
        (AUT, ok(AUT, &ret_envi_sinc(&p539))),
        (
            CON,
            ok(
                CON,
                &ret_sit(&outra, "100", "Autorizado o uso da NF-e", &prot_ok(&outra)),
            ),
        ),
    ]);
    let (c, _) = cliente(f.clone());
    let r = c.autorizar(&xml).await.unwrap();
    assert_eq!(
        r.desfecho,
        Desfecho::NumeroJaUsado {
            motivo: motivo.clone(),
            chave_existente: Some(outra.clone()),
            situacao_existente: Some("100".into())
        }
    );
    assert!(!r.autorizada() && !r.pode_reenviar() && !r.recuperada && r.nfe_proc().is_none());
    assert!(
        r.frase.contains(&outra) && r.frase.contains("nao emita uma terceira"),
        "{}",
        r.frase
    );
    // a consulta foi na chave da OUTRA nota
    assert!(f.chamadas()[1]
        .corpo
        .contains(&format!("<chNFe>{outra}</chNFe>")));
}

#[tokio::test]
async fn duplicidade_539_com_chave_de_outro_emitente_ou_numero_nao_e_consultada() {
    let (xml, chave) = nfe(17, 2);
    // a chave da mensagem e' de outro numero: nao e' a duplicidade desta nota, o cliente nao confia
    let (_, de_outro_numero) = nfe(18, 2);
    for citada in [de_outro_numero, chave.clone(), "123".to_string()] {
        let motivo = format!(
            "Rejeicao: Duplicidade de NF-e com diferenca na Chave de Acesso [chNFe:{citada}]"
        );
        let f = Falso::novo(vec![(
            AUT,
            ok(AUT, &ret_envi_sinc(&prot(&chave, "539", &motivo, ""))),
        )]);
        let (c, _) = cliente(f.clone());
        let r = c.autorizar(&xml).await.unwrap();
        assert!(
            matches!(
                &r.desfecho,
                Desfecho::NumeroJaUsado {
                    chave_existente: None,
                    situacao_existente: None,
                    ..
                }
            ),
            "{r:?}"
        );
        assert_eq!(f.chamadas().len(), 1);
    }
}

// ------------------------------------------------------------------------------------------------ protocolo de outra nota

#[tokio::test]
async fn protocolo_de_outra_chave_no_lote_nao_autoriza_esta_nota() {
    let (xml, chave) = nfe(19, 2);
    let (_, outra) = nfe(20, 2);
    let f = Falso::novo(vec![
        (AUT, ok(AUT, &ret_envi_sinc(&prot_ok(&outra)))),
        (
            CON,
            ok(
                CON,
                &ret_sit(
                    &chave,
                    "217",
                    "Rejeicao: NF-e nao consta na base de dados da SEFAZ",
                    "",
                ),
            ),
        ),
    ]);
    let (c, _) = cliente(f);
    let r = c.autorizar(&xml).await.unwrap();
    assert!(!r.autorizada(), "o protocolo era de outra chave: {r:?}");
    assert_eq!(r.desfecho, Desfecho::NaoRecebida);
    assert!(
        r.detalhe.contains("sem o protocolo desta nota"),
        "{}",
        r.detalhe
    );
}

#[tokio::test]
async fn consulta_com_protocolo_de_outra_chave_nao_vira_autorizada() {
    let (xml, chave) = nfe(21, 2);
    let (_, outra) = nfe(22, 2);
    let f = Falso::novo(vec![(
        CON,
        ok(
            CON,
            &ret_sit(&chave, "100", "Autorizado o uso da NF-e", &prot_ok(&outra)),
        ),
    )]);
    let (c, _) = cliente(f);
    let r = c.consultar(&chave, Some(&xml)).await.unwrap();
    assert_eq!(r.desfecho, Desfecho::SemResposta);
    assert!(r.nfe_proc().is_none());
}
