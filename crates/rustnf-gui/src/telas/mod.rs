// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! As telas: so' desenho e ligacao de eventos. O que decide (mascara, conferencia, estado do certificado, passos) mora
//! em `modelo`; o que mexe em disco, cofre e rede mora em `servicos`.

mod assistente;
mod campos;
mod configuracoes;
mod inicio;
mod layout;
mod pecas;
mod raiz;
#[cfg(test)]
mod testes;

use std::path::PathBuf;
use std::sync::Arc;

use gpui_kit::component::{set_locale, TitleBar};
use gpui_kit::{
    actions, px, size, App, AppContext as _, Context, Entity, KeyBinding, TitlebarOptions,
    WindowBounds, WindowOptions,
};

pub use raiz::{Raiz, Secao, Tela};

use crate::servicos::Servicos;

actions!(rustnf_gui, [SalvarConfiguracao, DescartarConfiguracao]);

/// Tamanho inicial da janela.
const LARGURA: f32 = 1120.0;
const ALTURA: f32 = 780.0;

/// Tamanho minimo: abaixo disso os formularios nao cabem.
const LARGURA_MINIMA: f32 = 940.0;
const ALTURA_MINIMA: f32 = 620.0;

/// Registra os atalhos de teclado: Ctrl+S salva nas Configuracoes.
pub fn registrar_atalhos(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("ctrl-s", SalvarConfiguracao, Some("Raiz"))]);
}

/// As opcoes da janela principal: barra de titulo propria, tamanho inicial e minimo.
#[must_use]
pub fn opcoes_da_janela(cx: &App) -> WindowOptions {
    let base = TitleBar::window_options();
    WindowOptions {
        titlebar: Some(TitlebarOptions {
            title: Some("rustnf".into()),
            ..TitleBar::title_bar_options()
        }),
        window_bounds: Some(WindowBounds::centered(size(px(LARGURA), px(ALTURA)), cx)),
        window_min_size: Some(size(px(LARGURA_MINIMA), px(ALTURA_MINIMA))),
        ..base
    }
}

/// Inicia os componentes, o idioma e os atalhos; chamar uma vez, antes de abrir a janela.
pub fn iniciar(cx: &mut App) {
    gpui_kit::init(cx);
    set_locale("pt-BR");
    registrar_atalhos(cx);
}

/// Abre a janela principal.
///
/// # Errors
///
/// O erro do GPUI se a janela nao puder ser aberta.
pub fn abrir_janela(
    servicos: Arc<Servicos>,
    cx: &mut App,
) -> gpui_kit::Result<(gpui_kit::AnyWindowHandle, Entity<Raiz>)> {
    let opcoes = opcoes_da_janela(cx);
    gpui_kit::open_window(opcoes, cx, move |window, cx| {
        cx.new(|cx: &mut Context<Raiz>| Raiz::new(servicos, window, cx))
    })
}

/// Abre a janela e roda ate ela ser fechada.
pub fn executar(config: Option<PathBuf>) {
    let servicos = Arc::new(Servicos::do_sistema(config));
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(move |cx| {
            iniciar(cx);
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            if abrir_janela(servicos, cx).is_err() {
                eprintln!("rustnf-gui: nao foi possivel abrir a janela.");
                cx.quit();
                return;
            }
            cx.activate(true);
        });
}
