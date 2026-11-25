// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! As telas que usam o emitente, o certificado e a pasta das notas (Nova nota, Notas e Consultar) com a configuracao
//! incompleta: um estado vazio que explica o que falta e leva ao Inicio ou as Configuracoes. Sem formulario nem lista.

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Icon, IconName, Sizable as _};
use gpui_kit::{
    div, px, AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, StatefulInteractiveElement as _, Styled as _,
};

use super::pecas::{cabecalho, cartao, Negrito as _};
use super::raiz::{Raiz, Tela};
use crate::modelo::passos::{configuracao_completa, o_que_falta};

impl Raiz {
    /// O conteudo da tela Nova nota: o formulario, ou o que falta configurar.
    pub(super) fn tela_nova_nota(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let filho = self.nova.clone().into_any_element();
        self.exigindo_configuracao(
            "Nova nota",
            "Antes de emitir, o rustnf precisa de alguns dados.",
            "A nota usa os dados do emitente, o certificado digital e a pasta das notas. Ainda falta:",
            filho,
            cx,
        )
    }

    /// O conteudo da tela Notas: a lista, ou o que falta configurar.
    pub(super) fn tela_notas(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let filho = self.notas.clone().into_any_element();
        self.exigindo_configuracao(
            "Notas",
            "Antes de listar as notas, o rustnf precisa de alguns dados.",
            "A lista usa a pasta das notas, e cancelar usa o certificado digital. Ainda falta:",
            filho,
            cx,
        )
    }

    /// O conteudo da tela Consultar: o campo da chave, ou o que falta configurar.
    pub(super) fn tela_consultar(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let filho = self.consultar.clone().into_any_element();
        self.exigindo_configuracao(
            "Consultar",
            "Antes de consultar, o rustnf precisa de alguns dados.",
            "A consulta usa o certificado digital e os dados do emitente. Ainda falta:",
            filho,
            cx,
        )
    }

    /// `filho` com a configuracao completa; senao, o estado vazio que explica o que falta.
    fn exigindo_configuracao(
        &mut self,
        titulo: &str,
        apoio: &str,
        explicacao: &str,
        filho: AnyElement,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if configuracao_completa(&self.salvo) {
            return filho;
        }
        let muted = cx.theme().muted_foreground;
        let itens = o_que_falta(&self.salvo);
        let mut lista = v_flex().gap_1().id("o-que-falta");
        for (i, item) in itens.iter().enumerate() {
            lista = lista.child(
                h_flex()
                    .id(SharedString::from(format!("falta-{i}")))
                    .gap_2()
                    .items_center()
                    .child(Icon::new(IconName::CircleAlert).text_color(cx.theme().warning))
                    .child(div().text_sm().child((*item).to_string())),
            );
        }
        div()
            .id("nota-sem-configuracao")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.rolagem)
            .child(
                v_flex()
                    .w_full()
                    .max_w(px(680.))
                    .mx_auto()
                    .gap_5()
                    .px_6()
                    .py_8()
                    .child(cabecalho(titulo, apoio, cx))
                    .child(
                        cartao(cx)
                            .gap_4()
                            .child(
                                h_flex()
                                    .gap_3()
                                    .items_center()
                                    .child(Icon::new(IconName::FileText).large().text_color(muted))
                                    .child(
                                        div()
                                            .text_lg()
                                            .font_semibold()
                                            .child("Falta configurar o rustnf"),
                                    ),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(muted)
                                    .child(explicacao.to_string()),
                            )
                            .child(lista)
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(
                                        Button::new("ir-para-o-inicio")
                                            .label("Ir para o Início")
                                            .icon(IconName::LayoutDashboard)
                                            .primary()
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.ir_para(Tela::Inicio, cx);
                                            })),
                                    )
                                    .child(
                                        Button::new("ir-para-as-configuracoes")
                                            .label("Abrir Configurações")
                                            .icon(IconName::Settings)
                                            .outline()
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.ir_para(Tela::Configuracoes, cx);
                                            })),
                                    ),
                            ),
                    ),
            )
            .into_any_element()
    }
}
