// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! `rustnf serve`: monta o estado, escuta e para limpo no Ctrl+C.

use std::future::Future;
use std::time::Duration;

use tokio::net::TcpListener;

use super::{certificado, ArgServe};
use crate::erro::Erro;
use crate::http::{roteador, Config, Estado, Token};

fn estado(args: &ArgServe) -> Result<Estado, Erro> {
    let config = Config {
        limite_corpo: args.limite_corpo,
        tempo_limite: Duration::from_secs(args.tempo_limite),
        origens_permitidas: args.origem_permitida.clone(),
        folga_relogio: args.folga_relogio,
    };
    if !args.permitir_emissao {
        if args.pfx.is_some() || args.token_env.is_some() {
            return Err(Erro::Uso(
                "--pfx e --token-env so' valem com --permitir-emissao".into(),
            ));
        }
        return Ok(Estado::somente_leitura(config));
    }
    let (Some(pfx), Some(ambiente), Some(variavel)) = (&args.pfx, args.ambiente, &args.token_env)
    else {
        return Err(Erro::Uso(
            "--permitir-emissao exige --pfx, --ambiente e --token-env".into(),
        ));
    };
    // sem token valido o servidor nao sobe: a emissao nunca fica aberta
    let token = match std::env::var(variavel) {
        Ok(t) if !t.is_empty() => Token::novo(t)?,
        _ => {
            return Err(Erro::Uso(format!(
                "sem token configurado: a variavel {variavel} nao esta definida ou esta vazia"
            )))
        }
    };
    let senha = certificado::senha(&args.senha, false)?;
    let emissor = certificado::emissor(pfx, &senha, ambiente)?;
    Ok(Estado::com_emissao(config, emissor, token))
}

/// Escuta ate `parada` terminar.
///
/// # Errors
///
/// [`Erro::Uso`] se as opcoes forem invalidas, [`Erro::Entrada`] se nao der para escutar ou abrir o certificado.
pub async fn servir(args: ArgServe) -> Result<(), Erro> {
    servir_ate(args, async {
        let _ = tokio::signal::ctrl_c().await;
    })
    .await
}

/// Como [`servir`], com a condicao de parada dada por quem chama.
///
/// # Errors
///
/// Os mesmos de [`servir`].
pub async fn servir_ate(
    args: ArgServe,
    parada: impl Future<Output = ()> + Send + 'static,
) -> Result<(), Erro> {
    let estado = estado(&args)?;
    let com_emissao = estado.emissao.is_some();
    let listener = TcpListener::bind((args.endereco, args.porta))
        .await
        .map_err(|_| {
            Erro::Entrada("nao foi possivel escutar no endereco e porta pedidos".into())
        })?;
    let local = listener.local_addr().map_err(|_| Erro::Interno)?;
    if !args.endereco.is_loopback() {
        eprintln!("aviso: fora do proprio computador o trafego nao e' cifrado; ponha um proxy com TLS na frente");
    }
    eprintln!(
        "rustnf: escutando em http://{local}{}",
        if com_emissao {
            " (emissao habilitada)"
        } else {
            ""
        }
    );
    axum::serve(listener, roteador(estado))
        .with_graceful_shutdown(parada)
        .await
        .map_err(|_| Erro::Interno)?;
    eprintln!("rustnf: servidor parado");
    Ok(())
}
