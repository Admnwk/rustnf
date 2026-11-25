// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O desenho da Nova nota: a coluna do formulario, em secoes, e a pagina inteira com o painel ao lado.

use gpui_kit::component::button::{Button, ButtonVariants as _, Toggle, ToggleGroup};
use gpui_kit::component::form::{v_form, Field};
use gpui_kit::component::input::{Input, Textarea};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{
    h_flex, v_flex, ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement as _, Render,
    SharedString, StatefulInteractiveElement as _, Styled as _, Window,
};

use super::{Etapa, Fase, NovaNota};
use crate::modelo::certificado::Gravidade;
use crate::modelo::mascara::{documento_para_resumo, rotulo_do_municipio, RotuloMunicipio};
use crate::modelo::nota::{
    CampoNota, SecaoNota, OPCOES_CONSUMIDOR_FINAL, OPCOES_RETENCAO, OPCOES_TRIBUTACAO,
};
use crate::modelo::sugestao::{
    descricao_da_classe, descricao_do_cind_op, descricao_do_nbs, descricao_do_servico, EstadoDoItem,
};
use crate::telas::pecas::{cabecalho, campo, cartao, faixa, grupo, Negrito as _, LARGURA_FORM};
use crate::telas::{LARGURA_DA_LATERAL, LARGURA_PARA_PAINEL_AO_LADO};

/// Texto de ajuda do municipio: nome e UF, ou o que falta.
fn ajuda_do_municipio(codigo: &str, vazio: Option<String>) -> Option<SharedString> {
    match rotulo_do_municipio(codigo) {
        RotuloMunicipio::Vazio => vazio.map(SharedString::from),
        outro => outro.texto().map(SharedString::from),
    }
}

impl NovaNota {
    /// Um campo de uma linha, com o id dado, desligado enquanto o formulario esta travado.
    fn entrada(&self, campo_da_tela: CampoNota, id: &'static str, direita: bool) -> AnyElement {
        match self.entradas.get(&campo_da_tela) {
            Some(e) => {
                let i = Input::new(e).id(id).disabled(self.travado());
                if direita {
                    i.text_right().into_any_element()
                } else {
                    i.into_any_element()
                }
            }
            None => div().into_any_element(),
        }
    }

    /// Um campo completo: rotulo, entrada, ajuda e o erro (so' depois de tocado ou de tentar emitir).
    fn campo_de(
        &self,
        rotulo: &str,
        c: CampoNota,
        id: &'static str,
        obrigatorio: bool,
        ajuda: Option<SharedString>,
        direita: bool,
    ) -> Field {
        campo(
            rotulo.to_string(),
            obrigatorio,
            ajuda,
            self.erro_do(c),
            self.entrada(c, id, direita),
        )
    }

    fn cartao_de_secao(
        &self,
        secao: SecaoNota,
        apoio: Option<&str>,
        cx: &Context<Self>,
    ) -> gpui_kit::Stateful<gpui_kit::Div> {
        cartao(cx)
            .id(SharedString::from(format!("secao-{}", secao.titulo())))
            .child(grupo(secao.titulo(), apoio, cx))
    }

    // ---------------------------------------------------------------- topo

    fn resumo_do_emitente(&self, cx: &Context<Self>) -> AnyElement {
        let tema = cx.theme();
        let e = &self.ctx.emitente;
        let documento = documento_para_resumo(self.ctx.documento_do_emitente());
        let municipio = match rotulo_do_municipio(&e.municipio) {
            RotuloMunicipio::Encontrado { nome, uf } => format!("{nome} - {uf}"),
            _ => e.municipio.clone(),
        };
        let regime = match e.op_simples {
            2 => "MEI",
            3 => "ME/EPP",
            _ => "Não optante pelo Simples",
        };
        let item = |rotulo: &str, valor: String| {
            h_flex()
                .gap_1p5()
                .child(
                    div()
                        .text_xs()
                        .text_color(tema.muted_foreground)
                        .child(rotulo.to_string()),
                )
                .child(div().text_xs().font_medium().child(valor))
        };
        h_flex()
            .id("resumo-do-emitente")
            .flex_wrap()
            .gap_x_5()
            .gap_y_1()
            .px_4()
            .py_3()
            .rounded_lg()
            .border_1()
            .border_color(tema.border)
            .bg(tema.muted.opacity(0.5))
            .child(item("Prestador", e.nome.clone()))
            .child(item("Documento", documento))
            .child(item("Município", municipio))
            .child(item("Regime", regime.to_string()))
            .child(item(
                "Numeração",
                format!(
                    "DPS nº {}, série {}",
                    self.numero_provisorio,
                    self.ctx.serie()
                ),
            ))
            .into_any_element()
    }

    // ---------------------------------------------------------------- tomador

    fn secao_tomador(&self, cx: &mut Context<Self>) -> AnyElement {
        let sem = self.dados.sem_tomador;
        let travado = self.travado();
        let mut cartao = self
            .cartao_de_secao(SecaoNota::Tomador, Some("Quem recebe o serviço."), cx)
            .child(
                Switch::new("sem-tomador")
                    .checked(sem)
                    .label("Nota sem tomador identificado")
                    .disabled(travado)
                    .on_click(cx.listener(|this, marcado: &bool, _, cx| {
                        this.escolher_sem_tomador(*marcado, cx);
                    })),
            );
        if sem {
            return cartao
                .child(
                    div()
                        .id("sem-tomador-explicacao")
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child("A nota sairá sem identificar o tomador. Com o ISS retido pelo tomador isso não é possível."),
                )
                .into_any_element();
        }
        let documento = self.dados.documento_do_tomador();
        let ajuda_documento = match documento.len() {
            11 => Some(SharedString::from("CPF")),
            14 => Some(SharedString::from("CNPJ")),
            _ => Some(SharedString::from(
                "Só os números; a máscara aparece sozinha.",
            )),
        };
        cartao = cartao.child(
            v_form()
                .columns(2)
                .child(
                    self.campo_de(
                        "CNPJ ou CPF",
                        CampoNota::TomadorDocumento,
                        "nota-tomador-documento",
                        true,
                        ajuda_documento,
                        false,
                    )
                    .col_span(1),
                )
                .child(
                    self.campo_de(
                        "Nome ou razão social",
                        CampoNota::TomadorNome,
                        "nota-tomador-nome",
                        true,
                        None,
                        false,
                    )
                    .col_span(1),
                )
                .child(
                    self.campo_de(
                        "Inscrição municipal",
                        CampoNota::TomadorIm,
                        "nota-tomador-im",
                        false,
                        None,
                        false,
                    )
                    .col_span(1),
                )
                .child(
                    self.campo_de(
                        "E-mail",
                        CampoNota::TomadorEmail,
                        "nota-tomador-email",
                        false,
                        None,
                        false,
                    )
                    .col_span(1),
                )
                .child(
                    self.campo_de(
                        "Telefone",
                        CampoNota::TomadorFone,
                        "nota-tomador-fone",
                        false,
                        None,
                        false,
                    )
                    .col_span(1),
                ),
        );
        if let Some(aviso) = self.tomador_preenchido.clone() {
            cartao = cartao.child(faixa(Gravidade::Ok, aviso, cx).id("tomador-preenchido"));
        }
        let municipio = ajuda_do_municipio(
            &self.dados.end_municipio,
            Some("Código IBGE de 7 dígitos.".into()),
        )
        .map(|t| {
            if matches!(
                rotulo_do_municipio(&self.dados.end_municipio),
                RotuloMunicipio::Encontrado { .. }
            ) {
                SharedString::from(format!("Município: {t}"))
            } else {
                t
            }
        });
        cartao
            .child(
                div()
                    .text_xs()
                    .font_semibold()
                    .text_color(cx.theme().muted_foreground)
                    .child("ENDEREÇO (OPCIONAL, MAS COMPLETO SE INFORMADO)"),
            )
            .child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .child(
                        Button::new("buscar-cep")
                            .label("Buscar endereço pelo CEP")
                            .icon(IconName::Search)
                            .outline()
                            .small()
                            .loading(self.buscando_cep)
                            .disabled(self.travado() || self.buscando_cep)
                            .on_click(
                                cx.listener(|this, _, window, cx| this.buscar_cep(window, cx)),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(match &self
                            .aviso_cep
                        {
                            Some(a) => a.clone(),
                            None => {
                                "O CEP é enviado ao ViaCEP (ou à BrasilAPI) só quando você clica."
                                    .to_string()
                            }
                        }),
                    ),
            )
            .child(
                v_form()
                    .columns(2)
                    .child(
                        self.campo_de(
                            "CEP",
                            CampoNota::EnderecoCep,
                            "nota-end-cep",
                            false,
                            None,
                            false,
                        )
                        .col_span(1),
                    )
                    .child(
                        self.campo_de(
                            "Município (código IBGE)",
                            CampoNota::EnderecoMunicipio,
                            "nota-end-municipio",
                            false,
                            municipio,
                            false,
                        )
                        .col_span(1),
                    )
                    .child(
                        self.campo_de(
                            "Logradouro",
                            CampoNota::EnderecoLogradouro,
                            "nota-end-logradouro",
                            false,
                            None,
                            false,
                        )
                        .col_span(2),
                    )
                    .child(
                        self.campo_de(
                            "Número",
                            CampoNota::EnderecoNumero,
                            "nota-end-numero",
                            false,
                            None,
                            false,
                        )
                        .col_span(1),
                    )
                    .child(
                        self.campo_de(
                            "Complemento",
                            CampoNota::EnderecoComplemento,
                            "nota-end-complemento",
                            false,
                            None,
                            false,
                        )
                        .col_span(1),
                    )
                    .child(
                        self.campo_de(
                            "Bairro",
                            CampoNota::EnderecoBairro,
                            "nota-end-bairro",
                            false,
                            None,
                            false,
                        )
                        .col_span(2),
                    ),
            )
            .into_any_element()
    }

    // ---------------------------------------------------------------- servico

    fn lista_de_servicos(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.resultados.is_empty() {
            return None;
        }
        let tema = cx.theme();
        let (borda, muted, hover) = (tema.border, tema.muted_foreground, tema.accent);
        let mut lista = v_flex()
            .id("resultados-da-busca")
            .max_h(px(260.))
            .overflow_y_scroll()
            .rounded_md()
            .border_1()
            .border_color(borda);
        for (i, s) in self.resultados.iter().enumerate() {
            let codigo = s.codigo.clone();
            lista = lista.child(
                h_flex()
                    .id(SharedString::from(format!("servico-{i}")))
                    .gap_3()
                    .items_start()
                    .px_3()
                    .py_2()
                    .cursor_pointer()
                    .hover(move |st| st.bg(hover))
                    .child(
                        div()
                            .w(px(64.))
                            .flex_shrink_0()
                            .text_sm()
                            .font_semibold()
                            .child(s.mascarado.clone()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_sm()
                            .text_color(muted)
                            .child(s.descricao.clone()),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.escolher_servico(&codigo, window, cx);
                    })),
            );
        }
        Some(lista.into_any_element())
    }

    fn secao_servico(&self, cx: &mut Context<Self>) -> AnyElement {
        let codigo_ajuda = match descricao_do_servico(&self.dados.ctribnac) {
            Some(d) => SharedString::from(d.to_string()),
            None => SharedString::from("Busque acima ou digite os 6 dígitos."),
        };
        let prestacao = ajuda_do_municipio(
            &self.dados.municipio_prestacao,
            Some(format!(
                "Vazio: o município do emitente ({}).",
                self.ctx.emitente.municipio
            )),
        );
        let nbs_ajuda = match descricao_do_nbs(&self.dados.nbs) {
            Some(d) => SharedString::from(d),
            None => SharedString::from("9 dígitos. A sugestão abaixo ajuda a escolher."),
        };
        let travado = self.travado();
        let descricao_erro = self.erro_do(CampoNota::Descricao);
        let tamanho = self.dados.descricao.chars().count();
        self.cartao_de_secao(SecaoNota::Servico, Some("O que foi prestado."), cx)
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        Input::new(&self.busca)
                            .id("busca-de-servico")
                            .disabled(travado)
                            .prefix(Icon::new(IconName::Search).small()),
                    )
                    .children(self.lista_de_servicos(cx)),
            )
            .child(
                v_form()
                    .columns(2)
                    .child(
                        self.campo_de(
                            "Código de tributação nacional",
                            CampoNota::Ctribnac,
                            "nota-ctribnac",
                            true,
                            Some(codigo_ajuda),
                            false,
                        )
                        .col_span(2),
                    )
                    .child(
                        campo(
                            "Descrição do serviço",
                            true,
                            Some(SharedString::from(format!("{tamanho} de 2000 caracteres."))),
                            descricao_erro,
                            div()
                                .id("nota-descricao")
                                .child(Textarea::new(&self.descricao).disabled(travado)),
                        )
                        .col_span(2),
                    )
                    .child(
                        self.campo_de(
                            "Município da prestação",
                            CampoNota::MunicipioPrestacao,
                            "nota-municipio-prestacao",
                            false,
                            prestacao,
                            false,
                        )
                        .col_span(1),
                    )
                    .child(
                        self.campo_de(
                            "NBS",
                            CampoNota::Nbs,
                            "nota-nbs",
                            self.dados.ibscbs_ativo,
                            Some(nbs_ajuda),
                            false,
                        )
                        .col_span(1),
                    )
                    .child(
                        self.campo_de(
                            "Código de tributação municipal",
                            CampoNota::CtribMun,
                            "nota-ctribmun",
                            false,
                            None,
                            false,
                        )
                        .col_span(1),
                    )
                    .child(
                        self.campo_de(
                            "Código interno",
                            CampoNota::CodInterno,
                            "nota-cod-interno",
                            false,
                            None,
                            false,
                        )
                        .col_span(1),
                    ),
            )
            .into_any_element()
    }

    // ---------------------------------------------------------------- valores

    fn secao_valores(&self, cx: &mut Context<Self>) -> AnyElement {
        self.cartao_de_secao(
            SecaoNota::Valores,
            Some("Em reais, no formato brasileiro (1.234,56)."),
            cx,
        )
        .child(
            v_form()
                .columns(2)
                .child(
                    self.campo_de(
                        "Valor do serviço (R$)",
                        CampoNota::ValorServico,
                        "nota-valor-servico",
                        true,
                        None,
                        true,
                    )
                    .col_span(1),
                )
                .child(
                    self.campo_de(
                        "Competência",
                        CampoNota::Competencia,
                        "nota-competencia",
                        true,
                        Some("Data do fato gerador.".into()),
                        false,
                    )
                    .col_span(1),
                )
                .child(
                    self.campo_de(
                        "Desconto incondicionado (R$)",
                        CampoNota::DescontoIncondicionado,
                        "nota-desc-incondicionado",
                        false,
                        None,
                        true,
                    )
                    .col_span(1),
                )
                .child(
                    self.campo_de(
                        "Desconto condicionado (R$)",
                        CampoNota::DescontoCondicionado,
                        "nota-desc-condicionado",
                        false,
                        None,
                        true,
                    )
                    .col_span(1),
                ),
        )
        .into_any_element()
    }

    // ---------------------------------------------------------------- iss

    fn escolha_de_u8(
        &self,
        id: &'static str,
        opcoes: &'static [(u8, &'static str)],
        atual: u8,
        cx: &mut Context<Self>,
        ao_escolher: fn(&mut Self, u8, &mut Context<Self>),
    ) -> AnyElement {
        ToggleGroup::new(id)
            .segmented()
            .single()
            .disabled(self.travado())
            .children(opcoes.iter().map(|(codigo, rotulo)| {
                Toggle::new(SharedString::from(format!("{id}-{codigo}")))
                    .label(*rotulo)
                    .checked(atual == *codigo)
            }))
            .on_click(cx.listener(move |this, estados: &Vec<bool>, _, cx| {
                if let Some(i) = estados.iter().position(|e| *e) {
                    if let Some((codigo, _)) = opcoes.get(i) {
                        ao_escolher(this, *codigo, cx);
                    }
                }
            }))
            .into_any_element()
    }

    fn secao_iss(&self, cx: &mut Context<Self>) -> AnyElement {
        let regime = self.ctx.regime();
        let d = &self.dados;
        let tributacao = self.escolha_de_u8(
            "tributacao",
            &OPCOES_TRIBUTACAO,
            d.tributacao,
            cx,
            |this, v, cx| this.escolher_tributacao(v, cx),
        );
        let retencao = self.escolha_de_u8(
            "retencao",
            &OPCOES_RETENCAO,
            d.retencao,
            cx,
            |this, v, cx| {
                this.escolher_retencao(v, cx);
            },
        );
        let mut forma = v_form()
            .columns(2)
            .child(
                campo(
                    "Tributação do ISS",
                    true,
                    Some("Imunidade e exportação de serviço: use a linha de comando.".into()),
                    self.erro_do(CampoNota::Tributacao),
                    tributacao,
                )
                .col_span(2),
            )
            .child(
                campo(
                    "Retenção do ISS",
                    true,
                    None,
                    self.erro_do(CampoNota::Retencao),
                    retencao,
                )
                .col_span(2),
            );
        if regime.mostra_aliquota_iss(d.tributacao) {
            forma = forma.child(
                self.campo_de(
                    "Alíquota do ISS (%)",
                    CampoNota::AliquotaIss,
                    "nota-aliquota-iss",
                    false,
                    Some("Opcional. De 0 a 5%.".into()),
                    true,
                )
                .col_span(1),
            );
        }
        if regime.pede_percentual_sn() {
            forma = forma.child(
                self.campo_de(
                    "Tributos do Simples Nacional (%)",
                    CampoNota::PercentualSn,
                    "nota-percentual-sn",
                    true,
                    Some(
                        "Percentual aproximado de tributos (Lei 12.741). Pergunte ao contador."
                            .into(),
                    ),
                    true,
                )
                .col_span(1),
            );
        }
        if regime.pede_totais_aproximados() {
            forma = forma
                .child(
                    self.campo_de(
                        "Tributos federais (%)",
                        CampoNota::PercentualFederal,
                        "nota-percentual-federal",
                        true,
                        Some("Total aproximado de tributos. Informe ao menos um.".into()),
                        true,
                    )
                    .col_span(1),
                )
                .child(
                    self.campo_de(
                        "Tributos estaduais (%)",
                        CampoNota::PercentualEstadual,
                        "nota-percentual-estadual",
                        false,
                        None,
                        true,
                    )
                    .col_span(1),
                )
                .child(
                    self.campo_de(
                        "Tributos municipais (%)",
                        CampoNota::PercentualMunicipal,
                        "nota-percentual-municipal",
                        false,
                        None,
                        true,
                    )
                    .col_span(1),
                );
        }
        let mut cartao = self
            .cartao_de_secao(
                SecaoNota::Iss,
                Some("Imposto sobre serviços e tributos aproximados."),
                cx,
            )
            .child(forma);
        if regime.op_simples == 2 {
            cartao = cartao.child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child("MEI não informa alíquota do ISS nem percentual de tributos."),
            );
        }
        cartao.into_any_element()
    }

    // ---------------------------------------------------------------- federais

    fn secao_federais(&self, cx: &mut Context<Self>) -> AnyElement {
        let aberta = self.federais_abertas;
        let cabeca = Button::new("alternar-federais")
            .label("Tributos federais (opcional)")
            .icon(if aberta {
                IconName::ChevronDown
            } else {
                IconName::ChevronRight
            })
            .ghost()
            .on_click(cx.listener(|this, _, _, cx| this.alternar_federais(cx)));
        let mut cartao = cartao(cx).id("secao-federais").child(
            v_flex().items_start().gap_1().child(cabeca).child(
                div()
                    .pl_3()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("Retenções de IRRF, CSLL e previdência; PIS e COFINS."),
            ),
        );
        if aberta {
            cartao = cartao.child(
                v_form()
                    .columns(2)
                    .child(
                        self.campo_de(
                            "IRRF retido (R$)",
                            CampoNota::Irrf,
                            "nota-irrf",
                            false,
                            None,
                            true,
                        )
                        .col_span(1),
                    )
                    .child(
                        self.campo_de(
                            "CSLL retida (R$)",
                            CampoNota::Csll,
                            "nota-csll",
                            false,
                            None,
                            true,
                        )
                        .col_span(1),
                    )
                    .child(
                        self.campo_de(
                            "Contribuição previdenciária retida (R$)",
                            CampoNota::Previdencia,
                            "nota-previdencia",
                            false,
                            None,
                            true,
                        )
                        .col_span(2),
                    )
                    .child(
                        self.campo_de(
                            "CST do PIS/COFINS",
                            CampoNota::PisCst,
                            "nota-pis-cst",
                            false,
                            Some("2 dígitos.".into()),
                            false,
                        )
                        .col_span(1),
                    )
                    .child(
                        self.campo_de(
                            "Base do PIS/COFINS (R$)",
                            CampoNota::PisBase,
                            "nota-pis-base",
                            false,
                            None,
                            true,
                        )
                        .col_span(1),
                    )
                    .child(
                        self.campo_de(
                            "Alíquota do PIS (%)",
                            CampoNota::PisAliquotaPis,
                            "nota-pis-aliquota",
                            false,
                            None,
                            true,
                        )
                        .col_span(1),
                    )
                    .child(
                        self.campo_de(
                            "Alíquota da COFINS (%)",
                            CampoNota::PisAliquotaCofins,
                            "nota-cofins-aliquota",
                            false,
                            None,
                            true,
                        )
                        .col_span(1),
                    )
                    .child(
                        self.campo_de(
                            "Tipo de retenção do PIS/COFINS",
                            CampoNota::PisTipoRetencao,
                            "nota-pis-retencao",
                            false,
                            Some("De 0 a 9, como na tabela da nota técnica.".into()),
                            false,
                        )
                        .col_span(2),
                    ),
            );
        }
        cartao.into_any_element()
    }

    // ---------------------------------------------------------------- ibs/cbs

    fn cartao_da_sugestao(&self, cx: &mut Context<Self>) -> AnyElement {
        let tema = cx.theme();
        let (borda, muted, hover) = (tema.border, tema.muted_foreground, tema.accent);
        let mut caixa = v_flex()
            .id("sugestao-da-correlacao")
            .gap_2()
            .p_3()
            .rounded_md()
            .border_1()
            .border_color(borda)
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(Icon::new(IconName::Info).small().text_color(muted))
                    .child(
                        div()
                            .text_sm()
                            .font_semibold()
                            .child("Sugestão pela correlação (Anexo VIII)"),
                    ),
            );
        if self.dados.ctribnac_digitos().len() != 6 {
            return caixa
                .child(
                    div().text_sm().text_color(muted).child(
                        "Escolha o serviço para ver a sugestão de NBS, cIndOp e cClassTrib.",
                    ),
                )
                .into_any_element();
        }
        let travado = self.travado();
        if let Some(p) = &self.sugestao.proposta {
            for item in p.itens(&self.dados) {
                let (nota, cor) = match item.estado {
                    EstadoDoItem::Preencher => ("vai preencher", tema.success),
                    EstadoDoItem::Igual => ("já preenchido", muted),
                    EstadoDoItem::Diferente => {
                        ("você digitou outro valor: fica o seu", tema.warning)
                    }
                };
                caixa = caixa.child(
                    h_flex()
                        .gap_2()
                        .items_start()
                        .text_sm()
                        .child(div().w(px(110.)).flex_shrink_0().text_color(muted).child(
                            match item.campo {
                                CampoNota::Nbs => "NBS",
                                CampoNota::CindOp => "cIndOp",
                                _ => "cClassTrib",
                            },
                        ))
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .child(div().font_semibold().child(item.valor.clone()))
                                .when(!item.descricao.is_empty(), |d| {
                                    d.child(
                                        div()
                                            .text_xs()
                                            .text_color(muted)
                                            .child(item.descricao.clone()),
                                    )
                                }),
                        )
                        .child(div().text_xs().text_color(cor).child(nota)),
                );
            }
            for aviso in &p.avisos {
                caixa = caixa.child(faixa(Gravidade::Aviso, aviso.clone(), cx));
            }
            caixa = caixa.child(
                h_flex().child(
                    Button::new("usar-sugestao-ibscbs")
                        .label("Usar a sugestão")
                        .icon(IconName::Check)
                        .outline()
                        .small()
                        .disabled(travado || !p.ha_o_que_preencher(&self.dados))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.aceitar_sugestao(window, cx);
                        })),
                ),
            );
        } else if let Some(aviso) = &self.sugestao.aviso {
            caixa = caixa.child(div().text_sm().text_color(muted).child(aviso.clone()));
        }
        if !self.sugestao.nbs.is_empty() && self.dados.nbs_digitos().len() != 9 {
            let mut lista = v_flex()
                .id("opcoes-de-nbs")
                .max_h(px(180.))
                .overflow_y_scroll()
                .rounded_md()
                .border_1()
                .border_color(borda);
            for (i, n) in self.sugestao.nbs.iter().enumerate() {
                let codigo = n.codigo.clone();
                lista = lista.child(
                    h_flex()
                        .id(SharedString::from(format!("nbs-{i}")))
                        .gap_3()
                        .items_start()
                        .px_3()
                        .py_1p5()
                        .cursor_pointer()
                        .hover(move |s| s.bg(hover))
                        .child(
                            div()
                                .w(px(96.))
                                .flex_shrink_0()
                                .text_sm()
                                .font_semibold()
                                .child(n.mascarado.clone()),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_sm()
                                .text_color(muted)
                                .child(n.descricao.clone()),
                        )
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.escolher_nbs(&codigo, window, cx);
                        })),
                );
            }
            caixa = caixa
                .child(
                    div()
                        .text_sm()
                        .child("Escolha o NBS do serviço (clique em uma linha):"),
                )
                .child(lista);
        }
        caixa.into_any_element()
    }

    fn secao_ibscbs(&self, cx: &mut Context<Self>) -> AnyElement {
        let ativo = self.dados.ibscbs_ativo;
        let travado = self.travado();
        let mut cartao = self
            .cartao_de_secao(
                SecaoNota::IbsCbs,
                Some("Reforma tributária do consumo."),
                cx,
            )
            .child(
                Switch::new("ibscbs-ativo")
                    .checked(ativo)
                    .label("Informar o grupo IBS/CBS nesta nota")
                    .disabled(travado)
                    .on_click(cx.listener(|this, marcado: &bool, _, cx| {
                        this.escolher_ibscbs(*marcado, cx);
                    })),
            );
        if !ativo {
            return cartao
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(
                        "A biblioteca aceita a nota sem este grupo. Ligue-o se o contador pedir.",
                    ),
                )
                .into_any_element();
        }
        let classe = descricao_da_classe(&self.dados.cclass_trib);
        let cind = descricao_do_cind_op(&self.dados.cind_op);
        let consumidor = ToggleGroup::new("consumidor-final")
            .segmented()
            .single()
            .disabled(travado)
            .children(
                OPCOES_CONSUMIDOR_FINAL
                    .iter()
                    .enumerate()
                    .map(|(i, (v, r))| {
                        Toggle::new(SharedString::from(format!("consumidor-final-{i}")))
                            .label(*r)
                            .checked(self.dados.ind_final == *v)
                    }),
            )
            .on_click(cx.listener(|this, estados: &Vec<bool>, _, cx| {
                if let Some(i) = estados.iter().position(|e| *e) {
                    if let Some((v, _)) = OPCOES_CONSUMIDOR_FINAL.get(i) {
                        this.escolher_consumidor_final(*v, cx);
                    }
                }
            }));
        cartao = cartao
            .child(self.cartao_da_sugestao(cx))
            .child(
                v_form()
                    .columns(2)
                    .child(
                        self.campo_de(
                            "Indicador da operação (cIndOp)",
                            CampoNota::CindOp,
                            "nota-cind-op",
                            true,
                            Some(if cind.is_empty() {
                                SharedString::from("6 dígitos, do Anexo VII.")
                            } else {
                                SharedString::from(cind)
                            }),
                            false,
                        )
                        .col_span(1),
                    )
                    .child(
                        self.campo_de(
                            "Classificação tributária (cClassTrib)",
                            CampoNota::ClassTrib,
                            "nota-class-trib",
                            true,
                            Some(if classe.is_empty() {
                                SharedString::from("6 dígitos; o CST sai dos 3 primeiros.")
                            } else {
                                SharedString::from(format!("CST {}: {classe}", self.dados.cst()))
                            }),
                            false,
                        )
                        .col_span(1),
                    )
                    .child(
                        campo(
                            "Consumidor final",
                            false,
                            None,
                            self.erro_do(CampoNota::IndFinal),
                            consumidor,
                        )
                        .col_span(2),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("Finalidade regular e destinatário igual ao tomador, como a biblioteca exige para esta tela."),
            );
        cartao.into_any_element()
    }

    // ---------------------------------------------------------------- a pagina

    fn banner_de_envio(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let Fase::Enviando { etapa, .. } = &self.fase else {
            return None;
        };
        let texto = match etapa {
            Etapa::AbrindoCertificado => "Abrindo o certificado...",
            Etapa::Enviando => "Enviando para a Sefin Nacional...",
        };
        Some(
            h_flex()
                .id("enviando-a-nota")
                .gap_2()
                .items_center()
                .p_3()
                .rounded_lg()
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().muted)
                .child(Icon::new(IconName::LoaderCircle).small())
                .child(div().text_sm().font_semibold().child(texto))
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child("O formulário fica travado até terminar."),
                )
                .into_any_element(),
        )
    }

    fn coluna_do_formulario(&mut self, ao_lado: bool, cx: &mut Context<Self>) -> AnyElement {
        let centrada = |el: AnyElement| -> AnyElement {
            div()
                .w_full()
                .max_w(px(LARGURA_FORM))
                .mx_auto()
                .child(el)
                .into_any_element()
        };
        let topo = v_flex()
            .gap_4()
            .child(cabecalho(
                "Nova nota",
                "Preencha os dados da nota. A conferência acontece enquanto você digita.",
                cx,
            ))
            .child(self.resumo_do_emitente(cx))
            .children(self.banner_de_envio(cx))
            .children(self.avisos_do_topo(cx))
            .into_any_element();
        let secoes = [
            self.secao_tomador(cx),
            self.secao_servico(cx),
            self.secao_valores(cx),
            self.secao_iss(cx),
            self.secao_federais(cx),
            self.secao_ibscbs(cx),
        ];
        let mut coluna = v_flex()
            .id("formulario-da-nota")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.rolagem)
            .gap_5()
            .px_6()
            .py_6()
            .child(centrada(topo));
        for s in secoes {
            coluna = coluna.child(centrada(s));
        }
        if !ao_lado {
            coluna = coluna.child(centrada(self.painel(false, cx)));
        }
        coluna.into_any_element()
    }
}

impl Render for NovaNota {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Fase::Concluida(c) = self.fase.clone() {
            return self.tela_de_resultado(c, cx);
        }
        let largura = window.viewport_size().width - px(LARGURA_DA_LATERAL);
        let ao_lado = largura >= px(LARGURA_PARA_PAINEL_AO_LADO);
        let formulario = self.coluna_do_formulario(ao_lado, cx);
        let base = div()
            .id("nova-nota")
            .key_context("NovaNota")
            .size_full()
            .on_action(
                cx.listener(|this, _: &crate::telas::EmitirNota, window, cx| {
                    this.pedir_emissao(window, cx);
                }),
            );
        if ao_lado {
            let painel = self.painel(true, cx);
            base.child(
                h_flex()
                    .size_full()
                    .child(div().flex_1().min_w_0().h_full().child(formulario))
                    .child(painel),
            )
            .into_any_element()
        } else {
            base.child(formulario).into_any_element()
        }
    }
}
