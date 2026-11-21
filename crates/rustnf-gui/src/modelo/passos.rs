// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O primeiro uso em 3 passos e o que falta para a configuracao estar completa.

use rustnf_app::config::Config;

use super::campo::Campo;

/// Um dos tres passos do primeiro uso.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Passo {
    /// Dados do emitente.
    Emitente,
    /// Certificado A1 e sua senha.
    Certificado,
    /// Pasta das notas.
    Notas,
}

impl Passo {
    /// Os passos, na ordem.
    pub const TODOS: [Passo; 3] = [Passo::Emitente, Passo::Certificado, Passo::Notas];

    /// Posicao do passo, a partir de 0.
    #[must_use]
    pub fn indice(self) -> usize {
        match self {
            Self::Emitente => 0,
            Self::Certificado => 1,
            Self::Notas => 2,
        }
    }

    /// Titulo curto.
    #[must_use]
    pub fn titulo(self) -> &'static str {
        match self {
            Self::Emitente => "Emitente",
            Self::Certificado => "Certificado",
            Self::Notas => "Pasta das notas",
        }
    }

    /// O passo seguinte, se houver.
    #[must_use]
    pub fn proximo(self) -> Option<Self> {
        Self::TODOS.get(self.indice() + 1).copied()
    }

    /// O passo anterior, se houver.
    #[must_use]
    pub fn anterior(self) -> Option<Self> {
        self.indice()
            .checked_sub(1)
            .and_then(|i| Self::TODOS.get(i).copied())
    }

    /// Os campos que o passo preenche.
    #[must_use]
    pub fn campos(self) -> &'static [Campo] {
        match self {
            Self::Emitente => &[
                Campo::Documento,
                Campo::Nome,
                Campo::InscricaoMunicipal,
                Campo::Municipio,
                Campo::OpSimples,
                Campo::RegimeApuracao,
            ],
            Self::Certificado => &[Campo::Pfx, Campo::Senha],
            Self::Notas => &[Campo::NotasPasta],
        }
    }

    /// `Passo 2 de 3`.
    #[must_use]
    pub fn indicador(self) -> String {
        format!("Passo {} de {}", self.indice() + 1, Self::TODOS.len())
    }
}

/// O passo ja esta preenchido na configuracao.
#[must_use]
pub fn passo_completo(config: &Config, passo: Passo) -> bool {
    let e = &config.emitente;
    match passo {
        Passo::Emitente => {
            (!e.cnpj.is_empty() || !e.cpf.is_empty())
                && !e.nome.trim().is_empty()
                && !e.inscricao_municipal.trim().is_empty()
                && !e.municipio.is_empty()
                && (e.op_simples != 3 || e.regime_apuracao != 0)
        }
        Passo::Certificado => !config.sefin.pfx.trim().is_empty(),
        Passo::Notas => !config.notas.pasta.trim().is_empty(),
    }
}

/// Os passos que ainda faltam, na ordem.
#[must_use]
pub fn pendentes(config: &Config) -> Vec<Passo> {
    Passo::TODOS
        .into_iter()
        .filter(|p| !passo_completo(config, *p))
        .collect()
}

/// Os tres passos estao preenchidos.
#[must_use]
pub fn configuracao_completa(config: &Config) -> bool {
    pendentes(config).is_empty()
}

/// O que falta, em palavras, para a faixa do Inicio.
#[must_use]
pub fn o_que_falta(config: &Config) -> Vec<&'static str> {
    pendentes(config)
        .into_iter()
        .map(|p| match p {
            Passo::Emitente => "dados do emitente",
            Passo::Certificado => "certificado digital",
            Passo::Notas => "pasta das notas",
        })
        .collect()
}

#[cfg(test)]
mod testes {
    use super::*;

    fn completa() -> Config {
        let mut c = Config::default();
        c.emitente.cnpj = "11222333000181".into();
        c.emitente.nome = "EMPRESA TESTE".into();
        c.emitente.inscricao_municipal = "123456".into();
        c.emitente.municipio = "3550308".into();
        c.sefin.pfx = "certificado.pfx".into();
        c.notas.pasta = "notas".into();
        c
    }

    #[test]
    fn configuracao_vazia_pede_os_tres_passos_na_ordem() {
        let c = Config::default();
        assert_eq!(pendentes(&c), Passo::TODOS.to_vec());
        assert!(!configuracao_completa(&c));
        assert_eq!(
            o_que_falta(&c),
            vec![
                "dados do emitente",
                "certificado digital",
                "pasta das notas"
            ]
        );
    }

    #[test]
    fn cada_passo_so_fecha_com_o_que_lhe_cabe() {
        let mut c = completa();
        assert!(configuracao_completa(&c));
        c.emitente.nome = "   ".into();
        assert_eq!(pendentes(&c), vec![Passo::Emitente]);
        c.emitente.nome = "X".into();
        c.emitente.op_simples = 3;
        // ME/EPP sem regime de apuracao ainda falta
        assert_eq!(pendentes(&c), vec![Passo::Emitente]);
        c.emitente.regime_apuracao = 1;
        c.sefin.pfx.clear();
        assert_eq!(pendentes(&c), vec![Passo::Certificado]);
        c.sefin.pfx = "a.pfx".into();
        c.notas.pasta.clear();
        assert_eq!(pendentes(&c), vec![Passo::Notas]);
        // CPF vale no lugar do CNPJ
        let mut c = completa();
        c.emitente.cnpj.clear();
        assert_eq!(pendentes(&c), vec![Passo::Emitente]);
        c.emitente.cpf = "52998224725".into();
        assert!(configuracao_completa(&c));
    }

    #[test]
    fn navegacao_entre_passos() {
        assert_eq!(Passo::Emitente.anterior(), None);
        assert_eq!(Passo::Emitente.proximo(), Some(Passo::Certificado));
        assert_eq!(Passo::Notas.proximo(), None);
        assert_eq!(Passo::Notas.anterior(), Some(Passo::Certificado));
        assert_eq!(Passo::Certificado.indicador(), "Passo 2 de 3");
    }
}
