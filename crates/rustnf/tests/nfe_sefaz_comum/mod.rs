// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Pecas comuns dos testes do cliente da SEFAZ: certificado de teste, NF-e assinada pela montagem, transporte falso
//! e as respostas montadas a mao (documentos e nomes ficticios).

#![allow(dead_code, unreachable_pub)]

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::Utc;
use rustnf::assinatura::cert_a1::{self, Certificado};
use rustnf::nfe::montagem::{assinar, montar, nota_de_json};
use rustnf::nfe::sefaz::{FalhaRede, Transporte};
use serde_json::{json, Value};

pub const NS: &str = "http://www.portalfiscal.inf.br/nfe";

pub fn certificado() -> Certificado {
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

/// O pedido da NF-e do emitente ficticio (SP), numero `numero`, ambiente `tp_amb`.
pub fn pedido(numero: i64, tp_amb: i64) -> Value {
    json!({
        "modelo": 55, "serie": 1, "numero": numero, "emissao": "2026-10-03T09:00:00-03:00",
        "tp_amb": tp_amb,
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
    })
}

/// A NF-e assinada pela montagem (como ela sai) e a chave.
pub fn nfe(numero: i64, tp_amb: i64) -> (String, String) {
    nfe_de(&pedido(numero, tp_amb))
}

pub fn nfe_de(p: &Value) -> (String, String) {
    let m = montar(&nota_de_json(p).unwrap()).unwrap();
    let xml = assinar(&m, &certificado()).unwrap();
    (xml, m.chave)
}

// ------------------------------------------------------------------------------------------------ transporte falso

#[derive(Clone)]
pub enum Resp {
    /// HTTP e corpo.
    R(u16, Vec<u8>),
    /// Sem resposta.
    F(FalhaRede),
}

/// Uma chamada que o cliente fez.
#[derive(Clone, Debug)]
pub struct Chamada {
    pub url: String,
    pub corpo: String,
    pub content_type: String,
}

/// Transporte com roteiro: cada chamada tira a proxima resposta; o roteiro diz que servico se espera (pelo espaco de
/// nomes do WSDL no corpo) e a chamada que nao bate derruba o teste.
#[derive(Clone, Default)]
pub struct Falso {
    roteiro: Arc<Mutex<VecDeque<(&'static str, Resp)>>>,
    chamadas: Arc<Mutex<Vec<Chamada>>>,
}

impl Falso {
    pub fn novo(r: Vec<(&'static str, Resp)>) -> Self {
        Falso {
            roteiro: Arc::new(Mutex::new(r.into())),
            ..Default::default()
        }
    }

    pub fn chamadas(&self) -> Vec<Chamada> {
        self.chamadas.lock().unwrap().clone()
    }

    pub fn restantes(&self) -> usize {
        self.roteiro.lock().unwrap().len()
    }
}

#[async_trait]
impl Transporte for Falso {
    async fn enviar(
        &self,
        url: &str,
        corpo: Vec<u8>,
        content_type: &str,
    ) -> Result<(u16, Vec<u8>), FalhaRede> {
        let texto = String::from_utf8(corpo).unwrap();
        self.chamadas.lock().unwrap().push(Chamada {
            url: url.to_string(),
            corpo: texto.clone(),
            content_type: content_type.to_string(),
        });
        let (espera, resp) = self
            .roteiro
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| panic!("chamada fora do roteiro: {url}"));
        assert!(
            texto.contains(&format!("wsdl/{espera}\"")),
            "o roteiro esperava {espera}, veio {url}\n{texto}"
        );
        match resp {
            Resp::R(st, b) => Ok((st, b)),
            Resp::F(f) => Err(f),
        }
    }
}

// ------------------------------------------------------------------------------------------------ respostas

pub const AUT: &str = "NFeAutorizacao4";
pub const RET: &str = "NFeRetAutorizacao4";
pub const CON: &str = "NFeConsultaProtocolo4";
pub const STA: &str = "NFeStatusServico4";

/// O `Body` SOAP 1.2 com o retorno dentro do `nfeResultMsg`.
pub fn soap(ns_wsdl: &str, ret: &str) -> Vec<u8> {
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?><soap:Envelope xmlns:soap=\"http://www.w3.org/2003/05/soap-envelope\" \
         xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"><soap:Body>\
         <nfeResultMsg xmlns=\"http://www.portalfiscal.inf.br/nfe/wsdl/{ns_wsdl}\">{ret}</nfeResultMsg>\
         </soap:Body></soap:Envelope>"
    )
    .into_bytes()
}

pub fn ok(ns_wsdl: &str, ret: &str) -> Resp {
    Resp::R(200, soap(ns_wsdl, ret))
}

pub fn fault(motivo: &str) -> Resp {
    Resp::R(
        500,
        format!(
            "<soap:Envelope xmlns:soap=\"http://www.w3.org/2003/05/soap-envelope\"><soap:Body><soap:Fault>\
             <soap:Code><soap:Value>soap:Receiver</soap:Value></soap:Code>\
             <soap:Reason><soap:Text xml:lang=\"pt\">{motivo}</soap:Text></soap:Reason></soap:Fault></soap:Body>\
             </soap:Envelope>"
        )
        .into_bytes(),
    )
}

pub fn tempo() -> Resp {
    Resp::F(FalhaRede::Tempo)
}

pub const NPROT: &str = "135260000000001";

/// `protNFe` de teste. `nprot` vazio = sem `nProt` (rejeicao).
pub fn prot(chave: &str, cstat: &str, motivo: &str, nprot: &str) -> String {
    let np = if nprot.is_empty() {
        String::new()
    } else {
        format!("<nProt>{nprot}</nProt>")
    };
    let dg = if nprot.is_empty() {
        ""
    } else {
        "<digVal>qZk+NkcGgWq6PiVxeFDCbJzQ2J0=</digVal>"
    };
    format!(
        "<protNFe versao=\"4.00\"><infProt><tpAmb>2</tpAmb><verAplic>SP_NFE_PL_009_V4</verAplic>\
         <chNFe>{chave}</chNFe><dhRecbto>2026-10-03T09:00:05-03:00</dhRecbto>{np}{dg}\
         <cStat>{cstat}</cStat><xMotivo>{motivo}</xMotivo></infProt></protNFe>"
    )
}

pub fn prot_ok(chave: &str) -> String {
    prot(chave, "100", "Autorizado o uso da NF-e", NPROT)
}

/// `retEnviNFe` com cStat do lote, e o miolo (protNFe ou infRec).
pub fn ret_envi(cstat: &str, motivo: &str, miolo: &str) -> String {
    format!(
        "<retEnviNFe versao=\"4.00\" xmlns=\"{NS}\"><tpAmb>2</tpAmb><verAplic>SP_NFE_PL_009_V4</verAplic>\
         <cStat>{cstat}</cStat><xMotivo>{motivo}</xMotivo><cUF>35</cUF><dhRecbto>2026-10-03T09:00:05-03:00</dhRecbto>\
         {miolo}</retEnviNFe>"
    )
}

pub fn ret_envi_sinc(p: &str) -> String {
    ret_envi("104", "Lote processado", p)
}

pub fn ret_envi_recibo(recibo: &str, tmed: u32) -> String {
    ret_envi(
        "103",
        "Lote recebido com sucesso",
        &format!("<infRec><nRec>{recibo}</nRec><tMed>{tmed}</tMed></infRec>"),
    )
}

pub fn ret_reci(recibo: &str, cstat: &str, motivo: &str, prots: &str) -> String {
    format!(
        "<retConsReciNFe versao=\"4.00\" xmlns=\"{NS}\"><tpAmb>2</tpAmb><verAplic>SP_NFE_PL_009_V4</verAplic>\
         <nRec>{recibo}</nRec><cStat>{cstat}</cStat><xMotivo>{motivo}</xMotivo><cUF>35</cUF>\
         <dhRecbto>2026-10-03T09:00:09-03:00</dhRecbto>{prots}</retConsReciNFe>"
    )
}

pub fn ret_sit(chave: &str, cstat: &str, motivo: &str, miolo: &str) -> String {
    format!(
        "<retConsSitNFe versao=\"4.00\" xmlns=\"{NS}\"><tpAmb>2</tpAmb><verAplic>SP_NFE_PL_009_V4</verAplic>\
         <cStat>{cstat}</cStat><xMotivo>{motivo}</xMotivo><cUF>35</cUF>\
         <dhRecbto>2026-10-03T09:00:20-03:00</dhRecbto><chNFe>{chave}</chNFe>{miolo}</retConsSitNFe>"
    )
}

pub fn ret_status(cstat: &str, motivo: &str, extra: &str) -> String {
    format!(
        "<retConsStatServ versao=\"4.00\" xmlns=\"{NS}\"><tpAmb>2</tpAmb><verAplic>SP_NFE_PL_009_V4</verAplic>\
         <cStat>{cstat}</cStat><xMotivo>{motivo}</xMotivo><cUF>35</cUF>\
         <dhRecbto>2026-10-03T09:00:30-03:00</dhRecbto>{extra}</retConsStatServ>"
    )
}
