// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Pecas dos testes de evento e inutilizacao: as respostas montadas a mao (documentos e nomes ficticios) e a NF-e de
//! contingencia.

#![allow(dead_code, unreachable_pub)]

use serde_json::{json, Value};

use super::{nfe_de, pedido, NS};

pub const EVT: &str = "NFeRecepcaoEvento4";
pub const INU: &str = "NFeInutilizacao4";
pub const NPROT_EVT: &str = "135260000000077";

/// O `retEvento` de um evento do lote. `extra` vai depois do `nProt` (por exemplo, `chNFePend`).
pub fn ret_evento(
    chave: &str,
    tp_evento: &str,
    seq: u32,
    cstat: &str,
    motivo: &str,
    nprot: &str,
    extra: &str,
) -> String {
    let np = if nprot.is_empty() {
        String::new()
    } else {
        format!("<nProt>{nprot}</nProt>")
    };
    format!(
        "<retEvento versao=\"1.00\"><infEvento><tpAmb>2</tpAmb><verAplic>SP_EVENTOS_PL_100</verAplic>\
         <cOrgao>35</cOrgao><cStat>{cstat}</cStat><xMotivo>{motivo}</xMotivo><chNFe>{chave}</chNFe>\
         <tpEvento>{tp_evento}</tpEvento><xEvento>Evento de teste</xEvento><nSeqEvento>{seq}</nSeqEvento>\
         <dhRegEvento>2026-10-03T09:00:03-03:00</dhRegEvento>{np}{extra}</infEvento></retEvento>"
    )
}

/// O `retEnvEvento` com o `cStat` do lote e os `retEvento`.
pub fn ret_env_evento(cstat: &str, motivo: &str, retevs: &str) -> String {
    format!(
        "<retEnvEvento versao=\"1.00\" xmlns=\"{NS}\"><idLote>1</idLote><tpAmb>2</tpAmb>\
         <verAplic>SP_EVENTOS_PL_100</verAplic><cOrgao>35</cOrgao><cStat>{cstat}</cStat>\
         <xMotivo>{motivo}</xMotivo>{retevs}</retEnvEvento>"
    )
}

/// Lote processado (128) com um unico `retEvento`.
pub fn ret_lote(
    chave: &str,
    tp_evento: &str,
    seq: u32,
    cstat: &str,
    motivo: &str,
    nprot: &str,
) -> String {
    ret_env_evento(
        "128",
        "Lote de Evento Processado",
        &ret_evento(chave, tp_evento, seq, cstat, motivo, nprot, ""),
    )
}

/// O `procEventoNFe` como a consulta da nota o devolve (o evento aqui e' so' o que a leitura precisa).
pub fn proc_na_consulta(
    chave: &str,
    tp_evento: &str,
    seq: u32,
    cstat: &str,
    nprot: &str,
) -> String {
    format!(
        "<procEventoNFe versao=\"1.00\"><evento versao=\"1.00\"><infEvento Id=\"ID{tp_evento}{chave}{seq:02}\">\
         <cOrgao>35</cOrgao><tpAmb>2</tpAmb><CNPJ>11222333000181</CNPJ><chNFe>{chave}</chNFe>\
         <dhEvento>2026-10-03T09:00:00-03:00</dhEvento><tpEvento>{tp_evento}</tpEvento>\
         <nSeqEvento>{seq}</nSeqEvento><verEvento>1.00</verEvento><detEvento versao=\"1.00\"/></infEvento></evento>\
         {}</procEventoNFe>",
        ret_evento(chave, tp_evento, seq, cstat, "Evento registrado", nprot, "")
    )
}

/// O `retInutNFe`. `nprot` vazio = sem `nProt` e sem a faixa (rejeicao).
pub fn ret_inut(cstat: &str, motivo: &str, serie: u32, ini: u64, fim: u64, nprot: &str) -> String {
    let faixa = if nprot.is_empty() {
        String::new()
    } else {
        format!(
            "<ano>26</ano><CNPJ>11222333000181</CNPJ><mod>55</mod><serie>{serie}</serie>\
             <nNFIni>{ini}</nNFIni><nNFFin>{fim}</nNFFin>"
        )
    };
    let np = if nprot.is_empty() {
        String::new()
    } else {
        format!("<nProt>{nprot}</nProt>")
    };
    format!(
        "<retInutNFe versao=\"4.00\" xmlns=\"{NS}\"><infInut><tpAmb>2</tpAmb><verAplic>SP_NFE_PL_009_V4</verAplic>\
         <cStat>{cstat}</cStat><xMotivo>{motivo}</xMotivo><cUF>35</cUF>{faixa}\
         <dhRecbto>2026-10-03T09:00:03-03:00</dhRecbto>{np}</infInut></retInutNFe>"
    )
}

/// A NF-e do emitente ficticio de SP com `tpEmis` 4 (EPEC) ou 6 (SVC-AN): leva `dhCont` e `xJust`.
pub fn nfe_em_contingencia(numero: i64, tp_emis: i64) -> (String, String) {
    let mut p = pedido(numero, 2);
    p["tp_emis"] = json!(tp_emis);
    p["dh_cont"] = json!("2026-10-03T08:30:00-03:00");
    p["x_just"] = json!("Falha na comunicacao com a SEFAZ de origem");
    nfe_de(&p)
}

/// A NF-e de um emitente do Parana (UF atendida pela SVC-RS), com o `tpEmis` pedido.
pub fn nfe_do_parana(numero: i64, tp_emis: i64) -> (String, String) {
    let mut p: Value = pedido(numero, 2);
    p["emit"]["endereco"] = json!({"logradouro": "Rua das Flores", "numero": "100", "bairro": "Centro",
        "cod_municipio": "4106902", "municipio": "Curitiba", "uf": "PR", "cep": "80010000"});
    p["cmun_fg"] = json!("4106902");
    p["dest"]["endereco"] = json!({"logradouro": "Rua A", "numero": "1", "bairro": "Centro",
        "cod_municipio": "4106902", "municipio": "Curitiba", "uf": "PR"});
    p["tp_emis"] = json!(tp_emis);
    p["dh_cont"] = json!("2026-10-03T08:30:00-03:00");
    p["x_just"] = json!("Falha na comunicacao com a SEFAZ de origem");
    nfe_de(&p)
}
