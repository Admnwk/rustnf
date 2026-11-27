// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O que se faz com a pasta de notas da NF-e alem de guardar: o filtro da lista, a lista em CSV, a copia dos XML para
//! outra pasta e o indice refeito a partir dos arquivos guardados.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use rustnf::nfe::cstat::{AUTORIZADA, DENEGADA};
use rustnf::nfe::sefaz::resposta_evento::ler_proc_evento;
use rustnf::nfe::sefaz::{protocolo_epec_danfe, DesfechoEvento, ResultadoEvento};

use super::super::filtro::{dia, dia_valido, sem_acento};
use super::super::manutencao::{campo, subpastas};
use super::super::{ErroNotas, Reindexado};
use super::{
    chave_do_modelo, gravar_json, EventoGuardado, Indice, Inutilizacao, NotasNfe, Registro,
    Situacao, INUTILIZACAO,
};
use crate::atomico;

/// O nome da copia do indice que [`NotasNfe::reindexar`] deixa na pasta antes de gravar o novo.
pub const INDICE_ANTERIOR: &str = "indice.anterior.json";
/// O nome da lista que [`NotasNfe::exportar`] grava junto dos XML.
pub const LISTA_CSV: &str = "notas.csv";
/// Teto de leitura de um XML ao refazer o indice: 16 MiB.
const LIMITE_XML: u64 = 16 * 1024 * 1024;

const TITULOS: &str =
    "emissao;serie;numero;valor;situacao;ambiente;protocolo;destinatario_documento;destinatario_nome;chave";

/// Quais notas interessam. Sem nada preenchido, todas.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Filtro {
    de: Option<String>,
    ate: Option<String>,
    situacao: Option<Situacao>,
    palavras: Vec<String>,
}

fn so_digitos(texto: &str) -> String {
    texto.chars().filter(char::is_ascii_digit).collect()
}

impl Filtro {
    /// O filtro de um periodo (`de` e `ate` em `AAAA-MM-DD`, os dois inclusive), de uma situacao e de uma busca: todas as
    /// palavras, em qualquer ordem, no nome do destinatario (sem acento), no CPF/CNPJ dele, no numero ou na chave.
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
                .map(String::from)
                .collect(),
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
        let nome = sem_acento(&r.destinatario_nome);
        let numeros = format!(
            "{} {} {}",
            so_digitos(&r.destinatario_documento),
            so_digitos(&r.numero),
            r.chave
        );
        self.palavras.iter().all(|p| {
            let digitos = so_digitos(p);
            nome.contains(p.as_str()) || (!digitos.is_empty() && numeros.contains(&digitos))
        })
    }

    /// As notas da lista que passam no filtro, na ordem em que vieram.
    #[must_use]
    pub fn aplicar(&self, notas: Vec<Registro>) -> Vec<Registro> {
        notas.into_iter().filter(|r| self.aceita(r)).collect()
    }
}

/// As notas em CSV para planilha: separador `;`, valor com virgula decimal, data `AAAA-MM-DD`, uma linha de titulos e
/// fim de linha CRLF.
#[must_use]
pub fn csv(registros: &[Registro]) -> String {
    let mut saida = format!("{TITULOS}\r\n");
    for r in registros {
        let linha = [
            r.emissao.get(..10).unwrap_or(&r.emissao).to_string(),
            r.serie.clone(),
            r.numero.clone(),
            r.valor.replace('.', ","),
            r.situacao.texto().to_string(),
            r.ambiente.clone(),
            r.protocolo.clone(),
            r.destinatario_documento.clone(),
            r.destinatario_nome.clone(),
            r.chave.clone(),
        ]
        .map(|c| campo(&c))
        .join(";");
        saida.push_str(&linha);
        saida.push_str("\r\n");
    }
    saida
}

/// O que [`NotasNfe::exportar`] fez.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Exportadas {
    /// Quantos XML foram copiados (notas e eventos).
    pub copiadas: usize,
    /// As chaves das notas da lista que nao tem nenhum XML na pasta (constam da lista, sem o arquivo).
    pub sem_xml: Vec<String>,
}

fn entre<'a>(texto: &'a str, abre: &str, fecha: &str) -> Option<&'a str> {
    let i = texto.find(abre)? + abre.len();
    let f = texto[i..].find(fecha)?;
    Some(&texto[i..i + f])
}

/// O protocolo do EPEC como o DANFE o imprime, a partir do `procEventoNFe` guardado: o mesmo calculo da biblioteca.
pub(crate) fn protocolo_epec(proc_evento: &str) -> Option<String> {
    let lido = ler_proc_evento(proc_evento).ok()?;
    let r = ResultadoEvento {
        desfecho: DesfechoEvento::Registrado {
            cstat: lido.registro.cstat.clone(),
            motivo: lido.registro.motivo.clone(),
            protocolo: lido.registro.protocolo.clone(),
            fora_do_prazo: false,
        },
        chave: lido.chave.clone(),
        tp_evento: lido.tp_evento.clone(),
        seq: lido.seq.unwrap_or(1),
        autorizador: String::new(),
        xml_enviado: String::new(),
        xml_retorno: String::new(),
        proc_evento: Some(proc_evento.to_string()),
        recuperado: false,
        frase: String::new(),
        detalhe: String::new(),
    };
    protocolo_epec_danfe(&r).ok()
}

impl NotasNfe {
    /// Copia para `destino` o XML de cada nota de `registros` (o `nfeProc`, ou a NF-e assinada se a nota so' tem ela) e os
    /// eventos dela, e grava la a lista em CSV ([`LISTA_CSV`]). Nada sai da pasta de notas: sao copias.
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
            let mut copias = vec![self.caminho(r, "procNFe.xml")?];
            if !copias[0].is_file() {
                copias = vec![self.caminho_da_assinada(r)?];
            }
            for e in &r.eventos {
                copias.push(self.caminho_do_evento(r, &e.tipo, e.sequencia)?);
            }
            let mut achou = false;
            for origem in copias.iter().filter(|c| c.is_file()) {
                let nome = origem.file_name().ok_or(ErroNotas::Gravacao)?;
                let bytes = std::fs::read(origem).map_err(|_| ErroNotas::Leitura)?;
                atomico::gravar(&destino.join(nome), &bytes).map_err(|_| ErroNotas::Gravacao)?;
                saida.copiadas += 1;
                achou = true;
            }
            if !achou {
                saida.sem_xml.push(r.chave.clone());
            }
        }
        atomico::gravar(&destino.join(LISTA_CSV), csv(registros).as_bytes())
            .map_err(|_| ErroNotas::Gravacao)?;
        Ok(saida)
    }

    fn ler_arquivo(caminho: &Path) -> Option<String> {
        (std::fs::metadata(caminho).ok()?.len() <= LIMITE_XML)
            .then(|| std::fs::read_to_string(caminho).ok())
            .flatten()
    }

    /// A linha de um `nfeProc` guardado em `caminho`, se for uma NF-e legivel, com a chave do nome do arquivo, na pasta do
    /// mes dela e com um protocolo que autoriza ou denega.
    fn registro_do_proc(&self, caminho: &Path, chave: &str) -> Option<Registro> {
        let xml = Self::ler_arquivo(caminho)?;
        let nfe = rustnf::nfe::ler(xml.as_bytes()).ok()?;
        let prot = nfe.protocolo.as_ref()?;
        let situacao = if AUTORIZADA.contains(&prot.cstat.as_str()) {
            Situacao::Autorizada
        } else if DENEGADA.contains(&prot.cstat.as_str()) {
            Situacao::Denegada
        } else {
            return None;
        };
        let r = Registro {
            protocolo: prot.numero.clone(),
            cstat: prot.cstat.clone(),
            ..Registro::da_nfe(&nfe, situacao)
        };
        (r.chave == chave && self.caminho(&r, "procNFe.xml").ok()? == caminho).then_some(r)
    }

    /// A linha de uma NF-e assinada guardada em `caminho` (pendente).
    fn registro_da_assinada(&self, caminho: &Path, chave: &str) -> Option<Registro> {
        let xml = Self::ler_arquivo(caminho)?;
        let nfe = rustnf::nfe::ler(xml.as_bytes()).ok()?;
        let r = Registro::da_nfe(&nfe, Situacao::Pendente);
        (r.chave == chave && self.caminho_da_assinada(&r).ok()? == caminho).then_some(r)
    }

    /// Refaz o `indice.json` a partir dos arquivos guardados: serve quando o indice se perdeu ou ficou ilegivel. O indice
    /// que havia fica copiado ao lado ([`INDICE_ANTERIOR`]). A situacao sai dos arquivos (o `nfeProc` autoriza ou denega, o
    /// evento 110111 cancela, o 110140 sem `nfeProc` e' EPEC, a NF-e assinada so' e' pendente); o que nao esta nos arquivos
    /// (quantas consultas a retomada fez) vem do indice anterior, quando ele ainda e' legivel. A numeracao (`estado.json`)
    /// nao muda.
    ///
    /// # Errors
    ///
    /// [`ErroNotas::Leitura`], [`ErroNotas::Gravacao`] ou [`ErroNotas::Bloqueada`].
    pub fn reindexar(&self) -> Result<Reindexado, ErroNotas> {
        let _trava = super::Trava::obter(&self.pasta)?;
        let anterior = self.indice();
        let mut feito = Reindexado {
            indice_ilegivel: anterior.is_err(),
            ..Reindexado::default()
        };
        let anterior = anterior.unwrap_or_default();
        let mut notas: BTreeMap<String, Registro> = BTreeMap::new();
        let mut assinadas: Vec<(String, PathBuf)> = Vec::new();
        let mut eventos: Vec<(String, EventoGuardado, String)> = Vec::new();
        for ano in subpastas(&self.pasta, 4)? {
            for mes in subpastas(&ano, 2)? {
                for entrada in std::fs::read_dir(&mes).map_err(|_| ErroNotas::Leitura)? {
                    let caminho = entrada.map_err(|_| ErroNotas::Leitura)?.path();
                    let nome = caminho
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    if nome.starts_with('.') || !nome.ends_with(".xml") {
                        continue;
                    }
                    if let Some(c) = nome.strip_suffix("-procNFe.xml") {
                        match self
                            .chave(c)
                            .ok()
                            .and_then(|c| self.registro_do_proc(&caminho, c))
                        {
                            Some(r) => {
                                notas.insert(r.chave.clone(), r);
                            }
                            None => feito.ignorados += 1,
                        }
                    } else if let Some(c) = nome.strip_suffix("-NFe.xml") {
                        assinadas.push((c.to_string(), caminho));
                    } else if let Some(e) = evento_do_arquivo(&nome, &caminho, self.modelo) {
                        eventos.push(e);
                    } else {
                        feito.ignorados += 1;
                    }
                }
            }
        }
        for (c, caminho) in assinadas {
            let achada = self
                .chave(&c)
                .ok()
                .and_then(|c| self.registro_da_assinada(&caminho, c));
            match achada {
                Some(r) => {
                    notas.entry(r.chave.clone()).or_insert(r);
                }
                None => feito.ignorados += 1,
            }
        }
        for (chave, ev, proc) in eventos {
            let Some(r) = notas.get_mut(&chave) else {
                feito.ignorados += 1;
                continue;
            };
            r.eventos.push(ev.clone());
            match ev.tipo.as_str() {
                "110111" => r.situacao = Situacao::Cancelada,
                "110140" => {
                    if r.situacao == Situacao::Pendente {
                        r.situacao = Situacao::Epec;
                    }
                    r.protocolo_epec = protocolo_epec(&proc).unwrap_or_default();
                }
                _ => {}
            }
        }
        for r in notas.values_mut() {
            r.eventos
                .sort_by(|a, b| (&a.tipo, a.sequencia).cmp(&(&b.tipo, b.sequencia)));
            if let Some(a) = anterior.notas.iter().find(|a| a.chave == r.chave) {
                (r.desde, r.tentativas, r.ultima) = (a.desde, a.tentativas, a.ultima);
                // a substituicao nao esta em nenhum XML: so' o indice a sabe, e sem ela a retomada reenviaria a pendente
                r.substituida_por.clone_from(&a.substituida_por);
                r.substitui.clone_from(&a.substitui);
                if r.situacao == Situacao::Pendente {
                    r.cstat.clone_from(&a.cstat);
                }
            }
            if r.situacao == Situacao::Cancelada {
                feito.canceladas += 1;
            }
        }
        feito.notas = notas.len();
        let inutilizacoes = self.inutilizacoes_dos_arquivos(&mut feito)?;

        let caminho_do_indice = self.pasta.join(super::INDICE);
        if let Ok(bytes) = std::fs::read(&caminho_do_indice) {
            atomico::gravar(&self.pasta.join(INDICE_ANTERIOR), &bytes)
                .map_err(|_| ErroNotas::Gravacao)?;
        }
        let mut novo = Indice {
            versao: 1,
            notas: notas.into_values().collect(),
            inutilizacoes,
        };
        novo.notas
            .sort_by(|a, b| (&a.emissao, &a.chave).cmp(&(&b.emissao, &b.chave)));
        gravar_json(&caminho_do_indice, &novo)?;
        Ok(feito)
    }

    fn inutilizacoes_dos_arquivos(
        &self,
        feito: &mut Reindexado,
    ) -> Result<Vec<Inutilizacao>, ErroNotas> {
        let raiz = self.pasta.join(INUTILIZACAO);
        let mut saida = Vec::new();
        if !raiz.is_dir() {
            return Ok(saida);
        }
        for ano in subpastas(&raiz, 4)? {
            let ano_txt = ano
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            for entrada in std::fs::read_dir(&ano).map_err(|_| ErroNotas::Leitura)? {
                let caminho = entrada.map_err(|_| ErroNotas::Leitura)?.path();
                let nome = caminho
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let Some(base) = nome.strip_suffix("-procInutNFe.xml") else {
                    continue;
                };
                match faixa_do_arquivo(base, &ano_txt, &caminho) {
                    Some(f) => saida.push(f),
                    None => feito.ignorados += 1,
                }
            }
        }
        Ok(saida)
    }
}

/// `<chave>-<tipo>-<seq>-procEventoNFe.xml` -> a chave, o evento lido do arquivo e o texto dele.
fn evento_do_arquivo(
    nome: &str,
    caminho: &Path,
    modelo: &str,
) -> Option<(String, EventoGuardado, String)> {
    let base = nome.strip_suffix("-procEventoNFe.xml")?;
    let mut partes = base.split('-');
    let (chave, tipo, seq) = (partes.next()?, partes.next()?, partes.next()?);
    chave_do_modelo(chave, modelo).ok()?;
    let sequencia: u32 = seq.parse().ok()?;
    let texto = NotasNfe::ler_arquivo(caminho)?;
    let lido = ler_proc_evento(&texto).ok()?;
    (partes.next().is_none() && lido.tp_evento == tipo && lido.chave == chave).then(|| {
        (
            chave.to_string(),
            EventoGuardado {
                tipo: tipo.to_string(),
                sequencia,
                protocolo: lido.registro.protocolo,
            },
            texto,
        )
    })
}

/// `<serie>-<inicio>-<fim>` do nome do arquivo, mais o `Id` e o `nProt` do que esta dentro.
fn faixa_do_arquivo(base: &str, ano: &str, caminho: &Path) -> Option<Inutilizacao> {
    let mut partes = base.split('-');
    let serie = partes.next()?.to_string();
    let inicio: u64 = partes.next()?.parse().ok()?;
    let fim: u64 = partes.next()?.parse().ok()?;
    let texto = NotasNfe::ler_arquivo(caminho)?;
    let id = entre(&texto, "<infInut Id=\"", "\"")?.to_string();
    let protocolo = entre(&texto, "<nProt>", "</nProt>")
        .unwrap_or_default()
        .to_string();
    (partes.next().is_none() && fim >= inicio).then_some(Inutilizacao {
        id,
        serie,
        inicio,
        fim,
        ano: ano.to_string(),
        protocolo,
        situacao: Situacao::Inutilizada,
    })
}
