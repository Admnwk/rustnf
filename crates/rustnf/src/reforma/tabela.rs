// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Tabela OFICIAL de CST e cClassTrib do IBS/CBS.
//!
//! Dado: `dados/cclasstrib.json` (portal SVRS Conformidade Facil + Calculadora RFB), embutido com
//! `include_str!`. O XSD NAO protege (`TCST` e' so' `\d{3}`, a escolha do grupo e' `xs:choice minOccurs="0"`): a
//! regra mora aqui, lida da tabela -- UB13-10 (1020), UB14-10 (1023), UB14-20 (1024), UB14-25 (1025) da NT 2025.002.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::Deserialize;

use super::{data_iso, escala, ErroReforma, Resultado};

const DADOS: &str = include_str!("../../dados/cclasstrib.json");

#[derive(Debug, Clone, Deserialize)]
pub struct Cst {
    pub cst: String,
    pub nome: String,
    #[serde(rename = "ind_gIBSCBS")]
    pub ind_g_ibscbs: bool,
    #[serde(rename = "ind_gIBSCBSMono")]
    pub ind_g_ibscbs_mono: bool,
    #[serde(rename = "ind_gRed")]
    pub ind_g_red: bool,
    #[serde(rename = "ind_gDif")]
    pub ind_g_dif: bool,
    #[serde(rename = "ind_gTransfCred")]
    pub ind_g_transf_cred: bool,
    #[serde(rename = "ind_gCredPresIBSZFM")]
    pub ind_g_cred_pres_ibszfm: bool,
    #[serde(rename = "ind_gAjusteCompet")]
    pub ind_g_ajuste_compet: bool,
    #[serde(rename = "ind_RedutorBC")]
    pub ind_redutor_bc: bool,
    pub ini_vig: Option<String>,
    pub fim_vig: Option<String>,
}

impl Cst {
    /// Grupo de valores que o CST EXIGE no item (None = so' CST + cClassTrib).
    pub fn grupo(&self) -> Option<&'static str> {
        if self.ind_g_ibscbs {
            Some("gIBSCBS")
        } else if self.ind_g_ibscbs_mono {
            Some("gIBSCBSMono")
        } else if self.ind_g_transf_cred {
            Some("gTransfCred")
        } else if self.ind_g_ajuste_compet {
            Some("gAjusteCompet")
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Rfb {
    pub cred_adquirente_cbs: bool,
    pub cred_adquirente_ibs: bool,
    #[serde(default)]
    pub cred_presumido_adquirente: bool,
    #[serde(default)]
    pub credito_operacao_antecedente: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ClassTrib {
    pub codigo: String,
    pub cst: String,
    pub nome: String,
    pub descricao: String,
    pub p_red_ibs: String,
    pub p_red_cbs: String,
    pub tipo_aliquota: String,
    #[serde(rename = "ind_gTribRegular")]
    pub ind_g_trib_regular: bool,
    #[serde(rename = "ind_gCredPresOper")]
    pub ind_g_cred_pres_oper: bool,
    #[serde(rename = "ind_gEstornoCred")]
    pub ind_g_estorno_cred: bool,
    #[serde(rename = "ind_gMonoPadrao")]
    pub ind_g_mono_padrao: bool,
    #[serde(rename = "ind_gMonoReten")]
    pub ind_g_mono_reten: bool,
    #[serde(rename = "ind_gMonoRet")]
    pub ind_g_mono_ret: bool,
    #[serde(rename = "ind_gMonoDif")]
    pub ind_g_mono_dif: bool,
    #[serde(rename = "ind_gpBioDiferenca")]
    pub ind_gp_bio_diferenca: bool,
    pub documentos: Vec<String>,
    pub ini_vig: Option<String>,
    pub fim_vig: Option<String>,
    pub publicacao: Option<String>,
    #[serde(default)]
    pub legislacao: String,
    pub anexo: Option<i64>,
    #[serde(default)]
    pub rfb: Option<Rfb>,
    /// tpRBSN (NT 2026.005, coluna TipoRbSn do CFF): 0 nao e' receita, 1-5 receita bruta, 9 incompativel com o SN.
    #[serde(default)]
    pub tipo_rb_sn: Option<i64>,
}

impl ClassTrib {
    pub fn vale_em(&self, documento: &str) -> bool {
        doc(documento)
            .map(|d| self.documentos.iter().any(|x| x == d))
            .unwrap_or(false)
    }

    pub fn vigente(&self, data: &str) -> Resultado<bool> {
        let d = data_iso(data)?;
        Ok(self.ini_vig.as_deref().is_none_or(|i| i <= d.as_str())
            && self.fim_vig.as_deref().is_none_or(|f| d.as_str() <= f))
    }

    /// Percentual de reducao (pRedAliq do gRed) em x10000, para "ibs" ou "cbs".
    pub fn p_red(&self, tributo: &str) -> i64 {
        let s = if tributo == "ibs" {
            &self.p_red_ibs
        } else {
            &self.p_red_cbs
        };
        escala(s, 4, "pRedAliq").unwrap_or(0) as i64
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Procedencia {
    pub fonte: String,
    pub sha256_pagina: String,
    pub extraido_em: String,
    pub publicacao_mais_recente: Option<String>,
    pub total_cst: usize,
    pub total_cclasstrib: usize,
    #[serde(default)]
    pub divergencias_entre_fontes: Vec<String>,
}

#[derive(Deserialize)]
struct Bruto {
    #[serde(flatten)]
    proc_: Procedencia,
    cst: Vec<Cst>,
    cclasstrib: Vec<ClassTrib>,
}

struct Dados {
    proc_: Procedencia,
    cst: BTreeMap<String, Cst>,
    cclass: BTreeMap<String, ClassTrib>,
}

fn dados() -> &'static Dados {
    static D: OnceLock<Dados> = OnceLock::new();
    D.get_or_init(|| {
        let b: Bruto = serde_json::from_str(DADOS).expect("cclasstrib.json embutido invalido");
        Dados {
            proc_: b.proc_,
            cst: b.cst.into_iter().map(|c| (c.cst.clone(), c)).collect(),
            cclass: b
                .cclasstrib
                .into_iter()
                .map(|c| (c.codigo.clone(), c))
                .collect(),
        }
    })
}

/// Documento ('nfe', 'nfce', '55', '65', 'nfse'...) -> nome curto da tabela.
pub fn doc(documento: &str) -> Resultado<&'static str> {
    let k = documento.trim().to_ascii_lowercase();
    let k = if k.is_empty() { "nfe".to_string() } else { k };
    let r = match k.as_str() {
        "55" | "nfe" => "nfe",
        "65" | "nfce" => "nfce",
        "57" | "cte" => "cte",
        "67" | "cteos" => "cteos",
        "63" | "bpe" => "bpe",
        "66" | "nf3e" => "nf3e",
        "62" | "nfcom" => "nfcom",
        "bpeta" => "bpeta",
        "bpetm" => "bpetm",
        "nfse" => "nfse",
        "nfag" => "nfag",
        "nfsvia" => "nfsvia",
        "nfabi" => "nfabi",
        "nfgas" => "nfgas",
        "dere" => "dere",
        "dir" => "dir",
        "duimp" => "duimp",
        _ => {
            let curto: String = k.chars().take(10).collect();
            return Err(ErroReforma::novo(
                format!(
                    "documento fiscal desconhecido: '{curto}' (ex.: nfe, nfce, nfse, cte, 55, 65)"
                ),
                "documento",
            ));
        }
    };
    Ok(r)
}

pub fn procedencia() -> &'static Procedencia {
    &dados().proc_
}

pub fn csts() -> Vec<&'static Cst> {
    dados().cst.values().collect()
}

pub fn cst(codigo: &str) -> Resultado<&'static Cst> {
    dados().cst.get(codigo.trim()).ok_or_else(|| {
        let todos: Vec<&str> = dados().cst.keys().map(String::as_str).collect();
        let curto: String = codigo.chars().take(5).collect();
        ErroReforma::novo(
            format!(
                "CST do IBS/CBS inexistente: '{curto}' -- a tabela oficial tem {} (NT 2025.002 UB13-10, rejeicao 1020)",
                todos.join(", ")
            ),
            "UB13-10/1020",
        )
    })
}

/// IT 2025.002 v1.70 (01/10/2026), sec. 07.09: cClassTrib criados pela v1.70. A tabela embarcada ja' e' a v1.71 (CFF 17/11/2026, publicacao 30/10); so' dispara com tabela anterior.
pub const NOVOS_IT_V170: [&str; 9] = [
    "000006", "200055", "200056", "400003", "400004", "550026", "550027", "550028", "550029",
];

pub fn classificacao(codigo: &str) -> Resultado<&'static ClassTrib> {
    dados().cclass.get(codigo.trim()).ok_or_else(|| {
        let curto: String = codigo.chars().take(8).collect();
        if NOVOS_IT_V170.contains(&codigo.trim()) {
            return ErroReforma::novo(
                format!(
                    "cClassTrib {} foi criado pela IT 2025.002 v1.70 (01/10/2026) e a tabela embarcada e' anterior \
                     (publicacao ate {}) -- atualize o rustnf quando o portal CFF publicar",
                    codigo.trim(),
                    dados().proc_.publicacao_mais_recente.as_deref().unwrap_or("?")
                ),
                "UB14-10/1023",
            );
        }
        ErroReforma::novo(
            format!(
                "cClassTrib inexistente: '{curto}' (tabela oficial SVRS, publicacao ate {}) -- NT 2025.002 UB14-10, \
                 rejeicao 1023",
                dados().proc_.publicacao_mais_recente.as_deref().unwrap_or("?")
            ),
            "UB14-10/1023",
        )
    })
}

/// Para o seletor da tela: filtra por CST, documento e vigencia.
pub fn classificacoes(
    cst_: Option<&str>,
    documento: Option<&str>,
    data: Option<&str>,
) -> Vec<&'static ClassTrib> {
    dados()
        .cclass
        .values()
        .filter(|c| cst_.is_none_or(|x| c.cst == x))
        .filter(|c| documento.is_none_or(|d| c.vale_em(d)))
        .filter(|c| data.is_none_or(|d| c.vigente(d).unwrap_or(false)))
        .collect()
}

/// CST + cClassTrib conferidos contra a tabela, o documento e a vigencia. Recusa com a regra da NT.
pub fn validar(
    cst_: &str,
    cclass_trib: &str,
    documento: &str,
    data: Option<&str>,
) -> Resultado<(&'static Cst, &'static ClassTrib)> {
    let c = cst(cst_)?;
    let k = classificacao(cclass_trib)?;
    if k.cst != c.cst {
        return Err(ErroReforma::novo(
            format!(
                "cClassTrib {} pertence ao CST {}, nao ao {} -- NT 2025.002 UB14-20, rejeicao 1024",
                k.codigo, k.cst, c.cst
            ),
            "UB14-20/1024",
        ));
    }
    let d = doc(documento)?;
    if !k.documentos.iter().any(|x| x == d) {
        return Err(ErroReforma::novo(
            format!(
                "cClassTrib {} nao e' permitido em {} pela tabela oficial -- NT 2025.002 UB14-25, rejeicao 1025",
                k.codigo,
                d.to_uppercase()
            ),
            "UB14-25/1025",
        ));
    }
    if let Some(dt) = data {
        if !k.vigente(dt)? {
            return Err(ErroReforma::novo(
                format!(
                    "cClassTrib {} fora de vigencia em {}",
                    k.codigo,
                    data_iso(dt)?
                ),
                "vigencia",
            ));
        }
    }
    Ok((c, k))
}

/// Grupo de valores que o item com este CST leva neste documento (`None` = so' CST e cClassTrib). Excecao das regras
/// UB13-30 e UB13-40: o `tpNFDebito` 07 (perda em estoque) nao leva `gIBSCBS` nem `gIBSCBSMono`.
pub fn grupo_exigido(
    cst_: &str,
    documento: &str,
    tp_nf_debito: Option<&str>,
) -> Resultado<Option<&'static str>> {
    let c = cst(cst_)?;
    let g = c.grupo();
    let d = doc(documento)?;
    if matches!(g, Some("gIBSCBS") | Some("gIBSCBSMono"))
        && matches!(tp_nf_debito, Some("07") | Some("7"))
    {
        return Ok(None);
    }
    if d == "nfce" {
        if let Some(gr) = g {
            if gr != "gIBSCBS" && gr != "gIBSCBSMono" {
                return Err(ErroReforma::novo(
                    format!(
                        "CST {} exige o grupo {gr}, que nao existe na NFC-e -- emita em NF-e",
                        c.cst
                    ),
                    "TTribNFCe",
                ));
            }
        }
    }
    Ok(g)
}

/// Credito do adquirente (regime regular) com este cClassTrib: Calculadora RFB, ou art. 49 derivado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Credito {
    pub ibs: bool,
    pub cbs: bool,
    pub presumido: bool,
    pub fonte: &'static str,
}

pub fn credito_adquirente(cclass_trib: &str) -> Resultado<Credito> {
    let k = classificacao(cclass_trib)?;
    if let Some(r) = &k.rfb {
        return Ok(Credito {
            ibs: r.cred_adquirente_ibs,
            cbs: r.cred_adquirente_cbs,
            presumido: r.cred_presumido_adquirente,
            fonte: "Calculadora RFB (dados abertos)",
        });
    }
    let pode = !matches!(
        k.cst.as_str(),
        "400" | "410" | "510" | "515" | "550" | "811" | "820"
    );
    Ok(Credito {
        ibs: pode && k.p_red("ibs") != 1_000_000,
        cbs: pode && k.p_red("cbs") != 1_000_000,
        presumido: false,
        fonte:
            "LC 214/2025 art. 49 (derivado do CST e da reducao; codigo ausente na Calculadora RFB)",
    })
}
