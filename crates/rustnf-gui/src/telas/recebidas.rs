// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! As NF-e RECEBIDAS (as compras: notas que fornecedores emitiram contra a empresa): a lista (lida fora da linha da tela e
//! desenhada so' no que cabe na janela), a busca, o filtro, o botao "Buscar notas novas" com a ultima busca e a espera do
//! Ambiente Nacional, e o detalhe ao lado (ou como pagina, em janela estreita). E' o desenho da lista da NF-e emitida, com
//! as colunas de quem compra: o que chegou, de quem, quanto e o que falta responder. O que decide mora em
//! `modelo::nfe_recebidas`; buscar, baixar e responder moram em `recebidas_dialogo`.

use std::ops::Range;
use std::sync::Arc;

use gpui_kit::component::button::{Button, ButtonVariants as _, Toggle, ToggleGroup};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{
    h_flex, v_flex, ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, uniform_list, AnyElement, AppContext as _, Context, Entity, FocusHandle,
    InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _, Render, ScrollStrategy,
    SharedString, StatefulInteractiveElement as _, Styled as _, Subscription,
    UniformListScrollHandle, Window,
};
use rustnf_app::config::Config;

use super::notas::{Carga, ALTURA_DA_LINHA, LARGURA_DO_DETALHE, LARGURA_PARA_DETALHE_AO_LADO};
use super::pecas::{cabecalho, cor_da, estado_vazio, faixa, selo_de, Negrito as _};
use super::recebidas_dialogo::{DialogoRecebidas, EventoDialogoRecebidas, OperacaoRecebida};
use crate::modelo::certificado::Gravidade;
use crate::modelo::nfe_recebidas::{FilaDaTela, FiltroRecebidas, ListaRecebidas};
use crate::modelo::nfe_recebidas_resultados::DesfechoBusca;
use crate::servicos::Servicos;
use crate::telas::LARGURA_DA_LATERAL;

mod detalhe;

/// O que a tela pede a janela.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum EventoRecebidas {
    /// Abrir as Configuracoes, na secao da NF-e (o que falta configurar).
    IrParaConfiguracoes,
}

/// A tela das NF-e recebidas.
pub(super) struct TelaRecebidas {
    servicos: Arc<Servicos>,
    config: Config,
    /// A configuracao da NF-e esta completa (a comum mais a UF): sem isso a tela explica o que falta.
    pronta: bool,
    pub(super) carga: Carga,
    atualizando: bool,
    reler: bool,
    pub(super) lista: ListaRecebidas,
    /// A ultima busca e a espera do Ambiente Nacional, como a pasta tem.
    pub(super) fila: FilaDaTela,
    /// O que a ultima busca respondeu (fica na tela ate a proxima acao).
    pub(super) ultima_resposta: Option<DesfechoBusca>,
    busca: Entity<InputState>,
    pub(super) dialogo: Entity<DialogoRecebidas>,
    /// A chave da nota escolhida.
    pub(super) escolhida: Option<String>,
    em_foco: Option<usize>,
    detalhe_aberto: bool,
    rolagem: UniformListScrollHandle,
    foco: FocusHandle,
    versao: u64,
    detalhe: detalhe::Conteudo,
    versao_do_detalhe: u64,
    danfe: detalhe::EstadoDoDanfe,
    erro: Option<String>,
    chave_copiada: bool,
    _subs: Vec<Subscription>,
}

impl gpui_kit::EventEmitter<EventoRecebidas> for TelaRecebidas {}

impl TelaRecebidas {
    pub(super) fn new(
        servicos: Arc<Servicos>,
        dialogo: Entity<DialogoRecebidas>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let busca =
            cx.new(|cx| InputState::new(window, cx).placeholder("Fornecedor, CNPJ ou chave"));
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
        let s2 = cx.subscribe_in(
            &dialogo,
            window,
            |this, _, evento: &EventoDialogoRecebidas, _, cx| {
                match evento {
                    EventoDialogoRecebidas::MudouAPasta => {
                        this.recarregar(cx);
                        this.reler_o_detalhe(cx);
                    }
                    EventoDialogoRecebidas::Buscou(d) => this.ultima_resposta = Some(d.clone()),
                }
                cx.notify();
            },
        );
        Self {
            servicos,
            config: Config::default(),
            pronta: false,
            carga: Carga::Nova,
            atualizando: false,
            reler: false,
            lista: ListaRecebidas::default(),
            fila: FilaDaTela::default(),
            ultima_resposta: None,
            busca,
            dialogo,
            escolhida: None,
            em_foco: None,
            detalhe_aberto: false,
            rolagem: UniformListScrollHandle::new(),
            foco: cx.focus_handle(),
            versao: 0,
            detalhe: detalhe::Conteudo::Vazio,
            versao_do_detalhe: 0,
            danfe: detalhe::EstadoDoDanfe::Parado,
            erro: None,
            chave_copiada: false,
            _subs: vec![s1, s2],
        }
    }

    /// O detalhe da nota escolhida ja foi lido (os testes esperam por ele).
    #[cfg(test)]
    #[allow(dead_code)]
    pub(super) fn detalhe_pronto(&self) -> bool {
        matches!(self.detalhe, detalhe::Conteudo::Lida(_))
    }

    /// A janela entrega a configuracao (a efetiva) quando ela chega ou muda. `pronta`: a NF-e pode ser usada.
    pub(super) fn definir_config(&mut self, config: Config, pronta: bool, cx: &mut Context<Self>) {
        let mudou = self.config.notas.pasta != config.notas.pasta
            || self.config.nfe.ambiente != config.nfe.ambiente;
        self.config = config;
        self.pronta = pronta;
        if pronta && (self.carga == Carga::Nova || mudou) {
            self.lista = ListaRecebidas::default();
            self.escolhida = None;
            self.detalhe = detalhe::Conteudo::Vazio;
            self.ultima_resposta = None;
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
                .background_spawn(async move {
                    let fila = servicos.fila_das_recebidas(&config).unwrap_or_default();
                    servicos.listar_recebidas(&config).map(|l| (l, fila))
                })
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
                    Ok((registros, fila)) => {
                        this.lista = this.lista.recarregada(&registros);
                        this.fila = fila;
                        this.carga = Carga::Pronta;
                        this.marcar_o_fim_da_espera(cx);
                    }
                    Err(e) => this.carga = Carga::Falha(e.texto()),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Quando a espera acaba, a tela se redesenha sozinha (o botao volta a funcionar na hora, sem a pessoa fazer nada).
    fn marcar_o_fim_da_espera(&mut self, cx: &mut Context<Self>) {
        let agora = self.servicos.agora();
        let Some(ate) = self.fila.esperando(agora) else {
            return;
        };
        let falta =
            (ate - agora).to_std().unwrap_or_default() + std::time::Duration::from_millis(250);
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(falta).await;
            this.update(cx, |_, cx| cx.notify()).ok();
        })
        .detach();
    }

    /// "Buscar notas novas": abre o dialogo, que busca sozinho. Antes da hora, nada sai.
    pub(super) fn buscar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.pronta || self.fila.esperando(self.servicos.agora()).is_some() {
            return;
        }
        self.dialogo.update(cx, |d, cx| {
            d.iniciar(OperacaoRecebida::Buscar, None, window, cx);
        });
    }

    /// Abre o detalhe da nota na posicao `i` entre as visiveis.
    pub(super) fn abrir(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(chave) = self.lista.linha(i).map(|l| l.chave.clone()) else {
            return;
        };
        self.em_foco = Some(i);
        self.escolhida = Some(chave.clone());
        self.detalhe_aberto = true;
        self.ler_o_detalhe(&chave, cx);
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

    fn escolher_filtro(&mut self, filtro: FiltroRecebidas, cx: &mut Context<Self>) {
        self.lista.definir_filtro(filtro);
        self.em_foco = None;
        cx.notify();
    }

    fn limpar_a_busca(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.busca.update(cx, |e, cx| e.set_value("", window, cx));
        self.lista.definir_busca("");
        self.lista.definir_filtro(FiltroRecebidas::Todas);
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
        let nome = if l.fornecedor.is_empty() {
            "(fornecedor não identificado)".to_string()
        } else {
            l.fornecedor.clone()
        };
        let (nota, resposta) = (
            selo_de(l.nota.rotulo(), cor_da(l.nota.gravidade(), cx)),
            selo_de(l.resposta.rotulo(), cor_da(l.resposta.gravidade(), cx)),
        );
        // `div()` do kit (e nao `h_flex()`): so' ele deixa a linha ser achada pelos testes da janela
        div()
            .flex()
            .flex_row()
            .items_center()
            .id(SharedString::from(format!("recebida-{i}")))
            .h(px(ALTURA_DA_LINHA))
            .w_full()
            .px_3()
            .gap_3()
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
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(div().text_sm().truncate().child(nome))
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
                    .w(px(104.))
                    .flex_shrink_0()
                    .text_sm()
                    .text_right()
                    .child(format!("R$ {}", l.valor)),
            )
            .child(
                h_flex()
                    .w(px(150.))
                    .flex_shrink_0()
                    .justify_end()
                    .child(nota),
            )
            .child(
                h_flex()
                    .w(px(116.))
                    .flex_shrink_0()
                    .justify_end()
                    .child(resposta),
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
            .id("recebidas-cabecalho-da-lista")
            .w_full()
            .h(px(32.))
            .px_3()
            .gap_3()
            .items_center()
            .border_b_1()
            .border_color(borda)
            .child(titulo("Data", Some(84.), false))
            .child(titulo("Fornecedor", None, false))
            .child(titulo("Valor", Some(104.), true))
            .child(titulo("A nota", Some(150.), true))
            .child(titulo("Sua resposta", Some(116.), true))
            .into_any_element()
    }

    fn barra(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let muted = cx.theme().muted_foreground;
        let atual = self.lista.filtro();
        let filtro = ToggleGroup::new("recebidas-filtro")
            .segmented()
            .single()
            .children(FiltroRecebidas::TODOS.iter().enumerate().map(|(i, f)| {
                Toggle::new(SharedString::from(format!("recebidas-filtro-{i}")))
                    .label(f.rotulo())
                    .checked(atual == *f)
            }))
            .on_click(cx.listener(|this, estados: &Vec<bool>, _, cx| {
                if let Some(f) = estados
                    .iter()
                    .position(|e| *e)
                    .and_then(|i| FiltroRecebidas::TODOS.get(i))
                {
                    this.escolher_filtro(*f, cx);
                }
            }));
        let contagem = if self.lista.vazia() {
            String::new()
        } else {
            self.lista.contagem()
        };
        let esperando = self.fila.esperando(self.servicos.agora()).is_some();
        h_flex()
            .gap_3()
            .items_center()
            .flex_wrap()
            .child(
                div().flex_1().min_w(px(200.)).max_w(px(340.)).child(
                    Input::new(&self.busca)
                        .id("recebidas-busca")
                        .prefix(Icon::new(IconName::Search).small()),
                ),
            )
            .child(filtro)
            .child(
                div()
                    .id("recebidas-contagem")
                    .text_sm()
                    .text_color(muted)
                    .child(contagem),
            )
            .child(div().flex_1())
            .child(
                Button::new("recebidas-buscar")
                    .label("Buscar notas novas")
                    .icon(IconName::RefreshCw)
                    .primary()
                    .small()
                    .disabled(!self.pronta || esperando)
                    .on_click(cx.listener(|this, _, window, cx| this.buscar(window, cx))),
            )
            .child(
                Button::new("recebidas-atualizar")
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

    /// A ultima busca e, se o Ambiente Nacional mandou esperar, ate quando.
    fn linha_da_fila(&self, cx: &mut Context<Self>) -> AnyElement {
        let muted = cx.theme().muted_foreground;
        let agora = self.servicos.agora();
        v_flex()
            .gap_0p5()
            .child(
                div()
                    .id("recebidas-ultima-busca")
                    .text_xs()
                    .text_color(muted)
                    .child(self.fila.texto_da_ultima_busca()),
            )
            .children(self.fila.texto_da_espera(agora).map(|t| {
                div()
                    .id("recebidas-espera")
                    .text_xs()
                    .font_semibold()
                    .text_color(cor_da(Gravidade::Aviso, cx))
                    .child(t)
            }))
            .into_any_element()
    }

    /// O destaque do que a pessoa tem de fazer, e o que a ultima busca respondeu.
    fn avisos(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let mut v = Vec::new();
        if let Some(d) = self.ultima_resposta.clone() {
            v.push(
                faixa(d.gravidade(), format!("{}. {}", d.titulo(), d.texto()), cx)
                    .id("recebidas-resultado-da-busca")
                    .into_any_element(),
            );
        }
        let n = self.lista.a_responder();
        if n > 0 && self.carga == Carga::Pronta {
            v.push(
                faixa(
                    Gravidade::Aviso,
                    format!(
                        "{} Abra a nota e escolha Responder: confirmar que recebeu, dar ciência (\"vi a nota\") ou dizer que não é sua.",
                        crate::modelo::nfe_resultados::contagem(
                            n,
                            "nota espera uma resposta da sua empresa.",
                            "notas esperam uma resposta da sua empresa."
                        ),
                    ),
                    cx,
                )
                .id("recebidas-a-responder")
                .into_any_element(),
            );
        }
        v
    }

    fn corpo_da_lista(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let muted = cx.theme().muted_foreground;
        match self.carga.clone() {
            Carga::Nova | Carga::Carregando => h_flex()
                .id("recebidas-carregando")
                .p_8()
                .gap_2()
                .text_sm()
                .text_color(muted)
                .child(Icon::new(IconName::LoaderCircle))
                .child("Lendo a pasta de notas...")
                .into_any_element(),
            Carga::Falha(texto) => v_flex()
                .id("recebidas-com-erro")
                .gap_3()
                .p_4()
                .child(faixa(Gravidade::Erro, texto, cx))
                .child(
                    h_flex().child(
                        Button::new("recebidas-tentar-de-novo-a-pasta")
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
                "Nenhuma nota recebida ainda",
                "Aqui aparecem as notas que seus fornecedores emitiram contra a sua empresa. Use Buscar notas novas para pedir ao Ambiente Nacional as que existem para o CNPJ da empresa.",
                Vec::new(),
                cx,
            )
            .id("recebidas-vazia")
            .into_any_element(),
            Carga::Pronta if self.lista.sem_resultado() => estado_vazio(
                IconName::Search,
                "Nenhuma nota encontrada",
                "Nenhuma nota recebida bate com a busca e o filtro. Confira o texto ou limpe a busca.",
                vec![Button::new("recebidas-limpar-busca")
                    .label("Limpar a busca")
                    .outline()
                    .on_click(cx.listener(|this, _, window, cx| this.limpar_a_busca(window, cx)))
                    .into_any_element()],
                cx,
            )
            .id("recebidas-sem-resultado")
            .into_any_element(),
            Carga::Pronta => {
                let total = self.lista.quantas();
                v_flex()
                    .size_full()
                    .child(self.cabecalho_da_lista(cx))
                    .child(
                        div().flex_1().min_h_0().child(
                            uniform_list(
                                "recebidas-lista",
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
        let avisos = self.avisos(cx);
        let fila = self.linha_da_fila(cx);
        v_flex()
            .size_full()
            .gap_3()
            .px_6()
            .py_4()
            .child(cabecalho(
                "Recebidas (compras)",
                "As notas que seus fornecedores emitiram contra a sua empresa, da mais recente para a mais antiga. Enter abre o detalhe.",
                cx,
            ))
            .child(self.barra(cx))
            .child(fila)
            .children(avisos)
            .child(div().flex_1().min_h_0().child(corpo))
            .into_any_element()
    }

    fn painel_do_detalhe(&mut self, ao_lado: bool, cx: &mut Context<Self>) -> AnyElement {
        let borda = cx.theme().border;
        let corpo = self.corpo_do_detalhe(cx);
        let rolavel = div()
            .id("recebidas-painel-do-detalhe")
            .size_full()
            .overflow_y_scroll()
            .p_5()
            .child(corpo);
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
                        Button::new("recebidas-voltar-a-lista")
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

    /// O que falta configurar, com o botao que leva as Configuracoes.
    fn falta_configurar(&mut self, cx: &mut Context<Self>) -> AnyElement {
        estado_vazio(
            IconName::Settings,
            "Falta configurar a NF-e",
            "Para ver as notas recebidas, informe nas Configurações, na seção NF-e, a UF do emitente. O CNPJ da empresa, o certificado e a pasta de notas são os mesmos da NFS-e.",
            vec![Button::new("recebidas-ir-para-configuracoes")
                .label("Abrir as configurações da NF-e")
                .icon(IconName::Settings)
                .primary()
                .on_click(cx.listener(|_, _, _, cx| cx.emit(EventoRecebidas::IrParaConfiguracoes)))
                .into_any_element()],
            cx,
        )
        .id("recebidas-falta-configurar")
        .into_any_element()
    }
}

impl Render for TelaRecebidas {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let largura = window.viewport_size().width - px(LARGURA_DA_LATERAL);
        let ao_lado = largura >= px(LARGURA_PARA_DETALHE_AO_LADO);
        let base = div()
            .id("tela-recebidas")
            .key_context("TelaNotas")
            .track_focus(&self.foco)
            .size_full()
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                this.ao_apertar(ev.keystroke.key.as_str(), window, cx);
            }));
        if !self.pronta {
            return base.child(self.falta_configurar(cx));
        }
        if ao_lado {
            let lista = self.coluna_da_lista(cx);
            let detalhe = self.painel_do_detalhe(true, cx);
            base.child(
                h_flex()
                    .size_full()
                    .child(div().flex_1().min_w_0().h_full().child(lista))
                    .child(detalhe),
            )
        } else if self.detalhe_aberto && !matches!(self.detalhe, detalhe::Conteudo::Vazio) {
            base.child(self.painel_do_detalhe(false, cx))
        } else {
            base.child(self.coluna_da_lista(cx))
        }
    }
}
