// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A tela Produtos: o cadastro que a Nova NF-e usa (lista, busca, editar, apagar). Os produtos novos entram sozinhos quando
//! uma NF-e e' autorizada; aqui a pessoa corrige o que precisar (NCM, valor, origem) e apaga o que nao vende mais. O
//! detalhe fiscal que ela fixou numa nota aparece e pode ser limpo, mas nao e' editado aqui (isso se faz na nota).

use std::collections::BTreeMap;
use std::sync::Arc;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::form::v_form;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{
    h_flex, v_flex, ActiveTheme as _, Disableable as _, IconName, Sizable as _, WindowExt as _,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, AnyElement, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _,
    Subscription, Window,
};
use rustnf_app::config::Config;

use super::pecas::{cabecalho, campo, cartao, estado_vazio, faixa, Negrito as _, LARGURA_FORM};
use crate::modelo::certificado::Gravidade;
use crate::modelo::nfe_campos::{formatar_ncm, CampoItem};
use crate::modelo::nfe_fiscal::Origem;
use crate::modelo::nfe_nova::ItemNfe;
use crate::modelo::nfe_produtos::{Cadastro, Produto};
use crate::servicos::Servicos;

/// Os campos de texto da edicao de um produto.
const CAMPOS: [CampoItem; 6] = [
    CampoItem::Codigo,
    CampoItem::Descricao,
    CampoItem::Unidade,
    CampoItem::Valor,
    CampoItem::Ncm,
    CampoItem::Cest,
];

/// O que se esta editando.
#[derive(Debug, Clone, PartialEq)]
enum Edicao {
    /// Nada.
    Nenhuma,
    /// Um produto novo.
    Novo,
    /// O produto da posicao (na lista inteira).
    Existente(usize),
}

/// A tela.
pub(super) struct TelaProdutos {
    servicos: Arc<Servicos>,
    config: Config,
    pronta: bool,
    cadastro: Cadastro,
    aviso: Option<String>,
    carregado: bool,
    busca: Entity<InputState>,
    edicao: Edicao,
    /// O produto em edicao, como item (os campos de texto o preenchem).
    buffer: ItemNfe,
    entradas: BTreeMap<CampoItem, Entity<InputState>>,
    erro: Option<String>,
    salvando: bool,
    _subs: Vec<Subscription>,
}

impl TelaProdutos {
    pub(super) fn new(
        servicos: Arc<Servicos>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let busca =
            cx.new(|cx| InputState::new(window, cx).placeholder("Buscar por nome, código ou NCM"));
        let mut subs = vec![
            cx.subscribe_in(&busca, window, |_, _, ev: &InputEvent, _, cx| {
                if matches!(ev, InputEvent::Change) {
                    cx.notify();
                }
            }),
        ];
        let mut entradas = BTreeMap::new();
        for c in CAMPOS {
            let e = cx.new(|cx| InputState::new(window, cx).placeholder(c.dica()));
            subs.push(cx.subscribe_in(
                &e,
                window,
                move |this, entrada, ev: &InputEvent, window, cx| {
                    if matches!(ev, InputEvent::Change) {
                        let texto = entrada.read(cx).value().to_string();
                        let normal = c.definir(&mut this.buffer, &texto);
                        if normal != texto {
                            entrada.update(cx, |e, cx| e.set_value(normal, window, cx));
                        }
                        this.erro = None;
                        cx.notify();
                    }
                },
            ));
            entradas.insert(c, e);
        }
        Self {
            servicos,
            config: Config::default(),
            pronta: false,
            cadastro: Cadastro::default(),
            aviso: None,
            carregado: false,
            busca,
            edicao: Edicao::Nenhuma,
            buffer: ItemNfe::default(),
            entradas,
            erro: None,
            salvando: false,
            _subs: subs,
        }
    }

    /// O erro que a tela mostra no formulario (os testes conferem).
    #[cfg(test)]
    pub(super) fn erro_atual(&self) -> Option<&str> {
        self.erro.as_deref()
    }

    /// Quantos produtos o cadastro tem (os testes esperam por isso).
    #[cfg(test)]
    pub(super) fn total(&self) -> usize {
        self.cadastro.produtos.len()
    }

    /// A janela entrega a configuracao (a efetiva).
    pub(super) fn definir_config(&mut self, config: Config, pronta: bool, cx: &mut Context<Self>) {
        let mudou = self.config.notas.pasta != config.notas.pasta;
        self.config = config;
        self.pronta = pronta;
        if pronta && (!self.carregado || mudou) {
            self.recarregar(cx);
        }
        cx.notify();
    }

    /// Le o cadastro de novo, fora da tela (a Nova NF-e pode ter guardado produtos).
    pub(super) fn recarregar(&mut self, cx: &mut Context<Self>) {
        if !self.pronta {
            return;
        }
        self.carregado = true;
        let servicos = Arc::clone(&self.servicos);
        let config = self.config.clone();
        cx.spawn(async move |this, cx| {
            let lido = cx
                .background_spawn(async move { servicos.ler_produtos(&config) })
                .await;
            this.update(cx, |this, cx| {
                this.cadastro = lido.cadastro;
                this.aviso = lido.aviso;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn abrir_edicao(&mut self, edicao: Edicao, window: &mut Window, cx: &mut Context<Self>) {
        self.buffer = match &edicao {
            Edicao::Existente(i) => self
                .cadastro
                .produtos
                .get(*i)
                .map_or_else(ItemNfe::default, Produto::para_item),
            _ => ItemNfe::default(),
        };
        self.edicao = edicao;
        self.erro = None;
        for c in CAMPOS {
            let texto = c.texto(&self.buffer);
            if let Some(e) = self.entradas.get(&c) {
                e.update(cx, |e, cx| e.set_value(texto, window, cx));
            }
        }
        cx.notify();
    }

    fn salvar(&mut self, cx: &mut Context<Self>) {
        if self.salvando {
            return;
        }
        let produto = Produto::do_item(&self.buffer);
        if produto.descricao.is_empty() {
            self.erro = Some("Falta dizer o que é o produto.".into());
            cx.notify();
            return;
        }
        if !produto.completo() {
            self.erro = Some("O NCM precisa de 8 dígitos. O programa não tem a tabela do NCM: procure na TIPI da Receita.".into());
            cx.notify();
            return;
        }
        let anterior = match &self.edicao {
            Edicao::Existente(i) => self.cadastro.produtos.get(*i).cloned(),
            _ => None,
        };
        self.salvando = true;
        let servicos = Arc::clone(&self.servicos);
        let config = self.config.clone();
        cx.spawn(async move |this, cx| {
            let r = cx
                .background_spawn(async move {
                    // se o codigo mudou, o antigo sai
                    if let Some(a) = anterior.filter(|a| a.codigo != produto.codigo) {
                        servicos.apagar_produto(&config, &a)?;
                    }
                    servicos.guardar_produtos(&config, vec![produto])?;
                    Ok::<_, crate::servicos::ErroTela>(servicos.ler_produtos(&config))
                })
                .await;
            this.update(cx, |this, cx| {
                this.salvando = false;
                match r {
                    Ok(lido) => {
                        this.cadastro = lido.cadastro;
                        this.aviso = lido.aviso;
                        this.edicao = Edicao::Nenhuma;
                    }
                    Err(e) => this.erro = Some(e.texto()),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn pedir_para_apagar(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(p) = self.cadastro.produtos.get(i).cloned() else {
            return;
        };
        let tela = cx.entity();
        window.open_alert_dialog(cx, move |alerta, _, _| {
            let (tela, p) = (tela.clone(), p.clone());
            alerta
                .title(format!("Apagar \"{}\" do cadastro?", p.descricao))
                .description("As notas já emitidas não mudam. O produto só deixa de aparecer na busca da Nova NF-e.")
                .confirm()
                .ok_text("Apagar")
                .cancel_text("Manter")
                .on_ok(move |_, window, cx| {
                    let p = p.clone();
                    tela.update(cx, |t, cx| t.apagar(p, window, cx));
                    true
                })
        });
    }

    pub(super) fn apagar(&mut self, p: Produto, window: &mut Window, cx: &mut Context<Self>) {
        let servicos = Arc::clone(&self.servicos);
        let config = self.config.clone();
        cx.spawn_in(window, async move |this, cx| {
            let r = cx
                .background_spawn(async move {
                    servicos.apagar_produto(&config, &p)?;
                    Ok::<_, crate::servicos::ErroTela>(servicos.ler_produtos(&config))
                })
                .await;
            this.update_in(cx, |this, window, cx| {
                match r {
                    Ok(lido) => {
                        this.cadastro = lido.cadastro;
                        this.aviso = lido.aviso;
                        this.edicao = Edicao::Nenhuma;
                    }
                    Err(e) => window.push_notification(
                        Notification::warning(e.texto()).title("Não foi possível apagar"),
                        cx,
                    ),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn limpar_o_fixado(&mut self, cx: &mut Context<Self>) {
        self.buffer.icms = None;
        self.buffer.aliquota_icms.clear();
        self.buffer.cfop = None;
        self.buffer.icms_codigo = None;
        self.buffer.pis_cst = None;
        cx.notify();
    }

    fn formulario(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let campo_de = |c: CampoItem, s: &Self| -> AnyElement {
            match s.entradas.get(&c) {
                Some(e) => Input::new(e)
                    .id(SharedString::from(format!("produto-{c:?}")))
                    .into_any_element(),
                None => div().into_any_element(),
            }
        };
        let muted = cx.theme().muted_foreground;
        let origem = self.buffer.origem;
        let fixado = self.buffer.icms.is_some()
            || self.buffer.cfop.is_some()
            || self.buffer.icms_codigo.is_some()
            || self.buffer.pis_cst.is_some();
        let titulo = if self.edicao == Edicao::Novo {
            "Produto novo"
        } else {
            "Editar o produto"
        };
        let mut c = cartao(cx)
            .id("produto-em-edicao")
            .child(div().font_semibold().child(titulo))
            .child(
                v_form()
                    .columns(3)
                    .child(
                        campo(
                            "O que é",
                            true,
                            None,
                            None,
                            campo_de(CampoItem::Descricao, self),
                        )
                        .col_span(2),
                    )
                    .child(
                        campo(
                            "Código",
                            false,
                            Some("Chave do produto no cadastro.".into()),
                            None,
                            campo_de(CampoItem::Codigo, self),
                        )
                        .col_span(1),
                    )
                    .child(
                        campo(
                            "Unidade",
                            true,
                            None,
                            None,
                            campo_de(CampoItem::Unidade, self),
                        )
                        .col_span(1),
                    )
                    .child(
                        campo(
                            "Valor de cada (R$)",
                            false,
                            None,
                            None,
                            campo_de(CampoItem::Valor, self),
                        )
                        .col_span(1),
                    )
                    .child(
                        campo(
                            "NCM",
                            true,
                            Some("8 dígitos (TIPI da Receita).".into()),
                            None,
                            campo_de(CampoItem::Ncm, self),
                        )
                        .col_span(1),
                    )
                    .child(
                        campo("CEST", false, None, None, campo_de(CampoItem::Cest, self))
                            .col_span(1),
                    ),
            )
            .child(
                h_flex()
                    .gap_2()
                    .flex_wrap()
                    .children(Origem::TODAS.iter().map(|o| {
                        let o = *o;
                        Button::new(SharedString::from(format!("produto-origem-{}", o.rotulo())))
                            .label(o.rotulo())
                            .small()
                            .when(origem == o, |b| b.primary())
                            .when(origem != o, |b| b.outline())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.buffer.origem = o;
                                cx.notify();
                            }))
                    })),
            );
        if fixado {
            c = c.child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(div().text_xs().text_color(muted).child(
                        "Este produto guarda um detalhe fiscal fixado numa nota (ICMS, CFOP ou CST). Ele vale em toda nota nova.",
                    ))
                    .child(
                        Button::new("produto-limpar-fixado")
                            .label("Limpar o detalhe fixado")
                            .ghost()
                            .small()
                            .on_click(cx.listener(|this, _, _, cx| this.limpar_o_fixado(cx))),
                    ),
            );
        }
        if let Some(e) = self.erro.clone() {
            c = c.child(faixa(Gravidade::Erro, e, cx).id("produto-erro"));
        }
        c.child(
            h_flex()
                .gap_2()
                .child(
                    Button::new("produto-salvar")
                        .label(if self.salvando {
                            "Guardando..."
                        } else {
                            "Guardar"
                        })
                        .primary()
                        .disabled(self.salvando)
                        .on_click(cx.listener(|this, _, _, cx| this.salvar(cx))),
                )
                .child(
                    Button::new("produto-cancelar")
                        .label("Cancelar")
                        .outline()
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.abrir_edicao(Edicao::Nenhuma, window, cx);
                        })),
                ),
        )
        .into_any_element()
    }
}

impl Render for TelaProdutos {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.pronta {
            return estado_vazio(
                IconName::Settings,
                "Falta configurar a NF-e",
                "O cadastro de produtos mora na pasta das notas: complete a configuração (emitente, certificado e pasta das notas) primeiro.",
                Vec::new(),
                cx,
            )
            .into_any_element();
        }
        let muted = cx.theme().muted_foreground;
        let termo = self.busca.read(cx).value().to_string();
        let todos = self.cadastro.produtos.clone();
        let achados = self.cadastro.indices(&termo, usize::MAX);
        let mut coluna = v_flex().gap_4().child(cabecalho(
            "Produtos",
            "O que a loja vende, com NCM e origem, para a próxima NF-e não pedir tudo de novo. Os produtos novos entram aqui quando a nota é autorizada.",
            cx,
        ));
        if let Some(a) = self.aviso.clone() {
            coluna = coluna.child(faixa(Gravidade::Aviso, a, cx).id("produtos-aviso"));
        }
        coluna = coluna.child(
            h_flex()
                .gap_2()
                .child(
                    div()
                        .flex_1()
                        .child(Input::new(&self.busca).id("produtos-busca")),
                )
                .child(
                    Button::new("produtos-novo")
                        .label("Produto novo")
                        .icon(IconName::Plus)
                        .primary()
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.abrir_edicao(Edicao::Novo, window, cx);
                        })),
                ),
        );
        if self.edicao != Edicao::Nenhuma {
            coluna = coluna.child(self.formulario(cx));
        }
        if todos.is_empty() {
            coluna = coluna.child(
                faixa(
                    Gravidade::Neutro,
                    "O cadastro está vazio. Use Produto novo, ou emita uma NF-e: os produtos dela entram aqui sozinhos.",
                    cx,
                )
                .id("produtos-vazio"),
            );
        } else if achados.is_empty() {
            coluna = coluna.child(
                div()
                    .text_sm()
                    .text_color(muted)
                    .child("Nenhum produto combina com a busca."),
            );
        } else {
            let mut lista = v_flex().gap_2().id("produtos-lista");
            for i in achados {
                let p = &todos[i];
                let resumo = format!(
                    "{}NCM {} · {} · {}",
                    if p.codigo.is_empty() {
                        String::new()
                    } else {
                        format!("Cód. {} · ", p.codigo)
                    },
                    formatar_ncm(&p.ncm),
                    if p.valor.is_empty() {
                        "sem valor".to_string()
                    } else {
                        format!("R$ {}", p.valor)
                    },
                    p.origem.rotulo(),
                );
                lista = lista.child(
                    h_flex()
                        .id(SharedString::from(format!("produto-linha-{i}")))
                        .gap_3()
                        .items_center()
                        .justify_between()
                        .p_3()
                        .rounded_md()
                        .border_1()
                        .border_color(cx.theme().border)
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .gap_0p5()
                                .child(div().text_sm().font_semibold().child(p.descricao.clone()))
                                .child(div().text_xs().text_color(muted).child(resumo))
                                .when(!p.fixado.vazio(), |d| {
                                    d.child(
                                        div()
                                            .text_xs()
                                            .text_color(muted)
                                            .child("Detalhe fiscal fixado"),
                                    )
                                }),
                        )
                        .child(
                            h_flex()
                                .gap_2()
                                .child(
                                    Button::new(SharedString::from(format!("produto-editar-{i}")))
                                        .label("Editar")
                                        .outline()
                                        .small()
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.abrir_edicao(Edicao::Existente(i), window, cx);
                                        })),
                                )
                                .child(
                                    Button::new(SharedString::from(format!("produto-apagar-{i}")))
                                        .label("Apagar")
                                        .icon(IconName::Delete)
                                        .ghost()
                                        .small()
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.pedir_para_apagar(i, window, cx);
                                        })),
                                ),
                        ),
                );
            }
            coluna = coluna.child(lista);
        }
        div()
            .id("tela-produtos")
            .size_full()
            .overflow_y_scroll()
            .child(
                div()
                    .w_full()
                    .max_w(px(LARGURA_FORM))
                    .mx_auto()
                    .px_6()
                    .py_6()
                    .child(coluna),
            )
            .into_any_element()
    }
}
