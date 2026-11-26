// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A manifestacao do destinatario: os quatro eventos que quem recebe a NF-e registra no Ambiente Nacional.
//!
//! * **Fontes.** MOC 7.00, Visao Geral, 3.1.2 e 3.2 (os eventos e como mudar de ideia), 5.11 (leiaute, tabela 5-41, e as
//!   regras H01 a H07 da tabela 5-42) e 5.8.1 (parte geral, `cOrgao` 91 = Ambiente Nacional); esquemas `e210200`,
//!   `e210210`, `e210220`, `e210240` e `leiauteConfRecebto` do pacote "Evento Manifesta Destinatario" v1.01.
//! * **Quem manifesta.** O destinatario da NF-e: o `envEvento` leva o CNPJ ou CPF DELE e e' assinado com o certificado
//!   do mesmo CNPJ-base (ou CPF) (MOC 5.11).
//! * **Os quatro eventos.** 210200 confirmacao da operacao, 210210 ciencia da operacao, 210220 desconhecimento da
//!   operacao, 210240 operacao nao realizada. `nSeqEvento` e' sempre 1 (H02, rejeicao 594): a mudanca de ideia e' OUTRO
//!   tipo de evento, e vale o ultimo (MOC 3.2.1.5).
//! * **Justificativa (`xJust`, 15 a 255 caracteres).** Obrigatoria na operacao nao realizada (H01, rejeicao 595) e
//!   proibida na confirmacao e na ciencia (os esquemas `e210200` e `e210210` nao a tem). No desconhecimento o MOC 5.11.1
//!   (HP20) diz que so' a operacao nao realizada a leva, mas o esquema `e210220`, que e' o que o Ambiente Nacional
//!   aplica, a aceita como OPCIONAL: segue-se o esquema.
//! * **O que se confere aqui.** So' o que depende do que o chamador sabe: ciencia ou desconhecimento de NF-e cancelada ou
//!   denegada (650 e 651), ciencia depois de manifestacao conclusiva (655) e o mesmo evento duas vezes (573). O prazo da
//!   manifestacao (596, "prazo vigente") e' o da legislacao e NAO e' conferido: o Ambiente Nacional decide e o resultado
//!   diz a rejeicao em palavras.

use chrono::{DateTime, FixedOffset};

use super::super::dfe::pedido::interessado;
use super::super::dfe::SituacaoResumo;
use super::super::erro::ErroSefaz;
use super::super::pedidos::chave_valida;
use super::super::tabela::uf_valida;
use super::comum::{montar, texto_livre, Cabecalho, Montado};
use super::epec::ORGAO_AMBIENTE_NACIONAL;
use crate::nfe::vigencias::Ambiente;

/// `tpEvento` da confirmacao da operacao.
pub const CONFIRMACAO: &str = "210200";
/// `tpEvento` da ciencia da operacao.
pub const CIENCIA: &str = "210210";
/// `tpEvento` do desconhecimento da operacao.
pub const DESCONHECIMENTO: &str = "210220";
/// `tpEvento` da operacao nao realizada.
pub const NAO_REALIZADA: &str = "210240";

/// Menor e maior `xJust` (esquemas `e210220` e `e210240`).
const JUST_MIN: usize = 15;
const JUST_MAX: usize = 255;

/// Os eventos da manifestacao do destinatario.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TipoManifestacao {
    /// 210200: confirma que a operacao ocorreu como a NF-e diz.
    Confirmacao,
    /// 210210: tem ciencia da NF-e, mas ainda nao tem como dar a manifestacao conclusiva (opcional).
    Ciencia,
    /// 210220: a operacao nao foi solicitada por ele.
    Desconhecimento,
    /// 210240: participou da operacao, mas ela nao ocorreu como a NF-e diz (exige justificativa).
    NaoRealizada,
}

impl TipoManifestacao {
    /// O `tpEvento`.
    #[must_use]
    pub fn codigo(self) -> &'static str {
        match self {
            Self::Confirmacao => CONFIRMACAO,
            Self::Ciencia => CIENCIA,
            Self::Desconhecimento => DESCONHECIMENTO,
            Self::NaoRealizada => NAO_REALIZADA,
        }
    }

    /// O `descEvento`, como o esquema o fixa (sem acento).
    #[must_use]
    pub fn descricao(self) -> &'static str {
        match self {
            Self::Confirmacao => "Confirmacao da Operacao",
            Self::Ciencia => "Ciencia da Operacao",
            Self::Desconhecimento => "Desconhecimento da Operacao",
            Self::NaoRealizada => "Operacao nao Realizada",
        }
    }

    /// O tipo de um `tpEvento`, se for um dos quatro.
    #[must_use]
    pub fn de_codigo(tp_evento: &str) -> Option<Self> {
        [
            Self::Confirmacao,
            Self::Ciencia,
            Self::Desconhecimento,
            Self::NaoRealizada,
        ]
        .into_iter()
        .find(|t| t.codigo() == tp_evento)
    }

    /// E' manifestacao conclusiva (todas menos a ciencia).
    #[must_use]
    pub fn conclusiva(self) -> bool {
        self != Self::Ciencia
    }
}

/// Os dados de uma manifestacao.
#[derive(Debug, Clone)]
pub struct Manifestacao<'a> {
    /// CNPJ ou CPF do DESTINATARIO (o autor do evento).
    pub documento: &'a str,
    /// Chave da NF-e (modelo 55).
    pub chave: &'a str,
    /// Qual manifestacao.
    pub tipo: TipoManifestacao,
    /// `xJust`, de 15 a 255 caracteres: obrigatoria na operacao nao realizada, opcional no desconhecimento, proibida nas
    /// outras duas.
    pub justificativa: Option<&'a str>,
    /// A situacao da NF-e no `resNFe` que o chamador recebeu (`cSitNFe`), se sabe.
    pub situacao_nfe: Option<SituacaoResumo>,
    /// A ultima manifestacao que o chamador tem registrada para a NF-e, se houver.
    pub anterior: Option<TipoManifestacao>,
    /// UF (sigla ou codigo) do autor: o `dhEvento` sai no fuso dela.
    pub uf_autor: &'a str,
    /// Ambiente.
    pub ambiente: Ambiente,
    /// Data e hora do evento.
    pub quando: DateTime<FixedOffset>,
}

/// As combinacoes que o MOC recusa e que o chamador ja sabe antes de enviar (650, 651, 655 e 573).
///
/// # Errors
///
/// [`ErroSefaz`] (campo `tpEvento`) com a rejeicao que o Ambiente Nacional devolveria.
pub fn conferir_combinacao(
    tipo: TipoManifestacao,
    situacao_nfe: Option<&SituacaoResumo>,
    anterior: Option<TipoManifestacao>,
) -> Result<(), ErroSefaz> {
    let encerrada = matches!(
        situacao_nfe,
        Some(SituacaoResumo::Cancelada | SituacaoResumo::Denegada)
    );
    let recusa = match tipo {
        TipoManifestacao::Ciencia if encerrada => {
            Some("650: ciencia da emissao para NF-e cancelada ou denegada")
        }
        TipoManifestacao::Desconhecimento if encerrada => {
            Some("651: desconhecimento da operacao para NF-e cancelada ou denegada")
        }
        TipoManifestacao::Ciencia if anterior.is_some_and(TipoManifestacao::conclusiva) => {
            Some("655: ciencia da emissao depois da manifestacao conclusiva do destinatario")
        }
        t if anterior == Some(t) => {
            Some("573: esta manifestacao ja esta registrada (duplicidade de evento)")
        }
        _ => None,
    };
    match recusa {
        Some(m) => Err(ErroSefaz::novo(
            format!("o Ambiente Nacional recusaria ({m}); o pedido nao foi enviado"),
            "tpEvento",
        )),
        None => Ok(()),
    }
}

fn justificativa_de(m: &Manifestacao<'_>) -> Result<String, ErroSefaz> {
    let dada = m.justificativa.filter(|j| !j.trim().is_empty());
    match (m.tipo, dada) {
        (TipoManifestacao::NaoRealizada, None) => Err(ErroSefaz::novo(
            "xJust: obrigatoria na operacao nao realizada, de 15 a 255 caracteres (rejeicao 595)",
            "xJust",
        )),
        (TipoManifestacao::NaoRealizada | TipoManifestacao::Desconhecimento, Some(j)) => {
            let t = texto_livre(j, "xJust", JUST_MIN, JUST_MAX)?;
            Ok(format!("<xJust>{t}</xJust>"))
        }
        (_, None) => Ok(String::new()),
        (t, Some(_)) => Err(ErroSefaz::novo(
            format!(
                "xJust: o evento {} ({}) nao leva justificativa",
                t.codigo(),
                t.descricao()
            ),
            "xJust",
        )),
    }
}

/// Monta o evento da manifestacao (sem assinatura): `cOrgao` 91, autor o destinatario, `nSeqEvento` 1.
///
/// # Errors
///
/// [`ErroSefaz`] se a chave nao e' de uma NF-e (modelo 55) valida, o CNPJ ou CPF nao confere, a UF e' desconhecida, a
/// justificativa nao serve para o tipo ou a combinacao com o que o chamador sabe e' recusada ([`conferir_combinacao`]).
pub fn manifestacao(m: &Manifestacao<'_>) -> Result<Montado, ErroSefaz> {
    let c = chave_valida(m.chave)?;
    let (tag, doc) = interessado(m.documento).map_err(|e| {
        ErroSefaz::novo(
            format!("destinatario: {}", e.mensagem),
            if e.campo == "CPF" { "CPF" } else { "CNPJ" },
        )
    })?;
    let uf = uf_valida(m.uf_autor)?;
    conferir_combinacao(m.tipo, m.situacao_nfe.as_ref(), m.anterior)?;
    let just = justificativa_de(m)?;
    let det = format!("<descEvento>{}</descEvento>{just}", m.tipo.descricao());
    Ok(montar(
        &Cabecalho {
            c_orgao: ORGAO_AMBIENTE_NACIONAL,
            ambiente: m.ambiente,
            autor_tag: tag,
            autor: &doc,
            chave: &c.chave,
            tp_evento: m.tipo.codigo(),
            seq: 1,
            uf_fuso: uf,
            quando: &m.quando,
        },
        &det,
    ))
}
