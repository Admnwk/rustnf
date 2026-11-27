// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A SEFAZ de mentira da NF-e: um transporte que responde por servico (status, autorizacao, consulta, evento e
//! inutilizacao) com respostas roteirizadas, sem abrir nenhuma conexao. E' o desenho do transporte falso da camada de
//! aplicacao (que e' so' de teste e a interface nao alcanca), em um arquivo so', usado pela demonstracao e pelos testes
//! da janela. Tambem semeia a pasta de ensaio: uma NF-e autorizada, uma cancelada e uma sem resposta (a que o
//! "Retomar" resolve).

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use chrono::{FixedOffset, Utc};
use rustnf::assinatura::cert_a1::{self, Certificado};
use rustnf::nfe::sefaz::{ClienteSefaz, FalhaRede, Transporte};
use rustnf_app::config::Config;
use rustnf_app::nfe::{self as app, Contexto};
use rustnf_app::notas::nfe::{NotasNfe, Situacao};
use serde_json::{json, Value};

use crate::modelo::nfe_produtos::{Cadastro, Produto};
use crate::servicos::{ClienteNfe, ErroTela, FabricaNfe, TransporteDin};

const NS: &str = "http://www.portalfiscal.inf.br/nfe";
const AUT: &str = "NFeAutorizacao4";
const CON: &str = "NFeConsultaProtocolo4";
const STA: &str = "NFeStatusServico4";
const EVT: &str = "NFeRecepcaoEvento4";
const INU: &str = "NFeInutilizacao4";
const SERVICOS: [&str; 5] = [AUT, CON, STA, EVT, INU];

/// O que a SEFAZ de mentira responde aos pedidos de evento e de inutilizacao.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoteiroNfe {
    /// Registra (o padrao).
    Registrado,
    /// Recusa: o cancelamento com 501 (prazo), a carta com 573 e a inutilizacao com 241.
    Rejeitado,
    /// A primeira tentativa fica sem resposta (a consulta da nota diz que nada foi registrado) e a segunda registra;
    /// a consulta da nota que esta sem resposta tambem falha na primeira vez.
    SemResposta,
}

/// O que a SEFAZ de mentira responde ao envio da NF-e (a emissao pela janela).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RoteiroEmissao {
    /// Autoriza (o padrao).
    #[default]
    Autorizada,
    /// A primeira tentativa e' recusada com um motivo comum (NCM inexistente, 778); a seguinte autoriza.
    Rejeitada,
    /// A primeira tentativa fica sem resposta e a consulta da mesma chave tambem; a retomada acha a nota autorizada.
    SemResposta,
    /// O servico esta parado (109): o status e o envio dizem isso e a consulta nao responde; so' o EPEC registra.
    Fora,
}

/// O roteiro de `--nfe-emissao`.
#[must_use]
pub fn roteiro_emissao_do_nome(nome: &str) -> Option<RoteiroEmissao> {
    match nome {
        "autorizada" => Some(RoteiroEmissao::Autorizada),
        "rejeitada" => Some(RoteiroEmissao::Rejeitada),
        "sem-resposta" => Some(RoteiroEmissao::SemResposta),
        "fora" => Some(RoteiroEmissao::Fora),
        _ => None,
    }
}

/// O roteiro de `--nfe-evento`.
#[must_use]
pub fn roteiro_nfe_do_nome(nome: &str) -> Option<RoteiroNfe> {
    match nome {
        "registrado" => Some(RoteiroNfe::Registrado),
        "rejeitado" => Some(RoteiroNfe::Rejeitado),
        "sem-resposta" => Some(RoteiroNfe::SemResposta),
        _ => None,
    }
}

/// O que o transporte faz neste momento.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Passo {
    /// A janela esta em uso: vale o roteiro.
    AoVivo,
    /// Semeando a pasta: tudo da certo.
    Autorizar,
    /// Semeando a nota sem resposta: autorizacao e consulta nao respondem.
    PerderTudo,
}

struct Estado {
    roteiro: RoteiroNfe,
    emissao: RoteiroEmissao,
    autorizacoes: u32,
    perdida: Option<String>,
    consulta_perdida: bool,
    passo: Passo,
    atraso: Duration,
    chamadas: Vec<&'static str>,
    tentativas: BTreeMap<String, u32>,
    pendente: Option<String>,
    protocolo: u64,
}

/// O transporte da SEFAZ de mentira. Quem o clona enxerga o mesmo estado.
#[derive(Clone)]
pub struct TransporteNfeDemonstracao {
    estado: Arc<Mutex<Estado>>,
}

impl TransporteNfeDemonstracao {
    /// O transporte com o roteiro e um atraso em cada resposta (para ver o "enviando" na tela).
    #[must_use]
    pub fn novo(roteiro: RoteiroNfe, atraso: Duration) -> Self {
        Self {
            estado: Arc::new(Mutex::new(Estado {
                roteiro,
                emissao: RoteiroEmissao::Autorizada,
                autorizacoes: 0,
                perdida: None,
                consulta_perdida: false,
                passo: Passo::AoVivo,
                atraso,
                chamadas: Vec::new(),
                tentativas: BTreeMap::new(),
                pendente: None,
                protocolo: 0,
            })),
        }
    }

    /// Troca o roteiro do envio da NF-e (a emissao pela janela).
    #[must_use]
    pub fn com_emissao(self, emissao: RoteiroEmissao) -> Self {
        self.com(|e| e.emissao = emissao);
        self
    }

    /// Os servicos chamados ate agora, na ordem (os testes conferem que nada saiu sem a marcacao).
    #[must_use]
    pub fn chamadas(&self) -> Vec<&'static str> {
        self.estado
            .lock()
            .map(|e| e.chamadas.clone())
            .unwrap_or_default()
    }

    /// Quantas chamadas foram feitas ao servico (`NFeRecepcaoEvento4`...).
    #[must_use]
    pub fn chamadas_de(&self, servico: &str) -> usize {
        self.chamadas().iter().filter(|s| **s == servico).count()
    }

    fn com<R>(&self, f: impl FnOnce(&mut Estado) -> R) -> Option<R> {
        self.estado.lock().ok().map(|mut e| f(&mut e))
    }

    fn passo(&self, p: Passo) {
        self.com(|e| e.passo = p);
    }

    /// A chave da nota que esta sem resposta: a consulta dela falha na primeira vez do roteiro `SemResposta`.
    pub fn marcar_pendente(&self, chave: &str) {
        self.com(|e| e.pendente = Some(chave.to_string()));
    }
}

fn entre<'a>(texto: &'a str, abre: &str, fecha: &str) -> &'a str {
    texto.find(abre).map_or("", |i| {
        let i = i + abre.len();
        texto[i..].find(fecha).map_or("", |f| &texto[i..i + f])
    })
}

fn soap(ns_wsdl: &str, ret: &str) -> Vec<u8> {
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?><soap:Envelope xmlns:soap=\"http://www.w3.org/2003/05/soap-envelope\" \
         xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"><soap:Body>\
         <nfeResultMsg xmlns=\"http://www.portalfiscal.inf.br/nfe/wsdl/{ns_wsdl}\">{ret}</nfeResultMsg>\
         </soap:Body></soap:Envelope>"
    )
    .into_bytes()
}

fn ok(ns_wsdl: &str, ret: &str) -> Result<(u16, Vec<u8>), FalhaRede> {
    Ok((200, soap(ns_wsdl, ret)))
}

fn prot(chave: &str, cstat: &str, motivo: &str, nprot: &str) -> String {
    format!(
        "<protNFe versao=\"4.00\"><infProt><tpAmb>2</tpAmb><verAplic>DEMO_NFE_1</verAplic>\
         <chNFe>{chave}</chNFe><dhRecbto>2026-10-03T09:00:05-03:00</dhRecbto><nProt>{nprot}</nProt>\
         <digVal>qZk+NkcGgWq6PiVxeFDCbJzQ2J0=</digVal><cStat>{cstat}</cStat><xMotivo>{motivo}</xMotivo></infProt></protNFe>"
    )
}

fn resposta_do_evento(
    corpo: &str,
    cstat: &str,
    motivo: &str,
    nprot: &str,
) -> Result<(u16, Vec<u8>), FalhaRede> {
    let chave = entre(corpo, "<chNFe>", "</chNFe>");
    let tipo = entre(corpo, "<tpEvento>", "</tpEvento>");
    let seq = entre(corpo, "<nSeqEvento>", "</nSeqEvento>");
    let np = if nprot.is_empty() {
        String::new()
    } else {
        format!("<nProt>{nprot}</nProt>")
    };
    let ret = format!(
        "<retEvento versao=\"1.00\"><infEvento><tpAmb>2</tpAmb><verAplic>DEMO_EVENTOS_1</verAplic>\
         <cOrgao>35</cOrgao><cStat>{cstat}</cStat><xMotivo>{motivo}</xMotivo><chNFe>{chave}</chNFe>\
         <tpEvento>{tipo}</tpEvento><xEvento>Evento de demonstracao</xEvento><nSeqEvento>{seq}</nSeqEvento>\
         <dhRegEvento>2026-10-03T09:00:03-03:00</dhRegEvento>{np}</infEvento></retEvento>"
    );
    ok(
        EVT,
        &format!(
            "<retEnvEvento versao=\"1.00\" xmlns=\"{NS}\"><idLote>1</idLote><tpAmb>2</tpAmb>\
             <verAplic>DEMO_EVENTOS_1</verAplic><cOrgao>35</cOrgao><cStat>128</cStat>\
             <xMotivo>Lote de Evento Processado</xMotivo>{ret}</retEnvEvento>"
        ),
    )
}

fn resposta_da_inutilizacao(
    corpo: &str,
    cstat: &str,
    motivo: &str,
    nprot: &str,
) -> Result<(u16, Vec<u8>), FalhaRede> {
    let (serie, ini, fim) = (
        entre(corpo, "<serie>", "</serie>"),
        entre(corpo, "<nNFIni>", "</nNFIni>"),
        entre(corpo, "<nNFFin>", "</nNFFin>"),
    );
    let (faixa, np) = if nprot.is_empty() {
        (String::new(), String::new())
    } else {
        (
            format!(
                "<ano>{}</ano><CNPJ>{}</CNPJ><mod>55</mod><serie>{serie}</serie><nNFIni>{ini}</nNFIni><nNFFin>{fim}</nNFFin>",
                entre(corpo, "<ano>", "</ano>"),
                entre(corpo, "<CNPJ>", "</CNPJ>"),
            ),
            format!("<nProt>{nprot}</nProt>"),
        )
    };
    ok(
        INU,
        &format!(
            "<retInutNFe versao=\"4.00\" xmlns=\"{NS}\"><infInut><tpAmb>2</tpAmb>\
             <verAplic>DEMO_NFE_1</verAplic><cStat>{cstat}</cStat><xMotivo>{motivo}</xMotivo>\
             <cUF>35</cUF>{faixa}<dhRecbto>2026-10-03T09:00:03-03:00</dhRecbto>{np}</infInut></retInutNFe>"
        ),
    )
}

impl Estado {
    fn novo_protocolo(&mut self) -> String {
        self.protocolo += 1;
        format!("1352600{:08}", self.protocolo)
    }

    /// Esta e' a primeira vez desta chave de pedido? So' o roteiro `SemResposta` perde a primeira.
    fn primeira_vez(&mut self, chave: &str) -> bool {
        let n = self.tentativas.entry(chave.to_string()).or_insert(0);
        *n += 1;
        *n == 1
    }

    fn responder(
        &mut self,
        servico: &'static str,
        corpo: &str,
    ) -> Result<(u16, Vec<u8>), FalhaRede> {
        let semeando = self.passo != Passo::AoVivo;
        let perde_a_primeira = |s: &mut Self, chave: &str| {
            s.roteiro == RoteiroNfe::SemResposta && !semeando && s.primeira_vez(chave)
        };
        match servico {
            STA => {
                let (cstat, motivo) = if self.emissao == RoteiroEmissao::Fora && !semeando {
                    ("109", "Servico Paralisado sem Previsao")
                } else {
                    ("107", "Servico em Operacao")
                };
                ok(
                    STA,
                    &format!(
                        "<retConsStatServ versao=\"4.00\" xmlns=\"{NS}\"><tpAmb>2</tpAmb><verAplic>DEMO_NFE_1</verAplic>\
                         <cStat>{cstat}</cStat><xMotivo>{motivo}</xMotivo><cUF>35</cUF>\
                         <dhRecbto>2026-10-03T09:00:30-03:00</dhRecbto></retConsStatServ>"
                    ),
                )
            }
            AUT => {
                if self.passo == Passo::PerderTudo {
                    return Err(FalhaRede::Tempo);
                }
                let chave = entre(corpo, "Id=\"NFe", "\"");
                if !semeando {
                    self.autorizacoes += 1;
                    match self.emissao {
                        RoteiroEmissao::SemResposta if self.autorizacoes == 1 => {
                            self.perdida = Some(chave.to_string());
                            return Err(FalhaRede::Tempo);
                        }
                        RoteiroEmissao::Rejeitada if self.autorizacoes == 1 => {
                            return ok(
                                AUT,
                                &format!(
                                    "<retEnviNFe versao=\"4.00\" xmlns=\"{NS}\"><tpAmb>2</tpAmb><verAplic>DEMO_NFE_1</verAplic>\
                                     <cStat>104</cStat><xMotivo>Lote processado</xMotivo><cUF>35</cUF>\
                                     <dhRecbto>2026-10-03T09:00:05-03:00</dhRecbto>{}</retEnviNFe>",
                                    prot(chave, "778", "Rejeicao: Informado NCM inexistente", "")
                                ),
                            );
                        }
                        RoteiroEmissao::Fora => {
                            return ok(
                                AUT,
                                &format!(
                                    "<retEnviNFe versao=\"4.00\" xmlns=\"{NS}\"><tpAmb>2</tpAmb><verAplic>DEMO_NFE_1</verAplic>\
                                     <cStat>109</cStat><xMotivo>Servico Paralisado sem Previsao</xMotivo><cUF>35</cUF>\
                                     <dhRecbto>2026-10-03T09:00:05-03:00</dhRecbto></retEnviNFe>"
                                ),
                            );
                        }
                        _ => {}
                    }
                }
                let nprot = self.novo_protocolo();
                ok(
                    AUT,
                    &format!(
                        "<retEnviNFe versao=\"4.00\" xmlns=\"{NS}\"><tpAmb>2</tpAmb><verAplic>DEMO_NFE_1</verAplic>\
                         <cStat>104</cStat><xMotivo>Lote processado</xMotivo><cUF>35</cUF>\
                         <dhRecbto>2026-10-03T09:00:05-03:00</dhRecbto>{}</retEnviNFe>",
                        prot(chave, "100", "Autorizado o uso da NF-e", &nprot)
                    ),
                )
            }
            CON => {
                if self.passo == Passo::PerderTudo {
                    return Err(FalhaRede::Tempo);
                }
                let chave = entre(corpo, "<chNFe>", "</chNFe>");
                if !semeando && self.emissao == RoteiroEmissao::Fora {
                    return Err(FalhaRede::Tempo);
                }
                if !semeando
                    && self.emissao == RoteiroEmissao::SemResposta
                    && self.perdida.as_deref() == Some(chave)
                    && !self.consulta_perdida
                {
                    self.consulta_perdida = true;
                    return Err(FalhaRede::Tempo);
                }
                if self.pendente.as_deref() == Some(chave)
                    && perde_a_primeira(self, &format!("consulta-{chave}"))
                {
                    return Err(FalhaRede::Tempo);
                }
                let nprot = self.novo_protocolo();
                ok(
                    CON,
                    &format!(
                        "<retConsSitNFe versao=\"4.00\" xmlns=\"{NS}\"><tpAmb>2</tpAmb><verAplic>DEMO_NFE_1</verAplic>\
                         <cStat>100</cStat><xMotivo>Autorizado o uso da NF-e</xMotivo><cUF>35</cUF>\
                         <dhRecbto>2026-10-03T09:00:20-03:00</dhRecbto><chNFe>{chave}</chNFe>{}</retConsSitNFe>",
                        prot(chave, "100", "Autorizado o uso da NF-e", &nprot)
                    ),
                )
            }
            EVT => {
                let tipo = entre(corpo, "<tpEvento>", "</tpEvento>");
                let chave = entre(corpo, "<chNFe>", "</chNFe>");
                let seq = entre(corpo, "<nSeqEvento>", "</nSeqEvento>");
                if perde_a_primeira(self, &format!("evento-{tipo}-{chave}-{seq}")) {
                    return Err(FalhaRede::Tempo);
                }
                if self.roteiro == RoteiroNfe::Rejeitado && !semeando {
                    return if tipo == "110111" {
                        resposta_do_evento(
                            corpo,
                            "501",
                            "Rejeicao: Prazo de Cancelamento superior ao previsto na Legislacao",
                            "",
                        )
                    } else {
                        resposta_do_evento(corpo, "573", "Rejeicao: Duplicidade de Evento", "")
                    };
                }
                let nprot = self.novo_protocolo();
                if tipo == "110140" {
                    // o EPEC so' existe sem a NF-e: a SEFAZ responde 136, nunca 135
                    return resposta_do_evento(
                        corpo,
                        "136",
                        "Evento registrado, mas nao vinculado a NF-e",
                        &nprot,
                    );
                }
                resposta_do_evento(corpo, "135", "Evento registrado e vinculado a NF-e", &nprot)
            }
            _ => {
                let faixa = format!(
                    "inutilizacao-{}-{}",
                    entre(corpo, "<nNFIni>", "</nNFIni>"),
                    entre(corpo, "<nNFFin>", "</nNFFin>")
                );
                if perde_a_primeira(self, &faixa) {
                    return Err(FalhaRede::Tempo);
                }
                if self.roteiro == RoteiroNfe::Rejeitado {
                    return resposta_da_inutilizacao(
                        corpo,
                        "241",
                        "Rejeicao: Um numero da faixa ja foi utilizado",
                        "",
                    );
                }
                let nprot = self.novo_protocolo();
                resposta_da_inutilizacao(corpo, "102", "Inutilizacao de numero homologada", &nprot)
            }
        }
    }
}

#[async_trait]
impl Transporte for TransporteNfeDemonstracao {
    async fn enviar(
        &self,
        _url: &str,
        corpo: Vec<u8>,
        _content_type: &str,
    ) -> Result<(u16, Vec<u8>), FalhaRede> {
        let texto = String::from_utf8_lossy(&corpo).into_owned();
        let servico = SERVICOS
            .iter()
            .find(|s| texto.contains(&format!("wsdl/{s}\"")))
            .copied();
        let atraso = self.com(|e| e.atraso).unwrap_or_default();
        if !atraso.is_zero() {
            tokio::time::sleep(atraso).await;
        }
        let Some(servico) = servico else {
            return Err(FalhaRede::Tempo);
        };
        self.com(|e| {
            e.chamadas.push(servico);
            e.responder(servico, &texto)
        })
        .unwrap_or(Err(FalhaRede::Tempo))
    }
}

/// A fabrica da demonstracao e dos testes: entrega o transporte de mentira e tira as esperas do cliente.
#[derive(Clone)]
pub struct FabricaNfeDemonstracao {
    transporte: TransporteNfeDemonstracao,
}

impl FabricaNfeDemonstracao {
    /// A fabrica sobre o transporte (a mesma instancia fica com quem quiser conferir as chamadas).
    #[must_use]
    pub fn nova(transporte: TransporteNfeDemonstracao) -> Arc<Self> {
        Arc::new(Self { transporte })
    }
}

impl FabricaNfe for FabricaNfeDemonstracao {
    fn transporte(
        &self,
        _certificado: &Certificado,
        _config: &Config,
    ) -> Result<Arc<dyn Transporte>, ErroTela> {
        Ok(Arc::new(self.transporte.clone()))
    }

    fn ajustar(&self, cliente: ClienteNfe) -> ClienteNfe {
        cliente.com_dormir(|_: Duration| Box::pin(async {}))
    }

    fn demonstracao(&self) -> bool {
        true
    }
}

/// As chaves das tres notas do ensaio.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotasDoEnsaio {
    /// A NF-e autorizada.
    pub autorizada: String,
    /// A NF-e cancelada.
    pub cancelada: String,
    /// A NF-e enviada sem resposta.
    pub pendente: String,
}

fn pedido(dias_atras: i64, destinatario: (&str, &str), item: (&str, u32, u32)) -> Value {
    let agora = Utc::now() - chrono::Duration::days(dias_atras) - chrono::Duration::minutes(3);
    let quando = match FixedOffset::west_opt(3 * 3600) {
        Some(brasilia) => agora
            .with_timezone(&brasilia)
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, false),
        None => agora.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
    };
    let (descricao, qtd, centavos) = item;
    let total = u64::from(qtd) * u64::from(centavos);
    let dinheiro = |c: u64| format!("{}.{:02}", c / 100, c % 100);
    json!({
        "emissao": quando,
        "nat_op": "VENDA",
        "dest": {"documento": destinatario.0, "nome": destinatario.1, "ind_ie_dest": 9,
                 "endereco": {"logradouro": "Rua das Acacias", "numero": "10", "bairro": "Centro",
                              "cod_municipio": "3550308", "municipio": "Sao Paulo", "uf": "SP"}},
        "ind_final": 1, "ind_pres": 1,
        "itens": [{"codigo": "1", "descricao": descricao, "ncm": "09012100", "cfop": "5102",
                   "unidade": "UN", "quantidade": qtd.to_string(), "valor_unitario": dinheiro(u64::from(centavos)),
                   "icms": {"codigo": "102"}, "pis": {"cst": "49"}, "cofins": {"cst": "49"}}],
        "pagamentos": [{"t_pag": "01", "valor": dinheiro(total)}]
    })
}

/// Poe na pasta de ensaio um cadastro de produtos (tres, com NCM), se ela ainda nao tem o arquivo. Sem pasta ou com o
/// arquivo ja la, nao faz nada.
fn semear_produtos(config: &Config) {
    let pasta = config.notas.pasta.trim();
    if pasta.is_empty() {
        return;
    }
    let arquivo = std::path::Path::new(pasta).join(crate::modelo::nfe_produtos::ARQUIVO);
    if arquivo.exists() {
        return;
    }
    let p = |codigo: &str, descricao: &str, valor: &str, ncm: &str| Produto {
        codigo: codigo.into(),
        descricao: descricao.into(),
        valor: valor.into(),
        ncm: ncm.into(),
        ..Produto::default()
    };
    let cadastro = Cadastro {
        produtos: vec![
            p("C1", "Cafe torrado 500g", "15,90", "09012100"),
            p("C2", "Cafe moido 250g", "12,00", "09012100"),
            p("F103", "Filtro de papel 103", "8,99", "48119090"),
        ],
    };
    let _ = rustnf_app::atomico::gravar(&arquivo, &cadastro.codificar());
}

/// Semeia a pasta de ensaio com uma NF-e autorizada, uma cancelada e uma sem resposta, pela mesma camada de aplicacao
/// que a janela usa (a SEFAZ e' a de mentira e o certificado, um de teste gerado na hora). Pasta que ja tem NF-e fica
/// como esta. O transporte volta ao roteiro normal e a nota sem resposta fica marcada nele.
///
/// # Errors
///
/// A frase do que falhou (configuracao da NF-e incompleta, pasta que nao abre...).
pub fn semear(
    config: &Config,
    transporte: &TransporteNfeDemonstracao,
) -> Result<NotasDoEnsaio, String> {
    semear_com(config, transporte, true)
}

/// Como [`semear`], escolhendo se a pasta ganha a NF-e sem resposta. Sem ela a pasta nao tem nota pendente, que e' o que a
/// contingencia EPEC exige (o roteiro `fora` da demonstracao).
///
/// # Errors
///
/// A frase do que falhou.
pub fn semear_com(
    config: &Config,
    transporte: &TransporteNfeDemonstracao,
    com_pendente: bool,
) -> Result<NotasDoEnsaio, String> {
    let notas =
        NotasNfe::abrir(config.notas.pasta.trim()).map_err(|_| "a pasta de notas nao abre")?;
    let existentes = notas.listar().map_err(|_| "a pasta de notas nao le")?;
    semear_produtos(config);
    let achar = |s: Situacao| {
        existentes
            .iter()
            .find(|r| r.situacao == s)
            .map(|r| r.chave.clone())
            .unwrap_or_default()
    };
    if !existentes.is_empty() {
        let n = NotasDoEnsaio {
            autorizada: achar(Situacao::Autorizada),
            cancelada: achar(Situacao::Cancelada),
            pendente: achar(Situacao::Pendente),
        };
        if !n.pendente.is_empty() {
            transporte.marcar_pendente(&n.pendente);
        }
        return Ok(n);
    }
    let documento = config.emitente.cnpj.clone();
    let pfx = cert_a1::gerar_teste(
        &documento,
        "EMPRESA DE ENSAIO",
        "ensaio",
        30,
        true,
        false,
        Utc::now(),
    )
    .map_err(|e| e.to_string())?;
    let certificado = cert_a1::carregar(&pfx, "ensaio").map_err(|e| e.to_string())?;
    let sefaz = app::config_sefaz(config).map_err(|e| e.to_string())?;
    let cliente =
        ClienteSefaz::com_transporte(sefaz, TransporteDin::de(Arc::new(transporte.clone())))
            .map_err(|e| e.to_string())?
            .com_dormir(|_: Duration| Box::pin(async {}));
    let ctx = Contexto::novo(&cliente, &certificado, config, Some(&notas));
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    let ctx = &ctx;
    let emitir =
        |p: Value| async move { app::emitir(ctx, p, false).await.map_err(|e| e.to_string()) };
    let semeou = rt.block_on(async {
        transporte.passo(Passo::Autorizar);
        let a = emitir(pedido(
            3,
            ("52998224725", "MARIA SOUZA DE TESTE"),
            ("Cafe torrado 500g", 2, 1590),
        ))
        .await?;
        let c = emitir(pedido(
            2,
            ("12345678909", "JOAO PEREIRA DE TESTE"),
            ("Filtro de papel 103", 1, 8990),
        ))
        .await?;
        app::cancelar(
            ctx,
            &c.chave,
            "Cancelamento de ensaio da demonstracao",
            None,
        )
        .await
        .map_err(|e| e.to_string())?;
        let pendente = if com_pendente {
            transporte.passo(Passo::PerderTudo);
            emitir(pedido(
                0,
                ("12345678909", "JOAO PEREIRA DE TESTE"),
                ("Cafe moido 250g", 3, 1200),
            ))
            .await?
            .chave
        } else {
            String::new()
        };
        Ok::<_, String>(NotasDoEnsaio {
            autorizada: a.chave,
            cancelada: c.chave,
            pendente,
        })
    });
    transporte.passo(Passo::AoVivo);
    let n = semeou?;
    if !n.pendente.is_empty() {
        transporte.marcar_pendente(&n.pendente);
    }
    // o que se contou semeando nao e' do uso da janela
    transporte.com(|e| {
        e.chamadas.clear();
        e.tentativas.clear();
    });
    Ok(n)
}
