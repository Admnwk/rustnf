// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O primeiro uso: tres passos, um de cada vez, com indicador de progresso e botoes Voltar e Continuar.

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{
    h_flex, v_flex, ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, StatefulInteractiveElement as _, Styled as _, Window,
};

use super::pecas::{cabecalho, cartao, faixa, Negrito as _};
use super::raiz::Raiz;
use crate::modelo::certificado::Gravidade;
use crate::modelo::passos::Passo;

impl Raiz {
    /// Indicador de progresso: uma bolinha por passo, ligadas por uma linha.
    fn indicador_de_passos(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tema = cx.theme();
        let atual = self.passo.indice();
        let mut linha = h_flex().items_start().w_full().id("indicador-de-passos");
        for passo in Passo::TODOS {
            let i = passo.indice();
            let feito = i < atual;
            let ativo = i == atual;
            let (fundo, texto, borda) = if feito {
                (tema.success, tema.success_foreground, tema.success)
            } else if ativo {
                (tema.primary, tema.primary_foreground, tema.primary)
            } else {
                (tema.background, tema.muted_foreground, tema.border)
            };
            let bolinha = div()
                .size(px(30.))
                .flex_shrink_0()
                .rounded_full()
                .border_1()
                .border_color(borda)
                .bg(fundo)
                .text_color(texto)
                .flex()
                .items_center()
                .justify_center()
                .text_sm()
                .font_semibold()
                .child(if feito {
                    Icon::new(IconName::Check).small().into_any_element()
                } else {
                    (i + 1).to_string().into_any_element()
                });
            if i > 0 {
                linha = linha.child(
                    div()
                        .flex_1()
                        .h(px(2.))
                        .mt(px(14.))
                        .mx_2()
                        .rounded_full()
                        .bg(if i <= atual {
                            tema.success
                        } else {
                            tema.border
                        }),
                );
            }
            linha = linha.child(
                v_flex()
                    .gap_1()
                    .items_center()
                    .w(px(96.))
                    .child(bolinha)
                    .child(
                        div()
                            .text_xs()
                            .font_medium()
                            .text_color(if ativo {
                                tema.foreground
                            } else {
                                tema.muted_foreground
                            })
                            .child(passo.titulo()),
                    ),
            );
        }
        linha
    }

    fn corpo_do_passo(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let _ = window;
        match self.passo {
            Passo::Emitente => v_flex()
                .gap_4()
                .child(self.campos_emitente(false, cx))
                .into_any_element(),
            Passo::Certificado => {
                let ocupado = self.ocupado.certificado || self.ocupado.passo;
                let resultado = self.resultado_do_certificado(cx);
                v_flex()
                    .gap_4()
                    .child(self.campo_do_certificado(cx))
                    .child(self.campo_da_senha(false, cx))
                    .child(
                        h_flex().gap_2().items_center().child(
                            Button::new("testar-certificado-passo")
                                .label("Testar certificado")
                                .icon(IconName::CircleCheck)
                                .outline()
                                .loading(self.ocupado.certificado)
                                .disabled(ocupado)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.testar_certificado(window, cx);
                                })),
                        ),
                    )
                    .children(resultado)
                    .into_any_element()
            }
            Passo::Notas => v_flex()
                .gap_4()
                .child(self.campo_da_pasta(true, cx))
                .into_any_element(),
        }
    }

    /// O primeiro uso.
    pub(super) fn assistente(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let muted = cx.theme().muted_foreground;
        let borda = cx.theme().border;
        let passo = self.passo;
        let (titulo, apoio): (&str, SharedString) = match passo {
            Passo::Emitente => (
                "Quem emite as notas?",
                "Estes dados completam cada pedido de nota. Você pode mudá-los depois em Configurações.".into(),
            ),
            Passo::Certificado => (
                "Certificado digital A1",
                "O certificado assina as notas. A senha vai para o cofre de senhas do Windows e nunca para o arquivo de configuração.".into(),
            ),
            Passo::Notas => (
                "Onde guardar as notas?",
                "As notas autorizadas ficam numa pasta sua, em arquivos XML, sem banco de dados.".into(),
            ),
        };
        let ultimo = passo.proximo().is_none();
        let ocupado = self.ocupado.passo;
        let aviso = self.aviso.clone();
        let corpo = self.corpo_do_passo(window, cx);
        let indicador = self.indicador_de_passos(cx);
        let a_cartao = cartao(cx).gap_5().p_6();

        div()
            .id("primeiro-uso")
            .size_full()
            .overflow_y_scroll()
            .child(
                v_flex()
                    .w_full()
                    .max_w(px(680.))
                    .mx_auto()
                    .gap_6()
                    .px_6()
                    .py_8()
                    .child(cabecalho(
                        "Bem-vindo ao rustnf",
                        "Vamos deixar tudo pronto em três passos curtos.",
                        cx,
                    ))
                    .child(indicador)
                    .child(
                        a_cartao
                            .child(
                                v_flex()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_semibold()
                                            .text_color(muted)
                                            .child(passo.indicador().to_uppercase()),
                                    )
                                    .child(div().text_xl().font_semibold().child(titulo))
                                    .child(div().text_sm().text_color(muted).child(apoio)),
                            )
                            .child(corpo)
                            .when_some(aviso, |d, a| {
                                d.child(faixa(
                                    if a.erro {
                                        Gravidade::Erro
                                    } else {
                                        Gravidade::Ok
                                    },
                                    a.texto,
                                    cx,
                                ))
                            })
                            .child(
                                h_flex()
                                    .pt_4()
                                    .border_t_1()
                                    .border_color(borda)
                                    .justify_between()
                                    .child(
                                        Button::new("voltar")
                                            .label("Voltar")
                                            .icon(IconName::ChevronLeft)
                                            .outline()
                                            .disabled(passo.anterior().is_none() || ocupado)
                                            .on_click(
                                                cx.listener(|this, _, _, cx| this.voltar(cx)),
                                            ),
                                    )
                                    .child(
                                        Button::new("continuar")
                                            .label(if ultimo { "Concluir" } else { "Continuar" })
                                            .primary()
                                            .loading(ocupado)
                                            .disabled(ocupado)
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.continuar(window, cx);
                                            })),
                                    ),
                            ),
                    ),
            )
            .into_any_element()
    }
}
