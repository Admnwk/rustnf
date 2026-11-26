// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Assinatura da NF-e montada: o `infNFe` e' assinado com o A1 (certificado de teste) por RSA-SHA1 e digest SHA-1, os
//! unicos algoritmos que o XSD da NF-e 4.00 aceita, e a conferencia do modulo `assinatura` aprova.

use chrono::Utc;
use rustnf::assinatura::cert_a1;
use rustnf::assinatura::xmldsig;
use rustnf::nfe::montagem::{assinar, montar, nota_de_json};
use serde_json::{json, Value};

fn pedido(tp_imp: Option<i64>) -> Value {
    let mut p = json!({
        "modelo": 55, "serie": 1, "numero": 10, "emissao": "2026-10-03T09:00:00-03:00",
        "nat_op": "VENDA", "cmun_fg": "3550308", "codigo": 12345670,
        "emit": {"documento": "11222333000181", "nome": "LOJA DE TESTE LTDA", "crt": 1,
                 "ie": "111111111111",
                 "endereco": {"logradouro": "Rua das Flores", "numero": "100", "bairro": "Centro",
                              "cod_municipio": "3550308", "municipio": "Sao Paulo", "uf": "SP",
                              "cep": "01001000"}},
        "dest": {"documento": "52998224725", "nome": "CLIENTE TESTE", "ind_ie_dest": 9,
                 "endereco": {"logradouro": "Rua A", "numero": "1", "bairro": "Centro",
                              "cod_municipio": "3550308", "municipio": "Sao Paulo", "uf": "SP"}},
        "ind_final": 1, "ind_pres": 1,
        "itens": [{"codigo": "1", "descricao": "Cafe torrado 500g", "ncm": "09012100", "cfop": "5102",
                   "unidade": "UN", "quantidade": "2", "valor_unitario": "15.90",
                   "icms": {"codigo": "102"}, "pis": {"cst": "49"}, "cofins": {"cst": "49"}}],
        "pagamentos": [{"t_pag": "01", "valor": "31.80"}]
    });
    if let Some(t) = tp_imp {
        p["tp_imp"] = json!(t);
    }
    p
}

fn certificado() -> cert_a1::Certificado {
    let pfx = cert_a1::gerar_teste(
        "11222333000181",
        "LOJA DE TESTE",
        "teste",
        365,
        true,
        false,
        Utc::now(),
    )
    .unwrap();
    cert_a1::carregar(&pfx, "teste").unwrap()
}

#[test]
fn o_infnfe_sai_assinado_em_sha1_e_a_assinatura_confere() {
    let nota = nota_de_json(&pedido(None)).unwrap();
    let m = montar(&nota).unwrap();
    let xml = assinar(&m, &certificado()).unwrap();
    assert!(xmldsig::conferido(&xml), "{xml}");
    // algoritmos fixados pelo XSD da NF-e (xmldsig-core-schema_v1.01.xsd)
    assert!(xml.contains("Algorithm=\"http://www.w3.org/2000/09/xmldsig#rsa-sha1\""));
    assert!(xml.contains("Algorithm=\"http://www.w3.org/2000/09/xmldsig#sha1\""));
    assert!(xml.contains("Algorithm=\"http://www.w3.org/TR/2001/REC-xml-c14n-20010315\""));
    // a Reference aponta para o Id do infNFe e a Signature e' o ultimo filho da NFe, depois do infNFe
    assert!(xml.contains(&format!("<Reference URI=\"#NFe{}\">", m.chave)));
    let fim_inf = xml.find("</infNFe>").unwrap();
    let sig = xml.find("<Signature").unwrap();
    assert!(fim_inf < sig);
    assert!(xml.ends_with("</Signature></NFe>"));
    // o resto do XML e' o montado, intacto
    assert_eq!(
        xml.replace(&xml[sig..xml.len() - "</NFe>".len()], ""),
        m.xml
    );
}

#[test]
fn alterar_o_xml_depois_de_assinar_derruba_a_conferencia() {
    let m = montar(&nota_de_json(&pedido(None)).unwrap()).unwrap();
    let xml = assinar(&m, &certificado()).unwrap();
    let adulterado = xml.replace("<vNF>31.80</vNF>", "<vNF>1.80</vNF>");
    assert_ne!(adulterado, xml);
    assert!(!xmldsig::conferido(&adulterado));
}

#[test]
fn o_danfe_simplificado_sai_assinado_com_o_qr_code() {
    let m = montar(&nota_de_json(&pedido(Some(6))).unwrap()).unwrap();
    let xml = assinar(&m, &certificado()).unwrap();
    assert!(xmldsig::conferido(&xml), "{xml}");
    // o QR Code (versao 3, sem CSC) fica entre o infNFe e a Signature
    let supl = xml.find("<infNFeSupl>").unwrap();
    assert!(xml.find("</infNFe>").unwrap() < supl && supl < xml.find("<Signature").unwrap());
    assert!(
        xml.contains(&format!("?p={}|3|2</qrCode>", m.chave)),
        "{xml}"
    );
}
