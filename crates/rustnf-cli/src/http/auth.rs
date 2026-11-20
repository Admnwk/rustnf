// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Token de acesso das rotas de emissao: `Authorization: Bearer <token>`, comparado em tempo constante.

use axum::http::header::AUTHORIZATION;
use axum::http::HeaderMap;
use subtle::ConstantTimeEq;

use crate::erro::Erro;
use crate::segredo::Segredo;

/// Token das rotas que emitem, consultam e cancelam.
#[derive(Debug)]
pub struct Token(Segredo);

impl Token {
    /// Tamanho minimo, em caracteres.
    pub const MINIMO: usize = 32;

    /// Guarda o token se tiver ao menos [`Token::MINIMO`] caracteres.
    ///
    /// # Errors
    ///
    /// [`Erro::Uso`] se for curto demais.
    pub fn novo(texto: String) -> Result<Self, Erro> {
        if texto.chars().count() < Self::MINIMO {
            return Err(Erro::Uso(format!(
                "o token precisa ter ao menos {} caracteres",
                Self::MINIMO
            )));
        }
        Ok(Self(Segredo::novo(texto)))
    }

    /// Compara com o apresentado em tempo constante (so' o tamanho pode vazar).
    #[must_use]
    pub fn confere(&self, apresentado: &str) -> bool {
        self.0
            .expor()
            .as_bytes()
            .ct_eq(apresentado.as_bytes())
            .into()
    }
}

/// O valor do `Authorization: Bearer`, se o cabecalho existir nesse formato.
#[must_use]
pub fn bearer(cabecalhos: &HeaderMap) -> Option<&str> {
    let valor = cabecalhos.get(AUTHORIZATION)?.to_str().ok()?;
    let (esquema, token) = valor.split_once(' ')?;
    esquema.eq_ignore_ascii_case("bearer").then(|| token.trim())
}

#[cfg(test)]
mod testes {
    use axum::http::HeaderValue;

    use super::*;

    fn token() -> Token {
        Token::novo("a".repeat(40)).unwrap()
    }

    #[test]
    fn token_curto_e_recusado() {
        assert!(Token::novo("curto".into()).is_err());
        assert!(Token::novo("x".repeat(31)).is_err());
        assert!(Token::novo("x".repeat(32)).is_ok());
    }

    #[test]
    fn confere_so_o_token_certo() {
        let t = token();
        assert!(t.confere(&"a".repeat(40)));
        assert!(!t.confere(&"a".repeat(39)));
        assert!(!t.confere(&"b".repeat(40)));
        assert!(!t.confere(""));
    }

    #[test]
    fn debug_do_token_e_redigido() {
        assert!(!format!("{:?}", token()).contains("aaaa"));
    }

    #[test]
    fn le_o_bearer() {
        let mut h = HeaderMap::new();
        assert_eq!(bearer(&h), None);
        h.insert(AUTHORIZATION, HeaderValue::from_static("Bearer abc"));
        assert_eq!(bearer(&h), Some("abc"));
        h.insert(AUTHORIZATION, HeaderValue::from_static("bearer abc"));
        assert_eq!(bearer(&h), Some("abc"));
        h.insert(AUTHORIZATION, HeaderValue::from_static("Basic abc"));
        assert_eq!(bearer(&h), None);
    }
}
