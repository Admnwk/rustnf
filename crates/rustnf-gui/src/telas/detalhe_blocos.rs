// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Os blocos de dados do detalhe de uma nota (tomador, servico, valores e emissao) e o selo do ambiente em que ela foi
//! gerada: desenho puro, sem estado. Vivem fora de `detalhe.rs` para o arquivo da tela nao crescer demais.

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{
    div, px, rgb, AnyElement, App, InteractiveElement as _, IntoElement, ParentElement as _,
    Styled as _,
};

use super::pecas::{cartao, linha_de_numero, linha_de_resumo, titulo_do_bloco, Negrito as _};
use crate::modelo::ambiente::Ambiente;
use crate::modelo::detalhe::DetalheDaNota;

pub(super) fn selo_do_ambiente(ambiente: Option<u8>) -> Option<AnyElement> {
    let a = match ambiente? {
        1 => Ambiente::Producao,
        _ => Ambiente::Restrita,
    };
    let c = a.cores();
    Some(
        div()
            .px_2()
            .py_0p5()
            .rounded_full()
            .text_xs()
            .font_semibold()
            .bg(rgb(c.fundo))
            .text_color(rgb(c.texto))
            .child(a.selo())
            .into_any_element(),
    )
}

pub(super) fn secoes(d: &DetalheDaNota, cx: &App) -> Vec<AnyElement> {
    let sem = |t: &str| {
        if t.is_empty() {
            "-".to_string()
        } else {
            t.to_string()
        }
    };
    let tomador = cartao(cx)
        .id("detalhe-tomador")
        .child(titulo_do_bloco("Tomador", cx))
        .child(linha_de_resumo("Nome", sem(&d.tomador), cx))
        .child(linha_de_resumo("Documento", sem(&d.tomador_documento), cx));
    let servico = cartao(cx)
        .id("detalhe-servico")
        .child(titulo_do_bloco("Serviço", cx))
        .child(linha_de_resumo(
            "Código",
            if d.servico_tributacao.is_empty() {
                sem(&d.servico_codigo)
            } else {
                format!("{} - {}", d.servico_codigo, d.servico_tributacao)
            },
            cx,
        ))
        .child(linha_de_resumo("Descrição", sem(&d.servico_descricao), cx));
    let valores = cartao(cx)
        .id("detalhe-valores")
        .child(titulo_do_bloco("Valores", cx))
        .child(linha_de_numero(
            "Valor do serviço",
            format!("R$ {}", d.valor_servico),
            false,
            cx,
        ))
        .child(linha_de_numero(
            "Base de cálculo",
            format!("R$ {}", d.base_calculo),
            false,
            cx,
        ))
        .child(linha_de_numero(
            if d.aliquota_iss.is_empty() {
                "ISS".to_string()
            } else {
                format!("ISS ({}%)", d.aliquota_iss)
            }
            .as_str(),
            format!("R$ {}", d.iss),
            false,
            cx,
        ))
        .child(linha_de_numero(
            "Total retido",
            format!("R$ {}", d.retido),
            false,
            cx,
        ))
        .child(div().h(px(1.)).w_full().bg(cx.theme().border).my_1())
        .child(linha_de_numero(
            "Valor líquido",
            format!("R$ {}", d.liquido),
            true,
            cx,
        ));
    let emissao = cartao(cx)
        .id("detalhe-emissao")
        .child(titulo_do_bloco("Emissão", cx))
        .child(linha_de_resumo("Emitida em", sem(&d.emissao), cx))
        .child(linha_de_resumo("Competência", sem(&d.competencia), cx))
        .child(linha_de_resumo("DPS", sem(&d.dps), cx))
        .child(linha_de_resumo(
            "Emitente",
            if d.emitente_documento.is_empty() {
                sem(&d.emitente)
            } else {
                format!("{} ({})", d.emitente, d.emitente_documento)
            },
            cx,
        ));
    vec![
        tomador.into_any_element(),
        servico.into_any_element(),
        valores.into_any_element(),
        emissao.into_any_element(),
    ]
}
