// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Uma "Sefin" falsa para os testes da camada de aplicacao: o cliente real da biblioteca sobre um transporte que
//! nunca toca a rede. Tem notas, DPS, eventos e notas que nao respondem; e quem chamou o que, e quando.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use chrono::Utc;
use rustnf::assinatura::cert_a1;
use rustnf::chave_dfe;
use rustnf::nfse::compactar;
use rustnf::nfse::sefin::{ClienteNfse, FalhaRede, Transporte};
use serde_json::json;

use crate::notas::{Notas, Registro, Situacao};
use crate::testes_comuns::PastaTemporaria;

/// A chave de acesso da nota `n` (digito verificador certo).
pub(crate) fn chave(n: u64) -> String {
    chave_dfe::montar_nfse(3550308, "11222333000181", n, "2610", Some(1), 2).unwrap()
}

/// O Id da DPS de numero `n`, serie 1.
pub(crate) fn id_dps(n: u64) -> String {
    format!("DPS3550308211222333000181{:05}{:015}", 1, n)
}

/// A linha do indice da nota `n`.
pub(crate) fn registro(n: u64, situacao: Situacao, tomador: &str) -> Registro {
    Registro {
        chave: chave(n),
        numero: n.to_string(),
        serie: "1".into(),
        numero_dps: n.to_string(),
        emissao: "2026-10-02T10:00:00-03:00".into(),
        valor_servico: "150.00".into(),
        situacao,
        tomador_documento: "52998224725".into(),
        tomador_nome: tomador.into(),
    }
}

/// O XML de uma NFS-e que a camada de aplicacao le.
pub(crate) fn nfse_xml(ch: &str) -> String {
    format!(
        "<NFSe xmlns=\"http://www.sped.fazenda.gov.br/nfse\" versao=\"1.01\"><infNFSe Id=\"NFS{ch}\">\
         <nNFSe>77</nNFSe><cStat>100</cStat><ambGer>2</ambGer><valores><vLiq>150.00</vLiq></valores>\
         <DPS versao=\"1.01\"><infDPS Id=\"DPS1\"><tpAmb>2</tpAmb><serie>1</serie><nDPS>42</nDPS></infDPS>\
         </DPS></infNFSe></NFSe>"
    )
}

/// O XML de um evento registrado (`<evento>` com o pedRegEvento dentro).
pub(crate) fn evento_xml(ch: &str, tipo: &str, seq: u32) -> String {
    format!(
        "<evento xmlns=\"http://www.sped.fazenda.gov.br/nfse\" versao=\"1.01\"><infEvento Id=\"EVT{ch}{tipo}\">\
         <nSeqEvento>{seq}</nSeqEvento><dhProc>2026-11-25T10:00:05-03:00</dhProc>\
         <pedRegEvento versao=\"1.01\"><infPedReg Id=\"PRE{ch}{tipo}\"><dhEvento>2026-11-25T10:00:00-03:00</dhEvento>\
         <CNPJAutor>11222333000181</CNPJAutor><chNFSe>{ch}</chNFSe><e{tipo}><xDesc>evento</xDesc></e{tipo}>\
         </infPedReg></pedRegEvento></infEvento></evento>"
    )
}

/// O que a "Sefin" tem.
#[derive(Default)]
pub(crate) struct Dados {
    notas: HashSet<String>,
    /// DPS que geraram nota: Id -> chave.
    dps: HashMap<String, String>,
    /// (chave, tipo, sequencia)
    eventos: HashSet<(String, String, u32)>,
    /// o evento (chave, tipo, sequencia) volta como o de OUTRA nota
    trocados: HashSet<(String, String, u32)>,
    mudos: HashSet<String>,
    /// as respostas dos proximos `POST`, em ordem
    respostas_de_post: VecDeque<Result<(u16, Vec<u8>), FalhaRede>>,
}

/// O transporte falso.
#[derive(Clone)]
pub(crate) struct Falso {
    dados: Arc<Mutex<Dados>>,
    atraso: Duration,
    /// URL, quando comecou e quando terminou, de cada chamada
    pub(crate) chamadas: Arc<Mutex<Vec<(String, Instant, Instant)>>>,
    ativas: Arc<AtomicUsize>,
    /// o maximo de chamadas ao mesmo tempo
    pub(crate) simultaneas: Arc<AtomicUsize>,
}

impl Falso {
    pub(crate) fn novo() -> Self {
        Self {
            dados: Arc::default(),
            atraso: Duration::ZERO,
            chamadas: Arc::default(),
            ativas: Arc::default(),
            simultaneas: Arc::default(),
        }
    }

    pub(crate) fn com_nota(self, n: u64) -> Self {
        self.dados.lock().unwrap().notas.insert(chave(n));
        self
    }

    pub(crate) fn com_dps(self, n: u64) -> Self {
        self.dados.lock().unwrap().dps.insert(id_dps(n), chave(n));
        self
    }

    pub(crate) fn com_evento(self, n: u64, tipo: &str, seq: u32) -> Self {
        self.dados
            .lock()
            .unwrap()
            .eventos
            .insert((chave(n), tipo.into(), seq));
        self
    }

    pub(crate) fn com_evento_trocado(self, n: u64, tipo: &str, seq: u32) -> Self {
        let mut d = self.dados.lock().unwrap();
        d.eventos.insert((chave(n), tipo.into(), seq));
        d.trocados.insert((chave(n), tipo.into(), seq));
        drop(d);
        self
    }

    pub(crate) fn muda(self, n: u64) -> Self {
        self.dados.lock().unwrap().mudos.insert(chave(n));
        self
    }

    /// A consulta da DPS `n` nao tem resposta.
    pub(crate) fn muda_dps(self, n: u64) -> Self {
        self.dados.lock().unwrap().mudos.insert(id_dps(n));
        self
    }

    pub(crate) fn com_atraso(mut self, atraso: Duration) -> Self {
        self.atraso = atraso;
        self
    }

    /// A resposta do proximo `POST` (registro de evento).
    pub(crate) fn com_post(self, resposta: Result<(u16, Vec<u8>), FalhaRede>) -> Self {
        self.dados
            .lock()
            .unwrap()
            .respostas_de_post
            .push_back(resposta);
        self
    }

    /// A resposta de evento registrado do `POST`.
    pub(crate) fn evento_registrado(self, n: u64, tipo: &str) -> Self {
        let corpo = json!({"eventoXmlGZipB64": compactar(&evento_xml(&chave(n), tipo, 1))});
        self.com_post(Ok((201, corpo.to_string().into_bytes())))
    }

    /// As URLs chamadas, na ordem.
    pub(crate) fn urls(&self) -> Vec<String> {
        self.chamadas
            .lock()
            .unwrap()
            .iter()
            .map(|c| c.0.clone())
            .collect()
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
        let comeco = Instant::now();
        let agora = self.ativas.fetch_add(1, Ordering::SeqCst) + 1;
        self.simultaneas.fetch_max(agora, Ordering::SeqCst);
        if !self.atraso.is_zero() {
            tokio::time::sleep(self.atraso).await;
        }
        let resposta = self.responder(metodo, url);
        self.ativas.fetch_sub(1, Ordering::SeqCst);
        self.chamadas
            .lock()
            .unwrap()
            .push((format!("{metodo} {url}"), comeco, Instant::now()));
        resposta
    }
}

fn json_de(status: u16, v: &serde_json::Value) -> Result<(u16, Vec<u8>), FalhaRede> {
    Ok((status, v.to_string().into_bytes()))
}

fn nao_achado(o_que: &str) -> Result<(u16, Vec<u8>), FalhaRede> {
    json_de(
        404,
        &json!({"erro": {"codigo": "E404", "descricao": o_que}}),
    )
}

impl Falso {
    fn responder(&self, metodo: &str, url: &str) -> Result<(u16, Vec<u8>), FalhaRede> {
        let mut d = self.dados.lock().unwrap();
        if metodo == "POST" {
            return d
                .respostas_de_post
                .pop_front()
                .unwrap_or_else(|| nao_achado("sem resposta programada"));
        }
        let partes: Vec<&str> = url.split('/').collect();
        if let Some(i) = partes.iter().position(|p| *p == "dps") {
            if d.mudos.contains(partes[i + 1]) {
                return Err(FalhaRede::Tempo);
            }
            return match d.dps.get(partes[i + 1]) {
                Some(ch) => json_de(200, &json!({"chaveAcesso": ch, "idDps": partes[i + 1]})),
                None => nao_achado("sem DPS"),
            };
        }
        let i = partes.iter().position(|p| *p == "nfse").unwrap();
        let ch = partes[i + 1].to_string();
        if d.mudos.contains(&ch) {
            return Err(FalhaRede::Tempo);
        }
        if partes.len() > i + 2 {
            let pedido = (
                ch,
                partes[i + 3].to_string(),
                partes[i + 4].parse().unwrap(),
            );
            if !d.eventos.contains(&pedido) {
                return nao_achado("sem evento");
            }
            let da_nota = if d.trocados.contains(&pedido) {
                chave(999)
            } else {
                pedido.0.clone()
            };
            let xml = evento_xml(&da_nota, &pedido.1, pedido.2);
            return json_de(200, &json!({"eventoXmlGZipB64": compactar(&xml)}));
        }
        if d.notas.contains(&ch) {
            return json_de(
                200,
                &json!({"chaveAcesso": ch, "nfseXmlGZipB64": compactar(&nfse_xml(&ch))}),
            );
        }
        nao_achado("sem nota")
    }
}

/// O cliente real da Sefin, de ambiente 2, sobre o transporte falso.
pub(crate) fn emissor(falso: &Falso) -> ClienteNfse<Falso> {
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
    ClienteNfse::com_transporte(
        cert_a1::carregar(&pfx, "teste").unwrap(),
        2,
        None,
        falso.clone(),
    )
    .unwrap()
}

/// Uma pasta de notas temporaria com estas notas guardadas.
pub(crate) fn pasta_com(rotulo: &str, notas: &[Registro]) -> (PastaTemporaria, Notas) {
    let p = PastaTemporaria::nova(rotulo);
    let n = Notas::abrir(p.caminho()).unwrap();
    for r in notas {
        n.guardar(r, &nfse_xml(&r.chave)).unwrap();
    }
    (p, n)
}

/// A situacao da nota `n` na pasta.
pub(crate) fn situacao_de(notas: &Notas, n: u64) -> Situacao {
    notas.registro(&chave(n)).unwrap().situacao
}
