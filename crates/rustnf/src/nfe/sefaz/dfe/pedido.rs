// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O pedido `distDFeInt` (versao 1.01) da distribuicao de DF-e. Parte pura: nada aqui faz rede.
//!
//! Fonte: NT 2014.002 v1.02d, item 3.1 (leiaute da mensagem de entrada) e o esquema `distDFeInt_v1.01.xsd` do pacote
//! de liberacao da distribuicao v1.04: `tpAmb`, `cUFAutor` (opcional), `CNPJ` ou `CPF` do interessado e UMA das tres
//! consultas (`distNSU`, `consNSU`, `consChNFe`).

use super::super::erro::ErroSefaz;
use super::super::pedidos::{chave_valida, tp_amb};
use super::super::tabela::codigo_da_uf;
use super::cursor::Nsu;
use crate::chave_dfe;
use crate::nfe::vigencias::Ambiente;
use crate::nfe::NS;

/// Versao do leiaute do pedido (`TVerDistDFe` do esquema).
pub const VERSAO_DISTRIBUICAO: &str = "1.01";

/// Que consulta o pedido faz.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Consulta<'a> {
    /// `distNSU`: tudo o que veio depois do ultimo NSU (ate 50 documentos).
    Distribuicao(Nsu),
    /// `consNSU`: um NSU so', para fechar uma lacuna ou reler um documento ilegivel.
    Nsu(Nsu),
    /// `consChNFe`: a NF-e dessa chave (so' ela, sem eventos).
    Chave(&'a str),
}

/// O interessado: `CNPJ` ou `CPF`, ja conferido.
pub(crate) fn interessado(documento: &str) -> Result<(&'static str, String), ErroSefaz> {
    let d: String = documento
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect::<String>()
        .to_uppercase();
    if d.len() == 11 && d.bytes().all(|b| b.is_ascii_digit()) {
        if !chave_dfe::cpf_valido(&d) {
            return Err(ErroSefaz::novo(
                "CPF do interessado invalido (digito verificador ou zeros, rejeicao 490)",
                "CPF",
            ));
        }
        return Ok(("CPF", d));
    }
    if d.len() == 14 {
        if !chave_dfe::cnpj_valido(&d) {
            return Err(ErroSefaz::novo(
                "CNPJ do interessado invalido (digito verificador ou zeros, rejeicao 489)",
                "CNPJ",
            ));
        }
        return Ok(("CNPJ", d));
    }
    Err(ErroSefaz::novo(
        "o interessado e' um CNPJ (14 posicoes) ou um CPF (11 digitos)",
        "CNPJ",
    ))
}

/// O `distDFeInt` do interessado. `uf_autor` e' a UF de quem consulta (`cUFAutor`).
///
/// # Errors
///
/// [`ErroSefaz`] se o CNPJ ou CPF nao confere, a UF e' desconhecida ou a chave nao e' de uma NF-e (modelo 55) valida.
pub fn dist_dfe_int(
    ambiente: Ambiente,
    uf_autor: &str,
    documento: &str,
    consulta: Consulta<'_>,
) -> Result<String, ErroSefaz> {
    let cuf = codigo_da_uf(uf_autor)?;
    let (tag, doc) = interessado(documento)?;
    let corpo = match consulta {
        Consulta::Distribuicao(ult) => format!("<distNSU><ultNSU>{ult}</ultNSU></distNSU>"),
        Consulta::Nsu(nsu) => format!("<consNSU><NSU>{nsu}</NSU></consNSU>"),
        Consulta::Chave(chave) => {
            let c = chave_valida(chave)?;
            format!("<consChNFe><chNFe>{}</chNFe></consChNFe>", c.chave)
        }
    };
    Ok(format!(
        "<distDFeInt xmlns=\"{NS}\" versao=\"{VERSAO_DISTRIBUICAO}\"><tpAmb>{}</tpAmb><cUFAutor>{cuf}</cUFAutor>\
         <{tag}>{doc}</{tag}>{corpo}</distDFeInt>",
        tp_amb(ambiente)
    ))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod testes {
    use super::*;

    #[test]
    fn o_interessado_e_cnpj_ou_cpf_com_digito() {
        assert_eq!(interessado("11.222.333/0001-81").unwrap().0, "CNPJ");
        assert_eq!(
            interessado("529.982.247-25").unwrap(),
            ("CPF", "52998224725".into())
        );
        assert_eq!(interessado("11222333000182").unwrap_err().campo, "CNPJ");
        assert_eq!(interessado("52998224726").unwrap_err().campo, "CPF");
        assert!(interessado("123").is_err());
        assert!(interessado("").is_err());
    }

    #[test]
    fn distnsu_vai_com_15_digitos() {
        let x = dist_dfe_int(
            Ambiente::Homologacao,
            "SP",
            "11222333000181",
            Consulta::Distribuicao(Nsu::ler("42").unwrap()),
        )
        .unwrap();
        assert!(
            x.contains("<distNSU><ultNSU>000000000000042</ultNSU></distNSU>"),
            "{x}"
        );
        assert!(
            x.contains("<tpAmb>2</tpAmb><cUFAutor>35</cUFAutor><CNPJ>11222333000181</CNPJ>"),
            "{x}"
        );
    }
}
