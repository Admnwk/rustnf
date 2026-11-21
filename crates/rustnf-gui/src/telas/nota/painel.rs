// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O painel fixo ao lado do formulario: pendencias (clicaveis), resumo de valores, previa do IBS/CBS e o botao
//! Emitir com o motivo quando esta desligado. Em janela estreita ele vai para baixo do formulario.

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{
    h_flex, v_flex, ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _,
};
use gpui_kit::{
    div, px, AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, StatefulInteractiveElement as _, Styled as _,
};

use super::{EstadoDoRascunho, Etapa, Fase, NovaNota};
use crate::modelo::certificado::Gravidade;
use crate::modelo::previa::{mostrar, totais};
use crate::telas::pecas::{
    cor_da, corpo_da_previa, faixa, icone_da, linha_de_numero, titulo_do_bloco,
};

/// Largura do painel quando fica ao lado do formulario.
pub(in crate::telas) const LARGURA_DO_PAINEL: f32 = 340.0;

impl NovaNota {
    /// O painel de conferencia. `ao_lado`: fixo a direita, com rolagem propria; senao, um bloco ao pe do formulario.
    pub(in crate::telas) fn painel(&mut self, ao_lado: bool, cx: &mut Context<Self>) -> AnyElement {
        let tema = cx.theme();
        let borda = tema.border;
        let mut caixa = v_flex().id("painel-da-nota").gap_5().p_5();
        caixa = if ao_lado {
            caixa
                .w(px(LARGURA_DO_PAINEL))
                .flex_shrink_0()
                .h_full()
                .border_l_1()
                .border_color(borda)
                .overflow_y_scroll()
        } else {
            caixa.w_full().rounded_lg().border_1().border_color(borda)
        };
        caixa
            .child(self.bloco_de_emitir(cx))
            .child(self.bloco_de_pendencias(cx))
            .child(self.bloco_de_valores(cx))
            .child(self.bloco_do_ibscbs(cx))
            .into_any_element()
    }

    fn bloco_de_pendencias(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let tema = cx.theme();
        let muted = tema.muted_foreground;
        let hover = tema.accent;
        let pendencias = self.conferencia.pendencias.clone();
        let total = pendencias.len();
        let mut bloco = v_flex().gap_2().child(titulo_do_bloco(
            &if total == 0 {
                "Conferência".to_string()
            } else {
                format!("Conferência ({total})")
            },
            cx,
        ));
        if total == 0 {
            let texto = if self.conferencia.da_biblioteca {
                "Tudo certo. A nota passou na conferência da biblioteca."
            } else {
                "Nada a corrigir por enquanto."
            };
            return bloco
                .child(faixa(Gravidade::Ok, texto, cx).id("conferencia-ok"))
                .into_any_element();
        }
        for (i, p) in pendencias.into_iter().enumerate() {
            let campo = p.campo;
            let mut linha =
                h_flex()
                    .id(SharedString::from(format!("pendencia-{i}")))
                    .gap_2()
                    .items_start()
                    .p_2()
                    .rounded_md()
                    .cursor_pointer()
                    .hover(move |s| s.bg(hover))
                    .child(
                        Icon::new(IconName::CircleAlert)
                            .small()
                            .text_color(cx.theme().danger)
                            .flex_shrink_0()
                            .mt(px(1.)),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_0p5()
                            .child(div().text_sm().child(p.mensagem.clone()))
                            .child(div().text_xs().text_color(muted).child(
                                match (&campo, &p.regra) {
                                    (Some(c), Some(r)) => format!("{} · regra {r}", c.rotulo()),
                                    (Some(c), None) => c.rotulo().to_string(),
                                    (None, Some(r)) => format!("regra {r}"),
                                    (None, None) => "Sem campo nesta tela".to_string(),
                                },
                            )),
                    );
            linha = linha.on_click(cx.listener(move |this, _, window, cx| match campo {
                Some(c) => this.ir_ao_campo(c, window, cx),
                None => {
                    this.mostrar_tudo = true;
                    cx.notify();
                }
            }));
            bloco = bloco.child(linha);
        }
        bloco.into_any_element()
    }

    fn bloco_de_valores(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let t = totais(&self.dados);
        let tem_servico = t.servico.is_some();
        let negativo = |c: i64| format!("- R$ {}", mostrar(c));
        let mut bloco = v_flex()
            .id("resumo-de-valores")
            .gap_1p5()
            .child(titulo_do_bloco("Valores", cx))
            .child(linha_de_numero(
                "Serviço",
                format!("R$ {}", mostrar(t.servico.unwrap_or(0))),
                false,
                cx,
            ));
        if t.desc_incondicionado > 0 {
            bloco = bloco.child(linha_de_numero(
                "Desconto incondicionado",
                negativo(t.desc_incondicionado),
                false,
                cx,
            ));
        }
        if t.desc_condicionado > 0 {
            bloco = bloco.child(linha_de_numero(
                "Desconto condicionado",
                negativo(t.desc_condicionado),
                false,
                cx,
            ));
        }
        if t.retencoes > 0 {
            bloco = bloco.child(linha_de_numero(
                "Retenções federais",
                negativo(t.retencoes),
                false,
                cx,
            ));
        }
        let borda = cx.theme().border;
        bloco
            .child(div().h(px(1.)).w_full().bg(borda).my_1())
            .child(linha_de_numero(
                "Valor líquido",
                if tem_servico {
                    format!("R$ {}", mostrar(t.liquido.unwrap_or(0)))
                } else {
                    "-".into()
                },
                true,
                cx,
            ))
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("Estimado: o ISS retido e o PIS/COFINS retidos não entram nesta conta."),
            )
            .into_any_element()
    }

    fn bloco_do_ibscbs(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let botao = Button::new("abrir-aliquotas")
            .label("Abrir as alíquotas")
            .icon(IconName::Settings)
            .outline()
            .small()
            .on_click(cx.listener(|this, _, _, cx| this.pedir_aliquotas(cx)))
            .into_any_element();
        let corpo = corpo_da_previa(&self.previa_ibs, Some(botao), cx);
        v_flex()
            .gap_2()
            .child(titulo_do_bloco("Prévia do IBS e da CBS", cx))
            .child(corpo)
            .into_any_element()
    }

    fn bloco_de_emitir(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let tema = cx.theme();
        let muted = tema.muted_foreground;
        let editando = matches!(self.fase, Fase::Editando);
        let enviando = matches!(self.fase, Fase::Enviando { .. });
        let pode = editando && self.conferencia.limpa();
        let motivo: Option<String> = match &self.fase {
            Fase::Editando => self.conferencia.motivo(),
            Fase::Enviando { etapa, .. } => Some(
                match etapa {
                    Etapa::AbrindoCertificado => "Abrindo o certificado...",
                    Etapa::Enviando => "Enviando para a Sefin Nacional...",
                }
                .into(),
            ),
            Fase::Pendente { .. } => {
                Some("Há uma tentativa sem resposta: tente de novo ou desista dela.".into())
            }
            Fase::Concluida(_) => None,
        };
        let numeracao = format!(
            "DPS nº {}, série {}",
            self.numero_provisorio,
            self.ctx.serie()
        );
        let rascunho = match self.rascunho {
            EstadoDoRascunho::Salvo => Some("Rascunho salvo"),
            EstadoDoRascunho::Alterado => Some("Salvando o rascunho..."),
            EstadoDoRascunho::Nenhum => None,
        };
        v_flex()
            .gap_3()
            .child(titulo_do_bloco("Emitir", cx))
            .child(div().text_sm().text_color(muted).child(numeracao))
            .child(
                Button::new("emitir")
                    .label("Emitir nota")
                    .icon(IconName::Check)
                    .primary()
                    .loading(enviando)
                    .disabled(!pode)
                    .on_click(cx.listener(|this, _, window, cx| this.pedir_emissao(window, cx))),
            )
            .children(motivo.map(|m| {
                h_flex()
                    .id("motivo-do-botao")
                    .gap_1p5()
                    .items_start()
                    .text_xs()
                    .text_color(if pode {
                        muted
                    } else {
                        cor_da(Gravidade::Aviso, cx)
                    })
                    .child(
                        Icon::new(icone_da(if enviando {
                            Gravidade::Neutro
                        } else {
                            Gravidade::Aviso
                        }))
                        .xsmall()
                        .mt(px(1.)),
                    )
                    .child(div().flex_1().min_w_0().child(m))
            }))
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .id("indicador-do-rascunho")
                            .text_xs()
                            .text_color(muted)
                            .children(rascunho),
                    )
                    .child(
                        Button::new("limpar")
                            .label("Limpar")
                            .icon(IconName::Delete)
                            .ghost()
                            .small()
                            .disabled(!editando || self.dados.vazio(&self.ctx.hoje()))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.pedir_para_limpar(window, cx);
                            })),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(muted)
                    .child("Ctrl+Enter emite, Esc fecha o diálogo."),
            )
            .into_any_element()
    }
}
