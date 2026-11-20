// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Compilacao do modelo Typst em PDF, dentro do processo: sem programa externo e sem fonte do sistema. As fontes e o
//! logotipo entram no binario por `include_bytes!`.

use serde_json::Value;
use typst::foundations::{Dict, IntoValue, Str};
use typst_as_lib::TypstEngine;
use typst_layout::PagedDocument;

use super::ErroDanfse;

/// Modelo Typst: le `sys.inputs.dados` (o JSON de [`super::dados`]) e executa as operacoes de `pagina`.
const MODELO: &str = include_str!("modelo.typ");
/// Liberation Sans (metrica da Arial, NT 008 item 2.4): regular e negrito, licenca SIL OFL 1.1.
const FONTES: [&[u8]; 2] = [
    include_bytes!("../../dados/fontes/LiberationSans-Regular.ttf"),
    include_bytes!("../../dados/fontes/LiberationSans-Bold.ttf"),
];
/// Logotipo da NFS-e (NT 008 item 2.4.3).
const LOGO: &[u8] = include_bytes!("../../dados/danfse_logo_nfse.png");
/// Nome com que o modelo abre o logotipo.
const ARQUIVO_LOGO: &str = "logo.png";

fn erro(motivo: impl std::fmt::Display) -> ErroDanfse {
    ErroDanfse::Pdf(motivo.to_string())
}

pub(super) fn pdf(dados: &Value) -> Result<Vec<u8>, ErroDanfse> {
    let mut entradas = Dict::new();
    entradas.insert(
        Str::from("dados"),
        Str::from(dados.to_string().as_str()).into_value(),
    );
    let motor = TypstEngine::builder()
        .main_file(MODELO)
        .fonts(FONTES)
        .with_static_file_resolver([(ARQUIVO_LOGO, LOGO)])
        .build();
    let documento: PagedDocument = motor.compile_with_input(entradas).output.map_err(erro)?;
    typst_pdf::pdf(&documento, &typst_pdf::PdfOptions::default()).map_err(|diagnosticos| {
        erro(
            diagnosticos
                .iter()
                .map(|d| d.message.as_str())
                .collect::<Vec<_>>()
                .join("; "),
        )
    })
}
