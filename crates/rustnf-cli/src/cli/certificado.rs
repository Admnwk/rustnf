// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A senha do certificado dada pela chamada e a ligacao com `rustnf_app::certificado`, que abre o A1 e monta o cliente
//! real da Sefin Nacional.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use rustnf::assinatura::xmldsig::Algoritmo;
use rustnf_app::certificado;

use super::{Ambiente, ArgSenha};
use crate::emissao::Emissor;
use crate::entrada;
use crate::erro::Erro;
use crate::segredo::Segredo;

/// A senha dada pela chamada: variavel de ambiente ou stdin. `None` se a chamada nao deu nenhuma das duas.
/// `stdin_ocupado` avisa que o stdin ja serve a outra entrada.
pub(super) fn senha_explicita(
    arg: &ArgSenha,
    stdin_ocupado: bool,
) -> Result<Option<Segredo>, Erro> {
    match (&arg.senha_env, arg.senha_stdin) {
        (Some(variavel), false) => entrada::senha_do_ambiente(variavel).map(Some),
        (None, true) if stdin_ocupado => Err(Erro::Uso(
            "o stdin ja esta lendo o pedido: passe a senha por --senha-env".into(),
        )),
        (None, true) => entrada::senha_do_stdin().map(Some),
        (None, false) => Ok(None),
        _ => Err(Erro::Uso(
            "use so' uma origem de senha: --senha-env ou --senha-stdin".into(),
        )),
    }
}

/// Texto do erro quando nenhuma origem de senha existe.
pub(super) const SEM_SENHA: &str = "informe a senha do certificado por --senha-env <VARIAVEL> ou --senha-stdin (ou guarde-a no cofre do sistema com `rustnf senha guardar`)";

/// So' confere que a senha abre o certificado.
pub(super) fn conferir(pfx: &str, senha: &Segredo) -> Result<(), Erro> {
    certificado::conferir(&arquivo(pfx)?, senha)
}

/// Abre o .pfx e monta o cliente da Sefin para o ambiente.
pub(super) fn emissor(
    pfx: &str,
    senha: &Segredo,
    ambiente: Ambiente,
    algoritmo: Algoritmo,
    tempo_limite: Duration,
) -> Result<Arc<dyn Emissor>, Erro> {
    certificado::emissor(
        &arquivo(pfx)?,
        senha,
        ambiente.codigo(),
        algoritmo,
        tempo_limite,
    )
}

fn arquivo(pfx: &str) -> Result<PathBuf, Erro> {
    if pfx == "-" {
        return Err(Erro::Uso("--pfx precisa de um arquivo, nao de '-'".into()));
    }
    Ok(PathBuf::from(pfx))
}
