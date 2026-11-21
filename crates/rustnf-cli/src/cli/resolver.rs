// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Precedencia de cada parametro: opcao da chamada > variavel de ambiente `RUSTNF_*` > arquivo > padrao. O ambiente
//! e o arquivo ja vem juntos em [`Efetiva`]; aqui entra a opcao.

use std::sync::Arc;
use std::time::Duration;

use rustnf::assinatura::xmldsig::Algoritmo;
use rustnf_app::cofre::{chave_do_certificado, Cofre};
use rustnf_app::config::Efetiva;

use super::{certificado, Ambiente, ArgSefin, Assinatura};
use crate::emissao::Emissor;
use crate::erro::Erro;
use crate::segredo::Segredo;

impl Ambiente {
    fn ler(texto: &str) -> Result<Self, Erro> {
        match texto {
            "producao" => Ok(Self::Producao),
            "restrita" => Ok(Self::Restrita),
            _ => Err(Erro::Uso("ambiente: use producao ou restrita".into())),
        }
    }
}

/// O ambiente da Sefin. Sem opcao, vale a variavel ou o arquivo; sem arquivo de configuracao e sem variavel, nao ha
/// padrao: escolher o ambiente de producao por omissao seria perigoso.
pub(super) fn ambiente(e: &Efetiva, opcao: Option<Ambiente>) -> Result<Ambiente, Erro> {
    if let Some(a) = opcao {
        return Ok(a);
    }
    if e.arquivo.is_some() || e.por_variavel("RUSTNF_AMBIENTE") {
        return Ambiente::ler(&e.config.sefin.ambiente);
    }
    Err(Erro::Uso(
        "informe --ambiente producao|restrita (ou RUSTNF_AMBIENTE, ou [sefin].ambiente no arquivo de configuracao)"
            .into(),
    ))
}

/// O algoritmo da assinatura: opcao, variavel ou arquivo; sem nenhum deles, o padrao (SHA-256).
pub(super) fn assinatura(e: &Efetiva, opcao: Option<Assinatura>) -> Algoritmo {
    opcao.map_or_else(|| e.config.sefin.algoritmo(), Assinatura::algoritmo)
}

/// O caminho do certificado: opcao, variavel ou arquivo.
pub(super) fn pfx(e: &Efetiva, opcao: Option<&str>) -> Result<String, Erro> {
    opcao
        .map(String::from)
        .or_else(|| Some(e.config.sefin.pfx.clone()).filter(|p| !p.is_empty()))
        .ok_or_else(|| {
            Erro::Uso(
                "informe --pfx (ou RUSTNF_PFX, ou [sefin].pfx no arquivo de configuracao)".into(),
            )
        })
}

/// A senha: `--senha-env` ou `--senha-stdin`; na falta das duas, a guardada no cofre do sistema. Nunca por argumento.
pub(super) fn senha(
    e: &Efetiva,
    cofre: &dyn Cofre,
    sefin: &super::ArgSenha,
    pfx: &str,
    stdin_ocupado: bool,
) -> Result<Segredo, Erro> {
    if let Some(s) = certificado::senha_explicita(sefin, stdin_ocupado)? {
        return Ok(s);
    }
    chave_do_certificado(&e.config.emitente, pfx)
        .and_then(|chave| cofre.ler(&chave).ok().flatten())
        .ok_or_else(|| Erro::Uso(certificado::SEM_SENHA.into()))
}

pub(super) fn tempo_sefin(e: &Efetiva) -> Duration {
    Duration::from_secs(e.config.sefin.tempo_limite_s)
}

/// Resolve certificado, senha e ambiente e monta o cliente da Sefin.
pub(super) fn emissor(
    e: &Efetiva,
    cofre: &dyn Cofre,
    sefin: &ArgSefin,
    stdin_ocupado: bool,
) -> Result<Arc<dyn Emissor>, Erro> {
    let pfx = pfx(e, sefin.pfx.as_deref())?;
    let ambiente = ambiente(e, sefin.ambiente)?;
    let senha = senha(e, cofre, &sefin.senha, &pfx, stdin_ocupado)?;
    certificado::emissor(
        &pfx,
        &senha,
        ambiente,
        assinatura(e, sefin.assinatura),
        tempo_sefin(e),
    )
}

#[cfg(test)]
mod testes {
    use std::collections::HashMap;
    use std::ffi::OsString;
    use std::path::PathBuf;

    use rustnf_app::config::Config;

    use super::*;

    fn efetiva(arquivo: Option<&str>, env: &[(&str, &str)]) -> Efetiva {
        let mapa: HashMap<String, String> = env
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        Efetiva::de(
            Config::de_texto(arquivo.unwrap_or("")).unwrap(),
            arquivo.map(|_| PathBuf::from("rustnf.toml")),
            &move |n| mapa.get(n).map(OsString::from),
        )
        .unwrap()
    }

    #[test]
    fn ambiente_opcao_vence_variavel_vence_arquivo() {
        let arq = Some("[sefin]\nambiente = \"producao\"\n");
        let env = [("RUSTNF_AMBIENTE", "restrita")];
        assert_eq!(
            ambiente(&efetiva(arq, &env), Some(Ambiente::Producao)).unwrap(),
            Ambiente::Producao,
            "opcao"
        );
        assert_eq!(
            ambiente(&efetiva(arq, &env), None).unwrap(),
            Ambiente::Restrita,
            "variavel"
        );
        assert_eq!(
            ambiente(&efetiva(arq, &[]), None).unwrap(),
            Ambiente::Producao,
            "arquivo"
        );
        assert_eq!(
            ambiente(&efetiva(None, &env), None).unwrap(),
            Ambiente::Restrita,
            "variavel sem arquivo"
        );
    }

    #[test]
    fn assinatura_opcao_vence_variavel_vence_arquivo_e_o_padrao_e_sha256() {
        let arq = Some("[sefin]\nassinatura = \"sha1\"\n");
        let env = [("RUSTNF_ASSINATURA", "sha256")];
        assert_eq!(
            assinatura(&efetiva(arq, &env), Some(Assinatura::Sha1)),
            Algoritmo::Sha1,
            "opcao"
        );
        assert_eq!(
            assinatura(&efetiva(arq, &env), None),
            Algoritmo::Sha256,
            "variavel"
        );
        assert_eq!(
            assinatura(&efetiva(arq, &[]), None),
            Algoritmo::Sha1,
            "arquivo"
        );
        assert_eq!(
            assinatura(&efetiva(None, &[]), None),
            Algoritmo::Sha256,
            "padrao"
        );
        assert_eq!(
            assinatura(&efetiva(None, &[("RUSTNF_ASSINATURA", "sha1")]), None),
            Algoritmo::Sha1,
            "variavel sem arquivo"
        );
    }

    #[test]
    fn sem_opcao_variavel_nem_arquivo_o_ambiente_continua_obrigatorio() {
        let erro = ambiente(&efetiva(None, &[]), None).unwrap_err();
        assert_eq!(erro.codigo_saida(), 2);
        assert!(erro.to_string().contains("--ambiente"));
        // arquivo que nao diz o ambiente: vale o padrao do arquivo, a restrita
        assert_eq!(
            ambiente(&efetiva(Some(""), &[]), None).unwrap(),
            Ambiente::Restrita
        );
    }

    #[test]
    fn certificado_opcao_vence_variavel_vence_arquivo() {
        let arq = Some("[sefin]\npfx = \"arquivo.pfx\"\n");
        let env = [("RUSTNF_PFX", "ambiente.pfx")];
        assert_eq!(
            pfx(&efetiva(arq, &env), Some("opcao.pfx")).unwrap(),
            "opcao.pfx"
        );
        assert_eq!(pfx(&efetiva(arq, &env), None).unwrap(), "ambiente.pfx");
        assert_eq!(pfx(&efetiva(arq, &[]), None).unwrap(), "arquivo.pfx");
        let erro = pfx(&efetiva(None, &[]), None).unwrap_err();
        assert_eq!(erro.codigo_saida(), 2);
        assert!(erro.to_string().contains("--pfx"));
    }
}
