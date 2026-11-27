// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! As ferramentas da NF-e do MCP. Cada uma chama o mesmo metodo de [`ServidorNfe`](crate::http::ServidorNfe) que a rota
//! `/nfe/` correspondente chama. Recusa fiscal volta como resultado com `isError: true` e o JSON completo da SEFAZ.

use base64::Engine;
use serde::Deserialize;
use serde_json::{json, Value};

use super::ferramentas::{argumentos, resultado_estruturado, Ferramenta};
use super::schemas_nfe as schemas;
use crate::erro::Erro;
use crate::http::nfe::{PedidoCancelar, PedidoCorrigir, PedidoInutilizar, Saida};
use crate::http::nfe_recebidas::{
    situacao_do_filtro, PedidoBaixar, PedidoManifestar, PedidoSincronizar,
};
use crate::http::Estado;

pub(super) const FERRAMENTAS: [Ferramenta; 15] = [
    Ferramenta {
        nome: "conferir_nfe",
        titulo: "Conferir a NF-e",
        descricao: "Confere o pedido da NF-e (modelo 55) contra as regras do MOC e das Notas Tecnicas, sem rede e sem gastar numero. Recusa volta com isError, o campo e a rejeicao que a SEFAZ devolveria; campo que o leiaute nao conhece tambem recusa.",
        emissao: false,
        schema: schemas::pedido,
    },
    Ferramenta {
        nome: "montar_nfe",
        titulo: "Montar o XML da NF-e",
        descricao: "Monta o XML da NF-e a partir do pedido. NUNCA assina (a assinatura e' da emissao): serve com ou sem certificado, nao envia nada e nao reserva o numero.",
        emissao: false,
        schema: schemas::pedido,
    },
    Ferramenta {
        nome: "gerar_danfe",
        titulo: "Gerar o DANFE",
        descricao: "Gera o DANFE em PDF a partir do XML da NF-e autorizada (nfeProc). O PDF volta em base64 num recurso embutido.",
        emissao: false,
        schema: schemas::danfe,
    },
    Ferramenta {
        nome: "emitir_nfe",
        titulo: "Emitir a NF-e",
        descricao: "Monta, assina com o certificado A1 do servidor e envia a NF-e a SEFAZ. O numero vem da pasta de notas; a nota e' gravada como pendente ANTES do envio, entao uma queda no meio nunca perde a chave (use retomar_pendentes_nfe). Campo que o leiaute nao conhece recusa o pedido antes do envio. Operacao com efeito fiscal.",
        emissao: true,
        schema: schemas::pedido,
    },
    Ferramenta {
        nome: "consultar_nfe",
        titulo: "Consultar a NF-e",
        descricao: "Consulta a NF-e na SEFAZ pela chave de acesso e atualiza a pasta de notas com o que ela responde (autorizada, denegada, cancelada). Nao emite nada.",
        emissao: true,
        schema: schemas::consulta,
    },
    Ferramenta {
        nome: "cancelar_nfe",
        titulo: "Cancelar a NF-e",
        descricao: "Registra o cancelamento (evento 110111) da NF-e autorizada e marca a nota como cancelada na pasta. Operacao com efeito fiscal.",
        emissao: true,
        schema: schemas::cancelamento,
    },
    Ferramenta {
        nome: "corrigir_nfe",
        titulo: "Corrigir a NF-e",
        descricao: "Registra a carta de correcao (evento 110110) da NF-e autorizada; a sequencia vem da pasta de notas. Operacao com efeito fiscal.",
        emissao: true,
        schema: schemas::correcao,
    },
    Ferramenta {
        nome: "inutilizar_nfe",
        titulo: "Inutilizar numeros da NF-e",
        descricao: "Inutiliza uma faixa de numeros da serie que nao serao mais usados. Operacao com efeito fiscal.",
        emissao: true,
        schema: schemas::inutilizacao,
    },
    Ferramenta {
        nome: "status_sefaz",
        titulo: "Estado do servico da SEFAZ",
        descricao: "Pergunta a SEFAZ da UF do emitente (ou a SVC) se o servico de autorizacao esta em operacao. Servico parado volta com isError. Nao altera nada.",
        emissao: true,
        schema: schemas::status,
    },
    Ferramenta {
        nome: "registrar_epec",
        titulo: "Registrar a NF-e em contingencia EPEC",
        descricao: "Emite a NF-e em contingencia EPEC: assina, registra o evento na SEFAZ e guarda a nota para a transmissao posterior (retomar_pendentes_nfe). Se a venda ja tinha uma NF-e sem resposta da SEFAZ, informe a chave dela em `substitui`: o EPEC sai com numero novo e a nota sem resposta passa a ser so' consultada (a retomada diz se cancela ou inutiliza). Operacao com efeito fiscal.",
        emissao: true,
        schema: schemas::pedido_epec,
    },
    Ferramenta {
        nome: "retomar_pendentes_nfe",
        titulo: "Retomar as NF-e pendentes",
        descricao: "Consulta a SEFAZ pela MESMA chave de cada nota que ficou sem resultado (e transmite as de EPEC); nunca gera outra nota no lugar. A nota que um EPEC substituiu so' e' consultada, nunca reenviada, e volta com `a_fazer`: cancelar (a SEFAZ a autorizou) ou inutilizar (a SEFAZ nao a tem). Operacao com efeito fiscal.",
        emissao: true,
        schema: schemas::sem_argumentos,
    },
    Ferramenta {
        nome: "sincronizar_recebidas",
        titulo: "Baixar os documentos recebidos",
        descricao: "Baixa do Ambiente Nacional (distribuicao de DF-e) as NF-e em que a empresa e destinataria e os eventos delas, do ultimo NSU ate o fim da fila, e grava na pasta de notas (nfe/recebidas) antes de andar o cursor. Respeita a espera de uma hora e o bloqueio 656: antes da hora responde quando tentar, sem chamar a rede. Antes de manifestar chega so o resumo de cada NF-e.",
        emissao: true,
        schema: schemas::sincronizacao,
    },
    Ferramenta {
        nome: "listar_recebidas",
        titulo: "Listar as NF-e recebidas",
        descricao: "Lista as NF-e recebidas que estao na pasta de notas (chave, emitente, valor, data e situacao: resumo, completa, ciencia, confirmada, desconhecida, nao_realizada, cancelada, denegada). So le a pasta: nao usa a rede.",
        emissao: true,
        schema: schemas::lista_recebidas,
    },
    Ferramenta {
        nome: "baixar_recebida",
        titulo: "Baixar uma NF-e recebida",
        descricao: "Baixa a NF-e de uma chave no Ambiente Nacional. So a nota completa vem para quem ja manifestou; com ciencia=true a ciencia da operacao (evento fiscal 210210) e registrada antes, se a nota ainda nao tem manifestacao. Operacao com efeito fiscal quando ciencia=true.",
        emissao: true,
        schema: schemas::baixa,
    },
    Ferramenta {
        nome: "manifestar_recebida",
        titulo: "Manifestar o destinatario",
        descricao: "Registra a manifestacao do destinatario no Ambiente Nacional: ciencia, confirmacao, desconhecimento ou nao_realizada (esta com justificativa). E um EVENTO FISCAL assinado com o certificado da empresa; vale a ultima manifestacao conclusiva e nao ha desfazer. Operacao com efeito fiscal.",
        emissao: true,
        schema: schemas::manifestacao,
    },
];

fn resultado(s: &Saida) -> Value {
    resultado_estruturado(s.corpo_com_erro(), s.erro.is_some())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PedidoDanfe {
    xml: String,
    #[serde(default)]
    canhoto: Option<bool>,
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
struct PedidoStatus {
    #[serde(default)]
    svc: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SemArgumentos {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PedidoLista {
    #[serde(default)]
    situacao: Option<String>,
    #[serde(default)]
    busca: String,
}

async fn danfe(estado: &Estado, args: Value) -> Result<Value, Erro> {
    let p: PedidoDanfe = argumentos(args)?;
    let pdf = estado.nfe.danfe(p.xml, p.canhoto, p.cancelada).await?;
    Ok(json!({
        "content": [
            {"type": "text", "text": format!("DANFE gerado: {} bytes, application/pdf.", pdf.len())},
            {"type": "resource", "resource": {
                "uri": "urn:rustnf:danfe.pdf",
                "mimeType": "application/pdf",
                "blob": base64::engine::general_purpose::STANDARD.encode(&pdf),
            }},
        ],
        "isError": false,
    }))
}

/// Executa a ferramenta da NF-e `nome`. O pedido com campo desconhecido e' recusado antes de qualquer envio (estrito).
pub(super) async fn rodar(nome: &str, estado: &Estado, args: Value) -> Result<Value, Erro> {
    let nfe = &estado.nfe;
    match nome {
        "conferir_nfe" => Ok(resultado(&nfe.conferir(args, true))),
        "montar_nfe" => Ok(resultado_estruturado(nfe.montar(args, true)?, false)),
        "gerar_danfe" => danfe(estado, args).await,
        "emitir_nfe" => Ok(resultado(&nfe.emitir(args, true, false).await?)),
        "registrar_epec" => Ok(resultado(&nfe.emitir(args, true, true).await?)),
        "consultar_nfe" => {
            let p: PedidoConsulta = argumentos(args)?;
            Ok(resultado(&nfe.consultar(&p.chave).await?))
        }
        "cancelar_nfe" => {
            let p: PedidoCancelar = argumentos(args)?;
            Ok(resultado(&nfe.cancelar(&p).await?))
        }
        "corrigir_nfe" => {
            let p: PedidoCorrigir = argumentos(args)?;
            Ok(resultado(&nfe.corrigir(&p).await?))
        }
        "inutilizar_nfe" => {
            let p: PedidoInutilizar = argumentos(args)?;
            Ok(resultado(&nfe.inutilizar(&p).await?))
        }
        "status_sefaz" => {
            let p: PedidoStatus = argumentos(args)?;
            Ok(resultado(&nfe.status(p.svc).await?))
        }
        "retomar_pendentes_nfe" => {
            let SemArgumentos {} = argumentos(args)?;
            Ok(resultado(&nfe.retomar().await?))
        }
        "sincronizar_recebidas" => {
            let p: PedidoSincronizar = argumentos(args)?;
            Ok(resultado(&nfe.sincronizar_recebidas(&p).await?))
        }
        "listar_recebidas" => {
            let p: PedidoLista = argumentos(args)?;
            let situacao = situacao_do_filtro(p.situacao.as_deref())?;
            Ok(resultado_estruturado(
                nfe.listar_recebidas(situacao, &p.busca)?,
                false,
            ))
        }
        "baixar_recebida" => {
            let p: PedidoBaixar = argumentos(args)?;
            Ok(resultado(&nfe.baixar_recebida(&p).await?))
        }
        "manifestar_recebida" => {
            let p: PedidoManifestar = argumentos(args)?;
            Ok(resultado(&nfe.manifestar_recebida(&p).await?))
        }
        _ => Err(Erro::Interno),
    }
}
