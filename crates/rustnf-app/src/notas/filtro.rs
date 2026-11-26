// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O filtro das notas da pasta: periodo, situacao e busca por texto.

use super::{ErroNotas, Registro, Situacao};

/// Uma chave de acesso colada na busca tem pelo menos este tamanho (a chave inteira tem 50).
const MINIMO_DA_CHAVE: usize = 20;

/// Quais notas do indice interessam. Sem nada preenchido, todas.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Filtro {
    de: Option<String>,
    ate: Option<String>,
    situacao: Option<Situacao>,
    palavras: Vec<Palavra>,
    chave: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Palavra {
    texto: String,
    digitos: String,
}

/// Minusculas e sem acento (so' as letras do portugues): a busca nao distingue `Joao` de `JOÃO`.
#[must_use]
pub fn sem_acento(texto: &str) -> String {
    texto
        .chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            outro => outro,
        })
        .collect()
}

fn so_digitos(texto: &str) -> String {
    texto.chars().filter(char::is_ascii_digit).collect()
}

/// `AAAA-MM-DD` com mes de 1 a 12 e dia de 1 a 31.
pub(super) fn dia_valido(data: &str) -> bool {
    let b = data.as_bytes();
    let numero = |de: usize, ate: usize| {
        data.get(de..ate)
            .filter(|t| t.bytes().all(|c| c.is_ascii_digit()))
            .and_then(|t| t.parse::<u32>().ok())
    };
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && numero(0, 4).is_some_and(|a| a >= 2000)
        && numero(5, 7).is_some_and(|m| (1..=12).contains(&m))
        && numero(8, 10).is_some_and(|d| (1..=31).contains(&d))
}

pub(super) fn dia(data: Option<&str>) -> Result<Option<String>, ErroNotas> {
    match data.map(str::trim).filter(|d| !d.is_empty()) {
        None => Ok(None),
        Some(d) if dia_valido(d) => Ok(Some(d.to_string())),
        Some(_) => Err(ErroNotas::DataInvalida),
    }
}

/// A busca como parte de uma chave de acesso: so' letras e digitos (com espacos ou pontuacao de agrupamento),
/// comecando pelos 7 digitos do municipio, com pelo menos 20 posicoes. Devolve a chave limpa, em maiusculas. Um nome
/// comprido, mesmo com numero no meio, nao vira chave por engano.
#[must_use]
pub fn chave_da_busca(busca: &str) -> Option<String> {
    let limpa: String = busca
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_uppercase())
        .collect();
    let so_chave = busca
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c.is_whitespace() || matches!(c, '.' | '-' | '/'));
    (so_chave
        && limpa.len() >= MINIMO_DA_CHAVE
        && limpa.bytes().take(7).all(|b| b.is_ascii_digit()))
    .then_some(limpa)
}

impl Filtro {
    /// O filtro de um periodo (`de` e `ate` em `AAAA-MM-DD`, os dois inclusive), de uma situacao e de uma busca. A
    /// busca acha todas as palavras, em qualquer ordem, no nome do tomador (sem acento), no CPF/CNPJ dele, no numero
    /// da NFS-e ou no da DPS; uma chave de acesso colada (inteira ou um trecho do comeco) acha a nota por ela.
    ///
    /// # Errors
    ///
    /// [`ErroNotas::DataInvalida`] se `de` ou `ate` nao for `AAAA-MM-DD`.
    pub fn novo(
        de: Option<&str>,
        ate: Option<&str>,
        situacao: Option<Situacao>,
        busca: &str,
    ) -> Result<Self, ErroNotas> {
        Ok(Self {
            de: dia(de)?,
            ate: dia(ate)?,
            situacao,
            palavras: sem_acento(busca)
                .split_whitespace()
                .map(|p| Palavra {
                    digitos: so_digitos(p),
                    texto: p.to_string(),
                })
                .collect(),
            chave: chave_da_busca(busca),
        })
    }

    /// O filtro das notas de um mes (`AAAA-MM`).
    ///
    /// # Errors
    ///
    /// [`ErroNotas::DataInvalida`] se `mes` nao for `AAAA-MM`.
    pub fn do_mes(mes: &str) -> Result<Self, ErroNotas> {
        let mes = mes.trim();
        let primeiro = format!("{mes}-01");
        if mes.len() != 7 || !dia_valido(&primeiro) {
            return Err(ErroNotas::DataInvalida);
        }
        Ok(Self {
            de: Some(primeiro),
            ate: Some(format!("{mes}-31")),
            ..Self::default()
        })
    }

    /// A nota passa no filtro.
    #[must_use]
    pub fn aceita(&self, r: &Registro) -> bool {
        let dia = r.emissao.get(..10).unwrap_or(&r.emissao);
        if self.de.as_deref().is_some_and(|de| dia < de)
            || self.ate.as_deref().is_some_and(|ate| dia > ate)
            || self.situacao.is_some_and(|s| s != r.situacao)
        {
            return false;
        }
        if let Some(chave) = &self.chave {
            return r.chave.contains(chave.as_str());
        }
        if self.palavras.is_empty() {
            return true;
        }
        let nome = sem_acento(&r.tomador_nome);
        let documento = so_digitos(&r.tomador_documento);
        let numeros = format!("{} {}", so_digitos(&r.numero), so_digitos(&r.numero_dps));
        self.palavras.iter().all(|p| {
            nome.contains(p.texto.as_str())
                || (!p.digitos.is_empty()
                    && (documento.contains(&p.digitos) || numeros.contains(&p.digitos)))
        })
    }

    /// As notas da lista que passam no filtro, na ordem em que vieram.
    #[must_use]
    pub fn aplicar(&self, notas: Vec<Registro>) -> Vec<Registro> {
        notas.into_iter().filter(|r| self.aceita(r)).collect()
    }
}
