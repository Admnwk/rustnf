// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Codigos de retorno (`cStat`) da NF-e: a mensagem oficial e a classe do codigo.
//!
//! A tabela (`dados/nfe_cstat.json`) vem do MOC 7.00, Anexo I, secao 4.4 (tabelas 4.4.1, 4.4.2 e 4.4.3) e das regras de
//! validacao das secoes 4.1 a 4.3; os codigos criados depois por Nota Tecnica entram com a NT em
//! [`Entrada::origem`]. O texto do MOC manda: a mensagem de um codigo do MOC nunca e' trocada pela da NT.
//!
//! Fontes: <https://www.confaz.fazenda.gov.br/legislacao/arquivo-manuais/moc7-anexo-i-leiaute-e-rv.pdf> e as Notas
//! Tecnicas do Portal Nacional da NF-e (<https://www.nfe.fazenda.gov.br/portal>), listadas em `docs/FONTES.md`.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::Deserialize;

const TABELA: &str = include_str!("../../dados/nfe_cstat.json");

/// `cStat` que o protocolo de uma NF-e autorizada traz: 100 (autorizado) e 150 (autorizado fora do prazo).
pub const AUTORIZADA: [&str; 2] = ["100", "150"];
/// `cStat` de cancelamento homologado: 101 e 151 (fora do prazo).
pub const CANCELADA: [&str; 2] = ["101", "151"];
/// `cStat` de uso denegado: 110, 301, 302 e 303.
pub const DENEGADA: [&str; 4] = ["110", "301", "302", "303"];

/// A que familia o codigo pertence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Classe {
    /// 100 e 150: uso autorizado.
    Autorizada,
    /// 101 e 151: cancelamento homologado.
    Cancelada,
    /// 110, 301, 302 e 303: uso denegado; o numero da nota esta consumido.
    Denegada,
    /// Rejeicao (tabela 4.4.2 e regras): a nota nao foi aceita; o motivo esta na mensagem.
    Rejeitada,
    /// Outro resultado de processamento (tabela 4.4.1): lote recebido ou em processamento, servico em operacao ou
    /// paralisado, evento registrado, consulta de cadastro...
    Processamento,
    /// Codigo que a tabela nao tem.
    Desconhecida,
}

impl Classe {
    /// A nota esta autorizada (100 ou 150).
    #[must_use]
    pub fn autorizada(self) -> bool {
        self == Self::Autorizada
    }
}

/// Uma linha da tabela.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entrada {
    /// O codigo, sem zero a esquerda.
    pub cstat: String,
    /// A mensagem como esta no MOC ou na NT (a SEFAZ pode acrescentar `[campo: valor]` ao `xMotivo`).
    pub mensagem: String,
    /// A classe do codigo.
    pub classe: Classe,
    /// De onde vem o codigo quando nao e' das tabelas 4.4 do MOC: a NT (`NT 2025.002 v1.53`) ou a regra de validacao.
    pub origem: Option<String>,
    /// Segunda mensagem de um codigo que a regra escreve de dois jeitos.
    pub outras: Vec<String>,
}

#[derive(Deserialize)]
struct Linha {
    efeito: String,
    mensagem: String,
    origem: Option<String>,
    #[serde(default)]
    outras: Vec<String>,
}

#[derive(Deserialize)]
struct Arquivo {
    cstat: BTreeMap<String, Linha>,
}

fn classe_da_linha(cod: &str, efeito: &str) -> Classe {
    if AUTORIZADA.contains(&cod) {
        Classe::Autorizada
    } else if CANCELADA.contains(&cod) {
        Classe::Cancelada
    } else if DENEGADA.contains(&cod) || efeito == "denegacao" {
        Classe::Denegada
    } else if efeito == "rejeicao" {
        Classe::Rejeitada
    } else {
        Classe::Processamento
    }
}

fn tabela() -> &'static BTreeMap<String, Entrada> {
    static T: OnceLock<BTreeMap<String, Entrada>> = OnceLock::new();
    T.get_or_init(|| {
        let arquivo: Arquivo = serde_json::from_str(TABELA).unwrap_or(Arquivo {
            cstat: BTreeMap::new(),
        });
        arquivo
            .cstat
            .into_iter()
            .map(|(cod, l)| {
                let classe = classe_da_linha(&cod, &l.efeito);
                let e = Entrada {
                    cstat: cod.clone(),
                    mensagem: l.mensagem,
                    classe,
                    origem: l.origem,
                    outras: l.outras,
                };
                (cod, e)
            })
            .collect()
    })
}

/// So' os algarismos, sem zero a esquerda (`" 0100 "` -> `"100"`); vazio se nao ha algarismo.
#[must_use]
pub fn normalizar(cstat: &str) -> String {
    let d: String = cstat.chars().filter(char::is_ascii_digit).collect();
    if d.is_empty() {
        return d;
    }
    let s = d.trim_start_matches('0');
    if s.is_empty() {
        "0".to_string()
    } else {
        s.to_string()
    }
}

/// A linha da tabela para o codigo; `None` se a tabela nao o tem.
#[must_use]
pub fn consultar(cstat: &str) -> Option<&'static Entrada> {
    tabela().get(&normalizar(cstat))
}

/// A mensagem oficial do codigo.
#[must_use]
pub fn mensagem(cstat: &str) -> Option<&'static str> {
    consultar(cstat).map(|e| e.mensagem.as_str())
}

/// A classe do codigo; [`Classe::Desconhecida`] se a tabela nao o tem.
#[must_use]
pub fn classe(cstat: &str) -> Classe {
    consultar(cstat).map_or(Classe::Desconhecida, |e| e.classe)
}

/// Todos os codigos da tabela, em ordem numerica.
pub fn todos() -> impl Iterator<Item = &'static Entrada> {
    let mut v: Vec<&'static Entrada> = tabela().values().collect();
    v.sort_by_key(|e| e.cstat.parse::<u32>().unwrap_or(u32::MAX));
    v.into_iter()
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn a_tabela_embutida_le_e_tem_o_tamanho_conferido() {
        assert_eq!(tabela().len(), 953);
    }

    #[test]
    fn normalizar_tira_zero_e_ruido() {
        assert_eq!(normalizar(" 0100 "), "100");
        assert_eq!(normalizar("cStat=539"), "539");
        assert_eq!(normalizar("000"), "0");
        assert_eq!(normalizar("abc"), "");
    }
}
