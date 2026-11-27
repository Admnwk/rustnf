// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Rolagem pelo teclado nas telas de formulario.
//!
//! * O campo que ganha o foco (Tab, Shift+Tab) pede para aparecer (`Window::request_autoscroll`, feito pelo proprio
//!   campo); so' a lista do GPUI atende esse pedido, e as telas rolam numa `div`. [`Revelar`] envolve a area com
//!   rolagem e atende: depois de preparar o conteudo, move a rolagem ate o pedido caber na area.
//! * As setas, PageUp/PageDown e Ctrl+Home/Ctrl+End rolam a tela aberta ([`rolar`]); a janela decide qual (as acoes
//!   ficam em `telas::mod`).

use gpui_kit::{
    px, AnyElement, App, Bounds, Element, ElementId, FocusHandle, GlobalElementId,
    InspectorElementId, IntoElement, LayoutId, Pixels, Point, ScrollHandle, SharedString, Window,
};

/// Quanto uma seta rola.
const LINHA: f32 = 48.0;
/// O que fica da pagina anterior a vista depois de PageUp/PageDown, para nao perder o fio.
const SOBRA_DA_PAGINA: f32 = 48.0;

/// Um passo de rolagem pelo teclado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Passo {
    /// Seta para cima.
    LinhaAcima,
    /// Seta para baixo.
    LinhaAbaixo,
    /// PageUp.
    PaginaAcima,
    /// PageDown.
    PaginaAbaixo,
    /// Ctrl+Home.
    Topo,
    /// Ctrl+End.
    Fim,
}

/// O novo deslocamento da rolagem para o passo. O GPUI guarda o deslocamento negativo (0 = no topo; `-maximo` = no
/// fim). `None` quando nada muda (ja' no topo ou no fim, ou sem o que rolar).
#[must_use]
pub(super) fn deslocamento_do_passo(
    passo: Passo,
    atual: Pixels,
    maximo: Pixels,
    altura: Pixels,
) -> Option<Pixels> {
    let pagina = (altura - px(SOBRA_DA_PAGINA)).max(px(LINHA));
    let novo = match passo {
        Passo::LinhaAcima => atual + px(LINHA),
        Passo::LinhaAbaixo => atual - px(LINHA),
        Passo::PaginaAcima => atual + pagina,
        Passo::PaginaAbaixo => atual - pagina,
        Passo::Topo => px(0.),
        Passo::Fim => -maximo,
    };
    let novo = novo.min(px(0.)).max(-maximo.max(px(0.)));
    (novo != atual).then_some(novo)
}

/// O novo deslocamento para o `pedido` (em coordenadas da janela) caber na `area` visivel. Pedido mais alto que a
/// area fica com o topo a vista. `None` quando ja' cabe ou quando o pedido nao e' desta area (fora dela na
/// horizontal).
#[must_use]
pub(super) fn deslocamento_para_revelar(
    area: Bounds<Pixels>,
    atual: Pixels,
    maximo: Pixels,
    pedido: Bounds<Pixels>,
) -> Option<Pixels> {
    if area.size.height <= px(0.) || pedido.right() <= area.left() || pedido.left() >= area.right()
    {
        return None;
    }
    let novo = if pedido.top() < area.top() || pedido.size.height > area.size.height {
        atual + (area.top() - pedido.top())
    } else if pedido.bottom() > area.bottom() {
        atual - (pedido.bottom() - area.bottom())
    } else {
        return None;
    };
    let novo = novo.min(px(0.)).max(-maximo.max(px(0.)));
    (novo != atual).then_some(novo)
}

/// Aplica o passo a rolagem; diz se moveu.
pub(super) fn rolar(rolagem: &ScrollHandle, passo: Passo) -> bool {
    let atual = rolagem.offset();
    match deslocamento_do_passo(
        passo,
        atual.y,
        rolagem.max_offset().y,
        rolagem.bounds().size.height,
    ) {
        Some(y) => {
            rolagem.set_offset(Point::new(atual.x, y));
            true
        }
        None => false,
    }
}

/// Envolve a area com rolagem (a `div` com `track_scroll(rolagem)`) para atender o pedido do campo focado.
pub(super) fn revelar(rolagem: &ScrollHandle, filho: impl IntoElement) -> Revelar {
    Revelar {
        filho: Some(filho.into_any_element()),
        rolagem: rolagem.clone(),
    }
}

/// O elemento de [`revelar`].
pub(super) struct Revelar {
    filho: Option<AnyElement>,
    rolagem: ScrollHandle,
}

impl Element for Revelar {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let layout = self
            .filho
            .as_mut()
            .map(|f| f.request_layout(window, cx))
            .unwrap_or_else(|| window.request_layout(gpui_kit::Style::default(), [], cx));
        (layout, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let Some(filho) = self.filho.as_mut() else {
            return;
        };
        filho.prepaint(window, cx);
        let Some(pedido) = window.take_autoscroll() else {
            return;
        };
        let atual = self.rolagem.offset();
        match deslocamento_para_revelar(
            self.rolagem.bounds(),
            atual.y,
            self.rolagem.max_offset().y,
            pedido,
        ) {
            Some(y) => {
                self.rolagem.set_offset(Point::new(atual.x, y));
                // o deslocamento novo vale no proximo quadro
                window.refresh();
            }
            None => {
                // nao era desta area (ou ja' cabe): devolve o pedido para uma area de fora
                window.request_autoscroll(pedido);
            }
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut (),
        _prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(filho) = self.filho.as_mut() {
            filho.paint(window, cx);
        }
    }
}

/// Envolve um botao (ou outro elemento que ganha o foco por Tab) para o foco por teclado trazer o elemento para a
/// vista. So' os campos de texto pedem a rolagem ao ganhar o foco; o botao nao, e o primeiro Tab de uma janela
/// recem-aberta ia para um botao fora da vista. O pedido sai uma vez, na hora em que o foco CHEGA (nao a cada quadro:
/// quem rola com o mouse nao e' puxado de volta), e o [`Revelar`] da tela o atende. `foco` e' o mesmo `FocusHandle`
/// entregue ao botao (`Button::track_focus`).
pub(super) fn revelar_foco(
    id: impl Into<SharedString>,
    foco: &FocusHandle,
    filho: impl IntoElement,
) -> RevelarFoco {
    RevelarFoco {
        id: ElementId::Name(id.into()),
        foco: foco.clone(),
        filho: Some(filho.into_any_element()),
    }
}

/// O elemento de [`revelar_foco`].
pub(super) struct RevelarFoco {
    id: ElementId,
    foco: FocusHandle,
    filho: Option<AnyElement>,
}

impl Element for RevelarFoco {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let layout = self
            .filho
            .as_mut()
            .map(|f| f.request_layout(window, cx))
            .unwrap_or_else(|| window.request_layout(gpui_kit::Style::default(), [], cx));
        (layout, ())
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(filho) = self.filho.as_mut() {
            filho.prepaint(window, cx);
        }
        let Some(id) = id else {
            return;
        };
        let focado = self.foco.is_focused(window);
        let acabou_de_chegar = window.with_element_state::<bool, bool>(id, |antes, _| {
            let chegou = focado && !antes.unwrap_or(false);
            (chegou, focado)
        });
        if acabou_de_chegar {
            window.request_autoscroll(bounds);
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut (),
        _prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(filho) = self.filho.as_mut() {
            filho.paint(window, cx);
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use gpui_kit::{point, size};

    fn area() -> Bounds<Pixels> {
        Bounds::new(point(px(200.), px(100.)), size(px(800.), px(500.)))
    }

    #[test]
    fn campo_abaixo_da_area_sobe_ate_caber_e_acima_desce() {
        // area de 100 a 600; campo de 640 a 680: rola 80 para baixo
        let campo = Bounds::new(point(px(300.), px(640.)), size(px(200.), px(40.)));
        assert_eq!(
            deslocamento_para_revelar(area(), px(0.), px(1000.), campo),
            Some(px(-80.))
        );
        // ja' rolado 300: campo de 60 a 100 (acima do topo) volta 40
        let campo = Bounds::new(point(px(300.), px(60.)), size(px(200.), px(40.)));
        assert_eq!(
            deslocamento_para_revelar(area(), px(-300.), px(1000.), campo),
            Some(px(-260.))
        );
        // dentro da area: nada muda
        let campo = Bounds::new(point(px(300.), px(300.)), size(px(200.), px(40.)));
        assert_eq!(
            deslocamento_para_revelar(area(), px(-300.), px(1000.), campo),
            None
        );
    }

    #[test]
    fn pedido_de_fora_da_area_e_limites_da_rolagem() {
        // a esquerda da area (barra lateral): nao e' desta area
        let lateral = Bounds::new(point(px(10.), px(900.)), size(px(150.), px(40.)));
        assert_eq!(
            deslocamento_para_revelar(area(), px(0.), px(1000.), lateral),
            None
        );
        // nao passa do fim
        let longe = Bounds::new(point(px(300.), px(5000.)), size(px(200.), px(40.)));
        assert_eq!(
            deslocamento_para_revelar(area(), px(0.), px(1000.), longe),
            Some(px(-1000.))
        );
        // campo mais alto que a area: topo a vista
        let alto = Bounds::new(point(px(300.), px(700.)), size(px(200.), px(900.)));
        assert_eq!(
            deslocamento_para_revelar(area(), px(0.), px(1000.), alto),
            Some(px(-600.))
        );
    }

    #[test]
    fn passos_do_teclado_respeitam_topo_e_fim() {
        let (max, alt) = (px(1000.), px(500.));
        assert_eq!(
            deslocamento_do_passo(Passo::LinhaAbaixo, px(0.), max, alt),
            Some(px(-48.))
        );
        assert_eq!(
            deslocamento_do_passo(Passo::PaginaAbaixo, px(0.), max, alt),
            Some(px(-452.))
        );
        assert_eq!(
            deslocamento_do_passo(Passo::PaginaAbaixo, px(-900.), max, alt),
            Some(px(-1000.))
        );
        assert_eq!(
            deslocamento_do_passo(Passo::Fim, px(-1000.), max, alt),
            None
        );
        assert_eq!(
            deslocamento_do_passo(Passo::LinhaAcima, px(0.), max, alt),
            None
        );
        assert_eq!(
            deslocamento_do_passo(Passo::Topo, px(-300.), max, alt),
            Some(px(0.))
        );
        // sem o que rolar (conteudo cabe): nada
        assert_eq!(
            deslocamento_do_passo(Passo::PaginaAbaixo, px(0.), px(0.), alt),
            None
        );
    }
}
