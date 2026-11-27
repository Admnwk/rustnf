// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Os produtos da Nova NF-e (as linhas de campos de texto, acrescentar, tirar, o cadastro) e o rascunho que sobrevive a
//! fechar a janela.

use std::collections::BTreeMap;
use std::sync::Arc;

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{AppContext as _, Context, Window};

use super::{EstadoDoRascunho, Fase, LinhaItem, NovaNfe, ESPERA_DO_RASCUNHO};
use crate::modelo::nfe_campos::CampoItem;
use crate::modelo::nfe_nova::ItemNfe;
use crate::modelo::nfe_produtos::Produto;
use crate::modelo::nfe_rascunho::Rascunho;

impl NovaNfe {
    // ---------------------------------------------------------------- itens

    fn novo_id(&mut self) -> u64 {
        self.proximo_id += 1;
        self.proximo_id
    }

    /// Cria a linha de tela de um item (os campos de texto com o que o item ja tem).
    fn nova_linha(
        &mut self,
        item: &ItemNfe,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> LinhaItem {
        let id = self.novo_id();
        let mut entradas = BTreeMap::new();
        let mut subs = Vec::new();
        for campo in CampoItem::TODOS {
            let texto = campo.texto(item);
            let e = cx.new(|cx| {
                let mut s = InputState::new(window, cx).placeholder(campo.dica());
                if !texto.is_empty() {
                    s.set_value(texto, window, cx);
                }
                s
            });
            subs.push(cx.subscribe_in(
                &e,
                window,
                move |this, entrada, evento: &InputEvent, window, cx| {
                    if matches!(evento, InputEvent::Change) {
                        let texto = entrada.read(cx).value().to_string();
                        this.ao_digitar_item(id, campo, &texto, window, cx);
                    }
                },
            ));
            entradas.insert(campo, e);
        }
        LinhaItem {
            id,
            entradas,
            detalhes: false,
            _subs: subs,
        }
    }

    /// Refaz as linhas da tela a partir dos itens dos dados.
    pub(super) fn reconstruir_linhas(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.linhas.clear();
        let itens = self.dados.itens.clone();
        for item in &itens {
            let linha = self.nova_linha(item, window, cx);
            self.linhas.push(linha);
        }
    }

    fn ao_digitar_item(
        &mut self,
        id: u64,
        campo: CampoItem,
        texto: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.travado() {
            return;
        }
        let Some(i) = self.linhas.iter().position(|l| l.id == id) else {
            return;
        };
        let Some(item) = self.dados.itens.get_mut(i) else {
            return;
        };
        let normal = campo.definir(item, texto);
        if normal != texto {
            if let Some(e) = self.linhas[i].entradas.get(&campo).cloned() {
                e.update(cx, |e, cx| e.set_value(normal, window, cx));
            }
        }
        self.mudou(cx);
    }

    /// Acrescenta um produto: o do cadastro, ou um em branco (produto novo).
    pub(super) fn adicionar_item(
        &mut self,
        item: ItemNfe,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.travado() {
            return;
        }
        let linha = self.nova_linha(&item, window, cx);
        self.dados.itens.push(item);
        self.linhas.push(linha);
        self.busca.update(cx, |e, cx| e.set_value("", window, cx));
        self.mudou(cx);
    }

    /// Acrescenta o produto do cadastro (quantidade 1).
    pub(super) fn adicionar_do_cadastro(
        &mut self,
        produto: &Produto,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut item = produto.para_item();
        // o regime e a origem do produto valem; o ICMS fixado so' vale se existe no regime de hoje
        if let Some(i) = item.icms {
            if i.codigo(self.ctx.crt).is_none() {
                item.icms = None;
            }
        }
        self.adicionar_item(item, window, cx);
    }

    /// Tira o produto da posicao `i`.
    pub(super) fn remover_item(&mut self, i: usize, cx: &mut Context<Self>) {
        if self.travado() || i >= self.dados.itens.len() {
            return;
        }
        self.dados.itens.remove(i);
        self.linhas.remove(i);
        self.mudou(cx);
    }

    /// Abre ou fecha os detalhes fiscais do produto.
    pub(super) fn alternar_detalhes(&mut self, i: usize, cx: &mut Context<Self>) {
        if let Some(l) = self.linhas.get_mut(i) {
            l.detalhes = !l.detalhes;
            cx.notify();
        }
    }

    // ---------------------------------------------------------------- rascunho

    pub(super) fn agendar_rascunho(&mut self, cx: &mut Context<Self>) {
        self.versao += 1;
        self.rascunho = if self.dados.vazio() {
            EstadoDoRascunho::Nenhum
        } else {
            EstadoDoRascunho::Alterado
        };
        let versao = self.versao;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(ESPERA_DO_RASCUNHO).await;
            this.update(cx, |this, cx| {
                if this.versao == versao {
                    this.gravar_rascunho(cx);
                }
            })
            .ok();
        })
        .detach();
    }

    /// Grava (ou apaga, se o assistente voltou a ficar vazio) o rascunho, fora da tela.
    fn gravar_rascunho(&mut self, cx: &mut Context<Self>) {
        if !self.pronta || !matches!(self.fase, Fase::Editando) {
            return;
        }
        let vazio = self.dados.vazio();
        let servicos = Arc::clone(&self.servicos);
        let config = self.config.clone();
        let rascunho = Rascunho {
            dados: self.dados.clone(),
            passo: self.passo,
        };
        let versao = self.versao;
        cx.spawn(async move |this, cx| {
            let ok = cx
                .background_spawn(async move {
                    if vazio {
                        servicos.apagar_rascunho_nfe(&config);
                        true
                    } else {
                        servicos.gravar_rascunho_nfe(&config, &rascunho).is_ok()
                    }
                })
                .await;
            this.update(cx, |this, cx| {
                if this.versao == versao {
                    this.rascunho = match (vazio, ok) {
                        (true, _) => EstadoDoRascunho::Nenhum,
                        (false, true) => EstadoDoRascunho::Salvo,
                        (false, false) => EstadoDoRascunho::Alterado,
                    };
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// A nota esta feita: o rascunho nao serve mais.
    pub(super) fn apagar_rascunho(&mut self, cx: &mut Context<Self>) {
        self.versao += 1;
        self.rascunho = EstadoDoRascunho::Nenhum;
        let servicos = Arc::clone(&self.servicos);
        let config = self.config.clone();
        cx.background_executor()
            .spawn(async move { servicos.apagar_rascunho_nfe(&config) })
            .detach();
    }
}
