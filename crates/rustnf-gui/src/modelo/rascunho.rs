// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O rascunho da Nova nota: o estado do formulario em JSON, com versao, guardado na pasta de notas. Rascunho
//! ilegivel ou de versao desconhecida e' ignorado: a tela comeca vazia, sem erro na cara de ninguem.

use serde::{Deserialize, Serialize};

use super::nota::DadosNota;

/// O nome do arquivo, na pasta de notas.
pub const ARQUIVO: &str = "rascunho.json";

/// A versao do formato do arquivo. Sobe quando um campo muda de sentido.
pub const VERSAO: u32 = 1;

/// Teto de leitura do arquivo: 1 MiB (um rascunho tem poucos KiB).
pub const LIMITE: u64 = 1024 * 1024;

#[derive(Serialize)]
struct Gravacao<'a> {
    versao: u32,
    dados: &'a DadosNota,
}

#[derive(Deserialize)]
struct Leitura {
    versao: u32,
    dados: DadosNota,
}

/// Os bytes do arquivo do rascunho.
#[must_use]
pub fn codificar(dados: &DadosNota) -> Vec<u8> {
    let mut texto = serde_json::to_string_pretty(&Gravacao {
        versao: VERSAO,
        dados,
    })
    .unwrap_or_default();
    texto.push('\n');
    texto.into_bytes()
}

/// Os dados de um arquivo de rascunho; `None` se for ilegivel, nao for um rascunho ou for de outra versao.
#[must_use]
pub fn decodificar(bytes: &[u8]) -> Option<DadosNota> {
    let lido: Leitura = serde_json::from_slice(bytes).ok()?;
    (lido.versao == VERSAO).then_some(lido.dados)
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::modelo::nota::CampoNota;

    fn dados() -> DadosNota {
        let mut d = DadosNota::novo("21/11/2026");
        d.definir(CampoNota::Descricao, "Revisão");
        d.definir(CampoNota::ValorServico, "1234,5");
        d.tributacao = 4;
        d.ind_final = Some(0);
        d
    }

    #[test]
    fn ida_e_volta_guarda_tudo() {
        let d = dados();
        let bytes = codificar(&d);
        assert_eq!(decodificar(&bytes), Some(d));
        let texto = String::from_utf8(bytes).unwrap();
        assert!(texto.contains("\"versao\": 1"));
        assert!(texto.ends_with('\n'));
    }

    #[test]
    fn ilegivel_ou_de_outra_versao_e_ignorado() {
        assert_eq!(decodificar(b""), None);
        assert_eq!(decodificar(b"{ nao e json"), None);
        assert_eq!(decodificar(b"[1,2]"), None);
        assert_eq!(decodificar(b"{\"dados\":{}}"), None, "sem versao");
        assert_eq!(decodificar(b"{\"versao\":2,\"dados\":{}}"), None);
        assert_eq!(decodificar(b"{\"versao\":0,\"dados\":{}}"), None);
        assert_eq!(decodificar(&[0xff, 0xfe, 0x00]), None);
        // versao certa com dados parciais: os campos que faltam ficam no padrao
        let d = decodificar(b"{\"versao\":1,\"dados\":{\"descricao\":\"x\"}}").unwrap();
        assert_eq!(d.descricao, "x");
        assert!(d.ibscbs_ativo);
    }
}
