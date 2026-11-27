// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O desenho da Nova NF-e: o cabecalho com os passos, o corpo de cada passo (para quem, como recebeu, como entrega) e o
//! rodape de navegacao. O passo "O que vendeu" mora em `itens`, o "Conferir e enviar" e os desfechos em `resultado`.

use chrono::Duration;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::form::v_form;
use gpui_kit::component::input::Input;
use gpui_kit::component::{
    h_flex, v_flex, ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, rgb, AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement as _,
    Render, SharedString, StatefulInteractiveElement as _, Styled as _, Window,
};

use super::{EstadoDoRascunho, Fase, NovaNfe};
use crate::modelo::ambiente::{CORES_DA_DEMONSTRACAO, SELO_DA_DEMONSTRACAO};
use crate::modelo::certificado::Gravidade;
use crate::modelo::mascara::{centavos_para_dinheiro, iso_para_data_br};
use crate::modelo::nfe_campos::CampoNova;
use crate::modelo::nfe_config::selo_do_ambiente_da_nfe;
use crate::modelo::nfe_fiscal::{Contribuinte, Entrega, Pagamento, Presenca};
use crate::modelo::nfe_nova::{municipio_do_codigo, Consumo, DadosNfe, PassoNfe, MAX_PARCELAS};
use crate::telas::pecas::{
    cabecalho, campo, cartao, estado_vazio, faixa, grupo, Negrito as _, LARGURA_FORM,
};

/// Centraliza o conteudo na largura maxima dos formularios.
pub(super) fn centrada(filho: impl IntoElement) -> AnyElement {
    div()
        .w_full()
        .max_w(px(LARGURA_FORM))
        .mx_auto()
        .child(filho)
        .into_any_element()
}

impl NovaNfe {
    /// O campo de texto do assistente.
    pub(super) fn campo_de_texto(&self, c: CampoNova) -> AnyElement {
        match self.entradas.get(&c) {
            Some(e) => Input::new(e)
                .id(SharedString::from(format!("nfe-{c:?}")))
                .into_any_element(),
            None => div().into_any_element(),
        }
    }

    /// Os botoes de escolha de uma pergunta simples: um por opcao, o marcado em destaque. Escolher nao envia nada.
    pub(super) fn escolhas<T: Copy + PartialEq + 'static>(
        &self,
        id: &'static str,
        opcoes: Vec<(T, String)>,
        atual: Option<T>,
        ao_escolher: fn(&mut DadosNfe, T),
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let travado = self.travado();
        h_flex()
            .id(SharedString::from(format!("{id}-grupo")))
            .flex_wrap()
            .gap_2()
            .children(opcoes.into_iter().enumerate().map(|(i, (valor, rotulo))| {
                let marcado = atual == Some(valor);
                Button::new(SharedString::from(format!("{id}-{i}")))
                    .label(rotulo)
                    .small()
                    .when(marcado, |b| b.primary())
                    .when(!marcado, |b| b.outline())
                    .disabled(travado)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.escolher(move |d| ao_escolher(d, valor), cx);
                    }))
            }))
            .into_any_element()
    }

    /// A lista do que falta neste passo, depois de a pessoa tentar continuar.
    pub(super) fn faixa_de_pendencias(
        &self,
        passo: PassoNfe,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if !self.tentou {
            return None;
        }
        let p = self.pendencias_do_passo(passo);
        if p.is_empty() {
            return None;
        }
        let tema_muted = cx.theme().muted_foreground;
        let mut col = v_flex().gap_2().id("pendencias-do-passo");
        col = col.child(div().font_semibold().child(if p.len() == 1 {
            "Falta uma coisa neste passo".to_string()
        } else {
            format!("Faltam {} coisas neste passo", p.len())
        }));
        for x in p {
            col = col.child(
                v_flex().gap_0p5().child(div().child(x.mensagem)).child(
                    div()
                        .text_xs()
                        .text_color(tema_muted)
                        .child(x.como_resolver),
                ),
            );
        }
        Some(
            faixa(Gravidade::Erro, "", cx)
                .id("faixa-de-pendencias")
                .child(div().flex_1().min_w_0().child(col))
                .into_any_element(),
        )
    }

    // ---------------------------------------------------------------- o cabecalho

    fn selo_do_ambiente(&self) -> AnyElement {
        let ambiente = self.ambiente();
        let cores = ambiente.cores();
        let demonstracao = self.servicos.demonstracao_nfe();
        h_flex()
            .gap_2()
            .items_center()
            .child(
                div()
                    .id("nfe-selo-do-ambiente")
                    .px_2p5()
                    .py_0p5()
                    .rounded_full()
                    .text_xs()
                    .font_semibold()
                    .bg(rgb(cores.fundo))
                    .text_color(rgb(cores.texto))
                    .child(selo_do_ambiente_da_nfe(ambiente)),
            )
            .when(demonstracao, |d| {
                d.child(
                    div()
                        .id("nfe-selo-da-demonstracao")
                        .px_2p5()
                        .py_0p5()
                        .rounded_md()
                        .text_xs()
                        .font_semibold()
                        .bg(rgb(CORES_DA_DEMONSTRACAO.fundo))
                        .text_color(rgb(CORES_DA_DEMONSTRACAO.texto))
                        .child(SELO_DA_DEMONSTRACAO),
                )
            })
            .into_any_element()
    }

    /// Os cinco passos: o atual em destaque, os anteriores clicaveis.
    fn lista_de_passos(&self, cx: &mut Context<Self>) -> AnyElement {
        let atual = self.passo;
        let tema = cx.theme();
        let (primaria, muted) = (tema.primary, tema.muted_foreground);
        h_flex()
            .id("nfe-passos")
            .flex_wrap()
            .gap_2()
            .children(PassoNfe::TODOS.into_iter().map(|p| {
                let feito = p < atual;
                let eh = p == atual;
                h_flex()
                    .id(SharedString::from(format!("nfe-passo-{}", p.indice())))
                    .gap_1p5()
                    .items_center()
                    .px_3()
                    .py_1p5()
                    .rounded_full()
                    .border_1()
                    .border_color(if eh { primaria } else { muted.opacity(0.4) })
                    .when(eh, |d| d.bg(primaria.opacity(0.12)))
                    .text_sm()
                    .child(if feito {
                        Icon::new(IconName::Check).small().text_color(primaria)
                    } else {
                        Icon::new(IconName::Dash).small().text_color(muted)
                    })
                    .child(div().when(eh, |d| d.font_semibold()).child(format!(
                        "{}. {}",
                        p.indice() + 1,
                        p.titulo()
                    )))
                    .when(feito && !self.travado(), |d| {
                        d.cursor_pointer()
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.ir_para_o_passo(p, window, cx);
                            }))
                    })
            }))
            .into_any_element()
    }

    /// Os avisos do topo: o que a SEFAZ recusou, o rascunho recuperado, o que veio da nota anterior, o cadastro ruim.
    fn avisos_do_topo(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let mut avisos: Vec<AnyElement> = Vec::new();
        if let Some(r) = self.rejeicao.clone() {
            avisos.push(self.faixa_da_rejeicao(&r, cx));
        }
        if self.recuperado {
            avisos.push(
                faixa(
                    Gravidade::Neutro,
                    "Rascunho recuperado: o que você tinha preenchido da última vez está aqui.",
                    cx,
                )
                .id("nfe-rascunho-recuperado")
                .into_any_element(),
            );
        }
        if !self.aviso_do_modelo.is_empty() {
            avisos.push(
                faixa(
                    Gravidade::Aviso,
                    format!(
                        "Nota anterior usada como modelo. Número, data e protocolo não são copiados. {}",
                        self.aviso_do_modelo.join(" ")
                    ),
                    cx,
                )
                .id("nfe-aviso-do-modelo")
                .into_any_element(),
            );
        }
        if let Some(a) = self.aviso_cadastro.clone() {
            avisos.push(
                faixa(Gravidade::Aviso, a, cx)
                    .id("nfe-aviso-do-cadastro")
                    .into_any_element(),
            );
        }
        avisos
    }

    // ---------------------------------------------------------------- passo 1: para quem

    fn passo_para_quem(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let d = self.dados.clone();
        let documento = self.campo_de_texto(CampoNova::Documento);
        let nome = self.campo_de_texto(CampoNova::Nome);
        let ie = self.campo_de_texto(CampoNova::Ie);
        let email = self.campo_de_texto(CampoNova::Email);
        let cep = self.campo_de_texto(CampoNova::Cep);
        let logradouro = self.campo_de_texto(CampoNova::Logradouro);
        let numero = self.campo_de_texto(CampoNova::Numero);
        let complemento = self.campo_de_texto(CampoNova::Complemento);
        let bairro = self.campo_de_texto(CampoNova::Bairro);
        let cod = self.campo_de_texto(CampoNova::CodMunicipio);
        let municipio = municipio_do_codigo(&d.cod_municipio);
        let ajuda_municipio = match (&municipio, d.cod_municipio.len()) {
            (Some((nome, uf)), _) => format!("{nome} - {uf}"),
            (None, 7) => "Código não encontrado na tabela de municípios.".to_string(),
            _ => "Use Buscar endereço pelo CEP, ou digite os 7 dígitos do código IBGE.".to_string(),
        };
        let consumo = self.escolhas(
            "nfe-consumo",
            Consumo::TODOS
                .iter()
                .map(|c| (*c, c.rotulo().to_string()))
                .collect(),
            d.consumo,
            |d, v| d.consumo = Some(v),
            cx,
        );
        let contribuinte = self.escolhas(
            "nfe-contribuinte",
            Contribuinte::TODOS
                .iter()
                .map(|c| (*c, c.rotulo().to_string()))
                .collect(),
            d.contribuinte,
            |d, v| d.contribuinte = Some(v),
            cx,
        );
        let presenca = self.escolhas(
            "nfe-presenca",
            Presenca::TODAS
                .iter()
                .map(|c| (*c, c.rotulo().to_string()))
                .collect(),
            Some(d.presenca),
            |d, v| d.presenca = v,
            cx,
        );
        let tema_muted = cx.theme().muted_foreground;
        let travado = self.travado();
        let buscando = self.buscando_cep;
        let aviso_cep = self.aviso_cep.clone();
        let preenchido = self.cliente_preenchido.clone();
        let destino = d.destino(&self.ctx).map(|x| {
            format!(
                "Venda {}: CFOP {}xxx na tabela.",
                x.frase(),
                x.digito_do_cfop()
            )
        });
        let mut coluna = v_flex().gap_4().child(grupo(
            "Quem compra",
            Some("Digite o CPF ou o CNPJ: se este cliente já recebeu uma NF-e, o cadastro dele é completado."),
            cx,
        ));
        coluna = coluna.child(
            v_form()
                .columns(2)
                .child(campo("CPF ou CNPJ", true, None, None, documento).col_span(1))
                .child(campo("Nome ou razão social", true, None, None, nome).col_span(1)),
        );
        if let Some(p) = preenchido {
            coluna = coluna.child(faixa(Gravidade::Neutro, p, cx).id("nfe-cliente-preenchido"));
        }
        coluna = coluna
            .child(
                v_flex()
                    .gap_1p5()
                    .child(
                        div()
                            .text_sm()
                            .font_semibold()
                            .child("O cliente compra para..."),
                    )
                    .child(consumo),
            )
            .child(
                v_flex()
                    .gap_1p5()
                    .child(
                        div()
                            .text_sm()
                            .font_semibold()
                            .child("O cliente tem inscrição estadual?"),
                    )
                    .child(contribuinte),
            );
        if d.contribuinte == Some(Contribuinte::ComIe) {
            coluna =
                coluna.child(v_form().columns(2).child(
                    campo("Inscrição estadual do cliente", true, None, None, ie).col_span(1),
                ));
        }
        coluna = coluna
            .child(
                v_flex()
                    .gap_1p5()
                    .child(
                        div()
                            .text_sm()
                            .font_semibold()
                            .child("Como o cliente comprou?"),
                    )
                    .child(presenca),
            )
            .child(grupo("Endereço do cliente", None, cx))
            .child(
                h_flex()
                    .gap_2()
                    .items_end()
                    .child(
                        div()
                            .w(px(200.))
                            .child(campo("CEP", false, None, None, cep)),
                    )
                    .child(
                        Button::new("nfe-buscar-cep")
                            .label(if buscando {
                                "Buscando..."
                            } else {
                                "Buscar endereço pelo CEP"
                            })
                            .icon(IconName::Search)
                            .outline()
                            .small()
                            .disabled(travado || buscando)
                            .on_click(
                                cx.listener(|this, _, window, cx| this.buscar_cep(window, cx)),
                            ),
                    ),
            )
            .children(aviso_cep.map(|a| faixa(Gravidade::Aviso, a, cx).id("nfe-aviso-cep")))
            .child(
                v_form()
                    .columns(2)
                    .child(campo("Rua", true, None, None, logradouro).col_span(2))
                    .child(campo("Número", true, None, None, numero).col_span(1))
                    .child(campo("Complemento", false, None, None, complemento).col_span(1))
                    .child(campo("Bairro", true, None, None, bairro).col_span(1))
                    .child(
                        campo(
                            "Município (código IBGE)",
                            true,
                            Some(SharedString::from(ajuda_municipio)),
                            None,
                            cod,
                        )
                        .col_span(1),
                    )
                    .child(campo("E-mail do cliente", false, None, None, email).col_span(2)),
            )
            .child(
                Checkbox::new("nfe-exterior")
                    .label("O cliente mora no exterior")
                    .checked(d.exterior)
                    .disabled(travado)
                    .on_click(cx.listener(|this, marcou: &bool, _, cx| {
                        let m = *marcou;
                        this.escolher(move |d| d.exterior = m, cx);
                    })),
            );
        if let Some(t) = destino {
            coluna = coluna.child(div().text_xs().text_color(tema_muted).child(t));
        }
        coluna.into_any_element()
    }

    // ---------------------------------------------------------------- passo 3: como recebeu

    /// Escolheu a forma de pagamento: a prazo ja vem com a data da primeira parcela daqui a 30 dias (a pessoa muda).
    fn escolher_pagamento(&mut self, p: Pagamento, window: &mut Window, cx: &mut Context<Self>) {
        if self.travado() {
            return;
        }
        self.dados.pagamento = p;
        if p == Pagamento::APrazo && self.dados.primeiro_vencimento.trim().is_empty() {
            let v = (self.ctx.hoje + Duration::days(30))
                .format("%Y-%m-%d")
                .to_string();
            let texto = iso_para_data_br(&v);
            let normal = CampoNova::Vencimento.definir(&mut self.dados, &texto);
            self.por_no_campo(CampoNova::Vencimento, &normal, window, cx);
        }
        self.mudou(cx);
    }

    fn passo_como_recebeu(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let d = self.dados.clone();
        let total = d.totais().total;
        let parcelas = self.campo_de_texto(CampoNova::Parcelas);
        let venc = self.campo_de_texto(CampoNova::Vencimento);
        let tema_muted = cx.theme().muted_foreground;
        let travado = self.travado();
        let opcoes = Pagamento::TODAS
            .iter()
            .map(|p| (*p, p.rotulo().to_string()));
        let lista = h_flex()
            .id("nfe-pagamento-grupo")
            .flex_wrap()
            .gap_2()
            .children(opcoes.enumerate().map(|(i, (p, r))| {
                let marcado = d.pagamento == p;
                Button::new(SharedString::from(format!("nfe-pagamento-{i}")))
                    .label(r)
                    .small()
                    .when(marcado, |b| b.primary())
                    .when(!marcado, |b| b.outline())
                    .disabled(travado)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.escolher_pagamento(p, window, cx);
                    }))
            }));
        let mut coluna = v_flex()
            .gap_4()
            .child(grupo(
                "Como o cliente pagou",
                Some("O valor da nota é o que o cliente paga: produtos menos o desconto."),
                cx,
            ))
            .child(div().text_sm().child(format!(
                "Total da nota: R$ {}",
                centavos_para_dinheiro(total)
            )))
            .child(lista);
        if d.pagamento == Pagamento::APrazo {
            let linhas = d.parcelas_da_nota();
            coluna = coluna
                .child(
                    v_form()
                        .columns(2)
                        .child(
                            campo(
                                "Quantas parcelas",
                                true,
                                Some(SharedString::from(format!("De 2 a {MAX_PARCELAS}."))),
                                None,
                                parcelas,
                            )
                            .col_span(1),
                        )
                        .child(
                            campo(
                                "Vencimento da primeira parcela",
                                true,
                                Some("As outras vencem de mês em mês.".into()),
                                None,
                                venc,
                            )
                            .col_span(1),
                        ),
                )
                .child(div().text_xs().text_color(tema_muted).child(
                    "A prazo sai como boleto em parcelas (a fatura e as duplicatas vão na nota).",
                ));
            if !linhas.is_empty() {
                let mut tabela = v_flex().gap_1().id("nfe-parcelas-calculadas");
                for (k, (v, data)) in linhas.iter().enumerate() {
                    tabela = tabela.child(
                        h_flex()
                            .gap_3()
                            .text_sm()
                            .child(div().w(px(90.)).child(format!("Parcela {}", k + 1)))
                            .child(
                                div()
                                    .w(px(110.))
                                    .child(format!("R$ {}", centavos_para_dinheiro(*v))),
                            )
                            .child(div().child(data.format("%d/%m/%Y").to_string())),
                    );
                }
                coluna = coluna.child(tabela);
            }
        }
        if d.pagamento.e_cartao() {
            coluna = coluna.child(div().text_xs().text_color(tema_muted).child(
                "Cartão: a nota leva o pagamento como não integrado ao sistema de automação (a maquininha é à parte).",
            ));
        }
        coluna.into_any_element()
    }

    // ---------------------------------------------------------------- passo 4: como entrega

    fn passo_como_entrega(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let d = self.dados.clone();
        let lista = self.escolhas(
            "nfe-entrega",
            Entrega::TODAS
                .iter()
                .map(|e| (*e, e.rotulo().to_string()))
                .collect(),
            Some(d.entrega),
            |d, v| d.entrega = v,
            cx,
        );
        let tema_muted = cx.theme().muted_foreground;
        let mut coluna = v_flex()
            .gap_4()
            .child(grupo(
                "Como a mercadoria chega ao cliente",
                Some("A escolha vira o tipo de frete da nota."),
                cx,
            ))
            .child(lista)
            .child(div().text_xs().text_color(tema_muted).child(format!(
                "Tipo de frete na nota: {} (MOC, campo modFrete).",
                d.entrega.mod_frete()
            )));
        if d.entrega.com_transportadora() {
            coluna = coluna.child(grupo("Transportadora", None, cx)).child(
                v_form()
                    .columns(2)
                    .child(
                        campo(
                            "CNPJ ou CPF",
                            false,
                            None,
                            None,
                            self.campo_de_texto(CampoNova::TranspDocumento),
                        )
                        .col_span(1),
                    )
                    .child(
                        campo(
                            "Nome",
                            true,
                            None,
                            None,
                            self.campo_de_texto(CampoNova::TranspNome),
                        )
                        .col_span(1),
                    )
                    .child(
                        campo(
                            "Inscrição estadual",
                            false,
                            Some("ISENTO se for o caso.".into()),
                            None,
                            self.campo_de_texto(CampoNova::TranspIe),
                        )
                        .col_span(1),
                    )
                    .child(
                        campo(
                            "UF",
                            false,
                            None,
                            None,
                            self.campo_de_texto(CampoNova::TranspUf),
                        )
                        .col_span(1),
                    )
                    .child(
                        campo(
                            "Endereço",
                            false,
                            None,
                            None,
                            self.campo_de_texto(CampoNova::TranspEndereco),
                        )
                        .col_span(1),
                    )
                    .child(
                        campo(
                            "Cidade",
                            false,
                            None,
                            None,
                            self.campo_de_texto(CampoNova::TranspMunicipio),
                        )
                        .col_span(1),
                    ),
            );
        }
        if d.entrega != Entrega::Balcao {
            coluna = coluna.child(grupo("Volumes (se houver)", None, cx)).child(
                v_form()
                    .columns(3)
                    .child(
                        campo(
                            "Quantos",
                            false,
                            None,
                            None,
                            self.campo_de_texto(CampoNova::Volumes),
                        )
                        .col_span(1),
                    )
                    .child(
                        campo(
                            "Espécie",
                            false,
                            None,
                            None,
                            self.campo_de_texto(CampoNova::Especie),
                        )
                        .col_span(1),
                    )
                    .child(
                        campo(
                            "Peso bruto (kg)",
                            false,
                            None,
                            None,
                            self.campo_de_texto(CampoNova::PesoBruto),
                        )
                        .col_span(1),
                    ),
            );
        }
        coluna.into_any_element()
    }

    // ---------------------------------------------------------------- o rodape

    fn rodape(&self, cx: &mut Context<Self>) -> AnyElement {
        let passo = self.passo;
        let travado = self.travado();
        h_flex()
            .id("nfe-rodape")
            .gap_2()
            .justify_between()
            .items_center()
            .child(
                Button::new("nfe-voltar")
                    .label("Voltar")
                    .icon(IconName::ChevronLeft)
                    .outline()
                    .disabled(travado || passo.anterior().is_none())
                    .on_click(cx.listener(|this, _, window, cx| this.voltar(window, cx))),
            )
            .child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(match self.rascunho {
                                EstadoDoRascunho::Nenhum => "",
                                EstadoDoRascunho::Alterado => "Guardando o rascunho...",
                                EstadoDoRascunho::Salvo => "Rascunho guardado",
                            }),
                    )
                    .when(passo.proximo().is_some(), |d| {
                        d.child(
                            Button::new("nfe-continuar")
                                .label("Continuar")
                                .icon(IconName::ChevronRight)
                                .primary()
                                .disabled(travado || !self.pronta)
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.continuar(window, cx)),
                                ),
                        )
                    }),
            )
            .into_any_element()
    }
}

impl Render for NovaNfe {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let titulo = cabecalho(
            "Nova NF-e",
            "Venda de mercadoria: cinco passos, em palavras de balcão. O programa escolhe os termos fiscais e mostra o que escolheu.",
            cx,
        );
        if !self.pronta {
            return estado_vazio(
                IconName::Settings,
                "Falta configurar a NF-e",
                "Informe a UF e o regime da empresa na seção NF-e das Configurações. O emitente, o certificado e a pasta das notas são os mesmos da NFS-e.",
                vec![Button::new("nfe-ir-configurar")
                    .label("Abrir a configuração da NF-e")
                    .icon(IconName::Settings)
                    .primary()
                    .on_click(cx.listener(|this, _, _, cx| this.ir_para_configuracoes(cx)))
                    .into_any_element()],
                cx,
            )
            .into_any_element();
        }
        if let Fase::Desfecho(d) = self.fase.clone() {
            return self.tela_do_desfecho(*d, titulo, cx);
        }
        let enviando = match self.fase {
            Fase::Enviando(e) => Some(e),
            _ => None,
        };
        let avisos = self.avisos_do_topo(cx);
        let corpo: AnyElement = match self.passo {
            PassoNfe::ParaQuem => self.passo_para_quem(cx),
            PassoNfe::OQueVendeu => self.passo_o_que_vendeu(cx),
            PassoNfe::ComoRecebeu => self.passo_como_recebeu(cx),
            PassoNfe::ComoEntrega => self.passo_como_entrega(cx),
            PassoNfe::Conferir => self.passo_conferir(cx),
        };
        let pendencias = self.faixa_de_pendencias(self.passo, cx);
        let rodape = self.rodape(cx);
        let mut coluna = v_flex()
            .id("nova-nfe")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.rolagem)
            .gap_4()
            .px_6()
            .py_6()
            .child(centrada(
                v_flex()
                    .gap_3()
                    .child(titulo)
                    .child(self.selo_do_ambiente())
                    .child(self.lista_de_passos(cx)),
            ));
        for a in avisos {
            coluna = coluna.child(centrada(a));
        }
        if let Some(e) = enviando {
            coluna = coluna.child(centrada(self.cartao_enviando(e, cx)));
        }
        coluna = coluna.child(centrada(
            cartao(cx)
                .id("nfe-cartao-do-passo")
                .gap_4()
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(format!(
                            "{} · {}",
                            self.passo.indicador(),
                            self.passo.titulo()
                        )),
                )
                .child(corpo)
                .children(pendencias),
        ));
        coluna = coluna.child(centrada(rodape));
        coluna.into_any_element()
    }
}
