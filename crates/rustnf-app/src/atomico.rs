// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Gravacao atomica: o arquivo final ou tem o conteudo antigo inteiro ou o novo inteiro, nunca a metade.

use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

static CONTADOR: AtomicU64 = AtomicU64::new(0);

/// Grava `bytes` em `destino`: cria a pasta se faltar, escreve um arquivo temporario na mesma pasta, forca a ida ao
/// disco e troca de nome por cima do destino. Se algo falhar, o temporario e' apagado e o destino fica como estava.
///
/// # Errors
///
/// O erro de E/S da criacao da pasta, da escrita ou da troca de nome.
pub fn gravar(destino: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let pasta = match destino.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };
    std::fs::create_dir_all(pasta)?;
    let nome = destino
        .file_name()
        .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::InvalidInput))?
        .to_string_lossy();
    let temporario = pasta.join(format!(
        ".{nome}.{}.{}.tmp",
        std::process::id(),
        CONTADOR.fetch_add(1, Ordering::Relaxed)
    ));
    let resultado =
        escrever(&temporario, bytes).and_then(|()| std::fs::rename(&temporario, destino));
    if resultado.is_err() {
        let _ = std::fs::remove_file(&temporario);
    }
    resultado
}

fn escrever(temporario: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(temporario)?;
    f.write_all(bytes)?;
    f.sync_all()
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::testes_comuns::PastaTemporaria;

    #[test]
    fn grava_substitui_e_cria_a_pasta() {
        let p = PastaTemporaria::nova("atomico-grava");
        let destino = p.caminho().join("a").join("b").join("x.txt");
        gravar(&destino, b"um").unwrap();
        gravar(&destino, b"dois").unwrap();
        assert_eq!(std::fs::read(&destino).unwrap(), b"dois");
        assert_eq!(
            std::fs::read_dir(destino.parent().unwrap())
                .unwrap()
                .count(),
            1
        );
    }

    #[test]
    fn falha_no_rename_nao_deixa_temporario_nem_mexe_no_destino() {
        let p = PastaTemporaria::nova("atomico-falha");
        // o destino e' uma pasta: o rename falha depois de o temporario ja estar escrito
        let destino = p.caminho().join("destino");
        std::fs::create_dir(&destino).unwrap();
        std::fs::write(destino.join("dentro.txt"), b"intacto").unwrap();
        assert!(gravar(&destino, b"novo").is_err());
        let nomes: Vec<_> = std::fs::read_dir(p.caminho())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(nomes, vec!["destino".to_string()]);
        assert_eq!(
            std::fs::read(destino.join("dentro.txt")).unwrap(),
            b"intacto"
        );
    }
}
