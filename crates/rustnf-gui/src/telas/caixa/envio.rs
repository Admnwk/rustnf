// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O envio do caixa: abrir o certificado, emitir fora da tela e tratar o desfecho. Nada sai sem o botao; em producao, nem com
//! ele sem a marcacao. Sem clique duplo: com o envio em curso o botao nao faz nada. A venda que a SEFAZ nao responde ANTES do
//! envio sai em contingencia off-line sozinha (`Modo::OfflineSeNecessario`); a que foi enviada e ficou sem resposta fica
//! pendente e se retoma, nunca vira uma segunda NFC-e para o mesmo cupom (`rustnf_app::nfce::emitir`).

use std::sync::Arc;

use gpui_kit::{AppContext as _, Context, Window};
use rustnf_app::erro::Erro;
use rustnf_app::nfce::Modo;
use rustnf_app::segredo::Segredo;

use super::{Etapa, EventoCaixa, Fase, TelaCaixa};
use crate::fora;
use crate::modelo::nfce_caixa::PendenciaCaixa;
use crate::modelo::nfce_desfecho::DesfechoCaixa;
use crate::servicos::{emitir_nfce, ErroTela};
use crate::telas::senha::{classificar, Credencial, DialogoSenha, EventoSenha};

impl TelaCaixa {
    /// Emitir (o botao e o Ctrl+Enter): confere de novo que pode, abre o certificado e envia.
    pub(super) fn emitir(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.pode_emitir() {
            self.executar(None, window, cx);
        }
    }

    fn executar(&mut self, digitada: Option<Segredo>, window: &mut Window, cx: &mut Context<Self>) {
        if !matches!(self.fase, Fase::Vendendo) {
            return;
        }
        let pedido = match self.cupom.pedido(&self.contexto) {
            Ok(p) => p,
            Err(_) => return,
        };
        self.fase = Fase::Enviando(Etapa::Certificado);
        self.erro = None;
        self.desfecho = None;
        self.chave_copiada = false;
        window.refresh();
        cx.notify();
        let servicos = Arc::clone(&self.servicos);
        let fabrica = servicos.fabrica_nfe();
        let config = self.config.clone();
        let digitou = digitada.is_some();
        cx.spawn_in(window, async move |this, cx| {
            let (s2, c2) = (Arc::clone(&servicos), config.clone());
            let preparado = cx
                .background_spawn(async move {
                    let preparo = s2
                        .preparar_nfe(&c2, digitada.as_ref())
                        .map_err(Falha::Certificado)?;
                    // o QR Code (e o CSC, na versao 2) so' se le do cofre depois de o certificado abrir
                    let qr = s2.opcoes_qr_nfce(&c2).map_err(Falha::Qr)?;
                    Ok((preparo, qr))
                })
                .await;
            let (preparo, qr) = match preparado {
                Ok(par) => par,
                Err(Falha::Certificado(e)) => {
                    match classificar(Err(e), digitou) {
                        Credencial::PedirSenha(erro) => {
                            this.update_in(cx, |this, window, cx| {
                                this.fase = Fase::Vendendo;
                                this.pedir_senha(erro, window, cx);
                            })
                            .ok();
                        }
                        Credencial::Falhou(texto) => {
                            this.update_in(cx, |this, window, cx| {
                                this.terminou(DesfechoCaixa::do_erro(&Erro::Uso(texto)), window, cx);
                            })
                            .ok();
                        }
                        Credencial::Pronta(_) => {}
                    }
                    return;
                }
                Err(Falha::Qr(e)) => {
                    this.update_in(cx, |this, window, cx| {
                        this.terminou(DesfechoCaixa::do_erro(&Erro::Uso(e.texto())), window, cx);
                    })
                    .ok();
                    return;
                }
            };
            this.update_in(cx, |this, window, cx| {
                this.fase = Fase::Enviando(Etapa::Enviando);
                window.refresh();
                cx.notify();
            })
            .ok();
            let trabalho = emitir_nfce(
                fabrica,
                preparo,
                config,
                qr,
                pedido,
                Modo::OfflineSeNecessario,
            );
            let d = match fora::rede(trabalho) {
                Some(tarefa) => tarefa.await.unwrap_or_else(|_| DesfechoCaixa::Recusada {
                    pendencias: vec![PendenciaCaixa {
                        mensagem: "A chamada à SEFAZ foi interrompida.".into(),
                        como_resolver: "Confira a lista de Notas antes de vender de novo: a nota pode ter ficado pendente.".into(),
                    }],
                }),
                None => DesfechoCaixa::Recusada {
                    pendencias: vec![PendenciaCaixa {
                        mensagem: "Não foi possível preparar a chamada de rede.".into(),
                        como_resolver: "Tente de novo: nada saiu da sua máquina.".into(),
                    }],
                },
            };
            this.update_in(cx, |this, window, cx| this.terminou(d, window, cx))
                .ok();
        })
        .detach();
    }

    /// Trata como a venda terminou: a venda feita (cupom, off-line, pendente) deixa o caixa pronto para a proxima; a recusada
    /// e a rejeitada deixam o cupom como estava para a correcao.
    fn terminou(&mut self, d: DesfechoCaixa, window: &mut Window, cx: &mut Context<Self>) {
        self.fase = Fase::Vendendo;
        let feita =
            d.tem_cupom() || d.deixa_pendente() || matches!(d, DesfechoCaixa::Denegada { .. });
        if feita {
            // a nota esta na pasta com o cupom dentro: o cupom do caixa nao pode virar uma segunda nota
            self.limpar(window, cx);
            self.recarregar_pendentes(cx);
            self.atualizar_o_contexto(cx);
            cx.emit(EventoCaixa::NotaMudou);
        }
        self.desfecho = Some(d);
        window.refresh();
        cx.notify();
    }

    // ---------------------------------------------------------------- senha

    fn pedir_senha(&mut self, erro: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        let desligado = self.servicos.cofre_desligado();
        let dialogo = cx.new(|cx| DialogoSenha::new(desligado, erro, window, cx));
        self.sub_senha = Some(cx.subscribe_in(
            &dialogo,
            window,
            |this, dialogo, evento: &EventoSenha, window, cx| {
                this.sub_senha = None;
                this.senha_pedida = None;
                if matches!(evento, EventoSenha::Usada) {
                    let senha = dialogo.update(cx, |d, _| d.tomar());
                    this.executar(senha, window, cx);
                }
            },
        ));
        self.senha_pedida = Some(dialogo.clone());
        DialogoSenha::abrir(&dialogo, window, cx);
    }
}

/// Por que nao se chegou ao envio.
enum Falha {
    /// O certificado ou a senha.
    Certificado(ErroTela),
    /// O QR Code (CSC).
    Qr(ErroTela),
}
