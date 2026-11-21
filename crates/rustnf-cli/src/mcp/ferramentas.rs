// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! As ferramentas do MCP: definicao (nome, descricao, `inputSchema`) e execucao. Erro de regra fiscal volta como
//! resultado da ferramenta com `isError: true`, nao como erro de protocolo.

use base64::Engine;
use rustnf::nfse::Dps;
use serde::Deserialize;
use serde_json::{json, Value};

use super::schemas;
use crate::emissao;
use crate::erro::Erro;
use crate::http::Estado;
use crate::nucleo::{self, SituacaoNota};

/// Definicao de uma ferramenta.
pub struct Ferramenta {
    /// Nome (letras, digitos, `_`, `-` e `.`).
    pub nome: &'static str,
    /// Titulo para exibicao.
    pub titulo: &'static str,
    /// O que ela faz.
    pub descricao: &'static str,
    /// So' existe e so' executa com `--permitir-emissao` e o Bearer.
    pub emissao: bool,
    schema: fn() -> Value,
}

const FERRAMENTAS: [Ferramenta; 7] = [
    Ferramenta {
        nome: "calcular_cbs_ibs",
        titulo: "Previa do IBS e da CBS",
        descricao: "Calcula a previa da base de calculo e do IBS/CBS de um servico, como a calculadora da Receita Federal. E' previa: vale o que voltar na NFS-e autorizada.",
        emissao: false,
        schema: schemas::operacao,
    },
    Ferramenta {
        nome: "conferir_dps",
        titulo: "Conferir a DPS",
        descricao: "Confere a DPS contra as regras do Anexo I sem enviar nada. Recusa volta com isError e o campo recusado. Campo que a DPS nao conhece (nome errado) nao e' recusa: volta listado em campos_desconhecidos.",
        emissao: false,
        schema: schemas::dps,
    },
    Ferramenta {
        nome: "montar_dps",
        titulo: "Montar o XML da DPS",
        descricao: "Monta o XML da DPS (nao assinado) a partir do pedido.",
        emissao: false,
        schema: schemas::dps,
    },
    Ferramenta {
        nome: "gerar_danfse",
        titulo: "Gerar o DANFSe",
        descricao: "Gera o DANFSe em PDF (NT 008) a partir do XML da NFS-e autorizada. O PDF volta em base64 num recurso embutido.",
        emissao: false,
        schema: schemas::danfse,
    },
    Ferramenta {
        nome: "emitir_nfse",
        titulo: "Emitir a NFS-e",
        descricao: "Monta, assina com o certificado A1 do servidor e envia a DPS a Sefin Nacional. O emitente da configuracao do servidor completa o que o pedido nao traz; com pasta de notas configurada, o numero da DPS pode faltar (vale o proximo da serie) e a nota autorizada e' guardada. Operacao com efeito fiscal.",
        emissao: true,
        schema: schemas::dps_de_emissao,
    },
    Ferramenta {
        nome: "consultar_nfse",
        titulo: "Consultar a NFS-e",
        descricao: "Consulta uma NFS-e na Sefin Nacional pela chave de acesso, com o certificado A1 do servidor. Nao altera nada.",
        emissao: true,
        schema: schemas::consulta,
    },
    Ferramenta {
        nome: "cancelar_nfse",
        titulo: "Cancelar a NFS-e",
        descricao: "Registra o cancelamento (evento 101101) da NFS-e e, com pasta de notas configurada, marca a nota como cancelada nela. Operacao com efeito fiscal.",
        emissao: true,
        schema: schemas::cancelamento,
    },
];

/// Acha a ferramenta pelo nome.
#[must_use]
pub fn buscar(nome: &str) -> Option<&'static Ferramenta> {
    FERRAMENTAS.iter().find(|f| f.nome == nome)
}

/// Definicoes para `tools/list`, em ordem fixa; as de emissao so' com `com_emissao`.
#[must_use]
pub fn listar(com_emissao: bool) -> Vec<Value> {
    FERRAMENTAS
        .iter()
        .filter(|f| com_emissao || !f.emissao)
        .map(|f| {
            json!({
                "name": f.nome,
                "title": f.titulo,
                "description": f.descricao,
                "inputSchema": (f.schema)(),
            })
        })
        .collect()
}

fn resultado_texto(texto: String, erro: bool) -> Value {
    json!({"content": [{"type": "text", "text": texto}], "isError": erro})
}

fn resultado_estruturado(v: Value, erro: bool) -> Value {
    json!({
        "content": [{"type": "text", "text": v.to_string()}],
        "structuredContent": v,
        "isError": erro,
    })
}

fn falha(e: &Erro) -> Value {
    resultado_texto(e.to_string(), true)
}

fn argumentos<T: for<'de> Deserialize<'de>>(args: Value) -> Result<T, Erro> {
    Ok(serde_json::from_value(args)?)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PedidoDanfse {
    xml: String,
    #[serde(default)]
    situacao: Option<String>,
    #[serde(default)]
    canhoto: Option<bool>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PedidoConsulta {
    chave: String,
}

async fn danfse(args: Value) -> Result<Value, Erro> {
    let p: PedidoDanfse = argumentos(args)?;
    let situacao = p
        .situacao
        .as_deref()
        .map(SituacaoNota::ler)
        .transpose()?
        .unwrap_or_default();
    let canhoto = p.canhoto.unwrap_or(true);
    let xml = p.xml;
    let pdf = tokio::task::spawn_blocking(move || nucleo::gerar_danfse(&xml, situacao, canhoto))
        .await
        .map_err(|_| Erro::Interno)??;
    Ok(json!({
        "content": [
            {"type": "text", "text": format!("DANFSe gerado: {} bytes, application/pdf.", pdf.len())},
            {"type": "resource", "resource": {
                "uri": "urn:rustnf:danfse.pdf",
                "mimeType": "application/pdf",
                "blob": base64::engine::general_purpose::STANDARD.encode(&pdf),
            }},
        ],
        "isError": false,
    }))
}

async fn rodar(f: &Ferramenta, estado: &Estado, args: Value) -> Result<Value, Erro> {
    match f.nome {
        "calcular_cbs_ibs" => {
            let op: nucleo::Operacao = argumentos(args)?;
            // sem `publicadas` no pedido, valem as `[[aliquotas]]` da configuracao
            let v = nucleo::calcular_com(&op, &estado.aliquotas)?;
            let recusado = v["recusado"].as_bool().unwrap_or(false);
            Ok(resultado_estruturado(v, recusado))
        }
        "conferir_dps" => {
            let desconhecidos = nucleo::campos_desconhecidos(&args);
            let dps: Dps = argumentos(args)?;
            nucleo::conferir_dps(&dps)?;
            let mut v = nucleo::ok_simples();
            if !desconhecidos.is_empty() {
                v["campos_desconhecidos"] = json!(desconhecidos);
            }
            Ok(resultado_estruturado(v, false))
        }
        "montar_dps" => {
            let dps: Dps = argumentos(args)?;
            Ok(resultado_texto(nucleo::montar_dps(&dps)?, false))
        }
        "gerar_danfse" => danfse(args).await,
        "emitir_nfse" => {
            let em = estado.emissao.as_ref().ok_or(Erro::Interno)?;
            let v = estado.emitir(em, args).await?;
            Ok(resultado_estruturado(v, false))
        }
        "consultar_nfse" => {
            let em = estado.emissao.as_ref().ok_or(Erro::Interno)?;
            let p: PedidoConsulta = argumentos(args)?;
            let v = emissao::consultar(em.emissor.as_ref(), &p.chave).await?;
            Ok(resultado_estruturado(v, false))
        }
        "cancelar_nfse" => {
            let em = estado.emissao.as_ref().ok_or(Erro::Interno)?;
            let p: nucleo::PedidoCancelamento = argumentos(args)?;
            let v = estado.cancelar(em, &p).await?;
            Ok(resultado_estruturado(v, false))
        }
        _ => Err(Erro::Interno),
    }
}

/// Executa a ferramenta. Toda falha vira resultado com `isError: true`.
pub(super) async fn executar(f: &Ferramenta, estado: &Estado, args: Value) -> Value {
    rodar(f, estado, args).await.unwrap_or_else(|e| falha(&e))
}
