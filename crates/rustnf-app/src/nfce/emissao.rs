// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A emissao da NFC-e: o mesmo caminho da NF-e (pedido -> completa -> confere -> monta e assina -> GRAVA como pendente ->
//! envia -> grava o resultado), com o modelo 65, o QR Code da configuracao, o envio sincrono e a contingencia off-line.
//!
//! Off-line (`tpEmis` 9): a nota e' assinada e gravada como pendente COM o prazo de transmissao e NAO vai a rede; o DANFC-e
//! sai com as mensagens do Manual e a retomada (`nfe::retomar`) a transmite com a MESMA chave. A decisao automatica so' existe
//! antes de enviar (o servico de status da NFC-e nao responde): depois de enviar sem resposta a nota PODE ter sido
//! autorizada, e uma segunda NFC-e para o mesmo cupom seria nota gemea, entao ela fica pendente e se retoma.

use rustnf::nfe::montagem;
use rustnf::nfe::sefaz::{SituacaoServico, Transporte};
use serde_json::{json, Map, Value};

use crate::config::ausente;
use crate::erro::Erro;
use crate::nfe::{emitir as emitir_nfe, preparar_e_gravar, Contexto, Emitida};

use super::pendentes::limite_por_extenso;

/// Como emitir.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modo {
    /// Envia a SEFAZ.
    Normal,
    /// Contingencia off-line: assina, grava pendente com o prazo e NAO envia.
    Offline,
    /// Pergunta ao servico de status da NFC-e antes: se ele nao responde (ou esta paralisado), emite off-line; senao envia.
    OfflineSeNecessario,
}

/// O que a emissao da NFC-e fez.
#[derive(Debug, Clone)]
pub enum Emissao {
    /// Enviada a SEFAZ (autorizada, rejeitada, denegada ou sem resposta).
    Enviada(Box<Emitida>),
    /// Emitida em contingencia off-line: gravada, por transmitir.
    Offline(Box<Gravada>),
}

/// A NFC-e off-line gravada na pasta.
#[derive(Debug, Clone)]
pub struct Gravada {
    /// Chave de acesso.
    pub chave: String,
    /// Serie.
    pub serie: String,
    /// Numero.
    pub numero: u64,
    /// Fim do prazo para transmitir (o primeiro dia util depois da emissao), como `dd/mm/aaaa hh:mm`.
    pub prazo: String,
    /// Por que saiu off-line: pedido da pessoa ou SEFAZ sem resposta antes de enviar.
    pub motivo: &'static str,
    /// Os avisos da montagem.
    pub avisos: Vec<montagem::Aviso>,
}

impl Gravada {
    /// A frase para a pessoa.
    #[must_use]
    pub fn frase(&self) -> String {
        format!(
            "NFC-e {} (serie {}) emitida em CONTINGENCIA OFF-LINE e guardada na pasta de notas: ainda NAO foi enviada a \
             SEFAZ. Entregue o DANFC-e ao consumidor e transmita a nota ate {} com `rustnf nfce pendentes --retomar`.",
            self.numero, self.serie, self.prazo
        )
    }
}

impl Emissao {
    /// A chave da nota.
    #[must_use]
    pub fn chave(&self) -> &str {
        match self {
            Self::Enviada(e) => &e.chave,
            Self::Offline(g) => &g.chave,
        }
    }

    /// O JSON: o da NF-e (`Emitida::para_json`) mais `offline`; na off-line, `chave`, `serie`, `numero`, `situacao`
    /// (`pendente`), `prazo`, `motivo`, `guardada`, `avisos` e `frase`.
    #[must_use]
    pub fn para_json(&self) -> Value {
        match self {
            Self::Enviada(e) => {
                let mut v = e.para_json();
                v["offline"] = json!(false);
                // a frase da NF-e fala em "NF-e": aqui a nota e' a NFC-e
                if let Some(f) = v["frase"].as_str() {
                    v["frase"] = json!(f.replace("A NF-e ficou", "A NFC-e ficou"));
                }
                v
            }
            Self::Offline(g) => json!({
                "status": "offline_pendente",
                "frase": g.frase(),
                "chave": g.chave,
                "serie": g.serie,
                "numero": g.numero,
                "situacao": "pendente",
                "guardada": true,
                "offline": true,
                "prazo": g.prazo,
                "motivo": g.motivo,
                "avisos": crate::nfe::avisos_json(&g.avisos),
            }),
        }
    }

    /// O erro do resultado (`None` = autorizada ou off-line gravada).
    #[must_use]
    pub fn erro(&self) -> Option<Erro> {
        match self {
            Self::Enviada(e) => e.erro(),
            Self::Offline(_) => None,
        }
    }
}

/// Poe o `modelo` 65 no pedido (recusa outro modelo).
fn fixar_modelo(pedido: &mut Value) -> Result<&mut Map<String, Value>, Erro> {
    let Some(raiz) = pedido.as_object_mut() else {
        return Err(Erro::Entrada(
            "o pedido da NFC-e tem de ser um objeto JSON".into(),
        ));
    };
    match raiz.get("modelo") {
        None => {
            raiz.insert("modelo".into(), json!(65));
        }
        Some(m) if m == &json!(65) || m == &json!("65") => {}
        Some(_) => {
            return Err(Erro::Uso(
                "o pedido da `rustnf nfce` e' do modelo 65: tire o campo modelo ou use `rustnf nfe` para a NF-e (55)".into(),
            ))
        }
    }
    Ok(raiz)
}

/// Poe no pedido a contingencia off-line: `tpEmis` 9, `dhCont` (a hora da emissao) e `xJust`.
fn como_offline<T: Transporte>(
    ctx: &Contexto<'_, T>,
    raiz: &mut Map<String, Value>,
) -> Result<(), Erro> {
    if !ausente(raiz, "tp_emis") && raiz.get("tp_emis") != Some(&json!(9)) {
        return Err(Erro::Uso(
            "a emissao off-line e' o tpEmis 9: tire o tp_emis do pedido".into(),
        ));
    }
    raiz.insert("tp_emis".into(), json!(9));
    if ausente(raiz, "emissao") {
        raiz.insert(
            "emissao".into(),
            json!(crate::nfe::emissao_agora(ctx.config, ctx.agora)),
        );
    }
    if let Some(e) = raiz.get("emissao").cloned() {
        crate::config::completar(raiz, "dh_cont", e);
    }
    crate::config::completar(raiz, "x_just", json!(ctx.config.nfce.motivo_offline()));
    Ok(())
}

/// Emite a NFC-e do pedido. O contexto tem de ser o da NFC-e: a configuracao de [`crate::config::Config::como_nfce`], a pasta
/// `nfce/` e as opcoes do QR Code de [`super::opcoes_qr`] (com a versao 2 sem CSC a recusa vem ANTES, la).
///
/// # Errors
///
/// Os de [`crate::nfe::emitir`]; [`Erro::Uso`] se o pedido nao e' do modelo 65. O que a SEFAZ responde volta em [`Emissao`].
pub async fn emitir<T: Transporte>(
    ctx: &Contexto<'_, T>,
    mut pedido: Value,
    estrito: bool,
    modo: Modo,
) -> Result<Emissao, Erro> {
    let raiz = fixar_modelo(&mut pedido)?;
    let motivo = match modo {
        Modo::Normal => None,
        Modo::Offline => Some("pedido do operador"),
        Modo::OfflineSeNecessario => {
            let s = ctx.cliente.status_nfce().await?;
            matches!(
                s.situacao,
                SituacaoServico::SemResposta | SituacaoServico::Paralisado
            )
            .then_some("a SEFAZ nao respondeu ao status do servico antes do envio")
        }
    };
    let Some(motivo) = motivo else {
        return Ok(Emissao::Enviada(Box::new(
            emitir_nfe(ctx, pedido, estrito).await?,
        )));
    };
    como_offline(ctx, raiz)?;
    let (p, _assinada, registro) = preparar_e_gravar(ctx, pedido, estrito, false)?;
    let fim = registro.prazo.ok_or(Erro::Interno)?;
    Ok(Emissao::Offline(Box::new(Gravada {
        prazo: limite_por_extenso(&registro, fim),
        chave: registro.chave,
        serie: p.serie,
        numero: p.numero,
        motivo,
        avisos: montagem::avisos(&p.nota),
    })))
}
