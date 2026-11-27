// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O dialogo da NF-e: cancelar, carta de correcao, inutilizar numeros, retomar as notas sem resposta e conferir uma
//! nota na SEFAZ. E' o mesmo desenho do cancelamento da NFS-e (ambiente em destaque, justificativa com contador,
//! marcacao explicita em producao, desfecho com o que fazer, sem clique duplo, senha pedida so' se nao esta no cofre),
//! generalizado para os pedidos da NF-e. Nada sai da maquina sem o botao; em producao, nem com ele sem a marcacao.

use std::sync::Arc;

use gpui_kit::component::button::Button;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::dialog::DialogFooter;
use gpui_kit::component::input::{InputEvent, InputState, TextareaState};
use gpui_kit::component::{
    h_flex, v_flex, ActiveTheme as _, Disableable as _, Icon, IconName, WindowExt as _,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, AnyElement, App, AppContext as _, Context, Entity, EventEmitter,
    InteractiveElement as _, IntoElement, ParentElement as _, StatefulInteractiveElement as _,
    Styled as _, Subscription, Window,
};
use rustnf_app::config::Config;
use rustnf_app::segredo::Segredo;

use super::pecas::botao_de_perigo;
use super::senha::{classificar, Credencial, DialogoSenha, EventoSenha};
use crate::fora;
use crate::modelo::ambiente::Ambiente;
use crate::modelo::certificado::Gravidade;
use crate::modelo::nfe_config::ambiente_da_nfe;
use crate::modelo::nfe_eventos::{
    DesfechoNfe, FormularioDeCancelamentoNfe, FormularioDeCorrecao, FormularioDeInutilizacao,
    PedidoNfe,
};
use crate::modelo::nfe_resultados::{ConsultaNfe, ResumoDaRetomada};
use crate::servicos::{
    cancelar_doc, conferir_cancelamento_doc, consultar_doc, corrigir_nfe, inutilizar_nfe,
    retomar_doc, Documento, Servicos,
};

/// A nota de que o dialogo trata, com o que ele mostra dela.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AlvoNfe {
    /// A chave de acesso.
    pub(super) chave: String,
    /// O numero.
    pub(super) numero: String,
    /// A serie.
    pub(super) serie: String,
    /// O destinatario.
    pub(super) destinatario: String,
    /// O valor, em dinheiro brasileiro.
    pub(super) valor: String,
    /// Quantas cartas de correcao a nota ja tem.
    pub(super) cartas: u32,
}

/// O que o dialogo faz.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Operacao {
    /// Cancelar a nota.
    Cancelar,
    /// Carta de correcao.
    Corrigir,
    /// Inutilizar numeros.
    Inutilizar,
    /// Retomar as notas sem resposta.
    Retomar,
    /// Conferir uma nota na SEFAZ.
    Conferir,
}

impl Operacao {
    fn titulo(self, numero: &str, doc: Documento) -> String {
        let nome = doc.nome();
        match self {
            Self::Cancelar => format!("Cancelar a {nome} nº {numero}"),
            Self::Corrigir => format!("Carta de correção da {nome} nº {numero}"),
            Self::Inutilizar => PedidoNfe::Inutilizar.titulo().into(),
            Self::Retomar if doc == Documento::Nfce => "Transmitir as NFC-e pendentes".into(),
            Self::Retomar => "Retomar as notas sem resposta".into(),
            Self::Conferir => format!("Conferir a {nome} nº {numero} na SEFAZ"),
        }
    }

    fn pedido(self) -> Option<PedidoNfe> {
        match self {
            Self::Cancelar => Some(PedidoNfe::Cancelar),
            Self::Corrigir => Some(PedidoNfe::Corrigir),
            Self::Inutilizar => Some(PedidoNfe::Inutilizar),
            Self::Retomar | Self::Conferir => None,
        }
    }
}

/// O que dizer enquanto roda.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Etapa {
    /// Abrindo o certificado.
    Certificado,
    /// Falando com a SEFAZ.
    Enviando,
}

/// O que o dialogo mostra depois.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Resposta {
    /// O pedido de evento ou de inutilizacao.
    Pedido(DesfechoNfe),
    /// A consulta.
    Consulta(ConsultaNfe),
    /// A retomada.
    Retomada(ResumoDaRetomada),
}

/// Onde o dialogo esta.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Fase {
    /// Preenchendo.
    Preenchendo,
    /// Rodando fora da tela.
    Trabalhando(Etapa),
    /// Terminou (ou ficou em aberto).
    Desfecho(Resposta),
}

/// O que o dialogo avisa as telas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EventoDialogoNfe {
    /// A pasta de notas pode ter mudado: a lista e o detalhe leem de novo.
    MudouAPasta,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Passo {
    Enviar,
    ConferirCancelamento,
}

/// O dialogo da NF-e.
pub(super) struct DialogoNfe {
    servicos: Arc<Servicos>,
    /// A NF-e ou a NFC-e: a NFC-e usa a pasta `nfce/` e a configuracao de `[nfce]`.
    pub(super) documento: Documento,
    config: Config,
    /// O aviso do prazo nacional de cancelamento da NFC-e (so' a NFC-e cancelada fora das 24 horas): nunca bloqueia.
    pub(super) aviso_de_prazo: Option<String>,
    /// A nota; `None` em inutilizar e retomar.
    pub(super) alvo: Option<AlvoNfe>,
    pub(super) operacao: Operacao,
    pub(super) cancelar: FormularioDeCancelamentoNfe,
    pub(super) corrigir: FormularioDeCorrecao,
    pub(super) inutilizar: FormularioDeInutilizacao,
    /// Em producao: marcou "Entendo..." na retomada.
    pub(super) retomar_entendido: bool,
    /// Quantas notas a retomada vai tratar.
    a_retomar: usize,
    texto_oficial_aberto: bool,
    tocada: bool,
    pub(super) fase: Fase,
    pub(super) texto: Entity<TextareaState>,
    serie: Entity<InputState>,
    inicio: Entity<InputState>,
    fim: Entity<InputState>,
    senha_da_sessao: Option<Segredo>,
    senha_pedida: Option<(Entity<DialogoSenha>, Passo)>,
    sub_senha: Option<Subscription>,
    _subs: Vec<Subscription>,
}

mod desenho;

pub(super) use desenho::selo_do_dialogo;

impl EventEmitter<EventoDialogoNfe> for DialogoNfe {}

impl DialogoNfe {
    pub(super) fn new(
        servicos: Arc<Servicos>,
        documento: Documento,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let texto = cx.new(|cx| {
            TextareaState::new(window, cx)
                .placeholder("Escreva aqui (fica registrado no documento fiscal)")
                .auto_grow(3, 8)
        });
        let entrada = |window: &mut Window, cx: &mut Context<Self>, dica: &'static str| {
            cx.new(|cx| InputState::new(window, cx).placeholder(dica))
        };
        let serie = entrada(window, cx, "Da configuração");
        let inicio = entrada(window, cx, "Ex.: 12");
        let fim = entrada(window, cx, "Vazio: só o primeiro");
        let mut subs =
            vec![
                cx.subscribe_in(&texto, window, |this, e, ev: &InputEvent, window, cx| {
                    if matches!(ev, InputEvent::Change) {
                        let t = e.read(cx).value().to_string();
                        match this.operacao {
                            Operacao::Cancelar => this.cancelar.justificativa = t,
                            Operacao::Corrigir => this.corrigir.texto = t,
                            Operacao::Inutilizar => this.inutilizar.justificativa = t,
                            _ => {}
                        }
                        this.tocada = true;
                        window.refresh();
                        cx.notify();
                    }
                }),
            ];
        for (i, e) in [&serie, &inicio, &fim].into_iter().enumerate() {
            subs.push(
                cx.subscribe_in(e, window, move |this, e, ev: &InputEvent, window, cx| {
                    if matches!(ev, InputEvent::Change) {
                        let t = e.read(cx).value().to_string();
                        let f = &mut this.inutilizar;
                        match i {
                            0 => f.serie = t,
                            1 => f.inicio = t,
                            _ => f.fim = t,
                        }
                        window.refresh();
                        cx.notify();
                    }
                }),
            );
        }
        Self {
            servicos,
            documento,
            aviso_de_prazo: None,
            config: Config::default(),
            alvo: None,
            operacao: Operacao::Cancelar,
            cancelar: FormularioDeCancelamentoNfe::default(),
            corrigir: FormularioDeCorrecao::default(),
            inutilizar: FormularioDeInutilizacao::default(),
            retomar_entendido: false,
            a_retomar: 0,
            texto_oficial_aberto: false,
            tocada: false,
            fase: Fase::Preenchendo,
            texto,
            serie,
            inicio,
            fim,
            senha_da_sessao: None,
            senha_pedida: None,
            sub_senha: None,
            _subs: subs,
        }
    }

    /// A configuracao que vale (a efetiva) chegou ou mudou.
    pub(super) fn definir_config(&mut self, config: Config, cx: &mut Context<Self>) {
        // a NFC-e enxerga o ambiente e a serie dela como se fossem os da NF-e: o resto do dialogo nao precisa saber
        self.config = match self.documento {
            Documento::Nfe => config,
            Documento::Nfce => config.como_nfce(),
        };
        cx.notify();
    }

    fn producao(&self) -> bool {
        ambiente_da_nfe(&self.config.nfe) == Ambiente::Producao
    }

    fn zerar(&mut self, op: Operacao, alvo: Option<AlvoNfe>, window: &mut Window, cx: &mut App) {
        self.operacao = op;
        self.cancelar = FormularioDeCancelamentoNfe::default();
        self.corrigir = FormularioDeCorrecao::da_nota(alvo.as_ref().map_or(0, |a| a.cartas));
        self.inutilizar = FormularioDeInutilizacao::default();
        self.retomar_entendido = false;
        self.texto_oficial_aberto = false;
        self.aviso_de_prazo = match (&alvo, op, self.documento) {
            (Some(a), Operacao::Cancelar, Documento::Nfce) => {
                self.servicos.aviso_do_prazo_nfce(&self.config, &a.chave)
            }
            _ => None,
        };
        self.alvo = alvo;
        self.tocada = false;
        self.fase = Fase::Preenchendo;
        self.senha_da_sessao = None;
        self.texto.update(cx, |e, cx| e.set_value("", window, cx));
        for e in [&self.serie, &self.inicio, &self.fim] {
            e.update(cx, |e, cx| e.set_value("", window, cx));
        }
    }

    fn ocupado(&self) -> bool {
        matches!(self.fase, Fase::Trabalhando(_))
    }

    /// Abre o dialogo de `op` para a nota.
    pub(super) fn iniciar(
        &mut self,
        op: Operacao,
        alvo: Option<AlvoNfe>,
        a_retomar: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.ocupado() {
            return;
        }
        self.a_retomar = a_retomar;
        self.zerar(op, alvo, window, cx);
        self.abrir_dialogo(window, cx);
        if op == Operacao::Conferir {
            self.executar(Passo::Enviar, None, window, cx);
        }
    }

    /// O pedido pode ser enviado agora.
    pub(super) fn pode_enviar(&self) -> bool {
        matches!(self.fase, Fase::Preenchendo) && self.motivo_de_nao_enviar().is_none()
    }

    fn motivo_de_nao_enviar(&self) -> Option<String> {
        let producao = self.producao();
        match self.operacao {
            Operacao::Cancelar => self.cancelar.motivo_de_nao_enviar(producao),
            Operacao::Corrigir => self.corrigir.motivo_de_nao_enviar(producao),
            Operacao::Inutilizar => self.inutilizar.motivo_de_nao_enviar(producao),
            Operacao::Retomar => (producao && !self.retomar_entendido).then(|| {
                "Em produção, marque que você entende que as notas pendentes podem ser autorizadas."
                    .to_string()
            }),
            Operacao::Conferir => Some("A conferência roda sozinha.".into()),
        }
    }

    /// Envia o pedido (o botao do dialogo).
    pub(super) fn enviar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.pode_enviar() {
            self.executar(Passo::Enviar, None, window, cx);
        }
    }

    fn tentar_de_novo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(&self.fase, Fase::Desfecho(Resposta::Pedido(d)) if d.deixa_em_aberto()) {
            self.executar(Passo::Enviar, None, window, cx);
        }
    }

    fn conferir_pedido(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(&self.fase, Fase::Desfecho(Resposta::Pedido(d)) if d.deixa_em_aberto()) {
            self.executar(Passo::ConferirCancelamento, None, window, cx);
        }
    }

    fn voltar(&mut self, cx: &mut Context<Self>) {
        self.fase = Fase::Preenchendo;
        cx.notify();
    }

    fn executar(
        &mut self,
        passo: Passo,
        digitada: Option<Segredo>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.ocupado() {
            return;
        }
        let anterior = self.fase.clone();
        self.fase = Fase::Trabalhando(Etapa::Certificado);
        window.refresh();
        cx.notify();
        let servicos = Arc::clone(&self.servicos);
        let fabrica = servicos.fabrica_nfe();
        let config = self.config.clone();
        let doc = self.documento;
        let op = self.operacao;
        let alvo = self.alvo.clone();
        let (cancelar, corrigir, inutilizar) = (
            self.cancelar.para_enviar(),
            self.corrigir.para_enviar(),
            self.inutilizar.clone(),
        );
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
                .background_spawn(async move { s2.preparar_nfe(&c2, usada.as_ref()) })
                .await;
            let preparo = match classificar(preparo, digitou) {
                Credencial::Pronta(p) => *p,
                Credencial::PedirSenha(erro) => {
                    this.update_in(cx, |this, window, cx| {
                        this.fase = anterior;
                        this.pedir_senha(passo, erro, window, cx);
                    })
                    .ok();
                    return;
                }
                Credencial::Falhou(texto) => {
                    this.update_in(cx, |this, window, cx| {
                        let r = Resposta::Pedido(DesfechoNfe::Recusado { texto });
                        this.aplicar(r, window, cx);
                    })
                    .ok();
                    return;
                }
            };
            this.update_in(cx, |this, window, cx| {
                if para_guardar.is_some() {
                    this.senha_da_sessao = para_guardar;
                }
                this.fase = Fase::Trabalhando(Etapa::Enviando);
                window.refresh();
                cx.notify();
            })
            .ok();
            let chave = alvo.as_ref().map(|a| a.chave.clone()).unwrap_or_default();
            let trabalho = async move {
                match (op, passo) {
                    (Operacao::Cancelar, Passo::Enviar) => Resposta::Pedido(
                        cancelar_doc(doc, fabrica, preparo, config, chave, cancelar).await,
                    ),
                    (Operacao::Corrigir, Passo::Enviar) => Resposta::Pedido(
                        corrigir_nfe(fabrica, preparo, config, chave, corrigir).await,
                    ),
                    (Operacao::Inutilizar, Passo::Enviar) => match inutilizar.faixa() {
                        Ok(faixa) => Resposta::Pedido(
                            inutilizar_nfe(
                                fabrica,
                                preparo,
                                config,
                                faixa,
                                inutilizar.para_enviar(),
                            )
                            .await,
                        ),
                        Err(texto) => Resposta::Pedido(DesfechoNfe::Recusado { texto }),
                    },
                    (Operacao::Retomar, _) => {
                        Resposta::Retomada(retomar_doc(doc, fabrica, preparo, config).await)
                    }
                    (Operacao::Conferir, _) => Resposta::Consulta(
                        consultar_doc(doc, fabrica, preparo, config, chave).await,
                    ),
                    // so' o cancelamento tem resposta definitiva na consulta; a carta nao aparece nela
                    (Operacao::Corrigir | Operacao::Inutilizar, Passo::ConferirCancelamento) => {
                        Resposta::Consulta(
                            consultar_doc(doc, fabrica, preparo, config, chave).await,
                        )
                    }
                    (Operacao::Cancelar, Passo::ConferirCancelamento) => Resposta::Pedido(
                        conferir_cancelamento_doc(doc, fabrica, preparo, config, chave).await,
                    ),
                }
            };
            let resposta = match fora::rede(trabalho) {
                Some(tarefa) => tarefa.await.unwrap_or_else(|_| {
                    Resposta::Pedido(DesfechoNfe::SemResposta {
                        texto: "A chamada à SEFAZ foi interrompida.".into(),
                    })
                }),
                None => Resposta::Pedido(DesfechoNfe::Recusado {
                    texto: "Não foi possível preparar a chamada de rede.".into(),
                }),
            };
            this.update_in(cx, |this, window, cx| this.aplicar(resposta, window, cx))
                .ok();
        })
        .detach();
    }

    fn aplicar(&mut self, resposta: Resposta, window: &mut Window, cx: &mut Context<Self>) {
        let mexeu = match &resposta {
            Resposta::Pedido(d) => d.registrado(),
            Resposta::Consulta(c) => c.mexeu_na_pasta,
            Resposta::Retomada(r) => !r.linhas.is_empty(),
        };
        if matches!(&resposta, Resposta::Pedido(d) if d.registrado()) {
            self.senha_da_sessao = None;
        }
        self.fase = Fase::Desfecho(resposta);
        if mexeu {
            cx.emit(EventoDialogoNfe::MudouAPasta);
        }
        window.refresh();
        cx.notify();
    }

    // ---------------------------------------------------------------- senha

    fn pedir_senha(
        &mut self,
        passo: Passo,
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
                    (EventoSenha::Usada, Some((_, passo))) => {
                        let senha = dialogo.update(cx, |d, _| d.tomar());
                        this.abrir_dialogo(window, cx);
                        this.executar(passo, senha, window, cx);
                    }
                    _ => this.abrir_dialogo(window, cx),
                }
            },
        ));
        self.senha_pedida = Some((dialogo.clone(), passo));
        DialogoSenha::abrir(&dialogo, window, cx);
    }

    /// Abre o dialogo. O conteudo e' refeito a cada desenho a partir do estado.
    pub(super) fn abrir_dialogo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let ent = cx.entity();
        window.open_dialog(cx, move |dialog, _, cx| {
            let c = ent.read(cx);
            let trabalhando = c.ocupado();
            let preenchendo = matches!(c.fase, Fase::Preenchendo);
            let producao = c.producao();
            let titulo = c.operacao.titulo(
                c.alvo.as_ref().map_or("", |a| a.numero.as_str()),
                c.documento,
            );
            let operacao = c.operacao;
            let pode = c.pode_enviar();
            let motivo = if preenchendo && operacao != Operacao::Conferir {
                c.motivo_de_nao_enviar()
            } else {
                None
            };
            let corpo: AnyElement = match c.fase.clone() {
                Fase::Preenchendo => c.formulario(&ent, cx),
                Fase::Trabalhando(etapa) => h_flex()
                    .id("nfe-trabalhando")
                    .gap_2()
                    .items_center()
                    .p_4()
                    .child(Icon::new(IconName::LoaderCircle))
                    .child(div().text_sm().child(c.trabalhando_texto(etapa)))
                    .into_any_element(),
                Fase::Desfecho(r) => c.desfecho(&ent, &r, cx),
            };
            let rotulo = operacao
                .pedido()
                .map_or("Retomar as notas", |p| p.enviar(producao));
            let (e_enviar, e_fechou) = (ent.clone(), ent.clone());
            let (muted, aviso) = (
                cx.theme().muted_foreground,
                crate::telas::pecas::cor_da(Gravidade::Aviso, cx),
            );
            let rodape = DialogFooter::new()
                .child(
                    Button::new("nfe-dialogo-fechar")
                        .label(if preenchendo && operacao != Operacao::Conferir {
                            "Voltar"
                        } else {
                            "Fechar"
                        })
                        .outline()
                        .disabled(trabalhando)
                        .on_click(|_, window, cx| window.close_dialog(cx)),
                )
                .when(preenchendo && operacao != Operacao::Conferir, |f| {
                    f.child(
                        Button::new("nfe-dialogo-enviar")
                            .label(rotulo)
                            .when(producao, |b| botao_de_perigo(b, cx))
                            .when(!producao, |b| b.primary())
                            .disabled(!pode)
                            .on_click(move |_, window, cx| {
                                e_enviar.update(cx, |c, cx| c.enviar(window, cx));
                            }),
                    )
                });
            dialog
                .title(titulo)
                .w(px(600.))
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
                    v_flex()
                        .id("nfe-corpo-do-dialogo")
                        .gap_3()
                        .max_h(px(540.))
                        .overflow_y_scroll()
                        .child(corpo)
                        .children(motivo.map(|m| {
                            div()
                                .id("nfe-motivo-do-botao")
                                .text_xs()
                                .text_color(if pode { muted } else { aviso })
                                .child(m)
                        })),
                )
                .footer(rodape)
        });
    }
}
