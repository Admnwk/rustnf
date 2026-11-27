// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A emissao da NF-e fora da tela: o contexto do assistente (regime, UF, proximo numero), a conferencia do pedido, o
//! envio, a contingencia EPEC, o cadastro de produtos, o rascunho e a procura do cliente ja usado. Tudo que fala com a
//! SEFAZ passa pela camada de aplicacao (`rustnf_app::nfe`), a mesma da linha de comando; aqui so' se monta o cliente,
//! se abre o certificado e se traduz o resultado em tipos da tela. Tudo bloqueia (disco) ou fala com a rede: quem chama
//! roda fora da linha da tela. Nenhum teste toca a rede, o cofre nem a pasta do usuario.

use std::io::Read as _;
use std::path::Path;
use std::sync::Arc;

use chrono::{FixedOffset, Offset as _};
use rustnf_app::config::Config;
use rustnf_app::nfe::{self as app};
use rustnf_app::notas::nfe::Situacao;
use serde_json::Value;

use super::nfe::{montar, Sessao};
use super::{ErroTela, FabricaNfe, Preparo, Servicos};
use crate::modelo::nfe_emissao::{
    conferencia, por_que_nao_ha_epec, ConferenciaNfe, DesfechoNfeEmissao,
};
use crate::modelo::nfe_modelo::{ClienteConhecido, ModeloNfe};
use crate::modelo::nfe_nova::{ContextoNfe, PendenciaNfe};
use crate::modelo::nfe_produtos::{self, Cadastro, Lido, Produto};
use crate::modelo::nfe_rascunho::{self, Rascunho};

/// Le o arquivo da pasta de notas ate o limite; `None` se nao existe, nao le ou passa do limite.
fn ler_arquivo(pasta: &str, nome: &str, limite: u64) -> Option<Vec<u8>> {
    let pasta = pasta.trim();
    if pasta.is_empty() {
        return None;
    }
    let arquivo = std::fs::File::open(Path::new(pasta).join(nome)).ok()?;
    let mut bytes = Vec::new();
    arquivo.take(limite + 1).read_to_end(&mut bytes).ok()?;
    (bytes.len() as u64 <= limite).then_some(bytes)
}

impl Servicos {
    /// O que o assistente da NF-e precisa da configuracao e do relogio: regime, UF, o regime do PIS e da COFINS, hoje (em
    /// Brasilia) e o proximo numero da serie (so' para a fatura: quem numera de verdade e' a pasta de notas).
    #[must_use]
    pub fn contexto_da_nfe(&self, config: &Config) -> ContextoNfe {
        let brasilia = FixedOffset::west_opt(3 * 3600).unwrap_or(chrono::Utc.fix());
        let hoje = self.agora().with_timezone(&brasilia).date_naive();
        let serie = if config.nfe.serie.trim().is_empty() {
            "1"
        } else {
            config.nfe.serie.trim()
        };
        let numero = self
            .notas_nfe(config)
            .ok()
            .and_then(|n| n.proximo_numero(serie).ok())
            .unwrap_or(1);
        ContextoNfe::de_config(config, hoje, numero)
    }

    /// Quantas notas da pasta estao sem resposta (pendentes ou em contingencia): enquanto houver, outra nota para a mesma
    /// venda seria uma nota gemea.
    #[must_use]
    pub fn pendentes_nfe(&self, config: &Config) -> usize {
        self.notas_nfe(config)
            .and_then(|n| n.listar().map_err(|_| ErroTela::LeituraDaPasta))
            .map_or(0, |l| {
                l.iter()
                    .filter(|r| matches!(r.situacao, Situacao::Pendente | Situacao::Epec))
                    .count()
            })
    }

    // ---------------------------------------------------------------- cadastro de produtos

    /// Le o cadastro de produtos da pasta de notas. Sem pasta ou sem arquivo: vazio, sem aviso; arquivo ruim: vazio, com
    /// o aviso.
    #[must_use]
    pub fn ler_produtos(&self, config: &Config) -> Lido {
        match ler_arquivo(
            &config.notas.pasta,
            nfe_produtos::ARQUIVO,
            nfe_produtos::LIMITE,
        ) {
            Some(bytes) => Cadastro::ler(&bytes),
            None => {
                let existe = !config.notas.pasta.trim().is_empty()
                    && Path::new(config.notas.pasta.trim())
                        .join(nfe_produtos::ARQUIVO)
                        .exists();
                Lido {
                    cadastro: Cadastro::default(),
                    aviso: existe.then(|| nfe_produtos::AVISO_ARQUIVO_RUIM.to_string()),
                }
            }
        }
    }

    /// Grava o cadastro de produtos (de forma atomica).
    ///
    /// # Errors
    ///
    /// [`ErroTela::Pasta`] se nao ha pasta ou nao foi possivel gravar.
    pub fn gravar_produtos(&self, config: &Config, cadastro: &Cadastro) -> Result<(), ErroTela> {
        let pasta = config.notas.pasta.trim();
        if pasta.is_empty() {
            return Err(ErroTela::Pasta);
        }
        rustnf_app::atomico::gravar(
            &Path::new(pasta).join(nfe_produtos::ARQUIVO),
            &cadastro.codificar(),
        )
        .map_err(|_| ErroTela::Pasta)
    }

    /// Guarda os produtos novos de uma nota autorizada no cadastro. Arquivo que nao se le nao e' trocado (a pessoa nao
    /// perde o que tinha por causa de um arquivo ruim). Devolve quantos produtos entraram ou mudaram.
    ///
    /// # Errors
    ///
    /// [`ErroTela::Pasta`] se nao foi possivel gravar.
    pub fn guardar_produtos(
        &self,
        config: &Config,
        novos: Vec<Produto>,
    ) -> Result<usize, ErroTela> {
        if novos.is_empty() {
            return Ok(0);
        }
        let lido = self.ler_produtos(config);
        if lido.aviso.is_some() {
            return Err(ErroTela::Pasta);
        }
        let mut cadastro = lido.cadastro;
        let n = novos.len();
        for p in novos {
            cadastro.guardar(p);
        }
        self.gravar_produtos(config, &cadastro)?;
        Ok(n)
    }

    /// Apaga um produto do cadastro (le o arquivo de novo, tira o que tem o mesmo codigo ou, sem codigo, a mesma
    /// descricao, e grava). Arquivo que nao se le nao e' trocado.
    ///
    /// # Errors
    ///
    /// [`ErroTela::Pasta`] se o arquivo nao se le ou nao foi possivel gravar.
    pub fn apagar_produto(&self, config: &Config, produto: &Produto) -> Result<(), ErroTela> {
        let lido = self.ler_produtos(config);
        if lido.aviso.is_some() {
            return Err(ErroTela::Pasta);
        }
        let mut cadastro = lido.cadastro;
        let mesmo = |p: &Produto| {
            if produto.codigo.is_empty() {
                p.codigo.is_empty() && p.descricao == produto.descricao
            } else {
                p.codigo == produto.codigo
            }
        };
        if let Some(i) = cadastro.produtos.iter().position(mesmo) {
            cadastro.apagar(i);
            self.gravar_produtos(config, &cadastro)?;
        }
        Ok(())
    }

    // ---------------------------------------------------------------- rascunho

    /// Le o rascunho da Nova NF-e. Sem pasta, sem arquivo, ilegivel ou de outra versao: `None`.
    #[must_use]
    pub fn ler_rascunho_nfe(&self, config: &Config) -> Option<Rascunho> {
        let bytes = ler_arquivo(
            &config.notas.pasta,
            nfe_rascunho::ARQUIVO,
            nfe_rascunho::LIMITE,
        )?;
        nfe_rascunho::decodificar(&bytes)
    }

    /// Grava o rascunho da Nova NF-e, de forma atomica.
    ///
    /// # Errors
    ///
    /// [`ErroTela::Pasta`] se nao ha pasta ou nao foi possivel gravar.
    pub fn gravar_rascunho_nfe(
        &self,
        config: &Config,
        rascunho: &Rascunho,
    ) -> Result<(), ErroTela> {
        let pasta = config.notas.pasta.trim();
        if pasta.is_empty() {
            return Err(ErroTela::Pasta);
        }
        rustnf_app::atomico::gravar(
            &Path::new(pasta).join(nfe_rascunho::ARQUIVO),
            &nfe_rascunho::codificar(rascunho),
        )
        .map_err(|_| ErroTela::Pasta)
    }

    /// Apaga o rascunho da Nova NF-e (nao e' erro se ele nao existe).
    pub fn apagar_rascunho_nfe(&self, config: &Config) {
        let pasta = config.notas.pasta.trim();
        if !pasta.is_empty() {
            let _ = std::fs::remove_file(Path::new(pasta).join(nfe_rascunho::ARQUIVO));
        }
    }

    // ---------------------------------------------------------------- cliente ja usado e nota como modelo

    /// O cliente da NF-e mais recente (qualquer situacao em que o XML guardado exista) para o documento, ou `None`.
    #[must_use]
    pub fn cliente_conhecido_nfe(
        &self,
        config: &Config,
        documento: &str,
    ) -> Option<ClienteConhecido> {
        let notas = self.notas_nfe(config).ok()?;
        let doc: String = documento
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .map(|c| c.to_ascii_uppercase())
            .collect();
        for r in notas.listar().ok()? {
            if r.destinatario_documento.trim() != doc || matches!(r.situacao, Situacao::Pendente) {
                continue;
            }
            let Ok(xml) = notas.ler_xml(&r.chave) else {
                continue;
            };
            if let Some(c) = rustnf::nfe::ler(xml.as_bytes())
                .ok()
                .and_then(|n| ClienteConhecido::da_nfe(&n))
            {
                return Some(c);
            }
        }
        None
    }

    /// A NF-e da pasta como assistente ("Usar como modelo").
    ///
    /// # Errors
    ///
    /// [`ErroTela::Pasta`], [`ErroTela::NotaAusente`] ou [`ErroTela::XmlAusente`] (a nota sem XML legivel nao serve de
    /// modelo).
    pub fn modelo_da_nfe(&self, config: &Config, chave: &str) -> Result<ModeloNfe, ErroTela> {
        let notas = self.notas_nfe(config)?;
        notas.registro(chave).map_err(|_| ErroTela::NotaAusente)?;
        let xml = notas.ler_xml(chave).map_err(|_| ErroTela::XmlAusente)?;
        let nfe = rustnf::nfe::ler(xml.as_bytes()).map_err(|_| ErroTela::XmlAusente)?;
        Ok(ModeloNfe::da_nfe(&nfe, config.nfe.crt))
    }

    // ---------------------------------------------------------------- conferencia (sem rede)

    /// Confere o pedido pela camada de aplicacao (as regras do leiaute e da SEFAZ), sem rede e sem gastar numero.
    #[must_use]
    pub fn conferir_pedido_nfe(&self, config: &Config, pedido: Value) -> ConferenciaNfe {
        let notas = self.notas_nfe(config).ok();
        conferencia(app::montar_pedido(config, notas.as_ref(), pedido, true))
    }
}

/// Emite a NF-e: o pedido e' completado, conferido, assinado, GRAVADO como pendente e so' entao enviado. O que a SEFAZ
/// responde (autorizada, rejeitada, sem resposta, servico parado) volta como tipo da tela.
pub async fn emitir_nfe(
    fabrica: Arc<dyn FabricaNfe>,
    preparo: Preparo,
    config: Config,
    pedido: Value,
) -> DesfechoNfeEmissao {
    let s: Sessao = match montar(&fabrica, preparo, config) {
        Ok(s) => s,
        Err(e) => return recusada(&e),
    };
    DesfechoNfeEmissao::de(app::emitir(&s.contexto(), pedido, true).await)
}

/// Emite em contingencia EPEC. Antes de qualquer coisa confere que nao ha nota sem resposta na pasta: o EPEC e' OUTRA
/// nota (outro numero, outra chave), e com uma pendente ela seria gemea da primeira, que a SEFAZ pode ja ter recebido.
pub async fn epec_nfe(
    fabrica: Arc<dyn FabricaNfe>,
    preparo: Preparo,
    config: Config,
    pedido: Value,
    pendentes: usize,
) -> DesfechoNfeEmissao {
    if pendentes > 0 {
        return DesfechoNfeEmissao::Recusada {
            pendencias: vec![PendenciaNfe {
                passo: crate::modelo::nfe_nova::PassoNfe::Conferir,
                item: None,
                mensagem: por_que_nao_ha_epec(pendentes),
                como_resolver: "Use Retomar (Notas, aba NF-e) e volte aqui.".into(),
            }],
        };
    }
    let s: Sessao = match montar(&fabrica, preparo, config) {
        Ok(s) => s,
        Err(e) => return recusada(&e),
    };
    DesfechoNfeEmissao::do_epec(app::epec(&s.contexto(), pedido, true).await)
}

fn recusada(e: &ErroTela) -> DesfechoNfeEmissao {
    DesfechoNfeEmissao::Recusada {
        pendencias: vec![PendenciaNfe {
            passo: crate::modelo::nfe_nova::PassoNfe::Conferir,
            item: None,
            mensagem: e.texto(),
            como_resolver: "Resolva isso e envie de novo: nada saiu da sua maquina.".into(),
        }],
    }
}
