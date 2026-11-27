// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A secao "NF-e (venda de mercadoria)" das Configuracoes: ambiente proprio da NF-e, emitente (UF, IE, regime, serie),
//! endereco, certificado compartilhado com a NFS-e, "Testar conexao com a SEFAZ", contingencia SVC, DANFE e o
//! avancado. Grava pelo mesmo Salvar (e o mesmo arquivo) da NFS-e.

use gpui_kit::component::button::{Button, Toggle, ToggleGroup};
use gpui_kit::component::form::{v_form, Field};
use gpui_kit::component::input::Input;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Disableable as _, IconName};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, Styled as _,
};

use super::pecas::{campo, faixa};
use super::raiz::Raiz;
use crate::modelo::certificado::Gravidade;
use crate::modelo::nfe_config::{rotulo_do_regime, CampoNfe, REGIMES, REGIMES_PIS_COFINS};

impl Raiz {
    /// Um campo de texto da secao, com o erro (ou a ajuda) embaixo.
    fn campo_nfe(
        &self,
        c: CampoNfe,
        rotulo: &'static str,
        obrigatorio: bool,
        ajuda: Option<&'static str>,
        id: &'static str,
    ) -> Field {
        let erro = self.form_nfe.erros().remove(&c);
        let controle = match self.entrada_nfe(c) {
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

    fn secao_nfe_ambiente(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let producao = self.form_nfe.dados.producao;
        let ambientes = ToggleGroup::new("nfe-ambiente")
            .segmented()
            .single()
            .children([false, true].map(|p| {
                Toggle::new(SharedString::from(format!("nfe-ambiente-{p}")))
                    .label(if p {
                        "Produção"
                    } else {
                        "Homologação (testes)"
                    })
                    .checked(producao == p)
            }))
            .on_click(cx.listener(|this, estados: &Vec<bool>, _, cx| {
                if let Some(i) = estados.iter().position(|e| *e) {
                    this.form_nfe.dados.producao = i == 1;
                    this.resultado_sefaz = None;
                }
                cx.notify();
            }));
        v_flex()
            .gap_3()
            .child(self.titulo_do_grupo(
                "Ambiente da NF-e",
                Some("É outro campo, separado do ambiente da NFS-e: cada documento tem o seu."),
                cx,
            ))
            .child(campo("Ambiente da SEFAZ", true, None, None, ambientes))
            .child(if producao {
                faixa(
                    Gravidade::Aviso,
                    "Em produção, as NF-e têm efeito fiscal: cancelar, corrigir e inutilizar não têm volta.",
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

    fn secao_nfe_emitente(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let crt = self.form_nfe.dados.crt;
        let pis = self.form_nfe.dados.regime_pis_cofins.clone();
        let regimes = ToggleGroup::new("nfe-regime")
            .segmented()
            .single()
            .children(REGIMES.iter().map(|(c, r)| {
                Toggle::new(SharedString::from(format!("nfe-regime-{c}")))
                    .label(*r)
                    .checked(crt == *c)
            }))
            .on_click(cx.listener(|this, estados: &Vec<bool>, _, cx| {
                if let Some((c, _)) = estados.iter().position(|e| *e).and_then(|i| REGIMES.get(i)) {
                    this.form_nfe.dados.crt = *c;
                }
                cx.notify();
            }));
        let pis_cofins = ToggleGroup::new("nfe-pis-cofins")
            .segmented()
            .single()
            .children(REGIMES_PIS_COFINS.iter().map(|(c, r)| {
                Toggle::new(SharedString::from(format!("nfe-pis-{c}")))
                    .label(*r)
                    .checked(pis == *c)
            }))
            .on_click(cx.listener(|this, estados: &Vec<bool>, _, cx| {
                if let Some((c, _)) = estados
                    .iter()
                    .position(|e| *e)
                    .and_then(|i| REGIMES_PIS_COFINS.get(i))
                {
                    this.form_nfe.dados.regime_pis_cofins = (*c).to_string();
                }
                cx.notify();
            }));
        v_flex()
            .gap_3()
            .child(self.titulo_do_grupo(
                "Emitente na NF-e",
                Some("O CNPJ e o nome são os da seção Emitente; aqui vai o que só a NF-e pede."),
                cx,
            ))
            .child(
                v_form()
                    .columns(2)
                    .child(self.campo_nfe(
                        CampoNfe::Uf,
                        "UF do emitente",
                        true,
                        Some("Sigla do estado: escolhe a SEFAZ que recebe as notas."),
                        "nfe-uf",
                    ))
                    .child(self.campo_nfe(
                        CampoNfe::Ie,
                        "Inscrição estadual",
                        true,
                        Some("Só dígitos. Obrigatória para emitir."),
                        "nfe-ie",
                    ))
                    .child(self.campo_nfe(
                        CampoNfe::Serie,
                        "Série",
                        true,
                        Some("De 0 a 999. A numeração é a da pasta de notas."),
                        "nfe-serie",
                    ))
                    .child(self.campo_nfe(
                        CampoNfe::Fantasia,
                        "Nome fantasia",
                        false,
                        None,
                        "nfe-fantasia",
                    ))
                    .child(
                        campo(
                            "Regime da empresa",
                            true,
                            Some(SharedString::from(format!(
                                "Hoje: {}. É o regime do seu CNPJ; o contador sabe.",
                                rotulo_do_regime(crt)
                            ))),
                            None,
                            regimes,
                        )
                        .col_span(2),
                    )
                    .when(crt == 3, |f| {
                        f.child(
                            campo(
                                "PIS e COFINS",
                                false,
                                Some("No regime normal. \"Não informar\" desliga só o aviso de CST que não combina."
                                    .into()),
                                None,
                                pis_cofins,
                            )
                            .col_span(2),
                        )
                    }),
            )
            .into_any_element()
    }

    fn secao_nfe_endereco(&mut self, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .gap_3()
            .child(self.titulo_do_grupo(
                "Endereço do emitente",
                Some(
                    "Vai no DANFE e na NF-e. O código do município vazio usa o da seção Emitente.",
                ),
                cx,
            ))
            .child(
                v_form()
                    .columns(2)
                    .child(self.campo_nfe(
                        CampoNfe::Logradouro,
                        "Rua",
                        false,
                        None,
                        "nfe-logradouro",
                    ))
                    .child(self.campo_nfe(CampoNfe::Numero, "Número", false, None, "nfe-numero"))
                    .child(self.campo_nfe(
                        CampoNfe::Complemento,
                        "Complemento",
                        false,
                        None,
                        "nfe-complemento",
                    ))
                    .child(self.campo_nfe(CampoNfe::Bairro, "Bairro", false, None, "nfe-bairro"))
                    .child(self.campo_nfe(
                        CampoNfe::Municipio,
                        "Município",
                        false,
                        None,
                        "nfe-municipio",
                    ))
                    .child(self.campo_nfe(
                        CampoNfe::CodMunicipio,
                        "Código IBGE do município",
                        false,
                        Some("7 dígitos."),
                        "nfe-cod-municipio",
                    ))
                    .child(self.campo_nfe(CampoNfe::Cep, "CEP", false, None, "nfe-cep"))
                    .child(self.campo_nfe(CampoNfe::Fone, "Telefone", false, None, "nfe-fone")),
            )
            .into_any_element()
    }

    fn secao_nfe_conexao(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let ocupado = self.ocupado.sefaz;
        let resultado = self.resultado_sefaz.clone();
        let cert = self.cert.texto();
        v_flex()
            .gap_3()
            .child(self.titulo_do_grupo(
                "Certificado e conexão",
                Some("O certificado A1 é o MESMO da NFS-e: o arquivo e a senha guardada no cofre são os da seção Sefin Nacional."),
                cx,
            ))
            .child(
                div()
                    .id("nfe-certificado-compartilhado")
                    .text_sm()
                    .child(format!("Certificado em uso: {cert}")),
            )
            .child(
                h_flex().child(
                    Button::new("nfe-testar-conexao")
                        .label("Testar conexão com a SEFAZ")
                        .icon(IconName::Globe)
                        .outline()
                        .loading(ocupado)
                        .disabled(ocupado)
                        .on_click(cx.listener(|this, _, window, cx| this.testar_sefaz(window, cx))),
                ),
            )
            .children(resultado.map(|r| {
                faixa(
                    if r.ok() { Gravidade::Ok } else { Gravidade::Erro },
                    r.texto(),
                    cx,
                )
                .id("nfe-resultado-da-conexao")
            }))
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("O teste só pergunta se o serviço da SEFAZ está no ar, com o certificado e no ambiente escolhido acima. Nenhuma nota é emitida."),
            )
            .into_any_element()
    }

    fn secao_nfe_contingencia(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let svc = self.form_nfe.dados.svc;
        let canhoto = self.form_nfe.dados.canhoto;
        v_flex()
            .gap_3()
            .child(self.titulo_do_grupo(
                "Contingência (SVC)",
                Some("Use só quando a SEFAZ do seu estado estiver fora do ar. Desligue quando ela voltar."),
                cx,
            ))
            .child(
                Switch::new("nfe-svc")
                    .checked(svc)
                    .label("Emitir pela SEFAZ Virtual de Contingência (SVC)")
                    .on_click(cx.listener(|this, marcado: &bool, _, cx| {
                        this.form_nfe.dados.svc = *marcado;
                        this.resultado_sefaz = None;
                        cx.notify();
                    })),
            )
            .when(svc, |d| {
                d.child(self.campo_nfe(
                    CampoNfe::MotivoContingencia,
                    "Motivo da contingência",
                    true,
                    Some("De 15 a 255 caracteres. Vai registrado na nota."),
                    "nfe-motivo-contingencia",
                ))
            })
            .child(
                Switch::new("nfe-canhoto")
                    .checked(canhoto)
                    .label("Canhoto de recebimento no DANFE")
                    .on_click(cx.listener(|this, marcado: &bool, _, cx| {
                        this.form_nfe.dados.canhoto = *marcado;
                        cx.notify();
                    })),
            )
            .into_any_element()
    }

    fn secao_nfe_avancado(&mut self, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .gap_3()
            .child(self.titulo_do_grupo("Avançado", None, cx))
            .child(
                v_form()
                    .columns(3)
                    .child(self.campo_nfe(
                        CampoNfe::Tempo,
                        "Tempo limite (segundos)",
                        false,
                        Some("De 1 a 600."),
                        "nfe-tempo",
                    ))
                    .child(self.campo_nfe(
                        CampoNfe::Tentativas,
                        "Consultas do recibo",
                        false,
                        Some("De 1 a 60."),
                        "nfe-tentativas",
                    ))
                    .child(self.campo_nfe(
                        CampoNfe::Intervalo,
                        "Espera entre elas (s)",
                        false,
                        Some("No mínimo 1."),
                        "nfe-intervalo",
                    )),
            )
            .into_any_element()
    }

    /// A secao NF-e das Configuracoes.
    pub(super) fn secao_nfe(&mut self, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .id("nfe-configuracoes")
            .gap_6()
            .child(self.secao_nfe_ambiente(cx))
            .child(self.secao_nfe_emitente(cx))
            .child(self.secao_nfe_endereco(cx))
            .child(self.secao_nfe_conexao(cx))
            .child(self.secao_nfe_contingencia(cx))
            .child(self.secao_nfe_avancado(cx))
            .into_any_element()
    }
}
