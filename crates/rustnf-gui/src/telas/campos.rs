// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Blocos de campos que o primeiro uso e as Configuracoes dividem: emitente, certificado e pasta das notas.

use gpui_kit::component::button::{Button, ButtonVariants as _, Toggle, ToggleGroup};
use gpui_kit::component::form::v_form;
use gpui_kit::component::input::Input;
use gpui_kit::component::select::Select;
use gpui_kit::component::{
    h_flex, v_flex, ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, Context, InteractiveElement as _, IntoElement, ParentElement as _, SharedString,
    Styled as _,
};

use super::pecas::{campo, cartao, cor_da, faixa, grupo, icone_da, Negrito as _};
use super::raiz::Raiz;
use crate::modelo::campo::Campo;
use crate::modelo::certificado::Gravidade;
use crate::modelo::formulario::OPCOES_SIMPLES;

impl Raiz {
    /// Escolha do Simples Nacional (1, 2 ou 3) como controle segmentado.
    fn escolha_do_simples(&self, id: &'static str, cx: &mut Context<Self>) -> impl IntoElement {
        let atual = self.formulario.dados.op_simples;
        ToggleGroup::new(id)
            .segmented()
            .single()
            .children(OPCOES_SIMPLES.map(|(codigo, rotulo)| {
                Toggle::new(SharedString::from(format!("{id}-{codigo}")))
                    .label(rotulo)
                    .checked(atual == codigo)
            }))
            .on_click(cx.listener(|this, estados: &Vec<bool>, window, cx| {
                // clicar no que ja esta marcado nao desmarca: o Simples sempre tem um valor
                if let Some(i) = estados.iter().position(|e| *e) {
                    this.formulario.definir_op_simples(OPCOES_SIMPLES[i].0);
                    this.sincronizar_selecoes(window, cx);
                }
                cx.notify();
            }))
    }

    /// Os campos do emitente. `completo` acrescenta a serie e o regime especial (so' nas Configuracoes).
    pub(super) fn campos_emitente(
        &mut self,
        completo: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let erros = self.erros_visiveis();
        let e = |c: Campo| erros.do_campo(c).map(str::to_string);
        let d = &self.formulario.dados;
        let op = d.op_simples;
        let municipio = self.formulario.rotulo_do_municipio();
        let rotulo_municipio = municipio.texto();
        let achou = matches!(
            municipio,
            crate::modelo::mascara::RotuloMunicipio::Encontrado { .. }
        );
        let ajuda_municipio = rotulo_municipio.map(|t| {
            if achou {
                SharedString::from(format!("Município: {t}"))
            } else {
                SharedString::from(t)
            }
        });

        let mut forma = v_form()
            .columns(2)
            .child(
                campo(
                    "CNPJ ou CPF",
                    true,
                    Some("Só os números; a máscara aparece sozinha.".into()),
                    e(Campo::Documento),
                    Input::new(&self.entradas.documento).id("documento"),
                )
                .col_span(1),
            )
            .child(
                campo(
                    "Inscrição municipal",
                    true,
                    None,
                    e(Campo::InscricaoMunicipal),
                    Input::new(&self.entradas.inscricao).id("inscricao"),
                )
                .col_span(1),
            )
            .child(
                campo(
                    "Nome ou razão social",
                    true,
                    None,
                    e(Campo::Nome),
                    Input::new(&self.entradas.nome).id("nome"),
                )
                .col_span(2),
            )
            .child(
                campo(
                    "Município (código IBGE)",
                    true,
                    ajuda_municipio,
                    e(Campo::Municipio),
                    Input::new(&self.entradas.municipio).id("municipio"),
                )
                .col_span(1),
            );
        if completo {
            forma = forma.child(
                campo(
                    "Série da DPS",
                    false,
                    Some("Até 5 dígitos. Vazio: a do pedido, ou 1.".into()),
                    e(Campo::Serie),
                    Input::new(&self.entradas.serie).id("serie"),
                )
                .col_span(1),
            );
        }
        let simples = self.escolha_do_simples("simples", cx);
        forma = forma.child(campo("Simples Nacional", true, None, None, simples).col_span(2));
        if op == 3 {
            forma = forma.child(
                campo(
                    "Regime de apuração (ME/EPP)",
                    true,
                    None,
                    e(Campo::RegimeApuracao),
                    Select::new(&self.apuracao)
                        .id("apuracao")
                        .placeholder("Escolha o regime"),
                )
                .col_span(2),
            );
        }
        if completo {
            forma = forma.child(
                campo(
                    "Regime especial de tributação",
                    false,
                    Some("MEI não tem regime especial.".into()),
                    e(Campo::RegimeEspecial),
                    Select::new(&self.especial).id("especial").disabled(op == 2),
                )
                .col_span(2),
            );
        }
        forma
    }

    /// O campo do arquivo do certificado, com o botao de escolher.
    pub(super) fn campo_do_certificado(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let erros = self.erros_visiveis();
        let erro = erros.do_campo(Campo::Pfx).map(str::to_string);
        campo(
            "Arquivo do certificado A1 (.pfx ou .p12)",
            true,
            Some("O arquivo fica onde está; só o caminho é guardado.".into()),
            erro,
            h_flex()
                .gap_2()
                .child(
                    div()
                        .flex_1()
                        .child(Input::new(&self.entradas.pfx).id("pfx")),
                )
                .child(
                    Button::new("escolher-pfx")
                        .label("Escolher arquivo...")
                        .icon(IconName::FolderOpen)
                        .outline()
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.escolher(Campo::Pfx, false, window, cx);
                        })),
                ),
        )
    }

    /// Senha do certificado, com o estado do cofre.
    pub(super) fn campo_da_senha(
        &self,
        com_botoes_do_cofre: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let erros = self.erros_visiveis();
        let erro = erros.do_campo(Campo::Senha).map(str::to_string);
        let ocupado = self.ocupado.senha;
        let guardada = self.senha_guardada;
        let desligado = self.servicos.cofre_desligado();
        let (grav, texto) = if desligado {
            (
                Gravidade::Aviso,
                "O cofre de senhas está desligado: a senha não será guardada. Digite-a sempre que for usar o certificado.",
            )
        } else {
            match guardada {
                Some(true) => (
                    Gravidade::Ok,
                    "Há uma senha guardada no cofre do Windows para este certificado.",
                ),
                Some(false) => (
                    Gravidade::Neutro,
                    "Nenhuma senha guardada. Ela vai para o cofre do Windows, nunca para o arquivo de configuração.",
                ),
                None => (
                    Gravidade::Aviso,
                    "O cofre de senhas do Windows não respondeu.",
                ),
            }
        };
        let cor = cor_da(grav, cx);
        let digitando = !self.entradas.senha.read(cx).value().is_empty();
        v_flex()
            .gap_2()
            .child(campo(
                "Senha do certificado",
                false,
                None,
                erro,
                Input::new(&self.entradas.senha).id("senha").mask_toggle(),
            ))
            .child(
                h_flex()
                    .gap_1p5()
                    .items_start()
                    .text_xs()
                    .text_color(cor)
                    .child(Icon::new(icone_da(grav)).xsmall().mt(px(1.)))
                    .child(div().flex_1().id("estado-da-senha").child(texto)),
            )
            .when(com_botoes_do_cofre && !desligado, |d| {
                d.child(
                    h_flex()
                        .gap_2()
                        .child(
                            Button::new("guardar-senha")
                                .label("Guardar no cofre")
                                .small()
                                .outline()
                                .loading(ocupado)
                                .disabled(ocupado || !digitando)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.guardar_senha(window, cx);
                                })),
                        )
                        .child(
                            Button::new("apagar-senha")
                                .label("Apagar do cofre")
                                .small()
                                .ghost()
                                .disabled(ocupado || guardada != Some(true))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.pedir_para_apagar_senha(window, cx);
                                })),
                        ),
                )
            })
    }

    /// O resultado do botao "Testar certificado".
    pub(super) fn resultado_do_certificado(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        let resultado = self.resultado_cert.as_ref()?;
        Some(match resultado {
            Ok(info) if info.apto => {
                let tema = cx.theme();
                let mut caixa = cartao(cx)
                    .id("resultado-do-certificado")
                    .gap_1p5()
                    .p_4()
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(Icon::new(IconName::CircleCheck).text_color(tema.success))
                            .child(
                                div()
                                    .font_semibold()
                                    .child("Certificado aberto com sucesso"),
                            ),
                    )
                    .child(super::pecas::linha_de_resumo(
                        "Titular",
                        info.titular.clone(),
                        cx,
                    ));
                if !info.documento.is_empty() {
                    caixa = caixa.child(super::pecas::linha_de_resumo(
                        "Documento",
                        info.documento.clone(),
                        cx,
                    ));
                }
                caixa = caixa.child(super::pecas::linha_de_resumo(
                    "Validade",
                    info.estado.texto().replacen("Certificado ", "", 1),
                    cx,
                ));
                for aviso in &info.avisos {
                    caixa = caixa.child(faixa(Gravidade::Aviso, aviso.clone(), cx));
                }
                caixa.into_any_element()
            }
            Ok(info) => faixa(
                Gravidade::Erro,
                info.problema
                    .clone()
                    .unwrap_or_else(|| "O certificado não serve para assinar.".into()),
                cx,
            )
            .id("resultado-do-certificado")
            .into_any_element(),
            Err(erro) => faixa(Gravidade::Erro, erro.texto(), cx)
                .id("resultado-do-certificado")
                .into_any_element(),
        })
    }

    /// O campo da pasta das notas, com o botao de escolher e, se houver, a sugestao.
    pub(super) fn campo_da_pasta(
        &self,
        com_sugestao: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let erros = self.erros_visiveis();
        let erro = erros.do_campo(Campo::NotasPasta).map(str::to_string);
        let sugestao = if com_sugestao {
            self.pasta_sugerida()
        } else {
            None
        };
        let vazio = self.formulario.dados.notas_pasta.trim().is_empty();
        v_flex()
            .gap_2()
            .child(campo(
                "Pasta das notas",
                true,
                Some("Cada nota autorizada é guardada aqui em XML, com um índice. A pasta é criada se não existir.".into()),
                erro,
                h_flex()
                    .gap_2()
                    .child(div().flex_1().child(Input::new(&self.entradas.notas).id("pasta-das-notas")))
                    .child(
                        Button::new("escolher-pasta")
                            .label("Escolher pasta...")
                            .icon(IconName::FolderOpen)
                            .outline()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.escolher(Campo::NotasPasta, true, window, cx);
                            })),
                    ),
            ))
            .when_some(sugestao.filter(|_| vazio), |d, caminho| {
                let texto = caminho.to_string_lossy().into_owned();
                let tema = cx.theme();
                d.child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .text_xs()
                        .text_color(tema.muted_foreground)
                        .child(div().child("Sugestão:"))
                        .child(div().id("sugestao-de-pasta").flex_1().min_w_0().child(texto))
                        .child(
                            Button::new("usar-sugestao")
                                .label("Usar")
                                .small()
                                .outline()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    if let Some(p) = this.pasta_sugerida() {
                                        this.definir_caminho(Campo::NotasPasta, &p, window, cx);
                                    }
                                })),
                        ),
                )
            })
    }

    pub(super) fn titulo_do_grupo(
        &self,
        titulo: &str,
        apoio: Option<&str>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        grupo(titulo, apoio, cx)
    }
}
