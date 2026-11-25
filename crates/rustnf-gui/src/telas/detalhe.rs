// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O detalhe de uma nota, o mesmo nas telas Notas e Consultar: dados lidos do XML, chave de acesso com "Copiar",
//! DANFSe, "Mostrar na pasta", "Usar como modelo", Cancelar, o historico de eventos na Sefin e os eventos que a pessoa
//! pede (analise fiscal do cancelamento; confirmar e recusar a nota recebida). A tela dona dele escolhe a nota (da
//! pasta ou da consulta); tudo o que lê disco ou gera PDF roda fora da linha da tela.

use std::sync::Arc;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Disableable as _, Icon, IconName};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, AnyElement, AppContext as _, Context, Entity, EventEmitter, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, Styled as _, Window,
};
use rustnf_app::config::Config;
use rustnf_app::notas::{Registro, Situacao};

use super::cancelar::{AlvoDoCancelamento, Cancelamento};
use super::detalhe_blocos::{secoes, selo_do_ambiente};
use super::historico::Historico;
use super::pecas::{
    cartao, faixa, linha_de_resumo, selo_da_situacao, titulo_do_bloco, Negrito as _,
};
use crate::modelo::certificado::Gravidade;
use crate::modelo::consulta::{guarda, Guarda, NotaConsultada};
use crate::modelo::detalhe::{DetalheDaNota, LeituraDoXml};
use crate::modelo::evento::{AcoesDeEvento, TipoDeEvento};
use crate::modelo::lista::LinhaDeNota;
use crate::modelo::nota::DadosNota;
use crate::modelo::pedido::Contexto;
use crate::modelo::reaproveitar::dados_da_nota;
use crate::servicos::Servicos;

mod arquivos;

/// O que o detalhe pede a quem o mostra.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum EventoDetalhe {
    /// "Usar como modelo": os dados da nota viraram o formulario da Nova nota.
    UsarComoModelo(Box<DadosNota>),
    /// A nota consultada foi guardada na pasta de notas.
    Guardou,
}

/// De onde veio a nota mostrada.
#[derive(Debug, Clone)]
enum Origem {
    /// Da pasta de notas.
    Pasta,
    /// Da consulta a Sefin, com a nota inteira e o que se pode fazer com ela na pasta.
    Consulta {
        nota: Box<NotaConsultada>,
        guarda: Guarda,
    },
}

/// O que ha para mostrar.
#[derive(Debug, Clone)]
enum Conteudo {
    /// Nenhuma nota escolhida.
    Vazio,
    /// Lendo a nota da pasta.
    Carregando,
    /// Os dados da nota.
    Lida {
        detalhe: Box<DetalheDaNota>,
        origem: Origem,
    },
    /// A nota consta do indice, mas o XML dela nao esta na pasta ou nao e' legivel.
    SemXml {
        registro: Box<Registro>,
        ilegivel: bool,
    },
    /// Nao deu para ler a nota: a frase diz por que.
    Falha(String),
}

/// A situacao do DANFSe da nota mostrada.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Danfse {
    Parado,
    Gerando,
    Falhou(String),
    /// Gravado; o nome do arquivo (nunca o caminho inteiro).
    Gravado(String),
}

/// O que o detalhe oferece fazer com a nota mostrada.
#[derive(Debug, Clone, Copy)]
struct Oferta {
    situacao: Situacao,
    da_pasta: bool,
    pode_modelo: bool,
    pode_cancelar: bool,
    eventos: AcoesDeEvento,
    com_xml: bool,
}

/// O detalhe de uma nota.
pub(super) struct VisaoDetalhe {
    servicos: Arc<Servicos>,
    config: Config,
    ctx: Contexto,
    cancelamento: Entity<Cancelamento>,
    historico: Entity<Historico>,
    conteudo: Conteudo,
    danfse: Danfse,
    copiada: bool,
    /// O XML da nota mostrada foi copiado para a area de transferencia.
    xml_copiado: bool,
    guardando: bool,
    /// Falha de uma acao (mostrar na pasta, guardar...).
    erro: Option<String>,
    /// Muda a cada nota escolhida: uma leitura que chega atrasada nao sobrescreve a nota de agora.
    versao: u64,
}

impl EventEmitter<EventoDetalhe> for VisaoDetalhe {}

impl VisaoDetalhe {
    /// O detalhe, ainda sem nota. `cancelamento` e' o dialogo de cancelar, compartilhado com a outra tela.
    pub(super) fn new(
        servicos: Arc<Servicos>,
        cancelamento: Entity<Cancelamento>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let config = Config::default();
        let ctx = servicos.contexto_da_nota(&config);
        let historico = {
            let servicos = Arc::clone(&servicos);
            cx.new(|_| Historico::new(servicos))
        };
        Self {
            servicos,
            config,
            ctx,
            cancelamento,
            historico,
            conteudo: Conteudo::Vazio,
            danfse: Danfse::Parado,
            copiada: false,
            xml_copiado: false,
            guardando: false,
            erro: None,
            versao: 0,
        }
    }

    /// A configuracao que vale (a efetiva) chegou ou mudou.
    pub(super) fn definir_config(&mut self, config: Config, cx: &mut Context<Self>) {
        self.ctx = self.servicos.contexto_da_nota(&config);
        let para_o_historico = config.clone();
        self.historico
            .update(cx, |h, cx| h.definir_config(para_o_historico, cx));
        self.config = config;
        cx.notify();
    }

    fn zerar(&mut self) {
        self.versao += 1;
        self.danfse = Danfse::Parado;
        self.copiada = false;
        self.xml_copiado = false;
        self.guardando = false;
        self.erro = None;
    }

    /// Onde esta a consulta do historico da nota mostrada.
    #[cfg(test)]
    pub(super) fn fase_do_historico(&self, cx: &gpui_kit::App) -> super::historico::Fase {
        self.historico.read(cx).fase.clone()
    }

    /// O XML da nota mostrada foi copiado (o botao "Copiar o XML" mostra isso).
    #[cfg(test)]
    pub(super) fn xml_copiado(&self) -> bool {
        self.xml_copiado
    }

    /// Nao ha nota escolhida.
    pub(super) fn limpar(&mut self, cx: &mut Context<Self>) {
        self.zerar();
        self.conteudo = Conteudo::Vazio;
        self.historico.update(cx, |h, cx| h.definir_nota(None, cx));
        cx.notify();
    }

    /// Ha uma nota escolhida (mesmo que ainda carregando).
    pub(super) fn tem_nota(&self) -> bool {
        !matches!(self.conteudo, Conteudo::Vazio)
    }

    /// A chave da nota mostrada, se ha.
    pub(super) fn chave(&self) -> Option<&str> {
        match &self.conteudo {
            Conteudo::Lida { detalhe, .. } => Some(&detalhe.chave),
            Conteudo::SemXml { registro, .. } => Some(&registro.chave),
            _ => None,
        }
    }

    /// Mostra a nota da pasta de notas com esta chave (le o XML fora da tela).
    pub(super) fn mostrar_da_pasta(&mut self, chave: &str, cx: &mut Context<Self>) {
        self.zerar();
        self.conteudo = Conteudo::Carregando;
        self.historico
            .update(cx, |h, cx| h.definir_nota(Some(chave), cx));
        cx.notify();
        let versao = self.versao;
        let servicos = Arc::clone(&self.servicos);
        let config = self.config.clone();
        let chave = chave.to_string();
        cx.spawn(async move |this, cx| {
            let lida = cx
                .background_spawn(async move { servicos.ler_nota(&config, &chave) })
                .await;
            this.update(cx, |this, cx| {
                if this.versao != versao {
                    return;
                }
                this.conteudo = match lida {
                    Ok(nota) => match nota.leitura {
                        LeituraDoXml::Lida(detalhe) => Conteudo::Lida {
                            detalhe,
                            origem: Origem::Pasta,
                        },
                        LeituraDoXml::Ausente => Conteudo::SemXml {
                            registro: Box::new(nota.registro),
                            ilegivel: false,
                        },
                        LeituraDoXml::Ilegivel => Conteudo::SemXml {
                            registro: Box::new(nota.registro),
                            ilegivel: true,
                        },
                    },
                    Err(e) => Conteudo::Falha(e.texto()),
                };
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Mostra a nota que a Sefin devolveu na consulta; confere fora da tela se ela ja esta na pasta.
    pub(super) fn mostrar_consulta(&mut self, nota: NotaConsultada, cx: &mut Context<Self>) {
        self.zerar();
        let e_do_emitente = nota.detalhe.e_do_emitente(self.ctx.documento_do_emitente());
        let detalhe = Box::new(nota.detalhe.clone());
        let inicial = guarda(e_do_emitente, false);
        let da_nota = nota.detalhe.chave.clone();
        self.historico
            .update(cx, |h, cx| h.definir_nota(Some(&da_nota), cx));
        self.conteudo = Conteudo::Lida {
            detalhe,
            origem: Origem::Consulta {
                nota: Box::new(nota.clone()),
                guarda: inicial,
            },
        };
        cx.notify();
        if !e_do_emitente {
            return;
        }
        let versao = self.versao;
        let servicos = Arc::clone(&self.servicos);
        let config = self.config.clone();
        let chave = nota.detalhe.chave;
        cx.spawn(async move |this, cx| {
            let na_pasta = cx
                .background_spawn(async move { servicos.nota_esta_na_pasta(&config, &chave) })
                .await;
            this.update(cx, |this, cx| {
                if this.versao != versao {
                    return;
                }
                if let Conteudo::Lida {
                    origem: Origem::Consulta { guarda: g, .. },
                    ..
                } = &mut this.conteudo
                {
                    *g = guarda(true, na_pasta);
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// A Sefin registrou um evento para a nota mostrada. `situacao` e' a que a nota passou a ter (cancelada, em
    /// analise fiscal); `None` nos eventos que nao mudam a situacao (confirmar, recusar). Atualiza o selo e esquece o
    /// historico ja consultado, que ficou velho.
    pub(super) fn evento_registrado(
        &mut self,
        chave: &str,
        situacao: Option<Situacao>,
        cx: &mut Context<Self>,
    ) {
        let mesma = self.chave().is_some_and(|c| c == chave);
        if !mesma {
            return;
        }
        if let Some(situacao) = situacao {
            match &mut self.conteudo {
                Conteudo::Lida { detalhe, origem } => {
                    detalhe.situacao = situacao;
                    if let Origem::Consulta { nota, .. } = origem {
                        nota.detalhe.situacao = situacao;
                        nota.registro.situacao = situacao;
                        nota.situacao_conferida = true;
                    }
                }
                Conteudo::SemXml { registro, .. } => registro.situacao = situacao,
                _ => {}
            }
        }
        self.danfse = Danfse::Parado;
        self.historico
            .update(cx, |h, cx| h.definir_nota(Some(chave), cx));
        cx.notify();
    }

    fn guardar(&mut self, cx: &mut Context<Self>) {
        let Conteudo::Lida {
            origem:
                Origem::Consulta {
                    nota,
                    guarda: Guarda::Guardar,
                },
            ..
        } = &self.conteudo
        else {
            return;
        };
        if self.guardando {
            return;
        }
        let nota = nota.clone();
        self.guardando = true;
        self.erro = None;
        cx.notify();
        let versao = self.versao;
        let servicos = Arc::clone(&self.servicos);
        let config = self.config.clone();
        cx.spawn(async move |this, cx| {
            let r = cx
                .background_spawn(async move { servicos.guardar_consultada(&config, &nota) })
                .await;
            this.update(cx, |this, cx| {
                if this.versao != versao {
                    return;
                }
                this.guardando = false;
                match r {
                    Ok(()) => {
                        if let Conteudo::Lida {
                            origem: Origem::Consulta { guarda: g, .. },
                            ..
                        } = &mut this.conteudo
                        {
                            *g = Guarda::JaEstaNaPasta;
                        }
                        cx.emit(EventoDetalhe::Guardou);
                    }
                    Err(e) => this.erro = Some(e.texto()),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn usar_como_modelo(&mut self, cx: &mut Context<Self>) {
        if let Conteudo::Lida { detalhe, .. } = &self.conteudo {
            let dados = dados_da_nota(detalhe, &self.ctx);
            cx.emit(EventoDetalhe::UsarComoModelo(Box::new(dados)));
        }
    }

    /// Abre o dialogo do evento que a pessoa pediu (cancelar, analise fiscal, confirmar ou recusar) para a nota
    /// mostrada.
    fn pedir(&mut self, tipo: TipoDeEvento, window: &mut Window, cx: &mut Context<Self>) {
        let (chave, numero, tomador, emitente, valor) = match &self.conteudo {
            Conteudo::Lida { detalhe, .. } => (
                detalhe.chave.clone(),
                detalhe.numero.clone(),
                detalhe.tomador.clone(),
                detalhe.emitente.clone(),
                detalhe.valor_servico.clone(),
            ),
            Conteudo::SemXml { registro, .. } => {
                let l = LinhaDeNota::do_registro(registro);
                (l.chave, l.numero, l.tomador, String::new(), l.valor)
            }
            _ => return,
        };
        let alvo = AlvoDoCancelamento {
            tipo,
            chave,
            numero,
            tomador,
            emitente,
            valor,
        };
        self.cancelamento
            .update(cx, |c, cx| c.iniciar(alvo, window, cx));
    }

    // ---------------------------------------------------------------- desenho

    fn acoes(&self, oferta: Oferta, cx: &mut Context<Self>) -> AnyElement {
        let Oferta {
            situacao,
            da_pasta,
            pode_modelo,
            pode_cancelar,
            eventos,
            com_xml,
        } = oferta;
        let gerando = self.danfse == Danfse::Gerando;
        let falhou = matches!(self.danfse, Danfse::Falhou(_));
        h_flex()
            .id("acoes-da-nota")
            .gap_2()
            .flex_wrap()
            .when(com_xml, |d| {
                d.child(
                    Button::new("abrir-danfse")
                        .label(if falhou {
                            "Tentar de novo o DANFSe"
                        } else {
                            "Abrir o DANFSe"
                        })
                        .icon(IconName::FileText)
                        .primary()
                        .loading(gerando)
                        .disabled(gerando)
                        .on_click(cx.listener(|this, _, window, cx| this.abrir_danfse(window, cx))),
                )
            })
            .when(da_pasta, |d| {
                d.child(
                    Button::new("mostrar-na-pasta")
                        .label("Mostrar na pasta")
                        .icon(IconName::FolderOpen)
                        .outline()
                        .on_click(cx.listener(|this, _, _, cx| this.mostrar_na_pasta(cx))),
                )
            })
            .when(com_xml, |d| {
                let copiado = self.xml_copiado;
                d.child(
                    Button::new("copiar-xml")
                        .label(if copiado {
                            "XML copiado"
                        } else {
                            "Copiar o XML"
                        })
                        .icon(if copiado {
                            IconName::Check
                        } else {
                            IconName::Copy
                        })
                        .outline()
                        .on_click(cx.listener(|this, _, _, cx| this.copiar_xml(cx))),
                )
            })
            .when(pode_modelo, |d| {
                d.child(
                    Button::new("usar-como-modelo")
                        .label("Usar como modelo")
                        .icon(IconName::Copy)
                        .outline()
                        .on_click(cx.listener(|this, _, _, cx| this.usar_como_modelo(cx))),
                )
            })
            .when(pode_cancelar && situacao == Situacao::Autorizada, |d| {
                d.child(
                    Button::new("cancelar-nota")
                        .label("Cancelar a nota")
                        .icon(IconName::Ban)
                        .outline()
                        .danger()
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.pedir(TipoDeEvento::Cancelar, window, cx);
                        })),
                )
            })
            .when(eventos.analise_fiscal, |d| {
                d.child(
                    Button::new("pedir-analise-fiscal")
                        .label(TipoDeEvento::AnaliseFiscal.rotulo_da_acao())
                        .icon(IconName::Eye)
                        .outline()
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.pedir(TipoDeEvento::AnaliseFiscal, window, cx);
                        })),
                )
            })
            .when(eventos.confirmar, |d| {
                d.child(
                    Button::new("confirmar-nota-recebida")
                        .label(TipoDeEvento::Confirmar.rotulo_da_acao())
                        .icon(IconName::CircleCheck)
                        .primary()
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.pedir(TipoDeEvento::Confirmar, window, cx);
                        })),
                )
            })
            .when(eventos.recusar, |d| {
                d.child(
                    Button::new("recusar-nota-recebida")
                        .label(TipoDeEvento::Recusar.rotulo_da_acao())
                        .icon(IconName::Ban)
                        .outline()
                        .danger()
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.pedir(TipoDeEvento::Recusar, window, cx);
                        })),
                )
            })
            .into_any_element()
    }

    fn corpo_da_nota(
        &mut self,
        d: &DetalheDaNota,
        origem: &Origem,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let muted = cx.theme().muted_foreground;
        let e_do_emitente = d.e_do_emitente(self.ctx.documento_do_emitente());
        let (da_pasta, avisos) = match origem {
            Origem::Pasta => (true, Vec::new()),
            Origem::Consulta { nota, guarda: g } => {
                let mut v: Vec<AnyElement> = Vec::new();
                if !nota.situacao_conferida {
                    v.push(
                        faixa(
                            Gravidade::Aviso,
                            "Não foi possível conferir se a nota foi cancelada. O selo mostra Autorizada porque o XML da nota não registra cancelamento.",
                            cx,
                        )
                        .id("situacao-nao-conferida")
                        .into_any_element(),
                    );
                }
                v.push(match g {
                    Guarda::JaEstaNaPasta => faixa(
                        Gravidade::Ok,
                        "Esta nota já está na pasta de notas.",
                        cx,
                    )
                    .id("ja-esta-na-pasta")
                    .into_any_element(),
                    Guarda::OutroEmitente => faixa(
                        Gravidade::Aviso,
                        "Esta nota é de outro emitente (não é o das Configurações), então não é guardada na pasta de notas.",
                        cx,
                    )
                    .id("outro-emitente")
                    .into_any_element(),
                    Guarda::Guardar => v_flex()
                        .id("guardar-na-pasta-bloco")
                        .gap_2()
                        .child(faixa(
                            Gravidade::Neutro,
                            "Esta nota ainda não está na pasta de notas.",
                            cx,
                        ))
                        .child(
                            h_flex().child(
                                Button::new("guardar-na-pasta")
                                    .label("Guardar na pasta de notas")
                                    .icon(IconName::FolderOpen)
                                    .outline()
                                    .loading(self.guardando)
                                    .disabled(self.guardando)
                                    .on_click(cx.listener(|this, _, _, cx| this.guardar(cx))),
                            ),
                        )
                        .into_any_element(),
                });
                (*g == Guarda::JaEstaNaPasta, v)
            }
        };
        let pode_modelo = da_pasta || e_do_emitente;
        let pode_cancelar = da_pasta || e_do_emitente;
        // o tomador e' o documento do certificado configurado (o do [emitente]); so' quem recebeu a nota a manifesta
        let eventos = AcoesDeEvento::da_nota(
            d.situacao,
            e_do_emitente,
            d.e_tomador(self.ctx.documento_do_emitente()),
        );
        let titulo = if d.numero.is_empty() {
            "NFS-e".to_string()
        } else {
            format!("NFS-e nº {}", d.numero)
        };
        let cabeca = h_flex()
            .gap_3()
            .items_center()
            .flex_wrap()
            .child(div().text_xl().font_semibold().child(titulo))
            .child(selo_da_situacao(d.situacao, cx).id("selo-da-situacao"))
            .children(selo_do_ambiente(d.ambiente));
        let mut coluna = v_flex()
            .gap_4()
            .child(cabeca)
            .child(self.acoes(
                Oferta {
                    situacao: d.situacao,
                    da_pasta,
                    pode_modelo,
                    pode_cancelar,
                    eventos,
                    com_xml: true,
                },
                cx,
            ))
            .children(self.aviso_do_danfse(cx))
            .children(self.erro.clone().map(|e| {
                faixa(Gravidade::Erro, e, cx)
                    .id("erro-do-detalhe")
                    .into_any_element()
            }))
            .children(avisos)
            .child(self.bloco_da_chave(&d.chave, cx));
        for s in secoes(d, cx) {
            coluna = coluna.child(s);
        }
        coluna = coluna.child(self.historico.clone());
        coluna
            .child(div().text_xs().text_color(muted).child(
                "Os dados vêm do XML da nota. O DANFSe é gerado por este programa a partir dele.",
            ))
            .into_any_element()
    }

    fn corpo_sem_xml(
        &mut self,
        registro: &Registro,
        ilegivel: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let l = LinhaDeNota::do_registro(registro);
        let texto = if ilegivel {
            "O arquivo XML desta nota está na pasta, mas não é uma NFS-e legível. Os dados abaixo são os do índice."
        } else {
            "O arquivo XML desta nota não está na pasta de notas. Os dados abaixo são os do índice; consulte a nota pela chave para guardá-la de novo."
        };
        let cancelavel = registro.situacao == Situacao::Autorizada;
        v_flex()
            .id("nota-sem-xml")
            .gap_4()
            .child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .child(
                        div()
                            .text_xl()
                            .font_semibold()
                            .child(format!("NFS-e nº {}", l.numero)),
                    )
                    .child(selo_da_situacao(registro.situacao, cx)),
            )
            .child(faixa(Gravidade::Aviso, texto, cx))
            .child(self.acoes(
                Oferta {
                    situacao: registro.situacao,
                    da_pasta: true,
                    pode_modelo: false,
                    pode_cancelar: cancelavel,
                    eventos: AcoesDeEvento::da_nota(registro.situacao, true, false),
                    com_xml: false,
                },
                cx,
            ))
            .child(self.bloco_da_chave(&l.chave, cx))
            .child(self.historico.clone())
            .child(
                cartao(cx)
                    .child(titulo_do_bloco("Do índice", cx))
                    .child(linha_de_resumo("Emissão", l.data, cx))
                    .child(linha_de_resumo("DPS", l.dps, cx))
                    .child(linha_de_resumo("Tomador", l.tomador, cx))
                    .child(linha_de_resumo("Valor", format!("R$ {}", l.valor), cx)),
            )
            .into_any_element()
    }
}

impl Render for VisaoDetalhe {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        let conteudo = self.conteudo.clone();
        let corpo: AnyElement = match conteudo {
            Conteudo::Vazio => div()
                .id("detalhe-vazio")
                .p_8()
                .text_sm()
                .text_color(muted)
                .child("Escolha uma nota para ver o detalhe.")
                .into_any_element(),
            Conteudo::Carregando => h_flex()
                .id("detalhe-carregando")
                .p_8()
                .gap_2()
                .text_sm()
                .text_color(muted)
                .child(Icon::new(IconName::LoaderCircle))
                .child("Lendo a nota...")
                .into_any_element(),
            Conteudo::Falha(t) => faixa(Gravidade::Erro, t, cx)
                .id("detalhe-falhou")
                .into_any_element(),
            Conteudo::SemXml { registro, ilegivel } => self.corpo_sem_xml(&registro, ilegivel, cx),
            Conteudo::Lida { detalhe, origem } => self.corpo_da_nota(&detalhe, &origem, cx),
        };
        v_flex()
            .id("detalhe-da-nota")
            .w_full()
            .max_w(px(760.))
            .mx_auto()
            .gap_4()
            .child(corpo)
    }
}
