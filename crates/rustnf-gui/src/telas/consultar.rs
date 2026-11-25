// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A tela Consultar: a chave de acesso (50 posicoes, colada com espacos e pontuacao, mostrada em grupos) ou a serie e o
//! numero da DPS (com o emitente da configuracao), a consulta a Sefin Nacional fora da linha da tela e o resultado no
//! mesmo detalhe da tela Notas. A nota devolvida so' vale se for a pedida. Consultar nao altera nada; "Guardar na
//! pasta de notas" so' aparece para nota do emitente.

use std::sync::Arc;

use gpui_kit::component::button::{Button, ButtonVariants as _, Toggle, ToggleGroup};
use gpui_kit::component::form::v_form;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Disableable as _, Icon, IconName};
use gpui_kit::{
    div, px, AppContext as _, Context, Entity, EventEmitter, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, ScrollHandle, SharedString, StatefulInteractiveElement as _,
    Styled as _, Subscription, Window,
};
use rustnf_app::config::Config;
use rustnf_app::notas::Situacao;
use rustnf_app::segredo::Segredo;

use super::cancelar::Cancelamento;
use super::detalhe::{EventoDetalhe, VisaoDetalhe};
use super::pecas::{cabecalho, campo, cartao, faixa};
use super::senha::{classificar, Credencial, DialogoSenha, EventoSenha};
use crate::fora;
use crate::modelo::certificado::Gravidade;
use crate::modelo::chave::{self, EstadoDaChave};
use crate::modelo::consulta::{DesfechoDaConsulta, TEXTO_NAO_ENCONTRADA};
use crate::modelo::consulta_dps::{
    conferir_dps, serie_efetiva, so_digitos, EstadoDaDps, ModoDeConsulta,
    TEXTO_NAO_ENCONTRADA as TEXTO_DPS_NAO_ENCONTRADA,
};
use crate::modelo::nota::DadosNota;
use crate::servicos::{consultar_dps_nota, consultar_nota, Servicos};

/// O que a tela pede a janela.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum EventoConsulta {
    /// A nota consultada foi guardada na pasta de notas: a lista deve ser lida de novo.
    Guardou,
    /// "Usar como modelo": a Nova nota recebe estes dados.
    UsarComoModelo(Box<DadosNota>),
}

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
    /// Nada consultado ainda (ou a chave mudou depois da ultima consulta).
    Parada,
    /// Rodando fora da tela.
    Trabalhando(Etapa),
    /// Terminou.
    Desfecho(DesfechoDaConsulta),
}

/// A tela.
pub(super) struct TelaConsultar {
    servicos: Arc<Servicos>,
    config: Config,
    entrada: Entity<InputState>,
    /// Como a pessoa acha a nota: pela chave ou pela serie e numero da DPS.
    pub(super) modo: ModoDeConsulta,
    /// A serie da DPS e o numero da DPS (so' digitos).
    serie_dps: Entity<InputState>,
    numero_dps: Entity<InputState>,
    pub(super) serie: String,
    pub(super) numero: String,
    /// A chave como esta no campo (em grupos).
    pub(super) texto: String,
    tocada: bool,
    pub(super) fase: Fase,
    pub(super) detalhe: Entity<VisaoDetalhe>,
    senha_pedida: Option<Entity<DialogoSenha>>,
    sub_senha: Option<Subscription>,
    _subs: Vec<Subscription>,
    /// A rolagem da tela (teclado e campo focado).
    pub(super) rolagem: ScrollHandle,
}

impl EventEmitter<EventoConsulta> for TelaConsultar {}

impl TelaConsultar {
    /// A tela, ainda sem configuracao.
    pub(super) fn new(
        servicos: Arc<Servicos>,
        cancelamento: Entity<Cancelamento>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let entrada = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Cole aqui a chave de acesso (50 posições)")
        });
        let serie_dps = cx.new(|cx| InputState::new(window, cx).placeholder("a da configuração"));
        let numero_dps = cx.new(|cx| InputState::new(window, cx).placeholder("Número da DPS"));
        let detalhe = {
            let servicos = Arc::clone(&servicos);
            cx.new(|cx| VisaoDetalhe::new(servicos, cancelamento, window, cx))
        };
        let s3 = cx.subscribe_in(
            &serie_dps,
            window,
            |this, entrada, evento: &InputEvent, window, cx| match evento {
                InputEvent::Change => this.ao_digitar_dps(entrada, true, window, cx),
                InputEvent::PressEnter { .. } => this.consultar(None, window, cx),
                _ => {}
            },
        );
        let s4 = cx.subscribe_in(
            &numero_dps,
            window,
            |this, entrada, evento: &InputEvent, window, cx| match evento {
                InputEvent::Change => this.ao_digitar_dps(entrada, false, window, cx),
                InputEvent::PressEnter { .. } => this.consultar(None, window, cx),
                InputEvent::Blur => {
                    this.tocada = true;
                    cx.notify();
                }
                InputEvent::Focus => {}
            },
        );
        let s1 = cx.subscribe_in(
            &entrada,
            window,
            |this, entrada, evento: &InputEvent, window, cx| match evento {
                InputEvent::Change => this.ao_digitar(entrada, window, cx),
                InputEvent::PressEnter { .. } => this.consultar(None, window, cx),
                InputEvent::Blur => {
                    this.tocada = true;
                    cx.notify();
                }
                InputEvent::Focus => {}
            },
        );
        let s2 =
            cx.subscribe_in(
                &detalhe,
                window,
                |_, _, evento: &EventoDetalhe, _, cx| match evento {
                    EventoDetalhe::Guardou => cx.emit(EventoConsulta::Guardou),
                    EventoDetalhe::UsarComoModelo(d) => {
                        cx.emit(EventoConsulta::UsarComoModelo(d.clone()));
                    }
                },
            );
        Self {
            servicos,
            config: Config::default(),
            entrada,
            modo: ModoDeConsulta::Chave,
            serie_dps,
            numero_dps,
            serie: String::new(),
            numero: String::new(),
            texto: String::new(),
            tocada: false,
            fase: Fase::Parada,
            detalhe,
            senha_pedida: None,
            sub_senha: None,
            _subs: vec![s1, s2, s3, s4],
            rolagem: ScrollHandle::new(),
        }
    }

    /// A janela entrega a configuracao (a efetiva) quando ela chega ou muda.
    pub(super) fn definir_config(&mut self, config: Config, cx: &mut Context<Self>) {
        self.config = config.clone();
        self.detalhe
            .update(cx, |d, cx| d.definir_config(config, cx));
        cx.notify();
    }

    /// A Sefin registrou um evento para uma nota: atualiza o resultado, se for a mesma.
    pub(super) fn evento_registrado(
        &mut self,
        chave: &str,
        situacao: Option<Situacao>,
        cx: &mut Context<Self>,
    ) {
        let chave = chave.to_string();
        self.detalhe
            .update(cx, |d, cx| d.evento_registrado(&chave, situacao, cx));
    }

    fn ao_digitar(
        &mut self,
        entrada: &Entity<InputState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let digitado = entrada.read(cx).value().to_string();
        let normal = chave::em_grupos(&digitado);
        if normal != digitado {
            entrada.update(cx, |e, cx| e.set_value(normal.clone(), window, cx));
        }
        if normal != self.texto {
            self.texto = normal;
            // a chave mudou: o resultado antigo nao e' mais desta chave
            if matches!(self.fase, Fase::Desfecho(_)) {
                self.fase = Fase::Parada;
                self.detalhe.update(cx, |d, cx| d.limpar(cx));
            }
        }
        cx.notify();
    }

    /// So' os digitos entram na serie e no numero da DPS; o resultado antigo nao e' mais desta consulta.
    fn ao_digitar_dps(
        &mut self,
        entrada: &Entity<InputState>,
        e_serie: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let digitado = entrada.read(cx).value().to_string();
        let limpo = so_digitos(&digitado);
        if limpo != digitado {
            entrada.update(cx, |e, cx| e.set_value(limpo.clone(), window, cx));
        }
        let campo = if e_serie {
            &mut self.serie
        } else {
            &mut self.numero
        };
        if *campo != limpo {
            *campo = limpo;
            self.esquecer_o_resultado(cx);
        }
        cx.notify();
    }

    fn esquecer_o_resultado(&mut self, cx: &mut Context<Self>) {
        if matches!(self.fase, Fase::Desfecho(_)) {
            self.fase = Fase::Parada;
            self.detalhe.update(cx, |d, cx| d.limpar(cx));
        }
    }

    /// Troca o modo: o resultado do outro modo some.
    pub(super) fn escolher_modo(&mut self, modo: ModoDeConsulta, cx: &mut Context<Self>) {
        if self.modo != modo && !matches!(self.fase, Fase::Trabalhando(_)) {
            self.modo = modo;
            self.tocada = false;
            self.esquecer_o_resultado(cx);
            cx.notify();
        }
    }

    /// O que a tela diz da serie e do numero da DPS digitados.
    pub(super) fn estado_da_dps(&self) -> EstadoDaDps {
        conferir_dps(&self.config.emitente, &self.serie, &self.numero)
    }

    /// A consulta de agora pode sair.
    fn pode_consultar(&self) -> bool {
        match self.modo {
            ModoDeConsulta::Chave => self.estado_da_chave().valida(),
            ModoDeConsulta::Dps => self.estado_da_dps().valido(),
        }
    }

    /// O que a tela diz da chave digitada.
    pub(super) fn estado_da_chave(&self) -> EstadoDaChave {
        chave::conferir(&self.texto)
    }

    /// Consulta a chave digitada (Enter ou o botao). Chave que nao confere nao sai.
    pub(super) fn consultar(
        &mut self,
        digitada: Option<Segredo>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(self.fase, Fase::Trabalhando(_)) {
            return;
        }
        self.tocada = true;
        if !self.pode_consultar() {
            cx.notify();
            return;
        }
        let chave = chave::limpar(&self.texto);
        let (modo, serie, numero) = (
            self.modo,
            serie_efetiva(&self.config.emitente, &self.serie),
            self.numero.clone(),
        );
        let anterior = self.fase.clone();
        self.fase = Fase::Trabalhando(Etapa::Certificado);
        self.detalhe.update(cx, |d, cx| d.limpar(cx));
        cx.notify();
        let servicos = Arc::clone(&self.servicos);
        let fabrica = servicos.fabrica();
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
                        this.fase = anterior;
                        this.pedir_senha(erro, window, cx);
                    })
                    .ok();
                    return;
                }
                Credencial::Falhou(texto) => {
                    this.update_in(cx, |this, _, cx| {
                        this.aplicar(DesfechoDaConsulta::Recusada { texto }, cx);
                    })
                    .ok();
                    return;
                }
            };
            this.update(cx, |this, cx| {
                this.fase = Fase::Trabalhando(Etapa::Consultando);
                cx.notify();
            })
            .ok();
            let trabalho = async move {
                match modo {
                    ModoDeConsulta::Chave => consultar_nota(fabrica, preparo, chave).await,
                    ModoDeConsulta::Dps => {
                        consultar_dps_nota(fabrica, preparo, config, serie, numero).await
                    }
                }
            };
            let desfecho = match fora::rede(trabalho) {
                Some(tarefa) => tarefa
                    .await
                    .unwrap_or_else(|_| DesfechoDaConsulta::SemResposta {
                        texto: crate::modelo::consulta::TEXTO_SEM_RESPOSTA.into(),
                    }),
                None => DesfechoDaConsulta::Recusada {
                    texto: "Não foi possível preparar a chamada de rede.".into(),
                },
            };
            this.update(cx, |this, cx| this.aplicar(desfecho, cx)).ok();
        })
        .detach();
    }

    fn aplicar(&mut self, desfecho: DesfechoDaConsulta, cx: &mut Context<Self>) {
        if let DesfechoDaConsulta::Encontrada(nota) = &desfecho {
            let nota = (**nota).clone();
            self.detalhe
                .update(cx, |d, cx| d.mostrar_consulta(nota, cx));
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

    fn resultado(&mut self, cx: &mut Context<Self>) -> Option<gpui_kit::AnyElement> {
        let muted = cx.theme().muted_foreground;
        let tentar = |cx: &mut Context<Self>| {
            Button::new("consulta-tentar-de-novo")
                .label("Tentar de novo")
                .icon(IconName::RefreshCw)
                .primary()
                .on_click(cx.listener(|this, _, window, cx| this.consultar(None, window, cx)))
        };
        match self.fase.clone() {
            Fase::Parada => None,
            Fase::Trabalhando(etapa) => Some(
                h_flex()
                    .id("consulta-trabalhando")
                    .gap_2()
                    .items_center()
                    .p_4()
                    .text_sm()
                    .text_color(muted)
                    .child(Icon::new(IconName::LoaderCircle))
                    .child(match etapa {
                        Etapa::Certificado => "Abrindo o certificado...",
                        Etapa::Consultando => "Consultando a Sefin Nacional...",
                    })
                    .into_any_element(),
            ),
            Fase::Desfecho(d) => Some(match d {
                DesfechoDaConsulta::Encontrada(_) => self.detalhe.clone().into_any_element(),
                DesfechoDaConsulta::Divergente { pedida, devolvida } => v_flex()
                    .id("consulta-divergente")
                    .gap_3()
                    .child(faixa(
                        Gravidade::Erro,
                        "A Sefin devolveu uma nota de outra chave, então ela não é mostrada como resultado desta consulta. Não use esta resposta.",
                        cx,
                    ))
                    .child(
                        v_flex()
                            .gap_1()
                            .text_xs()
                            .text_color(muted)
                            .child(format!("Chave pedida: {pedida}"))
                            .child(format!("Chave devolvida: {devolvida}")),
                    )
                    .into_any_element(),
                DesfechoDaConsulta::NaoEncontrada => v_flex()
                    .id("consulta-nao-encontrada")
                    .gap_3()
                    .child(faixa(
                        Gravidade::Aviso,
                        match self.modo {
                            ModoDeConsulta::Chave => TEXTO_NAO_ENCONTRADA,
                            ModoDeConsulta::Dps => TEXTO_DPS_NAO_ENCONTRADA,
                        },
                        cx,
                    ))
                    .into_any_element(),
                DesfechoDaConsulta::SemResposta { texto } => v_flex()
                    .id("consulta-sem-resposta")
                    .gap_3()
                    .child(faixa(Gravidade::Aviso, texto, cx))
                    .child(h_flex().child(tentar(cx)))
                    .into_any_element(),
                DesfechoDaConsulta::Recusada { texto } => v_flex()
                    .id("consulta-recusada")
                    .gap_3()
                    .child(faixa(Gravidade::Erro, format!("Nada foi consultado. {texto}"), cx))
                    .child(h_flex().child(tentar(cx)))
                    .into_any_element(),
            }),
        }
    }
}

impl Render for TelaConsultar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let ocupada = matches!(self.fase, Fase::Trabalhando(_));
        let pode = self.pode_consultar() && !ocupada;
        let modo = self.modo;
        let escolha = ToggleGroup::new("modo-de-consulta")
            .segmented()
            .single()
            .children(ModoDeConsulta::TODOS.iter().enumerate().map(|(i, m)| {
                Toggle::new(SharedString::from(format!("modo-consulta-{i}")))
                    .label(m.rotulo())
                    .checked(modo == *m)
            }))
            .on_click(cx.listener(|this, estados: &Vec<bool>, _, cx| {
                if let Some(i) = estados.iter().position(|e| *e) {
                    if let Some(m) = ModoDeConsulta::TODOS.get(i) {
                        this.escolher_modo(*m, cx);
                    }
                }
            }));
        let campos = match modo {
            ModoDeConsulta::Chave => {
                let estado = self.estado_da_chave();
                // chave inteira que nao confere diz o motivo na hora; o que ainda falta digitar so' aparece depois
                // de passar pelo campo
                let erro = if self.tocada || matches!(estado, EstadoDaChave::Invalida(_)) {
                    estado.mensagem()
                } else {
                    None
                };
                let ajuda = if estado == EstadoDaChave::Valida {
                    Some("Chave conferida: o dígito verificador está certo.".into())
                } else {
                    Some("Só a chave importa: espaços, pontos e traços são ignorados.".into())
                };
                v_form()
                    .columns(1)
                    .child(campo(
                        "Chave de acesso",
                        true,
                        ajuda,
                        erro,
                        Input::new(&self.entrada).id("consulta-chave"),
                    ))
                    .into_any_element()
            }
            ModoDeConsulta::Dps => {
                let estado = self.estado_da_dps();
                let erro = estado.mensagem();
                let serie = serie_efetiva(&self.config.emitente, &self.serie);
                let ajuda = match &estado {
                    EstadoDaDps::Valido(_) => Some(
                        format!(
                            "Vou procurar a DPS nº {}, série {serie}, do emitente das Configurações.",
                            self.numero
                        )
                        .into(),
                    ),
                    _ => Some(
                        format!(
                            "A série vazia vale {serie} (a da configuração). Só números."
                        )
                        .into(),
                    ),
                };
                v_form()
                    .columns(1)
                    .child(campo(
                        "Série da DPS",
                        false,
                        None,
                        None,
                        Input::new(&self.serie_dps).id("consulta-serie"),
                    ))
                    .child(campo(
                        "Número da DPS",
                        true,
                        ajuda,
                        erro,
                        Input::new(&self.numero_dps).id("consulta-numero-dps"),
                    ))
                    .into_any_element()
            }
        };
        let formulario = cartao(cx)
            .id("consulta-formulario")
            .gap_3()
            .child(escolha)
            .child(campos);
        let acoes = h_flex()
            .gap_3()
            .items_center()
            .child(
                Button::new("consultar")
                    .label("Consultar")
                    .icon(IconName::Search)
                    .primary()
                    .loading(ocupada)
                    .disabled(!pode)
                    .on_click(cx.listener(|this, _, window, cx| this.consultar(None, window, cx))),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("Enter no campo também consulta."),
            );
        let resultado = self.resultado(cx);
        div()
            .id("tela-consultar")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.rolagem)
            .child(
                v_flex()
                    .w_full()
                    .max_w(px(760.))
                    .mx_auto()
                    .gap_5()
                    .px_6()
                    .py_8()
                    .child(cabecalho(
                        "Consultar",
                        "Consulte uma NFS-e na Sefin Nacional pela chave de acesso ou pela série e número da DPS. Consultar não altera nada.",
                        cx,
                    ))
                    .child(formulario)
                    .child(acoes)
                    .children(resultado),
            )
    }
}
