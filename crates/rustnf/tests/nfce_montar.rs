// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Montagem da NFC-e (modelo 65) e da NF-e com DANFE Simplificado Tipo 2: `tests/casos/nfce_montar.json`. Os casos
//! trazem o XML esperado e a chave; as regras trazem uma entrada que a SEFAZ rejeitaria, com o `cStat` da rejeicao.
//! `montar` recusa com o codigo e `conferir` o lista na frente.

mod nfce_comum;

use nfce_comum::*;
use rustnf::nfe::montagem::{conferir, montar, nota_de_json};

#[test]
fn os_casos_montam_o_xml_e_a_chave_esperados() {
    let a = arquivo();
    let mut n = 0;
    for c in a["casos"].as_array().unwrap() {
        let nome = c["nome"].as_str().unwrap();
        let nota =
            nota_de_json(&pedido(&a, &c["altera"])).unwrap_or_else(|e| panic!("{nome}: {e}"));
        let m = montar(&nota).unwrap_or_else(|e| panic!("{nome}: {e}"));
        assert_eq!(m.chave, c["chave"].as_str().unwrap(), "{nome}: chave");
        assert_eq!(m.xml, c["xml"].as_str().unwrap(), "{nome}: xml");
        // a mesma entrada da o mesmo XML
        assert_eq!(montar(&nota).unwrap().xml, m.xml, "{nome}: determinismo");
        guardar(nome, &m.xml);
        n += 1;
    }
    assert_eq!(n, 8);
}

#[test]
fn cada_regra_recusa_com_o_codigo_e_o_conferir_lista_na_frente() {
    let a = arquivo();
    let mut n = 0;
    for r in a["regras"].as_array().unwrap() {
        let nome = r["nome"].as_str().unwrap();
        let cstat = r["cstat"].as_str().unwrap();
        let nota =
            nota_de_json(&pedido(&a, &r["altera"])).unwrap_or_else(|e| panic!("{nome}: {e}"));
        let e = montar(&nota).unwrap_err();
        assert_eq!(e.regra, cstat, "{nome}: {e}");
        let lista = conferir(&nota);
        assert!(
            lista
                .iter()
                .any(|l| l.starts_with(&format!("{cstat}-Rejeicao: "))),
            "{nome}: {lista:?}"
        );
        n += 1;
    }
    assert_eq!(n, 38);
}

#[test]
fn a_nfce_leva_os_padroes_do_modelo_65() {
    let a = arquivo();
    let nota = nota_de_json(&pedido(&a, &serde_json::json!({}))).unwrap();
    let m = montar(&nota).unwrap();
    for trecho in [
        "<mod>65</mod>",
        "<tpImp>4</tpImp>",
        "<indFinal>1</indFinal>",
        "<indPres>1</indPres>",
        "<modFrete>9</modFrete>",
    ] {
        assert!(m.xml.contains(trecho), "{trecho}: {}", m.xml);
    }
    // sem consumidor identificado a NFC-e nao tem o grupo dest
    assert!(!m.xml.contains("<dest>"));
    assert_eq!(&m.chave[20..22], "65");
}

#[test]
fn o_primeiro_item_em_homologacao_leva_o_texto_fixo() {
    let a = arquivo();
    let m = montar(&nota_de_json(&pedido(&a, &serde_json::json!({}))).unwrap()).unwrap();
    assert!(m.xml.contains(
        "<xProd>NOTA FISCAL EMITIDA EM AMBIENTE DE HOMOLOGACAO - SEM VALOR FISCAL</xProd>"
    ));
    let p = montar(&nota_de_json(&pedido(&a, &serde_json::json!({"tp_amb": 1}))).unwrap()).unwrap();
    assert!(p.xml.contains("<xProd>Cafe torrado 500g</xProd>"));
}

#[test]
fn a_nota_sem_regra_quebrada_nao_tem_rejeicao_de_varejo() {
    let a = arquivo();
    let nota = nota_de_json(&pedido(&a, &serde_json::json!({}))).unwrap();
    assert!(conferir(&nota).is_empty(), "{:?}", conferir(&nota));
}
