// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! cCredPres -- codigo do Credito Presumido do IBS e da CBS (Anexo IV da NT 2025.002).
//!
//! Dado: `dados/ccredpres.json` (IT 2025.002 v1.70, planilha `cCredPres` do Portal Nacional da NF-e), embutido com
//! `include_str!`. Cada linha diz, por codigo, se o grupo do IBS (`ind_gIBSCredPres`) e o da CBS (`ind_gCBSCredPres`)
//! sao exigidos, em que documento e por qual via (nota ou evento) o credito se apropria e a vigencia.
//!
//! [`conferir_grupos`] aplica as regras da NT 2025.002 v1.53 que usam a tabela, as mesmas no item da NF-e (grupo
//! `gCredPresOper`, regras `UB122-10`, `UB123-10`, `UB123-20`, `UB127-10` e `UB127-20`) e no evento 211110 (regras
//! `2P25a-10`, `2P26-10`, `2P26-20`, `2P30-10` e `2P30-20`): codigo inexistente (rejeicao 1055); grupo do IBS
//! informado onde o indicador e 0 (1053) ou ausente onde e 1 (1054); o mesmo para a CBS (1050 e 1058). Indicador
//! vazio na planilha (o codigo nao tem o tributo) conta como "nao permite": so' o valor 1 exige o grupo.
//!
//! As colunas "apropria via NF", "apropria via evento", `indNFe` e a vigencia ficam na consulta ([`registro`]), mas
//! nenhuma regra da NT 2025.002 v1.53 as converte em rejeicao; por isso [`conferir_grupos`] nao as usa.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::Deserialize;

const DADOS: &str = include_str!("../../dados/ccredpres.json");

/// Uma linha da tabela cCredPres. Indicador `None` = celula vazia na planilha.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct CodigoCredPres {
    /// Os 2 digitos do codigo (`01` a `13` na v1.70).
    pub codigo: String,
    /// A descricao oficial, com o artigo da LC 214/2025.
    pub descricao: String,
    /// "Apropria via NF?": o credito se apropria no grupo `gCredPresOper` da nota.
    pub via_nf: bool,
    /// "Apropria via evento?": o credito se apropria pelo evento 211110.
    pub via_evento: bool,
    /// `indDeduzCredPres`: o credito deduz do valor do IBS/CBS da nota.
    pub deduz: Option<bool>,
    /// `ind_gCBSCredPres`: 1 exige o grupo `gCBSCredPres`; 0 ou vazio nao o permite.
    pub ind_cbs: Option<bool>,
    /// `ind_gIBSCredPres`: 1 exige o grupo `gIBSCredPres`; 0 ou vazio nao o permite.
    pub ind_ibs: Option<bool>,
    /// `dIniVigCBS` (AAAA-MM-DD).
    pub ini_cbs: Option<String>,
    /// `dFimVigCBS`.
    pub fim_cbs: Option<String>,
    /// `dIniVigIBS`.
    pub ini_ibs: Option<String>,
    /// `dFimVigIBS`.
    pub fim_ibs: Option<String>,
    /// `indDecPag`: o evento leva a declaracao de pagamento ao fornecedor (texto na planilha).
    pub ind_dec_pag: Option<bool>,
    /// `indNFe`: o codigo vale na NF-e (modelo 55).
    pub ind_nfe: bool,
    /// `indNFCe`: o codigo vale na NFC-e (modelo 65).
    pub ind_nfce: bool,
    /// `indCTe`: o codigo vale no CT-e.
    pub ind_cte: bool,
    /// `indNFSe`: o codigo vale na NFS-e.
    pub ind_nfse: bool,
}

impl CodigoCredPres {
    /// O grupo `gIBSCredPres` e' obrigatorio para este codigo.
    #[must_use]
    pub fn exige_ibs(&self) -> bool {
        self.ind_ibs == Some(true)
    }

    /// O grupo `gCBSCredPres` e' obrigatorio para este codigo.
    #[must_use]
    pub fn exige_cbs(&self) -> bool {
        self.ind_cbs == Some(true)
    }
}

#[derive(Deserialize)]
struct Bruto {
    versao: String,
    codigos: Vec<CodigoCredPres>,
}

fn dados() -> &'static (String, BTreeMap<String, CodigoCredPres>) {
    static D: OnceLock<(String, BTreeMap<String, CodigoCredPres>)> = OnceLock::new();
    D.get_or_init(|| {
        let b: Bruto = serde_json::from_str(DADOS).expect("ccredpres.json embutido invalido");
        (
            b.versao,
            b.codigos
                .into_iter()
                .map(|c| (c.codigo.clone(), c))
                .collect(),
        )
    })
}

/// A tabela inteira, por codigo.
#[must_use]
pub fn tabela() -> &'static BTreeMap<String, CodigoCredPres> {
    &dados().1
}

/// Versao da planilha embutida (ex.: "1.70").
#[must_use]
pub fn versao() -> &'static str {
    &dados().0
}

/// A linha do codigo (2 digitos), ou `None` se ele nao existe na tabela.
#[must_use]
pub fn registro(codigo: &str) -> Option<&'static CodigoCredPres> {
    tabela().get(codigo.trim())
}

/// A descricao oficial do codigo, ou `None` se ele nao existe.
#[must_use]
pub fn descricao(codigo: &str) -> Option<&'static str> {
    registro(codigo).map(|c| c.descricao.as_str())
}

/// A primeira regra violada: rejeicao da NT, as regras (item da NF-e e evento) e o campo da tela/pedido.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violacao {
    /// Codigo da rejeicao (cStat): 1055, 1053, 1054, 1050 ou 1058.
    pub rejeicao: &'static str,
    /// Regra do item da NF-e (`UB122-10` ...).
    pub regra_item: &'static str,
    /// Regra do evento 211110 (`2P25a-10` ...).
    pub regra_evento: &'static str,
    /// `cCredPres`, `gIBSCredPres` ou `gCBSCredPres`.
    pub campo: &'static str,
    /// A frase para quem emite.
    pub mensagem: String,
}

/// Confere o codigo e os grupos presentes contra a tabela, na ordem da NT (1055, 1053, 1054, 1050, 1058): `None` =
/// passa. `codigo` ja' com 2 digitos; `tem_ibs` e `tem_cbs` dizem se `gIBSCredPres` e `gCBSCredPres` vao no XML.
#[must_use]
pub fn conferir_grupos(codigo: &str, tem_ibs: bool, tem_cbs: bool) -> Option<Violacao> {
    let c = codigo.trim();
    let Some(linha) = registro(c) else {
        return Some(Violacao {
            rejeicao: "1055",
            regra_item: "UB122-10",
            regra_evento: "2P25a-10",
            campo: "cCredPres",
            mensagem: format!(
                "cCredPres {c} inexistente na tabela de Credito Presumido do IBS e da CBS (v{})",
                versao()
            ),
        });
    };
    // (tributo, grupo, exigido, presente, regra de "informado indevidamente", idem "nao informado")
    let ibs = (
        "IBS",
        "gIBSCredPres",
        linha.exige_ibs(),
        tem_ibs,
        ("1053", "UB123-10", "2P26-10"),
        ("1054", "UB123-20", "2P26-20"),
    );
    let cbs = (
        "CBS",
        "gCBSCredPres",
        linha.exige_cbs(),
        tem_cbs,
        ("1050", "UB127-10", "2P30-10"),
        ("1058", "UB127-20", "2P30-20"),
    );
    for (nome, grupo, exige, tem, indevido, ausente) in [ibs, cbs] {
        let (falha, quando) = match (exige, tem) {
            (true, false) => (ausente, "exige"),
            (false, true) => (indevido, "nao permite"),
            _ => continue,
        };
        return Some(Violacao {
            rejeicao: falha.0,
            regra_item: falha.1,
            regra_evento: falha.2,
            campo: grupo,
            mensagem: format!("cCredPres {c} {quando} o credito presumido do {nome} ({grupo})"),
        });
    }
    None
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod testes {
    use super::*;

    #[test]
    fn dado_embutido_le_e_tem_13_codigos() {
        assert_eq!((versao(), tabela().len()), ("1.70", 13));
        assert!(registro(" 05 ").is_some());
        assert_eq!(registro("14"), None);
    }
}
