// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! As ferramentas da NFC-e do MCP. Cada uma chama o mesmo metodo de [`ServidorNfce`](crate::http::ServidorNfce) que a rota
//! `/nfce/` correspondente chama. Recusa fiscal volta como resultado com `isError: true` e o JSON completo da SEFAZ. Nenhuma
//! ferramenta grava, le ou apaga o CSC.

use base64::Engine;
use rustnf_app::nfce::danfce::Pedido as PedidoCupom;
use serde::Deserialize;
use serde_json::{json, Value};

use super::ferramentas::{argumentos, resultado_estruturado, Ferramenta};
use super::schemas_nfce as schemas;
use crate::erro::Erro;
use crate::http::nfce::separar_modo;
use crate::http::nfe::{PedidoCancelar, Saida};
use crate::http::Estado;

pub(super) const FERRAMENTAS: [Ferramenta; 7] = [
    Ferramenta {
        nome: "gerar_danfce",
        titulo: "Gerar o DANFC-e",
        descricao: "Gera o cupom DANFC-e da NFC-e autorizada (nfeProc) ou emitida em contingencia off-line: PDF de bobina ou, com formato=escpos, os bytes ESC/POS. O arquivo volta em base64 num recurso embutido; nao fala com impressora.",
        emissao: false,
        schema: schemas::danfce,
    },
    Ferramenta {
        nome: "emitir_nfce",
        titulo: "Emitir a NFC-e",
        descricao: "Monta, assina com o certificado A1 do servidor (com o QR Code da configuracao) e envia a NFC-e a SEFAZ, de forma sincrona. A nota e' gravada como pendente ANTES do envio: se a resposta se perde, ela segue pendente e nunca vira outra nota (use retomar_pendentes_nfce). Com offline=true emite em contingencia off-line, sem falar com a SEFAZ. Campo que o leiaute nao conhece, ou que fala em CSC, recusa o pedido antes do envio. Operacao com efeito fiscal.",
        emissao: true,
        schema: schemas::pedido,
    },
    Ferramenta {
        nome: "consultar_nfce",
        titulo: "Consultar a NFC-e",
        descricao: "Consulta a NFC-e na SEFAZ pela chave de acesso e atualiza a pasta de notas com o que ela responde (autorizada, denegada, cancelada). Nao emite nada.",
        emissao: true,
        schema: schemas::consulta,
    },
    Ferramenta {
        nome: "cancelar_nfce",
        titulo: "Cancelar a NFC-e",
        descricao: "Registra o cancelamento (evento 110111) da NFC-e autorizada e marca a nota como cancelada na pasta. Se o prazo nacional de 24 horas ja acabou, o resultado traz aviso_prazo, mas o pedido e' enviado e a SEFAZ decide. Operacao com efeito fiscal.",
        emissao: true,
        schema: schemas::cancelamento,
    },
    Ferramenta {
        nome: "status_sefaz_nfce",
        titulo: "Estado do servico da NFC-e",
        descricao: "Pergunta a SEFAZ da UF do emitente se o servico de autorizacao da NFC-e esta em operacao (a NFC-e nao tem SVC; por isso nao e' o status_sefaz da NF-e). Servico parado volta com isError. Nao altera nada.",
        emissao: true,
        schema: schemas::sem_argumentos,
    },
    Ferramenta {
        nome: "pendentes_nfce",
        titulo: "Listar as NFC-e pendentes",
        descricao: "Lista, sem rede, as NFC-e que ainda dependem da SEFAZ (off-line por transmitir e enviadas sem resposta) com o prazo que resta, e diz em pode_fechar_caixa se o caixa pode fechar (nada pendente) e em aviso o que fazer e ate quando.",
        emissao: true,
        schema: schemas::sem_argumentos,
    },
    Ferramenta {
        nome: "retomar_pendentes_nfce",
        titulo: "Retomar as NFC-e pendentes",
        descricao: "Consulta a SEFAZ pela MESMA chave de cada NFC-e que ficou sem resultado e transmite as emitidas off-line; nunca gera outra nota no lugar. O resultado diz o que foi feito e, em pode_fechar_caixa, se sobrou algo. Operacao com efeito fiscal.",
        emissao: true,
        schema: schemas::sem_argumentos,
    },
];

fn resultado(s: &Saida) -> Value {
    resultado_estruturado(s.corpo_com_erro(), s.erro.is_some())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PedidoDanfce {
    xml: String,
    #[serde(default)]
    formato: Option<String>,
    #[serde(default)]
    largura: Option<u8>,
    #[serde(default)]
    colunas: Option<u8>,
    #[serde(default)]
    resumido: bool,
    #[serde(default)]
    via_estabelecimento: bool,
    #[serde(default)]
    cancelada: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PedidoConsulta {
    chave: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SemArgumentos {}

async fn danfce(estado: &Estado, args: Value) -> Result<Value, Erro> {
    let p: PedidoDanfce = argumentos(args)?;
    let escpos = match p.formato.as_deref() {
        None | Some("pdf") => false,
        Some("escpos") => true,
        Some(_) => return Err(Erro::Entrada("formato: use pdf ou escpos".into())),
    };
    let pedido = PedidoCupom {
        largura: p.largura,
        colunas: p.colunas,
        resumido: p.resumido,
        via_estabelecimento: p.via_estabelecimento,
    };
    let bytes = estado
        .nfce
        .danfce(p.xml, pedido, p.cancelada, escpos)
        .await?;
    let (tipo, uri, nome) = if escpos {
        (
            "application/octet-stream",
            "urn:rustnf:danfce.bin",
            "ESC/POS",
        )
    } else {
        ("application/pdf", "urn:rustnf:danfce.pdf", "PDF")
    };
    Ok(json!({
        "content": [
            {"type": "text", "text": format!("DANFC-e gerado ({nome}): {} bytes, {tipo}.", bytes.len())},
            {"type": "resource", "resource": {
                "uri": uri,
                "mimeType": tipo,
                "blob": base64::engine::general_purpose::STANDARD.encode(&bytes),
            }},
        ],
        "isError": false,
    }))
}

/// Executa a ferramenta da NFC-e `nome`. O pedido com campo desconhecido ou de CSC e' recusado antes de qualquer envio.
pub(super) async fn rodar(nome: &str, estado: &Estado, args: Value) -> Result<Value, Erro> {
    let nfce = &estado.nfce;
    match nome {
        "gerar_danfce" => danfce(estado, args).await,
        "emitir_nfce" => {
            let (pedido, modo) = separar_modo(args)?;
            Ok(resultado(&nfce.emitir(pedido, true, modo).await?))
        }
        "consultar_nfce" => {
            let p: PedidoConsulta = argumentos(args)?;
            Ok(resultado(&nfce.consultar(&p.chave).await?))
        }
        "cancelar_nfce" => {
            let p: PedidoCancelar = argumentos(args)?;
            Ok(resultado(&nfce.cancelar(&p).await?))
        }
        "status_sefaz_nfce" => {
            let SemArgumentos {} = argumentos(args)?;
            Ok(resultado(&nfce.status().await?))
        }
        "pendentes_nfce" => {
            let SemArgumentos {} = argumentos(args)?;
            Ok(resultado_estruturado(nfce.pendentes()?, false))
        }
        "retomar_pendentes_nfce" => {
            let SemArgumentos {} = argumentos(args)?;
            Ok(resultado(&nfce.retomar().await?))
        }
        _ => Err(Erro::Interno),
    }
}
