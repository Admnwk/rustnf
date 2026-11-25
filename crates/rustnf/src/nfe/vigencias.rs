// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Calendario das Notas Tecnicas da NF-e: a data em que cada regra passa a valer, por ambiente.
//!
//! Cada linha traz a NT e a versao em que a data esta no "Historico de Alteracoes / Cronograma" da propria nota
//! tecnica (Portal Nacional da NF-e, <https://www.nfe.fazenda.gov.br/portal>). Quando uma versao nova da NT muda a
//! data, vale a da versao mais nova. Producao e' o ambiente de autorizacao; homologacao e' o de teste da SEFAZ, que
//! na maioria das NT abre antes (a data de homologacao pode variar por UF nesse intervalo).

use chrono::NaiveDate;

/// Ambiente da SEFAZ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ambiente {
    /// Autorizacao com valor fiscal (`tpAmb` 1).
    Producao,
    /// Testes (`tpAmb` 2).
    Homologacao,
}

/// Uma regra ou mudanca de leiaute e quando ela passa a valer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Vigencia {
    /// Identificador estavel da linha.
    pub chave: &'static str,
    /// A Nota Tecnica e a versao de onde vem a data.
    pub nt: &'static str,
    /// O que muda.
    pub descricao: &'static str,
    /// Inicio em producao.
    pub producao: NaiveDate,
    /// Inicio em homologacao; `None` = a NT nao da data propria (vale a de producao).
    pub homologacao: Option<NaiveDate>,
}

impl Vigencia {
    /// Quando a regra passa a valer no ambiente (homologacao sem data propria usa a de producao).
    #[must_use]
    pub fn inicio(&self, ambiente: Ambiente) -> NaiveDate {
        match ambiente {
            Ambiente::Producao => self.producao,
            Ambiente::Homologacao => self.homologacao.unwrap_or(self.producao),
        }
    }
}

/// Data fixa da tabela; data impossivel nao compila.
const fn d(ano: i32, mes: u32, dia: u32) -> NaiveDate {
    match NaiveDate::from_ymd_opt(ano, mes, dia) {
        Some(x) => x,
        None => panic!("data impossivel na tabela de vigencias"),
    }
}

const fn v(
    chave: &'static str,
    nt: &'static str,
    descricao: &'static str,
    producao: NaiveDate,
    homologacao: Option<NaiveDate>,
) -> Vigencia {
    Vigencia {
        chave,
        nt,
        descricao,
        producao,
        homologacao,
    }
}

const TABELA: [Vigencia; 16] = [
    v(
        "nt2021_003.grupo_i",
        "NT 2021.003 v1.50",
        "GTIN no Cadastro Centralizado (CCG), grupo I: tabaco, farmaceuticos e brinquedos",
        d(2022, 9, 12),
        Some(d(2022, 7, 25)),
    ),
    v(
        "nt2021_003.etapa_2",
        "NT 2021.003 v1.50",
        "GTIN no CCG, etapa 2: a NCM da nota confere com a do CCG",
        d(2023, 6, 12),
        Some(d(2023, 3, 6)),
    ),
    v(
        "nt2021_003.grupo_ii",
        "NT 2021.003 v1.50",
        "GTIN no CCG, grupo II: bebidas, cimento, higiene pessoal e cosmeticos (producao adiada na v1.21)",
        d(2023, 7, 3),
        Some(d(2023, 4, 3)),
    ),
    v(
        "nt2021_003.grupo_iii",
        "NT 2021.003 v1.50",
        "GTIN no CCG, grupo III",
        d(2024, 9, 2),
        Some(d(2024, 4, 1)),
    ),
    v(
        "nt2021_003.grupo_iv",
        "NT 2021.003 v1.50",
        "GTIN no CCG, grupo IV",
        d(2025, 10, 1),
        Some(d(2025, 7, 1)),
    ),
    v(
        "nt2021_003.grupo_v",
        "NT 2021.003 v1.50",
        "GTIN no CCG, grupo V",
        d(2027, 9, 1),
        Some(d(2027, 4, 1)),
    ),
    v(
        "nt2025_002.vc02_40",
        "NT 2025.002 v1.40",
        "VC02-40: emitente do DF-e referenciado na devolucao",
        d(2026, 8, 3),
        Some(d(2026, 7, 1)),
    ),
    v(
        "nt2025_002.vc02_14",
        "NT 2025.002 v1.52",
        "VC02-14: a devolucao referencia o documento so' no grupo DFeReferenciado (producao movida de 05/10 para 03/11/2026 na v1.52)",
        d(2026, 11, 3),
        Some(d(2026, 7, 1)),
    ),
    v(
        "nt2025_002.v1_53",
        "NT 2025.002 v1.53",
        "devolucao com a chave no cabecalho ou no item; no cabecalho, uma unica chave (B25-40)",
        d(2026, 11, 16),
        Some(d(2026, 11, 9)),
    ),
    v(
        "nt2026_002.vc02_04_tpimp6",
        "NT 2026.002 v1.10",
        "VC02-04 passa a valer tambem na NF-e com tpImp=6 (DANFE Simplificado)",
        d(2026, 10, 5),
        Some(d(2026, 9, 1)),
    ),
    v(
        "nt2026_002.v1_11_adiadas",
        "NT 2026.002 v1.11",
        "I08-180, BA02-35 e VC02-40 adiadas em producao de 05/10 para 14/12/2026 (homologacao em 01/09/2026, v1.10)",
        d(2026, 12, 14),
        Some(d(2026, 9, 1)),
    ),
    v(
        "pl_010h",
        "NT 2026.005 v1.00 e NT 2026.008 v1.01",
        "pacote de esquemas PL_010h (campos do Simples Nacional na NF-e e valor liquido do produto)",
        d(2026, 11, 16),
        Some(d(2026, 10, 26)),
    ),
    v(
        "nt2026_008.nb01_30",
        "NT 2026.008 v1.01",
        "NB01-30",
        d(2027, 3, 1),
        Some(d(2027, 2, 1)),
    ),
    v(
        "nt2023_002.inutilizacao_ac_ba_sp",
        "NT 2023.002 v1.02",
        "servico de inutilizacao eliminado em AC, BA e SP",
        d(2026, 11, 11),
        Some(d(2026, 11, 11)),
    ),
    v(
        "nt2023_002.assincrono",
        "NT 2023.002 v1.02",
        "autorizacao assincrona (indSinc=0) recusada com o cStat 129",
        d(2027, 4, 5),
        Some(d(2027, 2, 1)),
    ),
    v(
        "nt2026_010.danfe_rtc",
        "NT 2026.010 v1.00",
        "DANFE com o leiaute da Reforma Tributaria",
        d(2026, 12, 1),
        None,
    ),
];

/// A linha de `chave`; `None` se a tabela nao a tem.
#[must_use]
pub fn buscar(chave: &str) -> Option<&'static Vigencia> {
    TABELA.iter().find(|x| x.chave == chave)
}

/// Quando a regra `chave` passa a valer no ambiente; `None` se a chave nao existe.
#[must_use]
pub fn inicio(chave: &str, ambiente: Ambiente) -> Option<NaiveDate> {
    buscar(chave).map(|x| x.inicio(ambiente))
}

/// A regra `chave` vale no `dia` (inclusive) no ambiente? `None` se a chave nao existe.
#[must_use]
pub fn vigente(chave: &str, dia: NaiveDate, ambiente: Ambiente) -> Option<bool> {
    inicio(chave, ambiente).map(|i| dia >= i)
}

/// A tabela inteira em ordem de inicio em producao (e, no empate, pela chave).
#[must_use]
pub fn listar() -> Vec<&'static Vigencia> {
    let mut v: Vec<&'static Vigencia> = TABELA.iter().collect();
    v.sort_by_key(|x| (x.producao, x.chave));
    v
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn chaves_unicas_e_homologacao_nao_depois_da_producao() {
        let mut chaves: Vec<&str> = TABELA.iter().map(|x| x.chave).collect();
        chaves.sort_unstable();
        chaves.dedup();
        assert_eq!(chaves.len(), TABELA.len());
        for x in &TABELA {
            if let Some(h) = x.homologacao {
                assert!(h <= x.producao, "{}", x.chave);
            }
        }
    }
}
