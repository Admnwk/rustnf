// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Busca do endereco pelo CEP, so' quando a pessoa pede (o CEP sai da maquina para um servico de terceiro).
//!
//! Primeiro o ViaCEP; se ele nao responder (rede, tempo, erro do servidor), a BrasilAPI. CEP inexistente no ViaCEP
//! encerra a busca. Os dois devolvem o codigo IBGE do municipio, que a DPS exige.

use std::time::Duration;

use serde_json::Value;

/// Endereco do ViaCEP.
pub const URL_VIACEP: &str = "https://viacep.com.br/ws";
/// Endereco da BrasilAPI (CEP, versao 1).
pub const URL_BRASILAPI: &str = "https://brasilapi.com.br/api/cep/v1";

/// O endereco achado pelo CEP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnderecoDoCep {
    /// Logradouro (pode vir vazio: CEP geral de municipio).
    pub logradouro: String,
    /// Bairro (pode vir vazio).
    pub bairro: String,
    /// Codigo IBGE do municipio, 7 digitos.
    pub municipio: String,
    /// Nome do municipio.
    pub cidade: String,
    /// UF.
    pub uf: String,
}

/// Por que a busca nao trouxe o endereco.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErroCep {
    /// O texto nao tem 8 digitos.
    Formato,
    /// O CEP nao existe.
    NaoEncontrado,
    /// Nenhum servico respondeu com um endereco legivel.
    Indisponivel,
}

impl std::fmt::Display for ErroCep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Formato => "cep: informe os 8 digitos",
            Self::NaoEncontrado => "cep: nao encontrado",
            Self::Indisponivel => "cep: servico de busca indisponivel; preencha o endereco a mao",
        })
    }
}

impl std::error::Error for ErroCep {}

/// So' os 8 digitos do CEP digitado (`01001-000` -> `01001000`).
///
/// # Errors
///
/// [`ErroCep::Formato`] se nao sobrarem exatamente 8 digitos.
pub fn cep_limpo(texto: &str) -> Result<String, ErroCep> {
    let digitos: String = texto.chars().filter(char::is_ascii_digit).collect();
    let so_digitos_e_traco = texto
        .chars()
        .all(|c| c.is_ascii_digit() || c == '-' || c == '.' || c.is_whitespace());
    if digitos.len() == 8 && so_digitos_e_traco {
        Ok(digitos)
    } else {
        Err(ErroCep::Formato)
    }
}

fn texto(v: &Value, chave: &str) -> String {
    v.get(chave)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn ibge_valido(s: &str) -> bool {
    s.len() == 7 && s.chars().all(|c| c.is_ascii_digit())
}

/// Le a resposta do ViaCEP (HTTP 200).
///
/// # Errors
///
/// [`ErroCep::NaoEncontrado`] com `"erro"` na resposta (vem como texto `"true"` ou booleano); [`ErroCep::Indisponivel`]
/// se nao for o JSON esperado ou faltar o codigo IBGE.
pub fn ler_viacep(corpo: &[u8]) -> Result<EnderecoDoCep, ErroCep> {
    let v: Value = serde_json::from_slice(corpo).map_err(|_| ErroCep::Indisponivel)?;
    if v.get("erro")
        .is_some_and(|e| e.as_bool() == Some(true) || e.as_str() == Some("true"))
    {
        return Err(ErroCep::NaoEncontrado);
    }
    let municipio = texto(&v, "ibge");
    if !ibge_valido(&municipio) {
        return Err(ErroCep::Indisponivel);
    }
    Ok(EnderecoDoCep {
        logradouro: texto(&v, "logradouro"),
        bairro: texto(&v, "bairro"),
        municipio,
        cidade: texto(&v, "localidade"),
        uf: texto(&v, "uf"),
    })
}

/// Le a resposta da BrasilAPI (CEP, versao 1).
///
/// # Errors
///
/// [`ErroCep::NaoEncontrado`] com HTTP 404 (a BrasilAPI nao distingue CEP inexistente de servicos fora do ar);
/// [`ErroCep::Indisponivel`] para o resto.
pub fn ler_brasilapi(status: u16, corpo: &[u8]) -> Result<EnderecoDoCep, ErroCep> {
    if status == 404 {
        return Err(ErroCep::NaoEncontrado);
    }
    if status != 200 {
        return Err(ErroCep::Indisponivel);
    }
    let v: Value = serde_json::from_slice(corpo).map_err(|_| ErroCep::Indisponivel)?;
    let municipio = v.get("ibge").map(|i| texto(i, "city")).unwrap_or_default();
    if !ibge_valido(&municipio) {
        return Err(ErroCep::Indisponivel);
    }
    Ok(EnderecoDoCep {
        logradouro: texto(&v, "street"),
        bairro: texto(&v, "neighborhood"),
        municipio,
        cidade: texto(&v, "city"),
        uf: texto(&v, "state"),
    })
}

async fn pedir(cliente: &reqwest::Client, url: &str) -> Option<(u16, Vec<u8>)> {
    let r = cliente.get(url).send().await.ok()?;
    let status = r.status().as_u16();
    let corpo = r.bytes().await.ok()?.to_vec();
    Some((status, corpo))
}

/// Busca o endereco do CEP: ViaCEP e, se ele nao responder, BrasilAPI. Precisa de um runtime do tokio.
///
/// # Errors
///
/// [`ErroCep`]: formato, CEP inexistente ou nenhum servico respondeu.
pub async fn buscar(cep: &str, tempo_limite: Duration) -> Result<EnderecoDoCep, ErroCep> {
    let cep = cep_limpo(cep)?;
    let cliente = reqwest::Client::builder()
        .timeout(tempo_limite)
        .user_agent(concat!("rustnf/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|_| ErroCep::Indisponivel)?;
    if let Some((200, corpo)) = pedir(&cliente, &format!("{URL_VIACEP}/{cep}/json/")).await {
        match ler_viacep(&corpo) {
            Ok(e) => return Ok(e),
            Err(ErroCep::NaoEncontrado) => return Err(ErroCep::NaoEncontrado),
            Err(_) => {}
        }
    }
    match pedir(&cliente, &format!("{URL_BRASILAPI}/{cep}")).await {
        Some((status, corpo)) => ler_brasilapi(status, &corpo),
        None => Err(ErroCep::Indisponivel),
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn cep_digitado_com_ou_sem_mascara() {
        assert_eq!(cep_limpo("01001-000").unwrap(), "01001000");
        assert_eq!(cep_limpo(" 01001000 ").unwrap(), "01001000");
        assert_eq!(cep_limpo("01.001-000").unwrap(), "01001000");
        assert_eq!(cep_limpo("0100100").unwrap_err(), ErroCep::Formato);
        assert_eq!(cep_limpo("01001000a").unwrap_err(), ErroCep::Formato);
        assert_eq!(cep_limpo("").unwrap_err(), ErroCep::Formato);
    }

    #[test]
    fn resposta_do_viacep() {
        let ok = br#"{"cep":"01001-000","logradouro":"Praca da Se","complemento":"lado impar","bairro":"Se","localidade":"Sao Paulo","uf":"SP","ibge":"3550308","gia":"1004","ddd":"11","siafi":"7107"}"#;
        let e = ler_viacep(ok).unwrap();
        assert_eq!(e.logradouro, "Praca da Se");
        assert_eq!(e.bairro, "Se");
        assert_eq!(e.municipio, "3550308");
        assert_eq!((e.cidade.as_str(), e.uf.as_str()), ("Sao Paulo", "SP"));
        // CEP inexistente: o ViaCEP responde 200 com "erro" em texto ou booleano
        assert_eq!(
            ler_viacep(br#"{"erro":"true"}"#).unwrap_err(),
            ErroCep::NaoEncontrado
        );
        assert_eq!(
            ler_viacep(br#"{"erro":true}"#).unwrap_err(),
            ErroCep::NaoEncontrado
        );
        // sem IBGE ou fora do formato: indisponivel (tenta o outro servico)
        assert_eq!(
            ler_viacep(br#"{"cep":"x","ibge":""}"#).unwrap_err(),
            ErroCep::Indisponivel
        );
        assert_eq!(ler_viacep(b"<html>").unwrap_err(), ErroCep::Indisponivel);
    }

    #[test]
    fn resposta_da_brasilapi() {
        let ok = br#"{"cep":"01001000","state":"SP","city":"Sao Paulo","neighborhood":"Se","street":"Praca da Se","service":"viacep","ibge":{"city":"3550308","state":"35"}}"#;
        let e = ler_brasilapi(200, ok).unwrap();
        assert_eq!(e.municipio, "3550308");
        assert_eq!(e.logradouro, "Praca da Se");
        assert_eq!(e.uf, "SP");
        assert_eq!(
            ler_brasilapi(404, br#"{"message":"x","type":"service_error"}"#).unwrap_err(),
            ErroCep::NaoEncontrado
        );
        assert_eq!(ler_brasilapi(500, b"").unwrap_err(), ErroCep::Indisponivel);
        assert_eq!(
            ler_brasilapi(200, br#"{"city":"x"}"#).unwrap_err(),
            ErroCep::Indisponivel
        );
    }

    /// Chama os servicos de verdade (rede): so' a mao, `cargo test -p rustnf-app --lib cep -- --ignored`.
    #[test]
    #[ignore = "usa a rede"]
    fn busca_de_verdade_de_um_cep_publico() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let e = rt
            .block_on(buscar("01001-000", Duration::from_secs(10)))
            .unwrap();
        assert_eq!(e.municipio, "3550308");
        assert_eq!(e.uf, "SP");
        assert_eq!(
            rt.block_on(buscar("99999999", Duration::from_secs(10)))
                .unwrap_err(),
            ErroCep::NaoEncontrado
        );
    }
}
