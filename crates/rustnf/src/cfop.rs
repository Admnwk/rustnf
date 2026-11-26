// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Tabela de CFOP do Portal Nacional da NF-e (tabelas de apoio, indicadores da IT 2023.002 v2.00).
//!
//! Dado: `dados/cfop.json`, extraido da planilha oficial sem interpretacao e embutido com `include_str!`. O MOC 7.00
//! (Anexo I) manda conferir a devolucao por ela: as regras I08-140 e I08-144 citam "Tabela CFOP, indDevol=1"; a
//! montagem da NF-e as aplica (rejeicoes 327, 1179 e 328).
//!
//! [`origens`] devolve os CFOP que a DESCRICAO OFICIAL do CFOP de devolucao cita como a operacao devolvida (o codigo
//! que abre um titulo entre aspas, no grupo espelho: entrada 1/2/3 <-> saida 5/6/7); [`devolucoes_de`] e' o caminho
//! inverso (a venda 5102 volta como 1202). Nada e' inferido do titulo; o CFOP de devolucao que nao cita origem
//! (consignacao 1918 e 1919, vasilhame 5921 ...) devolve lista vazia. Aceitam-se "5.102" e "5102".

use std::collections::BTreeMap;
use std::sync::OnceLock;

use chrono::NaiveDate;
use serde::Deserialize;

const DADOS: &str = include_str!("../dados/cfop.json");

/// NT 2025.002 v1.53, I08-141 (rejeicao 1179): o MEI (CRT 4) com `idDest` diferente de 3, na devolucao ou no retorno,
/// so' usa estes CFOP.
pub const CFOP_DEVOLUCAO_MEI: [&str; 6] = ["1202", "1553", "2202", "2553", "5202", "6202"];

/// Um CFOP da tabela; os indicadores levam o nome da coluna da planilha e o valor como veio.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Cfop {
    /// Titulo oficial.
    pub titulo: String,
    /// Inicio da vigencia (AAAA-MM-DD).
    pub inicio: Option<String>,
    /// Fim da vigencia (AAAA-MM-DD); `None` = sem fim.
    pub fim: Option<String>,
    #[serde(rename = "indNFe")]
    pub ind_nfe: u8,
    #[serde(rename = "indComunica")]
    pub ind_comunica: u8,
    #[serde(rename = "indTransp")]
    pub ind_transp: u8,
    /// 1 = CFOP de devolucao de mercadoria (I08-140 e I08-144).
    #[serde(rename = "indDevol")]
    pub ind_devol: u8,
    #[serde(rename = "indRetor")]
    pub ind_retor: u8,
    #[serde(rename = "indAnula")]
    pub ind_anula: u8,
    #[serde(rename = "indRemes")]
    pub ind_remes: u8,
    /// 0, 1 ou 2 na planilha, guardado como veio.
    #[serde(rename = "indComb")]
    pub ind_comb: u8,
    #[serde(rename = "indExcIBSCBS")]
    pub ind_exc_ibscbs: u8,
    /// So' no CFOP de devolucao: os codigos que a descricao oficial cita como a operacao devolvida.
    #[serde(default)]
    pub origens: Vec<String>,
}

#[derive(Deserialize)]
struct Arquivo {
    versao: String,
    cfop: BTreeMap<String, Cfop>,
}

fn dados() -> &'static Arquivo {
    static D: OnceLock<Arquivo> = OnceLock::new();
    D.get_or_init(|| serde_json::from_str(DADOS).expect("cfop.json embutido invalido"))
}

fn limpo(cfop: &str) -> String {
    cfop.chars().filter(char::is_ascii_digit).collect()
}

/// A tabela inteira, por codigo de 4 digitos.
#[must_use]
pub fn tabela() -> &'static BTreeMap<String, Cfop> {
    &dados().cfop
}

/// Versao da tabela embutida (indicadores da IT 2023.002 v2.00).
#[must_use]
pub fn versao() -> &'static str {
    &dados().versao
}

/// O registro do CFOP (`5.102` ou `5102`); `None` = nao existe na tabela.
#[must_use]
pub fn info(cfop: &str) -> Option<&'static Cfop> {
    tabela().get(&limpo(cfop))
}

/// `indDevol` = 1: CFOP de devolucao de mercadoria.
#[must_use]
pub fn e_devolucao(cfop: &str) -> bool {
    info(cfop).is_some_and(|r| r.ind_devol == 1)
}

/// O CFOP existe e esta vigente na data (inicio <= data e, se ha fim, data <= fim).
#[must_use]
pub fn vigente(cfop: &str, quando: NaiveDate) -> bool {
    let Some(r) = info(cfop) else { return false };
    let d = |s: &Option<String>| {
        s.as_deref()
            .and_then(|x| NaiveDate::parse_from_str(x, "%Y-%m-%d").ok())
    };
    if d(&r.inicio).is_some_and(|i| quando < i) {
        return false;
    }
    !d(&r.fim).is_some_and(|f| quando > f)
}

/// Os CFOP que a descricao oficial do CFOP de devolucao cita como a operacao devolvida; vazio se ele nao cita nenhum
/// ou nao e' de devolucao.
#[must_use]
pub fn origens(cfop_devolucao: &str) -> Vec<String> {
    info(cfop_devolucao)
        .map(|r| r.origens.clone())
        .unwrap_or_default()
}

/// Os CFOP de devolucao cuja descricao oficial cita `cfop` como a operacao devolvida, em ordem crescente.
#[must_use]
pub fn devolucoes_de(cfop: &str) -> Vec<String> {
    let c = limpo(cfop);
    tabela()
        .iter()
        .filter(|(_, v)| v.origens.contains(&c))
        .map(|(k, _)| k.clone())
        .collect()
}
