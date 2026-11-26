// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A distribuicao de DF-e com transporte falso: o pedido e o envelope, cada desfecho (138 com documentos de todos os
//! tipos, 137, fila em dia, espera, 656, rejeicao, servico parado, sem resposta) e a leitura dos documentos (docZip
//! corrompido, acima do limite, esquema desconhecido). Lacunas, repeticao e o laco `sincronizar` estao em
//! `nfe_sefaz_dfe_lacunas.rs`.

mod nfe_sefaz_comum;

use chrono::Duration;
use nfe_sefaz_comum::dfe::*;
use nfe_sefaz_comum::*;
use rustnf::nfe::sefaz::dfe::{Conteudo, Cursor, DesfechoDfe, MotivoEspera, Nsu, SituacaoResumo};

fn nsu(v: u64) -> Nsu {
    Nsu::ler(&v.to_string()).unwrap()
}

fn cursor_em(ult: u64) -> Cursor {
    Cursor {
        ult_nsu: nsu(ult),
        ..Cursor::novo()
    }
}

// ------------------------------------------------------------------------------------------------ o pedido

#[tokio::test]
async fn o_pedido_vai_ao_ambiente_nacional_na_operacao_do_wsdl_com_o_ultimo_nsu() {
    let f = Falso::novo(vec![(DIST, nada(41))]);
    let (c, _) = cliente_dist(f.clone());
    let r = c.distribuir(CNPJ, &cursor_em(41)).await.unwrap();
    assert_eq!(r.desfecho, DesfechoDfe::EmDia);

    let ch = f.chamadas();
    assert_eq!(ch.len(), 1);
    assert_eq!(ch[0].url, URL_AN_HOMOLOGACAO);
    assert_eq!(
        ch[0].content_type,
        "application/soap+xml; charset=utf-8; action=\"http://www.portalfiscal.inf.br/nfe/wsdl/NFeDistribuicaoDFe/nfeDistDFeInteresse\""
    );
    let ns = "http://www.portalfiscal.inf.br/nfe/wsdl/NFeDistribuicaoDFe";
    assert!(
        ch[0].corpo.contains(&format!(
            "<nfeDistDFeInteresse xmlns=\"{ns}\"><nfeDadosMsg><distDFeInt xmlns=\"{NS}\" versao=\"1.01\">"
        )),
        "{}",
        ch[0].corpo
    );
    assert!(ch[0].corpo.contains(&format!(
        "<tpAmb>2</tpAmb><cUFAutor>35</cUFAutor><CNPJ>{CNPJ}</CNPJ><distNSU><ultNSU>000000000000041</ultNSU></distNSU>"
    )));
    assert!(ch[0].corpo.ends_with(
        "</distDFeInt></nfeDadosMsg></nfeDistDFeInteresse></soap12:Body></soap12:Envelope>"
    ));
}

#[tokio::test]
async fn consulta_por_nsu_e_por_chave_e_interessado_pessoa_fisica() {
    let (_, chave) = nfe(3, 2);
    let f = Falso::novo(vec![(DIST, nada(0)), (DIST, nada(0))]);
    let (c, _) = cliente_dist(f.clone());
    c.distribuir_nsu(CNPJ, &Cursor::novo(), nsu(7))
        .await
        .unwrap();
    c.distribuir_chave(CPF, &Cursor::novo(), &chave)
        .await
        .unwrap();
    let ch = f.chamadas();
    assert!(ch[0]
        .corpo
        .contains("<consNSU><NSU>000000000000007</NSU></consNSU>"));
    assert!(ch[1].corpo.contains(&format!(
        "<CPF>{CPF}</CPF><consChNFe><chNFe>{chave}</chNFe></consChNFe>"
    )));
}

#[tokio::test]
async fn pedido_torto_e_erro_de_uso_e_nao_sai_da_maquina() {
    let f = Falso::novo(vec![]);
    let (c, _) = cliente_dist(f.clone());
    let e = c
        .distribuir("11222333000182", &Cursor::novo())
        .await
        .unwrap_err();
    assert_eq!(e.campo, "CNPJ");
    assert!(e.mensagem.contains("489"), "{e}");
    let e = c
        .distribuir("52998224726", &Cursor::novo())
        .await
        .unwrap_err();
    assert_eq!(e.campo, "CPF");
    let e = c
        .distribuir_chave(CNPJ, &Cursor::novo(), "123")
        .await
        .unwrap_err();
    assert_eq!(e.campo, "chNFe");
    assert!(f.chamadas().is_empty());
}

// ------------------------------------------------------------------------------------------------ 138 com documentos

#[tokio::test]
async fn lote_138_traz_os_quatro_documentos_lidos_e_o_xml_inteiro() {
    let (proc, chave) = proc_nfe(1);
    let resumo = res_nfe(&chave, 1);
    let evento = proc_evento(&chave);
    let res_ev = res_evento(&chave, "210210", 1);
    let docs = [
        doc_zip(11, "resNFe_v1.01.xsd", &resumo),
        doc_zip(12, "procNFe_v4.00.xsd", &proc),
        doc_zip(13, "resEvento_v1.01.xsd", &res_ev),
        doc_zip(14, "procEventoNFe_v1.00.xsd", &evento),
    ];
    let f = Falso::novo(vec![(DIST, lote(14, 90, &docs))]);
    let (c, _) = cliente_dist(f);
    let r = c.distribuir(CNPJ, &cursor_em(10)).await.unwrap();

    assert_eq!(r.desfecho, DesfechoDfe::Parcial);
    assert!(r.ok() && r.ha_mais());
    assert_eq!((r.cstat.as_str(), r.documentos.len()), ("138", 4));
    // o cursor anda para o ultNSU DEVOLVIDO e nao tem espera
    assert_eq!(r.cursor.ult_nsu, nsu(14));
    assert_eq!(r.cursor.max_nsu, Some(nsu(90)));
    assert!(r.cursor.espera.is_none() && !r.cursor.tem_lacunas());
    // cada documento, no tipo certo, com o XML como veio
    let Conteudo::ResumoNfe(rn) = &r.documentos[0].conteudo else {
        panic!("{:?}", r.documentos[0].conteudo)
    };
    assert_eq!(
        (rn.chave.as_str(), rn.emitente.as_str(), rn.situacao.clone()),
        (chave.as_str(), CNPJ, SituacaoResumo::Autorizada)
    );
    assert_eq!(rn.valor.map(|v| v.to_string()), Some("31.80".to_string()));
    assert_eq!(rn.protocolo, "135260000000001");
    assert_eq!(r.documentos[0].xml, resumo);
    let Conteudo::Nfe(n) = &r.documentos[1].conteudo else {
        panic!("{:?}", r.documentos[1].conteudo)
    };
    assert_eq!(n.chave, chave);
    assert!(n.protocolo.is_some());
    assert_eq!(r.documentos[1].xml, proc);
    let Conteudo::ResumoEvento(re) = &r.documentos[2].conteudo else {
        panic!("{:?}", r.documentos[2].conteudo)
    };
    assert_eq!(
        (re.tp_evento.as_str(), re.seq, re.orgao.as_str()),
        ("210210", Some(1), "91")
    );
    let Conteudo::Evento(ev) = &r.documentos[3].conteudo else {
        panic!("{:?}", r.documentos[3].conteudo)
    };
    assert_eq!(
        (ev.tp_evento.as_str(), ev.chave.as_str()),
        ("110111", chave.as_str())
    );
    assert_eq!(r.documentos[3].xml, evento);
    for (d, k) in r.documentos.iter().zip(11..) {
        assert_eq!(d.nsu, nsu(k));
        assert_eq!(d.chave(), chave);
        assert!(!d.e_ilegivel());
    }
    assert!(r.ilegiveis().is_empty());
}

#[tokio::test]
async fn resumo_de_nota_cancelada_e_denegada_diz_a_situacao() {
    let (_, chave) = nfe(4, 2);
    let docs = [
        doc_zip(1, "resNFe_v1.01.xsd", &res_nfe(&chave, 3)),
        doc_zip(2, "resNFe_v1.01.xsd", &res_nfe(&chave, 2)),
        doc_zip(3, "resNFe_v1.01.xsd", &res_nfe(&chave, 9)),
    ];
    let (c, _) = cliente_dist(Falso::novo(vec![(DIST, lote(3, 3, &docs))]));
    let r = c.distribuir(CNPJ, &Cursor::novo()).await.unwrap();
    let sit: Vec<SituacaoResumo> = r
        .documentos
        .iter()
        .map(|d| match &d.conteudo {
            Conteudo::ResumoNfe(x) => x.situacao.clone(),
            o => panic!("{o:?}"),
        })
        .collect();
    assert_eq!(
        sit,
        vec![
            SituacaoResumo::Cancelada,
            SituacaoResumo::Denegada,
            SituacaoResumo::Outra("9".into())
        ]
    );
}

// ------------------------------------------------------------------------------------------------ fila em dia e espera

#[tokio::test]
async fn ultnsu_igual_ao_maxnsu_com_138_grava_a_espera_de_uma_hora() {
    let (_, chave) = nfe(5, 2);
    let docs = [doc_zip(5, "resNFe_v1.01.xsd", &res_nfe(&chave, 1))];
    let (c, _) = cliente_dist(Falso::novo(vec![(DIST, lote(5, 5, &docs))]));
    let r = c.distribuir(CNPJ, &cursor_em(4)).await.unwrap();
    assert_eq!(r.desfecho, DesfechoDfe::EmDia);
    assert_eq!(r.documentos.len(), 1);
    let e = r.cursor.espera.unwrap();
    assert_eq!(e.motivo, MotivoEspera::FilaEmDia);
    assert_eq!(e.ate, meio_dia() + Duration::hours(1));
    // 13:00 UTC = 10:00 em Brasilia
    assert!(r.frase.contains("10:00"), "{}", r.frase);
}

#[tokio::test]
async fn o_137_pede_a_espera_e_a_proxima_distribuicao_nem_sai_antes_de_uma_hora() {
    let f = Falso::novo(vec![(DIST, nada(9)), (DIST, nada(9))]);
    let (c, relogio) = cliente_dist(f.clone());
    let r1 = c.distribuir(CNPJ, &cursor_em(9)).await.unwrap();
    assert_eq!(r1.desfecho, DesfechoDfe::EmDia);
    assert_eq!((r1.cstat.as_str(), r1.documentos.len()), ("137", 0));

    // 59 minutos depois: nada sai
    *relogio.lock().unwrap() = meio_dia() + Duration::minutes(59);
    let r2 = c.distribuir(CNPJ, &r1.cursor).await.unwrap();
    assert!(
        matches!(
            r2.desfecho,
            DesfechoDfe::Aguarde {
                motivo: MotivoEspera::FilaEmDia,
                ..
            }
        ),
        "{:?}",
        r2.desfecho
    );
    assert_eq!(r2.cursor, r1.cursor);
    assert_eq!(f.chamadas().len(), 1);
    assert!(!r2.ok());

    // uma hora depois: sai
    *relogio.lock().unwrap() = meio_dia() + Duration::hours(1);
    let r3 = c.distribuir(CNPJ, &r1.cursor).await.unwrap();
    assert_eq!(r3.desfecho, DesfechoDfe::EmDia);
    assert_eq!(f.chamadas().len(), 2);
    assert_eq!(f.restantes(), 0);
}

#[tokio::test]
async fn a_espera_da_fila_nao_barra_a_consulta_de_um_nsu_nem_de_uma_chave() {
    // a NT manda esperar uma hora para a DISTRIBUICAO; consNSU e consChNFe sao outras consultas
    let (_, chave) = nfe(6, 2);
    let f = Falso::novo(vec![(DIST, nada(9)), (DIST, nada(0)), (DIST, nada(0))]);
    let (c, _) = cliente_dist(f.clone());
    let r = c.distribuir(CNPJ, &cursor_em(9)).await.unwrap();
    let n = c.distribuir_nsu(CNPJ, &r.cursor, nsu(4)).await.unwrap();
    assert_eq!(n.desfecho, DesfechoDfe::NaoLocalizado);
    let k = c.distribuir_chave(CNPJ, &r.cursor, &chave).await.unwrap();
    assert_eq!(k.desfecho, DesfechoDfe::NaoLocalizado);
    assert_eq!(f.chamadas().len(), 3);
}

// ------------------------------------------------------------------------------------------------ 656

fn consumo_indevido() -> Resp {
    ok_dist(&ret_dist(
        "656",
        "Rejeicao: Consumo Indevido. Deve ser aguardado 1 hora para efetuar nova solicitacao caso nao existam mais documentos a serem pesquisados. Tente apos 1 hora",
        0,
        0,
        &[],
    ))
}

#[tokio::test]
async fn o_656_bloqueia_por_uma_hora_diz_quando_tentar_e_trava_toda_consulta() {
    let (_, chave) = nfe(7, 2);
    let f = Falso::novo(vec![(DIST, consumo_indevido())]);
    let (c, relogio) = cliente_dist(f.clone());
    let r = c.distribuir(CNPJ, &cursor_em(30)).await.unwrap();

    let ate = meio_dia() + Duration::hours(1);
    assert_eq!(r.desfecho, DesfechoDfe::ConsumoIndevido { ate });
    assert!(!r.ok());
    // o cursor guarda o bloqueio e NAO anda
    assert_eq!(r.cursor.ult_nsu, nsu(30));
    let e = r.cursor.espera.unwrap();
    assert_eq!((e.ate, e.motivo), (ate, MotivoEspera::ConsumoIndevido));
    assert!(
        r.frase.contains("656") && r.frase.contains("10:00"),
        "{}",
        r.frase
    );
    assert_eq!(r.cstat, "656");

    // dentro do bloqueio nada sai: nem distribuicao, nem NSU, nem chave
    *relogio.lock().unwrap() = meio_dia() + Duration::minutes(30);
    for r2 in [
        c.distribuir(CNPJ, &r.cursor).await.unwrap(),
        c.distribuir_nsu(CNPJ, &r.cursor, nsu(1)).await.unwrap(),
        c.distribuir_chave(CNPJ, &r.cursor, &chave).await.unwrap(),
    ] {
        assert!(matches!(
            r2.desfecho,
            DesfechoDfe::Aguarde {
                motivo: MotivoEspera::ConsumoIndevido,
                ..
            }
        ));
    }
    assert_eq!(f.chamadas().len(), 1);
}

#[tokio::test]
async fn o_656_da_consulta_por_chave_tambem_grava_a_janela() {
    let (_, chave) = nfe(8, 2);
    let f = Falso::novo(vec![(DIST, consumo_indevido())]);
    let (c, _) = cliente_dist(f);
    let r = c
        .distribuir_chave(CNPJ, &cursor_em(2), &chave)
        .await
        .unwrap();
    assert!(matches!(r.desfecho, DesfechoDfe::ConsumoIndevido { .. }));
    assert_eq!(
        r.cursor.espera.unwrap().motivo,
        MotivoEspera::ConsumoIndevido
    );
}

// ------------------------------------------------------------------------------------------------ rejeicao, parado, sem resposta

#[tokio::test]
async fn rejeicao_nao_e_zero_notas_e_nao_mexe_no_cursor_nem_na_espera() {
    let ret = ret_dist(
        "593",
        "Rejeicao: CNPJ-Base consultado difere do CNPJ-Base do Certificado Digital",
        0,
        0,
        &[],
    );
    let (c, _) = cliente_dist(Falso::novo(vec![(DIST, ok_dist(&ret))]));
    let antes = cursor_em(12);
    let r = c.distribuir(CNPJ, &antes).await.unwrap();
    assert_eq!(
        r.desfecho,
        DesfechoDfe::Rejeitada {
            cstat: "593".into(),
            motivo: "Rejeicao: CNPJ-Base consultado difere do CNPJ-Base do Certificado Digital"
                .into()
        }
    );
    assert_eq!(r.cursor, antes);
    assert!(!r.ok() && r.documentos.is_empty());
    assert!(r.frase.contains("593"));
}

#[tokio::test]
async fn servico_parado_108_e_falta_de_resposta_voltam_indisponiveis_sem_mexer_no_cursor() {
    let ret = ret_dist(
        "108",
        "Servico Paralisado Momentaneamente (curto prazo)",
        0,
        0,
        &[],
    );
    let f = Falso::novo(vec![
        (DIST, ok_dist(&ret)),
        (DIST, tempo()),
        (DIST, fault("falha interna")),
        (DIST, Resp::R(200, b"<html>manutencao</html>".to_vec())),
    ]);
    let (c, _) = cliente_dist(f);
    let antes = cursor_em(12);
    for k in 0..4 {
        let r = c.distribuir(CNPJ, &antes).await.unwrap();
        assert_eq!(r.desfecho, DesfechoDfe::Indisponivel, "{k}");
        assert_eq!(r.cursor, antes, "{k}");
        assert!(r.documentos.is_empty());
        assert!(!r.frase.contains("html"), "{}", r.frase);
    }
}

// ------------------------------------------------------------------------------------------------ o que nao abre ou nao e' reconhecido

#[tokio::test]
async fn docszip_corrompido_vira_documento_ilegivel_com_o_original_e_o_cursor_segue() {
    let (_, chave) = nfe(9, 2);
    let docs = [
        doc_zip(21, "resNFe_v1.01.xsd", &res_nfe(&chave, 1)),
        doc_zip_bruto(22, "resNFe_v1.01.xsd", "@@@nao e base64@@@"),
        doc_zip_bruto(23, "resNFe_v1.01.xsd", &base64_de(b"isto nao e gzip")),
        doc_zip(24, "resNFe_v1.01.xsd", "<resNFe>quebrado"),
    ];
    let (c, _) = cliente_dist(Falso::novo(vec![(DIST, lote(24, 24, &docs))]));
    let r = c.distribuir(CNPJ, &cursor_em(20)).await.unwrap();

    // os quatro chegam: nada e' descartado, e o cursor anda ate o ultNSU
    assert_eq!(r.documentos.len(), 4);
    assert_eq!(r.cursor.ult_nsu, nsu(24));
    assert_eq!(r.ilegiveis(), vec![nsu(22), nsu(23), nsu(24)]);
    let ruim = &r.documentos[1];
    assert_eq!(ruim.doczip_original.as_deref(), Some("@@@nao e base64@@@"));
    assert!(ruim.xml.is_empty());
    let Conteudo::Ilegivel { motivo } = &ruim.conteudo else {
        panic!()
    };
    assert!(motivo.contains("base64"), "{motivo}");
    let Conteudo::Ilegivel { motivo } = &r.documentos[2].conteudo else {
        panic!()
    };
    assert!(motivo.contains("gzip"), "{motivo}");
    // XML que abre mas nao e' o resumo: o texto vem inteiro e o motivo diz o esquema
    let xml_ruim = &r.documentos[3];
    assert_eq!(xml_ruim.xml, "<resNFe>quebrado");
    assert!(xml_ruim.doczip_original.is_none());
    let Conteudo::Ilegivel { motivo } = &xml_ruim.conteudo else {
        panic!()
    };
    assert!(motivo.starts_with("resNFe_v1.01.xsd"), "{motivo}");
}

fn base64_de(b: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(b)
}

#[tokio::test]
async fn docszip_acima_do_limite_de_descompressao_e_recusado_sem_estourar_a_memoria() {
    // 21 MB de zeros cabem em ~21 KB de gzip: a "bomba"; o limite da leitura da NF-e e' 20 MB
    let bomba = gz(&vec![0u8; 21 * 1024 * 1024]);
    assert!(bomba.len() < 100 * 1024);
    let docs = [doc_zip_bruto(31, "procNFe_v4.00.xsd", &base64_de(&bomba))];
    let (c, _) = cliente_dist(Falso::novo(vec![(DIST, lote(31, 31, &docs))]));
    let r = c.distribuir(CNPJ, &cursor_em(30)).await.unwrap();
    let d = &r.documentos[0];
    let Conteudo::Ilegivel { motivo } = &d.conteudo else {
        panic!("{:?}", d.conteudo)
    };
    assert!(motivo.contains("passa de 20971520 bytes"), "{motivo}");
    assert!(d.xml.is_empty() && d.doczip_original.is_some());
    assert_eq!(r.ilegiveis(), vec![nsu(31)]);
}

#[tokio::test]
async fn esquema_desconhecido_volta_cru_e_nunca_e_descartado() {
    let estranho = "<resCTe xmlns=\"http://www.portalfiscal.inf.br/nfe\"><x>1</x></resCTe>";
    let docs = [doc_zip(41, "resCTe_v1.00.xsd", estranho)];
    let (c, _) = cliente_dist(Falso::novo(vec![(DIST, lote(41, 50, &docs))]));
    let r = c.distribuir(CNPJ, &cursor_em(40)).await.unwrap();
    assert_eq!(r.documentos.len(), 1);
    assert_eq!(r.documentos[0].conteudo, Conteudo::OutroEsquema);
    assert_eq!(r.documentos[0].xml, estranho);
    assert_eq!(r.documentos[0].esquema, "resCTe_v1.00.xsd");
    assert!(r.ilegiveis().is_empty());
}

#[tokio::test]
async fn nota_de_outro_modelo_ou_evento_quebrado_vira_ilegivel_com_o_xml() {
    let sem_ret = "<procEventoNFe xmlns=\"http://www.portalfiscal.inf.br/nfe\" versao=\"1.00\"><evento versao=\"1.00\"><infEvento Id=\"X\"/></evento></procEventoNFe>";
    let docs = [doc_zip(51, "procEventoNFe_v1.00.xsd", sem_ret)];
    let (c, _) = cliente_dist(Falso::novo(vec![(DIST, lote(51, 51, &docs))]));
    let r = c.distribuir(CNPJ, &cursor_em(50)).await.unwrap();
    assert!(r.documentos[0].e_ilegivel());
    assert_eq!(r.documentos[0].xml, sem_ret);
}

// ------------------------------------------------------------------------------------------------ consNSU e consChNFe

#[tokio::test]
async fn consnsu_rele_o_ilegivel_e_consulta_por_chave_traz_o_resumo() {
    let (_, chave) = nfe(10, 2);
    let f = Falso::novo(vec![
        (
            DIST,
            lote(
                22,
                22,
                &[doc_zip(22, "resNFe_v1.01.xsd", &res_nfe(&chave, 1))],
            ),
        ),
        (
            DIST,
            lote(
                70,
                70,
                &[doc_zip(70, "resNFe_v1.01.xsd", &res_nfe(&chave, 1))],
            ),
        ),
    ]);
    let (c, _) = cliente_dist(f);
    let antes = cursor_em(30);
    let r = c.distribuir_nsu(CNPJ, &antes, nsu(22)).await.unwrap();
    assert_eq!(r.desfecho, DesfechoDfe::Localizado);
    assert_eq!(r.documentos[0].nsu, nsu(22));
    // o NSU 22 ja estava coberto pelo cursor: reler nao mexe nele
    assert_eq!(r.cursor, antes);
    let k = c.distribuir_chave(CNPJ, &antes, &chave).await.unwrap();
    assert_eq!(k.desfecho, DesfechoDfe::Localizado);
    assert_eq!(k.cursor, antes);
    assert_eq!(k.documentos[0].chave(), chave);
}
