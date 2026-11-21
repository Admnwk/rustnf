// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A tela da nota autorizada, o painel de rejeicao da Sefin e o aviso de tentativa sem resposta.

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Icon, IconName, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, rgb, AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _,
};

use super::{Concluida, Fase, NovaNota};
use crate::modelo::certificado::Gravidade;
use crate::modelo::emissao::{MensagemSefin, EXPLICACAO_DE_TENTAR_DE_NOVO};
use crate::telas::pecas::{cabecalho, cartao, faixa, linha_de_resumo, Negrito as _};

/// A chave de acesso em grupos de 5, para ler e para quebrar a linha.
fn chave_em_grupos(chave: &str) -> String {
    chave
        .as_bytes()
        .chunks(5)
        .map(|c| String::from_utf8_lossy(c).into_owned())
        .collect::<Vec<_>>()
        .join(" ")
}

impl NovaNota {
    /// A tela da nota autorizada.
    pub(in crate::telas) fn tela_de_resultado(
        &mut self,
        c: Concluida,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tema = cx.theme();
        let ambiente = self.ambiente();
        let cores = ambiente.cores();
        let demonstracao = self.servicos.demonstracao();
        let sucesso = tema.success;
        let muted = tema.muted_foreground;
        let copiada = self.copiada;
        let mut caixa = cartao(cx).id("nota-autorizada").gap_4().child(
            h_flex()
                .gap_3()
                .items_center()
                .child(Icon::new(IconName::CircleCheck).large().text_color(sucesso))
                .child(
                    v_flex()
                        .gap_0p5()
                        .child(
                            div()
                                .text_xl()
                                .font_semibold()
                                .child(if c.numero.is_empty() {
                                    "NFS-e autorizada".to_string()
                                } else {
                                    format!("NFS-e nº {} autorizada", c.numero)
                                }),
                        )
                        .child(
                            h_flex()
                                .gap_2()
                                .items_center()
                                .child(
                                    div()
                                        .px_2p5()
                                        .py_0p5()
                                        .rounded_full()
                                        .text_xs()
                                        .font_semibold()
                                        .bg(rgb(cores.fundo))
                                        .text_color(rgb(cores.texto))
                                        .child(ambiente.selo()),
                                )
                                .when(demonstracao, |d| {
                                    d.child(
                                        div()
                                            .text_xs()
                                            .text_color(muted)
                                            .child("Demonstração: nada foi enviado de verdade."),
                                    )
                                }),
                        ),
                ),
        );
        if c.recuperada {
            caixa = caixa.child(faixa(
                Gravidade::Neutro,
                "A resposta do envio se perdeu e a nota foi recuperada pela consulta à Sefin. Não houve duplicidade.",
                cx,
            ));
        }
        caixa = caixa
            .child(linha_de_resumo("Número", c.numero.clone(), cx))
            .child(
                v_flex()
                    .gap_1p5()
                    .child(div().text_sm().text_color(muted).child("Chave de acesso"))
                    .child(
                        div()
                            .id("chave-de-acesso")
                            .p_3()
                            .rounded_md()
                            .border_1()
                            .border_color(cx.theme().border)
                            .bg(cx.theme().muted)
                            .text_sm()
                            .child(chave_em_grupos(&c.chave)),
                    )
                    .child(
                        h_flex().child(
                            Button::new("copiar-chave")
                                .label(if copiada {
                                    "Chave copiada"
                                } else {
                                    "Copiar a chave"
                                })
                                .icon(if copiada {
                                    IconName::Check
                                } else {
                                    IconName::Copy
                                })
                                .outline()
                                .small()
                                .on_click(cx.listener(move |this, _, _, cx| this.copiar_chave(cx))),
                        ),
                    ),
            );
        caixa = caixa.child(match &c.aviso {
            Some(a) => faixa(Gravidade::Aviso, a.clone(), cx).id("aviso-da-guarda"),
            None if c.guardada => {
                faixa(Gravidade::Ok, "A nota foi guardada na pasta de notas.", cx)
                    .id("nota-guardada")
            }
            None => faixa(
                Gravidade::Aviso,
                "A nota não foi guardada na pasta de notas. Consulte a nota pela chave de acesso.",
                cx,
            )
            .id("aviso-da-guarda"),
        });
        for a in &c.alertas {
            caixa = caixa.child(self.mensagem_da_sefin(a, Gravidade::Aviso, cx));
        }
        caixa = caixa.child(
            h_flex().child(
                Button::new("nova-nota-limpa")
                    .label("Nova nota")
                    .icon(IconName::Plus)
                    .primary()
                    .on_click(cx.listener(|this, _, window, cx| this.nova_nota(window, cx))),
            ),
        );
        div()
            .id("resultado-da-nota")
            .size_full()
            .overflow_y_scroll()
            .child(
                v_flex()
                    .w_full()
                    .max_w(px(680.))
                    .mx_auto()
                    .gap_5()
                    .px_6()
                    .py_8()
                    .child(cabecalho(
                        "Nova nota",
                        "A Sefin Nacional autorizou a nota.",
                        cx,
                    ))
                    .child(caixa),
            )
            .into_any_element()
    }

    /// Uma mensagem da Sefin: a descricao em destaque, o codigo e o complemento em segundo plano.
    pub(in crate::telas) fn mensagem_da_sefin(
        &self,
        m: &MensagemSefin,
        gravidade: Gravidade,
        cx: &mut Context<Self>,
    ) -> AnyElement {
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
        let cor = crate::telas::pecas::cor_da(gravidade, cx);
        h_flex()
            .gap_2()
            .items_start()
            .p_3()
            .rounded_md()
            .border_1()
            .border_color(cor.opacity(0.45))
            .bg(cor.opacity(0.10))
            .child(
                Icon::new(crate::telas::pecas::icone_da(gravidade))
                    .small()
                    .text_color(cor)
                    .flex_shrink_0()
                    .mt(px(1.)),
            )
            .child(div().flex_1().min_w_0().child(corpo))
            .into_any_element()
    }

    /// Os avisos do topo do formulario: tentativa sem resposta, rejeicao da Sefin, recusa antes de enviar, desistencia
    /// e rascunho recuperado.
    pub(in crate::telas) fn avisos_do_topo(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let mut avisos: Vec<AnyElement> = Vec::new();
        if let Fase::Pendente { tentativa, texto } = self.fase.clone() {
            avisos.push(
                v_flex()
                    .id("tentativa-sem-resposta")
                    .gap_3()
                    .p_4()
                    .rounded_lg()
                    .border_1()
                    .border_color(crate::telas::pecas::cor_da(Gravidade::Aviso, cx).opacity(0.6))
                    .bg(crate::telas::pecas::cor_da(Gravidade::Aviso, cx).opacity(0.10))
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                Icon::new(IconName::TriangleAlert).text_color(
                                    crate::telas::pecas::cor_da(Gravidade::Aviso, cx),
                                ),
                            )
                            .child(div().font_semibold().child("Sem resposta da Sefin")),
                    )
                    .child(div().text_sm().child(texto))
                    .child(div().text_sm().child(format!(
                        "Pedido congelado: DPS nº {}, série {}. O formulário fica travado até esta tentativa ter um desfecho.",
                        tentativa.numero, tentativa.serie
                    )))
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(EXPLICACAO_DE_TENTAR_DE_NOVO),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("tentar-de-novo-emissao")
                                    .label("Tentar de novo")
                                    .icon(IconName::RefreshCw)
                                    .primary()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.tentar_de_novo(window, cx);
                                    })),
                            )
                            .child(
                                Button::new("desistir-da-tentativa")
                                    .label("Desistir desta tentativa")
                                    .outline()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.pedir_para_desistir(window, cx);
                                    })),
                            ),
                    )
                    .into_any_element(),
            );
        }
        if let Some(msgs) = self.rejeicao.clone() {
            let mut painel = v_flex()
                .id("rejeicao-da-sefin")
                .gap_2()
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(
                            Icon::new(IconName::CircleAlert)
                                .text_color(cx.theme().danger),
                        )
                        .child(div().font_semibold().child("A Sefin recusou a nota")),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child("Seus dados estão intactos. Corrija o que a Sefin apontou e emita de novo; o número da DPS não foi gasto."),
                );
            for m in &msgs {
                painel = painel.child(self.mensagem_da_sefin(m, Gravidade::Erro, cx));
            }
            avisos.push(painel.into_any_element());
        }
        if let Some(p) = self.recusa.clone() {
            let mut texto = p.mensagem.clone();
            if let Some(r) = &p.regra {
                texto = format!("{texto} (regra {r})");
            }
            avisos.push(
                faixa(Gravidade::Erro, format!("Nada foi enviado. {texto}"), cx)
                    .id("recusa-antes-de-enviar")
                    .into_any_element(),
            );
        }
        if let Some(d) = self.desistencia.clone() {
            avisos.push(
                faixa(Gravidade::Aviso, d, cx)
                    .id("aviso-de-desistencia")
                    .into_any_element(),
            );
        }
        if self.recuperado {
            avisos.push(
                faixa(
                    Gravidade::Neutro,
                    "Rascunho recuperado: o que você tinha preenchido da última vez está aqui.",
                    cx,
                )
                .id("rascunho-recuperado")
                .into_any_element(),
            );
        }
        avisos
    }
}
