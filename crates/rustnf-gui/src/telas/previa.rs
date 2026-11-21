// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A tela Previa: a calculadora avulsa do IBS e da CBS. Nao emite nada e nao pede certificado nem pasta de notas: usa
//! as aliquotas publicadas da configuracao que houver. A conta e o quadro de resultado sao os da Nova nota; os campos
//! e a conta moram em `modelo::previa_avulsa`.

use std::collections::BTreeMap;
use std::sync::Arc;

use gpui_kit::component::button::{Button, Toggle, ToggleGroup};
use gpui_kit::component::form::v_form;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::select::{SearchableVec, Select, SelectEvent, SelectState};
use gpui_kit::component::{
    h_flex, v_flex, ActiveTheme as _, Icon, IconName, IndexPath, Sizable as _,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, AnyElement, AppContext as _, Context, Entity, EventEmitter, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, SharedString, StatefulInteractiveElement as _,
    Styled as _, Subscription, Window,
};
use rustnf_app::config::Config;

use super::pecas::{
    cabecalho, campo, cartao, corpo_da_previa, grupo, titulo_do_bloco, Negrito as _,
};
use crate::modelo::formulario::{OPCOES_APURACAO, OPCOES_SIMPLES};
use crate::modelo::pedido::Contexto;
use crate::modelo::previa::PreviaIbsCbs;
use crate::modelo::previa_avulsa::{
    buscar_classificacao, CampoPrevia, Classificacao, DadosDaPrevia,
};
use crate::modelo::sugestao::descricao_da_classe;
use crate::servicos::Servicos;
use crate::telas::pecas::LARGURA_FORM;
use crate::telas::LARGURA_DA_LATERAL;

/// Largura minima da area de conteudo para o resultado ficar ao lado dos campos.
const LARGURA_PARA_RESULTADO_AO_LADO: f32 = 960.0;

/// Largura do resultado quando fica ao lado.
const LARGURA_DO_RESULTADO: f32 = 380.0;

/// O que a tela pede a janela.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EventoPrevia {
    /// Ir as Configuracoes, na aba das aliquotas.
    IrParaAliquotas,
}

/// A tela.
pub(super) struct TelaPrevia {
    servicos: Arc<Servicos>,
    ctx: Contexto,
    pub(super) dados: DadosDaPrevia,
    entradas: BTreeMap<CampoPrevia, Entity<InputState>>,
    busca: Entity<InputState>,
    pub(super) resultados: Vec<Classificacao>,
    pub(super) previa: PreviaIbsCbs,
    apuracao: Entity<SelectState<SearchableVec<&'static str>>>,
    /// A pessoa ja mexeu em algum campo: a configuracao que chega depois nao refaz a tela.
    mexeu: bool,
    _subs: Vec<Subscription>,
}

impl EventEmitter<EventoPrevia> for TelaPrevia {}

fn dica(campo: CampoPrevia) -> &'static str {
    match campo {
        CampoPrevia::Data => "DD/MM/AAAA",
        CampoPrevia::ClassTrib => "6 dígitos",
        _ => "0,00",
    }
}

impl TelaPrevia {
    /// A tela, com a calculadora vazia (a configuracao a ajusta quando chega).
    pub(super) fn new(
        servicos: Arc<Servicos>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let ctx = servicos.contexto_da_nota(&Config::default());
        let dados = DadosDaPrevia::nova(&ctx);
        let mut entradas = BTreeMap::new();
        for campo in CampoPrevia::TODOS {
            let e = cx.new(|cx| InputState::new(window, cx).placeholder(dica(campo)));
            entradas.insert(campo, e);
        }
        let busca = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Buscar a classificação por palavra ou código")
        });
        let rotulos: Vec<&'static str> = OPCOES_APURACAO.iter().map(|(_, r)| *r).collect();
        let apuracao = cx
            .new(|cx| SelectState::new(SearchableVec::new(rotulos), None::<IndexPath>, window, cx));
        let mut subs = Vec::new();
        let pares: Vec<(CampoPrevia, Entity<InputState>)> =
            entradas.iter().map(|(c, e)| (*c, e.clone())).collect();
        for (campo, e) in pares {
            subs.push(cx.subscribe_in(
                &e,
                window,
                move |this, entrada, evento: &InputEvent, window, cx| {
                    if matches!(evento, InputEvent::Change) {
                        let texto = entrada.read(cx).value().to_string();
                        this.ao_digitar(campo, &texto, window, cx);
                    }
                },
            ));
        }
        subs.push(cx.subscribe_in(
            &busca,
            window,
            |this, entrada, evento: &InputEvent, _, cx| {
                if matches!(evento, InputEvent::Change) {
                    let termo = entrada.read(cx).value().to_string();
                    this.resultados =
                        buscar_classificacao(&termo, this.dados.data_iso().as_deref());
                    cx.notify();
                }
            },
        ));
        subs.push(cx.subscribe_in(
            &apuracao,
            window,
            |this, _, evento: &SelectEvent<SearchableVec<&'static str>>, _, cx| {
                if let SelectEvent::Confirm(Some(rotulo)) = evento {
                    if let Some((codigo, _)) = OPCOES_APURACAO.iter().find(|(_, r)| r == rotulo) {
                        this.dados.regime_apuracao = *codigo;
                        this.mexeu = true;
                        this.recalcular();
                        cx.notify();
                    }
                }
            },
        ));
        let previa = dados.calcular(&ctx.publicadas);
        Self {
            servicos,
            ctx,
            dados,
            entradas,
            busca,
            resultados: Vec::new(),
            previa,
            apuracao,
            mexeu: false,
            _subs: subs,
        }
    }

    /// A janela entrega a configuracao (a efetiva) quando ela chega ou muda: as aliquotas publicadas valem na hora; o
    /// regime e a data de hoje so' sao ajustados se a pessoa ainda nao mexeu.
    pub(super) fn definir_config(
        &mut self,
        config: &Config,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.ctx = self.servicos.contexto_da_nota(config);
        if !self.mexeu {
            self.dados = DadosDaPrevia::nova(&self.ctx);
            self.sincronizar(window, cx);
        }
        self.recalcular();
        cx.notify();
    }

    /// Poe nos campos da tela o texto de `self.dados` (sem disparar `Change`).
    fn sincronizar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for campo in CampoPrevia::TODOS {
            let texto = self.dados.texto(campo).to_string();
            if let Some(e) = self.entradas.get(&campo) {
                e.update(cx, |e, cx| e.set_value(texto, window, cx));
            }
        }
        let indice = OPCOES_APURACAO
            .iter()
            .position(|(c, _)| *c == self.dados.regime_apuracao);
        self.apuracao.update(cx, |s, cx| {
            s.set_selected_index(indice.map(IndexPath::new), window, cx);
        });
    }

    fn recalcular(&mut self) {
        self.previa = self.dados.calcular(&self.ctx.publicadas);
    }

    fn ao_digitar(
        &mut self,
        campo: CampoPrevia,
        texto: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let normal = self.dados.definir(campo, texto);
        if normal != texto {
            if let Some(e) = self.entradas.get(&campo) {
                e.update(cx, |e, cx| e.set_value(normal, window, cx));
            }
        }
        self.mexeu = true;
        self.recalcular();
        cx.notify();
    }

    fn escolher_regime(&mut self, codigo: u8, cx: &mut Context<Self>) {
        self.dados.op_simples = codigo;
        self.mexeu = true;
        self.recalcular();
        cx.notify();
    }

    /// Escolhe uma classificacao da lista de resultados da busca.
    pub(super) fn escolher_classificacao(
        &mut self,
        codigo: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let normal = self.dados.definir(CampoPrevia::ClassTrib, codigo);
        if let Some(e) = self.entradas.get(&CampoPrevia::ClassTrib) {
            e.update(cx, |e, cx| e.set_value(normal, window, cx));
        }
        self.busca.update(cx, |e, cx| e.set_value("", window, cx));
        self.resultados.clear();
        self.mexeu = true;
        self.recalcular();
        cx.notify();
    }

    // ---------------------------------------------------------------- desenho

    fn entrada(&self, c: CampoPrevia, id: &'static str) -> AnyElement {
        match self.entradas.get(&c) {
            Some(e) => Input::new(e).id(id).text_right().into_any_element(),
            None => div().into_any_element(),
        }
    }

    fn campo_de(
        &self,
        c: CampoPrevia,
        id: &'static str,
        obrigatorio: bool,
        ajuda: Option<SharedString>,
    ) -> gpui_kit::component::form::Field {
        campo(
            c.rotulo(),
            obrigatorio,
            ajuda,
            self.dados.erro_do(c),
            self.entrada(c, id),
        )
    }

    fn lista_de_classificacoes(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.resultados.is_empty() {
            return None;
        }
        let tema = cx.theme();
        let (borda, muted, hover) = (tema.border, tema.muted_foreground, tema.accent);
        let mut lista = v_flex()
            .id("resultados-da-classificacao")
            .max_h(px(240.))
            .overflow_y_scroll()
            .rounded_md()
            .border_1()
            .border_color(borda);
        for (i, c) in self.resultados.iter().enumerate() {
            let codigo = c.codigo.clone();
            lista = lista.child(
                div()
                    .flex()
                    .flex_row()
                    .id(SharedString::from(format!("classe-{i}")))
                    .gap_3()
                    .items_start()
                    .px_3()
                    .py_2()
                    .cursor_pointer()
                    .hover(move |s| s.bg(hover))
                    .child(
                        div()
                            .w(px(64.))
                            .flex_shrink_0()
                            .text_sm()
                            .font_semibold()
                            .child(c.codigo.clone()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_sm()
                            .text_color(muted)
                            .child(c.nome.clone()),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.escolher_classificacao(&codigo, window, cx);
                    })),
            );
        }
        Some(lista.into_any_element())
    }

    fn regime(&self, cx: &mut Context<Self>) -> AnyElement {
        let atual = self.dados.op_simples;
        let escolha = ToggleGroup::new("previa-regime")
            .segmented()
            .single()
            .children(OPCOES_SIMPLES.iter().map(|(codigo, rotulo)| {
                Toggle::new(SharedString::from(format!("previa-regime-{codigo}")))
                    .label(*rotulo)
                    .checked(atual == *codigo)
            }))
            .on_click(cx.listener(|this, estados: &Vec<bool>, _, cx| {
                if let Some(i) = estados.iter().position(|e| *e) {
                    if let Some((codigo, _)) = OPCOES_SIMPLES.get(i) {
                        this.escolher_regime(*codigo, cx);
                    }
                }
            }));
        v_flex()
            .gap_2()
            .child(campo(
                "Regime simulado",
                true,
                Some("Começa pelo regime do emitente; troque para simular outro.".into()),
                None,
                escolha,
            ))
            .when(atual == 3, |d| {
                d.child(campo(
                    "Regime de apuração do IBS/CBS",
                    true,
                    None,
                    None,
                    Select::new(&self.apuracao)
                        .placeholder("Escolha o regime de apuração")
                        .into_any_element(),
                ))
            })
            .into_any_element()
    }

    fn campos(&self, cx: &mut Context<Self>) -> AnyElement {
        let classe = descricao_da_classe(&self.dados.cclass_trib);
        let ajuda_da_classe: Option<SharedString> = if classe.is_empty() {
            Some("Busque acima ou digite os 6 dígitos.".into())
        } else {
            Some(classe.into())
        };
        cartao(cx)
            .id("previa-campos")
            .child(grupo(
                "Operação",
                Some("Informe o que seria cobrado; nada é emitido."),
                cx,
            ))
            .child(self.regime(cx))
            .child(
                v_form()
                    .columns(2)
                    .child(
                        self.campo_de(CampoPrevia::Valor, "previa-valor", true, None)
                            .col_span(1),
                    )
                    .child(
                        self.campo_de(
                            CampoPrevia::Desconto,
                            "previa-desconto",
                            false,
                            Some("Sai da base do IBS e da CBS.".into()),
                        )
                        .col_span(1),
                    )
                    .child(
                        self.campo_de(
                            CampoPrevia::Data,
                            "previa-data",
                            true,
                            Some("O ano da data decide as alíquotas.".into()),
                        )
                        .col_span(2),
                    ),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        Input::new(&self.busca)
                            .id("previa-busca-classe")
                            .prefix(Icon::new(IconName::Search).small()),
                    )
                    .children(self.lista_de_classificacoes(cx)),
            )
            .child(v_form().columns(1).child(self.campo_de(
                CampoPrevia::ClassTrib,
                "previa-class-trib",
                true,
                ajuda_da_classe,
            )))
            .child(grupo(
                "Tributos que saem da base",
                Some("Valores em reais, se houver. A biblioteca decide o que entra em cada ano."),
                cx,
            ))
            .child(
                v_form()
                    .columns(3)
                    .child(
                        self.campo_de(CampoPrevia::Issqn, "previa-issqn", false, None)
                            .col_span(1),
                    )
                    .child(
                        self.campo_de(CampoPrevia::Pis, "previa-pis", false, None)
                            .col_span(1),
                    )
                    .child(
                        self.campo_de(CampoPrevia::Cofins, "previa-cofins", false, None)
                            .col_span(1),
                    ),
            )
            .into_any_element()
    }

    fn resultado(&self, cx: &mut Context<Self>) -> AnyElement {
        let botao = Button::new("previa-abrir-aliquotas")
            .label("Abrir as alíquotas")
            .icon(IconName::Settings)
            .outline()
            .small()
            .on_click(cx.listener(|_, _, _, cx| cx.emit(EventoPrevia::IrParaAliquotas)))
            .into_any_element();
        let corpo = corpo_da_previa(&self.previa, Some(botao), cx);
        let calculada = matches!(self.previa, PreviaIbsCbs::Calculada(_));
        v_flex()
            .id("previa-resultado")
            .gap_3()
            .child(titulo_do_bloco("Resultado", cx))
            .child(corpo)
            .when(!calculada, |d| {
                d.child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child("O quadro aparece quando o valor, a data e a classificação estão preenchidos."),
                )
            })
            .into_any_element()
    }
}

impl Render for TelaPrevia {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let largura = window.viewport_size().width - px(LARGURA_DA_LATERAL);
        let ao_lado = largura >= px(LARGURA_PARA_RESULTADO_AO_LADO);
        let topo = cabecalho(
            "Prévia do IBS e da CBS",
            "Calculadora avulsa: simule a operação e veja as alíquotas e os valores. Nada é emitido e não precisa de certificado.",
            cx,
        );
        let campos = self.campos(cx);
        let resultado = self.resultado(cx);
        let borda = cx.theme().border;
        let coluna = |filho: AnyElement, topo: gpui_kit::Div| {
            div()
                .id("previa-formulario")
                .size_full()
                .overflow_y_scroll()
                .child(
                    v_flex()
                        .w_full()
                        .max_w(px(LARGURA_FORM))
                        .mx_auto()
                        .gap_5()
                        .px_6()
                        .py_6()
                        .child(topo)
                        .child(filho),
                )
        };
        if ao_lado {
            div().id("tela-previa").size_full().child(
                h_flex()
                    .size_full()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .child(coluna(campos, topo)),
                    )
                    .child(
                        div()
                            .id("previa-resultado-lateral")
                            .w(px(LARGURA_DO_RESULTADO))
                            .flex_shrink_0()
                            .h_full()
                            .border_l_1()
                            .border_color(borda)
                            .overflow_y_scroll()
                            .p_5()
                            .child(resultado),
                    ),
            )
        } else {
            let filho = v_flex()
                .gap_5()
                .child(campos)
                .child(cartao(cx).child(resultado))
                .into_any_element();
            div()
                .id("tela-previa")
                .size_full()
                .child(coluna(filho, topo))
        }
    }
}
