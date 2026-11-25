// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A secao "Historico na Sefin" do detalhe de uma nota: um botao que consulta os eventos da nota (cancelamento,
//! substituicao, pedido de analise fiscal, decisao do fisco, manifestacoes) e mostra, em ordem, quando cada um
//! aconteceu e o que significa. So' consulta: nao altera nada na Sefin nem na pasta de notas. A consulta roda fora da
//! linha da tela, pelo mesmo fluxo de certificado e senha da emissao.

use std::sync::Arc;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Disableable as _, Icon, IconName};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, AnyElement, App, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, Styled as _, Subscription, Window,
};
use rustnf_app::config::Config;
use rustnf_app::segredo::Segredo;

use super::pecas::{cartao, faixa, titulo_do_bloco, Negrito as _};
use super::senha::{classificar, Credencial, DialogoSenha, EventoSenha};
use crate::fora;
use crate::modelo::certificado::Gravidade;
use crate::modelo::historico::{
    DesfechoDoHistorico, ItemDoHistorico, TEXTO_SEM_EVENTOS, TEXTO_SEM_RESPOSTA,
};
use crate::servicos::{historico_da_nota, Servicos};

/// O que esta acontecendo de verdade na consulta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Etapa {
    /// Abrindo o certificado.
    Certificado,
    /// Perguntando a Sefin.
    Consultando,
}

/// Onde a consulta esta.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Fase {
    /// Nada consultado para esta nota.
    Parada,
    /// Rodando fora da tela.
    Trabalhando(Etapa),
    /// Terminou.
    Desfecho(DesfechoDoHistorico),
}

/// A secao do historico.
pub(super) struct Historico {
    servicos: Arc<Servicos>,
    config: Config,
    /// A nota que o detalhe mostra; `None` sem nota.
    chave: Option<String>,
    pub(super) fase: Fase,
    /// Muda a cada nota escolhida: a resposta que chega atrasada nao vale para a nota de agora.
    versao: u64,
    senha_pedida: Option<Entity<DialogoSenha>>,
    sub_senha: Option<Subscription>,
}

impl Historico {
    /// A secao, sem nota.
    pub(super) fn new(servicos: Arc<Servicos>) -> Self {
        Self {
            servicos,
            config: Config::default(),
            chave: None,
            fase: Fase::Parada,
            versao: 0,
            senha_pedida: None,
            sub_senha: None,
        }
    }

    /// A configuracao que vale (a efetiva) chegou ou mudou.
    pub(super) fn definir_config(&mut self, config: Config, cx: &mut Context<Self>) {
        self.config = config;
        cx.notify();
    }

    /// O detalhe passou a mostrar outra nota (ou nenhuma): o historico antigo some.
    pub(super) fn definir_nota(&mut self, chave: Option<&str>, cx: &mut Context<Self>) {
        if self.chave.as_deref() == chave && !matches!(self.fase, Fase::Desfecho(_)) {
            return;
        }
        self.versao += 1;
        self.chave = chave.map(str::to_string);
        self.fase = Fase::Parada;
        self.senha_pedida = None;
        self.sub_senha = None;
        cx.notify();
    }

    /// Consulta os eventos da nota. A senha digitada so' vale para esta consulta.
    pub(super) fn consultar(
        &mut self,
        digitada: Option<Segredo>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(chave) = self.chave.clone() else {
            return;
        };
        if matches!(self.fase, Fase::Trabalhando(_)) {
            return;
        }
        let anterior = self.fase.clone();
        self.fase = Fase::Trabalhando(Etapa::Certificado);
        cx.notify();
        let versao = self.versao;
        let servicos = Arc::clone(&self.servicos);
        let fabrica = servicos.fabrica();
        let intervalo = servicos.intervalo();
        let config = self.config.clone();
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
                        if this.versao == versao {
                            this.fase = anterior;
                            this.pedir_senha(erro, window, cx);
                        }
                    })
                    .ok();
                    return;
                }
                Credencial::Falhou(texto) => {
                    this.update(cx, |this, cx| {
                        this.aplicar(versao, DesfechoDoHistorico::Recusado { texto }, cx);
                    })
                    .ok();
                    return;
                }
            };
            this.update(cx, |this, cx| {
                if this.versao == versao {
                    this.fase = Fase::Trabalhando(Etapa::Consultando);
                    cx.notify();
                }
            })
            .ok();
            let desfecho = match fora::rede(historico_da_nota(fabrica, preparo, chave, intervalo)) {
                Some(tarefa) => tarefa
                    .await
                    .unwrap_or_else(|_| DesfechoDoHistorico::Incompleto {
                        itens: Vec::new(),
                        texto: TEXTO_SEM_RESPOSTA.into(),
                    }),
                None => DesfechoDoHistorico::Recusado {
                    texto: "Não foi possível preparar a chamada de rede.".into(),
                },
            };
            this.update(cx, |this, cx| this.aplicar(versao, desfecho, cx))
                .ok();
        })
        .detach();
    }

    fn aplicar(&mut self, versao: u64, desfecho: DesfechoDoHistorico, cx: &mut Context<Self>) {
        if self.versao != versao {
            return;
        }
        self.fase = Fase::Desfecho(desfecho);
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
                    this.consultar(senha, window, cx);
                }
            },
        ));
        self.senha_pedida = Some(dialogo.clone());
        DialogoSenha::abrir(&dialogo, window, cx);
    }

    // ---------------------------------------------------------------- desenho

    fn item(i: usize, item: &ItemDoHistorico, cx: &App) -> AnyElement {
        let muted = cx.theme().muted_foreground;
        let borda = cx.theme().border;
        let detalhes = [item.quem.as_str(), item.motivo.as_str()]
            .iter()
            .filter(|t| !t.is_empty())
            .copied()
            .collect::<Vec<_>>()
            .join(" - ");
        // `div()` do kit (e nao `v_flex()`): so' ele deixa o item ser achado pelos testes da janela
        div()
            .id(SharedString::from(format!("historico-item-{i}")))
            .flex()
            .flex_col()
            .gap_0p5()
            .py_2()
            .border_t_1()
            .border_color(borda)
            .child(
                h_flex()
                    .gap_2()
                    .justify_between()
                    .flex_wrap()
                    .child(div().text_sm().font_semibold().child(item.nome.clone()))
                    .child(div().text_xs().text_color(muted).child(item.quando.clone())),
            )
            .when_some(Some(detalhes).filter(|d| !d.is_empty()), |d, texto| {
                d.child(div().text_xs().text_color(muted).child(texto))
            })
            .into_any_element()
    }

    fn lista(itens: &[ItemDoHistorico], cx: &App) -> AnyElement {
        v_flex()
            .id("historico-lista")
            .children(
                itens
                    .iter()
                    .enumerate()
                    .map(|(i, it)| Self::item(i, it, cx)),
            )
            .into_any_element()
    }

    fn resultado(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let muted = cx.theme().muted_foreground;
        let tentar = |cx: &mut Context<Self>| {
            Button::new("historico-tentar-de-novo")
                .label("Tentar de novo")
                .icon(IconName::RefreshCw)
                .primary()
                .on_click(cx.listener(|this, _, window, cx| this.consultar(None, window, cx)))
        };
        match self.fase.clone() {
            Fase::Parada => None,
            Fase::Trabalhando(etapa) => Some(
                h_flex()
                    .id("historico-trabalhando")
                    .gap_2()
                    .items_center()
                    .text_sm()
                    .text_color(muted)
                    .child(Icon::new(IconName::LoaderCircle))
                    .child(match etapa {
                        Etapa::Certificado => "Abrindo o certificado...",
                        Etapa::Consultando => "Consultando os eventos da nota na Sefin Nacional...",
                    })
                    .into_any_element(),
            ),
            Fase::Desfecho(DesfechoDoHistorico::Eventos(itens)) if itens.is_empty() => Some(
                faixa(Gravidade::Neutro, TEXTO_SEM_EVENTOS, cx)
                    .id("historico-vazio")
                    .into_any_element(),
            ),
            Fase::Desfecho(DesfechoDoHistorico::Eventos(itens)) => {
                Some(
                    v_flex()
                        .gap_1()
                        .child(Self::lista(&itens, cx))
                        .child(div().id("historico-fim").text_xs().text_color(muted).child(
                            format!(
                                "{} na Sefin, do mais antigo para o mais novo.",
                                if itens.len() == 1 {
                                    "1 evento".to_string()
                                } else {
                                    format!("{} eventos", itens.len())
                                }
                            ),
                        ))
                        .into_any_element(),
                )
            }
            Fase::Desfecho(DesfechoDoHistorico::Incompleto { itens, texto }) => Some(
                v_flex()
                    .id("historico-incompleto")
                    .gap_2()
                    .child(faixa(Gravidade::Aviso, texto, cx))
                    .when(!itens.is_empty(), |d| {
                        d.child(
                            div()
                                .text_xs()
                                .text_color(muted)
                                .child("O que a Sefin respondeu até aqui:"),
                        )
                        .child(Self::lista(&itens, cx))
                    })
                    .child(h_flex().child(tentar(cx)))
                    .into_any_element(),
            ),
            Fase::Desfecho(DesfechoDoHistorico::Recusado { texto }) => Some(
                v_flex()
                    .id("historico-recusado")
                    .gap_2()
                    .child(faixa(
                        Gravidade::Erro,
                        format!("Nada foi consultado. {texto}"),
                        cx,
                    ))
                    .child(h_flex().child(tentar(cx)))
                    .into_any_element(),
            ),
        }
    }
}

impl Render for Historico {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        let ocupada = matches!(self.fase, Fase::Trabalhando(_));
        let ja_consultou = matches!(self.fase, Fase::Desfecho(_));
        let resultado = self.resultado(cx);
        cartao(cx)
            .id("historico-na-sefin")
            .child(titulo_do_bloco("Histórico na Sefin", cx))
            .child(div().text_xs().text_color(muted).child(
                "O que a Sefin tem registrado desta nota: cancelamento, substituição, pedido de análise fiscal, decisão do fisco e manifestações. Só consulta: não altera nada.",
            ))
            .child(
                h_flex().child(
                    Button::new("consultar-historico")
                        .label(if ja_consultou {
                            "Consultar de novo"
                        } else {
                            "Ver o histórico na Sefin"
                        })
                        .icon(IconName::Search)
                        .outline()
                        .loading(ocupada)
                        .disabled(ocupada || self.chave.is_none())
                        .on_click(
                            cx.listener(|this, _, window, cx| this.consultar(None, window, cx)),
                        ),
                ),
            )
            .children(resultado)
    }
}
