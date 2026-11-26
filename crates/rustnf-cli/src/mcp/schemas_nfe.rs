// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! `inputSchema` (JSON Schema 2020-12) das ferramentas da NF-e. O pedido da NF-e e' grande (`docs/NFE_PEDIDO.md`): o
//! schema nomeia os campos da raiz e deixa o miolo (itens, impostos, transporte) para o documento. Campo desconhecido o
//! servidor recusa antes de qualquer envio.

use serde_json::{json, Value};

fn texto(descricao: &str) -> Value {
    json!({"type": "string", "description": descricao})
}

fn objeto(descricao: &str) -> Value {
    json!({"type": "object", "description": descricao})
}

fn lista(descricao: &str) -> Value {
    json!({"type": "array", "description": descricao})
}

/// Os campos da raiz do pedido da NF-e: nome e descricao. O que o pedido nao traz vem do `[emitente]`, do `[nfe]` e da
/// pasta de notas do servidor.
const CAMPOS_DA_RAIZ: [(&str, &str); 14] = [
    ("serie", "Serie; sem ela vale [nfe].serie."),
    (
        "numero",
        "Numero da nota; sem ele vale o proximo da serie na pasta de notas.",
    ),
    (
        "emissao",
        "Data e hora da emissao (dhEmi), com fuso; sem ela vale agora.",
    ),
    ("nat_op", "Natureza da operacao (natOp)."),
    (
        "dest",
        "Destinatario: documento, nome, ind_ie_dest e endereco.",
    ),
    (
        "itens",
        "Itens da nota (de 1 a 990): produto, CFOP, valores e impostos.",
    ),
    ("pagamentos", "Pagamentos (de 1 a 100): t_pag e valor."),
    ("ind_final", "Consumidor final: 1 sim, 0 nao."),
    ("ind_pres", "Presenca do comprador (indPres)."),
    ("tp_nf", "0 entrada, 1 saida."),
    ("fin_nfe", "Finalidade da NF-e (finNFe)."),
    (
        "transp",
        "Transporte: modalidade do frete, transportadora, volumes.",
    ),
    ("cobr", "Cobranca: fatura e duplicatas."),
    ("inf_cpl", "Informacoes complementares (infCpl)."),
];

const CAMPOS_ESTRUTURADOS: [&str; 5] = ["dest", "itens", "pagamentos", "transp", "cobr"];

/// Os nomes dos campos da raiz que o schema do pedido declara (os testes conferem cada um contra o leiaute).
#[must_use]
pub fn campos_do_pedido() -> Vec<&'static str> {
    CAMPOS_DA_RAIZ.iter().map(|(nome, _)| *nome).collect()
}

/// Schema do pedido da NF-e (`conferir_nfe`, `montar_nfe`, `emitir_nfe` e `registrar_epec`).
#[must_use]
pub fn pedido() -> Value {
    let mut props = serde_json::Map::new();
    for (nome, descricao) in CAMPOS_DA_RAIZ {
        let schema = match nome {
            "itens" | "pagamentos" => lista(descricao),
            n if CAMPOS_ESTRUTURADOS.contains(&n) => objeto(descricao),
            _ => json!({"type": ["string", "number"], "description": descricao}),
        };
        props.insert(nome.to_string(), schema);
    }
    props.insert(
        "emit".into(),
        objeto("Emitente; sem ele vale o [emitente] e o [nfe] da configuracao do servidor."),
    );
    json!({
        "type": "object",
        "description": "Pedido da NF-e (docs/NFE_PEDIDO.md). Campo que o leiaute nao conhece recusa o pedido.",
        "properties": props,
        "required": ["itens", "pagamentos"]
    })
}

/// Schema de `gerar_danfe`.
#[must_use]
pub fn danfe() -> Value {
    json!({
        "type": "object",
        "properties": {
            "xml": texto("XML da NF-e autorizada (nfeProc)."),
            "canhoto": {"type": "boolean", "description": "Com o canhoto; sem o campo vale [nfe].canhoto."},
            "cancelada": {"type": "boolean", "description": "Marca d'agua de cancelada; padrao false."}
        },
        "required": ["xml"],
        "additionalProperties": false
    })
}

/// Schema de `consultar_nfe`.
#[must_use]
pub fn consulta() -> Value {
    json!({
        "type": "object",
        "properties": {"chave": texto("Chave de acesso da NF-e (44 digitos).")},
        "required": ["chave"],
        "additionalProperties": false
    })
}

/// Schema de `cancelar_nfe`.
#[must_use]
pub fn cancelamento() -> Value {
    json!({
        "type": "object",
        "properties": {
            "chave": texto("Chave de acesso da NF-e (44 digitos)."),
            "justificativa": texto("Justificativa (15 a 255 caracteres); fica no documento fiscal."),
            "protocolo": texto("Protocolo de autorizacao; so' se a nota nao esta na pasta de notas do servidor.")
        },
        "required": ["chave", "justificativa"],
        "additionalProperties": false
    })
}

/// Schema de `corrigir_nfe`.
#[must_use]
pub fn correcao() -> Value {
    json!({
        "type": "object",
        "properties": {
            "chave": texto("Chave de acesso da NF-e (44 digitos)."),
            "texto": texto("Texto da correcao (15 a 1000 caracteres)."),
            "sequencia": {"type": "integer", "minimum": 1, "description": "Sequencia do evento; sem ela a pasta de notas numera."}
        },
        "required": ["chave", "texto"],
        "additionalProperties": false
    })
}

/// Schema de `inutilizar_nfe`.
#[must_use]
pub fn inutilizacao() -> Value {
    json!({
        "type": "object",
        "properties": {
            "serie": texto("Serie; sem ela vale [nfe].serie."),
            "inicio": {"type": "integer", "minimum": 1, "description": "Primeiro numero da faixa."},
            "fim": {"type": "integer", "minimum": 1, "description": "Ultimo numero da faixa."},
            "justificativa": texto("Justificativa (15 a 255 caracteres).")
        },
        "required": ["inicio", "fim", "justificativa"],
        "additionalProperties": false
    })
}

/// Schema de `status_sefaz`.
#[must_use]
pub fn status() -> Value {
    json!({
        "type": "object",
        "properties": {
            "svc": {"type": "boolean", "description": "Perguntar a SVC (contingencia) em vez da SEFAZ da UF; padrao false."}
        },
        "additionalProperties": false
    })
}

/// Schema de `retomar_pendentes_nfe`: sem argumentos.
#[must_use]
pub fn sem_argumentos() -> Value {
    json!({"type": "object", "properties": {}, "additionalProperties": false})
}
