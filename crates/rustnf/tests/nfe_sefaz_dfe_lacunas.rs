// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A sequencia de NSU: a lacuna que se fecha por `consNSU`, o NSU repetido que nao se entrega de novo, o `ultNSU` que
//! recua ou nao avanca, o salto grande que nao se persegue e o laco `sincronizar` (lote a lote, `gravar` com os
//! documentos e o cursor juntos, parada quando a aplicacao nao consegue gravar).

mod nfe_sefaz_comum;

use nfe_sefaz_comum::dfe::*;
use nfe_sefaz_comum::*;
use rustnf::nfe::sefaz::dfe::{
    Cursor, DesfechoDfe, Doc, ErroSincronizacao, FaixaNsu, MotivoEspera, Nsu,
};

fn nsu(v: u64) -> Nsu {
    Nsu::ler(&v.to_string()).unwrap()
}

fn cursor_em(ult: u64) -> Cursor {
    Cursor {
        ult_nsu: nsu(ult),
        ..Cursor::novo()
    }
}

fn faixa(a: u64, b: u64) -> FaixaNsu {
    FaixaNsu {
        inicio: nsu(a),
        fim: nsu(b),
    }
}

fn resumo_no(n: u64, chave: &str) -> String {
    doc_zip(n, "resNFe_v1.01.xsd", &res_nfe(chave, 1))
}

// ------------------------------------------------------------------------------------------------ lacunas

#[tokio::test]
async fn nsu_que_falta_no_meio_do_lote_vira_lacuna_no_cursor_e_nada_e_pulado() {
    let (_, chave) = nfe(1, 2);
    // o cursor estava em 10; o lote traz 11, 13 e 16: faltam 12, 14 e 15
    let docs = [
        resumo_no(11, &chave),
        resumo_no(13, &chave),
        resumo_no(16, &chave),
    ];
    let (c, _) = cliente_dist(Falso::novo(vec![(DIST, lote(16, 40, &docs))]));
    let r = c.distribuir(CNPJ, &cursor_em(10)).await.unwrap();
    assert_eq!(r.desfecho, DesfechoDfe::Parcial);
    assert_eq!(r.cursor.ult_nsu, nsu(16));
    assert_eq!(r.cursor.lacunas, vec![faixa(12, 12), faixa(14, 15)]);
    assert!(r.frase.contains("consNSU"), "{}", r.frase);
    // o cursor com lacuna vai e volta pelo texto que a aplicacao guarda
    assert_eq!(Cursor::de_json(&r.cursor.para_json()).unwrap(), r.cursor);
}

#[tokio::test]
async fn lacuna_antes_do_primeiro_documento_conta_quando_o_cursor_ja_tinha_andado() {
    let (_, chave) = nfe(2, 2);
    let (c, _) = cliente_dist(Falso::novo(vec![(
        DIST,
        lote(13, 40, &[resumo_no(13, &chave)]),
    )]));
    let r = c.distribuir(CNPJ, &cursor_em(10)).await.unwrap();
    assert_eq!(r.cursor.lacunas, vec![faixa(11, 12)]);
    // cursor novo (zero) nao tem o que cobrar antes do primeiro documento
    let (_, chave) = nfe(3, 2);
    let (c, _) = cliente_dist(Falso::novo(vec![(
        DIST,
        lote(500, 600, &[resumo_no(500, &chave)]),
    )]));
    let r = c.distribuir(CNPJ, &Cursor::novo()).await.unwrap();
    assert!(r.cursor.lacunas.is_empty());
    assert_eq!(r.cursor.ult_nsu, nsu(500));
}

#[tokio::test]
async fn salto_maior_que_o_limite_nao_e_perseguido_e_vai_no_resultado() {
    let (_, chave) = nfe(4, 2);
    let (c, _) = cliente_dist(Falso::novo(vec![(
        DIST,
        lote(9000, 9000, &[resumo_no(9000, &chave)]),
    )]));
    let r = c.distribuir(CNPJ, &cursor_em(10)).await.unwrap();
    assert!(r.cursor.lacunas.is_empty());
    assert_eq!(r.irrecuperaveis, vec![faixa(11, 8999)]);
    assert_eq!(r.cursor.ult_nsu, nsu(9000));
}

#[tokio::test]
async fn sincronizar_fecha_a_lacuna_por_consnsu_e_grava_cada_passo() {
    let (_, chave) = nfe(5, 2);
    let f = Falso::novo(vec![
        (
            DIST,
            lote(14, 14, &[resumo_no(11, &chave), resumo_no(14, &chave)]),
        ),
        (DIST, lote(12, 12, &[resumo_no(12, &chave)])),
        // o 13 o Ambiente Nacional diz que nao existe
        (DIST, nada(0)),
    ]);
    let (c, _) = cliente_dist(f.clone());
    let mut gravados: Vec<(Vec<u64>, String)> = Vec::new();
    let r = c
        .sincronizar(
            CNPJ,
            &cursor_em(10),
            |docs: &[Doc], cur: &Cursor| -> Result<(), String> {
                gravados.push((
                    docs.iter().map(|d| d.nsu.valor()).collect(),
                    cur.para_json(),
                ));
                Ok(())
            },
            20,
        )
        .await
        .unwrap();

    // 11 e 14 do lote, 12 pela lacuna; o 13 nao existe
    let nsus: Vec<u64> = r.documentos.iter().map(|d| d.nsu.valor()).collect();
    assert_eq!(nsus, vec![11, 14, 12]);
    assert_eq!(r.desfecho, DesfechoDfe::EmDia);
    assert!(r.cursor.lacunas.is_empty());
    assert_eq!(r.cursor.ult_nsu, nsu(14));
    assert_eq!(r.lotes, 1);
    // tres gravacoes: o lote (com a lacuna 12-13 no cursor), o consNSU 12, o consNSU 13 (vazio)
    assert_eq!(gravados.len(), 3);
    assert_eq!(gravados[0].0, vec![11, 14]);
    assert_eq!(gravados[1].0, vec![12]);
    assert!(gravados[2].0.is_empty());
    assert_eq!(
        Cursor::de_json(&gravados[0].1).unwrap().lacunas,
        vec![faixa(12, 13)]
    );
    assert_eq!(
        Cursor::de_json(&gravados[1].1).unwrap().lacunas,
        vec![faixa(13, 13)]
    );
    assert!(Cursor::de_json(&gravados[2].1).unwrap().lacunas.is_empty());
    // as consultas: a distribuicao e depois dois consNSU
    let ch = f.chamadas();
    assert!(ch[1]
        .corpo
        .contains("<consNSU><NSU>000000000000012</NSU></consNSU>"));
    assert!(ch[2]
        .corpo
        .contains("<consNSU><NSU>000000000000013</NSU></consNSU>"));
    assert_eq!(f.restantes(), 0);
}

#[tokio::test]
async fn a_lacuna_se_fecha_mesmo_durante_a_espera_da_fila_mas_nao_durante_o_656() {
    let (_, chave) = nfe(6, 2);
    let mut cur = cursor_em(14);
    cur.lacunas = vec![faixa(12, 12)];
    cur.espera = Some(rustnf::nfe::sefaz::dfe::Espera {
        ate: meio_dia() + chrono::Duration::minutes(30),
        motivo: MotivoEspera::FilaEmDia,
    });
    let f = Falso::novo(vec![(DIST, lote(12, 12, &[resumo_no(12, &chave)]))]);
    let (c, _) = cliente_dist(f.clone());
    let r = c
        .sincronizar(
            CNPJ,
            &cur,
            |_: &[Doc], _: &Cursor| -> Result<(), String> { Ok(()) },
            5,
        )
        .await
        .unwrap();
    // a distribuicao esperou (nada saiu), o consNSU foi
    assert!(matches!(r.desfecho, DesfechoDfe::Aguarde { .. }));
    assert_eq!(r.documentos.len(), 1);
    assert!(r.cursor.lacunas.is_empty());
    assert_eq!(f.chamadas().len(), 1);

    // no 656 nada sai
    let mut cur = cursor_em(14);
    cur.lacunas = vec![faixa(12, 12)];
    cur.espera = Some(rustnf::nfe::sefaz::dfe::Espera {
        ate: meio_dia() + chrono::Duration::minutes(30),
        motivo: MotivoEspera::ConsumoIndevido,
    });
    let f = Falso::novo(vec![]);
    let (c, _) = cliente_dist(f.clone());
    let r = c
        .sincronizar(
            CNPJ,
            &cur,
            |_: &[Doc], _: &Cursor| -> Result<(), String> { Ok(()) },
            5,
        )
        .await
        .unwrap();
    assert!(matches!(r.desfecho, DesfechoDfe::Aguarde { .. }));
    assert_eq!(r.cursor.lacunas, vec![faixa(12, 12)]);
    assert!(f.chamadas().is_empty());
}

// ------------------------------------------------------------------------------------------------ nunca repete, nunca volta

#[tokio::test]
async fn nsu_que_o_cursor_ja_cobria_nao_e_entregue_de_novo() {
    let (_, chave) = nfe(7, 2);
    let docs = [
        resumo_no(9, &chave),
        resumo_no(10, &chave),
        resumo_no(11, &chave),
        resumo_no(11, &chave),
    ];
    let (c, _) = cliente_dist(Falso::novo(vec![(DIST, lote(11, 30, &docs))]));
    let r = c.distribuir(CNPJ, &cursor_em(10)).await.unwrap();
    assert_eq!(r.documentos.len(), 1);
    assert_eq!(r.documentos[0].nsu, nsu(11));
    assert_eq!(r.repetidos, vec![nsu(9), nsu(10), nsu(11)]);
    assert_eq!(r.cursor.ult_nsu, nsu(11));
}

#[tokio::test]
async fn ultnsu_que_recua_ou_nao_avanca_ou_passa_do_ultimo_documento_nao_move_o_cursor() {
    let (_, chave) = nfe(8, 2);
    let casos = vec![
        // recuou: pediu 20, veio 5
        lote(5, 50, &[]),
        // nao avancou com mais na fila
        lote(20, 50, &[]),
        // um documento com NSU acima do ultNSU
        lote(21, 50, &[resumo_no(25, &chave)]),
        // NSU lixo
        ok_dist(
            &ret_dist("138", "ok", 0, 0, &[])
                .replace("<ultNSU>000000000000000</ultNSU>", "<ultNSU>abc</ultNSU>"),
        ),
    ];
    let n = casos.len();
    let f = Falso::novo(casos.into_iter().map(|c| (DIST, c)).collect());
    let (c, _) = cliente_dist(f);
    let antes = cursor_em(20);
    for k in 0..n {
        let r = c.distribuir(CNPJ, &antes).await.unwrap();
        assert_eq!(r.desfecho, DesfechoDfe::Indisponivel, "caso {k}");
        assert_eq!(r.cursor, antes, "caso {k}");
        assert!(r.documentos.is_empty(), "caso {k}");
    }
}

#[tokio::test]
async fn docszip_sem_nsu_valido_na_distribuicao_nao_avanca_nada() {
    let (_, chave) = nfe(9, 2);
    let sem_nsu = format!(
        "<docZip schema=\"resNFe_v1.01.xsd\">{}</docZip>",
        gz_b64(&res_nfe(&chave, 1))
    );
    let (c, _) = cliente_dist(Falso::novo(vec![(DIST, lote(11, 11, &[sem_nsu]))]));
    let antes = cursor_em(10);
    let r = c.distribuir(CNPJ, &antes).await.unwrap();
    assert_eq!(r.desfecho, DesfechoDfe::Indisponivel);
    assert_eq!(r.cursor, antes);
}

// ------------------------------------------------------------------------------------------------ o laco

#[tokio::test]
async fn sincronizar_le_lote_a_lote_ate_a_fila_acabar_e_grava_cada_lote() {
    let (_, chave) = nfe(10, 2);
    let f = Falso::novo(vec![
        (
            DIST,
            lote(2, 4, &[resumo_no(1, &chave), resumo_no(2, &chave)]),
        ),
        (
            DIST,
            lote(4, 4, &[resumo_no(3, &chave), resumo_no(4, &chave)]),
        ),
    ]);
    let (c, _) = cliente_dist(f.clone());
    let mut cursores: Vec<Nsu> = Vec::new();
    let r = c
        .sincronizar(
            CNPJ,
            &Cursor::novo(),
            |docs: &[Doc], cur: &Cursor| -> Result<(), String> {
                assert!(!docs.is_empty());
                cursores.push(cur.ult_nsu);
                Ok(())
            },
            20,
        )
        .await
        .unwrap();
    assert_eq!(r.desfecho, DesfechoDfe::EmDia);
    assert_eq!(r.lotes, 2);
    assert_eq!(
        r.documentos
            .iter()
            .map(|d| d.nsu.valor())
            .collect::<Vec<_>>(),
        vec![1, 2, 3, 4]
    );
    assert_eq!(cursores, vec![nsu(2), nsu(4)]);
    assert_eq!(r.cursor.ult_nsu, nsu(4));
    assert_eq!(r.cursor.espera.unwrap().motivo, MotivoEspera::FilaEmDia);
    // o segundo lote partiu do ultNSU que o primeiro devolveu
    assert!(f.chamadas()[1]
        .corpo
        .contains("<ultNSU>000000000000002</ultNSU>"));
}

#[tokio::test]
async fn sincronizar_para_no_maximo_de_lotes_e_diz_que_ha_mais() {
    let (_, chave) = nfe(11, 2);
    let f = Falso::novo(vec![
        (DIST, lote(1, 9, &[resumo_no(1, &chave)])),
        (DIST, lote(2, 9, &[resumo_no(2, &chave)])),
    ]);
    let (c, _) = cliente_dist(f.clone());
    let r = c
        .sincronizar(
            CNPJ,
            &Cursor::novo(),
            |_: &[Doc], _: &Cursor| -> Result<(), String> { Ok(()) },
            2,
        )
        .await
        .unwrap();
    assert_eq!(r.desfecho, DesfechoDfe::Parcial);
    assert!(r.ha_mais());
    assert_eq!((r.lotes, r.documentos.len()), (2, 2));
    assert_eq!(r.cursor.ult_nsu, nsu(2));
    assert!(r.frase.contains("2 lote(s)"), "{}", r.frase);
}

#[tokio::test]
async fn erro_ao_gravar_para_o_laco_sem_avancar_o_cursor_daquele_lote() {
    let (_, chave) = nfe(12, 2);
    let f = Falso::novo(vec![
        (DIST, lote(1, 9, &[resumo_no(1, &chave)])),
        (DIST, lote(2, 9, &[resumo_no(2, &chave)])),
    ]);
    let (c, _) = cliente_dist(f.clone());
    let mut chamadas = 0;
    let e = c
        .sincronizar(
            CNPJ,
            &Cursor::novo(),
            |_: &[Doc], _: &Cursor| -> Result<(), String> {
                chamadas += 1;
                if chamadas == 2 {
                    Err("disco cheio".to_string())
                } else {
                    Ok(())
                }
            },
            20,
        )
        .await
        .unwrap_err();
    assert!(
        matches!(e, ErroSincronizacao::Gravar(ref m) if m == "disco cheio"),
        "{e:?}"
    );
    assert_eq!(f.chamadas().len(), 2);
}

#[tokio::test]
async fn o_656_no_meio_do_laco_grava_a_espera_sem_documentos_e_para() {
    let (_, chave) = nfe(13, 2);
    let f = Falso::novo(vec![
        (DIST, lote(1, 9, &[resumo_no(1, &chave)])),
        (
            DIST,
            ok_dist(&ret_dist("656", "Rejeicao: Consumo Indevido", 0, 0, &[])),
        ),
    ]);
    let (c, _) = cliente_dist(f);
    let mut gravacoes: Vec<(usize, bool)> = Vec::new();
    let r = c
        .sincronizar(
            CNPJ,
            &Cursor::novo(),
            |docs: &[Doc], cur: &Cursor| -> Result<(), String> {
                gravacoes.push((docs.len(), cur.espera.is_some()));
                Ok(())
            },
            20,
        )
        .await
        .unwrap();
    assert!(matches!(r.desfecho, DesfechoDfe::ConsumoIndevido { .. }));
    // o lote, e o 656 SEM documentos mas com a espera no cursor
    assert_eq!(gravacoes, vec![(1, false), (0, true)]);
    assert_eq!(r.documentos.len(), 1);
    assert_eq!(r.cursor.ult_nsu, nsu(1));
    assert_eq!(
        r.cursor.espera.unwrap().motivo,
        MotivoEspera::ConsumoIndevido
    );
}
