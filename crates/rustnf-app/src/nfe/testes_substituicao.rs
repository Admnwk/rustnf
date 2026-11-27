// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O EPEC no lugar de uma NF-e que ficou sem resposta: a recusa antes da rede, o vinculo na pasta, a retomada que so'
//! consulta a substituida, a inutilizacao do numero que a SEFAZ nao tem e o cancelamento da que ela autorizou.

use chrono::Duration;
use rustnf::chave_dfe::{montar, Montar};
use serde_json::{json, Value};

use super::testes_comuns::*;
use super::{
    cancelar, emitir, epec, epec_substituindo, erro_da_retomada, inutilizar, pendentes_json,
    retomar, separar_substitui, Contexto,
};
use crate::notas::nfe::{NotasNfe, Registro, Situacao};

const NAO_CONSTA: &str = "Rejeicao: NF-e nao consta na base de dados da SEFAZ";
const NPROT_EPEC: &str = "891260000000001";
const JUST_INU: &str = "Numero da nota sem resposta substituida pelo EPEC";

fn depois(c: &Cenario, segundos: i64) -> Contexto<'_, Falso> {
    c.ctx().com_agora(agora() + Duration::seconds(segundos))
}

fn pedido_epec() -> Value {
    let mut p = pedido();
    p["x_just"] = json!("Falha na comunicacao com a SEFAZ de origem");
    p
}

fn envios_da(c: &Cenario, chave: &str) -> usize {
    c.falso
        .corpos()
        .iter()
        .filter(|b| b.contains("<enviNFe") && b.contains(&format!("Id=\"NFe{chave}\"")))
        .count()
}

/// A nota 1 enviada sem resposta (pendente) e, no lugar dela, o EPEC (a nota 2) que a substitui.
async fn com_substituida(rotulo: &str) -> (Cenario, String, String) {
    let c = Cenario::novo(rotulo, vec![(AUT, tempo()), (CON, tempo())]);
    let pendente = emitir(&c.ctx(), pedido(), true).await.unwrap();
    assert_eq!(pendente.situacao, Some(Situacao::Pendente));
    c.roteiro(vec![(
        EVT,
        evento(
            "136",
            "Evento registrado, mas nao vinculado a NF-e",
            NPROT_EPEC,
        ),
    )]);
    let r = epec_substituindo(&c.ctx(), pedido_epec(), true, Some(&pendente.chave))
        .await
        .unwrap();
    assert_eq!(r.registrado.situacao, Some(Situacao::Epec));
    (c, pendente.chave, r.registrado.chave)
}

fn chave_de(emitente: &str, numero: u64) -> String {
    montar(&Montar {
        cuf: 35,
        aamm: "2610",
        emitente,
        modelo: 55,
        serie: 1,
        numero,
        tp_emis: 1,
        codigo: Some(12_345_670),
    })
    .unwrap()
}

/// Poe na pasta uma NF-e pendente `numero` do emitente e do ambiente dados.
fn pendente_de(notas: &NotasNfe, emitente: &str, ambiente: &str, numero: u64) -> String {
    let chave = chave_de(emitente, numero);
    let r = Registro {
        chave: chave.clone(),
        serie: "1".into(),
        numero: numero.to_string(),
        emissao: "2026-10-03T09:00:00-03:00".into(),
        ambiente: ambiente.into(),
        ..Registro::default()
    };
    notas.guardar_pendente(&r, "<NFe/>").unwrap();
    chave
}

#[tokio::test]
async fn epec_substituindo_numera_de_novo_liga_as_duas_notas_e_so_quem_nao_tem_resposta_se_substitui(
) {
    let (c, pendente, epec) = com_substituida("nfe-subst-liga").await;
    let (p, e) = (
        c.notas.registro(&pendente).unwrap(),
        c.notas.registro(&epec).unwrap(),
    );
    // numero NOVO (o proximo livre), como manda o Manual de Contingencia; a pendente segue pendente, com o vinculo
    assert_eq!((p.numero.as_str(), e.numero.as_str()), ("1", "2"));
    assert_eq!(
        (p.situacao, e.situacao),
        (Situacao::Pendente, Situacao::Epec)
    );
    assert_eq!(
        (p.substituida_por.as_str(), p.substitui.as_str()),
        (epec.as_str(), "")
    );
    assert_eq!(
        (e.substitui.as_str(), e.substituida_por.as_str()),
        (pendente.as_str(), "")
    );
    assert_eq!(p.a_fazer(), "consultar");
    assert_eq!(e.a_fazer(), "");

    // a mesma nota nao e' substituida duas vezes, e a ja substituida nao vai a rede de novo
    let antes = c.falso.servicos().len();
    let erro = epec_substituindo(&c.ctx(), pedido_epec(), true, Some(&pendente))
        .await
        .unwrap_err();
    assert_eq!(erro.codigo_saida(), 3);
    assert!(erro.to_string().contains("ja foi substituida"), "{erro}");
    assert_eq!(c.falso.servicos().len(), antes);
    assert_eq!(c.notas.listar().unwrap().len(), 2);
}

#[tokio::test]
async fn epec_com_substitui_invalido_recusa_em_palavras_antes_da_rede_e_sem_gastar_numero() {
    let c = Cenario::novo("nfe-subst-invalida", vec![(AUT, tempo()), (CON, tempo())]);
    let ok = emitir(&c.ctx(), pedido(), true).await.unwrap();
    let falta = chave_de("11222333000181", 90);
    let outro_ambiente = pendente_de(&c.notas, "11222333000181", "producao", 92);
    let (antes_rede, antes_notas) = (c.falso.servicos().len(), c.notas.listar().unwrap().len());
    let proximo = c.notas.proximo_numero("1").unwrap();

    for (chave, trecho) in [
        (falta.as_str(), "nao e' de uma NF-e"),
        ("123", "nao e' de uma NF-e"),
        (
            outro_ambiente.as_str(),
            "e' de producao e a configuracao esta em homologacao",
        ),
    ] {
        let erro = epec_substituindo(&c.ctx(), pedido_epec(), true, Some(chave))
            .await
            .unwrap_err();
        assert_eq!(erro.codigo_saida(), 3, "{chave}: {erro}");
        assert!(erro.to_string().contains(trecho), "{chave}: {erro}");
    }
    // a configuracao de outro emitente (CPF): a nota pendente do CNPJ que emitiu aqui nao e' dele
    let mut outra = c.config.clone();
    outra.emitente.cnpj = String::new();
    outra.emitente.cpf = "52998224725".into();
    let ctx = Contexto::novo(&c.cliente, &c.certificado, &outra, Some(&c.notas)).com_agora(agora());
    let erro = epec_substituindo(&ctx, pedido_epec(), true, Some(&ok.chave))
        .await
        .unwrap_err();
    assert!(erro.to_string().contains("outro emitente"), "{erro}");
    // a nota que NAO esta pendente (autorizada) tambem nao se substitui
    c.notas
        .concluir(&ok.chave, Situacao::Autorizada, None, NPROT, "100")
        .unwrap();
    let erro = epec_substituindo(&c.ctx(), pedido_epec(), true, Some(&ok.chave))
        .await
        .unwrap_err();
    assert!(
        erro.to_string().contains("so' se substitui NF-e pendente"),
        "{erro}"
    );

    // nada foi a rede, nada foi gravado e nenhum numero foi gasto
    assert_eq!(c.falso.servicos().len(), antes_rede);
    assert_eq!(c.notas.listar().unwrap().len(), antes_notas);
    assert_eq!(c.notas.proximo_numero("1").unwrap(), proximo);
}

#[tokio::test]
async fn epec_recusado_nao_deixa_vinculo_e_o_substitui_do_pedido_sai_do_pedido() {
    let c = Cenario::novo("nfe-subst-recusado", vec![(AUT, tempo()), (CON, tempo())]);
    let pendente = emitir(&c.ctx(), pedido(), true).await.unwrap();
    c.roteiro(vec![(
        EVT,
        evento("228", "Rejeicao: Data de emissao muito atrasada", ""),
    )]);
    let r = epec_substituindo(&c.ctx(), pedido_epec(), true, Some(&pendente.chave))
        .await
        .unwrap();
    assert_eq!(r.registrado.situacao, None);
    assert_eq!(r.substitui, "");
    assert!(r.para_json().get("substitui").is_none());
    let p = c.notas.registro(&pendente.chave).unwrap();
    assert_eq!(p.substituida_por, "");
    assert_eq!(c.notas.listar().unwrap().len(), 1);

    // a pendente segue como era: a retomada a trataria como qualquer outra (consultaria e reenviaria)
    let mut pedido = json!({"substitui": pendente.chave, "x": 1});
    assert_eq!(
        separar_substitui(&mut pedido).unwrap().as_deref(),
        Some(pendente.chave.as_str())
    );
    assert_eq!(pedido, json!({"x": 1}));
    assert_eq!(separar_substitui(&mut pedido).unwrap(), None);
    assert!(separar_substitui(&mut json!({"substitui": 7})).is_err());
    assert!(separar_substitui(&mut json!({"substitui": ""})).is_err());
}

#[tokio::test]
async fn sem_substitui_o_epec_e_o_de_sempre() {
    let c = Cenario::novo(
        "nfe-subst-sem",
        vec![(EVT, evento("136", "Evento registrado", NPROT_EPEC))],
    );
    let r = epec(&c.ctx(), pedido_epec(), true).await.unwrap();
    assert_eq!(r.situacao, Some(Situacao::Epec));
    let reg = c.notas.registro(&r.chave).unwrap();
    assert_eq!(
        (reg.substitui.as_str(), reg.substituida_por.as_str()),
        ("", "")
    );
    assert!(!serde_json::to_string(&reg).unwrap().contains("substitui"));
}

#[tokio::test]
async fn retomada_da_substituida_autorizada_so_consulta_guarda_o_nfeproc_e_manda_cancelar() {
    let (c, pendente, epec) = com_substituida("nfe-subst-aut").await;
    c.roteiro(vec![
        (CON, consulta("100", "Autorizado o uso da NF-e", NPROT)),
        (
            AUT,
            envio("100", "Autorizado o uso da NF-e", "135260000000002"),
        ),
    ]);
    let feitos = retomar(&depois(&c, 5)).await.unwrap();
    assert_eq!(feitos.len(), 2);

    let f = &feitos[0];
    assert_eq!(
        (f.chave.as_str(), f.acao, f.status),
        (pendente.as_str(), "consultou", "autorizada")
    );
    assert_eq!(f.situacao, Some(Situacao::Autorizada));
    assert_eq!(
        (f.a_fazer(), f.substituida_por()),
        ("cancelar", epec.as_str())
    );
    let aviso = f.aviso.as_deref().unwrap();
    assert!(
        aviso.contains("a venda ja esta na NF-e 2") && aviso.contains("cancele esta nota"),
        "{aviso}"
    );
    let j = f.para_json();
    assert_eq!(
        (j["a_fazer"].as_str(), j["substituida_por"].as_str()),
        (Some("cancelar"), Some(epec.as_str()))
    );

    // a nota autorizada fica certa na pasta: nfeProc com o protocolo, e a copia assinada de antes sai
    let p = c.notas.registro(&pendente).unwrap();
    assert_eq!(
        (p.situacao, p.protocolo.as_str()),
        (Situacao::Autorizada, NPROT)
    );
    assert_eq!(
        p.a_fazer(),
        "cancelar",
        "a nota segue autorizada ate ser cancelada"
    );
    assert!(c.notas.ler_xml(&pendente).unwrap().contains("<nfeProc"));
    assert_eq!(c.notas.ler_assinada(&pendente).unwrap(), None);

    // so' o EPEC foi transmitido: a substituida foi enviada UMA vez, na emissao
    assert_eq!(c.falso.servicos(), vec![AUT, CON, EVT, CON, AUT]);
    assert_eq!(
        envios_da(&c, &pendente),
        1,
        "a substituida nunca e' reenviada"
    );
    assert_eq!(envios_da(&c, &epec), 1);
    assert_eq!(feitos[1].acao, "transmitiu");
    assert!(erro_da_retomada(&feitos).is_none());

    // a norma manda cancelar a autorizada: o caminho de sempre serve
    c.roteiro(vec![(
        EVT,
        evento(
            "135",
            "Evento registrado e vinculado a NF-e",
            "135260000000077",
        ),
    )]);
    let r = cancelar(
        &c.ctx(),
        &pendente,
        "Venda ja registrada na NF-e do EPEC",
        None,
    )
    .await
    .unwrap();
    assert!(r.erro().is_none());
    assert_eq!(r.situacao, Some(Situacao::Cancelada));
}

#[tokio::test]
async fn retomada_da_substituida_que_a_sefaz_nao_tem_nao_reenvia_e_manda_inutilizar() {
    let (c, pendente, epec) = com_substituida("nfe-subst-217").await;
    // so' a consulta do 217 e o envio do EPEC: se a substituida fosse reenviada, o roteiro nao bateria
    c.roteiro(vec![
        (CON, consulta("217", NAO_CONSTA, "")),
        (
            AUT,
            envio("100", "Autorizado o uso da NF-e", "135260000000002"),
        ),
    ]);
    let feitos = retomar(&depois(&c, 5)).await.unwrap();
    let f = &feitos[0];
    assert_eq!(
        (f.chave.as_str(), f.acao, f.status),
        (pendente.as_str(), "consultou", "nao_recebida")
    );
    assert_eq!(f.situacao, Some(Situacao::Pendente));
    assert_eq!(f.a_fazer(), "inutilizar");
    let aviso = f.aviso.as_deref().unwrap();
    assert!(
        aviso.contains("a SEFAZ nao tem esta nota: inutilize o numero 1"),
        "{aviso}"
    );
    assert_eq!(c.falso.servicos(), vec![AUT, CON, EVT, CON, AUT]);
    assert_eq!(
        envios_da(&c, &pendente),
        1,
        "a substituida nunca e' reenviada"
    );
    assert_eq!(envios_da(&c, &epec), 1);

    // o 217 fica no indice, e e' ele que libera a inutilizacao; esperar a inutilizacao nao e' "tente de novo"
    let p = c.notas.registro(&pendente).unwrap();
    assert_eq!((p.cstat.as_str(), p.tentativas), ("217", 1));
    assert_eq!(p.a_fazer(), "inutilizar");
    assert!(erro_da_retomada(&feitos).is_none());
    let lista = pendentes_json(&c.notas.a_resolver().unwrap()).unwrap();
    assert_eq!(lista[0]["a_fazer"], "inutilizar");
    assert_eq!(lista[0]["substituida_por"], epec.as_str());
}

#[tokio::test]
async fn a_consulta_seguinte_sem_217_desfaz_a_permissao_de_inutilizar() {
    let (c, pendente, _) = com_substituida("nfe-subst-desfaz").await;
    // o EPEC sem resposta de novo: o envio e a consulta de recuperacao dele
    c.roteiro(vec![
        (CON, consulta("217", NAO_CONSTA, "")),
        (AUT, tempo()),
        (CON, tempo()),
    ]);
    retomar(&depois(&c, 5)).await.unwrap();
    assert!(c
        .notas
        .registro(&pendente)
        .unwrap()
        .inutilizavel_por_substituicao());
    c.roteiro(vec![(CON, tempo()), (AUT, tempo()), (CON, tempo())]);
    let feitos = retomar(&depois(&c, 10)).await.unwrap();
    assert_eq!(feitos[0].status, "sem_resposta");
    let p = c.notas.registro(&pendente).unwrap();
    assert!(
        !p.inutilizavel_por_substituicao(),
        "sem resposta nao prova que a SEFAZ nao a tem"
    );
    assert_eq!(p.cstat, "");
}

#[tokio::test]
async fn inutilizar_recusa_a_pendente_comum_e_a_substituida_que_ainda_nao_se_sabe_se_a_sefaz_tem() {
    let (c, pendente, epec) = com_substituida("nfe-subst-recusa-inu").await;
    let comum = pendente_de(&c.notas, "11222333000181", "homologacao", 10);
    let antes = c.falso.servicos().len();

    // substituida, mas a retomada ainda nao confirmou o 217
    let erro = inutilizar(&c.ctx(), None, 1, 1, JUST_INU)
        .await
        .unwrap_err();
    assert_eq!(erro.codigo_saida(), 3);
    assert!(erro.to_string().contains(&pendente), "{erro}");
    // pendente comum, ainda que a SEFAZ tenha dito 217: ela nao foi substituida e pode ter sido autorizada
    c.notas.anotar_consulta(&comum, "217").unwrap();
    let erro = inutilizar(&c.ctx(), None, 10, 10, JUST_INU)
        .await
        .unwrap_err();
    assert_eq!(erro.codigo_saida(), 3);
    assert!(erro.to_string().contains(&comum), "{erro}");
    // a faixa que mistura a liberada com notas que nao se inutilizam tambem nao vai
    c.notas.anotar_consulta(&pendente, "217").unwrap();
    let erro = inutilizar(&c.ctx(), None, 1, 10, JUST_INU)
        .await
        .unwrap_err();
    assert!(erro.to_string().contains(&epec), "{erro}");
    assert_eq!(c.falso.servicos().len(), antes, "nenhuma foi a rede");
    assert_eq!(c.notas.listar().unwrap().len(), 3);
}

#[tokio::test]
async fn inutilizar_o_numero_da_substituida_que_a_sefaz_nao_tem_tira_a_nota_da_pasta_e_guarda_o_xml(
) {
    let (c, pendente, epec) = com_substituida("nfe-subst-inu").await;
    c.roteiro(vec![
        (CON, consulta("217", NAO_CONSTA, "")),
        (
            AUT,
            envio("100", "Autorizado o uso da NF-e", "135260000000002"),
        ),
    ]);
    retomar(&depois(&c, 5)).await.unwrap();
    let assinada = c.notas.ler_assinada(&pendente).unwrap().unwrap();

    c.roteiro(vec![(
        INU,
        inutilizacao(
            "102",
            "Inutilizacao de numero homologado",
            "135260000000300",
        ),
    )]);
    let r = inutilizar(&c.ctx(), None, 1, 1, JUST_INU).await.unwrap();
    assert!(r.erro().is_none());
    assert!(r.guardada);
    assert_eq!(r.substituidas, vec![pendente.clone()]);
    assert_eq!(r.para_json()["substituidas"], json!([pendente]));

    // o numero 1 e' agora o da faixa inutilizada; a nota saiu da pasta e nada pendente sobrou
    let faixas = c.notas.inutilizacoes().unwrap();
    assert_eq!((faixas[0].inicio, faixas[0].fim), (1, 1));
    assert!(c.notas.registro(&pendente).is_err());
    assert!(c.notas.a_resolver().unwrap().is_empty());
    assert_eq!(c.notas.proximo_numero("1").unwrap(), 3);
    // o EPEC, autorizado na retomada, segue la com o vinculo para a historia
    let e = c.notas.registro(&epec).unwrap();
    assert_eq!(
        (e.situacao, e.substitui.as_str()),
        (Situacao::Autorizada, pendente.as_str())
    );

    // o XML assinado NAO foi apagado: fica ao lado da faixa, e o original do mes sai
    let base = c.pasta.caminho().join("notas/nfe");
    let guardado = base.join(format!("inutilizacao/2026/1-1-{pendente}-NFe.xml"));
    assert_eq!(std::fs::read_to_string(guardado).unwrap(), assinada);
    assert!(!base.join(format!("2026/10/{pendente}-NFe.xml")).exists());
    // refazer o indice nao ressuscita a nota
    c.notas.reindexar().unwrap();
    let chaves: Vec<String> = c
        .notas
        .listar()
        .unwrap()
        .into_iter()
        .map(|n| n.chave)
        .collect();
    assert_eq!(chaves, vec![epec]);
}

#[tokio::test]
async fn o_indice_antigo_le_igual_e_o_vinculo_sobrevive_ao_indice_refeito() {
    // o indice gravado pela versao anterior, sem os campos novos
    let c = Cenario::novo("nfe-subst-indice", vec![]);
    let antigo = r#"{"versao":1,"notas":[{"chave":"35261011222333000181550010000000011234567012","serie":"1","numero":"1","emissao":"2026-10-03T09:00:00-03:00","valor":"31.80","ambiente":"homologacao","situacao":"pendente","protocolo":"","cstat":"","destinatario_documento":"52998224725","destinatario_nome":"CLIENTE TESTE","protocolo_epec":"","desde":1790000000,"tentativas":2,"ultima":1790000100,"eventos":[]}],"inutilizacoes":[]}"#;
    std::fs::write(c.notas.pasta().join("indice.json"), antigo).unwrap();
    let lista = c.notas.listar().unwrap();
    assert_eq!(lista.len(), 1);
    let r = &lista[0];
    assert_eq!((r.situacao, r.tentativas), (Situacao::Pendente, 2));
    assert_eq!(
        (
            r.substituida_por.as_str(),
            r.substitui.as_str(),
            r.a_fazer()
        ),
        ("", "", "")
    );
    assert!(!r.inutilizavel_por_substituicao());
    // e uma nota comum grava igual a de antes: sem as chaves novas
    let texto = serde_json::to_string(r).unwrap();
    assert!(
        !texto.contains("substituida_por") && !texto.contains("\"substitui\""),
        "{texto}"
    );

    // o vinculo vive so' no indice: refazer o indice dos arquivos nao pode solta-lo (a retomada reenviaria a nota)
    let (c, pendente, epec) = com_substituida("nfe-subst-reindexa").await;
    c.notas.anotar_consulta(&pendente, "217").unwrap();
    c.notas.reindexar().unwrap();
    let p = c.notas.registro(&pendente).unwrap();
    assert_eq!(
        (p.substituida_por.as_str(), p.cstat.as_str()),
        (epec.as_str(), "217")
    );
    assert_eq!(c.notas.registro(&epec).unwrap().substitui, pendente);
}
