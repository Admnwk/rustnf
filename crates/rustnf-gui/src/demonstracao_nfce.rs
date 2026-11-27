// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A SEFAZ de mentira da NFC-e (modelo 65): as respostas roteirizadas do status, da autorizacao (sincrona, como a NFC-e exige)
//! e da consulta, sobre o mesmo transporte da NF-e (`demonstracao_nfe`), e a semeadura da pasta de ensaio `nfce/`: uma NFC-e
//! autorizada, uma cancelada, uma emitida sem internet (dentro do prazo) e uma sem resposta. O cancelamento da NFC-e usa o
//! mesmo servico de eventos da NF-e (`NFeRecepcaoEvento4`), que ja e' roteirizado la.
//!
//! O CSC de ensaio: a demonstracao nao usa o cofre do sistema e o QR Code padrao (versao 3) nao leva CSC; quem quiser ver a
//! tela do CSC a ve com o cofre desligado (`RUSTNF_COFRE=desligado`), que explica por que o CSC nao pode ser guardado. A
//! senha do certificado de ensaio e' a que a janela pede no dialogo, como na NF-e.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use chrono::{FixedOffset, Utc};
use rustnf::assinatura::cert_a1;
use rustnf::nfe::sefaz::{ClienteSefaz, FalhaRede};
use rustnf_app::config::Config;
use rustnf_app::nfce::{self, Modo};
use rustnf_app::nfe::{self as app, Contexto};
use rustnf_app::notas::nfe::Situacao;
use serde_json::{json, Value};

use crate::demonstracao_nfe::{
    entre, ok, prot, Passo, TransporteNfeDemonstracao, AUT, CON, NS, STA,
};
use crate::servicos::TransporteDin;

/// O que a SEFAZ de mentira responde ao envio da NFC-e (a venda pelo caixa).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RoteiroNfce {
    /// Autoriza (o padrao).
    #[default]
    Autorizada,
    /// A primeira tentativa e' recusada (NCM inexistente, 778); a seguinte autoriza.
    Rejeitada,
    /// A primeira tentativa fica sem resposta e a consulta da mesma chave tambem; a retomada acha a nota autorizada.
    SemResposta,
    /// O servico de status nao responde (a primeira vez): a venda sai em contingencia off-line. Depois a SEFAZ volta, e
    /// "Fechar o caixa" transmite a MESMA nota.
    Fora,
}

/// O roteiro de `--nfce-emissao`.
#[must_use]
pub fn roteiro_nfce_do_nome(nome: &str) -> Option<RoteiroNfce> {
    match nome {
        "autorizada" => Some(RoteiroNfce::Autorizada),
        "rejeitada" => Some(RoteiroNfce::Rejeitada),
        "sem-resposta" => Some(RoteiroNfce::SemResposta),
        "fora" => Some(RoteiroNfce::Fora),
        _ => None,
    }
}

/// O que o transporte lembra da NFC-e.
#[derive(Debug, Default)]
pub(crate) struct EstadoNfce {
    pub(crate) roteiro: RoteiroNfce,
    autorizacoes: u32,
    status: u32,
    /// As chaves que a SEFAZ de mentira "recebeu" (autorizou).
    recebidas: BTreeSet<String>,
    /// A chave enviada sem resposta: a consulta falha na primeira vez e depois a acha autorizada.
    perdida: Option<String>,
    consulta_perdida: bool,
    /// Chaves de ensaio sem resposta (a consulta falha na primeira vez, depois acha).
    pub(crate) pendentes: BTreeSet<String>,
    primeira_consulta: BTreeSet<String>,
}

fn retorno(protocolo: &str) -> String {
    format!(
        "<retEnviNFe versao=\"4.00\" xmlns=\"{NS}\"><tpAmb>2</tpAmb><verAplic>DEMO_NFCE_1</verAplic>         <cStat>104</cStat><xMotivo>Lote processado</xMotivo><cUF>35</cUF>         <dhRecbto>2026-10-03T09:00:05-03:00</dhRecbto>{protocolo}</retEnviNFe>"
    )
}

impl EstadoNfce {
    /// A resposta ao status, a autorizacao ou a consulta da NFC-e.
    pub(crate) fn responder(
        &mut self,
        servico: &str,
        corpo: &str,
        passo: Passo,
        nprot: &str,
    ) -> Result<(u16, Vec<u8>), FalhaRede> {
        let semeando = passo != Passo::AoVivo;
        match servico {
            STA => {
                self.status += 1;
                let parado = !semeando && self.roteiro == RoteiroNfce::Fora && self.status == 1;
                if parado {
                    return Err(FalhaRede::Tempo);
                }
                let (cstat, motivo) = ("107", "Servico em Operacao");
                ok(
                    STA,
                    &format!(
                        "<retConsStatServ versao=\"4.00\" xmlns=\"{NS}\"><tpAmb>2</tpAmb><verAplic>DEMO_NFCE_1</verAplic>\
                         <cStat>{cstat}</cStat><xMotivo>{motivo}</xMotivo><cUF>35</cUF>\
                         <dhRecbto>2026-10-03T09:00:30-03:00</dhRecbto></retConsStatServ>"
                    ),
                )
            }
            AUT => {
                if passo == Passo::PerderTudo {
                    return Err(FalhaRede::Tempo);
                }
                let chave = entre(corpo, "Id=\"NFe", "\"");
                if !semeando {
                    self.autorizacoes += 1;
                    match self.roteiro {
                        RoteiroNfce::SemResposta if self.autorizacoes == 1 => {
                            self.perdida = Some(chave.to_string());
                            return Err(FalhaRede::Tempo);
                        }
                        RoteiroNfce::Rejeitada if self.autorizacoes == 1 => {
                            return ok(
                                AUT,
                                &retorno(&prot(
                                    chave,
                                    "778",
                                    "Rejeicao: Informado NCM inexistente",
                                    "",
                                )),
                            );
                        }
                        _ => {}
                    }
                }
                self.recebidas.insert(chave.to_string());
                ok(
                    AUT,
                    &retorno(&prot(chave, "100", "Autorizado o uso da NFC-e", nprot)),
                )
            }
            _ => {
                if passo == Passo::PerderTudo {
                    return Err(FalhaRede::Tempo);
                }
                let chave = entre(corpo, "<chNFe>", "</chNFe>");
                let perdida_agora =
                    self.perdida.as_deref() == Some(chave) && !self.consulta_perdida;
                if !semeando && perdida_agora {
                    self.consulta_perdida = true;
                    return Err(FalhaRede::Tempo);
                }
                if self.pendentes.contains(chave)
                    && self.primeira_consulta.insert(chave.to_string())
                {
                    return Err(FalhaRede::Tempo);
                }
                let conhecida = self.recebidas.contains(chave)
                    || self.perdida.as_deref() == Some(chave)
                    || self.pendentes.contains(chave);
                if !conhecida && !semeando {
                    // a nota emitida sem internet: a SEFAZ nunca a viu (217), a retomada reenvia a MESMA nota assinada
                    return ok(
                        CON,
                        &format!(
                            "<retConsSitNFe versao=\"4.00\" xmlns=\"{NS}\"><tpAmb>2</tpAmb><verAplic>DEMO_NFCE_1</verAplic>\
                             <cStat>217</cStat><xMotivo>Rejeicao: NF-e nao consta na base de dados da SEFAZ</xMotivo><cUF>35</cUF>\
                             <dhRecbto>2026-10-03T09:00:20-03:00</dhRecbto><chNFe>{chave}</chNFe></retConsSitNFe>"
                        ),
                    );
                }
                ok(
                    CON,
                    &format!(
                        "<retConsSitNFe versao=\"4.00\" xmlns=\"{NS}\"><tpAmb>2</tpAmb><verAplic>DEMO_NFCE_1</verAplic>\
                         <cStat>100</cStat><xMotivo>Autorizado o uso da NFC-e</xMotivo><cUF>35</cUF>\
                         <dhRecbto>2026-10-03T09:00:20-03:00</dhRecbto><chNFe>{chave}</chNFe>{}</retConsSitNFe>",
                        prot(chave, "100", "Autorizado o uso da NFC-e", nprot)
                    ),
                )
            }
        }
    }
}

/// As chaves das quatro NFC-e do ensaio.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NotasDoEnsaioNfce {
    /// A NFC-e autorizada.
    pub autorizada: String,
    /// A NFC-e cancelada.
    pub cancelada: String,
    /// A NFC-e emitida sem internet, dentro do prazo para transmitir.
    pub offline: String,
    /// A NFC-e enviada sem resposta.
    pub pendente: String,
}

fn pedido(item: (&str, &str, u32, u32), pagamento: &str) -> Value {
    let agora = Utc::now() - chrono::Duration::minutes(3);
    let quando = match FixedOffset::west_opt(3 * 3600) {
        Some(brasilia) => agora
            .with_timezone(&brasilia)
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, false),
        None => agora.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
    };
    let (codigo, descricao, qtd, centavos) = item;
    let total = u64::from(qtd) * u64::from(centavos);
    let dinheiro = |c: u64| format!("{}.{:02}", c / 100, c % 100);
    json!({
        "modelo": 65,
        "emissao": quando,
        "nat_op": "VENDA",
        "itens": [{"codigo": codigo, "descricao": descricao, "ncm": "09012100", "cfop": "5102",
                   "unidade": "UN", "quantidade": qtd.to_string(), "valor_unitario": dinheiro(u64::from(centavos)),
                   "icms": {"codigo": "102"}, "pis": {"cst": "49"}, "cofins": {"cst": "49"}}],
        "pagamentos": [{"t_pag": pagamento, "valor": dinheiro(total)}]
    })
}

/// Semeia a pasta `nfce/` de ensaio com uma NFC-e autorizada, uma cancelada, uma emitida sem internet e uma sem resposta, pela
/// mesma camada de aplicacao que a janela usa (a SEFAZ e' a de mentira e o certificado, um de teste gerado na hora). Pasta que
/// ja tem NFC-e fica como esta.
///
/// # Errors
///
/// A frase do que falhou.
pub fn semear_nfce(
    config: &Config,
    transporte: &TransporteNfeDemonstracao,
) -> Result<NotasDoEnsaioNfce, String> {
    let cfg = config.como_nfce();
    crate::demonstracao_nfe::semear_produtos(config);
    let notas =
        nfce::abrir_pasta(cfg.notas.pasta.trim()).map_err(|_| "a pasta de notas nao abre")?;
    let existentes = notas.listar().map_err(|_| "a pasta de notas nao le")?;
    let achar = |s: Situacao, offline: bool| {
        existentes
            .iter()
            .find(|r| r.situacao == s && r.offline == offline)
            .map(|r| r.chave.clone())
            .unwrap_or_default()
    };
    if !existentes.is_empty() {
        let n = NotasDoEnsaioNfce {
            autorizada: achar(Situacao::Autorizada, false),
            cancelada: achar(Situacao::Cancelada, false),
            offline: achar(Situacao::Pendente, true),
            pendente: achar(Situacao::Pendente, false),
        };
        if !n.pendente.is_empty() {
            transporte.marcar_pendente_nfce(&n.pendente);
        }
        return Ok(n);
    }
    let pfx = cert_a1::gerar_teste(
        &cfg.emitente.cnpj,
        "EMPRESA DE ENSAIO",
        "ensaio",
        30,
        true,
        false,
        Utc::now(),
    )
    .map_err(|e| e.to_string())?;
    let certificado = cert_a1::carregar(&pfx, "ensaio").map_err(|e| e.to_string())?;
    let sefaz = app::config_sefaz(&cfg).map_err(|e| e.to_string())?;
    let cliente =
        ClienteSefaz::com_transporte(sefaz, TransporteDin::de(Arc::new(transporte.clone())))
            .map_err(|e| e.to_string())?
            .com_dormir(|_: Duration| Box::pin(async {}));
    let ctx = Contexto::novo(&cliente, &certificado, &cfg, Some(&notas));
    let ctx = &ctx;
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    let emitir = |p: Value, modo: Modo| async move {
        nfce::emitir(ctx, p, false, modo)
            .await
            .map_err(|e| e.to_string())
    };
    let semeou = rt.block_on(async {
        transporte.passo_do_ensaio(Passo::Autorizar);
        let a = emitir(
            pedido(("C1", "Cafe torrado 500g", 2, 1590), "01"),
            Modo::Normal,
        )
        .await?;
        let c = emitir(
            pedido(("F103", "Filtro de papel 103", 1, 899), "17"),
            Modo::Normal,
        )
        .await?;
        app::cancelar(
            ctx,
            c.chave(),
            "Cancelamento de ensaio da demonstracao",
            None,
        )
        .await
        .map_err(|e| e.to_string())?;
        let off = emitir(
            pedido(("C2", "Cafe moido 250g", 1, 1200), "01"),
            Modo::Offline,
        )
        .await?;
        transporte.passo_do_ensaio(Passo::PerderTudo);
        let pend = emitir(
            pedido(("C1", "Cafe torrado 500g", 1, 1590), "03"),
            Modo::Normal,
        )
        .await?;
        Ok::<_, String>(NotasDoEnsaioNfce {
            autorizada: a.chave().to_string(),
            cancelada: c.chave().to_string(),
            offline: off.chave().to_string(),
            pendente: pend.chave().to_string(),
        })
    });
    transporte.passo_do_ensaio(Passo::AoVivo);
    let n = semeou?;
    transporte.marcar_pendente_nfce(&n.pendente);
    // o que se contou semeando nao e' do uso da janela
    transporte.zerar_contagens();
    Ok(n)
}
