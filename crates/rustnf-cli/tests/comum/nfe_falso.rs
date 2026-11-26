// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O transporte falso da SEFAZ e as respostas montadas a mao, com documentos e nomes ficticios. Nenhum teste chama a rede.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use rustnf::nfe::sefaz::{FalhaRede, Transporte};

pub const NS: &str = "http://www.portalfiscal.inf.br/nfe";
pub const NPROT: &str = "135260000000001";

pub type Resposta = Box<dyn Fn(&str) -> Result<(u16, Vec<u8>), FalhaRede> + Send + Sync>;

/// Transporte com roteiro: cada chamada tira a proxima resposta, montada do pedido que chegou.
#[derive(Clone, Default)]
pub struct FalsoNfe(Arc<Mutex<VecDeque<(&'static str, Resposta)>>>);

impl FalsoNfe {
    pub fn novo(r: Vec<(&'static str, Resposta)>) -> Self {
        Self(Arc::new(Mutex::new(r.into())))
    }

    /// Poe mais uma resposta no fim do roteiro.
    pub fn empilhar(&self, servico: &'static str, resposta: Resposta) {
        self.0.lock().unwrap().push_back((servico, resposta));
    }

    pub fn restantes(&self) -> usize {
        self.0.lock().unwrap().len()
    }
}

#[async_trait]
impl Transporte for FalsoNfe {
    async fn enviar(
        &self,
        url: &str,
        corpo: Vec<u8>,
        _content_type: &str,
    ) -> Result<(u16, Vec<u8>), FalhaRede> {
        let texto = String::from_utf8(corpo).unwrap();
        let (espera, resp) = self
            .0
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| panic!("chamada fora do roteiro: {url}"));
        assert!(
            texto.contains(&format!("wsdl/{espera}\"")),
            "esperava {espera}: {texto}"
        );
        resp(&texto)
    }
}

pub fn entre<'a>(t: &'a str, a: &str, f: &str) -> &'a str {
    t.find(a).map_or("", |i| {
        let i = i + a.len();
        t[i..].find(f).map_or("", |j| &t[i..i + j])
    })
}

pub fn soap(ns: &str, ret: &str) -> Result<(u16, Vec<u8>), FalhaRede> {
    Ok((
        200,
        format!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?><soap:Envelope xmlns:soap=\"http://www.w3.org/2003/05/soap-envelope\">\
             <soap:Body><nfeResultMsg xmlns=\"{NS}/wsdl/{ns}\">{ret}</nfeResultMsg></soap:Body></soap:Envelope>"
        )
        .into_bytes(),
    ))
}

pub fn prot(chave: &str, cstat: &str, motivo: &str, nprot: &str) -> String {
    let np = if nprot.is_empty() {
        String::new()
    } else {
        format!("<nProt>{nprot}</nProt><digVal>qZk+NkcGgWq6PiVxeFDCbJzQ2J0=</digVal>")
    };
    format!(
        "<protNFe versao=\"4.00\"><infProt><tpAmb>2</tpAmb><verAplic>SP_NFE_PL_009_V4</verAplic><chNFe>{chave}</chNFe>\
         <dhRecbto>2026-10-03T09:00:05-03:00</dhRecbto>{np}<cStat>{cstat}</cStat><xMotivo>{motivo}</xMotivo></infProt></protNFe>"
    )
}

pub fn envio(cstat: &'static str, motivo: &'static str, nprot: &'static str) -> Resposta {
    Box::new(move |corpo| {
        let chave = entre(corpo, "Id=\"NFe", "\"");
        soap(
            "NFeAutorizacao4",
            &format!(
                "<retEnviNFe versao=\"4.00\" xmlns=\"{NS}\"><tpAmb>2</tpAmb><verAplic>SP</verAplic><cStat>104</cStat>\
                 <xMotivo>Lote processado</xMotivo><cUF>35</cUF><dhRecbto>2026-10-03T09:00:05-03:00</dhRecbto>{}</retEnviNFe>",
                prot(chave, cstat, motivo, nprot)
            ),
        )
    })
}

pub fn consulta(cstat: &'static str, motivo: &'static str, nprot: &'static str) -> Resposta {
    Box::new(move |corpo| {
        let chave = entre(corpo, "<chNFe>", "</chNFe>");
        let miolo = if nprot.is_empty() {
            String::new()
        } else {
            prot(chave, "100", "Autorizado o uso da NF-e", nprot)
        };
        soap(
            "NFeConsultaProtocolo4",
            &format!(
                "<retConsSitNFe versao=\"4.00\" xmlns=\"{NS}\"><tpAmb>2</tpAmb><verAplic>SP</verAplic><cStat>{cstat}</cStat>\
                 <xMotivo>{motivo}</xMotivo><cUF>35</cUF><dhRecbto>2026-10-03T09:00:20-03:00</dhRecbto><chNFe>{chave}</chNFe>{miolo}</retConsSitNFe>"
            ),
        )
    })
}

pub fn evento(cstat: &'static str, motivo: &'static str, nprot: &'static str) -> Resposta {
    Box::new(move |corpo| {
        let (chave, tipo, seq) = (
            entre(corpo, "<chNFe>", "</chNFe>"),
            entre(corpo, "<tpEvento>", "</tpEvento>"),
            entre(corpo, "<nSeqEvento>", "</nSeqEvento>"),
        );
        let np = if nprot.is_empty() {
            String::new()
        } else {
            format!("<nProt>{nprot}</nProt>")
        };
        soap(
            "NFeRecepcaoEvento4",
            &format!(
                "<retEnvEvento versao=\"1.00\" xmlns=\"{NS}\"><idLote>1</idLote><tpAmb>2</tpAmb><verAplic>SP</verAplic>\
                 <cOrgao>35</cOrgao><cStat>128</cStat><xMotivo>Lote de Evento Processado</xMotivo>\
                 <retEvento versao=\"1.00\"><infEvento><tpAmb>2</tpAmb><verAplic>SP</verAplic><cOrgao>35</cOrgao>\
                 <cStat>{cstat}</cStat><xMotivo>{motivo}</xMotivo><chNFe>{chave}</chNFe><tpEvento>{tipo}</tpEvento>\
                 <xEvento>Evento</xEvento><nSeqEvento>{seq}</nSeqEvento><dhRegEvento>2026-10-03T09:00:03-03:00</dhRegEvento>{np}\
                 </infEvento></retEvento></retEnvEvento>"
            ),
        )
    })
}

pub fn inutilizacao(nprot: &'static str) -> Resposta {
    Box::new(move |corpo| {
        let (serie, ini, fim) = (
            entre(corpo, "<serie>", "</serie>"),
            entre(corpo, "<nNFIni>", "</nNFIni>"),
            entre(corpo, "<nNFFin>", "</nNFFin>"),
        );
        soap(
            "NFeInutilizacao4",
            &format!(
                "<retInutNFe versao=\"4.00\" xmlns=\"{NS}\"><infInut><tpAmb>2</tpAmb><verAplic>SP</verAplic><cStat>102</cStat>\
                 <xMotivo>Inutilizacao de numero homologado</xMotivo><cUF>35</cUF><ano>26</ano><CNPJ>11222333000181</CNPJ>\
                 <mod>55</mod><serie>{serie}</serie><nNFIni>{ini}</nNFIni><nNFFin>{fim}</nNFFin>\
                 <dhRecbto>2026-10-03T09:00:03-03:00</dhRecbto><nProt>{nprot}</nProt></infInut></retInutNFe>"
            ),
        )
    })
}

pub fn status(cstat: &'static str, motivo: &'static str) -> Resposta {
    Box::new(move |_| {
        soap(
            "NFeStatusServico4",
            &format!(
                "<retConsStatServ versao=\"4.00\" xmlns=\"{NS}\"><tpAmb>2</tpAmb><verAplic>SP</verAplic><cStat>{cstat}</cStat>\
                 <xMotivo>{motivo}</xMotivo><cUF>35</cUF><dhRecbto>2026-10-03T09:00:30-03:00</dhRecbto></retConsStatServ>"
            ),
        )
    })
}

pub fn perdida() -> Resposta {
    Box::new(|_| Err(FalhaRede::Tempo))
}

// ---- o servidor com a NF-e, para os testes das rotas e das ferramentas ----

use axum::Router;
use chrono::{TimeZone, Utc};
use rustnf::nfe::sefaz::ClienteSefaz;
use rustnf_app::config::Config as ConfigApp;
use rustnf_app::nfe::config_sefaz;
use rustnf_app::notas::nfe::NotasNfe;
use rustnf_cli::http::nfe::dinamico;
use rustnf_cli::http::{roteador, Config, Estado, ServidorNfe, Token};
use serde_json::{json, Value};

use super::{a1, emissor_falso, Falso, Pasta, TOKEN};

/// A configuracao de um emitente ficticio de SP no Simples, com a pasta de notas dentro de `p`.
pub fn config_nfe(p: &Pasta) -> ConfigApp {
    let texto = format!(
        "[emitente]\ncnpj = \"11222333000181\"\nnome = \"LOJA DE TESTE LTDA\"\nmunicipio = \"3550308\"\n\
         [notas]\npasta = '{}'\n[nfe]\nuf = \"SP\"\nie = \"111111111111\"\ncrt = 1\n\
         [nfe.endereco]\nlogradouro = \"Rua das Flores\"\nnumero = \"100\"\nbairro = \"Centro\"\n\
         municipio = \"Sao Paulo\"\ncep = \"01001000\"\n",
        p.caminho("notas").to_string_lossy()
    );
    ConfigApp::de_texto(&texto).unwrap()
}

/// O pedido de uma NF-e de venda de cafe, sem emitente nem numero.
pub fn pedido_nfe() -> Value {
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

/// O servidor com emissao e a NF-e sobre o transporte falso com o `roteiro`. Devolve tambem o transporte (para contar o que
/// sobrou do roteiro) e a pasta de notas.
pub fn app_nfe(
    pasta: &Pasta,
    roteiro: Vec<(&'static str, Resposta)>,
) -> (Router, FalsoNfe, NotasNfe) {
    let config = config_nfe(pasta);
    let notas = NotasNfe::abrir(pasta.caminho("notas")).unwrap();
    let falso = FalsoNfe::novo(roteiro);
    let cliente =
        ClienteSefaz::com_transporte(config_sefaz(&config).unwrap(), dinamico(falso.clone()))
            .unwrap()
            .com_relogio(|| Utc.with_ymd_and_hms(2026, 10, 3, 12, 0, 0).unwrap())
            .com_dormir(|_| Box::pin(async {}));
    let nfe = ServidorNfe::com_cliente(config, Some(notas.clone()), cliente, a1());
    let estado = Estado::com_emissao(
        Config::default(),
        emissor_falso(Falso::default()),
        Token::novo(TOKEN.into()).unwrap(),
    )
    .com_nfe(nfe);
    (roteador(estado), falso, notas)
}

/// O servidor com emissao e SEM `[nfe]`: as acoes de efeito fiscal da NF-e dizem o que configurar.
pub fn app_nfe_sem_configuracao() -> Router {
    roteador(Estado::com_emissao(
        Config::default(),
        emissor_falso(Falso::default()),
        Token::novo(TOKEN.into()).unwrap(),
    ))
}

/// O servidor so' de leitura, com a configuracao da NF-e.
pub fn app_nfe_leitura(pasta: &Pasta) -> Router {
    let nfe = ServidorNfe::sem_emissao(
        config_nfe(pasta),
        Some(NotasNfe::abrir(pasta.caminho("notas")).unwrap()),
        "o servidor nao foi iniciado com --permitir-emissao",
    );
    roteador(Estado::somente_leitura(Config::default()).com_nfe(nfe))
}
