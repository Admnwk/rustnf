// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! CFOP x finalidade da NF-e (modelo 55) pela Tabela de CFOP do Portal ([`crate::cfop`], `indDevol`), com o codigo da
//! rejeicao. As tres regras valem so' para o modelo 55 (coluna "Modelo" das tres no Anexo I do MOC 7.00 e na
//! NT 2025.002 v1.53); a NFC-e nao as tem.
//!
//! * **I08-140 (327)**, NT 2025.002 v1.53 com a Excecao 1 da NT 2026.009 v1.00: na devolucao (`finNFe` 4) ou na nota
//!   de credito (`finNFe` 5) de `tpNFCredito` 03 ou 06 so' se aceita CFOP de devolucao (`indDevol` 1). Excecao 1: o
//!   1.949 e o 2.949 se aceitam so' quando `finNFe` = 4 (a NT 2026.009 tirou a condicao de destinatario nao
//!   contribuinte da v1.51). Excecao 2: o 5.949 e o 6.949 na devolucao simbolica de gas natural (NCM 27112100, Ajuste
//!   SINIEF 22/21).
//! * **I08-141 (1179)**: emitente MEI (CRT 4) com `idDest` diferente de 3, na mesma situacao da I08-140, so' usa
//!   1.202, 1.553, 2.202, 2.553, 5.202 e 6.202.
//! * **I08-144 (328)**: fora do `finNFe` 2 e 4, o CFOP de devolucao e' recusado; a regra nao vale para a nota de credito
//!   de `tpNFCredito` 03, 04 ou 06.
//!
//! Por item, uma rejeicao so': a 1179 so' sai quando a 327 aceitou o CFOP (a SEFAZ devolve a primeira regra).

use super::modelo::{Item, Nota};
use super::nfce::Rej;
use crate::cfop;

const NCM_GAS_NATURAL: &str = "27112100";

fn digitos(s: &str) -> String {
    s.chars().filter(char::is_ascii_digit).collect()
}

/// `tpNFCredito` com 2 digitos na nota de credito (`finNFe` 5); vazio fora dela ou sem o campo.
fn tipo_credito(n: &Nota) -> String {
    match (&n.tp_nf_credito, n.fin_nfe) {
        (Some(c), 5) if !c.trim().is_empty() => format!("{:0>2}", c.trim()),
        _ => String::new(),
    }
}

fn aceito_na_devolucao(it: &Item, c: &str, fin_nfe: i64) -> bool {
    cfop::e_devolucao(c)
        || (fin_nfe == 4 && (c == "1949" || c == "2949"))
        || ((c == "5949" || c == "6949") && digitos(&it.ncm) == NCM_GAS_NATURAL)
}

/// As rejeicoes 327, 1179 e 328 da nota, por item; vazio fora do modelo 55.
pub(crate) fn regras(n: &Nota) -> Vec<Rej> {
    let mut out = Vec::new();
    if n.modelo != 55 {
        return out;
    }
    let cred = tipo_credito(n);
    let devolve = n.fin_nfe == 4 || cred == "03" || cred == "06";
    for (i, it) in n.itens.iter().enumerate() {
        let (k, c) = (i + 1, digitos(&it.cfop));
        let e_dev = cfop::e_devolucao(&c);
        if devolve {
            if !aceito_na_devolucao(it, &c, n.fin_nfe) {
                out.push(rej(
                    "327",
                    format!("CFOP invalido para Nota Fiscal de devolucao ou de retorno de mercadoria [nItem: {k}]"),
                ));
            } else if n.emit.crt == 4
                && n.id_dest != 3
                && !cfop::CFOP_DEVOLUCAO_MEI.contains(&c.as_str())
            {
                out.push(rej(
                    "1179",
                    format!(
                        "CFOP invalido para Nota Fiscal de devolucao ou de retorno de mercadoria emitida por MEI \
                         [nItem: {k}]"
                    ),
                ));
            }
        } else if e_dev
            && !matches!(n.fin_nfe, 2 | 4)
            && !matches!(cred.as_str(), "03" | "04" | "06")
        {
            out.push(rej(
                "328",
                format!(
                    "CFOP de devolucao de mercadoria para NF-e que nao e de devolucao ou de retorno de mercadoria \
                     [nItem: {k}]"
                ),
            ));
        }
    }
    out
}

fn rej(cstat: &'static str, msg: String) -> Rej {
    Rej {
        cstat,
        campo: "CFOP",
        msg,
    }
}
