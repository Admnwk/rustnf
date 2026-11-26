// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! `inputSchema` (JSON Schema 2020-12) das ferramentas da NFC-e. O pedido e' o da NF-e (`docs/NFE_PEDIDO.md`; o modelo 65 vem
//! sozinho), com `offline` e `offline_se_necessario` a mais. Nenhum schema tem campo de CSC: ele so' se guarda pela linha de
//! comando, e o pedido que o traz e' recusado antes de qualquer envio.

use serde_json::{json, Value};

use super::schemas_nfe;

/// Schema de `emitir_nfce`: o pedido da NF-e mais o modo de contingencia.
#[must_use]
pub fn pedido() -> Value {
    let mut s = schemas_nfe::pedido();
    s["description"] = json!(
        "Pedido da NFC-e (docs/NFE_PEDIDO.md; o modelo 65 vem sozinho). Campo que o leiaute nao conhece recusa o pedido, e o CSC nunca entra por aqui."
    );
    if let Some(props) = s["properties"].as_object_mut() {
        props.insert(
            "offline".into(),
            json!({"type": "boolean", "description": "Contingencia off-line (tpEmis 9): assina e guarda a nota como pendente de transmissao, SEM falar com a SEFAZ. Entregue o DANFC-e ao consumidor e transmita no prazo (retomar_pendentes_nfce)."}),
        );
        props.insert(
            "offline_se_necessario".into(),
            json!({"type": "boolean", "description": "Pergunta ao status do servico antes de enviar: se a SEFAZ nao responde (ou esta paralisada), emite off-line; se responde, envia. Depois de enviar sem resposta NUNCA vira off-line: fica pendente."}),
        );
    }
    s["required"] = json!(["itens", "pagamentos"]);
    s
}

/// Schema de `gerar_danfce`.
#[must_use]
pub fn danfce() -> Value {
    json!({
        "type": "object",
        "properties": {
            "xml": {"type": "string", "description": "XML da NFC-e autorizada (nfeProc) ou da NFe de contingencia off-line."},
            "formato": {"type": "string", "enum": ["pdf", "escpos"], "description": "pdf (bobina) ou escpos (bytes para a impressora); padrao pdf."},
            "largura": {"type": "integer", "enum": [80, 58], "description": "Largura da bobina em mm; sem ela vale [nfce].largura_danfce."},
            "colunas": {"type": "integer", "enum": [48, 42, 32], "description": "Colunas do ESC/POS; sem elas vale [nfce].colunas_escpos."},
            "resumido": {"type": "boolean", "description": "Cupom resumido, sem a divisao II (so' onde a UF permite); padrao false."},
            "via_estabelecimento": {"type": "boolean", "description": "A via do estabelecimento (so' existe na contingencia off-line); padrao false."},
            "cancelada": {"type": "boolean", "description": "Marca de nota cancelada; padrao false."}
        },
        "required": ["xml"],
        "additionalProperties": false
    })
}

/// Schema de `consultar_nfce`.
#[must_use]
pub fn consulta() -> Value {
    let mut s = schemas_nfe::consulta();
    s["properties"]["chave"]["description"] = json!("Chave de acesso da NFC-e (44 digitos).");
    s
}

/// Schema de `cancelar_nfce`.
#[must_use]
pub fn cancelamento() -> Value {
    let mut s = schemas_nfe::cancelamento();
    s["properties"]["chave"]["description"] = json!("Chave de acesso da NFC-e (44 digitos).");
    s
}

/// Schema de `status_sefaz_nfce`, `pendentes_nfce` e `retomar_pendentes_nfce`: sem argumentos.
#[must_use]
pub fn sem_argumentos() -> Value {
    schemas_nfe::sem_argumentos()
}
