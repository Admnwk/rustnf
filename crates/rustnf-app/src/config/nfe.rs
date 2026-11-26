// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! `[nfe]`: o que a NF-e (modelo 55) precisa e a NFS-e nao tem. O resto vem do que ja existe no arquivo: o CNPJ ou
//! CPF e o nome do `[emitente]`, o municipio do `[emitente]`, o certificado de `[sefin].pfx` (a senha vai pelo mesmo
//! cofre), a folga do relogio de `[sefin].folga_relogio_s` e a pasta de `[notas].pasta`.

use std::collections::BTreeMap;

use rustnf::nfe::montagem::RegimePisCofins;
use rustnf::nfe::sefaz::tabela::uf_valida;
use serde::{Deserialize, Serialize};

use super::ErroConfig;

/// Os nomes aceitos em `[nfe.urls]`, com o servico de cada um.
pub const SERVICOS: [(&str, &str); 6] = [
    ("status", "NfeStatusServico"),
    ("autorizacao", "NFeAutorizacao"),
    ("retorno", "NFeRetAutorizacao"),
    ("consulta", "NfeConsultaProtocolo"),
    ("eventos", "RecepcaoEvento"),
    ("inutilizacao", "NfeInutilizacao"),
];

/// `[nfe.endereco]`: o endereco do emitente na NF-e (`enderEmit`). O codigo do municipio, se vazio, e' o de
/// `[emitente].municipio`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EnderecoNfe {
    /// Logradouro (`xLgr`).
    pub logradouro: String,
    /// Numero (`nro`; `S/N` se nao tem).
    pub numero: String,
    /// Complemento (`xCpl`).
    pub complemento: String,
    /// Bairro (`xBairro`).
    pub bairro: String,
    /// Codigo IBGE do municipio, 7 digitos (`cMun`); vazio = `[emitente].municipio`.
    pub cod_municipio: String,
    /// Nome do municipio (`xMun`).
    pub municipio: String,
    /// CEP, 8 digitos.
    pub cep: String,
    /// Telefone, de 6 a 14 digitos.
    pub fone: String,
}

/// `[nfe]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Nfe {
    /// `homologacao` (padrao) ou `producao`: o `tpAmb` da nota e o ambiente da SEFAZ. Tem o proprio campo porque o
    /// da NFS-e (`[sefin].ambiente`) e' outro servico, com outro nome de ambiente.
    pub ambiente: String,
    /// Sigla da UF do emitente: escolhe o autorizador da SEFAZ e vai no `cUF` da chave.
    pub uf: String,
    /// Inscricao estadual do emitente (`IE`), so' digitos.
    pub ie: String,
    /// Regime tributario (`CRT`): 1 Simples Nacional, 2 Simples com excesso de sublimite, 3 regime normal, 4 MEI;
    /// 0 = nao informado (a emissao recusa).
    pub crt: u8,
    /// Regime do PIS e da COFINS (CRT 3): `cumulativo` ou `nao_cumulativo`; vazio = sem o aviso de CST que nao combina
    /// com o regime.
    pub regime_pis_cofins: String,
    /// Serie da NF-e, de 0 a 999.
    pub serie: String,
    /// `normal` (padrao) ou `svc`: emitir pela SEFAZ Virtual de Contingencia (`tpEmis` 6 ou 7, conforme a UF).
    pub contingencia: String,
    /// Por que entrou em contingencia, de 15 a 255 caracteres (`xJust`); obrigatorio com `contingencia = "svc"`.
    pub motivo_contingencia: String,
    /// Nome fantasia do emitente (`xFant`).
    pub fantasia: String,
    /// O endereco do emitente.
    pub endereco: EnderecoNfe,
    /// Tempo limite de cada chamada a SEFAZ, em segundos.
    pub tempo_limite_s: u64,
    /// Quantas vezes se consulta o recibo de um lote que segue em processamento.
    pub tentativas_retorno: u32,
    /// Espera entre essas consultas, em segundos (a SEFAZ rejeita consulta em rajada: no minimo 1).
    pub intervalo_retorno_s: u64,
    /// Canhoto de recebimento no DANFE.
    pub canhoto: bool,
    /// Arquivo PEM com uma raiz de confianca a mais para o TLS (simulador ou homologacao propria). Vazio = so' as
    /// raizes publicas e as da ICP-Brasil que a biblioteca ja traz.
    pub ca_pem: String,
    /// Enderecos que trocam os da tabela oficial, por servico (`status`, `autorizacao`, `retorno`, `consulta`,
    /// `eventos`, `inutilizacao`): para um simulador. `https`, ou `http` so' no proprio computador.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub urls: BTreeMap<String, String>,
}

impl Default for Nfe {
    fn default() -> Self {
        Self {
            ambiente: "homologacao".into(),
            uf: String::new(),
            ie: String::new(),
            crt: 0,
            regime_pis_cofins: String::new(),
            serie: "1".into(),
            contingencia: "normal".into(),
            motivo_contingencia: String::new(),
            fantasia: String::new(),
            endereco: EnderecoNfe::default(),
            tempo_limite_s: 60,
            tentativas_retorno: 5,
            intervalo_retorno_s: 3,
            canhoto: true,
            ca_pem: String::new(),
            urls: BTreeMap::new(),
        }
    }
}

impl Nfe {
    /// O `tpAmb`: 1 producao, 2 homologacao.
    #[must_use]
    pub fn tp_amb(&self) -> u8 {
        if self.ambiente == "producao" {
            1
        } else {
            2
        }
    }

    /// O regime do PIS e da COFINS, se foi informado.
    #[must_use]
    pub fn regime_pis_cofins(&self) -> Option<RegimePisCofins> {
        RegimePisCofins::de_texto(&self.regime_pis_cofins)
    }

    /// A emissao e' pela SVC.
    #[must_use]
    pub fn em_contingencia(&self) -> bool {
        self.contingencia == "svc"
    }
}

fn so_digitos(s: &str) -> bool {
    s.bytes().all(|b| b.is_ascii_digit())
}

fn invalido(campo: &str, motivo: &str) -> ErroConfig {
    ErroConfig::invalido("nfe", campo, motivo)
}

fn endereco(e: &EnderecoNfe) -> Result<(), ErroConfig> {
    let sec = "nfe.endereco";
    let digitos = |campo: &str, valor: &str, tam: std::ops::RangeInclusive<usize>, o_que: &str| {
        if !valor.is_empty() && !(so_digitos(valor) && tam.contains(&valor.len())) {
            return Err(ErroConfig::invalido(sec, campo, o_que));
        }
        Ok(())
    };
    digitos(
        "cod_municipio",
        &e.cod_municipio,
        7..=7,
        "codigo IBGE de 7 digitos",
    )?;
    digitos("cep", &e.cep, 8..=8, "CEP de 8 digitos, sem pontuacao")?;
    digitos("fone", &e.fone, 6..=14, "de 6 a 14 digitos, sem pontuacao")
}

/// Confere os valores de `[nfe]`: cada recusa aponta a secao e o campo.
pub(super) fn validar(n: &Nfe) -> Result<(), ErroConfig> {
    if !matches!(n.ambiente.as_str(), "homologacao" | "producao") {
        return Err(invalido("ambiente", "use \"homologacao\" ou \"producao\""));
    }
    if !n.uf.is_empty() && uf_valida(&n.uf).map(|s| s != n.uf).unwrap_or(true) {
        return Err(invalido(
            "uf",
            "sigla da UF em maiuscula, de uma UF que emite NF-e (ex.: \"SP\")",
        ));
    }
    if !n.ie.is_empty() && !(so_digitos(&n.ie) && (2..=14).contains(&n.ie.len())) {
        return Err(invalido("ie", "so' digitos, de 2 a 14 (sem pontuacao)"));
    }
    if n.crt > 4 {
        return Err(invalido("crt", "use 1, 2, 3 ou 4 (0 = nao informado)"));
    }
    if !n.regime_pis_cofins.is_empty() && n.regime_pis_cofins().is_none() {
        return Err(invalido(
            "regime_pis_cofins",
            "use \"cumulativo\" ou \"nao_cumulativo\"",
        ));
    }
    if !(!n.serie.is_empty() && n.serie.len() <= 3 && so_digitos(&n.serie)) {
        return Err(invalido("serie", "de 0 a 999"));
    }
    contingencia(n)?;
    if !(1..=600).contains(&n.tempo_limite_s) {
        return Err(invalido("tempo_limite_s", "entre 1 e 600 segundos"));
    }
    if !(1..=60).contains(&n.tentativas_retorno) {
        return Err(invalido("tentativas_retorno", "entre 1 e 60"));
    }
    if !(1..=300).contains(&n.intervalo_retorno_s) {
        return Err(invalido("intervalo_retorno_s", "entre 1 e 300 segundos"));
    }
    if n.ca_pem.contains('\0') {
        return Err(invalido("ca_pem", "caminho invalido"));
    }
    for (nome, url) in &n.urls {
        if !SERVICOS.iter().any(|(s, _)| s == nome) {
            return Err(ErroConfig::invalido(
                "nfe.urls",
                nome.as_str(),
                "servico desconhecido: use status, autorizacao, retorno, consulta, eventos ou inutilizacao",
            ));
        }
        let local = ["http://127.0.0.1", "http://localhost", "http://[::1]"]
            .iter()
            .any(|p| url.starts_with(p));
        if !(url.starts_with("https://") || local) || url.contains(char::is_whitespace) {
            return Err(ErroConfig::invalido(
                "nfe.urls",
                nome.as_str(),
                "use https (http so' no proprio computador) e sem espacos",
            ));
        }
    }
    endereco(&n.endereco)
}

fn contingencia(n: &Nfe) -> Result<(), ErroConfig> {
    match n.contingencia.as_str() {
        "normal" => Ok(()),
        "svc" => {
            let tam = n.motivo_contingencia.trim().chars().count();
            if (15..=255).contains(&tam) {
                Ok(())
            } else {
                Err(invalido(
                    "motivo_contingencia",
                    "com contingencia \"svc\", o motivo tem de 15 a 255 caracteres",
                ))
            }
        }
        _ => Err(invalido("contingencia", "use \"normal\" ou \"svc\"")),
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn campo(n: &Nfe) -> (String, String) {
        match validar(n) {
            Err(ErroConfig::Invalido { secao, campo, .. }) => (secao, campo),
            outro => panic!("esperava Invalido, veio {outro:?}"),
        }
    }

    #[test]
    fn o_arquivo_traz_a_secao_recusa_campo_desconhecido_e_a_variavel_vence() {
        use std::ffi::OsString;

        use crate::config::{Config, Efetiva};
        let c = Config::de_texto(
            "[nfe]\nuf = \"SP\"\ncrt = 3\nserie = \"2\"\n[nfe.endereco]\ncep = \"01001000\"\n\
             [nfe.urls]\nautorizacao = \"http://127.0.0.1:4050/x\"\n",
        )
        .unwrap();
        assert_eq!(
            (c.nfe.uf.as_str(), c.nfe.crt, c.nfe.serie.as_str()),
            ("SP", 3, "2")
        );
        assert_eq!(c.nfe.endereco.cep, "01001000");
        assert_eq!(c.nfe.urls["autorizacao"], "http://127.0.0.1:4050/x");
        assert!(Config::de_texto("[nfe]\nuff = \"SP\"\n").is_err());
        assert!(Config::de_texto("[nfe.endereco]\nrua = \"x\"\n").is_err());
        // a NFS-e continua com os valores de sempre
        assert_eq!(c.sefin.ambiente, "restrita");

        let env = |n: &str| -> Option<OsString> {
            match n {
                "RUSTNF_NFE_AMBIENTE" => Some("producao".into()),
                "RUSTNF_NFE_SERIE" => Some("5".into()),
                "RUSTNF_NFE_TEMPO_LIMITE" => Some("90".into()),
                _ => None,
            }
        };
        let e = Efetiva::de(c.clone(), None, &env).unwrap();
        assert_eq!(e.config.nfe.tp_amb(), 1);
        assert_eq!(
            (e.config.nfe.serie.as_str(), e.config.nfe.tempo_limite_s),
            ("5", 90)
        );
        assert!(e.por_variavel("RUSTNF_NFE_AMBIENTE"));
        // o ambiente da NFS-e nao e' tocado pelo da NF-e
        assert_eq!(e.config.sefin.ambiente, "restrita");
        // valor que nao serve aponta a variavel, sem o valor
        let ruim = |n: &str| -> Option<OsString> {
            (n == "RUSTNF_NFE_TEMPO_LIMITE").then(|| "muito".into())
        };
        let erro = Efetiva::de(c, None, &ruim).unwrap_err();
        assert!(
            erro.to_string().contains("RUSTNF_NFE_TEMPO_LIMITE"),
            "{erro}"
        );
        assert!(!erro.to_string().contains("muito"));
    }

    #[test]
    fn o_padrao_e_valido_e_e_homologacao() {
        let n = Nfe::default();
        assert_eq!(validar(&n), Ok(()));
        assert_eq!(n.tp_amb(), 2);
    }

    #[test]
    fn cada_recusa_aponta_a_secao_e_o_campo() {
        type Muda = Box<dyn Fn(&mut Nfe)>;
        let casos: [(&str, Muda); 9] = [
            ("ambiente", Box::new(|n| n.ambiente = "restrita".into())),
            ("uf", Box::new(|n| n.uf = "sp".into())),
            ("uf", Box::new(|n| n.uf = "ZZ".into())),
            ("ie", Box::new(|n| n.ie = "11.111".into())),
            ("crt", Box::new(|n| n.crt = 5)),
            (
                "regime_pis_cofins",
                Box::new(|n| n.regime_pis_cofins = "lucro".into()),
            ),
            ("serie", Box::new(|n| n.serie = "1000".into())),
            ("contingencia", Box::new(|n| n.contingencia = "epec".into())),
            ("tempo_limite_s", Box::new(|n| n.tempo_limite_s = 0)),
        ];
        for (esperado, muda) in casos {
            let mut n = Nfe::default();
            muda(&mut n);
            assert_eq!(campo(&n), ("nfe".to_string(), esperado.to_string()));
        }
    }

    #[test]
    fn svc_exige_o_motivo_e_o_endereco_e_as_urls_sao_conferidos() {
        let mut n = Nfe {
            contingencia: "svc".into(),
            ..Nfe::default()
        };
        assert_eq!(campo(&n).1, "motivo_contingencia");
        n.motivo_contingencia = "Falha de internet na matriz".into();
        assert_eq!(validar(&n), Ok(()));
        n.endereco.cep = "01001-000".into();
        assert_eq!(campo(&n), ("nfe.endereco".into(), "cep".into()));
        n.endereco.cep = String::new();
        n.urls
            .insert("autorizar".into(), "https://nfe.exemplo.gov.br/ws".into());
        assert_eq!(campo(&n), ("nfe.urls".into(), "autorizar".into()));
        n.urls.clear();
        n.urls
            .insert("autorizacao".into(), "http://exemplo.com.br/x".into());
        assert_eq!(campo(&n), ("nfe.urls".into(), "autorizacao".into()));
        n.urls
            .insert("autorizacao".into(), "http://127.0.0.1:4050/x".into());
        assert_eq!(validar(&n), Ok(()));
    }
}
