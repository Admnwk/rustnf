// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! `cCredPres` no item da NF-e (grupo `gCredPresOper`): as regras da NT 2025.002 v1.53 que usam a tabela oficial
//! (`UB122-10`, `UB123-10`, `UB123-20`, `UB127-10` e `UB127-20`). O cClassTrib `000003` (regime automotivo, art. 311)
//! admite o credito presumido da operacao; o codigo `05` da tabela exige so' o grupo da CBS.

use rustnf::reforma::calculo::{
    ibscbs_do_item, AliqInformada, CredPres, CredPresTrib, EntradaItem,
};
use rustnf::reforma::ErroReforma;

fn trib() -> Option<CredPresTrib> {
    Some(CredPresTrib {
        p: "11.6".into(),
        cond_sus: false,
    })
}

fn entrada(codigo: &str, ibs: bool, cbs: bool) -> EntradaItem {
    EntradaItem {
        cst: "000".into(),
        cclass_trib: "000003".into(),
        vbc: "100.00".into(),
        data: "2026-11-20".into(),
        aliquotas: Some(AliqInformada {
            ibs_uf: "0.1".into(),
            ibs_mun: "0".into(),
            cbs: "0.9".into(),
        }),
        cred_pres: Some(CredPres {
            c_cred_pres: codigo.into(),
            vbc: None,
            ibs: if ibs { trib() } else { None },
            cbs: if cbs { trib() } else { None },
        }),
        ..EntradaItem::default()
    }
}

fn recusa(e: &EntradaItem) -> ErroReforma {
    ibscbs_do_item(e).unwrap_err()
}

#[test]
fn codigo_05_so_com_o_grupo_da_cbs_passa_e_sai_no_xml() {
    let it = ibscbs_do_item(&entrada("05", false, true)).unwrap();
    let xml = it.xml();
    assert!(
        xml.contains("<cCredPres>05</cCredPres><gCBSCredPres><pCredPres>11.6000</pCredPres><vCredPres>11.60</vCredPres></gCBSCredPres>"),
        "{xml}"
    );
    assert!(!xml.contains("gIBSCredPres"), "{xml}");
    // o codigo de 1 digito e' completado com zero, como o XSD pede
    assert!(ibscbs_do_item(&entrada("5", false, true))
        .unwrap()
        .xml()
        .contains("<cCredPres>05</cCredPres>"));
}

#[test]
fn ub122_10_codigo_inexistente_1055() {
    for c in ["00", "14", "99"] {
        let e = recusa(&entrada(c, false, true));
        assert_eq!(e.regra, "UB122-10/1055", "{c}: {e}");
        assert!(e.mensagem.contains("inexistente"), "{e}");
    }
}

#[test]
fn ub123_10_ibs_informado_onde_o_codigo_nao_permite_1053() {
    let e = recusa(&entrada("05", true, true));
    assert_eq!(e.regra, "UB123-10/1053", "{e}");
    assert!(e.mensagem.contains("gIBSCredPres"), "{e}");
}

#[test]
fn ub123_20_ibs_ausente_onde_o_codigo_exige_1054() {
    // 08 exige o IBS (e nao permite a CBS); o item da NF-e nao o leva sem o grupo
    let e = recusa(&entrada("08", false, false));
    assert_eq!(e.regra, "UB123-20/1054", "{e}");
}

#[test]
fn ub127_10_cbs_informada_onde_o_codigo_nao_permite_1050() {
    // 08 e 07 nao permitem a CBS (indicador 0 e vazio); a ordem da NT confere o IBS antes e ele esta certo
    for c in ["08", "07"] {
        let e = recusa(&entrada(c, true, true));
        assert_eq!(e.regra, "UB127-10/1050", "{c}: {e}");
    }
}

#[test]
fn ub127_20_cbs_ausente_onde_o_codigo_exige_1058() {
    // 01 exige os dois: com o IBS presente, falta a CBS
    let e = recusa(&entrada("01", true, false));
    assert_eq!(e.regra, "UB127-20/1058", "{e}");
}

#[test]
fn a_ordem_da_nt_confere_o_ibs_antes_da_cbs() {
    // 05 nao permite o IBS e exige a CBS: sem nenhum grupo so' a CBS falha (1058); com IBS e sem CBS, o IBS vem antes
    assert_eq!(recusa(&entrada("05", false, false)).regra, "UB127-20/1058");
    assert_eq!(recusa(&entrada("05", true, false)).regra, "UB123-10/1053");
}
