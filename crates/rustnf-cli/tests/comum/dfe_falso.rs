// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Pecas dos testes dos documentos recebidos: o retorno da distribuicao de DF-e montado a mao, com documentos e nomes
//! ficticios (gzip e base64 gerados aqui). Nenhum teste chama o Ambiente Nacional.

use std::io::Write;

use base64::Engine;
use flate2::{write::GzEncoder, Compression};
use rustnf::nfe::sefaz::resposta::nfe_proc;
use rustnf_app::config::Config as ConfigApp;
use rustnf_app::nfe::montar_pedido;

use super::nfe_falso::{pedido_nfe, prot, Resposta, NPROT, NS};

/// O nome do servico no roteiro do transporte falso.
pub const DIST: &str = "NFeDistribuicaoDFe";

/// Um `docZip` com o XML compactado.
pub fn doc_zip(nsu: u64, esquema: &str, xml: &str) -> String {
    let mut e = GzEncoder::new(Vec::new(), Compression::default());
    e.write_all(xml.as_bytes()).unwrap();
    let b64 = base64::engine::general_purpose::STANDARD.encode(e.finish().unwrap());
    format!("<docZip NSU=\"{nsu:015}\" schema=\"{esquema}\">{b64}</docZip>")
}

fn resposta(
    cstat: &'static str,
    motivo: &'static str,
    ult: u64,
    max: u64,
    docs: Vec<String>,
) -> Resposta {
    Box::new(move |_| {
        let lote = if docs.is_empty() {
            String::new()
        } else {
            format!("<loteDistDFeInt>{}</loteDistDFeInt>", docs.concat())
        };
        let ret = format!(
            "<retDistDFeInt xmlns=\"{NS}\" versao=\"1.01\"><tpAmb>2</tpAmb><verAplic>1.4.0</verAplic><cStat>{cstat}</cStat>\
             <xMotivo>{motivo}</xMotivo><dhResp>2026-10-03T09:00:00-03:00</dhResp><ultNSU>{ult:015}</ultNSU>\
             <maxNSU>{max:015}</maxNSU>{lote}</retDistDFeInt>"
        );
        Ok((
            200,
            format!(
                "<?xml version=\"1.0\" encoding=\"utf-8\"?><soap:Envelope xmlns:soap=\"http://www.w3.org/2003/05/soap-envelope\">\
                 <soap:Body><nfeDistDFeInteresseResponse xmlns=\"{NS}/wsdl/{DIST}\">\
                 <nfeDistDFeInteresseResult>{ret}</nfeDistDFeInteresseResult></nfeDistDFeInteresseResponse>\
                 </soap:Body></soap:Envelope>"
            )
            .into_bytes(),
        ))
    })
}

/// 138 com os documentos dados.
pub fn lote(ult: u64, max: u64, docs: &[String]) -> Resposta {
    resposta("138", "Documento(s) localizado(s)", ult, max, docs.to_vec())
}

/// 137, fila em dia.
pub fn nada(ult: u64) -> Resposta {
    resposta("137", "Nenhum documento localizado", ult, ult, Vec::new())
}

/// 656, consumo indevido.
pub fn bloqueio(ult: u64) -> Resposta {
    resposta("656", "Consumo Indevido", ult, ult, Vec::new())
}

/// A NF-e completa (`procNFe`) de numero `numero` e a chave dela.
pub fn nota_recebida(config: &ConfigApp, numero: u64) -> (String, String) {
    let mut p = pedido_nfe();
    p["numero"] = numero.into();
    let m = montar_pedido(config, None, p, true).unwrap();
    let proc = nfe_proc(
        &m.xml,
        &prot(&m.chave, "100", "Autorizado o uso da NF-e", NPROT),
    );
    (proc, m.chave)
}

/// O `docZip` do resumo da NF-e (`resNFe`).
pub fn doc_resumo(nsu: u64, chave: &str) -> String {
    let xml = format!(
        "<resNFe xmlns=\"{NS}\" versao=\"1.01\"><chNFe>{chave}</chNFe><CNPJ>11222333000181</CNPJ>\
         <xNome>LOJA DE TESTE LTDA</xNome><IE>111111111111</IE><dhEmi>2026-10-03T09:00:00-03:00</dhEmi>\
         <tpNF>1</tpNF><vNF>31.80</vNF><digVal>qZk+NkcGgWq6PiVxeFDCbJzQ2J0=</digVal>\
         <dhRecbto>2026-10-03T09:00:05-03:00</dhRecbto><nProt>{NPROT}</nProt><cSitNFe>1</cSitNFe></resNFe>"
    );
    doc_zip(nsu, "resNFe_v1.01.xsd", &xml)
}

/// O `docZip` da NF-e completa.
pub fn doc_completa(nsu: u64, proc: &str) -> String {
    doc_zip(nsu, "procNFe_v4.00.xsd", proc)
}
