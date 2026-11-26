// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Leitura da NF-e (modelo 55): `tests/casos/nfe_leitura.json`, com XML ficticios. A leitura NAO confere assinatura
//! nem `digVal`; um dos casos carrega um grupo `Signature` com digest falso e tem de ler do mesmo jeito.

use rustnf::nfe::{conferir, ler, ErroLeitura, Situacao};
use serde_json::{to_value, Value};

fn casos() -> Value {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/casos/nfe_leitura.json");
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn caso<'a>(c: &'a Value, nome: &str) -> &'a Value {
    c["ler"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["nome"] == nome)
        .unwrap()
}

#[test]
fn contrato_ler() {
    let c = casos();
    let todos = c["ler"].as_array().unwrap();
    assert!(todos.len() >= 20);
    for x in todos {
        let nome = x["nome"].as_str().unwrap();
        let nfe =
            ler(x["xml"].as_str().unwrap().as_bytes()).unwrap_or_else(|e| panic!("{nome}: {e}"));
        let json = to_value(&nfe).unwrap();
        for (ponteiro, esperado) in x["esperado"].as_object().unwrap() {
            let achado = json.pointer(ponteiro).unwrap_or(&Value::Null);
            assert_eq!(achado, esperado, "{nome}: {ponteiro}");
        }
        assert_eq!(
            nfe.situacao().como_texto(),
            x["situacao"],
            "{nome}: situacao"
        );
        assert_eq!(
            to_value(conferir(&nfe)).unwrap(),
            x["conferir"],
            "{nome}: conferir"
        );
        if let Some(a) = x.get("autorizada") {
            assert_eq!(nfe.autorizada(), a.as_bool().unwrap(), "{nome}: autorizada");
        }
        if let Some(cst) = x.get("cst_icms") {
            let achados: Vec<String> = nfe.itens.iter().map(|i| i.imposto.cst_icms()).collect();
            assert_eq!(to_value(achados).unwrap(), *cst, "{nome}: cst_icms");
        }
    }
}

#[test]
fn contrato_erros() {
    let c = casos();
    let erros = c["erros"].as_array().unwrap();
    assert!(erros.len() >= 24);
    for x in erros {
        let nome = x["nome"].as_str().unwrap();
        let e = ler(x["xml"].as_str().unwrap().as_bytes()).unwrap_err();
        assert_eq!(e.to_string(), x["erro"].as_str().unwrap(), "{nome}");
    }
}

#[test]
fn o_erro_e_tipado() {
    let c = casos();
    let por_nome = |n: &str| {
        let x = c["erros"]
            .as_array()
            .unwrap()
            .iter()
            .find(|x| x["nome"] == n)
            .unwrap();
        ler(x["xml"].as_str().unwrap().as_bytes()).unwrap_err()
    };
    assert_eq!(por_nome("dtd_vazio"), ErroLeitura::Dtd);
    assert_eq!(por_nome("mal_formado"), ErroLeitura::MalFormado);
    assert_eq!(por_nome("leiaute_200"), ErroLeitura::Leiaute("2.00".into()));
    assert_eq!(por_nome("modelo_57"), ErroLeitura::Modelo("57".into()));
    assert_eq!(por_nome("item_sem_prod"), ErroLeitura::ItemSemProd(1));
}

#[test]
fn situacao_so_vem_do_protocolo() {
    let c = casos();
    let situacao = |nome: &str| {
        ler(caso(&c, nome)["xml"].as_str().unwrap().as_bytes())
            .unwrap()
            .situacao()
    };
    assert_eq!(situacao("proc_autorizada"), Situacao::Autorizada);
    assert_eq!(
        situacao("autorizada_fora_do_prazo_150"),
        Situacao::Autorizada
    );
    assert_eq!(situacao("nfe_solta_sem_protocolo"), Situacao::SemProtocolo);
    assert_eq!(situacao("denegada_110"), Situacao::Denegada);
    assert_eq!(situacao("denegada_301"), Situacao::Denegada);
    assert_eq!(situacao("cancelada_101"), Situacao::Cancelada);
    assert_eq!(situacao("rejeitada_539"), Situacao::Rejeitada);
    assert_eq!(
        situacao("protocolo_de_outra_chave"),
        Situacao::ProtocoloDivergente
    );
}

fn bytes_utf16(texto: &str, little: bool) -> Vec<u8> {
    let mut b = if little {
        vec![0xff, 0xfe]
    } else {
        vec![0xfe, 0xff]
    };
    for u in texto.encode_utf16() {
        b.extend_from_slice(&if little {
            u.to_le_bytes()
        } else {
            u.to_be_bytes()
        });
    }
    b
}

#[test]
fn codificacoes_do_arquivo() {
    let c = casos();
    let xml = caso(&c, "proc_autorizada")["xml"]
        .as_str()
        .unwrap()
        .replace("Sao Paulo", "S\u{e3}o Paulo");
    let esperado = ler(xml.as_bytes()).unwrap();
    assert_eq!(
        esperado.emit.endereco.as_ref().unwrap().municipio,
        "S\u{e3}o Paulo"
    );
    assert_eq!(
        ler(&bytes_utf16(&xml, true)).unwrap(),
        esperado,
        "UTF-16 LE com BOM"
    );
    assert_eq!(
        ler(&bytes_utf16(&xml, false)).unwrap(),
        esperado,
        "UTF-16 BE com BOM"
    );
    let mut com_bom = vec![0xef, 0xbb, 0xbf];
    com_bom.extend_from_slice(xml.as_bytes());
    assert_eq!(ler(&com_bom).unwrap(), esperado, "UTF-8 com BOM");
    let latin1: Vec<u8> = xml
        .replace("UTF-8", "ISO-8859-1")
        .chars()
        .map(|ch| u8::try_from(u32::from(ch)).unwrap())
        .collect();
    assert_eq!(ler(&latin1).unwrap(), esperado, "ISO-8859-1 declarado");
    let erro = ler(b"<?xml version=\"1.0\" encoding=\"koi8-r\"?><NFe/>").unwrap_err();
    assert_eq!(erro, ErroLeitura::Codificacao("koi8-r".into()));
}

#[test]
fn arquivo_grande_demais_e_recusado() {
    let grande = vec![b' '; rustnf::nfe::LIMITE_BYTES + 1];
    assert_eq!(ler(&grande).unwrap_err(), ErroLeitura::Grande);
}

#[test]
fn a_leitura_nao_confere_a_assinatura() {
    // o digest do Signature do caso e' falso e a leitura segue: quem precisa de prova confere a assinatura a parte
    let c = casos();
    let x = caso(&c, "proc_com_assinatura_que_nao_e_conferida");
    let nfe = ler(x["xml"].as_str().unwrap().as_bytes()).unwrap();
    assert!(nfe.autorizada() && nfe.chave_valida());
}
