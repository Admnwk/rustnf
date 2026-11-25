// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O historico de uma nota na Sefin: os eventos que ela tem (cancelamento, substituicao, pedido de analise fiscal, a
//! decisao do fisco, manifestacoes), em ordem, com o nome que uma pessoa entende. O codigo do evento fica em segundo
//! plano. Sem janela.

use rustnf::nfse::eventos::{MOTIVOS_CANCELAMENTO, MOTIVOS_REJEICAO, TIPOS};
use rustnf::nfse::leitura::Evento;

use super::detalhe::data_e_hora;

/// O que a tela diz de um tipo de evento (o nome que uma pessoa entende). Tipo que a tela nao conhece volta com o
/// `xDesc` da biblioteca, se houver, ou so' com o codigo.
#[must_use]
pub fn nome_do_evento(tipo: &str) -> String {
    let nome = match tipo {
        "101101" => "Cancelamento da nota",
        "105102" => "Nota substituída por outra",
        "101103" => "Pedido de análise fiscal do cancelamento",
        "105104" => "Cancelamento deferido pelo fisco",
        "105105" => "Cancelamento indeferido pelo fisco",
        "202201" => "Confirmação do serviço pelo prestador",
        "203202" => "Confirmação do serviço pelo tomador",
        "204203" => "Confirmação do serviço pelo intermediário",
        "205204" => "Confirmação automática (tácita)",
        "202205" => "Recusa da nota pelo prestador",
        "203206" => "Recusa da nota pelo tomador",
        "204207" => "Recusa da nota pelo intermediário",
        "205208" => "Recusa anulada pelo fisco",
        "305101" => "Cancelamento por ofício",
        "305102" => "Bloqueio da nota por ofício",
        "305103" => "Desbloqueio da nota por ofício",
        outro => {
            return TIPOS
                .iter()
                .find(|t| t.0 == outro)
                .map_or_else(|| format!("Evento {outro}"), |t| t.1.to_string());
        }
    };
    nome.into()
}

/// Quem registra o tipo de evento, para a tela ("pelo fisco").
#[must_use]
pub fn quem_registra(tipo: &str) -> &'static str {
    match TIPOS.iter().find(|t| t.0 == tipo).map(|t| t.2) {
        Some("emitente") => "pelo emitente",
        Some("tomador") => "pelo tomador",
        Some("prestador") => "pelo prestador",
        Some("intermediario") => "pelo intermediário",
        Some("fisco") => "pelo fisco",
        Some("sistema") => "pelo sistema da Sefin",
        _ => "",
    }
}

/// O motivo como a pessoa le: o codigo de uma posicao (`3`) vira o texto da tabela da biblioteca; texto escrito
/// (a justificativa) fica como esta.
fn motivo_legivel(tipo: &str, motivo: &str) -> String {
    let m = motivo.trim();
    if m.chars().count() > 1 || m.is_empty() {
        return m.to_string();
    }
    let tabela = match tipo {
        "101101" | "101103" => MOTIVOS_CANCELAMENTO,
        _ => MOTIVOS_REJEICAO,
    };
    tabela
        .iter()
        .find(|t| t.0 == m)
        .map_or_else(|| m.to_string(), |t| t.1.to_string())
}

/// Um evento da nota, com o texto da tela.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemDoHistorico {
    /// O `tpEvento` (segundo plano).
    pub tipo: String,
    /// O que aconteceu, em palavras.
    pub nome: String,
    /// Quando, `DD/MM/AAAA HH:MM` (o processamento pela Sefin; na falta, o pedido).
    pub quando: String,
    /// Quem registrou: "pelo emitente", "pelo fisco"...
    pub quem: String,
    /// O motivo ou a justificativa, se o evento traz.
    pub motivo: String,
    /// A sequencia entre os eventos do mesmo tipo.
    pub sequencia: u32,
    /// A data e hora como a Sefin escreveu, para ordenar.
    ordem: String,
}

impl ItemDoHistorico {
    /// O item de um evento lido do XML que a Sefin devolveu.
    #[must_use]
    pub fn do_evento(e: &Evento, sequencia: u32) -> Self {
        let ordem = if e.processado_em.trim().is_empty() {
            e.data_evento.trim()
        } else {
            e.processado_em.trim()
        }
        .to_string();
        let autor = quem_registra(&e.tipo);
        Self {
            tipo: e.tipo.clone(),
            nome: nome_do_evento(&e.tipo),
            quando: data_e_hora(&ordem),
            quem: if autor.is_empty() {
                String::new()
            } else {
                format!("registrado {autor}")
            },
            motivo: motivo_legivel(&e.tipo, &e.motivo),
            sequencia,
            ordem,
        }
    }
}

/// Poe os eventos em ordem: do mais antigo para o mais novo; na mesma hora, pelo tipo e pela sequencia.
pub fn ordenar(itens: &mut [ItemDoHistorico]) {
    itens.sort_by(|a, b| (&a.ordem, &a.tipo, a.sequencia).cmp(&(&b.ordem, &b.tipo, b.sequencia)));
}

/// Como a consulta do historico terminou.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DesfechoDoHistorico {
    /// A Sefin respondeu por todos os tipos: estes sao os eventos da nota (pode ser nenhum).
    Eventos(Vec<ItemDoHistorico>),
    /// A Sefin parou de responder no meio: o que veio ate ali e a frase.
    Incompleto {
        /// O que ja se achou.
        itens: Vec<ItemDoHistorico>,
        /// A frase para a tela.
        texto: String,
    },
    /// Nao deu para consultar (certificado, senha, chave recusada): nada saiu.
    Recusado {
        /// A frase para a tela.
        texto: String,
    },
}

/// O que a tela diz quando a Sefin nao respondeu a consulta dos eventos.
pub const TEXTO_SEM_RESPOSTA: &str = "A Sefin Nacional não respondeu a toda a consulta (ou está indisponível). Consultar não altera nada: pode tentar de novo.";

/// O que a tela diz quando a nota nao tem nenhum evento.
pub const TEXTO_SEM_EVENTOS: &str = "A Sefin não tem nenhum evento para esta nota: ela não foi cancelada, substituída nem manifestada, e não há pedido de análise fiscal.";

#[cfg(test)]
mod testes {
    use super::*;

    fn evento(tipo: &str, processado: &str, motivo: &str) -> Evento {
        Evento {
            tipo: tipo.into(),
            chave: "x".into(),
            sequencia: "1".into(),
            processado_em: processado.into(),
            data_evento: "2026-11-20T09:00:00-03:00".into(),
            id: String::new(),
            autor: "11222333000181".into(),
            motivo: motivo.into(),
        }
    }

    #[test]
    fn todo_tipo_da_biblioteca_tem_nome_para_a_pessoa() {
        for (codigo, desc, _) in TIPOS {
            let nome = nome_do_evento(codigo);
            assert!(
                nome != format!("Evento {codigo}") && nome != *desc,
                "{codigo} sem nome proprio na tela"
            );
            assert!(!nome.contains(codigo), "{nome}");
        }
        assert_eq!(nome_do_evento("999999"), "Evento 999999");
    }

    #[test]
    fn quem_registra_cada_tipo() {
        assert_eq!(quem_registra("101103"), "pelo emitente");
        assert_eq!(quem_registra("105104"), "pelo fisco");
        assert_eq!(quem_registra("203202"), "pelo tomador");
        assert_eq!(quem_registra("999999"), "");
    }

    #[test]
    fn o_item_traz_data_do_processamento_quem_e_o_motivo_em_palavras() {
        let e = evento("203206", "2026-11-21T14:05:09-03:00", "3");
        let i = ItemDoHistorico::do_evento(&e, 1);
        assert_eq!(i.nome, "Recusa da nota pelo tomador");
        assert_eq!(i.quando, "21/11/2026 14:05");
        assert_eq!(i.quem, "registrado pelo tomador");
        assert_eq!(i.motivo, "Nao ocorrencia do fato gerador");
        // justificativa escrita fica como esta
        let j = ItemDoHistorico::do_evento(
            &evento(
                "101103",
                "2026-11-21T14:05:09-03:00",
                "Valor digitado errado",
            ),
            2,
        );
        assert_eq!(j.motivo, "Valor digitado errado");
        assert_eq!(j.sequencia, 2);
    }

    #[test]
    fn sem_processamento_vale_a_data_do_pedido() {
        let i = ItemDoHistorico::do_evento(&evento("101101", "", "1"), 1);
        assert_eq!(i.quando, "20/11/2026 09:00");
        assert_eq!(i.motivo, "Erro na emissao");
    }

    #[test]
    fn a_ordem_e_do_mais_antigo_para_o_mais_novo() {
        let mut itens = vec![
            ItemDoHistorico::do_evento(&evento("105104", "2026-11-23T10:00:00-03:00", ""), 1),
            ItemDoHistorico::do_evento(&evento("101103", "2026-11-21T10:00:00-03:00", ""), 1),
            ItemDoHistorico::do_evento(&evento("203202", "2026-11-21T10:00:00-03:00", ""), 1),
        ];
        ordenar(&mut itens);
        let tipos: Vec<&str> = itens.iter().map(|i| i.tipo.as_str()).collect();
        assert_eq!(tipos, vec!["101103", "203202", "105104"]);
    }
}
