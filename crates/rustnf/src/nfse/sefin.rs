// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Cliente do SEFIN Nacional (emissao, consultas e eventos). mTLS com o A1 EM MEMORIA
//! (`Certificado::identidade_pem` -> `reqwest::Identity`), verificacao do servidor sempre ligada: a cadeia do
//! SEFIN (AC SERPRO AR46 OV TLS CA 2025 -> GlobalSign Root R46) esta no `webpki-roots`; nao precisa embutir raiz
//! ICP-Brasil.
//!
//! Timeout, queda ou 5xx no `POST /nfse` NAO quer dizer que a nota nao saiu. Aqui a
//! DPS e' assinada UMA vez; falha de transporte ou E0014 -> `GET /dps/{id}`: achou a chave = `Autorizada` com
//! `recuperada`; 404 = `Pendente` com o MESMO XML para `reenviar` (o SEFIN deduplica pelo Id); nunca DPS nova.
//! Transporte por trait (`Transporte`): o teste injeta o falso; `TransporteReqwest` e' o real.

use std::time::Duration;

use async_trait::async_trait;
use serde_json::{json, Value};

use super::base::{casa_id_dps, so_alnum, ErroNfse};
use super::dps::{montar, Dps};
use super::eventos::{self, Pedido};
use super::leitura::{compactar, descompactar, ler_evento, ler_nfse, mensagens, Mensagem, Nfse};
use crate::assinatura::cert_a1::Certificado;
use crate::assinatura::xmldsig::{self, Algoritmo};

pub const SEFIN_PRODUCAO: &str = "https://sefin.nfse.gov.br/sefinnacional";
pub const SEFIN_RESTRITA: &str = "https://sefin.producaorestrita.nfse.gov.br/SefinNacional";
/// Servico parado / LCR do certificado inacessivel: nao e' da nota, reenviar o mesmo XML depois resolve.
/// E1206 = Anexo I v1.01, aba RN_RECEPCAO_DPS. E1217 ("paralisado para manutencao") nao consta da aba conferida: e'
/// tratado como transitorio por prudencia (reenviar o mesmo XML e' sempre seguro).
pub const TRANSITORIAS: &[&str] = &["E1217", "E1206"];
/// Serie+numero+municipio+inscricao ja geraram NFS-e.
pub const JA_EXISTE: &str = "E0014";
/// Quantas sequencias de um mesmo tipo de evento a recuperacao de um pedido sem resposta confere, no maximo.
const SEQUENCIAS_CONFERIDAS: u32 = 20;

pub use crate::https::FalhaRede;

/// Porta de transporte: (metodo, url, corpo JSON, Accept) -> (status, corpo).
#[async_trait]
pub trait Transporte: Send + Sync {
    async fn enviar(
        &self,
        metodo: &str,
        url: &str,
        corpo: Option<Vec<u8>>,
        aceita: &str,
    ) -> Result<(u16, Vec<u8>), FalhaRede>;
}

/// reqwest 0.12 + rustls, mTLS com o A1 em memoria, sem redirecionamento (credencial nao segue redirect).
pub struct TransporteReqwest {
    cliente: reqwest::Client,
}

impl TransporteReqwest {
    /// `ca_pem` = raiz EXTRA de confianca (por exemplo, de um simulador local), somada as do webpki-roots.
    pub fn novo(
        cert: &Certificado,
        ca_pem: Option<&[u8]>,
        timeout: Duration,
        ver_aplic: &str,
    ) -> Result<Self, String> {
        let cliente = crate::https::cliente(cert, ca_pem, &[], timeout, ver_aplic)?;
        Ok(TransporteReqwest { cliente })
    }
}

#[async_trait]
impl Transporte for TransporteReqwest {
    async fn enviar(
        &self,
        metodo: &str,
        url: &str,
        corpo: Option<Vec<u8>>,
        aceita: &str,
    ) -> Result<(u16, Vec<u8>), FalhaRede> {
        let mut r = if metodo == "POST" {
            self.cliente.post(url)
        } else {
            self.cliente.get(url)
        };
        r = r.header("Accept", aceita);
        if let Some(c) = corpo {
            r = r.header("Content-Type", "application/json").body(c);
        }
        let resp = r
            .send()
            .await
            .map_err(|e| crate::https::falha_do_envio(&e))?;
        let st = resp.status().as_u16();
        let b = resp
            .bytes()
            .await
            .map_err(|e| crate::https::falha_do_corpo(&e))?;
        Ok((st, b.to_vec()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Autorizada,
    Rejeitada,
    /// Sem confirmacao: reenviar o MESMO xml_assinado (nunca DPS nova).
    Pendente,
    /// O SEFIN disse que esta parado (E1217/E1206): mesmo XML, mais tarde.
    Indisponivel,
    Erro,
    Ok,
    NaoEncontrada,
    Registrado,
}

#[derive(Debug, Clone)]
pub struct Resultado {
    pub status: Status,
    /// Para a tela.
    pub frase: String,
    pub chave: String,
    pub id_dps: String,
    pub nfse: Option<Nfse>,
    pub xml_assinado: String,
    pub xml_retorno: String,
    pub erros: Vec<Mensagem>,
    pub alertas: Vec<Mensagem>,
    pub http: u16,
    pub recuperada: bool,
    pub corpo: Option<Value>,
    /// So' LOG.
    pub detalhe: String,
}

impl Resultado {
    fn novo(status: Status, frase: impl Into<String>) -> Self {
        Resultado {
            status,
            frase: frase.into(),
            chave: String::new(),
            id_dps: String::new(),
            nfse: None,
            xml_assinado: String::new(),
            xml_retorno: String::new(),
            erros: vec![],
            alertas: vec![],
            http: 0,
            recuperada: false,
            corpo: None,
            detalhe: String::new(),
        }
    }
    pub fn ok(&self) -> bool {
        matches!(
            self.status,
            Status::Autorizada | Status::Ok | Status::Registrado
        )
    }
    /// True = reenviar o MESMO `xml_assinado` e' seguro.
    pub fn pode_reenviar(&self) -> bool {
        matches!(self.status, Status::Pendente | Status::Indisponivel)
            && !self.xml_assinado.is_empty()
    }
}

fn juntar(m: &[Mensagem]) -> String {
    m.iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("; ")
}

fn base_segura(url: &str) -> Result<String, ErroNfse> {
    let u = url.trim().trim_end_matches('/');
    let local = |h: &str| matches!(h, "127.0.0.1" | "localhost" | "[::1]");
    if let Some(r) = u.strip_prefix("https://") {
        if !r.is_empty() {
            return Ok(u.into());
        }
    } else if let Some(r) = u.strip_prefix("http://") {
        let host = r.split(['/', ':']).next().unwrap_or("");
        if local(host) {
            return Ok(u.into());
        }
    }
    Err(ErroNfse::novo(
        "",
        "endereco do servico invalido: use https (http so' no proprio computador, para simulador)",
    ))
}

pub struct ClienteNfse<T: Transporte> {
    pub certificado: Certificado,
    pub ambiente: u8,
    pub sefin: String,
    /// Algoritmo da assinatura da DPS e dos eventos. Padrao: SHA-256; troque o campo para usar SHA-1.
    pub algoritmo: Algoritmo,
    pub ver_aplic: String,
    transporte: T,
}

impl ClienteNfse<TransporteReqwest> {
    /// Cliente real (`ambiente` 1 producao, 2 producao restrita). `sefin` sobrepoe a base (simulador local).
    pub fn real(
        certificado: Certificado,
        ambiente: u8,
        sefin: Option<&str>,
        ca_pem: Option<&[u8]>,
        timeout: Duration,
    ) -> Result<Self, ErroNfse> {
        let t = TransporteReqwest::novo(&certificado, ca_pem, timeout, "rustnf")
            .map_err(|e| ErroNfse::novo("", e))?;
        Self::com_transporte(certificado, ambiente, sefin, t)
    }
}

impl<T: Transporte> ClienteNfse<T> {
    pub fn com_transporte(
        certificado: Certificado,
        ambiente: u8,
        sefin: Option<&str>,
        transporte: T,
    ) -> Result<Self, ErroNfse> {
        let s = match ambiente {
            1 => SEFIN_PRODUCAO,
            2 => SEFIN_RESTRITA,
            _ => {
                return Err(ErroNfse::novo(
                    "tpAmb",
                    "ambiente 1 (producao) ou 2 (producao restrita)",
                ))
            }
        };
        Ok(ClienteNfse {
            certificado,
            ambiente,
            sefin: base_segura(sefin.unwrap_or(s))?,
            algoritmo: Algoritmo::default(),
            ver_aplic: "rustnf".into(),
            transporte,
        })
    }

    async fn chamar(
        &self,
        metodo: &str,
        url: &str,
        corpo: Option<Value>,
        aceita: &str,
    ) -> (u16, Vec<u8>, Option<Value>, String) {
        let dados = corpo.map(|c| c.to_string().into_bytes());
        match self.transporte.enviar(metodo, url, dados, aceita).await {
            Ok((st, b)) => {
                let j = serde_json::from_slice::<Value>(&b)
                    .ok()
                    .filter(Value::is_object);
                (st, b, j, String::new())
            }
            Err(f) => (0, Vec::new(), None, f.to_string()),
        }
    }

    pub fn assinar_dps(&self, xml: &str) -> Result<String, xmldsig::ErroAssinatura> {
        xmldsig::assinar(
            xml,
            &self.certificado.chave,
            &self.certificado.cert_der,
            "infDPS",
            self.algoritmo,
            None,
        )
    }

    /// (xml_assinado, id_dps): do modelo, do XML montado ou do XML ja assinado (reenvio).
    pub fn preparar_dps(&self, dps: &Dps) -> Result<(String, String), ErroNfse> {
        let mut d = dps.clone();
        if d.ver_aplic == "rustnf" {
            d.ver_aplic = self.ver_aplic.clone();
        }
        self.preparar_xml(&montar(&d)?)
    }

    pub fn preparar_xml(&self, xml: &str) -> Result<(String, String), ErroNfse> {
        let (id, amb, assinado) = {
            let doc = xmldsig::ler(xml).map_err(|e| ErroNfse::novo("", e.0))?;
            let inf = xmldsig::achar(&doc, "infDPS")
                .filter(|n| n.attribute("Id").is_some())
                .ok_or_else(|| ErroNfse::novo("infDPS/@Id", "XML sem DPS/infDPS com Id"))?;
            let amb = inf
                .children()
                .find(|c| c.tag_name().name() == "tpAmb")
                .and_then(|c| c.text())
                .unwrap_or("")
                .to_string();
            let assinado = doc
                .descendants()
                .any(|n| n.tag_name().name() == "Signature");
            (inf.attribute("Id").unwrap_or("").to_string(), amb, assinado)
        };
        if amb != self.ambiente.to_string() {
            return Err(ErroNfse::novo(
                "infDPS/tpAmb",
                "a DPS e' de outro ambiente que o deste cliente (E0006)",
            ));
        }
        let xml = if assinado {
            xml.to_string()
        } else {
            self.assinar_dps(xml)
                .map_err(|e| ErroNfse::novo("", format!("Nao foi possivel assinar a DPS: {e}")))?
        };
        Ok((xml, id))
    }

    /// Monta, assina UMA vez, envia e, se a resposta se perder, descobre se a nota saiu.
    pub async fn emitir(&self, dps: &Dps) -> Resultado {
        match self.preparar_dps(dps) {
            Ok((x, id)) => self.enviar_dps(x, id).await,
            Err(e) => Resultado::novo(Status::Erro, e.to_string()),
        }
    }

    /// Reenvio do MESMO XML (Pendente/Indisponivel). Idempotente: o SEFIN deduplica pelo Id.
    pub async fn reenviar(&self, xml_assinado: &str) -> Resultado {
        match self.preparar_xml(xml_assinado) {
            Ok((x, id)) => self.enviar_dps(x, id).await,
            Err(e) => Resultado::novo(Status::Erro, e.to_string()),
        }
    }

    async fn enviar_dps(&self, xml: String, id: String) -> Resultado {
        let (st, _, corpo, rede) = self
            .chamar(
                "POST",
                &format!("{}/nfse", self.sefin),
                Some(json!({"dpsXmlGZipB64": compactar(&xml)})),
                "application/json",
            )
            .await;
        if (st == 200 || st == 201)
            && corpo.as_ref().is_some_and(|c| {
                c.get("chaveAcesso").is_some() || c.get("nfseXmlGZipB64").is_some()
            })
        {
            return self.autorizada(corpo.expect("conferido"), xml, id, st);
        }
        let (erros, alertas) = corpo.as_ref().map(mensagens).unwrap_or_default();
        let codigos: Vec<&str> = erros.iter().map(|e| e.codigo.as_str()).collect();
        let transitoria = codigos.iter().any(|c| TRANSITORIAS.contains(c));
        let ja_existe = codigos.contains(&JA_EXISTE);
        let base = |mut r: Resultado| {
            r.id_dps = id.clone();
            r.xml_assinado = xml.clone();
            r.http = st;
            r.corpo = corpo.clone();
            r
        };
        if corpo.is_some() && !erros.is_empty() && !transitoria && !ja_existe && st < 500 {
            let mut r = base(Resultado::novo(
                Status::Rejeitada,
                format!("O SEFIN recusou a DPS: {}", juntar(&erros)),
            ));
            r.erros = erros;
            r.alertas = alertas;
            return r;
        }
        if transitoria {
            let mut r = base(Resultado::novo(
                Status::Indisponivel,
                format!(
                    "O SEFIN esta indisponivel agora ({}). A mesma DPS pode ser reenviada depois.",
                    juntar(&erros)
                ),
            ));
            r.erros = erros;
            return r;
        }
        let motivo = if rede.is_empty() {
            format!(
                "HTTP {st}{}",
                if corpo.is_some() { "" } else { " sem JSON" }
            )
        } else {
            rede
        };
        let mut achou = self.consultar_dps(&id, true).await;
        if achou.status == Status::Autorizada {
            achou.xml_assinado = xml;
            achou.recuperada = true;
            achou.detalhe = format!("recuperada pela consulta da DPS depois de: {motivo}");
            achou.frase = "NFS-e autorizada (a resposta do envio se perdeu; a nota foi achada pela consulta).".into();
            return achou;
        }
        let mut r = if achou.status == Status::NaoEncontrada && ja_existe {
            let mut r = Resultado::novo(
                Status::Rejeitada,
                format!(
                    "O SEFIN diz que esta serie/numero ja foi usada, mas a \
                consulta pela DPS nao achou a nota: confira a numeracao ({}).",
                    juntar(&erros)
                ),
            );
            r.erros = erros;
            r
        } else if achou.status == Status::NaoEncontrada {
            Resultado::novo(
                Status::Pendente,
                format!(
                    "A DPS nao chegou ao SEFIN ({motivo}). Reenvie a MESMA DPS -- nao emita outra."
                ),
            )
        } else {
            Resultado::novo(
                Status::Pendente,
                format!(
                    "Sem resposta do SEFIN ({motivo}) e sem como confirmar agora. A nota pode \
                ter sido emitida: consulte de novo ou reenvie a MESMA DPS -- nunca emita outra."
                ),
            )
        };
        r.detalhe = motivo;
        base(r)
    }

    fn autorizada(&self, corpo: Value, xml_assinado: String, id: String, st: u16) -> Resultado {
        let (_, alertas) = mensagens(&corpo);
        let mut chave = so_alnum(
            corpo
                .get("chaveAcesso")
                .and_then(Value::as_str)
                .unwrap_or(""),
        );
        let b64 = corpo
            .get("nfseXmlGZipB64")
            .or_else(|| corpo.get("nfseXmlGzipB64"))
            .and_then(Value::as_str)
            .unwrap_or("");
        let mut nota = None;
        let mut xml_nota = String::new();
        if !b64.is_empty() {
            match descompactar(b64)
                .ok()
                .and_then(|x| ler_nfse(&x).ok().map(|n| (x, n)))
            {
                Some((x, n)) => {
                    if chave.is_empty() {
                        chave = n.chave.clone();
                    }
                    xml_nota = x;
                    nota = Some(n);
                }
                None => {
                    let mut r = Resultado::novo(
                        Status::Pendente,
                        "O SEFIN respondeu autorizado, mas a NFS-e veio ilegivel: \
                        consulte a nota pela chave.",
                    );
                    r.chave = chave;
                    r.id_dps = id;
                    r.xml_assinado = xml_assinado;
                    r.http = st;
                    r.corpo = Some(corpo);
                    return r;
                }
            }
        }
        let num = nota
            .as_ref()
            .map(|n| n.numero.clone())
            .filter(|n| !n.is_empty());
        let mut r = Resultado::novo(
            Status::Autorizada,
            match num {
                Some(n) => format!("NFS-e autorizada: numero {n}."),
                None => "NFS-e autorizada.".into(),
            },
        );
        r.chave = chave;
        r.id_dps = corpo
            .get("idDps")
            .or_else(|| corpo.get("idDPS"))
            .and_then(Value::as_str)
            .map(String::from)
            .unwrap_or(id);
        r.nfse = nota;
        r.xml_assinado = xml_assinado;
        r.xml_retorno = xml_nota;
        r.alertas = alertas;
        r.http = st;
        r.corpo = Some(corpo);
        r
    }

    fn falha_consulta(st: u16, corpo: Option<Value>, rede: String, o_que: &str) -> Resultado {
        let (erros, _) = corpo.as_ref().map(mensagens).unwrap_or_default();
        let mut r = if st == 0 {
            Resultado::novo(
                Status::Pendente,
                format!("Sem resposta do SEFIN ao {o_que} ({rede})."),
            )
        } else if st == 404 {
            Resultado::novo(Status::NaoEncontrada, format!("Nao encontrado ao {o_que}."))
        } else {
            let m = if erros.is_empty() {
                format!("HTTP {st}")
            } else {
                juntar(&erros)
            };
            Resultado::novo(Status::Erro, format!("Falha ao {o_que}: {m}"))
        };
        r.erros = erros;
        r.http = st;
        r.corpo = corpo;
        r.detalhe = rede;
        r
    }

    /// `GET /dps/{id}` devolve SO' a chave; com `com_nota` busca a NFS-e pela chave.
    ///
    /// O `id_dps` precisa ter a forma do Id da DPS (45 posicoes); fora dela volta `Erro` sem tocar a rede.
    pub async fn consultar_dps(&self, id_dps: &str, com_nota: bool) -> Resultado {
        let id = id_dps.trim();
        if !casa_id_dps(id) {
            let mut r = Resultado::novo(
                Status::Erro,
                "O Id da DPS tem 45 posicoes: DPS + municipio(7) + tipo de inscricao(1) + inscricao(14) + serie(5) + numero(15).",
            );
            r.id_dps = id.into();
            return r;
        }
        let (st, _, corpo, rede) = self
            .chamar(
                "GET",
                &format!("{}/dps/{id}", self.sefin),
                None,
                "application/json",
            )
            .await;
        let chave = if st == 200 {
            so_alnum(
                corpo
                    .as_ref()
                    .and_then(|c| c.get("chaveAcesso"))
                    .and_then(Value::as_str)
                    .unwrap_or(""),
            )
        } else {
            String::new()
        };
        if chave.is_empty() {
            let mut r = if st == 200 {
                Resultado::novo(
                    Status::NaoEncontrada,
                    "O SEFIN respondeu sem a chave: nao ha NFS-e para esta DPS.",
                )
            } else {
                Self::falha_consulta(st, corpo, rede, "consultar a DPS")
            };
            r.id_dps = id.into();
            return r;
        }
        if !com_nota {
            let mut r = Resultado::novo(Status::Autorizada, "Ha NFS-e para esta DPS.");
            r.chave = chave;
            r.id_dps = id.into();
            return r;
        }
        let mut nota = self.consultar_nfse(&chave).await;
        nota.id_dps = id.into();
        if nota.status != Status::Autorizada {
            let mut r = Resultado::novo(
                Status::Autorizada,
                "Ha NFS-e para esta DPS (o XML da nota nao veio agora; consulte pela chave).",
            );
            r.chave = chave;
            r.id_dps = id.into();
            r.detalhe = nota.detalhe;
            return r;
        }
        nota
    }

    /// `GET /nfse/{chave}`.
    pub async fn consultar_nfse(&self, chave: &str) -> Resultado {
        let ch = so_alnum(chave);
        if ch.len() != 50 {
            return Resultado::novo(Status::Erro, "Chave da NFS-e Nacional tem 50 posicoes.");
        }
        let (st, _, corpo, rede) = self
            .chamar(
                "GET",
                &format!("{}/nfse/{ch}", self.sefin),
                None,
                "application/json",
            )
            .await;
        if st == 200
            && corpo
                .as_ref()
                .is_some_and(|c| c.get("nfseXmlGZipB64").is_some())
        {
            let mut r =
                self.autorizada(corpo.expect("conferido"), String::new(), String::new(), st);
            if r.chave.is_empty() {
                r.chave = ch;
            }
            return r;
        }
        let mut r = Self::falha_consulta(st, corpo, rede, "consultar a NFS-e");
        r.chave = ch;
        r
    }

    /// Assina o pedRegEvento e manda em `POST /nfse/{chave}/eventos`; sem resposta, consulta o evento antes.
    pub async fn registrar_evento(&self, pedido: &Pedido) -> Resultado {
        let xml = match xmldsig::assinar(
            &pedido.xml,
            &self.certificado.chave,
            &self.certificado.cert_der,
            "infPedReg",
            self.algoritmo,
            None,
        ) {
            Ok(x) => x,
            Err(e) => {
                let mut r = Resultado::novo(
                    Status::Erro,
                    format!("Nao foi possivel assinar o pedido: {e}"),
                );
                r.chave = pedido.chave.clone();
                return r;
            }
        };
        let (st, _, corpo, rede) = self
            .chamar(
                "POST",
                &format!("{}/nfse/{}/eventos", self.sefin, pedido.chave),
                Some(json!({"pedidoRegistroEventoXmlGZipB64": compactar(&xml)})),
                "application/json",
            )
            .await;
        let b64 = corpo
            .as_ref()
            .and_then(|c| {
                c.get("eventoXmlGZipB64")
                    .or_else(|| c.get("eventoXmlGzipB64"))
            })
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if (st == 200 || st == 201) && !b64.is_empty() {
            let mut r = Resultado::novo(
                Status::Registrado,
                format!("Evento {} registrado.", pedido.tipo),
            );
            r.chave = pedido.chave.clone();
            r.xml_assinado = xml;
            r.xml_retorno = descompactar(&b64).unwrap_or_default();
            r.http = st;
            r.corpo = corpo;
            return r;
        }
        let (erros, alertas) = corpo.as_ref().map(mensagens).unwrap_or_default();
        if corpo.is_some()
            && !erros.is_empty()
            && st < 500
            && !erros
                .iter()
                .any(|e| TRANSITORIAS.contains(&e.codigo.as_str()))
        {
            let mut r = Resultado::novo(
                Status::Rejeitada,
                format!("O SEFIN recusou o evento: {}", juntar(&erros)),
            );
            r.chave = pedido.chave.clone();
            r.xml_assinado = xml;
            r.erros = erros;
            r.alertas = alertas;
            r.http = st;
            r.corpo = corpo;
            return r;
        }
        if let Some(mut achou) = self.achar_registrado(pedido).await {
            achou.status = Status::Registrado;
            achou.recuperada = true;
            achou.xml_assinado = xml;
            achou.frase = format!(
                "Evento {} registrado (achado pela consulta depois de a resposta se perder).",
                pedido.tipo
            );
            return achou;
        }
        let motivo = if rede.is_empty() {
            format!("HTTP {st}")
        } else {
            rede
        };
        let mut r = Resultado::novo(
            Status::Pendente,
            format!(
                "Sem confirmacao do evento ({motivo}). Consulte a nota ou reenvie o MESMO pedido."
            ),
        );
        r.chave = pedido.chave.clone();
        r.xml_assinado = xml;
        r.http = st;
        r.detalhe = motivo;
        r
    }

    /// O pedido ja esta registrado? O SEFIN numera os eventos de um mesmo tipo (um 101103 pode vir depois de outro
    /// ja decidido), entao o evento da sequencia 1 pode ser de um pedido anterior. Percorre as sequencias do tipo
    /// ate a primeira que nao existe e so' aceita o evento que traz o mesmo `dhEvento` do pedido (o pedido assinado
    /// segue dentro do evento registrado).
    async fn achar_registrado(&self, pedido: &Pedido) -> Option<Resultado> {
        let enviado = ler_evento(&pedido.xml).ok()?.data_evento;
        if enviado.is_empty() {
            return None;
        }
        for sequencia in 1..=SEQUENCIAS_CONFERIDAS {
            let r = self
                .consultar_evento(&pedido.chave, &pedido.tipo, sequencia)
                .await;
            if r.status != Status::Ok {
                return None;
            }
            if ler_evento(&r.xml_retorno).is_ok_and(|e| e.data_evento == enviado) {
                return Some(r);
            }
        }
        None
    }

    /// 101101. Autor = documento do A1 (E0812/E0815: tem de ser o mesmo).
    pub async fn cancelar(&self, chave: &str, motivo: &str, justificativa: &str) -> Resultado {
        self.registrar(eventos::CANCELAMENTO, chave, motivo, justificativa)
            .await
    }

    /// Monta, assina e registra um evento que o contribuinte pode pedir (`tipo` e' o `tpEvento`). Autor = documento
    /// do A1. Tipo desconhecido, de outro autor ou com motivo fora da regra volta `Erro` sem tocar a rede.
    pub async fn registrar(
        &self,
        tipo: &str,
        chave: &str,
        motivo: &str,
        justificativa: &str,
    ) -> Resultado {
        match eventos::montar(&eventos::DadosEvento {
            tipo,
            chave,
            autor: self.certificado.documento(),
            motivo,
            justificativa,
            ambiente: self.ambiente,
            ver_aplic: &self.ver_aplic,
            ..Default::default()
        }) {
            Ok(p) => self.registrar_evento(&p).await,
            Err(e) => {
                let mut r = Resultado::novo(Status::Erro, e.to_string());
                r.chave = so_alnum(chave);
                r
            }
        }
    }

    /// `GET /nfse/{chave}/eventos/{tipo}/{seq}`.
    ///
    /// `Ok` so' quando o evento devolvido e' mesmo o da chave e do tipo pedidos; outro evento, ou um corpo que nao e'
    /// um evento, volta `Erro`. Chave, tipo (6 digitos) ou sequencia (a partir de 1) fora da forma voltam `Erro` sem
    /// tocar a rede.
    pub async fn consultar_evento(&self, chave: &str, tipo: &str, sequencia: u32) -> Resultado {
        let ch = so_alnum(chave);
        if ch.len() != 50
            || tipo.len() != 6
            || !tipo.bytes().all(|b| b.is_ascii_digit())
            || sequencia == 0
        {
            let mut r = Resultado::novo(
                Status::Erro,
                "Consulta de evento: a chave tem 50 posicoes, o tipo 6 digitos e a sequencia comeca em 1.",
            );
            r.chave = ch;
            return r;
        }
        let (st, _, corpo, rede) = self
            .chamar(
                "GET",
                &format!("{}/nfse/{ch}/eventos/{tipo}/{sequencia}", self.sefin),
                None,
                "application/json",
            )
            .await;
        let b64 = corpo
            .as_ref()
            .and_then(|c| c.get("eventoXmlGZipB64"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if st == 200 && !b64.is_empty() {
            let xml = descompactar(&b64).unwrap_or_default();
            let confere =
                ler_evento(&xml).is_ok_and(|e| so_alnum(&e.chave) == ch && e.tipo == tipo);
            let mut r = if confere {
                Resultado::novo(Status::Ok, format!("Evento {tipo} registrado na nota."))
            } else {
                Resultado::novo(
                    Status::Erro,
                    "O SEFIN respondeu com um evento que nao e' o pedido (outra nota, outro tipo ou ilegivel).",
                )
            };
            r.chave = ch;
            r.xml_retorno = xml;
            r.http = st;
            r.corpo = corpo;
            return r;
        }
        let mut r = Self::falha_consulta(st, corpo, rede, "consultar o evento");
        r.chave = ch;
        r
    }

    /// Cancelada? (101101 ou 105102). Ok = sim, NaoEncontrada = nao.
    pub async fn cancelada(&self, chave: &str) -> Resultado {
        let r = self.consultar_evento(chave, eventos::CANCELAMENTO, 1).await;
        if r.status == Status::NaoEncontrada {
            let r2 = self.consultar_evento(chave, eventos::SUBSTITUICAO, 1).await;
            if r2.status == Status::Ok {
                return r2;
            }
        }
        r
    }

    /// Botao Testar: A1 valido + SEFIN aceita o mTLS (consulta de chave inexistente: resposta de NEGOCIO = ok).
    pub async fn testar(&self) -> (bool, String) {
        let t = self.certificado.testar(
            None,
            chrono::Utc::now(),
            crate::assinatura::cert_a1::AVISO_DIAS,
        );
        if !t.ok {
            return (false, t.frase);
        }
        let (st, _, corpo, rede) = self
            .chamar(
                "GET",
                &format!("{}/nfse/{}", self.sefin, "0".repeat(50)),
                None,
                "application/json",
            )
            .await;
        if st == 0 {
            return (false, format!("Sem conexao com o SEFIN ({rede})."));
        }
        if st == 401 || st == 403 {
            return (false, format!("O SEFIN recusou o certificado (HTTP {st})."));
        }
        if corpo.is_none() {
            return (false, format!("Resposta inesperada do SEFIN (HTTP {st})."));
        }
        let mut frase = "Conexao com o SEFIN ok, certificado aceito.".to_string();
        if !t.avisos.is_empty() {
            frase.push(' ');
            frase.push_str(&t.avisos.join(" "));
        }
        (true, frase)
    }
}
