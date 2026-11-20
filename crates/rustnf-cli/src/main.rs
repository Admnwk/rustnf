// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Ponto de entrada do binario `rustnf`.

use std::process::ExitCode;

use clap::Parser;
use rustnf_cli::cli::{executar, Cli};

fn main() -> ExitCode {
    // erro de uso do clap sai com 2; --help e --version, com 0
    let cli = Cli::parse();
    let Ok(runtime) = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    else {
        eprintln!("erro: nao foi possivel iniciar o executor assincrono");
        return ExitCode::from(1);
    };
    match runtime.block_on(executar(cli)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("erro: {e}");
            ExitCode::from(e.codigo_saida())
        }
    }
}
