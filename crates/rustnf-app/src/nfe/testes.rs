// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A emissao: o pedido completado pela configuracao e um teste por desfecho, com as regras de numeracao.

use serde_json::json;

use super::testes_comuns::*;
use super::{completar_pedido, emitir, Contexto};
use crate::erro::Erro;
use crate::notas::nfe::Situacao;
use crate::testes_comuns::PastaTemporaria;

#[test]
fn o_pedido_e_completado_com_o_emitente_a_uf_o_ambiente_e_o_numero_da_pasta() {
    let pasta = PastaTemporaria::nova("nfe-completar");
    let config = config(&pasta);
    let mut p = pedido();
    completar_pedido(&mut p, &config, Some(7), agora(), false).unwrap();
    assert_eq!(p["emit"]["documento"], "11222333000181");
    assert_eq!(p["emit"]["nome"], "LOJA DE TESTE LTDA");
    assert_eq!(p["emit"]["crt"], 1);
    assert_eq!(p["emit"]["ie"], "111111111111");
    assert_eq!(p["emit"]["endereco"]["uf"], "SP");
    assert_eq!(p["emit"]["endereco"]["cod_municipio"], "3550308");
    assert_eq!(p["cmun_fg"], "3550308");
    assert_eq!(p["tp_amb"], 2, "homologacao e' o padrao");
    assert_eq!(p["serie"], "1");
    assert_eq!(p["numero"], 7);
    assert_eq!(p["modelo"], 55);
    assert!(p["codigo"].as_u64().is_some(), "o cNF e' gerado");
    // o que o pedido traz vale
    assert_eq!(p["emissao"], "2026-10-03T09:00:00-03:00");
}

#[test]
fn o_pedido_vence_a_configuracao_e_a_data_ausente_sai_em_brasilia_com_a_folga() {
    let pasta = PastaTemporaria::nova("nfe-vence");
    let config = config(&pasta);
    let mut p = json!({"serie": 9, "numero": 12, "tp_amb": 1, "codigo": 12345670,
                       "emit": {"nome": "OUTRO NOME", "crt": 3}});
    completar_pedido(&mut p, &config, Some(99), agora(), false).unwrap();
    assert_eq!(p["serie"], 9);
    assert_eq!(p["numero"], 12);
    assert_eq!(p["tp_amb"], 1);
    assert_eq!(p["codigo"], 12345670);
    assert_eq!(p["emit"]["nome"], "OUTRO NOME");
    assert_eq!(p["emit"]["crt"], 3);
    // 12:00 UTC menos a folga de 60 s do [sefin], em horario de Brasilia
    assert_eq!(p["emissao"], "2026-10-03T08:59:00-03:00");
}

#[test]
fn contingencia_svc_poe_o_tpemis_da_uf_o_motivo_e_a_data_de_entrada() {
    let pasta = PastaTemporaria::nova("nfe-svc");
    let mut config = config(&pasta);
    config.nfe.contingencia = "svc".into();
    config.nfe.motivo_contingencia = "Falha de internet na matriz".into();
    let mut p = pedido();
    completar_pedido(&mut p, &config, Some(1), agora(), false).unwrap();
    // SP e' atendida pela SVC-AN (tpEmis 6)
    assert_eq!(p["tp_emis"], 6);
    assert_eq!(p["x_just"], "Falha de internet na matriz");
    assert_eq!(p["dh_cont"], p["emissao"]);
}

#[test]
fn sem_dado_de_configuracao_o_erro_diz_onde_informar() {
    let pasta = PastaTemporaria::nova("nfe-faltas");
    let mut config = config(&pasta);
    config.nfe.uf = String::new();
    let erro = completar_pedido(&mut pedido(), &config, Some(1), agora(), false).unwrap_err();
    assert!(
        matches!(erro, Erro::Uso(ref m) if m.contains("[nfe].uf")),
        "{erro}"
    );
    let mut config = crate::nfe::testes_comuns::config(&pasta);
    config.nfe.crt = 0;
    let erro = completar_pedido(&mut pedido(), &config, Some(1), agora(), false).unwrap_err();
    assert!(
        matches!(erro, Erro::Uso(ref m) if m.contains("[nfe].crt")),
        "{erro}"
    );
}

#[tokio::test]
async fn autorizada_guarda_o_nfeproc_gasta_o_numero_e_manda_o_emitente_da_configuracao() {
    let c = Cenario::novo(
        "nfe-aut",
        vec![(AUT, envio("100", "Autorizado o uso da NF-e", NPROT))],
    );
    let e = emitir(&c.ctx(), pedido(), true).await.unwrap();
    assert_eq!(e.situacao, Some(Situacao::Autorizada));
    assert_eq!((e.serie.as_str(), e.numero), ("1", 1));
    assert!(e.erro().is_none());
    assert!(e.frase().contains("guardada na pasta"), "{}", e.frase());
    let j = e.para_json();
    assert_eq!(j["status"], "autorizada");
    assert_eq!(j["protocolo"], NPROT);
    assert_eq!(j["guardada"], true);

    let r = c.notas.registro(&e.chave).unwrap();
    assert_eq!(r.situacao, Situacao::Autorizada);
    assert_eq!(r.protocolo, NPROT);
    assert_eq!(r.valor, "31.80");
    // em homologacao a biblioteca poe o texto do MOC no nome do destinatario
    assert_eq!(
        r.destinatario_nome,
        "NF-E EMITIDA EM AMBIENTE DE HOMOLOGACAO - SEM VALOR FISCAL"
    );
    let dir = c.pasta.caminho().join("notas/nfe/2026/10");
    assert!(dir.join(format!("{}-procNFe.xml", e.chave)).is_file());
    assert!(
        !dir.join(format!("{}-NFe.xml", e.chave)).exists(),
        "a copia assinada de antes sai quando o nfeProc entra"
    );
    assert!(c.notas.ler_xml(&e.chave).unwrap().contains("<nfeProc"));
    assert_eq!(c.notas.proximo_numero("1").unwrap(), 2);

    // o que foi para a SEFAZ tem os dados da configuracao
    let corpo = &c.falso.corpos()[0];
    for trecho in [
        "<CNPJ>11222333000181</CNPJ>",
        "<IE>111111111111</IE>",
        "<UF>SP</UF>",
        "<CRT>1</CRT>",
        "<tpAmb>2</tpAmb>",
    ] {
        assert!(corpo.contains(trecho), "faltou {trecho}");
    }
}

#[tokio::test]
async fn rejeitada_nao_gasta_o_numero_e_a_mesma_numeracao_serve_ao_pedido_corrigido() {
    let c = Cenario::novo(
        "nfe-rej",
        vec![(
            AUT,
            envio("225", "Rejeicao: Falha no Schema XML do lote de NFe", ""),
        )],
    );
    let e = emitir(&c.ctx(), pedido(), true).await.unwrap();
    assert_eq!(e.situacao, None, "a rejeitada sai da pasta");
    let erro = e.erro().unwrap();
    assert_eq!(erro.codigo_saida(), 3);
    assert!(e.frase().contains("continua livre"), "{}", e.frase());
    assert_eq!(e.para_json()["status"], "rejeitada");
    assert!(c.notas.listar().unwrap().is_empty());
    assert_eq!(
        c.notas.proximo_numero("1").unwrap(),
        1,
        "rejeitada nao consome"
    );
    let dir = c.pasta.caminho().join("notas/nfe/2026/10");
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0, "nada ficou");

    // o mesmo numero, corrigido, e' autorizado
    c.roteiro(vec![(AUT, envio("100", "Autorizado o uso da NF-e", NPROT))]);
    let e2 = emitir(&c.ctx(), pedido(), true).await.unwrap();
    assert_eq!(e2.numero, 1);
    assert_eq!(e2.situacao, Some(Situacao::Autorizada));
    assert_eq!(c.notas.proximo_numero("1").unwrap(), 2);
}

#[tokio::test]
async fn denegada_gasta_o_numero_e_e_recusa_de_regra() {
    let c = Cenario::novo(
        "nfe-den",
        vec![(
            AUT,
            envio(
                "302",
                "Uso Denegado: Irregularidade fiscal do destinatario",
                NPROT,
            ),
        )],
    );
    let e = emitir(&c.ctx(), pedido(), true).await.unwrap();
    assert_eq!(e.situacao, Some(Situacao::Denegada));
    assert_eq!(e.erro().unwrap().codigo_saida(), 3);
    assert_eq!(e.para_json()["status"], "denegada");
    assert_eq!(
        c.notas.proximo_numero("1").unwrap(),
        2,
        "denegada gasta o numero"
    );
    assert_eq!(
        c.notas.registro(&e.chave).unwrap().situacao,
        Situacao::Denegada
    );
    assert!(c.notas.ler_xml(&e.chave).unwrap().contains("<nfeProc"));
}

#[tokio::test]
async fn numero_ja_autorizado_com_outra_chave_gasta_o_numero_sem_guardar_nota() {
    let c = Cenario::novo(
        "nfe-539",
        vec![(
            AUT,
            envio(
                "539",
                "Rejeicao: Duplicidade de NF-e com diferenca na Chave de Acesso",
                "",
            ),
        )],
    );
    let e = emitir(&c.ctx(), pedido(), true).await.unwrap();
    assert_eq!(e.situacao, None);
    assert_eq!(e.para_json()["status"], "numero_ja_usado");
    assert!(c.notas.listar().unwrap().is_empty());
    assert_eq!(
        c.notas.proximo_numero("1").unwrap(),
        2,
        "a SEFAZ tem o numero 1"
    );
}

#[tokio::test]
async fn sem_resposta_a_nota_fica_pendente_na_pasta_e_o_numero_segue_reservado() {
    let c = Cenario::novo("nfe-pend", vec![(AUT, tempo()), (CON, tempo())]);
    let e = emitir(&c.ctx(), pedido(), true).await.unwrap();
    assert_eq!(e.situacao, Some(Situacao::Pendente));
    assert_eq!(e.erro().unwrap().codigo_saida(), 4);
    assert!(e.frase().contains("PENDENTE"), "{}", e.frase());
    assert!(e.para_json()["guardada"].as_bool().unwrap());
    let r = c.notas.registro(&e.chave).unwrap();
    assert_eq!(r.situacao, Situacao::Pendente);
    assert_eq!(r.desde, agora().timestamp());
    let dir = c.pasta.caminho().join("notas/nfe/2026/10");
    assert!(dir.join(format!("{}-NFe.xml", e.chave)).is_file());
    assert_eq!(c.notas.proximo_numero("1").unwrap(), 2);
}

#[tokio::test]
async fn a_nfe_ja_esta_gravada_como_pendente_quando_a_rede_e_tocada() {
    // o transporte que, ao receber o envio, olha a pasta: a NF-e assinada e o indice ja tem de estar la
    let c = Cenario::novo("nfe-antes", vec![]);
    let raiz = c.pasta.caminho().join("notas/nfe");
    let confere = move |corpo: &str| {
        // o envio leva a chave no Id da NF-e; a consulta, no chNFe
        let chave = if corpo.contains("<enviNFe") {
            entre(corpo, "Id=\"NFe", "\"")
        } else {
            entre(corpo, "<chNFe>", "</chNFe>")
        }
        .to_string();
        assert_eq!(chave.len(), 44);
        let assinada = raiz.join(format!("2026/10/{chave}-NFe.xml"));
        assert!(assinada.is_file(), "a NF-e assinada nao estava na pasta");
        assert!(std::fs::read_to_string(assinada)
            .unwrap()
            .contains("<Signature"));
        let indice = std::fs::read_to_string(raiz.join("indice.json")).unwrap();
        assert!(
            indice.contains(&chave) && indice.contains("\"pendente\""),
            "{indice}"
        );
        tempo()
    };
    let confere2 = confere.clone();
    c.roteiro(vec![
        (AUT, Resp::Din(Box::new(confere))),
        (CON, Resp::Din(Box::new(confere2))),
    ]);
    let e = emitir(&c.ctx(), pedido(), true).await.unwrap();
    assert_eq!(e.situacao, Some(Situacao::Pendente));
}

#[tokio::test]
async fn numero_ja_gasto_ou_pendente_e_recusado_antes_da_rede() {
    let c = Cenario::novo(
        "nfe-ocupado",
        vec![(AUT, envio("100", "Autorizado o uso da NF-e", NPROT))],
    );
    emitir(&c.ctx(), pedido(), true).await.unwrap();
    let mut p = pedido();
    p["numero"] = json!(1);
    let erro = emitir(&c.ctx(), p, true).await.unwrap_err();
    assert_eq!(erro.codigo_saida(), 3);
    assert!(erro.to_string().contains("ja foi gasto"), "{erro}");
    assert_eq!(c.falso.servicos(), vec![AUT], "nada foi enviado de novo");
}

#[tokio::test]
async fn pedido_que_a_conferencia_recusa_nao_gasta_numero_nem_vai_a_rede() {
    let c = Cenario::novo("nfe-confere", vec![]);
    let mut p = pedido();
    p["itens"][0]["ncm"] = json!("123");
    let erro = emitir(&c.ctx(), p, true).await.unwrap_err();
    assert_eq!(erro.codigo_saida(), 3, "{erro}");
    assert!(c.notas.listar().unwrap().is_empty());
    assert!(c.falso.servicos().is_empty());
    assert_eq!(c.notas.proximo_numero("1").unwrap(), 1);
}

#[tokio::test]
async fn campo_desconhecido_e_recusado_por_padrao_e_ignorado_com_aviso_quando_admitido() {
    let c = Cenario::novo(
        "nfe-desc",
        vec![(AUT, envio("100", "Autorizado o uso da NF-e", NPROT))],
    );
    let mut p = pedido();
    p["campo_inventado"] = json!("x");
    let erro = emitir(&c.ctx(), p.clone(), true).await.unwrap_err();
    assert_eq!(erro.codigo_saida(), 3);
    assert!(erro.to_string().contains("campo_inventado"), "{erro}");
    let e = emitir(&c.ctx(), p, false).await.unwrap();
    assert_eq!(e.desconhecidos, vec!["campo_inventado".to_string()]);
    assert_eq!(e.para_json()["campos_desconhecidos"][0], "campo_inventado");
}

#[tokio::test]
async fn sem_pasta_de_notas_a_emissao_recusa_em_vez_de_perder_a_chave() {
    let c = Cenario::novo("nfe-sem-pasta", vec![]);
    let ctx = Contexto::novo(&c.cliente, &c.certificado, &c.config, None).com_agora(agora());
    let erro = emitir(&ctx, pedido(), true).await.unwrap_err();
    assert!(
        matches!(erro, Erro::Uso(ref m) if m.contains("pasta de notas")),
        "{erro}"
    );
    assert!(c.falso.servicos().is_empty());
}

#[tokio::test]
async fn aviso_da_montagem_volta_junto_e_nao_impede_a_nota() {
    // PIS/COFINS no regime cumulativo com CST 01 e aliquota que nao e' a do regime, so' no CRT 3
    let c = Cenario::com(
        "nfe-aviso",
        "",
        vec![(AUT, envio("100", "Autorizado o uso da NF-e", NPROT))],
    );
    let mut config = c.config.clone();
    config.nfe.crt = 3;
    config.nfe.regime_pis_cofins = "cumulativo".into();
    let ctx =
        Contexto::novo(&c.cliente, &c.certificado, &config, Some(&c.notas)).com_agora(agora());
    let mut p = pedido();
    p["itens"][0]["icms"] = json!({"codigo": "00", "orig": 0, "mod_bc": 3, "vbc": "31.80", "p_icms": "18", "v_icms": "5.72"});
    p["itens"][0]["pis"] =
        json!({"cst": "01", "vbc": "31.80", "aliquota": "1.65", "valor": "0.52"});
    p["itens"][0]["cofins"] =
        json!({"cst": "01", "vbc": "31.80", "aliquota": "7.60", "valor": "2.42"});
    let e = emitir(&ctx, p, true).await.unwrap();
    assert_eq!(e.situacao, Some(Situacao::Autorizada));
    assert!(
        e.avisos
            .iter()
            .any(|a| a.tipo.codigo() == "pis_cofins_regime"),
        "{:?}",
        e.avisos
    );
    assert_eq!(e.para_json()["avisos"][0]["codigo"], "pis_cofins_regime");
}
