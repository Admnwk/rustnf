// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O fluxo do envio: ler o retorno do lote, esperar o recibo, e a recuperacao da nota cuja resposta se perdeu.
//!
//! A regra: resposta perdida NAO quer dizer nota nao enviada. Antes de qualquer reenvio, `consultar` a chave.
//! * achou autorizada, denegada ou cancelada: devolve a nota (`recuperada`);
//! * a SEFAZ diz que nao consta (217): reenviar a MESMA NF-e e' seguro (a chave nao muda);
//! * sem resposta nem da consulta: `SemResposta`, que manda consultar de novo e nunca emitir outra nota;
//! * duplicidade (204, a mesma chave ja esta na base): a consulta devolve a nota ja autorizada;
//! * duplicidade com outra chave (539): o numero ja foi usado por OUTRA nota, que vale no lugar desta (MOC 7.00,
//!   Anexo I, regra 2B08-10); o cliente confere a outra pela consulta e diz qual e'.

use std::time::Duration;

use super::cliente::{ClienteSefaz, Resp, ESPERA_MAXIMA_RECIBO};
use super::erro::ErroSefaz;
use super::pedidos;
use super::resposta::{self, ProtNfe, Retorno};
use super::resultado::{Desfecho, Resultado};
use super::tabela::Servico;
use super::transporte::Transporte;
use crate::chave_dfe;
use crate::nfe::cstat;

/// O que as etapas do envio precisam saber da nota.
pub(super) struct Ctx<'a> {
    pub(super) chave: &'a str,
    pub(super) nfe: &'a str,
    pub(super) pedido: &'a str,
}

/// O codigo do `Desfecho` que tem um (para a conferencia do 539).
fn cstat_do(d: &Desfecho) -> Option<&str> {
    match d {
        Desfecho::Autorizada { cstat, .. }
        | Desfecho::Denegada { cstat, .. }
        | Desfecho::Cancelada { cstat, .. }
        | Desfecho::Rejeitada { cstat, .. }
        | Desfecho::ConsultaRecusada { cstat, .. }
        | Desfecho::Indisponivel { cstat, .. } => Some(cstat),
        _ => None,
    }
}

/// A chave que a rejeicao 539 traz na mensagem (`[chNFe: <44 digitos>]`).
fn chave_do_motivo(motivo: &str) -> Option<String> {
    let i = motivo.find("chNFe:")?;
    let d: String = motivo[i + "chNFe:".len()..]
        .trim_start()
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    (d.len() == 44).then_some(d)
}

fn fechar(mut r: Resultado, ctx: &Ctx<'_>, retorno: &str, recibo: Option<&str>) -> Resultado {
    r.xml_enviado = ctx.pedido.to_string();
    r.xml_retorno = retorno.to_string();
    r.recibo = recibo.map(str::to_string);
    r
}

impl<T: Transporte> ClienteSefaz<T> {
    pub(super) async fn enviar(
        &self,
        nfe_assinada: &str,
        sincrono: bool,
    ) -> Result<Resultado, ErroSefaz> {
        let nfe = self.conferir(nfe_assinada)?;
        let chave = nfe.chave.chave.clone();
        let pedido = pedidos::envi_nfe(&[nfe.texto], &self.id_lote(), sincrono)?;
        let ctx = Ctx {
            chave: &chave,
            nfe: nfe.texto,
            pedido: &pedido,
        };
        let destino = self.destino_da_chave(&chave, self.uf);
        match self.chamar(Servico::Autorizacao, &pedido, &destino).await? {
            Resp::Sem(s) => {
                let porque = format!("o envio ficou sem resposta ({})", s.detalhe);
                self.recuperar(&ctx, &porque, false).await
            }
            Resp::Ok(r) => self.ler_envio(&r, &ctx).await,
        }
    }

    async fn ler_envio(&self, r: &Retorno, ctx: &Ctx<'_>) -> Result<Resultado, ErroSefaz> {
        let ret = match resposta::ler_envio(&r.texto) {
            Ok(x) => x,
            Err(e) => {
                let porque = format!("retorno do envio ilegivel ({e})");
                return self.recuperar(ctx, &porque, false).await;
            }
        };
        let (cs, motivo) = (ret.cab.cstat.clone(), ret.cab.motivo.clone());
        match cs.as_str() {
            // lote recebido: ha recibo para consultar (assincrono, ou a SEFAZ que nao implementa o sincrono)
            "103" => {
                let Some(rec) = ret.recibo.filter(|x| !x.numero.is_empty()) else {
                    return self.recuperar(ctx, "lote recebido sem recibo", false).await;
                };
                let espera = rec
                    .tempo_medio
                    .map(|s| Duration::from_secs(u64::from(s)).min(ESPERA_MAXIMA_RECIBO));
                self.aguardar_recibo(&rec.numero, ctx.chave, ctx.nfe, ctx.pedido, espera)
                    .await
            }
            "104" => self.no_lote(&ret.protocolos, ctx, &r.texto, None).await,
            "108" | "109" => Ok(fechar(
                Resultado::novo(Desfecho::Indisponivel { cstat: cs, motivo }, ctx.chave),
                ctx,
                &r.texto,
                None,
            )),
            "" => self.recuperar(ctx, "retorno sem cStat", false).await,
            _ => Ok(fechar(
                Resultado::novo(Desfecho::Rejeitada { cstat: cs, motivo }, ctx.chave),
                ctx,
                &r.texto,
                None,
            )),
        }
    }

    /// Espera `espera` (se houver) e consulta o recibo ate `tentativas` vezes.
    pub(super) async fn aguardar_recibo(
        &self,
        recibo: &str,
        chave: &str,
        nfe: &str,
        pedido: &str,
        espera: Option<Duration>,
    ) -> Result<Resultado, ErroSefaz> {
        let ctx = Ctx { chave, nfe, pedido };
        if let Some(e) = espera.filter(|e| !e.is_zero()) {
            (self.dormir)(e).await;
        }
        let consulta = pedidos::cons_reci(self.ambiente, recibo)?;
        let destino = self.destino_da_chave(chave, self.uf);
        let mut ultimo = String::new();
        for i in 0..self.tentativas {
            if i > 0 {
                (self.dormir)(self.intervalo).await;
            }
            let r = match self
                .chamar(Servico::RetAutorizacao, &consulta, &destino)
                .await?
            {
                Resp::Ok(r) => r,
                Resp::Sem(s) => {
                    ultimo = s.detalhe;
                    continue;
                }
            };
            let rr = match resposta::ler_recibo(&r.texto) {
                Ok(x) => x,
                Err(e) => {
                    ultimo = e.to_string();
                    continue;
                }
            };
            match rr.cab.cstat.as_str() {
                "104" => {
                    return self
                        .no_lote(&rr.protocolos, &ctx, &r.texto, Some(recibo))
                        .await
                }
                // 105 em processamento; 108/109 servico parado: o recibo continua valendo, tenta de novo
                "105" | "108" | "109" => {
                    ultimo = format!("{} {}", rr.cab.cstat, rr.cab.motivo);
                }
                "106" => {
                    let porque = format!("recibo nao localizado ({})", rr.cab.motivo);
                    return self.recuperar(&ctx, &porque, false).await;
                }
                _ => {
                    let d = Desfecho::Rejeitada {
                        cstat: rr.cab.cstat,
                        motivo: rr.cab.motivo,
                    };
                    return Ok(fechar(
                        Resultado::novo(d, chave),
                        &ctx,
                        &r.texto,
                        Some(recibo),
                    ));
                }
            }
        }
        let mut r = fechar(
            Resultado::novo(
                Desfecho::EmProcessamento {
                    recibo: recibo.to_string(),
                },
                chave,
            ),
            &ctx,
            "",
            Some(recibo),
        );
        r.detalhe = ultimo;
        Ok(r)
    }

    /// Acha o protocolo DESTA nota no lote e o interpreta. Protocolo de outra chave nao serve: nao autoriza esta.
    async fn no_lote(
        &self,
        prots: &[ProtNfe],
        ctx: &Ctx<'_>,
        retorno: &str,
        recibo: Option<&str>,
    ) -> Result<Resultado, ErroSefaz> {
        let Some(p) = prots.iter().find(|p| p.chave == ctx.chave) else {
            return self
                .recuperar(ctx, "o lote voltou sem o protocolo desta nota", false)
                .await;
        };
        let cs = p.cstat.as_str();
        let desfecho = if cstat::AUTORIZADA.contains(&cs) {
            if p.numero.is_empty() {
                let porque = format!("cStat {cs} sem nProt");
                return self.recuperar(ctx, &porque, false).await;
            }
            Desfecho::Autorizada {
                cstat: p.cstat.clone(),
                motivo: p.motivo.clone(),
                protocolo: p.numero.clone(),
                nfe_proc: Some(resposta::nfe_proc(ctx.nfe, &p.cru)),
            }
        } else if cstat::DENEGADA.contains(&cs) {
            Desfecho::Denegada {
                cstat: p.cstat.clone(),
                motivo: p.motivo.clone(),
                protocolo: p.numero.clone(),
                nfe_proc: (!p.numero.is_empty()).then(|| resposta::nfe_proc(ctx.nfe, &p.cru)),
            }
        } else if cs == "204" {
            // a mesma chave ja esta na base: a consulta devolve a nota autorizada, nunca uma nota nova
            let porque = format!("204 duplicidade ({})", p.motivo);
            return self.recuperar(ctx, &porque, true).await;
        } else if cs == "539" {
            return self.numero_ja_usado(p, ctx, retorno, recibo).await;
        } else {
            Desfecho::Rejeitada {
                cstat: p.cstat.clone(),
                motivo: p.motivo.clone(),
            }
        };
        Ok(fechar(
            Resultado::novo(desfecho, ctx.chave),
            ctx,
            retorno,
            recibo,
        ))
    }

    /// Rejeicao 539: o numero ja foi autorizado com outra chave. Le a chave que a mensagem traz, confere que e' do
    /// mesmo emitente, modelo, serie e numero e a consulta; o desfecho diz qual nota vale.
    async fn numero_ja_usado(
        &self,
        p: &ProtNfe,
        ctx: &Ctx<'_>,
        retorno: &str,
        recibo: Option<&str>,
    ) -> Result<Resultado, ErroSefaz> {
        let mut chave_existente = None;
        let mut situacao = None;
        let nossa = chave_dfe::decompor(ctx.chave, true).ok();
        let da_mensagem = chave_do_motivo(&p.motivo).and_then(|c| pedidos::chave_valida(&c).ok());
        if let (Some(n), Some(o)) = (nossa, da_mensagem) {
            let mesmo_numero = o.emitente == n.emitente
                && o.modelo == n.modelo
                && o.serie == n.serie
                && o.numero == n.numero;
            if mesmo_numero && o.chave != n.chave {
                let destino = self.destino_da_chave(&o.chave, o.uf);
                let c = self.consultar_chave(&o.chave, &destino, None).await?;
                situacao = cstat_do(&c.desfecho).map(str::to_string);
                chave_existente = Some(o.chave);
            }
        }
        let d = Desfecho::NumeroJaUsado {
            motivo: p.motivo.clone(),
            chave_existente,
            situacao_existente: situacao,
        };
        Ok(fechar(Resultado::novo(d, ctx.chave), ctx, retorno, recibo))
    }

    /// A resposta nao decidiu: consulta a chave antes de qualquer reenvio. `deve_existir`: a SEFAZ ja disse que a nota
    /// esta na base (204), entao 217 na consulta e' contradicao, nao "nao chegou".
    pub(super) async fn recuperar(
        &self,
        ctx: &Ctx<'_>,
        porque: &str,
        deve_existir: bool,
    ) -> Result<Resultado, ErroSefaz> {
        let destino = self.destino_da_chave(ctx.chave, self.uf);
        let c = self
            .consultar_chave(ctx.chave, &destino, Some(ctx.nfe))
            .await?;
        let da_consulta = c.detalhe.clone();
        let mut r = match &c.desfecho {
            Desfecho::Autorizada { .. }
            | Desfecho::Denegada { .. }
            | Desfecho::Cancelada { .. } => {
                let mut x = c;
                x.recuperada = true;
                x
            }
            Desfecho::NaoRecebida if !deve_existir => c,
            _ => Resultado::novo(Desfecho::SemResposta, ctx.chave),
        };
        r.xml_enviado = ctx.pedido.to_string();
        r.detalhe = format!("{porque} | consulta: {da_consulta}");
        Ok(r)
    }

    /// `consultar` sobre a chave ja conferida; `uf` e' a da chave (ou a SVC que a atende).
    pub(super) async fn consultar_chave(
        &self,
        chave: &str,
        uf: &str,
        nfe: Option<&str>,
    ) -> Result<Resultado, ErroSefaz> {
        let pedido = pedidos::cons_sit_qualquer(self.ambiente, chave)?;
        let sem = |detalhe: String| {
            let mut r = Resultado::novo(Desfecho::SemResposta, chave);
            r.detalhe = detalhe;
            r
        };
        let r = match self.chamar(Servico::ConsultaProtocolo, &pedido, uf).await? {
            Resp::Ok(r) => r,
            Resp::Sem(s) => return Ok(sem(s.detalhe)),
        };
        let ret = match resposta::ler_consulta(&r.texto) {
            Ok(x) => x,
            Err(e) => return Ok(sem(e.to_string())),
        };
        let mut res = Resultado::novo(Self::desfecho_da_consulta(&ret, chave, nfe), chave);
        res.xml_retorno = r.texto;
        res.detalhe = format!("cStat {}", ret.cab.cstat);
        Ok(res)
    }
}
