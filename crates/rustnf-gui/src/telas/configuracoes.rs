// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A pagina de Configuracoes: secoes em abas (emitente, Sefin Nacional, notas e DANFSe, aliquotas, aparencia), largura
//! maxima de formulario e a barra com Salvar e Descartar.

use gpui_kit::component::button::{Button, ButtonVariants as _, Toggle, ToggleGroup};
use gpui_kit::component::form::v_form;
use gpui_kit::component::input::Input;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::{
    h_flex, v_flex, ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _,
    WindowExt as _,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, StatefulInteractiveElement as _, Styled as _, Window,
};

use super::pecas::{cabecalho, campo, faixa, Negrito as _, LARGURA_FORM};
use super::raiz::{Raiz, Secao};
use crate::modelo::ambiente::{Ambiente, Tema};
use crate::modelo::campo::{Campo, CampoAliquota};
use crate::modelo::certificado::Gravidade;
use crate::modelo::formulario::Assinatura;

impl Raiz {
    /// Pergunta antes de apagar a senha do cofre.
    pub(super) fn pedir_para_apagar_senha(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let raiz = cx.entity();
        window.open_alert_dialog(cx, move |alerta, _, _| {
            let raiz = raiz.clone();
            alerta
                .title("Apagar a senha do cofre?")
                .description(
                    "A senha deste certificado será removida do cofre do Windows. Para emitir notas de novo você precisará guardá-la outra vez.",
                )
                .confirm()
                .ok_text("Apagar")
                .cancel_text("Cancelar")
                .on_ok(move |_, window, cx| {
                    raiz.update(cx, |this, cx| this.apagar_senha(window, cx));
                    true
                })
        });
    }

    fn abas(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let atual = Secao::TODAS
            .iter()
            .position(|s| *s == self.secao)
            .unwrap_or(0);
        TabBar::new("secoes")
            .selected_index(atual)
            .children(Secao::TODAS.map(|s| Tab::new().label(s.titulo())))
            .on_click(cx.listener(|this, indice: &usize, _, cx| {
                if let Some(s) = Secao::TODAS.get(*indice) {
                    this.secao = *s;
                    cx.notify();
                }
            }))
    }

    fn secao_emitente(&mut self, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .gap_5()
            .child(self.titulo_do_grupo(
                "Dados do emitente",
                Some("Completam cada pedido de nota; nunca sobrescrevem o que o pedido já traz."),
                cx,
            ))
            .child(self.campos_emitente(true, cx))
            .into_any_element()
    }

    fn secao_sefin(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let atual = self.formulario.dados.ambiente;
        let producao = atual == Ambiente::Producao;
        let assinatura = self.formulario.dados.assinatura;
        let erros = self.erros_visiveis();
        let tempo_erro = erros.do_campo(Campo::TempoLimite).map(str::to_string);
        let folga_erro = erros.do_campo(Campo::FolgaRelogio).map(str::to_string);
        let ocupado_cert = self.ocupado.certificado;
        let ocupado_conexao = self.ocupado.conexao;
        let resultado = self.resultado_do_certificado(cx);
        let conexao = self.resultado_conexao.clone();

        let ambientes = ToggleGroup::new("ambiente")
            .segmented()
            .single()
            .children([Ambiente::Restrita, Ambiente::Producao].map(|a| {
                Toggle::new(SharedString::from(format!("ambiente-{}", a.para_config())))
                    .label(a.selo())
                    .checked(atual == a)
            }))
            .on_click(cx.listener(|this, estados: &Vec<bool>, _, cx| {
                if let Some(i) = estados.iter().position(|e| *e) {
                    this.formulario.dados.ambiente = [Ambiente::Restrita, Ambiente::Producao][i];
                    this.resultado_conexao = None;
                }
                cx.notify();
            }));
        let assinaturas = ToggleGroup::new("assinatura")
            .segmented()
            .single()
            .children(Assinatura::TODAS.map(|a| {
                Toggle::new(SharedString::from(format!(
                    "assinatura-{}",
                    a.para_config()
                )))
                .label(a.rotulo())
                .checked(assinatura == a)
            }))
            .on_click(cx.listener(|this, estados: &Vec<bool>, _, cx| {
                if let Some(i) = estados.iter().position(|e| *e) {
                    this.formulario.dados.assinatura = Assinatura::TODAS[i];
                }
                cx.notify();
            }));

        v_flex()
            .gap_6()
            .child(
                v_flex()
                    .gap_3()
                    .child(self.titulo_do_grupo(
                        "Ambiente",
                        Some("Produção restrita é o ambiente de testes da Sefin: use-o até o fluxo estar certo."),
                        cx,
                    ))
                    .child(campo("Ambiente da Sefin Nacional", true, None, None, ambientes))
                    .child(if producao {
                        faixa(
                            Gravidade::Aviso,
                            "Em produção, as notas emitidas têm efeito fiscal.",
                            cx,
                        )
                    } else {
                        faixa(
                            Gravidade::Ok,
                            "Produção restrita: ambiente de testes, sem efeito fiscal.",
                            cx,
                        )
                    }),
            )
            .child(
                v_flex()
                    .gap_3()
                    .child(self.titulo_do_grupo(
                        "Certificado digital A1",
                        Some("A senha é guardada só no cofre do Windows."),
                        cx,
                    ))
                    .child(self.campo_do_certificado(cx))
                    .child(self.campo_da_senha(true, cx))
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("testar-certificado")
                                    .label("Testar certificado")
                                    .icon(IconName::CircleCheck)
                                    .outline()
                                    .loading(ocupado_cert)
                                    .disabled(ocupado_cert)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.testar_certificado(window, cx);
                                    })),
                            )
                            .child(
                                Button::new("testar-conexao")
                                    .label("Testar conexão com a Sefin")
                                    .icon(IconName::Globe)
                                    .outline()
                                    .loading(ocupado_conexao)
                                    .disabled(ocupado_conexao)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.testar_conexao(window, cx);
                                    })),
                            ),
                    )
                    .children(resultado)
                    .children(conexao.map(|(ok, texto)| {
                        faixa(
                            if ok { Gravidade::Ok } else { Gravidade::Erro },
                            texto,
                            cx,
                        )
                        .id("resultado-da-conexao")
                    }))
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child("\"Testar conexão\" faz uma chamada de verdade à Sefin Nacional, com o certificado, no ambiente escolhido acima."),
                    ),
            )
            .child(
                v_flex()
                    .gap_3()
                    .child(self.titulo_do_grupo("Avançado", None, cx))
                    .child(
                        v_form()
                            .columns(2)
                            .child(
                                campo("Assinatura da DPS e dos eventos", false, None, None, assinaturas)
                                    .col_span(2),
                            )
                            .child(campo(
                                "Tempo limite da Sefin (segundos)",
                                false,
                                Some("De 1 a 600.".into()),
                                tempo_erro,
                                Input::new(&self.entradas.tempo).id("tempo-limite"),
                            ))
                            .child(campo(
                                "Folga do relógio (segundos)",
                                false,
                                Some("Tirada do horário ao preencher a emissão. Até 3600.".into()),
                                folga_erro,
                                Input::new(&self.entradas.folga).id("folga-do-relogio"),
                            )),
                    ),
            )
            .into_any_element()
    }

    fn secao_notas(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let canhoto = self.formulario.dados.canhoto;
        v_flex()
            .gap_6()
            .child(
                v_flex()
                    .gap_3()
                    .child(self.titulo_do_grupo("Notas", None, cx))
                    .child(self.campo_da_pasta(false, cx)),
            )
            .child(self.bloco_da_numeracao(cx))
            .child(
                v_flex()
                    .gap_3()
                    .child(self.titulo_do_grupo(
                        "DANFSe",
                        Some("O documento auxiliar da NFS-e, em PDF."),
                        cx,
                    ))
                    .child(
                        Switch::new("canhoto")
                            .checked(canhoto)
                            .label("Canhoto de cientificação no rodapé")
                            .on_click(cx.listener(|this, marcado: &bool, _, cx| {
                                this.formulario.dados.canhoto = *marcado;
                                cx.notify();
                            })),
                    ),
            )
            .into_any_element()
    }

    /// A numeracao da DPS: o proximo numero da serie e, para quem ja emitia por outro sistema, onde continuar.
    fn bloco_da_numeracao(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let erro = self.erros_externos.get(&Campo::ProximoDps).cloned();
        let pasta_gravada = !self.salvo.notas.pasta.trim().is_empty();
        let ocupado = self.ocupado.numero;
        let serie = self
            .servicos
            .contexto_da_nota(&self.servicos.efetiva(&self.salvo))
            .serie();
        let situacao = match (pasta_gravada, self.proximo_dps) {
            (false, _) => "Salve a pasta das notas para ver e ajustar a numeração.".to_string(),
            (true, None) => "Lendo a numeração da pasta de notas...".to_string(),
            (true, Some(n)) => format!("Série {serie}: a próxima DPS será a nº {n}."),
        };
        let muted = cx.theme().muted_foreground;
        v_flex()
            .gap_3()
            .child(self.titulo_do_grupo(
                "Numeração da DPS",
                Some("Cada série numera as suas DPS em sequência, a partir da 1."),
                cx,
            ))
            .child(
                div()
                    .id("proxima-dps")
                    .text_sm()
                    .child(situacao),
            )
            .child(campo(
                "Já emitia por outro sistema?",
                false,
                Some(
                    "Informe o número seguinte ao da última DPS que você emitiu lá. A numeração só anda para a frente."
                        .into(),
                ),
                erro,
                h_flex()
                    .gap_2()
                    .child(
                        div()
                            .w(px(180.))
                            .child(Input::new(&self.entradas.proximo).id("proximo-numero-dps")),
                    )
                    .child(
                        Button::new("continuar-deste-numero")
                            .label("Continuar deste número")
                            .outline()
                            .loading(ocupado)
                            .disabled(!pasta_gravada || ocupado)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.pedir_proximo_dps(window, cx);
                            })),
                    ),
            ))
            .child(
                div()
                    .text_xs()
                    .text_color(muted)
                    .child("Na linha de comando: rustnf notas numero --proximo <N>."),
            )
            .into_any_element()
    }

    fn secao_aliquotas(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let erros = self.erros_visiveis();
        let tema = cx.theme();
        let muted = tema.muted_foreground;
        let borda = tema.border;
        let linhas = self.entradas.aliquotas.clone();
        // a fonte, que e' texto livre, fica com o que sobra da largura
        let larguras = [60., 92., 76., 96., 108.];

        let mut cabecalho_da_tabela = h_flex()
            .gap_2()
            .pb_1()
            .text_xs()
            .font_semibold()
            .text_color(muted);
        for (coluna, largura) in CampoAliquota::TODOS
            .iter()
            .zip(larguras.iter().map(Some).chain([None]))
        {
            cabecalho_da_tabela = cabecalho_da_tabela.child(match largura {
                Some(l) => div().w(px(*l)).flex_shrink_0().child(coluna.rotulo()),
                None => div().flex_1().min_w_0().child(coluna.rotulo()),
            });
        }
        cabecalho_da_tabela = cabecalho_da_tabela.child(div().w(px(36.)).flex_shrink_0());

        let mut tabela = v_flex().gap_2().child(cabecalho_da_tabela);
        for (i, linha) in linhas.iter().enumerate() {
            let mut celulas = h_flex().gap_2().items_start();
            for (j, coluna) in CampoAliquota::TODOS.iter().enumerate() {
                let mut entrada =
                    Input::new(&linha[j]).id(SharedString::from(format!("aliquota-{i}-{j}")));
                // percentuais alinhados a direita, para comparar entre as linhas; so' a fonte e' texto
                if *coluna != CampoAliquota::Fonte {
                    entrada = entrada.text_right();
                }
                celulas = celulas.child(match larguras.get(j) {
                    Some(l) => div().w(px(*l)).flex_shrink_0().child(entrada),
                    None => div().flex_1().min_w_0().child(entrada),
                });
            }
            celulas = celulas.child(
                div().w(px(36.)).flex_shrink_0().child(
                    Button::new(SharedString::from(format!("remover-aliquota-{i}")))
                        .icon(IconName::Delete)
                        .ghost()
                        .tooltip("Remover esta linha")
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.remover_aliquota(i, window, cx);
                        })),
                ),
            );
            let mut bloco = v_flex().gap_1().child(celulas);
            for coluna in CampoAliquota::TODOS {
                if let Some(e) = erros.da_aliquota(i, coluna) {
                    bloco = bloco.child(
                        h_flex()
                            .gap_1()
                            .text_xs()
                            .text_color(cx.theme().danger)
                            .child(Icon::new(IconName::CircleAlert).xsmall())
                            .child(format!("{}: {e}", coluna.rotulo())),
                    );
                }
            }
            tabela = tabela.child(bloco);
        }

        let vazia = linhas.is_empty();
        v_flex()
            .gap_4()
            .child(self.titulo_do_grupo(
                "Alíquotas publicadas",
                Some("Percentuais por ano, usados pela prévia do IBS/CBS (rustnf calcular) quando o pedido não traz os seus. De 2027 a 2028 use a CBS de referência ou a CBS; de 2029 em diante, CBS, IBS estadual e IBS municipal."),
                cx,
            ))
            .when(vazia, |d| {
                d.child(
                    v_flex()
                        .id("aliquotas-vazias")
                        .gap_2()
                        .items_center()
                        .p_6()
                        .rounded_lg()
                        .border_1()
                        .border_dashed()
                        .border_color(borda)
                        .child(Icon::new(IconName::Inbox).large().text_color(muted))
                        .child(div().font_medium().child("Nenhuma alíquota cadastrada"))
                        .child(
                            div()
                                .text_sm()
                                .text_color(muted)
                                .child("Sem a tabela, a prévia de 2027 em diante usa o que a biblioteca assume. Acrescente um ano para usar o número publicado."),
                        ),
                )
            })
            .when(!vazia, |d| d.child(tabela))
            .child(
                h_flex().child(
                    Button::new("adicionar-aliquota")
                        .label("Adicionar ano")
                        .icon(IconName::Plus)
                        .outline()
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.adicionar_aliquota(window, cx);
                        })),
                ),
            )
            .into_any_element()
    }

    fn secao_aparencia(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let atual = self.formulario.dados.tema;
        v_flex()
            .gap_3()
            .child(self.titulo_do_grupo(
                "Tema",
                Some("\"Sistema\" acompanha o tema do Windows e muda junto com ele."),
                cx,
            ))
            .child(campo(
                "Tema da interface",
                false,
                None,
                None,
                ToggleGroup::new("tema")
                    .segmented()
                    .single()
                    .children(Tema::TODOS.map(|t| {
                        Toggle::new(SharedString::from(format!("tema-{}", t.para_config())))
                            .label(t.rotulo())
                            .checked(atual == t)
                    }))
                    .on_click(cx.listener(|this, estados: &Vec<bool>, window, cx| {
                        if let Some(i) = estados.iter().position(|e| *e) {
                            this.formulario.dados.tema = Tema::TODOS[i];
                            this.aplicar_tema(window, cx);
                        }
                        cx.notify();
                    })),
            ))
            .into_any_element()
    }

    fn barra_de_salvar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tema = cx.theme();
        let alterado = self.alterado();
        let motivo = self.motivo_sem_salvar();
        let salvando = self.ocupado.salvando;
        let (grav, texto) = match (&motivo, alterado) {
            (None, _) => (Gravidade::Aviso, "Há alterações não salvas.".to_string()),
            (Some(m), true) => (Gravidade::Erro, m.clone()),
            (Some(_), false) => match self.formulario.erros().primeira() {
                Some(p) => (
                    Gravidade::Erro,
                    format!("O arquivo tem valores a corrigir: {p}"),
                ),
                None => (Gravidade::Neutro, "Tudo salvo.".to_string()),
            },
        };
        let cor = super::pecas::cor_da(grav, cx);
        h_flex()
            .px_6()
            .py_3()
            .gap_4()
            .items_center()
            .justify_between()
            .border_t_1()
            .border_color(tema.border)
            .bg(tema.background)
            .child(
                h_flex()
                    .id("motivo-do-salvar")
                    .gap_2()
                    .items_center()
                    .flex_1()
                    .min_w_0()
                    .text_sm()
                    .text_color(cor)
                    .child(Icon::new(super::pecas::icone_da(grav)).small())
                    .child(div().flex_1().min_w_0().child(texto)),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("descartar")
                            .label("Descartar")
                            .outline()
                            .disabled(!alterado || salvando)
                            .on_click(
                                cx.listener(|this, _, window, cx| this.descartar(window, cx)),
                            ),
                    )
                    .child(
                        Button::new("salvar")
                            .label("Salvar")
                            .primary()
                            .loading(salvando)
                            .disabled(motivo.is_some() || salvando)
                            .on_click(cx.listener(|this, _, window, cx| this.salvar(window, cx))),
                    ),
            )
    }

    /// O conteudo da tela Configuracoes.
    pub(super) fn tela_configuracoes(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let _ = window;
        let secao = match self.secao {
            Secao::Emitente => self.secao_emitente(cx),
            Secao::Sefin => self.secao_sefin(cx),
            Secao::Notas => self.secao_notas(cx),
            Secao::Nfe => self.secao_nfe(cx),
            Secao::Aliquotas => self.secao_aliquotas(cx),
            Secao::Aparencia => self.secao_aparencia(cx),
        };
        let aviso = self.aviso.clone();
        let variaveis = self
            .info
            .as_ref()
            .map(|i| i.variaveis.join(", "))
            .filter(|v| !v.is_empty());
        let geral = self.formulario.erros().gerais;
        let arquivo = self.info.as_ref().map(|i| (i.nome.clone(), i.novo));
        let abas = self.abas(cx);
        let barra = self.barra_de_salvar(cx);
        let tema = cx.theme();
        let muted = tema.muted_foreground;

        v_flex()
            .size_full()
            .child(
                div()
                    .id("configuracoes")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.rolagem)
                    .child(
                        v_flex()
                            .w_full()
                            .max_w(px(LARGURA_FORM))
                            .gap_5()
                            .px_6()
                            .py_6()
                            .child(cabecalho(
                                "Configurações",
                                "O mesmo arquivo que a linha de comando lê. Salve com Ctrl+S; descarte o que mudou com Ctrl+Shift+D.",
                                cx,
                            ))
                            .when_some(arquivo, |d, (nome, novo)| {
                                d.child(div().text_xs().text_color(muted).child(if novo {
                                    format!("Arquivo: {nome} (ainda não existe; será criado ao salvar)")
                                } else {
                                    format!("Arquivo: {nome}")
                                }))
                            })
                            .when_some(variaveis, |d, v| {
                                d.child(faixa(
                                    Gravidade::Aviso,
                                    format!("As variáveis de ambiente {v} valem no lugar do arquivo quando o rustnf roda. Esta tela edita o arquivo."),
                                    cx,
                                ))
                            })
                            .children(geral.into_iter().map(|g| faixa(Gravidade::Erro, g, cx)))
                            .when_some(aviso, |d, a| {
                                d.child(faixa(
                                    if a.erro { Gravidade::Erro } else { Gravidade::Ok },
                                    a.texto,
                                    cx,
                                ))
                            })
                            .child(abas)
                            .child(secao),
                    ),
            )
            .child(barra)
            .into_any_element()
    }
}
