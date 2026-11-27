// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O desenho do dialogo das NF-e recebidas: o formulario de cada operacao e o desfecho de cada resposta. So' desenho; o
//! estado e o envio moram em `recebidas_dialogo`.

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::Textarea;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Icon, IconName};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, AnyElement, App, Entity, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, StatefulInteractiveElement as _, Styled as _, TestSupportExt as _,
};

use super::{marcacao_da_operacao, DialogoRecebidas, Etapa, OperacaoRecebida, Resultado};
use crate::modelo::certificado::Gravidade;
use crate::modelo::nfe_eventos::DesfechoNfe;
use crate::modelo::nfe_recebidas_manifestar::{AlvoDaResposta, Resposta, EFEITO_DA_RESPOSTA};
use crate::modelo::nfe_recebidas_resultados::{DesfechoBaixa, DesfechoBusca};
use crate::telas::nfe_dialogo::selo_do_dialogo;
use crate::telas::pecas::{campo, faixa, linha_de_resumo, Negrito as _};

/// O efeito da baixa que registra a ciencia, para o alto do dialogo.
const EFEITO_DA_CIENCIA: &str = "Baixar a nota completa registra a ciência da operação na SEFAZ, em nome da sua empresa (é um evento fiscal). A ciência só diz \"vi a nota\": depois você ainda pode confirmar ou desconhecer.";

impl DialogoRecebidas {
    pub(super) fn titulo(&self) -> String {
        let fornecedor = self.alvo.as_ref().map_or("", |a| a.fornecedor.as_str());
        match self.operacao {
            OperacaoRecebida::Buscar => "Buscar notas novas".into(),
            OperacaoRecebida::Baixar => "Baixar a nota completa".into(),
            OperacaoRecebida::Responder if fornecedor.is_empty() => "Responder à nota".into(),
            OperacaoRecebida::Responder => format!("Responder à nota de {fornecedor}"),
        }
    }

    pub(super) fn rotulo_do_botao(&self, producao: bool) -> &'static str {
        match (self.operacao, self.formulario.escolha) {
            (OperacaoRecebida::Responder, Some(r)) => r.enviar(producao),
            (OperacaoRecebida::Responder, None) => "Enviar a resposta",
            (OperacaoRecebida::Baixar, _) if self.baixa_registra_ciencia() && producao => {
                "Dar ciência e baixar em produção"
            }
            (OperacaoRecebida::Baixar, _) if self.baixa_registra_ciencia() => {
                "Dar ciência e baixar"
            }
            _ => "Baixar a nota completa",
        }
    }

    pub(super) fn trabalhando_texto(&self, etapa: Etapa) -> &'static str {
        match (etapa, self.operacao, self.formulario.escolha) {
            (Etapa::Certificado, _, _) => "Abrindo o certificado...",
            (Etapa::Enviando, OperacaoRecebida::Buscar, _) => {
                "Buscando notas novas no Ambiente Nacional..."
            }
            (Etapa::Enviando, OperacaoRecebida::Baixar, _) => {
                "Baixando a nota completa do Ambiente Nacional..."
            }
            (Etapa::Enviando, OperacaoRecebida::Responder, Some(r)) => r.enviando(),
            (Etapa::Enviando, OperacaoRecebida::Responder, None) => "Enviando...",
        }
    }

    fn resumo_da_nota(&self, a: &AlvoDaResposta, cx: &App) -> AnyElement {
        v_flex()
            .id("recebidas-resumo-da-nota")
            .gap_1p5()
            .p_3()
            .rounded_md()
            .border_1()
            .border_color(cx.theme().border)
            .child(linha_de_resumo(
                "Fornecedor",
                if a.fornecedor.is_empty() {
                    "-".to_string()
                } else {
                    a.fornecedor.clone()
                },
                cx,
            ))
            .child(linha_de_resumo(
                "Nota",
                format!(
                    "R$ {}, emitida em {} ({})",
                    a.valor,
                    a.data,
                    a.nota.rotulo()
                ),
                cx,
            ))
            .child(linha_de_resumo(
                "Sua resposta até agora",
                a.resposta_dada.rotulo().to_string(),
                cx,
            ))
            .into_any_element()
    }

    fn marcacao(&self, dialogo: &Entity<Self>, marcado: bool) -> AnyElement {
        let d = dialogo.clone();
        Checkbox::new("recebidas-entendo-efeito-fiscal")
            .label(marcacao_da_operacao(self.operacao))
            .checked(marcado)
            .on_click(move |marcou: &bool, window, cx| {
                d.update(cx, |c, cx| {
                    match c.operacao {
                        OperacaoRecebida::Baixar => c.baixa_entendida = *marcou,
                        _ => c.formulario.efeito_fiscal_entendido = *marcou,
                    }
                    cx.notify();
                });
                window.refresh();
            })
            .into_any_element()
    }

    /// Uma das quatro respostas: a frase, quando usar e, se a SEFAZ recusaria, por que esta desligada.
    fn opcao(
        &self,
        dialogo: &Entity<Self>,
        i: usize,
        r: Resposta,
        a: &AlvoDaResposta,
        cx: &App,
    ) -> AnyElement {
        let tema = cx.theme();
        let (borda, acento, muted) = (tema.border, tema.primary, tema.muted_foreground);
        let escolhida = self.formulario.escolha == Some(r);
        let recusada = a.recusada(r);
        let d = dialogo.clone();
        div()
            .id(SharedString::from(format!("recebidas-resposta-{i}")))
            // com o suporte de teste ligado, a opcao pode ser achada e clicada pelos testes da janela
            .test_support()
            .w_full()
            .p_3()
            .rounded_md()
            .border_1()
            .border_color(if escolhida { acento } else { borda })
            .when(escolhida, |d| d.bg(acento.opacity(0.08)))
            .when(recusada.is_some(), |d| d.opacity(0.55))
            .when(recusada.is_none(), |d| d.cursor_pointer())
            .child(
                h_flex()
                    .gap_3()
                    .items_start()
                    .child(
                        div()
                            .mt_0p5()
                            .size_4()
                            .flex_shrink_0()
                            .rounded_full()
                            .border_1()
                            .border_color(if escolhida { acento } else { muted })
                            .when(escolhida, |d| d.bg(acento)),
                    )
                    .child(
                        v_flex()
                            .gap_1()
                            .flex_1()
                            .min_w_0()
                            .child(div().text_sm().font_semibold().child(r.rotulo()))
                            .child(div().text_xs().text_color(muted).child(r.quando_usar()))
                            .children(recusada.map(|t| {
                                div()
                                    .id(SharedString::from(format!(
                                        "recebidas-resposta-recusada-{i}"
                                    )))
                                    .text_xs()
                                    .font_semibold()
                                    .text_color(cx.theme().warning)
                                    .child(t)
                            })),
                    ),
            )
            .on_click(move |_, window, cx| {
                d.update(cx, |c, cx| c.escolher(r, cx));
                window.refresh();
            })
            .into_any_element()
    }

    pub(super) fn formulario_do_dialogo(&self, dialogo: &Entity<Self>, cx: &App) -> AnyElement {
        let producao = self.producao();
        let Some(a) = self.alvo.clone() else {
            return div()
                .text_sm()
                .child("Escolha uma nota.")
                .into_any_element();
        };
        match self.operacao {
            OperacaoRecebida::Buscar => div().text_sm().child("Buscando...").into_any_element(),
            OperacaoRecebida::Baixar => {
                let ciencia = self.baixa_registra_ciencia();
                let mut c = v_flex()
                    .gap_3()
                    .child(selo_do_dialogo(
                        self.ambiente(),
                        self.demonstracao(),
                        ciencia.then_some(EFEITO_DA_CIENCIA),
                    ))
                    .child(self.resumo_da_nota(&a, cx))
                    .child(
                        div().id("recebidas-baixar-explicacao").text_sm().child(if ciencia {
                            "O Ambiente Nacional só entrega a nota completa a quem já respondeu sobre ela. Por isso, para baixar, o programa registra primeiro a ciência da operação e só então busca a nota."
                        } else {
                            "Sua empresa já respondeu sobre esta nota, então o Ambiente Nacional pode entregá-la completa. Nada será registrado na SEFAZ: o programa só busca a nota."
                        }),
                    );
                if producao && ciencia {
                    c = c.child(self.marcacao(dialogo, self.baixa_entendida));
                }
                c.into_any_element()
            }
            OperacaoRecebida::Responder => {
                let mut c = v_flex()
                    .gap_3()
                    .child(selo_do_dialogo(
                        self.ambiente(),
                        self.demonstracao(),
                        Some(EFEITO_DA_RESPOSTA),
                    ))
                    .child(self.resumo_da_nota(&a, cx))
                    .child(
                        div()
                            .text_sm()
                            .font_semibold()
                            .child("O que você quer responder?"),
                    )
                    .child(
                        v_flex().id("recebidas-respostas").gap_2().children(
                            Resposta::TODAS
                                .iter()
                                .enumerate()
                                .map(|(i, r)| self.opcao(dialogo, i, *r, &a, cx)),
                        ),
                    );
                if self
                    .formulario
                    .escolha
                    .is_some_and(Resposta::pede_justificativa)
                {
                    c = c.child(campo(
                        "Justificativa",
                        true,
                        Some(SharedString::from(self.formulario.contador())),
                        self.formulario.erro_da_justificativa(self.tocada),
                        div()
                            .id("recebidas-dialogo-texto")
                            .child(Textarea::new(&self.texto)),
                    ));
                }
                if producao {
                    c = c.child(self.marcacao(dialogo, self.formulario.efeito_fiscal_entendido));
                }
                c.into_any_element()
            }
        }
    }

    fn botao(rotulo: &'static str, id: &'static str, icone: IconName, primario: bool) -> Button {
        let b = Button::new(id).label(rotulo).icon(icone);
        if primario {
            b.primary()
        } else {
            b.outline()
        }
    }

    fn botao_de_tentar(&self, dialogo: &Entity<Self>) -> Button {
        let d = dialogo.clone();
        Self::botao(
            "Tentar de novo",
            "recebidas-tentar-de-novo",
            IconName::RefreshCw,
            true,
        )
        .on_click(move |_, window, cx| {
            d.update(cx, |c, cx| c.tentar_de_novo(window, cx));
        })
    }

    fn botao_de_voltar(&self, dialogo: &Entity<Self>) -> Button {
        let d = dialogo.clone();
        Self::botao(
            "Voltar ao formulário",
            "recebidas-voltar",
            IconName::ChevronLeft,
            false,
        )
        .on_click(move |_, _, cx| {
            d.update(cx, |c, cx| c.voltar(cx));
        })
    }

    fn desfecho_da_resposta(
        &self,
        dialogo: &Entity<Self>,
        d: &DesfechoNfe,
        r: Resposta,
        cx: &App,
    ) -> AnyElement {
        let muted = cx.theme().muted_foreground;
        match d {
            DesfechoNfe::Registrado {
                protocolo, aviso, ..
            } => v_flex()
                .id("recebidas-registrado")
                .gap_3()
                .child(faixa(Gravidade::Ok, r.registrada(), cx))
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
                .id("recebidas-rejeitado")
                .gap_3()
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(Icon::new(IconName::CircleAlert).text_color(cx.theme().danger))
                        .child(div().font_semibold().child("A SEFAZ recusou a resposta")),
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
                .child(h_flex().child(self.botao_de_voltar(dialogo)))
                .into_any_element(),
            DesfechoNfe::NaoRegistrado => v_flex()
                .id("recebidas-nao-registrado")
                .gap_3()
                .child(faixa(
                    Gravidade::Neutro,
                    "A SEFAZ confirma que esta resposta não foi registrada: nada mudou. Pode enviar de novo.",
                    cx,
                ))
                .child(h_flex().child(self.botao_de_tentar(dialogo)))
                .into_any_element(),
            DesfechoNfe::SemResposta { texto } => v_flex()
                .id("recebidas-sem-resposta")
                .gap_3()
                .child(faixa(Gravidade::Aviso, texto.clone(), cx))
                .child(h_flex().child(self.botao_de_tentar(dialogo)))
                .into_any_element(),
            DesfechoNfe::Recusado { texto } => v_flex()
                .id("recebidas-recusado")
                .gap_3()
                .child(faixa(Gravidade::Erro, format!("Nada foi enviado. {texto}"), cx))
                .child(h_flex().child(self.botao_de_voltar(dialogo)))
                .into_any_element(),
        }
    }

    pub(super) fn desfecho(&self, dialogo: &Entity<Self>, r: &Resultado, cx: &App) -> AnyElement {
        match r {
            Resultado::Busca(d) => {
                let sem_resposta = matches!(d, DesfechoBusca::SemResposta { .. });
                v_flex()
                    .id("recebidas-busca-resultado")
                    .gap_3()
                    .child(faixa(
                        d.gravidade(),
                        format!("{}. {}", d.titulo(), d.texto()),
                        cx,
                    ))
                    .when(sem_resposta, |c| {
                        c.child(h_flex().child(self.botao_de_tentar(dialogo)))
                    })
                    .into_any_element()
            }
            Resultado::Baixa(d) => {
                let tentar = matches!(d, DesfechoBaixa::SemResposta { .. });
                v_flex()
                    .id("recebidas-baixa-resultado")
                    .gap_3()
                    .child(faixa(
                        d.gravidade(),
                        format!("{}. {}", d.titulo(), d.texto()),
                        cx,
                    ))
                    .when(tentar, |c| {
                        c.child(h_flex().child(self.botao_de_tentar(dialogo)))
                    })
                    .into_any_element()
            }
            Resultado::Resposta(d, r) => self.desfecho_da_resposta(dialogo, d, *r, cx),
        }
    }
}
