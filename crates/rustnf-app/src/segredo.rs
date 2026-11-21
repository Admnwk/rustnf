// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Segredo em memoria: `Debug` redigido e zerado ao ser solto.

use std::fmt;

use zeroize::Zeroizing;

/// Senha do certificado ou token: nunca aparece em `Debug` nem em mensagem de erro, e e' zerado da memoria ao ser
/// solto.
pub struct Segredo(Zeroizing<String>);

impl Segredo {
    /// Guarda o texto.
    #[must_use]
    pub fn novo(texto: String) -> Self {
        Self(Zeroizing::new(texto))
    }

    /// O texto, para quem precisa dele (abrir o certificado, comparar o token).
    #[must_use]
    pub fn expor(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Segredo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Segredo(<oculto>)")
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn debug_nao_mostra_o_conteudo() {
        let s = Segredo::novo("senha-super-secreta".into());
        let texto = format!("{s:?}");
        assert!(!texto.contains("secreta"));
        assert_eq!(s.expor(), "senha-super-secreta");
    }
}
