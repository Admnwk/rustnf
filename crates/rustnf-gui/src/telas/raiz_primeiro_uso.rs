// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O primeiro uso da janela: os tres passos (emitente, certificado, pasta de notas) e o que acontece ao voltar, continuar e
//! concluir. Tudo que pode demorar roda fora da linha da tela.

use std::path::PathBuf;
use std::sync::Arc;

use gpui_kit::component::notification::Notification;
use gpui_kit::component::WindowExt as _;
use gpui_kit::{AppContext as _, Context, Window};

use super::raiz::{Aviso, Raiz};
use crate::modelo::campo::Campo;
use crate::modelo::passos::Passo;
use crate::servicos::ErroTela;

impl Raiz {
    pub(super) fn voltar(&mut self, cx: &mut Context<Self>) {
        if let Some(anterior) = self.passo.anterior() {
            self.passo = anterior;
            self.tentou_continuar = false;
            self.aviso = None;
            cx.notify();
        }
    }

    /// "Continuar" (ou "Concluir", no ultimo passo): confere o passo e segue.
    pub(super) fn continuar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.ocupado.passo {
            return;
        }
        self.tentou_continuar = true;
        self.aviso = None;
        if !self.formulario.erros_do_passo(self.passo).vazio() || !self.erros_externos.is_empty() {
            cx.notify();
            return;
        }
        match self.passo {
            Passo::Emitente => self.seguir(cx),
            Passo::Certificado => self.conferir_certificado_do_passo(window, cx),
            Passo::Notas => self.concluir(window, cx),
        }
    }

    pub(super) fn seguir(&mut self, cx: &mut Context<Self>) {
        if let Some(proximo) = self.passo.proximo() {
            self.passo = proximo;
            self.tentou_continuar = false;
        }
        cx.notify();
    }

    /// Passo 2: a senha tem de abrir o certificado; e vai para o cofre.
    pub(super) fn conferir_certificado_do_passo(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let config = self.formulario.para_config();
        let digitada = self.senha_digitada(cx);
        let servicos = Arc::clone(&self.servicos);
        self.ocupado.passo = true;
        self.erros_externos.remove(&Campo::Pfx);
        self.erros_externos.remove(&Campo::Senha);
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let r: Result<Option<ErroTela>, ErroTela> = cx
                .background_spawn(async move {
                    match digitada {
                        Some(senha) => {
                            // so' guarda a senha que de fato abre o certificado
                            servicos.testar_certificado(&config, Some(&senha))?;
                            if servicos.cofre_desligado() {
                                Ok(Some(ErroTela::Cofre(
                                    rustnf_app::cofre::ErroCofre::Desligado,
                                )))
                            } else {
                                servicos.guardar_senha(&config, &senha)?;
                                Ok(None)
                            }
                        }
                        None => {
                            servicos.testar_certificado(&config, None)?;
                            Ok(None)
                        }
                    }
                })
                .await;
            this.update_in(cx, |this, window, cx| {
                this.ocupado.passo = false;
                match r {
                    Ok(aviso) => {
                        if let Some(a) = aviso {
                            window.push_notification(
                                Notification::warning(a.texto()).title("Senha não guardada"),
                                cx,
                            );
                        }
                        this.entradas
                            .senha
                            .update(cx, |e, cx| e.set_value("", window, cx));
                        this.atualizar_certificado(cx);
                        this.seguir(cx);
                    }
                    Err(erro) => {
                        this.erro_no_campo(&erro);
                        cx.notify();
                    }
                }
            })
            .ok();
        })
        .detach();
    }

    /// Passo 3: confere a pasta, grava o arquivo e termina.
    pub(super) fn concluir(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let erros = self.formulario.erros();
        if let Some(primeiro) = erros.primeira() {
            self.aviso = Some(Aviso {
                erro: true,
                texto: format!("Corrija antes de concluir: {primeiro}"),
            });
            cx.notify();
            return;
        }
        let config = self.formulario.para_config();
        let servicos = Arc::clone(&self.servicos);
        let pasta = config.notas.pasta.clone();
        let a_gravar = config.clone();
        self.ocupado.passo = true;
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let r = cx
                .background_spawn(async move {
                    servicos.conferir_pasta(&pasta)?;
                    servicos.salvar(&a_gravar)
                })
                .await;
            this.update_in(cx, |this, window, cx| {
                this.ocupado.passo = false;
                match r {
                    Ok(()) => {
                        this.formulario.marcar_salvo(&config);
                        this.salvo = config;
                        window.push_notification(
                            Notification::success("O rustnf está configurado e pronto para uso.")
                                .title("Configuração salva"),
                            cx,
                        );
                        this.atualizar_certificado(cx);
                        this.entregar_config(window, cx);
                    }
                    Err(erro) => this.erro_no_campo(&erro),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Caminho sugerido para as notas: uma pasta `notas` ao lado do arquivo de configuracao.
    pub(super) fn pasta_sugerida(&self) -> Option<PathBuf> {
        self.servicos
            .local_do_arquivo()
            .and_then(|p| p.parent().map(|d| d.join("notas")))
    }
}
