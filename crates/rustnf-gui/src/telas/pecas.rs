// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Pecas de desenho que as telas repetem: campo com ajuda e erro, titulo de grupo, cartao, aviso e resultado de teste.
//! So' cores do tema; a unica cor propria e' a do selo de ambiente (`modelo::ambiente`).

use gpui_kit::component::form::{field, Field};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Icon, IconName, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, AnyElement, App, Div, FontWeight, Hsla, IntoElement, ParentElement as _, SharedString,
    Styled,
};

use crate::modelo::certificado::Gravidade;

/// Pesos de fonte com nome curto.
pub(super) trait Negrito: Styled + Sized {
    /// Semi-negrito.
    fn font_semibold(self) -> Self {
        self.font_weight(FontWeight::SEMIBOLD)
    }

    /// Medio.
    fn font_medium(self) -> Self {
        self.font_weight(FontWeight::MEDIUM)
    }
}

impl<T: Styled> Negrito for T {}

/// Largura maxima dos formularios.
pub(super) const LARGURA_FORM: f32 = 760.0;

/// Um campo de formulario: rotulo, controle e, embaixo, o erro (em vermelho) ou, se nao houver, a ajuda discreta.
pub(super) fn campo(
    rotulo: impl Into<SharedString>,
    obrigatorio: bool,
    ajuda: Option<SharedString>,
    erro: Option<String>,
    controle: impl IntoElement,
) -> Field {
    field()
        .label(rotulo.into())
        .required(obrigatorio)
        .description_fn(move |_, cx| mensagem_do_campo(ajuda.clone(), erro.clone(), cx))
        .child(controle)
}

fn mensagem_do_campo(ajuda: Option<SharedString>, erro: Option<String>, cx: &App) -> AnyElement {
    let tema = cx.theme();
    match (erro, ajuda) {
        (Some(e), _) => h_flex()
            .gap_1()
            .items_start()
            .text_color(tema.danger)
            .child(Icon::new(IconName::CircleAlert).xsmall().mt(px(1.)))
            .child(div().flex_1().child(e))
            .into_any_element(),
        (None, Some(a)) => div().child(a).into_any_element(),
        (None, None) => div().into_any_element(),
    }
}

/// Titulo de um grupo de campos, com uma frase de apoio opcional.
pub(super) fn grupo(titulo: &str, apoio: Option<&str>, cx: &App) -> Div {
    let tema = cx.theme();
    v_flex()
        .gap_0p5()
        .pb_1()
        .border_b_1()
        .border_color(tema.border)
        .child(div().text_sm().font_semibold().child(titulo.to_string()))
        .when_some(apoio, |d, a| {
            d.child(
                div()
                    .text_xs()
                    .text_color(tema.muted_foreground)
                    .child(a.to_string()),
            )
        })
}

/// Cartao: fundo do tema, borda e canto arredondado.
pub(super) fn cartao(cx: &App) -> Div {
    let tema = cx.theme();
    v_flex()
        .gap_3()
        .p_5()
        .rounded_lg()
        .border_1()
        .border_color(tema.border)
        .bg(tema.background)
        .shadow_sm()
}

/// Cabecalho de pagina: titulo grande e frase de apoio.
pub(super) fn cabecalho(titulo: &str, apoio: &str, cx: &App) -> Div {
    let tema = cx.theme();
    v_flex()
        .gap_1()
        .child(div().text_2xl().font_semibold().child(titulo.to_string()))
        .child(
            div()
                .text_sm()
                .text_color(tema.muted_foreground)
                .child(apoio.to_string()),
        )
}

/// A cor do texto para uma gravidade.
pub(super) fn cor_da(gravidade: Gravidade, cx: &App) -> Hsla {
    let tema = cx.theme();
    match gravidade {
        Gravidade::Ok => tema.success,
        Gravidade::Aviso => tema.warning,
        Gravidade::Erro => tema.danger,
        Gravidade::Neutro => tema.muted_foreground,
    }
}

/// O icone para uma gravidade.
pub(super) fn icone_da(gravidade: Gravidade) -> IconName {
    match gravidade {
        Gravidade::Ok => IconName::CircleCheck,
        Gravidade::Aviso => IconName::TriangleAlert,
        Gravidade::Erro => IconName::CircleAlert,
        Gravidade::Neutro => IconName::Info,
    }
}

/// Uma linha "rotulo: valor" de resumo.
pub(super) fn linha_de_resumo(rotulo: &str, valor: impl IntoElement, cx: &App) -> Div {
    let tema = cx.theme();
    h_flex()
        .gap_3()
        .items_start()
        .child(
            div()
                .w(px(112.))
                .flex_shrink_0()
                .text_sm()
                .text_color(tema.muted_foreground)
                .child(rotulo.to_string()),
        )
        .child(div().flex_1().min_w_0().text_sm().child(valor))
}

/// Faixa de aviso com icone: erro, atencao ou confirmacao.
pub(super) fn faixa(gravidade: Gravidade, texto: impl Into<SharedString>, cx: &App) -> Div {
    let tema = cx.theme();
    let cor = cor_da(gravidade, cx);
    h_flex()
        .gap_2()
        .items_start()
        .p_3()
        .rounded_md()
        .border_1()
        .border_color(cor.opacity(0.45))
        .bg(cor.opacity(0.10))
        .text_sm()
        .text_color(tema.foreground)
        .child(
            Icon::new(icone_da(gravidade))
                .small()
                .text_color(cor)
                .flex_shrink_0()
                .mt(px(1.)),
        )
        .child(div().flex_1().min_w_0().child(texto.into()))
}
