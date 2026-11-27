// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A secao "NF-e" das Configuracoes: o que se edita de `[nfe]` e `[nfe.endereco]`, com as mascaras e as conferencias
//! campo a campo, o regime em palavras e o resultado do "Testar conexao com a SEFAZ". O que a tela nao edita
//! (`ca_pem` e `[nfe.urls]`, de simulador) segue como esta no arquivo. Sem janela.

use std::collections::BTreeMap;

use rustnf::nfe::sefaz::tabela::uf_valida;
use rustnf::nfe::sefaz::{SituacaoServico, StatusServico};
use rustnf_app::config::{Config, Nfe};

use super::ambiente::Ambiente;
use super::mascara::{formatar_cep, formatar_telefone, frase, so_digitos};

/// Os campos de texto da secao.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CampoNfe {
    /// Sigla da UF.
    Uf,
    /// Inscricao estadual.
    Ie,
    /// Serie.
    Serie,
    /// Nome fantasia.
    Fantasia,
    /// Logradouro.
    Logradouro,
    /// Numero do endereco.
    Numero,
    /// Complemento.
    Complemento,
    /// Bairro.
    Bairro,
    /// Codigo IBGE do municipio.
    CodMunicipio,
    /// Nome do municipio.
    Municipio,
    /// CEP.
    Cep,
    /// Telefone.
    Fone,
    /// Motivo da contingencia.
    MotivoContingencia,
    /// Tempo limite.
    Tempo,
    /// Tentativas do recibo.
    Tentativas,
    /// Espera entre as tentativas.
    Intervalo,
}

impl CampoNfe {
    /// Os campos, na ordem da tela.
    pub const TODOS: [CampoNfe; 16] = [
        Self::Uf,
        Self::Ie,
        Self::Serie,
        Self::Fantasia,
        Self::Logradouro,
        Self::Numero,
        Self::Complemento,
        Self::Bairro,
        Self::CodMunicipio,
        Self::Municipio,
        Self::Cep,
        Self::Fone,
        Self::MotivoContingencia,
        Self::Tempo,
        Self::Tentativas,
        Self::Intervalo,
    ];
}

/// Os regimes tributarios (`CRT`) com o nome que o emitente conhece.
pub const REGIMES: [(u8, &str); 4] = [
    (1, "Simples Nacional"),
    (2, "Simples com excesso de sublimite"),
    (3, "Regime normal"),
    (4, "MEI"),
];

/// O regime do PIS e da COFINS (so' no regime normal).
pub const REGIMES_PIS_COFINS: [(&str, &str); 3] = [
    ("", "Não informar"),
    ("cumulativo", "Cumulativo"),
    ("nao_cumulativo", "Não cumulativo"),
];

/// O que a tela edita da NF-e.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DadosNfe {
    /// `[nfe].ambiente`: producao ou nao.
    pub producao: bool,
    /// UF.
    pub uf: String,
    /// IE.
    pub ie: String,
    /// CRT (0 = nao informado).
    pub crt: u8,
    /// Regime do PIS e da COFINS.
    pub regime_pis_cofins: String,
    /// Serie.
    pub serie: String,
    /// Contingencia SVC ligada.
    pub svc: bool,
    /// Motivo da contingencia.
    pub motivo_contingencia: String,
    /// Fantasia.
    pub fantasia: String,
    /// Logradouro.
    pub logradouro: String,
    /// Numero.
    pub numero: String,
    /// Complemento.
    pub complemento: String,
    /// Bairro.
    pub bairro: String,
    /// Codigo do municipio.
    pub cod_municipio: String,
    /// Municipio.
    pub municipio: String,
    /// CEP, com mascara.
    pub cep: String,
    /// Telefone, com mascara.
    pub fone: String,
    /// Canhoto no DANFE.
    pub canhoto: bool,
    /// Tempo limite, em segundos.
    pub tempo_limite_s: String,
    /// Tentativas do recibo.
    pub tentativas: String,
    /// Espera entre elas.
    pub intervalo_s: String,
}

impl DadosNfe {
    /// Os dados de `[nfe]`.
    #[must_use]
    pub fn de_config(n: &Nfe) -> Self {
        let e = &n.endereco;
        Self {
            producao: n.ambiente == "producao",
            uf: n.uf.clone(),
            ie: n.ie.clone(),
            crt: n.crt,
            regime_pis_cofins: n.regime_pis_cofins.clone(),
            serie: n.serie.clone(),
            svc: n.contingencia == "svc",
            motivo_contingencia: n.motivo_contingencia.clone(),
            fantasia: n.fantasia.clone(),
            logradouro: e.logradouro.clone(),
            numero: e.numero.clone(),
            complemento: e.complemento.clone(),
            bairro: e.bairro.clone(),
            cod_municipio: e.cod_municipio.clone(),
            municipio: e.municipio.clone(),
            cep: formatar_cep(&e.cep),
            fone: formatar_telefone(&e.fone),
            canhoto: n.canhoto,
            tempo_limite_s: n.tempo_limite_s.to_string(),
            tentativas: n.tentativas_retorno.to_string(),
            intervalo_s: n.intervalo_retorno_s.to_string(),
        }
    }

    /// O texto de um campo.
    #[must_use]
    pub fn texto(&self, c: CampoNfe) -> &str {
        match c {
            CampoNfe::Uf => &self.uf,
            CampoNfe::Ie => &self.ie,
            CampoNfe::Serie => &self.serie,
            CampoNfe::Fantasia => &self.fantasia,
            CampoNfe::Logradouro => &self.logradouro,
            CampoNfe::Numero => &self.numero,
            CampoNfe::Complemento => &self.complemento,
            CampoNfe::Bairro => &self.bairro,
            CampoNfe::CodMunicipio => &self.cod_municipio,
            CampoNfe::Municipio => &self.municipio,
            CampoNfe::Cep => &self.cep,
            CampoNfe::Fone => &self.fone,
            CampoNfe::MotivoContingencia => &self.motivo_contingencia,
            CampoNfe::Tempo => &self.tempo_limite_s,
            CampoNfe::Tentativas => &self.tentativas,
            CampoNfe::Intervalo => &self.intervalo_s,
        }
    }

    fn campo(&mut self, c: CampoNfe) -> &mut String {
        match c {
            CampoNfe::Uf => &mut self.uf,
            CampoNfe::Ie => &mut self.ie,
            CampoNfe::Serie => &mut self.serie,
            CampoNfe::Fantasia => &mut self.fantasia,
            CampoNfe::Logradouro => &mut self.logradouro,
            CampoNfe::Numero => &mut self.numero,
            CampoNfe::Complemento => &mut self.complemento,
            CampoNfe::Bairro => &mut self.bairro,
            CampoNfe::CodMunicipio => &mut self.cod_municipio,
            CampoNfe::Municipio => &mut self.municipio,
            CampoNfe::Cep => &mut self.cep,
            CampoNfe::Fone => &mut self.fone,
            CampoNfe::MotivoContingencia => &mut self.motivo_contingencia,
            CampoNfe::Tempo => &mut self.tempo_limite_s,
            CampoNfe::Tentativas => &mut self.tentativas,
            CampoNfe::Intervalo => &mut self.intervalo_s,
        }
    }
}

/// O texto como a mascara do campo o deixa.
#[must_use]
pub fn normalizar(c: CampoNfe, texto: &str) -> String {
    match c {
        CampoNfe::Uf => texto
            .chars()
            .filter(char::is_ascii_alphabetic)
            .map(|c| c.to_ascii_uppercase())
            .take(2)
            .collect(),
        CampoNfe::Ie => so_digitos(texto, 14),
        CampoNfe::Serie => so_digitos(texto, 3),
        CampoNfe::CodMunicipio => so_digitos(texto, 7),
        CampoNfe::Cep => formatar_cep(texto),
        CampoNfe::Fone => formatar_telefone(texto),
        CampoNfe::Tempo | CampoNfe::Tentativas | CampoNfe::Intervalo => so_digitos(texto, 4),
        _ => texto.to_string(),
    }
}

/// O formulario da secao: o que se edita e o ultimo estado gravado.
#[derive(Debug, Clone)]
pub struct FormularioNfe {
    /// O que esta sendo editado.
    pub dados: DadosNfe,
    inicial: DadosNfe,
}

fn numero(texto: &str, padrao: u64) -> u64 {
    texto.trim().parse().unwrap_or(padrao)
}

impl FormularioNfe {
    /// O formulario com os valores da configuracao.
    #[must_use]
    pub fn de_config(config: &Config) -> Self {
        let dados = DadosNfe::de_config(&config.nfe);
        Self {
            inicial: dados.clone(),
            dados,
        }
    }

    /// Muda um campo e devolve o texto normalizado (para a tela repor no campo).
    pub fn definir(&mut self, c: CampoNfe, texto: &str) -> String {
        let normal = normalizar(c, texto);
        *self.dados.campo(c) = normal.clone();
        normal
    }

    /// Poe a secao na configuracao; o que a tela nao edita fica como estava.
    pub fn aplicar(&self, config: &mut Config) {
        let d = &self.dados;
        let n = &mut config.nfe;
        n.ambiente = if d.producao {
            "producao"
        } else {
            "homologacao"
        }
        .into();
        n.uf.clone_from(&d.uf);
        n.ie.clone_from(&d.ie);
        n.crt = d.crt;
        n.regime_pis_cofins.clone_from(&d.regime_pis_cofins);
        n.serie.clone_from(&d.serie);
        n.contingencia = if d.svc { "svc" } else { "normal" }.into();
        n.motivo_contingencia = d.motivo_contingencia.trim().to_string();
        n.fantasia = d.fantasia.trim().to_string();
        n.endereco.logradouro = d.logradouro.trim().to_string();
        n.endereco.numero = d.numero.trim().to_string();
        n.endereco.complemento = d.complemento.trim().to_string();
        n.endereco.bairro = d.bairro.trim().to_string();
        n.endereco.cod_municipio.clone_from(&d.cod_municipio);
        n.endereco.municipio = d.municipio.trim().to_string();
        n.endereco.cep = so_digitos(&d.cep, 8);
        n.endereco.fone = so_digitos(&d.fone, 14);
        n.canhoto = d.canhoto;
        n.tempo_limite_s = numero(&d.tempo_limite_s, 0);
        n.tentativas_retorno = u32::try_from(numero(&d.tentativas, 0)).unwrap_or(0);
        n.intervalo_retorno_s = numero(&d.intervalo_s, 0);
    }

    /// Mudou alguma coisa desde que foi carregado ou gravado.
    #[must_use]
    pub fn alterado(&self) -> bool {
        self.dados != self.inicial
    }

    /// Marca o estado atual como o gravado.
    pub fn marcar_salvo(&mut self) {
        self.inicial = self.dados.clone();
    }

    /// Joga fora as mudancas.
    pub fn descartar(&mut self) {
        self.dados = self.inicial.clone();
    }

    /// Recomeca a partir da configuracao lida.
    pub fn recarregar(&mut self, config: &Config) {
        *self = Self::de_config(config);
    }

    /// O erro de cada campo que nao serve, em frase para quem nao e' contador.
    #[must_use]
    pub fn erros(&self) -> BTreeMap<CampoNfe, String> {
        let d = &self.dados;
        let mut e = BTreeMap::new();
        let mut erro = |c: CampoNfe, t: &str| {
            e.insert(c, t.to_string());
        };
        if !d.uf.is_empty() && uf_valida(&d.uf).map(|s| s != d.uf).unwrap_or(true) {
            erro(
                CampoNfe::Uf,
                "Use a sigla de uma UF que emite NF-e, em maiúscula (por exemplo, SP).",
            );
        }
        if !d.ie.is_empty() && !(2..=14).contains(&d.ie.len()) {
            erro(CampoNfe::Ie, "A inscrição estadual tem de 2 a 14 dígitos.");
        }
        if d.serie.is_empty() {
            erro(CampoNfe::Serie, "Informe a série (de 0 a 999).");
        }
        if !d.cod_municipio.is_empty() && d.cod_municipio.len() != 7 {
            erro(
                CampoNfe::CodMunicipio,
                "O código IBGE do município tem 7 dígitos.",
            );
        }
        let cep = so_digitos(&d.cep, 8);
        if !cep.is_empty() && cep.len() != 8 {
            erro(CampoNfe::Cep, "O CEP tem 8 dígitos.");
        }
        let fone = so_digitos(&d.fone, 14);
        if !fone.is_empty() && !(6..=14).contains(&fone.len()) {
            erro(
                CampoNfe::Fone,
                "O telefone tem de 6 a 14 dígitos, com o DDD.",
            );
        }
        if d.svc {
            let n = d.motivo_contingencia.trim().chars().count();
            if !(15..=255).contains(&n) {
                erro(
                    CampoNfe::MotivoContingencia,
                    "Com a contingência ligada, o motivo é obrigatório e tem de 15 a 255 caracteres.",
                );
            }
        }
        if !(1..=600).contains(&numero(&d.tempo_limite_s, 0)) {
            erro(CampoNfe::Tempo, "Entre 1 e 600 segundos.");
        }
        if !(1..=60).contains(&numero(&d.tentativas, 0)) {
            erro(CampoNfe::Tentativas, "Entre 1 e 60.");
        }
        if numero(&d.intervalo_s, 0) < 1 {
            erro(
                CampoNfe::Intervalo,
                "No mínimo 1 segundo: a SEFAZ recusa consulta em rajada.",
            );
        }
        e
    }

    /// Por que a secao nao pode ser gravada, ou `None`.
    #[must_use]
    pub fn motivo_sem_salvar(&self, config: &Config) -> Option<String> {
        if let Some((_, t)) = self.erros().into_iter().next() {
            return Some(format!("NF-e: {t}"));
        }
        let mut c = config.clone();
        self.aplicar(&mut c);
        c.validar().err().map(|e| frase(&e.to_string()))
    }
}

/// O regime em palavras.
#[must_use]
pub fn rotulo_do_regime(crt: u8) -> &'static str {
    REGIMES
        .iter()
        .find(|(c, _)| *c == crt)
        .map_or("Não informado", |(_, r)| r)
}

/// O ambiente da NF-e, no selo da janela (o da NFS-e e' outro campo).
#[must_use]
pub fn ambiente_da_nfe(n: &Nfe) -> Ambiente {
    if n.ambiente == "producao" {
        Ambiente::Producao
    } else {
        Ambiente::Restrita
    }
}

/// O texto do selo do ambiente da NF-e: a SEFAZ chama de homologacao o que a NFS-e chama de producao restrita.
#[must_use]
pub fn selo_do_ambiente_da_nfe(a: Ambiente) -> &'static str {
    match a {
        Ambiente::Producao => "Produção",
        Ambiente::Restrita => "Homologação (testes)",
    }
}

/// O que o "Testar conexao com a SEFAZ" achou, em palavras e com o que fazer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResultadoSefaz {
    /// O servico respondeu em operacao.
    EmOperacao {
        /// A sigla da UF consultada.
        uf: String,
        /// O ambiente da consulta.
        producao: bool,
    },
    /// O servico respondeu parado ou fora do ar.
    Parado {
        /// O que a SEFAZ disse.
        motivo: String,
    },
    /// Nao respondeu.
    SemResposta,
    /// Nada saiu: certificado, senha ou configuracao.
    Recusado {
        /// A frase.
        texto: String,
    },
}

impl ResultadoSefaz {
    /// O resultado do status do servico.
    #[must_use]
    pub fn do_status(s: &StatusServico, uf: &str, producao: bool) -> Self {
        match s.situacao {
            SituacaoServico::EmOperacao => Self::EmOperacao {
                uf: uf.to_string(),
                producao,
            },
            SituacaoServico::SemResposta => Self::SemResposta,
            _ => Self::Parado {
                motivo: frase(&s.frase),
            },
        }
    }

    /// A frase para a tela.
    #[must_use]
    pub fn texto(&self) -> String {
        match self {
            Self::EmOperacao { uf, producao } => format!(
                "A SEFAZ de {uf} está respondendo ({}). O certificado e a conexão estão funcionando.",
                if *producao { "produção" } else { "homologação" }
            ),
            Self::Parado { motivo } => format!(
                "{motivo} Nada está errado com o seu cadastro: o serviço da SEFAZ é que não está disponível agora. Tente de novo mais tarde; se for urgente, a contingência (SVC) está nesta tela."
            ),
            Self::SemResposta => "A SEFAZ não respondeu. Confira a internet e o tempo limite (em Avançado) e tente de novo. Nenhuma nota foi enviada: este teste só pergunta se o serviço está no ar.".into(),
            Self::Recusado { texto } => format!("Nada foi enviado. {texto}"),
        }
    }

    /// Deu certo.
    #[must_use]
    pub fn ok(&self) -> bool {
        matches!(self, Self::EmOperacao { .. })
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn config() -> Config {
        Config::default()
    }

    #[test]
    fn o_padrao_nao_tem_erro_e_aplicar_nao_muda_nada() {
        let c = config();
        let f = FormularioNfe::de_config(&c);
        assert!(f.erros().is_empty(), "{:?}", f.erros());
        let mut c2 = c.clone();
        f.aplicar(&mut c2);
        assert_eq!(c2.nfe, c.nfe);
        assert!(!f.alterado());
    }

    #[test]
    fn a_mascara_e_a_conferencia_de_cada_campo() {
        let mut f = FormularioNfe::de_config(&config());
        assert_eq!(f.definir(CampoNfe::Uf, "sp1"), "SP");
        assert_eq!(f.definir(CampoNfe::Ie, "111.222-3"), "1112223");
        assert_eq!(f.definir(CampoNfe::Cep, "01001000"), "01001-000");
        assert_eq!(f.definir(CampoNfe::Fone, "11988887777"), "(11) 98888-7777");
        assert!(f.erros().is_empty());
        f.definir(CampoNfe::Uf, "XX");
        f.definir(CampoNfe::Ie, "1");
        f.definir(CampoNfe::Cep, "0100");
        f.definir(CampoNfe::CodMunicipio, "355");
        f.definir(CampoNfe::Serie, "");
        let e = f.erros();
        for c in [
            CampoNfe::Uf,
            CampoNfe::Ie,
            CampoNfe::Cep,
            CampoNfe::CodMunicipio,
            CampoNfe::Serie,
        ] {
            assert!(e.contains_key(&c), "{c:?}: {e:?}");
        }
        assert!(f.alterado());
    }

    #[test]
    fn a_contingencia_exige_o_motivo_de_15_a_255() {
        let mut f = FormularioNfe::de_config(&config());
        f.dados.svc = true;
        assert!(f.erros().contains_key(&CampoNfe::MotivoContingencia));
        f.definir(
            CampoNfe::MotivoContingencia,
            "SEFAZ do estado fora do ar desde as 9h",
        );
        assert!(f.erros().is_empty());
        f.definir(CampoNfe::MotivoContingencia, &"x".repeat(256));
        assert!(f.erros().contains_key(&CampoNfe::MotivoContingencia));
    }

    #[test]
    fn gravar_leva_so_digitos_e_poe_a_secao_na_configuracao_completa() {
        let mut c = config();
        c.nfe
            .urls
            .insert("status".into(), "http://127.0.0.1:1/x".into());
        let mut f = FormularioNfe::de_config(&c);
        f.dados.producao = true;
        f.definir(CampoNfe::Uf, "SP");
        f.definir(CampoNfe::Cep, "01001000");
        f.definir(CampoNfe::Fone, "1133334444");
        f.dados.crt = 1;
        let mut c2 = c.clone();
        f.aplicar(&mut c2);
        assert_eq!(c2.nfe.ambiente, "producao");
        assert_eq!(c2.nfe.endereco.cep, "01001000");
        assert_eq!(c2.nfe.endereco.fone, "1133334444");
        assert_eq!(c2.nfe.crt, 1);
        assert_eq!(
            c2.nfe.urls, c.nfe.urls,
            "o que a tela nao edita nao se perde"
        );
        assert_eq!(f.motivo_sem_salvar(&c), None);
        f.marcar_salvo();
        assert!(!f.alterado());
    }

    #[test]
    fn regime_e_ambiente_em_palavras() {
        assert_eq!(rotulo_do_regime(1), "Simples Nacional");
        assert_eq!(rotulo_do_regime(0), "Não informado");
        assert_eq!(
            selo_do_ambiente_da_nfe(Ambiente::Restrita),
            "Homologação (testes)"
        );
    }
}
