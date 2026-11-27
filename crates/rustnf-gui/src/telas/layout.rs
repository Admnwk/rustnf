// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O esqueleto da janela: barra de titulo com o selo de ambiente, barra lateral, area de conteudo e barra de estado.

use gpui_kit::component::sidebar::{
    Sidebar, SidebarFooter, SidebarGroup, SidebarMenu, SidebarMenuItem,
};
use gpui_kit::component::status_bar::StatusBar;
use gpui_kit::component::{
    h_flex, v_flex, ActiveTheme as _, Icon, IconName, Sizable as _, TitleBar,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, rgb, AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement as _,
    Render, Styled as _, Window,
};

use super::pecas::{cor_da, faixa, icone_da, Negrito as _};
use super::raiz::{Estado, Raiz, Tela};
use super::rolagem::{revelar, Passo};
use super::{
    DescartarConfiguracao, IrAoFim, IrAoTopo, IrParaConsultar, IrParaNotas, IrParaNovaNota,
    IrParaPrevia, PaginaAbaixo, PaginaAcima, RolarAbaixo, RolarAcima, SalvarConfiguracao,
};
use crate::modelo::ambiente::{Ambiente, CORES_DA_DEMONSTRACAO, SELO_DA_DEMONSTRACAO};
use crate::modelo::certificado::Gravidade;
use crate::modelo::nfe_config::selo_do_ambiente_da_nfe;

impl Raiz {
    /// O ambiente em vigor, para o selo.
    fn ambiente_do_selo(&self) -> Ambiente {
        self.info.as_ref().map_or_else(
            || Ambiente::de_config(&self.salvo.sefin.ambiente),
            |i| i.ambiente,
        )
    }

    fn barra_de_titulo(&self) -> TitleBar {
        let ambiente = self.ambiente_do_selo();
        let (rotulo_do_selo, cores) = if self.servicos.demonstracao() {
            (SELO_DA_DEMONSTRACAO, CORES_DA_DEMONSTRACAO)
        } else if self.mostrando_a_nfe() {
            // o selo mostra o ambiente do documento que esta na tela: o da NF-e e' outro campo
            let a = self.ambiente_da_nfe_em_vigor();
            (selo_do_ambiente_da_nfe(a), a.cores())
        } else {
            (ambiente.selo(), ambiente.cores())
        };
        TitleBar::new()
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(div().text_sm().font_semibold().child("rustnf"))
                    .child(
                        div()
                            .text_xs()
                            .opacity(0.6)
                            .child("emissor de notas fiscais"),
                    ),
            )
            .child(
                div()
                    .id("selo-de-ambiente")
                    .mr_3()
                    .px_2p5()
                    .py_0p5()
                    .rounded_full()
                    .text_xs()
                    .font_semibold()
                    .bg(rgb(cores.fundo))
                    .text_color(rgb(cores.texto))
                    .child(rotulo_do_selo),
            )
    }

    fn lateral(&self, cx: &mut Context<Self>) -> Sidebar<SidebarGroup<SidebarMenu>> {
        let tela = self.tela;
        Sidebar::new("lateral")
            .w(px(224.))
            .collapsible(false)
            .child(
                SidebarGroup::new("Menu").child(
                    SidebarMenu::new()
                        .child(
                            SidebarMenuItem::new("Início")
                                .icon(IconName::LayoutDashboard)
                                .active(tela == Tela::Inicio)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.ir_para(Tela::Inicio, cx);
                                })),
                        )
                        .child(
                            SidebarMenuItem::new("Nova nota")
                                .icon(IconName::FileText)
                                .active(tela == Tela::NovaNota)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.ir_para(Tela::NovaNota, cx);
                                })),
                        )
                        .child(
                            SidebarMenuItem::new("Notas")
                                .icon(IconName::Inbox)
                                .active(tela == Tela::Notas)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.ir_para(Tela::Notas, cx);
                                })),
                        )
                        .child(
                            SidebarMenuItem::new("Produtos")
                                .icon(IconName::BookOpen)
                                .active(tela == Tela::Produtos)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.ir_para(Tela::Produtos, cx);
                                })),
                        )
                        .child(
                            SidebarMenuItem::new("Consultar")
                                .icon(IconName::Search)
                                .active(tela == Tela::Consultar)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.ir_para(Tela::Consultar, cx);
                                })),
                        )
                        .child(
                            SidebarMenuItem::new("Prévia")
                                .icon(IconName::ChartPie)
                                .active(tela == Tela::Previa)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.ir_para(Tela::Previa, cx);
                                })),
                        )
                        .child(
                            SidebarMenuItem::new("Configurações")
                                .icon(IconName::Settings)
                                .active(tela == Tela::Configuracoes)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.ir_para(Tela::Configuracoes, cx);
                                })),
                        ),
                ),
            )
            .footer(
                SidebarFooter::new().child(
                    div()
                        .text_xs()
                        .opacity(0.6)
                        .child(format!("Versão {}", env!("CARGO_PKG_VERSION"))),
                ),
            )
    }

    fn barra_de_estado(&self, cx: &mut Context<Self>) -> StatusBar {
        let gravidade = self.cert.gravidade();
        let cor = cor_da(gravidade, cx);
        let tema = cx.theme();
        StatusBar::new()
            .left(
                h_flex()
                    .id("estado-do-certificado")
                    .gap_1p5()
                    .items_center()
                    .child(Icon::new(icone_da(gravidade)).small().text_color(cor))
                    .child(div().text_color(cor).child(self.cert.texto())),
            )
            .right(
                h_flex()
                    .gap_3()
                    .items_center()
                    .text_color(tema.muted_foreground)
                    .when(self.servicos.cofre_desligado(), |d| {
                        d.child(div().child("Cofre de senhas desligado"))
                    })
                    .when_some(self.info.as_ref(), |d, i| {
                        d.child(div().child(if i.nome.is_empty() {
                            "sem arquivo".to_string()
                        } else if i.novo {
                            format!("{} (novo)", i.nome)
                        } else {
                            i.nome.clone()
                        }))
                    }),
            )
    }

    fn conteudo(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        match &self.estado {
            Estado::Carregando => v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .gap_2()
                .text_color(cx.theme().muted_foreground)
                .child(Icon::new(IconName::LoaderCircle).large())
                .child("Lendo a configuração...")
                .into_any_element(),
            Estado::Falha(texto) => {
                let texto = texto.clone();
                v_flex()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .p_8()
                    .child(
                        v_flex()
                            .max_w(px(560.))
                            .gap_3()
                            .child(div().text_xl().font_semibold().child("Não foi possível abrir a configuração"))
                            .child(faixa(Gravidade::Erro, texto, cx))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("Corrija o arquivo (o mesmo que a linha de comando usa) ou abra o programa com --config <arquivo>."),
                            )
                            .child(
                                gpui_kit::component::button::Button::new("tentar-de-novo")
                                    .label("Tentar de novo")
                                    .outline()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.carregar(window, cx);
                                    })),
                            ),
                    )
                    .into_any_element()
            }
            Estado::Pronto => match self.tela {
                Tela::Inicio => self.tela_inicio(window, cx),
                Tela::NovaNota => self.tela_nova_nota(cx),
                Tela::Notas => self.tela_notas(cx),
                Tela::Consultar => self.tela_consultar(cx),
                Tela::Produtos => self.produtos.clone().into_any_element(),
                Tela::Previa => self.previa.clone().into_any_element(),
                Tela::Configuracoes => self.tela_configuracoes(window, cx),
            },
        }
    }
}

impl Render for Raiz {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let fundo = cx.theme().background;
        let texto = cx.theme().foreground;
        let titulo = self.barra_de_titulo();
        let lateral = self.lateral(cx);
        let conteudo = self.conteudo(window, cx);
        let rolagem = self.rolagem_da_tela(cx);
        let estado = self.barra_de_estado(cx);
        v_flex()
            .id("raiz")
            .key_context("Raiz")
            .size_full()
            .bg(fundo)
            .text_color(texto)
            .on_action(cx.listener(|this, _: &SalvarConfiguracao, window, cx| {
                if this.tela == Tela::Configuracoes {
                    this.salvar(window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &DescartarConfiguracao, window, cx| {
                if this.tela == Tela::Configuracoes {
                    this.descartar(window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &IrParaNovaNota, _, cx| {
                this.ir_para(Tela::NovaNota, cx);
            }))
            .on_action(cx.listener(|this, _: &IrParaNotas, _, cx| {
                this.ir_para(Tela::Notas, cx);
            }))
            .on_action(cx.listener(|this, _: &IrParaConsultar, _, cx| {
                this.ir_para(Tela::Consultar, cx);
            }))
            .on_action(cx.listener(|this, _: &IrParaPrevia, _, cx| {
                this.ir_para(Tela::Previa, cx);
            }))
            .on_action(cx.listener(|this, _: &RolarAcima, _, cx| {
                if !this.rolar_tela(Passo::LinhaAcima, cx) {
                    cx.propagate();
                }
            }))
            .on_action(cx.listener(|this, _: &RolarAbaixo, _, cx| {
                if !this.rolar_tela(Passo::LinhaAbaixo, cx) {
                    cx.propagate();
                }
            }))
            .on_action(cx.listener(|this, _: &PaginaAcima, _, cx| {
                if !this.rolar_tela(Passo::PaginaAcima, cx) {
                    cx.propagate();
                }
            }))
            .on_action(cx.listener(|this, _: &PaginaAbaixo, _, cx| {
                if !this.rolar_tela(Passo::PaginaAbaixo, cx) {
                    cx.propagate();
                }
            }))
            .on_action(cx.listener(|this, _: &IrAoTopo, _, cx| {
                if !this.rolar_tela(Passo::Topo, cx) {
                    cx.propagate();
                }
            }))
            .on_action(cx.listener(|this, _: &IrAoFim, _, cx| {
                if !this.rolar_tela(Passo::Fim, cx) {
                    cx.propagate();
                }
            }))
            .child(titulo)
            .child(h_flex().flex_1().min_h_0().child(lateral).child(
                div().flex_1().min_w_0().h_full().child(match rolagem {
                    // o campo que ganha o foco aparece na tela
                    Some(r) => revelar(&r, conteudo).into_any_element(),
                    None => conteudo,
                }),
            ))
            .child(estado)
    }
}
