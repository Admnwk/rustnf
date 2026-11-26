// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O cliente da SEFAZ montado a partir da configuracao: UF, ambiente, tempo limite, esperas do retorno, enderecos que
//! trocam os da tabela (simulador) e a raiz de confianca extra.

use std::path::Path;
use std::time::Duration;

use rustnf::assinatura::cert_a1::Certificado;
use rustnf::nfe::sefaz::{ClienteSefaz, Config as ConfigSefaz, Servico, TransporteReqwest};
use rustnf::nfe::vigencias::Ambiente;

use crate::config::Config;
use crate::erro::Erro;

/// O servico de cada nome de `[nfe.urls]`.
fn servico(nome: &str) -> Option<Servico> {
    Some(match nome {
        "status" => Servico::StatusServico,
        "autorizacao" => Servico::Autorizacao,
        "retorno" => Servico::RetAutorizacao,
        "consulta" => Servico::ConsultaProtocolo,
        "eventos" => Servico::RecepcaoEvento,
        "inutilizacao" => Servico::Inutilizacao,
        "distribuicao" => Servico::DistribuicaoDfe,
        _ => return None,
    })
}

/// A configuracao do cliente da biblioteca.
///
/// # Errors
///
/// [`Erro::Uso`] se `[nfe].uf` esta vazia (sem UF nao ha autorizador).
pub fn config_sefaz(config: &Config) -> Result<ConfigSefaz, Erro> {
    let n = &config.nfe;
    if n.uf.is_empty() {
        return Err(Erro::Uso(
            "informe a UF do emitente em [nfe].uf do arquivo de configuracao".into(),
        ));
    }
    let ambiente = if n.ambiente == "producao" {
        Ambiente::Producao
    } else {
        Ambiente::Homologacao
    };
    let mut c = ConfigSefaz::novo(&n.uf, ambiente);
    c.timeout = Duration::from_secs(n.tempo_limite_s);
    c.tentativas = n.tentativas_retorno as usize;
    c.intervalo = Duration::from_secs(n.intervalo_retorno_s);
    for (nome, url) in &n.urls {
        if let Some(s) = servico(nome) {
            c.urls.insert(s, url.clone());
        }
    }
    Ok(c)
}

/// A raiz de confianca extra de `[nfe].ca_pem`, se houver.
///
/// # Errors
///
/// [`Erro::Entrada`] se o arquivo nao existe ou nao abre (a mensagem nao cita o caminho).
pub fn ler_ca_pem(config: &Config) -> Result<Option<Vec<u8>>, Erro> {
    let caminho = &config.nfe.ca_pem;
    if caminho.is_empty() {
        return Ok(None);
    }
    std::fs::read(Path::new(caminho)).map(Some).map_err(|e| {
        Erro::Entrada(match e.kind() {
            std::io::ErrorKind::NotFound => {
                "a raiz de confianca ([nfe].ca_pem): arquivo nao encontrado".into()
            }
            _ => "nao foi possivel ler a raiz de confianca ([nfe].ca_pem)".to_string(),
        })
    })
}

/// O cliente real: `reqwest` com TLS mutuo pelo A1, as raizes da ICP-Brasil e a CA extra da configuracao.
///
/// # Errors
///
/// [`Erro::Uso`] sem UF, [`Erro::Entrada`] se a CA extra nao le e [`Erro::Recusa`] se o certificado nao serve para TLS.
pub fn cliente_real(
    certificado: &Certificado,
    config: &Config,
) -> Result<ClienteSefaz<TransporteReqwest>, Erro> {
    let ca = ler_ca_pem(config)?;
    Ok(ClienteSefaz::real(
        certificado,
        config_sefaz(config)?,
        ca.as_deref(),
    )?)
}
