// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A pasta dos documentos recebidos: cursor por ambiente e interessado, cursor ilegivel, convivencia com a pasta das notas
//! emitidas (a da 0.4.0) e o que a reindexacao enxerga.

use rustnf::nfe::sefaz::dfe::Cursor;

use crate::nfe::testes_comuns::*;
use crate::nfe::testes_dfe::*;
use crate::notas::nfe::{NotasNfe, SituacaoRecebida};
use crate::notas::nfe::{Registro, Situacao};
use crate::notas::ErroNotas;
use crate::testes_comuns::PastaTemporaria;

#[test]
fn o_cursor_e_de_cada_ambiente_e_de_cada_interessado() {
    let p = PastaTemporaria::nova("recebidas-cursor");
    let notas = NotasNfe::abrir(p.caminho()).unwrap();
    let r = notas.recebidas().unwrap();
    assert_eq!(
        r.cursor("homologacao", "11222333000181")
            .unwrap()
            .ult_nsu
            .valor(),
        0
    );
    let mut c = Cursor::novo();
    c.ult_nsu = rustnf::nfe::sefaz::dfe::Nsu::ler("77").unwrap();
    r.gravar_lote("homologacao", "11222333000181", &[], &c)
        .unwrap();
    assert_eq!(
        r.cursor("homologacao", "11222333000181")
            .unwrap()
            .ult_nsu
            .valor(),
        77
    );
    // outro ambiente ou outro interessado comecam do zero: os NSU de um nao valem para o outro
    assert_eq!(
        r.cursor("producao", "11222333000181")
            .unwrap()
            .ult_nsu
            .valor(),
        0
    );
    assert_eq!(
        r.cursor("homologacao", "52998224725")
            .unwrap()
            .ult_nsu
            .valor(),
        0
    );
    // e nada que nao seja digito vira pedaco de nome de arquivo
    assert!(r.cursor("homologacao", "../x").is_err());
    assert!(r.cursor("outro", "11222333000181").is_err());
}

#[test]
fn cursor_ilegivel_nao_vira_recomece_do_zero() {
    let p = PastaTemporaria::nova("recebidas-ilegivel");
    let notas = NotasNfe::abrir(p.caminho()).unwrap();
    let r = notas.recebidas().unwrap();
    std::fs::write(
        r.pasta().join("cursor-homologacao-11222333000181.json"),
        "{\"ult_nsu\": \"abc\"}",
    )
    .unwrap();
    let e = r.cursor("homologacao", "11222333000181").unwrap_err();
    assert_eq!(e, ErroNotas::CursorIlegivel);
    assert!(e.to_string().contains("656"), "{e}");
}

#[tokio::test]
async fn a_pasta_das_emitidas_da_0_4_0_segue_lida_e_a_reindexacao_ignora_os_recebidos() {
    let c = Cenario::novo("recebidas-convivencia", vec![]);
    // uma nota emitida como a 0.4.0 a guardava: so' o indice e o estado, sem a pasta `recebidas`
    let mut p5 = pedido();
    p5["numero"] = 5.into();
    let m5 = crate::nfe::montar_pedido(&c.config, None, p5, true).unwrap();
    let registro = Registro {
        chave: m5.chave.clone(),
        serie: "1".into(),
        numero: "5".into(),
        emissao: "2026-10-03T09:00:00-03:00".into(),
        valor: "31.80".into(),
        ambiente: "homologacao".into(),
        desde: 100,
        ..Registro::default()
    };
    c.notas.guardar_pendente(&registro, &m5.xml).unwrap();
    std::fs::remove_dir_all(c.notas.pasta().join("recebidas")).ok();

    let (proc, chave) = proc_nfe(&c.config, 6);
    c.roteiro(vec![(DIST, lote(1, 1, &[doc_completa(1, &proc)]))]);
    crate::nfe::sincronizar(&c.ctx(), None).await.unwrap();

    // as duas pastas lado a lado, cada uma com o seu indice
    assert_eq!(c.notas.listar().unwrap().len(), 1);
    assert_eq!(c.notas.listar().unwrap()[0].situacao, Situacao::Pendente);
    assert_eq!(c.notas.recebidas().unwrap().listar().unwrap().len(), 1);
    assert_eq!(
        c.notas.proximo_numero("1").unwrap(),
        6,
        "a numeracao das emitidas nao muda"
    );
    // a reindexacao das emitidas nao confunde a nota recebida com uma emitida
    let feito = c.notas.reindexar().unwrap();
    assert_eq!(feito.ignorados, 0);
    assert_eq!(c.notas.listar().unwrap().len(), 1);
    assert_eq!(
        c.notas
            .recebidas()
            .unwrap()
            .registro(&chave)
            .unwrap()
            .situacao,
        SituacaoRecebida::Completa
    );
}
