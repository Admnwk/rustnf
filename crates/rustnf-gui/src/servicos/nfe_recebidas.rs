// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! As NF-e RECEBIDAS (as compras) fora da tela: ler a pasta `nfe/recebidas`, gerar o DANFE da nota completa e falar com o
//! Ambiente Nacional (buscar notas novas, baixar a nota completa, responder a nota). O que o Ambiente Nacional responde
//! passa pela camada de aplicacao (`rustnf_app::nfe::recebidas`), a mesma da linha de comando; aqui so' se monta o cliente
//! com o transporte que a fabrica da NF-e der (o de verdade, ou o roteirizado da demonstracao e dos testes), se abre o
//! certificado e se traduz o resultado em tipos da tela. Tudo bloqueia (disco, PDF) ou fala com a rede: quem chama roda
//! fora da linha da tela. A espera entre buscas e o bloqueio sao da biblioteca: este arquivo nunca os contorna. Nenhum
//! teste toca a rede, o cofre nem a pasta do usuario.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use rustnf_app::config::Config;
use rustnf_app::nfe::{self as app, FiltroRecebidas};
use rustnf_app::notas::nfe::Recebida;

use super::nfe::{montar, recusado};
use super::{ErroTela, FabricaNfe, Preparo, Servicos};
use crate::modelo::mascara::frase;
use crate::modelo::nfe_detalhe::LeituraNfe;
use crate::modelo::nfe_eventos::DesfechoNfe;
use crate::modelo::nfe_recebidas::FilaDaTela;
use crate::modelo::nfe_recebidas_manifestar::Resposta;
use crate::modelo::nfe_recebidas_resultados::{DesfechoBaixa, DesfechoBusca};

/// Uma NF-e recebida da pasta, com o que se leu do XML guardado (so' a nota completa tem XML de NF-e).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecebidaDaPasta {
    /// A linha do indice.
    pub registro: Recebida,
    /// O que a leitura deu: a nota completa lida, ou so' o que o resumo traz.
    pub leitura: LeituraNfe,
}

impl Servicos {
    /// As NF-e recebidas do indice da pasta, da mais recente para a mais antiga.
    ///
    /// # Errors
    ///
    /// [`ErroTela::Pasta`] ou [`ErroTela::LeituraDaPasta`].
    pub fn listar_recebidas(&self, config: &Config) -> Result<Vec<Recebida>, ErroTela> {
        let notas = self.notas_nfe(config)?;
        app::listar(&notas, &FiltroRecebidas::default()).map_err(|_| ErroTela::LeituraDaPasta)
    }

    /// A ultima busca e a espera em vigor, como a pasta tem. So' le a pasta.
    ///
    /// # Errors
    ///
    /// [`ErroTela::Pasta`], [`ErroTela::LeituraDaPasta`] ou [`ErroTela::Valor`] (sem CNPJ nas configuracoes).
    pub fn fila_das_recebidas(&self, config: &Config) -> Result<FilaDaTela, ErroTela> {
        let notas = self.notas_nfe(config)?;
        match app::fila(&notas, config, self.agora()) {
            Ok(f) => Ok(FilaDaTela {
                ultima_busca: f.ultima_busca,
                espera_ate: f.espera.map(|e| e.ate),
            }),
            Err(rustnf_app::erro::Erro::Uso(t)) => Err(ErroTela::Valor(frase(&t))),
            Err(_) => Err(ErroTela::LeituraDaPasta),
        }
    }

    /// A NF-e recebida da pasta com o que o XML guardado traz. O resumo nao e' uma NF-e: so' a nota completa e' lida.
    ///
    /// # Errors
    ///
    /// [`ErroTela::Pasta`] ou [`ErroTela::NotaAusente`].
    pub fn ler_recebida(&self, config: &Config, chave: &str) -> Result<RecebidaDaPasta, ErroTela> {
        let notas = self.notas_nfe(config)?;
        let pasta = notas.recebidas().map_err(|_| ErroTela::Pasta)?;
        let registro = pasta.registro(chave).map_err(|_| ErroTela::NotaAusente)?;
        let xml = registro
            .completa
            .then(|| pasta.ler_xml(chave).ok())
            .flatten();
        let leitura = LeituraNfe::da_recebida(xml.as_deref(), &registro);
        Ok(RecebidaDaPasta { registro, leitura })
    }

    /// Gera o DANFE da nota completa recebida (com a marca de cancelada e o canhoto da configuracao) e o grava ao lado do
    /// XML (de forma atomica). Devolve onde ficou.
    ///
    /// # Errors
    ///
    /// [`ErroTela::NotaAusente`], [`ErroTela::Danfse`] (so' ha o resumo, ou o PDF nao sai) ou
    /// [`ErroTela::GravacaoDoArquivo`].
    pub fn danfe_da_recebida(&self, config: &Config, chave: &str) -> Result<PathBuf, ErroTela> {
        let notas = self.notas_nfe(config)?;
        let pdf = app::danfe_da_recebida(&notas, config, chave).map_err(|e| match e {
            rustnf_app::erro::Erro::NaoEncontrada(_) => ErroTela::NotaAusente,
            outro => ErroTela::Danfse(frase(&outro.mensagem())),
        })?;
        notas
            .recebidas()
            .and_then(|p| p.guardar_danfe(chave, &pdf))
            .map_err(|_| ErroTela::GravacaoDoArquivo)
    }

    /// Onde esta o `procNFe` guardado de uma nota recebida (para "Mostrar na pasta").
    ///
    /// # Errors
    ///
    /// [`ErroTela::NotaAusente`] ou [`ErroTela::XmlAusente`] (so' ha o resumo).
    pub fn caminho_do_xml_recebida(
        &self,
        config: &Config,
        chave: &str,
    ) -> Result<PathBuf, ErroTela> {
        let caminho = self
            .notas_nfe(config)?
            .recebidas()
            .and_then(|p| p.caminho_do_proc(chave))
            .map_err(|_| ErroTela::NotaAusente)?;
        if caminho.is_file() {
            Ok(caminho)
        } else {
            Err(ErroTela::XmlAusente)
        }
    }

    /// O erro do pedido em `Erro` da aplicacao, para o servico que nao chega a montar a sessao.
    pub(super) fn recusa_da_sessao(e: &ErroTela) -> rustnf_app::erro::Erro {
        rustnf_app::erro::Erro::Uso(e.texto())
    }

    /// Abre o certificado e reune o que as chamadas ao Ambiente Nacional precisam (o mesmo da NF-e).
    ///
    /// # Errors
    ///
    /// Os de [`Servicos::preparar_nfe`].
    pub fn preparar_recebidas(
        &self,
        config: &Config,
        digitada: Option<&rustnf_app::segredo::Segredo>,
    ) -> Result<Preparo, ErroTela> {
        self.preparar_nfe(config, digitada)
    }
}

fn chaves(notas: Option<&rustnf_app::notas::nfe::NotasNfe>) -> BTreeSet<String> {
    notas
        .and_then(|n| app::listar(n, &FiltroRecebidas::default()).ok())
        .map(|v| v.into_iter().map(|r| r.chave).collect())
        .unwrap_or_default()
}

/// "Buscar notas novas": a distribuicao de DF-e do Ambiente Nacional, do ultimo NSU ate o fim da fila. A biblioteca decide
/// o proximo NSU e QUANDO ESPERAR; antes da hora nenhuma chamada sai.
pub async fn buscar_notas(
    fabrica: Arc<dyn FabricaNfe>,
    preparo: Preparo,
    config: Config,
) -> DesfechoBusca {
    let s = match montar(&fabrica, preparo, config) {
        Ok(s) => s,
        Err(e) => return DesfechoBusca::da_busca(Err(Servicos::recusa_da_sessao(&e)), 0),
    };
    let antes = chaves(s.notas.as_ref());
    let r = app::sincronizar(&s.contexto(), None).await;
    let novas = chaves(s.notas.as_ref()).difference(&antes).count();
    DesfechoBusca::da_busca(r, novas)
}

/// "Baixar a nota completa": consulta a nota pela chave e, com `ciencia`, registra antes a CIENCIA da operacao (um evento
/// fiscal) se a nota ainda nao tem resposta. O Ambiente Nacional so' entrega a nota completa a quem ja respondeu.
pub async fn baixar_nota(
    fabrica: Arc<dyn FabricaNfe>,
    preparo: Preparo,
    config: Config,
    chave: String,
    ciencia: bool,
) -> DesfechoBaixa {
    let s = match montar(&fabrica, preparo, config) {
        Ok(s) => s,
        Err(e) => return DesfechoBaixa::da_baixa(Err(Servicos::recusa_da_sessao(&e))),
    };
    DesfechoBaixa::da_baixa(app::baixar(&s.contexto(), &chave, ciencia).await)
}

/// Responde a nota (a manifestacao do destinatario): confirmacao, ciencia, desconhecimento ou operacao nao realizada. E' um
/// EVENTO FISCAL: a tela so' chama depois da escolha e, em producao, da marcacao do dialogo.
pub async fn responder_nota(
    fabrica: Arc<dyn FabricaNfe>,
    preparo: Preparo,
    config: Config,
    chave: String,
    resposta: Resposta,
    justificativa: Option<String>,
) -> DesfechoNfe {
    let s = match montar(&fabrica, preparo, config) {
        Ok(s) => s,
        Err(e) => return recusado(&e),
    };
    DesfechoNfe::da_manifestacao(
        app::manifestar(
            &s.contexto(),
            &chave,
            resposta.tipo(),
            justificativa.as_deref(),
        )
        .await,
        resposta.tipo(),
    )
}
