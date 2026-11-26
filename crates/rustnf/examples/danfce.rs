// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Gera o DANFC-e de uma NFC-e autorizada (`nfeProc` em XML): o PDF de bobina e os bytes ESC/POS.
//!
//! ```text
//! cargo run -p rustnf --features danfce --example danfce -- nfce.xml cupom.pdf cupom.escpos
//! ```
//!
//! O `.escpos` guarda os bytes prontos para a impressora termica; mande-os a ela pelo canal que o seu sistema usa.

use rustnf::danfce::{escpos, pdf, Opcoes};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let (Some(entrada), Some(saida_pdf), Some(saida_escpos)) =
        (args.next(), args.next(), args.next())
    else {
        eprintln!("uso: danfce <nfce.xml> <cupom.pdf> <cupom.escpos>");
        std::process::exit(2);
    };
    let xml = std::fs::read(entrada)?;
    let op = Opcoes::default();
    std::fs::write(saida_pdf, pdf(&xml, &op)?)?;
    std::fs::write(saida_escpos, escpos(&xml, &op)?)?;
    Ok(())
}
