// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A tela Notas: a lista das notas da pasta de notas (lida fora da linha da tela, desenhada so' no que cabe na janela),
//! busca por texto, filtro por situacao e o detalhe da nota escolhida ao lado (ou como pagina, em janela estreita).
//! A lista, a busca e o filtro moram em `modelo::lista`; aqui so' ha desenho e ligacao de eventos.

use std::ops::Range;
use std::sync::Arc;

use gpui_kit::component::button::{Button, ButtonVariants as _, Toggle, ToggleGroup};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{
    h_flex, v_flex, ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, uniform_list, AnyElement, AppContext as _, Context, Entity, EventEmitter, FocusHandle,
    InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _, Render, ScrollStrategy,
    SharedString, StatefulInteractiveElement as _, Styled as _, Subscription,
    UniformListScrollHandle, Window,
};
use rustnf_app::config::Config;

use super::cancelar::Cancelamento;
use super::detalhe::{EventoDetalhe, VisaoDetalhe};
use super::pecas::{cabecalho, estado_vazio, faixa, selo_da_situacao, Negrito as _};
use crate::modelo::certificado::Gravidade;
use crate::modelo::lista::{contagem_de_notas, FiltroDeSituacao, ListaDeNotas};
use crate::modelo::nota::DadosNota;
use crate::servicos::Servicos;
use crate::telas::LARGURA_DA_LATERAL;

/// Largura minima da area de conteudo para o detalhe ficar ao lado da lista; abaixo disso ele e' uma pagina.
const LARGURA_PARA_DETALHE_AO_LADO: f32 = 1060.0;

/// Largura do detalhe quando fica ao lado da lista.
const LARGURA_DO_DETALHE: f32 = 480.0;

/// Altura de cada linha da lista (todas iguais: a lista so' desenha as que aparecem).
const ALTURA_DA_LINHA: f32 = 56.0;

/// O que a tela pede a janela.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum EventoNotas {
    /// "Usar como modelo": a Nova nota recebe estes dados.
    UsarComoModelo(Box<DadosNota>),
    /// Ir a Nova nota (o botao da pasta vazia).
    IrParaNovaNota,
}

/// O estado da leitura da pasta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Carga {
    /// Ainda nao leu.
    Nova,
    /// Lendo a pasta pela primeira vez.
    Carregando,
    /// Nao deu para ler: a frase diz por que.
    Falha(String),
    /// Leu.
    Pronta,
}

/// A tela.
pub(super) struct TelaNotas {
    servicos: Arc<Servicos>,
    config: Config,
    pronta: bool,
    pub(super) carga: Carga,
    /// Lendo de novo (a lista antiga continua na tela ate a nova chegar).
    atualizando: bool,
    /// Pediram uma leitura enquanto outra rodava: a que esta rodando pode ter lido antes da mudanca, entao le de novo.
    reler: bool,
    pub(super) lista: ListaDeNotas,
    busca: Entity<InputState>,
    pub(super) detalhe: Entity<VisaoDetalhe>,
    /// A chave da nota escolhida.
    pub(super) escolhida: Option<String>,
    /// A linha em foco para o teclado (posicao entre as visiveis).
    em_foco: Option<usize>,
    /// Em janela estreita, o detalhe e' uma pagina: este e' o botao "Voltar" dela.
    detalhe_aberto: bool,
    rolagem: UniformListScrollHandle,
    foco: FocusHandle,
    versao: u64,
    _subs: Vec<Subscription>,
}

impl EventEmitter<EventoNotas> for TelaNotas {}

impl TelaNotas {
    /// A tela, ainda sem configuracao.
    pub(super) fn new(
        servicos: Arc<Servicos>,
        cancelamento: Entity<Cancelamento>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let busca = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Buscar por nome, CPF/CNPJ ou número")
        });
        let detalhe = {
            let servicos = Arc::clone(&servicos);
            cx.new(|cx| VisaoDetalhe::new(servicos, cancelamento, window, cx))
        };
        let s1 = cx.subscribe_in(
            &busca,
            window,
            |this, entrada, evento: &InputEvent, window, cx| match evento {
                InputEvent::Change => {
                    let texto = entrada.read(cx).value().to_string();
                    this.lista.definir_busca(&texto);
                    this.em_foco = None;
                    cx.notify();
                }
                InputEvent::PressEnter { .. } if this.lista.quantas() > 0 => {
                    this.abrir(this.em_foco.unwrap_or(0), window, cx);
                }
                _ => {}
            },
        );
        let s2 = cx.subscribe_in(&detalhe, window, |_, _, evento: &EventoDetalhe, _, cx| {
            if let EventoDetalhe::UsarComoModelo(d) = evento {
                cx.emit(EventoNotas::UsarComoModelo(d.clone()));
            }
        });
        Self {
            servicos,
            config: Config::default(),
            pronta: false,
            carga: Carga::Nova,
            atualizando: false,
            reler: false,
            lista: ListaDeNotas::default(),
            busca,
            detalhe,
            escolhida: None,
            em_foco: None,
            detalhe_aberto: false,
            rolagem: UniformListScrollHandle::new(),
            foco: cx.focus_handle(),
            versao: 0,
            _subs: vec![s1, s2],
        }
    }

    /// A janela entrega a configuracao (a efetiva) quando ela chega ou muda. Configuracao completa: le a pasta.
    pub(super) fn definir_config(
        &mut self,
        config: Config,
        completa: bool,
        cx: &mut Context<Self>,
    ) {
        let mudou_a_pasta = self.config.notas.pasta != config.notas.pasta;
        self.config = config.clone();
        self.pronta = completa;
        self.detalhe
            .update(cx, |d, cx| d.definir_config(config, cx));
        if completa && (self.carga == Carga::Nova || mudou_a_pasta) {
            self.lista = ListaDeNotas::default();
            self.escolhida = None;
            self.detalhe.update(cx, |d, cx| d.limpar(cx));
            self.recarregar(cx);
        }
        cx.notify();
    }

    /// Le a pasta de notas de novo, fora da tela. A lista, a busca, o filtro e a nota escolhida continuam como estavam.
    pub(super) fn recarregar(&mut self, cx: &mut Context<Self>) {
        if !self.pronta {
            return;
        }
        if self.atualizando {
            self.reler = true;
            return;
        }
        self.atualizando = true;
        if self.carga != Carga::Pronta {
            self.carga = Carga::Carregando;
        }
        cx.notify();
        self.versao += 1;
        let versao = self.versao;
        let servicos = Arc::clone(&self.servicos);
        let config = self.config.clone();
        cx.spawn(async move |this, cx| {
            let r = cx
                .background_spawn(async move { servicos.listar_notas(&config) })
                .await;
            this.update(cx, |this, cx| {
                this.atualizando = false;
                if std::mem::take(&mut this.reler) {
                    this.recarregar(cx);
                    return;
                }
                if this.versao != versao {
                    return;
                }
                match r {
                    Ok(registros) => {
                        this.lista = this.lista.recarregada(&registros);
                        this.carga = Carga::Pronta;
                    }
                    Err(e) => this.carga = Carga::Falha(e.texto()),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Uma nota foi cancelada: atualiza o detalhe aberto e a lista.
    pub(super) fn cancelada(&mut self, chave: &str, cx: &mut Context<Self>) {
        let chave = chave.to_string();
        self.detalhe
            .update(cx, |d, cx| d.marcar_cancelada(&chave, cx));
        self.recarregar(cx);
    }

    /// Abre o detalhe da nota na posicao `i` entre as visiveis.
    pub(super) fn abrir(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(chave) = self.lista.linha(i).map(|l| l.chave.clone()) else {
            return;
        };
        self.em_foco = Some(i);
        self.escolhida = Some(chave.clone());
        self.detalhe_aberto = true;
        self.detalhe
            .update(cx, |d, cx| d.mostrar_da_pasta(&chave, cx));
        self.foco.focus(window, cx);
        cx.notify();
    }

    fn mover_foco(&mut self, passo: isize, cx: &mut Context<Self>) {
        let total = self.lista.quantas();
        if total == 0 {
            return;
        }
        let atual = self.em_foco.map_or(-1, |i| isize::try_from(i).unwrap_or(0));
        let novo = (atual + passo).clamp(0, isize::try_from(total).unwrap_or(1) - 1);
        let novo = usize::try_from(novo).unwrap_or(0);
        self.em_foco = Some(novo);
        self.rolagem.scroll_to_item(novo, ScrollStrategy::Nearest);
        cx.notify();
    }

    fn ao_apertar(&mut self, tecla: &str, window: &mut Window, cx: &mut Context<Self>) {
        match tecla {
            "down" => self.mover_foco(1, cx),
            "up" => self.mover_foco(-1, cx),
            "enter" => {
                if let Some(i) = self.em_foco {
                    self.abrir(i, window, cx);
                }
            }
            "escape" if self.detalhe_aberto => {
                self.detalhe_aberto = false;
                cx.notify();
            }
            _ => {}
        }
    }

    fn escolher_filtro(&mut self, filtro: FiltroDeSituacao, cx: &mut Context<Self>) {
        self.lista.definir_filtro(filtro);
        self.em_foco = None;
        cx.notify();
    }

    fn limpar_a_busca(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.busca.update(cx, |e, cx| e.set_value("", window, cx));
        self.lista.definir_busca("");
        self.lista.definir_filtro(FiltroDeSituacao::Todas);
        cx.notify();
    }

    // ---------------------------------------------------------------- desenho

    fn linha(&self, i: usize, cx: &mut Context<Self>) -> AnyElement {
        let Some(l) = self.lista.linha(i).cloned() else {
            return div().into_any_element();
        };
        let tema = cx.theme();
        let (borda, muted, selecionada, hover) = (
            tema.border,
            tema.muted_foreground,
            tema.accent,
            tema.accent.opacity(0.5),
        );
        let escolhida = self.escolhida.as_deref() == Some(l.chave.as_str());
        let em_foco = self.em_foco == Some(i);
        let tomador = if l.tomador.is_empty() {
            "(tomador não identificado)".to_string()
        } else {
            l.tomador.clone()
        };
        // `div()` do kit (e nao `h_flex()`): so' ele deixa a linha ser achada pelos testes da janela
        div()
            .flex()
            .flex_row()
            .items_center()
            .id(SharedString::from(format!("nota-{i}")))
            .h(px(ALTURA_DA_LINHA))
            .w_full()
            .px_3()
            .gap_3()
            .items_center()
            .border_b_1()
            .border_color(borda)
            .cursor_pointer()
            .when(escolhida || em_foco, |d| d.bg(selecionada))
            .when(!(escolhida || em_foco), |d| d.hover(move |s| s.bg(hover)))
            .child(
                div()
                    .w(px(84.))
                    .flex_shrink_0()
                    .text_sm()
                    .child(l.data.clone()),
            )
            .child(
                div()
                    .w(px(56.))
                    .flex_shrink_0()
                    .text_sm()
                    .font_semibold()
                    .child(l.numero.clone()),
            )
            .child(
                div()
                    .w(px(84.))
                    .flex_shrink_0()
                    .text_xs()
                    .text_color(muted)
                    .child(l.dps.clone()),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(div().text_sm().truncate().child(tomador))
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .truncate()
                            .child(l.documento.clone()),
                    ),
            )
            .child(
                div()
                    .w(px(112.))
                    .flex_shrink_0()
                    .text_sm()
                    .text_right()
                    .child(format!("R$ {}", l.valor)),
            )
            .child(
                h_flex()
                    .w(px(96.))
                    .flex_shrink_0()
                    .justify_end()
                    .child(selo_da_situacao(l.situacao, cx)),
            )
            .on_click(cx.listener(move |this, _, window, cx| this.abrir(i, window, cx)))
            .into_any_element()
    }

    fn cabecalho_da_lista(&self, cx: &mut Context<Self>) -> AnyElement {
        let tema = cx.theme();
        let (borda, muted) = (tema.border, tema.muted_foreground);
        let titulo = |t: &'static str, w: Option<f32>, direita: bool| {
            div()
                .when_some(w, |d, w| d.w(px(w)).flex_shrink_0())
                .when(w.is_none(), |d| d.flex_1().min_w_0())
                .when(direita, |d| d.text_right())
                .text_xs()
                .font_semibold()
                .text_color(muted)
                .child(t.to_uppercase())
        };
        h_flex()
            .id("cabecalho-da-lista")
            .w_full()
            .h(px(32.))
            .px_3()
            .gap_3()
            .items_center()
            .border_b_1()
            .border_color(borda)
            .child(titulo("Data", Some(84.), false))
            .child(titulo("Nº", Some(56.), false))
            .child(titulo("DPS", Some(84.), false))
            .child(titulo("Tomador", None, false))
            .child(titulo("Valor", Some(112.), true))
            .child(titulo("Situação", Some(96.), true))
            .into_any_element()
    }

    fn barra(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let muted = cx.theme().muted_foreground;
        let atual = self.lista.filtro();
        let filtro = ToggleGroup::new("filtro-de-notas")
            .segmented()
            .single()
            .children(FiltroDeSituacao::TODOS.iter().enumerate().map(|(i, f)| {
                Toggle::new(SharedString::from(format!("filtro-notas-{i}")))
                    .label(f.rotulo())
                    .checked(atual == *f)
            }))
            .on_click(cx.listener(|this, estados: &Vec<bool>, _, cx| {
                if let Some(i) = estados.iter().position(|e| *e) {
                    if let Some(f) = FiltroDeSituacao::TODOS.get(i) {
                        this.escolher_filtro(*f, cx);
                    }
                }
            }));
        let contagem = if self.lista.vazia() {
            String::new()
        } else {
            contagem_de_notas(self.lista.quantas(), self.lista.total())
        };
        h_flex()
            .gap_3()
            .items_center()
            .flex_wrap()
            .child(
                div().w(px(340.)).child(
                    Input::new(&self.busca)
                        .id("busca-de-notas")
                        .prefix(Icon::new(IconName::Search).small()),
                ),
            )
            .child(filtro)
            .child(
                div()
                    .id("contagem-de-notas")
                    .text_sm()
                    .text_color(muted)
                    .child(contagem),
            )
            .child(div().flex_1())
            .child(
                Button::new("atualizar-notas")
                    .label("Atualizar")
                    .icon(IconName::RefreshCw)
                    .outline()
                    .small()
                    .loading(self.atualizando)
                    .disabled(self.atualizando)
                    .on_click(cx.listener(|this, _, _, cx| this.recarregar(cx))),
            )
            .into_any_element()
    }

    fn corpo_da_lista(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let muted = cx.theme().muted_foreground;
        match self.carga.clone() {
            Carga::Nova | Carga::Carregando => h_flex()
                .id("notas-carregando")
                .p_8()
                .gap_2()
                .text_sm()
                .text_color(muted)
                .child(Icon::new(IconName::LoaderCircle))
                .child("Lendo a pasta de notas...")
                .into_any_element(),
            Carga::Falha(texto) => v_flex()
                .id("notas-com-erro")
                .gap_3()
                .p_4()
                .child(faixa(Gravidade::Erro, texto, cx))
                .child(
                    h_flex().child(
                        Button::new("notas-tentar-de-novo")
                            .label("Tentar de novo")
                            .icon(IconName::RefreshCw)
                            .primary()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.carga = Carga::Nova;
                                this.recarregar(cx);
                            })),
                    ),
                )
                .into_any_element(),
            Carga::Pronta if self.lista.vazia() => estado_vazio(
                IconName::Inbox,
                "Nenhuma nota na pasta ainda",
                "As notas autorizadas pela Sefin Nacional aparecem aqui. Emita a primeira, ou consulte uma nota pela chave e guarde-a na pasta.",
                vec![Button::new("notas-ir-para-nova")
                    .label("Emitir uma nota")
                    .icon(IconName::FileText)
                    .primary()
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(EventoNotas::IrParaNovaNota)))
                    .into_any_element()],
                cx,
            )
            .id("notas-vazia")
            .into_any_element(),
            Carga::Pronta if self.lista.sem_resultado() => estado_vazio(
                IconName::Search,
                "Nenhuma nota encontrada",
                "Nenhuma nota da pasta bate com a busca e o filtro. Confira o texto ou limpe a busca.",
                vec![Button::new("notas-limpar-busca")
                    .label("Limpar a busca")
                    .outline()
                    .on_click(cx.listener(|this, _, window, cx| this.limpar_a_busca(window, cx)))
                    .into_any_element()],
                cx,
            )
            .id("notas-sem-resultado")
            .into_any_element(),
            Carga::Pronta => {
                let total = self.lista.quantas();
                v_flex()
                    .size_full()
                    .child(self.cabecalho_da_lista(cx))
                    .child(
                        div().flex_1().min_h_0().child(
                            uniform_list(
                                "lista-de-notas",
                                total,
                                cx.processor(|this, intervalo: Range<usize>, _, cx| {
                                    intervalo
                                        .map(|i| this.linha(i, cx))
                                        .collect::<Vec<AnyElement>>()
                                }),
                            )
                            .track_scroll(&self.rolagem)
                            .size_full(),
                        ),
                    )
                    .into_any_element()
            }
        }
    }

    fn coluna_da_lista(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let corpo = self.corpo_da_lista(cx);
        v_flex()
            .size_full()
            .gap_3()
            .px_6()
            .py_6()
            .child(cabecalho(
                "Notas",
                "As notas da pasta de notas, da mais recente para a mais antiga. Enter abre o detalhe.",
                cx,
            ))
            .child(self.barra(cx))
            .child(div().flex_1().min_h_0().child(corpo))
            .into_any_element()
    }

    fn painel_do_detalhe(&mut self, ao_lado: bool, cx: &mut Context<Self>) -> AnyElement {
        let borda = cx.theme().border;
        let rolavel = div()
            .id("painel-do-detalhe")
            .size_full()
            .overflow_y_scroll()
            .p_5()
            .child(self.detalhe.clone());
        if ao_lado {
            div()
                .w(px(LARGURA_DO_DETALHE))
                .flex_shrink_0()
                .h_full()
                .border_l_1()
                .border_color(borda)
                .child(rolavel)
                .into_any_element()
        } else {
            v_flex()
                .size_full()
                .child(
                    h_flex().px_6().pt_5().child(
                        Button::new("voltar-a-lista")
                            .label("Voltar à lista")
                            .icon(IconName::ChevronLeft)
                            .outline()
                            .small()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.detalhe_aberto = false;
                                cx.notify();
                            })),
                    ),
                )
                .child(div().flex_1().min_h_0().child(rolavel))
                .into_any_element()
        }
    }
}

impl Render for TelaNotas {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let largura = window.viewport_size().width - px(LARGURA_DA_LATERAL);
        let ao_lado = largura >= px(LARGURA_PARA_DETALHE_AO_LADO);
        let base = div()
            .id("tela-notas")
            .key_context("TelaNotas")
            .track_focus(&self.foco)
            .size_full()
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                this.ao_apertar(ev.keystroke.key.as_str(), window, cx);
            }));
        if ao_lado {
            let lista = self.coluna_da_lista(cx);
            let detalhe = self.painel_do_detalhe(true, cx);
            base.child(
                h_flex()
                    .size_full()
                    .child(div().flex_1().min_w_0().h_full().child(lista))
                    .child(detalhe),
            )
        } else if self.detalhe_aberto && self.detalhe.read(cx).tem_nota() {
            base.child(self.painel_do_detalhe(false, cx))
        } else {
            base.child(self.coluna_da_lista(cx))
        }
    }
}
