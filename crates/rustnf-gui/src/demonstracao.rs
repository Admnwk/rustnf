// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O emissor de demonstracao: implementa `Emissor` sem rede, para clicar na janela inteira (e para os testes da
//! janela) sem falar com a Sefin Nacional. A resposta "autorizada" e' uma NFS-e legivel pela camada de aplicacao,
//! montada a partir da propria DPS recebida: o registro na pasta de notas, o indice e a numeracao funcionam como na
//! emissao de verdade. So' existe com a feature `demonstracao` (e nos testes); o executavel normal nao tem como
//! entrar nesse modo.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use rustnf::chave_dfe;
use rustnf::nfse::base::esc;
use rustnf::nfse::leitura::{ler_nfse, Mensagem};
use rustnf::nfse::sefin::{Resultado, Status};
use rustnf::nfse::Dps;
use rustnf_app::emissao::Emissor;

use crate::servicos::{ErroTela, FabricaDeEmissor, Preparo};

/// Como o emissor de demonstracao responde a uma chamada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Desfecho {
    /// A Sefin autoriza a nota.
    Autorizada,
    /// A resposta do envio se perde e a consulta acha a nota: autorizada e recuperada.
    Recuperada,
    /// A Sefin recusa a DPS, com duas mensagens.
    Rejeitada,
    /// Sem resposta: a nota nao e' confirmada.
    SemResposta,
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
}

impl EmissorDemonstracao {
    /// Um emissor para o `ambiente` (1 producao, 2 producao restrita), que responde segundo o `roteiro` depois de
    /// `atraso` (um pouco de espera faz o "enviando" ser visivel).
    #[must_use]
    pub fn novo(ambiente: u8, roteiro: &[Desfecho], atraso: Duration) -> Arc<Self> {
        Arc::new(Self {
            ambiente,
            atraso,
            roteiro: Mutex::new(roteiro.iter().copied().collect()),
            chamadas: Mutex::new(Vec::new()),
        })
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
    let doc_tag = if so_digitos(&documento).len() == 11 {
        "CPF"
    } else {
        "CNPJ"
    };
    let tomador = dps.tomador.as_ref().map_or_else(String::new, |t| {
        let tag = if so_digitos(&t.documento).len() == 11 {
            "CPF"
        } else {
            "CNPJ"
        };
        format!(
            "<toma><{tag}>{}</{tag}><xNome>{}</xNome></toma>",
            esc(&t.documento),
            esc(&t.nome)
        )
    });
    let valor = dps.valores.servico.as_str();
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
        trib = esc(&rustnf::nfse::ctribnac::descricao(&so_digitos(dps.servico.ctribnac.as_str()))),
        documento = esc(&documento),
        im = esc(&dps.prestador.pessoa.im),
        emitente = esc(&dps.prestador.pessoa.nome),
        id = format!("{cmun}2{documento:0>14}{:0>5}{numero:0>15}", dps.serie.as_str().trim()),
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
        if !self.atraso.is_zero() {
            tokio::time::sleep(self.atraso).await;
        }
        match self.proximo() {
            Desfecho::Autorizada => autorizada(dps, self.ambiente, false),
            Desfecho::Recuperada => autorizada(dps, self.ambiente, true),
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

    async fn consultar(&self, _chave: &str) -> Resultado {
        resultado(Status::NaoEncontrada, "A demonstracao nao consulta notas.")
    }

    async fn cancelar(&self, _chave: &str, _motivo: &str, _justificativa: &str) -> Resultado {
        resultado(Status::Erro, "A demonstracao nao cancela notas.")
    }
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
}
