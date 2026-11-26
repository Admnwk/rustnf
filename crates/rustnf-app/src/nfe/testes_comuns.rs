// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Pecas comuns dos testes da NF-e na aplicacao: configuracao, pedido, certificado, transporte falso (que nunca toca a
//! rede) e as respostas da SEFAZ montadas a mao, com documentos e nomes ficticios.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use rustnf::assinatura::cert_a1::{self, Certificado};
use rustnf::nfe::sefaz::{ClienteSefaz, FalhaRede, Transporte};
use serde_json::{json, Value};

use super::Contexto;
use crate::config::Config;
use crate::notas::nfe::NotasNfe;
use crate::testes_comuns::PastaTemporaria;

pub(crate) const NS: &str = "http://www.portalfiscal.inf.br/nfe";
pub(crate) const AUT: &str = "NFeAutorizacao4";
pub(crate) const CON: &str = "NFeConsultaProtocolo4";
pub(crate) const STA: &str = "NFeStatusServico4";
pub(crate) const EVT: &str = "NFeRecepcaoEvento4";
pub(crate) const INU: &str = "NFeInutilizacao4";
pub(crate) const NPROT: &str = "135260000000001";

/// O relogio dos testes: o meio do dia da emissao do pedido.
pub(crate) fn agora() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 3, 12, 0, 0).unwrap()
}

pub(crate) fn certificado() -> Certificado {
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

/// A configuracao de um emitente ficticio de SP no Simples, com a pasta de notas em `pasta`.
pub(crate) fn config(pasta: &PastaTemporaria) -> Config {
    config_com(pasta, "")
}

/// Como [`config`], com `extra` (TOML) no fim do arquivo.
pub(crate) fn config_com(pasta: &PastaTemporaria, extra: &str) -> Config {
    let texto = format!(
        "[emitente]\ncnpj = \"11222333000181\"\nnome = \"LOJA DE TESTE LTDA\"\nmunicipio = \"3550308\"\n\
         [notas]\npasta = '{}'\n\
         [nfe]\nuf = \"SP\"\nie = \"111111111111\"\ncrt = 1\nserie = \"1\"\n\
         [nfe.endereco]\nlogradouro = \"Rua das Flores\"\nnumero = \"100\"\nbairro = \"Centro\"\n\
         municipio = \"Sao Paulo\"\ncep = \"01001000\"\n{extra}",
        pasta.caminho().join("notas").to_string_lossy()
    );
    let c = Config::de_texto(&texto).unwrap();
    c.validar().unwrap();
    c
}

/// O pedido da NF-e SEM o emitente (a configuracao o completa) e sem numero (a pasta o da).
pub(crate) fn pedido() -> Value {
    json!({
        "emissao": "2026-10-03T09:00:00-03:00",
        "nat_op": "VENDA",
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

/// O trecho entre duas marcas, ou vazio.
pub(crate) fn entre<'a>(texto: &'a str, abre: &str, fecha: &str) -> &'a str {
    texto.find(abre).map_or("", |i| {
        let i = i + abre.len();
        texto[i..].find(fecha).map_or("", |f| &texto[i..i + f])
    })
}

/// Resposta programada do transporte falso.
pub(crate) enum Resp {
    /// HTTP e corpo.
    R(u16, Vec<u8>),
    /// Sem resposta de rede.
    F(FalhaRede),
    /// Resposta montada a partir do pedido que chegou (para saber a chave que o app gerou).
    Din(Box<dyn Fn(&str) -> Resp + Send + Sync>),
}

pub(crate) fn soap(ns_wsdl: &str, ret: &str) -> Vec<u8> {
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?><soap:Envelope xmlns:soap=\"http://www.w3.org/2003/05/soap-envelope\" \
         xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"><soap:Body>\
         <nfeResultMsg xmlns=\"http://www.portalfiscal.inf.br/nfe/wsdl/{ns_wsdl}\">{ret}</nfeResultMsg>\
         </soap:Body></soap:Envelope>"
    )
    .into_bytes()
}

pub(crate) fn ok(ns_wsdl: &str, ret: &str) -> Resp {
    Resp::R(200, soap(ns_wsdl, ret))
}

pub(crate) fn tempo() -> Resp {
    Resp::F(FalhaRede::Tempo)
}

pub(crate) fn prot(chave: &str, cstat: &str, motivo: &str, nprot: &str) -> String {
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

fn ret_envi(cstat: &str, motivo: &str, miolo: &str) -> String {
    format!(
        "<retEnviNFe versao=\"4.00\" xmlns=\"{NS}\"><tpAmb>2</tpAmb><verAplic>SP_NFE_PL_009_V4</verAplic>\
         <cStat>{cstat}</cStat><xMotivo>{motivo}</xMotivo><cUF>35</cUF><dhRecbto>2026-10-03T09:00:05-03:00</dhRecbto>\
         {miolo}</retEnviNFe>"
    )
}

pub(crate) fn ret_sit(chave: &str, cstat: &str, motivo: &str, miolo: &str) -> String {
    format!(
        "<retConsSitNFe versao=\"4.00\" xmlns=\"{NS}\"><tpAmb>2</tpAmb><verAplic>SP_NFE_PL_009_V4</verAplic>\
         <cStat>{cstat}</cStat><xMotivo>{motivo}</xMotivo><cUF>35</cUF>\
         <dhRecbto>2026-10-03T09:00:20-03:00</dhRecbto><chNFe>{chave}</chNFe>{miolo}</retConsSitNFe>"
    )
}

pub(crate) fn ret_status(cstat: &str, motivo: &str) -> String {
    format!(
        "<retConsStatServ versao=\"4.00\" xmlns=\"{NS}\"><tpAmb>2</tpAmb><verAplic>SP_NFE_PL_009_V4</verAplic>\
         <cStat>{cstat}</cStat><xMotivo>{motivo}</xMotivo><cUF>35</cUF>\
         <dhRecbto>2026-10-03T09:00:30-03:00</dhRecbto></retConsStatServ>"
    )
}

/// O envio sincrono respondido com o protocolo `cstat` para a chave que chegou no pedido.
pub(crate) fn envio(cstat: &'static str, motivo: &'static str, nprot: &'static str) -> Resp {
    Resp::Din(Box::new(move |corpo| {
        let chave = entre(corpo, "Id=\"NFe", "\"").to_string();
        ok(
            AUT,
            &ret_envi(
                "104",
                "Lote processado",
                &prot(&chave, cstat, motivo, nprot),
            ),
        )
    }))
}

/// A consulta pela chave que chegou, respondida com o `cstat` (e o protocolo da autorizacao, se `nprot` nao e' vazio).
pub(crate) fn consulta(cstat: &'static str, motivo: &'static str, nprot: &'static str) -> Resp {
    Resp::Din(Box::new(move |corpo| {
        let chave = entre(corpo, "<chNFe>", "</chNFe>").to_string();
        let miolo = if nprot.is_empty() {
            String::new()
        } else {
            prot(&chave, "100", "Autorizado o uso da NF-e", nprot)
        };
        ok(CON, &ret_sit(&chave, cstat, motivo, &miolo))
    }))
}

/// O lote de evento (128) com um `retEvento` do `cstat` dado, para o tipo e a chave que chegaram no pedido.
pub(crate) fn evento(cstat: &'static str, motivo: &'static str, nprot: &'static str) -> Resp {
    Resp::Din(Box::new(move |corpo| {
        let chave = entre(corpo, "<chNFe>", "</chNFe>").to_string();
        let tipo = entre(corpo, "<tpEvento>", "</tpEvento>").to_string();
        let seq = entre(corpo, "<nSeqEvento>", "</nSeqEvento>").to_string();
        let np = if nprot.is_empty() {
            String::new()
        } else {
            format!("<nProt>{nprot}</nProt>")
        };
        let ret = format!(
            "<retEvento versao=\"1.00\"><infEvento><tpAmb>2</tpAmb><verAplic>SP_EVENTOS_PL_100</verAplic>\
             <cOrgao>35</cOrgao><cStat>{cstat}</cStat><xMotivo>{motivo}</xMotivo><chNFe>{chave}</chNFe>\
             <tpEvento>{tipo}</tpEvento><xEvento>Evento de teste</xEvento><nSeqEvento>{seq}</nSeqEvento>\
             <dhRegEvento>2026-10-03T09:00:03-03:00</dhRegEvento>{np}</infEvento></retEvento>"
        );
        ok(
            EVT,
            &format!(
                "<retEnvEvento versao=\"1.00\" xmlns=\"{NS}\"><idLote>1</idLote><tpAmb>2</tpAmb>\
                 <verAplic>SP_EVENTOS_PL_100</verAplic><cOrgao>35</cOrgao><cStat>128</cStat>\
                 <xMotivo>Lote de Evento Processado</xMotivo>{ret}</retEnvEvento>"
            ),
        )
    }))
}

/// O `retInutNFe` para a faixa que chegou no pedido.
pub(crate) fn inutilizacao(cstat: &'static str, motivo: &'static str, nprot: &'static str) -> Resp {
    Resp::Din(Box::new(move |corpo| {
        let serie = entre(corpo, "<serie>", "</serie>");
        let ini = entre(corpo, "<nNFIni>", "</nNFIni>");
        let fim = entre(corpo, "<nNFFin>", "</nNFFin>");
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
        ok(
            INU,
            &format!(
                "<retInutNFe versao=\"4.00\" xmlns=\"{NS}\"><infInut><tpAmb>2</tpAmb>\
                 <verAplic>SP_NFE_PL_009_V4</verAplic><cStat>{cstat}</cStat><xMotivo>{motivo}</xMotivo>\
                 <cUF>35</cUF>{faixa}<dhRecbto>2026-10-03T09:00:03-03:00</dhRecbto>{np}</infInut></retInutNFe>"
            ),
        )
    }))
}

/// Transporte com roteiro: cada chamada tira a proxima resposta; a chamada que nao bate com o servico esperado derruba o
/// teste.
#[derive(Clone, Default)]
pub(crate) struct Falso {
    roteiro: Arc<Mutex<VecDeque<(&'static str, Resp)>>>,
    chamadas: Arc<Mutex<Vec<String>>>,
    corpos: Arc<Mutex<Vec<String>>>,
}

impl Falso {
    pub(crate) fn novo(r: Vec<(&'static str, Resp)>) -> Self {
        Falso {
            roteiro: Arc::new(Mutex::new(r.into())),
            ..Default::default()
        }
    }

    /// O servico de cada chamada feita, na ordem.
    pub(crate) fn servicos(&self) -> Vec<String> {
        self.chamadas.lock().unwrap().clone()
    }

    /// O corpo de cada chamada feita, na ordem.
    pub(crate) fn corpos(&self) -> Vec<String> {
        self.corpos.lock().unwrap().clone()
    }

    pub(crate) fn restantes(&self) -> usize {
        self.roteiro.lock().unwrap().len()
    }
}

#[async_trait]
impl Transporte for Falso {
    async fn enviar(
        &self,
        url: &str,
        corpo: Vec<u8>,
        _content_type: &str,
    ) -> Result<(u16, Vec<u8>), FalhaRede> {
        let texto = String::from_utf8(corpo).unwrap();
        let (espera, mut resp) = self
            .roteiro
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| panic!("chamada fora do roteiro: {url}"));
        assert!(
            texto.contains(&format!("wsdl/{espera}\"")),
            "o roteiro esperava {espera}, veio {url}\n{texto}"
        );
        self.chamadas.lock().unwrap().push(espera.to_string());
        self.corpos.lock().unwrap().push(texto.clone());
        while let Resp::Din(f) = resp {
            resp = f(&texto);
        }
        match resp {
            Resp::R(st, b) => Ok((st, b)),
            Resp::F(f) => Err(f),
            Resp::Din(_) => unreachable!(),
        }
    }
}

/// O cliente da biblioteca, montado como o app o monta (`config_sefaz`), sobre o transporte falso e o relogio fixo.
pub(crate) fn cliente(config: &Config, f: Falso) -> ClienteSefaz<Falso> {
    ClienteSefaz::com_transporte(super::config_sefaz(config).unwrap(), f)
        .unwrap()
        .com_relogio(agora)
        .com_dormir(|_: Duration| Box::pin(async {}))
}

/// Tudo de um teste: pasta temporaria, configuracao, certificado, pasta de notas e cliente com o roteiro dado.
pub(crate) struct Cenario {
    pub(crate) pasta: PastaTemporaria,
    pub(crate) config: Config,
    pub(crate) certificado: Certificado,
    pub(crate) notas: NotasNfe,
    pub(crate) cliente: ClienteSefaz<Falso>,
    pub(crate) falso: Falso,
}

impl Cenario {
    pub(crate) fn novo(rotulo: &str, roteiro: Vec<(&'static str, Resp)>) -> Self {
        Self::com(rotulo, "", roteiro)
    }

    pub(crate) fn com(rotulo: &str, extra: &str, roteiro: Vec<(&'static str, Resp)>) -> Self {
        let pasta = PastaTemporaria::nova(rotulo);
        let config = config_com(&pasta, extra);
        let notas = NotasNfe::abrir(pasta.caminho().join("notas")).unwrap();
        let falso = Falso::novo(roteiro);
        let cliente = cliente(&config, falso.clone());
        Self {
            pasta,
            config,
            certificado: certificado(),
            notas,
            cliente,
            falso,
        }
    }

    pub(crate) fn ctx(&self) -> Contexto<'_, Falso> {
        Contexto::novo(
            &self.cliente,
            &self.certificado,
            &self.config,
            Some(&self.notas),
        )
        .com_agora(agora())
    }

    /// Troca o roteiro do transporte falso (o cliente e a pasta seguem os mesmos).
    pub(crate) fn roteiro(&self, r: Vec<(&'static str, Resp)>) {
        *self.falso.roteiro.lock().unwrap() = r.into();
    }
}
