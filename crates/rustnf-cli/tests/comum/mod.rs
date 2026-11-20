// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Ajudantes dos testes de integracao: contratos da biblioteca, transporte falso da Sefin e roteador em memoria.
#![allow(dead_code, unreachable_pub)]

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode};
use axum::Router;
use chrono::Utc;
use rustnf::assinatura::cert_a1::{self, Certificado};
use rustnf::chave_dfe;
use rustnf::nfse::compactar;
use rustnf::nfse::sefin::{ClienteNfse, FalhaRede, Transporte};
use rustnf_cli::emissao::Emissor;
use rustnf_cli::http::{roteador, Config, Estado, Token};
use serde_json::{json, Value};
use tower::ServiceExt;

pub const TOKEN: &str = "token-de-teste-com-bem-mais-de-trinta-e-dois-caracteres";
pub const ID_DPS: &str = "DPS355030821122233300018100001000000000000042";

pub fn casos(arquivo: &str) -> Value {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../rustnf/tests/casos")
        .join(arquivo);
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

/// A entrada do caso `i` da secao `dps` do contrato da DPS.
pub fn dps_valida() -> Value {
    casos("nfse_nacional.json")["dps"][0]["entrada"].clone()
}

pub fn xml_da_dps_valida() -> String {
    casos("nfse_nacional.json")["dps"][0]["xml"]
        .as_str()
        .unwrap()
        .to_string()
}

/// O primeiro caso de recusa do contrato da DPS: (entrada, campo).
pub fn dps_recusada() -> (Value, String) {
    let c = &casos("nfse_nacional.json")["dps_erro"][0];
    (c["entrada"].clone(), c["campo"].as_str().unwrap().into())
}

pub fn nfse_xml_para_danfse() -> String {
    casos("danfse.json")["casos"][0]["xml"]
        .as_str()
        .unwrap()
        .to_string()
}

pub fn chave() -> String {
    chave_dfe::montar_nfse(3550308, "11222333000181", 42, "2610", Some(1), 2).unwrap()
}

fn nfse_xml(ch: &str) -> String {
    format!("<NFSe xmlns=\"http://www.sped.fazenda.gov.br/nfse\" versao=\"1.01\"><infNFSe Id=\"NFS{ch}\"><nNFSe>77</nNFSe>\
             <cStat>100</cStat><ambGer>2</ambGer><valores><vLiq>150.00</vLiq></valores><DPS versao=\"1.01\">\
             <infDPS Id=\"{ID_DPS}\"><tpAmb>2</tpAmb><serie>1</serie><nDPS>42</nDPS></infDPS></DPS></infNFSe></NFSe>")
}

pub fn a1() -> Certificado {
    let pfx = cert_a1::gerar_teste(
        "11222333000181",
        "OFICINA TESTE",
        "teste",
        365,
        true,
        false,
        Utc::now(),
    )
    .unwrap();
    cert_a1::carregar(&pfx, "teste").unwrap()
}

/// Resposta programada do transporte falso.
#[derive(Clone)]
pub enum Resp {
    R(u16, Vec<u8>),
    F(FalhaRede),
}

pub fn json_resp(status: u16, v: Value) -> Resp {
    Resp::R(status, v.to_string().into_bytes())
}

/// Resposta de emissao autorizada.
pub fn autorizada(ch: &str) -> Resp {
    json_resp(
        201,
        json!({"chaveAcesso": ch, "idDps": ID_DPS, "nfseXmlGZipB64": compactar(&nfse_xml(ch))}),
    )
}

/// Resposta de consulta da NFS-e.
pub fn nota(ch: &str) -> Resp {
    json_resp(
        200,
        json!({"chaveAcesso": ch, "nfseXmlGZipB64": compactar(&nfse_xml(ch))}),
    )
}

pub type Roteiro = Vec<(&'static str, String, Resp)>;

/// Transporte falso: nunca toca a rede.
#[derive(Clone, Default)]
pub struct Falso {
    roteiro: Arc<Mutex<Roteiro>>,
    pub pedidos: Arc<Mutex<Vec<(String, String)>>>,
    pub atraso: Duration,
}

impl Falso {
    pub fn novo(roteiro: Roteiro) -> Self {
        Self {
            roteiro: Arc::new(Mutex::new(roteiro)),
            ..Self::default()
        }
    }

    pub fn com_atraso(mut self, atraso: Duration) -> Self {
        self.atraso = atraso;
        self
    }

    pub fn chamadas(&self) -> Vec<(String, String)> {
        self.pedidos.lock().unwrap().clone()
    }
}

#[async_trait]
impl Transporte for Falso {
    async fn enviar(
        &self,
        metodo: &str,
        url: &str,
        _corpo: Option<Vec<u8>>,
        _aceita: &str,
    ) -> Result<(u16, Vec<u8>), FalhaRede> {
        self.pedidos
            .lock()
            .unwrap()
            .push((metodo.into(), url.into()));
        if !self.atraso.is_zero() {
            tokio::time::sleep(self.atraso).await;
        }
        let mut r = self.roteiro.lock().unwrap();
        if let Some(i) = r
            .iter()
            .position(|(m, s, _)| *m == metodo && url.contains(s.as_str()))
        {
            return match r.remove(i).2 {
                Resp::R(st, b) => Ok((st, b)),
                Resp::F(f) => Err(f),
            };
        }
        Ok((
            404,
            br#"{"erro":{"codigo":"E404","descricao":"nao achado"}}"#.to_vec(),
        ))
    }
}

pub fn emissor_falso(falso: Falso) -> Arc<dyn Emissor> {
    Arc::new(ClienteNfse::com_transporte(a1(), 2, None, falso).unwrap())
}

pub fn app_leitura() -> Router {
    roteador(Estado::somente_leitura(Config::default()))
}

pub fn app_leitura_com(config: Config) -> Router {
    roteador(Estado::somente_leitura(config))
}

pub fn app_emissao(falso: Falso) -> Router {
    app_emissao_com(falso, Config::default())
}

pub fn app_emissao_com(falso: Falso, config: Config) -> Router {
    roteador(Estado::com_emissao(
        config,
        emissor_falso(falso),
        Token::novo(TOKEN.into()).unwrap(),
    ))
}

pub fn requisicao(
    metodo: &str,
    rota: &str,
    cabecalhos: &[(&str, &str)],
    corpo: Vec<u8>,
) -> Request<Body> {
    let mut b = Request::builder().method(metodo).uri(rota);
    for (k, v) in cabecalhos {
        b = b.header(*k, *v);
    }
    b.body(Body::from(corpo)).unwrap()
}

pub fn post_json(rota: &str, corpo: &Value) -> Request<Body> {
    requisicao(
        "POST",
        rota,
        &[("content-type", "application/json")],
        corpo.to_string().into_bytes(),
    )
}

pub fn post_json_token(rota: &str, corpo: &Value) -> Request<Body> {
    let auth = format!("Bearer {TOKEN}");
    requisicao(
        "POST",
        rota,
        &[
            ("content-type", "application/json"),
            ("authorization", auth.as_str()),
        ],
        corpo.to_string().into_bytes(),
    )
}

pub async fn enviar(app: &Router, req: Request<Body>) -> (StatusCode, HeaderMap, Vec<u8>) {
    let resp = app.clone().oneshot(req).await.unwrap();
    let (partes, corpo) = resp.into_parts();
    let bytes = axum::body::to_bytes(corpo, usize::MAX).await.unwrap();
    (partes.status, partes.headers, bytes.to_vec())
}

pub fn json_do_corpo(corpo: &[u8]) -> Value {
    serde_json::from_slice(corpo).unwrap_or_else(|e| {
        panic!(
            "corpo nao e' JSON ({e}): {}",
            String::from_utf8_lossy(corpo)
        )
    })
}
