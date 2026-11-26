// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Gera o DANFE em PDF a partir do XML da NF-e (`nfeProc`, ou a `NFe` solta para uma previa).
//!
//! ```text
//! cargo run -p rustnf --example danfe --features danfe -- nfe_proc.xml danfe.pdf
//! ```

use rustnf::danfe::{self, Opcoes};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let (Some(entrada), Some(saida)) = (args.next(), args.next()) else {
        return Err("uso: danfe <nfe_proc.xml> <saida.pdf>".into());
    };
    let xml = std::fs::read(entrada)?;
    let pdf = danfe::pdf(&xml, &Opcoes::default())?;
    std::fs::write(&saida, pdf)?;
    println!("DANFE gravado em {saida}");
    Ok(())
}
