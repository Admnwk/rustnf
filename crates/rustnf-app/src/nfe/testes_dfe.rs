// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Pecas dos testes dos documentos recebidos: o retorno da distribuicao montado a mao, com documentos e nomes ficticios
//! (gzip e base64 gerados aqui).

use std::io::Write;

use base64::Engine;
use flate2::{write::GzEncoder, Compression};
use rustnf::nfe::sefaz::resposta::nfe_proc;

use super::montar_pedido;
use super::testes_comuns::*;
use crate::config::Config;

/// O nome do servico no roteiro do transporte falso.
pub(crate) const DIST: &str = "NFeDistribuicaoDFe";

fn gz_b64(xml: &str) -> String {
    let mut e = GzEncoder::new(Vec::new(), Compression::default());
    e.write_all(xml.as_bytes()).unwrap();
    base64::engine::general_purpose::STANDARD.encode(e.finish().unwrap())
}

/// Um `docZip` com o XML compactado.
pub(crate) fn doc_zip(nsu: u64, esquema: &str, xml: &str) -> String {
    format!(
        "<docZip NSU=\"{nsu:015}\" schema=\"{esquema}\">{}</docZip>",
        gz_b64(xml)
    )
}

/// Um `docZip` com o conteudo que o teste dita (base64 ja pronto, que pode nao ser gzip).
pub(crate) fn doc_zip_bruto(nsu: u64, esquema: &str, base64: &str) -> String {
    format!("<docZip NSU=\"{nsu:015}\" schema=\"{esquema}\">{base64}</docZip>")
}

fn ret_dist(cstat: &str, motivo: &str, ult: u64, max: u64, docs: &[String]) -> String {
    let lote = if docs.is_empty() {
        String::new()
    } else {
        format!("<loteDistDFeInt>{}</loteDistDFeInt>", docs.concat())
    };
    format!(
        "<retDistDFeInt xmlns=\"{NS}\" versao=\"1.01\"><tpAmb>2</tpAmb><verAplic>1.4.0</verAplic><cStat>{cstat}</cStat>\
         <xMotivo>{motivo}</xMotivo><dhResp>2026-10-03T09:00:00-03:00</dhResp><ultNSU>{ult:015}</ultNSU>\
         <maxNSU>{max:015}</maxNSU>{lote}</retDistDFeInt>"
    )
}

fn envelope(ret: &str) -> Resp {
    Resp::R(
        200,
        format!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?><soap:Envelope xmlns:soap=\"http://www.w3.org/2003/05/soap-envelope\">\
             <soap:Body><nfeDistDFeInteresseResponse xmlns=\"{NS}/wsdl/{DIST}\">\
             <nfeDistDFeInteresseResult>{ret}</nfeDistDFeInteresseResult></nfeDistDFeInteresseResponse>\
             </soap:Body></soap:Envelope>"
        )
        .into_bytes(),
    )
}

/// 138 com os documentos dados.
pub(crate) fn lote(ult: u64, max: u64, docs: &[String]) -> Resp {
    envelope(&ret_dist(
        "138",
        "Documento(s) localizado(s)",
        ult,
        max,
        docs,
    ))
}

/// 137, fila em dia.
pub(crate) fn nada(ult: u64) -> Resp {
    envelope(&ret_dist(
        "137",
        "Nenhum documento localizado",
        ult,
        ult,
        &[],
    ))
}

/// 656, consumo indevido.
pub(crate) fn bloqueio(ult: u64) -> Resp {
    envelope(&ret_dist("656", "Consumo Indevido", ult, ult, &[]))
}

/// A NF-e completa (`procNFe`) de numero `numero`, montada pelo pedido de teste, e a chave dela.
pub(crate) fn proc_nfe(config: &Config, numero: u64) -> (String, String) {
    let mut p = pedido();
    p["numero"] = numero.into();
    let m = montar_pedido(config, None, p, true).unwrap();
    (
        nfe_proc(
            &m.xml,
            &prot(&m.chave, "100", "Autorizado o uso da NF-e", NPROT),
        ),
        m.chave,
    )
}

/// O `resNFe` da chave. `situacao`: `cSitNFe`.
pub(crate) fn res_nfe(chave: &str, situacao: u8) -> String {
    format!(
        "<resNFe xmlns=\"{NS}\" versao=\"1.01\"><chNFe>{chave}</chNFe><CNPJ>11222333000181</CNPJ>\
         <xNome>LOJA DE TESTE LTDA</xNome><IE>111111111111</IE><dhEmi>2026-10-03T09:00:00-03:00</dhEmi>\
         <tpNF>1</tpNF><vNF>31.80</vNF><digVal>qZk+NkcGgWq6PiVxeFDCbJzQ2J0=</digVal>\
         <dhRecbto>2026-10-03T09:00:05-03:00</dhRecbto><nProt>{NPROT}</nProt><cSitNFe>{situacao}</cSitNFe></resNFe>"
    )
}

/// O `resEvento` de um evento da NF-e.
pub(crate) fn res_evento(chave: &str, tp_evento: &str, seq: u32) -> String {
    format!(
        "<resEvento xmlns=\"{NS}\" versao=\"1.01\"><cOrgao>91</cOrgao><CNPJ>11222333000181</CNPJ><chNFe>{chave}</chNFe>\
         <dhEvento>2026-10-03T10:00:00-03:00</dhEvento><tpEvento>{tp_evento}</tpEvento><nSeqEvento>{seq}</nSeqEvento>\
         <xEvento>Evento de teste</xEvento><dhRecbto>2026-10-03T10:00:03-03:00</dhRecbto><nProt>135260000000077</nProt>\
         </resEvento>"
    )
}

/// O `procEventoNFe` de um evento registrado.
pub(crate) fn proc_evento(chave: &str, tp_evento: &str, seq: u32) -> String {
    format!(
        "<procEventoNFe xmlns=\"{NS}\" versao=\"1.00\"><evento versao=\"1.00\"><infEvento Id=\"ID{tp_evento}{chave}{seq:02}\">\
         <cOrgao>91</cOrgao><tpAmb>2</tpAmb><CNPJ>11222333000181</CNPJ><chNFe>{chave}</chNFe>\
         <dhEvento>2026-10-03T10:00:00-03:00</dhEvento><tpEvento>{tp_evento}</tpEvento>\
         <nSeqEvento>{seq}</nSeqEvento><verEvento>1.00</verEvento><detEvento versao=\"1.00\"/></infEvento></evento>\
         <retEvento versao=\"1.00\"><infEvento><tpAmb>2</tpAmb><verAplic>AN</verAplic><cOrgao>91</cOrgao><cStat>135</cStat>\
         <xMotivo>Evento registrado e vinculado a NF-e</xMotivo><chNFe>{chave}</chNFe><tpEvento>{tp_evento}</tpEvento>\
         <xEvento>Evento de teste</xEvento><nSeqEvento>{seq}</nSeqEvento><dhRegEvento>2026-10-03T10:00:03-03:00</dhRegEvento>\
         <nProt>135260000000077</nProt></infEvento></retEvento></procEventoNFe>"
    )
}

/// O `docZip` do resumo (`resNFe_v1.01.xsd`).
pub(crate) fn doc_resumo(nsu: u64, chave: &str) -> String {
    doc_zip(nsu, "resNFe_v1.01.xsd", &res_nfe(chave, 1))
}

/// O `docZip` da NF-e completa.
pub(crate) fn doc_completa(nsu: u64, proc: &str) -> String {
    doc_zip(nsu, "procNFe_v4.00.xsd", proc)
}
