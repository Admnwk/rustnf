// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Lista, filtro, CSV, exportacao e o indice refeito dos arquivos, sobre uma pasta que as acoes da aplicacao encheram:
//! uma autorizada, uma pendente, uma cancelada e uma faixa inutilizada.

use super::*;
use crate::nfe::testes_comuns::*;
use crate::nfe::{cancelar, emitir, inutilizar};

async fn pasta_cheia(rotulo: &str) -> Cenario {
    let c = Cenario::novo(
        rotulo,
        vec![
            (AUT, envio("100", "Autorizado o uso da NF-e", NPROT)),
            (AUT, tempo()),
            (CON, tempo()),
            (
                AUT,
                envio("100", "Autorizado o uso da NF-e", "135260000000003"),
            ),
            (
                EVT,
                evento(
                    "135",
                    "Evento registrado e vinculado a NF-e",
                    "135260000000200",
                ),
            ),
            (
                INU,
                inutilizacao(
                    "102",
                    "Inutilizacao de numero homologado",
                    "135260000000300",
                ),
            ),
        ],
    );
    let ctx = c.ctx();
    emitir(&ctx, pedido(), true).await.unwrap();
    emitir(&ctx, pedido(), true).await.unwrap();
    let terceira = emitir(&ctx, pedido(), true).await.unwrap();
    cancelar(
        &ctx,
        &terceira.chave,
        "Cancelamento de teste da pasta",
        None,
    )
    .await
    .unwrap();
    inutilizar(&ctx, None, 4, 5, "Numeros pulados no teste da pasta")
        .await
        .unwrap();
    assert_eq!(c.falso.restantes(), 0);
    c
}

fn resumo(n: &NotasNfe) -> Vec<(String, Situacao, String, String, usize)> {
    n.listar()
        .unwrap()
        .into_iter()
        .map(|r| (r.chave, r.situacao, r.numero, r.protocolo, r.eventos.len()))
        .collect()
}

#[tokio::test]
async fn a_pasta_cheia_tem_cada_situacao_e_a_faixa() {
    let c = pasta_cheia("nfe-man-cheia").await;
    let por_numero = |n: &str| {
        c.notas
            .listar()
            .unwrap()
            .into_iter()
            .find(|r| r.numero == n)
            .unwrap()
    };
    assert_eq!(por_numero("1").situacao, Situacao::Autorizada);
    assert_eq!(por_numero("2").situacao, Situacao::Pendente);
    assert_eq!(por_numero("3").situacao, Situacao::Cancelada);
    assert_eq!(c.notas.inutilizacoes().unwrap().len(), 1);
    assert_eq!(c.notas.proximo_numero("1").unwrap(), 6);
}

#[tokio::test]
async fn reindexar_refaz_o_indice_dos_arquivos_e_guarda_o_que_so_o_indice_sabia() {
    let c = pasta_cheia("nfe-man-reindexa").await;
    let antes = resumo(&c.notas);
    let faixas = c.notas.inutilizacoes().unwrap();
    let indice = c.notas.pasta().join("indice.json");
    std::fs::remove_file(&indice).unwrap();
    assert!(c.notas.listar().unwrap().is_empty());

    let feito = c.notas.reindexar().unwrap();
    assert_eq!((feito.notas, feito.canceladas, feito.ignorados), (3, 1, 0));
    assert_eq!(resumo(&c.notas), antes);
    assert_eq!(c.notas.inutilizacoes().unwrap(), faixas);
    // a numeracao nao muda
    assert_eq!(c.notas.proximo_numero("1").unwrap(), 6);

    // a retomada anota quantas vezes consultou: isso so' o indice sabe, e a reindexacao o aproveita
    let pendente = c.notas.a_resolver().unwrap()[0].chave.clone();
    c.notas.marcar_tentativa(&pendente, 777).unwrap();
    c.notas.reindexar().unwrap();
    let r = c.notas.registro(&pendente).unwrap();
    assert_eq!((r.tentativas, r.ultima), (1, Some(777)));
    // o indice anterior ficou ao lado
    assert!(c.notas.pasta().join(INDICE_ANTERIOR).is_file());

    // indice ilegivel: refeito mesmo assim, e o aviso diz que a marca de antes se perdeu
    std::fs::write(&indice, "{ isto nao e' json").unwrap();
    let feito = c.notas.reindexar().unwrap();
    assert!(feito.indice_ilegivel);
    assert_eq!(resumo(&c.notas), antes);
}

#[tokio::test]
async fn arquivo_estranho_na_pasta_do_mes_e_contado_e_nao_entra_no_indice() {
    let c = pasta_cheia("nfe-man-estranho").await;
    let dir = c.notas.pasta().join("2026/10");
    std::fs::write(dir.join("qualquer-coisa.xml"), "<x/>").unwrap();
    std::fs::write(dir.join(format!("{}-procNFe.xml", "1".repeat(44))), "<x/>").unwrap();
    let feito = c.notas.reindexar().unwrap();
    assert_eq!((feito.notas, feito.ignorados), (3, 2));
}

#[tokio::test]
async fn filtro_por_periodo_situacao_e_busca() {
    let c = pasta_cheia("nfe-man-filtro").await;
    let todas = c.notas.listar().unwrap();
    let conta = |f: Filtro| f.aplicar(todas.clone()).len();
    assert_eq!(conta(Filtro::default()), 3);
    assert_eq!(conta(Filtro::do_mes("2026-10").unwrap()), 3);
    assert_eq!(conta(Filtro::do_mes("2026-11").unwrap()), 0);
    assert_eq!(
        conta(Filtro::novo(Some("2026-10-04"), None, None, "").unwrap()),
        0
    );
    assert_eq!(
        conta(Filtro::novo(None, Some("2026-10-03"), None, "").unwrap()),
        3
    );
    assert_eq!(
        conta(Filtro::novo(None, None, Some(Situacao::Cancelada), "").unwrap()),
        1
    );
    assert_eq!(
        conta(Filtro::novo(None, None, None, "homologacao sem valor").unwrap()),
        3
    );
    assert_eq!(
        conta(Filtro::novo(None, None, None, "52998224725").unwrap()),
        3
    );
    assert_eq!(conta(Filtro::novo(None, None, None, "joao").unwrap()), 0);
    assert_eq!(
        Filtro::novo(Some("03/10/2026"), None, None, "").unwrap_err(),
        ErroNotas::DataInvalida
    );
    assert_eq!(
        Filtro::do_mes("2026-13").unwrap_err(),
        ErroNotas::DataInvalida
    );
}

#[tokio::test]
async fn csv_e_exportacao_levam_o_xml_o_evento_e_a_lista() {
    let c = pasta_cheia("nfe-man-exporta").await;
    let notas = c.notas.listar().unwrap();
    let texto = csv(&notas);
    let linhas: Vec<&str> = texto.split("\r\n").collect();
    assert_eq!(
        linhas[0],
        "emissao;serie;numero;valor;situacao;ambiente;protocolo;destinatario_documento;destinatario_nome;chave"
    );
    assert_eq!(
        linhas.len(),
        5,
        "titulo, tres notas e o fim da ultima linha"
    );
    assert!(linhas[1].starts_with("2026-10-03;1;"), "{}", linhas[1]);
    assert!(
        linhas[1].contains(";31,80;"),
        "valor com virgula: {}",
        linhas[1]
    );

    let destino = c.pasta.caminho().join("contabilidade");
    let feito = c.notas.exportar(&notas, &destino).unwrap();
    // o nfeProc de duas notas, o evento do cancelamento e a NF-e assinada da pendente
    assert_eq!((feito.copiadas, feito.sem_xml.len()), (4, 0));
    assert!(destino.join(LISTA_CSV).is_file());
    let nomes: Vec<String> = std::fs::read_dir(&destino)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    let conta = |sufixo: &str| nomes.iter().filter(|n| n.ends_with(sufixo)).count();
    assert_eq!(conta("-procNFe.xml"), 2);
    assert_eq!(conta("-110111-1-procEventoNFe.xml"), 1);
    assert_eq!(conta("-NFe.xml"), 1);
}
