// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! As NF-e da pasta de notas (venda de mercadoria): a lista (lida fora da linha da tela e desenhada so' no que cabe na
//! janela), a busca, o filtro, o destaque das notas sem resposta com o botao Retomar, e o detalhe ao lado (ou como
//! pagina, em janela estreita). E' o desenho da lista da NFS-e, com as colunas da NF-e. O que decide mora em
//! `modelo::nfe_lista`; o cancelamento, a carta, a inutilizacao e a retomada moram em `nfe_dialogo`.

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

use super::nfe_dialogo::{DialogoNfe, EventoDialogoNfe, Operacao};
use super::notas::{Carga, ALTURA_DA_LINHA, LARGURA_DO_DETALHE, LARGURA_PARA_DETALHE_AO_LADO};
use super::pecas::{cabecalho, estado_vazio, faixa, selo_da_situacao_nfe, Negrito as _};
use crate::modelo::certificado::Gravidade;
use crate::modelo::nfe_lista::{FiltroNfe, ListaNfe};
use crate::servicos::Servicos;
use crate::telas::LARGURA_DA_LATERAL;

mod detalhe;

/// O que a tela pede a janela.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EventoNfe {
    /// Abrir as Configuracoes, na secao da NF-e (o que falta configurar).
    IrParaConfiguracoes,
}

/// A tela das NF-e.
pub(super) struct TelaNfe {
    servicos: Arc<Servicos>,
    config: Config,
    /// A configuracao da NF-e esta completa (a comum mais a UF): sem isso a tela explica o que falta.
    pronta: bool,
    pub(super) carga: Carga,
    atualizando: bool,
    reler: bool,
    pub(super) lista: ListaNfe,
    busca: Entity<InputState>,
    pub(super) dialogo: Entity<DialogoNfe>,
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

impl EventEmitter<EventoNfe> for TelaNfe {}

impl TelaNfe {
    pub(super) fn new(
        servicos: Arc<Servicos>,
        dialogo: Entity<DialogoNfe>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let busca =
            cx.new(|cx| InputState::new(window, cx).placeholder("Nome, CPF/CNPJ, nº ou chave"));
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
        let s2 = cx.subscribe_in(&dialogo, window, |this, _, _: &EventoDialogoNfe, _, cx| {
            this.recarregar(cx);
            this.reler_o_detalhe(cx);
        });
        Self {
            servicos,
            config: Config::default(),
            pronta: false,
            carga: Carga::Nova,
            atualizando: false,
            reler: false,
            lista: ListaNfe::default(),
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
    pub(super) fn detalhe_pronto(&self) -> bool {
        matches!(self.detalhe, detalhe::Conteudo::Lida(_))
    }

    /// A janela entrega a configuracao (a efetiva) quando ela chega ou muda. `pronta`: a NF-e pode ser usada.
    pub(super) fn definir_config(&mut self, config: Config, pronta: bool, cx: &mut Context<Self>) {
        let mudou = self.config.notas.pasta != config.notas.pasta;
        self.config = config;
        self.pronta = pronta;
        if pronta && (self.carga == Carga::Nova || mudou) {
            self.lista = ListaNfe::default();
            self.escolhida = None;
            self.detalhe = detalhe::Conteudo::Vazio;
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
                .background_spawn(async move { servicos.listar_nfe(&config) })
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

    fn escolher_filtro(&mut self, filtro: FiltroNfe, cx: &mut Context<Self>) {
        self.lista.definir_filtro(filtro);
        self.em_foco = None;
        cx.notify();
    }

    fn limpar_a_busca(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.busca.update(cx, |e, cx| e.set_value("", window, cx));
        self.lista.definir_busca("");
        self.lista.definir_filtro(FiltroNfe::Todas);
        cx.notify();
    }

    fn retomar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let n = self.lista.a_resolver();
        self.dialogo.update(cx, |d, cx| {
            d.iniciar(Operacao::Retomar, None, n, window, cx)
        });
    }

    fn inutilizar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.dialogo.update(cx, |d, cx| {
            d.iniciar(Operacao::Inutilizar, None, 0, window, cx)
        });
    }

    // ---------------------------------------------------------------- desenho

    fn linha(&self, i: usize, cx: &mut Context<Self>) -> AnyElement {
        let Some(l) = self.lista.linha(i).cloned() else {
            return div().into_any_element();
        };
        let tema = cx.theme();
        let (borda, muted, selecionada, hover, aviso) = (
            tema.border,
            tema.muted_foreground,
            tema.accent,
            tema.accent.opacity(0.5),
            super::pecas::cor_da(Gravidade::Aviso, cx),
        );
        let escolhida = self.escolhida.as_deref() == Some(l.chave.as_str());
        let em_foco = self.em_foco == Some(i);
        let nome = if l.destinatario.is_empty() {
            "(destinatário não identificado)".to_string()
        } else {
            l.destinatario.clone()
        };
        let segunda =
            if l.aviso.is_some() && l.situacao != rustnf_app::notas::nfe::Situacao::Denegada {
                Some(("Sem resposta: use Retomar".to_string(), aviso))
            } else {
                Some((l.documento.clone(), muted))
            };
        // `div()` do kit (e nao `h_flex()`): so' ele deixa a linha ser achada pelos testes da janela
        div()
            .flex()
            .flex_row()
            .items_center()
            .id(SharedString::from(format!("nfe-{i}")))
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
                    .w(px(72.))
                    .flex_shrink_0()
                    .child(
                        div()
                            .text_sm()
                            .font_semibold()
                            .child(format!("nº {}", l.numero)),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .child(format!("série {}", l.serie)),
                    ),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(div().text_sm().truncate().child(nome))
                    .children(
                        segunda.map(|(t, cor)| div().text_xs().text_color(cor).truncate().child(t)),
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
                    .w(px(136.))
                    .flex_shrink_0()
                    .justify_end()
                    .child(selo_da_situacao_nfe(l.situacao, cx)),
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
            .id("nfe-cabecalho-da-lista")
            .w_full()
            .h(px(32.))
            .px_3()
            .gap_3()
            .items_center()
            .border_b_1()
            .border_color(borda)
            .child(titulo("Data", Some(84.), false))
            .child(titulo("Nota", Some(72.), false))
            .child(titulo("Destinatário", None, false))
            .child(titulo("Valor", Some(112.), true))
            .child(titulo("Situação", Some(136.), true))
            .into_any_element()
    }

    fn barra(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let muted = cx.theme().muted_foreground;
        let atual = self.lista.filtro();
        let filtro = ToggleGroup::new("nfe-filtro")
            .segmented()
            .single()
            .children(FiltroNfe::TODOS.iter().enumerate().map(|(i, f)| {
                Toggle::new(SharedString::from(format!("nfe-filtro-{i}")))
                    .label(f.rotulo())
                    .checked(atual == *f)
            }))
            .on_click(cx.listener(|this, estados: &Vec<bool>, _, cx| {
                if let Some(f) = estados
                    .iter()
                    .position(|e| *e)
                    .and_then(|i| FiltroNfe::TODOS.get(i))
                {
                    this.escolher_filtro(*f, cx);
                }
            }));
        let contagem = if self.lista.vazia() {
            String::new()
        } else {
            self.lista.contagem()
        };
        h_flex()
            .gap_3()
            .items_center()
            .flex_wrap()
            .child(
                div().flex_1().min_w(px(200.)).max_w(px(340.)).child(
                    Input::new(&self.busca)
                        .id("nfe-busca")
                        .prefix(Icon::new(IconName::Search).small()),
                ),
            )
            .child(filtro)
            .child(
                div()
                    .id("nfe-contagem")
                    .text_sm()
                    .text_color(muted)
                    .child(contagem),
            )
            .child(div().flex_1())
            .child(
                Button::new("nfe-inutilizar")
                    .label("Inutilizar números")
                    .icon(IconName::Ban)
                    .outline()
                    .small()
                    .disabled(!self.pronta)
                    .on_click(cx.listener(|this, _, window, cx| this.inutilizar(window, cx))),
            )
            .child(
                Button::new("nfe-atualizar")
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

    /// O destaque das notas sem resposta: o que aconteceu e o botao Retomar.
    fn faixa_dos_pendentes(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let n = self.lista.a_resolver();
        if n == 0 || self.carga != Carga::Pronta {
            return None;
        }
        Some(
            v_flex()
                .id("nfe-pendentes")
                .gap_2()
                .child(faixa(
                    Gravidade::Aviso,
                    format!(
                        "{n} {} enviada(s) sem resposta da SEFAZ. Elas podem ou não ter sido autorizadas. Retomar consulta cada uma pela mesma chave e, só se a SEFAZ não a recebeu, reenvia a mesma nota: nunca cria uma nota nova no lugar.",
                        if n == 1 { "nota" } else { "notas" }
                    ),
                    cx,
                ))
                .child(
                    h_flex().child(
                        Button::new("nfe-retomar")
                            .label("Retomar as notas sem resposta")
                            .icon(IconName::RefreshCw)
                            .primary()
                            .on_click(cx.listener(|this, _, window, cx| this.retomar(window, cx))),
                    ),
                )
                .into_any_element(),
        )
    }

    fn corpo_da_lista(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let muted = cx.theme().muted_foreground;
        match self.carga.clone() {
            Carga::Nova | Carga::Carregando => h_flex()
                .id("nfe-carregando")
                .p_8()
                .gap_2()
                .text_sm()
                .text_color(muted)
                .child(Icon::new(IconName::LoaderCircle))
                .child("Lendo a pasta de notas...")
                .into_any_element(),
            Carga::Falha(texto) => v_flex()
                .id("nfe-com-erro")
                .gap_3()
                .p_4()
                .child(faixa(Gravidade::Erro, texto, cx))
                .child(
                    h_flex().child(
                        Button::new("nfe-tentar-de-novo")
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
                "Nenhuma NF-e na pasta ainda",
                "As NF-e autorizadas pela SEFAZ aparecem aqui. A emissão pela janela chega na próxima versão; por enquanto, emita pela linha de comando (rustnf nfe emitir) e as notas aparecem nesta lista.",
                Vec::new(),
                cx,
            )
            .id("nfe-vazia")
            .into_any_element(),
            Carga::Pronta if self.lista.sem_resultado() => estado_vazio(
                IconName::Search,
                "Nenhuma nota encontrada",
                "Nenhuma NF-e da pasta bate com a busca e o filtro. Confira o texto ou limpe a busca.",
                vec![Button::new("nfe-limpar-busca")
                    .label("Limpar a busca")
                    .outline()
                    .on_click(cx.listener(|this, _, window, cx| this.limpar_a_busca(window, cx)))
                    .into_any_element()],
                cx,
            )
            .id("nfe-sem-resultado")
            .into_any_element(),
            Carga::Pronta => {
                let total = self.lista.quantas();
                v_flex()
                    .size_full()
                    .child(self.cabecalho_da_lista(cx))
                    .child(
                        div().flex_1().min_h_0().child(
                            uniform_list(
                                "nfe-lista",
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
        let pendentes = self.faixa_dos_pendentes(cx);
        v_flex()
            .size_full()
            .gap_3()
            .px_6()
            .py_4()
            .child(cabecalho(
                "NF-e (venda de mercadoria)",
                "As NF-e da pasta de notas, da mais recente para a mais antiga. Enter abre o detalhe.",
                cx,
            ))
            .child(self.barra(cx))
            .children(pendentes)
            .child(div().flex_1().min_h_0().child(corpo))
            .into_any_element()
    }

    fn painel_do_detalhe(&mut self, ao_lado: bool, cx: &mut Context<Self>) -> AnyElement {
        let borda = cx.theme().border;
        let corpo = self.corpo_do_detalhe(cx);
        let rolavel = div()
            .id("nfe-painel-do-detalhe")
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
                        Button::new("nfe-voltar-a-lista")
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
            "Para ver e mexer nas NF-e, informe nas Configurações, na seção NF-e, a UF do emitente. Os dados do emitente, o certificado e a pasta de notas são os mesmos da NFS-e.",
            vec![Button::new("nfe-ir-para-configuracoes")
                .label("Abrir as configurações da NF-e")
                .icon(IconName::Settings)
                .primary()
                .on_click(cx.listener(|_, _, _, cx| cx.emit(EventoNfe::IrParaConfiguracoes)))
                .into_any_element()],
            cx,
        )
        .id("nfe-falta-configurar")
        .into_any_element()
    }
}

impl Render for TelaNfe {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let largura = window.viewport_size().width - px(LARGURA_DA_LATERAL);
        let ao_lado = largura >= px(LARGURA_PARA_DETALHE_AO_LADO);
        let base = div()
            .id("tela-nfe")
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
