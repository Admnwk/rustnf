// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O que as telas Notas, Consultar e Cancelar pedem ao mundo: ler a pasta de notas, abrir o XML guardado, gerar e
//! guardar o DANFSe, guardar uma nota consultada, e as tres chamadas ao emissor (cancelar, conferir o cancelamento e
//! consultar). Tudo bloqueia (disco, PDF) ou fala com a rede: quem chama roda fora da linha da tela. Os testes usam
//! o emissor de demonstracao e uma pasta temporaria.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use rustnf_app::config::Config;
use rustnf_app::emissao::consultar_completa;
use rustnf_app::notas::{Notas, Registro, Situacao};
use rustnf_app::nucleo::{gerar_danfse, SituacaoNota};
use rustnf_app::pedido::cancelar_pedido_completo;

use super::{ErroTela, FabricaDeEmissor, Preparo, Servicos};
use crate::modelo::cancelamento::DesfechoDoCancelamento;
use crate::modelo::consulta::{DesfechoDaConsulta, NotaConsultada};
use crate::modelo::detalhe::LeituraDoXml;
use crate::modelo::mascara::frase;

/// O que se pede ao sistema para fazer com um arquivo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Destino {
    /// Abrir no programa padrao (o leitor de PDF).
    Abrir,
    /// Mostrar o arquivo na pasta, no explorador de arquivos.
    Mostrar,
}

/// Quem abre arquivos no sistema. Os programas externos so' abrem pela tela (o GPUI e' quem sabe fazer isso no
/// Windows); este servico decide SE a tela deve pedir, para os testes trocarem por um que anota o pedido e nao abre
/// nada.
pub trait AbrirArquivos: Send + Sync {
    /// A tela deve pedir ao sistema que faca `destino` com `caminho`? O de verdade diz que sim; o dos testes anota o
    /// pedido e diz que nao.
    fn pedir(&self, destino: Destino, caminho: &Path) -> bool;
}

/// O de verdade: a tela pede ao sistema.
#[derive(Debug, Clone, Copy, Default)]
pub struct DoSistema;

impl AbrirArquivos for DoSistema {
    fn pedir(&self, _destino: Destino, _caminho: &Path) -> bool {
        true
    }
}

/// Uma nota da pasta, com o que se leu do XML guardado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotaDaPasta {
    /// A linha do indice.
    pub registro: Registro,
    /// O que a leitura do XML deu.
    pub leitura: LeituraDoXml,
}

impl Servicos {
    /// Troca quem abre arquivos no sistema (os testes).
    #[must_use]
    pub fn com_abridor(mut self, abridor: Arc<dyn AbrirArquivos>) -> Self {
        self.abridor = abridor;
        self
    }

    /// Quem abre arquivos no sistema.
    #[must_use]
    pub fn abridor(&self) -> Arc<dyn AbrirArquivos> {
        Arc::clone(&self.abridor)
    }

    fn notas_da(&self, config: &Config) -> Result<Notas, ErroTela> {
        Notas::abrir(config.notas.pasta.trim()).map_err(|_| ErroTela::Pasta)
    }

    /// As notas do indice da pasta, da mais recente para a mais antiga.
    ///
    /// # Errors
    ///
    /// [`ErroTela::Pasta`] ou [`ErroTela::LeituraDaPasta`].
    pub fn listar_notas(&self, config: &Config) -> Result<Vec<Registro>, ErroTela> {
        self.notas_da(config)?
            .listar()
            .map_err(|_| ErroTela::LeituraDaPasta)
    }

    /// A nota da pasta com o que o XML guardado traz. XML ausente ou ilegivel nao e' erro: a leitura diz isso.
    ///
    /// # Errors
    ///
    /// [`ErroTela::Pasta`], [`ErroTela::LeituraDaPasta`] ou [`ErroTela::NotaAusente`] (a chave nao consta do indice).
    pub fn ler_nota(&self, config: &Config, chave: &str) -> Result<NotaDaPasta, ErroTela> {
        let notas = self.notas_da(config)?;
        let registro = notas
            .listar()
            .map_err(|_| ErroTela::LeituraDaPasta)?
            .into_iter()
            .find(|r| r.chave == chave)
            .ok_or(ErroTela::NotaAusente)?;
        let xml = notas.ler_xml(chave).ok();
        let leitura = LeituraDoXml::do_xml(xml.as_deref(), registro.situacao);
        Ok(NotaDaPasta { registro, leitura })
    }

    /// O XML guardado de uma nota da pasta, inteiro.
    ///
    /// # Errors
    ///
    /// [`ErroTela::Pasta`] ou [`ErroTela::XmlAusente`].
    pub fn xml_da_nota(&self, config: &Config, chave: &str) -> Result<String, ErroTela> {
        self.notas_da(config)?
            .ler_xml(chave)
            .map_err(|_| ErroTela::XmlAusente)
    }

    /// A nota (do emitente) consta do indice da pasta.
    #[must_use]
    pub fn nota_esta_na_pasta(&self, config: &Config, chave: &str) -> bool {
        self.listar_notas(config)
            .is_ok_and(|l| l.iter().any(|r| r.chave == chave))
    }

    /// O DANFSe em PDF de um XML de NFS-e, com a marca de cancelada quando for o caso e o `[danfse] canhoto` da
    /// configuracao.
    ///
    /// # Errors
    ///
    /// [`ErroTela::Danfse`] com a causa em frase.
    pub fn gerar_pdf(
        &self,
        config: &Config,
        xml: &str,
        situacao: Situacao,
    ) -> Result<Vec<u8>, ErroTela> {
        let situacao = match situacao {
            Situacao::Autorizada => SituacaoNota::Valida,
            Situacao::Cancelada => SituacaoNota::Cancelada,
        };
        gerar_danfse(xml, situacao, config.danfse.canhoto)
            .map_err(|e| ErroTela::Danfse(frase(&e.mensagem())))
    }

    /// Gera o DANFSe da nota da pasta e o grava ao lado do XML dela (`<chave>.pdf`, de forma atomica). O nome do
    /// arquivo sai so' da chave conferida do indice. Devolve onde ficou.
    ///
    /// # Errors
    ///
    /// [`ErroTela::NotaAusente`], [`ErroTela::XmlAusente`], [`ErroTela::Danfse`] ou
    /// [`ErroTela::GravacaoDoArquivo`].
    pub fn danfse_da_pasta(&self, config: &Config, chave: &str) -> Result<PathBuf, ErroTela> {
        let notas = self.notas_da(config)?;
        let registro = notas
            .listar()
            .map_err(|_| ErroTela::LeituraDaPasta)?
            .into_iter()
            .find(|r| r.chave == chave)
            .ok_or(ErroTela::NotaAusente)?;
        let xml = notas.ler_xml(chave).map_err(|_| ErroTela::XmlAusente)?;
        let pdf = self.gerar_pdf(config, &xml, registro.situacao)?;
        notas
            .guardar_danfse(chave, &pdf)
            .map_err(|_| ErroTela::GravacaoDoArquivo)
    }

    /// Onde esta o XML guardado de uma nota da pasta (para "Mostrar na pasta").
    ///
    /// # Errors
    ///
    /// [`ErroTela::NotaAusente`] ou [`ErroTela::XmlAusente`].
    pub fn caminho_do_xml(&self, config: &Config, chave: &str) -> Result<PathBuf, ErroTela> {
        let caminho = self
            .notas_da(config)?
            .caminho_do_xml_guardado(chave)
            .map_err(|_| ErroTela::NotaAusente)?;
        if caminho.is_file() {
            Ok(caminho)
        } else {
            Err(ErroTela::XmlAusente)
        }
    }

    /// Grava bytes num arquivo escolhido pela pessoa (o "Salvar como..." do DANFSe), de forma atomica.
    ///
    /// # Errors
    ///
    /// [`ErroTela::GravacaoDoArquivo`].
    pub fn gravar_arquivo(&self, destino: &Path, bytes: &[u8]) -> Result<(), ErroTela> {
        rustnf_app::atomico::gravar(destino, bytes).map_err(|_| ErroTela::GravacaoDoArquivo)
    }

    /// Guarda na pasta de notas a nota consultada (o XML e a linha do indice) e confirma o numero da DPS dela. So' se
    /// chama para nota do emitente da configuracao.
    ///
    /// # Errors
    ///
    /// [`ErroTela::Pasta`] ou [`ErroTela::GravacaoDoArquivo`].
    pub fn guardar_consultada(
        &self,
        config: &Config,
        nota: &NotaConsultada,
    ) -> Result<(), ErroTela> {
        let notas = self.notas_da(config)?;
        notas
            .guardar(&nota.registro, &nota.xml)
            .map_err(|_| ErroTela::GravacaoDoArquivo)?;
        if let Ok(numero) = nota.registro.numero_dps.trim().parse::<u64>() {
            if !nota.registro.serie.trim().is_empty() {
                let _ = notas.confirmar_numero(nota.registro.serie.trim(), numero);
            }
        }
        Ok(())
    }
}

fn recusa_antes_de_enviar(texto: String) -> DesfechoDoCancelamento {
    DesfechoDoCancelamento::Recusado { texto }
}

/// Pede o cancelamento da nota e devolve como terminou. Roda fora da linha da tela (`fora::rede`): monta o emissor, e
/// a camada de aplicacao marca a nota como cancelada na pasta so' quando a Sefin registrou o evento.
pub async fn cancelar_nota(
    fabrica: Arc<dyn FabricaDeEmissor>,
    preparo: Preparo,
    config: Config,
    chave: String,
    motivo: String,
    justificativa: String,
) -> DesfechoDoCancelamento {
    let emissor = match fabrica.criar(preparo) {
        Ok(e) => e,
        Err(e) => return recusa_antes_de_enviar(e.texto()),
    };
    let notas = Notas::abrir(config.notas.pasta.trim()).ok();
    let c =
        cancelar_pedido_completo(&*emissor, &chave, &motivo, &justificativa, notas.as_ref()).await;
    DesfechoDoCancelamento::de(c)
}

/// Confere na Sefin se a nota tem o cancelamento registrado (depois de um pedido sem resposta). Achando, marca a nota
/// como cancelada na pasta.
pub async fn conferir_cancelamento(
    fabrica: Arc<dyn FabricaDeEmissor>,
    preparo: Preparo,
    config: Config,
    chave: String,
) -> DesfechoDoCancelamento {
    let emissor = match fabrica.criar(preparo) {
        Ok(e) => e,
        Err(e) => return recusa_antes_de_enviar(e.texto()),
    };
    let r = emissor.cancelada(&chave).await;
    let marcada = if r.ok() {
        Notas::abrir(config.notas.pasta.trim())
            .ok()
            .map(|n| n.marcar_cancelada(&chave).is_ok())
    } else {
        None
    };
    DesfechoDoCancelamento::da_conferencia(r.status, marcada)
}

/// Consulta a nota pela chave de acesso e devolve como terminou. Roda fora da linha da tela. A nota devolvida so'
/// vale se a chave dela for a pedida.
pub async fn consultar_nota(
    fabrica: Arc<dyn FabricaDeEmissor>,
    preparo: Preparo,
    chave: String,
) -> DesfechoDaConsulta {
    let emissor = match fabrica.criar(preparo) {
        Ok(e) => e,
        Err(e) => {
            return DesfechoDaConsulta::Recusada { texto: e.texto() };
        }
    };
    let c = consultar_completa(&*emissor, &chave).await;
    DesfechoDaConsulta::de(c, &chave)
}
