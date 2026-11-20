// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! `reforma::indop`: `dados/indop.json`: tabela cIndOp do Anexo VII RTC v1.03.00.

use rustnf::reforma::indop::{permitido, registro, tabela, versao};

#[test]
fn tabela_oficial_extraida() {
    assert_eq!((versao(), tabela().len()), ("1.03.00", 40));
    let na_nfe: Vec<&str> = tabela()
        .values()
        .filter(|x| x.ind_nfe)
        .map(|x| x.codigo.as_str())
        .collect();
    assert_eq!(na_nfe, ["010104", "010105"]);
    assert!(registro("010104")
        .unwrap()
        .dispositivo
        .starts_with("Art. 11, I"));
    assert!(registro("020102").is_some(), "020102 entrou na v1.03.00");
    assert!(registro("999999").is_none() && registro("").is_none());
}

#[test]
fn permitido_por_documento() {
    assert!(permitido("010104", "55").unwrap() && permitido("010104", "nfe").unwrap());
    assert!(!permitido("010101", "55").unwrap() && permitido("010101", "nfse").unwrap());
    assert!(permitido("080101", "nfsvia").unwrap() && !permitido("080101", "nfse").unwrap());
    assert!(!permitido("010104", "65").unwrap() && !permitido("010104", "nfce").unwrap());
    assert!(
        !permitido("010201", "nfe").unwrap() && !permitido("010201", "nfse").unwrap(),
        "veiculo: nenhum DF-e hoje"
    );
    assert!(permitido("010104", "cte").is_err());
}
