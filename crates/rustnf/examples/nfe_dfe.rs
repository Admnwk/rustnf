// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Baixa as notas e os eventos em que o seu CNPJ ou CPF e' interessado (distribuicao de DF-e do Ambiente Nacional) e,
//! se pedido, manifesta uma nota recebida. **Chama o Ambiente Nacional de verdade** se nenhum endereco for dado: o
//! certificado e' o seu A1, a senha vem da variavel `SENHA_A1`.
//!
//! ```text
//! SENHA_A1=... cargo run -p rustnf --example nfe_dfe --features nfe_sefaz -- SP certificado.pfx CNPJ cursor.json
//! SENHA_A1=... cargo run -p rustnf --example nfe_dfe --features nfe_sefaz -- SP certificado.pfx CNPJ cursor.json manifestar CHAVE 210210
//! SENHA_A1=... cargo run -p rustnf --example nfe_dfe --features nfe_sefaz -- SP certificado.pfx CNPJ cursor.json manifestar CHAVE 210240 "Justificativa de 15 a 255 caracteres"
//! ```
//!
//! O terceiro argumento e' o CNPJ (ou CPF) do interessado e o quarto, o arquivo onde o cursor fica guardado entre uma
//! execucao e outra. A variavel `DFE_BASE` (por exemplo `http://127.0.0.1:4050`) troca os tres enderecos que o exemplo usa
//! (distribuicao, recepcao de evento e consulta da NF-e) pelos de um simulador local; sem ela, vale a tabela oficial.
//! Para outro transporte (proxy, fila), implemente `rustnf::nfe::sefaz::Transporte` e use
//! `ClienteSefaz::com_transporte`: o resto do codigo e' o mesmo.
//!
//! O que a aplicacao faz aqui e' o que a biblioteca pede dela: gravar, a cada lote, os documentos e o cursor novo juntos
//! (aqui, o cursor num arquivo; numa aplicacao de verdade, os dois na mesma transacao do banco).

use rustnf::assinatura::cert_a1;
use rustnf::nfe::sefaz::dfe::{Conteudo, Cursor, Doc};
use rustnf::nfe::sefaz::evento::{Manifestacao, TipoManifestacao};
use rustnf::nfe::sefaz::{ClienteSefaz, Config, Servico};
use rustnf::nfe::vigencias::Ambiente;

fn linha(d: &Doc) -> String {
    let o_que = match &d.conteudo {
        Conteudo::ResumoNfe(r) => format!("resumo da NF-e {} ({})", r.chave, r.nome),
        Conteudo::Nfe(n) => format!("NF-e completa {}", n.chave),
        Conteudo::ResumoEvento(r) => {
            format!("resumo do evento {} da NF-e {}", r.tp_evento, r.chave)
        }
        Conteudo::Evento(e) => format!("evento {} da NF-e {}", e.tp_evento, e.chave),
        Conteudo::OutroEsquema => format!("documento de outro esquema ({})", d.esquema),
        Conteudo::Ilegivel { motivo } => format!("ILEGIVEL, guarde o NSU e releia: {motivo}"),
        _ => "documento".to_string(),
    };
    format!("NSU {}: {o_que}", d.nsu)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut a = std::env::args().skip(1);
    let (Some(uf), Some(pfx), Some(documento), Some(arquivo_cursor)) =
        (a.next(), a.next(), a.next(), a.next())
    else {
        return Err("uso: nfe_dfe UF certificado.pfx CNPJ_OU_CPF cursor.json [manifestar CHAVE TIPO [JUSTIFICATIVA]]".into());
    };
    let senha = std::env::var("SENHA_A1")?;
    let certificado = cert_a1::carregar(&std::fs::read(pfx)?, &senha)?;

    let mut config = Config::novo(&uf, Ambiente::Homologacao);
    if let Ok(base) = std::env::var("DFE_BASE") {
        let base = base.trim_end_matches('/');
        for (servico, caminho) in [
            (
                Servico::DistribuicaoDfe,
                "NFeDistribuicaoDFe/NFeDistribuicaoDFe.asmx",
            ),
            (
                Servico::RecepcaoEvento,
                "NFeRecepcaoEvento4/NFeRecepcaoEvento4.asmx",
            ),
            (
                Servico::ConsultaProtocolo,
                "NFeConsultaProtocolo4/NFeConsultaProtocolo4.asmx",
            ),
        ] {
            config.urls.insert(servico, format!("{base}/{caminho}"));
        }
    }
    let cliente = ClienteSefaz::real(&certificado, config, None)?;

    if a.next().as_deref() == Some("manifestar") {
        let (Some(chave), Some(tp)) = (a.next(), a.next()) else {
            return Err("uso: ... manifestar CHAVE TIPO [JUSTIFICATIVA]".into());
        };
        let tipo =
            TipoManifestacao::de_codigo(&tp).ok_or("TIPO: 210200, 210210, 210220 ou 210240")?;
        let justificativa = a.next();
        let pedido = Manifestacao {
            documento: &documento,
            chave: &chave,
            tipo,
            justificativa: justificativa.as_deref(),
            situacao_nfe: None,
            anterior: None,
            // a biblioteca troca a UF do autor, o ambiente e a hora pelos do cliente
            uf_autor: &uf,
            ambiente: Ambiente::Homologacao,
            quando: chrono::Utc::now().fixed_offset(),
        };
        let r = cliente.manifestar(&certificado, &pedido).await?;
        println!("{}", r.frase);
        if r.registrado() {
            // guarde o procEventoNFe: e' o documento do evento
            println!("{}", r.proc_evento.as_deref().unwrap_or(""));
        } else if r.deve_consultar() {
            println!("consulte a NF-e de novo antes de qualquer reenvio");
        }
        return Ok(());
    }

    // o cursor guardado da vez anterior (vazio na primeira)
    let guardado = std::fs::read_to_string(&arquivo_cursor).unwrap_or_default();
    let cursor = Cursor::de_json(&guardado)?;
    let r = cliente
        .sincronizar(
            &documento,
            &cursor,
            |docs: &[Doc], novo: &Cursor| -> std::io::Result<()> {
                for d in docs {
                    println!("{}", linha(d));
                }
                // numa aplicacao de verdade: os documentos e o cursor na MESMA transacao
                std::fs::write(&arquivo_cursor, novo.para_json())
            },
            20,
        )
        .await
        .map_err(|e| format!("{e:?}"))?;
    println!("{}", r.frase);
    println!(
        "{} documento(s) em {} lote(s); cursor em {}",
        r.documentos.len(),
        r.lotes,
        r.cursor.ult_nsu
    );
    Ok(())
}
