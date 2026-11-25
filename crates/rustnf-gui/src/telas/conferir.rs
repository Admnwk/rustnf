// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! "Conferir situacao na Sefin", na tela Notas: confere as notas que a lista mostra (a busca, o filtro e o periodo da
//! tela), uma de cada vez, fora da linha da tela, com o andamento ("Conferindo 12 de 40...") e o botao Parar. Com
//! muitas notas diz quanto tempo leva e pede confirmacao. O resultado vem em linguagem simples: o que mudou (de ->
//! para), quantas estavam iguais e, de cada falha, o que aconteceu e o que fazer. Aqui so' ha estado e desenho; a
//! conta e o texto moram em `modelo::conferir`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Disableable as _, Icon, IconName};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, AnyElement, App, AppContext as _, Context, Entity, EventEmitter,
    InteractiveElement as _, IntoElement, ParentElement as _, Render,
    StatefulInteractiveElement as _, Styled as _, Subscription, Window,
};
use rustnf_app::config::Config;
use rustnf_app::notas::Registro;
use rustnf_app::segredo::Segredo;

use super::pecas::{cartao, faixa, titulo_do_bloco};
use super::senha::{classificar, Credencial, DialogoSenha, EventoSenha};
use crate::fora;
use crate::modelo::certificado::Gravidade;
use crate::modelo::conferir::{
    pergunta_de_confirmacao, precisa_confirmar, Andamento, DesfechoDoLote, ResultadoDoLote,
    TEXTO_SEM_NOTAS,
};
use crate::servicos::{conferir_lote, ParadaDoLote, Servicos};

/// Quantas linhas de mudanca ou de falha o resultado desenha antes de dizer "e mais N" (a lista rola).
const LINHAS_MAXIMAS: usize = 60;

/// O que a conferencia avisa a tela.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum EventoConferir {
    /// A pasta de notas mudou: a lista deve ser lida de novo. Leva as chaves das notas que mudaram.
    Atualizou(Vec<String>),
}

/// O que esta acontecendo de verdade.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Etapa {
    /// Abrindo o certificado.
    Certificado,
    /// Conferindo as notas, uma por vez.
    Conferindo,
}

/// Onde a conferencia esta.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Fase {
    /// Nada acontecendo (o cartao nem aparece).
    Parada,
    /// Lote grande: diz quanto tempo leva e espera a confirmacao.
    Confirmando(usize),
    /// Rodando fora da tela.
    Trabalhando(Etapa),
    /// Terminou (ou foi parada, ou nem comecou).
    Resultado(DesfechoDoLote),
    /// Nao ha nota na lista.
    SemNotas,
}

/// A conferencia em lote.
pub(super) struct ConferirNaSefin {
    servicos: Arc<Servicos>,
    config: Config,
    /// As notas do lote: as que a lista mostrava quando a pessoa clicou.
    registros: Vec<Registro>,
    pub(super) fase: Fase,
    pub(super) andamento: Andamento,
    parada: ParadaDoLote,
    /// A pessoa apertou Parar e o lote termina a nota de agora.
    parando: bool,
    /// Muda a cada lote: o andamento e o resultado de um lote velho nao valem para o de agora.
    lote: u64,
    senha_pedida: Option<Entity<DialogoSenha>>,
    sub_senha: Option<Subscription>,
}

impl EventEmitter<EventoConferir> for ConferirNaSefin {}

impl ConferirNaSefin {
    /// A conferencia, parada.
    pub(super) fn new(servicos: Arc<Servicos>) -> Self {
        Self {
            servicos,
            config: Config::default(),
            registros: Vec::new(),
            fase: Fase::Parada,
            andamento: Andamento::default(),
            parada: Arc::new(AtomicBool::new(false)),
            parando: false,
            lote: 0,
            senha_pedida: None,
            sub_senha: None,
        }
    }

    /// A configuracao que vale (a efetiva) chegou ou mudou.
    pub(super) fn definir_config(&mut self, config: Config, cx: &mut Context<Self>) {
        self.config = config;
        cx.notify();
    }

    /// Um lote esta rodando (ou esperando confirmacao): o botao da tela fica desligado.
    pub(super) fn ocupada(&self) -> bool {
        matches!(self.fase, Fase::Trabalhando(_) | Fase::Confirmando(_))
    }

    /// "Conferir situacao na Sefin" para as notas de `registros`. Lote grande pede confirmacao antes.
    pub(super) fn iniciar(
        &mut self,
        registros: Vec<Registro>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.ocupada() {
            return;
        }
        if registros.is_empty() {
            self.fase = Fase::SemNotas;
            cx.notify();
            return;
        }
        let total = registros.len();
        self.registros = registros;
        if precisa_confirmar(total) {
            self.fase = Fase::Confirmando(total);
            cx.notify();
            return;
        }
        self.conferir(None, window, cx);
    }

    fn fechar(&mut self, cx: &mut Context<Self>) {
        if !self.ocupada() {
            self.fase = Fase::Parada;
            self.registros.clear();
            cx.notify();
        }
    }

    fn parar(&mut self, cx: &mut Context<Self>) {
        if matches!(self.fase, Fase::Trabalhando(Etapa::Conferindo)) {
            self.parada.store(true, Ordering::SeqCst);
            self.parando = true;
            cx.notify();
        }
    }

    fn confirmar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(self.fase, Fase::Confirmando(_)) {
            self.conferir(None, window, cx);
        }
    }

    /// Abre o certificado e, com ele, roda o lote fora da linha da tela.
    fn conferir(&mut self, digitada: Option<Segredo>, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(self.fase, Fase::Trabalhando(_)) || self.registros.is_empty() {
            return;
        }
        let anterior = self.fase.clone();
        self.fase = Fase::Trabalhando(Etapa::Certificado);
        self.parando = false;
        self.parada.store(false, Ordering::SeqCst);
        self.andamento = Andamento {
            feitas: 0,
            total: self.registros.len(),
        };
        self.lote += 1;
        let lote = self.lote;
        cx.notify();
        let servicos = Arc::clone(&self.servicos);
        let fabrica = servicos.fabrica();
        let intervalo = servicos.intervalo();
        let config = self.config.clone();
        let registros = self.registros.clone();
        let parada = Arc::clone(&self.parada);
        let digitou = digitada.is_some();
        cx.spawn_in(window, async move |this, cx| {
            let (s2, c2) = (Arc::clone(&servicos), config.clone());
            let preparo = cx
                .background_spawn(async move { s2.preparar_emissao(&c2, digitada.as_ref()) })
                .await;
            let preparo = match classificar(preparo, digitou) {
                Credencial::Pronta(p) => *p,
                Credencial::PedirSenha(erro) => {
                    this.update_in(cx, |this, window, cx| {
                        this.fase = anterior;
                        this.pedir_senha(erro, window, cx);
                    })
                    .ok();
                    return;
                }
                Credencial::Falhou(texto) => {
                    this.update(cx, |this, cx| {
                        this.aplicar(lote, DesfechoDoLote::Recusado { texto }, cx);
                    })
                    .ok();
                    return;
                }
            };
            this.update(cx, |this, cx| {
                this.fase = Fase::Trabalhando(Etapa::Conferindo);
                cx.notify();
            })
            .ok();
            let (envia, mut recebe) = tokio::sync::mpsc::unbounded_channel::<Andamento>();
            // o andamento chega da rede a cada nota; a tela o mostra sem esperar o lote acabar
            cx.spawn({
                let this = this.clone();
                async move |cx| {
                    while let Some(a) = recebe.recv().await {
                        let visto = this.update(cx, |this, cx| {
                            if this.lote == lote {
                                this.andamento = a;
                                cx.notify();
                            }
                        });
                        if visto.is_err() {
                            break;
                        }
                    }
                }
            })
            .detach();
            let trabalho =
                conferir_lote(fabrica, preparo, config, registros, intervalo, parada, envia);
            let desfecho = match fora::rede(trabalho) {
                Some(tarefa) => tarefa.await.unwrap_or_else(|_| DesfechoDoLote::Recusado {
                    texto: "A conferência foi interrompida por um erro do programa. Confira a lista e tente de novo.".into(),
                }),
                None => DesfechoDoLote::Recusado {
                    texto: "Não foi possível preparar a chamada de rede.".into(),
                },
            };
            this.update(cx, |this, cx| this.aplicar(lote, desfecho, cx))
                .ok();
        })
        .detach();
    }

    fn aplicar(&mut self, lote: u64, desfecho: DesfechoDoLote, cx: &mut Context<Self>) {
        if self.lote != lote {
            return;
        }
        self.parando = false;
        if let DesfechoDoLote::Feito(r) = &desfecho {
            if r.mudou_a_pasta() {
                let chaves = r.mudaram.iter().map(|m| m.chave.clone()).collect();
                cx.emit(EventoConferir::Atualizou(chaves));
            }
        }
        self.fase = Fase::Resultado(desfecho);
        cx.notify();
    }

    fn pedir_senha(&mut self, erro: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        let desligado = self.servicos.cofre_desligado();
        let dialogo = cx.new(|cx| DialogoSenha::new(desligado, erro, window, cx));
        self.sub_senha = Some(cx.subscribe_in(
            &dialogo,
            window,
            |this, dialogo, evento: &EventoSenha, window, cx| {
                this.sub_senha = None;
                this.senha_pedida = None;
                if *evento == EventoSenha::Usada {
                    let senha = dialogo.update(cx, |d, _| d.tomar());
                    this.conferir(senha, window, cx);
                } else if matches!(this.fase, Fase::Confirmando(_)) {
                    cx.notify();
                }
            },
        ));
        self.senha_pedida = Some(dialogo.clone());
        DialogoSenha::abrir(&dialogo, window, cx);
    }

    // ---------------------------------------------------------------- desenho

    fn barra(&self, cx: &App) -> AnyElement {
        let borda = cx.theme().border;
        let preenchida = cx.theme().primary;
        let fracao = self.andamento.fracao().clamp(0.0, 1.0);
        div()
            .id("barra-de-andamento")
            .h(px(8.))
            .w_full()
            .rounded_full()
            .bg(borda)
            .child(
                div()
                    .h_full()
                    .rounded_full()
                    .bg(preenchida)
                    .w(gpui_kit::relative(fracao)),
            )
            .into_any_element()
    }

    fn resultado(r: &ResultadoDoLote, cx: &App) -> AnyElement {
        let muted = cx.theme().muted_foreground;
        let borda = cx.theme().border;
        let linhas = |textos: Vec<(String, Option<String>)>, id: &'static str| {
            let total = textos.len();
            let mut coluna = v_flex()
                .id(id)
                .gap_1p5()
                .max_h(px(260.))
                .overflow_y_scroll();
            for (i, (principal, segundo)) in textos.into_iter().enumerate() {
                if i == LINHAS_MAXIMAS {
                    coluna = coluna.child(div().text_xs().text_color(muted).child(format!(
                        "... e mais {} (a lista da tela mostra todas).",
                        total - i
                    )));
                    break;
                }
                coluna = coluna.child(
                    v_flex()
                        .gap_0p5()
                        .py_1p5()
                        .border_t_1()
                        .border_color(borda)
                        .child(div().text_sm().child(principal))
                        .children(segundo.map(|t| div().text_xs().text_color(muted).child(t))),
                );
            }
            coluna
        };
        let sem_mudanca = r.falhas.is_empty() && r.mudaram.is_empty();
        let gravidade = if !r.falhas.is_empty() {
            Gravidade::Aviso
        } else if r.parado {
            Gravidade::Neutro
        } else {
            Gravidade::Ok
        };
        let mut c =
            v_flex()
                .id("conferencia-resultado")
                .gap_3()
                .child(faixa(gravidade, r.resumo(), cx));
        if !r.mudaram.is_empty() {
            c = c.child(titulo_do_bloco("O que mudou", cx)).child(linhas(
                r.mudaram.iter().map(|m| (m.texto(), None)).collect(),
                "conferencia-mudaram",
            ));
        }
        if !r.falhas.is_empty() {
            c = c
                .child(titulo_do_bloco("Não deu para conferir", cx))
                .child(linhas(
                    r.falhas
                        .iter()
                        .map(|f| {
                            let quem = if f.tomador.is_empty() {
                                format!("nº {}", f.numero)
                            } else {
                                format!("nº {}, {}", f.numero, f.tomador)
                            };
                            (
                                format!("{quem}: {}", f.o_que_houve),
                                Some(f.o_que_fazer.clone()),
                            )
                        })
                        .collect(),
                    "conferencia-falhas",
                ));
        }
        if sem_mudanca && !r.parado && r.conferidas > 0 {
            c = c.child(
                div()
                    .text_xs()
                    .text_color(muted)
                    .child("A pasta de notas já estava igual ao que a Sefin mostra."),
            );
        }
        c.into_any_element()
    }

    fn corpo(&self, cx: &mut Context<Self>) -> AnyElement {
        let muted = cx.theme().muted_foreground;
        match self.fase.clone() {
            Fase::Parada => div().into_any_element(),
            Fase::SemNotas => faixa(Gravidade::Neutro, TEXTO_SEM_NOTAS, cx)
                .id("conferencia-sem-notas")
                .into_any_element(),
            Fase::Confirmando(total) => v_flex()
                .id("conferencia-confirmar")
                .gap_3()
                .child(faixa(
                    Gravidade::Aviso,
                    pergunta_de_confirmacao(total, self.servicos.intervalo()),
                    cx,
                ))
                .child(
                    h_flex()
                        .gap_2()
                        .child(
                            Button::new("conferencia-confirmar-sim")
                                .label("Conferir mesmo assim")
                                .icon(IconName::RefreshCw)
                                .primary()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.confirmar(window, cx);
                                })),
                        )
                        .child(
                            Button::new("conferencia-confirmar-nao")
                                .label("Agora não")
                                .outline()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.fase = Fase::Parada;
                                    this.registros.clear();
                                    cx.notify();
                                })),
                        ),
                )
                .into_any_element(),
            Fase::Trabalhando(etapa) => {
                let texto = match etapa {
                    Etapa::Certificado => "Abrindo o certificado...".to_string(),
                    Etapa::Conferindo if self.parando => format!(
                        "Parando: termino a nota de agora e paro. {}",
                        self.andamento.texto()
                    ),
                    Etapa::Conferindo => self.andamento.texto(),
                };
                v_flex()
                    .id("conferencia-trabalhando")
                    .gap_2()
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(Icon::new(IconName::LoaderCircle))
                            .child(div().id("conferencia-andamento").text_sm().child(texto))
                            .child(div().flex_1())
                            .child(
                                Button::new("conferencia-parar")
                                    .label("Parar")
                                    .outline()
                                    .disabled(
                                        self.parando
                                            || !matches!(etapa, Etapa::Conferindo),
                                    )
                                    .on_click(cx.listener(|this, _, _, cx| this.parar(cx))),
                            ),
                    )
                    .child(self.barra(cx))
                    .child(div().text_xs().text_color(muted).child(
                        "Uma nota e uma consulta por vez, com pausa entre elas. Conferir só consulta a Sefin e atualiza a pasta de notas; o que já foi conferido fica gravado.",
                    ))
                    .into_any_element()
            }
            Fase::Resultado(DesfechoDoLote::Feito(r)) => Self::resultado(&r, cx),
            Fase::Resultado(DesfechoDoLote::Recusado { texto }) => v_flex()
                .id("conferencia-recusada")
                .gap_2()
                .child(faixa(
                    Gravidade::Erro,
                    format!("Nada foi conferido. {texto}"),
                    cx,
                ))
                .into_any_element(),
        }
    }
}

impl Render for ConferirNaSefin {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.fase == Fase::Parada {
            return div().id("conferencia-parada").into_any_element();
        }
        let fechavel = !self.ocupada();
        let corpo = self.corpo(cx);
        cartao(cx)
            .id("conferencia-na-sefin")
            .child(
                h_flex()
                    .items_center()
                    .child(titulo_do_bloco("Conferir situação na Sefin", cx))
                    .child(div().flex_1())
                    .when_some(fechavel.then_some(()), |d, ()| {
                        d.child(
                            Button::new("conferencia-fechar")
                                .label("Fechar")
                                .outline()
                                .on_click(cx.listener(|this, _, _, cx| this.fechar(cx))),
                        )
                    }),
            )
            .child(corpo)
            .into_any_element()
    }
}
