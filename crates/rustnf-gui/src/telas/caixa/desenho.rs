// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O desenho do caixa: a busca, o cupom com o total grande, o pagamento com o troco, o aviso de como a venda terminou e o
//! "Fechar o caixa". So' desenho e ligacao de eventos; o que decide mora em `modelo`.

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::Input;
use gpui_kit::component::{
    h_flex, v_flex, ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement as _, Render,
    SharedString, StatefulInteractiveElement as _, Styled as _, Window,
};

use super::{EstadoDoCupom, Etapa, Fase, TelaCaixa, Vista};
use crate::modelo::certificado::Gravidade;
use crate::modelo::mascara::centavos_para_dinheiro;
use crate::modelo::nfce_caixa::{quantidade_em_texto, FORMAS};
use crate::modelo::nfce_desfecho::{
    aviso_do_fechamento, frase_do_pendente, rotulo_do_pendente, DesfechoCaixa,
};
use crate::modelo::nfe_resultados::contagem;
use crate::telas::pecas::{cabecalho, cartao, estado_vazio, faixa, selo_de, Negrito as _};

fn dinheiro(c: i64) -> String {
    format!("R$ {}", centavos_para_dinheiro(c))
}

fn gravidade_do(d: &DesfechoCaixa) -> Gravidade {
    match d {
        DesfechoCaixa::Autorizada { .. } => Gravidade::Ok,
        DesfechoCaixa::Rejeitada { .. }
        | DesfechoCaixa::Denegada { .. }
        | DesfechoCaixa::NumeroJaUsado { .. }
        | DesfechoCaixa::Recusada { .. } => Gravidade::Erro,
        DesfechoCaixa::SemResposta { .. }
        | DesfechoCaixa::SefazFora { .. }
        | DesfechoCaixa::Offline { .. } => Gravidade::Aviso,
    }
}

impl TelaCaixa {
    // ---------------------------------------------------------------- o aviso do desfecho

    fn aviso_do_desfecho(&mut self, d: &DesfechoCaixa, cx: &mut Context<Self>) -> AnyElement {
        let gerando = self.cupom_do_desfecho == EstadoDoCupom::Gerando;
        let (muted, copiada) = (cx.theme().muted_foreground, self.chave_copiada);
        let mut col = v_flex()
            .id("caixa-desfecho")
            .gap_2()
            .child(
                div()
                    .id("caixa-desfecho-titulo")
                    .text_lg()
                    .font_semibold()
                    .child(d.titulo()),
            )
            .child(faixa(gravidade_do(d), d.texto(), cx).id("caixa-desfecho-texto"));
        if let DesfechoCaixa::Recusada { pendencias } = d {
            for (i, p) in pendencias.iter().skip(1).enumerate() {
                col = col.child(
                    faixa(
                        Gravidade::Erro,
                        format!("{} {}", p.mensagem, p.como_resolver),
                        cx,
                    )
                    .id(SharedString::from(format!("caixa-recusa-{i}"))),
                );
            }
        }
        let mut acoes = h_flex().gap_2().flex_wrap();
        if d.tem_cupom() {
            acoes = acoes
                .child(
                    Button::new("caixa-cupom-abrir")
                        .label(if gerando {
                            "Gerando o cupom..."
                        } else {
                            "Abrir o cupom"
                        })
                        .icon(IconName::FileText)
                        .primary()
                        .loading(gerando)
                        .disabled(gerando)
                        .on_click(cx.listener(|this, _, _, cx| this.abrir_o_cupom(cx))),
                )
                .child(
                    Button::new("caixa-cupom-escpos")
                        .label("Salvar para a impressora (ESC/POS)")
                        .icon(IconName::FolderOpen)
                        .outline()
                        .on_click(cx.listener(|this, _, _, cx| this.salvar_escpos(cx))),
                );
        }
        if d.deixa_pendente() {
            acoes = acoes.child(
                Button::new("caixa-retomar")
                    .label(if matches!(d, DesfechoCaixa::Offline { .. }) {
                        "Transmitir agora"
                    } else {
                        "Retomar"
                    })
                    .icon(IconName::RefreshCw)
                    .when(matches!(d, DesfechoCaixa::Offline { .. }), |b| b.outline())
                    .when(!matches!(d, DesfechoCaixa::Offline { .. }), |b| b.primary())
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.retomar_do_desfecho(window, cx);
                    })),
            );
        }
        if d.chave().is_some() {
            acoes = acoes.child(
                Button::new("caixa-copiar-chave")
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
                    .on_click(cx.listener(|this, _, _, cx| this.copiar_a_chave(cx))),
            );
        }
        if d.chave().is_some() {
            acoes = acoes.child(
                Button::new("caixa-ver-nas-notas")
                    .label("Ver nas Notas")
                    .icon(IconName::Inbox)
                    .outline()
                    .small()
                    .on_click(cx.listener(|this, _, _, cx| this.ver_nas_notas(cx))),
            );
        }
        acoes = acoes.child(
            Button::new("caixa-fechar-aviso")
                .label("Fechar o aviso")
                .ghost()
                .small()
                .on_click(cx.listener(|this, _, _, cx| this.fechar_o_aviso(cx))),
        );
        col = col.child(acoes);
        if d.tem_cupom() {
            col = col.child(div().text_xs().text_color(muted).child(
                "O programa gera o arquivo do cupom (PDF ou ESC/POS); ele não fala com a impressora: quem imprime é o programa do sistema ou o da sua impressora.",
            ));
        }
        cartao(cx).child(col).into_any_element()
    }

    // ---------------------------------------------------------------- a busca e o cupom

    fn bloco_da_busca(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let travado = !matches!(self.fase, Fase::Vendendo);
        let texto = self.e.busca.read(cx).value().to_string();
        let achados: Vec<(usize, crate::modelo::nfe_produtos::Produto)> = if texto.trim().is_empty()
        {
            Vec::new()
        } else {
            self.cadastro
                .indices(&texto, 6)
                .into_iter()
                .filter_map(|i| self.cadastro.produtos.get(i).cloned().map(|p| (i, p)))
                .collect()
        };
        let muted = cx.theme().muted_foreground;
        let mut col = v_flex().gap_2().child(
            h_flex()
                .gap_2()
                .items_center()
                .child(
                    div().flex_1().min_w_0().child(
                        Input::new(&self.e.busca)
                            .id("caixa-busca")
                            .disabled(travado)
                            .prefix(Icon::new(IconName::Search).small()),
                    ),
                )
                .child(
                    div().w(px(84.)).child(
                        Input::new(&self.e.quantidade)
                            .id("caixa-quantidade")
                            .disabled(travado),
                    ),
                )
                .child(
                    Button::new("caixa-produto-novo")
                        .label("Produto novo")
                        .icon(IconName::Plus)
                        .outline()
                        .disabled(travado)
                        .on_click(
                            cx.listener(|this, _, window, cx| this.abrir_o_cadastro(window, cx)),
                        ),
                ),
        );
        for (k, (i, p)) in achados.iter().enumerate() {
            let i = *i;
            col = col.child(
                h_flex()
                    .id(SharedString::from(format!("caixa-adicionar-{k}")))
                    .gap_3()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .border_1()
                    .border_color(cx.theme().border)
                    .cursor_pointer()
                    .hover(|s| s.bg(cx.theme().accent.opacity(0.5)))
                    .child(
                        div()
                            .w(px(64.))
                            .text_xs()
                            .text_color(muted)
                            .child(p.codigo.clone()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_sm()
                            .child(p.descricao.clone()),
                    )
                    .child(div().text_sm().child(format!("R$ {}", p.valor)))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.adicionar_do_cadastro(i, window, cx);
                    })),
            );
        }
        if !texto.trim().is_empty() && achados.is_empty() {
            col = col.child(div().id("caixa-nada-achado").text_sm().text_color(muted).child(
                "Nenhum produto com esse código ou nome. Aperte Enter (ou \"Produto novo\") para cadastrar.",
            ));
        }
        if let Some(a) = &self.aviso_cadastro {
            col = col.child(faixa(Gravidade::Aviso, a.clone(), cx));
        }
        col.into_any_element()
    }

    fn bloco_do_produto_novo(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let erro = self.erro_do_novo.clone();
        cartao(cx)
            .id("caixa-produto-novo-form")
            .child(div().text_sm().font_semibold().child("Cadastrar o produto e pôr no cupom"))
            .child(
                h_flex()
                    .gap_2()
                    .child(div().flex_1().child(Input::new(&self.e.np_descricao).id("caixa-novo-nome")))
                    .child(div().w(px(110.)).child(Input::new(&self.e.np_valor).id("caixa-novo-valor")))
                    .child(div().w(px(130.)).child(Input::new(&self.e.np_ncm).id("caixa-novo-ncm"))),
            )
            .child(div().text_xs().text_color(cx.theme().muted_foreground).child(
                "Nome, preço e NCM (8 dígitos, obrigatório na nota). Fica no mesmo cadastro da NF-e; os detalhes fiscais (ICMS, CFOP) vêm da tabela da empresa e se ajustam em Produtos.",
            ))
            .children(erro.map(|e| faixa(Gravidade::Erro, e, cx).id("caixa-novo-erro")))
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("caixa-novo-guardar")
                            .label("Guardar e pôr no cupom")
                            .primary()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.guardar_o_produto_novo(window, cx);
                            })),
                    )
                    .child(
                        Button::new("caixa-novo-cancelar")
                            .label("Cancelar")
                            .outline()
                            .on_click(cx.listener(|this, _, _, cx| this.fechar_o_cadastro(cx))),
                    ),
            )
            .into_any_element()
    }

    fn bloco_do_cupom(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let (muted, borda) = (cx.theme().muted_foreground, cx.theme().border);
        let travado = !matches!(self.fase, Fase::Vendendo);
        if self.cupom.itens.is_empty() {
            return estado_vazio(
                IconName::Inbox,
                "Cupom vazio",
                "Procure o produto pelo código ou pelo nome e aperte Enter. A quantidade é a do campo ao lado (vazio = 1).",
                Vec::new(),
                cx,
            )
            .id("caixa-cupom-vazio")
            .into_any_element();
        }
        let mut col = v_flex().id("caixa-cupom").gap_0();
        for (i, it) in self.cupom.itens.clone().iter().enumerate() {
            let sub = it.produto_centavos().unwrap_or(0);
            col = col.child(
                h_flex()
                    .id(SharedString::from(format!("caixa-linha-{i}")))
                    .gap_3()
                    .items_center()
                    .py_2()
                    .border_b_1()
                    .border_color(borda)
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(div().text_sm().truncate().child(it.descricao.clone()))
                            .child(div().text_xs().text_color(muted).child(format!(
                                "{} x R$ {}",
                                quantidade_em_texto(it),
                                it.valor
                            ))),
                    )
                    .child(
                        h_flex()
                            .gap_1()
                            .items_center()
                            .child(
                                Button::new(SharedString::from(format!("caixa-menos-{i}")))
                                    .icon(IconName::Minus)
                                    .outline()
                                    .small()
                                    .disabled(
                                        travado || it.quantidade_milesimos().unwrap_or(0) <= 1000,
                                    )
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.mudar_quantidade(i, -1000, cx);
                                    })),
                            )
                            .child(
                                div()
                                    .w(px(44.))
                                    .text_sm()
                                    .text_center()
                                    .child(quantidade_em_texto(it)),
                            )
                            .child(
                                Button::new(SharedString::from(format!("caixa-mais-{i}")))
                                    .icon(IconName::Plus)
                                    .outline()
                                    .small()
                                    .disabled(travado)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.mudar_quantidade(i, 1000, cx);
                                    })),
                            ),
                    )
                    .child(div().w(px(96.)).text_sm().text_right().child(dinheiro(sub)))
                    .child(
                        Button::new(SharedString::from(format!("caixa-tirar-{i}")))
                            .icon(IconName::Close)
                            .ghost()
                            .small()
                            .disabled(travado)
                            .on_click(cx.listener(move |this, _, _, cx| this.tirar_item(i, cx))),
                    ),
            );
        }
        col.into_any_element()
    }

    // ---------------------------------------------------------------- o total e o pagamento

    fn painel_do_pagamento(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let (muted, travado) = (
            cx.theme().muted_foreground,
            !matches!(self.fase, Fase::Vendendo),
        );
        let t = self.cupom.totais();
        let (falta, troco) = (self.cupom.falta(), self.cupom.troco());
        let por_que = self.por_que_nao_emite();
        let pode = por_que.is_none();
        let enviando = match self.fase {
            Fase::Enviando(Etapa::Certificado) => Some("Abrindo o certificado..."),
            Fase::Enviando(Etapa::Enviando) => Some("Enviando a venda à SEFAZ..."),
            Fase::Vendendo => None,
        };
        let mut col = v_flex()
            .gap_3()
            .child(
                v_flex()
                    .child(div().text_xs().text_color(muted).child("TOTAL"))
                    .child(div().id("caixa-total").text_3xl().font_semibold().child(dinheiro(t.total)))
                    .when(t.desconto > 0, |d| {
                        d.child(div().text_xs().text_color(muted).child(format!(
                            "Produtos {} - desconto {}",
                            dinheiro(t.produtos),
                            dinheiro(t.desconto)
                        )))
                    }),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        v_flex()
                            .flex_1()
                            .gap_1()
                            .child(div().text_xs().text_color(muted).child("Desconto (R$)"))
                            .child(Input::new(&self.e.desconto).id("caixa-desconto").disabled(travado)),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .gap_1()
                            .child(div().text_xs().text_color(muted).child("CPF na nota? (opcional)"))
                            .child(Input::new(&self.e.cpf).id("caixa-cpf").disabled(travado)),
                    ),
            )
            .children((!self.cupom.cpf_confere()).then(|| {
                faixa(
                    Gravidade::Erro,
                    "O CPF não confere: o dígito final não bate. Corrija ou apague, ele é opcional.",
                    cx,
                )
                .id("caixa-cpf-erro")
            }))
            .child(div().h(px(1.)).w_full().bg(cx.theme().border))
            .child(div().text_xs().text_color(muted).child("PAGAMENTO"))
            .child(
                Input::new(&self.e.recebido)
                    .id("caixa-recebido")
                    .disabled(travado),
            )
            .child(
                h_flex().gap_2().flex_wrap().children(FORMAS.iter().enumerate().map(|(i, f)| {
                    let forma = *f;
                    Button::new(SharedString::from(format!("caixa-pagar-{i}")))
                        .label(forma.rotulo())
                        .outline()
                        .small()
                        .disabled(travado || t.total <= 0)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.pagar(forma, window, cx);
                        }))
                })),
            );
        for (i, p) in self.cupom.pagamentos.clone().iter().enumerate() {
            col = col.child(
                h_flex()
                    .id(SharedString::from(format!("caixa-pagamento-{i}")))
                    .gap_2()
                    .items_center()
                    .child(div().flex_1().text_sm().child(p.forma.rotulo()))
                    .child(div().text_sm().child(dinheiro(p.centavos)))
                    .child(
                        Button::new(SharedString::from(format!("caixa-tirar-pagamento-{i}")))
                            .icon(IconName::Close)
                            .ghost()
                            .small()
                            .disabled(travado)
                            .on_click(
                                cx.listener(move |this, _, _, cx| this.tirar_pagamento(i, cx)),
                            ),
                    ),
            );
        }
        if !self.cupom.pagamentos.is_empty() {
            col = col.child(if troco > 0 {
                div()
                    .id("caixa-troco")
                    .text_xl()
                    .font_semibold()
                    .child(format!("Troco {}", dinheiro(troco)))
            } else if falta > 0 {
                div()
                    .id("caixa-falta")
                    .text_lg()
                    .text_color(cx.theme().warning)
                    .child(format!("Falta {}", dinheiro(falta)))
            } else {
                div()
                    .id("caixa-pago")
                    .text_sm()
                    .text_color(cx.theme().success)
                    .child("Pago: sem troco")
            });
        }
        if self.exige_efeito_fiscal() {
            col = col.child(
                Checkbox::new("caixa-entendo")
                    .label("Entendo que a nota tem efeito fiscal")
                    .checked(self.entendo)
                    .disabled(travado)
                    .on_click(cx.listener(|this, marcou: &bool, _, cx| {
                        this.entendo = *marcou;
                        cx.notify();
                    })),
            );
        }
        col = col.child(
            Button::new("caixa-emitir")
                .label(if enviando.is_some() {
                    "Enviando..."
                } else {
                    "Emitir o cupom (Ctrl+Enter)"
                })
                .icon(IconName::Check)
                .primary()
                .loading(enviando.is_some())
                .disabled(!pode)
                .on_click(cx.listener(|this, _, window, cx| this.emitir(window, cx))),
        );
        if let Some(t) = enviando {
            col = col.child(
                div()
                    .id("caixa-enviando")
                    .text_sm()
                    .text_color(muted)
                    .child(t),
            );
        } else if let Some(m) = por_que {
            if !self.cupom.vazio() || self.por_que_o_caixa_trava() {
                col = col.child(
                    div()
                        .id("caixa-motivo")
                        .text_xs()
                        .text_color(muted)
                        .child(m),
                );
            }
        }
        cartao(cx).child(col).into_any_element()
    }

    /// O motivo vale ser dito mesmo com o cupom vazio (configuracao, CSC).
    fn por_que_o_caixa_trava(&self) -> bool {
        crate::modelo::nfce_config::por_que_o_csc_trava_o_caixa(
            self.config.nfce.qr_versao,
            self.csc,
        )
        .is_some()
    }

    // ---------------------------------------------------------------- fechar o caixa

    fn tela_do_fechamento(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let (muted, borda) = (cx.theme().muted_foreground, cx.theme().border);
        let aviso = aviso_do_fechamento(&self.pendentes);
        let mut lista = v_flex().id("caixa-pendentes").gap_0();
        for (i, p) in self.pendentes.iter().enumerate() {
            lista = lista.child(
                h_flex()
                    .id(SharedString::from(format!("caixa-pendente-{i}")))
                    .gap_3()
                    .items_start()
                    .py_3()
                    .border_b_1()
                    .border_color(borda)
                    .child(
                        div()
                            .w(px(72.))
                            .text_sm()
                            .font_semibold()
                            .child(format!("nº {}", p.registro.numero)),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_1()
                            .child(div().text_sm().child(frase_do_pendente(p))),
                    )
                    .child(selo_de(
                        rotulo_do_pendente(p),
                        if p.destaque() {
                            cx.theme().danger
                        } else {
                            cx.theme().warning
                        },
                    )),
            );
        }
        let n = self.pendentes.len();
        let mut col = v_flex()
            .id("caixa-fechamento")
            .size_full()
            .gap_4()
            .px_6()
            .py_4()
            .child(cabecalho(
                "Fechar o caixa",
                "As NFC-e que ainda dependem da SEFAZ: as emitidas sem internet (com prazo para transmitir) e as que ficaram sem resposta.",
                cx,
            ));
        col = match aviso {
            Some(a) => col.child(faixa(Gravidade::Aviso, a, cx).id("caixa-aviso-de-fechamento")),
            None if self.pendentes_lidos => col.child(
                faixa(
                    Gravidade::Ok,
                    "Nenhuma NFC-e por transmitir. Pode fechar o caixa.",
                    cx,
                )
                .id("caixa-fechamento-limpo"),
            ),
            None => col.child(
                div()
                    .text_sm()
                    .text_color(muted)
                    .child("Lendo a pasta de notas..."),
            ),
        };
        col.child(
            h_flex()
                .gap_2()
                .child(
                    Button::new("caixa-transmitir")
                        .label(if n == 0 {
                            "Transmitir agora".to_string()
                        } else {
                            format!("Transmitir agora ({})", contagem(n, "nota", "notas"))
                        })
                        .icon(IconName::RefreshCw)
                        .primary()
                        .disabled(n == 0)
                        .on_click(
                            cx.listener(|this, _, window, cx| this.transmitir_agora(window, cx)),
                        ),
                )
                .child(
                    Button::new("caixa-atualizar-pendentes")
                        .label("Atualizar")
                        .icon(IconName::RefreshCw)
                        .outline()
                        .on_click(cx.listener(|this, _, _, cx| this.recarregar_pendentes(cx))),
                )
                .child(
                    Button::new("caixa-voltar-a-venda")
                        .label("Voltar à venda")
                        .icon(IconName::ChevronLeft)
                        .outline()
                        .on_click(cx.listener(|this, _, _, cx| this.voltar_a_venda(cx))),
                ),
        )
        .child(
            div()
                .id("caixa-pendentes-rolagem")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .child(lista),
        )
        .into_any_element()
    }

    // ---------------------------------------------------------------- a venda

    fn tela_da_venda(&mut self, ao_lado: bool, cx: &mut Context<Self>) -> AnyElement {
        let n = self.pendentes.len();
        let desfecho = self.desfecho.clone();
        let aviso = desfecho.as_ref().map(|d| self.aviso_do_desfecho(d, cx));
        let csc = crate::modelo::nfce_config::por_que_o_csc_trava_o_caixa(
            self.config.nfce.qr_versao,
            self.csc,
        );
        let busca = self.bloco_da_busca(cx);
        let novo = self.novo_aberto.then(|| self.bloco_do_produto_novo(cx));
        let cupom = self.bloco_do_cupom(cx);
        let pagamento = self.painel_do_pagamento(cx);
        let erro = self.erro.clone();
        let esquerda = v_flex()
            .gap_3()
            .flex_1()
            .min_w_0()
            .children(aviso)
            .children(csc.map(|t| {
                v_flex()
                    .id("caixa-csc")
                    .gap_2()
                    .child(faixa(Gravidade::Erro, t, cx))
                    .child(
                        h_flex().child(
                            Button::new("caixa-ir-para-csc")
                                .label("Abrir as configurações da NFC-e")
                                .icon(IconName::Settings)
                                .outline()
                                .small()
                                .on_click(cx.listener(|_, _, _, cx| {
                                    cx.emit(super::EventoCaixa::IrParaConfiguracoes);
                                })),
                        ),
                    )
            }))
            .children(erro.map(|e| faixa(Gravidade::Erro, e, cx).id("caixa-erro")))
            .child(busca)
            .children(novo)
            .child(cupom);
        let direita = div()
            .when(ao_lado, |d| d.w(px(360.)).flex_shrink_0())
            .child(pagamento);
        v_flex()
            .id("caixa-venda")
            .gap_4()
            .px_6()
            .py_4()
            .child(
                h_flex()
                    .items_start()
                    .justify_between()
                    .gap_3()
                    .child(div().flex_1().min_w_0().child(cabecalho(
                        "Caixa",
                        "A venda rápida do balcão: o cupom (NFC-e) sai na hora. Enter na busca põe o produto, Enter no valor recebido lança o dinheiro, Ctrl+Enter emite.",
                        cx,
                    )))
                    .child(
                        Button::new("caixa-fechar")
                            .label(if n == 0 {
                                "Fechar o caixa".to_string()
                            } else {
                                format!("Fechar o caixa ({n})")
                            })
                            .icon(IconName::Inbox)
                            .outline()
                            .on_click(cx.listener(|this, _, _, cx| this.abrir_o_fechamento(cx))),
                    ),
            )
            .child(if ao_lado {
                h_flex().gap_5().items_start().child(esquerda).child(direita).into_any_element()
            } else {
                v_flex().gap_4().child(esquerda).child(direita).into_any_element()
            })
            .into_any_element()
    }

    fn falta_configurar(&mut self, cx: &mut Context<Self>) -> AnyElement {
        estado_vazio(
            IconName::Settings,
            "Falta configurar a NFC-e",
            "Para vender no caixa, informe nas Configurações, na seção NF-e, a UF do emitente. Os dados do emitente, o certificado e a pasta de notas são os mesmos da NFS-e; o ambiente e a série do cupom ficam na seção NFC-e.",
            vec![Button::new("caixa-ir-para-configuracoes")
                .label("Abrir as configurações da NFC-e")
                .icon(IconName::Settings)
                .primary()
                .on_click(cx.listener(|_, _, _, cx| cx.emit(super::EventoCaixa::IrParaConfiguracoes)))
                .into_any_element()],
            cx,
        )
        .id("caixa-falta-configurar")
        .into_any_element()
    }
}

impl Render for TelaCaixa {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let largura = window.viewport_size().width - px(crate::telas::LARGURA_DA_LATERAL);
        let ao_lado = largura >= px(840.);
        let base = div()
            .id("tela-caixa")
            .key_context("Caixa")
            .track_focus(&self.foco)
            .size_full()
            .on_action(
                cx.listener(|this, _: &crate::telas::EmitirCupom, window, cx| {
                    this.emitir(window, cx);
                }),
            );
        if !self.pronta {
            return base.child(self.falta_configurar(cx));
        }
        if self.vista == Vista::Fechamento {
            return base.child(self.tela_do_fechamento(cx));
        }
        let corpo = self.tela_da_venda(ao_lado, cx);
        base.child(
            div()
                .id("caixa-rolagem")
                .size_full()
                .overflow_y_scroll()
                .track_scroll(&self.rolagem)
                .child(corpo),
        )
    }
}
