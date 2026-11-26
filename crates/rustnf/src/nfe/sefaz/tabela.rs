// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Tabela de enderecos dos webservices da NF-e: autorizador -> ambiente -> servico -> URL e versao, e a relacao
//! UF -> autorizador.
//!
//! Os dados (`dados/nfe_ws.json`) saem das paginas "Relacao de Servicos Web" do Portal Nacional da NF-e (producao e
//! homologacao); a data da leitura e as duas paginas estao em [`fonte`] e em `docs/FONTES.md`. Servico que a pagina
//! lista e o cliente ainda nao usa (consulta de cadastro) fica na tabela para a parte seguinte.
//!
//! A pagina publica alguns enderecos com `?wsdl` no fim (GO, MT, PR e um de PE). O `?wsdl` e' o pedido da descricao do
//! servico, nao o endereco da chamada: [`Endereco::url_da_chamada`] o tira, e [`Endereco::url`] guarda o texto como a
//! pagina o publica.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::Deserialize;

use super::erro::ErroSefaz;
use crate::nfe::montagem::{uf_codigo, uf_sigla};
use crate::nfe::vigencias::Ambiente;

const TABELA: &str = include_str!("../../../dados/nfe_ws.json");
/// Os enderecos da NFC-e (modelo 65): o autorizador da UF e a tabela dos servicos da versao 4.00.
const TABELA_NFCE: &str = include_str!("../../../dados/nfce_ws.json");

/// Prefixo do espaco de nomes do WSDL de cada servico (MOC 7.00, Visao Geral, 4.2.2: o `nfeDadosMsg` leva o espaco de
/// nomes do servico que esta sendo chamado).
const NS_WSDL: &str = "http://www.portalfiscal.inf.br/nfe/wsdl/";

/// Os servicos que o cliente chama.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum Servico {
    /// `NfeStatusServico`: o servico esta no ar?
    StatusServico,
    /// `NFeAutorizacao`: envio do lote de NF-e.
    Autorizacao,
    /// `NFeRetAutorizacao`: resultado do lote pelo recibo.
    RetAutorizacao,
    /// `NfeConsultaProtocolo`: situacao da NF-e pela chave.
    ConsultaProtocolo,
    /// `RecepcaoEvento`: cancelamento, carta de correcao, EPEC e os eventos da Reforma Tributaria.
    RecepcaoEvento,
    /// `NfeInutilizacao`: inutilizacao de faixa de numeracao.
    Inutilizacao,
    /// `NFeDistribuicaoDFe`: distribuicao de DF-e de interesse, do Ambiente Nacional (NT 2014.002).
    DistribuicaoDfe,
}

impl Servico {
    /// Os quatro servicos da emissao, na ordem do fluxo (status, envio, recibo, consulta).
    pub const TODOS: [Servico; 4] = [
        Servico::StatusServico,
        Servico::Autorizacao,
        Servico::RetAutorizacao,
        Servico::ConsultaProtocolo,
    ];

    /// Os dois servicos que registram algo na nota ou na numeracao: eventos e inutilizacao.
    pub const DE_REGISTRO: [Servico; 2] = [Servico::RecepcaoEvento, Servico::Inutilizacao];

    /// O nome do servico na pagina "Relacao de Servicos Web".
    #[must_use]
    pub fn nome_na_pagina(self) -> &'static str {
        match self {
            Servico::StatusServico => "NfeStatusServico",
            Servico::Autorizacao => "NFeAutorizacao",
            Servico::RetAutorizacao => "NFeRetAutorizacao",
            Servico::ConsultaProtocolo => "NfeConsultaProtocolo",
            Servico::RecepcaoEvento => "RecepcaoEvento",
            Servico::Inutilizacao => "NfeInutilizacao",
            Servico::DistribuicaoDfe => "NFeDistribuicaoDFe",
        }
    }

    /// O espaco de nomes do WSDL do servico (versao 4.00 do leiaute), onde vai o `nfeDadosMsg`.
    #[must_use]
    pub fn namespace(self) -> String {
        format!("{NS_WSDL}{}", self.wsdl())
    }

    /// A acao SOAP (`action` do `Content-Type`): o espaco de nomes do servico mais o metodo.
    #[must_use]
    pub fn acao(self) -> String {
        let metodo = match self {
            Servico::StatusServico => "nfeStatusServicoNF",
            Servico::Autorizacao => "nfeAutorizacaoLote",
            Servico::RetAutorizacao => "nfeRetAutorizacaoLote",
            Servico::ConsultaProtocolo => "nfeConsultaNF",
            Servico::RecepcaoEvento => "nfeRecepcaoEvento",
            Servico::Inutilizacao => "nfeInutilizacaoNF",
            Servico::DistribuicaoDfe => "nfeDistDFeInteresse",
        };
        format!("{}/{metodo}", self.namespace())
    }

    fn wsdl(self) -> &'static str {
        match self {
            Servico::StatusServico => "NFeStatusServico4",
            Servico::Autorizacao => "NFeAutorizacao4",
            Servico::RetAutorizacao => "NFeRetAutorizacao4",
            Servico::ConsultaProtocolo => "NFeConsultaProtocolo4",
            Servico::RecepcaoEvento => "NFeRecepcaoEvento4",
            Servico::Inutilizacao => "NFeInutilizacao4",
            Servico::DistribuicaoDfe => "NFeDistribuicaoDFe",
        }
    }
}

/// Uma linha da pagina: a versao do leiaute e o endereco como foi publicado.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Endereco {
    /// Versao do servico (`4.00`).
    pub versao: String,
    /// O endereco como a pagina o publica (pode terminar em `?wsdl`).
    pub url: String,
}

impl Endereco {
    /// O endereco da chamada: o publicado, sem o `?wsdl` do fim.
    #[must_use]
    pub fn url_da_chamada(&self) -> &str {
        let u = self.url.trim();
        u.strip_suffix("?wsdl")
            .or_else(|| u.strip_suffix("?WSDL"))
            .unwrap_or(u)
    }
}

/// De onde a tabela veio.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Fonte {
    /// A pagina de producao.
    pub producao: String,
    /// A pagina de homologacao.
    pub homologacao: String,
    /// O dia da leitura (`AAAA-MM-DD`).
    pub lido_em: String,
}

#[derive(Deserialize)]
struct Autorizador {
    producao: BTreeMap<String, Endereco>,
    homologacao: BTreeMap<String, Endereco>,
}

#[derive(Deserialize)]
struct Arquivo {
    #[serde(rename = "_fonte")]
    fonte: Fonte,
    uf_autorizador: BTreeMap<String, String>,
    autorizadores: BTreeMap<String, Autorizador>,
}

fn arquivo() -> &'static Arquivo {
    static T: OnceLock<Arquivo> = OnceLock::new();
    T.get_or_init(|| ler_tabela(TABELA))
}

fn arquivo_nfce() -> &'static Arquivo {
    static T: OnceLock<Arquivo> = OnceLock::new();
    T.get_or_init(|| ler_tabela(TABELA_NFCE))
}

fn ler_tabela(texto: &str) -> Arquivo {
    match serde_json::from_str(texto) {
        Ok(a) => a,
        // o JSON e' embutido e conferido por teste: este ramo nao roda
        Err(_) => Arquivo {
            fonte: Fonte {
                producao: String::new(),
                homologacao: String::new(),
                lido_em: String::new(),
            },
            uf_autorizador: BTreeMap::new(),
            autorizadores: BTreeMap::new(),
        },
    }
}

/// As duas paginas de onde a tabela veio e o dia da leitura.
#[must_use]
pub fn fonte() -> &'static Fonte {
    &arquivo().fonte
}

/// A sigla da UF (maiuscula) a partir da sigla ou do codigo IBGE (`"35"`).
///
/// # Errors
///
/// [`ErroSefaz`] se a UF e' vazia ou desconhecida: sem UF nao ha autorizador, e nada cai em outro calado.
pub fn uf_valida(uf: &str) -> Result<&'static str, ErroSefaz> {
    let u = uf.trim().to_uppercase();
    let por_codigo = if !u.is_empty() && u.bytes().all(|b| b.is_ascii_digit()) {
        u.parse::<u32>().ok().and_then(uf_sigla)
    } else {
        None
    };
    let sigla = por_codigo.or_else(|| {
        uf_codigo(&u)?;
        arquivo()
            .uf_autorizador
            .get_key_value(&u)
            .map(|(k, _)| k.as_str())
    });
    sigla.ok_or_else(|| {
        let cortada: String = uf.chars().take(5).collect();
        ErroSefaz::novo(
            format!("UF '{cortada}' desconhecida: sem UF nao ha autorizador"),
            "uf",
        )
    })
}

/// O codigo IBGE da UF (a sigla ou o proprio codigo).
///
/// # Errors
///
/// [`ErroSefaz`] se a UF e' desconhecida.
pub fn codigo_da_uf(uf: &str) -> Result<u32, ErroSefaz> {
    let sigla = uf_valida(uf)?;
    uf_codigo(sigla).ok_or_else(|| ErroSefaz::novo("UF desconhecida", "uf"))
}

/// O autorizador da UF: ela mesma, `SVAN` ou `SVRS` (pagina "Relacao de Servicos Web").
///
/// # Errors
///
/// [`ErroSefaz`] se a UF e' desconhecida.
pub fn autorizador_da_uf(uf: &str) -> Result<&'static str, ErroSefaz> {
    let sigla = uf_valida(uf)?;
    arquivo()
        .uf_autorizador
        .get(sigla)
        .map(String::as_str)
        .ok_or_else(|| ErroSefaz::novo(format!("a tabela nao tem autorizador para {sigla}"), "uf"))
}

/// Os autorizadores da tabela (UF com webservice proprio, `SVAN`, `SVRS`, `SVC-AN`, `SVC-RS` e `AN`).
pub fn autorizadores() -> impl Iterator<Item = &'static str> {
    arquivo().autorizadores.keys().map(String::as_str)
}

/// Os servicos que o autorizador oferece no ambiente, pelo nome da pagina, em ordem alfabetica.
///
/// # Errors
///
/// [`ErroSefaz`] se o autorizador nao esta na tabela.
pub fn servicos(
    autorizador: &str,
    ambiente: Ambiente,
) -> Result<impl Iterator<Item = (&'static str, &'static Endereco)>, ErroSefaz> {
    let a = arquivo().autorizadores.get(autorizador).ok_or_else(|| {
        let cortado: String = autorizador.chars().take(12).collect();
        ErroSefaz::novo(
            format!("autorizador '{cortado}' fora da tabela"),
            "autorizador",
        )
    })?;
    let m = match ambiente {
        Ambiente::Producao => &a.producao,
        Ambiente::Homologacao => &a.homologacao,
    };
    Ok(m.iter().map(|(k, v)| (k.as_str(), v)))
}

/// O endereco de um servico, pelo nome que a pagina da.
///
/// # Errors
///
/// [`ErroSefaz`] se o autorizador nao esta na tabela ou nao oferece o servico no ambiente (nada se inventa).
pub fn endereco(
    autorizador: &str,
    ambiente: Ambiente,
    servico: &str,
) -> Result<&'static Endereco, ErroSefaz> {
    servicos(autorizador, ambiente)?
        .find(|(nome, _)| *nome == servico)
        .map(|(_, e)| e)
        .ok_or_else(|| {
            ErroSefaz::novo(
                format!("{servico} nao e' oferecido por {autorizador} neste ambiente"),
                "servico",
            )
        })
}

/// O endereco da chamada de um servico para a UF no ambiente: UF -> autorizador -> linha da tabela, sem o `?wsdl`.
///
/// # Errors
///
/// [`ErroSefaz`] se a UF e' desconhecida ou o servico nao existe no autorizador.
pub fn url(uf: &str, ambiente: Ambiente, servico: Servico) -> Result<String, ErroSefaz> {
    let aut = autorizador_da_uf(uf)?;
    url_do_autorizador(aut, ambiente, servico)
}

/// O endereco da chamada de um servico num autorizador pelo nome (`SVC-AN`, `SVC-RS`, `AN`, `SVRS`...), sem o `?wsdl`.
///
/// # Errors
///
/// [`ErroSefaz`] se o autorizador esta fora da tabela ou nao oferece o servico no ambiente.
pub fn url_do_autorizador(
    autorizador: &str,
    ambiente: Ambiente,
    servico: Servico,
) -> Result<String, ErroSefaz> {
    Ok(endereco(autorizador, ambiente, servico.nome_na_pagina())?
        .url_da_chamada()
        .to_string())
}

/// O autorizador da NFC-e da UF: ela mesma ou a `SVRS` (mapa de autorizadores do portal da SVRS).
///
/// # Errors
///
/// [`ErroSefaz`] se a UF e' desconhecida.
pub fn autorizador_nfce_da_uf(uf: &str) -> Result<&'static str, ErroSefaz> {
    let sigla = uf_valida(uf)?;
    arquivo_nfce()
        .uf_autorizador
        .get(sigla)
        .map(String::as_str)
        .ok_or_else(|| {
            ErroSefaz::novo(
                format!("a tabela da NFC-e nao tem autorizador para {sigla}"),
                "uf",
            )
        })
}

/// O endereco da chamada de um servico da NFC-e para a UF no ambiente: UF -> autorizador -> linha da tabela, sem o
/// `?wsdl`. A NFC-e usa tabela propria (os enderecos do modelo 65 sao outros que os do 55).
///
/// # Errors
///
/// [`ErroSefaz`] se a UF e' desconhecida ou o autorizador nao oferece o servico no ambiente.
pub fn url_nfce(uf: &str, ambiente: Ambiente, servico: Servico) -> Result<String, ErroSefaz> {
    let aut = autorizador_nfce_da_uf(uf)?;
    let a = arquivo_nfce().autorizadores.get(aut).ok_or_else(|| {
        ErroSefaz::novo(
            format!("autorizador '{aut}' fora da tabela da NFC-e"),
            "autorizador",
        )
    })?;
    let m = match ambiente {
        Ambiente::Producao => &a.producao,
        Ambiente::Homologacao => &a.homologacao,
    };
    m.get(servico.nome_na_pagina())
        .map(|e| e.url_da_chamada().to_string())
        .ok_or_else(|| {
            ErroSefaz::novo(
                format!(
                    "{} nao e' oferecido por {aut} para a NFC-e neste ambiente",
                    servico.nome_na_pagina()
                ),
                "servico",
            )
        })
}

/// Os autorizadores da tabela da NFC-e (as UF com webservice proprio e a `SVRS`).
pub fn autorizadores_nfce() -> impl Iterator<Item = &'static str> {
    arquivo_nfce().autorizadores.keys().map(String::as_str)
}
