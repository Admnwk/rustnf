// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O rascunho da Nova NF-e: o assistente em andamento em JSON, com versao, num arquivo proprio da pasta de notas (o da
//! NFS-e e' outro). Rascunho ilegivel ou de versao desconhecida e' ignorado: o assistente comeca vazio, sem erro.

use serde::{Deserialize, Serialize};

use super::nfe_nova::{DadosNfe, PassoNfe};

/// O nome do arquivo, na pasta de notas.
pub const ARQUIVO: &str = "rascunho_nfe.json";

/// A versao do formato do arquivo.
pub const VERSAO: u32 = 1;

/// Teto de leitura do arquivo: 2 MiB.
pub const LIMITE: u64 = 2 * 1024 * 1024;

/// O que o arquivo guarda: os dados e o passo em que a pessoa estava.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rascunho {
    /// Os dados.
    pub dados: DadosNfe,
    /// O passo.
    pub passo: PassoNfe,
}

#[derive(Serialize)]
struct Gravacao<'a> {
    versao: u32,
    #[serde(flatten)]
    rascunho: &'a Rascunho,
}

#[derive(Deserialize)]
struct Leitura {
    versao: u32,
    #[serde(flatten)]
    rascunho: Rascunho,
}

/// Os bytes do arquivo.
#[must_use]
pub fn codificar(rascunho: &Rascunho) -> Vec<u8> {
    let mut texto = serde_json::to_string_pretty(&Gravacao {
        versao: VERSAO,
        rascunho,
    })
    .unwrap_or_default();
    texto.push('\n');
    texto.into_bytes()
}

/// O rascunho de um arquivo; `None` se for ilegivel, nao for um rascunho ou for de outra versao.
#[must_use]
pub fn decodificar(bytes: &[u8]) -> Option<Rascunho> {
    if bytes.len() as u64 > LIMITE {
        return None;
    }
    let lido: Leitura = serde_json::from_slice(bytes).ok()?;
    (lido.versao == VERSAO).then_some(lido.rascunho)
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::modelo::nfe_nova::ItemNfe;

    fn rascunho() -> Rascunho {
        let mut d = DadosNfe {
            nome: "Maria".into(),
            ..DadosNfe::default()
        };
        d.itens.push(ItemNfe {
            descricao: "Cafe".into(),
            icms_codigo: Some("900".into()),
            ..ItemNfe::default()
        });
        Rascunho {
            dados: d,
            passo: PassoNfe::OQueVendeu,
        }
    }

    #[test]
    fn ida_e_volta_guarda_tudo() {
        let r = rascunho();
        let bytes = codificar(&r);
        assert_eq!(decodificar(&bytes), Some(r));
        let texto = String::from_utf8(bytes).unwrap();
        assert!(texto.contains("\"versao\": 1"));
        assert!(texto.ends_with('\n'));
    }

    #[test]
    fn ilegivel_ou_de_outra_versao_e_ignorado() {
        assert_eq!(decodificar(b""), None);
        assert_eq!(decodificar(b"{ nao e json"), None);
        assert_eq!(decodificar(b"[1,2]"), None);
        assert_eq!(
            decodificar(b"{\"dados\":{},\"passo\":\"Conferir\"}"),
            None,
            "sem versao"
        );
        assert_eq!(
            decodificar(b"{\"versao\":2,\"dados\":{},\"passo\":\"Conferir\"}"),
            None
        );
        assert_eq!(decodificar(&[0xff, 0xfe, 0x00]), None);
        // versao certa com dados parciais: o que falta fica no padrao
        let r = decodificar(b"{\"versao\":1,\"dados\":{\"nome\":\"x\"},\"passo\":\"Conferir\"}")
            .unwrap();
        assert_eq!(r.dados.nome, "x");
        assert_eq!(r.dados.parcelas, "2");
        assert_eq!(r.passo, PassoNfe::Conferir);
    }
}
