// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O dialogo das NF-e recebidas: buscar notas novas, baixar a nota completa e responder a uma nota (ciencia, confirmacao,
//! desconhecimento, operacao nao realizada). E' o mesmo desenho do dialogo da NF-e emitida (ambiente em destaque, desfecho
//! com o que fazer, sem clique duplo, senha pedida so' se nao esta no cofre, marcacao explicita em producao), com o que e'
//! das recebidas: as quatro respostas em palavras de quem compra. A resposta e a ciencia sao EVENTOS FISCAIS: nada sai da
//! maquina sem o botao, e em producao nem com ele sem a marcacao. A espera entre buscas e' da biblioteca: este arquivo
//! nunca a contorna.

use std::sync::Arc;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::dialog::DialogFooter;
use gpui_kit::component::input::{InputEvent, TextareaState};
use gpui_kit::component::{
    h_flex, v_flex, ActiveTheme as _, Disableable as _, Icon, IconName, WindowExt as _,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, AnyElement, App, AppContext as _, Context, Entity, EventEmitter,
    InteractiveElement as _, IntoElement, ParentElement as _, ScrollHandle,
    StatefulInteractiveElement as _, Styled as _, Subscription, Window,
};
use rustnf_app::config::Config;
use rustnf_app::segredo::Segredo;

use super::pecas::botao_de_perigo;
use super::rolagem::revelar;
use super::senha::{classificar, Credencial, DialogoSenha, EventoSenha};
use crate::fora;
use crate::modelo::ambiente::Ambiente;
use crate::modelo::certificado::Gravidade;
use crate::modelo::nfe_config::ambiente_da_nfe;
use crate::modelo::nfe_eventos::DesfechoNfe;
use crate::modelo::nfe_recebidas::RespostaDada;
use crate::modelo::nfe_recebidas_manifestar::{
    AlvoDaResposta, FormularioDaResposta, Resposta, MARCACAO_DE_PRODUCAO,
};
use crate::modelo::nfe_recebidas_resultados::{DesfechoBaixa, DesfechoBusca};
use crate::servicos::{baixar_nota, buscar_notas, responder_nota, Servicos};

mod desenho;

/// O que o dialogo faz.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OperacaoRecebida {
    /// Buscar notas novas no Ambiente Nacional (roda sozinho ao abrir).
    Buscar,
    /// Baixar a nota completa (com a ciencia, se a empresa ainda nao respondeu).
    Baixar,
    /// Responder a nota.
    Responder,
}

/// O que dizer enquanto roda.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Etapa {
    /// Abrindo o certificado.
    Certificado,
    /// Falando com o Ambiente Nacional.
    Enviando,
}

/// O que o dialogo mostra depois.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Resultado {
    /// A busca.
    Busca(DesfechoBusca),
    /// A baixa.
    Baixa(DesfechoBaixa),
    /// A resposta a nota, com a que foi pedida.
    Resposta(DesfechoNfe, Resposta),
}

/// Onde o dialogo esta.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Fase {
    /// Preenchendo.
    Preenchendo,
    /// Rodando fora da tela.
    Trabalhando(Etapa),
    /// Terminou (ou ficou em aberto).
    Desfecho(Resultado),
}

/// O que o dialogo avisa a tela.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum EventoDialogoRecebidas {
    /// A pasta de notas pode ter mudado: a lista e o detalhe leem de novo.
    MudouAPasta,
    /// Uma busca terminou: a tela mostra o resultado ate a proxima acao.
    Buscou(DesfechoBusca),
}

/// O dialogo das NF-e recebidas.
pub(super) struct DialogoRecebidas {
    servicos: Arc<Servicos>,
    config: Config,
    pub(super) operacao: OperacaoRecebida,
    pub(super) alvo: Option<AlvoDaResposta>,
    pub(super) formulario: FormularioDaResposta,
    /// Em producao: marcou "Entendo..." na baixa que registra a ciencia.
    pub(super) baixa_entendida: bool,
    tocada: bool,
    pub(super) fase: Fase,
    pub(super) texto: Entity<TextareaState>,
    /// A rolagem do corpo do dialogo: o campo da justificativa pede para aparecer quando ganha o foco.
    rolagem: ScrollHandle,
    senha_da_sessao: Option<Segredo>,
    senha_pedida: Option<Entity<DialogoSenha>>,
    sub_senha: Option<Subscription>,
    _subs: Vec<Subscription>,
}

impl EventEmitter<EventoDialogoRecebidas> for DialogoRecebidas {}

impl DialogoRecebidas {
    pub(super) fn new(
        servicos: Arc<Servicos>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let texto = cx.new(|cx| {
            TextareaState::new(window, cx)
                .placeholder("Escreva aqui (fica registrado no documento fiscal)")
                .auto_grow(3, 6)
        });
        let sub = cx.subscribe_in(&texto, window, |this, e, ev: &InputEvent, window, cx| {
            if matches!(ev, InputEvent::Change) {
                this.formulario.justificativa = e.read(cx).value().to_string();
                this.tocada = true;
                window.refresh();
                cx.notify();
            }
        });
        Self {
            servicos,
            config: Config::default(),
            operacao: OperacaoRecebida::Buscar,
            alvo: None,
            formulario: FormularioDaResposta::default(),
            baixa_entendida: false,
            tocada: false,
            fase: Fase::Preenchendo,
            texto,
            rolagem: ScrollHandle::new(),
            senha_da_sessao: None,
            senha_pedida: None,
            sub_senha: None,
            _subs: vec![sub],
        }
    }

    /// A configuracao que vale (a efetiva) chegou ou mudou.
    pub(super) fn definir_config(&mut self, config: Config, cx: &mut Context<Self>) {
        self.config = config;
        cx.notify();
    }

    pub(super) fn producao(&self) -> bool {
        ambiente_da_nfe(&self.config.nfe) == Ambiente::Producao
    }

    pub(super) fn ambiente(&self) -> Ambiente {
        ambiente_da_nfe(&self.config.nfe)
    }

    pub(super) fn demonstracao(&self) -> bool {
        self.servicos.demonstracao_nfe()
    }

    fn zerar(
        &mut self,
        op: OperacaoRecebida,
        alvo: Option<AlvoDaResposta>,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.operacao = op;
        self.formulario = FormularioDaResposta::default();
        // a unica resposta possivel sugere-se sozinha; com varias, a pessoa escolhe
        self.alvo = alvo;
        self.baixa_entendida = false;
        self.tocada = false;
        self.fase = Fase::Preenchendo;
        self.senha_da_sessao = None;
        self.texto.update(cx, |e, cx| e.set_value("", window, cx));
    }

    fn ocupado(&self) -> bool {
        matches!(self.fase, Fase::Trabalhando(_))
    }

    /// A baixa registra a ciencia (evento fiscal): a empresa ainda nao respondeu sobre a nota.
    pub(super) fn baixa_registra_ciencia(&self) -> bool {
        self.alvo
            .as_ref()
            .is_some_and(|a| a.resposta_dada == RespostaDada::Nenhuma)
    }

    /// Abre o dialogo de `op` (para a nota, se for o caso). A busca roda sozinha.
    pub(super) fn iniciar(
        &mut self,
        op: OperacaoRecebida,
        alvo: Option<AlvoDaResposta>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.ocupado() {
            return;
        }
        self.zerar(op, alvo, window, cx);
        self.abrir_dialogo(window, cx);
        if op == OperacaoRecebida::Buscar {
            self.executar(None, window, cx);
        }
    }

    /// Escolhe uma das respostas (clique na opcao).
    pub(super) fn escolher(&mut self, r: Resposta, window: &mut Window, cx: &mut Context<Self>) {
        if self.alvo.as_ref().is_some_and(|a| a.recusada(r).is_some()) {
            return;
        }
        self.formulario.escolha = Some(r);
        if r.pede_justificativa() {
            // o campo da justificativa aparece abaixo das opcoes: o foco o traz para a vista (o campo pede a rolagem
            // ao ganhar o foco e o `revelar` do corpo atende) e a pessoa ja pode digitar
            self.texto.update(cx, |e, cx| e.focus(window, cx));
        }
        cx.notify();
    }

    /// O pedido pode ser enviado agora.
    pub(super) fn pode_enviar(&self) -> bool {
        matches!(self.fase, Fase::Preenchendo) && self.motivo_de_nao_enviar().is_none()
    }

    pub(super) fn motivo_de_nao_enviar(&self) -> Option<String> {
        let producao = self.producao();
        match (self.operacao, self.alvo.as_ref()) {
            (OperacaoRecebida::Buscar, _) => Some("A busca roda sozinha.".into()),
            (OperacaoRecebida::Responder, Some(a)) => {
                self.formulario.motivo_de_nao_enviar(a, producao)
            }
            (OperacaoRecebida::Baixar, Some(_)) => {
                (producao && self.baixa_registra_ciencia() && !self.baixa_entendida).then(|| {
                    "Em produção, marque que você entende que a ciência é registrada na SEFAZ."
                        .to_string()
                })
            }
            _ => Some("Escolha uma nota.".into()),
        }
    }

    /// Envia o pedido (o botao do dialogo).
    pub(super) fn enviar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.pode_enviar() {
            self.executar(None, window, cx);
        }
    }

    fn tentar_de_novo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.fase_em_aberto() {
            self.executar(None, window, cx);
        }
    }

    /// O desfecho deixa a pessoa tentar de novo (sem resposta, ou confirmado como nao registrado).
    pub(super) fn fase_em_aberto(&self) -> bool {
        match &self.fase {
            Fase::Desfecho(Resultado::Resposta(d, _)) => d.deixa_em_aberto(),
            Fase::Desfecho(Resultado::Baixa(DesfechoBaixa::SemResposta { .. })) => true,
            Fase::Desfecho(Resultado::Busca(DesfechoBusca::SemResposta { .. })) => true,
            _ => false,
        }
    }

    fn voltar(&mut self, cx: &mut Context<Self>) {
        self.fase = Fase::Preenchendo;
        cx.notify();
    }

    fn executar(&mut self, digitada: Option<Segredo>, window: &mut Window, cx: &mut Context<Self>) {
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
        let op = self.operacao;
        let chave = self
            .alvo
            .as_ref()
            .map(|a| a.chave.clone())
            .unwrap_or_default();
        let ciencia = self.baixa_registra_ciencia();
        let (resposta, justificativa) = (self.formulario.escolha, self.formulario.para_enviar());
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
                .background_spawn(async move { s2.preparar_recebidas(&c2, usada.as_ref()) })
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
                    this.update_in(cx, |this, window, cx| {
                        let r = Resultado::Resposta(
                            DesfechoNfe::Recusado { texto },
                            resposta.unwrap_or(Resposta::Ciencia),
                        );
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
            let trabalho = async move {
                match (op, resposta) {
                    (OperacaoRecebida::Buscar, _) => {
                        Resultado::Busca(buscar_notas(fabrica, preparo, config).await)
                    }
                    (OperacaoRecebida::Baixar, _) => Resultado::Baixa(
                        baixar_nota(fabrica, preparo, config, chave, ciencia).await,
                    ),
                    (OperacaoRecebida::Responder, Some(r)) => Resultado::Resposta(
                        responder_nota(fabrica, preparo, config, chave, r, justificativa).await,
                        r,
                    ),
                    (OperacaoRecebida::Responder, None) => Resultado::Resposta(
                        DesfechoNfe::Recusado {
                            texto: "Escolha uma das respostas.".into(),
                        },
                        Resposta::Ciencia,
                    ),
                }
            };
            let resultado = match fora::rede(trabalho) {
                Some(tarefa) => tarefa.await.unwrap_or_else(|_| semresposta(op, resposta)),
                None => Resultado::Resposta(
                    DesfechoNfe::Recusado {
                        texto: "Não foi possível preparar a chamada de rede.".into(),
                    },
                    resposta.unwrap_or(Resposta::Ciencia),
                ),
            };
            this.update_in(cx, |this, window, cx| this.aplicar(resultado, window, cx))
                .ok();
        })
        .detach();
    }

    fn aplicar(&mut self, resultado: Resultado, window: &mut Window, cx: &mut Context<Self>) {
        let mexeu = match &resultado {
            // a busca sempre mexe no cursor da pasta (a ultima busca e a espera), mesmo sem nota nova
            Resultado::Busca(_) => true,
            Resultado::Baixa(d) => d.mexeu_na_pasta(),
            Resultado::Resposta(d, _) => d.registrado(),
        };
        if matches!(&resultado, Resultado::Resposta(d, _) if d.registrado()) {
            self.senha_da_sessao = None;
        }
        if let Resultado::Busca(d) = &resultado {
            cx.emit(EventoDialogoRecebidas::Buscou(d.clone()));
        }
        self.fase = Fase::Desfecho(resultado);
        if mexeu {
            cx.emit(EventoDialogoRecebidas::MudouAPasta);
        }
        window.refresh();
        cx.notify();
    }

    // ---------------------------------------------------------------- senha

    fn pedir_senha(&mut self, erro: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
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
                    (EventoSenha::Usada, Some(_)) => {
                        let senha = dialogo.update(cx, |d, _| d.tomar());
                        this.abrir_dialogo(window, cx);
                        this.executar(senha, window, cx);
                    }
                    _ => this.abrir_dialogo(window, cx),
                }
            },
        ));
        self.senha_pedida = Some(dialogo.clone());
        DialogoSenha::abrir(&dialogo, window, cx);
    }

    /// Abre o dialogo. O conteudo e' refeito a cada desenho a partir do estado.
    pub(super) fn abrir_dialogo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let ent = cx.entity();
        window.open_dialog(cx, move |dialog, window, cx| {
            let c = ent.read(cx);
            let rolagem = c.rolagem.clone();
            // o dialogo nunca passa da janela: titulo, rodape e margens ficam de fora do corpo
            let cabe = (window.viewport_size().height - px(260.)).max(px(240.));
            let trabalhando = c.ocupado();
            let preenchendo = matches!(c.fase, Fase::Preenchendo);
            let producao = c.producao();
            let operacao = c.operacao;
            let titulo = c.titulo();
            let pode = c.pode_enviar();
            // o erro da justificativa ja aparece embaixo do campo: o rodape nao o repete
            let erro_do_campo = c.formulario.erro_da_justificativa(c.tocada);
            let motivo = if preenchendo && operacao != OperacaoRecebida::Buscar {
                c.motivo_de_nao_enviar()
                    .filter(|m| erro_do_campo.as_deref() != Some(m.as_str()))
            } else {
                None
            };
            let corpo: AnyElement = match c.fase.clone() {
                Fase::Preenchendo => c.formulario_do_dialogo(&ent, cx),
                Fase::Trabalhando(etapa) => h_flex()
                    .id("recebidas-trabalhando")
                    .gap_2()
                    .items_center()
                    .p_4()
                    .child(Icon::new(IconName::LoaderCircle))
                    .child(div().text_sm().child(c.trabalhando_texto(etapa)))
                    .into_any_element(),
                Fase::Desfecho(r) => c.desfecho(&ent, &r, cx),
            };
            let rotulo = c.rotulo_do_botao(producao);
            let (e_enviar, e_fechou) = (ent.clone(), ent.clone());
            let (muted, aviso) = (
                cx.theme().muted_foreground,
                crate::telas::pecas::cor_da(Gravidade::Aviso, cx),
            );
            let rodape = DialogFooter::new()
                .child(
                    Button::new("recebidas-dialogo-fechar")
                        .label(if preenchendo && operacao != OperacaoRecebida::Buscar {
                            "Voltar"
                        } else {
                            "Fechar"
                        })
                        .outline()
                        .disabled(trabalhando)
                        .on_click(|_, window, cx| window.close_dialog(cx)),
                )
                .when(preenchendo && operacao != OperacaoRecebida::Buscar, |f| {
                    f.child(
                        Button::new("recebidas-dialogo-enviar")
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
                .w(px(640.))
                .keyboard(!trabalhando)
                .overlay_closable(false)
                .close_button(!trabalhando)
                .on_close(move |_, _, cx| {
                    e_fechou.update(cx, |c, cx| {
                        c.senha_da_sessao = None;
                        cx.notify();
                    });
                })
                .child(revelar(
                    &rolagem,
                    v_flex()
                        .id("recebidas-corpo-do-dialogo")
                        .gap_3()
                        // as quatro respostas, o resumo e a marcacao cabem sem rolar numa janela alta; os outros
                        // dialogos sao menores. Em janela baixa o corpo rola e o campo focado aparece
                        .max_h(
                            px(if operacao == OperacaoRecebida::Responder {
                                760.
                            } else {
                                640.
                            })
                            .min(cabe),
                        )
                        .overflow_y_scroll()
                        .track_scroll(&rolagem)
                        .child(corpo)
                        .children(motivo.map(|m| {
                            div()
                                .id("recebidas-motivo-do-botao")
                                .text_xs()
                                .text_color(if pode { muted } else { aviso })
                                .child(m)
                        })),
                ))
                .footer(rodape)
        });
    }
}

/// O que o dialogo diz quando a chamada foi interrompida no meio.
fn semresposta(op: OperacaoRecebida, resposta: Option<Resposta>) -> Resultado {
    let texto = "A chamada ao Ambiente Nacional foi interrompida.".to_string();
    match op {
        OperacaoRecebida::Buscar => Resultado::Busca(DesfechoBusca::SemResposta { texto }),
        OperacaoRecebida::Baixar => Resultado::Baixa(DesfechoBaixa::SemResposta { texto }),
        OperacaoRecebida::Responder => Resultado::Resposta(
            DesfechoNfe::SemResposta { texto },
            resposta.unwrap_or(Resposta::Ciencia),
        ),
    }
}

/// A marcacao que o dialogo mostra em producao, por operacao.
pub(super) fn marcacao_da_operacao(op: OperacaoRecebida) -> &'static str {
    match op {
        OperacaoRecebida::Baixar => {
            "Entendo que a ciência é registrada na SEFAZ, com efeito fiscal"
        }
        _ => MARCACAO_DE_PRODUCAO,
    }
}
