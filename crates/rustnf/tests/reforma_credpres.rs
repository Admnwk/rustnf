// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! `reforma::credpres`: `dados/ccredpres.json`, tabela cCredPres da IT 2025.002 v1.70, e as regras da NT 2025.002 v1.53
//! que a usam. A tabela esperada abaixo foi escrita a mao, linha a linha, da planilha oficial.

use rustnf::reforma::credpres::{conferir_grupos, descricao, registro, tabela, versao};

/// codigo, indicador do IBS, indicador da CBS (None = celula vazia), apropria via NF, via evento, indNFe, indNFCe,
/// indCTe, indNFSe.
type Linha = (
    &'static str,
    Option<bool>,
    Option<bool>,
    bool,
    bool,
    [bool; 4],
);

const OFICIAL: [Linha; 13] = [
    (
        "01",
        Some(true),
        Some(true),
        false,
        true,
        [true, false, false, true],
    ),
    (
        "02",
        Some(true),
        Some(true),
        false,
        true,
        [false, false, true, true],
    ),
    (
        "03",
        Some(true),
        Some(true),
        false,
        true,
        [true, false, false, false],
    ),
    (
        "04",
        Some(true),
        Some(true),
        true,
        false,
        [true, true, false, false],
    ),
    (
        "05",
        Some(false),
        Some(true),
        true,
        false,
        [true, false, false, false],
    ),
    (
        "06",
        Some(false),
        Some(true),
        true,
        false,
        [true, false, false, false],
    ),
    (
        "07",
        Some(true),
        None,
        true,
        false,
        [true, false, false, false],
    ),
    (
        "08",
        Some(true),
        Some(false),
        false,
        true,
        [true, false, false, false],
    ),
    (
        "09",
        Some(true),
        Some(false),
        false,
        true,
        [true, false, false, false],
    ),
    (
        "10",
        Some(false),
        Some(true),
        true,
        false,
        [true, false, false, false],
    ),
    (
        "11",
        Some(true),
        Some(false),
        true,
        false,
        [true, false, false, false],
    ),
    (
        "12",
        Some(true),
        Some(false),
        false,
        true,
        [true, false, false, false],
    ),
    (
        "13",
        Some(false),
        Some(true),
        true,
        false,
        [true, false, false, false],
    ),
];

#[test]
fn tabela_oficial_extraida() {
    assert_eq!((versao(), tabela().len()), ("1.70", 13));
    for (cod, ibs, cbs, via_nf, via_evento, [nfe, nfce, cte, nfse]) in OFICIAL {
        let r = registro(cod).unwrap_or_else(|| panic!("{cod} faltando"));
        assert_eq!((r.ind_ibs, r.ind_cbs), (ibs, cbs), "indicadores de {cod}");
        assert_eq!(
            (r.via_nf, r.via_evento),
            (via_nf, via_evento),
            "via de {cod}"
        );
        assert_eq!(
            (r.ind_nfe, r.ind_nfce, r.ind_cte, r.ind_nfse),
            (nfe, nfce, cte, nfse),
            "documentos de {cod}"
        );
    }
    assert!(registro("14").is_none() && registro("").is_none() && registro("1").is_none());
}

#[test]
fn consulta_da_descricao_e_do_que_o_codigo_exige() {
    assert!(descricao("01").unwrap().contains("art. 168"));
    assert!(descricao("05").unwrap().contains("regime automotivo"));
    assert!(descricao("13").unwrap().contains("art. 467"));
    assert_eq!(descricao("99"), None);
    let sete = registro("07").unwrap();
    assert!(sete.exige_ibs() && !sete.exige_cbs(), "CBS vazia nao exige");
    assert_eq!(sete.ini_ibs.as_deref(), Some("2027-01-01"));
    assert_eq!(sete.ini_cbs, None);
    assert_eq!(
        registro("03").unwrap().ini_ibs.as_deref(),
        Some("2029-01-01")
    );
    assert_eq!(registro("04").unwrap().deduz, Some(true));
    assert_eq!(registro("01").unwrap().deduz, None);
}

#[test]
fn todo_codigo_contra_as_quatro_combinacoes_de_grupos() {
    for (cod, ibs, cbs, ..) in OFICIAL {
        for tem_ibs in [false, true] {
            for tem_cbs in [false, true] {
                // escrito sem olhar o codigo da biblioteca: o grupo e' exigido so' com indicador 1
                let esperado = match (ibs == Some(true), tem_ibs, cbs == Some(true), tem_cbs) {
                    (true, false, _, _) => Some("1054"),
                    (false, true, _, _) => Some("1053"),
                    (_, _, true, false) => Some("1058"),
                    (_, _, false, true) => Some("1050"),
                    _ => None,
                };
                let obtido = conferir_grupos(cod, tem_ibs, tem_cbs).map(|v| v.rejeicao);
                assert_eq!(obtido, esperado, "{cod} ibs={tem_ibs} cbs={tem_cbs}");
            }
        }
    }
}

#[test]
fn codigo_inexistente_e_a_rejeicao_1055_nos_dois_documentos() {
    for c in ["00", "14", "99", "", "1", "001", "AB"] {
        let v = conferir_grupos(c, true, true).unwrap();
        assert_eq!(
            (v.rejeicao, v.regra_item, v.regra_evento, v.campo),
            ("1055", "UB122-10", "2P25a-10", "cCredPres"),
            "{c:?}"
        );
    }
}

#[test]
fn cada_rejeicao_tem_as_suas_regras_e_o_seu_campo() {
    let casos = [
        (
            "05",
            true,
            true,
            ("1053", "UB123-10", "2P26-10", "gIBSCredPres"),
        ),
        (
            "08",
            false,
            false,
            ("1054", "UB123-20", "2P26-20", "gIBSCredPres"),
        ),
        (
            "08",
            true,
            true,
            ("1050", "UB127-10", "2P30-10", "gCBSCredPres"),
        ),
        (
            "01",
            true,
            false,
            ("1058", "UB127-20", "2P30-20", "gCBSCredPres"),
        ),
    ];
    for (c, ibs, cbs, (rej, item, evento, campo)) in casos {
        let v = conferir_grupos(c, ibs, cbs).unwrap();
        assert_eq!(
            (v.rejeicao, v.regra_item, v.regra_evento, v.campo),
            (rej, item, evento, campo),
            "{c} ibs={ibs} cbs={cbs}"
        );
        assert!(
            v.mensagem.starts_with(&format!("cCredPres {c}")),
            "{}",
            v.mensagem
        );
    }
}

#[test]
fn indicador_vazio_nao_permite_o_grupo() {
    // 07 (CBS vazia): so' o IBS, e a CBS informada e' indevida
    assert!(conferir_grupos("07", true, false).is_none());
    assert_eq!(conferir_grupos("07", true, true).unwrap().rejeicao, "1050");
}
