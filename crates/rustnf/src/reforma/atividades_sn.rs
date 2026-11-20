// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! cAtivSN -- catalogo OFICIAL de atividades do Simples Nacional (NT 2026.005, I17d).
//!
//! Dado: `dados/atividades_sn.json` (base da Calculadora RTC da RFB), embutido com `include_str!`. 15 atividades (01..14 + 90): anexo de apuracao (a 09 depende do
//! fator r: >= 28% = III, < 28% = V, LC 123/2006 art. 18 par. 5o-J/5o-M), documento em que e' admitida (NF-e/NFC-e =
//! 01-04 e 90, CT-e/CT-e OS/BP-e/BP-e TM = 05 e 90, NFCom = 06 e 90, NFS-e = 07-14 e 90), indicadores de ISS.
//! Motivos e derivacao de anexos = os da calculadora (`MotivoNaoCalculoSimplesEnum`, `VSN-023`,
//! `DerivacaoAnexosSimples`). O "UB14-90" (90 x cClassTrib de tpRBSN 1-5) NAO esta na NT 2026.005 v1.00 publicada:
//! aqui e' so' um motivo informado, nao uma rejeicao. Contrato: `tests/casos/reforma_atividades_sn.json`.

use std::sync::OnceLock;

use serde::Deserialize;

use super::tabela;
use super::{data_iso, ErroReforma, Resultado};

const DADOS: &str = include_str!("../../dados/atividades_sn.json");

pub const AUSENTE: &str = "CATIVSN_AUSENTE";
pub const DESCONHECIDO: &str = "CATIVSN_DESCONHECIDO";
pub const INADMISSIVEL: &str = "CATIVSN_INADMISSIVEL_NO_DOCUMENTO";
pub const INCOMPATIVEL: &str = "CATIVSN_INCOMPATIVEL_CCLASSTRIB";

/// Anexos do Simples, na ordem canonica.
pub const ANEXOS: [&str; 5] = ["I", "II", "III", "IV", "V"];
pub const NAO_APLICAVEL: &str = "NAO_APLICAVEL";
pub const OPCIONAL: &str = "OPCIONAL";
pub const OBRIGATORIO: &str = "OBRIGATORIO";
pub const INDISPONIVEL: &str = "INDISPONIVEL";

/// `data=None` = a linha atual (sem fim de vigencia).
fn vale(ini: &str, fim: Option<&str>, data: Option<&str>) -> bool {
    match data {
        None => fim.is_none(),
        Some(d) => (ini.is_empty() || ini <= d) && fim.is_none_or(|f| d <= f),
    }
}

fn data_opt(data: Option<&str>) -> Resultado<Option<String>> {
    data.map(data_iso).transpose()
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Vinculo {
    pub modelo: String,
    pub sigla: String,
    pub ini_vig: String,
    pub fim_vig: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct AtividadeSN {
    pub cativsn: String,
    pub ordem: i64,
    pub descricao: String,
    pub anexo_sem_fator_r: Option<String>,
    pub anexo_com_fator_r: Option<String>,
    pub rotulo: String,
    pub rotulo_curto: String,
    pub nao_tributavel: bool,
    pub iss_no_simples: bool,
    pub admite_beneficio_municipal: bool,
    pub ini_vig: String,
    pub fim_vig: Option<String>,
    pub documentos: Vec<Vinculo>,
}

impl AtividadeSN {
    /// `data=None` = a linha atual do catalogo (sem fim de vigencia).
    pub fn vigente(&self, data: Option<&str>) -> Resultado<bool> {
        let d = data_opt(data)?;
        Ok(vale(&self.ini_vig, self.fim_vig.as_deref(), d.as_deref()))
    }

    #[must_use]
    pub fn sujeita_fator_r(&self) -> bool {
        self.anexo_com_fator_r.is_some()
    }

    /// Modelos em que a atividade e' admitida na data, ordenados.
    pub fn modelos(&self, data: Option<&str>) -> Resultado<Vec<String>> {
        let d = data_opt(data)?;
        let mut v: Vec<String> = self
            .documentos
            .iter()
            .filter(|x| vale(&x.ini_vig, x.fim_vig.as_deref(), d.as_deref()))
            .map(|x| x.modelo.clone())
            .collect();
        v.sort();
        v.dedup();
        Ok(v)
    }

    pub fn admite(&self, documento: &str, data: Option<&str>) -> Resultado<bool> {
        let m = modelo(documento)?;
        Ok(self.modelos(data)?.iter().any(|x| x == m))
    }
}

#[derive(Debug, Clone, Deserialize)]
struct TipoReceita {
    tp_rbsn: i64,
    vedado_nao_tributavel: bool,
    ini_vig: String,
    fim_vig: Option<String>,
    vocabulario: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Procedencia {
    pub fonte: String,
    pub commit: String,
    pub data_commit: String,
    pub ultima_migracao: String,
    pub total: usize,
}

#[derive(Deserialize)]
struct Bruto {
    fonte: String,
    commit: String,
    data_commit: String,
    ultima_migracao: String,
    total: usize,
    atividades: Vec<AtividadeSN>,
    tipos_receita_bruta: Vec<TipoReceita>,
}

struct Dados {
    proc_: Procedencia,
    atividades: Vec<AtividadeSN>,
    tipos: Vec<TipoReceita>,
}

fn dados() -> &'static Dados {
    static D: OnceLock<Dados> = OnceLock::new();
    D.get_or_init(|| {
        let b: Bruto = serde_json::from_str(DADOS).expect("atividades_sn.json embutido invalido");
        Dados {
            proc_: Procedencia {
                fonte: b.fonte,
                commit: b.commit,
                data_commit: b.data_commit,
                ultima_migracao: b.ultima_migracao,
                total: b.total,
            },
            atividades: b.atividades,
            tipos: b.tipos_receita_bruta,
        }
    })
}

/// De onde veio o catalogo: repositorio RFB, commit, data, ultima migracao Flyway, total.
#[must_use]
pub fn procedencia() -> &'static Procedencia {
    &dados().proc_
}

/// Todas as linhas do catalogo (inclusive as encerradas), na ordem oficial de exibicao.
#[must_use]
pub fn tabela() -> &'static [AtividadeSN] {
    &dados().atividades
}

/// Documento -> modelo (TPDF_TIPO da base RFB; NFS-e Nacional = 91 la').
pub fn modelo(documento: &str) -> Resultado<&'static str> {
    let k = documento.trim().to_ascii_lowercase();
    Ok(match k.as_str() {
        "55" | "nfe" => "55",
        "65" | "nfce" => "65",
        "57" | "cte" => "57",
        "67" | "cteos" => "67",
        "63" | "bpe" => "63",
        "93" | "bpetm" => "93",
        "62" | "nfcom" => "62",
        "91" | "nfse" => "91",
        _ => {
            let curto: String = k.chars().take(10).collect();
            return Err(ErroReforma::novo(
                format!(
                    "documento fiscal sem cAtivSN: '{curto}' (use 55/nfe, 65/nfce, 57/cte, 67/cteos, 63/bpe, 93/bpetm, \
                     62/nfcom ou nfse)"
                ),
                "documento",
            ));
        }
    })
}

/// A linha do codigo vigente na `data` (`None` = a linha atual). Codigo fora do catalogo = `Ok(None)`.
pub fn atividade(cativsn: &str, data: Option<&str>) -> Resultado<Option<&'static AtividadeSN>> {
    let c = cativsn.trim();
    for a in tabela() {
        if a.cativsn == c && a.vigente(data)? {
            return Ok(Some(a));
        }
    }
    Ok(None)
}

/// Anexo de apuracao (I..V); `None` na 90. Na 09 o fator r e' OBRIGATORIO (`Some(true)` = r >= 28% = III,
/// `Some(false)` = V); nas demais nao muda nada.
pub fn anexo(
    cativsn: &str,
    fator_r_atingido: Option<bool>,
    data: Option<&str>,
) -> Resultado<Option<&'static str>> {
    let Some(a) = atividade(cativsn, data)? else {
        let curto: String = cativsn.chars().take(5).collect();
        return Err(ErroReforma::novo(
            format!(
                "cAtivSN '{curto}' nao existe ou nao esta vigente no catalogo do Simples Nacional (RFB {})",
                procedencia().ultima_migracao
            ),
            DESCONHECIDO,
        ));
    };
    if let Some(com) = a.anexo_com_fator_r.as_deref() {
        let sem = a.anexo_sem_fator_r.as_deref().unwrap_or("?");
        return match fator_r_atingido {
            None => Err(ErroReforma::novo(
                format!(
                    "cAtivSN {} depende do fator r: informe se r >= 28% (Anexo {com}) ou nao (Anexo {sem})",
                    a.cativsn
                ),
                "fator_r",
            )),
            Some(true) => Ok(Some(com)),
            Some(false) => Ok(a.anexo_sem_fator_r.as_deref()),
        };
    }
    Ok(a.anexo_sem_fator_r.as_deref())
}

/// O tpRBSN veda a atividade nao tributavel (a 90)? Vocabulario do documento (NFS-e = `NFSE`, demais = `CRT`);
/// tpRBSN sem linha vigente = erro `VSN-026` (a calculadora tambem recusa).
pub fn vedado_nao_tributavel(tp_rbsn: i64, documento: &str, data: Option<&str>) -> Resultado<bool> {
    let voc = if modelo(documento)? == "91" {
        "NFSE"
    } else {
        "CRT"
    };
    let d = data_opt(data)?;
    dados()
        .tipos
        .iter()
        .find(|t| {
            t.vocabulario == voc
                && t.tp_rbsn == tp_rbsn
                && vale(&t.ini_vig, t.fim_vig.as_deref(), d.as_deref())
        })
        .map(|t| t.vedado_nao_tributavel)
        .ok_or_else(|| {
            ErroReforma::novo(
                format!("tpRBSN '{tp_rbsn}' sem efeito vigente na tabela da RFB ({voc})"),
                "VSN-026",
            )
        })
}

/// Por que o IBS/CBS do item NAO sai pelo Simples com esse cAtivSN: `Some((codigo, mensagem))`, ou `None` se serve.
/// `cclasstrib` (opcional) se confere ANTES do codigo (inexistente = `UB14-10/1023`, fora da vigencia = `VSN-021`).
pub fn motivo(
    cativsn: Option<&str>,
    documento: &str,
    cclasstrib: Option<&str>,
    data: Option<&str>,
) -> Resultado<Option<(&'static str, String)>> {
    let m = modelo(documento)?;
    let ct = cclasstrib.map(tabela::classificacao).transpose()?;
    if let (Some(ct), Some(d)) = (ct, data) {
        if !ct.vigente(d)? {
            return Err(ErroReforma::novo(
                format!(
                    "cClassTrib {} nao vigente em {} (vigencia {} a {})",
                    ct.codigo,
                    data_iso(d)?,
                    ct.ini_vig.as_deref().unwrap_or("..."),
                    ct.fim_vig.as_deref().unwrap_or("...")
                ),
                "VSN-021",
            ));
        }
    }
    let c = cativsn.unwrap_or("").trim();
    if c.is_empty() {
        return Ok(Some((
            AUSENTE,
            "o codigo de atividade do Simples Nacional (cAtivSN) nao foi informado; sem ele o IBS e a CBS do item nao \
             sao calculados (NT 2026.005, par. 5.3.3)"
                .into(),
        )));
    }
    let Some(a) = atividade(c, data)? else {
        let curto: String = c.chars().take(5).collect();
        return Ok(Some((
            DESCONHECIDO,
            format!(
                "o codigo de atividade '{curto}' nao existe ou nao esta vigente na data; o IBS e a CBS do item nao sao \
                 calculados"
            ),
        )));
    };
    let modelos = a.modelos(data)?;
    if !modelos.iter().any(|x| x == m) {
        let lista = if modelos.is_empty() {
            "nenhum".to_string()
        } else {
            modelos.join(", ")
        };
        return Ok(Some((
            INADMISSIVEL,
            format!(
                "a atividade '{c}' nao e' admitida no documento modelo {m} (admitida em: {lista}); o IBS e a CBS do \
                 item nao sao calculados"
            ),
        )));
    }
    if let Some(ct) = ct {
        if a.nao_tributavel {
            if let Some(tp) = ct.tipo_rb_sn {
                if vedado_nao_tributavel(tp, m, data)? {
                    return Ok(Some((
                        INCOMPATIVEL,
                        format!(
                            "cAtivSN {c} (operacoes nao tributaveis) e' incompativel com o cClassTrib {}, que declara \
                             receita bruta (tpRBSN {tp}) -- VSN-023 da calculadora RFB",
                            ct.codigo
                        ),
                    )));
                }
            }
        }
    }
    Ok(None)
}

/// `(disponivel, anexos_faltantes, modo_fator_r)` frente aos anexos da empresa -- `DerivacaoAnexosSimples.derivar`
/// da calculadora RFB. `anexos=None` = sem filtro (todos); vazio = nenhum.
pub fn derivar_anexos(
    atv: &AtividadeSN,
    anexos: Option<&[&str]>,
) -> Resultado<(bool, Vec<&'static str>, &'static str)> {
    let conf: Vec<&str> = match anexos {
        None => ANEXOS.to_vec(),
        Some(v) => {
            let mut fora: Vec<String> = v
                .iter()
                .filter(|x| !ANEXOS.contains(x))
                .map(|x| x.chars().take(5).collect())
                .collect();
            if !fora.is_empty() {
                fora.sort();
                return Err(ErroReforma::novo(
                    format!(
                        "anexo do Simples fora de I, II, III, IV, V: {}",
                        fora.join(", ")
                    ),
                    "anexos",
                ));
            }
            v.to_vec()
        }
    };
    let sem = atv.anexo_sem_fator_r.as_deref();
    let com = atv.anexo_com_fator_r.as_deref();
    if com.is_some() && sem.is_none() {
        return Err(ErroReforma::novo(
            format!(
                "catalogo inconsistente: cAtivSN {} so' com anexo do fator r",
                atv.cativsn
            ),
            "catalogo",
        ));
    }
    let tem = |x: &str| conf.contains(&x);
    let possiveis: Vec<&'static str> = ANEXOS
        .iter()
        .copied()
        .filter(|x| Some(*x) == sem || Some(*x) == com)
        .collect();
    let faltantes: Vec<&'static str> = possiveis.iter().copied().filter(|x| !tem(x)).collect();
    let disponivel = possiveis.is_empty() || possiveis.iter().any(|x| tem(x));
    let modo = match com {
        None => NAO_APLICAVEL,
        Some(c) if tem(c) => {
            if sem.is_some_and(tem) {
                OPCIONAL
            } else {
                OBRIGATORIO
            }
        }
        Some(_) => INDISPONIVEL,
    };
    Ok((disponivel, faltantes, modo))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opcao {
    pub atividade: &'static AtividadeSN,
    /// documento/cClassTrib; `None` = serve
    pub motivo: Option<(&'static str, String)>,
    pub anexos_faltantes: Vec<&'static str>,
    pub anexos_ok: bool,
    pub modo_fator_r: &'static str,
}

impl Opcao {
    #[must_use]
    pub fn disponivel(&self) -> bool {
        self.motivo.is_none() && self.anexos_ok
    }
}

/// As atividades vigentes, na ordem oficial, cada uma com os motivos de estar indisponivel -- o endpoint
/// `/atividades` da calculadora RFB. A indisponivel fica na lista (a tela mostra desabilitada com a dica).
pub fn opcoes(
    documento: &str,
    cclasstrib: Option<&str>,
    anexos: Option<&[&str]>,
    data: Option<&str>,
) -> Resultado<Vec<Opcao>> {
    let mut out = Vec::new();
    for a in tabela() {
        if a.vigente(data)? {
            let (ok, falt, modo) = derivar_anexos(a, anexos)?;
            let mot = motivo(Some(&a.cativsn), documento, cclasstrib, data)?;
            out.push(Opcao {
                atividade: a,
                motivo: mot,
                anexos_faltantes: falt,
                anexos_ok: ok,
                modo_fator_r: modo,
            });
        }
    }
    Ok(out)
}
