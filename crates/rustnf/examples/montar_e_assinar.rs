// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Monta uma DPS a partir do pedido em JSON, assina com um certificado A1 e confere a assinatura, sem rede.
//!
//! O certificado aqui e' gerado na hora, autoassinado, so' para o exemplo rodar sem arquivo nenhum: na emissao de
//! verdade use `cert_a1::carregar` com o `.pfx` e a senha do seu A1. O envio a Sefin Nacional fica de fora (feature
//! `sefin`, veja `nfse::sefin`).
//!
//! ```text
//! cargo run -p rustnf --example montar_e_assinar --features cert_a1_teste
//! ```

use chrono::Utc;
use rustnf::assinatura::cert_a1;
use rustnf::assinatura::xmldsig::{self, Algoritmo};
use rustnf::nfse::{montar, Dps};

/// O pedido no mesmo formato que a linha de comando le (`rustnf dps esquema` imprime o schema completo).
const PEDIDO: &str = r#"{
  "prestador": {
    "documento": "11222333000181",
    "nome": "OFICINA TESTE",
    "op_simples": 3,
    "apuracao_sn": 1,
    "im": "123456"
  },
  "servico": { "ctribnac": "140101", "descricao": "Revisao" },
  "valores": { "servico": "150", "total": { "percentual_sn": "4.50" } },
  "cmun_emissor": "3550308",
  "numero": 42,
  "tomador": { "nome": "Cliente", "documento": "52998224725" },
  "emissao": "2026-10-02T10:00:00",
  "competencia": "2026-10-01"
}"#;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. O pedido vira a DPS; `montar` confere as regras do leiaute antes de escrever o XML.
    let dps: Dps = serde_json::from_str(PEDIDO)?;
    let xml = montar(&dps)?;

    // 2. Um A1 de teste no lugar do seu certificado.
    let pfx = cert_a1::gerar_teste(
        "11222333000181",
        "OFICINA TESTE",
        "teste",
        365,
        true,
        false,
        Utc::now(),
    )?;
    let a1 = cert_a1::carregar(&pfx, "teste")?;

    // 3. Assina o `infDPS` (SHA-256, o padrao) e confere a assinatura.
    let assinada = xmldsig::assinar(
        &xml,
        &a1.chave,
        &a1.cert_der,
        "infDPS",
        Algoritmo::Sha256,
        None,
    )?;
    for r in xmldsig::conferir(&assinada) {
        println!(
            "assinatura {}: {} ({})",
            if r.ok { "valida" } else { "INVALIDA" },
            r.titular,
            r.algoritmo
        );
    }
    println!("\n{assinada}");
    Ok(())
}
