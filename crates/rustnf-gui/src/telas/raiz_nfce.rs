// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O que a `Raiz` sabe da NFC-e: o caixa, a lista de NFC-e em Notas, os campos da secao NFC-e das Configuracoes (que gravam
//! pela mesma via das outras secoes, `Servicos::salvar`, num arquivo so') e o CSC no cofre (guardar, ver se esta guardado,
//! apagar: nunca mostrar). O valor do CSC mora so' no campo de senha da tela, no `Segredo` do momento de guardar e no cofre.

use std::collections::BTreeMap;
use std::sync::Arc;

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::WindowExt as _;
use gpui_kit::{AppContext as _, Context, Entity, Window};
use rustnf_app::config::Config;
use rustnf_app::segredo::Segredo;

use super::caixa::{EventoCaixa, TelaCaixa};
use super::nfe::{EventoNfe, TelaNfe};
use super::nfe_dialogo::DialogoNfe;
use super::raiz::{Raiz, Secao, Tela};
use crate::modelo::ambiente::Ambiente;
use crate::modelo::nfce_config::{ambiente_da_nfce, CampoNfce, EstadoCsc};
use crate::modelo::nfe_lista::EscolhaDeNotas;
use crate::servicos::{Documento, Servicos};

/// Os campos de texto da secao NFC-e e o campo do CSC (de senha: nunca mostra o que foi digitado).
pub(super) struct EntradasNfce {
    pub(super) por_campo: BTreeMap<CampoNfce, Entity<InputState>>,
    pub(super) csc: Entity<InputState>,
}

/// As telas da NFC-e.
pub(super) struct TelasNfce {
    pub(super) caixa: Entity<TelaCaixa>,
    pub(super) lista: Entity<TelaNfe>,
    pub(super) dialogo: Entity<DialogoNfe>,
}

fn dica(c: CampoNfce) -> &'static str {
    match c {
        CampoNfce::Serie => "1",
        CampoNfce::IdCsc => "Ex.: 000001",
        CampoNfce::Motivo => "Vazio: o texto padrão",
    }
}

impl Raiz {
    /// O caixa, a lista de NFC-e e o dialogo dela.
    pub(super) fn criar_telas_nfce(
        servicos: &Arc<Servicos>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> TelasNfce {
        let s = Arc::clone(servicos);
        let dialogo = cx.new(|cx| DialogoNfe::new(s, Documento::Nfce, window, cx));
        let (s1, d1) = (Arc::clone(servicos), dialogo.clone());
        let lista = cx.new(|cx| TelaNfe::new(s1, d1, Documento::Nfce, window, cx));
        let (s2, d2) = (Arc::clone(servicos), dialogo.clone());
        let caixa = cx.new(|cx| TelaCaixa::new(s2, d2, window, cx));
        TelasNfce {
            caixa,
            lista,
            dialogo,
        }
    }

    /// Os campos da secao NFC-e, ainda vazios.
    pub(super) fn criar_entradas_nfce(window: &mut Window, cx: &mut Context<Self>) -> EntradasNfce {
        let por_campo = CampoNfce::TODOS
            .into_iter()
            .map(|c| {
                (
                    c,
                    cx.new(|cx| InputState::new(window, cx).placeholder(dica(c))),
                )
            })
            .collect();
        let csc = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Cole aqui o CSC da SEFAZ")
                .masked(true)
        });
        EntradasNfce { por_campo, csc }
    }

    /// O campo de texto da secao NFC-e.
    pub(super) fn entrada_nfce(&self, c: CampoNfce) -> Option<&Entity<InputState>> {
        self.entradas_nfce.por_campo.get(&c)
    }

    /// Liga os campos da secao NFC-e, o caixa e a lista (chamar uma vez, depois de criar a janela).
    pub(super) fn ligar_nfce(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let pares: Vec<(CampoNfce, Entity<InputState>)> = self
            .entradas_nfce
            .por_campo
            .iter()
            .map(|(c, e)| (*c, e.clone()))
            .collect();
        for (campo, e) in pares {
            let s = cx.subscribe_in(
                &e,
                window,
                move |this, entrada, evento: &InputEvent, window, cx| {
                    if matches!(evento, InputEvent::Change) {
                        let texto = entrada.read(cx).value().to_string();
                        let normal = this.form_nfce.definir(campo, &texto);
                        if normal != texto {
                            entrada.update(cx, |estado, cx| estado.set_value(normal, window, cx));
                        }
                        this.resultado_csc = None;
                        cx.notify();
                    }
                },
            );
            self.fixas.push(s);
        }
        let s = cx.subscribe_in(
            &self.caixa.clone(),
            window,
            |this, _, evento: &EventoCaixa, _, cx| {
                match evento {
                    EventoCaixa::IrParaConfiguracoes => {
                        this.tela = Tela::Configuracoes;
                        this.secao = Secao::Nfce;
                    }
                    EventoCaixa::NotaMudou => this.nfce.update(cx, |n, cx| n.recarregar(cx)),
                    EventoCaixa::IrParaNotas => {
                        this.escolher_notas(EscolhaDeNotas::Nfce, cx);
                        this.ir_para(Tela::Notas, cx);
                    }
                }
                cx.notify();
            },
        );
        self.fixas.push(s);
        let s = cx.subscribe_in(
            &self.nfce.clone(),
            window,
            |this, _, evento: &EventoNfe, _, cx| {
                if matches!(evento, EventoNfe::IrParaConfiguracoes) {
                    this.tela = Tela::Configuracoes;
                    this.secao = Secao::Nfce;
                    cx.notify();
                }
            },
        );
        self.fixas.push(s);
    }

    /// Poe nos campos da tela o texto do formulario da NFC-e (sem disparar `Change` de volta).
    pub(super) fn sincronizar_nfce(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for c in CampoNfce::TODOS {
            let texto = self.form_nfce.dados.texto(c).to_string();
            if let Some(e) = self.entrada_nfce(c) {
                e.update(cx, |estado, cx| estado.set_value(texto, window, cx));
            }
        }
        self.entradas_nfce
            .csc
            .update(cx, |estado, cx| estado.set_value("", window, cx));
        self.resultado_csc = None;
    }

    /// Entrega a configuracao que vale ao caixa, a lista de NFC-e e ao dialogo dela.
    pub(super) fn entregar_config_nfce(
        &mut self,
        efetiva: &Config,
        pronta: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let e = efetiva.clone();
        self.dialogo_nfce
            .update(cx, |d, cx| d.definir_config(e, cx));
        let e = efetiva.clone();
        self.nfce
            .update(cx, |n, cx| n.definir_config(e, pronta, cx));
        let e = efetiva.clone();
        self.caixa
            .update(cx, |c, cx| c.definir_config(e, pronta, window, cx));
        self.atualizar_csc(efetiva.clone(), cx);
    }

    /// O que as telas da NFC-e fazem ao abrir.
    pub(super) fn ao_abrir_tela_nfce(&mut self, tela: Tela, cx: &mut Context<Self>) {
        if tela == Tela::Caixa {
            self.caixa.update(cx, |c, cx| {
                c.atualizar_o_contexto(cx);
                c.recarregar_pendentes(cx);
            });
        }
    }

    /// O ambiente da NFC-e em vigor (arquivo com as variaveis por cima).
    pub(super) fn ambiente_da_nfce_em_vigor(&self) -> Ambiente {
        ambiente_da_nfce(&self.servicos.efetiva(&self.salvo).nfce)
    }

    /// A tela Notas esta mostrando a NFC-e.
    pub(super) fn mostrando_a_nfce(&self) -> bool {
        (self.tela == Tela::Notas && self.escolha_notas == EscolhaDeNotas::Nfce)
            || self.tela == Tela::Caixa
            || (self.tela == Tela::NovaNota && self.escolha_notas == EscolhaDeNotas::Nfce)
    }

    // ---------------------------------------------------------------- o CSC

    /// Le fora da tela se o CSC esta guardado (nunca o valor).
    pub(super) fn atualizar_csc(&mut self, efetiva: Config, cx: &mut Context<Self>) {
        let servicos = Arc::clone(&self.servicos);
        cx.spawn(async move |this, cx| {
            let estado = cx
                .background_spawn(async move { servicos.estado_do_csc(&efetiva) })
                .await;
            this.update(cx, |this, cx| {
                this.csc = estado;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// A configuracao do formulario (o que a pessoa ve), com as variaveis por cima, para saber onde o CSC mora no cofre.
    fn config_do_csc(&self) -> Config {
        self.servicos.efetiva(&self.config_para_gravar())
    }

    /// "Guardar no cofre": confere o CSC e o guarda; o campo e' esvaziado de qualquer jeito.
    pub(super) fn guardar_csc(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.ocupado.csc {
            return;
        }
        let texto = self.entradas_nfce.csc.read(cx).value().to_string();
        let config = self.config_do_csc();
        let servicos = Arc::clone(&self.servicos);
        self.ocupado.csc = true;
        self.resultado_csc = None;
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let segredo = Segredo::novo(texto);
            let c2 = config.clone();
            let r = cx
                .background_spawn(async move { servicos.guardar_csc(&c2, &segredo) })
                .await;
            this.update_in(cx, |this, window, cx| {
                this.ocupado.csc = false;
                this.entradas_nfce
                    .csc
                    .update(cx, |e, cx| e.set_value("", window, cx));
                match r {
                    Ok(()) => {
                        this.resultado_csc =
                            Some((true, "CSC guardado no cofre deste computador.".into()));
                        window.push_notification(
                            Notification::success("O CSC foi guardado no cofre.")
                                .title("CSC guardado"),
                            cx,
                        );
                        this.csc = EstadoCsc::Guardado;
                    }
                    Err(e) => this.resultado_csc = Some((false, e.texto())),
                }
                let efetiva = this.servicos.efetiva(&this.salvo);
                this.atualizar_csc(efetiva, cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// "Apagar do cofre".
    pub(super) fn apagar_csc(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.ocupado.csc {
            return;
        }
        let config = self.config_do_csc();
        let servicos = Arc::clone(&self.servicos);
        self.ocupado.csc = true;
        self.resultado_csc = None;
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let r = cx
                .background_spawn(async move { servicos.apagar_csc(&config) })
                .await;
            this.update_in(cx, |this, window, cx| {
                this.ocupado.csc = false;
                match r {
                    Ok(()) => {
                        this.resultado_csc = Some((true, "CSC apagado do cofre.".into()));
                        window.push_notification(
                            Notification::info("O CSC foi apagado do cofre."),
                            cx,
                        );
                    }
                    Err(e) => this.resultado_csc = Some((false, e.texto())),
                }
                let efetiva = this.servicos.efetiva(&this.salvo);
                this.atualizar_csc(efetiva, cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}
