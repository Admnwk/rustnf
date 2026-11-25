// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O que se faz com a pasta de notas alem de guardar e listar: a linha de uma nota, a lista em CSV, a copia dos XML
//! para outra pasta e o indice refeito a partir dos XML guardados.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use rustnf::nfse::leitura::ler_nfse;

use super::trava::Trava;
use super::{chave_estrita, gravar_json, ErroNotas, Indice, Notas, Registro, Situacao, INDICE};
use crate::atomico;

/// O nome da copia do indice que [`Notas::reindexar`] deixa na pasta antes de gravar o novo.
pub const INDICE_ANTERIOR: &str = "indice.anterior.json";
/// O nome da lista que [`Notas::exportar`] grava junto dos XML.
pub const LISTA_CSV: &str = "notas.csv";
/// Teto de leitura de um XML de nota ao refazer o indice: 16 MiB.
const LIMITE_XML: u64 = 16 * 1024 * 1024;

const TITULOS: &str =
    "emissao;numero;serie;numero_dps;valor_servico;situacao;tomador_documento;tomador_nome;chave";

/// Um campo do CSV. Texto que comeca por `=`, `+`, `-` ou `@` ganha um apostrofo na frente, para a planilha nao o
/// ler como formula; campo com `;`, aspas ou quebra de linha vai entre aspas.
fn campo(texto: &str) -> String {
    let texto = if texto.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{texto}")
    } else {
        texto.to_string()
    };
    if texto.contains([';', '"', '\n', '\r']) {
        format!("\"{}\"", texto.replace('"', "\"\""))
    } else {
        texto
    }
}

/// As notas em CSV para planilha: separador `;`, valor com virgula decimal, data `AAAA-MM-DD`, uma linha de titulos
/// e fim de linha CRLF. O CPF/CNPJ do tomador vai inteiro: e' a lista de quem emitiu, para a contabilidade.
#[must_use]
pub fn csv(registros: &[Registro]) -> String {
    let mut saida = format!("{TITULOS}\r\n");
    for r in registros {
        let linha = [
            r.emissao.get(..10).unwrap_or(&r.emissao).to_string(),
            r.numero.clone(),
            r.serie.clone(),
            r.numero_dps.clone(),
            r.valor_servico.replace('.', ","),
            r.situacao.texto().to_string(),
            r.tomador_documento.clone(),
            r.tomador_nome.clone(),
            r.chave.clone(),
        ]
        .map(|c| campo(&c))
        .join(";");
        saida.push_str(&linha);
        saida.push_str("\r\n");
    }
    saida
}

/// O que [`Notas::exportar`] fez.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Exportadas {
    /// Quantos XML foram copiados.
    pub copiadas: usize,
    /// As chaves das notas do indice cujo XML nao esta na pasta (constam da lista, sem o arquivo).
    pub sem_xml: Vec<String>,
}

/// O que [`Notas::reindexar`] fez.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Reindexado {
    /// Quantas notas o indice novo tem.
    pub notas: usize,
    /// Quantos arquivos `.xml` da pasta ficaram de fora (nao sao uma NFS-e legivel, ou estao fora do lugar).
    pub ignorados: usize,
    /// Quantas notas continuaram marcadas como canceladas (a marca vem do indice anterior: o XML nao a traz).
    pub canceladas: usize,
    /// O indice anterior nao pode ser lido: nenhuma marca de cancelamento foi aproveitada.
    pub indice_ilegivel: bool,
}

/// As subpastas de `pasta` cujo nome tem exatamente `digitos` digitos (`AAAA` ou `MM`).
fn subpastas(pasta: &Path, digitos: usize) -> Result<Vec<PathBuf>, ErroNotas> {
    let mut saida = Vec::new();
    for entrada in std::fs::read_dir(pasta).map_err(|_| ErroNotas::Leitura)? {
        let entrada = entrada.map_err(|_| ErroNotas::Leitura)?;
        let nome = entrada.file_name();
        let nome = nome.to_string_lossy();
        if nome.len() == digitos
            && nome.bytes().all(|b| b.is_ascii_digit())
            && entrada.path().is_dir()
        {
            saida.push(entrada.path());
        }
    }
    saida.sort();
    Ok(saida)
}

impl Notas {
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

    /// Copia para `destino` o XML de cada nota de `registros` (`<chave>.xml`) e grava la a lista delas em CSV
    /// ([`LISTA_CSV`]). Nada sai da pasta de notas: sao copias. Nota do indice sem o XML na pasta entra na lista e
    /// volta em [`Exportadas::sem_xml`].
    ///
    /// # Errors
    ///
    /// [`ErroNotas::Gravacao`] se nao der para criar a pasta de destino ou gravar nela.
    pub fn exportar(
        &self,
        registros: &[Registro],
        destino: &Path,
    ) -> Result<Exportadas, ErroNotas> {
        std::fs::create_dir_all(destino).map_err(|_| ErroNotas::Gravacao)?;
        let mut saida = Exportadas::default();
        for r in registros {
            let origem = self.caminho_do_xml(&r.chave, &r.emissao)?;
            match std::fs::read(&origem) {
                Ok(xml) => {
                    atomico::gravar(&destino.join(format!("{}.xml", r.chave)), &xml)
                        .map_err(|_| ErroNotas::Gravacao)?;
                    saida.copiadas += 1;
                }
                Err(_) => saida.sem_xml.push(r.chave.clone()),
            }
        }
        atomico::gravar(&destino.join(LISTA_CSV), csv(registros).as_bytes())
            .map_err(|_| ErroNotas::Gravacao)?;
        Ok(saida)
    }

    /// A linha do indice de um XML guardado em `caminho`, se ele for uma NFS-e legivel, com a chave do nome do
    /// arquivo e na pasta do mes dela.
    fn registro_do_xml(&self, caminho: &Path) -> Option<Registro> {
        let nome = caminho.file_stem()?.to_str()?;
        let chave = chave_estrita(nome).ok()?;
        if std::fs::metadata(caminho).ok()?.len() > LIMITE_XML {
            return None;
        }
        let xml = std::fs::read_to_string(caminho).ok()?;
        let mut registro = Registro::da_nfse(&ler_nfse(&xml).ok()?);
        if registro.chave.is_empty() {
            registro.chave = chave.to_string();
        }
        let no_lugar = self
            .caminho_do_xml(&registro.chave, &registro.emissao)
            .is_ok_and(|esperado| esperado == caminho);
        (registro.chave == chave && no_lugar).then_some(registro)
    }

    /// Refaz o `indice.json` a partir dos XML guardados em `<AAAA>/<MM>/<chave>.xml`: serve quando o indice se
    /// perdeu ou ficou ilegivel. O indice que havia fica copiado ao lado ([`INDICE_ANTERIOR`]). O XML da nota nao
    /// diz se ela foi cancelada: a situacao que nao e' autorizada (cancelada, substituida, em analise fiscal) e'
    /// aproveitada do indice anterior, quando ele ainda e' legivel; sem ele, toda nota volta como autorizada e a
    /// situacao se confere na Sefin (`notas atualizar`). A numeracao da DPS
    /// (`estado.json`) nao muda.
    ///
    /// # Errors
    ///
    /// [`ErroNotas::Leitura`], [`ErroNotas::Gravacao`] ou [`ErroNotas::Bloqueada`].
    pub fn reindexar(&self) -> Result<Reindexado, ErroNotas> {
        let _trava = Trava::obter(&self.pasta)?;
        let anterior = self.indice();
        let mut feito = Reindexado {
            indice_ilegivel: anterior.is_err(),
            ..Reindexado::default()
        };
        let marcadas: BTreeMap<String, Situacao> = anterior
            .map(|i| i.notas)
            .unwrap_or_default()
            .into_iter()
            .filter(|r| r.situacao != Situacao::Autorizada)
            .map(|r| (r.chave, r.situacao))
            .collect();

        let mut notas = Vec::new();
        for ano in subpastas(&self.pasta, 4)? {
            for mes in subpastas(&ano, 2)? {
                for entrada in std::fs::read_dir(&mes).map_err(|_| ErroNotas::Leitura)? {
                    let caminho = entrada.map_err(|_| ErroNotas::Leitura)?.path();
                    let oculto = caminho
                        .file_name()
                        .is_some_and(|n| n.to_string_lossy().starts_with('.'));
                    if oculto || caminho.extension().is_none_or(|e| e != "xml") {
                        continue;
                    }
                    match self.registro_do_xml(&caminho) {
                        Some(mut registro) => {
                            if let Some(situacao) = marcadas.get(&registro.chave) {
                                registro.situacao = *situacao;
                                if *situacao == Situacao::Cancelada {
                                    feito.canceladas += 1;
                                }
                            }
                            notas.push(registro);
                        }
                        None => feito.ignorados += 1,
                    }
                }
            }
        }
        notas.sort_by(|a, b| (&a.emissao, &a.chave).cmp(&(&b.emissao, &b.chave)));
        feito.notas = notas.len();

        let caminho_do_indice = self.pasta.join(INDICE);
        if let Ok(bytes) = std::fs::read(&caminho_do_indice) {
            atomico::gravar(&self.pasta.join(INDICE_ANTERIOR), &bytes)
                .map_err(|_| ErroNotas::Gravacao)?;
        }
        gravar_json(&caminho_do_indice, &Indice { versao: 1, notas })?;
        Ok(feito)
    }
}
