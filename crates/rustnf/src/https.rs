// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Pecas comuns dos clientes HTTPS com certificado de cliente (mTLS): o cliente `reqwest` montado com o A1 em
//! memoria, a raiz de confianca extra (simulador local) e a classificacao da falha de rede.
//!
//! Usado pelo cliente da Sefin Nacional (`nfse::sefin`) e pelo da SEFAZ (`nfe::sefaz`). O certificado nunca vai para
//! o disco: [`Certificado::identidade_pem`] entrega o PEM em memoria, que o `reqwest` le e solta.

use std::time::Duration;

use crate::assinatura::cert_a1::Certificado;

/// Falha sem resposta HTTP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FalhaRede {
    /// Tempo esgotado.
    Tempo,
    /// Certificado recusado ou servidor nao confiavel.
    Tls(String),
    /// Conexao que nao abriu ou caiu.
    Rede(String),
}

impl std::fmt::Display for FalhaRede {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FalhaRede::Tempo => f.write_str("tempo esgotado"),
            FalhaRede::Tls(d) => write!(f, "TLS: {d}"),
            FalhaRede::Rede(d) => write!(f, "rede: {d}"),
        }
    }
}

/// O cliente HTTPS com o A1 como identidade: sem redirecionamento (a credencial nao segue redirect), conexao com
/// limite de 10 s, `agente` como `User-Agent`, as raizes do `webpki-roots` mais as `raizes` e mais a CA extra.
///
/// # Errors
///
/// Texto para a tela quando o certificado nao serve para TLS, a CA extra nao le ou o cliente nao monta.
pub(crate) fn cliente(
    cert: &Certificado,
    ca_pem: Option<&[u8]>,
    raizes: &[reqwest::Certificate],
    timeout: Duration,
    agente: &str,
) -> Result<reqwest::Client, String> {
    let id = reqwest::Identity::from_pem(&cert.identidade_pem())
        .map_err(|_| "certificado A1 nao serve para TLS".to_string())?;
    let mut b = reqwest::Client::builder()
        .identity(id)
        .timeout(timeout)
        .connect_timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(agente)
        .use_rustls_tls();
    for c in raizes {
        b = b.add_root_certificate(c.clone());
    }
    if let Some(ca) = ca_pem {
        for c in ca_extra(ca)? {
            b = b.add_root_certificate(c);
        }
    }
    b.build()
        .map_err(|_| "nao foi possivel montar o cliente HTTPS".to_string())
}

/// A falha de um `send()` do `reqwest`.
pub(crate) fn falha_do_envio(e: &reqwest::Error) -> FalhaRede {
    if e.is_timeout() {
        return FalhaRede::Tempo;
    }
    let t = format!("{e:?}");
    if t.contains("Certificate")
        || t.contains("tls")
        || t.contains("Tls")
        || t.contains("handshake")
    {
        FalhaRede::Tls("certificado recusado ou servidor nao confiavel".into())
    } else {
        FalhaRede::Rede(if e.is_connect() {
            "sem conexao".into()
        } else {
            "falha de transporte".into()
        })
    }
}

/// A falha ao ler o corpo da resposta.
pub(crate) fn falha_do_corpo(e: &reqwest::Error) -> FalhaRede {
    if e.is_timeout() {
        FalhaRede::Tempo
    } else {
        FalhaRede::Rede("corpo interrompido".into())
    }
}

/// CA extra: PEM colado achatado passa pelo `pem::remontar`; texto sem nenhum certificado e' erro (antes
/// `from_pem_bundle` podia devolver lista vazia e a CA sumia calada). So' espaco = sem CA extra.
pub(crate) fn ca_extra(ca: &[u8]) -> Result<Vec<reqwest::Certificate>, String> {
    if ca.trim_ascii().is_empty() {
        return Ok(Vec::new());
    }
    let v = reqwest::Certificate::from_pem_bundle(&crate::assinatura::pem::remontar_bytes(ca))
        .map_err(|_| "CA extra ilegivel".to_string())?;
    if v.is_empty() {
        return Err("CA extra ilegivel".to_string());
    }
    Ok(v)
}
