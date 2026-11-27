// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O certificado e o cofre da janela: o estado do rodape, "Testar certificado", guardar e apagar a senha e "Testar
//! conexao com a Sefin". Moram aqui para `raiz.rs` nao crescer: sao metodos da mesma `Raiz`.

use std::sync::Arc;

use gpui_kit::component::notification::Notification;
use gpui_kit::component::WindowExt as _;
use gpui_kit::{AppContext as _, Context, Window};

use super::raiz::{Aviso, Raiz};
use crate::fora;
use crate::modelo::campo::Campo;
use crate::servicos::ErroTela;

impl Raiz {
    // ---------------------------------------------------------------- certificado e cofre

    /// Confere o estado do certificado fora da tela e atualiza o rodape.
    pub(super) fn atualizar_certificado(&mut self, cx: &mut Context<Self>) {
        let config = self.formulario.para_config();
        let servicos = Arc::clone(&self.servicos);
        let servicos2 = Arc::clone(&self.servicos);
        let config2 = config.clone();
        cx.spawn(async move |this, cx| {
            let (estado, guardada) = cx
                .background_spawn(async move {
                    (
                        servicos.estado_do_certificado(&config),
                        servicos2.senha_guardada(&config2),
                    )
                })
                .await;
            this.update(cx, |this, cx| {
                this.cert = estado;
                this.senha_guardada = guardada;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn testar_certificado(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.ocupado.certificado {
            return;
        }
        let config = self.formulario.para_config();
        let senha = self.senha_digitada(cx);
        let servicos = Arc::clone(&self.servicos);
        self.ocupado.certificado = true;
        self.resultado_cert = None;
        self.erros_externos.remove(&Campo::Pfx);
        self.erros_externos.remove(&Campo::Senha);
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let r = cx
                .background_spawn(
                    async move { servicos.testar_certificado(&config, senha.as_ref()) },
                )
                .await;
            this.update_in(cx, |this, _, cx| {
                this.ocupado.certificado = false;
                if let Err(erro) = &r {
                    this.erro_no_campo(erro);
                }
                this.resultado_cert = Some(r);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Mostra o erro embaixo do campo que o originou (ou como aviso, se nao for de campo nenhum).
    pub(super) fn erro_no_campo(&mut self, erro: &ErroTela) {
        match erro.campo() {
            Some(campo) => {
                self.erros_externos.insert(campo, erro.texto());
            }
            None => {
                self.aviso = Some(Aviso {
                    erro: true,
                    texto: erro.texto(),
                });
            }
        }
    }

    pub(super) fn guardar_senha(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.ocupado.senha {
            return;
        }
        let Some(senha) = self.senha_digitada(cx) else {
            self.erros_externos
                .insert(Campo::Senha, ErroTela::SemSenha.texto());
            cx.notify();
            return;
        };
        let config = self.formulario.para_config();
        let servicos = Arc::clone(&self.servicos);
        self.ocupado.senha = true;
        self.erros_externos.remove(&Campo::Senha);
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let r = cx
                .background_spawn(async move { servicos.guardar_senha(&config, &senha) })
                .await;
            this.update_in(cx, |this, window, cx| {
                this.ocupado.senha = false;
                match r {
                    Ok(()) => {
                        this.entradas
                            .senha
                            .update(cx, |e, cx| e.set_value("", window, cx));
                        this.aviso = None;
                        window.push_notification(
                            Notification::success("Senha guardada no cofre do Windows.")
                                .title("Senha guardada"),
                            cx,
                        );
                        this.atualizar_certificado(cx);
                    }
                    Err(erro) => this.erro_no_campo(&erro),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn apagar_senha(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.ocupado.senha {
            return;
        }
        let config = self.formulario.para_config();
        let servicos = Arc::clone(&self.servicos);
        self.ocupado.senha = true;
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let r = cx
                .background_spawn(async move { servicos.apagar_senha(&config) })
                .await;
            this.update_in(cx, |this, window, cx| {
                this.ocupado.senha = false;
                match r {
                    Ok(()) => {
                        window.push_notification(
                            Notification::success("A senha foi apagada do cofre do Windows.")
                                .title("Senha apagada"),
                            cx,
                        );
                        this.resultado_cert = None;
                        this.atualizar_certificado(cx);
                    }
                    Err(erro) => this.erro_no_campo(&erro),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// O botao "Testar conexao com a Sefin": uma chamada de rede de verdade, feita so' quando a pessoa clica.
    pub(super) fn testar_conexao(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.ocupado.conexao {
            return;
        }
        let config = self.formulario.para_config();
        let senha = self.senha_digitada(cx);
        let servicos = Arc::clone(&self.servicos);
        let conexao = servicos.conexao();
        self.ocupado.conexao = true;
        self.resultado_conexao = None;
        self.erros_externos.remove(&Campo::Pfx);
        self.erros_externos.remove(&Campo::Senha);
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            // 1) abre o certificado (calculo e disco) no pool de segundo plano
            let preparo = cx
                .background_spawn(async move { servicos.preparar_conexao(&config, senha.as_ref()) })
                .await;
            // 2) a chamada de rede roda no runtime proprio; aqui so' se espera o resultado
            let resultado: Result<(bool, String), ErroTela> = match preparo {
                Ok(p) => match fora::rede(conexao.testar(p)) {
                    Some(tarefa) => tarefa
                        .await
                        .map_err(|_| ErroTela::Rede("A chamada à Sefin foi interrompida.".into())),
                    None => Err(ErroTela::Rede(
                        "Não foi possível preparar a chamada de rede.".into(),
                    )),
                },
                Err(e) => Err(e),
            };
            this.update_in(cx, |this, _, cx| {
                this.ocupado.conexao = false;
                match resultado {
                    Ok(r) => this.resultado_conexao = Some(r),
                    Err(erro) => {
                        this.erro_no_campo(&erro);
                        this.resultado_conexao = Some((false, erro.texto()));
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}
