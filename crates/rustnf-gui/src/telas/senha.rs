// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O pedido da senha do certificado, quando nao ha senha guardada no cofre: um dialogo com campo mascarado, usado pela
//! emissao, pelo cancelamento e pela consulta. A senha vale so' para aquela operacao: vai direto para o certificado,
//! nao e' guardada e o campo e' esvaziado ao usar.

use std::cell::Cell;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::dialog::DialogFooter;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{v_flex, WindowExt as _};
use gpui_kit::{
    div, px, AppContext as _, Context, Entity, EventEmitter, Focusable as _, ParentElement as _,
    Styled as _, Subscription, Window,
};
use rustnf_app::segredo::Segredo;

use super::pecas::faixa;
use crate::modelo::certificado::Gravidade;
use crate::servicos::{ErroTela, Preparo};

/// O que fazer depois de tentar abrir o certificado para uma operacao (emitir, cancelar, consultar).
pub(super) enum Credencial {
    /// O certificado abriu: segue para o envio.
    Pronta(Box<Preparo>),
    /// Falta a senha (nao ha no cofre) ou a digitada nao abre o certificado: pede a senha, com o erro se houver.
    PedirSenha(Option<String>),
    /// Nao deu para abrir o certificado: a frase diz por que, e nada sai.
    Falhou(String),
}

/// Classifica o resultado de abrir o certificado. `digitou`: a senha usada foi a digitada nesta operacao (senha
/// incorreta digitada pede de novo; incorreta do cofre e' erro).
pub(super) fn classificar(resultado: Result<Preparo, ErroTela>, digitou: bool) -> Credencial {
    match resultado {
        Ok(p) => Credencial::Pronta(Box::new(p)),
        Err(ErroTela::SemSenha) => Credencial::PedirSenha(None),
        Err(ErroTela::SenhaIncorreta) if digitou => {
            Credencial::PedirSenha(Some(ErroTela::SenhaIncorreta.texto()))
        }
        Err(outro) => Credencial::Falhou(outro.texto()),
    }
}

/// O que o dialogo avisa a quem o abriu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EventoSenha {
    /// A pessoa digitou a senha e confirmou: quem abriu a retira com `tomar`.
    Usada,
    /// O dialogo foi fechado sem usar a senha.
    Fechada,
}

/// O dialogo da senha do certificado.
pub(super) struct DialogoSenha {
    entrada: Entity<InputState>,
    /// O erro que o dialogo mostra (senha incorreta ou vazia).
    pub(super) erro: Option<String>,
    /// O cofre esta desligado: a frase explica que a senha nunca e' guardada.
    desligado: bool,
    senha: Option<Segredo>,
    usada: bool,
    _sub: Subscription,
}

impl EventEmitter<EventoSenha> for DialogoSenha {}

impl DialogoSenha {
    /// O dialogo, ainda fechado. `erro` aparece no corpo (por exemplo, "Senha do certificado incorreta.").
    pub(super) fn new(
        desligado: bool,
        erro: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let entrada = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Senha do certificado")
                .masked(true)
        });
        let sub = cx.subscribe_in(
            &entrada,
            window,
            |this, _, evento: &InputEvent, window, cx| {
                if matches!(evento, InputEvent::PressEnter { .. }) {
                    this.usar(window, cx);
                }
            },
        );
        Self {
            entrada,
            erro,
            desligado,
            senha: None,
            usada: false,
            _sub: sub,
        }
    }

    /// Retira a senha digitada (so' existe depois de `Usada`).
    pub(super) fn tomar(&mut self) -> Option<Segredo> {
        self.senha.take()
    }

    /// Usa a senha digitada: guarda-a para quem abriu, esvazia o campo, fecha o dialogo e avisa.
    fn usar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let texto = self.entrada.read(cx).value().to_string();
        if texto.is_empty() {
            self.erro = Some("Digite a senha do certificado.".into());
            window.refresh();
            cx.notify();
            return;
        }
        self.senha = Some(Segredo::novo(texto));
        self.usada = true;
        self.entrada.update(cx, |e, cx| e.set_value("", window, cx));
        window.close_dialog(cx);
        cx.emit(EventoSenha::Usada);
    }

    /// Abre o dialogo (Esc e Cancelar o fecham; Enter usa a senha).
    pub(super) fn abrir(dialogo: &Entity<Self>, window: &mut Window, cx: &mut gpui_kit::App) {
        let d = dialogo.clone();
        let primeira = Cell::new(true);
        window.open_dialog(cx, move |dialog, window, cx| {
            let entrada = d.read(cx).entrada.clone();
            if primeira.replace(false) {
                entrada.focus_handle(cx).focus(window, cx);
            }
            let corpo = d.read(cx);
            let erro = corpo.erro.clone();
            let desligado = corpo.desligado;
            let usar = d.clone();
            let enter = d.clone();
            let fechou = d.clone();
            dialog
                .title("Senha do certificado")
                .w(px(460.))
                // Enter usa a senha digitada (o dialogo so' fecha se ela for usada)
                .on_ok(move |_, window, cx| {
                    enter.update(cx, |d, cx| d.usar(window, cx));
                    false
                })
                .on_close(move |_, _, cx| {
                    fechou.update(cx, |d, cx| {
                        if !d.usada {
                            cx.emit(EventoSenha::Fechada);
                        }
                    });
                })
                .child(
                    v_flex()
                        .gap_3()
                        .child(div().text_sm().child(if desligado {
                            "O cofre de senhas está desligado. Digite a senha do certificado: ela vale só para esta operação e não é guardada."
                        } else {
                            "Não há senha guardada no cofre para este certificado. Digite-a: ela vale só para esta operação e não é guardada."
                        }))
                        .child(Input::new(&entrada).id("senha-da-emissao").mask_toggle())
                        .children(erro.map(|e| faixa(Gravidade::Erro, e, cx))),
                )
                .footer(
                    DialogFooter::new()
                        .child(
                            Button::new("cancelar-senha")
                                .label("Cancelar")
                                .outline()
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("usar-senha")
                                .label("Usar agora")
                                .primary()
                                .on_click(move |_, window, cx| {
                                    usar.update(cx, |d, cx| d.usar(window, cx));
                                }),
                        ),
                )
        });
    }
}
