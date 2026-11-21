// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Os argumentos do `rustnf-gui`: so' `--config <arquivo>`, `--version` e `--help`, lidos a mao (sem `clap`).

use std::path::PathBuf;

/// O que a linha de comando pediu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pedido {
    /// Abrir a janela, com o arquivo de configuracao indicado (ou o padrao).
    Abrir {
        /// Arquivo de `--config`.
        config: Option<PathBuf>,
    },
    /// Mostrar a versao e sair.
    Versao,
    /// Mostrar a ajuda e sair.
    Ajuda,
}

/// A ajuda, em texto puro.
pub const AJUDA: &str = "rustnf-gui: interface de desktop do rustnf (somente Windows).\n\n\
Uso: rustnf-gui [--config <arquivo>]\n\n\
Opcoes:\n  \
--config <arquivo>  arquivo de configuracao (o mesmo que o rustnf usa); sem a opcao vale a\n                      \
variavel RUSTNF_CONFIG e, na falta dela, o local padrao do usuario\n  \
-V, --version       mostra a versao\n  \
-h, --help          mostra esta ajuda\n\n\
RUSTNF_COFRE=desligado desliga o cofre de senhas: a senha do certificado nao e' guardada.\n";

/// A versao, uma linha.
#[must_use]
pub fn versao() -> String {
    format!("rustnf-gui {}", env!("CARGO_PKG_VERSION"))
}

/// Interpreta os argumentos (sem o nome do programa).
///
/// # Errors
///
/// Texto de uso para a opcao desconhecida, repetida ou sem valor. Nunca repete o valor dado a `--config`.
pub fn interpretar<I>(args: I) -> Result<Pedido, String>
where
    I: IntoIterator<Item = String>,
{
    let mut config: Option<PathBuf> = None;
    let mut it = args.into_iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "-h" | "--help" => return Ok(Pedido::Ajuda),
            "-V" | "--version" => return Ok(Pedido::Versao),
            "--config" => {
                let valor = it
                    .next()
                    .filter(|v| !v.is_empty())
                    .ok_or("a opcao --config precisa de um arquivo")?;
                if config.replace(PathBuf::from(valor)).is_some() {
                    return Err("a opcao --config foi dada duas vezes".into());
                }
            }
            outro => {
                if let Some(valor) = outro.strip_prefix("--config=") {
                    if valor.is_empty() {
                        return Err("a opcao --config precisa de um arquivo".into());
                    }
                    if config.replace(PathBuf::from(valor)).is_some() {
                        return Err("a opcao --config foi dada duas vezes".into());
                    }
                } else if outro.starts_with('-') {
                    return Err(format!("opcao desconhecida: {outro} (veja --help)"));
                } else {
                    return Err("argumento inesperado (veja --help)".into());
                }
            }
        }
    }
    Ok(Pedido::Abrir { config })
}

#[cfg(test)]
mod testes {
    use super::*;

    fn args(a: &[&str]) -> Result<Pedido, String> {
        interpretar(a.iter().map(|s| (*s).to_string()))
    }

    #[test]
    fn sem_argumento_abre_com_o_local_padrao() {
        assert_eq!(args(&[]), Ok(Pedido::Abrir { config: None }));
    }

    #[test]
    fn config_com_valor_separado_ou_com_igual() {
        let esperado = Ok(Pedido::Abrir {
            config: Some(PathBuf::from("a b.toml")),
        });
        assert_eq!(args(&["--config", "a b.toml"]), esperado);
        assert_eq!(args(&["--config=a b.toml"]), esperado);
    }

    #[test]
    fn versao_e_ajuda_valem_em_qualquer_posicao() {
        assert_eq!(args(&["--version"]), Ok(Pedido::Versao));
        assert_eq!(args(&["-V"]), Ok(Pedido::Versao));
        assert_eq!(args(&["--help"]), Ok(Pedido::Ajuda));
        assert_eq!(args(&["--config", "x.toml", "-h"]), Ok(Pedido::Ajuda));
        assert!(versao().starts_with("rustnf-gui "));
    }

    #[test]
    fn o_resto_e_recusado_com_uma_mensagem_de_uso() {
        assert!(args(&["--config"]).is_err());
        assert!(args(&["--config="]).is_err());
        assert!(args(&["--config", ""]).is_err());
        assert!(args(&["--config", "a", "--config", "b"]).is_err());
        assert!(args(&["--outra"]).unwrap_err().contains("--outra"));
        let erro = args(&["arquivo-solto.toml"]).unwrap_err();
        assert!(!erro.contains("arquivo-solto"));
    }

    #[test]
    fn a_ajuda_e_ascii() {
        assert!(AJUDA.is_ascii());
        assert!(AJUDA.contains("--config"));
    }
}
