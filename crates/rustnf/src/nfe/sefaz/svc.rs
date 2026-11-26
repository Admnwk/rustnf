// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Contingencia SVC: qual SEFAZ Virtual de Contingencia atende cada UF, o que a NF-e emitida nela leva e o que a SVC
//! atende.
//!
//! * **Quem atende quem.** O Ato COTEPE/ICMS 39/12, art. 1o, na redacao do Ato COTEPE/ICMS 110/22 (efeitos a partir de
//!   02/12/2022), da a SVC-AN (Receita Federal) a AC, AL, AP, CE, ES, MG, PA, PB, PI, RJ, RS, RN, RO, RR, SC, SE, SP,
//!   TO e DF, e a SVC-RS as UF AM, BA, GO, MA, MT, MS, PE e PR. Pagina do Confaz lida em 25/11/2026:
//!   <https://www.confaz.fazenda.gov.br/legislacao/atos/2012/ac039_12>. A pagina "Relacao de Servicos Web" de producao,
//!   lida no mesmo dia, lista as mesmas UF em cada SVC. **Divergencia medida:** a de HOMOLOGACAO lista o PI na SVC-RS; o
//!   cliente segue o Ato e, em PI e homologacao, aceita tambem o `tpEmis` 7 ([`homologacao_diverge_do_ato`]).
//! * **O que a nota leva.** `tpEmis` 6 na SVC-AN e 7 na SVC-RS (MOC 7.00, Anexo I, regra B25-40, rejeicao 713: a SVC
//!   nao aceita `tpEmis` diferente do da propria SVC; Anexo III, 2.1.3.5). A NFC-e nao e' autorizada pela SVC (rejeicao
//!   783). O Anexo III manda gerar a NF-e com o motivo (`xJust`) e o inicio (`dhCont`) da contingencia, que saem no
//!   DANFE; o Anexo I so' tem rejeicao para a falta deles nos `tpEmis` 2, 4, 5 e 9 (557), e a montagem exige os dois
//!   nos 6 e 7 sem codigo de rejeicao.
//! * **O que a SVC atende** (MOC 7.00, Anexo III, 2.1.3.4, lido em 26/11/2026 em
//!   <https://www.confaz.fazenda.gov.br/legislacao/arquivo-manuais/moc7-anexo-iii-manual-contingencia-nf-e.pdf>):
//!   autorizacao e retorno do lote; **cancelamento (110111) so' da nota autorizada na propria SVC**; consulta da
//!   situacao so' da nota autorizada na propria SVC; status. **Carta de correcao e os demais eventos nao** ("inicialmente
//!   nao sera disponibilizado", item d) e **a inutilizacao nao deve ser oferecida pela SVC** (item e): o que se queira
//!   inutilizar durante a contingencia fica represado para a SEFAZ de origem. O cancelamento de nota autorizada no
//!   ambiente normal tambem fica represado para a SEFAZ de origem (item c).
//!
//! A nota autorizada na SVC e o cancelamento dela vivem na SVC enquanto a SEFAZ de origem esta fora: o cliente consulta e
//! cancela no autorizador que a chave indica (`tpEmis` 6 ou 7), e no da UF nas demais. O Portal lista `NfeInutilizacao`
//! na SVC-AN, contra o item e do Anexo III: vale o manual, e [`Svc::oferece`] diz que nenhuma SVC oferece a inutilizacao.

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

    /// A SVC oferece o servico no ambiente: o MOC (Anexo III, 2.1.3.4) a limita a autorizacao, retorno, evento,
    /// consulta e status (nunca a inutilizacao) e o endereco tem de estar na tabela do Portal. O `RecepcaoEvento` so'
    /// serve ao cancelamento: veja [`Svc::atende_evento`].
    #[must_use]
    pub fn oferece(self, ambiente: Ambiente, servico: Servico) -> bool {
        servico != Servico::Inutilizacao
            && tabela::endereco(self.autorizador(), ambiente, servico.nome_na_pagina()).is_ok()
    }

    /// A SVC registra este evento? So' o cancelamento (110111), e so' da nota autorizada nela (Anexo III, 2.1.3.4, itens
    /// c e d). Carta de correcao e os demais tipos nao.
    #[must_use]
    pub fn atende_evento(tp_evento: &str) -> bool {
        tp_evento == "110111"
    }
}

/// A pagina "Relacao de Servicos Web" de HOMOLOGACAO, lida em 25/11/2026, lista o PI na SVC-RS; a de producao e o Ato
/// listam o PI na SVC-AN. O pedido sai para a SVC que o `tpEmis` da nota diz, e a conferencia local nao recusa, em PI e
/// homologacao, o `tpEmis` 7 que o Ato nao previu.
#[must_use]
pub fn homologacao_diverge_do_ato(uf: &str, ambiente: Ambiente, tp_emis: u32) -> bool {
    ambiente == Ambiente::Homologacao && tabela::uf_valida(uf) == Ok("PI") && tp_emis == 7
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
