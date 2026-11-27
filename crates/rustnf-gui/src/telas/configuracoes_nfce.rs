// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A secao "NFC-e (cupom do caixa)" das Configuracoes: ambiente proprio da NFC-e, serie, o `idCSC` e o CSC (campo de senha que
//! grava no cofre; a tela so' diz "guardado" ou "nao guardado", nunca o valor), a versao do QR Code, o cupom (largura da bobina
//! e colunas do ESC/POS) e o motivo da contingencia off-line. Grava pelo mesmo Salvar (e o mesmo arquivo) das outras secoes.

use gpui_kit::component::button::{Button, ButtonVariants as _, Toggle, ToggleGroup};
use gpui_kit::component::form::{v_form, Field};
use gpui_kit::component::input::Input;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Disableable as _, IconName};
use gpui_kit::{
    div, AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, Styled as _,
};

use super::pecas::{campo, faixa};
use super::raiz::Raiz;
use crate::modelo::certificado::Gravidade;
use crate::modelo::nfce_config::{CampoNfce, EstadoCsc, COLUNAS, LARGURAS, VERSOES_DO_QR};

impl Raiz {
    fn campo_nfce(
        &self,
        c: CampoNfce,
        rotulo: &'static str,
        obrigatorio: bool,
        ajuda: Option<&'static str>,
        id: &'static str,
    ) -> Field {
        let erro = self.form_nfce.erros().remove(&c);
        let controle = match self.entrada_nfce(c) {
            Some(e) => Input::new(e).id(id).into_any_element(),
            None => div().into_any_element(),
        };
        campo(
            rotulo,
            obrigatorio,
            ajuda.map(SharedString::from),
            erro,
            controle,
        )
    }

    fn secao_nfce_ambiente(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let producao = self.form_nfce.dados.producao;
        let ambientes = ToggleGroup::new("nfce-ambiente")
            .segmented()
            .single()
            .children([false, true].map(|p| {
                Toggle::new(SharedString::from(format!("nfce-ambiente-{p}")))
                    .label(if p {
                        "Produção"
                    } else {
                        "Homologação (testes)"
                    })
                    .checked(producao == p)
            }))
            .on_click(cx.listener(|this, estados: &Vec<bool>, _, cx| {
                if let Some(i) = estados.iter().position(|e| *e) {
                    this.form_nfce.dados.producao = i == 1;
                }
                cx.notify();
            }));
        v_flex()
            .gap_3()
            .child(self.titulo_do_grupo(
                "Ambiente e série do cupom",
                Some("O cupom tem o ambiente e a série dele, separados dos da NF-e e da NFS-e. O emitente, o certificado e a pasta de notas são os mesmos."),
                cx,
            ))
            .child(
                v_form()
                    .columns(2)
                    .child(campo("Ambiente da SEFAZ", true, None, None, ambientes))
                    .child(self.campo_nfce(
                        CampoNfce::Serie,
                        "Série",
                        true,
                        Some("De 0 a 999. A numeração do cupom é a da pasta de notas."),
                        "nfce-serie",
                    )),
            )
            .child(if producao {
                faixa(
                    Gravidade::Aviso,
                    "Em produção, os cupons têm efeito fiscal. O caixa pede uma marcação antes da primeira venda da sessão.",
                    cx,
                )
            } else {
                faixa(
                    Gravidade::Ok,
                    "Homologação: ambiente de testes da SEFAZ, sem efeito fiscal.",
                    cx,
                )
            })
            .into_any_element()
    }

    fn secao_nfce_csc(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let qr = self.form_nfce.dados.qr_versao;
        let versoes = ToggleGroup::new("nfce-qr")
            .segmented()
            .single()
            .children(VERSOES_DO_QR.iter().map(|(v, r)| {
                Toggle::new(SharedString::from(format!("nfce-qr-{v}")))
                    .label(*r)
                    .checked(qr == *v)
            }))
            .on_click(cx.listener(|this, estados: &Vec<bool>, _, cx| {
                if let Some((v, _)) = estados
                    .iter()
                    .position(|e| *e)
                    .and_then(|i| VERSOES_DO_QR.get(i))
                {
                    this.form_nfce.dados.qr_versao = *v;
                }
                cx.notify();
            }));
        let ocupado = self.ocupado.csc;
        let estado = self.csc;
        let resultado = self.resultado_csc.clone();
        let gravidade = match estado {
            EstadoCsc::Guardado => Gravidade::Ok,
            EstadoCsc::NaoGuardado | EstadoCsc::SemIdCsc | EstadoCsc::Verificando => {
                Gravidade::Aviso
            }
            EstadoCsc::SemCofre | EstadoCsc::CofreFora => Gravidade::Erro,
        };
        let sem_cofre = estado == EstadoCsc::SemCofre;
        v_flex()
            .gap_3()
            .child(self.titulo_do_grupo(
                "QR Code e CSC",
                Some("O QR Code versão 3 (padrão) não usa CSC. A versão 2 usa o CSC, um código secreto que a SEFAZ da sua UF dá ao contribuinte."),
                cx,
            ))
            .child(
                v_form()
                    .columns(2)
                    .child(campo("Versão do QR Code", true, None, None, versoes).col_span(2))
                    .child(self.campo_nfce(
                        CampoNfce::IdCsc,
                        "idCSC (número do CSC)",
                        qr == 2,
                        Some("Até 6 dígitos. Vem junto com o CSC, no portal da SEFAZ da sua UF (área do contribuinte, NFC-e). É público: vai na nota."),
                        "nfce-id-csc",
                    )),
            )
            .child(faixa(gravidade, estado.texto(), cx).id("nfce-estado-do-csc"))
            .child(
                h_flex()
                    .gap_2()
                    .items_end()
                    .child(
                        div().flex_1().child(
                            campo(
                                "CSC",
                                false,
                                Some(SharedString::from(
                                    "Cole o CSC e aperte \"Guardar no cofre\". Ele nunca aparece na tela, nem no arquivo de configuração.",
                                )),
                                None,
                                Input::new(&self.entradas_nfce.csc)
                                    .id("nfce-csc")
                                    .disabled(sem_cofre),
                            ),
                        ),
                    )
                    .child(
                        Button::new("nfce-guardar-csc")
                            .label("Guardar no cofre")
                            .icon(IconName::Check)
                            .primary()
                            .loading(ocupado)
                            .disabled(ocupado || sem_cofre)
                            .on_click(cx.listener(|this, _, window, cx| this.guardar_csc(window, cx))),
                    )
                    .child(
                        Button::new("nfce-apagar-csc")
                            .label("Apagar")
                            .icon(IconName::Delete)
                            .outline()
                            .disabled(ocupado || estado != EstadoCsc::Guardado)
                            .on_click(cx.listener(|this, _, window, cx| this.apagar_csc(window, cx))),
                    ),
            )
            .children(resultado.map(|(ok, t)| {
                faixa(if ok { Gravidade::Ok } else { Gravidade::Erro }, t, cx).id("nfce-resultado-do-csc")
            }))
            .child(
                div().text_xs().text_color(cx.theme().muted_foreground).child(
                    "O CSC é guardado no cofre de senhas do Windows, para este emitente, este ambiente e este idCSC. Trocar de ambiente ou de idCSC não enxerga o CSC do outro. Salve as configurações para o caixa usar o idCSC novo.",
                ),
            )
            .into_any_element()
    }

    fn secao_nfce_cupom(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let (largura, colunas) = (self.form_nfce.dados.largura, self.form_nfce.dados.colunas);
        let larguras = ToggleGroup::new("nfce-largura")
            .segmented()
            .single()
            .children(LARGURAS.iter().map(|(v, r)| {
                Toggle::new(SharedString::from(format!("nfce-largura-{v}")))
                    .label(*r)
                    .checked(largura == *v)
            }))
            .on_click(cx.listener(|this, estados: &Vec<bool>, _, cx| {
                if let Some((v, _)) = estados
                    .iter()
                    .position(|e| *e)
                    .and_then(|i| LARGURAS.get(i))
                {
                    this.form_nfce.dados.largura = *v;
                }
                cx.notify();
            }));
        let colunas_ui = ToggleGroup::new("nfce-colunas")
            .segmented()
            .single()
            .children(COLUNAS.iter().map(|(v, r)| {
                Toggle::new(SharedString::from(format!("nfce-colunas-{v}")))
                    .label(*r)
                    .checked(colunas == *v)
            }))
            .on_click(cx.listener(|this, estados: &Vec<bool>, _, cx| {
                if let Some((v, _)) = estados.iter().position(|e| *e).and_then(|i| COLUNAS.get(i)) {
                    this.form_nfce.dados.colunas = *v;
                }
                cx.notify();
            }));
        v_flex()
            .gap_3()
            .child(self.titulo_do_grupo(
                "Cupom e contingência",
                Some("O programa gera o arquivo do cupom (PDF de bobina ou ESC/POS); ele não fala com a impressora."),
                cx,
            ))
            .child(
                v_form()
                    .columns(2)
                    .child(campo("Largura da bobina", true, None, None, larguras))
                    .child(campo(
                        "Colunas do ESC/POS",
                        false,
                        Some("Automático escolhe pela largura da bobina.".into()),
                        None,
                        colunas_ui,
                    ))
                    .child(
                        self.campo_nfce(
                            CampoNfce::Motivo,
                            "Motivo da contingência off-line",
                            false,
                            Some("De 15 a 255 letras. Vai registrado na nota emitida sem internet; vazio usa o texto padrão."),
                            "nfce-motivo",
                        )
                        .col_span(2),
                    ),
            )
            .into_any_element()
    }

    /// A secao NFC-e das Configuracoes.
    pub(super) fn secao_nfce(&mut self, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .id("nfce-configuracoes")
            .gap_6()
            .child(self.secao_nfce_ambiente(cx))
            .child(self.secao_nfce_csc(cx))
            .child(self.secao_nfce_cupom(cx))
            .into_any_element()
    }
}
