// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A sincronizacao dos documentos recebidos com transporte falso: varios lotes ate o 137, queda no meio de um lote (nunca
//! pula NSU), o bloqueio 656 e a espera, a manifestacao e o download por chave.

use rustnf::nfe::sefaz::evento::TipoManifestacao;

use super::testes_comuns::*;
use super::testes_dfe::*;
use super::{baixar, listar, manifestar, sincronizar, FiltroRecebidas};
use crate::erro::Erro;
use crate::notas::nfe::SituacaoRecebida;

fn pasta_recebidas(c: &Cenario) -> std::path::PathBuf {
    c.notas.pasta().join("recebidas")
}

/// Cenario com tres NF-e: o `procNFe` de cada uma e a chave.
fn notas_de_teste(c: &Cenario) -> Vec<(String, String)> {
    (1..=3).map(|n| proc_nfe(&c.config, n)).collect()
}

fn base() -> Cenario {
    Cenario::novo("recebidas", vec![])
}

#[tokio::test]
async fn sincroniza_varios_lotes_ate_o_137_gravando_antes_de_andar_o_cursor() {
    let c = base();
    let notas = notas_de_teste(&c);
    c.roteiro(vec![
        (
            DIST,
            lote(
                2,
                5,
                &[doc_resumo(1, &notas[0].1), doc_completa(2, &notas[1].0)],
            ),
        ),
        (
            DIST,
            lote(
                4,
                5,
                &[
                    doc_resumo(3, &notas[2].1),
                    doc_zip(
                        4,
                        "resEvento_v1.01.xsd",
                        &res_evento(&notas[0].1, "110111", 1),
                    ),
                ],
            ),
        ),
        (DIST, nada(4)),
    ]);
    // o terceiro pedido e' o 137: a espera ainda nao esta no cursor, entao o laco chama de novo
    let r = sincronizar(&c.ctx(), None).await.unwrap();
    assert!(r.erro().is_none(), "{:?}", r.erro());
    assert_eq!(r.resultado.lotes, 3);
    assert_eq!(c.falso.restantes(), 0);
    assert_eq!(r.contagem.total(), 4);
    assert_eq!(
        (r.contagem.resumos, r.contagem.notas, r.contagem.eventos),
        (2, 1, 1)
    );
    assert!(r.frase.contains("a fila esta em dia"), "{}", r.frase);
    assert!(r.frase.contains("10:00"), "{}", r.frase);
    assert_eq!(r.para_json()["status"], "em_dia");
    assert_eq!(r.para_json()["cursor"]["ult_nsu"], "000000000000004");

    // a pasta: arquivos pela chave, indice e cursor
    let mes = pasta_recebidas(&c).join("2026").join("10");
    assert!(mes.join(format!("{}-resNFe.xml", notas[0].1)).is_file());
    assert!(mes.join(format!("{}-procNFe.xml", notas[1].1)).is_file());
    assert!(mes
        .join(format!("{}-110111-1-resEvento.xml", notas[0].1))
        .is_file());
    let todas = listar(&c.notas, &FiltroRecebidas::default()).unwrap();
    assert_eq!(todas.len(), 3);
    let por_chave = |ch: &str| todas.iter().find(|x| x.chave == ch).unwrap();
    // a primeira foi cancelada pelo emitente (o evento chegou), a segunda veio completa, a terceira so' em resumo
    assert_eq!(por_chave(&notas[0].1).situacao, SituacaoRecebida::Cancelada);
    assert_eq!(por_chave(&notas[1].1).situacao, SituacaoRecebida::Completa);
    assert_eq!(por_chave(&notas[1].1).valor, "31.80");
    assert_eq!(por_chave(&notas[2].1).situacao, SituacaoRecebida::Resumo);
    let cursor = c
        .notas
        .recebidas()
        .unwrap()
        .cursor("homologacao", "11222333000181")
        .unwrap();
    assert_eq!(cursor.ult_nsu.valor(), 4);
    assert!(cursor.espera.is_some());
}

#[tokio::test]
async fn a_chamada_antes_da_hora_diz_quando_tentar_sem_ir_a_rede() {
    let c = base();
    let notas = notas_de_teste(&c);
    c.roteiro(vec![(DIST, lote(1, 1, &[doc_resumo(1, &notas[0].1)]))]);
    let r = sincronizar(&c.ctx(), None).await.unwrap();
    assert_eq!(r.para_json()["status"], "em_dia");
    // nada no roteiro: se a segunda chamada fosse a rede, o transporte falso derrubaria o teste
    let de_novo = sincronizar(&c.ctx(), None).await.unwrap();
    assert_eq!(de_novo.para_json()["status"], "aguarde");
    assert!(de_novo.erro().is_none(), "esperar nao e' falha");
    assert!(de_novo.frase.contains("10:00"), "{}", de_novo.frase);
    assert_eq!(c.falso.servicos().len(), 1);
}

#[tokio::test]
async fn o_656_grava_o_bloqueio_e_a_proxima_chamada_diz_a_hora_sem_ir_a_rede() {
    let c = base();
    c.roteiro(vec![(DIST, bloqueio(0))]);
    let r = sincronizar(&c.ctx(), None).await.unwrap();
    assert_eq!(r.para_json()["status"], "consumo_indevido");
    assert_eq!(r.para_json()["cstat"], "656");
    assert!(matches!(r.erro(), Some(Erro::Recusa { .. })));
    assert_eq!(r.erro().unwrap().codigo_saida(), 3);
    let texto =
        std::fs::read_to_string(pasta_recebidas(&c).join("cursor-homologacao-11222333000181.json"))
            .unwrap();
    assert!(texto.contains("consumo_indevido"), "{texto}");

    let de_novo = sincronizar(&c.ctx(), None).await.unwrap();
    assert_eq!(de_novo.para_json()["status"], "aguarde");
    assert!(de_novo.frase.contains("10:00"), "{}", de_novo.frase);
    assert_eq!(c.falso.servicos().len(), 1, "o bloqueio nao chama de novo");
}

#[tokio::test]
async fn queda_no_meio_de_um_lote_nunca_pula_nsu() {
    let c = base();
    let notas = notas_de_teste(&c);
    let lote_dois = || {
        lote(
            4,
            4,
            &[doc_resumo(3, &notas[2].1), doc_resumo(4, &notas[0].1)],
        )
    };
    c.roteiro(vec![
        (
            DIST,
            lote(
                2,
                4,
                &[doc_resumo(1, &notas[0].1), doc_completa(2, &notas[1].0)],
            ),
        ),
        (DIST, lote_dois()),
    ]);
    // o arquivo do primeiro documento do segundo lote nao pode ser gravado (ha uma pasta no lugar dele)
    let mes = pasta_recebidas(&c).join("2026").join("10");
    std::fs::create_dir_all(mes.join(format!("{}-resNFe.xml", notas[2].1))).unwrap();
    let e = sincronizar(&c.ctx(), None).await.unwrap_err();
    assert!(matches!(e, Erro::Entrada(_)), "{e:?}");

    // o cursor ficou no fim do primeiro lote, que estava gravado
    let pasta = c.notas.recebidas().unwrap();
    assert_eq!(
        pasta
            .cursor("homologacao", "11222333000181")
            .unwrap()
            .ult_nsu
            .valor(),
        2
    );
    assert_eq!(pasta.listar().unwrap().len(), 2);

    // a causa some: a proxima chamada pede a partir do NSU 2, grava o mesmo lote e completa, sem repetir nem pular
    std::fs::remove_dir_all(mes.join(format!("{}-resNFe.xml", notas[2].1))).unwrap();
    c.roteiro(vec![(DIST, lote_dois())]);
    let r = sincronizar(&c.ctx(), None).await.unwrap();
    assert!(r.erro().is_none());
    assert!(
        c.falso
            .corpos()
            .last()
            .unwrap()
            .contains("<ultNSU>000000000000002</ultNSU>"),
        "o pedido recomeca de onde o cursor parou"
    );
    assert_eq!(
        pasta
            .cursor("homologacao", "11222333000181")
            .unwrap()
            .ult_nsu
            .valor(),
        4
    );
    assert_eq!(
        pasta.listar().unwrap().len(),
        3,
        "uma linha por NF-e, sem duplicar"
    );
}

#[tokio::test]
async fn documento_que_nao_abre_e_guardado_e_relido_na_vez_seguinte() {
    let c = base();
    let notas = notas_de_teste(&c);
    c.roteiro(vec![(
        DIST,
        lote(
            2,
            2,
            &[
                doc_resumo(1, &notas[0].1),
                doc_zip_bruto(2, "procNFe_v4.00.xsd", "bm9wZQ=="),
            ],
        ),
    )]);
    let r = sincronizar(&c.ctx(), None).await.unwrap();
    assert_eq!(r.contagem.ilegiveis, 1);
    assert_eq!(r.para_json()["cursor"]["ilegiveis"][0], "000000000000002");
    assert!(pasta_recebidas(&c)
        .join("outros")
        .join("000000000000002-ilegivel.b64")
        .is_file());

    // a vez seguinte (ainda dentro da espera): o consNSU do 2 traz a nota completa
    c.roteiro(vec![(DIST, lote(2, 2, &[doc_completa(2, &notas[1].0)]))]);
    let r = sincronizar(&c.ctx(), None).await.unwrap();
    assert_eq!(r.para_json()["status"], "aguarde");
    assert_eq!(r.relidos, 1);
    assert!(r.para_json()["cursor"]["ilegiveis"]
        .as_array()
        .unwrap()
        .is_empty());
    assert!(c
        .falso
        .corpos()
        .last()
        .unwrap()
        .contains("<consNSU><NSU>000000000000002</NSU></consNSU>"));
    assert_eq!(
        c.notas
            .recebidas()
            .unwrap()
            .registro(&notas[1].1)
            .unwrap()
            .situacao,
        SituacaoRecebida::Completa
    );
}

#[tokio::test]
async fn manifesta_ciencia_e_confirmacao_pela_pasta() {
    let c = base();
    let notas = notas_de_teste(&c);
    c.roteiro(vec![(DIST, lote(1, 1, &[doc_resumo(1, &notas[0].1)]))]);
    sincronizar(&c.ctx(), None).await.unwrap();
    let chave = &notas[0].1;

    c.roteiro(vec![(
        EVT,
        evento(
            "135",
            "Evento registrado e vinculado a NF-e",
            "135260000000099",
        ),
    )]);
    let m = manifestar(&c.ctx(), chave, TipoManifestacao::Ciencia, None)
        .await
        .unwrap();
    assert!(m.erro().is_none(), "{:?}", m.erro());
    assert!(m.guardado);
    assert_eq!(m.situacao, Some(SituacaoRecebida::Ciencia));
    assert_eq!(m.para_json()["tipo"], "210210");
    let mes = pasta_recebidas(&c).join("2026").join("10");
    assert!(mes
        .join(format!("{chave}-210210-1-procEventoNFe.xml"))
        .is_file());
    assert!(c
        .falso
        .corpos()
        .last()
        .unwrap()
        .contains("<tpEvento>210210</tpEvento>"));

    c.roteiro(vec![(
        EVT,
        evento(
            "135",
            "Evento registrado e vinculado a NF-e",
            "135260000000100",
        ),
    )]);
    let m = manifestar(&c.ctx(), chave, TipoManifestacao::Confirmacao, None)
        .await
        .unwrap();
    assert_eq!(m.situacao, Some(SituacaoRecebida::Confirmada));

    // a pasta sabe da confirmacao: ciencia depois dela (655) e a mesma confirmacao (573) nem saem da maquina
    c.roteiro(vec![]);
    for tipo in [TipoManifestacao::Ciencia, TipoManifestacao::Confirmacao] {
        let e = manifestar(&c.ctx(), chave, tipo, None).await.unwrap_err();
        assert!(matches!(e, Erro::Recusa { .. }), "{e:?}");
    }
    // nao realizada pede justificativa
    let e = manifestar(&c.ctx(), chave, TipoManifestacao::NaoRealizada, None)
        .await
        .unwrap_err();
    assert!(e.mensagem().contains("xJust"), "{}", e.mensagem());
    assert_eq!(
        c.notas
            .recebidas()
            .unwrap()
            .registro(chave)
            .unwrap()
            .eventos
            .len(),
        2
    );
}

#[tokio::test]
async fn a_recusa_do_ambiente_nacional_nao_guarda_nada_e_vira_codigo_de_saida() {
    let c = base();
    let (_, chave) = proc_nfe(&c.config, 9);
    c.roteiro(vec![(
        EVT,
        evento("596", "Rejeicao: prazo de manifestacao vencido", ""),
    )]);
    let m = manifestar(&c.ctx(), &chave, TipoManifestacao::Ciencia, None)
        .await
        .unwrap();
    assert!(!m.guardado);
    assert_eq!(m.para_json()["status"], "rejeitado");
    assert_eq!(m.erro().unwrap().codigo_saida(), 3);
    assert!(c.notas.recebidas().unwrap().listar().unwrap().is_empty());
}

#[tokio::test]
async fn baixar_sem_ciencia_diz_que_veio_so_o_resumo_e_com_ciencia_traz_a_nota() {
    let c = base();
    let (proc, chave) = proc_nfe(&c.config, 1);
    c.roteiro(vec![(DIST, lote(1, 1, &[doc_resumo(1, &chave)]))]);
    let b = baixar(&c.ctx(), &chave, false).await.unwrap();
    assert!(!b.completa);
    assert!(b.frase.contains("--ciencia"), "{}", b.frase);
    assert_eq!(b.erro().unwrap().codigo_saida(), 3);
    assert!(c.falso.corpos()[0].contains(&format!("<consChNFe><chNFe>{chave}</chNFe></consChNFe>")));

    // com a ciencia: consulta, registra a ciencia (evento fiscal) e consulta de novo
    c.roteiro(vec![
        (DIST, lote(1, 1, &[doc_resumo(1, &chave)])),
        (
            EVT,
            evento(
                "135",
                "Evento registrado e vinculado a NF-e",
                "135260000000099",
            ),
        ),
        (DIST, lote(2, 2, &[doc_completa(2, &proc)])),
    ]);
    let b = baixar(&c.ctx(), &chave, true).await.unwrap();
    assert!(b.completa, "{}", b.frase);
    assert!(b.erro().is_none(), "{:?}", b.erro());
    assert_eq!(b.situacao, Some(SituacaoRecebida::Ciencia));
    assert_eq!(b.para_json()["ciencia"]["tipo"], "210210");
    assert_eq!(c.falso.servicos(), [DIST, DIST, EVT, DIST]);
    let xml = c.notas.recebidas().unwrap().ler_xml(&chave).unwrap();
    assert!(
        xml.contains("<nfeProc"),
        "a nota completa vale mais que o resumo"
    );
}

#[tokio::test]
async fn listar_filtra_por_situacao_e_busca_sem_usar_a_rede() {
    let c = base();
    let notas = notas_de_teste(&c);
    c.roteiro(vec![(
        DIST,
        lote(
            2,
            2,
            &[doc_resumo(1, &notas[0].1), doc_completa(2, &notas[1].0)],
        ),
    )]);
    sincronizar(&c.ctx(), None).await.unwrap();
    let so = |situacao, busca: &str| {
        listar(
            &c.notas,
            &FiltroRecebidas {
                situacao,
                busca: busca.to_string(),
            },
        )
        .unwrap()
    };
    assert_eq!(so(None, "").len(), 2);
    assert_eq!(so(Some(SituacaoRecebida::Completa), "").len(), 1);
    assert_eq!(
        so(None, "loja de teste").len(),
        2,
        "o nome casa sem acento nem caixa"
    );
    assert_eq!(so(None, &notas[0].1).len(), 1);
    assert_eq!(so(None, "nao existe").len(), 0);
    assert_eq!(c.falso.servicos().len(), 1);
}

#[tokio::test]
async fn sem_cnpj_ou_sem_pasta_a_mensagem_diz_o_que_configurar() {
    let c = base();
    let mut sem_cnpj = c.config.clone();
    sem_cnpj.emitente.cnpj.clear();
    let ctx = crate::nfe::Contexto::novo(&c.cliente, &c.certificado, &sem_cnpj, Some(&c.notas));
    let e = sincronizar(&ctx, None).await.unwrap_err();
    assert!(e.mensagem().contains("[emitente].cnpj"), "{}", e.mensagem());
    let sem_pasta = crate::nfe::Contexto::novo(&c.cliente, &c.certificado, &c.config, None);
    assert!(matches!(
        sincronizar(&sem_pasta, None).await,
        Err(Erro::Uso(_))
    ));
    assert!(matches!(
        sincronizar(&c.ctx(), Some(0)).await,
        Err(Erro::Uso(_))
    ));
}

#[tokio::test]
async fn evento_completo_de_terceiro_e_guardado_ao_lado_da_nota() {
    let c = base();
    let notas = notas_de_teste(&c);
    let chave = &notas[1].1;
    c.roteiro(vec![(
        DIST,
        lote(
            2,
            2,
            &[
                doc_completa(1, &notas[1].0),
                doc_zip(
                    2,
                    "procEventoNFe_v1.00.xsd",
                    &proc_evento(chave, "110110", 1),
                ),
            ],
        ),
    )]);
    let r = sincronizar(&c.ctx(), None).await.unwrap();
    assert_eq!((r.contagem.notas, r.contagem.eventos), (1, 1));
    let mes = pasta_recebidas(&c).join("2026").join("10");
    assert!(mes
        .join(format!("{chave}-110110-1-procEventoNFe.xml"))
        .is_file());
    let linha = c.notas.recebidas().unwrap().registro(chave).unwrap();
    assert_eq!(linha.eventos.len(), 1);
    assert_eq!(
        linha.situacao,
        SituacaoRecebida::Completa,
        "carta de correcao nao muda a situacao"
    );
}
