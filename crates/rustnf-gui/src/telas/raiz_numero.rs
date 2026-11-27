// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A numeracao das DPS na janela: ler o proximo numero da serie e "Continuar deste numero" (a secao Notas das
//! Configuracoes), com a pergunta antes de mudar, porque a numeracao nao volta atras.

use std::sync::Arc;

use gpui_kit::component::notification::Notification;
use gpui_kit::component::WindowExt as _;
use gpui_kit::{AppContext as _, Context, Window};
use rustnf_app::config::Config;

use super::raiz::Raiz;
use crate::modelo::campo::Campo;

impl Raiz {
    /// Le de novo o proximo numero de DPS da serie (para a secao Notas das Configuracoes).
    pub(super) fn atualizar_proximo_dps(
        &mut self,
        efetiva: Config,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.proximo_dps = None;
        if efetiva.notas.pasta.trim().is_empty() {
            return;
        }
        let servicos = Arc::clone(&self.servicos);
        cx.spawn_in(window, async move |this, cx| {
            let r = cx
                .background_spawn(async move { servicos.proximo_numero(&efetiva) })
                .await;
            this.update(cx, |this, cx| {
                this.proximo_dps = r.ok();
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// "Continuar deste numero": confere o que foi digitado e pergunta antes de mudar a numeracao.
    pub(super) fn pedir_proximo_dps(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let digitado = self.entradas.proximo.read(cx).value().trim().to_string();
        let Some(proximo) = digitado
            .parse::<u64>()
            .ok()
            .filter(|n| *n > 0 && digitado.chars().all(|c| c.is_ascii_digit()))
        else {
            self.erros_externos.insert(
                Campo::ProximoDps,
                "Digite só o número da próxima DPS, por exemplo 1501.".into(),
            );
            cx.notify();
            return;
        };
        self.erros_externos.remove(&Campo::ProximoDps);
        let serie = self
            .servicos
            .contexto_da_nota(&self.servicos.efetiva(&self.salvo))
            .serie();
        let raiz = cx.entity();
        window.open_alert_dialog(cx, move |alerta, _, _| {
            let raiz = raiz.clone();
            alerta
                .title(format!("A próxima DPS da série {serie} será a nº {proximo}?"))
                .description(
                    "Use o número seguinte ao da última DPS emitida no outro sistema. Um número já usado faz a Sefin devolver a nota antiga em vez de autorizar a nova. A numeração não volta atrás depois.",
                )
                .confirm()
                .ok_text("Continuar deste número")
                .cancel_text("Cancelar")
                .on_ok(move |_, window, cx| {
                    raiz.update(cx, |this, cx| this.definir_proximo_dps(proximo, window, cx));
                    true
                })
        });
    }

    pub(super) fn definir_proximo_dps(
        &mut self,
        proximo: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.ocupado.numero {
            return;
        }
        let efetiva = self.servicos.efetiva(&self.salvo);
        let servicos = Arc::clone(&self.servicos);
        self.ocupado.numero = true;
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let r = cx
                .background_spawn(async move { servicos.definir_proximo(&efetiva, proximo) })
                .await;
            this.update_in(cx, |this, window, cx| {
                this.ocupado.numero = false;
                match r {
                    Ok(n) => {
                        this.entradas
                            .proximo
                            .update(cx, |e, cx| e.set_value("", window, cx));
                        window.push_notification(
                            Notification::success(format!("A próxima DPS será a nº {n}."))
                                .title("Numeração alterada"),
                            cx,
                        );
                        this.proximo_dps = Some(n);
                        // a Nova nota mostra o numero da proxima DPS: le de novo
                        this.nova.update(cx, |nova, cx| nova.atualizar_numero(cx));
                    }
                    Err(erro) => {
                        this.erros_externos.insert(Campo::ProximoDps, erro.texto());
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}
