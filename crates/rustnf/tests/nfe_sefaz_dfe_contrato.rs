// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O contrato do pedido `distDFeInt` e do evento da manifestacao: `tests/casos/nfe_dfe.json`. O XML esperado foi escrito a
//! parte (script de geracao em Python) e validado nos esquemas oficiais fora do repositorio; aqui a biblioteca tem de
//! escrever o MESMO texto, byte a byte, e recusar o que o contrato manda recusar.
//!
//! O ultimo teste, `escreve_os_xml_para_o_esquema`, so' faz algo quando a variavel `RUSTNF_XML_SAIDA` aponta uma pasta:
//! grava ali os XML (pedidos, `envEvento` assinados e os retornos e documentos que os testes usam) para quem queira
//! conferi-los nos esquemas XSD oficiais.

mod nfe_sefaz_comum;

use chrono::{DateTime, FixedOffset};
use nfe_sefaz_comum::dfe::*;
use nfe_sefaz_comum::eventos::{ret_lote, NPROT_EVT};
use nfe_sefaz_comum::*;
use rustnf::nfe::sefaz::dfe::pedido::{dist_dfe_int, Consulta};
use rustnf::nfe::sefaz::dfe::Nsu;
use rustnf::nfe::sefaz::evento::{manifestacao, Manifestacao, TipoManifestacao};
use rustnf::nfe::vigencias::Ambiente;
use serde_json::Value;

fn casos() -> Value {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/casos/nfe_dfe.json");
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn s(v: &Value) -> &str {
    v.as_str().unwrap()
}

fn ambiente(v: &Value) -> Ambiente {
    match v.as_u64() {
        Some(1) => Ambiente::Producao,
        _ => Ambiente::Homologacao,
    }
}

fn tipo(tp: &str) -> TipoManifestacao {
    TipoManifestacao::de_codigo(tp).unwrap()
}

fn quando(v: &Value) -> DateTime<FixedOffset> {
    DateTime::parse_from_rfc3339(s(v)).unwrap()
}

fn pedido_do_caso<'a>(k: &'a Value, chave: &'a str) -> Manifestacao<'a> {
    Manifestacao {
        documento: s(&k["documento"]),
        chave,
        tipo: tipo(s(&k["tp_evento"])),
        justificativa: k.get("justificativa").and_then(Value::as_str),
        situacao_nfe: None,
        anterior: None,
        uf_autor: s(&k["uf_autor"]),
        ambiente: ambiente(&k["ambiente"]),
        quando: quando(&k["quando"]),
    }
}

#[test]
fn os_pedidos_distdfeint_saem_como_o_contrato() {
    let c = casos();
    let chave = s(&c["chave"]);
    for k in c["pedidos"].as_array().unwrap() {
        let valor = s(&k["valor"]);
        let consulta = match s(&k["consulta"]) {
            "distNSU" => Consulta::Distribuicao(Nsu::ler(valor).unwrap()),
            "consNSU" => Consulta::Nsu(Nsu::ler(valor).unwrap()),
            _ => Consulta::Chave(chave),
        };
        let xml = dist_dfe_int(
            ambiente(&k["ambiente"]),
            s(&k["uf"]),
            s(&k["documento"]),
            consulta,
        )
        .unwrap();
        assert_eq!(xml, s(&k["xml"]), "{}", s(&k["nome"]));
    }
}

#[test]
fn os_eventos_da_manifestacao_saem_como_o_contrato() {
    let c = casos();
    for k in c["manifestacoes"].as_array().unwrap() {
        let m = manifestacao(&pedido_do_caso(k, s(&k["chave"]))).unwrap();
        assert_eq!(m.xml, s(&k["xml"]), "{}", s(&k["nome"]));
        assert_eq!(m.seq, 1);
    }
}

#[test]
fn as_recusas_do_contrato() {
    let c = casos();
    let chave = s(&c["chave"]);
    for k in c["recusas"].as_array().unwrap() {
        let mut caso = k.clone();
        caso["documento"] = Value::from("11222333000181");
        caso["uf_autor"] = Value::from("SP");
        caso["ambiente"] = Value::from(2);
        caso["quando"] = Value::from("2026-11-26T09:00:00-03:00");
        let e = manifestacao(&pedido_do_caso(&caso, chave)).unwrap_err();
        assert_eq!(e.campo, s(&k["campo"]), "{}", s(&k["nome"]));
        assert!(
            e.mensagem.contains(s(&k["contem"])),
            "{}: {e}",
            s(&k["nome"])
        );
    }
}

#[test]
fn escreve_os_xml_para_o_esquema() {
    let Ok(pasta) = std::env::var("RUSTNF_XML_SAIDA") else {
        return;
    };
    let pasta = std::path::PathBuf::from(pasta);
    std::fs::create_dir_all(&pasta).unwrap();
    let grava = |nome: &str, xml: &str| std::fs::write(pasta.join(nome), xml).unwrap();
    let c = casos();
    let chave = s(&c["chave"]);
    // os pedidos do contrato e os eventos, assinados e embrulhados no envEvento
    for (i, k) in c["pedidos"].as_array().unwrap().iter().enumerate() {
        grava(&format!("distDFeInt_{i}.xml"), s(&k["xml"]));
    }
    let cert = certificado();
    for (i, k) in c["manifestacoes"].as_array().unwrap().iter().enumerate() {
        let m = manifestacao(&pedido_do_caso(k, chave)).unwrap();
        let tp = s(&k["tp_evento"]);
        let env = m.assinar(&cert, "1").unwrap().env_evento;
        grava(&format!("envEvento_{tp}_{i}.xml"), &env);
    }
    // os documentos e o retorno que os testes usam
    let (proc, chave_nfe) = proc_nfe(1);
    grava("resNFe.xml", &res_nfe(&chave_nfe, 1));
    grava("resEvento.xml", &res_evento(&chave_nfe, "210210", 1));
    grava("procNFe.xml", &proc);
    let docs = [
        doc_zip(1, "resNFe_v1.01.xsd", &res_nfe(&chave_nfe, 1)),
        doc_zip(
            2,
            "resEvento_v1.01.xsd",
            &res_evento(&chave_nfe, "210210", 1),
        ),
    ];
    grava(
        "retDistDFeInt_138.xml",
        &ret_dist("138", "Documento(s) localizado(s)", 2, 9, &docs),
    );
    grava(
        "retDistDFeInt_137.xml",
        &ret_dist("137", "Nenhum documento localizado", 9, 9, &[]),
    );
    grava(
        "retDistDFeInt_656.xml",
        &ret_dist("656", "Rejeicao: Consumo Indevido", 0, 0, &[]),
    );
    // o procEventoNFe da manifestacao registrada, como o cliente o guarda
    let f = Falso::novo(vec![(
        "NFeRecepcaoEvento4",
        ok(
            "NFeRecepcaoEvento4",
            &ret_lote(
                &chave_nfe,
                "210210",
                1,
                "135",
                "Evento registrado e vinculado a NF-e",
                NPROT_EVT,
            ),
        ),
    )]);
    let (cli, _) = cliente(f);
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let pedido = Manifestacao {
        documento: CNPJ,
        chave: &chave_nfe,
        tipo: TipoManifestacao::Ciencia,
        justificativa: None,
        situacao_nfe: None,
        anterior: None,
        uf_autor: "SP",
        ambiente: Ambiente::Homologacao,
        quando: quando(&Value::from("2026-11-26T09:00:00-03:00")),
    };
    let r = rt.block_on(cli.manifestar(&cert, &pedido)).unwrap();
    grava(
        "procEventoNFe_210210.xml",
        r.proc_evento.as_deref().unwrap(),
    );
}
