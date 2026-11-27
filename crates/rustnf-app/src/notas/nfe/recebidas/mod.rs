// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A pasta dos documentos RECEBIDOS (distribuicao de DF-e), `recebidas/` dentro da pasta da NF-e e separada das notas
//! emitidas.
//!
//! ```text
//! <pasta>/nfe/recebidas/<AAAA>/<MM>/<chave>-resNFe.xml                  o resumo (so' isto, ate manifestar)
//! <pasta>/nfe/recebidas/<AAAA>/<MM>/<chave>-procNFe.xml                 a NF-e completa
//! <pasta>/nfe/recebidas/<AAAA>/<MM>/<chave>-<tipo>-<seq>-procEventoNFe.xml   evento completo (de terceiros ou o seu)
//! <pasta>/nfe/recebidas/<AAAA>/<MM>/<chave>-<tipo>-<seq>-resEvento.xml       resumo de evento
//! <pasta>/nfe/recebidas/outros/<NSU>-<...>                             o que nao tem chave valida ou nao abriu
//! <pasta>/nfe/recebidas/indice.json                                     uma linha por NF-e recebida
//! <pasta>/nfe/recebidas/cursor-<ambiente>-<documento>.json              o cursor de NSU de cada interessado
//! ```
//!
//! `AAAA/MM` sao o ano e o mes da CHAVE (o da emissao). Toda gravacao e' atomica e protegida por uma trava de arquivo.
//!
//! # Quando o cursor anda
//!
//! [`NotasRecebidas::gravar_lote`] grava os documentos do lote e o indice e SO' ENTAO o cursor. Uma queda no meio deixa o
//! cursor no lote anterior: o proximo `sincronizar` pede o mesmo lote e regrava os mesmos arquivos (os nomes vem da chave,
//! entao repetir nao duplica nada). O cursor nunca pula um NSU.

mod registro;
#[cfg(test)]
mod testes;

use std::path::{Path, PathBuf};

use rustnf::nfe::sefaz::dfe::{Conteudo, Cursor, Doc};
use serde::{Deserialize, Serialize};

pub use registro::{Recebida, SituacaoRecebida};

use super::{chave_estrita, gravar_json, EventoGuardado};
use crate::atomico;
use crate::notas::trava::Trava;
use crate::notas::{ler_json, ErroNotas};

const INDICE: &str = "indice.json";
const OUTROS: &str = "outros";

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct Indice {
    versao: u32,
    recebidas: Vec<Recebida>,
    /// NSU cujo documento nao abriu ou nao foi lido: o cursor ja passou por eles, e `sincronizar` os pede de novo.
    ilegiveis: Vec<String>,
}

/// A pasta dos documentos recebidos.
#[derive(Debug, Clone)]
pub struct NotasRecebidas {
    pasta: PathBuf,
}

/// So' digitos (o documento do interessado vira pedaco de nome de arquivo).
fn so_digitos(texto: &str) -> Result<&str, ErroNotas> {
    if texto.is_empty() || !texto.bytes().all(|b| b.is_ascii_digit()) {
        return Err(ErroNotas::Gravacao);
    }
    Ok(texto)
}

fn ambiente_valido(ambiente: &str) -> Result<&str, ErroNotas> {
    match ambiente {
        "homologacao" | "producao" => Ok(ambiente),
        _ => Err(ErroNotas::Gravacao),
    }
}

/// `tpEvento` e sequencia como pedaco de nome de arquivo.
fn nome_do_evento(tipo: &str, seq: u64) -> Result<String, ErroNotas> {
    if tipo.len() != 6 || !tipo.bytes().all(|b| b.is_ascii_digit()) || seq == 0 || seq > 999 {
        return Err(ErroNotas::Gravacao);
    }
    Ok(format!("{tipo}-{seq}"))
}

impl NotasRecebidas {
    /// Abre `<nfe>/recebidas`, criando-a se faltar. `nfe` e' a pasta da NF-e (`NotasNfe::pasta`).
    ///
    /// # Errors
    ///
    /// [`ErroNotas::Pasta`] se a pasta nao puder ser criada.
    pub fn abrir(nfe: &Path) -> Result<Self, ErroNotas> {
        let pasta = nfe.join("recebidas");
        std::fs::create_dir_all(&pasta).map_err(|_| ErroNotas::Pasta)?;
        Ok(Self { pasta })
    }

    /// A pasta `nfe/recebidas`.
    #[must_use]
    pub fn pasta(&self) -> &Path {
        &self.pasta
    }

    fn indice(&self) -> Result<Indice, ErroNotas> {
        ler_json(&self.pasta.join(INDICE))
    }

    fn gravar_indice(&self, mut indice: Indice) -> Result<(), ErroNotas> {
        indice.versao = 1;
        indice
            .recebidas
            .sort_by(|a, b| (&a.emissao, &a.chave).cmp(&(&b.emissao, &b.chave)));
        indice.ilegiveis.sort();
        indice.ilegiveis.dedup();
        gravar_json(&self.pasta.join(INDICE), &indice)
    }

    fn arquivo_do_cursor(&self, ambiente: &str, documento: &str) -> Result<PathBuf, ErroNotas> {
        Ok(self.pasta.join(format!(
            "cursor-{}-{}.json",
            ambiente_valido(ambiente)?,
            so_digitos(documento)?
        )))
    }

    /// A pasta do mes da chave: `<AAAA>/<MM>`, tirados da propria chave (o ano e o mes da emissao).
    fn do_mes(&self, chave: &str) -> Result<PathBuf, ErroNotas> {
        let chave = chave_estrita(chave)?;
        let (aa, mm) = (&chave[2..4], &chave[4..6]);
        if !(1..=12).contains(&mm.parse::<u32>().unwrap_or(0)) {
            return Err(ErroNotas::ChaveInvalida);
        }
        Ok(self.pasta.join(format!("20{aa}")).join(mm))
    }

    // ------------------------------------------------------------------------------------------------- consulta

    /// O cursor do interessado no ambiente; sem arquivo, o de quem nunca consultou. Cursor ilegivel NAO vira "recomece do
    /// zero": recomecar e' o caminho do bloqueio 656.
    ///
    /// # Errors
    ///
    /// [`ErroNotas::CursorIlegivel`] se o arquivo nao e' o cursor; [`ErroNotas::Leitura`] se nao abre.
    pub fn cursor(&self, ambiente: &str, documento: &str) -> Result<Cursor, ErroNotas> {
        let caminho = self.arquivo_do_cursor(ambiente, documento)?;
        match std::fs::read_to_string(&caminho) {
            Ok(texto) => Cursor::de_json(&texto).map_err(|_| ErroNotas::CursorIlegivel),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Cursor::novo()),
            Err(_) => Err(ErroNotas::Leitura),
        }
    }

    /// As NF-e recebidas, da mais recente para a mais antiga.
    ///
    /// # Errors
    ///
    /// [`ErroNotas::Corrompido`] ou [`ErroNotas::Leitura`].
    pub fn listar(&self) -> Result<Vec<Recebida>, ErroNotas> {
        let mut v = self.indice()?.recebidas;
        v.sort_by(|a, b| (&b.emissao, &b.chave).cmp(&(&a.emissao, &a.chave)));
        Ok(v)
    }

    /// A linha de uma NF-e recebida.
    ///
    /// # Errors
    ///
    /// [`ErroNotas::ChaveInvalida`] ou [`ErroNotas::NaoEncontrada`].
    pub fn registro(&self, chave: &str) -> Result<Recebida, ErroNotas> {
        let chave = chave_estrita(chave)?;
        self.indice()?
            .recebidas
            .into_iter()
            .find(|r| r.chave == chave)
            .ok_or(ErroNotas::NaoEncontrada)
    }

    /// Os NSU cujo documento nao abriu ou nao foi lido, para pedir de novo.
    ///
    /// # Errors
    ///
    /// [`ErroNotas::Corrompido`] ou [`ErroNotas::Leitura`].
    pub fn ilegiveis(&self) -> Result<Vec<String>, ErroNotas> {
        Ok(self.indice()?.ilegiveis)
    }

    /// O XML que ha da nota: a NF-e completa se ja veio, senao o resumo.
    ///
    /// # Errors
    ///
    /// [`ErroNotas::ChaveInvalida`], [`ErroNotas::NaoEncontrada`] ou [`ErroNotas::Leitura`].
    pub fn ler_xml(&self, chave: &str) -> Result<String, ErroNotas> {
        let r = self.registro(chave)?;
        let base = self.do_mes(&r.chave)?;
        let (completa, resumo) = (
            base.join(format!("{}-procNFe.xml", r.chave)),
            base.join(format!("{}-resNFe.xml", r.chave)),
        );
        let caminho = if completa.is_file() { completa } else { resumo };
        std::fs::read_to_string(caminho).map_err(|_| ErroNotas::Leitura)
    }

    /// Onde esta (ou estaria) o `procNFe` da NF-e recebida: o arquivo da nota completa, que o resumo nao e'.
    ///
    /// # Errors
    ///
    /// [`ErroNotas::ChaveInvalida`] ou [`ErroNotas::NaoEncontrada`].
    pub fn caminho_do_proc(&self, chave: &str) -> Result<PathBuf, ErroNotas> {
        let r = self.registro(chave)?;
        Ok(self
            .do_mes(&r.chave)?
            .join(format!("{}-procNFe.xml", r.chave)))
    }

    /// Guarda o DANFE em PDF ao lado do `procNFe` (gravacao atomica). So' a NF-e completa tem DANFE: o resumo nao e' uma
    /// nota.
    ///
    /// # Errors
    ///
    /// [`ErroNotas::NaoEncontrada`] se a nota completa nao esta na pasta; [`ErroNotas::Gravacao`].
    pub fn guardar_danfe(&self, chave: &str, pdf: &[u8]) -> Result<PathBuf, ErroNotas> {
        let proc = self.caminho_do_proc(chave)?;
        if !proc.is_file() {
            return Err(ErroNotas::NaoEncontrada);
        }
        let destino = proc.with_extension("pdf");
        atomico::gravar(&destino, pdf).map_err(|_| ErroNotas::Gravacao)?;
        Ok(destino)
    }

    /// Quando o cursor foi gravado pela ultima vez (a hora do arquivo): e' a ultima vez que uma busca andou ou parou na
    /// espera. `None` se o interessado nunca buscou neste ambiente.
    #[must_use]
    pub fn cursor_gravado_em(
        &self,
        ambiente: &str,
        documento: &str,
    ) -> Option<chrono::DateTime<chrono::Utc>> {
        let caminho = self.arquivo_do_cursor(ambiente, documento).ok()?;
        let feita = std::fs::metadata(caminho).and_then(|m| m.modified()).ok()?;
        Some(feita.into())
    }

    // ------------------------------------------------------------------------------------------------- gravacao

    /// Grava os documentos do lote e o indice e, so' depois, o cursor (veja o inicio deste modulo). Chamar de novo com o
    /// mesmo lote regrava os mesmos arquivos e nao duplica nada.
    ///
    /// # Errors
    ///
    /// [`ErroNotas::Gravacao`], [`ErroNotas::Bloqueada`] ou os de leitura do indice. Com erro, o cursor NAO foi gravado.
    pub fn gravar_lote(
        &self,
        ambiente: &str,
        documento: &str,
        docs: &[Doc],
        cursor: &Cursor,
    ) -> Result<(), ErroNotas> {
        let alvo = self.arquivo_do_cursor(ambiente, documento)?;
        let _trava = Trava::obter(&self.pasta)?;
        self.gravar_docs(ambiente, docs)?;
        atomico::gravar(&alvo, cursor.para_json().as_bytes()).map_err(|_| ErroNotas::Gravacao)
    }

    /// Grava documentos que nao mexem no cursor (uma nota pedida pela chave, um NSU relido).
    ///
    /// # Errors
    ///
    /// Os de [`NotasRecebidas::gravar_lote`].
    pub fn gravar_documentos(&self, ambiente: &str, docs: &[Doc]) -> Result<(), ErroNotas> {
        let _trava = Trava::obter(&self.pasta)?;
        self.gravar_docs(ambiente, docs)
    }

    fn gravar_docs(&self, ambiente: &str, docs: &[Doc]) -> Result<(), ErroNotas> {
        ambiente_valido(ambiente)?;
        if docs.is_empty() {
            return Ok(());
        }
        let mut indice = self.indice()?;
        for d in docs {
            self.gravar_um(&mut indice, ambiente, d)?;
        }
        self.gravar_indice(indice)
    }

    /// Um documento: o arquivo e a linha do indice. O arquivo vai antes da linha.
    fn gravar_um(&self, indice: &mut Indice, ambiente: &str, d: &Doc) -> Result<(), ErroNotas> {
        let nsu = d.nsu.to_string();
        let guardar = |caminho: PathBuf, texto: &str| {
            atomico::gravar(&caminho, texto.as_bytes()).map_err(|_| ErroNotas::Gravacao)
        };
        if !d.e_ilegivel() {
            indice.ilegiveis.retain(|n| *n != nsu);
        }
        // sem chave de NF-e valida (outro esquema, ilegivel, chave torta): vai para `outros/`, nunca se perde
        let chave = d.chave();
        let mes = if chave.is_empty() {
            None
        } else {
            self.do_mes(chave).ok()
        };
        let (Some(mes), false) = (mes, d.e_ilegivel()) else {
            return self.guardar_a_parte(indice, d, &nsu);
        };
        let linha = |indice: &mut Indice| -> usize {
            match indice.recebidas.iter().position(|r| r.chave == chave) {
                Some(i) => i,
                None => {
                    indice.recebidas.push(Recebida::nova(chave, ambiente));
                    indice.recebidas.len() - 1
                }
            }
        };
        match &d.conteudo {
            Conteudo::ResumoNfe(r) => {
                guardar(mes.join(format!("{chave}-resNFe.xml")), &d.xml)?;
                let i = linha(indice);
                indice.recebidas[i].do_resumo(r, &nsu);
            }
            Conteudo::Nfe(n) => {
                guardar(mes.join(format!("{chave}-procNFe.xml")), &d.xml)?;
                let i = linha(indice);
                indice.recebidas[i].da_nota(n, &nsu);
            }
            Conteudo::ResumoEvento(e) => {
                let seq = e.seq.unwrap_or(1);
                let Ok(nome) = nome_do_evento(&e.tp_evento, seq) else {
                    return self.guardar_a_parte(indice, d, &nsu);
                };
                guardar(mes.join(format!("{chave}-{nome}-resEvento.xml")), &d.xml)?;
                let i = linha(indice);
                indice.recebidas[i].anotar_evento(EventoGuardado {
                    tipo: e.tp_evento.clone(),
                    sequencia: u32::try_from(seq).unwrap_or(1),
                    protocolo: e.protocolo.clone(),
                });
                indice.recebidas[i].nsu = nsu;
            }
            Conteudo::Evento(e) => {
                let seq = u64::from(e.seq.unwrap_or(1));
                let Ok(nome) = nome_do_evento(&e.tp_evento, seq) else {
                    return self.guardar_a_parte(indice, d, &nsu);
                };
                guardar(
                    mes.join(format!("{chave}-{nome}-procEventoNFe.xml")),
                    &d.xml,
                )?;
                let i = linha(indice);
                indice.recebidas[i].anotar_evento(EventoGuardado {
                    tipo: e.tp_evento.clone(),
                    sequencia: e.seq.unwrap_or(1),
                    protocolo: e.registro.protocolo.clone(),
                });
                indice.recebidas[i].nsu = nsu;
            }
            _ => return self.guardar_a_parte(indice, d, &nsu),
        }
        Ok(())
    }

    /// O documento sem lugar no indice vai inteiro para `outros/<NSU>-...`; o ilegivel entra na lista dos que se pedem
    /// de novo.
    fn guardar_a_parte(&self, indice: &mut Indice, d: &Doc, nsu: &str) -> Result<(), ErroNotas> {
        let (nome, texto) = match (&d.conteudo, &d.doczip_original) {
            (Conteudo::Ilegivel { .. }, Some(b64)) => (format!("{nsu}-ilegivel.b64"), b64.as_str()),
            (Conteudo::Ilegivel { .. }, None) => (format!("{nsu}-ilegivel.xml"), d.xml.as_str()),
            _ => (format!("{nsu}-outro.xml"), d.xml.as_str()),
        };
        atomico::gravar(&self.pasta.join(OUTROS).join(nome), texto.as_bytes())
            .map_err(|_| ErroNotas::Gravacao)?;
        if d.e_ilegivel() && !indice.ilegiveis.iter().any(|n| n == nsu) {
            indice.ilegiveis.push(nsu.to_string());
        }
        Ok(())
    }

    /// Guarda a manifestacao que o Ambiente Nacional registrou: o `procEventoNFe` ao lado da nota e a linha em dia.
    ///
    /// # Errors
    ///
    /// [`ErroNotas::NaoEncontrada`] se a nota nao esta no indice, e os de gravacao.
    pub fn guardar_manifestacao(
        &self,
        chave: &str,
        tipo: &str,
        protocolo: &str,
        proc_evento: &str,
    ) -> Result<Recebida, ErroNotas> {
        let chave = chave_estrita(chave)?;
        let caminho = self.do_mes(chave)?.join(format!(
            "{chave}-{}-procEventoNFe.xml",
            nome_do_evento(tipo, 1)?
        ));
        let _trava = Trava::obter(&self.pasta)?;
        let mut indice = self.indice()?;
        let r = indice
            .recebidas
            .iter_mut()
            .find(|r| r.chave == chave)
            .ok_or(ErroNotas::NaoEncontrada)?;
        atomico::gravar(&caminho, proc_evento.as_bytes()).map_err(|_| ErroNotas::Gravacao)?;
        r.anotar_evento(EventoGuardado {
            tipo: tipo.to_string(),
            sequencia: 1,
            protocolo: protocolo.to_string(),
        });
        let nova = r.clone();
        self.gravar_indice(indice)?;
        Ok(nova)
    }

    /// Poe a nota no indice como resumo vazio se ainda nao esta (para guardar a manifestacao de uma nota que o
    /// usuario informou pela chave e que a distribuicao ainda nao trouxe).
    ///
    /// # Errors
    ///
    /// [`ErroNotas::ChaveInvalida`] e os de gravacao.
    pub fn garantir_linha(&self, chave: &str, ambiente: &str) -> Result<Recebida, ErroNotas> {
        let chave = chave_estrita(chave)?;
        ambiente_valido(ambiente)?;
        let _trava = Trava::obter(&self.pasta)?;
        let mut indice = self.indice()?;
        if let Some(r) = indice.recebidas.iter().find(|r| r.chave == chave) {
            return Ok(r.clone());
        }
        let nova = Recebida::nova(chave, ambiente);
        indice.recebidas.push(nova.clone());
        self.gravar_indice(indice)?;
        Ok(nova)
    }
}
