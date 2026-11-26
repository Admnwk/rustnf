// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Os documentos RECEBIDOS dentro do servidor: o que as rotas `/nfe/recebidas/` e as ferramentas do MCP compartilham. Cada
//! metodo chama a MESMA funcao de `rustnf_app::nfe` que a linha de comando chama. `sincronizar`, `baixar` e `manifestar`
//! falam com o Ambiente Nacional (e a manifestacao e' um evento fiscal); `listar` e `cursor` so' leem a pasta.

use rustnf::nfe::sefaz::evento::TipoManifestacao;
use rustnf_app::nfe::{self, FiltroRecebidas};
use rustnf_app::notas::nfe::SituacaoRecebida;
use serde::Deserialize;
use serde_json::{json, Value};

use super::nfe::{Saida, ServidorNfe};
use crate::erro::Erro;

/// `sincronizar_recebidas` / `POST /nfe/recebidas/sincronizar`.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PedidoSincronizar {
    /// No maximo quantos lotes (de ate 50 documentos) ler; sem ele, 20.
    #[serde(default)]
    pub lotes: Option<usize>,
}

/// `baixar_recebida` / `POST /nfe/recebidas/baixar`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PedidoBaixar {
    /// Chave de acesso (44 posicoes).
    pub chave: String,
    /// Registra a ciencia da operacao (evento fiscal 210210) antes, se a nota ainda nao tem manifestacao.
    #[serde(default)]
    pub ciencia: bool,
}

/// O tipo da manifestacao, como o pedido o escreve.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TipoPedido {
    /// 210210.
    Ciencia,
    /// 210200.
    Confirmacao,
    /// 210220.
    Desconhecimento,
    /// 210240.
    NaoRealizada,
}

impl From<TipoPedido> for TipoManifestacao {
    fn from(t: TipoPedido) -> Self {
        match t {
            TipoPedido::Ciencia => Self::Ciencia,
            TipoPedido::Confirmacao => Self::Confirmacao,
            TipoPedido::Desconhecimento => Self::Desconhecimento,
            TipoPedido::NaoRealizada => Self::NaoRealizada,
        }
    }
}

/// `manifestar_recebida` / `POST /nfe/recebidas/manifestar`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PedidoManifestar {
    /// Chave de acesso (44 posicoes).
    pub chave: String,
    /// Qual manifestacao.
    pub tipo: TipoPedido,
    /// De 15 a 255 caracteres: obrigatoria na operacao nao realizada, opcional no desconhecimento, proibida nas outras.
    #[serde(default)]
    pub justificativa: Option<String>,
}

/// Os nomes de `situacao` aceitos no filtro da lista.
const SITUACOES: [SituacaoRecebida; 8] = [
    SituacaoRecebida::Resumo,
    SituacaoRecebida::Completa,
    SituacaoRecebida::Ciencia,
    SituacaoRecebida::Confirmada,
    SituacaoRecebida::Desconhecida,
    SituacaoRecebida::NaoRealizada,
    SituacaoRecebida::Cancelada,
    SituacaoRecebida::Denegada,
];

/// `situacao` do filtro: o texto da situacao, ou nada.
///
/// # Errors
///
/// [`Erro::Entrada`] se o texto nao e' uma situacao.
pub fn situacao_do_filtro(texto: Option<&str>) -> Result<Option<SituacaoRecebida>, Erro> {
    let Some(t) = texto.filter(|t| !t.is_empty()) else {
        return Ok(None);
    };
    SITUACOES
        .into_iter()
        .find(|s| s.texto() == t)
        .map(Some)
        .ok_or_else(|| {
            let nomes: Vec<&str> = SITUACOES.iter().map(|s| s.texto()).collect();
            Erro::Entrada(format!("situacao: use {}", nomes.join(", ")))
        })
}

impl ServidorNfe {
    /// Baixa o que ha de novo na distribuicao de DF-e e grava na pasta dos recebidos. Uma sincronizacao por vez.
    ///
    /// # Errors
    ///
    /// Os de [`nfe::sincronizar`].
    pub async fn sincronizar_recebidas(&self, p: &PedidoSincronizar) -> Result<Saida, Erro> {
        let ctx = self.contexto()?;
        let _vez = self.fila.lock().await;
        let s = nfe::sincronizar(&ctx, p.lotes).await?;
        Ok(Saida {
            corpo: s.para_json(),
            erro: s.erro(),
        })
    }

    /// Baixa a NF-e de uma chave (com a ciencia da operacao, se pedida).
    ///
    /// # Errors
    ///
    /// Os de [`nfe::baixar`].
    pub async fn baixar_recebida(&self, p: &PedidoBaixar) -> Result<Saida, Erro> {
        let ctx = self.contexto()?;
        let _vez = self.fila.lock().await;
        let b = nfe::baixar(&ctx, &p.chave, p.ciencia).await?;
        Ok(Saida {
            corpo: b.para_json(),
            erro: b.erro(),
        })
    }

    /// Registra a manifestacao do destinatario (evento fiscal).
    ///
    /// # Errors
    ///
    /// Os de [`nfe::manifestar`].
    pub async fn manifestar_recebida(&self, p: &PedidoManifestar) -> Result<Saida, Erro> {
        let ctx = self.contexto()?;
        let _vez = self.fila.lock().await;
        let m = nfe::manifestar(&ctx, &p.chave, p.tipo.into(), p.justificativa.as_deref()).await?;
        Ok(Saida {
            corpo: m.para_json(),
            erro: m.erro(),
        })
    }

    /// As NF-e recebidas da pasta, sem rede.
    ///
    /// # Errors
    ///
    /// [`Erro::Uso`] sem pasta de notas.
    pub fn listar_recebidas(
        &self,
        situacao: Option<SituacaoRecebida>,
        busca: &str,
    ) -> Result<Value, Erro> {
        let notas = self.notas.as_ref().ok_or_else(|| {
            Erro::Uso(
                "sem pasta de notas: configure [notas].pasta no arquivo ou RUSTNF_NOTAS_PASTA"
                    .into(),
            )
        })?;
        let lista = nfe::listar(
            notas,
            &FiltroRecebidas {
                situacao,
                busca: busca.to_string(),
            },
        )?;
        Ok(json!({"notas": serde_json::to_value(lista)?}))
    }

    /// O cursor da distribuicao, sem rede.
    ///
    /// # Errors
    ///
    /// [`Erro::Uso`] sem pasta de notas ou sem `[emitente].cnpj`.
    pub fn cursor_recebidas(&self) -> Result<Value, Erro> {
        let notas = self.notas.as_ref().ok_or_else(|| {
            Erro::Uso(
                "sem pasta de notas: configure [notas].pasta no arquivo ou RUSTNF_NOTAS_PASTA"
                    .into(),
            )
        })?;
        nfe::cursor(notas, &self.config)
    }
}

/// O valor de um parametro da URL sem a codificacao (`+` e `%XX`): a busca por nome com acento chega assim.
#[must_use]
pub fn sem_codificacao(texto: &str) -> String {
    let b = texto.as_bytes();
    let mut saida: Vec<u8> = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let hex = |x: u8| {
            char::from(x)
                .to_digit(16)
                .and_then(|d| u8::try_from(d).ok())
        };
        match b[i] {
            b'+' => saida.push(b' '),
            b'%' if i + 2 < b.len() && hex(b[i + 1]).is_some() && hex(b[i + 2]).is_some() => {
                saida.push(hex(b[i + 1]).unwrap_or(0) * 16 + hex(b[i + 2]).unwrap_or(0));
                i += 2;
            }
            c => saida.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&saida).into_owned()
}
