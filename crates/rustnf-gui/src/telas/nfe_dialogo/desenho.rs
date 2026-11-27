// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O desenho do dialogo da NF-e: o formulario de cada pedido e o desfecho de cada resposta. So' desenho; o estado e o
//! envio moram em `nfe_dialogo`.

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, Textarea};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Icon, IconName};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, rgb, AnyElement, App, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, SharedString, Styled as _,
};

use super::Etapa;
use super::{DialogoNfe, Operacao, Resposta};
use crate::modelo::ambiente::{Ambiente, CORES_DA_DEMONSTRACAO, SELO_DA_DEMONSTRACAO};
use crate::modelo::certificado::Gravidade;
use crate::modelo::nfe_config::{ambiente_da_nfe, selo_do_ambiente_da_nfe};
use crate::modelo::nfe_eventos::{
    DesfechoNfe, PedidoNfe, O_QUE_A_CARTA_NAO_CORRIGE, TEXTO_OFICIAL_DA_CARTA,
};
use crate::telas::pecas::{campo, faixa, linha_de_resumo, Negrito as _};

impl DialogoNfe {
    // ---------------------------------------------------------------- desenho

    pub(super) fn selo(&self) -> AnyElement {
        let ambiente = ambiente_da_nfe(&self.config.nfe);
        let cores = ambiente.cores();
        let demonstracao = self.servicos.demonstracao_nfe();
        let pedido = self.operacao.pedido();
        v_flex()
            .gap_2()
            .child(
                h_flex().gap_2().items_center().child(
                    div()
                        .id("nfe-selo-do-dialogo")
                        .px_3()
                        .py_1()
                        .rounded_full()
                        .text_sm()
                        .font_semibold()
                        .bg(rgb(cores.fundo))
                        .text_color(rgb(cores.texto))
                        .child(selo_do_ambiente_da_nfe(ambiente)),
                ),
            )
            .when(demonstracao, |d| {
                d.child(
                    div()
                        .id("nfe-aviso-de-demonstracao")
                        .px_3()
                        .py_1()
                        .rounded_md()
                        .text_sm()
                        .font_semibold()
                        .bg(rgb(CORES_DA_DEMONSTRACAO.fundo))
                        .text_color(rgb(CORES_DA_DEMONSTRACAO.texto))
                        .child(SELO_DA_DEMONSTRACAO),
                )
            })
            .when_some(pedido, |d, p| {
                d.child(div().text_sm().child(p.efeito())).when(
                    ambiente == Ambiente::Restrita,
                    |d| {
                        d.child(
                            div()
                                .text_sm()
                                .font_semibold()
                                .child("Homologação: ambiente de testes, sem efeito fiscal."),
                        )
                    },
                )
            })
            .into_any_element()
    }

    pub(super) fn resumo(&self, cx: &App) -> Option<AnyElement> {
        let a = self.alvo.as_ref()?;
        Some(
            v_flex()
                .id("nfe-resumo-da-nota")
                .gap_1p5()
                .p_3()
                .rounded_md()
                .border_1()
                .border_color(cx.theme().border)
                .child(linha_de_resumo(
                    "Nota",
                    format!("nº {}, série {}", a.numero, a.serie),
                    cx,
                ))
                .child(linha_de_resumo(
                    "Destinatário",
                    if a.destinatario.is_empty() {
                        "-".into()
                    } else {
                        a.destinatario.clone()
                    },
                    cx,
                ))
                .child(linha_de_resumo("Valor", format!("R$ {}", a.valor), cx))
                .into_any_element(),
        )
    }

    pub(super) fn marcacao(
        &self,
        dialogo: &Entity<Self>,
        marcado: bool,
        rotulo: &'static str,
    ) -> AnyElement {
        let d = dialogo.clone();
        Checkbox::new("nfe-entendo-efeito-fiscal")
            .label(rotulo)
            .checked(marcado)
            .on_click(move |marcou: &bool, window, cx| {
                d.update(cx, |c, cx| {
                    match c.operacao {
                        Operacao::Cancelar => c.cancelar.efeito_fiscal_entendido = *marcou,
                        Operacao::Corrigir => c.corrigir.efeito_fiscal_entendido = *marcou,
                        Operacao::Inutilizar => c.inutilizar.efeito_fiscal_entendido = *marcou,
                        _ => c.retomar_entendido = *marcou,
                    }
                    cx.notify();
                });
                window.refresh();
            })
            .into_any_element()
    }

    pub(super) fn campo_de_texto(
        &self,
        rotulo: &'static str,
        erro: Option<String>,
        contador: String,
    ) -> AnyElement {
        campo(
            rotulo,
            true,
            Some(SharedString::from(contador)),
            erro,
            div()
                .id("nfe-dialogo-texto")
                .child(Textarea::new(&self.texto)),
        )
        .into_any_element()
    }

    pub(super) fn formulario(&self, dialogo: &Entity<Self>, cx: &App) -> AnyElement {
        let producao = self.producao();
        let mut c = v_flex().gap_3().child(self.selo());
        if let Some(r) = self.resumo(cx) {
            c = c.child(r);
        }
        let marcado = match self.operacao {
            Operacao::Cancelar => self.cancelar.efeito_fiscal_entendido,
            Operacao::Corrigir => self.corrigir.efeito_fiscal_entendido,
            Operacao::Inutilizar => self.inutilizar.efeito_fiscal_entendido,
            _ => self.retomar_entendido,
        };
        let marcacao = self.operacao.pedido().map_or(
            "Entendo que as notas pendentes podem ser autorizadas pela SEFAZ",
            PedidoNfe::marcacao_de_producao,
        );
        match self.operacao {
            Operacao::Cancelar => {
                if let Some(a) = &self.aviso_de_prazo {
                    // o prazo nacional so' avisa: quem decide e' a SEFAZ da UF (MOC 7.00, regra 2P12-14)
                    c = c.child(
                        faixa(Gravidade::Aviso, format!("Atenção: {a}."), cx)
                            .id("nfe-aviso-do-prazo"),
                    );
                }
                c = c.child(self.campo_de_texto(
                    "Justificativa",
                    self.cancelar.erro(self.tocada),
                    self.cancelar.contador(),
                ));
            }
            Operacao::Corrigir => {
                let d = dialogo.clone();
                let aberto = self.texto_oficial_aberto;
                c = c
                    .child(
                        faixa(Gravidade::Neutro, self.corrigir.aviso_das_cartas(), cx)
                            .id("nfe-aviso-das-cartas"),
                    )
                    // o campo vem logo depois do aviso: o que a carta nao corrige e o texto oficial ficam abaixo
                    .child(self.campo_de_texto(
                        "Correção",
                        self.corrigir.erro(self.tocada),
                        self.corrigir.contador(),
                    ))
                    .child(self.o_que_a_carta_nao_corrige(cx))
                    .child(
                        h_flex().child(
                            Button::new("nfe-ver-texto-oficial")
                                .label(if aberto {
                                    "Esconder o texto oficial"
                                } else {
                                    "Ver o texto oficial"
                                })
                                .icon(IconName::Eye)
                                .outline()
                                .on_click(move |_, window, cx| {
                                    d.update(cx, |c, cx| {
                                        c.texto_oficial_aberto = !c.texto_oficial_aberto;
                                        cx.notify();
                                    });
                                    window.refresh();
                                }),
                        ),
                    )
                    .when(aberto, |d| {
                        d.child(
                            div()
                                .id("nfe-texto-oficial")
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(TEXTO_OFICIAL_DA_CARTA),
                        )
                    });
            }
            Operacao::Inutilizar => {
                c = c
                    .child(
                        h_flex()
                            .gap_3()
                            .items_start()
                            .child(div().w(px(150.)).child(campo(
                                "Série",
                                false,
                                None,
                                None,
                                Input::new(&self.serie).id("nfe-inutilizar-serie"),
                            )))
                            .child(div().w(px(150.)).child(campo(
                                "Do número",
                                true,
                                None,
                                None,
                                Input::new(&self.inicio).id("nfe-inutilizar-inicio"),
                            )))
                            .child(div().w(px(200.)).child(campo(
                                "Até o número",
                                false,
                                None,
                                None,
                                Input::new(&self.fim).id("nfe-inutilizar-fim"),
                            ))),
                    )
                    .children(
                        self.inutilizar
                            .erro_da_faixa()
                            .map(|e| faixa(Gravidade::Erro, e, cx).id("nfe-erro-da-faixa")),
                    )
                    .child(self.campo_de_texto(
                        "Justificativa",
                        self.inutilizar.erro_da_justificativa(self.tocada),
                        self.inutilizar.contador(),
                    ));
            }
            Operacao::Retomar => {
                c = c.child(
                    div().id("nfe-retomar-explicacao").text_sm().child(format!(
                        "{} sem resposta da SEFAZ. Para cada uma, o programa consulta a mesma chave: se a SEFAZ já a autorizou, a pasta é posta em dia; se a SEFAZ não a recebeu, reenvia a mesma nota que está guardada, assinada. Nunca cria uma nota nova no lugar.",
                        crate::modelo::nfe_resultados::contagem(
                            self.a_retomar,
                            "nota está",
                            "notas estão"
                        )
                    )),
                );
            }
            Operacao::Conferir => {}
        }
        if producao && self.operacao != Operacao::Conferir {
            c = c.child(self.marcacao(dialogo, marcado, marcacao));
        }
        c.into_any_element()
    }

    pub(super) fn o_que_a_carta_nao_corrige(&self, cx: &App) -> AnyElement {
        let muted = cx.theme().muted_foreground;
        v_flex()
            .id("nfe-o-que-a-carta-nao-corrige")
            .gap_1()
            .child(
                div()
                    .text_sm()
                    .font_semibold()
                    .child("A carta NÃO pode corrigir:"),
            )
            .children(
                O_QUE_A_CARTA_NAO_CORRIGE
                    .iter()
                    .map(|t| div().text_sm().text_color(muted).child(format!("• {t}"))),
            )
            .into_any_element()
    }

    pub(super) fn botao(
        rotulo: &'static str,
        id: &'static str,
        icone: IconName,
        primario: bool,
    ) -> Button {
        let b = Button::new(id).label(rotulo).icon(icone);
        if primario {
            b.primary()
        } else {
            b.outline()
        }
    }

    pub(super) fn desfecho_do_pedido(
        &self,
        dialogo: &Entity<Self>,
        d: &DesfechoNfe,
        cx: &App,
    ) -> AnyElement {
        let muted = cx.theme().muted_foreground;
        let pedido = self.operacao.pedido().unwrap_or(PedidoNfe::Cancelar);
        let (d_tentar, d_conferir, d_voltar) = (dialogo.clone(), dialogo.clone(), dialogo.clone());
        let tentar = Self::botao(
            "Tentar de novo",
            "nfe-tentar-de-novo",
            IconName::RefreshCw,
            true,
        )
        .on_click(move |_, window, cx| {
            d_tentar.update(cx, |c, cx| c.tentar_de_novo(window, cx));
        });
        let conferir = Self::botao(
            "Conferir na SEFAZ",
            "nfe-conferir-pedido",
            IconName::Search,
            false,
        )
        .on_click(move |_, window, cx| {
            d_conferir.update(cx, |c, cx| c.conferir_pedido(window, cx));
        });
        let voltar = Self::botao(
            "Voltar ao formulário",
            "nfe-voltar",
            IconName::ChevronLeft,
            false,
        )
        .on_click(move |_, _, cx| {
            d_voltar.update(cx, |c, cx| c.voltar(cx));
        });
        match d {
            DesfechoNfe::Registrado {
                protocolo,
                recuperado,
                aviso,
            } => v_flex()
                .id("nfe-registrado")
                .gap_3()
                .child(faixa(Gravidade::Ok, pedido.registrado(), cx))
                .when(*recuperado, |c| {
                    c.child(div().text_sm().text_color(muted).child(
                        "O pedido já estava registrado na SEFAZ (de um envio cuja resposta se perdeu) e foi achado pela consulta.",
                    ))
                })
                .children(aviso.clone().map(|a| faixa(Gravidade::Aviso, a, cx)))
                .when(!protocolo.is_empty(), |c| {
                    c.child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .child(format!("Protocolo {protocolo}")),
                    )
                })
                .into_any_element(),
            DesfechoNfe::Rejeitado {
                codigo,
                descricao,
                orientacao,
            } => v_flex()
                .id("nfe-rejeitado")
                .gap_3()
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(Icon::new(IconName::CircleAlert).text_color(cx.theme().danger))
                        .child(div().font_semibold().child("A SEFAZ recusou o pedido")),
                )
                .child(
                    v_flex()
                        .gap_0p5()
                        .p_3()
                        .rounded_md()
                        .border_1()
                        .border_color(cx.theme().danger.opacity(0.45))
                        .child(div().text_sm().font_semibold().child(descricao.clone()))
                        .when(!codigo.is_empty(), |c| {
                            c.child(div().text_xs().text_color(muted).child(format!("Código {codigo}")))
                        }),
                )
                .child(div().text_sm().text_color(muted).child(orientacao.clone()))
                .child(h_flex().child(voltar))
                .into_any_element(),
            DesfechoNfe::NaoRegistrado => v_flex()
                .id("nfe-nao-registrado")
                .gap_3()
                .child(faixa(
                    Gravidade::Neutro,
                    "A SEFAZ confirma que este pedido não foi registrado: nada mudou. Pode pedir de novo.",
                    cx,
                ))
                .child(h_flex().child(tentar))
                .into_any_element(),
            DesfechoNfe::SemResposta { texto } => v_flex()
                .id("nfe-sem-resposta")
                .gap_3()
                .child(faixa(Gravidade::Aviso, texto.clone(), cx))
                .child(div().text_sm().text_color(muted).child(
                    "Tentar de novo reenvia o MESMO pedido, o mesmo evento com a mesma sequência: a SEFAZ não o registra duas vezes (recusa a repetição, e a tela mostra isso). Conferir na SEFAZ só consulta a nota, sem reenviar.",
                ))
                .child(if self.alvo.is_some() {
                    h_flex().gap_2().child(tentar).child(conferir)
                } else {
                    h_flex().gap_2().child(tentar)
                })
                .into_any_element(),
            DesfechoNfe::Recusado { texto } => v_flex()
                .id("nfe-recusado")
                .gap_3()
                .child(faixa(Gravidade::Erro, format!("Nada foi enviado. {texto}"), cx))
                .child(h_flex().child(voltar))
                .into_any_element(),
        }
    }

    pub(super) fn desfecho(&self, dialogo: &Entity<Self>, r: &Resposta, cx: &App) -> AnyElement {
        let muted = cx.theme().muted_foreground;
        match r {
            Resposta::Pedido(d) => self.desfecho_do_pedido(dialogo, d, cx),
            Resposta::Consulta(c) => v_flex()
                .id("nfe-consulta")
                .gap_2()
                .child(faixa(c.gravidade, format!("{}. {}", c.titulo, c.texto), cx))
                .when(!c.protocolo.is_empty(), |d| {
                    d.child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .child(format!("Protocolo {}", c.protocolo)),
                    )
                })
                .when(!c.codigo.is_empty(), |d| {
                    d.child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .child(format!("Código {}", c.codigo)),
                    )
                })
                .into_any_element(),
            Resposta::Retomada(r) => v_flex()
                .id("nfe-retomada")
                .gap_2()
                .child(faixa(r.gravidade, r.texto.clone(), cx))
                .children(r.linhas.iter().map(|l| {
                    faixa(
                        l.gravidade,
                        format!("Nota nº {}: {}.", l.numero, l.texto),
                        cx,
                    )
                }))
                .into_any_element(),
        }
    }

    pub(super) fn trabalhando_texto(&self, etapa: Etapa) -> &'static str {
        match (etapa, self.operacao) {
            (Etapa::Certificado, _) => "Abrindo o certificado...",
            (Etapa::Enviando, Operacao::Conferir) => "Consultando a nota na SEFAZ...",
            (Etapa::Enviando, Operacao::Retomar) => "Retomando as notas na SEFAZ...",
            (Etapa::Enviando, op) => op.pedido().map_or("Enviando...", PedidoNfe::enviando),
        }
    }
}
