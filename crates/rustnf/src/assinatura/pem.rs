// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! PEM colado em campo de UMA linha -> PEM que o leitor aceita. Puro. Contrato: `tests/casos/pem.json`.
//!
//! O navegador tira as quebras do que se cola num `<input>` e a caixa de segredo as vira em espaco; o leitor estrito
//! recusa o bloco achatado (`rustls::pki_types::pem`, `reqwest::Certificate::from_pem_bundle`, `rsa`
//! `from_pkcs8_pem` = pem-rfc7468, o `push`) -- comprovado em `tests/pem.rs`.
//!
//! Saida = RFC 7468 sec. 2: cada bloco com o corpo sem espaco em linhas de 64 e `\n` no fim. Espaco = propriedade
//! Unicode White_Space (`char::is_whitespace`; inclui o NBSP). Bloco com caractere fora do base64 no corpo (cabecalho
//! RFC 1421 `Proc-Type:`) sai como veio. Texto fora dos blocos cai. Sem bloco inteiro: o texto sem os espacos das
//! pontas.

const INICIO: &str = "-----BEGIN ";
const TRACOS: &str = "-----";

fn base64_puro(s: &str) -> bool {
    s.bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'/' || b == b'=')
}

/// Cada bloco PEM do texto no formato da RFC 7468 (corpo em linhas de 64). PEM ja' certo sai igual.
#[must_use]
pub fn remontar(texto: &str) -> String {
    let mut saida = String::new();
    let mut resto = texto;
    while let Some(a) = resto.find(INICIO) {
        let depois = &resto[a + INICIO.len()..];
        let Some(f) = depois.find(TRACOS) else { break };
        let rotulo = depois[..f].trim();
        let corpo_e_resto = &depois[f + TRACOS.len()..];
        let fim = format!("-----END {rotulo}-----");
        let Some(b) = corpo_e_resto.find(&fim) else {
            break;
        };
        let bruto = &corpo_e_resto[..b];
        let corpo: String = bruto.chars().filter(|c| !c.is_whitespace()).collect();
        saida.push_str(INICIO);
        saida.push_str(rotulo);
        saida.push_str(TRACOS);
        if base64_puro(&corpo) {
            saida.push('\n');
            for linha in corpo.as_bytes().chunks(64) {
                // corpo so' tem ASCII (base64_puro): o pedaco e' UTF-8 valido
                saida.push_str(std::str::from_utf8(linha).unwrap_or_default());
                saida.push('\n');
            }
        } else {
            saida.push_str(bruto);
        }
        saida.push_str(&fim);
        saida.push('\n');
        resto = &corpo_e_resto[b + fim.len()..];
    }
    if saida.is_empty() {
        texto.trim().to_string()
    } else {
        saida
    }
}

/// Igual a [`remontar`] sobre bytes, lidos como latin-1 (ida e volta: byte fora do ASCII nao some nem quebra).
/// Para as entradas `ca_pem: Option<&[u8]>` dos clientes TLS.
#[must_use]
pub fn remontar_bytes(texto: &[u8]) -> Vec<u8> {
    let s: String = texto.iter().map(|&b| char::from(b)).collect();
    // a saida so' tem caracteres da entrada (<= U+00FF) e ASCII: o `as u8` nao perde nada
    remontar(&s)
        .chars()
        .map(|c| u8::try_from(u32::from(c)).unwrap_or(b'?'))
        .collect()
}
