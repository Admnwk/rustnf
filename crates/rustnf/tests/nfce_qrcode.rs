// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! QR Code e `urlChave` da NFC-e e da NF-e com DANFE Simplificado Tipo 2 (`infNFeSupl`): versao 3 sem CSC, versao 2 com
//! CSC, on-line e off-line, a tabela de enderecos por UF e as recusas. O hash e a assinatura dos XML gerados aqui sao
//! conferidos por um calculo independente (script fora do repositorio); o CSC dos testes e' ficticio.

mod nfce_comum;

use nfce_comum::*;
use rustnf::assinatura::xmldsig;
use rustnf::nfe::montagem::{
    assinar, assinar_com, hash_v2, montar, nota_de_json, ufs_com_qrcode, url_chave, url_qrcode,
    Csc, OpcoesQr,
};
use rustnf::nfe::vigencias::Ambiente::{Homologacao, Producao};
use serde_json::json;

/// CSC ficticio (16 a 36 caracteres alfanumericos): nao e' de ninguem.
const CSC_FICTICIO: &str = "ABCDEFGHIJKLMNOP1234567890ABCDEF";

const OFF: &str = r#"{"tp_emis": 9, "dh_cont": "2026-10-03T08:40:00-03:00", "x_just": "Falha de comunicacao com a SEFAZ"}"#;

fn assinada(nome: &str, altera: serde_json::Value, op: &OpcoesQr) -> (String, String) {
    let a = arquivo();
    let m = montar(&nota_de_json(&pedido(&a, &altera)).unwrap()).unwrap();
    let xml = assinar_com(&m, &certificado(), op).unwrap();
    guardar(nome, &xml);
    (xml, m.chave)
}

fn qr_de(xml: &str) -> String {
    let i = xml.find("<qrCode>").unwrap() + "<qrCode>".len();
    let f = i + xml[i..].find("</qrCode>").unwrap();
    xml[i..f].to_string()
}

fn com_csc() -> OpcoesQr {
    OpcoesQr {
        versao: None,
        csc: Some(Csc::novo("000001", CSC_FICTICIO).unwrap()),
    }
}

fn off() -> serde_json::Value {
    serde_json::from_str(OFF).unwrap()
}

#[test]
fn o_hash_da_versao_2_bate_com_o_calculo_independente() {
    // O Manual de Padroes Tecnicos do DANFE NFC-e e QR Code 6.0 (item 4.3.6.2, contingencia off-line, versao 2) publica
    // um exemplo e a saida do SHA-1 dele (`4615A93B...`); o calculo independente do script de conferencia reproduz esse
    // valor. Aqui os mesmos parametros com a chave ficticia do repositorio: o `DigestValue` do exemplo em hexa (56
    // caracteres), o dia 02, o valor 60.90 e o CSC de exemplo do manual, com o hash que o mesmo script calcula.
    let parametros = concat!(
        "35261011222333000181650010000001001123456706|2|1|02|60.90|",
        "797a4759685578312f5859597a6b7357422b6650523351633530633d|1"
    );
    assert_eq!(
        hash_v2(parametros, "SEU-CODIGO-CSC-CONTRIBUINTE-36-CARACTERES"),
        "23C4848930B3E411555D28F0016C758D2ED6372B"
    );
}

#[test]
fn versao_3_on_line_sao_so_chave_versao_e_ambiente() {
    let (xml, chave) = assinada("nfce_v3_online_hom", json!({}), &OpcoesQr::default());
    assert!(xmldsig::conferido(&xml), "{xml}");
    assert_eq!(
        qr_de(&xml),
        format!("https://www.homologacao.nfce.fazenda.sp.gov.br/qrcode?p={chave}|3|2")
    );
    assert!(xml
        .contains("<urlChave>https://www.homologacao.nfce.fazenda.sp.gov.br/consulta</urlChave>"));
    // infNFeSupl entre o infNFe e a Signature, e a Signature continua a ultima
    let (inf, supl, sig) = (
        xml.find("</infNFe>").unwrap(),
        xml.find("<infNFeSupl>").unwrap(),
        xml.find("<Signature").unwrap(),
    );
    assert!(inf < supl && supl < sig && xml.ends_with("</Signature></NFe>"));
    let (xml, chave) = assinada(
        "nfce_v3_online_prod",
        json!({"tp_amb": 1}),
        &OpcoesQr::default(),
    );
    assert_eq!(
        qr_de(&xml),
        format!("https://www.nfce.fazenda.sp.gov.br/qrcode?p={chave}|3|1")
    );
}

#[test]
fn versao_2_on_line_leva_o_id_do_csc_sem_zeros_e_o_hash() {
    let (xml, chave) = assinada("nfce_v2_online", json!({}), &com_csc());
    let base = format!("{chave}|2|2|1");
    let esperado = format!(
        "https://www.homologacao.nfce.fazenda.sp.gov.br/qrcode?p={base}|{}",
        hash_v2(&base, CSC_FICTICIO)
    );
    assert_eq!(qr_de(&xml), esperado);
    // o CSC nunca vai ao XML
    assert!(!xml.contains(CSC_FICTICIO));
}

#[test]
fn versao_2_off_line_leva_dia_valor_e_o_digest_em_hexa() {
    let (xml, chave) = assinada("nfce_v2_offline", off(), &com_csc());
    let i = xml.find("<DigestValue>").unwrap() + "<DigestValue>".len();
    let digest = &xml[i..i + xml[i..].find("</DigestValue>").unwrap()];
    let hexa: String = digest.bytes().map(|b| format!("{b:02x}")).collect();
    assert_eq!(hexa.len(), 56);
    let base = format!("{chave}|2|2|03|31.80|{hexa}|1");
    let esperado = format!(
        "https://www.homologacao.nfce.fazenda.sp.gov.br/qrcode?p={base}|{}",
        hash_v2(&base, CSC_FICTICIO)
    );
    assert_eq!(qr_de(&xml), esperado);
    assert!(xmldsig::conferido(&xml));
}

#[test]
fn versao_3_off_line_leva_dia_valor_destinatario_e_a_assinatura() {
    // sem destinatario: dois separadores vazios antes da assinatura
    let (xml, chave) = assinada("nfce_v3_offline", off(), &OpcoesQr::default());
    let qr = qr_de(&xml);
    let pre =
        format!("https://www.homologacao.nfce.fazenda.sp.gov.br/qrcode?p={chave}|3|2|03|31.80|||");
    assert!(qr.starts_with(&pre), "{qr}");
    let sig = &qr[pre.len()..];
    // RSA de 2048 bits: 256 octetos, 344 caracteres em base64
    assert_eq!(sig.len(), 344, "{sig}");
    assert!(sig
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b"+/=".contains(&b)));
    // com CPF: tipo 2 e o numero
    let mut com_cpf = off();
    com_cpf["dest"] = json!({"documento": "52998224725", "ind_ie_dest": 9});
    let (xml, chave) = assinada("nfce_v3_offline_cpf", com_cpf, &OpcoesQr::default());
    let pre = format!(
        "https://www.homologacao.nfce.fazenda.sp.gov.br/qrcode?p={chave}|3|2|03|31.80|2|52998224725|"
    );
    assert!(qr_de(&xml).starts_with(&pre), "{}", qr_de(&xml));
}

#[test]
fn a_nfe_com_danfe_simplificado_usa_o_qr_da_versao_3_e_recusa_a_2() {
    let a = arquivo();
    let caso = a["casos"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["nome"] == "nfe55_danfe_simplificado")
        .unwrap();
    let altera = caso["altera"].clone();
    let (xml, chave) = assinada("nfe55_tpimp6_v3", altera.clone(), &OpcoesQr::default());
    assert_eq!(
        qr_de(&xml),
        format!("https://www.homologacao.nfce.fazenda.sp.gov.br/qrcode?p={chave}|3|2")
    );
    // a versao 2 pedida e' recusada (rejeicao 672)
    let m = montar(&nota_de_json(&pedido(&a, &altera)).unwrap()).unwrap();
    let pede_2 = OpcoesQr {
        versao: Some(2),
        csc: Some(Csc::novo("1", CSC_FICTICIO).unwrap()),
    };
    let e = assinar_com(&m, &certificado(), &pede_2).unwrap_err();
    assert_eq!((e.campo.as_str(), e.regra.as_str()), ("qrCode", "672"));
}

#[test]
fn a_nfe_comum_continua_sem_qr_code() {
    let a = arquivo();
    let altera = json!({"modelo": 55, "tp_imp": 1,
        "dest": {"documento": "52998224725", "nome": "CLIENTE", "ind_ie_dest": 9,
                 "endereco": {"logradouro": "Rua A", "numero": "1", "bairro": "Centro",
                              "cod_municipio": "3550308", "municipio": "Sao Paulo", "uf": "SP"}}});
    let m = montar(&nota_de_json(&pedido(&a, &altera)).unwrap()).unwrap();
    let xml = assinar(&m, &certificado()).unwrap();
    assert!(!xml.contains("infNFeSupl"));
}

#[test]
fn a_versao_2_sem_csc_e_o_csc_invalido_sao_recusados_sem_repetir_o_csc() {
    let a = arquivo();
    let m = montar(&nota_de_json(&pedido(&a, &json!({}))).unwrap()).unwrap();
    let so_v2 = OpcoesQr {
        versao: Some(2),
        csc: None,
    };
    assert_eq!(
        assinar_com(&m, &certificado(), &so_v2).unwrap_err().campo,
        "CSC"
    );
    for (id, csc) in [
        ("", CSC_FICTICIO),
        ("1234567", CSC_FICTICIO),
        ("x", CSC_FICTICIO),
        ("1", "CURTO"),
        ("1", "ABCDEFGHIJKLMNOP-1234567890ABCDEF"),
    ] {
        let e = Csc::novo(id, csc).unwrap_err();
        assert!(e.campo == "idCSC" || e.campo == "CSC", "{e:?}");
        assert!(!e.mensagem.contains(csc), "{e}");
    }
    let c = Csc::novo("000012", CSC_FICTICIO).unwrap();
    assert_eq!(c.id(), "12");
    assert!(
        !format!("{c:?}").contains(CSC_FICTICIO),
        "o Debug nao pode mostrar o CSC"
    );
}

#[test]
fn o_produtor_rural_cpf_nao_usa_a_versao_2() {
    let a = arquivo();
    let altera = json!({"emit.documento": "52998224725", "tp_amb": 1});
    let m = montar(&nota_de_json(&pedido(&a, &altera)).unwrap()).unwrap();
    let e = assinar_com(&m, &certificado(), &com_csc()).unwrap_err();
    assert_eq!(e.regra, "444");
    // sem CSC o padrao e' a versao 3
    let xml = assinar(&m, &certificado()).unwrap();
    assert!(qr_de(&xml).ends_with("|3|1"));
}

#[test]
fn uf_fora_da_tabela_diz_por_que_e_nao_chuta() {
    let a = arquivo();
    let altera = json!({"emit.endereco.uf": "AM", "emit.endereco.cod_municipio": "1302603",
                        "emit.endereco.municipio": "Manaus", "cmun_fg": "1302603"});
    let m = montar(&nota_de_json(&pedido(&a, &altera)).unwrap()).unwrap();
    let e = assinar(&m, &certificado()).unwrap_err();
    assert_eq!(e.campo, "qrCode");
    assert!(
        e.mensagem.contains("AM") && e.mensagem.contains("protocolo"),
        "{e}"
    );
    assert!(url_qrcode("AM", Producao).is_err() && url_qrcode("MA", Homologacao).is_err());
}

#[test]
fn a_tabela_cobre_25_uf_com_endereco_completo_e_a_consulta_por_chave_as_27() {
    let ufs: Vec<&str> = ufs_com_qrcode().collect();
    assert_eq!(ufs.len(), 25, "{ufs:?}");
    for uf in &ufs {
        for amb in [Producao, Homologacao] {
            // a unica linha que falta na tabela e' a homologacao de RR (publicada so' como numero IP)
            if (*uf, amb) == ("RR", Homologacao) {
                assert!(url_qrcode(uf, amb).unwrap_err().mensagem.contains("IP"));
                continue;
            }
            let u = url_qrcode(uf, amb).unwrap();
            assert!(
                u.starts_with("http://") || u.starts_with("https://"),
                "{uf}: {u}"
            );
            assert!(!u.contains('?') && !u.contains(' '), "{uf}: {u}");
        }
    }
    for uf in [
        "AC", "AL", "AM", "AP", "BA", "CE", "DF", "ES", "GO", "MA", "MG", "MS", "MT", "PA", "PB",
        "PE", "PI", "PR", "RJ", "RN", "RO", "RR", "RS", "SC", "SE", "SP", "TO",
    ] {
        for amb in [Producao, Homologacao] {
            let u = url_chave(uf, amb).unwrap();
            assert!((20..=85).contains(&u.len()), "{uf}: {u}");
        }
    }
}
