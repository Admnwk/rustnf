// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Ajudantes dos testes: pasta temporaria de nome unico (apagada no fim), certificado de teste e cofre em memoria.
//! Nenhum teste toca o cofre do sistema, a pasta de configuracao do usuario nem a rede.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use chrono::Utc;
use rustnf::assinatura::cert_a1::gerar_teste;
use rustnf_app::cofre::CofreEmMemoria;

#[cfg(windows)]
use crate::servicos::{AbrirArquivos, Destino};
use crate::servicos::{Conexao, Servicos};

static CONTADOR: AtomicU64 = AtomicU64::new(0);

/// Uma pasta nova dentro de `std::env::temp_dir()`, apagada quando o valor e' solto.
pub(crate) struct PastaTemporaria(PathBuf);

impl PastaTemporaria {
    pub(crate) fn nova(rotulo: &str) -> Self {
        let caminho = std::env::temp_dir().join(format!(
            "rustnf-gui-{rotulo}-{}-{}",
            std::process::id(),
            CONTADOR.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&caminho).unwrap();
        Self(caminho)
    }

    pub(crate) fn caminho(&self) -> &Path {
        &self.0
    }

    /// Grava um certificado A1 de teste (autoassinado) na pasta e devolve o caminho.
    pub(crate) fn certificado(
        &self,
        nome: &str,
        documento: &str,
        senha: &str,
        dias: i64,
    ) -> PathBuf {
        let bytes = gerar_teste(
            documento,
            "EMPRESA DE TESTE",
            senha,
            dias,
            false,
            false,
            Utc::now(),
        )
        .unwrap();
        let caminho = self.0.join(nome);
        std::fs::write(&caminho, bytes).unwrap();
        caminho
    }
}

impl Drop for PastaTemporaria {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Servicos para teste: arquivo em `config`, ambiente vazio, cofre em memoria, relogio do sistema.
pub(crate) fn servicos_de_teste(
    config: &Path,
    conexao: Arc<dyn Conexao>,
) -> (Servicos, Arc<CofreEmMemoria>) {
    let cofre = Arc::new(CofreEmMemoria::novo());
    let servicos = Servicos::com(
        Some(config.to_path_buf()),
        Arc::new(|_| None),
        cofre.clone(),
        false,
        conexao,
        Arc::new(Utc::now),
    );
    (servicos, cofre)
}

/// Quem "abre" arquivos nos testes: anota o pedido e nao abre programa nenhum. So' os testes da janela o usam.
#[cfg(windows)]
#[derive(Default)]
pub(crate) struct AbridorDeTeste {
    pedidos: std::sync::Mutex<Vec<(Destino, PathBuf)>>,
}

#[cfg(windows)]
impl AbridorDeTeste {
    /// Os pedidos que a tela fez, na ordem.
    pub(crate) fn pedidos(&self) -> Vec<(Destino, PathBuf)> {
        self.pedidos.lock().map(|p| p.clone()).unwrap_or_default()
    }
}

#[cfg(windows)]
impl AbrirArquivos for AbridorDeTeste {
    fn pedir(&self, destino: Destino, caminho: &Path) -> bool {
        if let Ok(mut p) = self.pedidos.lock() {
            p.push((destino, caminho.to_path_buf()));
        }
        false
    }
}
