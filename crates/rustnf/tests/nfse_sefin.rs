// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Cliente da Sefin Nacional com transporte falso: autorizada, rejeitada, resposta perdida com nota emitida ->
//! recuperada, sem nota -> mesmo XML, E0014, E1206/E1217 (transitorias), HTML de proxy, ambiente trocado, eventos e
//! Testar.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::Utc;
use rustnf::assinatura::cert_a1::{self, Certificado};
use rustnf::assinatura::xmldsig;
use rustnf::chave_dfe;
use rustnf::nfse::sefin::{ClienteNfse, FalhaRede, Status, Transporte};
use rustnf::nfse::{compactar, descompactar, Dps};
use serde_json::{json, Value};

const ID: &str = "DPS355030821122233300018100001000000000000042";

fn chave() -> String {
    chave_dfe::montar_nfse(3550308, "11222333000181", 42, "2610", Some(1), 2).unwrap()
}

fn casos() -> Value {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/casos/nfse_nacional.json");
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn dps() -> Dps {
    serde_json::from_value(casos()["dps"][0]["entrada"].clone()).unwrap()
}

fn a1() -> Certificado {
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

fn nfse_xml(ch: &str) -> String {
    format!("<NFSe xmlns=\"http://www.sped.fazenda.gov.br/nfse\" versao=\"1.01\"><infNFSe Id=\"NFS{ch}\"><nNFSe>77</nNFSe>\
             <cStat>100</cStat><ambGer>2</ambGer><valores><vLiq>150.00</vLiq></valores><DPS versao=\"1.01\">\
             <infDPS Id=\"{ID}\"><tpAmb>2</tpAmb><serie>1</serie><nDPS>42</nDPS></infDPS></DPS></infNFSe></NFSe>")
}

#[derive(Clone)]
enum Resp {
    R(u16, Vec<u8>),
    F(FalhaRede),
}

fn j(st: u16, v: Value) -> Resp {
    Resp::R(st, v.to_string().into_bytes())
}

type Roteiro = Vec<(&'static str, String, Resp)>;
type Pedidos = Vec<(String, String, Option<Value>)>;

#[derive(Clone, Default)]
struct Falso {
    roteiro: Arc<Mutex<Roteiro>>,
    pedidos: Arc<Mutex<Pedidos>>,
}

impl Falso {
    fn novo(r: Vec<(&'static str, String, Resp)>) -> Self {
        Falso {
            roteiro: Arc::new(Mutex::new(r)),
            ..Default::default()
        }
    }
    fn pedidos(&self) -> Vec<(String, String, Option<Value>)> {
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
        self.pedidos.lock().unwrap().push((
            metodo.into(),
            url.into(),
            corpo.map(|c| serde_json::from_slice(&c).unwrap()),
        ));
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

fn cli(r: Vec<(&'static str, String, Resp)>) -> (ClienteNfse<Falso>, Falso) {
    let f = Falso::novo(r);
    (
        ClienteNfse::com_transporte(a1(), 2, None, f.clone()).unwrap(),
        f,
    )
}

fn ok_nfse(ch: &str) -> Resp {
    j(
        201,
        json!({"chaveAcesso": ch, "idDps": ID, "nfseXmlGZipB64": compactar(&nfse_xml(ch))}),
    )
}

#[tokio::test]
async fn emitir_autorizada_e_documento_assinado() {
    let ch = chave();
    let (c, f) = cli(vec![("POST", "/nfse".into(), ok_nfse(&ch))]);
    let r = c.emitir(&dps()).await;
    assert!(
        r.status == Status::Autorizada && r.ok() && r.chave == ch && !r.recuperada,
        "{}",
        r.frase
    );
    assert_eq!(r.nfse.unwrap().numero, "77");
    let p = f.pedidos();
    assert!(p[0].1.ends_with("/SefinNacional/nfse"));
    let enviado =
        descompactar(p[0].2.as_ref().unwrap()["dpsXmlGZipB64"].as_str().unwrap()).unwrap();
    assert!(xmldsig::conferido(&enviado));
}

#[tokio::test]
async fn rejeicao_traz_o_motivo() {
    let (c, _) = cli(vec![(
        "POST",
        "/nfse".into(),
        j(
            400,
            json!({"erros": [{"codigo": "E0310", "descricao": "cTribNac"}]}),
        ),
    )]);
    let r = c.emitir(&dps()).await;
    assert!(r.status == Status::Rejeitada && r.frase.contains("E0310") && !r.pode_reenviar());
}

#[tokio::test]
async fn resposta_perdida_e_nota_emitida_vira_autorizada_recuperada() {
    let ch = chave();
    let (c, f) = cli(vec![
        ("POST", "/nfse".into(), Resp::F(FalhaRede::Tempo)),
        (
            "GET",
            format!("/dps/{ID}"),
            j(200, json!({"idDps": ID, "chaveAcesso": ch})),
        ),
        (
            "GET",
            format!("/nfse/{ch}"),
            j(
                200,
                json!({"chaveAcesso": ch, "nfseXmlGZipB64": compactar(&nfse_xml(&ch))}),
            ),
        ),
    ]);
    let r = c.emitir(&dps()).await;
    assert!(
        r.status == Status::Autorizada && r.recuperada && r.chave == ch,
        "{}",
        r.frase
    );
    assert_eq!(
        f.pedidos().iter().map(|p| p.0.clone()).collect::<Vec<_>>(),
        vec!["POST", "GET", "GET"]
    );
}

#[tokio::test]
async fn resposta_perdida_sem_nota_manda_reenviar_o_mesmo_xml() {
    let ch = chave();
    let (c, _) = cli(vec![
        (
            "POST",
            "/nfse".into(),
            Resp::F(FalhaRede::Rede("reset".into())),
        ),
        (
            "GET",
            format!("/dps/{ID}"),
            j(404, json!({"erro": {"codigo": "E", "descricao": "nao"}})),
        ),
    ]);
    let r = c.emitir(&dps()).await;
    assert!(
        r.status == Status::Pendente
            && r.pode_reenviar()
            && r.frase.contains("MESMA")
            && r.id_dps == ID
    );
    let (c2, f2) = cli(vec![("POST", "/nfse".into(), ok_nfse(&ch))]);
    let r2 = c2.reenviar(&r.xml_assinado).await;
    assert_eq!(r2.status, Status::Autorizada);
    let enviado = descompactar(
        f2.pedidos()[0].2.as_ref().unwrap()["dpsXmlGZipB64"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(enviado, r.xml_assinado);
}

#[tokio::test]
async fn sem_resposta_nem_na_consulta_fica_pendente() {
    let (c, _) = cli(vec![
        ("POST", "/nfse".into(), Resp::F(FalhaRede::Tempo)),
        ("GET", "/dps/".into(), Resp::F(FalhaRede::Tempo)),
    ]);
    let r = c.emitir(&dps()).await;
    assert!(r.status == Status::Pendente && r.frase.contains("nunca emita outra"));
}

#[tokio::test]
async fn e0014_com_nota_existente_recupera_e_e1217_e_indisponivel() {
    let ch = chave();
    let (c, _) = cli(vec![
        (
            "POST",
            "/nfse".into(),
            j(
                400,
                json!({"erros": [{"codigo": "E0014", "descricao": "ja existe"}]}),
            ),
        ),
        (
            "GET",
            format!("/dps/{ID}"),
            j(200, json!({"chaveAcesso": ch})),
        ),
        (
            "GET",
            format!("/nfse/{ch}"),
            j(200, json!({"nfseXmlGZipB64": compactar(&nfse_xml(&ch))})),
        ),
    ]);
    let r = c.emitir(&dps()).await;
    assert!(r.status == Status::Autorizada && r.recuperada);
    let (c, _) = cli(vec![(
        "POST",
        "/nfse".into(),
        j(
            400,
            json!({"erros": [{"codigo": "E1217", "descricao": "manutencao"}]}),
        ),
    )]);
    let r = c.emitir(&dps()).await;
    assert!(r.status == Status::Indisponivel && r.pode_reenviar());
    let (c, _) = cli(vec![(
        "POST",
        "/nfse".into(),
        j(
            400,
            json!({"erros": [{"codigo": "E1206", "descricao": "LCR inacessivel"}]}),
        ),
    )]);
    let r = c.emitir(&dps()).await;
    assert!(
        r.status == Status::Indisponivel && r.pode_reenviar(),
        "{}",
        r.frase
    );
}

#[tokio::test]
async fn html_de_proxy_nao_vira_sucesso_e_ambiente_trocado_nao_sai() {
    let (c, _) = cli(vec![
        (
            "POST",
            "/nfse".into(),
            Resp::R(502, b"<html>bad gateway</html>".to_vec()),
        ),
        ("GET", "/dps/".into(), Resp::R(404, Vec::new())),
    ]);
    let r = c.emitir(&dps()).await;
    assert!(r.status == Status::Pendente && !r.ok());
    let (c, f) = cli(vec![]);
    let mut d = dps();
    d.ambiente = 1;
    let r = c.emitir(&d).await;
    assert!(r.status == Status::Erro && r.frase.contains("E0006") && f.pedidos().is_empty());
}

#[tokio::test]
async fn consultar_dps_200_sem_chave_nao_e_sucesso() {
    let (c, _) = cli(vec![(
        "GET",
        format!("/dps/{ID}"),
        j(200, json!({"idDps": ID})),
    )]);
    assert_eq!(
        c.consultar_dps(ID, true).await.status,
        Status::NaoEncontrada
    );
}

#[tokio::test]
async fn cancelar_assina_e_sem_resposta_consulta_o_evento() {
    let ch = chave();
    let (c, f) = cli(vec![(
        "POST",
        "/eventos".into(),
        j(201, json!({"eventoXmlGZipB64": compactar("<evento/>")})),
    )]);
    let r = c.cancelar(&ch, "1", "Valor digitado errado na nota").await;
    assert_eq!(r.status, Status::Registrado);
    let pedido = descompactar(
        f.pedidos()[0].2.as_ref().unwrap()["pedidoRegistroEventoXmlGZipB64"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert!(
        xmldsig::conferido(&pedido) && pedido.contains("<CNPJAutor>11222333000181</CNPJAutor>")
    );
    let (c, _) = cli(vec![
        ("POST", "/eventos".into(), Resp::F(FalhaRede::Tempo)),
        (
            "GET",
            format!("/nfse/{ch}/eventos/101101/1"),
            j(200, json!({"eventoXmlGZipB64": compactar("<e/>")})),
        ),
    ]);
    let r = c.cancelar(&ch, "1", "Valor digitado errado na nota").await;
    assert!(r.status == Status::Registrado && r.recuperada);
}

#[tokio::test]
async fn testar_aceita_resposta_de_negocio_e_acusa_recusa_do_certificado() {
    let (c, _) = cli(vec![(
        "GET",
        format!("/nfse/{}", "0".repeat(50)),
        j(400, json!({"erro": {"codigo": "E1", "descricao": "chave"}})),
    )]);
    let (ok, frase) = c.testar().await;
    assert!(ok && frase.contains("certificado aceito"), "{frase}");
    let (c, _) = cli(vec![(
        "GET",
        "/nfse/".into(),
        Resp::R(403, b"forbidden".to_vec()),
    )]);
    assert_eq!(
        c.testar().await,
        (
            false,
            "O SEFIN recusou o certificado (HTTP 403).".to_string()
        )
    );
}

#[tokio::test]
async fn consultar_nfse_e_cancelada() {
    let ch = chave();
    let (c, _) = cli(vec![(
        "GET",
        format!("/nfse/{ch}"),
        j(
            200,
            json!({"chaveAcesso": ch, "nfseXmlGZipB64": compactar(&nfse_xml(&ch))}),
        ),
    )]);
    let r = c.consultar_nfse(&ch).await;
    assert!(
        r.status == Status::Autorizada && r.nfse.is_some(),
        "{}",
        r.frase
    );
    let (c, _) = cli(vec![(
        "GET",
        format!("/nfse/{ch}/eventos/101101/1"),
        j(200, json!({"eventoXmlGZipB64": compactar("<e/>")})),
    )]);
    assert_eq!(c.cancelada(&ch).await.status, Status::Ok);
}

#[test]
fn base_http_so_no_loopback() {
    assert!(
        ClienteNfse::com_transporte(a1(), 2, Some("http://127.0.0.1:9"), Falso::default()).is_ok()
    );
    assert!(
        ClienteNfse::com_transporte(a1(), 2, Some("http://exemplo.com.br"), Falso::default())
            .is_err()
    );
}
