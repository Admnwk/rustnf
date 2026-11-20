// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Leitura de arquivo, de stdin e de senha. As mensagens de erro nao citam o caminho.

use std::io::{BufRead, Read};
use std::path::Path;

use crate::erro::Erro;
use crate::segredo::Segredo;

/// Teto de um arquivo de entrada (pedido, XML da NFS-e ou .pfx): 16 MiB.
pub const LIMITE_ARQUIVO: u64 = 16 * 1024 * 1024;

fn ler_limitado(mut origem: impl Read, o_que: &str) -> Result<Vec<u8>, Erro> {
    let mut bytes = Vec::new();
    origem
        .by_ref()
        .take(LIMITE_ARQUIVO + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Erro::Entrada(format!("nao foi possivel ler {o_que}")))?;
    if bytes.len() as u64 > LIMITE_ARQUIVO {
        return Err(Erro::Entrada(format!("{o_que} passa de 16 MiB")));
    }
    Ok(bytes)
}

/// Bytes de um arquivo; `-` le do stdin.
///
/// # Errors
///
/// [`Erro::Entrada`] se nao existir, nao abrir ou passar do teto.
pub fn ler_bytes(arquivo: &str, o_que: &str) -> Result<Vec<u8>, Erro> {
    if arquivo == "-" {
        return ler_limitado(std::io::stdin().lock(), o_que);
    }
    let f = std::fs::File::open(Path::new(arquivo)).map_err(|e| {
        Erro::Entrada(match e.kind() {
            std::io::ErrorKind::NotFound => format!("{o_que}: arquivo nao encontrado"),
            _ => format!("nao foi possivel abrir {o_que}"),
        })
    })?;
    ler_limitado(f, o_que)
}

/// Texto UTF-8 de um arquivo; `-` le do stdin.
///
/// # Errors
///
/// [`Erro::Entrada`] como em [`ler_bytes`], ou se nao for UTF-8.
pub fn ler_texto(arquivo: &str, o_que: &str) -> Result<String, Erro> {
    String::from_utf8(ler_bytes(arquivo, o_que)?)
        .map_err(|_| Erro::Entrada(format!("{o_que} nao e' texto UTF-8")))
}

/// Senha de uma variavel de ambiente.
///
/// # Errors
///
/// [`Erro::Uso`] se a variavel nao existir ou estiver vazia.
pub fn senha_do_ambiente(variavel: &str) -> Result<Segredo, Erro> {
    match std::env::var(variavel) {
        Ok(v) if !v.is_empty() => Ok(Segredo::novo(v)),
        _ => Err(Erro::Uso(format!(
            "a variavel de ambiente {variavel} nao esta definida ou esta vazia"
        ))),
    }
}

/// Senha da primeira linha do stdin.
///
/// # Errors
///
/// [`Erro::Uso`] se o stdin vier vazio.
pub fn senha_do_stdin() -> Result<Segredo, Erro> {
    let mut linha = String::new();
    std::io::stdin()
        .lock()
        .read_line(&mut linha)
        .map_err(|_| Erro::Uso("nao foi possivel ler a senha do stdin".into()))?;
    let senha = linha.trim_end_matches(['\r', '\n']).to_string();
    if senha.is_empty() {
        return Err(Erro::Uso("a senha do stdin veio vazia".into()));
    }
    Ok(Segredo::novo(senha))
}
