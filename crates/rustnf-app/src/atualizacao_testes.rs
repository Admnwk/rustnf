// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A atualizacao em lote com o cliente real da biblioteca sobre um transporte falso: nada sai para a rede.

use std::sync::atomic::Ordering;
use std::time::Duration;

use super::*;
use crate::testes_sefin::{chave, emissor, pasta_com, registro, situacao_de, Falso};

async fn rodar(falso: &Falso, notas: &Notas, filtro: &Filtro) -> Relatorio {
    atualizar_situacoes(&emissor(falso), notas, filtro, Duration::ZERO)
        .await
        .unwrap()
}

#[tokio::test]
async fn cada_linha_da_regra_da_situacao() {
    let (_p, notas) = pasta_com(
        "atualizacao-regra",
        &[
            registro(1, Situacao::Autorizada, "Cancelada"),
            registro(2, Situacao::Autorizada, "Deferida"),
            registro(3, Situacao::Autorizada, "Substituida"),
            registro(4, Situacao::Autorizada, "Em analise"),
            registro(5, Situacao::Autorizada, "Indeferida"),
            registro(6, Situacao::Autorizada, "Sem evento"),
        ],
    );
    let falso = Falso::novo()
        .com_evento(1, "101101", 1)
        .com_evento(2, "101103", 1)
        .com_evento(2, "105104", 1)
        .com_evento(3, "105102", 1)
        .com_evento(4, "101103", 1)
        .com_evento(5, "101103", 1)
        .com_evento(5, "105105", 1)
        .com_nota(5)
        .com_nota(6);
    let r = rodar(&falso, &notas, &Filtro::default()).await;
    assert_eq!(r.conferidas, 6);
    assert!(r.falhas.is_empty(), "{:?}", r.falhas);
    let mudancas: Vec<(u64, Situacao)> = [1, 2, 3, 4]
        .iter()
        .map(|n| (*n, situacao_de(&notas, *n)))
        .collect();
    assert_eq!(
        mudancas,
        vec![
            (1, Situacao::Cancelada),
            (2, Situacao::Cancelada),
            (3, Situacao::Substituida),
            (4, Situacao::AnaliseFiscal),
        ]
    );
    assert_eq!(situacao_de(&notas, 5), Situacao::Autorizada);
    assert_eq!(situacao_de(&notas, 6), Situacao::Autorizada);
    assert_eq!(r.mudaram.len(), 4);
    assert_eq!(r.iguais, 2);
    let uma = r.mudaram.iter().find(|m| m.chave == chave(4)).unwrap();
    assert_eq!(
        (uma.de, uma.para),
        (Situacao::Autorizada, Situacao::AnaliseFiscal)
    );
}

#[tokio::test]
async fn para_de_consultar_no_primeiro_evento_que_decide() {
    let (_p, notas) = pasta_com(
        "atualizacao-para",
        &[registro(1, Situacao::Autorizada, "Cancelada")],
    );
    let falso = Falso::novo()
        .com_evento(1, "101101", 1)
        .com_evento(1, "105102", 1);
    let r = rodar(&falso, &notas, &Filtro::default()).await;
    assert_eq!(r.mudaram.len(), 1);
    assert_eq!(falso.urls().len(), 1, "{:?}", falso.urls());
    assert!(falso.urls()[0].ends_with("/eventos/101101/1"));
}

#[tokio::test]
async fn novo_pedido_de_analise_depois_de_um_indeferido_volta_a_analise() {
    let (_p, notas) = pasta_com(
        "atualizacao-pedidos",
        &[registro(1, Situacao::Autorizada, "Dois pedidos")],
    );
    let falso = Falso::novo()
        .com_evento(1, "101103", 1)
        .com_evento(1, "105105", 1)
        .com_evento(1, "101103", 2);
    let r = rodar(&falso, &notas, &Filtro::default()).await;
    assert_eq!(r.mudaram.len(), 1, "{r:?}");
    assert_eq!(situacao_de(&notas, 1), Situacao::AnaliseFiscal);
}

#[tokio::test]
async fn divergencia_nunca_devolve_a_autorizada() {
    let (_p, notas) = pasta_com(
        "atualizacao-divergencia",
        &[
            registro(1, Situacao::Cancelada, "A"),
            registro(2, Situacao::Substituida, "B"),
            registro(3, Situacao::AnaliseFiscal, "C"),
            registro(4, Situacao::Cancelada, "D"),
        ],
    );
    // a Sefin tem as notas e nenhum evento; da 4 so' um pedido de analise
    let falso = Falso::novo()
        .com_nota(1)
        .com_nota(2)
        .com_nota(3)
        .com_nota(4)
        .com_evento(4, "101103", 1);
    let r = rodar(&falso, &notas, &Filtro::default()).await;
    assert!(r.mudaram.is_empty(), "{r:?}");
    assert_eq!(r.falhas.len(), 4, "{r:?}");
    assert!(r.falhas.iter().all(|f| f.motivo.contains("divergencia")));
    assert_eq!(situacao_de(&notas, 1), Situacao::Cancelada);
    assert_eq!(situacao_de(&notas, 2), Situacao::Substituida);
    assert_eq!(situacao_de(&notas, 3), Situacao::AnaliseFiscal);
    assert_eq!(situacao_de(&notas, 4), Situacao::Cancelada);
}

#[tokio::test]
async fn cancelada_na_sefin_troca_substituida_da_pasta_e_analise_indeferida_volta_a_autorizada() {
    let (_p, notas) = pasta_com(
        "atualizacao-trocas",
        &[
            registro(1, Situacao::Substituida, "A"),
            registro(2, Situacao::AnaliseFiscal, "B"),
        ],
    );
    let falso = Falso::novo()
        .com_evento(1, "101101", 1)
        .com_evento(2, "101103", 1)
        .com_evento(2, "105105", 1);
    let r = rodar(&falso, &notas, &Filtro::default()).await;
    assert!(r.falhas.is_empty(), "{r:?}");
    assert_eq!(situacao_de(&notas, 1), Situacao::Cancelada);
    assert_eq!(situacao_de(&notas, 2), Situacao::Autorizada);
}

#[tokio::test]
async fn nota_nao_encontrada_vai_para_as_falhas_sem_mudar_o_indice() {
    let (_p, notas) = pasta_com(
        "atualizacao-sem-nota",
        &[registro(1, Situacao::Autorizada, "Fantasma")],
    );
    // a Sefin nao tem a nota nem evento algum
    let r = rodar(&Falso::novo(), &notas, &Filtro::default()).await;
    assert_eq!(r.conferidas, 1);
    assert_eq!(r.falhas.len(), 1);
    assert_eq!(r.falhas[0].chave, chave(1));
    assert!(r.falhas[0].motivo.contains("nao encontrou"));
    assert_eq!(r.iguais, 0);
    assert_eq!(situacao_de(&notas, 1), Situacao::Autorizada);
}

#[tokio::test]
async fn a_falha_de_uma_nota_nao_para_as_outras() {
    let (_p, notas) = pasta_com(
        "atualizacao-falha",
        &[
            registro(1, Situacao::Autorizada, "Muda"),
            registro(2, Situacao::Autorizada, "Cancelada"),
            registro(3, Situacao::Autorizada, "Evento de outra nota"),
            registro(4, Situacao::Autorizada, "Normal"),
        ],
    );
    let falso = Falso::novo()
        .muda(1)
        .com_evento(2, "101101", 1)
        .com_evento_trocado(3, "101101", 1)
        .com_nota(4);
    let r = rodar(&falso, &notas, &Filtro::default()).await;
    assert_eq!(r.conferidas, 4);
    assert_eq!(r.falhas.len(), 2, "{r:?}");
    let chaves: Vec<&str> = r.falhas.iter().map(|f| f.chave.as_str()).collect();
    assert!(chaves.contains(&chave(1).as_str()) && chaves.contains(&chave(3).as_str()));
    assert_eq!(r.mudaram.len(), 1);
    assert_eq!(r.iguais, 1);
    assert_eq!(situacao_de(&notas, 1), Situacao::Autorizada);
    assert_eq!(situacao_de(&notas, 2), Situacao::Cancelada);
    // o evento que veio de outra nota nao cancela a nota pedida
    assert_eq!(situacao_de(&notas, 3), Situacao::Autorizada);
}

#[tokio::test]
async fn respeita_o_intervalo_e_nunca_consulta_em_paralelo() {
    let (_p, notas) = pasta_com(
        "atualizacao-ritmo",
        &[
            registro(1, Situacao::Autorizada, "A"),
            registro(2, Situacao::Autorizada, "B"),
        ],
    );
    let falso = Falso::novo()
        .com_nota(1)
        .com_nota(2)
        .com_atraso(Duration::from_millis(15));
    let intervalo = Duration::from_millis(60);
    let r = atualizar_situacoes(&emissor(&falso), &notas, &Filtro::default(), intervalo)
        .await
        .unwrap();
    assert_eq!(r.iguais, 2);
    // 4 consultas de evento + 1 da nota, por nota
    let chamadas = falso.chamadas.lock().unwrap().clone();
    assert_eq!(chamadas.len(), 10);
    assert_eq!(
        falso.simultaneas.load(Ordering::SeqCst),
        1,
        "nenhuma consulta em paralelo"
    );
    for par in chamadas.windows(2) {
        let folga = par[1].1.duration_since(par[0].2);
        assert!(
            folga >= intervalo.mul_f32(0.9),
            "so' {folga:?} entre duas consultas"
        );
    }
}

#[tokio::test]
async fn o_filtro_escolhe_as_notas_conferidas() {
    let (_p, notas) = pasta_com(
        "atualizacao-filtro",
        &[
            registro(1, Situacao::Autorizada, "Maria Souza"),
            registro(2, Situacao::Autorizada, "Joao Lima"),
            registro(3, Situacao::Cancelada, "Joao Cancelado"),
        ],
    );
    let falso = Falso::novo().com_nota(1).com_nota(2).com_nota(3);
    let filtro = Filtro::novo(None, None, Some(Situacao::Autorizada), "joao").unwrap();
    let r = rodar(&falso, &notas, &filtro).await;
    assert_eq!(r.conferidas, 1);
    assert!(
        falso.urls().iter().all(|u| u.contains(&chave(2))),
        "{:?}",
        falso.urls()
    );
    // a nota fora do filtro nem foi consultada, mesmo divergente
    assert_eq!(situacao_de(&notas, 3), Situacao::Cancelada);
}

#[tokio::test]
async fn pasta_vazia_nao_consulta_nada() {
    let (_p, notas) = pasta_com("atualizacao-vazia", &[]);
    let falso = Falso::novo();
    let r = rodar(&falso, &notas, &Filtro::default()).await;
    assert_eq!(r, Relatorio::default());
    assert!(falso.urls().is_empty());
}
