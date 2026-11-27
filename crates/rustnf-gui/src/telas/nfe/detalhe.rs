// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O detalhe de uma NF-e: os dados do XML guardado (emitente, destinatario, itens resumidos, totais, protocolo), a
//! situacao com o que fazer, e as acoes: abrir o DANFE, mostrar na pasta, copiar a chave, conferir na SEFAZ, cancelar,
//! carta de correcao e retomar. O PDF e a leitura do disco rodam fora da linha da tela.

use std::path::Path;
use std::sync::Arc;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{
    h_flex, v_flex, ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, rgb, AnyElement, App, AppContext as _, ClipboardItem, Context,
    InteractiveElement as _, IntoElement, ParentElement as _, Styled as _, Window,
};
use rustnf_app::notas::nfe::Situacao;

use super::TelaNfe;
use crate::modelo::ambiente::Ambiente;
use crate::modelo::certificado::Gravidade;
use crate::modelo::nfe_config::selo_do_ambiente_da_nfe;
use crate::modelo::nfe_detalhe::{DetalheNfe, LeituraNfe};
use crate::servicos::{Destino, Documento, NotaNfeDaPasta};
use crate::telas::nfe_dialogo::{AlvoNfe, Operacao};
use crate::telas::pecas::{
    cartao, faixa, linha_de_numero, linha_de_resumo, selo_da_situacao_nfe, titulo_do_bloco,
    Negrito as _,
};

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
    Lida(Box<NotaNfeDaPasta>),
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

impl TelaNfe {
    /// Le a nota `chave` fora da tela e a mostra.
    pub(super) fn ler_o_detalhe(&mut self, chave: &str, cx: &mut Context<Self>) {
        self.versao_do_detalhe += 1;
        let versao = self.versao_do_detalhe;
        self.detalhe = Conteudo::Carregando;
        self.erro = None;
        self.danfe = EstadoDoDanfe::Parado;
        self.chave_copiada = false;
        let (servicos, config, chave, documento) = (
            Arc::clone(&self.servicos),
            self.config.clone(),
            chave.to_string(),
            self.documento,
        );
        cx.spawn(async move |this, cx| {
            let r = cx
                .background_spawn(async move {
                    match documento {
                        Documento::Nfe => servicos.ler_nfe(&config, &chave),
                        Documento::Nfce => servicos.ler_nfce(&config, &chave),
                    }
                })
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

    /// Le de novo a nota aberta (um evento a mudou).
    pub(super) fn reler_o_detalhe(&mut self, cx: &mut Context<Self>) {
        if let Some(chave) = self.escolhida.clone() {
            let danfe = self.danfe;
            self.ler_o_detalhe(&chave, cx);
            self.danfe = danfe;
        }
    }

    fn alvo(n: &NotaNfeDaPasta) -> AlvoNfe {
        let d = n.leitura.detalhe();
        AlvoNfe {
            chave: n.registro.chave.clone(),
            numero: n.registro.numero.clone(),
            serie: n.registro.serie.clone(),
            destinatario: d.destinatario.trim_matches('-').to_string(),
            valor: d.total.clone(),
            cartas: u32::try_from(d.correcoes).unwrap_or(0),
        }
    }

    fn pedir(&mut self, op: Operacao, window: &mut Window, cx: &mut Context<Self>) {
        let Conteudo::Lida(n) = &self.detalhe else {
            return;
        };
        let alvo = Self::alvo(n);
        let a_retomar = self.lista.a_resolver();
        self.dialogo
            .update(cx, |d, cx| d.iniciar(op, Some(alvo), a_retomar, window, cx));
    }

    /// "Usar como modelo": a janela leva a nota para a Nova NF-e.
    fn usar_como_modelo(&mut self, cx: &mut Context<Self>) {
        if let Conteudo::Lida(n) = &self.detalhe {
            cx.emit(super::EventoNfe::UsarComoModelo(n.registro.chave.clone()));
        }
    }

    fn copiar_chave(&mut self, cx: &mut Context<Self>) {
        if let Conteudo::Lida(n) = &self.detalhe {
            cx.write_to_clipboard(ClipboardItem::new_string(n.registro.chave.clone()));
            self.chave_copiada = true;
            cx.notify();
        }
    }

    /// Pede ao sistema que abra (ou mostre) o arquivo, se o servico de abrir arquivos deixar.
    fn pedir_ao_sistema(&self, destino: Destino, caminho: &Path, cx: &mut App) {
        if self.servicos.abridor().pedir(destino, caminho) {
            match destino {
                Destino::Abrir => cx.open_with_system(caminho),
                Destino::Mostrar => cx.reveal_path(caminho),
            }
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
        let (servicos, config, documento) = (
            Arc::clone(&self.servicos),
            self.config.clone(),
            self.documento,
        );
        cx.spawn(async move |this, cx| {
            let r = cx
                .background_spawn(async move {
                    match documento {
                        Documento::Nfe => servicos.danfe_da_pasta(&config, &chave),
                        Documento::Nfce => servicos.danfce_da_pasta(&config, &chave),
                    }
                })
                .await;
            this.update(cx, |this, cx| {
                if this.versao_do_detalhe != versao {
                    return;
                }
                match r {
                    Ok(caminho) => {
                        this.danfe = EstadoDoDanfe::Parado;
                        this.pedir_ao_sistema(Destino::Abrir, &caminho, cx);
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
        let (servicos, config, documento) = (
            Arc::clone(&self.servicos),
            self.config.clone(),
            self.documento,
        );
        cx.spawn(async move |this, cx| {
            let r = cx
                .background_spawn(async move {
                    match documento {
                        Documento::Nfe => servicos.caminho_do_xml_nfe(&config, &chave),
                        Documento::Nfce => servicos.caminho_do_xml_nfce(&config, &chave),
                    }
                })
                .await;
            this.update(cx, |this, cx| {
                match r {
                    Ok(c) => this.pedir_ao_sistema(Destino::Mostrar, &c, cx),
                    Err(e) => this.erro = Some(e.texto()),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn acoes(&mut self, n: &NotaNfeDaPasta, cx: &mut Context<Self>) -> AnyElement {
        let s = n.registro.situacao;
        let gerando = self.danfe == EstadoDoDanfe::Gerando;
        let previa = s == Situacao::Pendente || s == Situacao::Epec;
        h_flex()
            .id("nfe-acoes-da-nota")
            .gap_2()
            .flex_wrap()
            .child(
                Button::new("nfe-abrir-danfe")
                    .label(match (self.danfe, previa, self.documento) {
                        (EstadoDoDanfe::Falhou, _, Documento::Nfce) => "Tentar de novo o cupom",
                        (_, true, Documento::Nfce) => "Abrir a prévia do cupom",
                        (_, _, Documento::Nfce) => "Abrir o cupom",
                        (EstadoDoDanfe::Falhou, _, _) => "Tentar de novo o DANFE",
                        (_, true, _) => "Abrir a prévia do DANFE",
                        _ => "Abrir o DANFE",
                    })
                    .icon(IconName::FileText)
                    .primary()
                    .loading(gerando)
                    .disabled(gerando)
                    .on_click(cx.listener(|this, _, _, cx| this.abrir_danfe(cx))),
            )
            .when(!previa, |d| {
                d.child(
                    Button::new("nfe-mostrar-na-pasta")
                        .label("Mostrar na pasta")
                        .icon(IconName::FolderOpen)
                        .outline()
                        .on_click(cx.listener(|this, _, _, cx| this.mostrar_na_pasta(cx))),
                )
            })
            .child(
                Button::new("nfe-conferir")
                    .label("Conferir na SEFAZ")
                    .icon(IconName::Search)
                    .outline()
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.pedir(Operacao::Conferir, window, cx);
                    })),
            )
            .when(s == Situacao::Autorizada, |d| {
                // a NFC-e nao tem carta de correcao (o evento 110110 e' so' da NF-e)
                d.when(self.documento == Documento::Nfe, |d| {
                    d.child(
                        Button::new("nfe-corrigir")
                            .label("Carta de correção")
                            .icon(IconName::File)
                            .outline()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.pedir(Operacao::Corrigir, window, cx);
                            })),
                    )
                })
                .child(
                    Button::new("nfe-cancelar")
                        .label("Cancelar a nota")
                        .icon(IconName::Ban)
                        .outline()
                        .danger()
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.pedir(Operacao::Cancelar, window, cx);
                        })),
                )
            })
            .when(!previa && self.documento == Documento::Nfe, |d| {
                d.child(
                    Button::new("nfe-usar-como-modelo")
                        .label("Usar como modelo")
                        .icon(IconName::Copy)
                        .outline()
                        .on_click(cx.listener(|this, _, _, cx| this.usar_como_modelo(cx))),
                )
            })
            .when(previa, |d| {
                d.child(
                    Button::new("nfe-retomar-esta")
                        .label("Retomar")
                        .icon(IconName::RefreshCw)
                        .primary()
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.pedir(Operacao::Retomar, window, cx);
                        })),
                )
            })
            .into_any_element()
    }

    fn bloco_da_chave(&self, d: &DetalheNfe, cx: &mut Context<Self>) -> AnyElement {
        let copiada = self.chave_copiada;
        cartao(cx)
            .id("nfe-chave")
            .child(titulo_do_bloco("Chave de acesso", cx))
            .child(div().text_sm().child(d.chave_em_grupos.clone()))
            .child(
                h_flex().child(
                    Button::new("nfe-copiar-chave")
                        .label(if copiada {
                            "Chave copiada"
                        } else {
                            "Copiar a chave"
                        })
                        .icon(if copiada {
                            IconName::Check
                        } else {
                            IconName::Copy
                        })
                        .outline()
                        .small()
                        .on_click(cx.listener(|this, _, _, cx| this.copiar_chave(cx))),
                ),
            )
            .into_any_element()
    }

    fn secoes(d: &DetalheNfe, cx: &App) -> Vec<AnyElement> {
        let sem = |t: &str| {
            if t.is_empty() {
                "-".to_string()
            } else {
                t.to_string()
            }
        };
        let muted = cx.theme().muted_foreground;
        let destinatario = cartao(cx)
            .id("nfe-detalhe-destinatario")
            .child(titulo_do_bloco("Destinatário", cx))
            .child(linha_de_resumo("Nome", sem(&d.destinatario), cx))
            .child(linha_de_resumo(
                "Documento",
                sem(&d.destinatario_documento),
                cx,
            ));
        let mut itens = cartao(cx)
            .id("nfe-detalhe-itens")
            .child(titulo_do_bloco("Itens", cx));
        if d.itens.is_empty() {
            itens = itens.child(
                div()
                    .text_sm()
                    .text_color(muted)
                    .child("O arquivo da nota não está legível: sem itens para mostrar."),
            );
        }
        for i in &d.itens {
            itens = itens.child(linha_de_numero(
                &format!("{} ({})", i.descricao, i.quantidade),
                format!("R$ {}", i.valor),
                false,
                cx,
            ));
        }
        if d.itens_a_mais > 0 {
            itens = itens.child(div().text_xs().text_color(muted).child(format!(
                "e mais {} item(ns): o DANFE traz todos.",
                d.itens_a_mais
            )));
        }
        let mut totais = cartao(cx)
            .id("nfe-detalhe-totais")
            .child(titulo_do_bloco("Totais", cx));
        if !d.total_produtos.is_empty() {
            totais = totais.child(linha_de_numero(
                "Produtos",
                format!("R$ {}", d.total_produtos),
                false,
                cx,
            ));
        }
        if !d.desconto.is_empty() {
            totais = totais.child(linha_de_numero(
                "Desconto",
                format!("R$ {}", d.desconto),
                false,
                cx,
            ));
        }
        if !d.frete.is_empty() {
            totais = totais.child(linha_de_numero(
                "Frete",
                format!("R$ {}", d.frete),
                false,
                cx,
            ));
        }
        totais = totais
            .child(div().h(px(1.)).w_full().bg(cx.theme().border).my_1())
            .child(linha_de_numero(
                "Total da nota",
                format!("R$ {}", d.total),
                true,
                cx,
            ));
        let mut emissao = cartao(cx)
            .id("nfe-detalhe-emissao")
            .child(titulo_do_bloco("Emissão", cx))
            .child(linha_de_resumo("Emitida em", sem(&d.emissao), cx))
            .child(linha_de_resumo("Operação", sem(&d.natureza), cx))
            .child(linha_de_resumo(
                "Emitente",
                if d.emitente_documento.is_empty() {
                    sem(&d.emitente)
                } else {
                    format!("{} ({})", d.emitente, d.emitente_documento)
                },
                cx,
            ))
            .child(linha_de_resumo("Protocolo", sem(&d.protocolo), cx));
        if !d.protocolo_do_cancelamento.is_empty() {
            emissao = emissao.child(linha_de_resumo(
                "Cancelamento",
                format!("protocolo {}", d.protocolo_do_cancelamento),
                cx,
            ));
        }
        if d.correcoes > 0 {
            emissao = emissao.child(linha_de_resumo(
                "Correções",
                crate::modelo::nfe_resultados::contagem(
                    d.correcoes,
                    "carta de correção",
                    "cartas de correção",
                ),
                cx,
            ));
        }
        vec![
            destinatario.into_any_element(),
            itens.into_any_element(),
            totais.into_any_element(),
            emissao.into_any_element(),
        ]
    }

    fn corpo_da_nota(&mut self, n: &NotaNfeDaPasta, cx: &mut Context<Self>) -> AnyElement {
        let d = n.leitura.detalhe().clone();
        let ambiente = if d.producao {
            Ambiente::Producao
        } else {
            Ambiente::Restrita
        };
        let cores = ambiente.cores();
        let muted = cx.theme().muted_foreground;
        let aviso_de_xml = match &n.leitura {
            LeituraNfe::Lida(_) => None,
            LeituraNfe::Ausente(_) => Some(
                "O arquivo XML desta nota não está na pasta de notas. Os dados abaixo são os do índice.",
            ),
            LeituraNfe::Ilegivel(_) => Some(
                "O arquivo XML desta nota está na pasta, mas não é uma NF-e legível. Os dados abaixo são os do índice.",
            ),
        };
        let cancelada = n.registro.situacao == Situacao::Cancelada;
        let mut coluna = v_flex()
            .gap_4()
            .child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .flex_wrap()
                    .child(div().text_xl().font_semibold().child(format!(
                        "{} {}",
                        self.documento.nome(),
                        d.numero
                    )))
                    .child(selo_da_situacao_nfe(n.registro.situacao, cx).id("nfe-selo-da-situacao"))
                    .child(
                        div()
                            .id("nfe-selo-do-ambiente")
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
            .children(d.aviso.clone().map(|a| {
                faixa(
                    if n.registro.situacao == Situacao::Denegada {
                        Gravidade::Erro
                    } else {
                        Gravidade::Aviso
                    },
                    a,
                    cx,
                )
                .id("nfe-aviso-da-situacao")
            }))
            .children(aviso_de_xml.map(|t| faixa(Gravidade::Aviso, t, cx).id("nfe-aviso-do-xml")))
            .child(self.acoes(n, cx))
            .children(
                self.erro
                    .clone()
                    .map(|e| faixa(Gravidade::Erro, e, cx).id("nfe-erro-do-detalhe")),
            )
            .child(self.bloco_da_chave(&d, cx));
        for s in Self::secoes(&d, cx) {
            coluna = coluna.child(s);
        }
        coluna
            .child(div().text_xs().text_color(muted).child(match (cancelada, self.documento) {
                (true, Documento::Nfce) => "Os dados vêm do XML da nota. O cupom sai com a marca de nota cancelada. O programa gera o arquivo do cupom; quem imprime é outro programa.",
                (false, Documento::Nfce) => "Os dados vêm do XML da nota. O cupom (DANFC-e) é gerado por este programa a partir dele; quem imprime é outro programa.",
                (true, _) => "Os dados vêm do XML da nota. O DANFE sai com a marca de nota cancelada.",
                (false, _) => "Os dados vêm do XML da nota. O DANFE é gerado por este programa a partir dele.",
            }))
            .into_any_element()
    }

    /// O corpo do painel do detalhe.
    pub(super) fn corpo_do_detalhe(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let muted = cx.theme().muted_foreground;
        let corpo: AnyElement = match self.detalhe.clone() {
            Conteudo::Vazio => div()
                .id("nfe-detalhe-vazio")
                .p_8()
                .text_sm()
                .text_color(muted)
                .child("Escolha uma nota para ver o detalhe.")
                .into_any_element(),
            Conteudo::Carregando => h_flex()
                .id("nfe-detalhe-carregando")
                .p_8()
                .gap_2()
                .text_sm()
                .text_color(muted)
                .child(Icon::new(IconName::LoaderCircle))
                .child("Lendo a nota...")
                .into_any_element(),
            Conteudo::Falha(t) => faixa(Gravidade::Erro, t, cx)
                .id("nfe-detalhe-falhou")
                .into_any_element(),
            Conteudo::Lida(n) => self.corpo_da_nota(&n, cx),
        };
        v_flex()
            .id("nfe-detalhe-da-nota")
            .w_full()
            .max_w(px(760.))
            .mx_auto()
            .gap_4()
            .child(corpo)
            .into_any_element()
    }
}
