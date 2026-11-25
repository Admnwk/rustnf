// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Os campos da tela e o conjunto de erros que a conferencia devolve, campo a campo.

use std::collections::BTreeMap;

/// Coluna de uma linha da tabela de aliquotas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CampoAliquota {
    /// Ano do fato gerador.
    Ano,
    /// CBS de referencia.
    CbsReferencia,
    /// CBS.
    Cbs,
    /// IBS estadual.
    IbsUf,
    /// IBS municipal.
    IbsMun,
    /// Norma que publicou as aliquotas.
    Fonte,
}

impl CampoAliquota {
    /// As colunas, na ordem da tela.
    pub const TODOS: [CampoAliquota; 6] = [
        CampoAliquota::Ano,
        CampoAliquota::CbsReferencia,
        CampoAliquota::Cbs,
        CampoAliquota::IbsUf,
        CampoAliquota::IbsMun,
        CampoAliquota::Fonte,
    ];

    /// Nome da coluna para a tela.
    #[must_use]
    pub fn rotulo(self) -> &'static str {
        match self {
            Self::Ano => "Ano",
            Self::CbsReferencia => "CBS ref. (%)",
            Self::Cbs => "CBS (%)",
            Self::IbsUf => "IBS estadual (%)",
            Self::IbsMun => "IBS municipal (%)",
            Self::Fonte => "Fonte",
        }
    }
}

/// Um campo de formulario, o que identifica de onde veio cada erro.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Campo {
    /// CNPJ ou CPF do emitente.
    Documento,
    /// Nome ou razao social.
    Nome,
    /// Inscricao municipal.
    InscricaoMunicipal,
    /// Codigo IBGE do municipio.
    Municipio,
    /// Simples Nacional (1, 2 ou 3).
    OpSimples,
    /// Regime de apuracao do Simples (ME/EPP).
    RegimeApuracao,
    /// Regime especial de tributacao.
    RegimeEspecial,
    /// Serie da DPS.
    Serie,
    /// Aliquota padrao do ISS do emitente.
    AliquotaIssPadrao,
    /// Arquivo do certificado.
    Pfx,
    /// Senha do certificado (nunca guardada neste modelo: so' o erro dela).
    Senha,
    /// Tempo limite da Sefin.
    TempoLimite,
    /// Folga do relogio.
    FolgaRelogio,
    /// Pasta das notas.
    NotasPasta,
    /// O numero em que a serie da DPS deve continuar (nao e' gravado no arquivo: vai para a pasta de notas).
    ProximoDps,
    /// Uma celula da tabela de aliquotas: linha e coluna.
    Aliquota(usize, CampoAliquota),
}

/// Erros de uma conferencia: por campo, e os que nao pertencem a nenhum campo da tela.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Erros {
    /// Mensagem por campo (uma por campo).
    pub campos: BTreeMap<Campo, String>,
    /// Avisos que nao sao de campo nenhum (por exemplo, valor invalido numa secao que a tela nao edita).
    pub gerais: Vec<String>,
}

impl Erros {
    /// Nao ha erro nenhum.
    #[must_use]
    pub fn vazio(&self) -> bool {
        self.campos.is_empty() && self.gerais.is_empty()
    }

    /// Quantos erros ha.
    #[must_use]
    pub fn total(&self) -> usize {
        self.campos.len() + self.gerais.len()
    }

    /// A mensagem do campo, se ele errou.
    #[must_use]
    pub fn do_campo(&self, campo: Campo) -> Option<&str> {
        self.campos.get(&campo).map(String::as_str)
    }

    /// A mensagem de uma celula da tabela de aliquotas.
    #[must_use]
    pub fn da_aliquota(&self, linha: usize, coluna: CampoAliquota) -> Option<&str> {
        self.do_campo(Campo::Aliquota(linha, coluna))
    }

    /// A primeira mensagem (por ordem de campo; os gerais por ultimo).
    #[must_use]
    pub fn primeira(&self) -> Option<&str> {
        self.campos
            .values()
            .next()
            .or_else(|| self.gerais.first())
            .map(String::as_str)
    }

    /// So' os erros dos campos dados (os gerais ficam de fora).
    #[must_use]
    pub fn restrito_a(&self, campos: &[Campo]) -> Self {
        Self {
            campos: self
                .campos
                .iter()
                .filter(|(c, _)| campos.contains(c))
                .map(|(c, m)| (*c, m.clone()))
                .collect(),
            gerais: Vec::new(),
        }
    }

    /// Soma os erros de `outro` aos daqui; o erro que ja esta aqui para o mesmo campo prevalece.
    pub fn unir(&mut self, outro: Self) {
        for (c, m) in outro.campos {
            self.campos.entry(c).or_insert(m);
        }
        for g in outro.gerais {
            if !self.gerais.contains(&g) {
                self.gerais.push(g);
            }
        }
    }
}
