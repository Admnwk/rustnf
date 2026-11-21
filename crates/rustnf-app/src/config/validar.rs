// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Conferencia dos valores do arquivo: cada recusa aponta a secao e o campo.

use std::collections::BTreeSet;
use std::net::IpAddr;

use rustnf::assinatura::xmldsig::Algoritmo;
use rustnf::chave_dfe::{cnpj_valido, cpf_valido};

use super::modelo::{Aliquota, Config, Emitente, Interface, Sefin, Servidor, TEMAS};
use super::ErroConfig;

type Resultado = Result<(), ErroConfig>;

/// Teto do corpo da requisicao aceito no arquivo: 256 MiB.
const LIMITE_CORPO_MAXIMO: usize = 256 * 1024 * 1024;

impl Config {
    /// Confere todos os valores.
    ///
    /// # Errors
    ///
    /// [`ErroConfig::Invalido`] no primeiro campo que nao serve, com a secao e o campo.
    pub fn validar(&self) -> Result<(), ErroConfig> {
        emitente(&self.emitente)?;
        sefin(&self.sefin)?;
        servidor(&self.servidor)?;
        interface(&self.interface)?;
        aliquotas(&self.aliquotas)
    }
}

fn so_digitos(s: &str) -> bool {
    s.bytes().all(|b| b.is_ascii_digit())
}

fn emitente(e: &Emitente) -> Resultado {
    let sec = "emitente";
    if !e.cnpj.is_empty() && !e.cpf.is_empty() {
        return Err(ErroConfig::invalido(
            sec,
            "cpf",
            "informe cnpj ou cpf, nao os dois",
        ));
    }
    if !e.cnpj.is_empty() && !cnpj_valido(&e.cnpj) {
        return Err(ErroConfig::invalido(
            sec,
            "cnpj",
            "CNPJ invalido (14 caracteres, sem pontuacao, com digitos verificadores certos)",
        ));
    }
    if !e.cpf.is_empty() && !cpf_valido(&e.cpf) {
        return Err(ErroConfig::invalido(
            sec,
            "cpf",
            "CPF invalido (11 digitos, sem pontuacao, com digitos verificadores certos)",
        ));
    }
    if !e.municipio.is_empty() && !(e.municipio.len() == 7 && so_digitos(&e.municipio)) {
        return Err(ErroConfig::invalido(
            sec,
            "municipio",
            "codigo IBGE de 7 digitos",
        ));
    }
    if !(1..=3).contains(&e.op_simples) {
        return Err(ErroConfig::invalido(
            sec,
            "op_simples",
            "use 1 (nao optante), 2 (MEI) ou 3 (ME/EPP)",
        ));
    }
    if e.regime_apuracao > 3 {
        return Err(ErroConfig::invalido(
            sec,
            "regime_apuracao",
            "use 0 (nao informar), 1, 2 ou 3",
        ));
    }
    if !e.serie.is_empty() && !(e.serie.len() <= 5 && so_digitos(&e.serie)) {
        return Err(ErroConfig::invalido(sec, "serie", "ate 5 digitos"));
    }
    Ok(())
}

fn sefin(s: &Sefin) -> Resultado {
    let sec = "sefin";
    if !matches!(s.ambiente.as_str(), "producao" | "restrita") {
        return Err(ErroConfig::invalido(
            sec,
            "ambiente",
            "use \"producao\" ou \"restrita\"",
        ));
    }
    if !(1..=600).contains(&s.tempo_limite_s) {
        return Err(ErroConfig::invalido(
            sec,
            "tempo_limite_s",
            "entre 1 e 600 segundos",
        ));
    }
    if s.folga_relogio_s > 3600 {
        return Err(ErroConfig::invalido(
            sec,
            "folga_relogio_s",
            "no maximo 3600 segundos",
        ));
    }
    if Algoritmo::de_nome(&s.assinatura).is_none() {
        return Err(ErroConfig::invalido(
            sec,
            "assinatura",
            "use \"sha256\" ou \"sha1\"",
        ));
    }
    if s.pfx.contains('\0') {
        return Err(ErroConfig::invalido(sec, "pfx", "caminho invalido"));
    }
    Ok(())
}

fn servidor(s: &Servidor) -> Resultado {
    let sec = "servidor";
    if s.endereco.parse::<IpAddr>().is_err() {
        return Err(ErroConfig::invalido(
            sec,
            "endereco",
            "endereco IP (por exemplo 127.0.0.1)",
        ));
    }
    if !(1..=LIMITE_CORPO_MAXIMO).contains(&s.limite_corpo) {
        return Err(ErroConfig::invalido(
            sec,
            "limite_corpo",
            "entre 1 e 268435456 bytes",
        ));
    }
    if !(1..=3600).contains(&s.tempo_limite_s) {
        return Err(ErroConfig::invalido(
            sec,
            "tempo_limite_s",
            "entre 1 e 3600 segundos",
        ));
    }
    for o in &s.origens_permitidas {
        let ok = (o.starts_with("http://") || o.starts_with("https://"))
            && !o.contains(char::is_whitespace)
            && o.len() > 8;
        if !ok {
            return Err(ErroConfig::invalido(
                sec,
                "origens_permitidas",
                "cada origem tem a forma esquema://endereco, com http ou https, e nao tem espacos",
            ));
        }
    }
    Ok(())
}

fn interface(i: &Interface) -> Resultado {
    if !TEMAS.contains(&i.tema.as_str()) {
        return Err(ErroConfig::invalido(
            "interface",
            "tema",
            "use \"sistema\", \"claro\" ou \"escuro\"",
        ));
    }
    Ok(())
}

/// Percentual decimal sem sinal: ate 3 digitos inteiros, ate 4 casas (a biblioteca guarda x10000), no maximo 100.
pub(super) fn decimal_valido(texto: &str) -> bool {
    let t = texto.trim().replace(',', ".");
    let (inteiros, fracao) = t.split_once('.').unwrap_or((t.as_str(), ""));
    if inteiros.is_empty() && fracao.is_empty() {
        return false;
    }
    if !so_digitos(inteiros) || !so_digitos(fracao) || inteiros.len() > 3 || fracao.len() > 4 {
        return false;
    }
    let inteiro: u32 = inteiros.parse().unwrap_or(0);
    inteiro < 100 || (inteiro == 100 && fracao.bytes().all(|b| b == b'0'))
}

fn aliquota(i: usize, a: &Aliquota) -> Resultado {
    let sec = format!("aliquotas[{i}]");
    if !(2026..=2099).contains(&a.ano) {
        return Err(ErroConfig::invalido(sec, "ano", "entre 2026 e 2099"));
    }
    let campos = [
        ("cbs_referencia", &a.cbs_referencia),
        ("cbs", &a.cbs),
        ("ibs_uf", &a.ibs_uf),
        ("ibs_mun", &a.ibs_mun),
    ];
    for (nome, valor) in campos {
        if !valor.is_empty() && !decimal_valido(valor) {
            return Err(ErroConfig::invalido(
                sec,
                nome,
                "numero decimal de 0 a 100, com ate 4 casas (ex.: \"8.8\")",
            ));
        }
    }
    if !a.cbs.is_empty() && !a.cbs_referencia.is_empty() {
        return Err(ErroConfig::invalido(
            sec,
            "cbs",
            "informe cbs ou cbs_referencia, nao os dois",
        ));
    }
    if campos.iter().all(|(_, v)| v.is_empty()) {
        return Err(ErroConfig::invalido(
            sec,
            "cbs",
            "informe ao menos uma aliquota",
        ));
    }
    Ok(())
}

fn aliquotas(lista: &[Aliquota]) -> Resultado {
    let mut vistos = BTreeSet::new();
    for (i, a) in lista.iter().enumerate() {
        aliquota(i, a)?;
        if !vistos.insert(a.ano) {
            return Err(ErroConfig::invalido(
                format!("aliquotas[{i}]"),
                "ano",
                format!("o ano {} aparece mais de uma vez", a.ano),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;

    fn campo_do_erro(c: &Config) -> (String, String) {
        match c.validar() {
            Err(ErroConfig::Invalido { secao, campo, .. }) => (secao, campo),
            outro => panic!("esperava Invalido, veio {outro:?}"),
        }
    }

    #[test]
    fn padrao_e_valido() {
        assert_eq!(Config::default().validar(), Ok(()));
    }

    #[test]
    fn ambiente_municipio_e_numero_apontam_secao_e_campo() {
        let mut c = Config::default();
        c.sefin.ambiente = "homologacao".into();
        assert_eq!(campo_do_erro(&c), ("sefin".into(), "ambiente".into()));
        let mut c = Config::default();
        c.sefin.assinatura = "sha512".into();
        assert_eq!(campo_do_erro(&c), ("sefin".into(), "assinatura".into()));
        c.sefin.assinatura = "sha1".into();
        assert!(c.validar().is_ok());

        let mut c = Config::default();
        c.emitente.municipio = "35503".into();
        assert_eq!(campo_do_erro(&c), ("emitente".into(), "municipio".into()));

        let mut c = Config::default();
        c.aliquotas.push(Aliquota {
            cbs: "oito".into(),
            ..Aliquota::default()
        });
        assert_eq!(campo_do_erro(&c), ("aliquotas[0]".into(), "cbs".into()));
    }

    #[test]
    fn ano_de_aliquota_repetido_e_recusado() {
        let mut c = Config::default();
        let a = Aliquota {
            cbs_referencia: "8.8".into(),
            ..Aliquota::default()
        };
        c.aliquotas = vec![a.clone(), a];
        assert_eq!(campo_do_erro(&c), ("aliquotas[1]".into(), "ano".into()));
    }

    #[test]
    fn cnpj_e_cpf_conferem_digito_e_nao_vem_juntos() {
        let mut c = Config::default();
        c.emitente.cnpj = "11222333000181".into();
        assert_eq!(c.validar(), Ok(()));
        c.emitente.cnpj = "11222333000182".into();
        assert_eq!(campo_do_erro(&c), ("emitente".into(), "cnpj".into()));
        c.emitente.cnpj = "11222333000181".into();
        c.emitente.cpf = "52998224725".into();
        assert_eq!(campo_do_erro(&c), ("emitente".into(), "cpf".into()));
    }

    #[test]
    fn servidor_recusa_endereco_e_origem_ruins() {
        let mut c = Config::default();
        c.servidor.endereco = "localhost".into();
        assert_eq!(campo_do_erro(&c), ("servidor".into(), "endereco".into()));
        let mut c = Config::default();
        c.servidor.origens_permitidas = vec!["app.exemplo.com.br".into()];
        assert_eq!(
            campo_do_erro(&c),
            ("servidor".into(), "origens_permitidas".into())
        );
    }

    #[test]
    fn tema_da_interface_so_aceita_os_tres_conhecidos() {
        for ok in ["sistema", "claro", "escuro"] {
            let mut c = Config::default();
            c.interface.tema = ok.into();
            assert_eq!(c.validar(), Ok(()), "{ok}");
        }
        let mut c = Config::default();
        assert_eq!(c.interface.tema, "sistema");
        c.interface.tema = "azul".into();
        assert_eq!(campo_do_erro(&c), ("interface".into(), "tema".into()));
    }

    #[test]
    fn decimais() {
        for ok in ["8.8", "0", "8,8", "100", "100.0000", "27.5000"] {
            assert!(decimal_valido(ok), "{ok}");
        }
        for ruim in ["", ".", "-1", "101", "1.23456", "1e3", "a", "1.2.3"] {
            assert!(!decimal_valido(ruim), "{ruim}");
        }
    }
}
