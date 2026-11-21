// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Os dialogos da Nova nota: confirmar a emissao (com o ambiente em destaque), pedir a senha do certificado, limpar o
//! formulario e desistir de uma tentativa sem resposta. Esc fecha qualquer um.

use std::cell::Cell;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::dialog::DialogFooter;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Disableable as _, WindowExt as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, rgb, AppContext as _, Context, Focusable as _, InteractiveElement as _,
    ParentElement as _, Styled as _, Window,
};
use rustnf_app::segredo::Segredo;

use super::{Fase, NovaNota};
use crate::modelo::ambiente::{CORES_DA_DEMONSTRACAO, SELO_DA_DEMONSTRACAO};
use crate::modelo::certificado::Gravidade;
use crate::modelo::mascara::documento_para_resumo;
use crate::modelo::pedido::PedidoCongelado;
use crate::modelo::previa::mostrar;
use crate::telas::pecas::{faixa, linha_de_resumo, Negrito as _};

/// O que a confirmacao mostra: tomador, servico, valor e valor liquido, e a DPS que vai ser usada.
struct ResumoDaConfirmacao {
    tomador: String,
    servico: String,
    valor: String,
    liquido: String,
    dps: String,
}

impl NovaNota {
    fn resumo_da_confirmacao(&self) -> ResumoDaConfirmacao {
        let d = &self.dados;
        let tomador = if d.sem_tomador {
            "Sem tomador identificado".to_string()
        } else {
            format!(
                "{} ({})",
                d.tomador_nome.trim(),
                documento_para_resumo(&d.documento_do_tomador())
            )
        };
        let t = crate::modelo::previa::totais(d);
        let descricao: String = d.descricao.split_whitespace().collect::<Vec<_>>().join(" ");
        let curta: String = descricao.chars().take(90).collect();
        let servico = format!(
            "{} - {}{}",
            d.ctribnac,
            curta,
            if descricao.chars().count() > 90 {
                "..."
            } else {
                ""
            }
        );
        ResumoDaConfirmacao {
            tomador,
            servico,
            valor: format!("R$ {}", mostrar(t.servico.unwrap_or(0))),
            liquido: format!("R$ {}", mostrar(t.liquido.unwrap_or(0))),
            dps: format!(
                "DPS nº {}, série {}",
                self.numero_provisorio,
                self.ctx.serie()
            ),
        }
    }

    /// O dialogo de confirmacao: ambiente em destaque (as cores do selo) e, em producao, a marcacao explicita.
    pub(in crate::telas) fn abrir_confirmacao(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let nova = cx.entity();
        window.open_dialog(cx, move |dialog, _, cx| {
            let n = nova.read(cx);
            let ambiente = n.ambiente();
            let cores = ambiente.cores();
            let demonstracao = n.servicos.demonstracao();
            let exige = n.exige_valor_fiscal();
            let marcado = n.valor_fiscal_entendido;
            let pode = n.pode_confirmar();
            let r = n.resumo_da_confirmacao();
            let producao = exige;
            let nova_marcar = nova.clone();
            let nova_emitir = nova.clone();
            dialog
                .title("Confirmar a emissão")
                .w(px(540.))
                .child(
                    v_flex()
                        .gap_3()
                        .child(
                            h_flex().gap_2().items_center().child(
                                div()
                                    .id("selo-da-confirmacao")
                                    .px_3()
                                    .py_1()
                                    .rounded_full()
                                    .text_sm()
                                    .font_semibold()
                                    .bg(rgb(cores.fundo))
                                    .text_color(rgb(cores.texto))
                                    .child(ambiente.selo()),
                            ),
                        )
                        .when(demonstracao, |d| {
                            d.child(
                                div()
                                    .id("aviso-de-demonstracao")
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
                        .child(div().text_sm().child(ambiente.efeito()))
                        .child(
                            v_flex()
                                .gap_2()
                                .p_3()
                                .rounded_md()
                                .border_1()
                                .border_color(cx.theme().border)
                                .child(linha_de_resumo("Tomador", r.tomador, cx))
                                .child(linha_de_resumo("Serviço", r.servico, cx))
                                .child(linha_de_resumo("Valor", r.valor, cx))
                                .child(linha_de_resumo("Valor líquido", r.liquido, cx))
                                .child(linha_de_resumo("Numeração", r.dps, cx)),
                        )
                        .when(exige, |d| {
                            d.child(
                                Checkbox::new("entendo-valor-fiscal")
                                    .label("Entendo que esta nota tem valor fiscal")
                                    .checked(marcado)
                                    .on_click(move |marcou: &bool, window, cx| {
                                        nova_marcar.update(cx, |n, cx| {
                                            n.valor_fiscal_entendido = *marcou;
                                            cx.notify();
                                        });
                                        window.refresh();
                                    }),
                            )
                        }),
                )
                .footer(
                    DialogFooter::new()
                        .child(
                            Button::new("cancelar-emissao")
                                .label("Cancelar")
                                .outline()
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("confirmar-emissao")
                                .label(if producao {
                                    "Emitir em produção"
                                } else {
                                    "Emitir nota"
                                })
                                .when(producao, |b| b.danger())
                                .when(!producao, |b| b.primary())
                                .disabled(!pode)
                                .on_click(move |_, window, cx| {
                                    window.close_dialog(cx);
                                    nova_emitir.update(cx, |n, cx| n.confirmar_emissao(window, cx));
                                }),
                        ),
                )
        });
    }

    /// Pede a senha do certificado em um dialogo com campo mascarado. Ela vale so' para esta emissao: vai direto para
    /// o certificado, nao e' guardada e o campo e' esvaziado ao usar.
    pub(in crate::telas) fn pedir_senha(
        &mut self,
        tentativa: PedidoCongelado,
        erro: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.erro_senha = erro;
        let entrada = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Senha do certificado")
                .masked(true)
        });
        self.senha_pedida = Some((entrada.clone(), tentativa));
        self.sub_senha = Some(cx.subscribe_in(
            &entrada,
            window,
            |this, _, evento: &InputEvent, window, cx| {
                if matches!(evento, InputEvent::PressEnter { .. }) {
                    this.usar_senha(window, cx);
                }
            },
        ));
        let nova = cx.entity();
        let primeira = Cell::new(true);
        window.open_dialog(cx, move |dialog, window, cx| {
            if primeira.replace(false) {
                entrada.focus_handle(cx).focus(window, cx);
            }
            let erro = nova.read(cx).erro_senha.clone();
            let desligado = nova.read(cx).servicos.cofre_desligado();
            let nova_usar = nova.clone();
            let nova_fechou = nova.clone();
            dialog
                .title("Senha do certificado")
                .w(px(460.))
                .on_close(move |_, _, cx| {
                    nova_fechou.update(cx, |n, _| {
                        n.senha_pedida = None;
                        n.sub_senha = None;
                    });
                })
                .child(
                    v_flex()
                        .gap_3()
                        .child(div().text_sm().child(if desligado {
                            "O cofre de senhas está desligado. Digite a senha do certificado: ela vale só para esta emissão e não é guardada."
                        } else {
                            "Não há senha guardada no cofre para este certificado. Digite-a: ela vale só para esta emissão e não é guardada."
                        }))
                        .child(Input::new(&entrada).id("senha-da-emissao").mask_toggle())
                        .children(erro.map(|e| faixa(Gravidade::Erro, e, cx))),
                )
                .footer(
                    DialogFooter::new()
                        .child(
                            Button::new("cancelar-senha")
                                .label("Cancelar")
                                .outline()
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("usar-senha")
                                .label("Usar nesta emissão")
                                .primary()
                                .on_click(move |_, window, cx| {
                                    nova_usar.update(cx, |n, cx| n.usar_senha(window, cx));
                                }),
                        ),
                )
        });
    }

    /// Usa a senha digitada: fecha o dialogo, esvazia o campo e emite.
    pub(in crate::telas) fn usar_senha(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((entrada, tentativa)) = self.senha_pedida.clone() else {
            return;
        };
        let texto = entrada.read(cx).value().to_string();
        if texto.is_empty() {
            self.erro_senha = Some("Digite a senha do certificado.".into());
            window.refresh();
            cx.notify();
            return;
        }
        let senha = Segredo::novo(texto);
        entrada.update(cx, |e, cx| e.set_value("", window, cx));
        self.senha_pedida = None;
        self.sub_senha = None;
        window.close_dialog(cx);
        self.iniciar_envio(Some(tentativa), Some(senha), window, cx);
    }

    /// Pergunta antes de limpar o formulario.
    pub(in crate::telas) fn pedir_para_limpar(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.travado() || self.dados.vazio(&self.ctx.hoje()) {
            return;
        }
        let nova = cx.entity();
        window.open_alert_dialog(cx, move |alerta, _, _| {
            let nova = nova.clone();
            alerta
                .title("Limpar o formulário?")
                .description("Tudo o que foi preenchido e o rascunho salvo serão apagados.")
                .confirm()
                .ok_text("Limpar")
                .cancel_text("Cancelar")
                .on_ok(move |_, window, cx| {
                    nova.update(cx, |n, cx| n.limpar(window, cx));
                    true
                })
        });
    }

    /// Pergunta antes de desistir de uma tentativa sem resposta, com o aviso de que a nota pode ter sido autorizada.
    pub(in crate::telas) fn pedir_para_desistir(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Fase::Pendente { tentativa, .. } = &self.fase else {
            return;
        };
        let aviso = crate::modelo::emissao::aviso_de_desistir(&tentativa.serie, tentativa.numero);
        let nova = cx.entity();
        window.open_alert_dialog(cx, move |alerta, _, _| {
            let nova = nova.clone();
            alerta
                .title("Desistir desta tentativa?")
                .description(aviso.clone())
                .confirm()
                .ok_text("Desistir")
                .cancel_text("Voltar")
                .on_ok(move |_, _, cx| {
                    nova.update(cx, |n, cx| n.desistir(cx));
                    true
                })
        });
    }
}
