// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O Inicio: o primeiro uso, se a configuracao estiver incompleta, ou o resumo do que esta configurado.

use gpui_kit::component::button::Button;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Icon, IconName, Sizable as _};
use gpui_kit::{
    div, px, AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, Window,
};

use super::pecas::{cabecalho, cartao, cor_da, icone_da, linha_de_resumo, Negrito as _};
use super::raiz::{Raiz, Tela};
use crate::modelo::ambiente::Ambiente;
use crate::modelo::formulario::OPCOES_SIMPLES;
use crate::modelo::mascara::{documento_para_resumo, rotulo_do_municipio, RotuloMunicipio};

impl Raiz {
    /// O conteudo da tela Inicio.
    pub(super) fn tela_inicio(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if self.pergunta_o_primeiro_uso() {
            self.assistente(window, cx)
        } else {
            self.resumo(cx)
        }
    }

    fn resumo(&self, cx: &mut Context<Self>) -> AnyElement {
        let c = &self.salvo;
        let tema = cx.theme();
        let documento = if c.emitente.cnpj.is_empty() {
            &c.emitente.cpf
        } else {
            &c.emitente.cnpj
        };
        let municipio = match rotulo_do_municipio(&c.emitente.municipio) {
            RotuloMunicipio::Encontrado { nome, uf } => {
                format!("{nome} - {uf} ({})", c.emitente.municipio)
            }
            _ => c.emitente.municipio.clone(),
        };
        let simples = OPCOES_SIMPLES
            .iter()
            .find(|(codigo, _)| *codigo == c.emitente.op_simples)
            .map_or("", |(_, rotulo)| *rotulo);
        let ambiente = self
            .info
            .as_ref()
            .map_or_else(|| Ambiente::de_config(&c.sefin.ambiente), |i| i.ambiente);
        let gravidade = self.cert.gravidade();
        let cor_cert = cor_da(gravidade, cx);
        let muted = tema.muted_foreground;
        let pasta = c.notas.pasta.clone();
        let variaveis = self
            .info
            .as_ref()
            .map(|i| i.variaveis.join(", "))
            .filter(|v| !v.is_empty());

        let emitente = cartao(cx)
            .id("resumo-emitente")
            .child(titulo_do_cartao("Emitente", IconName::Building2, cx))
            .child(linha_de_resumo("Nome", c.emitente.nome.clone(), cx))
            .child(linha_de_resumo(
                "Documento",
                documento_para_resumo(documento),
                cx,
            ))
            .child(linha_de_resumo(
                "Inscrição",
                c.emitente.inscricao_municipal.clone(),
                cx,
            ))
            .child(linha_de_resumo("Município", municipio, cx))
            .child(linha_de_resumo("Simples", simples, cx));

        let cores = ambiente.cores();
        let ambiente_cartao = cartao(cx)
            .id("resumo-ambiente")
            .child(titulo_do_cartao("Ambiente", IconName::Globe, cx))
            .child(
                h_flex().child(
                    div()
                        .px_2p5()
                        .py_0p5()
                        .rounded_full()
                        .text_xs()
                        .font_semibold()
                        .bg(gpui_kit::rgb(cores.fundo))
                        .text_color(gpui_kit::rgb(cores.texto))
                        .child(ambiente.selo()),
                ),
            )
            .child(div().text_sm().text_color(muted).child(ambiente.efeito()))
            .child(div().text_sm().text_color(muted).child(
                "Assinatura: ".to_string()
                    + if c.sefin.assinatura == "sha1" {
                        "SHA-1"
                    } else {
                        "SHA-256"
                    },
            ));

        let certificado =
            cartao(cx)
                .id("resumo-certificado")
                .child(titulo_do_cartao("Certificado", IconName::FileText, cx))
                .child(
                    h_flex()
                        .gap_2()
                        .items_start()
                        .text_sm()
                        .text_color(cor_cert)
                        .child(Icon::new(icone_da(gravidade)).small().mt(px(1.)))
                        .child(div().flex_1().child(self.cert.texto())),
                )
                .child(div().text_sm().text_color(muted).child(
                    if self.servicos.cofre_desligado() {
                        "O cofre de senhas está desligado."
                    } else {
                        "A senha fica no cofre de senhas do Windows."
                    },
                ));

        let notas = cartao(cx)
            .id("resumo-notas")
            .child(titulo_do_cartao(
                "Pasta das notas",
                IconName::FolderOpen,
                cx,
            ))
            .child(div().text_sm().id("resumo-pasta").child(pasta))
            .child(
                div()
                    .text_sm()
                    .text_color(muted)
                    .child("Cada nota autorizada é guardada aqui em XML, com um índice."),
            );

        let proximos = cartao(cx)
            .id("proximos-passos")
            .child(titulo_do_cartao("O que fazer a seguir", IconName::Info, cx))
            .child(div().text_sm().child(
                "Esta janela configura o rustnf. Para montar, emitir, consultar ou cancelar notas, use a linha de comando do mesmo programa:",
            ))
            .child(
                v_flex()
                    .gap_1()
                    .text_sm()
                    .text_color(muted)
                    .child("rustnf dps conferir pedido.json   (confere o pedido antes de enviar)")
                    .child("rustnf emitir pedido.json   (usa estes mesmos dados e o certificado)")
                    .child("rustnf consultar --chave <chave>   |   rustnf cancelar ..."),
            )
            .child(div().text_sm().child(
                if ambiente == Ambiente::Producao {
                    "Atenção: em produção as notas têm efeito fiscal. Teste antes na produção restrita."
                } else {
                    "Você está na produção restrita: é o lugar certo para testar. Nada aqui tem efeito fiscal."
                },
            ))
            .child(
                h_flex().child(
                    Button::new("abrir-configuracoes")
                        .label("Abrir Configurações")
                        .icon(IconName::Settings)
                        .outline()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.tela = Tela::Configuracoes;
                            cx.notify();
                        })),
                ),
            );

        div()
            .id("inicio")
            .size_full()
            .overflow_y_scroll()
            .child(
                v_flex()
                    .w_full()
                    .max_w(px(920.))
                    .mx_auto()
                    .gap_5()
                    .px_6()
                    .py_8()
                    .child(cabecalho(
                        "Início",
                        "Tudo configurado. Veja abaixo o que o rustnf vai usar.",
                        cx,
                    ))
                    .when_some(variaveis, |d, v| {
                        d.child(super::pecas::faixa(
                            crate::modelo::certificado::Gravidade::Aviso,
                            format!("Variáveis de ambiente em uso: {v}. Elas valem no lugar do arquivo quando o rustnf roda."),
                            cx,
                        ))
                    })
                    .child(
                        h_flex()
                            .gap_5()
                            .items_start()
                            .child(v_flex().flex_1().min_w_0().gap_5().child(emitente).child(notas))
                            .child(v_flex().flex_1().min_w_0().gap_5().child(ambiente_cartao).child(certificado)),
                    )
                    .child(proximos),
            )
            .into_any_element()
    }
}

fn titulo_do_cartao(titulo: &str, icone: IconName, cx: &mut Context<Raiz>) -> impl IntoElement {
    let tema = cx.theme();
    h_flex()
        .gap_2()
        .items_center()
        .text_color(tema.foreground)
        .child(Icon::new(icone).small().text_color(tema.muted_foreground))
        .child(div().font_semibold().child(titulo.to_string()))
}

use gpui_kit::prelude::FluentBuilder as _;
