// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Trava da pasta de notas: um arquivo criado com exclusividade, para dois processos nao reescreverem o indice ao
//! mesmo tempo.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use super::ErroNotas;

const ESPERA: Duration = Duration::from_millis(50);
const TENTATIVAS: u32 = 100;
/// Trava mais velha que isto e' de um processo que morreu.
const ABANDONADA: Duration = Duration::from_secs(60);

pub(super) struct Trava(PathBuf);

impl Trava {
    pub(super) fn obter(pasta: &Path) -> Result<Self, ErroNotas> {
        let caminho = pasta.join(".trava");
        for _ in 0..TENTATIVAS {
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&caminho)
            {
                Ok(_) => return Ok(Self(caminho)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    if abandonada(&caminho) {
                        let _ = std::fs::remove_file(&caminho);
                    } else {
                        std::thread::sleep(ESPERA);
                    }
                }
                Err(_) => return Err(ErroNotas::Gravacao),
            }
        }
        Err(ErroNotas::Bloqueada)
    }
}

fn abandonada(caminho: &Path) -> bool {
    std::fs::metadata(caminho)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| SystemTime::now().duration_since(t).ok())
        .is_some_and(|idade| idade > ABANDONADA)
}

impl Drop for Trava {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
