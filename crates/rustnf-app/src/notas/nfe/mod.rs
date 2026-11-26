// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A pasta de notas da NF-e (modelo 55): arquivos, sem banco, DENTRO da mesma `[notas].pasta` da NFS-e e separada dela.
//!
//! ```text
//! <pasta>/nfe/<AAAA>/<MM>/<chave>-procNFe.xml                  o nfeProc (NF-e mais o protocolo): autorizada, denegada
//! <pasta>/nfe/<AAAA>/<MM>/<chave>-NFe.xml                      a NF-e assinada, gravada ANTES de ir a rede (pendente)
//! <pasta>/nfe/<AAAA>/<MM>/<chave>-<tipo>-<seq>-procEventoNFe.xml   cada evento registrado (cancelamento, carta, EPEC)
//! <pasta>/nfe/inutilizacao/<AAAA>/<serie>-<inicio>-<fim>-procInutNFe.xml   cada faixa inutilizada
//! <pasta>/nfe/indice.json                                      uma linha por nota e por faixa inutilizada
//! <pasta>/nfe/estado.json                                      o ultimo numero consumido em cada serie
//! ```
//!
//! Toda gravacao e' atomica (arquivo temporario na mesma pasta e `rename`) e protegida por uma trava de arquivo. O nome
//! de cada arquivo vem so' da chave conferida: nada vindo de fora vira pedaco de caminho.
//!
//! # A numeracao
//!
//! * Nota AUTORIZADA, DENEGADA (e a cancelada, que foi autorizada antes) **gasta** o numero: a SEFAZ o guarda na base e
//!   recusa outro igual (MOC 7.00, Anexo I, regras 2B08-10 e 2B08-20, rejeicao 539 para o mesmo numero com outra chave; 2B08-40,
//!   rejeicao 205, para a denegada; 2B08-30, rejeicao 218, para a cancelada).
//! * Nota REJEITADA **nao gasta**: a regra de duplicidade so' olha a NF-e "ja cadastrada" e a Visao Geral (4.2, consumo
//!   indevido) manda corrigir a causa e reenviar. Por isso a rejeitada sai da pasta e o numero volta a estar livre (o MOC nao
//!   diz isso numa frase so': e' o que as duas regras juntas mostram).
//! * Numero PULADO, que nao vai mais ser usado, se INUTILIZA (MOC 7.00, Visao Geral, 5.3; rejeicao 206 se alguem tentar
//!   emiti-lo depois, regra 3B08-100).
//! * Nota PENDENTE (enviada sem resultado) conta como gasta ate se saber: ela pode ter sido autorizada.

mod manutencao;
mod registro;
#[cfg(test)]
mod testes;
#[cfg(test)]
mod testes_manutencao;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use rustnf::chave_dfe;
use serde::{Deserialize, Serialize};

pub(crate) use manutencao::protocolo_epec;
pub use manutencao::{csv, Exportadas, Filtro, INDICE_ANTERIOR, LISTA_CSV};
pub use registro::{EventoGuardado, Inutilizacao, Registro, Situacao};

use super::trava::Trava;
use super::{ano_e_mes, ler_json, ErroNotas};
use crate::atomico;

const INDICE: &str = "indice.json";
const ESTADO: &str = "estado.json";
const INUTILIZACAO: &str = "inutilizacao";
/// Maior `nNF`: 9 digitos.
const NUMERO_MAXIMO: u64 = 999_999_999;

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct Indice {
    versao: u32,
    notas: Vec<Registro>,
    inutilizacoes: Vec<Inutilizacao>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct Estado {
    versao: u32,
    /// Ultimo numero consumido, por serie.
    series: BTreeMap<String, u64>,
    /// Ate onde a numeracao veio de outro sistema (`definir_proximo`): o que esta ate aqui nao e' numero pulado.
    bases: BTreeMap<String, u64>,
}

/// Quem esta com um numero.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ocupante {
    /// Uma nota da pasta (a chave e a situacao).
    Nota(String, Situacao),
    /// Uma faixa inutilizada.
    Inutilizada,
}

/// A pasta de notas da NF-e.
#[derive(Debug, Clone)]
pub struct NotasNfe {
    pasta: PathBuf,
}

/// So' digitos, 44 posicoes, digito verificador e emitente certos e modelo 55: a chave vira nome de arquivo.
pub(crate) fn chave_estrita(chave: &str) -> Result<&str, ErroNotas> {
    let forma = chave.len() == 44 && chave.bytes().all(|b| b.is_ascii_digit());
    let valida = forma
        && chave_dfe::decompor(chave, true)
            .map(|c| c.valida() && c.modelo == "55")
            .unwrap_or(false);
    if valida {
        Ok(chave)
    } else {
        Err(ErroNotas::ChaveInvalida)
    }
}

/// Serie da NF-e: de 0 a 999, sem zeros a esquerda.
pub(crate) fn serie_nfe(serie: &str) -> Result<String, ErroNotas> {
    if serie.is_empty() || serie.len() > 3 || !serie.bytes().all(|b| b.is_ascii_digit()) {
        return Err(ErroNotas::SerieInvalida);
    }
    Ok(serie
        .parse::<u32>()
        .map_err(|_| ErroNotas::SerieInvalida)?
        .to_string())
}

fn gravar_json<T: Serialize>(caminho: &Path, valor: &T) -> Result<(), ErroNotas> {
    let texto = serde_json::to_string_pretty(valor).map_err(|_| ErroNotas::Gravacao)?;
    atomico::gravar(caminho, format!("{texto}\n").as_bytes()).map_err(|_| ErroNotas::Gravacao)
}

fn numero_da_serie(r: &Registro, serie: &str) -> Option<u64> {
    (serie_nfe(&r.serie).as_deref() == Ok(serie)).then(|| r.numero_inteiro())
}

impl NotasNfe {
    /// Abre `<raiz>/nfe`, criando-a se faltar. `raiz` e' a `[notas].pasta`.
    ///
    /// # Errors
    ///
    /// [`ErroNotas::Pasta`] se o caminho for vazio ou a pasta nao puder ser criada.
    pub fn abrir(raiz: impl AsRef<Path>) -> Result<Self, ErroNotas> {
        let raiz = raiz.as_ref();
        if raiz.as_os_str().is_empty() {
            return Err(ErroNotas::Pasta);
        }
        let pasta = raiz.join("nfe");
        std::fs::create_dir_all(&pasta).map_err(|_| ErroNotas::Pasta)?;
        Ok(Self { pasta })
    }

    /// A pasta `nfe`.
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

    fn gravar_indice(&self, mut indice: Indice) -> Result<(), ErroNotas> {
        indice.versao = 1;
        indice
            .notas
            .sort_by(|a, b| (&a.emissao, &a.chave).cmp(&(&b.emissao, &b.chave)));
        indice
            .inutilizacoes
            .sort_by(|a, b| (&a.serie, a.inicio).cmp(&(&b.serie, b.inicio)));
        gravar_json(&self.pasta.join(INDICE), &indice)
    }

    fn dos_arquivos(&self, r: &Registro) -> Result<PathBuf, ErroNotas> {
        chave_estrita(&r.chave)?;
        let (ano, mes) = ano_e_mes(&r.emissao)?;
        Ok(self.pasta.join(ano).join(mes))
    }

    fn caminho(&self, r: &Registro, sufixo: &str) -> Result<PathBuf, ErroNotas> {
        Ok(self
            .dos_arquivos(r)?
            .join(format!("{}-{sufixo}", chave_estrita(&r.chave)?)))
    }

    /// Onde esta (ou estaria) o `nfeProc` da nota.
    ///
    /// # Errors
    ///
    /// [`ErroNotas::ChaveInvalida`], [`ErroNotas::NaoEncontrada`] ou [`ErroNotas::DataInvalida`].
    pub fn caminho_do_proc(&self, chave: &str) -> Result<PathBuf, ErroNotas> {
        self.caminho(&self.registro(chave)?, "procNFe.xml")
    }

    fn caminho_da_assinada(&self, r: &Registro) -> Result<PathBuf, ErroNotas> {
        self.caminho(r, "NFe.xml")
    }

    fn caminho_do_evento(&self, r: &Registro, tipo: &str, seq: u32) -> Result<PathBuf, ErroNotas> {
        if tipo.len() != 6 || !tipo.bytes().all(|b| b.is_ascii_digit()) {
            return Err(ErroNotas::Gravacao);
        }
        self.caminho(r, &format!("{tipo}-{seq}-procEventoNFe.xml"))
    }

    // ------------------------------------------------------------------------------------------------- consulta

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

    /// As faixas inutilizadas, da serie e da faixa menores para as maiores.
    ///
    /// # Errors
    ///
    /// [`ErroNotas::Corrompido`] ou [`ErroNotas::Leitura`].
    pub fn inutilizacoes(&self) -> Result<Vec<Inutilizacao>, ErroNotas> {
        Ok(self.indice()?.inutilizacoes)
    }

    /// A linha do indice de uma nota.
    ///
    /// # Errors
    ///
    /// [`ErroNotas::ChaveInvalida`] ou [`ErroNotas::NaoEncontrada`].
    pub fn registro(&self, chave: &str) -> Result<Registro, ErroNotas> {
        let chave = chave_estrita(chave)?;
        self.indice()?
            .notas
            .into_iter()
            .find(|r| r.chave == chave)
            .ok_or(ErroNotas::NaoEncontrada)
    }

    /// As notas pendentes (enviadas sem resultado) e as em EPEC (a transmitir), da mais antiga para a mais nova.
    ///
    /// # Errors
    ///
    /// [`ErroNotas::Corrompido`] ou [`ErroNotas::Leitura`].
    pub fn a_resolver(&self) -> Result<Vec<Registro>, ErroNotas> {
        let mut v: Vec<Registro> = self
            .indice()?
            .notas
            .into_iter()
            .filter(|r| matches!(r.situacao, Situacao::Pendente | Situacao::Epec))
            .collect();
        v.sort_by(|a, b| (a.desde, &a.chave).cmp(&(b.desde, &b.chave)));
        Ok(v)
    }

    /// O XML que ha da nota: o `nfeProc` se ja existe, senao a NF-e assinada guardada (a pendente).
    ///
    /// # Errors
    ///
    /// [`ErroNotas::ChaveInvalida`], [`ErroNotas::NaoEncontrada`] ou [`ErroNotas::Leitura`].
    pub fn ler_xml(&self, chave: &str) -> Result<String, ErroNotas> {
        let r = self.registro(chave)?;
        let proc = self.caminho(&r, "procNFe.xml")?;
        let caminho = if proc.is_file() {
            proc
        } else {
            self.caminho_da_assinada(&r)?
        };
        std::fs::read_to_string(caminho).map_err(|_| ErroNotas::Leitura)
    }

    /// A NF-e assinada guardada (a que se reenvia), ou `None` se a nota ja tem o `nfeProc` e ela foi apagada.
    ///
    /// # Errors
    ///
    /// [`ErroNotas::ChaveInvalida`] ou [`ErroNotas::NaoEncontrada`].
    pub fn ler_assinada(&self, chave: &str) -> Result<Option<String>, ErroNotas> {
        let r = self.registro(chave)?;
        Ok(std::fs::read_to_string(self.caminho_da_assinada(&r)?).ok())
    }

    /// O `nfeProc` guardado (so' autorizada, denegada, cancelada ou EPEC ja transmitida).
    ///
    /// # Errors
    ///
    /// [`ErroNotas::ChaveInvalida`], [`ErroNotas::NaoEncontrada`] ou [`ErroNotas::Leitura`].
    pub fn ler_proc(&self, chave: &str) -> Result<String, ErroNotas> {
        let caminho = self.caminho_do_proc(chave)?;
        std::fs::read_to_string(caminho).map_err(|_| ErroNotas::Leitura)
    }

    /// Quem esta com o numero da serie, se alguem esta.
    ///
    /// # Errors
    ///
    /// [`ErroNotas::SerieInvalida`] ou erro de leitura.
    pub fn ocupante(&self, serie: &str, numero: u64) -> Result<Option<Ocupante>, ErroNotas> {
        let serie = serie_nfe(serie)?;
        Ok(ocupante_em(&self.indice()?, &serie, numero))
    }

    /// Os numeros da faixa que ja estao com uma nota ou com uma faixa inutilizada, em ordem: o que impede de inutiliza-la.
    ///
    /// # Errors
    ///
    /// [`ErroNotas::SerieInvalida`] ou erro de leitura.
    pub fn ocupados_na_faixa(
        &self,
        serie: &str,
        inicio: u64,
        fim: u64,
    ) -> Result<Vec<(u64, Ocupante)>, ErroNotas> {
        let serie = serie_nfe(serie)?;
        let indice = self.indice()?;
        let mut v: Vec<(u64, Ocupante)> = indice
            .notas
            .iter()
            .filter(|r| r.situacao.consome_numero())
            .filter_map(|r| {
                numero_da_serie(r, &serie)
                    .filter(|n| (inicio..=fim).contains(n))
                    .map(|n| (n, Ocupante::Nota(r.chave.clone(), r.situacao)))
            })
            .collect();
        for i in &indice.inutilizacoes {
            if serie_nfe(&i.serie).as_deref() == Ok(serie.as_str())
                && i.inicio <= fim
                && i.fim >= inicio
            {
                v.push((i.inicio.max(inicio), Ocupante::Inutilizada));
            }
        }
        v.sort_by_key(|(n, _)| *n);
        Ok(v)
    }

    /// O proximo numero da serie: um acima do maior ja consumido, pendente ou inutilizado; 1 numa serie nova. Nao
    /// reserva nada: quem reserva e' [`NotasNfe::guardar_pendente`].
    ///
    /// # Errors
    ///
    /// [`ErroNotas::SerieInvalida`] ou erro de leitura.
    pub fn proximo_numero(&self, serie: &str) -> Result<u64, ErroNotas> {
        let serie = serie_nfe(serie)?;
        Ok(self.ultimo_numero(&serie)?.saturating_add(1))
    }

    fn ultimo_numero(&self, serie: &str) -> Result<u64, ErroNotas> {
        let indice = self.indice()?;
        let confirmado = self.estado()?.series.get(serie).copied().unwrap_or(0);
        let nas_notas = indice
            .notas
            .iter()
            .filter(|r| r.situacao.consome_numero())
            .filter_map(|r| numero_da_serie(r, serie))
            .max()
            .unwrap_or(0);
        let nas_faixas = indice
            .inutilizacoes
            .iter()
            .filter(|i| serie_nfe(&i.serie).as_deref() == Ok(serie))
            .map(|i| i.fim)
            .max()
            .unwrap_or(0);
        Ok(confirmado.max(nas_notas).max(nas_faixas))
    }

    /// Os numeros PULADOS da serie: abaixo do ultimo consumido e sem nota nem faixa inutilizada. Sao os que se inutilizam.
    ///
    /// # Errors
    ///
    /// [`ErroNotas::SerieInvalida`] ou erro de leitura.
    pub fn pulados(&self, serie: &str) -> Result<Vec<u64>, ErroNotas> {
        let serie = serie_nfe(serie)?;
        let indice = self.indice()?;
        let ultimo = self.ultimo_numero(&serie)?;
        let base = self.estado()?.bases.get(&serie).copied().unwrap_or(0);
        Ok((base + 1..=ultimo)
            .filter(|n| ocupante_em(&indice, &serie, *n).is_none())
            .collect())
    }

    /// A proxima sequencia do evento (`nSeqEvento`) do tipo na nota: 1 mais a maior ja registrada.
    ///
    /// # Errors
    ///
    /// [`ErroNotas::ChaveInvalida`] ou [`ErroNotas::NaoEncontrada`].
    pub fn proxima_sequencia(&self, chave: &str, tipo: &str) -> Result<u32, ErroNotas> {
        Ok(self
            .registro(chave)?
            .eventos
            .iter()
            .filter(|e| e.tipo == tipo)
            .map(|e| e.sequencia)
            .max()
            .unwrap_or(0)
            + 1)
    }

    // ------------------------------------------------------------------------------------------------- gravacao

    /// Grava a NF-e ASSINADA como pendente ANTES de ela ir a rede, e com isso reserva o numero: uma queda no meio do
    /// envio nunca perde a chave. A nota que ja esta pendente (mesma chave) e' regravada igual.
    ///
    /// # Errors
    ///
    /// [`ErroNotas::NumeroNfeOcupado`] se outra nota ou faixa inutilizada tem o numero; chave, data ou serie invalidas;
    /// erro de pasta.
    pub fn guardar_pendente(
        &self,
        registro: &Registro,
        nfe_assinada: &str,
    ) -> Result<(), ErroNotas> {
        let serie = serie_nfe(&registro.serie)?;
        let numero = registro.numero_inteiro();
        if numero == 0 || numero > NUMERO_MAXIMO {
            return Err(ErroNotas::NumeroInvalido);
        }
        let caminho = self.caminho_da_assinada(registro)?;
        let _trava = Trava::obter(&self.pasta)?;
        let mut indice = self.indice()?;
        match ocupante_em(&indice, &serie, numero) {
            Some(Ocupante::Nota(chave, Situacao::Pendente)) if chave == registro.chave => {}
            Some(_) => return Err(ErroNotas::NumeroNfeOcupado),
            None => {}
        }
        atomico::gravar(&caminho, nfe_assinada.as_bytes()).map_err(|_| ErroNotas::Gravacao)?;
        indice.notas.retain(|r| r.chave != registro.chave);
        indice.notas.push(Registro {
            situacao: Situacao::Pendente,
            ..registro.clone()
        });
        self.gravar_indice(indice)
    }

    /// Muda a linha de uma nota sob a trava; `f` recebe a linha e devolve `true` para gravar.
    fn alterar(
        &self,
        chave: &str,
        f: impl FnOnce(&mut Registro, &mut Estado) -> Result<(), ErroNotas>,
    ) -> Result<(), ErroNotas> {
        let chave = chave_estrita(chave)?;
        let _trava = Trava::obter(&self.pasta)?;
        let mut indice = self.indice()?;
        let mut estado = self.estado()?;
        let antes = estado.series.clone();
        let r = indice
            .notas
            .iter_mut()
            .find(|r| r.chave == chave)
            .ok_or(ErroNotas::NaoEncontrada)?;
        f(r, &mut estado)?;
        if estado.series != antes {
            estado.versao = 1;
            gravar_json(&self.pasta.join(ESTADO), &estado)?;
        }
        self.gravar_indice(indice)
    }

    /// Fecha a nota com o resultado da SEFAZ: guarda o `nfeProc` (se veio), anota situacao, protocolo e `cStat` e, quando
    /// a SEFAZ tem o numero (autorizada, denegada, cancelada, EPEC), o consome.
    ///
    /// # Errors
    ///
    /// [`ErroNotas::NaoEncontrada`] e os de gravacao.
    pub fn concluir(
        &self,
        chave: &str,
        situacao: Situacao,
        nfe_proc: Option<&str>,
        protocolo: &str,
        cstat: &str,
    ) -> Result<(), ErroNotas> {
        let atual = self.registro(chave)?;
        if let Some(proc) = nfe_proc {
            let caminho = self.caminho(&atual, "procNFe.xml")?;
            atomico::gravar(&caminho, proc.as_bytes()).map_err(|_| ErroNotas::Gravacao)?;
        }
        self.alterar(chave, |r, estado| {
            r.situacao = situacao;
            r.cstat = cstat.to_string();
            if !protocolo.is_empty() {
                r.protocolo = protocolo.to_string();
            }
            if situacao.consome_numero() {
                consumir(estado, r);
            }
            Ok(())
        })?;
        if nfe_proc.is_some() && situacao != Situacao::Epec {
            // o nfeProc contem a NF-e: a copia assinada de antes nao faz mais falta
            let _ = std::fs::remove_file(self.caminho_da_assinada(&atual)?);
        }
        Ok(())
    }

    /// Desfaz a pendente: a SEFAZ REJEITOU a nota, nada foi gasto e o numero volta a ficar livre.
    ///
    /// # Errors
    ///
    /// [`ErroNotas::NaoEncontrada`], os de gravacao, ou [`ErroNotas::NumeroNfeOcupado`] se a nota nao e' pendente
    /// (autorizada nao se descarta).
    pub fn descartar_pendente(&self, chave: &str) -> Result<(), ErroNotas> {
        let atual = self.registro(chave)?;
        let _trava = Trava::obter(&self.pasta)?;
        let mut indice = self.indice()?;
        if atual.situacao != Situacao::Pendente {
            return Err(ErroNotas::NumeroNfeOcupado);
        }
        indice.notas.retain(|r| r.chave != atual.chave);
        self.gravar_indice(indice)?;
        let _ = std::fs::remove_file(self.caminho_da_assinada(&atual)?);
        Ok(())
    }

    /// O numero ficou com outra chave na SEFAZ (rejeicao 539): consome o numero da serie sem nota nesta pasta.
    ///
    /// # Errors
    ///
    /// [`ErroNotas::SerieInvalida`], [`ErroNotas::NumeroInvalido`] ou erro de gravacao.
    pub fn consumir_numero(&self, serie: &str, numero: u64) -> Result<(), ErroNotas> {
        if numero == 0 || numero > NUMERO_MAXIMO {
            return Err(ErroNotas::NumeroInvalido);
        }
        let _trava = Trava::obter(&self.pasta)?;
        self.consumir_sob_trava(serie, numero)
    }

    /// Faz a serie continuar em `proximo`: para quem ja emitia por outro sistema. So' anda para a frente: `proximo` abaixo
    /// do [`NotasNfe::proximo_numero`] de agora seria um numero ja gasto.
    ///
    /// # Errors
    ///
    /// [`ErroNotas::SerieInvalida`], [`ErroNotas::NumeroInvalido`], [`ErroNotas::NumeroNfeOcupado`] ou erro de pasta.
    pub fn definir_proximo(&self, serie: &str, proximo: u64) -> Result<(), ErroNotas> {
        let normal = serie_nfe(serie)?;
        if proximo == 0 || proximo > NUMERO_MAXIMO {
            return Err(ErroNotas::NumeroInvalido);
        }
        let _trava = Trava::obter(&self.pasta)?;
        if proximo < self.proximo_numero(&normal)? {
            return Err(ErroNotas::NumeroNfeOcupado);
        }
        self.consumir_sob_trava(&normal, proximo - 1)?;
        let mut estado = self.estado()?;
        let base = estado.bases.entry(normal).or_insert(0);
        *base = (*base).max(proximo - 1);
        estado.versao = 1;
        gravar_json(&self.pasta.join(ESTADO), &estado)
    }

    /// Conta uma consulta da retomada: `tentativas` mais uma e `ultima` no instante dado.
    ///
    /// # Errors
    ///
    /// [`ErroNotas::NaoEncontrada`] e os de gravacao.
    pub fn marcar_tentativa(&self, chave: &str, agora: i64) -> Result<(), ErroNotas> {
        self.alterar(chave, |r, _| {
            r.tentativas = r.tentativas.saturating_add(1);
            r.ultima = Some(agora);
            Ok(())
        })
    }

    /// Guarda o evento registrado (`procEventoNFe`) e o anota na nota. O cancelamento (110111) a poe como cancelada; o
    /// EPEC (110140) a poe como EPEC se ela ainda nao tem o `nfeProc`. Guardar o mesmo evento de novo troca o arquivo.
    ///
    /// # Errors
    ///
    /// [`ErroNotas::NaoEncontrada`] e os de gravacao.
    pub fn guardar_evento(
        &self,
        chave: &str,
        evento: &EventoGuardado,
        proc_evento: &str,
        protocolo_epec: &str,
    ) -> Result<(), ErroNotas> {
        let atual = self.registro(chave)?;
        let caminho = self.caminho_do_evento(&atual, &evento.tipo, evento.sequencia)?;
        atomico::gravar(&caminho, proc_evento.as_bytes()).map_err(|_| ErroNotas::Gravacao)?;
        self.alterar(chave, |r, estado| {
            r.eventos
                .retain(|e| !(e.tipo == evento.tipo && e.sequencia == evento.sequencia));
            r.eventos.push(evento.clone());
            match evento.tipo.as_str() {
                "110111" => r.situacao = Situacao::Cancelada,
                "110140" => {
                    if r.situacao == Situacao::Pendente {
                        r.situacao = Situacao::Epec;
                    }
                    r.protocolo_epec = protocolo_epec.to_string();
                }
                _ => {}
            }
            if r.situacao.consome_numero() {
                consumir(estado, r);
            }
            Ok(())
        })
    }

    /// Guarda a faixa inutilizada (`ProcInutNFe`) e consome os numeros dela.
    ///
    /// # Errors
    ///
    /// [`ErroNotas::NumeroNfeOcupado`] se algum numero da faixa e' de uma nota; [`ErroNotas::SerieInvalida`],
    /// [`ErroNotas::NumeroInvalido`] e os de gravacao.
    pub fn guardar_inutilizacao(
        &self,
        faixa: &Inutilizacao,
        proc_inut: &str,
    ) -> Result<(), ErroNotas> {
        let serie = serie_nfe(&faixa.serie)?;
        if faixa.inicio == 0 || faixa.fim < faixa.inicio || faixa.fim > NUMERO_MAXIMO {
            return Err(ErroNotas::NumeroInvalido);
        }
        let ano = faixa.ano.as_str();
        if ano.len() != 4 || !ano.bytes().all(|b| b.is_ascii_digit()) || faixa.id.len() > 60 {
            return Err(ErroNotas::DataInvalida);
        }
        let caminho = self.pasta.join(INUTILIZACAO).join(ano).join(format!(
            "{serie}-{}-{}-procInutNFe.xml",
            faixa.inicio, faixa.fim
        ));
        let _trava = Trava::obter(&self.pasta)?;
        let mut indice = self.indice()?;
        let nota_na_faixa = indice.notas.iter().any(|r| {
            numero_da_serie(r, &serie).is_some_and(|n| (faixa.inicio..=faixa.fim).contains(&n))
        });
        if nota_na_faixa {
            return Err(ErroNotas::NumeroNfeOcupado);
        }
        atomico::gravar(&caminho, proc_inut.as_bytes()).map_err(|_| ErroNotas::Gravacao)?;
        indice.inutilizacoes.retain(|i| i.id != faixa.id);
        indice.inutilizacoes.push(Inutilizacao {
            situacao: Situacao::Inutilizada,
            serie,
            ..faixa.clone()
        });
        self.gravar_indice(indice)?;
        self.consumir_sob_trava(&faixa.serie, faixa.fim)
    }

    fn consumir_sob_trava(&self, serie: &str, numero: u64) -> Result<(), ErroNotas> {
        let serie = serie_nfe(serie)?;
        let mut estado = self.estado()?;
        let atual = estado.series.entry(serie).or_insert(0);
        if numero > *atual {
            *atual = numero;
            estado.versao = 1;
            gravar_json(&self.pasta.join(ESTADO), &estado)?;
        }
        Ok(())
    }

    /// Guarda o DANFE (PDF) ao lado do XML (`<chave>-procNFe.pdf`, ou `<chave>-NFe.pdf` se a nota so' tem a assinada) e
    /// devolve o caminho. O PDF nao entra no indice.
    ///
    /// # Errors
    ///
    /// [`ErroNotas::ChaveInvalida`], [`ErroNotas::NaoEncontrada`] ou [`ErroNotas::Gravacao`].
    pub fn guardar_danfe(&self, chave: &str, pdf: &[u8]) -> Result<PathBuf, ErroNotas> {
        let r = self.registro(chave)?;
        let proc = self.caminho(&r, "procNFe.xml")?;
        let base = if proc.is_file() {
            proc
        } else {
            self.caminho_da_assinada(&r)?
        };
        let destino = base.with_extension("pdf");
        atomico::gravar(&destino, pdf).map_err(|_| ErroNotas::Gravacao)?;
        Ok(destino)
    }
}

fn ocupante_em(indice: &Indice, serie: &str, numero: u64) -> Option<Ocupante> {
    if let Some(r) = indice
        .notas
        .iter()
        .filter(|r| r.situacao.consome_numero())
        .find(|r| numero_da_serie(r, serie) == Some(numero))
    {
        return Some(Ocupante::Nota(r.chave.clone(), r.situacao));
    }
    indice
        .inutilizacoes
        .iter()
        .any(|i| {
            serie_nfe(&i.serie).as_deref() == Ok(serie) && (i.inicio..=i.fim).contains(&numero)
        })
        .then_some(Ocupante::Inutilizada)
}

/// Poe o numero da nota no estado da serie (so' para a frente).
fn consumir(estado: &mut Estado, r: &Registro) {
    if let (Ok(serie), n) = (serie_nfe(&r.serie), r.numero_inteiro()) {
        let atual = estado.series.entry(serie).or_insert(0);
        if n > *atual {
            *atual = n;
        }
    }
}
