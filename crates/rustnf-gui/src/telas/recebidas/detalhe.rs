// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O detalhe de uma NF-e recebida. Com a nota completa, e' o MESMO detalhe da NF-e emitida (as secoes e o cartao da chave
//! sao as de `nfe::detalhe`), com o DANFE; com so' o resumo, mostra o que o resumo traz e "Baixar a nota completa". As
//! acoes: responder a nota, baixar a nota completa, abrir o DANFE, mostrar na pasta e copiar a chave. O PDF e a leitura do
//! disco rodam fora da linha da tela.

use std::sync::Arc;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Disableable as _, Icon, IconName};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, rgb, AnyElement, App, AppContext as _, ClipboardItem, Context,
    InteractiveElement as _, IntoElement, ParentElement as _, Styled as _, Window,
};

use super::TelaRecebidas;
use crate::modelo::ambiente::Ambiente;
use crate::modelo::certificado::Gravidade;
use crate::modelo::nfe_config::selo_do_ambiente_da_nfe;
use crate::modelo::nfe_detalhe::LeituraNfe;
use crate::modelo::nfe_recebidas::{LinhaRecebida, RespostaDada, SituacaoDaNota};
use crate::modelo::nfe_recebidas_manifestar::AlvoDaResposta;
use crate::servicos::{Destino, RecebidaDaPasta};
use crate::telas::nfe::detalhe::{
    botao_de_copiar_a_chave, cartao_da_chave, pedir_ao_sistema, secoes_da_nota,
};
use crate::telas::pecas::{
    cartao, cor_da, faixa, linha_de_resumo, selo_de, titulo_do_bloco, Negrito as _,
};
use crate::telas::recebidas_dialogo::OperacaoRecebida;

/// O que o detalhe mostra.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Conteudo {
    /// Nenhuma nota escolhida.
    Vazio,
    /// Lendo a nota.
    Carregando,
    /// Nao deu para ler: a frase.
    Falha(String),
    /// Leu.
    Lida(Box<RecebidaDaPasta>),
}

/// O DANFE da nota escolhida.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EstadoDoDanfe {
    /// Nada pedido.
    Parado,
    /// Gerando o PDF.
    Gerando,
    /// Nao deu para gerar (a frase esta em `erro`).
    Falhou,
}

impl TelaRecebidas {
    /// Le a nota `chave` fora da tela e a mostra.
    pub(super) fn ler_o_detalhe(&mut self, chave: &str, cx: &mut Context<Self>) {
        self.versao_do_detalhe += 1;
        let versao = self.versao_do_detalhe;
        self.detalhe = Conteudo::Carregando;
        self.erro = None;
        self.danfe = EstadoDoDanfe::Parado;
        self.chave_copiada = false;
        let (servicos, config, chave) = (
            Arc::clone(&self.servicos),
            self.config.clone(),
            chave.to_string(),
        );
        cx.spawn(async move |this, cx| {
            let r = cx
                .background_spawn(async move { servicos.ler_recebida(&config, &chave) })
                .await;
            this.update(cx, |this, cx| {
                // a pessoa escolheu outra nota enquanto esta era lida
                if this.versao_do_detalhe != versao {
                    return;
                }
                this.detalhe = match r {
                    Ok(n) => Conteudo::Lida(Box::new(n)),
                    Err(e) => Conteudo::Falha(e.texto()),
                };
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    /// Le de novo a nota aberta (uma resposta ou uma baixa a mudou).
    pub(super) fn reler_o_detalhe(&mut self, cx: &mut Context<Self>) {
        if let Some(chave) = self.escolhida.clone() {
            let danfe = self.danfe;
            self.ler_o_detalhe(&chave, cx);
            self.danfe = danfe;
        }
    }

    fn alvo(n: &RecebidaDaPasta) -> AlvoDaResposta {
        AlvoDaResposta::da_recebida(&n.registro)
    }

    fn pedir(&mut self, op: OperacaoRecebida, window: &mut Window, cx: &mut Context<Self>) {
        let Conteudo::Lida(n) = &self.detalhe else {
            return;
        };
        let alvo = Self::alvo(n);
        self.dialogo
            .update(cx, |d, cx| d.iniciar(op, Some(alvo), window, cx));
    }

    fn copiar_chave(&mut self, cx: &mut Context<Self>) {
        if let Conteudo::Lida(n) = &self.detalhe {
            cx.write_to_clipboard(ClipboardItem::new_string(n.registro.chave.clone()));
            self.chave_copiada = true;
            cx.notify();
        }
    }

    pub(super) fn abrir_danfe(&mut self, cx: &mut Context<Self>) {
        let Conteudo::Lida(n) = &self.detalhe else {
            return;
        };
        if self.danfe == EstadoDoDanfe::Gerando {
            return;
        }
        let chave = n.registro.chave.clone();
        self.danfe = EstadoDoDanfe::Gerando;
        self.erro = None;
        cx.notify();
        let versao = self.versao_do_detalhe;
        let (servicos, config) = (Arc::clone(&self.servicos), self.config.clone());
        cx.spawn(async move |this, cx| {
            let r = cx
                .background_spawn({
                    let servicos = Arc::clone(&servicos);
                    async move { servicos.danfe_da_recebida(&config, &chave) }
                })
                .await;
            this.update(cx, |this, cx| {
                if this.versao_do_detalhe != versao {
                    return;
                }
                match r {
                    Ok(caminho) => {
                        this.danfe = EstadoDoDanfe::Parado;
                        pedir_ao_sistema(&this.servicos, Destino::Abrir, &caminho, cx);
                    }
                    Err(e) => {
                        this.danfe = EstadoDoDanfe::Falhou;
                        this.erro = Some(e.texto());
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn mostrar_na_pasta(&mut self, cx: &mut Context<Self>) {
        let Conteudo::Lida(n) = &self.detalhe else {
            return;
        };
        let chave = n.registro.chave.clone();
        self.erro = None;
        let (servicos, config) = (Arc::clone(&self.servicos), self.config.clone());
        cx.spawn(async move |this, cx| {
            let r = cx
                .background_spawn(async move { servicos.caminho_do_xml_recebida(&config, &chave) })
                .await;
            this.update(cx, |this, cx| {
                match r {
                    Ok(c) => pedir_ao_sistema(&this.servicos, Destino::Mostrar, &c, cx),
                    Err(e) => this.erro = Some(e.texto()),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// As acoes da nota: o que fazer com ela, na ordem de quem compra.
    fn acoes(&mut self, n: &RecebidaDaPasta, cx: &mut Context<Self>) -> AnyElement {
        let l = LinhaRecebida::do_registro(&n.registro);
        let gerando = self.danfe == EstadoDoDanfe::Gerando;
        let alvo = Self::alvo(n);
        let pode_responder =
            !alvo.respostas_possiveis().is_empty() && l.nota == SituacaoDaNota::Autorizada;
        // a ciencia (e o desconhecimento) de nota encerrada a SEFAZ recusa; o desconhecimento nao libera a nota completa
        let pode_baixar = !l.completa
            && l.nota == SituacaoDaNota::Autorizada
            && l.resposta != RespostaDada::Desconhecida;
        h_flex()
            .id("recebidas-acoes-da-nota")
            .gap_2()
            .flex_wrap()
            .when(pode_responder, |d| {
                d.child(
                    Button::new("recebidas-responder")
                        .label("Responder à nota")
                        .icon(IconName::Check)
                        .primary()
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.pedir(OperacaoRecebida::Responder, window, cx);
                        })),
                )
            })
            .when(pode_baixar, |d| {
                d.child(
                    Button::new("recebidas-baixar")
                        .label("Baixar a nota completa")
                        .icon(IconName::FileText)
                        .when(pode_responder, |b| b.outline())
                        .when(!pode_responder, |b| b.primary())
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.pedir(OperacaoRecebida::Baixar, window, cx);
                        })),
                )
            })
            .when(l.completa, |d| {
                d.child(
                    Button::new("recebidas-abrir-danfe")
                        .label(if self.danfe == EstadoDoDanfe::Falhou {
                            "Tentar de novo o DANFE"
                        } else {
                            "Abrir o DANFE"
                        })
                        .icon(IconName::FileText)
                        .when(pode_responder, |b| b.outline())
                        .when(!pode_responder, |b| b.primary())
                        .loading(gerando)
                        .disabled(gerando)
                        .on_click(cx.listener(|this, _, _, cx| this.abrir_danfe(cx))),
                )
                .child(
                    Button::new("recebidas-mostrar-na-pasta")
                        .label("Mostrar na pasta")
                        .icon(IconName::FolderOpen)
                        .outline()
                        .on_click(cx.listener(|this, _, _, cx| this.mostrar_na_pasta(cx))),
                )
            })
            .into_any_element()
    }

    /// O que a nota pede da empresa, em uma frase (e a cor dela).
    fn aviso_da_nota(l: &LinhaRecebida) -> Option<(Gravidade, &'static str)> {
        match (l.nota, l.resposta, l.completa) {
            (SituacaoDaNota::Cancelada, _, _) => Some((
                Gravidade::Erro,
                "O emitente cancelou esta nota: ela deixou de valer. Não há o que responder.",
            )),
            (SituacaoDaNota::Denegada, _, _) => Some((
                Gravidade::Erro,
                "A SEFAZ negou o uso desta nota: ela não vale. Não há o que responder.",
            )),
            (_, RespostaDada::Nenhuma, _) => Some((
                Gravidade::Aviso,
                "Esta nota espera uma resposta da sua empresa. Se a compra aconteceu, confirme; se ainda não conferiu, dê ciência; se não foi você quem comprou, diga que não comprou.",
            )),
            (_, RespostaDada::Ciencia, _) => Some((
                Gravidade::Neutro,
                "Você deu ciência (\"vi a nota\"). Quando conferir a compra, volte e dê a resposta final.",
            )),
            (_, RespostaDada::Desconhecida, false) => Some((
                Gravidade::Neutro,
                "Você disse que não fez esta compra. O Ambiente Nacional não entrega a nota completa depois do desconhecimento: só o resumo fica aqui.",
            )),
            _ => None,
        }
    }

    fn cartao_do_resumo(
        l: &LinhaRecebida,
        d: &crate::modelo::nfe_detalhe::DetalheNfe,
        cx: &App,
    ) -> AnyElement {
        let sem = |t: &str| {
            if t.is_empty() {
                "-".to_string()
            } else {
                t.to_string()
            }
        };
        cartao(cx)
            .id("recebidas-detalhe-resumo")
            .child(titulo_do_bloco("O que o resumo traz", cx))
            .child(linha_de_resumo(
                "Fornecedor",
                if d.emitente_documento.is_empty() {
                    sem(&d.emitente)
                } else {
                    format!("{} ({})", d.emitente, d.emitente_documento)
                },
                cx,
            ))
            .child(linha_de_resumo("Emitida em", sem(&d.emissao), cx))
            .child(linha_de_resumo(
                "Valor da nota",
                format!("R$ {}", l.valor),
                cx,
            ))
            .child(linha_de_resumo("Protocolo", sem(&d.protocolo), cx))
            .child(linha_de_resumo(
                "Itens e impostos",
                "só na nota completa".to_string(),
                cx,
            ))
            .into_any_element()
    }

    fn corpo_da_nota(&mut self, n: &RecebidaDaPasta, cx: &mut Context<Self>) -> AnyElement {
        let d = n.leitura.detalhe().clone();
        let l = LinhaRecebida::do_registro(&n.registro);
        let ambiente = if d.producao {
            Ambiente::Producao
        } else {
            Ambiente::Restrita
        };
        let cores = ambiente.cores();
        let muted = cx.theme().muted_foreground;
        let aviso_de_xml = match &n.leitura {
            LeituraNfe::Ilegivel(_) => Some(
                "O arquivo XML desta nota está na pasta, mas não é uma NF-e legível. Os dados abaixo são os do índice.",
            ),
            LeituraNfe::Ausente(_) if l.completa => Some(
                "O arquivo XML desta nota não está na pasta de notas. Os dados abaixo são os do índice.",
            ),
            _ => None,
        };
        let aviso = Self::aviso_da_nota(&l);
        let botao_chave = botao_de_copiar_a_chave(self.chave_copiada)
            .on_click(cx.listener(|this, _, _, cx| this.copiar_chave(cx)));
        let mut coluna = v_flex()
            .gap_4()
            .child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .flex_wrap()
                    .child(
                        div()
                            .text_xl()
                            .font_semibold()
                            .child(format!("NF-e {}", d.numero)),
                    )
                    .child(
                        selo_de(l.nota.rotulo(), cor_da(l.nota.gravidade(), cx))
                            .id("recebidas-selo-da-nota"),
                    )
                    .child(
                        selo_de(l.resposta.rotulo(), cor_da(l.resposta.gravidade(), cx))
                            .id("recebidas-selo-da-resposta"),
                    )
                    .child(
                        div()
                            .id("recebidas-selo-do-ambiente")
                            .px_2()
                            .py_0p5()
                            .rounded_full()
                            .text_xs()
                            .font_semibold()
                            .bg(rgb(cores.fundo))
                            .text_color(rgb(cores.texto))
                            .child(selo_do_ambiente_da_nfe(ambiente)),
                    ),
            )
            .children(aviso.map(|(g, t)| faixa(g, t, cx).id("recebidas-aviso-da-nota")))
            .children(
                aviso_de_xml.map(|t| faixa(Gravidade::Aviso, t, cx).id("recebidas-aviso-do-xml")),
            )
            .child(self.acoes(n, cx))
            .children(
                self.erro
                    .clone()
                    .map(|e| faixa(Gravidade::Erro, e, cx).id("recebidas-erro-do-detalhe")),
            )
            .child(cartao_da_chave(&d.chave_em_grupos, botao_chave, cx));
        if matches!(n.leitura, LeituraNfe::Lida(_)) {
            for s in secoes_da_nota(&d, cx) {
                coluna = coluna.child(s);
            }
        } else {
            coluna = coluna.child(Self::cartao_do_resumo(&l, &d, cx));
            if !l.completa
                && l.nota == SituacaoDaNota::Autorizada
                && l.resposta != RespostaDada::Desconhecida
            {
                // a aplicacao so' registra a ciencia ao baixar se a nota ainda nao tem resposta nesta pasta
                // (`rustnf-app/src/nfe/recebidas.rs`, doc de `baixar`)
                let texto = if l.resposta == RespostaDada::Nenhuma {
                    "Você tem só o resumo desta nota. A nota completa (itens, impostos e DANFE) só é entregue depois que a sua empresa responde sobre ela: baixá-la registra a ciência da operação na SEFAZ."
                } else {
                    "Você tem só o resumo desta nota. A sua empresa já respondeu sobre ela, então a SEFAZ já entrega a nota completa (itens, impostos e DANFE): baixá-la não registra nada de novo."
                };
                coluna =
                    coluna.child(faixa(Gravidade::Neutro, texto, cx).id("recebidas-so-o-resumo"));
            }
        }
        coluna
            .child(div().text_xs().text_color(muted).child(
                if l.nota == SituacaoDaNota::Cancelada {
                    "Os dados vêm do XML da nota. O DANFE sai com a marca de nota cancelada."
                } else {
                    "Os dados vêm do XML da nota. O DANFE é gerado por este programa a partir dele."
                },
            ))
            .into_any_element()
    }

    /// O corpo do painel do detalhe.
    pub(super) fn corpo_do_detalhe(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let muted = cx.theme().muted_foreground;
        let corpo: AnyElement = match self.detalhe.clone() {
            Conteudo::Vazio => div()
                .id("recebidas-detalhe-vazio")
                .p_8()
                .text_sm()
                .text_color(muted)
                .child("Escolha uma nota para ver o detalhe.")
                .into_any_element(),
            Conteudo::Carregando => h_flex()
                .id("recebidas-detalhe-carregando")
                .p_8()
                .gap_2()
                .text_sm()
                .text_color(muted)
                .child(Icon::new(IconName::LoaderCircle))
                .child("Lendo a nota...")
                .into_any_element(),
            Conteudo::Falha(t) => faixa(Gravidade::Erro, t, cx)
                .id("recebidas-detalhe-falhou")
                .into_any_element(),
            Conteudo::Lida(n) => self.corpo_da_nota(&n, cx),
        };
        v_flex()
            .id("recebidas-detalhe-da-nota")
            .w_full()
            .max_w(px(760.))
            .mx_auto()
            .gap_4()
            .child(corpo)
            .into_any_element()
    }
}
