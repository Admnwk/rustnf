// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A configuracao efetiva: arquivo (ou padrao) com as variaveis de ambiente `RUSTNF_*` por cima. A opcao da
//! chamada, que vence tudo, e' aplicada por quem chama sobre o resultado.

use std::path::{Path, PathBuf};
use std::str::FromStr;

use super::arquivo::{LerAmbiente, VARIAVEL_CONFIG};
use super::{Config, ErroConfig};

/// Variaveis de ambiente que sobrescrevem o arquivo, uma por parametro. `RUSTNF_CONFIG` indica o proprio arquivo.
pub const VARIAVEIS: [&str; 14] = [
    "RUSTNF_AMBIENTE",
    "RUSTNF_PFX",
    "RUSTNF_TEMPO_LIMITE_SEFIN",
    "RUSTNF_FOLGA_RELOGIO",
    "RUSTNF_NOTAS_PASTA",
    "RUSTNF_CANHOTO",
    "RUSTNF_ENDERECO",
    "RUSTNF_PORTA",
    "RUSTNF_LIMITE_CORPO",
    "RUSTNF_TEMPO_LIMITE",
    "RUSTNF_ORIGENS_PERMITIDAS",
    VARIAVEL_CONFIG,
    "RUSTNF_SERIE",
    "RUSTNF_ASSINATURA",
];

/// A configuracao depois da precedencia ambiente > arquivo > padrao.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Efetiva {
    /// Os valores a usar (ja conferidos por [`Config::validar`]).
    pub config: Config,
    /// O arquivo lido, se houve.
    pub arquivo: Option<PathBuf>,
    /// As variaveis de ambiente que mudaram algum valor.
    pub variaveis: Vec<&'static str>,
}

impl Efetiva {
    /// Acha o arquivo (veja [`Config::carregar`]), aplica o ambiente e confere os valores.
    ///
    /// # Errors
    ///
    /// Qualquer [`ErroConfig`].
    pub fn carregar(explicito: Option<&Path>) -> Result<Self, ErroConfig> {
        Self::carregar_com(explicito, &|n| std::env::var_os(n))
    }

    /// Como [`Efetiva::carregar`], lendo o ambiente por `env`.
    ///
    /// # Errors
    ///
    /// Qualquer [`ErroConfig`].
    pub fn carregar_com(
        explicito: Option<&Path>,
        env: LerAmbiente<'_>,
    ) -> Result<Self, ErroConfig> {
        let (config, arquivo) = Config::carregar_com(explicito, env)?;
        Self::de(config, arquivo, env)
    }

    /// Aplica o ambiente a uma configuracao ja lida e confere os valores.
    ///
    /// # Errors
    ///
    /// [`ErroConfig::Variavel`] para variavel com valor que nao serve; [`ErroConfig::Invalido`] da conferencia.
    pub fn de(
        mut config: Config,
        arquivo: Option<PathBuf>,
        env: LerAmbiente<'_>,
    ) -> Result<Self, ErroConfig> {
        let mut variaveis = Vec::new();
        let mut le = |nome: &'static str| -> Result<Option<String>, ErroConfig> {
            match env(nome).filter(|v| !v.is_empty()) {
                None => Ok(None),
                Some(v) => {
                    variaveis.push(nome);
                    v.into_string().map(Some).map_err(|_| ErroConfig::Variavel {
                        nome: nome.into(),
                        motivo: "texto invalido".into(),
                    })
                }
            }
        };
        if let Some(v) = le("RUSTNF_AMBIENTE")? {
            config.sefin.ambiente = v;
        }
        if let Some(v) = le("RUSTNF_PFX")? {
            config.sefin.pfx = v;
        }
        numero(
            le("RUSTNF_TEMPO_LIMITE_SEFIN")?,
            "RUSTNF_TEMPO_LIMITE_SEFIN",
            &mut config.sefin.tempo_limite_s,
        )?;
        numero(
            le("RUSTNF_FOLGA_RELOGIO")?,
            "RUSTNF_FOLGA_RELOGIO",
            &mut config.sefin.folga_relogio_s,
        )?;
        if let Some(v) = le("RUSTNF_ASSINATURA")? {
            config.sefin.assinatura = v;
        }
        if let Some(v) = le("RUSTNF_NOTAS_PASTA")? {
            config.notas.pasta = v;
        }
        numero(
            le("RUSTNF_CANHOTO")?,
            "RUSTNF_CANHOTO",
            &mut config.danfse.canhoto,
        )?;
        if let Some(v) = le("RUSTNF_ENDERECO")? {
            config.servidor.endereco = v;
        }
        numero(
            le("RUSTNF_PORTA")?,
            "RUSTNF_PORTA",
            &mut config.servidor.porta,
        )?;
        numero(
            le("RUSTNF_LIMITE_CORPO")?,
            "RUSTNF_LIMITE_CORPO",
            &mut config.servidor.limite_corpo,
        )?;
        numero(
            le("RUSTNF_TEMPO_LIMITE")?,
            "RUSTNF_TEMPO_LIMITE",
            &mut config.servidor.tempo_limite_s,
        )?;
        if let Some(v) = le("RUSTNF_ORIGENS_PERMITIDAS")? {
            config.servidor.origens_permitidas = v
                .split(',')
                .map(str::trim)
                .filter(|o| !o.is_empty())
                .map(String::from)
                .collect();
        }
        if let Some(v) = le("RUSTNF_SERIE")? {
            config.emitente.serie = v;
        }
        config.validar()?;
        Ok(Self {
            config,
            arquivo,
            variaveis,
        })
    }

    /// `true` se `variavel` mudou algum valor.
    #[must_use]
    pub fn por_variavel(&self, variavel: &str) -> bool {
        self.variaveis.contains(&variavel)
    }
}

fn numero<T: FromStr>(
    texto: Option<String>,
    nome: &str,
    destino: &mut T,
) -> Result<(), ErroConfig> {
    if let Some(t) = texto {
        *destino = t.trim().parse().map_err(|_| ErroConfig::Variavel {
            nome: nome.into(),
            motivo: "valor invalido".into(),
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod testes {
    use std::collections::HashMap;
    use std::ffi::OsString;

    use super::*;

    fn env(pares: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> {
        let mapa: HashMap<String, String> = pares
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        move |n| mapa.get(n).map(OsString::from)
    }

    fn arquivo() -> Config {
        Config::de_texto(
            "[sefin]\nambiente = \"producao\"\npfx = \"do-arquivo.pfx\"\nfolga_relogio_s = 30\n\
             [servidor]\nporta = 9000\n",
        )
        .unwrap()
    }

    #[test]
    fn arquivo_vence_o_padrao() {
        let e = Efetiva::de(arquivo(), None, &env(&[])).unwrap();
        assert_eq!(e.config.sefin.ambiente, "producao");
        assert_eq!(e.config.sefin.pfx, "do-arquivo.pfx");
        assert_eq!(e.config.sefin.folga_relogio_s, 30);
        assert_eq!(e.config.servidor.porta, 9000);
        // o que o arquivo nao diz fica no padrao
        assert_eq!(e.config.servidor.limite_corpo, 1024 * 1024);
        assert!(e.variaveis.is_empty());
    }

    #[test]
    fn ambiente_vence_o_arquivo() {
        let e = Efetiva::de(
            arquivo(),
            None,
            &env(&[
                ("RUSTNF_AMBIENTE", "restrita"),
                ("RUSTNF_PFX", "do-ambiente.pfx"),
                ("RUSTNF_PORTA", "9100"),
                ("RUSTNF_FOLGA_RELOGIO", "5"),
                (
                    "RUSTNF_ORIGENS_PERMITIDAS",
                    "https://a.exemplo.com.br, https://b.exemplo.com.br",
                ),
            ]),
        )
        .unwrap();
        assert_eq!(e.config.sefin.ambiente, "restrita");
        assert_eq!(e.config.sefin.pfx, "do-ambiente.pfx");
        assert_eq!(e.config.servidor.porta, 9100);
        assert_eq!(e.config.sefin.folga_relogio_s, 5);
        assert_eq!(e.config.servidor.origens_permitidas.len(), 2);
        assert!(e.por_variavel("RUSTNF_PORTA"));
        assert!(!e.por_variavel("RUSTNF_ENDERECO"));
    }

    #[test]
    fn variavel_vazia_e_como_ausente() {
        let e = Efetiva::de(arquivo(), None, &env(&[("RUSTNF_PORTA", "")])).unwrap();
        assert_eq!(e.config.servidor.porta, 9000);
        assert!(e.variaveis.is_empty());
    }

    #[test]
    fn variavel_ruim_aponta_o_nome_e_nao_o_valor() {
        let erro = Efetiva::de(
            Config::default(),
            None,
            &env(&[("RUSTNF_PORTA", "tcp-8080")]),
        )
        .unwrap_err();
        assert!(matches!(erro, ErroConfig::Variavel { ref nome, .. } if nome == "RUSTNF_PORTA"));
        assert!(!erro.to_string().contains("tcp-8080"));
        // valor que nao passa na conferencia
        let erro = Efetiva::de(
            Config::default(),
            None,
            &env(&[("RUSTNF_AMBIENTE", "teste")]),
        )
        .unwrap_err();
        assert!(matches!(erro, ErroConfig::Invalido { .. }));
    }
}
