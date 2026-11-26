// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Contingencia SVC: qual SEFAZ Virtual de Contingencia atende cada UF e o que a NF-e emitida nela leva.
//!
//! * **Quem atende quem.** O Ato COTEPE/ICMS 39/12, art. 1o, na redacao do Ato COTEPE/ICMS 110/22 (efeitos a partir de
//!   02/12/2022), da a SVC-AN (Receita Federal) a AC, AL, AP, CE, ES, MG, PA, PB, PI, RJ, RS, RN, RO, RR, SC, SE, SP,
//!   TO e DF, e a SVC-RS as UF AM, BA, GO, MA, MT, MS, PE e PR. Pagina do Confaz lida em 25/11/2026:
//!   <https://www.confaz.fazenda.gov.br/legislacao/atos/2012/ac039_12>; a pagina "Relacao de Servicos Web" do Portal
//!   Nacional da NF-e, lida no mesmo dia, lista as mesmas UF em cada SVC.
//! * **O que a nota leva.** `tpEmis` 6 na SVC-AN e 7 na SVC-RS (MOC 7.00, Anexo I, B22 e regra B25-40, rejeicao 713:
//!   a SVC nao aceita `tpEmis` diferente do da propria SVC). A NFC-e nao e' autorizada pela SVC (rejeicao 783). O MOC
//!   so' manda informar `dhCont` e `xJust` nos `tpEmis` 2, 4, 5 e 9 (rejeicao 557); nos 6 e 7 os dois sao opcionais (e
//!   so' valem juntos).
//! * **O que a SVC atende.** O que a pagina "Relacao de Servicos Web" lista para ela: a SVC-AN tem status, consulta,
//!   evento, autorizacao, retorno e inutilizacao; a SVC-RS tem tudo isso MENOS a inutilizacao. O cliente consulta a
//!   propria tabela (`Svc::oferece`) e nao envia a SVC o que ela nao lista.
//!
//! A nota autorizada na SVC e seus eventos vivem na SVC enquanto a SEFAZ de origem esta fora: o cliente consulta e
//! registra evento no autorizador que a chave indica (`tpEmis` 6 ou 7), e no da UF nas demais.

use super::erro::ErroSefaz;
use super::tabela::{self, Servico};
use crate::nfe::vigencias::Ambiente;

/// A norma que distribui as UF entre as duas SVC.
pub const ATO_COTEPE: &str =
    "Ato COTEPE/ICMS 39/12, art. 1o, na redacao do Ato COTEPE/ICMS 110/22 (efeitos a partir de 02/12/2022)";

/// UF atendidas pela SVC-AN.
pub const UF_DA_SVC_AN: [&str; 19] = [
    "AC", "AL", "AP", "CE", "DF", "ES", "MG", "PA", "PB", "PI", "RJ", "RN", "RO", "RR", "RS", "SC",
    "SE", "SP", "TO",
];

/// UF atendidas pela SVC-RS.
pub const UF_DA_SVC_RS: [&str; 8] = ["AM", "BA", "GO", "MA", "MS", "MT", "PE", "PR"];

/// A SEFAZ Virtual de Contingencia.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Svc {
    /// SVC-AN, da Receita Federal (`tpEmis` 6).
    An,
    /// SVC-RS, do Rio Grande do Sul (`tpEmis` 7).
    Rs,
}

impl Svc {
    /// O nome do autorizador na tabela de enderecos.
    #[must_use]
    pub fn autorizador(self) -> &'static str {
        match self {
            Svc::An => "SVC-AN",
            Svc::Rs => "SVC-RS",
        }
    }

    /// O `tpEmis` da NF-e autorizada nesta SVC: 6 na SVC-AN, 7 na SVC-RS.
    #[must_use]
    pub fn tp_emis(self) -> u32 {
        match self {
            Svc::An => 6,
            Svc::Rs => 7,
        }
    }

    /// A SVC que o `tpEmis` indica (6 ou 7).
    #[must_use]
    pub fn do_tp_emis(tp_emis: u32) -> Option<Svc> {
        match tp_emis {
            6 => Some(Svc::An),
            7 => Some(Svc::Rs),
            _ => None,
        }
    }

    /// A SVC lista o servico na tabela do Portal para o ambiente? So' o que a tabela lista e' enviado a ela.
    #[must_use]
    pub fn oferece(self, ambiente: Ambiente, servico: Servico) -> bool {
        tabela::endereco(self.autorizador(), ambiente, servico.nome_na_pagina()).is_ok()
    }
}

/// A SVC que atende a UF (sigla ou codigo IBGE), pelo Ato COTEPE/ICMS 39/12.
///
/// # Errors
///
/// [`ErroSefaz`] se a UF e' desconhecida.
pub fn svc_da_uf(uf: &str) -> Result<Svc, ErroSefaz> {
    let sigla = tabela::uf_valida(uf)?;
    if UF_DA_SVC_RS.contains(&sigla) {
        Ok(Svc::Rs)
    } else {
        Ok(Svc::An)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn as_duas_listas_cobrem_as_27_uf_sem_repetir() {
        let mut todas: Vec<&str> = UF_DA_SVC_AN.iter().chain(&UF_DA_SVC_RS).copied().collect();
        todas.sort_unstable();
        todas.dedup();
        assert_eq!(todas.len(), 27);
        for uf in &todas {
            assert!(tabela::uf_valida(uf).is_ok(), "{uf}");
        }
    }
}
