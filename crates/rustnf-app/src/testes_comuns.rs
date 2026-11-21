// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Ajudantes dos testes unitarios: pasta temporaria de nome unico, apagada no fim.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static CONTADOR: AtomicU64 = AtomicU64::new(0);

/// Uma pasta nova dentro de `std::env::temp_dir()`, apagada quando o valor e' solto.
pub(crate) struct PastaTemporaria(PathBuf);

impl PastaTemporaria {
    pub(crate) fn nova(rotulo: &str) -> Self {
        let caminho = std::env::temp_dir().join(format!(
            "rustnf-app-{rotulo}-{}-{}",
            std::process::id(),
            CONTADOR.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&caminho).unwrap();
        Self(caminho)
    }

    pub(crate) fn caminho(&self) -> &Path {
        &self.0
    }

    pub(crate) fn texto(&self) -> String {
        self.0.to_string_lossy().into_owned()
    }

    /// Grava um arquivo de texto na pasta e devolve o caminho.
    pub(crate) fn escrever(&self, nome: &str, conteudo: &str) -> PathBuf {
        let c = self.0.join(nome);
        std::fs::write(&c, conteudo).unwrap();
        c
    }
}

impl Drop for PastaTemporaria {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
