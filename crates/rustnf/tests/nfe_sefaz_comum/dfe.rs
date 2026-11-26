// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Pecas dos testes da distribuicao de DF-e: o retorno do Ambiente Nacional montado a mao (documentos ficticios, gzip e
//! base64 gerados aqui) e os documentos que ele distribui.

#![allow(dead_code, unreachable_pub)]

use std::io::Write;
use std::sync::{Arc, Mutex};

use base64::Engine;
use chrono::{DateTime, TimeZone, Utc};
use flate2::{write::GzEncoder, Compression};
use rustnf::nfe::sefaz::resposta::nfe_proc;
use rustnf::nfe::sefaz::{ClienteSefaz, Config};
use rustnf::nfe::vigencias::Ambiente::Homologacao;

use super::eventos::proc_na_consulta;
use super::{nfe, prot_ok, Falso, Resp, NS};

/// O espaco de nomes do WSDL (e o nome do servico no roteiro do transporte falso).
pub const DIST: &str = "NFeDistribuicaoDFe";
/// O CNPJ ficticio do interessado (destinatario).
pub const CNPJ: &str = "11222333000181";
/// O CPF ficticio de um interessado pessoa fisica.
pub const CPF: &str = "52998224725";
pub const URL_AN_HOMOLOGACAO: &str =
    "https://hom1.nfe.fazenda.gov.br/NFeDistribuicaoDFe/NFeDistribuicaoDFe.asmx";

pub fn gz(dados: &[u8]) -> Vec<u8> {
    let mut e = GzEncoder::new(Vec::new(), Compression::default());
    e.write_all(dados).unwrap();
    e.finish().unwrap()
}

pub fn gz_b64(xml: &str) -> String {
    base64::engine::general_purpose::STANDARD.encode(gz(xml.as_bytes()))
}

/// Um `docZip` com o XML compactado.
pub fn doc_zip(nsu: u64, esquema: &str, xml: &str) -> String {
    format!(
        "<docZip NSU=\"{nsu:015}\" schema=\"{esquema}\">{}</docZip>",
        gz_b64(xml)
    )
}

/// Um `docZip` com o conteudo que o teste dita (base64 ja pronto).
pub fn doc_zip_bruto(nsu: u64, esquema: &str, base64: &str) -> String {
    format!("<docZip NSU=\"{nsu:015}\" schema=\"{esquema}\">{base64}</docZip>")
}

/// O `retDistDFeInt`. `docs` sao `docZip` ja montados.
pub fn ret_dist(cstat: &str, motivo: &str, ult: u64, max: u64, docs: &[String]) -> String {
    let lote = if docs.is_empty() {
        String::new()
    } else {
        format!("<loteDistDFeInt>{}</loteDistDFeInt>", docs.concat())
    };
    format!(
        "<retDistDFeInt xmlns=\"{NS}\" versao=\"1.01\"><tpAmb>2</tpAmb><verAplic>1.4.0</verAplic><cStat>{cstat}</cStat>\
         <xMotivo>{motivo}</xMotivo><dhResp>2026-11-26T09:00:00-03:00</dhResp><ultNSU>{ult:015}</ultNSU>\
         <maxNSU>{max:015}</maxNSU>{lote}</retDistDFeInt>"
    )
}

/// O `Body` SOAP 1.2 como o Ambiente Nacional o devolve: o retorno dentro de `nfeDistDFeInteresseResult`.
pub fn ok_dist(ret: &str) -> Resp {
    Resp::R(
        200,
        format!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?><soap:Envelope xmlns:soap=\"http://www.w3.org/2003/05/soap-envelope\">\
             <soap:Body><nfeDistDFeInteresseResponse xmlns=\"http://www.portalfiscal.inf.br/nfe/wsdl/{DIST}\">\
             <nfeDistDFeInteresseResult>{ret}</nfeDistDFeInteresseResult></nfeDistDFeInteresseResponse>\
             </soap:Body></soap:Envelope>"
        )
        .into_bytes(),
    )
}

/// 138 com os documentos dados.
pub fn lote(ult: u64, max: u64, docs: &[String]) -> Resp {
    ok_dist(&ret_dist(
        "138",
        "Documento(s) localizado(s)",
        ult,
        max,
        docs,
    ))
}

/// 137, fila em dia (`ultNSU` = `maxNSU`).
pub fn nada(ult: u64) -> Resp {
    ok_dist(&ret_dist(
        "137",
        "Nenhum documento localizado",
        ult,
        ult,
        &[],
    ))
}

/// `resNFe` do emitente ficticio. `situacao`: `cSitNFe`.
pub fn res_nfe(chave: &str, situacao: u8) -> String {
    format!(
        "<resNFe xmlns=\"{NS}\" versao=\"1.01\"><chNFe>{chave}</chNFe><CNPJ>{CNPJ}</CNPJ><xNome>LOJA DE TESTE LTDA</xNome>\
         <IE>111111111111</IE><dhEmi>2026-10-03T09:00:00-03:00</dhEmi><tpNF>1</tpNF><vNF>31.80</vNF>\
         <digVal>qZk+NkcGgWq6PiVxeFDCbJzQ2J0=</digVal><dhRecbto>2026-10-03T09:00:05-03:00</dhRecbto>\
         <nProt>135260000000001</nProt><cSitNFe>{situacao}</cSitNFe></resNFe>"
    )
}

/// `resEvento` de um evento do emitente ficticio.
pub fn res_evento(chave: &str, tp_evento: &str, seq: u32) -> String {
    format!(
        "<resEvento xmlns=\"{NS}\" versao=\"1.01\"><cOrgao>91</cOrgao><CNPJ>{CNPJ}</CNPJ><chNFe>{chave}</chNFe>\
         <dhEvento>2026-10-03T10:00:00-03:00</dhEvento><tpEvento>{tp_evento}</tpEvento><nSeqEvento>{seq}</nSeqEvento>\
         <xEvento>Evento de teste</xEvento><dhRecbto>2026-10-03T10:00:03-03:00</dhRecbto><nProt>135260000000077</nProt>\
         </resEvento>"
    )
}

/// O `procNFe` (a NF-e assinada pela montagem com o protocolo de autorizacao) e a chave.
pub fn proc_nfe(numero: i64) -> (String, String) {
    let (xml, chave) = nfe(numero, 2);
    (nfe_proc(&xml, &prot_ok(&chave)), chave)
}

/// O `procEventoNFe` de um cancelamento registrado.
pub fn proc_evento(chave: &str) -> String {
    proc_na_consulta(chave, "110111", 1, "135", "135260000000077").replacen(
        "<procEventoNFe ",
        &format!("<procEventoNFe xmlns=\"{NS}\" "),
        1,
    )
}

/// O relogio do teste: 26/11/2026, 12:00 UTC (09:00 em Brasilia); o teste o adianta.
pub type Relogio = Arc<Mutex<DateTime<Utc>>>;

pub fn meio_dia() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 26, 12, 0, 0).unwrap()
}

/// Cliente de SP, em homologacao, com o relogio nas maos do teste.
pub fn cliente_dist(f: Falso) -> (ClienteSefaz<Falso>, Relogio) {
    let relogio: Relogio = Arc::new(Mutex::new(meio_dia()));
    let r2 = relogio.clone();
    let c = ClienteSefaz::com_transporte(Config::novo("SP", Homologacao), f)
        .unwrap()
        .com_relogio(move || *r2.lock().unwrap());
    (c, relogio)
}
