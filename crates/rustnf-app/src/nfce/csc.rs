// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O CSC (Codigo de Seguranca do Contribuinte) da NFC-e no cofre. O VALOR nunca vai ao arquivo de configuracao, ao log, a
//! mensagem de erro, ao JSON de saida nem a pasta de notas: so' o `idCSC` (que e' publico, vai na URL do QR Code) fica em
//! `[nfce].id_csc`. A chave do item no cofre e' propria do emitente, do ambiente e do `idCSC`: trocar de ambiente ou de
//! id nao enxerga o CSC do outro.

use rustnf::nfe::montagem::{Csc, OpcoesQr};

use crate::cofre::Cofre;
use crate::config::Config;
use crate::erro::Erro;
use crate::segredo::Segredo;

/// A chave do item no cofre: `csc:<documento do emitente>:<ambiente>:<idCSC>`.
///
/// # Errors
///
/// [`Erro::Uso`] se faltam o documento do emitente ou o `[nfce].id_csc`.
pub fn chave_do_csc(config: &Config) -> Result<String, Erro> {
    let e = &config.emitente;
    let documento = if e.cnpj.is_empty() { &e.cpf } else { &e.cnpj };
    if documento.is_empty() {
        return Err(Erro::Uso(
            "informe o CNPJ ou CPF do emitente em [emitente]: o CSC fica no cofre sob esse documento".into(),
        ));
    }
    if config.nfce.id_csc.is_empty() {
        return Err(Erro::Uso(
            "informe o idCSC em [nfce].id_csc (o numero que a SEFAZ da UF deu junto com o CSC)"
                .into(),
        ));
    }
    Ok(format!(
        "csc:{documento}:{}:{}",
        config.nfce.ambiente, config.nfce.id_csc
    ))
}

/// Confere o CSC (16 a 36 caracteres alfanumericos) e o guarda no cofre.
///
/// # Errors
///
/// [`Erro::Uso`] sem documento ou `idCSC`; [`Erro::Recusa`] se o CSC nao serve (a mensagem nao o repete); o erro do cofre.
pub fn guardar(cofre: &dyn Cofre, config: &Config, valor: &Segredo) -> Result<(), Erro> {
    let chave = chave_do_csc(config)?;
    Csc::novo(&config.nfce.id_csc, valor.expor())?;
    cofre.guardar(&chave, valor)?;
    Ok(())
}

/// Ha CSC no cofre para este emitente, ambiente e `idCSC`? Nunca devolve o valor.
///
/// # Errors
///
/// Como em [`guardar`].
pub fn existe(cofre: &dyn Cofre, config: &Config) -> Result<bool, Erro> {
    Ok(cofre.ler(&chave_do_csc(config)?)?.is_some())
}

/// Apaga o CSC do cofre (CSC ausente nao e' erro).
///
/// # Errors
///
/// Como em [`guardar`].
pub fn apagar(cofre: &dyn Cofre, config: &Config) -> Result<(), Erro> {
    cofre.apagar(&chave_do_csc(config)?)?;
    Ok(())
}

/// As opcoes do QR Code da configuracao. Com a versao 2 o CSC TEM de estar no cofre: se faltar, a recusa vem aqui, antes
/// de numerar ou assinar qualquer coisa. A versao 3 (padrao) nao usa CSC.
///
/// # Errors
///
/// [`Erro::Uso`] com a versao 2 sem CSC guardado; os de [`chave_do_csc`] e do cofre.
pub fn opcoes_qr(cofre: &dyn Cofre, config: &Config) -> Result<OpcoesQr, Erro> {
    if config.nfce.qr_versao != 2 {
        return Ok(OpcoesQr {
            versao: Some(3),
            csc: None,
        });
    }
    let chave = chave_do_csc(config)?;
    let Some(valor) = cofre.ler(&chave)? else {
        return Err(Erro::Uso(
            "o QR Code versao 2 precisa do CSC e ele nao esta guardado neste ambiente: rode `rustnf nfce csc guardar` \
             (ou use qr_versao = 3 em [nfce], que dispensa o CSC)"
                .into(),
        ));
    };
    Ok(OpcoesQr {
        versao: Some(2),
        csc: Some(Csc::novo(&config.nfce.id_csc, valor.expor())?),
    })
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::cofre::CofreEmMemoria;

    const CSC_TESTE: &str = "ZZTESTECSC0123456789ABCDEF000000";

    fn config(v2: bool) -> Config {
        let mut c = Config::default();
        c.emitente.cnpj = "11222333000181".into();
        c.nfce.id_csc = "000001".into();
        c.nfce.qr_versao = if v2 { 2 } else { 3 };
        c
    }

    #[test]
    fn guardar_testar_e_apagar_sem_mostrar_o_valor() {
        let cofre = CofreEmMemoria::novo();
        let c = config(true);
        assert!(!existe(&cofre, &c).unwrap());
        guardar(&cofre, &c, &Segredo::novo(CSC_TESTE.into())).unwrap();
        assert!(existe(&cofre, &c).unwrap());
        let opcoes = opcoes_qr(&cofre, &c).unwrap();
        assert_eq!(opcoes.versao, Some(2));
        assert!(!format!("{opcoes:?}").contains(CSC_TESTE));
        // outro ambiente nao enxerga o CSC deste
        let mut outro = c.clone();
        outro.nfce.ambiente = "producao".into();
        assert!(!existe(&cofre, &outro).unwrap());
        apagar(&cofre, &c).unwrap();
        assert!(!existe(&cofre, &c).unwrap());
    }

    #[test]
    fn a_versao_2_sem_csc_recusa_e_a_3_dispensa() {
        let cofre = CofreEmMemoria::novo();
        let erro = opcoes_qr(&cofre, &config(true)).unwrap_err();
        assert!(erro.to_string().contains("nfce csc guardar"), "{erro}");
        let v3 = opcoes_qr(&cofre, &config(false)).unwrap();
        assert!(v3.csc.is_none() && v3.versao == Some(3));
    }

    #[test]
    fn csc_ruim_e_recusado_sem_ser_repetido() {
        let cofre = CofreEmMemoria::novo();
        let ruim = "curto-demais!";
        let erro = guardar(&cofre, &config(true), &Segredo::novo(ruim.into())).unwrap_err();
        assert!(!erro.to_string().contains(ruim), "{erro}");
        assert!(!existe(&cofre, &config(true)).unwrap());
    }
}
