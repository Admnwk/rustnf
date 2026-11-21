// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! `rustnf-gui`: a interface de desktop do rustnf. So' existe para Windows nesta versao.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use rustnf_gui::modelo::argumentos::{self, Pedido};

fn main() -> std::process::ExitCode {
    let pedido = match argumentos::interpretar(std::env::args().skip(1)) {
        Ok(p) => p,
        Err(erro) => {
            eprintln!("rustnf-gui: {erro}");
            return std::process::ExitCode::from(2);
        }
    };
    match pedido {
        Pedido::Versao => {
            println!("{}", argumentos::versao());
            std::process::ExitCode::SUCCESS
        }
        Pedido::Ajuda => {
            print!("{}", argumentos::AJUDA);
            std::process::ExitCode::SUCCESS
        }
        Pedido::Abrir { config } => abrir(config),
    }
}

#[cfg(windows)]
fn abrir(config: Option<std::path::PathBuf>) -> std::process::ExitCode {
    rustnf_gui::telas::executar(config);
    std::process::ExitCode::SUCCESS
}

#[cfg(not(windows))]
fn abrir(_config: Option<std::path::PathBuf>) -> std::process::ExitCode {
    eprintln!("rustnf-gui: a interface de desktop so' existe para Windows nesta versao.");
    std::process::ExitCode::from(2)
}
