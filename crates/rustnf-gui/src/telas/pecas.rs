// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Pecas de desenho que as telas repetem: campo com ajuda e erro, titulo de grupo, cartao, aviso e resultado de teste.
//! So' cores do tema; a unica cor propria e' a do selo de ambiente (`modelo::ambiente`).

use gpui_kit::component::form::{field, Field};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Icon, IconName, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, AnyElement, App, Div, FontWeight, Hsla, InteractiveElement as _, IntoElement,
    ParentElement as _, SharedString, Styled,
};

use rustnf_app::notas::Situacao;

use crate::modelo::certificado::Gravidade;
use crate::modelo::emissao::MensagemSefin;
use crate::modelo::lista::rotulo_da_situacao;
use crate::modelo::previa::{PreviaIbsCbs, ValoresIbsCbs};

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
        Gravidade::Aviso => legivel_no_claro(tema.warning, tema.is_dark()),
        Gravidade::Erro => tema.danger,
        Gravidade::Neutro => tema.muted_foreground,
    }
}

/// Luminosidade maxima de uma cor de aviso usada como TEXTO sobre o fundo claro.
const LUMINOSIDADE_DO_AVISO_NO_CLARO: f32 = 0.33;

/// O amarelo de aviso do tema e' claro demais para texto sobre fundo claro: no tema claro ele escurece ate dar
/// contraste de leitura; no escuro fica como o tema definiu.
fn legivel_no_claro(cor: Hsla, escuro: bool) -> Hsla {
    if escuro {
        cor
    } else {
        Hsla {
            l: cor.l.min(LUMINOSIDADE_DO_AVISO_NO_CLARO),
            ..cor
        }
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

/// O titulo pequeno, em maiusculas, de um bloco do painel ou de um cartao.
pub(super) fn titulo_do_bloco(titulo: &str, cx: &App) -> Div {
    div()
        .text_xs()
        .font_semibold()
        .text_color(cx.theme().muted_foreground)
        .child(titulo.to_uppercase())
}

/// Uma linha de numero: o rotulo a esquerda e o valor alinhado a direita.
pub(super) fn linha_de_numero(rotulo: &str, valor: String, negrito: bool, cx: &App) -> Div {
    let tema = cx.theme();
    h_flex()
        .justify_between()
        .gap_3()
        .child(
            div()
                .text_sm()
                .text_color(if negrito {
                    tema.foreground
                } else {
                    tema.muted_foreground
                })
                .when(negrito, |d| d.font_semibold())
                .child(rotulo.to_string()),
        )
        .child(
            div()
                .text_sm()
                .text_right()
                .when(negrito, |d| d.font_semibold())
                .child(valor),
        )
}

/// O quadro de resultado do IBS e da CBS: base, cada parcela com a aliquota efetiva, totais e as observacoes da
/// biblioteca. E' o mesmo na Nova nota e na Previa.
pub(super) fn quadro_do_ibscbs(v: &ValoresIbsCbs, cx: &App) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    let parcela = |rotulo: &str, aliquota: &str, valor: &str| {
        linha_de_numero(
            &format!("{rotulo} ({aliquota}%)"),
            format!("R$ {valor}"),
            false,
            cx,
        )
    };
    let mut bloco = v_flex()
        .id("previa-calculada")
        .gap_1p5()
        .child(linha_de_numero(
            "Base de cálculo",
            format!("R$ {}", v.base),
            false,
            cx,
        ))
        .child(parcela("IBS estadual", &v.ibs_uf.aliquota, &v.ibs_uf.valor))
        .child(parcela(
            "IBS municipal",
            &v.ibs_mun.aliquota,
            &v.ibs_mun.valor,
        ))
        .child(parcela("CBS", &v.cbs.aliquota, &v.cbs.valor))
        .child(linha_de_numero(
            "IBS total",
            format!("R$ {}", v.ibs_total),
            false,
            cx,
        ))
        .child(div().h(px(1.)).w_full().bg(cx.theme().border).my_1())
        .child(linha_de_numero(
            "Total IBS e CBS",
            format!("R$ {}", v.total),
            true,
            cx,
        ));
    for aviso in &v.avisos {
        bloco = bloco.child(div().text_xs().text_color(muted).child(aviso.clone()));
    }
    bloco
        .child(div().text_xs().text_color(muted).child(format!(
            "Prévia com as alíquotas de {}. Vale o que a Sefin calcular ao autorizar.",
            v.ano
        )))
        .into_any_element()
}

/// O corpo da previa do IBS/CBS conforme o que a conta devolveu. `abrir_aliquotas` e' o botao que leva as
/// Configuracoes (so' aparece quando falta a aliquota do ano); quem desenha a previa o monta, porque o clique e' dela.
pub(super) fn corpo_da_previa(
    previa: &PreviaIbsCbs,
    abrir_aliquotas: Option<AnyElement>,
    cx: &App,
) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    match previa {
        PreviaIbsCbs::Desligada => div()
            .id("previa-desligada")
            .text_sm()
            .text_color(muted)
            .child("O grupo IBS/CBS está desligado nesta nota.")
            .into_any_element(),
        PreviaIbsCbs::Incompleta(m) | PreviaIbsCbs::NaoSeAplica(m) => div()
            .id("previa-incompleta")
            .text_sm()
            .text_color(muted)
            .child(m.clone())
            .into_any_element(),
        PreviaIbsCbs::SemAliquota { ano } => v_flex()
            .id("previa-sem-aliquota")
            .gap_2()
            .child(faixa(
                Gravidade::Aviso,
                format!(
                    "Não há alíquota cadastrada para {ano}, então não há prévia. Cadastre a alíquota publicada na aba Alíquotas das Configurações."
                ),
                cx,
            ))
            .children(abrir_aliquotas.map(|b| h_flex().child(b)))
            .into_any_element(),
        PreviaIbsCbs::Recusada(m) => faixa(Gravidade::Erro, m.clone(), cx)
            .id("previa-recusada")
            .into_any_element(),
        PreviaIbsCbs::Calculada(v) => quadro_do_ibscbs(v, cx),
    }
}

/// O selo da situacao de uma nota: autorizada (verde), cancelada (vermelho), substituida (cinza) ou em analise fiscal
/// (amarelo), com as cores do tema.
pub(super) fn selo_da_situacao(situacao: Situacao, cx: &App) -> Div {
    let tema = cx.theme();
    let cor = match situacao {
        Situacao::Autorizada => tema.success,
        Situacao::Cancelada => tema.danger,
        Situacao::Substituida => tema.muted_foreground,
        Situacao::AnaliseFiscal => tema.warning,
    };
    div()
        .px_2()
        .py_0p5()
        .rounded_full()
        .text_xs()
        .font_semibold()
        .border_1()
        .border_color(cor.opacity(0.5))
        .bg(cor.opacity(0.12))
        .text_color(cor)
        .child(rotulo_da_situacao(situacao))
}

/// Uma mensagem da Sefin: a descricao em destaque, o codigo e o complemento em segundo plano.
pub(super) fn mensagem_da_sefin(m: &MensagemSefin, gravidade: Gravidade, cx: &App) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    let mut segundo = Vec::new();
    if !m.codigo.is_empty() {
        segundo.push(format!("Código {}", m.codigo));
    }
    if !m.complemento.is_empty() {
        segundo.push(m.complemento.clone());
    }
    if let Some(c) = m.campo {
        segundo.push(format!("Campo: {}", c.rotulo()));
    }
    let corpo = v_flex()
        .gap_0p5()
        .child(div().text_sm().font_semibold().child(m.descricao.clone()))
        .when(!segundo.is_empty(), |d| {
            d.child(div().text_xs().text_color(muted).child(segundo.join(" · ")))
        })
        .when(!m.dica.is_empty(), |d| {
            d.child(div().text_xs().text_color(muted).child(m.dica.clone()))
        });
    let cor = cor_da(gravidade, cx);
    h_flex()
        .gap_2()
        .items_start()
        .p_3()
        .rounded_md()
        .border_1()
        .border_color(cor.opacity(0.45))
        .bg(cor.opacity(0.10))
        .child(
            Icon::new(icone_da(gravidade))
                .small()
                .text_color(cor)
                .flex_shrink_0()
                .mt(px(1.)),
        )
        .child(div().flex_1().min_w_0().child(corpo))
        .into_any_element()
}

/// Um estado vazio: icone, titulo, explicacao e os botoes. Centralizado na area de conteudo.
pub(super) fn estado_vazio(
    icone: IconName,
    titulo: &str,
    texto: &str,
    botoes: Vec<AnyElement>,
    cx: &App,
) -> Div {
    let muted = cx.theme().muted_foreground;
    v_flex()
        .size_full()
        .items_center()
        .justify_center()
        .gap_3()
        .p_8()
        .child(Icon::new(icone).large().text_color(muted))
        .child(div().text_lg().font_semibold().child(titulo.to_string()))
        .child(
            div()
                .max_w(px(460.))
                .text_sm()
                .text_color(muted)
                .text_center()
                .child(texto.to_string()),
        )
        .when(!botoes.is_empty(), |d| {
            d.child(h_flex().gap_2().children(botoes))
        })
}
