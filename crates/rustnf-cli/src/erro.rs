// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O erro das operacoes vive em `rustnf-app`; aqui ficam so' o status HTTP de cada variante e a resposta.

use axum::http::header::WWW_AUTHENTICATE;
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};

pub use rustnf_app::erro::Erro;

use crate::http::resposta_json;

/// Status HTTP de um [`Erro`].
pub trait StatusHttp {
    /// Status HTTP da resposta.
    fn status_http(&self) -> u16;
}

impl StatusHttp for Erro {
    fn status_http(&self) -> u16 {
        match self {
            Self::Uso(_) | Self::Entrada(_) => 400,
            Self::NaoAutorizado => 401,
            Self::Recusa { .. } => 422,
            Self::Grande(_) => 413,
            Self::NaoEncontrada(_) => 404,
            Self::Rede(_) => 502,
            // Interno e qualquer variante futura: falha do proprio programa
            _ => 500,
        }
    }
}

/// Um [`Erro`] pronto para virar resposta HTTP com o envelope unico.
#[derive(Debug)]
pub struct ErroHttp(pub Erro);

impl From<Erro> for ErroHttp {
    fn from(e: Erro) -> Self {
        Self(e)
    }
}

impl IntoResponse for ErroHttp {
    fn into_response(self) -> Response {
        let status =
            StatusCode::from_u16(self.0.status_http()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let mut r = resposta_json(status, &self.0.envelope());
        if self.0 == Erro::NaoAutorizado {
            r.headers_mut()
                .insert(WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
        }
        r
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn status_http_de_cada_variante() {
        assert_eq!(Erro::recusa("a", "b").status_http(), 422);
        assert_eq!(Erro::NaoAutorizado.status_http(), 401);
        assert_eq!(Erro::Grande(1).status_http(), 413);
        assert_eq!(Erro::Rede("x".into()).status_http(), 502);
    }
}
