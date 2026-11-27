// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O rascunho do caixa: o cupom em andamento em JSON, com versao, num arquivo proprio da pasta de notas (o da NF-e e o da
//! NFS-e sao outros). Rascunho ilegivel ou de versao desconhecida e' ignorado: o caixa comeca vazio, sem erro.

use serde::{Deserialize, Serialize};

use super::nfce_caixa::Cupom;

/// O nome do arquivo, na pasta de notas.
pub const ARQUIVO: &str = "rascunho_caixa.json";

/// A versao do formato do arquivo.
pub const VERSAO: u32 = 1;

/// Teto de leitura do arquivo: 2 MiB.
pub const LIMITE: u64 = 2 * 1024 * 1024;

#[derive(Serialize)]
struct Gravacao<'a> {
    versao: u32,
    cupom: &'a Cupom,
}

#[derive(Deserialize)]
struct Leitura {
    versao: u32,
    cupom: Cupom,
}

/// Os bytes do arquivo.
#[must_use]
pub fn codificar(cupom: &Cupom) -> Vec<u8> {
    let mut texto = serde_json::to_string_pretty(&Gravacao {
        versao: VERSAO,
        cupom,
    })
    .unwrap_or_default();
    texto.push('\n');
    texto.into_bytes()
}

/// O cupom de um arquivo; `None` se for ilegivel, nao for um rascunho ou for de outra versao.
#[must_use]
pub fn decodificar(bytes: &[u8]) -> Option<Cupom> {
    if bytes.len() as u64 > LIMITE {
        return None;
    }
    let lido: Leitura = serde_json::from_slice(bytes).ok()?;
    (lido.versao == VERSAO).then_some(lido.cupom)
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::modelo::nfe_fiscal::Pagamento;
    use crate::modelo::nfe_nova::ItemNfe;

    #[test]
    fn ida_e_volta_guarda_o_cupom_inteiro() {
        let mut c = Cupom::default();
        c.itens.push(ItemNfe {
            codigo: "C1".into(),
            descricao: "Cafe".into(),
            valor: "15,90".into(),
            ..ItemNfe::default()
        });
        c.desconto = "1,00".into();
        c.cpf = "52998224725".into();
        c.pagar(Pagamento::Pix, 500);
        let bytes = codificar(&c);
        assert_eq!(decodificar(&bytes), Some(c));
        assert!(String::from_utf8(bytes).unwrap().ends_with('\n'));
    }

    #[test]
    fn ilegivel_ou_de_outra_versao_e_ignorado() {
        assert_eq!(decodificar(b""), None);
        assert_eq!(decodificar(b"{ nao e json"), None);
        assert_eq!(decodificar(b"{\"cupom\":{}}"), None, "sem versao");
        assert_eq!(decodificar(b"{\"versao\":2,\"cupom\":{}}"), None);
        let c = decodificar(b"{\"versao\":1,\"cupom\":{\"cpf\":\"1\"}}").unwrap();
        assert_eq!(c.cpf, "1");
        assert!(c.itens.is_empty());
    }
}
