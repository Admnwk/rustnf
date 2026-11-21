// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O fluxo de emitir: confirmar, congelar o pedido, abrir o certificado, enviar fora da tela e tratar o desfecho.

use std::sync::Arc;

use gpui_kit::component::WindowExt as _;
use gpui_kit::{AppContext as _, Context, Window};
use rustnf_app::segredo::Segredo;

use super::{Concluida, EstadoDoRascunho, Etapa, Fase, NovaNota};
use crate::fora;
use crate::modelo::ambiente::Ambiente;
use crate::modelo::conferencia::Pendencia;
use crate::modelo::emissao::{aviso_de_desistir, DesfechoEmissao};
use crate::modelo::pedido::PedidoCongelado;
use crate::servicos::{emitir, ErroTela};

impl NovaNota {
    /// O botao Emitir (ou Ctrl+Enter): com pendencia, leva ao primeiro campo; sem ela, abre a confirmacao.
    pub(in crate::telas) fn pedir_emissao(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !matches!(self.fase, Fase::Editando) || !self.pronta {
            return;
        }
        self.mostrar_tudo = true;
        if let Some(p) = self.conferencia.primeira().cloned() {
            match p.campo {
                Some(campo) => self.ir_ao_campo(campo, window, cx),
                None => cx.notify(),
            }
            return;
        }
        if window.has_active_dialog(cx) {
            return;
        }
        self.valor_fiscal_entendido = false;
        self.abrir_confirmacao(window, cx);
    }

    /// O ambiente em vigor.
    pub(in crate::telas) fn ambiente(&self) -> Ambiente {
        Ambiente::de_config(&self.config.sefin.ambiente)
    }

    /// Em producao a confirmacao pede a marcacao explicita antes de habilitar o botao.
    pub(in crate::telas) fn exige_valor_fiscal(&self) -> bool {
        self.ambiente() == Ambiente::Producao
    }

    /// O botao da confirmacao: so' habilita em producao depois da marcacao.
    pub(in crate::telas) fn pode_confirmar(&self) -> bool {
        matches!(self.fase, Fase::Editando)
            && self.conferencia.limpa()
            && (!self.exige_valor_fiscal() || self.valor_fiscal_entendido)
    }

    /// Confirmado: congela o pedido e envia.
    pub(in crate::telas) fn confirmar_emissao(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.pode_confirmar() {
            return;
        }
        self.iniciar_envio(None, None, window, cx);
    }

    /// "Tentar de novo": reenvia EXATAMENTE o mesmo pedido (mesmo numero, mesma data e hora de emissao).
    pub(in crate::telas) fn tentar_de_novo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Fase::Pendente { tentativa, .. } = self.fase.clone() else {
            return;
        };
        self.iniciar_envio(Some(tentativa), None, window, cx);
    }

    /// "Desistir desta tentativa": volta a editar, com o aviso de que a nota pode ter sido autorizada.
    pub(in crate::telas) fn desistir(&mut self, cx: &mut Context<Self>) {
        let Fase::Pendente { tentativa, .. } = &self.fase else {
            return;
        };
        self.desistencia = Some(aviso_de_desistir(&tentativa.serie, tentativa.numero));
        self.fase = Fase::Editando;
        self.atualizar_numero(cx);
        cx.notify();
    }

    /// Congela o pedido (se ainda nao esta), abre o certificado e envia, tudo fora da linha da tela.
    pub(in crate::telas) fn iniciar_envio(
        &mut self,
        tentativa: Option<PedidoCongelado>,
        senha: Option<Segredo>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(self.fase, Fase::Enviando { .. }) {
            return;
        }
        let anterior = self.fase.clone();
        self.fase = Fase::Enviando {
            etapa: Etapa::AbrindoCertificado,
            tentativa: tentativa.clone(),
        };
        self.erro_senha = None;
        self.recuperado = false;
        self.rejeicao = None;
        self.recusa = None;
        self.desistencia = None;
        cx.notify();
        let servicos = Arc::clone(&self.servicos);
        let fabrica = servicos.fabrica();
        let config = self.config.clone();
        let dados = self.dados.clone();
        let digitou = senha.is_some();
        cx.spawn_in(window, async move |this, cx| {
            let (s2, c2) = (Arc::clone(&servicos), config.clone());
            let preparado = cx
                .background_spawn(async move {
                    let t = match tentativa {
                        Some(t) => t,
                        None => s2.congelar_pedido(&c2, &dados)?,
                    };
                    let p = s2.preparar_emissao(&c2, senha.as_ref());
                    Ok::<_, ErroTela>((t, p))
                })
                .await;
            let (tentativa, preparo) = match preparado {
                Ok(par) => par,
                Err(erro) => {
                    this.update_in(cx, |this, _, cx| {
                        this.voltar_com(anterior, Pendencia::geral(erro.texto()), cx);
                    })
                    .ok();
                    return;
                }
            };
            let preparo = match preparo {
                Ok(p) => p,
                Err(ErroTela::SemSenha) => {
                    this.update_in(cx, |this, window, cx| {
                        this.fase = anterior;
                        this.pedir_senha(tentativa, None, window, cx);
                    })
                    .ok();
                    return;
                }
                Err(ErroTela::SenhaIncorreta) if digitou => {
                    this.update_in(cx, |this, window, cx| {
                        this.fase = anterior;
                        this.pedir_senha(
                            tentativa,
                            Some(ErroTela::SenhaIncorreta.texto()),
                            window,
                            cx,
                        );
                    })
                    .ok();
                    return;
                }
                Err(outro) => {
                    this.update_in(cx, |this, _, cx| {
                        this.voltar_com(anterior, Pendencia::geral(outro.texto()), cx);
                    })
                    .ok();
                    return;
                }
            };
            let enviada = tentativa.clone();
            this.update_in(cx, |this, _, cx| {
                this.fase = Fase::Enviando {
                    etapa: Etapa::Enviando,
                    tentativa: Some(enviada),
                };
                cx.notify();
            })
            .ok();
            let desfecho = match fora::rede(emitir(fabrica, preparo, config, tentativa.clone())) {
                Some(tarefa) => tarefa
                    .await
                    .unwrap_or_else(|_| DesfechoEmissao::SemResposta {
                        texto: crate::modelo::emissao::TEXTO_SEM_RESPOSTA.into(),
                    }),
                None => DesfechoEmissao::Recusada {
                    pendencia: Pendencia::geral("Não foi possível preparar a chamada de rede."),
                },
            };
            this.update_in(cx, |this, _, cx| {
                this.aplicar_desfecho(desfecho, tentativa, cx);
            })
            .ok();
        })
        .detach();
    }

    /// Volta a fase anterior, mostrando por que nada foi enviado.
    fn voltar_com(&mut self, anterior: Fase, motivo: Pendencia, cx: &mut Context<Self>) {
        self.fase = anterior;
        self.recusa = Some(motivo);
        cx.notify();
    }

    /// Trata como a emissao terminou.
    pub(in crate::telas) fn aplicar_desfecho(
        &mut self,
        desfecho: DesfechoEmissao,
        tentativa: PedidoCongelado,
        cx: &mut Context<Self>,
    ) {
        match desfecho {
            DesfechoEmissao::Autorizada {
                numero,
                chave,
                guardada,
                aviso,
                recuperada,
                alertas,
            } => {
                self.fase = Fase::Concluida(Concluida {
                    numero,
                    chave,
                    guardada,
                    aviso,
                    recuperada,
                    alertas,
                });
                self.copiada = false;
                self.versao_do_rascunho_para_apagar(cx);
                self.atualizar_numero(cx);
            }
            DesfechoEmissao::Rejeitada { mensagens } => {
                self.fase = Fase::Editando;
                for m in &mensagens {
                    if let Some(campo) = m.campo {
                        self.marcas.insert(campo, m.descricao.clone());
                        self.tocados.insert(campo);
                    }
                }
                self.rejeicao = Some(mensagens);
                self.rolagem.scroll_to_top_of_item(0);
                self.atualizar_numero(cx);
            }
            DesfechoEmissao::SemResposta { texto } => {
                self.fase = Fase::Pendente { tentativa, texto };
            }
            DesfechoEmissao::Recusada { pendencia } => {
                self.fase = Fase::Editando;
                if let Some(campo) = pendencia.campo {
                    self.marcas.insert(campo, pendencia.mensagem.clone());
                    self.tocados.insert(campo);
                }
                self.recusa = Some(pendencia);
                self.rolagem.scroll_to_top_of_item(0);
            }
        }
        cx.notify();
    }

    /// Nota autorizada: o rascunho nao serve mais.
    fn versao_do_rascunho_para_apagar(&mut self, cx: &mut Context<Self>) {
        self.versao += 1;
        self.rascunho = EstadoDoRascunho::Nenhum;
        let servicos = Arc::clone(&self.servicos);
        let config = self.config.clone();
        cx.background_executor()
            .spawn(async move { servicos.apagar_rascunho(&config) })
            .detach();
    }

    /// Depois da nota autorizada: formulario limpo para a proxima.
    pub(in crate::telas) fn nova_nota(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !matches!(self.fase, Fase::Concluida(_)) {
            return;
        }
        self.fase = Fase::Editando;
        self.limpar(window, cx);
    }

    /// Copia a chave de acesso da nota autorizada.
    pub(in crate::telas) fn copiar_chave(&mut self, cx: &mut Context<Self>) {
        if let Fase::Concluida(c) = &self.fase {
            cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(c.chave.clone()));
            self.copiada = true;
            cx.notify();
        }
    }
}
