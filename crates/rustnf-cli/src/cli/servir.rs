// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! `rustnf serve`: monta o estado, escuta e para limpo no Ctrl+C.

use std::future::Future;
use std::net::IpAddr;
use std::path::Path;
use std::time::Duration;

use rustnf_app::cofre::Cofre;
use rustnf_app::config::Efetiva;
use tokio::net::TcpListener;

use super::{configuracao, resolver, ArgServe};
use crate::erro::Erro;
use crate::http::{roteador, Config, Estado, ServidorNfe, Token};

/// Ajustes do servidor: opcao > variavel > arquivo > padrao.
fn ajustes(args: &ArgServe, e: &Efetiva) -> Config {
    let s = &e.config.servidor;
    Config {
        limite_corpo: args.limite_corpo.unwrap_or(s.limite_corpo),
        tempo_limite: Duration::from_secs(args.tempo_limite.unwrap_or(s.tempo_limite_s)),
        origens_permitidas: if args.origem_permitida.is_empty() {
            s.origens_permitidas.clone()
        } else {
            args.origem_permitida.clone()
        },
        folga_relogio: args.folga_relogio.unwrap_or(e.config.sefin.folga_relogio_s),
    }
}

fn estado(args: &ArgServe, e: &Efetiva, cofre: &dyn Cofre) -> Result<Estado, Erro> {
    let config = ajustes(args, e);
    if !args.permitir_emissao {
        if args.pfx.is_some() || args.token_env.is_some() {
            return Err(Erro::Uso(
                "--pfx e --token-env so' valem com --permitir-emissao".into(),
            ));
        }
        let nfe = ServidorNfe::sem_emissao(
            e.config.clone(),
            super::nfe::notas_da(e)?,
            "o servidor nao foi iniciado com --permitir-emissao",
        );
        return Ok(Estado::somente_leitura(config)
            .com_aliquotas(e.config.publicadas())
            .com_nfe(nfe));
    }
    let Some(variavel) = &args.token_env else {
        return Err(Erro::Uso(
            "--permitir-emissao exige --token-env (e certificado e ambiente: --pfx e --ambiente, ou o arquivo de configuracao)"
                .into(),
        ));
    };
    let pfx = resolver::pfx(e, args.pfx.as_deref())?;
    let ambiente = resolver::ambiente(e, args.ambiente)?;
    // sem token valido o servidor nao sobe: a emissao nunca fica aberta
    let token = match std::env::var(variavel) {
        Ok(t) if !t.is_empty() => Token::novo(t)?,
        _ => {
            return Err(Erro::Uso(format!(
                "sem token configurado: a variavel {variavel} nao esta definida ou esta vazia"
            )))
        }
    };
    let senha = resolver::senha(e, cofre, &args.senha, &pfx, false)?;
    let emissor = super::certificado::emissor(
        &pfx,
        &senha,
        ambiente,
        resolver::assinatura(e, args.assinatura),
        resolver::tempo_sefin(e),
    )?;
    // a NF-e usa o mesmo certificado e a `[nfe]` do mesmo arquivo de configuracao que a linha de comando le; o ambiente
    // dela e' o de `[nfe].ambiente`, que nao muda com o `--ambiente` da NFS-e
    let certificado = super::certificado::abrir(&pfx, &senha)?;
    let nfe = ServidorNfe::real(e.config.clone(), super::nfe::notas_da(e)?, certificado)?;
    if nfe.pronta() {
        eprintln!(
            "rustnf: NF-e pronta (uf {}, ambiente {})",
            e.config.nfe.uf, e.config.nfe.ambiente
        );
    } else {
        eprintln!("rustnf: NF-e sem emissao: informe [nfe].uf no arquivo de configuracao");
    }
    // o mesmo arquivo de configuracao da linha de comando: emitente, pasta de notas e aliquotas
    Ok(Estado::com_emissao(config, emissor, token)
        .com_aliquotas(e.config.publicadas())
        .com_emitente(e.config.emitente.clone())
        .com_notas(configuracao::notas_da(e)?)
        .com_nfe(nfe))
}

/// Endereco e porta de escuta: opcao > variavel > arquivo > padrao.
fn escuta(args: &ArgServe, e: &Efetiva) -> Result<(IpAddr, u16), Erro> {
    let s = &e.config.servidor;
    let endereco = match args.endereco {
        Some(a) => a,
        None => s
            .endereco
            .parse()
            .map_err(|_| Erro::Uso("[servidor].endereco invalido".into()))?,
    };
    Ok((endereco, args.porta.unwrap_or(s.porta)))
}

/// Escuta ate `parada` terminar.
///
/// # Errors
///
/// [`Erro::Uso`] se as opcoes forem invalidas, [`Erro::Entrada`] se nao der para escutar ou abrir o certificado.
pub async fn servir(args: ArgServe, config: Option<&Path>, cofre: &dyn Cofre) -> Result<(), Erro> {
    let efetiva = configuracao::carregar(config)?;
    servir_ate(args, &efetiva, cofre, async {
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
    efetiva: &Efetiva,
    cofre: &dyn Cofre,
    parada: impl Future<Output = ()> + Send + 'static,
) -> Result<(), Erro> {
    let estado = estado(&args, efetiva, cofre)?;
    let com_emissao = estado.emissao.is_some();
    let (endereco, porta) = escuta(&args, efetiva)?;
    let listener = TcpListener::bind((endereco, porta)).await.map_err(|_| {
        Erro::Entrada("nao foi possivel escutar no endereco e porta pedidos".into())
    })?;
    let local = listener.local_addr().map_err(|_| Erro::Interno)?;
    if !endereco.is_loopback() {
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

#[cfg(test)]
mod testes {
    use std::collections::HashMap;
    use std::ffi::OsString;

    use clap::Parser;
    use rustnf_app::config::Config;

    use super::*;
    use crate::cli::{Cli, Comando};

    fn args(linha: &[&str]) -> ArgServe {
        let mut todos = vec!["rustnf", "serve"];
        todos.extend(linha);
        match Cli::try_parse_from(todos).unwrap().comando {
            Comando::Serve(a) => a,
            outro => panic!("esperava serve: {outro:?}"),
        }
    }

    fn efetiva(arquivo: &str, env: &[(&str, &str)]) -> Efetiva {
        let mapa: HashMap<String, String> = env
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        Efetiva::de(Config::de_texto(arquivo).unwrap(), None, &move |n| {
            mapa.get(n).map(OsString::from)
        })
        .unwrap()
    }

    #[test]
    fn porta_opcao_vence_ambiente_vence_arquivo_vence_padrao() {
        let arq = "[servidor]\nporta = 3000\n";
        let env = [("RUSTNF_PORTA", "2000")];
        assert_eq!(
            escuta(&args(&["--porta", "1000"]), &efetiva(arq, &env))
                .unwrap()
                .1,
            1000
        );
        assert_eq!(escuta(&args(&[]), &efetiva(arq, &env)).unwrap().1, 2000);
        assert_eq!(escuta(&args(&[]), &efetiva(arq, &[])).unwrap().1, 3000);
        assert_eq!(escuta(&args(&[]), &efetiva("", &[])).unwrap().1, 8080);
    }

    #[test]
    fn endereco_segue_a_mesma_ordem() {
        let arq = "[servidor]\nendereco = \"10.0.0.2\"\n";
        let env = [("RUSTNF_ENDERECO", "10.0.0.3")];
        let e = |a: &ArgServe, f: &Efetiva| escuta(a, f).unwrap().0.to_string();
        assert_eq!(
            e(&args(&["--endereco", "10.0.0.1"]), &efetiva(arq, &env)),
            "10.0.0.1"
        );
        assert_eq!(e(&args(&[]), &efetiva(arq, &env)), "10.0.0.3");
        assert_eq!(e(&args(&[]), &efetiva(arq, &[])), "10.0.0.2");
        assert_eq!(e(&args(&[]), &efetiva("", &[])), "127.0.0.1");
    }

    #[test]
    fn folga_limite_tempo_e_origens_seguem_a_ordem() {
        let arq =
            "[sefin]\nfolga_relogio_s = 30\n[servidor]\nlimite_corpo = 5000\ntempo_limite_s = 7\n\
                   origens_permitidas = [\"https://arquivo.exemplo.com.br\"]\n";
        let env = [
            ("RUSTNF_FOLGA_RELOGIO", "20"),
            (
                "RUSTNF_ORIGENS_PERMITIDAS",
                "https://ambiente.exemplo.com.br",
            ),
        ];
        let c = ajustes(
            &args(&["--folga-relogio", "10", "--limite-corpo", "9"]),
            &efetiva(arq, &env),
        );
        assert_eq!(c.folga_relogio, 10, "opcao");
        assert_eq!(c.limite_corpo, 9, "opcao");
        assert_eq!(c.tempo_limite, Duration::from_secs(7), "arquivo");
        assert_eq!(
            c.origens_permitidas,
            ["https://ambiente.exemplo.com.br"],
            "ambiente"
        );

        let c = ajustes(
            &args(&["--origem-permitida", "https://opcao.exemplo.com.br"]),
            &efetiva(arq, &env),
        );
        assert_eq!(c.folga_relogio, 20, "ambiente");
        assert_eq!(
            c.origens_permitidas,
            ["https://opcao.exemplo.com.br"],
            "opcao"
        );

        let c = ajustes(&args(&[]), &efetiva(arq, &[]));
        assert_eq!(c.folga_relogio, 30, "arquivo");
        assert_eq!(c.origens_permitidas, ["https://arquivo.exemplo.com.br"]);

        let c = ajustes(&args(&[]), &efetiva("", &[]));
        assert_eq!((c.folga_relogio, c.limite_corpo), (60, 1024 * 1024));
        assert_eq!(c.tempo_limite, Duration::from_secs(60));
        assert!(c.origens_permitidas.is_empty());
    }
}
