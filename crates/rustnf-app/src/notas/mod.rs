// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A pasta de notas: arquivos, sem banco.
//!
//! ```text
//! <pasta>/<AAAA>/<MM>/<chave>.xml   o XML da NFS-e autorizada que a Sefin devolve
//! <pasta>/indice.json               uma linha por nota (chave, numero, serie, data, valor, situacao, tomador)
//! <pasta>/estado.json               o ultimo numero de DPS usado em cada serie
//! ```
//!
//! Toda gravacao e' atomica (arquivo temporario na mesma pasta e `rename`) e protegida por uma trava de arquivo. O
//! numero da DPS so' avanca em nota autorizada ou recuperada ([`Notas::confirmar_numero`]). O caminho de cada XML e'
//! montado so' com a chave conferida: nada vindo de fora vira pedaco de caminho.

mod filtro;
mod manutencao;
mod registro;
#[cfg(test)]
mod testes;
#[cfg(test)]
mod testes_manutencao;
mod trava;

use std::collections::BTreeMap;
use std::fmt;
use std::io::Read;
use std::path::{Path, PathBuf};

use rustnf::chave_dfe;
use serde::{Deserialize, Serialize};

pub use filtro::{chave_da_busca, sem_acento, Filtro};
pub use manutencao::{csv, Exportadas, Reindexado, INDICE_ANTERIOR, LISTA_CSV};
pub use registro::{Registro, Situacao};

use crate::atomico;
use crate::erro::Erro;
use trava::Trava;

const INDICE: &str = "indice.json";
const ESTADO: &str = "estado.json";
/// Teto de leitura do indice e do estado: 64 MiB.
const LIMITE_LEITURA: u64 = 64 * 1024 * 1024;
/// Maior nDPS: 15 digitos.
const NUMERO_MAXIMO: u64 = 999_999_999_999_999;

/// Por que a pasta de notas nao serviu. A mensagem nunca leva caminho de arquivo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErroNotas {
    /// A chave de acesso nao e' uma chave valida de 50 posicoes (so' `0-9A-Z`).
    ChaveInvalida,
    /// A data de emissao nao comeca por `AAAA-MM`.
    DataInvalida,
    /// A serie nao tem de 1 a 5 digitos.
    SerieInvalida,
    /// O numero da DPS e' zero ou passa de 15 digitos.
    NumeroInvalido,
    /// Nao ha nota com essa chave.
    NaoEncontrada,
    /// A pasta e' vazia ou nao deu para cria-la.
    Pasta,
    /// Nao deu para ler um arquivo da pasta.
    Leitura,
    /// Nao deu para gravar.
    Gravacao,
    /// O indice ou o estado nao e' o JSON esperado.
    Corrompido,
    /// Outro processo segura a trava da pasta.
    Bloqueada,
    /// O proximo numero pedido ja foi usado nesta serie (confirmado ou guardado na pasta).
    NumeroJaUsado,
}

impl fmt::Display for ErroNotas {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::ChaveInvalida => "notas: chave de acesso invalida",
            Self::DataInvalida => "notas: data de emissao invalida (esperado AAAA-MM-DD)",
            Self::SerieInvalida => "notas: serie invalida (1 a 5 digitos)",
            Self::NumeroInvalido => "notas: numero da DPS invalido (1 a 15 digitos)",
            Self::NaoEncontrada => "notas: nota nao encontrada na pasta",
            Self::Pasta => "notas: a pasta de notas nao existe e nao foi possivel cria-la",
            Self::Leitura => "notas: nao foi possivel ler a pasta de notas",
            Self::Gravacao => "notas: nao foi possivel gravar na pasta de notas",
            Self::Corrompido => {
                "notas: indice.json ou estado.json ilegivel; confira os arquivos da pasta"
            }
            Self::Bloqueada => "notas: a pasta esta em uso por outro processo; tente de novo",
            Self::NumeroJaUsado => {
                "notas: esse numero de DPS ja foi usado nesta serie; o proximo so' pode ir para a frente"
            }
        })
    }
}

impl std::error::Error for ErroNotas {}

impl From<ErroNotas> for Erro {
    fn from(e: ErroNotas) -> Self {
        match e {
            ErroNotas::NaoEncontrada => Self::NaoEncontrada(e.to_string()),
            _ => Self::Entrada(e.to_string()),
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct Indice {
    versao: u32,
    notas: Vec<Registro>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct Estado {
    versao: u32,
    /// Ultimo numero de DPS usado, por serie.
    series: BTreeMap<String, u64>,
}

/// A pasta de notas.
#[derive(Debug, Clone)]
pub struct Notas {
    pasta: PathBuf,
}

/// So' `0-9A-Z`, 50 posicoes e digitos verificadores certos. A conferencia da biblioteca ignora pontuacao; aqui nao,
/// porque a chave vira nome de arquivo.
fn chave_estrita(chave: &str) -> Result<&str, ErroNotas> {
    let forma = chave.len() == 50
        && chave
            .bytes()
            .all(|b| b.is_ascii_digit() || b.is_ascii_uppercase());
    if !forma || chave_dfe::conferir(chave).is_err() {
        return Err(ErroNotas::ChaveInvalida);
    }
    Ok(chave)
}

/// `AAAA` e `MM` de uma data que comeca por `AAAA-MM`.
fn ano_e_mes(data: &str) -> Result<(&str, &str), ErroNotas> {
    let (ano, mes) = (data.get(0..4), data.get(5..7));
    match (ano, mes, data.get(4..5)) {
        (Some(a), Some(m), Some("-"))
            if a.bytes().all(|b| b.is_ascii_digit())
                && m.bytes().all(|b| b.is_ascii_digit())
                && (1..=12).contains(&m.parse::<u32>().unwrap_or(0))
                && a >= "2000" =>
        {
            Ok((a, m))
        }
        _ => Err(ErroNotas::DataInvalida),
    }
}

/// Serie sem zeros a esquerda, de 1 a 5 digitos.
fn serie_normal(serie: &str) -> Result<String, ErroNotas> {
    if serie.is_empty() || serie.len() > 5 || !serie.bytes().all(|b| b.is_ascii_digit()) {
        return Err(ErroNotas::SerieInvalida);
    }
    Ok(serie
        .parse::<u32>()
        .map_err(|_| ErroNotas::SerieInvalida)?
        .to_string())
}

fn ler_json<T: for<'de> Deserialize<'de> + Default>(caminho: &Path) -> Result<T, ErroNotas> {
    let f = match std::fs::File::open(caminho) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(T::default()),
        Err(_) => return Err(ErroNotas::Leitura),
    };
    let mut bytes = Vec::new();
    f.take(LIMITE_LEITURA + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| ErroNotas::Leitura)?;
    if bytes.len() as u64 > LIMITE_LEITURA {
        return Err(ErroNotas::Corrompido);
    }
    serde_json::from_slice(&bytes).map_err(|_| ErroNotas::Corrompido)
}

fn gravar_json<T: Serialize>(caminho: &Path, valor: &T) -> Result<(), ErroNotas> {
    let texto = serde_json::to_string_pretty(valor).map_err(|_| ErroNotas::Gravacao)?;
    atomico::gravar(caminho, format!("{texto}\n").as_bytes()).map_err(|_| ErroNotas::Gravacao)
}

impl Notas {
    /// Abre a pasta, criando-a se faltar.
    ///
    /// # Errors
    ///
    /// [`ErroNotas::Pasta`] se o caminho for vazio ou a pasta nao puder ser criada.
    pub fn abrir(pasta: impl AsRef<Path>) -> Result<Self, ErroNotas> {
        let pasta = pasta.as_ref();
        if pasta.as_os_str().is_empty() {
            return Err(ErroNotas::Pasta);
        }
        std::fs::create_dir_all(pasta).map_err(|_| ErroNotas::Pasta)?;
        Ok(Self {
            pasta: pasta.to_path_buf(),
        })
    }

    /// A pasta.
    #[must_use]
    pub fn pasta(&self) -> &Path {
        &self.pasta
    }

    fn indice(&self) -> Result<Indice, ErroNotas> {
        ler_json(&self.pasta.join(INDICE))
    }

    fn estado(&self) -> Result<Estado, ErroNotas> {
        ler_json(&self.pasta.join(ESTADO))
    }

    fn caminho_do_xml(&self, chave: &str, emissao: &str) -> Result<PathBuf, ErroNotas> {
        let chave = chave_estrita(chave)?;
        let (ano, mes) = ano_e_mes(emissao)?;
        Ok(self.pasta.join(ano).join(mes).join(format!("{chave}.xml")))
    }

    /// Guarda o XML da NFS-e autorizada e poe (ou troca) a linha no indice. Guardar de novo a mesma chave troca o
    /// XML e a linha, mas uma nota que ja saiu de autorizada (cancelada, substituida, em analise fiscal) continua
    /// como estava.
    ///
    /// # Errors
    ///
    /// [`ErroNotas::ChaveInvalida`], [`ErroNotas::DataInvalida`], [`ErroNotas::SerieInvalida`] ou erro de pasta.
    pub fn guardar(&self, registro: &Registro, xml: &str) -> Result<(), ErroNotas> {
        let caminho = self.caminho_do_xml(&registro.chave, &registro.emissao)?;
        if !registro.serie.is_empty() {
            serie_normal(&registro.serie)?;
        }
        let _trava = Trava::obter(&self.pasta)?;
        let mut indice = self.indice()?;
        let mut novo = registro.clone();
        if let Some(i) = indice.notas.iter().position(|r| r.chave == novo.chave) {
            if indice.notas[i].situacao != Situacao::Autorizada {
                novo.situacao = indice.notas[i].situacao;
            }
            indice.notas.remove(i);
        }
        atomico::gravar(&caminho, xml.as_bytes()).map_err(|_| ErroNotas::Gravacao)?;
        indice.versao = 1;
        indice.notas.push(novo);
        indice
            .notas
            .sort_by(|a, b| (&a.emissao, &a.chave).cmp(&(&b.emissao, &b.chave)));
        gravar_json(&self.pasta.join(INDICE), &indice)
    }

    /// As notas do indice, da mais recente para a mais antiga.
    ///
    /// # Errors
    ///
    /// [`ErroNotas::Corrompido`] ou [`ErroNotas::Leitura`].
    pub fn listar(&self) -> Result<Vec<Registro>, ErroNotas> {
        let mut notas = self.indice()?.notas;
        notas.sort_by(|a, b| (&b.emissao, &b.chave).cmp(&(&a.emissao, &a.chave)));
        Ok(notas)
    }

    /// O XML guardado de uma nota.
    ///
    /// # Errors
    ///
    /// [`ErroNotas::ChaveInvalida`] se a chave nao for uma chave valida (nada chega ao sistema de arquivos);
    /// [`ErroNotas::NaoEncontrada`] se nao estiver no indice.
    pub fn ler_xml(&self, chave: &str) -> Result<String, ErroNotas> {
        let chave = chave_estrita(chave)?;
        let indice = self.indice()?;
        let registro = indice
            .notas
            .iter()
            .find(|r| r.chave == chave)
            .ok_or(ErroNotas::NaoEncontrada)?;
        let caminho = self.caminho_do_xml(&registro.chave, &registro.emissao)?;
        std::fs::read_to_string(caminho).map_err(|_| ErroNotas::Leitura)
    }

    /// Onde esta o XML guardado de uma nota que consta do indice. O caminho e' montado so' com a chave conferida e a
    /// data do indice, nunca com texto vindo de fora.
    ///
    /// # Errors
    ///
    /// [`ErroNotas::ChaveInvalida`] ou [`ErroNotas::NaoEncontrada`] (a nota nao consta do indice).
    pub fn caminho_do_xml_guardado(&self, chave: &str) -> Result<PathBuf, ErroNotas> {
        let chave = chave_estrita(chave)?;
        let indice = self.indice()?;
        let registro = indice
            .notas
            .iter()
            .find(|r| r.chave == chave)
            .ok_or(ErroNotas::NaoEncontrada)?;
        self.caminho_do_xml(&registro.chave, &registro.emissao)
    }

    /// Guarda o DANFSe (PDF) de uma nota do indice ao lado do XML dela (`<chave>.pdf`), de forma atomica, e devolve o
    /// caminho. O PDF nao entra no indice nem na numeracao: [`Notas::listar`] nao o ve.
    ///
    /// # Errors
    ///
    /// [`ErroNotas::ChaveInvalida`], [`ErroNotas::NaoEncontrada`] ou [`ErroNotas::Gravacao`].
    pub fn guardar_danfse(&self, chave: &str, pdf: &[u8]) -> Result<PathBuf, ErroNotas> {
        let xml = self.caminho_do_xml_guardado(chave)?;
        let destino = xml.with_extension("pdf");
        atomico::gravar(&destino, pdf).map_err(|_| ErroNotas::Gravacao)?;
        Ok(destino)
    }

    /// Marca a nota como cancelada no indice.
    ///
    /// # Errors
    ///
    /// [`ErroNotas::ChaveInvalida`] ou [`ErroNotas::NaoEncontrada`].
    pub fn marcar_cancelada(&self, chave: &str) -> Result<(), ErroNotas> {
        self.definir_situacao(chave, Situacao::Cancelada)
    }

    /// Poe a situacao da nota no indice, sob a trava da pasta. Nao confere a transicao: quem chama decide (a
    /// atualizacao em lote nunca devolve a autorizada uma nota cancelada ou substituida).
    ///
    /// # Errors
    ///
    /// [`ErroNotas::ChaveInvalida`] ou [`ErroNotas::NaoEncontrada`].
    pub fn definir_situacao(&self, chave: &str, situacao: Situacao) -> Result<(), ErroNotas> {
        let chave = chave_estrita(chave)?;
        let _trava = Trava::obter(&self.pasta)?;
        let mut indice = self.indice()?;
        let registro = indice
            .notas
            .iter_mut()
            .find(|r| r.chave == chave)
            .ok_or(ErroNotas::NaoEncontrada)?;
        registro.situacao = situacao;
        gravar_json(&self.pasta.join(INDICE), &indice)
    }

    /// O proximo numero de DPS da serie: um acima do maior ja confirmado ou guardado; 1 numa serie nova. Nao
    /// reserva nada: o numero so' avanca em [`Notas::confirmar_numero`].
    ///
    /// # Errors
    ///
    /// [`ErroNotas::SerieInvalida`] ou erro de leitura.
    pub fn proximo_numero(&self, serie: &str) -> Result<u64, ErroNotas> {
        let serie = serie_normal(serie)?;
        let confirmado = self.estado()?.series.get(&serie).copied().unwrap_or(0);
        let guardado = self
            .indice()?
            .notas
            .iter()
            .filter(|r| serie_normal(&r.serie).is_ok_and(|s| s == serie))
            .filter_map(|r| r.numero_dps.parse::<u64>().ok())
            .max()
            .unwrap_or(0);
        Ok(confirmado.max(guardado).saturating_add(1))
    }

    /// Registra que o `numero` de DPS foi usado numa nota autorizada ou recuperada. Nunca volta atras: numero menor
    /// ou igual ao ja confirmado nao muda nada.
    ///
    /// # Errors
    ///
    /// [`ErroNotas::SerieInvalida`], [`ErroNotas::NumeroInvalido`] ou erro de pasta.
    pub fn confirmar_numero(&self, serie: &str, numero: u64) -> Result<(), ErroNotas> {
        let serie = serie_normal(serie)?;
        if numero == 0 || numero > NUMERO_MAXIMO {
            return Err(ErroNotas::NumeroInvalido);
        }
        let _trava = Trava::obter(&self.pasta)?;
        let mut estado = self.estado()?;
        let atual = estado.series.entry(serie).or_insert(0);
        if numero > *atual {
            *atual = numero;
            estado.versao = 1;
            gravar_json(&self.pasta.join(ESTADO), &estado)?;
        }
        Ok(())
    }

    /// Faz a serie continuar em `proximo`: para quem ja emitia por outro sistema, o numero seguinte a ultima DPS de
    /// la. So' anda para a frente: `proximo` abaixo do [`Notas::proximo_numero`] de agora seria um numero ja usado, e
    /// a Sefin devolveria a nota antiga em vez de autorizar uma nova.
    ///
    /// # Errors
    ///
    /// [`ErroNotas::SerieInvalida`], [`ErroNotas::NumeroInvalido`], [`ErroNotas::NumeroJaUsado`] ou erro de pasta.
    pub fn definir_proximo(&self, serie: &str, proximo: u64) -> Result<(), ErroNotas> {
        let normal = serie_normal(serie)?;
        if proximo == 0 || proximo > NUMERO_MAXIMO {
            return Err(ErroNotas::NumeroInvalido);
        }
        let _trava = Trava::obter(&self.pasta)?;
        if proximo < self.proximo_numero(serie)? {
            return Err(ErroNotas::NumeroJaUsado);
        }
        let mut estado = self.estado()?;
        let atual = estado.series.entry(normal).or_insert(0);
        if proximo - 1 > *atual {
            *atual = proximo - 1;
            estado.versao = 1;
            gravar_json(&self.pasta.join(ESTADO), &estado)?;
        }
        Ok(())
    }
}
