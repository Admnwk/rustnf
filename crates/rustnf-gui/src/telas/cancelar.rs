// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O cancelamento de uma nota, num dialogo: motivo, justificativa (com os limites da biblioteca), o ambiente em
//! destaque e, em producao, a marcacao explicita do efeito fiscal. O envio roda fora da linha da tela, pelo mesmo
//! fluxo de certificado e senha da emissao. Cada desfecho tem o seu texto; sem resposta oferece tentar de novo e
//! conferir na Sefin. O mesmo dialogo serve aos outros eventos que a pessoa pede (pedir a analise fiscal do
//! cancelamento, confirmar e recusar a nota recebida): `TipoDeEvento` traz o texto e as regras de cada um.

use std::sync::Arc;

use gpui_kit::component::button::{Button, ButtonVariants as _, Toggle, ToggleGroup};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::dialog::DialogFooter;
use gpui_kit::component::input::{InputEvent, Textarea, TextareaState};
use gpui_kit::component::radio::RadioGroup;
use gpui_kit::component::{
    h_flex, v_flex, ActiveTheme as _, Disableable as _, Icon, IconName, WindowExt as _,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, rgb, AnyElement, App, AppContext as _, Context, Entity, EventEmitter,
    InteractiveElement as _, IntoElement, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, Subscription, Window,
};
use rustnf_app::config::Config;
use rustnf_app::segredo::Segredo;

use super::pecas::{campo, faixa, linha_de_resumo, mensagem_da_sefin, Negrito as _};
use super::senha::{classificar, Credencial, DialogoSenha, EventoSenha};
use crate::fora;
use crate::modelo::ambiente::{Ambiente, CORES_DA_DEMONSTRACAO, SELO_DA_DEMONSTRACAO};
use crate::modelo::cancelamento::{
    DesfechoDoCancelamento, FormularioDeCancelamento, AVISO_DA_PASTA, TEXTO_DA_OFERTA_DE_ANALISE,
    TEXTO_DA_RECUPERADA,
};
use crate::modelo::certificado::Gravidade;
use crate::modelo::evento::TipoDeEvento;
use crate::servicos::{conferir_evento, registrar_evento_nota, Servicos};

/// A nota que se vai cancelar, com o que o dialogo mostra dela.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AlvoDoCancelamento {
    /// O que se pede a Sefin sobre a nota.
    pub(super) tipo: TipoDeEvento,
    /// A chave de acesso.
    pub(super) chave: String,
    /// O numero da NFS-e.
    pub(super) numero: String,
    /// O nome do tomador.
    pub(super) tomador: String,
    /// O nome do emitente (o resumo o mostra no lugar do tomador quando a nota e' recebida).
    pub(super) emitente: String,
    /// O valor do servico, em dinheiro brasileiro.
    pub(super) valor: String,
}

/// O que o dialogo avisa as telas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum EventoCancelamento {
    /// A Sefin registrou o evento pedido para esta nota.
    Registrado {
        /// A chave de acesso.
        chave: String,
        /// O que foi registrado.
        tipo: TipoDeEvento,
    },
}

/// O que esta acontecendo de verdade no envio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Etapa {
    /// Abrindo o certificado.
    Certificado,
    /// Enviando o pedido de cancelamento.
    Enviando,
    /// Conferindo na Sefin se o cancelamento foi registrado.
    Conferindo,
}

/// Onde o dialogo esta.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Fase {
    /// Preenchendo o pedido.
    Preenchendo,
    /// Rodando fora da tela.
    Trabalhando(Etapa),
    /// Terminou (ou ficou em aberto).
    Desfecho(DesfechoDoCancelamento),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Operacao {
    Cancelar,
    Conferir,
}

/// O cancelamento.
pub(super) struct Cancelamento {
    servicos: Arc<Servicos>,
    config: Config,
    /// A nota que se cancela; `None` com o dialogo fechado.
    pub(super) alvo: Option<AlvoDoCancelamento>,
    pub(super) form: FormularioDeCancelamento,
    /// A justificativa ja foi tocada (so' assim a falta dela vira erro na cara).
    tocada: bool,
    pub(super) fase: Fase,
    /// O campo da justificativa.
    pub(super) justificativa: Entity<TextareaState>,
    /// A senha digitada, enquanto o dialogo estiver aberto (nunca guardada: some ao fechar).
    senha_da_sessao: Option<Segredo>,
    senha_pedida: Option<(Entity<DialogoSenha>, Operacao)>,
    sub_senha: Option<Subscription>,
    _sub: Subscription,
}

impl EventEmitter<EventoCancelamento> for Cancelamento {}

impl Cancelamento {
    /// O cancelamento, com o dialogo fechado.
    pub(super) fn new(
        servicos: Arc<Servicos>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let justificativa = cx.new(|cx| {
            TextareaState::new(window, cx)
                .placeholder("Escreva aqui (fica registrado no documento fiscal)")
                .auto_grow(3, 6)
        });
        let sub = cx.subscribe_in(
            &justificativa,
            window,
            |this, entrada, evento: &InputEvent, window, cx| {
                if matches!(evento, InputEvent::Change) {
                    this.form.justificativa = entrada.read(cx).value().to_string();
                    this.tocada = true;
                    window.refresh();
                    cx.notify();
                }
            },
        );
        Self {
            servicos,
            config: Config::default(),
            alvo: None,
            form: FormularioDeCancelamento::default(),
            tocada: false,
            fase: Fase::Preenchendo,
            justificativa,
            senha_da_sessao: None,
            senha_pedida: None,
            sub_senha: None,
            _sub: sub,
        }
    }

    /// A configuracao que vale (a efetiva) chegou ou mudou.
    pub(super) fn definir_config(&mut self, config: Config, cx: &mut Context<Self>) {
        self.config = config;
        cx.notify();
    }

    fn ambiente(&self) -> Ambiente {
        Ambiente::de_config(&self.config.sefin.ambiente)
    }

    /// Abre o dialogo para a nota. Cada abertura comeca com o formulario limpo.
    pub(super) fn iniciar(
        &mut self,
        alvo: AlvoDoCancelamento,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(self.fase, Fase::Trabalhando(_)) {
            return;
        }
        self.form = FormularioDeCancelamento::de(alvo.tipo);
        self.alvo = Some(alvo);
        self.tocada = false;
        self.fase = Fase::Preenchendo;
        self.senha_da_sessao = None;
        self.justificativa
            .update(cx, |e, cx| e.set_value("", window, cx));
        self.abrir_dialogo(window, cx);
    }

    fn escolher_motivo(&mut self, codigo: &str, cx: &mut Context<Self>) {
        self.form.escolher_motivo(codigo);
        cx.notify();
    }

    fn marcar_efeito_fiscal(&mut self, marcou: bool, cx: &mut Context<Self>) {
        self.form.efeito_fiscal_entendido = marcou;
        cx.notify();
    }

    /// Volta ao formulario depois de uma rejeicao ou de uma recusa, com tudo como estava.
    fn voltar_ao_formulario(&mut self, cx: &mut Context<Self>) {
        self.fase = Fase::Preenchendo;
        cx.notify();
    }

    /// Depois da recusa por prazo, o mesmo dialogo passa a pedir a analise fiscal do cancelamento, com o motivo e a
    /// justificativa ja escritos. A marcacao de producao volta desmarcada: e' outro pedido, e se confirma de novo.
    fn pedir_analise(&mut self, cx: &mut Context<Self>) {
        let prazo = matches!(self.fase, Fase::Desfecho(ref d) if d.recusado_por_prazo());
        if !prazo || self.form.tipo != TipoDeEvento::Cancelar {
            return;
        }
        let Some(alvo) = self.alvo.as_mut() else {
            return;
        };
        alvo.tipo = TipoDeEvento::AnaliseFiscal;
        let anterior = std::mem::replace(
            &mut self.form,
            FormularioDeCancelamento::de(TipoDeEvento::AnaliseFiscal),
        );
        if let Some(m) = anterior.motivo {
            self.form.escolher_motivo(m);
        }
        self.form.justificativa = anterior.justificativa;
        self.fase = Fase::Preenchendo;
        cx.notify();
    }

    /// O pedido pode ser enviado agora.
    pub(super) fn pode_enviar(&self) -> bool {
        matches!(self.fase, Fase::Preenchendo) && self.form.pode_enviar(self.ambiente())
    }

    /// Cancelar a nota: envia o pedido.
    pub(super) fn enviar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.pode_enviar() {
            return;
        }
        self.executar(Operacao::Cancelar, None, window, cx);
    }

    /// Tentar de novo (reenvia o mesmo pedido).
    fn tentar_de_novo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(self.fase, Fase::Desfecho(ref d) if d.deixa_em_aberto() || *d == DesfechoDoCancelamento::NaoRegistrado)
        {
            self.executar(Operacao::Cancelar, None, window, cx);
        }
    }

    /// Conferir na Sefin se o cancelamento foi registrado.
    fn conferir(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(self.fase, Fase::Desfecho(ref d) if d.deixa_em_aberto()) {
            self.executar(Operacao::Conferir, None, window, cx);
        }
    }

    fn executar(
        &mut self,
        operacao: Operacao,
        digitada: Option<Segredo>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(alvo) = self.alvo.clone() else {
            return;
        };
        if matches!(self.fase, Fase::Trabalhando(_)) {
            return;
        }
        let anterior = self.fase.clone();
        self.fase = Fase::Trabalhando(Etapa::Certificado);
        window.refresh();
        cx.notify();
        let servicos = Arc::clone(&self.servicos);
        let fabrica = servicos.fabrica();
        let config = self.config.clone();
        let tipo = self.form.tipo;
        let motivo = self
            .form
            .motivo
            .unwrap_or(if tipo.motivos().is_empty() { "" } else { "1" })
            .to_string();
        let justificativa = self.form.justificativa_para_enviar();
        let usada = digitada.or_else(|| {
            self.senha_da_sessao
                .as_ref()
                .map(|s| Segredo::novo(s.expor().to_string()))
        });
        let digitou = usada.is_some();
        let para_guardar = usada.as_ref().map(|s| Segredo::novo(s.expor().to_string()));
        cx.spawn_in(window, async move |this, cx| {
            let (s2, c2) = (Arc::clone(&servicos), config.clone());
            let preparo = cx
                .background_spawn(async move { s2.preparar_emissao(&c2, usada.as_ref()) })
                .await;
            let preparo = match classificar(preparo, digitou) {
                Credencial::Pronta(p) => *p,
                Credencial::PedirSenha(erro) => {
                    this.update_in(cx, |this, window, cx| {
                        this.fase = anterior;
                        this.pedir_senha(operacao, erro, window, cx);
                    })
                    .ok();
                    return;
                }
                Credencial::Falhou(texto) => {
                    this.update_in(cx, |this, window, cx| {
                        this.aplicar(DesfechoDoCancelamento::Recusado { texto }, window, cx);
                    })
                    .ok();
                    return;
                }
            };
            this.update_in(cx, |this, window, cx| {
                if para_guardar.is_some() {
                    this.senha_da_sessao = para_guardar;
                }
                this.fase = Fase::Trabalhando(match operacao {
                    Operacao::Cancelar => Etapa::Enviando,
                    Operacao::Conferir => Etapa::Conferindo,
                });
                window.refresh();
                cx.notify();
            })
            .ok();
            let chave = alvo.chave.clone();
            let trabalho = async move {
                match operacao {
                    Operacao::Cancelar => {
                        registrar_evento_nota(
                            fabrica,
                            preparo,
                            config,
                            tipo,
                            chave,
                            motivo,
                            justificativa,
                        )
                        .await
                    }
                    Operacao::Conferir => {
                        conferir_evento(fabrica, preparo, config, tipo, chave).await
                    }
                }
            };
            let desfecho = match fora::rede(trabalho) {
                Some(tarefa) => {
                    tarefa
                        .await
                        .unwrap_or_else(|_| DesfechoDoCancelamento::SemResposta {
                            texto: tipo.sem_resposta(),
                        })
                }
                None => DesfechoDoCancelamento::Recusado {
                    texto: "Não foi possível preparar a chamada de rede.".into(),
                },
            };
            this.update_in(cx, |this, window, cx| this.aplicar(desfecho, window, cx))
                .ok();
        })
        .detach();
    }

    fn aplicar(
        &mut self,
        desfecho: DesfechoDoCancelamento,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if desfecho.registrado() {
            self.senha_da_sessao = None;
            if let Some(a) = &self.alvo {
                cx.emit(EventoCancelamento::Registrado {
                    chave: a.chave.clone(),
                    tipo: a.tipo,
                });
            }
        }
        self.fase = Fase::Desfecho(desfecho);
        window.refresh();
        cx.notify();
    }

    // ---------------------------------------------------------------- senha

    /// Pede a senha do certificado. O dialogo do cancelamento sai da frente e volta depois da senha.
    fn pedir_senha(
        &mut self,
        operacao: Operacao,
        erro: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.close_dialog(cx);
        let desligado = self.servicos.cofre_desligado();
        let dialogo = cx.new(|cx| DialogoSenha::new(desligado, erro, window, cx));
        self.sub_senha = Some(cx.subscribe_in(
            &dialogo,
            window,
            |this, dialogo, evento: &EventoSenha, window, cx| {
                this.sub_senha = None;
                let pedida = this.senha_pedida.take();
                match (evento, pedida) {
                    (EventoSenha::Usada, Some((_, operacao))) => {
                        let senha = dialogo.update(cx, |d, _| d.tomar());
                        this.abrir_dialogo(window, cx);
                        this.executar(operacao, senha, window, cx);
                    }
                    _ => this.abrir_dialogo(window, cx),
                }
            },
        ));
        self.senha_pedida = Some((dialogo.clone(), operacao));
        DialogoSenha::abrir(&dialogo, window, cx);
    }

    // ---------------------------------------------------------------- desenho

    fn trabalhando_texto(tipo: TipoDeEvento, etapa: Etapa) -> &'static str {
        match etapa {
            Etapa::Certificado => "Abrindo o certificado...",
            Etapa::Enviando => tipo.enviando(),
            Etapa::Conferindo => tipo.conferindo(),
        }
    }

    fn selo(&self) -> AnyElement {
        let ambiente = self.ambiente();
        let cores = ambiente.cores();
        let demonstracao = self.servicos.demonstracao();
        v_flex()
            .gap_2()
            .child(
                h_flex().gap_2().items_center().child(
                    div()
                        .id("selo-do-cancelamento")
                        .px_3()
                        .py_1()
                        .rounded_full()
                        .text_sm()
                        .font_semibold()
                        .bg(rgb(cores.fundo))
                        .text_color(rgb(cores.texto))
                        .child(ambiente.selo()),
                ),
            )
            .when(demonstracao, |d| {
                d.child(
                    div()
                        .id("aviso-de-demonstracao-cancelamento")
                        .px_3()
                        .py_1()
                        .rounded_md()
                        .text_sm()
                        .font_semibold()
                        .bg(rgb(CORES_DA_DEMONSTRACAO.fundo))
                        .text_color(rgb(CORES_DA_DEMONSTRACAO.texto))
                        .child(SELO_DA_DEMONSTRACAO),
                )
            })
            .child(div().text_sm().child(
                if self.form.tipo == TipoDeEvento::Cancelar && ambiente == Ambiente::Restrita {
                    self.form.tipo.efeito_nos_testes()
                } else {
                    self.form.tipo.efeito()
                },
            ))
            .when(
                self.form.tipo != TipoDeEvento::Cancelar && ambiente == Ambiente::Restrita,
                |d| {
                    d.child(
                        div()
                            .text_sm()
                            .font_semibold()
                            .child(self.form.tipo.efeito_nos_testes()),
                    )
                },
            )
            .into_any_element()
    }

    fn formulario(&self, dialogo: &Entity<Self>, cx: &App) -> AnyElement {
        let alvo = self.alvo.clone().unwrap_or(AlvoDoCancelamento {
            tipo: TipoDeEvento::Cancelar,
            chave: String::new(),
            numero: String::new(),
            tomador: String::new(),
            emitente: String::new(),
            valor: String::new(),
        });
        let tipo = self.form.tipo;
        let recebida = matches!(tipo, TipoDeEvento::Confirmar | TipoDeEvento::Recusar);
        let ambiente = self.ambiente();
        let motivo_atual = self.form.motivo;
        let d1 = dialogo.clone();
        let d2 = dialogo.clone();
        let motivos = tipo.rotulos_dos_motivos();
        let motivo: AnyElement = if motivos.len() > 3 {
            // seis motivos nao cabem lado a lado: uma lista, um por linha
            RadioGroup::vertical("motivo-do-evento")
                .selected_index(motivo_atual.and_then(|c| motivos.iter().position(|m| m.0 == c)))
                .children(motivos.iter().map(|(_, rotulo)| *rotulo))
                .on_click(move |i: &usize, window, cx| {
                    if let Some((codigo, _)) = motivos.get(*i) {
                        d1.update(cx, |c, cx| c.escolher_motivo(codigo, cx));
                        window.refresh();
                    }
                })
                .into_any_element()
        } else {
            ToggleGroup::new("motivo-do-cancelamento")
                .segmented()
                .single()
                .children(motivos.iter().map(|(codigo, rotulo)| {
                    Toggle::new(SharedString::from(format!("motivo-cancelamento-{codigo}")))
                        .label(*rotulo)
                        .checked(motivo_atual == Some(*codigo))
                }))
                .on_click(move |estados: &Vec<bool>, window, cx| {
                    if let Some(i) = estados.iter().position(|e| *e) {
                        if let Some((codigo, _)) = motivos.get(i) {
                            d1.update(cx, |c, cx| c.escolher_motivo(codigo, cx));
                            window.refresh();
                        }
                    }
                })
                .into_any_element()
        };
        let erro = self.form.erro_da_justificativa(self.tocada);
        let contador = SharedString::from(self.form.contador());
        let resumo = v_flex()
            .gap_1p5()
            .p_3()
            .rounded_md()
            .border_1()
            .border_color(cx.theme().border)
            .child(linha_de_resumo("Nota", format!("nº {}", alvo.numero), cx))
            .child(linha_de_resumo(
                if recebida { "Emitente" } else { "Tomador" },
                match if recebida {
                    &alvo.emitente
                } else {
                    &alvo.tomador
                } {
                    nome if nome.is_empty() => "-".to_string(),
                    nome => nome.clone(),
                },
                cx,
            ))
            .child(linha_de_resumo("Valor", format!("R$ {}", alvo.valor), cx));
        let produ = ambiente == Ambiente::Producao;
        let marcado = self.form.efeito_fiscal_entendido;
        v_flex()
            .gap_3()
            .child(self.selo())
            .child(resumo)
            .when(!motivos.is_empty(), |d| {
                d.child(campo(
                    if recebida {
                        "Motivo da recusa"
                    } else {
                        "Motivo"
                    },
                    true,
                    None,
                    None,
                    motivo,
                ))
            })
            .when(tipo.tem_justificativa(), |d| {
                d.child(campo(
                    "Justificativa",
                    tipo.exige_justificativa(motivo_atual),
                    Some(contador),
                    erro,
                    div()
                        .id("cancelamento-justificativa")
                        .child(Textarea::new(&self.justificativa)),
                ))
            })
            .when(produ, |d| {
                d.child(
                    Checkbox::new("entendo-efeito-fiscal")
                        .label(tipo.marcacao_de_producao())
                        .checked(marcado)
                        .on_click(move |marcou: &bool, window, cx| {
                            d2.update(cx, |c, cx| c.marcar_efeito_fiscal(*marcou, cx));
                            window.refresh();
                        }),
                )
            })
            .into_any_element()
    }

    fn desfecho(&self, dialogo: &Entity<Self>, d: &DesfechoDoCancelamento, cx: &App) -> AnyElement {
        let muted = cx.theme().muted_foreground;
        let tipo = self.form.tipo;
        let nova = |rotulo: &'static str, id: &'static str, icone: IconName, primario: bool| {
            let b = Button::new(id).label(rotulo).icon(icone);
            if primario {
                b.primary()
            } else {
                b.outline()
            }
        };
        let (d_tentar, d_conferir, d_voltar) = (dialogo.clone(), dialogo.clone(), dialogo.clone());
        let tentar = nova(
            "Tentar de novo",
            "cancelamento-tentar-de-novo",
            IconName::RefreshCw,
            true,
        )
        .on_click(move |_, window, cx| {
            d_tentar.update(cx, |c, cx| c.tentar_de_novo(window, cx));
        });
        let conferir = nova(
            "Conferir na Sefin",
            "cancelamento-conferir",
            IconName::Search,
            false,
        )
        .on_click(move |_, window, cx| {
            d_conferir.update(cx, |c, cx| c.conferir(window, cx));
        });
        let voltar = nova(
            "Voltar ao formulário",
            "cancelamento-voltar",
            IconName::ChevronLeft,
            false,
        )
        .on_click(move |_, _, cx| {
            d_voltar.update(cx, |c, cx| c.voltar_ao_formulario(cx));
        });
        match d {
            DesfechoDoCancelamento::Registrado { recuperada, marcada } => v_flex()
                .id("cancelamento-registrado")
                .gap_3()
                .child(faixa(Gravidade::Ok, tipo.registrado(), cx))
                .when(*recuperada, |c| {
                    c.child(div().text_sm().text_color(muted).child(TEXTO_DA_RECUPERADA))
                })
                .when(*marcada == Some(false), |c| {
                    c.child(faixa(Gravidade::Aviso, AVISO_DA_PASTA, cx).id("aviso-da-pasta"))
                })
                .into_any_element(),
            DesfechoDoCancelamento::Rejeitado { mensagens } => {
                // na recusa por prazo, "corrija e tente de novo" nao serve: a faixa da oferta diz o que fazer
                let prazo = tipo == TipoDeEvento::Cancelar && d.recusado_por_prazo();
                let mut c = v_flex()
                    .id("cancelamento-rejeitado")
                    .gap_3()
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(Icon::new(IconName::CircleAlert).text_color(cx.theme().danger))
                            .child(div().font_semibold().child(tipo.recusou())),
                    )
                    .when(!prazo, |c| {
                        c.child(
                            div()
                                .text_sm()
                                .text_color(muted)
                                .child(tipo.depois_de_recusar()),
                        )
                    });
                for m in mensagens {
                    c = c.child(mensagem_da_sefin(m, Gravidade::Erro, cx));
                }
                if prazo {
                    let d_analise = dialogo.clone();
                    let analise = nova(
                        TipoDeEvento::AnaliseFiscal.rotulo_da_acao(),
                        "cancelamento-pedir-analise",
                        IconName::Eye,
                        true,
                    )
                    .on_click(move |_, window, cx| {
                        d_analise.update(cx, |c, cx| c.pedir_analise(cx));
                        window.refresh();
                    });
                    c = c
                        .child(faixa(Gravidade::Neutro, TEXTO_DA_OFERTA_DE_ANALISE, cx))
                        .child(h_flex().gap_2().child(analise).child(voltar));
                    return c.into_any_element();
                }
                c.child(h_flex().child(voltar)).into_any_element()
            }
            DesfechoDoCancelamento::SemResposta { texto } => v_flex()
                .id("cancelamento-sem-resposta")
                .gap_3()
                .child(faixa(Gravidade::Aviso, texto.clone(), cx))
                .child(
                    div()
                        .text_sm()
                        .text_color(muted)
                        .child(tipo.explicacao_de_tentar_de_novo()),
                )
                .child(if tipo == TipoDeEvento::Cancelar {
                    h_flex().gap_2().child(tentar).child(conferir)
                } else {
                    // so' o cancelamento tem pedido que se reenvia sem risco: os outros conferem primeiro
                    h_flex().gap_2().child(conferir)
                })
                .into_any_element(),
            DesfechoDoCancelamento::NaoRegistrado => v_flex()
                .id("cancelamento-nao-registrado")
                .gap_3()
                .child(faixa(
                    Gravidade::Neutro,
                    if tipo == TipoDeEvento::Cancelar {
                        "A Sefin não tem cancelamento registrado para esta nota: ela continua válida. Pode tentar de novo."
                    } else {
                        "A Sefin confirma que este pedido não foi registrado: nada mudou na nota. Pode pedir de novo."
                    },
                    cx,
                ))
                .child(h_flex().gap_2().child(tentar))
                .into_any_element(),
            DesfechoDoCancelamento::Recusado { texto } => v_flex()
                .id("cancelamento-recusado")
                .gap_3()
                .child(faixa(Gravidade::Erro, format!("Nada foi enviado. {texto}"), cx))
                .child(h_flex().child(voltar))
                .into_any_element(),
        }
    }

    /// Abre o dialogo do cancelamento. O conteudo e' refeito a cada desenho a partir do estado.
    pub(super) fn abrir_dialogo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let ent = cx.entity();
        window.open_dialog(cx, move |dialog, _, cx| {
            let c = ent.read(cx);
            let trabalhando = matches!(c.fase, Fase::Trabalhando(_));
            let preenchendo = matches!(c.fase, Fase::Preenchendo);
            let producao = c.ambiente() == Ambiente::Producao;
            let titulo = c
                .form
                .tipo
                .titulo(c.alvo.as_ref().map_or("", |a| a.numero.as_str()));
            let tipo = c.form.tipo;
            let pode = c.pode_enviar();
            let motivo = if preenchendo {
                c.form.motivo_de_nao_enviar(c.ambiente())
            } else {
                None
            };
            let corpo: AnyElement = match c.fase.clone() {
                Fase::Preenchendo => c.formulario(&ent, cx),
                Fase::Trabalhando(etapa) => h_flex()
                    .id("cancelamento-trabalhando")
                    .gap_2()
                    .items_center()
                    .p_4()
                    .child(Icon::new(IconName::LoaderCircle))
                    .child(div().text_sm().child(Self::trabalhando_texto(tipo, etapa)))
                    .into_any_element(),
                Fase::Desfecho(d) => c.desfecho(&ent, &d, cx),
            };
            let (e_enviar, e_fechou) = (ent.clone(), ent.clone());
            let (muted, aviso) = (
                cx.theme().muted_foreground,
                crate::telas::pecas::cor_da(Gravidade::Aviso, cx),
            );
            let rodape = DialogFooter::new()
                .child(
                    Button::new("cancelamento-fechar")
                        .label(if preenchendo { "Voltar" } else { "Fechar" })
                        .outline()
                        .disabled(trabalhando)
                        .on_click(|_, window, cx| window.close_dialog(cx)),
                )
                .when(preenchendo, |f| {
                    f.child(
                        Button::new("cancelamento-enviar")
                            .label(tipo.rotulo_de_enviar(c.ambiente()))
                            .when(producao, |b| b.danger())
                            .when(!producao, |b| b.primary())
                            .disabled(!pode)
                            .on_click(move |_, window, cx| {
                                e_enviar.update(cx, |c, cx| c.enviar(window, cx));
                            }),
                    )
                });
            dialog
                .title(titulo)
                .w(px(580.))
                .keyboard(!trabalhando)
                .overlay_closable(false)
                .close_button(!trabalhando)
                .on_close(move |_, _, cx| {
                    e_fechou.update(cx, |c, cx| {
                        c.senha_da_sessao = None;
                        cx.notify();
                    });
                })
                .child(
                    // o corpo rola quando passa da janela (a lista de motivos da recusa e' longa): o rodape fica sempre a vista
                    v_flex()
                        .id("corpo-do-dialogo")
                        .gap_3()
                        .max_h(px(520.))
                        .overflow_y_scroll()
                        .child(corpo)
                        .children(motivo.map(|m| {
                            div()
                                .id("motivo-do-botao-cancelar")
                                .text_xs()
                                .text_color(if pode { muted } else { aviso })
                                .child(m)
                        })),
                )
                .footer(rodape)
        });
    }
}
