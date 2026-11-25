// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O que o emissor de demonstracao sabe de eventos: o roteiro de `--evento`, o evento que ele "registrou" nesta
//! execucao (para a consulta de evento e o historico devolverem) e a nota "recebida" de outro emitente, para clicar em
//! Confirmar e Recusar sem rede. Nada aqui fala com a Sefin.

use chrono::Utc;
use rustnf::nfse::base::{brasilia, esc};
use rustnf::nfse::eventos::TIPOS;
use rustnf::nfse::Dps;
use serde_json::json;

use crate::demonstracao::nfse_autorizada;

/// O CPF ficticio de quem "emitiu" a nota recebida da demonstracao.
const CPF_DO_PRESTADOR_RECEBIDO: &str = "12345678909";

/// Como o emissor de demonstracao responde a um pedido de evento (analise fiscal, confirmar, recusar).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistroDeEvento {
    /// A Sefin registra o evento.
    Registrado,
    /// A Sefin recusa o pedido, com duas mensagens.
    Rejeitado,
    /// Sem resposta: o evento nao e' confirmado (e nao foi registrado).
    SemResposta,
}

/// O roteiro de um dos nomes aceitos por `--evento`: `registrado`, `rejeitado` ou `sem-resposta` (o primeiro pedido fica
/// sem resposta, e nao foi registrado; o segundo, depois de a tela conferir, registra).
#[must_use]
pub fn evento_do_nome(nome: &str) -> Option<Vec<RegistroDeEvento>> {
    match nome {
        "registrado" => Some(vec![RegistroDeEvento::Registrado]),
        "rejeitado" => Some(vec![RegistroDeEvento::Rejeitado]),
        "sem-resposta" => Some(vec![
            RegistroDeEvento::SemResposta,
            RegistroDeEvento::Registrado,
        ]),
        _ => None,
    }
}

/// Um pedido de evento como o emissor o recebeu (para o teste conferir o que foi enviado e o que nao foi).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PedidoDeEvento {
    /// O `tpEvento`.
    pub tipo: String,
    /// A chave de acesso.
    pub chave: String,
    /// O codigo do motivo.
    pub motivo: String,
    /// A justificativa.
    pub justificativa: String,
}

/// Um evento que a demonstracao "registrou": a consulta de evento o devolve como a Sefin o devolveria.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EventoGuardado {
    pub(crate) chave: String,
    pub(crate) tipo: String,
    pub(crate) sequencia: u32,
    pub(crate) autor: String,
    pub(crate) motivo: String,
    pub(crate) justificativa: String,
    /// Quando foi pedido e processado, ISO 8601 de Brasilia.
    pub(crate) quando: String,
}

/// Agora, em ISO 8601 de Brasilia (`dhEvento` e `dhProc`).
pub(crate) fn agora_iso() -> String {
    Utc::now()
        .with_timezone(&brasilia())
        .format("%Y-%m-%dT%H:%M:%S%:z")
        .to_string()
}

impl EventoGuardado {
    /// O XML do evento registrado, com o que `ler_evento` le.
    pub(crate) fn xml(&self) -> String {
        let desc = TIPOS.iter().find(|t| t.0 == self.tipo).map_or("", |t| t.1);
        let (chave, tipo) = (&self.chave, &self.tipo);
        let (autor, tag) = (
            &self.autor,
            if self.autor.len() == 11 {
                "CPFAutor"
            } else {
                "CNPJAutor"
            },
        );
        let grupo = match (self.motivo.is_empty(), self.justificativa.is_empty()) {
            (true, _) => String::new(),
            (false, true) => format!("<cMotivo>{}</cMotivo>", esc(&self.motivo)),
            (false, false) => format!(
                "<cMotivo>{}</cMotivo><xMotivo>{}</xMotivo>",
                esc(&self.motivo),
                esc(&self.justificativa)
            ),
        };
        format!(
            "<evento xmlns=\"http://www.sped.fazenda.gov.br/nfse\" versao=\"1.01\"><infEvento Id=\"EVT{chave}{tipo}\">\
             <nSeqEvento>{seq}</nSeqEvento><dhProc>{quando}</dhProc><pedRegEvento versao=\"1.01\">\
             <infPedReg Id=\"PRE{chave}{tipo}\"><dhEvento>{quando}</dhEvento><{tag}>{autor}</{tag}>\
             <chNFSe>{chave}</chNFSe><e{tipo}><xDesc>{desc}</xDesc>{grupo}</e{tipo}></infPedReg>\
             </pedRegEvento></infEvento></evento>",
            seq = self.sequencia,
            quando = self.quando,
            desc = esc(desc),
        )
    }
}

/// A nota "recebida" da demonstracao: emitida por outra pessoa (CPF ficticio), com `tomador` (o documento do emitente
/// da configuracao) como tomador. Devolve a chave e o XML, como a Sefin os devolveria.
pub(crate) fn nota_recebida(ambiente: u8, tomador: &str) -> Option<(String, String)> {
    let dps: Dps = serde_json::from_value(json!({
        "prestador": {"documento": CPF_DO_PRESTADOR_RECEBIDO, "nome": "PRESTADOR AVULSO DE TESTE", "im": "1", "op_simples": 1},
        "cmun_emissor": "3550308",
        "numero": 4001,
        "serie": 1,
        "emissao": "2026-11-18T09:30:00-03:00",
        "competencia": "2026-11-18",
        "tomador": {"documento": tomador, "nome": "EMPRESA DA CONFIGURACAO (TOMADORA)"},
        "servico": {"ctribnac": "140101", "descricao": "Manutencao recebida pela empresa"},
        "valores": {"servico": "480.00"}
    }))
    .ok()?;
    nfse_autorizada(&dps, ambiente)
}
