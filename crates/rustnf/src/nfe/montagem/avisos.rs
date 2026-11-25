// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Canal de avisos da montagem: o que a nota tem de estranho sem ser recusado. Aviso nunca impede a montagem (a recusa
//! continua sendo [`super::ErroMontagem`]) e nao muda o XML. Dois avisos:
//!
//! * **PIS/COFINS por regime** (`regime_pis_cofins` do pedido, que nao vai ao XML): o CST ou a aliquota do item nao
//!   combina com o regime informado. Fontes: Lei 10.637/2002, art. 2o (1,65%), art. 3o (credito) e art. 8o (quem fica
//!   no cumulativo); Lei 10.833/2003, art. 2o (7,6%), art. 3o e art. 10; Lei 9.715/1998, art. 8o, I (0,65%); Lei
//!   9.718/1998, art. 8o (3%); tabela de CST do PIS e da COFINS no MOC 7.00, Anexo I, grupos Q e S. Monofasico,
//!   aliquota zero, substituicao, suspensao, isencao e sem incidencia (CST 02 a 09, 49, 70 a 75, 98 e 99) nao geram aviso.
//! * **Simples sem regime regular e `gIBSCBS`**: a tabela 2.3 da NT 2026.005 v1.00 manda nao informar o grupo; a regra
//!   UB13-30 da NT 2025.002 v1.53 (rejeicao 1022) o exige quando o CST o pede.

use chrono::Datelike;

use super::formato::{na_uf, Dec};
use super::modelo::{Nota, PisCofins};
use super::partes::regime_sn;
use super::uf_nota;
use crate::reforma::tabela::grupo_exigido;

/// Regime do PIS e da COFINS do emitente do regime normal (CRT 3): o XML nao o distingue, porque lucro real e lucro
/// presumido sao ambos CRT 3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegimePisCofins {
    /// Cumulativo (lucro presumido ou arbitrado; Lei 10.637, art. 8o, II, e Lei 10.833, art. 10, II): PIS 0,65% e
    /// COFINS 3%, sem credito.
    Cumulativo,
    /// Nao cumulativo (lucro real): PIS 1,65% e COFINS 7,6%, com credito.
    NaoCumulativo,
}

impl RegimePisCofins {
    /// `cumulativo` ou `nao_cumulativo`.
    #[must_use]
    pub fn de_texto(t: &str) -> Option<Self> {
        match t.trim().to_lowercase().as_str() {
            "cumulativo" => Some(Self::Cumulativo),
            "nao_cumulativo" => Some(Self::NaoCumulativo),
            _ => None,
        }
    }

    /// (aliquota basica do PIS, da COFINS) em percentual.
    fn basicas(self) -> (&'static str, &'static str) {
        match self {
            Self::Cumulativo => ("0.65", "3"),
            Self::NaoCumulativo => ("1.65", "7.6"),
        }
    }
}

/// Que aviso e'.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum TipoAviso {
    /// O CST ou a aliquota do PIS ou da COFINS nao combina com o `regime_pis_cofins`.
    PisCofinsRegime,
    /// Simples sem regime regular do IBS/CBS, com CST que exige o `gIBSCBS`: as duas NT nao se conciliam.
    SimplesSemGIbscbs,
}

impl TipoAviso {
    /// Codigo estavel do aviso, para quem filtra por ele.
    #[must_use]
    pub fn codigo(self) -> &'static str {
        match self {
            Self::PisCofinsRegime => "pis_cofins_regime",
            Self::SimplesSemGIbscbs => "simples_sem_gibscbs",
        }
    }
}

/// Um aviso da montagem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aviso {
    /// Que aviso e'.
    pub tipo: TipoAviso,
    /// O item (a partir de 1), ou `None` quando e' da nota.
    pub item: Option<usize>,
    /// O texto, para quem emite.
    pub mensagem: String,
}

fn aviso(tipo: TipoAviso, item: Option<usize>, mensagem: String) -> Aviso {
    Aviso {
        tipo,
        item,
        mensagem,
    }
}

/// Os avisos da nota, em ordem de item. Nao confere o que a montagem recusa: uma nota recusada nao tem aviso.
#[must_use]
pub fn avisos(n: &Nota) -> Vec<Aviso> {
    let mut v = Vec::new();
    pis_cofins(n, &mut v);
    simples(n, &mut v);
    v
}

fn dec(s: &str) -> Dec {
    Dec::parse(s, "aliquota").unwrap_or(Dec::ZERO)
}

/// CST de credito (50 a 56) e de credito presumido (60 a 67).
fn cst_de_credito(cst: &str) -> bool {
    matches!(cst.parse::<u8>(), Ok(50..=56 | 60..=67))
}

fn pis_cofins(n: &Nota, v: &mut Vec<Aviso>) {
    let Some(regime) = n.regime_pis_cofins else {
        return;
    };
    // A LC 214/2025, art. 542, revoga a partir de 1o/1/2027 os artigos das leis acima que este aviso usa (incisos IX, X,
    // XVIII e XXI; efeito pelo art. 544, III, na redacao da LC 227/2026): o aviso so' vale para a emissao ate 2026.
    let Ok(uf) = uf_nota(n) else { return };
    if na_uf(&n.emissao, &uf).year() >= 2027 {
        return;
    }
    if n.emit.crt != 3 {
        v.push(aviso(
            TipoAviso::PisCofinsRegime,
            None,
            format!(
                "regime_pis_cofins informado para emitente do Simples Nacional (CRT {}): o PIS e a COFINS do \
                 optante vao no DAS (Lei 10.637/2002, art. 8o, III; Lei 10.833/2003, art. 10, III) e o campo so' \
                 vale para o CRT 3",
                n.emit.crt
            ),
        ));
        return;
    }
    for (i, it) in n.itens.iter().enumerate() {
        for (nome, pc) in [("PIS", &it.pis), ("COFINS", &it.cofins)] {
            if let Some(pc) = pc {
                if let Some(m) = conferir(regime, nome, pc) {
                    v.push(aviso(
                        TipoAviso::PisCofinsRegime,
                        Some(i + 1),
                        format!("item {}: {m}", i + 1),
                    ));
                }
            }
        }
    }
}

fn conferir(regime: RegimePisCofins, nome: &str, pc: &PisCofins) -> Option<String> {
    let cst = pc.cst.trim();
    if cst_de_credito(cst) && regime == RegimePisCofins::Cumulativo {
        return Some(format!(
            "{nome} com CST {cst} (credito) e regime cumulativo: o credito e' do regime nao cumulativo (Lei \
             10.637/2002, art. 3o; Lei 10.833/2003, art. 3o) e o cumulativo nao o admite (Lei 10.637, art. 8o, II; \
             Lei 10.833, art. 10, II)"
        ));
    }
    if cst != "01" {
        return None;
    }
    let (pis, cofins) = regime.basicas();
    let basica = dec(if nome == "PIS" { pis } else { cofins });
    let informada = pc.aliquota.as_deref().map(dec)?;
    if informada.eh_zero() || informada.menos(basica).m == 0 {
        return None;
    }
    let (texto, lei) = match regime {
        RegimePisCofins::Cumulativo => (
            "cumulativo",
            "Lei 9.715/1998, art. 8o, I (PIS 0,65%) e Lei 9.718/1998, art. 8o (COFINS 3%)",
        ),
        RegimePisCofins::NaoCumulativo => (
            "nao cumulativo",
            "Lei 10.637/2002, art. 2o (PIS 1,65%) e Lei 10.833/2003, art. 2o (COFINS 7,6%)",
        ),
    };
    let ressalva = if regime == RegimePisCofins::NaoCumulativo {
        "; vale se a receita e' das que o art. 8o da Lei 10.637 e o art. 10 da Lei 10.833 mantem no cumulativo"
    } else {
        ""
    };
    Some(format!(
        "{nome} com CST 01 e aliquota {} no regime {texto}: a aliquota basica e' {} ({lei}){ressalva}",
        informada.fixo(informada.casas().max(2)),
        basica.fixo(basica.casas().max(2)),
    ))
}

fn simples(n: &Nota, v: &mut Vec<Aviso>) {
    let Ok(regime) = regime_sn(n) else { return };
    if regime.g_ibscbs {
        return;
    }
    for (i, it) in n.itens.iter().enumerate() {
        let Some(ib) = &it.ibscbs else { continue };
        let exige = matches!(
            grupo_exigido(&ib.cst, "nfe", n.tp_nf_debito.as_deref()),
            Ok(Some("gIBSCBS"))
        );
        if exige {
            v.push(aviso(
                TipoAviso::SimplesSemGIbscbs,
                Some(i + 1),
                format!(
                    "item {}: o CST {} exige o gIBSCBS e o emitente e' do Simples sem regime regular do IBS/CBS (CRT 1 \
                     sem tpRegimeIBSCBS): a montagem omite o grupo pela tabela 2.3 da NT 2026.005 v1.00 (\"nao \
                     informar\"), mas a regra UB13-30 da NT 2025.002 v1.53 (rejeicao 1022) o exige quando o CST o \
                     pede, sem excecao para o CRT, e a nota (*) da propria tabela manda informa-lo quando o CST o \
                     exige. As duas Notas Tecnicas ainda nao se conciliam: a SEFAZ pode rejeitar com 1022",
                    i + 1,
                    ib.cst
                ),
            ));
        }
    }
}
