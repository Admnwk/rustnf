// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Ajudantes dos testes de integracao: contratos da biblioteca, transporte falso da Sefin e roteador em memoria.
#![allow(dead_code, unreachable_pub)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
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
use rustnf_app::notas::Notas;
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
    /// Os corpos enviados, na ordem das chamadas.
    pub corpos: Arc<Mutex<Vec<Vec<u8>>>>,
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
        corpo: Option<Vec<u8>>,
        _aceita: &str,
    ) -> Result<(u16, Vec<u8>), FalhaRede> {
        self.pedidos
            .lock()
            .unwrap()
            .push((metodo.into(), url.into()));
        self.corpos.lock().unwrap().push(corpo.unwrap_or_default());
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

/// O arquivo de configuracao de exemplo, lido.
pub fn config_de_exemplo() -> rustnf_app::config::Config {
    rustnf_app::config::Config::ler(&exemplo()).unwrap()
}

/// Servidor so' de leitura com as `[[aliquotas]]` do arquivo de exemplo.
pub fn app_leitura_com_aliquotas() -> Router {
    roteador(
        Estado::somente_leitura(Config::default()).com_aliquotas(config_de_exemplo().publicadas()),
    )
}

/// Servidor com emissao, o `[emitente]` do arquivo de exemplo e a pasta de notas dada.
pub fn app_emissao_com_pasta(falso: Falso, pasta: &Path) -> Router {
    roteador(
        Estado::com_emissao(
            Config::default(),
            emissor_falso(falso),
            Token::novo(TOKEN.into()).unwrap(),
        )
        .com_emitente(config_de_exemplo().emitente)
        .com_notas(Some(Notas::abrir(pasta).unwrap())),
    )
}

pub fn id_dps(numero: u64) -> String {
    format!(
        "DPS{}{}{}{:05}{:015}",
        "3550308", "2", "11222333000181", 1, numero
    )
}

/// Resposta de emissao autorizada com a nota inteira (data de processamento e do tomador).
pub fn autorizada_completa(numero: u64) -> (String, Resp) {
    let ch =
        chave_dfe::montar_nfse(3550308, "11222333000181", 70 + numero, "2610", Some(1), 2).unwrap();
    let xml = format!(
        "<NFSe xmlns=\"http://www.sped.fazenda.gov.br/nfse\" versao=\"1.01\"><infNFSe Id=\"NFS{ch}\">\
         <nNFSe>{}</nNFSe><dhProc>2026-10-02T10:00:01-03:00</dhProc><cStat>100</cStat><ambGer>2</ambGer>\
         <valores><vLiq>150.00</vLiq></valores><DPS versao=\"1.01\"><infDPS Id=\"{}\"><tpAmb>2</tpAmb>\
         <serie>1</serie><nDPS>{numero}</nDPS><dCompet>2026-10-01</dCompet>\
         <toma><CNPJ>11222333000181</CNPJ><xNome>Cliente</xNome></toma>\
         <valores><vServPrest><vServ>150.00</vServ></vServPrest></valores></infDPS></DPS></infNFSe></NFSe>",
        70 + numero,
        id_dps(numero)
    );
    let resp = json_resp(
        201,
        json!({"chaveAcesso": ch, "idDps": id_dps(numero), "nfseXmlGZipB64": compactar(&xml)}),
    );
    (ch, resp)
}

/// O pedido valido sem nada do que o `[emitente]` completa.
pub fn pedido_sem_emitente() -> Value {
    let mut v = dps_valida();
    v["prestador"] = json!({});
    for campo in ["cmun_emissor", "numero"] {
        v.as_object_mut().unwrap().remove(campo);
    }
    v
}

/// Resposta de evento registrado.
pub fn evento_registrado() -> Resp {
    json_resp(201, json!({"eventoXmlGZipB64": compactar("<evento/>")}))
}

/// O XML da DPS que o transporte falso recebeu na chamada `i`.
pub fn dps_enviada(falso: &Falso, i: usize) -> String {
    let corpo: Value = serde_json::from_slice(&falso.corpos.lock().unwrap()[i]).unwrap();
    rustnf::nfse::descompactar(corpo["dpsXmlGZipB64"].as_str().unwrap()).unwrap()
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

/// Variaveis que mudam o comportamento do binario: o teste as tira do ambiente para nao depender da maquina.
const VARIAVEIS_DO_RUSTNF: [&str; 14] = [
    "RUSTNF_CONFIG",
    "RUSTNF_AMBIENTE",
    "RUSTNF_PFX",
    "RUSTNF_TEMPO_LIMITE_SEFIN",
    "RUSTNF_FOLGA_RELOGIO",
    "RUSTNF_NOTAS_PASTA",
    "RUSTNF_CANHOTO",
    "RUSTNF_ENDERECO",
    "RUSTNF_PORTA",
    "RUSTNF_LIMITE_CORPO",
    "RUSTNF_TEMPO_LIMITE",
    "RUSTNF_ORIGENS_PERMITIDAS",
    "RUSTNF_SERIE",
    "RUSTNF_COFRE",
];

/// Isola o processo do computador de quem roda o teste: sem variaveis `RUSTNF_*`, com a pasta de configuracao do
/// usuario apontando para uma pasta vazia (`vazia`) e com o cofre do sistema desligado.
pub fn isolar(cmd: &mut Command, vazia: &Path) {
    for v in VARIAVEIS_DO_RUSTNF {
        cmd.env_remove(v);
    }
    cmd.env("APPDATA", vazia)
        .env("XDG_CONFIG_HOME", vazia)
        .env("HOME", vazia)
        .env("RUSTNF_COFRE", "desligado");
}

/// Pasta nova em std::env::temp_dir() com nome unico, apagada no fim.
static CONTADOR: AtomicU64 = AtomicU64::new(0);

/// Pasta nova em `std::env::temp_dir()`, apagada no fim.
pub struct Pasta(pub PathBuf);

impl Pasta {
    pub fn nova() -> Self {
        let p = std::env::temp_dir().join(format!(
            "rustnf-cli-config-{}-{}",
            std::process::id(),
            CONTADOR.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&p).unwrap();
        Self(p)
    }

    pub fn caminho(&self, nome: &str) -> PathBuf {
        self.0.join(nome)
    }

    pub fn texto(&self, nome: &str) -> String {
        self.caminho(nome).to_string_lossy().into_owned()
    }

    pub fn escrever(&self, nome: &str, conteudo: &[u8]) -> String {
        std::fs::write(self.caminho(nome), conteudo).unwrap();
        self.texto(nome)
    }
}

impl Drop for Pasta {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub fn exemplo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("exemplos")
        .join("rustnf.toml")
}

pub fn pfx_de_teste(pasta: &Pasta) -> String {
    let bytes = cert_a1::gerar_teste(
        "11222333000181",
        "OFICINA TESTE",
        "teste",
        365,
        true,
        false,
        Utc::now(),
    )
    .unwrap();
    pasta.escrever("teste.pfx", &bytes)
}

/// Texto TOML com o caminho entre aspas simples (literal): serve para caminho do Windows, com barra invertida.
pub fn toml_com_pfx(pfx: &str, extra: &str) -> String {
    format!("[sefin]\nambiente = \"restrita\"\npfx = '{pfx}'\n{extra}")
}
