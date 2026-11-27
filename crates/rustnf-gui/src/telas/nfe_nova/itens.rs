// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O passo "O que vendeu": a busca no cadastro de produtos, um cartao por produto (o que e', quanto, o NCM) e, recolhidos, os
//! "Detalhes fiscais" (ICMS, CFOP, CST, origem), com o que a tabela escolheu e a fonte. O que a pessoa digitar nos detalhes
//! vale no lugar da tabela e nunca e' sobrescrito.

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::form::v_form;
use gpui_kit::component::input::Input;
use gpui_kit::component::{
    h_flex, v_flex, ActiveTheme as _, Disableable as _, IconName, Sizable as _,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, Styled as _,
};

use super::NovaNfe;
use crate::modelo::certificado::Gravidade;
use crate::modelo::mascara::centavos_para_dinheiro;
use crate::modelo::nfe_campos::CampoItem;
use crate::modelo::nfe_fiscal::{cst_pis_cofins, opcoes_de_icms, Icms, Origem, RegimePisCofins};
use crate::modelo::nfe_nova::{DadosNfe, ItemNfe};
use crate::telas::pecas::{campo, cartao, faixa, grupo, Negrito as _};

/// O texto da origem da mercadoria (`orig`, MOC 7.00, N11).
fn origem_da_mercadoria(o: u8) -> &'static str {
    match o {
        0 => "Nacional (exceto as dos códigos 3, 4, 5 e 8)",
        1 => "Estrangeira, importação direta (exceto a do código 6)",
        2 => "Estrangeira, adquirida no mercado interno (exceto a do código 7)",
        3 => "Nacional, com conteúdo de importação entre 40% e 70%",
        4 => "Nacional, produzida conforme os processos produtivos básicos",
        5 => "Nacional, com conteúdo de importação até 40%",
        6 => "Estrangeira, importação direta, sem similar nacional",
        7 => "Estrangeira, adquirida no mercado interno, sem similar nacional",
        _ => "Nacional, com conteúdo de importação acima de 70%",
    }
}

impl NovaNfe {
    fn texto_do_item(&self, i: usize, c: CampoItem) -> AnyElement {
        match self
            .linhas
            .get(i)
            .and_then(|l| l.entradas.get(&c).map(|e| (l.id, e)))
        {
            Some((id, e)) => Input::new(e)
                .id(SharedString::from(format!("nfe-item-{id}-{c:?}")))
                .into_any_element(),
            None => div().into_any_element(),
        }
    }

    /// Botoes de escolha de um produto.
    fn escolhas_do_item<T: Copy + PartialEq + 'static>(
        &self,
        id: String,
        i: usize,
        opcoes: Vec<(T, String)>,
        atual: Option<T>,
        aplicar: fn(&mut ItemNfe, T),
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let travado = self.travado();
        h_flex()
            .id(SharedString::from(format!("{id}-grupo")))
            .flex_wrap()
            .gap_2()
            .children(opcoes.into_iter().enumerate().map(|(k, (valor, rotulo))| {
                let marcado = atual == Some(valor);
                Button::new(SharedString::from(format!("{id}-{k}")))
                    .label(rotulo)
                    .small()
                    .when(marcado, |b| b.primary())
                    .when(!marcado, |b| b.outline())
                    .disabled(travado)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.escolher(
                            move |d: &mut DadosNfe| {
                                if let Some(item) = d.itens.get_mut(i) {
                                    aplicar(item, valor);
                                }
                            },
                            cx,
                        );
                    }))
            }))
            .into_any_element()
    }

    /// O que a tabela escolheu para o produto, em uma linha: CFOP, ICMS e PIS/COFINS.
    fn resumo_fiscal(&self, item: &ItemNfe) -> String {
        let crt = self.ctx.crt;
        let cfop = match self.dados.destino(&self.ctx) {
            Some(d) => item.cfop_efetivo(d, crt).map_or_else(
                || "CFOP: veja os detalhes".to_string(),
                |c| format!("CFOP {c}"),
            ),
            None => "CFOP: depende do cliente (passo 1)".to_string(),
        };
        let tipo = if matches!(crt, 1 | 4) { "CSOSN" } else { "CST" };
        let icms = item.icms_codigo_efetivo(crt).map_or_else(
            || "ICMS: veja os detalhes".to_string(),
            |c| format!("{tipo} {c}"),
        );
        let pis = item
            .pis_cst
            .clone()
            .filter(|p| !p.is_empty())
            .unwrap_or_else(|| cst_pis_cofins(crt).to_string());
        format!("{cfop} · {icms} · PIS/COFINS CST {pis}")
    }

    /// O passo "O que vendeu".
    pub(super) fn passo_o_que_vendeu(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let tema_muted = cx.theme().muted_foreground;
        let travado = self.travado();
        let termo = self.busca.read(cx).value().to_string();
        let achados: Vec<_> = self
            .cadastro
            .buscar(&termo, 6)
            .into_iter()
            .cloned()
            .collect();
        let vazio = self.cadastro.produtos.is_empty();
        let mut coluna = v_flex().gap_4().child(grupo(
            "O que você vendeu",
            Some("Escolha do cadastro ou inclua um produto novo. O NCM é obrigatório e fica guardado para a próxima nota."),
            cx,
        ));
        let busca = Input::new(&self.busca).id("nfe-busca-de-produto");
        coluna = coluna.child(
            h_flex()
                .gap_2()
                .items_end()
                .child(
                    div()
                        .flex_1()
                        .child(campo("Produto do cadastro", false, None, None, busca)),
                )
                .child(
                    Button::new("nfe-produto-novo")
                        .label("Produto novo")
                        .icon(IconName::Plus)
                        .outline()
                        .disabled(travado)
                        .on_click(cx.listener(|this, _, window, cx| {
                            // o ICMS comeca na opcao de uso comum do regime e a tela a mostra marcada
                            let item = ItemNfe::default();
                            this.adicionar_item(item, window, cx);
                        })),
                ),
        );
        if vazio {
            coluna = coluna.child(div().text_xs().text_color(tema_muted).child(
                "O cadastro de produtos está vazio. Os produtos novos desta nota entram nele quando ela for autorizada.",
            ));
        } else if achados.is_empty() {
            coluna = coluna.child(
                div()
                    .text_xs()
                    .text_color(tema_muted)
                    .child("Nenhum produto do cadastro combina com a busca."),
            );
        } else {
            let mut lista = v_flex().gap_1().id("nfe-resultados-da-busca");
            for (k, p) in achados.into_iter().enumerate() {
                let rotulo = format!(
                    "{}{} · R$ {} · NCM {}",
                    if p.codigo.is_empty() {
                        String::new()
                    } else {
                        format!("{} - ", p.codigo)
                    },
                    p.descricao,
                    if p.valor.is_empty() {
                        "-".to_string()
                    } else {
                        p.valor.clone()
                    },
                    crate::modelo::nfe_campos::formatar_ncm(&p.ncm),
                );
                lista = lista.child(
                    Button::new(SharedString::from(format!("nfe-adicionar-produto-{k}")))
                        .label(rotulo)
                        .icon(IconName::Plus)
                        .outline()
                        .small()
                        .disabled(travado)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.adicionar_do_cadastro(&p, window, cx);
                        })),
                );
            }
            coluna = coluna.child(lista);
        }
        for i in 0..self.dados.itens.len() {
            coluna = coluna.child(self.cartao_do_item(i, cx));
        }
        if self.dados.itens.is_empty() {
            coluna = coluna.child(
                faixa(
                    Gravidade::Neutro,
                    "Nenhum produto ainda. Escolha um do cadastro ou use Produto novo.",
                    cx,
                )
                .id("nfe-sem-produtos"),
            );
        }
        if self.ctx.pergunta_o_regime_pis() {
            let escolha = self.escolhas(
                "nfe-regime-pis",
                RegimePisCofins::TODOS
                    .iter()
                    .map(|r| (*r, r.rotulo().to_string()))
                    .collect(),
                self.dados.regime_pis,
                |d, v| d.regime_pis = Some(v),
                cx,
            );
            coluna = coluna.child(
                v_flex()
                    .gap_1p5()
                    .child(div().text_sm().font_semibold().child("Qual o regime do PIS e da COFINS da empresa?"))
                    .child(escolha)
                    .child(div().text_xs().text_color(tema_muted).child(
                        "A configuração da NF-e não diz qual é: escolha (as alíquotas mudam) ou guarde na seção NF-e das Configurações.",
                    )),
            );
        }
        let t = self.dados.totais();
        coluna = coluna.child(
            h_flex()
                .gap_4()
                .text_sm()
                .child(format!(
                    "Produtos: R$ {}",
                    centavos_para_dinheiro(t.produtos)
                ))
                .when(t.desconto > 0, |d| {
                    d.child(format!(
                        "Desconto: R$ {}",
                        centavos_para_dinheiro(t.desconto)
                    ))
                })
                .child(
                    div()
                        .font_semibold()
                        .child(format!("Total: R$ {}", centavos_para_dinheiro(t.total))),
                ),
        );
        coluna.into_any_element()
    }

    /// O cartao de um produto.
    fn cartao_do_item(&mut self, i: usize, cx: &mut Context<Self>) -> AnyElement {
        let Some(item) = self.dados.itens.get(i).cloned() else {
            return div().into_any_element();
        };
        let tema_muted = cx.theme().muted_foreground;
        let travado = self.travado();
        let detalhes = self.linhas.get(i).is_some_and(|l| l.detalhes);
        let liquido = item.liquido_centavos();
        let origem = self.escolhas_do_item(
            format!("nfe-origem-{i}"),
            i,
            Origem::TODAS
                .iter()
                .map(|o| (*o, o.rotulo().to_string()))
                .collect(),
            Some(item.origem),
            |it, v| it.origem = v,
            cx,
        );
        let titulo = if item.descricao.trim().is_empty() {
            format!("Produto {}", i + 1)
        } else {
            format!("Produto {}: {}", i + 1, item.descricao.trim())
        };
        let mut c = cartao(cx)
            .id(SharedString::from(format!("nfe-produto-{i}")))
            .gap_3()
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .child(div().font_semibold().child(titulo))
                    .child(
                        Button::new(SharedString::from(format!("nfe-remover-{i}")))
                            .label("Tirar da nota")
                            .icon(IconName::Delete)
                            .ghost()
                            .small()
                            .disabled(travado)
                            .on_click(cx.listener(move |this, _, _, cx| this.remover_item(i, cx))),
                    ),
            )
            .child(
                v_form()
                    .columns(4)
                    .child(campo("O que é", true, None, None, self.texto_do_item(i, CampoItem::Descricao)).col_span(3))
                    .child(campo("Código", false, None, None, self.texto_do_item(i, CampoItem::Codigo)).col_span(1))
                    .child(campo("Quantidade", true, None, None, self.texto_do_item(i, CampoItem::Quantidade)).col_span(1))
                    .child(campo("Unidade", true, None, None, self.texto_do_item(i, CampoItem::Unidade)).col_span(1))
                    .child(campo("Valor de cada (R$)", true, None, None, self.texto_do_item(i, CampoItem::Valor)).col_span(1))
                    .child(campo("Desconto (R$)", false, None, None, self.texto_do_item(i, CampoItem::Desconto)).col_span(1))
                    .child(
                        campo(
                            "NCM",
                            true,
                            Some("8 dígitos. O programa não tem a tabela do NCM: procure na TIPI da Receita.".into()),
                            None,
                            self.texto_do_item(i, CampoItem::Ncm),
                        )
                        .col_span(4),
                    ),
            )
            .child(
                v_flex()
                    .gap_1p5()
                    .child(div().text_sm().font_semibold().child("Esta mercadoria..."))
                    .child(origem),
            )
            .child(
                h_flex()
                    .gap_4()
                    .text_sm()
                    .child(div().child(format!(
                        "Total do produto: R$ {}",
                        liquido.map_or_else(|| "-".to_string(), centavos_para_dinheiro)
                    ))),
            )
            .child(div().text_xs().text_color(tema_muted).child(format!(
                "O programa escolheu: {}",
                self.resumo_fiscal(&item)
            )))
            .child(
                Checkbox::new(SharedString::from(format!("nfe-guardar-{i}")))
                    .label(if item.do_cadastro {
                        "Atualizar o cadastro com o que mudou aqui"
                    } else {
                        "Guardar este produto no cadastro quando a nota for autorizada"
                    })
                    .checked(item.guardar)
                    .disabled(travado)
                    .on_click(cx.listener(move |this, marcou: &bool, _, cx| {
                        let m = *marcou;
                        this.escolher(
                            move |d| {
                                if let Some(it) = d.itens.get_mut(i) {
                                    it.guardar = m;
                                }
                            },
                            cx,
                        );
                    })),
            )
            .child(
                Button::new(SharedString::from(format!("nfe-detalhes-{i}")))
                    .label(if detalhes { "Esconder os detalhes fiscais" } else { "Detalhes fiscais" })
                    .icon(if detalhes { IconName::ChevronUp } else { IconName::ChevronDown })
                    .ghost()
                    .small()
                    .on_click(cx.listener(move |this, _, _, cx| this.alternar_detalhes(i, cx))),
            );
        if detalhes {
            c = c.child(self.detalhes_fiscais(i, &item, cx));
        }
        c.into_any_element()
    }

    /// Os detalhes fiscais de um produto, recolhidos por padrao.
    fn detalhes_fiscais(&mut self, i: usize, item: &ItemNfe, cx: &mut Context<Self>) -> AnyElement {
        let crt = self.ctx.crt;
        let tema_muted = cx.theme().muted_foreground;
        let icms_atual = item.icms_efetivo(crt);
        let opcoes: Vec<(Icms, String)> = opcoes_de_icms(crt)
            .iter()
            .map(|o| (*o, o.rotulo().to_string()))
            .collect();
        let icms = self.escolhas_do_item(
            format!("nfe-icms-{i}"),
            i,
            opcoes,
            icms_atual,
            |it, v| it.icms = Some(v),
            cx,
        );
        let origens = self.escolhas_do_item(
            format!("nfe-orig-{i}"),
            i,
            (0u8..=8).map(|o| (o, o.to_string())).collect(),
            Some(item.origem_mercadoria),
            |it, v| it.origem_mercadoria = v,
            cx,
        );
        let destino = self.dados.destino(&self.ctx);
        let tabela = destino.and_then(|d| {
            let sem_manual = ItemNfe {
                cfop: None,
                ..item.clone()
            };
            sem_manual.cfop_efetivo(d, crt)
        });
        let tipo = if matches!(crt, 1 | 4) { "CSOSN" } else { "CST" };
        let tabela_icms = ItemNfe {
            icms_codigo: None,
            ..item.clone()
        }
        .icms_codigo_efetivo(crt);
        let tabela_pis = cst_pis_cofins(crt);
        let mut col = v_flex()
            .id(SharedString::from(format!("nfe-detalhes-fiscais-{i}")))
            .gap_3()
            .p_3()
            .rounded_md()
            .border_1()
            .border_color(cx.theme().border)
            .child(div().text_sm().font_semibold().child("Detalhes fiscais"))
            .child(div().text_xs().text_color(tema_muted).child(
                "O programa escolheu pelas suas respostas (tabela de CFOP do Portal da NF-e; MOC 7.00, Anexo I). Quem sabe pode trocar: o que você digitar aqui vale no lugar da tabela e não é mudado sozinho.",
            ))
            .child(
                v_flex()
                    .gap_1p5()
                    .child(div().text_sm().child("Como o ICMS se comporta"))
                    .child(icms),
            );
        if icms_atual == Some(Icms::Tributado) && item.icms_codigo.is_none() {
            col = col.child(
                v_form().columns(2).child(
                    campo(
                        "Alíquota do ICMS (%)",
                        true,
                        Some("O programa não chuta a alíquota do seu estado: digite a que vale para esta mercadoria.".into()),
                        None,
                        self.texto_do_item(i, CampoItem::AliquotaIcms),
                    )
                    .col_span(1),
                ),
            );
        }
        if icms_atual == Some(Icms::JaCobradoAntes) && item.icms_codigo.is_none() {
            col = col
                .child(div().text_xs().text_color(tema_muted).child(
                    "O ICMS já cobrado antes vem da nota de compra da mercadoria: digite a base, a alíquota e o valor retidos.",
                ))
                .child(
                    v_form()
                        .columns(3)
                        .child(campo("Base retida (R$)", true, None, None, self.texto_do_item(i, CampoItem::StBase)).col_span(1))
                        .child(campo("Alíquota (%)", true, None, None, self.texto_do_item(i, CampoItem::StAliquota)).col_span(1))
                        .child(campo("ICMS retido (R$)", true, None, None, self.texto_do_item(i, CampoItem::StValor)).col_span(1)),
                )
                .child(
                    v_form().columns(2).child(
                        campo("CEST", false, Some("A mercadoria com substituição tributária tem CEST.".into()), None, self.texto_do_item(i, CampoItem::Cest)).col_span(1),
                    ),
                );
        } else {
            col = col.child(
                v_form().columns(2).child(
                    campo(
                        "CEST",
                        false,
                        Some("Só se a mercadoria tem.".into()),
                        None,
                        self.texto_do_item(i, CampoItem::Cest),
                    )
                    .col_span(1),
                ),
            );
        }
        col = col
            .child(
                v_flex()
                    .gap_1p5()
                    .child(div().text_sm().child("Origem da mercadoria"))
                    .child(origens)
                    .child(div().text_xs().text_color(tema_muted).child(format!(
                        "{}: {}",
                        item.origem_mercadoria,
                        origem_da_mercadoria(item.origem_mercadoria)
                    ))),
            )
            .child(
                v_form()
                    .columns(3)
                    .child(
                        campo(
                            "CFOP",
                            false,
                            Some(SharedString::from(match &tabela {
                                Some(t) => format!("A tabela escolheu {t}. Vazio = a da tabela."),
                                None => "Vazio = o da tabela (depende do cliente).".to_string(),
                            })),
                            None,
                            self.texto_do_item(i, CampoItem::Cfop),
                        )
                        .col_span(1),
                    )
                    .child(
                        campo(
                            format!("{tipo} do ICMS"),
                            false,
                            Some(SharedString::from(match &tabela_icms {
                                Some(t) => format!("A tabela escolheu {t}. Vazio = a da tabela."),
                                None => "Vazio = o da tabela.".to_string(),
                            })),
                            None,
                            self.texto_do_item(i, CampoItem::IcmsCodigo),
                        )
                        .col_span(1),
                    )
                    .child(
                        campo(
                            "CST do PIS e da COFINS",
                            false,
                            Some(SharedString::from(format!(
                                "A tabela escolheu {tabela_pis}. Vazio = a da tabela."
                            ))),
                            None,
                            self.texto_do_item(i, CampoItem::PisCst),
                        )
                        .col_span(1),
                    ),
            );
        col.into_any_element()
    }
}
