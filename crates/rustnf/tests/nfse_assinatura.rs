// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! xmldsig + cert_a1: C14N octeto a octeto (`tests/casos/nfse_nacional.json`, secao `c14n`), DPS assinada fora
//! desta biblioteca e conferida aqui, assinatura feita aqui e conferida aqui, e o certificado A1.

use std::path::PathBuf;

use chrono::{Duration, Utc};
use rustnf::assinatura::cert_a1::{self, Situacao};
use rustnf::assinatura::xmldsig::{self, Algoritmo};
use rustnf::nfse::{montar, Dps};
use serde_json::Value;

fn casos() -> Value {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/casos/nfse_nacional.json");
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn a1(doc: &str) -> cert_a1::Certificado {
    let pfx =
        cert_a1::gerar_teste(doc, "OFICINA TESTE", "teste", 365, true, false, Utc::now()).unwrap();
    cert_a1::carregar(&pfx, "teste").unwrap()
}

fn dps_minima() -> String {
    let c = casos();
    let d: Dps = serde_json::from_value(c["dps"][0]["entrada"].clone()).unwrap();
    montar(&d).unwrap()
}

#[test]
fn c14n_confere_octeto_a_octeto() {
    for c in casos()["c14n"].as_array().unwrap() {
        let xml = c["xml"].as_str().unwrap();
        let doc = xmldsig::ler(xml).unwrap();
        let alvo = xmldsig::achar(&doc, c["alvo"].as_str().unwrap()).unwrap();
        let r = xmldsig::c14n(
            alvo,
            c["exclusiva"].as_bool().unwrap(),
            c["comentarios"].as_bool().unwrap(),
            None,
        );
        assert_eq!(r, c["saida"].as_str().unwrap(), "caso {c}");
    }
}

#[test]
fn confere_dps_assinada_externamente() {
    let c = casos();
    for a in c["assinada_externa"].as_array().unwrap() {
        let r = xmldsig::conferir(a["xml"].as_str().unwrap());
        assert_eq!(r.len(), 1);
        assert!(r[0].ok, "{:?}", r[0].motivo);
        assert_eq!(r[0].algoritmo, a["algoritmo"].as_str().unwrap());
        assert!(
            r[0].titular.contains("ASSINADO EXTERNO"),
            "{}",
            r[0].titular
        );
    }
    let r = xmldsig::conferir(c["adulterada_externa"].as_str().unwrap());
    assert!(!r[0].ok && !r[0].digest_ok && r[0].assinatura_ok);
}

#[test]
fn assina_confere_e_detecta_adulteracao() {
    let a = a1("11222333000181");
    for alg in [Algoritmo::Sha1, Algoritmo::Sha256] {
        let s =
            xmldsig::assinar(&dps_minima(), &a.chave, &a.cert_der, "infDPS", alg, None).unwrap();
        let r = xmldsig::conferir(&s);
        assert!(
            r.len() == 1 && r[0].ok && r[0].algoritmo == alg.nome(),
            "{:?}",
            r[0].motivo
        );
        assert!(
            s.find("</infDPS>").unwrap() < s.find("<Signature").unwrap()
                && s.trim_end().ends_with("</DPS>")
        );
        assert!(!s.contains("ds:"));
        assert!(!xmldsig::conferido(
            &s.replace("<vServ>150.00</vServ>", "<vServ>15.00</vServ>")
        ));
    }
}

#[test]
fn assina_xml_de_outra_origem_e_dentro_do_elemento() {
    let a = a1("11222333000181");
    let xml = "<?xml version='1.0' encoding='UTF-8'?>\r\n<pedRegEvento xmlns='http://www.sped.fazenda.gov.br/nfse' \
               xmlns:xsi='http://www.w3.org/2001/XMLSchema-instance' versao='1.01'>\r\n  <infPedReg versao='x' \
               Id='PRE1'><tpAmb>2</tpAmb><vazio/><t b='2' a='1'>D&apos;A</t></infPedReg>\r\n</pedRegEvento>";
    let s = xmldsig::assinar(
        xml,
        &a.chave,
        &a.cert_der,
        "infPedReg",
        Algoritmo::Sha1,
        None,
    )
    .unwrap();
    assert!(
        xmldsig::conferido(&s),
        "{:?}",
        xmldsig::conferir(&s)[0].motivo
    );
    let s = xmldsig::assinar(
        &dps_minima(),
        &a.chave,
        &a.cert_der,
        "infDPS",
        Algoritmo::Sha1,
        Some("infDPS"),
    )
    .unwrap();
    assert!(s.find("<Signature").unwrap() < s.find("</infDPS>").unwrap());
    assert!(xmldsig::conferido(&s));
}

#[test]
fn recusa_id_repetido_e_dtd() {
    let a = a1("11222333000181");
    let x = dps_minima().replace(
        "<serie>",
        "<x Id=\"DPS355030821122233300018100001000000000000042\"></x><serie>",
    );
    assert!(
        xmldsig::assinar(&x, &a.chave, &a.cert_der, "infDPS", Algoritmo::Sha1, None)
            .unwrap_err()
            .0
            .contains("repetido")
    );
    assert!(xmldsig::conferir(
        "<!DOCTYPE a [<!ENTITY e SYSTEM \"file:///c:/windows/win.ini\">]><a>&e;</a>"
    )
    .is_empty());
}

#[test]
fn certificado_documento_pelo_campo_icp_e_avisos() {
    let agora = Utc::now();
    let pf = a1("52998224725");
    assert_eq!(
        (pf.cpf.as_str(), pf.cnpj.as_str(), pf.documento_pelo_cn),
        ("52998224725", "", false)
    );
    let alfa = a1("12ABC34501DE35");
    assert!(alfa.cnpj == "12ABC34501DE35" && alfa.icp_brasil);
    let t = alfa.testar(Some("12ABC34599DE35"), agora, 30);
    assert!(
        t.ok && t.avisos.iter().any(|a| a
            == "O certificado e' de outra filial da mesma empresa (raiz do CNPJ igual): a NFS-e aceita, a regra e' a raiz."),
        "{t:?}"
    );
    assert!(alfa.testar(Some("11222333000181"), agora, 30).avisos.iter().any(|a| a
        == "O certificado e' de outro documento (CNPJ/CPF) que nao o da empresa configurada: a nota assinada com ele volta \
            recusada (NFS-e E0718)."));
    let curto =
        cert_a1::gerar_teste("11222333000181", "X", "teste", 10, true, true, agora).unwrap();
    let c = cert_a1::carregar(&curto, "teste").unwrap();
    assert_eq!(c.situacao(agora, 30), Situacao::VenceEmBreve);
    assert_eq!(c.titular, "X");
    let velho = cert_a1::gerar_teste(
        "11222333000181",
        "X",
        "teste",
        1,
        false,
        false,
        agora - Duration::days(30),
    )
    .unwrap();
    let v = cert_a1::carregar(&velho, "teste").unwrap();
    assert_eq!(v.situacao(agora, 30), Situacao::Vencido);
    assert!(!v.testar(None, agora, 30).ok);
}

#[test]
fn resumo_diz_quem_e_ate_quando_vale_e_se_assina() {
    let agora = Utc::now();
    // pessoa juridica, valido por um ano: CNPJ inteiro, datas e dias
    let pj = a1("11222333000181");
    let r = pj.resumo(Some("11222333000181"), agora, 30);
    assert!(r.ok && r.avisos.is_empty(), "{r:?}");
    assert_eq!(
        (r.titular.as_str(), r.documento.as_str(), r.situacao),
        ("OFICINA TESTE", "11222333000181", "valido")
    );
    assert_eq!(
        (r.icp_brasil, r.emissor.as_str()),
        (true, cert_a1::AC_TESTE)
    );
    assert_eq!(r.dias_restantes, pj.dias_restantes(agora));
    assert!((364..=365).contains(&r.dias_restantes), "{r:?}");
    for data in [&r.valido_de, &r.valido_ate] {
        assert!(
            data.len() == 10 && data.as_bytes()[4] == b'-' && data.as_bytes()[7] == b'-',
            "{data}"
        );
    }
    assert!(r.valido_de < r.valido_ate);
    // o aviso de outro documento e' o da biblioteca
    let outro = pj.resumo(Some("12ABC34501DE35"), agora, 30);
    assert!(
        outro.ok && outro.avisos.iter().any(|a| a.contains("E0718")),
        "{outro:?}"
    );

    // pessoa fisica: o CPF sai mascarado, tambem no JSON
    let pf = a1("52998224725").resumo(None, agora, 30);
    assert_eq!(pf.documento, "529.***.***-25");
    let json = serde_json::to_string(&pf).unwrap();
    assert!(!json.contains("52998224725"), "{json}");
    assert!(json.contains("\"dias_restantes\""), "{json}");

    // vence em breve e vencido
    let curto =
        cert_a1::gerar_teste("11222333000181", "X", "teste", 10, true, true, agora).unwrap();
    let r = cert_a1::carregar(&curto, "teste")
        .unwrap()
        .resumo(None, agora, 30);
    assert!(
        r.ok && r.situacao == "vence_em_breve" && !r.avisos.is_empty(),
        "{r:?}"
    );
    let velho = cert_a1::gerar_teste(
        "11222333000181",
        "X",
        "teste",
        1,
        false,
        false,
        agora - Duration::days(30),
    )
    .unwrap();
    let r = cert_a1::carregar(&velho, "teste")
        .unwrap()
        .resumo(None, agora, 30);
    assert!(
        !r.ok && r.situacao == "vencido" && r.dias_restantes < 0,
        "{r:?}"
    );
    assert!(r.frase.contains("VENCIDO"), "{r:?}");
}

#[test]
fn testar_frase_mascara_o_documento_e_nao_fala_so_de_fisco() {
    // a frase vai para a tela: documento mascarado
    for (doc, mascara) in [
        ("12345678909", "123.***.***-09"),
        ("11222333000181", "11.***.***/****-81"),
    ] {
        let pfx =
            cert_a1::gerar_teste(doc, "FULANO", "teste", 365, false, true, Utc::now()).unwrap();
        let t = cert_a1::carregar(&pfx, "teste")
            .unwrap()
            .testar(None, Utc::now(), 30);
        assert!(
            t.ok && !t.frase.contains(doc) && t.frase.contains(mascara),
            "{t:?}"
        );
        assert!(
            !t.avisos.iter().any(|a| a.contains("emissao"))
                && t.avisos.iter().any(|a| a.contains("validade juridica")),
            "{t:?}"
        );
    }
}

#[test]
fn certificado_erros_sao_frases_de_tela() {
    let pfx = cert_a1::gerar_teste(
        "11222333000181",
        "X",
        " com espaco ",
        30,
        false,
        false,
        Utc::now(),
    )
    .unwrap();
    assert_eq!(
        cert_a1::carregar(&pfx, " com espaco ").unwrap().cnpj,
        "11222333000181"
    );
    assert_eq!(
        cert_a1::testar(&pfx, "com espaco", None, Utc::now()).frase,
        "Senha do certificado incorreta."
    );
    assert!(cert_a1::testar(b"PK\x03\x04lixo", "x", None, Utc::now())
        .frase
        .contains("nao e' um certificado"));
    let id = a1("11222333000181").identidade_pem();
    let s = String::from_utf8(id.to_vec()).unwrap();
    assert!(
        s.starts_with("-----BEGIN PRIVATE KEY-----") && s.contains("-----BEGIN CERTIFICATE-----")
    );
}

/// A1 AUTOASSINADO com O=ICP-Brasil escrito a mao nao e' ICP-Brasil: `icp_brasil` fica falso.
#[test]
fn autoassinado_com_o_icp_brasil_nao_e_icp() {
    let agora = Utc::now();
    let g = a1("11222333000181");
    assert!(g.icp_brasil && g.emissor == cert_a1::AC_TESTE, "{g:?}");
    assert!(!g
        .testar(None, agora, 30)
        .avisos
        .iter()
        .any(|a| a.contains("ICP-Brasil")));
    let s = cert_a1::carregar(
        &cert_a1::gerar_teste("11222333000181", "X", "teste", 30, false, true, agora).unwrap(),
        "teste",
    )
    .unwrap();
    assert!(!s.icp_brasil && s.emissor == "X:11222333000181", "{s:?}");
}

#[test]
fn de_pem_abre_o_a1_do_cofre_igual_ao_pfx() {
    use rsa::pkcs1::EncodeRsaPrivateKey;
    let a = a1("12ABC34501DE35");
    let p = cert_a1::de_pem(std::str::from_utf8(&a.identidade_pem()).unwrap()).unwrap();
    assert_eq!(
        (
            p.cnpj.as_str(),
            p.titular.as_str(),
            p.serie.as_str(),
            p.icp_brasil
        ),
        ("12ABC34501DE35", "OFICINA TESTE", a.serie.as_str(), true)
    );
    assert!(p.cert_der == a.cert_der && p.chave == a.chave && p.validade_fim == a.validade_fim);
    let s = xmldsig::assinar(
        &dps_minima(),
        &p.chave,
        &p.cert_der,
        "infDPS",
        Algoritmo::Sha1,
        None,
    )
    .unwrap();
    assert!(xmldsig::conferido(&s));
    // PKCS#1, certificado ANTES da chave, com cadeia: o titular sai pela chave, nao pela ordem.
    let outro = a1("11222333000181");
    let pk1 = a
        .chave
        .to_pkcs1_pem(rsa::pkcs8::LineEnding::CRLF)
        .unwrap()
        .to_string();
    let txt = format!("{}{}{pk1}", outro.pem_certificados(), a.pem_certificados());
    let p = cert_a1::de_pem(&txt).unwrap();
    // cadeia_der dos dois = a AC de teste de cada um (gerar_teste com icp_brasil)
    let esperada: Vec<Vec<u8>> = std::iter::once(outro.cert_der.clone())
        .chain(outro.cadeia_der.clone())
        .chain(a.cadeia_der.clone())
        .collect();
    assert!(p.cert_der == a.cert_der && p.cadeia_der == esperada && p.cnpj == "12ABC34501DE35");
    let erro = |t: &str| cert_a1::de_pem(t).unwrap_err().0;
    assert_eq!(
        erro(&a.pem_certificados()),
        "O PEM nao traz a chave privada do certificado."
    );
    assert_eq!(
        erro(&format!("{pk1}{}", outro.pem_certificados())),
        "Nenhum certificado do PEM e' o da chave privada."
    );
    assert_eq!(erro(&pk1), "O PEM nao traz o certificado do titular.");
    assert!(erro(
        "-----BEGIN ENCRYPTED PRIVATE KEY-----\nAAAA\n-----END ENCRYPTED PRIVATE KEY-----\n"
    )
    .contains("cifrada"));
    assert_eq!(
        erro("-----BEGIN CERTIFICATE-----\n@@@\n-----END CERTIFICATE-----\n"),
        "O texto PEM do certificado esta ilegivel."
    );
}

/// RUSTSEC-2023-0071 (Marvin): o `assinar` usa `sign_with_rng` (blinding). PKCS#1 v1.5 e' deterministico, entao a
/// assinatura com blinding tem de sair IGUAL byte a byte a do `sign` sem rng que se usava antes -- senao a Sefin veria outra coisa.
#[test]
fn assinatura_com_blinding_sai_igual_a_deterministica() {
    use base64::Engine;
    use rsa::pkcs1v15::SigningKey;
    use rsa::signature::{SignatureEncoding, Signer};
    let a = a1("11222333000181");
    for sha1 in [true, false] {
        let alg = if sha1 {
            Algoritmo::Sha1
        } else {
            Algoritmo::Sha256
        };
        let s =
            xmldsig::assinar(&dps_minima(), &a.chave, &a.cert_der, "infDPS", alg, None).unwrap();
        let doc = xmldsig::ler(&s).unwrap();
        let si = xmldsig::c14n(
            xmldsig::achar(&doc, "SignedInfo").unwrap(),
            false,
            false,
            None,
        );
        let esperado = if sha1 {
            SigningKey::<sha1::Sha1>::new(a.chave.clone())
                .sign(si.as_bytes())
                .to_vec()
        } else {
            SigningKey::<sha2::Sha256>::new(a.chave.clone())
                .sign(si.as_bytes())
                .to_vec()
        };
        let (i, f) = (
            s.find("<SignatureValue>").unwrap() + 16,
            s.find("</SignatureValue>").unwrap(),
        );
        let valor: String = s[i..f].chars().filter(|c| !c.is_whitespace()).collect();
        let obtido = base64::engine::general_purpose::STANDARD
            .decode(valor)
            .unwrap();
        assert_eq!(obtido, esperado, "sha1={sha1}");
    }
}
