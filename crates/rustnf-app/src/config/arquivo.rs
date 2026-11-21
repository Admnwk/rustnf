// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Onde o arquivo mora, como se le e como se grava.

use std::ffi::OsString;
use std::io::Read;
use std::path::{Path, PathBuf};

use super::erro::Indicado;
use super::{Config, ErroConfig};
use crate::atomico;

/// Variavel de ambiente que indica o arquivo de configuracao.
pub const VARIAVEL_CONFIG: &str = "RUSTNF_CONFIG";

/// Teto do arquivo: 1 MiB.
const LIMITE_ARQUIVO: u64 = 1024 * 1024;

/// Nome do arquivo no local padrao.
const NOME_ARQUIVO: &str = "rustnf.toml";

/// Leitura de variavel de ambiente, para os testes poderem trocar o ambiente.
pub type LerAmbiente<'a> = &'a dyn Fn(&str) -> Option<OsString>;

fn nao_vazia(env: LerAmbiente<'_>, nome: &str) -> Option<OsString> {
    env(nome).filter(|v| !v.is_empty())
}

/// O local padrao do arquivo: no Windows `%APPDATA%\rustnf\rustnf.toml`; nos outros sistemas
/// `$XDG_CONFIG_HOME/rustnf/rustnf.toml` ou `$HOME/.config/rustnf/rustnf.toml`. `None` se as variaveis faltarem.
#[must_use]
pub fn caminho_padrao(env: LerAmbiente<'_>) -> Option<PathBuf> {
    caminho_padrao_de(cfg!(windows), env)
}

fn caminho_padrao_de(windows: bool, env: LerAmbiente<'_>) -> Option<PathBuf> {
    let base = if windows {
        PathBuf::from(nao_vazia(env, "APPDATA")?)
    } else if let Some(xdg) = nao_vazia(env, "XDG_CONFIG_HOME") {
        PathBuf::from(xdg)
    } else {
        PathBuf::from(nao_vazia(env, "HOME")?).join(".config")
    };
    Some(base.join("rustnf").join(NOME_ARQUIVO))
}

fn ler_texto(caminho: &Path) -> Result<String, ErroConfig> {
    let arquivo = std::fs::File::open(caminho).map_err(|_| ErroConfig::Leitura)?;
    let mut bytes = Vec::new();
    arquivo
        .take(LIMITE_ARQUIVO + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| ErroConfig::Leitura)?;
    if bytes.len() as u64 > LIMITE_ARQUIVO {
        return Err(ErroConfig::Leitura);
    }
    String::from_utf8(bytes).map_err(|_| ErroConfig::Leitura)
}

/// O caminho do arquivo e de onde o caminho veio: a opcao, `RUSTNF_CONFIG` ou o local padrao (`None` na 3a origem).
fn localizar(
    explicito: Option<&Path>,
    env: LerAmbiente<'_>,
) -> (Option<PathBuf>, Option<Indicado>) {
    if let Some(c) = explicito {
        (Some(c.to_path_buf()), Some(Indicado::Opcao))
    } else if let Some(v) = nao_vazia(env, VARIAVEL_CONFIG) {
        (Some(PathBuf::from(v)), Some(Indicado::Variavel))
    } else {
        (caminho_padrao(env), None)
    }
}

/// O arquivo de configuracao que seria usado, exista ou nao: a opcao, `RUSTNF_CONFIG` ou o local padrao.
#[must_use]
pub fn caminho_do_arquivo(explicito: Option<&Path>, env: LerAmbiente<'_>) -> Option<PathBuf> {
    localizar(explicito, env).0
}

fn linha_do_erro(texto: &str, posicao: usize) -> usize {
    let ate = &texto.as_bytes()[..posicao.min(texto.len())];
    ate.iter().filter(|b| **b == b'\n').count() + 1
}

impl Config {
    /// Le o arquivo TOML. Nao confere os valores: veja [`Config::validar`].
    ///
    /// # Errors
    ///
    /// [`ErroConfig::Leitura`] ou [`ErroConfig::Sintaxe`]; a mensagem nunca traz o trecho do arquivo.
    pub fn ler(caminho: &Path) -> Result<Self, ErroConfig> {
        Self::de_texto(&ler_texto(caminho)?)
    }

    /// Le a configuracao de um texto TOML.
    ///
    /// # Errors
    ///
    /// [`ErroConfig::Sintaxe`] para TOML invalido, campo desconhecido (inclusive `senha` e `token`) ou tipo errado.
    pub fn de_texto(texto: &str) -> Result<Self, ErroConfig> {
        toml::from_str(texto).map_err(|e| ErroConfig::Sintaxe {
            linha: e.span().map(|s| linha_do_erro(texto, s.start)),
            mensagem: e.message().to_string(),
        })
    }

    /// Acha e le o arquivo: 1o o caminho explicito (inexistente e' erro), 2o a variavel `RUSTNF_CONFIG`
    /// (inexistente e' erro), 3o o local padrao, que se nao existir devolve [`Config::default`] sem erro. Devolve
    /// tambem o caminho do arquivo lido, se houve.
    ///
    /// # Errors
    ///
    /// [`ErroConfig::NaoEncontrado`], [`ErroConfig::Leitura`] ou [`ErroConfig::Sintaxe`].
    pub fn carregar(explicito: Option<&Path>) -> Result<(Self, Option<PathBuf>), ErroConfig> {
        Self::carregar_com(explicito, &|n| std::env::var_os(n))
    }

    /// Como [`Config::carregar`], lendo o ambiente por `env`.
    ///
    /// # Errors
    ///
    /// Os mesmos de [`Config::carregar`].
    pub fn carregar_com(
        explicito: Option<&Path>,
        env: LerAmbiente<'_>,
    ) -> Result<(Self, Option<PathBuf>), ErroConfig> {
        let (caminho, indicado) = localizar(explicito, env);
        let Some(caminho) = caminho else {
            return Ok((Self::default(), None));
        };
        if !caminho.is_file() {
            return match indicado {
                Some(i) => Err(ErroConfig::NaoEncontrado(i)),
                None => Ok((Self::default(), None)),
            };
        }
        Ok((Self::ler(&caminho)?, Some(caminho)))
    }

    /// Grava o arquivo (depois de conferir os valores), de forma atomica: um arquivo temporario na mesma pasta e um
    /// `rename`. Cria a pasta se faltar. Comentarios de um arquivo anterior nao sobrevivem.
    ///
    /// # Errors
    ///
    /// [`ErroConfig::Invalido`] se os valores nao servirem; [`ErroConfig::Gravacao`] se nao der para gravar.
    pub fn salvar(&self, caminho: &Path) -> Result<(), ErroConfig> {
        self.validar()?;
        atomico::gravar(caminho, self.para_toml()?.as_bytes()).map_err(|_| ErroConfig::Gravacao)
    }

    /// O texto TOML da configuracao (o que [`Config::salvar`] grava). Nao tem segredo: o arquivo nao tem campo para
    /// isso.
    ///
    /// # Errors
    ///
    /// [`ErroConfig::Gravacao`] se o TOML nao puder ser escrito.
    pub fn para_toml(&self) -> Result<String, ErroConfig> {
        toml::to_string_pretty(self).map_err(|_| ErroConfig::Gravacao)
    }
}

#[cfg(test)]
mod testes {
    use std::collections::HashMap;

    use super::*;
    use crate::config::Aliquota;
    use crate::testes_comuns::PastaTemporaria;

    fn ambiente(pares: &[(&str, String)]) -> impl Fn(&str) -> Option<OsString> {
        let mapa: HashMap<String, String> = pares
            .iter()
            .map(|(k, v)| ((*k).to_string(), v.clone()))
            .collect();
        move |n| mapa.get(n).map(OsString::from)
    }

    #[test]
    fn local_padrao_por_sistema() {
        let env = ambiente(&[
            ("APPDATA", "AD".into()),
            ("HOME", "H".into()),
            ("XDG_CONFIG_HOME", "X".into()),
        ]);
        assert_eq!(
            caminho_padrao_de(true, &env).unwrap(),
            PathBuf::from("AD").join("rustnf").join("rustnf.toml")
        );
        assert_eq!(
            caminho_padrao_de(false, &env).unwrap(),
            PathBuf::from("X").join("rustnf").join("rustnf.toml")
        );
        let sem_xdg = ambiente(&[("HOME", "H".into())]);
        assert_eq!(
            caminho_padrao_de(false, &sem_xdg).unwrap(),
            PathBuf::from("H")
                .join(".config")
                .join("rustnf")
                .join("rustnf.toml")
        );
        assert_eq!(caminho_padrao_de(true, &ambiente(&[])), None);
    }

    #[test]
    fn padrao_ausente_devolve_o_padrao_sem_erro() {
        let pasta = PastaTemporaria::nova("cfg-padrao-ausente");
        let env = ambiente(&[
            ("APPDATA", pasta.texto()),
            ("XDG_CONFIG_HOME", pasta.texto()),
            ("HOME", pasta.texto()),
        ]);
        let (c, origem) = Config::carregar_com(None, &env).unwrap();
        assert_eq!(c, Config::default());
        assert_eq!(origem, None);
    }

    #[test]
    fn explicito_ausente_e_erro() {
        let pasta = PastaTemporaria::nova("cfg-explicito-ausente");
        let falta = pasta.caminho().join("nao-existe.toml");
        let erro = Config::carregar_com(Some(&falta), &ambiente(&[])).unwrap_err();
        assert_eq!(erro, ErroConfig::NaoEncontrado(Indicado::Opcao));
        assert!(!erro.to_string().contains("nao-existe"));
    }

    #[test]
    fn variavel_ausente_e_erro_e_variavel_presente_vale() {
        let pasta = PastaTemporaria::nova("cfg-variavel");
        let falta = pasta.caminho().join("falta.toml");
        let env = ambiente(&[(VARIAVEL_CONFIG, falta.to_string_lossy().into_owned())]);
        assert_eq!(
            Config::carregar_com(None, &env).unwrap_err(),
            ErroConfig::NaoEncontrado(Indicado::Variavel)
        );
        let existe = pasta.escrever("v.toml", "[servidor]\nporta = 9001\n");
        let env = ambiente(&[(VARIAVEL_CONFIG, existe.to_string_lossy().into_owned())]);
        let (c, origem) = Config::carregar_com(None, &env).unwrap();
        assert_eq!(c.servidor.porta, 9001);
        assert_eq!(origem, Some(existe));
    }

    #[test]
    fn explicito_vence_a_variavel() {
        let pasta = PastaTemporaria::nova("cfg-explicito-vence");
        let a = pasta.escrever("a.toml", "[servidor]\nporta = 1111\n");
        let b = pasta.escrever("b.toml", "[servidor]\nporta = 2222\n");
        let env = ambiente(&[(VARIAVEL_CONFIG, b.to_string_lossy().into_owned())]);
        let (c, _) = Config::carregar_com(Some(&a), &env).unwrap();
        assert_eq!(c.servidor.porta, 1111);
    }

    #[test]
    fn campo_desconhecido_e_recusado_inclusive_senha_e_token() {
        for texto in [
            "[sefin]\nsenha = \"abc\"\n",
            "[sefin]\ntoken = \"abc\"\n",
            "[servidor]\ntoken = \"abc\"\n",
            "senha = \"abc\"\n",
            "[secao_nova]\nx = 1\n",
            "[emitente]\nsenha_do_pfx = \"abc\"\n",
        ] {
            match Config::de_texto(texto) {
                Err(ErroConfig::Sintaxe { mensagem, linha }) => {
                    assert!(mensagem.contains("unknown field"), "{texto}: {mensagem}");
                    assert!(linha.is_some());
                    // a mensagem nao ecoa o valor lido
                    assert!(!mensagem.contains("abc"), "{mensagem}");
                }
                outro => panic!("{texto}: esperava Sintaxe, veio {outro:?}"),
            }
        }
    }

    #[test]
    fn tipo_errado_e_toml_quebrado_dao_sintaxe() {
        assert!(matches!(
            Config::de_texto("[servidor]\nporta = \"oito\"\n"),
            Err(ErroConfig::Sintaxe { .. })
        ));
        assert!(matches!(
            Config::de_texto("[servidor\n"),
            Err(ErroConfig::Sintaxe { .. })
        ));
    }

    #[test]
    fn salvar_e_ler_devolve_o_mesmo_e_nao_deixa_temporario() {
        let pasta = PastaTemporaria::nova("cfg-salvar");
        let caminho = pasta.caminho().join("sub").join("rustnf.toml");
        let mut c = Config::default();
        c.emitente.cnpj = "11222333000181".into();
        c.servidor.origens_permitidas = vec!["https://app.exemplo.com.br".into()];
        c.aliquotas.push(Aliquota {
            ano: 2027,
            cbs_referencia: "8.8".into(),
            fonte: "norma de exemplo".into(),
            ..Aliquota::default()
        });
        c.salvar(&caminho).unwrap();
        assert_eq!(Config::ler(&caminho).unwrap(), c);
        let nomes: Vec<_> = std::fs::read_dir(caminho.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(nomes, vec![OsString::from("rustnf.toml")]);
    }

    #[test]
    fn salvar_recusa_valor_invalido_sem_tocar_no_arquivo() {
        let pasta = PastaTemporaria::nova("cfg-salvar-invalido");
        let caminho = pasta.escrever("rustnf.toml", "[servidor]\nporta = 7\n");
        let mut c = Config::default();
        c.sefin.ambiente = "outro".into();
        assert!(matches!(
            c.salvar(&caminho),
            Err(ErroConfig::Invalido { .. })
        ));
        assert_eq!(Config::ler(&caminho).unwrap().servidor.porta, 7);
    }
}
