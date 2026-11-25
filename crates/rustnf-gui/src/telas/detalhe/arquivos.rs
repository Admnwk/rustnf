// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! As acoes do detalhe que mexem com arquivo ou area de transferencia: copiar a chave e o XML, mostrar o XML na pasta
//! e gerar o DANFSe (gravando ao lado do XML ou perguntando onde salvar); mais o bloco da chave de acesso e o aviso do
//! DANFSe, que andam com elas. Tudo o que le disco ou gera PDF roda fora da linha da tela.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui_kit::component::button::Button;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, IconName, Sizable as _};
use gpui_kit::{
    div, AnyElement, App, AppContext as _, ClipboardItem, Context, InteractiveElement as _,
    IntoElement, ParentElement as _, Styled as _, Window,
};
use rustnf_app::config::Config;

use super::{Conteudo, Danfse, Guarda, Origem, VisaoDetalhe};
use crate::modelo::certificado::Gravidade;
use crate::modelo::chave;
use crate::servicos::{Destino, ErroTela};
use crate::telas::pecas::{faixa, titulo_do_bloco};

impl VisaoDetalhe {
    // ---------------------------------------------------------------- acoes

    pub(super) fn copiar_chave(&mut self, cx: &mut Context<Self>) {
        if let Some(c) = self.chave().map(str::to_string) {
            cx.write_to_clipboard(ClipboardItem::new_string(c));
            self.copiada = true;
            cx.notify();
        }
    }

    /// "Copiar o XML": poe na area de transferencia o XML da nota, inteiro. Nota da pasta: le o arquivo guardado, fora
    /// da linha da tela. Nota que veio da consulta: o XML ja esta na memoria.
    pub(super) fn copiar_xml(&mut self, cx: &mut Context<Self>) {
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
    pub(super) fn pedir_ao_sistema(&self, destino: Destino, caminho: &Path, cx: &mut App) {
        if self.servicos.abridor().pedir(destino, caminho) {
            match destino {
                Destino::Abrir => cx.open_with_system(caminho),
                Destino::Mostrar => cx.reveal_path(caminho),
            }
        }
    }

    pub(super) fn mostrar_na_pasta(&mut self, cx: &mut Context<Self>) {
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
    pub(super) fn abrir_danfse(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
    pub(super) fn salvar_como(
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

    pub(super) fn danfse_pronto(&mut self, r: Result<PathBuf, ErroTela>, cx: &mut Context<Self>) {
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

    pub(super) fn bloco_da_chave(&self, chave_da_nota: &str, cx: &mut Context<Self>) -> AnyElement {
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

    pub(super) fn aviso_do_danfse(&self, cx: &App) -> Option<AnyElement> {
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
}
