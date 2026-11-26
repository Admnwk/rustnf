// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Consulta o status do servico da SEFAZ e, em homologacao, envia uma NF-e ja assinada (a saida de
//! `nfe::montagem::assinar`), cancela uma nota ou registra uma carta de correcao. **Chama a SEFAZ de verdade**: o
//! certificado e' o seu A1, a senha vem da variavel `SENHA_A1`.
//!
//! ```text
//! SENHA_A1=... cargo run -p rustnf --example nfe_sefaz --features nfe_sefaz -- SP certificado.pfx nfe_assinada.xml
//! SENHA_A1=... cargo run -p rustnf --example nfe_sefaz --features nfe_sefaz -- SP certificado.pfx cancelar CHAVE NPROT "Justificativa de 15 a 255 caracteres"
//! SENHA_A1=... cargo run -p rustnf --example nfe_sefaz --features nfe_sefaz -- SP certificado.pfx corrigir CHAVE SEQ "Texto da correcao, de 15 a 1000 caracteres"
//! ```
//!
//! Sem o terceiro argumento so' consulta o status. `NPROT` e' o protocolo da AUTORIZACAO da nota; `SEQ` vai de 1 a 20 e
//! a carta nova substitui as anteriores.

use rustnf::assinatura::cert_a1;
use rustnf::nfe::sefaz::{ClienteSefaz, Config, Desfecho, ResultadoEvento};
use rustnf::nfe::vigencias::Ambiente;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut a = std::env::args().skip(1);
    let (Some(uf), Some(pfx)) = (a.next(), a.next()) else {
        return Err("uso: nfe_sefaz UF certificado.pfx [nfe_assinada.xml]".into());
    };
    let senha = std::env::var("SENHA_A1")?;
    let certificado = cert_a1::carregar(&std::fs::read(pfx)?, &senha)?;
    let cliente = ClienteSefaz::real(&certificado, Config::novo(&uf, Ambiente::Homologacao), None)?;

    let status = cliente.status().await?;
    println!("status: {}", status.frase);

    match a.next().as_deref() {
        None => {}
        Some("cancelar") => {
            let (Some(chave), Some(nprot), Some(just)) = (a.next(), a.next(), a.next()) else {
                return Err("uso: ... cancelar CHAVE NPROT JUSTIFICATIVA".into());
            };
            let r = cliente
                .cancelar(&certificado, &chave, &nprot, &just)
                .await?;
            mostrar_evento(&r);
        }
        Some("corrigir") => {
            let (Some(chave), Some(seq), Some(texto)) = (a.next(), a.next(), a.next()) else {
                return Err("uso: ... corrigir CHAVE SEQ TEXTO".into());
            };
            let r = cliente
                .corrigir(&certificado, &chave, &texto, seq.parse()?)
                .await?;
            mostrar_evento(&r);
        }
        Some(arquivo) => {
            let resultado = cliente
                .autorizar(&std::fs::read_to_string(arquivo)?)
                .await?;
            println!("{}", resultado.frase);
            match resultado.desfecho {
                // guarde o nfeProc: e' o documento fiscal da nota
                Desfecho::Autorizada { .. } => println!("{}", resultado.nfe_proc().unwrap_or("")),
                _ if resultado.pode_reenviar() => println!("pode reenviar a mesma NF-e"),
                _ if resultado.deve_consultar() => {
                    println!("consulte a chave de novo: {}", resultado.chave)
                }
                _ => {}
            }
        }
    }
    Ok(())
}

fn mostrar_evento(r: &ResultadoEvento) {
    println!("{}", r.frase);
    if let Some(proc_evento) = &r.proc_evento {
        // guarde o procEventoNFe: e' o documento do evento
        println!("{proc_evento}");
    } else if r.deve_consultar() {
        println!(
            "consulte a NF-e de novo antes de qualquer reenvio: {}",
            r.chave
        );
    } else if r.pode_reenviar() {
        println!("pode reenviar o MESMO evento (mesmo nSeqEvento)");
    }
}
