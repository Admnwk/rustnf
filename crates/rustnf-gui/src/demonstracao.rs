// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O emissor de demonstracao: implementa `Emissor` sem rede, para clicar na janela inteira (e para os testes da
//! janela) sem falar com a Sefin Nacional. A resposta "autorizada" e' uma NFS-e legivel pela camada de aplicacao,
//! montada a partir da propria DPS recebida: o registro na pasta de notas, o indice e a numeracao funcionam como na
//! emissao de verdade. So' existe com a feature `demonstracao` (e nos testes); o executavel normal nao tem como
//! entrar nesse modo.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use rustnf::chave_dfe;
use rustnf::nfse::base::esc;
use rustnf::nfse::leitura::{ler_nfse, Mensagem};
use rustnf::nfse::sefin::{Resultado, Status};
use rustnf::nfse::Dps;
use rustnf_app::emissao::Emissor;
use rustnf_app::notas::{Notas, Situacao};

use crate::servicos::{ErroTela, FabricaDeEmissor, Preparo};

/// Como o emissor de demonstracao responde a uma chamada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Desfecho {
    /// A Sefin autoriza a nota.
    Autorizada,
    /// A consulta acha a nota que ja existe com esta serie e este numero de DPS: a da PRIMEIRA chamada com eles
    /// (a mesma do pedido num reenvio; outra, se os dados mudaram depois de uma desistencia).
    Recuperada,
    /// A Sefin recusa a DPS, com duas mensagens.
    Rejeitada,
    /// Sem resposta: a nota nao e' confirmada.
    SemResposta,
}

/// Como o emissor de demonstracao responde a um pedido de cancelamento.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cancelamento {
    /// A Sefin registra o cancelamento.
    Registrado,
    /// A Sefin recusa o pedido, com duas mensagens.
    Rejeitado,
    /// Sem resposta: o cancelamento nao e' confirmado (e nao foi registrado).
    SemResposta,
}

/// Um pedido de cancelamento como o emissor o recebeu (para o teste comparar os reenvios).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PedidoDeCancelamento {
    /// A chave de acesso.
    pub chave: String,
    /// O codigo do motivo.
    pub motivo: String,
    /// A justificativa.
    pub justificativa: String,
}

/// O que a demonstracao responde alem da emissao: o roteiro do cancelamento e a pasta de notas do ensaio.
#[derive(Debug, Clone, Default)]
pub struct Ensaio {
    /// Um desfecho por pedido de cancelamento; o ultimo se repete. Vazio: registrado.
    pub cancelamento: Vec<Cancelamento>,
    /// A pasta de notas do ensaio, de onde a consulta tambem le.
    pub pasta: Option<PathBuf>,
}

/// O que o emissor recebeu em cada chamada (para o teste comparar os reenvios).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chamada {
    /// `nDPS`.
    pub numero: String,
    /// `serie`.
    pub serie: String,
    /// `dhEmi`, como chegou (o pedido congelado ja traz).
    pub emissao: Option<String>,
    /// `dCompet`.
    pub competencia: Option<String>,
    /// O nome do tomador.
    pub tomador: Option<String>,
    /// O valor do servico.
    pub valor: String,
}

/// O emissor de demonstracao: segue um roteiro de desfechos (um por chamada; o ultimo se repete).
pub struct EmissorDemonstracao {
    ambiente: u8,
    atraso: Duration,
    roteiro: Mutex<VecDeque<Desfecho>>,
    chamadas: Mutex<Vec<Chamada>>,
    /// A primeira DPS que chegou com cada serie e numero: e' a nota que "ja existe" numa recuperacao.
    primeiras: Mutex<HashMap<(String, String), Dps>>,
    /// As NFS-e que este emissor "autorizou" nesta execucao: chave e XML.
    autorizadas: Mutex<HashMap<String, String>>,
    /// A pasta de notas do ensaio: a consulta tambem acha as notas que estao nela.
    pasta: Option<PathBuf>,
    cancelamento: Mutex<VecDeque<Cancelamento>>,
    cancelamentos: Mutex<Vec<PedidoDeCancelamento>>,
    canceladas: Mutex<HashSet<String>>,
    consultas: Mutex<Vec<String>>,
}

impl EmissorDemonstracao {
    /// Um emissor para o `ambiente` (1 producao, 2 producao restrita), que responde segundo o `roteiro` depois de
    /// `atraso` (um pouco de espera faz o "enviando" ser visivel).
    #[must_use]
    pub fn novo(ambiente: u8, roteiro: &[Desfecho], atraso: Duration) -> Arc<Self> {
        Self::com_ensaio(ambiente, roteiro, Ensaio::default(), atraso)
    }

    /// Como [`EmissorDemonstracao::novo`], com o roteiro do cancelamento e a pasta de notas do ensaio.
    #[must_use]
    pub fn com_ensaio(
        ambiente: u8,
        roteiro: &[Desfecho],
        ensaio: Ensaio,
        atraso: Duration,
    ) -> Arc<Self> {
        let cancelamento = if ensaio.cancelamento.is_empty() {
            VecDeque::from([Cancelamento::Registrado])
        } else {
            ensaio.cancelamento.into_iter().collect()
        };
        Arc::new(Self {
            ambiente,
            atraso,
            roteiro: Mutex::new(roteiro.iter().copied().collect()),
            chamadas: Mutex::new(Vec::new()),
            primeiras: Mutex::new(HashMap::new()),
            autorizadas: Mutex::new(HashMap::new()),
            pasta: ensaio.pasta,
            cancelamento: Mutex::new(cancelamento),
            cancelamentos: Mutex::new(Vec::new()),
            canceladas: Mutex::new(HashSet::new()),
            consultas: Mutex::new(Vec::new()),
        })
    }

    /// Os pedidos de cancelamento que o emissor recebeu, na ordem.
    #[must_use]
    pub fn pedidos_de_cancelamento(&self) -> Vec<PedidoDeCancelamento> {
        self.cancelamentos
            .lock()
            .map(|c| c.clone())
            .unwrap_or_default()
    }

    /// As chaves que o emissor foi chamado para consultar, na ordem.
    #[must_use]
    pub fn chaves_consultadas(&self) -> Vec<String> {
        self.consultas.lock().map(|c| c.clone()).unwrap_or_default()
    }

    /// O XML de uma nota que este emissor autorizou nesta execucao, ou o da pasta de notas do ensaio.
    fn xml_da_nota(&self, chave: &str) -> Option<String> {
        if let Some(xml) = self
            .autorizadas
            .lock()
            .ok()
            .and_then(|a| a.get(chave).cloned())
        {
            return Some(xml);
        }
        let notas = Notas::abrir(self.pasta.as_ref()?).ok()?;
        notas.ler_xml(chave).ok()
    }

    /// A nota tem cancelamento registrado: nesta execucao, ou na pasta de notas do ensaio.
    fn esta_cancelada(&self, chave: &str) -> bool {
        if self.canceladas.lock().is_ok_and(|c| c.contains(chave)) {
            return true;
        }
        self.pasta
            .as_ref()
            .and_then(|p| Notas::abrir(p).ok())
            .and_then(|n| n.listar().ok())
            .is_some_and(|l| {
                l.iter()
                    .any(|r| r.chave == chave && r.situacao == Situacao::Cancelada)
            })
    }

    fn proximo_cancelamento(&self) -> Cancelamento {
        let Ok(mut roteiro) = self.cancelamento.lock() else {
            return Cancelamento::SemResposta;
        };
        if roteiro.len() > 1 {
            roteiro.pop_front().unwrap_or(Cancelamento::SemResposta)
        } else {
            roteiro
                .front()
                .copied()
                .unwrap_or(Cancelamento::SemResposta)
        }
    }

    async fn esperar(&self) {
        if !self.atraso.is_zero() {
            tokio::time::sleep(self.atraso).await;
        }
    }

    /// Guarda a NFS-e que a demonstracao acabou de "autorizar", para a consulta devolve-la nesta execucao.
    fn lembrar(&self, r: Resultado) -> Resultado {
        if r.status == Status::Autorizada && !r.chave.is_empty() && !r.xml_retorno.is_empty() {
            if let Ok(mut a) = self.autorizadas.lock() {
                a.insert(r.chave.clone(), r.xml_retorno.clone());
            }
        }
        r
    }

    /// Quantas vezes o emissor foi chamado.
    #[must_use]
    pub fn total_de_chamadas(&self) -> usize {
        self.chamadas.lock().map_or(0, |c| c.len())
    }

    /// O que cada chamada recebeu, na ordem.
    #[must_use]
    pub fn chamadas(&self) -> Vec<Chamada> {
        self.chamadas.lock().map(|c| c.clone()).unwrap_or_default()
    }

    /// A DPS que "ja existe" na serie e no numero desta: a primeira que chegou com eles (esta, se for a primeira).
    fn a_que_ja_existe(&self, dps: &Dps) -> Dps {
        let id = (
            dps.serie.as_str().trim().to_string(),
            dps.numero.as_str().trim().to_string(),
        );
        match self.primeiras.lock() {
            Ok(mut p) => p.entry(id).or_insert_with(|| dps.clone()).clone(),
            Err(_) => dps.clone(),
        }
    }

    fn proximo(&self) -> Desfecho {
        let Ok(mut roteiro) = self.roteiro.lock() else {
            return Desfecho::SemResposta;
        };
        if roteiro.len() > 1 {
            roteiro.pop_front().unwrap_or(Desfecho::SemResposta)
        } else {
            roteiro.front().copied().unwrap_or(Desfecho::SemResposta)
        }
    }
}

fn mensagem(codigo: &str, descricao: &str, complemento: &str) -> Mensagem {
    Mensagem {
        codigo: codigo.into(),
        descricao: descricao.into(),
        complemento: complemento.into(),
    }
}

fn resultado(status: Status, frase: &str) -> Resultado {
    Resultado {
        status,
        frase: frase.into(),
        chave: String::new(),
        id_dps: String::new(),
        nfse: None,
        xml_assinado: String::new(),
        xml_retorno: String::new(),
        erros: Vec::new(),
        alertas: Vec::new(),
        http: 0,
        recuperada: false,
        corpo: None,
        detalhe: String::new(),
    }
}

fn so_digitos(t: &str) -> String {
    t.chars().filter(char::is_ascii_digit).collect()
}

/// `CPF` para 11 caracteres; senao `CNPJ` (que pode ter letras: conta-se letra e digito).
fn tag_do_documento(documento: &str) -> &'static str {
    if documento
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .count()
        == 11
    {
        "CPF"
    } else {
        "CNPJ"
    }
}

/// A NFS-e autorizada para a DPS, como a Sefin a devolveria (so' o que a camada de aplicacao le).
fn nfse_autorizada(dps: &Dps, ambiente: u8) -> Option<(String, String)> {
    let numero: u64 = dps.numero.as_str().trim().parse().ok()?;
    let emissao = dps.emissao.clone().unwrap_or_default();
    let competencia = dps
        .competencia
        .clone()
        .filter(|c| c.len() >= 10)
        .or_else(|| emissao.get(..10).map(str::to_string))?;
    let aamm = format!("{}{}", competencia.get(2..4)?, competencia.get(5..7)?);
    let cmun: u64 = so_digitos(dps.cmun_emissor.as_str()).parse().ok()?;
    let documento = dps.prestador.pessoa.documento.clone();
    let chave =
        chave_dfe::montar_nfse(cmun, &documento, numero, &aamm, Some(123_456_789), 2).ok()?;
    let doc_tag = tag_do_documento(&documento);
    let tomador = dps.tomador.as_ref().map_or_else(String::new, |t| {
        let tag = tag_do_documento(&t.documento);
        let opcional = |nome: &str, valor: &str| {
            if valor.trim().is_empty() {
                String::new()
            } else {
                format!("<{nome}>{}</{nome}>", esc(valor.trim()))
            }
        };
        // o endereco nacional como a DPS o escreve (endNac, depois logradouro, numero, complemento e bairro)
        let endereco = t
            .endereco
            .as_ref()
            .filter(|e| !e.cmun.trim().is_empty())
            .map_or_else(String::new, |e| {
                format!(
                    "<end><endNac><cMun>{}</cMun><CEP>{}</CEP></endNac>{}{}{}{}</end>",
                    esc(e.cmun.trim()),
                    so_digitos(&e.cep),
                    opcional("xLgr", &e.logradouro),
                    opcional("nro", &e.numero),
                    opcional("xCpl", &e.complemento),
                    opcional("xBairro", &e.bairro),
                )
            });
        format!(
            "<toma><{tag}>{}</{tag}>{}<xNome>{}</xNome>{endereco}{}{}</toma>",
            esc(&t.documento),
            opcional("IM", &t.im),
            esc(&t.nome),
            opcional("fone", &so_digitos(&t.fone)),
            opcional("email", &t.email),
        )
    });
    let valor = dps.valores.servico.as_str();
    let id_da_dps = format!(
        "{cmun}2{documento:0>14}{:0>5}{numero:0>15}",
        dps.serie.as_str().trim()
    );
    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><NFSe xmlns=\"http://www.sped.fazenda.gov.br/nfse\" versao=\"1.00\">\
<infNFSe Id=\"NFS{chave}\"><xLocEmi>Demonstracao</xLocEmi><xLocPrestacao>Demonstracao</xLocPrestacao>\
<nNFSe>{numero}</nNFSe><cLocIncid>{cmun}</cLocIncid><xLocIncid>Demonstracao</xLocIncid>\
<xTribNac>{trib}</xTribNac><verAplic>demonstracao</verAplic><ambGer>2</ambGer><tpEmis>1</tpEmis><procEmi>1</procEmi>\
<cStat>100</cStat><dhProc>{emissao}</dhProc><nDFSe>{numero}</nDFSe>\
<emit><{doc_tag}>{documento}</{doc_tag}><IM>{im}</IM><xNome>{emitente}</xNome></emit>\
<valores><vBC>{valor}</vBC><vISSQN>0.00</vISSQN><vTotalRet>0.00</vTotalRet><vLiq>{valor}</vLiq></valores>\
<DPS xmlns=\"http://www.sped.fazenda.gov.br/nfse\" versao=\"1.00\"><infDPS Id=\"DPS{id}\">\
<tpAmb>{ambiente}</tpAmb><dhEmi>{emissao}</dhEmi><verAplic>demonstracao</verAplic><serie>{serie}</serie>\
<nDPS>{numero}</nDPS><dCompet>{competencia}</dCompet><tpEmit>1</tpEmit><cLocEmi>{cmun}</cLocEmi>{tomador}\
<serv><locPrest><cLocPrestacao>{cmun}</cLocPrestacao></locPrest><cServ><cTribNac>{ctribnac}</cTribNac>\
<xDescServ>{descricao}</xDescServ></cServ></serv>\
<valores><vServPrest><vServ>{valor}</vServ></vServPrest></valores></infDPS></DPS></infNFSe></NFSe>",
        trib = esc(rustnf::nfse::ctribnac::descricao(&so_digitos(dps.servico.ctribnac.as_str()))),
        documento = esc(&documento),
        im = esc(&dps.prestador.pessoa.im),
        emitente = esc(&dps.prestador.pessoa.nome),
        id = id_da_dps,
        serie = esc(dps.serie.as_str().trim()),
        ctribnac = esc(&so_digitos(dps.servico.ctribnac.as_str())),
        descricao = esc(&dps.servico.descricao),
    );
    Some((chave, xml))
}

fn autorizada(dps: &Dps, ambiente: u8, recuperada: bool) -> Resultado {
    let Some((chave, xml)) = nfse_autorizada(dps, ambiente) else {
        return resultado(
            Status::Erro,
            "A demonstracao nao conseguiu montar a NFS-e de resposta.",
        );
    };
    let mut r = resultado(
        Status::Autorizada,
        if recuperada {
            "NFS-e autorizada (a resposta do envio se perdeu; a nota foi achada pela consulta)."
        } else {
            "NFS-e autorizada."
        },
    );
    r.nfse = ler_nfse(&xml).ok();
    r.chave = chave;
    r.xml_retorno = xml;
    r.recuperada = recuperada;
    r.http = 201;
    r
}

#[async_trait]
impl Emissor for EmissorDemonstracao {
    fn ambiente(&self) -> u8 {
        self.ambiente
    }

    async fn emitir(&self, dps: &Dps) -> Resultado {
        if let Ok(mut c) = self.chamadas.lock() {
            c.push(Chamada {
                numero: dps.numero.as_str().to_string(),
                serie: dps.serie.as_str().to_string(),
                emissao: dps.emissao.clone(),
                competencia: dps.competencia.clone(),
                tomador: dps.tomador.as_ref().map(|t| t.nome.clone()),
                valor: dps.valores.servico.as_str().to_string(),
            });
        }
        let existente = self.a_que_ja_existe(dps);
        self.esperar().await;
        match self.proximo() {
            Desfecho::Autorizada => self.lembrar(autorizada(dps, self.ambiente, false)),
            Desfecho::Recuperada => self.lembrar(autorizada(&existente, self.ambiente, true)),
            Desfecho::Rejeitada => {
                let mut r = resultado(Status::Rejeitada, "A Sefin recusou a DPS (demonstracao).");
                r.http = 400;
                r.erros = vec![
                    mensagem(
                        "E0312",
                        "O município de incidência não administra esse código de tributação nacional na competência informada.",
                        "Demonstração: nenhuma regra real foi consultada.",
                    ),
                    mensagem(
                        "E0037",
                        "O município emissor não está ativo no sistema nacional neste ambiente.",
                        "",
                    ),
                ];
                r
            }
            Desfecho::SemResposta => {
                let mut r = resultado(
                    Status::Pendente,
                    "Sem resposta do SEFIN (demonstracao) e sem como confirmar agora.",
                );
                r.detalhe = "demonstracao".into();
                r
            }
        }
    }

    async fn consultar(&self, chave: &str) -> Resultado {
        let chave = so_alnum_maiusculo(chave);
        if let Ok(mut c) = self.consultas.lock() {
            c.push(chave.clone());
        }
        self.esperar().await;
        match self.xml_da_nota(&chave).as_deref().map(ler_nfse) {
            Some(Ok(nfse)) => {
                let mut r = resultado(Status::Autorizada, "NFS-e encontrada (demonstracao).");
                r.chave = chave;
                r.xml_retorno = nfse.xml.clone();
                r.nfse = Some(nfse);
                r.http = 200;
                r
            }
            _ => {
                let mut r = resultado(
                    Status::NaoEncontrada,
                    "A demonstracao nao conhece uma nota com esta chave.",
                );
                r.chave = chave;
                r.http = 404;
                r
            }
        }
    }

    async fn cancelar(&self, chave: &str, motivo: &str, justificativa: &str) -> Resultado {
        let chave = so_alnum_maiusculo(chave);
        if let Ok(mut c) = self.cancelamentos.lock() {
            c.push(PedidoDeCancelamento {
                chave: chave.clone(),
                motivo: motivo.to_string(),
                justificativa: justificativa.to_string(),
            });
        }
        self.esperar().await;
        let desfecho = self.proximo_cancelamento();
        let conhecida = self.xml_da_nota(&chave).is_some();
        let mut r = match desfecho {
            Cancelamento::Registrado if conhecida => {
                if let Ok(mut c) = self.canceladas.lock() {
                    c.insert(chave.clone());
                }
                let mut r = resultado(
                    Status::Registrado,
                    "Evento 101101 registrado (demonstracao).",
                );
                r.http = 201;
                r
            }
            Cancelamento::Registrado | Cancelamento::Rejeitado => {
                let mut r = resultado(
                    Status::Rejeitada,
                    "A Sefin recusou o evento (demonstracao).",
                );
                r.http = 400;
                r.erros = if conhecida {
                    vec![
                        mensagem(
                            "DEMO-C1",
                            "O prazo para cancelar esta nota diretamente no sistema já passou.",
                            "Demonstração: nenhuma regra real foi consultada.",
                        ),
                        mensagem(
                            "DEMO-C2",
                            "Peça a análise fiscal do cancelamento ao município.",
                            "",
                        ),
                    ]
                } else {
                    vec![mensagem(
                        "DEMO-C3",
                        "A demonstração não conhece uma nota com esta chave.",
                        "",
                    )]
                };
                r
            }
            Cancelamento::SemResposta => {
                let mut r = resultado(
                    Status::Pendente,
                    "Sem confirmacao do evento (demonstracao).",
                );
                r.detalhe = "demonstracao".into();
                r
            }
        };
        r.chave = chave;
        r
    }

    async fn cancelada(&self, chave: &str) -> Resultado {
        let chave = so_alnum_maiusculo(chave);
        self.esperar().await;
        let cancelada = self.esta_cancelada(&chave);
        let mut r = if cancelada {
            resultado(
                Status::Ok,
                "Evento 101101 registrado na nota (demonstracao).",
            )
        } else {
            resultado(
                Status::NaoEncontrada,
                "Nenhum cancelamento registrado (demonstracao).",
            )
        };
        r.chave = chave;
        r.http = if cancelada { 200 } else { 404 };
        r
    }
}

fn so_alnum_maiusculo(texto: &str) -> String {
    texto
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

/// A fabrica da demonstracao: entrega sempre o mesmo emissor, e diz que nada e' enviado de verdade.
pub struct FabricaDemonstracao {
    emissor: Arc<EmissorDemonstracao>,
}

impl FabricaDemonstracao {
    /// A fabrica que entrega `emissor`.
    #[must_use]
    pub fn nova(emissor: Arc<EmissorDemonstracao>) -> Arc<Self> {
        Arc::new(Self { emissor })
    }
}

impl FabricaDeEmissor for FabricaDemonstracao {
    fn criar(&self, _preparo: Preparo) -> Result<Arc<dyn Emissor>, ErroTela> {
        Ok(self.emissor.clone())
    }

    fn demonstracao(&self) -> bool {
        true
    }
}

/// O roteiro de um dos nomes aceitos por `--resultado`: `autorizada`, `rejeitada` ou `sem-resposta` (a primeira
/// tentativa fica sem resposta e a segunda recupera a nota).
#[must_use]
pub fn roteiro_do_nome(nome: &str) -> Option<Vec<Desfecho>> {
    match nome {
        "autorizada" => Some(vec![Desfecho::Autorizada]),
        "rejeitada" => Some(vec![Desfecho::Rejeitada]),
        "sem-resposta" => Some(vec![Desfecho::SemResposta, Desfecho::Recuperada]),
        _ => None,
    }
}

/// O roteiro de um dos nomes aceitos por `--cancelamento`: `registrado`, `rejeitado` ou `sem-resposta` (o primeiro
/// pedido fica sem resposta, e nao foi registrado; o segundo, "Tentar de novo", registra).
#[must_use]
pub fn cancelamento_do_nome(nome: &str) -> Option<Vec<Cancelamento>> {
    match nome {
        "registrado" => Some(vec![Cancelamento::Registrado]),
        "rejeitado" => Some(vec![Cancelamento::Rejeitado]),
        "sem-resposta" => Some(vec![Cancelamento::SemResposta, Cancelamento::Registrado]),
        _ => None,
    }
}

#[cfg(test)]
mod testes {
    use rustnf::nfse::sefin::Status;
    use rustnf_app::notas::Registro;

    use super::*;

    fn rodar<T>(futuro: impl std::future::Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(futuro)
    }

    fn dps() -> Dps {
        serde_json::from_value(serde_json::json!({
            "prestador": {"documento": "11222333000181", "nome": "OFICINA DE TESTE", "im": "123456", "op_simples": 1},
            "cmun_emissor": "3550308",
            "numero": 7,
            "serie": 1,
            "emissao": "2026-11-21T10:00:00-03:00",
            "competencia": "2026-11-21",
            "tomador": {"documento": "52998224725", "nome": "Maria & Filhos <ME>"},
            "servico": {"ctribnac": "140101", "descricao": "Revisão \"geral\""},
            "valores": {"servico": "100.50"}
        }))
        .unwrap()
    }

    #[test]
    fn a_resposta_autorizada_e_uma_nfse_que_a_camada_de_aplicacao_le() {
        rodar(async {
            let e = EmissorDemonstracao::novo(2, &[Desfecho::Autorizada], Duration::ZERO);
            let r = e.emitir(&dps()).await;
            assert_eq!(r.status, Status::Autorizada);
            assert_eq!(r.chave.len(), 50);
            chave_dfe::conferir(&r.chave).unwrap();
            let nfse = r.nfse.as_ref().expect("nfse lida");
            assert_eq!(nfse.numero, "7");
            assert_eq!(nfse.tomador_nome, "Maria & Filhos <ME>");
            assert_eq!(nfse.valor_servico, "100.50");
            let registro = Registro::da_nfse(nfse);
            assert_eq!(registro.chave, r.chave);
            assert_eq!(registro.numero_dps, "7");
            assert!(registro.emissao.starts_with("2026-11-21"));
            assert_eq!(e.total_de_chamadas(), 1);
            assert_eq!(e.chamadas()[0].numero, "7");
        });
    }

    #[test]
    fn o_roteiro_segue_a_ordem_e_repete_o_ultimo() {
        rodar(async {
            let e = EmissorDemonstracao::novo(
                2,
                &[Desfecho::SemResposta, Desfecho::Recuperada],
                Duration::ZERO,
            );
            assert_eq!(e.emitir(&dps()).await.status, Status::Pendente);
            let r = e.emitir(&dps()).await;
            assert_eq!(r.status, Status::Autorizada);
            assert!(r.recuperada);
            assert_eq!(e.emitir(&dps()).await.status, Status::Autorizada);
            assert_eq!(e.total_de_chamadas(), 3);
        });
    }

    #[test]
    fn rejeitada_traz_duas_mensagens_e_nenhuma_nfse() {
        rodar(async {
            let e = EmissorDemonstracao::novo(2, &[Desfecho::Rejeitada], Duration::ZERO);
            let r = e.emitir(&dps()).await;
            assert_eq!(r.status, Status::Rejeitada);
            assert_eq!(r.erros.len(), 2);
            assert!(r.nfse.is_none());
            assert!(r.xml_retorno.is_empty());
        });
    }

    #[test]
    fn os_nomes_do_roteiro() {
        assert_eq!(
            roteiro_do_nome("autorizada"),
            Some(vec![Desfecho::Autorizada])
        );
        assert_eq!(
            roteiro_do_nome("rejeitada"),
            Some(vec![Desfecho::Rejeitada])
        );
        assert_eq!(
            roteiro_do_nome("sem-resposta"),
            Some(vec![Desfecho::SemResposta, Desfecho::Recuperada])
        );
        assert_eq!(roteiro_do_nome("outra"), None);
    }

    #[test]
    fn os_nomes_do_roteiro_de_cancelamento() {
        assert_eq!(
            cancelamento_do_nome("registrado"),
            Some(vec![Cancelamento::Registrado])
        );
        assert_eq!(
            cancelamento_do_nome("rejeitado"),
            Some(vec![Cancelamento::Rejeitado])
        );
        assert_eq!(
            cancelamento_do_nome("sem-resposta"),
            Some(vec![Cancelamento::SemResposta, Cancelamento::Registrado])
        );
        assert_eq!(cancelamento_do_nome("outro"), None);
    }

    #[test]
    fn consulta_devolve_a_nota_autorizada_nesta_execucao_e_nao_acha_o_resto() {
        rodar(async {
            let e = EmissorDemonstracao::novo(2, &[Desfecho::Autorizada], Duration::ZERO);
            let nota = e.emitir(&dps()).await;
            let chave = nota.chave.clone();
            // a chave pode vir com espaco e em minuscula
            let r = e.consultar(&chave.to_lowercase()).await;
            assert_eq!(r.status, Status::Autorizada);
            assert_eq!(r.chave, chave);
            assert_eq!(r.nfse.as_ref().unwrap().numero, "7");
            assert!(r.xml_retorno.contains("<NFSe"));
            let outra = chave_dfe::montar_nfse(3_550_308, "11222333000181", 99, "2611", Some(1), 2)
                .unwrap();
            assert_eq!(e.consultar(&outra).await.status, Status::NaoEncontrada);
            assert_eq!(e.chaves_consultadas().len(), 2);
        });
    }

    #[test]
    fn consulta_tambem_le_a_pasta_de_notas_do_ensaio() {
        rodar(async {
            let pasta =
                std::env::temp_dir().join(format!("rustnf-gui-demo-{}", std::process::id()));
            let notas = Notas::abrir(&pasta).unwrap();
            let autora = EmissorDemonstracao::novo(2, &[Desfecho::Autorizada], Duration::ZERO);
            let nota = autora.emitir(&dps()).await;
            let n = nota.nfse.as_ref().unwrap();
            notas
                .guardar(&Registro::da_nfse(n), &nota.xml_retorno)
                .unwrap();
            let e = EmissorDemonstracao::com_ensaio(
                2,
                &[Desfecho::Autorizada],
                Ensaio {
                    cancelamento: vec![],
                    pasta: Some(pasta.clone()),
                },
                Duration::ZERO,
            );
            assert_eq!(e.consultar(&nota.chave).await.status, Status::Autorizada);
            assert_eq!(e.cancelada(&nota.chave).await.status, Status::NaoEncontrada);
            notas.marcar_cancelada(&nota.chave).unwrap();
            assert_eq!(e.cancelada(&nota.chave).await.status, Status::Ok);
            let _ = std::fs::remove_dir_all(&pasta);
        });
    }

    #[test]
    fn o_cancelamento_segue_o_roteiro_e_so_registra_nota_que_conhece() {
        rodar(async {
            let e = EmissorDemonstracao::com_ensaio(
                2,
                &[Desfecho::Autorizada],
                Ensaio {
                    cancelamento: cancelamento_do_nome("sem-resposta").unwrap(),
                    pasta: None,
                },
                Duration::ZERO,
            );
            let chave = e.emitir(&dps()).await.chave;
            let r = e.cancelar(&chave, "1", "justificativa de teste").await;
            assert_eq!(r.status, Status::Pendente);
            assert_eq!(e.cancelada(&chave).await.status, Status::NaoEncontrada);
            // o reenvio e' o mesmo pedido
            let r = e.cancelar(&chave, "1", "justificativa de teste").await;
            assert_eq!(r.status, Status::Registrado);
            assert_eq!(e.cancelada(&chave).await.status, Status::Ok);
            let pedidos = e.pedidos_de_cancelamento();
            assert_eq!(pedidos.len(), 2);
            assert_eq!(pedidos[0], pedidos[1]);
            // chave desconhecida: a demonstracao recusa
            let outra = chave_dfe::montar_nfse(3_550_308, "11222333000181", 98, "2611", Some(1), 2)
                .unwrap();
            let r = e.cancelar(&outra, "1", "justificativa de teste").await;
            assert_eq!(r.status, Status::Rejeitada);
            assert_eq!(r.erros.len(), 1);
            // rejeitado: duas mensagens
            let e = EmissorDemonstracao::com_ensaio(
                2,
                &[Desfecho::Autorizada],
                Ensaio {
                    cancelamento: cancelamento_do_nome("rejeitado").unwrap(),
                    pasta: None,
                },
                Duration::ZERO,
            );
            let chave = e.emitir(&dps()).await.chave;
            let r = e.cancelar(&chave, "9", "justificativa de teste").await;
            assert_eq!(r.status, Status::Rejeitada);
            assert_eq!(r.erros.len(), 2);
            assert_eq!(e.cancelada(&chave).await.status, Status::NaoEncontrada);
        });
    }
}
