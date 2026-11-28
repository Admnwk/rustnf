// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A retomada do que ficou sem resultado, a consulta, o status do servico e o DANFE.

use chrono::Duration;
use serde_json::json;

use super::testes_comuns::*;
use super::{
    consultar, danfe_da_pasta, danfe_do_xml, emitir, epec, estado_do_servico, retomar, status,
    Contexto,
};
use crate::erro::Erro;
use crate::notas::nfe::Situacao;

/// O contexto de quem pede a retomada alguns segundos depois do envio.
fn depois<'a>(c: &'a Cenario, segundos: i64) -> Contexto<'a, Falso> {
    c.ctx().com_agora(agora() + Duration::seconds(segundos))
}

/// A nota 1 enviada sem resposta (nem da consulta): pendente na pasta.
async fn com_pendente(rotulo: &str) -> (Cenario, String) {
    let c = Cenario::novo(rotulo, vec![(AUT, tempo()), (CON, tempo())]);
    let e = emitir(&c.ctx(), pedido(), true).await.unwrap();
    assert_eq!(e.situacao, Some(Situacao::Pendente));
    (c, e.chave)
}

fn envios(c: &Cenario) -> Vec<String> {
    c.falso
        .corpos()
        .into_iter()
        .filter(|b| b.contains("<enviNFe"))
        .collect()
}

#[tokio::test]
async fn queda_depois_do_envio_a_pendente_e_retomada_e_vira_autorizada_sem_nota_gemea() {
    let (c, chave) = com_pendente("nfe-retoma").await;
    c.roteiro(vec![(
        CON,
        consulta("100", "Autorizado o uso da NF-e", NPROT),
    )]);
    let feitos = retomar(&depois(&c, 5)).await.unwrap();
    assert_eq!(feitos.len(), 1);
    let f = &feitos[0];
    assert_eq!(
        (f.chave.as_str(), f.acao, f.status),
        (chave.as_str(), "consultou", "autorizada")
    );
    assert_eq!(f.situacao, Some(Situacao::Autorizada));
    assert_eq!(f.para_json()["resultado"]["protocolo"], NPROT);

    // uma nota so', a mesma chave, o nfeProc guardado e a copia assinada de antes removida
    let lista = c.notas.listar().unwrap();
    assert_eq!(lista.len(), 1);
    assert_eq!(lista[0].chave, chave);
    assert_eq!(lista[0].situacao, Situacao::Autorizada);
    assert_eq!(lista[0].protocolo, NPROT);
    assert!(c.notas.ler_xml(&chave).unwrap().contains("<nfeProc"));
    assert_eq!(c.notas.ler_assinada(&chave).unwrap(), None);
    assert_eq!(
        envios(&c).len(),
        1,
        "a NF-e foi enviada UMA vez: nao nasceu outra nota"
    );
    assert_eq!(c.falso.servicos(), vec![AUT, CON, CON]);
    assert_eq!(c.notas.proximo_numero("1").unwrap(), 2);
    assert!(c.notas.a_resolver().unwrap().is_empty());
    // retomar de novo nao tem mais nada
    assert!(retomar(&depois(&c, 10)).await.unwrap().is_empty());
}

#[tokio::test]
async fn se_a_sefaz_diz_que_nao_recebeu_a_mesma_nfe_assinada_e_reenviada() {
    let (c, chave) = com_pendente("nfe-retoma-reenvia").await;
    c.roteiro(vec![
        (
            CON,
            consulta(
                "217",
                "Rejeicao: NF-e nao consta na base de dados da SEFAZ",
                "",
            ),
        ),
        (AUT, envio("100", "Autorizado o uso da NF-e", NPROT)),
    ]);
    let feitos = retomar(&depois(&c, 5)).await.unwrap();
    assert_eq!(
        (feitos[0].acao, feitos[0].status),
        ("reenviou", "autorizada")
    );
    let e = envios(&c);
    assert_eq!(e.len(), 2);
    let nfe = |b: &str| format!("<NFe{}</NFe>", entre(b, "<NFe", "</NFe>"));
    assert_eq!(
        nfe(&e[0]),
        nfe(&e[1]),
        "as duas vezes a mesma NF-e, byte a byte"
    );
    assert_eq!(c.notas.listar().unwrap().len(), 1);
    assert_eq!(
        c.notas.registro(&chave).unwrap().situacao,
        Situacao::Autorizada
    );
}

#[tokio::test]
async fn sem_resposta_de_novo_conta_a_tentativa_e_a_nota_segue_pendente() {
    let (c, chave) = com_pendente("nfe-retoma-sem").await;
    c.roteiro(vec![(CON, tempo())]);
    let feitos = retomar(&depois(&c, 5)).await.unwrap();
    assert_eq!(feitos[0].situacao, Some(Situacao::Pendente));
    assert_eq!(feitos[0].status, "sem_resposta");
    let r = c.notas.registro(&chave).unwrap();
    assert_eq!((r.tentativas, r.ultima), (1, Some(agora().timestamp() + 5)));
    // a mesma nota, ainda la: nada de nota nova
    assert_eq!(c.notas.proximo_numero("1").unwrap(), 2);
}

#[tokio::test]
async fn rejeitada_na_retomada_sai_da_pasta_e_libera_o_numero() {
    let (c, _) = com_pendente("nfe-retoma-rej").await;
    c.roteiro(vec![
        (
            CON,
            consulta(
                "217",
                "Rejeicao: NF-e nao consta na base de dados da SEFAZ",
                "",
            ),
        ),
        (
            AUT,
            envio("225", "Rejeicao: Falha no Schema XML do lote de NFe", ""),
        ),
    ]);
    let feitos = retomar(&depois(&c, 5)).await.unwrap();
    assert_eq!(feitos[0].situacao, None);
    assert_eq!(feitos[0].status, "rejeitada");
    assert!(c.notas.listar().unwrap().is_empty());
    assert_eq!(c.notas.proximo_numero("1").unwrap(), 1);
}

const CONSUMO_INDEVIDO: &str = "Rejeicao: Consumo indevido pelo aplicativo da empresa";

#[tokio::test]
async fn consulta_recusada_na_retomada_nao_tira_a_nota_da_pasta_nem_reenvia() {
    let (c, chave) = com_pendente("nfe-retoma-656").await;
    c.roteiro(vec![(CON, consulta("656", CONSUMO_INDEVIDO, ""))]);
    let feitos = retomar(&depois(&c, 5)).await.unwrap();
    let f = &feitos[0];
    assert_eq!(
        (f.acao, f.status, f.situacao),
        ("consultou", "consulta_recusada", Some(Situacao::Pendente))
    );
    let aviso = f.aviso.as_deref().unwrap();
    assert!(aviso.contains("656") && aviso.contains("segue"), "{aviso}");
    // a recusa e' do PEDIDO de consulta: a nota pode estar autorizada, entao fica tudo como estava
    let r = c.notas.registro(&chave).unwrap();
    assert_eq!(r.situacao, Situacao::Pendente);
    assert_eq!((r.tentativas, r.recusa.as_str()), (1, ""));
    assert!(c.notas.ler_assinada(&chave).unwrap().is_some());
    assert_eq!(c.notas.proximo_numero("1").unwrap(), 2);
    assert_eq!(envios(&c).len(), 1, "nada se reenvia");
    assert_eq!(c.falso.servicos(), vec![AUT, CON, CON]);
}

#[tokio::test]
async fn consultar_a_pendente_com_a_consulta_recusada_nao_mexe_na_pasta() {
    let (c, chave) = com_pendente("nfe-consulta-656").await;
    c.roteiro(vec![(CON, consulta("656", CONSUMO_INDEVIDO, ""))]);
    let r = consultar(&c.ctx(), &chave).await.unwrap();
    assert_eq!(r.situacao, Some(Situacao::Pendente));
    assert!(r.resultado.deve_consultar());
    assert_eq!(
        c.notas.registro(&chave).unwrap().situacao,
        Situacao::Pendente
    );
    assert!(c.notas.ler_assinada(&chave).unwrap().is_some());
    assert_eq!(c.notas.proximo_numero("1").unwrap(), 2);
}

#[tokio::test]
async fn a_nota_em_epec_e_transmitida_a_sefaz_da_uf_na_retomada() {
    let c = Cenario::novo(
        "nfe-retoma-epec",
        vec![(
            EVT,
            evento(
                "136",
                "Evento registrado, mas nao vinculado a NF-e",
                "891260000000001",
            ),
        )],
    );
    let mut p = pedido();
    p["x_just"] = json!("Falha na comunicacao com a SEFAZ de origem");
    let r = epec(&c.ctx(), p, true).await.unwrap();
    assert_eq!(r.situacao, Some(Situacao::Epec));
    c.roteiro(vec![(AUT, envio("100", "Autorizado o uso da NF-e", NPROT))]);
    let feitos = retomar(&depois(&c, 5)).await.unwrap();
    assert_eq!(
        (feitos[0].acao, feitos[0].status),
        ("transmitiu", "autorizada")
    );
    let reg = c.notas.registro(&r.chave).unwrap();
    assert_eq!(reg.situacao, Situacao::Autorizada);
    assert!(c.notas.ler_xml(&r.chave).unwrap().contains("<nfeProc"));
    assert!(c
        .notas
        .pasta()
        .join("2026/10")
        .join(format!("{}-110140-1-procEventoNFe.xml", r.chave))
        .is_file());
}

#[tokio::test]
async fn consultar_poe_a_pasta_em_dia_e_a_consulta_nunca_desfaz_um_cancelamento() {
    let (c, chave) = com_pendente("nfe-consulta").await;
    c.roteiro(vec![(
        CON,
        consulta("100", "Autorizado o uso da NF-e", NPROT),
    )]);
    let r = consultar(&c.ctx(), &chave).await.unwrap();
    assert_eq!(r.situacao, Some(Situacao::Autorizada));
    assert!(r.erro().is_none());
    assert_eq!(r.para_json()["situacao"], "autorizada");
    assert!(c.notas.ler_xml(&chave).unwrap().contains("<nfeProc"));

    // a SEFAZ diz que foi cancelada (por fora): a pasta acompanha
    c.roteiro(vec![(
        CON,
        consulta("101", "Cancelamento de NF-e homologado", NPROT),
    )]);
    let r = consultar(&c.ctx(), &chave).await.unwrap();
    assert_eq!(r.situacao, Some(Situacao::Cancelada));
    assert_eq!(r.erro().unwrap().codigo_saida(), 4);

    // e uma consulta que volte "autorizada" nao a desfaz
    c.roteiro(vec![(
        CON,
        consulta("100", "Autorizado o uso da NF-e", NPROT),
    )]);
    let r = consultar(&c.ctx(), &chave).await.unwrap();
    assert_eq!(r.situacao, Some(Situacao::Cancelada));
}

#[tokio::test]
async fn consultar_chave_de_fora_da_pasta_so_informa() {
    let c = Cenario::novo("nfe-consulta-fora", vec![]);
    let chave = {
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
    };
    c.roteiro(vec![(
        CON,
        consulta(
            "217",
            "Rejeicao: NF-e nao consta na base de dados da SEFAZ",
            "",
        ),
    )]);
    let r = consultar(&c.ctx(), &chave).await.unwrap();
    assert_eq!(r.situacao, None);
    assert_eq!(r.para_json()["status"], "nao_recebida");
    assert!(c.notas.listar().unwrap().is_empty());
    let erro = consultar(&c.ctx(), "123").await.unwrap_err();
    assert_eq!(erro.codigo_saida(), 3, "{erro}");
}

#[tokio::test]
async fn status_do_servico_em_operacao_paralisado_e_sem_resposta() {
    let c = Cenario::novo(
        "nfe-status",
        vec![
            (STA, ok(STA, &ret_status("107", "Servico em Operacao"))),
            (
                STA,
                ok(
                    STA,
                    &ret_status("108", "Servico Paralisado Momentaneamente"),
                ),
            ),
            (STA, tempo()),
        ],
    );
    let s = status(&c.cliente, false).await.unwrap();
    let j = estado_do_servico(&c.cliente, &s);
    assert_eq!(
        (
            j["status"].as_str(),
            j["uf"].as_str(),
            j["ambiente"].as_str()
        ),
        (Some("em_operacao"), Some("SP"), Some("homologacao"))
    );
    assert!(s.em_operacao());
    let s = status(&c.cliente, false).await.unwrap();
    assert_eq!(estado_do_servico(&c.cliente, &s)["status"], "paralisado");
    let s = status(&c.cliente, false).await.unwrap();
    assert_eq!(estado_do_servico(&c.cliente, &s)["status"], "sem_resposta");
}

#[tokio::test]
async fn danfe_da_pasta_gera_o_pdf_marca_a_cancelada_e_guarda_ao_lado_do_xml() {
    let c = Cenario::novo(
        "nfe-danfe",
        vec![(AUT, envio("100", "Autorizado o uso da NF-e", NPROT))],
    );
    let e = emitir(&c.ctx(), pedido(), true).await.unwrap();
    let pdf = danfe_da_pasta(&c.notas, &c.config, &e.chave).unwrap();
    assert!(pdf.starts_with(b"%PDF"));
    let caminho = c.notas.guardar_danfe(&e.chave, &pdf).unwrap();
    assert_eq!(
        caminho,
        c.notas
            .pasta()
            .join("2026/10")
            .join(format!("{}-procNFe.pdf", e.chave))
    );
    assert_eq!(std::fs::read(&caminho).unwrap(), pdf);
    // sem canhoto, outro PDF
    let mut sem = c.config.clone();
    sem.nfe.canhoto = false;
    assert_ne!(danfe_da_pasta(&c.notas, &sem, &e.chave).unwrap(), pdf);

    // cancelada: a marca d'agua muda o desenho
    let ev = crate::notas::nfe::EventoGuardado {
        tipo: "110111".into(),
        sequencia: 1,
        protocolo: "1".into(),
    };
    c.notas
        .guardar_evento(&e.chave, &ev, "<procEventoNFe/>", "")
        .unwrap();
    assert_ne!(danfe_da_pasta(&c.notas, &c.config, &e.chave).unwrap(), pdf);

    // fora da pasta e XML que nao e' NF-e
    let outra = "1".repeat(44);
    assert_eq!(
        danfe_da_pasta(&c.notas, &c.config, &outra)
            .unwrap_err()
            .codigo_saida(),
        2
    );
    assert!(matches!(
        danfe_do_xml("<x/>", true, false, None),
        Err(Erro::Entrada(_))
    ));
}

#[tokio::test]
async fn danfe_da_nota_pendente_sai_como_previa() {
    let (c, chave) = com_pendente("nfe-danfe-previa").await;
    let pdf = danfe_da_pasta(&c.notas, &c.config, &chave).unwrap();
    assert!(pdf.starts_with(b"%PDF"));
    let caminho = c.notas.guardar_danfe(&chave, &pdf).unwrap();
    assert!(caminho.to_string_lossy().ends_with("-NFe.pdf"));
}
