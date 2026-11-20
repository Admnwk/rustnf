// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Abre o certificado A1 e monta o cliente real da Sefin Nacional.

use std::sync::Arc;
use std::time::Duration;

use rustnf::assinatura::cert_a1;
use rustnf::nfse::sefin::ClienteNfse;
use zeroize::Zeroizing;

use super::{Ambiente, ArgSenha};
use crate::emissao::Emissor;
use crate::entrada;
use crate::erro::Erro;
use crate::segredo::Segredo;

/// Tempo limite de cada chamada a Sefin Nacional.
const TEMPO_SEFIN: Duration = Duration::from_secs(60);

/// A senha, da variavel de ambiente ou do stdin. `stdin_ocupado` avisa que o stdin ja serve a outra entrada.
pub(super) fn senha(arg: &ArgSenha, stdin_ocupado: bool) -> Result<Segredo, Erro> {
    match (&arg.senha_env, arg.senha_stdin) {
        (Some(variavel), false) => entrada::senha_do_ambiente(variavel),
        (None, true) if stdin_ocupado => Err(Erro::Uso(
            "o stdin ja esta lendo o pedido: passe a senha por --senha-env".into(),
        )),
        (None, true) => entrada::senha_do_stdin(),
        _ => Err(Erro::Uso(
            "informe a senha do certificado por --senha-env <VARIAVEL> ou --senha-stdin".into(),
        )),
    }
}

/// Abre o .pfx e monta o cliente da Sefin para o ambiente.
pub(super) fn emissor(
    pfx: &str,
    senha: &Segredo,
    ambiente: Ambiente,
) -> Result<Arc<dyn Emissor>, Erro> {
    if pfx == "-" {
        return Err(Erro::Uso("--pfx precisa de um arquivo, nao de '-'".into()));
    }
    let bytes = Zeroizing::new(entrada::ler_bytes(pfx, "o certificado")?);
    let certificado =
        cert_a1::carregar(&bytes, senha.expor()).map_err(|e| Erro::Entrada(e.to_string()))?;
    let cliente = ClienteNfse::real(certificado, ambiente.codigo(), None, None, TEMPO_SEFIN)?;
    Ok(Arc::new(cliente))
}
