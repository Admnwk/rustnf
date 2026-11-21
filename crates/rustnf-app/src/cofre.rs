// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O cofre da senha do certificado. No Windows e' o Gerenciador de Credenciais do sistema; nos outros sistemas o
//! cofre do sistema nao e' usado ([`ErroCofre::NaoSuportado`]) e a senha continua vindo de variavel de ambiente ou
//! do stdin. [`CofreEmMemoria`] serve aos testes.

use std::collections::HashMap;
use std::fmt;
use std::path::Path;
use std::sync::Mutex;

use zeroize::Zeroizing;

use crate::config::Emitente;
use crate::erro::Erro;
use crate::segredo::Segredo;

/// Nome do servico sob o qual os itens ficam no cofre.
pub const SERVICO: &str = "rustnf";

/// Variavel que desliga o cofre do sistema (servidor sem sessao de usuario, por exemplo): `desligado`.
pub const VARIAVEL_COFRE: &str = "RUSTNF_COFRE";

/// Por que o cofre nao respondeu. A mensagem nunca leva a chave do item nem o segredo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErroCofre {
    /// Este sistema nao tem cofre ligado ao rustnf.
    NaoSuportado,
    /// O cofre foi desligado por quem opera (`RUSTNF_COFRE=desligado`).
    Desligado,
    /// O cofre do sistema nao esta disponivel (nao abriu ou negou o acesso).
    Indisponivel,
    /// A chave do item e' vazia ou tem caractere de controle.
    ChaveInvalida,
}

impl fmt::Display for ErroCofre {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NaoSuportado => {
                "cofre de senhas: este sistema nao tem cofre ligado; use --senha-env ou --senha-stdin"
            }
            Self::Desligado => {
                "cofre de senhas: desligado (RUSTNF_COFRE=desligado); use --senha-env ou --senha-stdin"
            }
            Self::Indisponivel => "cofre de senhas: o cofre do sistema nao respondeu",
            Self::ChaveInvalida => "cofre de senhas: identificacao do certificado invalida",
        })
    }
}

impl std::error::Error for ErroCofre {}

impl From<ErroCofre> for Erro {
    fn from(e: ErroCofre) -> Self {
        match e {
            ErroCofre::ChaveInvalida => Self::Uso(e.to_string()),
            _ => Self::Entrada(e.to_string()),
        }
    }
}

/// Onde a senha do certificado fica guardada. A `chave` identifica o certificado sem expor o caminho inteiro
/// (veja [`chave_do_certificado`]).
pub trait Cofre: Send + Sync {
    /// Guarda (ou troca) o segredo.
    ///
    /// # Errors
    ///
    /// [`ErroCofre`] se o cofre nao responder.
    fn guardar(&self, chave: &str, segredo: &Segredo) -> Result<(), ErroCofre>;

    /// Le o segredo; `Ok(None)` se nao ha item com essa chave.
    ///
    /// # Errors
    ///
    /// [`ErroCofre`] se o cofre nao responder.
    fn ler(&self, chave: &str) -> Result<Option<Segredo>, ErroCofre>;

    /// Apaga o item; item ausente nao e' erro.
    ///
    /// # Errors
    ///
    /// [`ErroCofre`] se o cofre nao responder.
    fn apagar(&self, chave: &str) -> Result<(), ErroCofre>;
}

fn conferir_chave(chave: &str) -> Result<(), ErroCofre> {
    if chave.trim().is_empty() || chave.chars().any(char::is_control) {
        return Err(ErroCofre::ChaveInvalida);
    }
    Ok(())
}

/// Identifica o certificado no cofre: o CNPJ (ou CPF) do emitente quando o arquivo de configuracao o traz; senao o
/// nome do arquivo `.pfx`, sem a pasta. `None` se nem um nem outro existir.
#[must_use]
pub fn chave_do_certificado(emitente: &Emitente, pfx: &str) -> Option<String> {
    let documento = if emitente.cnpj.is_empty() {
        &emitente.cpf
    } else {
        &emitente.cnpj
    };
    if !documento.is_empty() {
        return Some(documento.clone());
    }
    let nome = Path::new(pfx).file_name()?.to_string_lossy().into_owned();
    (!nome.is_empty()).then_some(nome)
}

/// O cofre desta execucao: o do sistema, ou nenhum com `RUSTNF_COFRE=desligado`.
#[must_use]
pub fn cofre_do_ambiente() -> Box<dyn Cofre> {
    cofre_conforme(std::env::var(VARIAVEL_COFRE).ok().as_deref())
}

/// O cofre para o valor de `RUSTNF_COFRE`: `desligado` recusa tudo; qualquer outro valor (ou nenhum) usa o do
/// sistema.
#[must_use]
pub fn cofre_conforme(valor: Option<&str>) -> Box<dyn Cofre> {
    match valor {
        Some("desligado") => Box::new(CofreDesligado),
        _ => Box::new(CofreDoSistema),
    }
}

/// Cofre so' em memoria, para teste.
#[derive(Default)]
pub struct CofreEmMemoria {
    itens: Mutex<HashMap<String, Zeroizing<String>>>,
}

impl CofreEmMemoria {
    /// Cofre vazio.
    #[must_use]
    pub fn novo() -> Self {
        Self::default()
    }

    fn itens(&self) -> std::sync::MutexGuard<'_, HashMap<String, Zeroizing<String>>> {
        self.itens
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl Cofre for CofreEmMemoria {
    fn guardar(&self, chave: &str, segredo: &Segredo) -> Result<(), ErroCofre> {
        conferir_chave(chave)?;
        self.itens().insert(
            chave.to_string(),
            Zeroizing::new(segredo.expor().to_string()),
        );
        Ok(())
    }

    fn ler(&self, chave: &str) -> Result<Option<Segredo>, ErroCofre> {
        conferir_chave(chave)?;
        Ok(self
            .itens()
            .get(chave)
            .map(|s| Segredo::novo(s.as_str().to_string())))
    }

    fn apagar(&self, chave: &str) -> Result<(), ErroCofre> {
        conferir_chave(chave)?;
        self.itens().remove(chave);
        Ok(())
    }
}

/// Cofre que recusa tudo: o operador desligou o cofre do sistema.
#[derive(Debug, Clone, Copy, Default)]
pub struct CofreDesligado;

impl Cofre for CofreDesligado {
    fn guardar(&self, _chave: &str, _segredo: &Segredo) -> Result<(), ErroCofre> {
        Err(ErroCofre::Desligado)
    }

    fn ler(&self, _chave: &str) -> Result<Option<Segredo>, ErroCofre> {
        Err(ErroCofre::Desligado)
    }

    fn apagar(&self, _chave: &str) -> Result<(), ErroCofre> {
        Err(ErroCofre::Desligado)
    }
}

/// O cofre do sistema: no Windows, o Gerenciador de Credenciais (crates `keyring-core` e
/// `windows-native-keyring-store`); nos outros sistemas, toda chamada devolve [`ErroCofre::NaoSuportado`].
#[derive(Debug, Clone, Copy, Default)]
pub struct CofreDoSistema;

#[cfg(windows)]
mod windows {
    use std::collections::HashMap;

    use keyring_core::api::CredentialStoreApi;
    use windows_native_keyring_store::Store;

    use super::{conferir_chave, Cofre, CofreDoSistema, ErroCofre, Segredo, SERVICO};

    /// O item fica so' nesta maquina: nao acompanha o perfil do usuario para outras maquinas do dominio.
    const PERSISTENCIA: &str = "Local";

    fn entrada(chave: &str) -> Result<keyring_core::Entry, ErroCofre> {
        conferir_chave(chave)?;
        let modificadores = HashMap::from([("persistence", PERSISTENCIA)]);
        Store::new()
            .and_then(|cofre| cofre.build(SERVICO, chave, Some(&modificadores)))
            .map_err(|_| ErroCofre::Indisponivel)
    }

    impl Cofre for CofreDoSistema {
        fn guardar(&self, chave: &str, segredo: &Segredo) -> Result<(), ErroCofre> {
            entrada(chave)?
                .set_password(segredo.expor())
                .map_err(|_| ErroCofre::Indisponivel)
        }

        fn ler(&self, chave: &str) -> Result<Option<Segredo>, ErroCofre> {
            match entrada(chave)?.get_password() {
                Ok(s) => Ok(Some(Segredo::novo(s))),
                Err(keyring_core::Error::NoEntry) => Ok(None),
                Err(_) => Err(ErroCofre::Indisponivel),
            }
        }

        fn apagar(&self, chave: &str) -> Result<(), ErroCofre> {
            match entrada(chave)?.delete_credential() {
                Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
                Err(_) => Err(ErroCofre::Indisponivel),
            }
        }
    }
}

#[cfg(not(windows))]
impl Cofre for CofreDoSistema {
    fn guardar(&self, _chave: &str, _segredo: &Segredo) -> Result<(), ErroCofre> {
        Err(ErroCofre::NaoSuportado)
    }

    fn ler(&self, _chave: &str) -> Result<Option<Segredo>, ErroCofre> {
        Err(ErroCofre::NaoSuportado)
    }

    fn apagar(&self, _chave: &str) -> Result<(), ErroCofre> {
        Err(ErroCofre::NaoSuportado)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn guardar_ler_trocar_e_apagar() {
        let c = CofreEmMemoria::novo();
        assert!(c.ler("11222333000181").unwrap().is_none());
        c.guardar("11222333000181", &Segredo::novo("um".into()))
            .unwrap();
        assert_eq!(c.ler("11222333000181").unwrap().unwrap().expor(), "um");
        c.guardar("11222333000181", &Segredo::novo("dois".into()))
            .unwrap();
        assert_eq!(c.ler("11222333000181").unwrap().unwrap().expor(), "dois");
        assert!(c.ler("outro").unwrap().is_none());
        c.apagar("11222333000181").unwrap();
        assert!(c.ler("11222333000181").unwrap().is_none());
        // apagar item ausente nao e' erro
        c.apagar("11222333000181").unwrap();
    }

    #[test]
    fn cofre_desligado_pela_variavel_recusa_tudo() {
        let c = cofre_conforme(Some("desligado"));
        assert_eq!(c.ler("x").map(|_| ()), Err(ErroCofre::Desligado));
        assert_eq!(
            c.guardar("x", &Segredo::novo("s".into())),
            Err(ErroCofre::Desligado)
        );
        assert_eq!(c.apagar("x"), Err(ErroCofre::Desligado));
    }

    #[test]
    fn chave_vazia_ou_com_controle_e_recusada() {
        let c = CofreEmMemoria::novo();
        let s = Segredo::novo("x".into());
        assert_eq!(c.guardar("", &s), Err(ErroCofre::ChaveInvalida));
        assert_eq!(c.ler("a\nb").map(|_| ()), Err(ErroCofre::ChaveInvalida));
    }

    #[test]
    fn a_mensagem_de_erro_nao_leva_chave_nem_segredo() {
        for e in [
            ErroCofre::NaoSuportado,
            ErroCofre::Desligado,
            ErroCofre::Indisponivel,
            ErroCofre::ChaveInvalida,
        ] {
            assert!(!e.to_string().contains("1122"));
        }
    }

    #[test]
    fn chave_do_certificado_prefere_o_documento_e_cai_no_nome_do_arquivo() {
        let mut e = Emitente::default();
        assert_eq!(
            chave_do_certificado(&e, "pasta/interna/cert.pfx"),
            Some("cert.pfx".into())
        );
        assert_eq!(chave_do_certificado(&e, ""), None);
        e.cnpj = "11222333000181".into();
        assert_eq!(
            chave_do_certificado(&e, "pasta/interna/cert.pfx"),
            Some("11222333000181".into())
        );
        let e = Emitente {
            cpf: "52998224725".into(),
            ..Emitente::default()
        };
        assert_eq!(chave_do_certificado(&e, ""), Some("52998224725".into()));
    }

    /// Toca o Gerenciador de Credenciais de verdade, com um item proprio do teste que e' apagado no fim:
    /// `cargo test -p rustnf-app --lib cofre_do_sistema_no_windows -- --ignored`.
    #[cfg(windows)]
    #[test]
    #[ignore = "grava e apaga um item no Gerenciador de Credenciais do Windows"]
    fn cofre_do_sistema_no_windows_guarda_le_troca_e_apaga() {
        let c = CofreDoSistema;
        let chave = format!("teste-do-cofre-{}", std::process::id());
        c.apagar(&chave).unwrap();
        assert!(c.ler(&chave).unwrap().is_none());
        c.guardar(&chave, &Segredo::novo("um".into())).unwrap();
        assert_eq!(c.ler(&chave).unwrap().unwrap().expor(), "um");
        c.guardar(&chave, &Segredo::novo("dois".into())).unwrap();
        assert_eq!(c.ler(&chave).unwrap().unwrap().expor(), "dois");
        c.apagar(&chave).unwrap();
        assert!(c.ler(&chave).unwrap().is_none());
        c.apagar(&chave).unwrap();
    }

    #[cfg(not(windows))]
    #[test]
    fn cofre_do_sistema_fora_do_windows_diz_que_nao_suporta() {
        let c = CofreDoSistema;
        assert_eq!(c.ler("x").map(|_| ()), Err(ErroCofre::NaoSuportado));
        assert_eq!(
            c.guardar("x", &Segredo::novo("s".into())),
            Err(ErroCofre::NaoSuportado)
        );
        assert_eq!(c.apagar("x"), Err(ErroCofre::NaoSuportado));
    }
}
