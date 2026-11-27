// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O que a `Raiz` sabe das NF-e recebidas (as compras): a lista em Notas, o dialogo de buscar, baixar e responder, e a
//! configuracao que chega a eles. A escolha "Recebidas" do seletor mora com as outras em `raiz_nfe`.

use std::sync::Arc;

use gpui_kit::{AppContext as _, Context, Entity, Window};
use rustnf_app::config::Config;

use super::raiz::{Raiz, Secao, Tela};
use super::recebidas::{EventoRecebidas, TelaRecebidas};
use super::recebidas_dialogo::DialogoRecebidas;
use crate::servicos::Servicos;

/// As telas das NF-e recebidas.
pub(super) struct TelasRecebidas {
    pub(super) lista: Entity<TelaRecebidas>,
    pub(super) dialogo: Entity<DialogoRecebidas>,
}

impl Raiz {
    /// A lista de NF-e recebidas e o dialogo dela.
    pub(super) fn criar_telas_recebidas(
        servicos: &Arc<Servicos>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> TelasRecebidas {
        let s = Arc::clone(servicos);
        let dialogo = cx.new(|cx| DialogoRecebidas::new(s, window, cx));
        let (s, d) = (Arc::clone(servicos), dialogo.clone());
        let lista = cx.new(|cx| TelaRecebidas::new(s, d, window, cx));
        TelasRecebidas { lista, dialogo }
    }

    /// Liga a lista de recebidas a janela (chamar uma vez, depois de criar a janela).
    pub(super) fn ligar_recebidas(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let s = cx.subscribe_in(
            &self.recebidas.lista.clone(),
            window,
            |this, _, evento: &EventoRecebidas, _, cx| {
                let EventoRecebidas::IrParaConfiguracoes = evento;
                this.tela = Tela::Configuracoes;
                this.secao = Secao::Nfe;
                cx.notify();
            },
        );
        self.fixas.push(s);
    }

    /// Entrega a configuracao que vale a lista e ao dialogo das recebidas.
    pub(super) fn entregar_config_recebidas(
        &mut self,
        efetiva: &Config,
        pronta: bool,
        cx: &mut Context<Self>,
    ) {
        let e = efetiva.clone();
        self.recebidas
            .dialogo
            .update(cx, |d, cx| d.definir_config(e, cx));
        let e = efetiva.clone();
        self.recebidas
            .lista
            .update(cx, |l, cx| l.definir_config(e, pronta, cx));
    }

    /// Le a pasta das recebidas de novo (a pessoa escolheu "Recebidas").
    pub(super) fn recarregar_recebidas(&mut self, cx: &mut Context<Self>) {
        self.recebidas.lista.update(cx, |l, cx| l.recarregar(cx));
    }
}
