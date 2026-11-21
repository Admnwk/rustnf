// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O detalhe de uma nota, o mesmo nas telas Notas e Consultar: dados lidos do XML, chave de acesso com "Copiar",
//! DANFSe, "Mostrar na pasta", "Usar como modelo" e Cancelar. A tela dona dele escolhe a nota (da pasta ou da consulta);
//! tudo o que lê disco ou gera PDF roda fora da linha da tela.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{
    h_flex, v_flex, ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, rgb, AnyElement, App, AppContext as _, ClipboardItem, Context, Entity, EventEmitter,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, Styled as _, Window,
};
use rustnf_app::config::Config;
use rustnf_app::notas::{Registro, Situacao};

use super::cancelar::{AlvoDoCancelamento, Cancelamento};
use super::pecas::{
    cartao, faixa, linha_de_numero, linha_de_resumo, selo_da_situacao, titulo_do_bloco,
    Negrito as _,
};
use crate::modelo::ambiente::Ambiente;
use crate::modelo::certificado::Gravidade;
use crate::modelo::chave;
use crate::modelo::consulta::{guarda, Guarda, NotaConsultada};
use crate::modelo::detalhe::{DetalheDaNota, LeituraDoXml};
use crate::modelo::lista::LinhaDeNota;
use crate::modelo::nota::DadosNota;
use crate::modelo::pedido::Contexto;
use crate::modelo::reaproveitar::dados_da_nota;
use crate::servicos::{Destino, ErroTela, Servicos};

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

/// O detalhe de uma nota.
pub(super) struct VisaoDetalhe {
    servicos: Arc<Servicos>,
    config: Config,
    ctx: Contexto,
    cancelamento: Entity<Cancelamento>,
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
        _cx: &mut Context<Self>,
    ) -> Self {
        let config = Config::default();
        let ctx = servicos.contexto_da_nota(&config);
        Self {
            servicos,
            config,
            ctx,
            cancelamento,
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

    /// O XML da nota mostrada foi copiado (o botao "Copiar o XML" mostra isso).
    #[cfg(test)]
    pub(super) fn xml_copiado(&self) -> bool {
        self.xml_copiado
    }

    /// Nao ha nota escolhida.
    pub(super) fn limpar(&mut self, cx: &mut Context<Self>) {
        self.zerar();
        self.conteudo = Conteudo::Vazio;
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

    /// A nota mostrada agora esta cancelada (a pasta e a Sefin dizem): atualiza o selo.
    pub(super) fn marcar_cancelada(&mut self, chave: &str, cx: &mut Context<Self>) {
        let mesma = self.chave().is_some_and(|c| c == chave);
        if !mesma {
            return;
        }
        match &mut self.conteudo {
            Conteudo::Lida { detalhe, origem } => {
                detalhe.situacao = Situacao::Cancelada;
                if let Origem::Consulta { nota, .. } = origem {
                    nota.detalhe.situacao = Situacao::Cancelada;
                    nota.registro.situacao = Situacao::Cancelada;
                    nota.situacao_conferida = true;
                }
            }
            Conteudo::SemXml { registro, .. } => registro.situacao = Situacao::Cancelada,
            _ => {}
        }
        self.danfse = Danfse::Parado;
        cx.notify();
    }

    // ---------------------------------------------------------------- acoes

    fn copiar_chave(&mut self, cx: &mut Context<Self>) {
        if let Some(c) = self.chave().map(str::to_string) {
            cx.write_to_clipboard(ClipboardItem::new_string(c));
            self.copiada = true;
            cx.notify();
        }
    }

    /// "Copiar o XML": poe na area de transferencia o XML da nota, inteiro. Nota da pasta: le o arquivo guardado, fora
    /// da linha da tela. Nota que veio da consulta: o XML ja esta na memoria.
    fn copiar_xml(&mut self, cx: &mut Context<Self>) {
        let chave = match &self.conteudo {
            Conteudo::Lida {
                origem: Origem::Consulta { nota, .. },
                ..
            } => {
                cx.write_to_clipboard(ClipboardItem::new_string(nota.xml.clone()));
                self.xml_copiado = true;
                cx.notify();
                return;
            }
            Conteudo::Lida {
                detalhe,
                origem: Origem::Pasta,
            } => detalhe.chave.clone(),
            _ => return,
        };
        self.erro = None;
        let versao = self.versao;
        let servicos = Arc::clone(&self.servicos);
        let config = self.config.clone();
        cx.spawn(async move |this, cx| {
            let r = cx
                .background_spawn(async move { servicos.xml_da_nota(&config, &chave) })
                .await;
            this.update(cx, |this, cx| {
                // a pessoa escolheu outra nota enquanto o arquivo era lido: nao copia o XML da anterior
                if this.versao != versao {
                    return;
                }
                match r {
                    Ok(xml) => {
                        cx.write_to_clipboard(ClipboardItem::new_string(xml));
                        this.xml_copiado = true;
                    }
                    Err(e) => this.erro = Some(e.texto()),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
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

    fn mostrar_na_pasta(&mut self, cx: &mut Context<Self>) {
        let Some(chave) = self.chave().map(str::to_string) else {
            return;
        };
        self.erro = None;
        let servicos = Arc::clone(&self.servicos);
        let config = self.config.clone();
        cx.spawn(async move |this, cx| {
            let r = cx
                .background_spawn(async move { servicos.caminho_do_xml(&config, &chave) })
                .await;
            this.update(cx, |this, cx| {
                match r {
                    Ok(caminho) => this.pedir_ao_sistema(Destino::Mostrar, &caminho, cx),
                    Err(e) => this.erro = Some(e.texto()),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// "Abrir o DANFSe": gera o PDF fora da tela. Nota da pasta: grava ao lado do XML e abre. Nota que so' veio da
    /// consulta: pergunta onde salvar.
    fn abrir_danfse(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.danfse == Danfse::Gerando {
            return;
        }
        let (chave, da_pasta, xml, situacao) = match &self.conteudo {
            Conteudo::Lida {
                detalhe,
                origem: Origem::Pasta,
            } => (detalhe.chave.clone(), true, None, detalhe.situacao),
            Conteudo::Lida {
                detalhe,
                origem: Origem::Consulta { nota, guarda: g },
            } => (
                detalhe.chave.clone(),
                *g == Guarda::JaEstaNaPasta,
                Some(nota.xml.clone()),
                detalhe.situacao,
            ),
            _ => return,
        };
        self.danfse = Danfse::Gerando;
        self.erro = None;
        cx.notify();
        let servicos = Arc::clone(&self.servicos);
        let config = self.config.clone();
        if da_pasta {
            cx.spawn_in(window, async move |this, cx| {
                let c2 = config.clone();
                let r = cx
                    .background_spawn(async move { servicos.danfse_da_pasta(&c2, &chave) })
                    .await;
                this.update(cx, |this, cx| this.danfse_pronto(r, cx)).ok();
            })
            .detach();
            return;
        }
        let Some(xml) = xml else {
            self.danfse = Danfse::Falhou(ErroTela::XmlAusente.texto());
            return;
        };
        cx.spawn_in(window, async move |this, cx| {
            let (s2, c2) = (Arc::clone(&servicos), config.clone());
            let pdf = cx
                .background_spawn(async move { s2.gerar_pdf(&c2, &xml, situacao) })
                .await;
            this.update_in(cx, |this, window, cx| match pdf {
                Ok(bytes) => this.salvar_como(&chave, bytes, &config, window, cx),
                Err(e) => this.danfse_pronto(Err(e), cx),
            })
            .ok();
        })
        .detach();
    }

    /// Pergunta onde salvar o PDF de uma nota que nao esta na pasta e o grava.
    fn salvar_como(
        &mut self,
        chave: &str,
        bytes: Vec<u8>,
        config: &Config,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let pasta = PathBuf::from(config.notas.pasta.trim());
        let nome = format!("{chave}.pdf");
        let recebe = cx.prompt_for_new_path(&pasta, Some(&nome));
        let servicos = Arc::clone(&self.servicos);
        cx.spawn_in(window, async move |this, cx| {
            let escolhido = match recebe.await {
                Ok(Ok(Some(caminho))) => caminho,
                _ => {
                    this.update(cx, |this, cx| {
                        this.danfse = Danfse::Parado;
                        cx.notify();
                    })
                    .ok();
                    return;
                }
            };
            let destino = escolhido.clone();
            let r = cx
                .background_spawn(async move {
                    servicos.gravar_arquivo(&destino, &bytes).map(|()| destino)
                })
                .await;
            this.update(cx, |this, cx| this.danfse_pronto(r, cx)).ok();
        })
        .detach();
    }

    fn danfse_pronto(&mut self, r: Result<PathBuf, ErroTela>, cx: &mut Context<Self>) {
        match r {
            Ok(caminho) => {
                let nome = caminho
                    .file_name()
                    .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                self.pedir_ao_sistema(Destino::Abrir, &caminho, cx);
                self.danfse = Danfse::Gravado(nome);
            }
            Err(e) => self.danfse = Danfse::Falhou(e.texto()),
        }
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

    fn cancelar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (chave, numero, tomador, valor) = match &self.conteudo {
            Conteudo::Lida { detalhe, .. } => (
                detalhe.chave.clone(),
                detalhe.numero.clone(),
                detalhe.tomador.clone(),
                detalhe.valor_servico.clone(),
            ),
            Conteudo::SemXml { registro, .. } => {
                let l = LinhaDeNota::do_registro(registro);
                (l.chave, l.numero, l.tomador, l.valor)
            }
            _ => return,
        };
        let alvo = AlvoDoCancelamento {
            chave,
            numero,
            tomador,
            valor,
        };
        self.cancelamento
            .update(cx, |c, cx| c.iniciar(alvo, window, cx));
    }

    // ---------------------------------------------------------------- desenho

    fn selo_do_ambiente(ambiente: Option<u8>) -> Option<AnyElement> {
        let a = match ambiente? {
            1 => Ambiente::Producao,
            _ => Ambiente::Restrita,
        };
        let c = a.cores();
        Some(
            div()
                .px_2()
                .py_0p5()
                .rounded_full()
                .text_xs()
                .font_semibold()
                .bg(rgb(c.fundo))
                .text_color(rgb(c.texto))
                .child(a.selo())
                .into_any_element(),
        )
    }

    fn bloco_da_chave(&self, chave_da_nota: &str, cx: &mut Context<Self>) -> AnyElement {
        let tema = cx.theme();
        let (borda, fundo, muted) = (tema.border, tema.muted, tema.muted_foreground);
        let copiada = self.copiada;
        v_flex()
            .gap_1p5()
            .child(titulo_do_bloco("Chave de acesso", cx))
            .child(
                div()
                    .id("chave-de-acesso")
                    .p_3()
                    .rounded_md()
                    .border_1()
                    .border_color(borda)
                    .bg(fundo)
                    .text_sm()
                    .child(chave::em_grupos(chave_da_nota)),
            )
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        Button::new("copiar-chave-detalhe")
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
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .child("50 posições, em grupos de 5 para conferir."),
                    ),
            )
            .into_any_element()
    }

    fn acoes(
        &self,
        situacao: Situacao,
        da_pasta: bool,
        pode_modelo: bool,
        pode_cancelar: bool,
        com_xml: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
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
                        .on_click(cx.listener(|this, _, window, cx| this.cancelar(window, cx))),
                )
            })
            .into_any_element()
    }

    fn aviso_do_danfse(&self, cx: &App) -> Option<AnyElement> {
        match &self.danfse {
            Danfse::Parado | Danfse::Gerando => None,
            Danfse::Falhou(t) => Some(
                faixa(Gravidade::Erro, t.clone(), cx)
                    .id("danfse-falhou")
                    .into_any_element(),
            ),
            Danfse::Gravado(nome) => Some(
                faixa(
                    Gravidade::Ok,
                    format!("DANFSe gerado e aberto no leitor de PDF. Arquivo: {nome}."),
                    cx,
                )
                .id("danfse-gravado")
                .into_any_element(),
            ),
        }
    }

    fn secoes(d: &DetalheDaNota, cx: &App) -> Vec<AnyElement> {
        let sem = |t: &str| {
            if t.is_empty() {
                "-".to_string()
            } else {
                t.to_string()
            }
        };
        let tomador = cartao(cx)
            .id("detalhe-tomador")
            .child(titulo_do_bloco("Tomador", cx))
            .child(linha_de_resumo("Nome", sem(&d.tomador), cx))
            .child(linha_de_resumo("Documento", sem(&d.tomador_documento), cx));
        let servico = cartao(cx)
            .id("detalhe-servico")
            .child(titulo_do_bloco("Serviço", cx))
            .child(linha_de_resumo(
                "Código",
                if d.servico_tributacao.is_empty() {
                    sem(&d.servico_codigo)
                } else {
                    format!("{} - {}", d.servico_codigo, d.servico_tributacao)
                },
                cx,
            ))
            .child(linha_de_resumo("Descrição", sem(&d.servico_descricao), cx));
        let valores = cartao(cx)
            .id("detalhe-valores")
            .child(titulo_do_bloco("Valores", cx))
            .child(linha_de_numero(
                "Valor do serviço",
                format!("R$ {}", d.valor_servico),
                false,
                cx,
            ))
            .child(linha_de_numero(
                "Base de cálculo",
                format!("R$ {}", d.base_calculo),
                false,
                cx,
            ))
            .child(linha_de_numero(
                if d.aliquota_iss.is_empty() {
                    "ISS".to_string()
                } else {
                    format!("ISS ({}%)", d.aliquota_iss)
                }
                .as_str(),
                format!("R$ {}", d.iss),
                false,
                cx,
            ))
            .child(linha_de_numero(
                "Total retido",
                format!("R$ {}", d.retido),
                false,
                cx,
            ))
            .child(div().h(px(1.)).w_full().bg(cx.theme().border).my_1())
            .child(linha_de_numero(
                "Valor líquido",
                format!("R$ {}", d.liquido),
                true,
                cx,
            ));
        let emissao = cartao(cx)
            .id("detalhe-emissao")
            .child(titulo_do_bloco("Emissão", cx))
            .child(linha_de_resumo("Emitida em", sem(&d.emissao), cx))
            .child(linha_de_resumo("Competência", sem(&d.competencia), cx))
            .child(linha_de_resumo("DPS", sem(&d.dps), cx))
            .child(linha_de_resumo(
                "Emitente",
                if d.emitente_documento.is_empty() {
                    sem(&d.emitente)
                } else {
                    format!("{} ({})", d.emitente, d.emitente_documento)
                },
                cx,
            ));
        vec![
            tomador.into_any_element(),
            servico.into_any_element(),
            valores.into_any_element(),
            emissao.into_any_element(),
        ]
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
            .children(Self::selo_do_ambiente(d.ambiente));
        let mut coluna = v_flex()
            .gap_4()
            .child(cabeca)
            .child(self.acoes(d.situacao, da_pasta, pode_modelo, pode_cancelar, true, cx))
            .children(self.aviso_do_danfse(cx))
            .children(self.erro.clone().map(|e| {
                faixa(Gravidade::Erro, e, cx)
                    .id("erro-do-detalhe")
                    .into_any_element()
            }))
            .children(avisos)
            .child(self.bloco_da_chave(&d.chave, cx));
        for s in Self::secoes(d, cx) {
            coluna = coluna.child(s);
        }
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
            .child(self.acoes(registro.situacao, true, false, cancelavel, false, cx))
            .child(self.bloco_da_chave(&l.chave, cx))
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
