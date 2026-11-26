// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Transporte do SOAP: o trait que o teste troca por um falso e o `reqwest` com mTLS do real.
//!
//! Os servidores das SEFAZ usam certificado de cadeia ICP-Brasil, que o `webpki-roots` nao tem (a Sefin Nacional usa
//! raiz publica; por isso o cliente dela nao precisa disto). As raizes vigentes da AC Raiz, do repositorio do ITI,
//! vao no binario (`dados/icp_brasil_raizes.crt`) e SO' este cliente as soma as do `webpki-roots`.

use std::time::Duration;

use async_trait::async_trait;

use crate::assinatura::cert_a1::Certificado;
pub use crate::https::FalhaRede;

/// Raizes vigentes da AC Raiz da ICP-Brasil (ITI); origem, validade e SHA-256 de cada uma no proprio arquivo e em
/// `docs/FONTES.md`.
const ICP_BRASIL: &str = include_str!("../../../dados/icp_brasil_raizes.crt");

/// Tamanho maximo de uma resposta: um retorno de consulta com eventos passa de 1 MB, nunca de 16.
const LIMITE_RESPOSTA: usize = 16 * 1024 * 1024;

/// Porta de transporte: um POST com o envelope SOAP 1.2 -> (status HTTP, corpo).
#[async_trait]
pub trait Transporte: Send + Sync {
    /// Envia o POST. `Err` so' quando nao houve resposta HTTP (tempo, TLS, rede); HTTP 500 com Fault e' resposta.
    async fn enviar(
        &self,
        url: &str,
        corpo: Vec<u8>,
        content_type: &str,
    ) -> Result<(u16, Vec<u8>), FalhaRede>;
}

/// `reqwest` 0.12 + rustls, mTLS com o A1 em memoria, sem redirecionamento. Confianca no servidor: as raizes do
/// `webpki-roots` mais as da ICP-Brasil embutidas e, se vier, a CA extra (simulador local).
pub struct TransporteReqwest {
    cliente: reqwest::Client,
}

impl TransporteReqwest {
    /// `ca_extra_pem` = raiz EXTRA de confianca (por exemplo, a de um simulador local).
    ///
    /// # Errors
    ///
    /// Texto para a tela: o certificado nao serve para TLS, a CA extra nao le ou o cliente nao monta.
    pub fn novo(
        cert: &Certificado,
        ca_extra_pem: Option<&[u8]>,
        timeout: Duration,
    ) -> Result<Self, String> {
        let raizes = reqwest::Certificate::from_pem_bundle(ICP_BRASIL.as_bytes())
            .map_err(|_| "raizes ICP-Brasil ilegiveis".to_string())?;
        let cliente = crate::https::cliente(
            cert,
            ca_extra_pem,
            &raizes,
            timeout,
            concat!("rustnf/", env!("CARGO_PKG_VERSION")),
        )?;
        Ok(TransporteReqwest { cliente })
    }
}

#[async_trait]
impl Transporte for TransporteReqwest {
    async fn enviar(
        &self,
        url: &str,
        corpo: Vec<u8>,
        content_type: &str,
    ) -> Result<(u16, Vec<u8>), FalhaRede> {
        let mut resp = self
            .cliente
            .post(url)
            .header("Content-Type", content_type)
            .header("Accept", "application/soap+xml, text/xml")
            .body(corpo)
            .send()
            .await
            .map_err(|e| crate::https::falha_do_envio(&e))?;
        let st = resp.status().as_u16();
        let mut b = Vec::new();
        while let Some(parte) = resp
            .chunk()
            .await
            .map_err(|e| crate::https::falha_do_corpo(&e))?
        {
            if b.len() + parte.len() > LIMITE_RESPOSTA {
                return Err(FalhaRede::Rede("resposta grande demais".into()));
            }
            b.extend_from_slice(&parte);
        }
        Ok((st, b))
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn as_raizes_embutidas_leem_e_sao_sete() {
        let v = reqwest::Certificate::from_pem_bundle(ICP_BRASIL.as_bytes()).map(|v| v.len());
        assert_eq!(v.ok(), Some(7));
    }
}
