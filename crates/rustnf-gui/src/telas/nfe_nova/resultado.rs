// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O passo "Conferir e enviar" (o resumo em palavras, os erros e avisos com o que fazer, o ambiente em destaque, a marcacao
//! de producao, a verificacao da SEFAZ e a contingencia EPEC) e as telas de desfecho: autorizada, sem resposta, SEFAZ fora,
//! recusada, EPEC. Nada aqui chama a rede: so' desenha o estado e liga os botoes a `envio`.

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::Input;
use gpui_kit::component::{
    h_flex, v_flex, ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, AnyElement, Context, Div, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, StatefulInteractiveElement as _, Styled as _,
};

use super::envio::{MOTIVO_MAX, MOTIVO_MIN};
use super::passos::centrada;
use super::{Etapa, NovaNfe};
use crate::modelo::certificado::Gravidade;
use crate::modelo::chave::em_grupos;
use crate::modelo::mascara::{centavos_para_dinheiro, documento_para_resumo};
use crate::modelo::nfe_config::ResultadoSefaz;
use crate::modelo::nfe_emissao::{
    depois_do_epec, por_que_nao_ha_epec, DesfechoNfeEmissao, O_QUE_E_EPEC, TEXTO_SEFAZ_FORA,
    TEXTO_SEM_RESPOSTA,
};
use crate::modelo::nfe_nova::{PassoNfe, PendenciaNfe};
use crate::modelo::nfe_resultados::contagem;
use crate::telas::pecas::{cartao, cor_da, faixa, grupo, linha_de_resumo, Negrito as _};

impl NovaNfe {
    /// O que esta acontecendo enquanto envia.
    pub(super) fn cartao_enviando(&self, etapa: Etapa, cx: &mut Context<Self>) -> AnyElement {
        let texto = match etapa {
            Etapa::Certificado => "Abrindo o certificado...",
            Etapa::Enviando => "Enviando a nota para a SEFAZ. Não feche a janela.",
            Etapa::Epec => {
                "Registrando a contingência (EPEC) no Ambiente Nacional. Não feche a janela."
            }
            Etapa::Retomando => "Consultando a SEFAZ pela mesma chave...",
        };
        cartao(cx)
            .id("nfe-enviando")
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(Icon::new(IconName::LoaderCircle))
                    .child(div().text_sm().child(texto)),
            )
            .into_any_element()
    }

    /// O aviso da ultima tentativa que nao deu certo (recusa da SEFAZ, nada enviado, EPEC recusado).
    pub(super) fn faixa_da_rejeicao(
        &mut self,
        r: &DesfechoNfeEmissao,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let muted = cx.theme().muted_foreground;
        let corpo: AnyElement = match r {
            DesfechoNfeEmissao::Rejeitada {
                cstat,
                motivo,
                o_que_fazer,
                ..
            } => v_flex()
                .gap_1()
                .child(div().font_semibold().child("A SEFAZ recusou a nota"))
                .child(div().child(motivo.clone()))
                .when(!cstat.is_empty(), |d| {
                    d.child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .child(format!("Código {cstat}")),
                    )
                })
                .child(div().text_sm().child(o_que_fazer.clone()))
                .into_any_element(),
            DesfechoNfeEmissao::Recusada { pendencias } => {
                let mut col = v_flex()
                    .gap_1()
                    .child(div().font_semibold().child("Nada foi enviado"));
                for p in pendencias {
                    col = col.child(self.linha_de_pendencia(p, cx));
                }
                col.into_any_element()
            }
            DesfechoNfeEmissao::EpecRecusado { texto } => v_flex()
                .gap_1()
                .child(
                    div()
                        .font_semibold()
                        .child("A contingência não foi registrada"),
                )
                .child(div().child(texto.clone()))
                .into_any_element(),
            _ => div().into_any_element(),
        };
        faixa(Gravidade::Erro, "", cx)
            .id("nfe-rejeicao")
            .child(div().flex_1().min_w_0().child(corpo))
            .child(
                Button::new("nfe-fechar-rejeicao")
                    .label("Entendi")
                    .ghost()
                    .small()
                    .on_click(cx.listener(|this, _, _, cx| this.fechar_rejeicao(cx))),
            )
            .into_any_element()
    }

    /// Uma pendencia: o que esta errado, o que fazer e o botao que leva ao passo.
    fn linha_de_pendencia(&self, p: &PendenciaNfe, cx: &mut Context<Self>) -> AnyElement {
        let muted = cx.theme().muted_foreground;
        let passo = p.passo;
        let travado = self.travado();
        h_flex()
            .gap_3()
            .items_start()
            .justify_between()
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_0p5()
                    .child(div().text_sm().child(p.mensagem.clone()))
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .child(p.como_resolver.clone()),
                    ),
            )
            .when(passo != self.passo || p.item.is_some(), |d| {
                d.child(
                    Button::new(SharedString::from(format!(
                        "nfe-ir-{}-{}",
                        passo.indice(),
                        p.mensagem.len()
                    )))
                    .label(format!("Ir para \"{}\"", passo.titulo()))
                    .outline()
                    .small()
                    .disabled(travado)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.ir_para_o_passo(passo, window, cx);
                    })),
                )
            })
            .into_any_element()
    }

    // ---------------------------------------------------------------- passo 5: conferir e enviar

    /// O resumo da nota em palavras.
    fn resumo_da_nota(&self, cx: &mut Context<Self>) -> AnyElement {
        let d = &self.dados;
        let t = d.totais();
        let destino = d
            .destino(&self.ctx)
            .map_or_else(String::new, |x| format!(" · venda {}", x.frase()));
        let mut col = v_flex()
            .gap_2()
            .id("nfe-resumo")
            .child(grupo("Resumo", None, cx))
            .child(linha_de_resumo(
                "Cliente",
                format!(
                    "{} ({}){destino}",
                    d.nome.trim(),
                    documento_para_resumo(&d.documento_limpo())
                ),
                cx,
            ));
        for (i, it) in d.itens.iter().enumerate() {
            col = col.child(linha_de_resumo(
                if i == 0 { "Produtos" } else { "" },
                format!(
                    "{} x {} - {}: R$ {}",
                    it.quantidade.trim(),
                    it.unidade.trim(),
                    it.descricao.trim(),
                    it.liquido_centavos()
                        .map_or_else(|| "-".to_string(), centavos_para_dinheiro)
                ),
                cx,
            ));
        }
        col = col
            .child(linha_de_resumo("Recebeu", d.pagamento.rotulo(), cx))
            .child(linha_de_resumo("Entrega", d.entrega.rotulo(), cx))
            .child(linha_de_resumo(
                "Total",
                div()
                    .font_semibold()
                    .child(format!("R$ {}", centavos_para_dinheiro(t.total))),
                cx,
            ));
        for (nome, valor) in &self.conferencia.impostos {
            col = col.child(linha_de_resumo(nome, format!("R$ {valor}"), cx));
        }
        if self.conferencia.numero > 0 {
            col = col.child(linha_de_resumo(
                "Número",
                format!(
                    "nº {}, série {}",
                    self.conferencia.numero, self.conferencia.serie
                ),
                cx,
            ));
        }
        col.into_any_element()
    }

    /// A marcacao de producao: sem ela, o botao de enviar nao habilita e nenhuma chamada sai.
    fn marcacao_de_producao(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.exige_efeito_fiscal() {
            return None;
        }
        Some(
            Checkbox::new("nfe-entendo-efeito-fiscal")
                .label("Entendo que a nota tem efeito fiscal")
                .checked(self.entendo)
                .disabled(self.travado())
                .on_click(cx.listener(|this, marcou: &bool, _, cx| {
                    this.entendo = *marcou;
                    cx.notify();
                }))
                .into_any_element(),
        )
    }

    /// A SEFAZ esta fora do ar? A pergunta, a resposta e, se a SEFAZ nao responde, a contingencia.
    fn bloco_da_sefaz(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let muted = cx.theme().muted_foreground;
        let verificando = self.verificando;
        let status = self.status.clone();
        let mut col = v_flex()
            .id("nfe-bloco-da-sefaz")
            .gap_2()
            .child(grupo(
                "A SEFAZ está fora do ar?",
                Some("Só pergunta se o serviço responde; nada é emitido."),
                cx,
            ))
            .child(
                h_flex().child(
                    Button::new("nfe-verificar-sefaz")
                        .label(if verificando {
                            "Verificando..."
                        } else {
                            "Verificar a SEFAZ"
                        })
                        .icon(IconName::RefreshCw)
                        .outline()
                        .small()
                        .disabled(verificando || self.travado() || !self.pronta)
                        .on_click(
                            cx.listener(|this, _, window, cx| this.verificar_a_sefaz(window, cx)),
                        ),
                ),
            );
        if let Some(s) = &status {
            let g = if s.ok() {
                Gravidade::Ok
            } else {
                Gravidade::Aviso
            };
            col = col.child(faixa(g, s.texto(), cx).id("nfe-resposta-da-sefaz"));
        }
        if self.epec_aberto
            && matches!(
                status,
                Some(ResultadoSefaz::Parado { .. } | ResultadoSefaz::SemResposta)
            )
        {
            col = col.child(self.cartao_da_contingencia(muted, cx));
        }
        col.into_any_element()
    }

    /// A contingencia EPEC: o que e', quando usar, o motivo e o botao.
    fn cartao_da_contingencia(
        &mut self,
        muted: gpui_kit::Hsla,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let n = self.texto_do_motivo(cx).chars().count();
        let pode = self.pode_epec(cx);
        let producao = self.exige_efeito_fiscal();
        let motivo = Input::new(&self.motivo).id("nfe-motivo-da-contingencia");
        let pendentes = self.pendentes;
        let mut c = v_flex()
            .id("nfe-contingencia")
            .gap_2()
            .p_3()
            .rounded_md()
            .border_1()
            .border_color(cx.theme().border)
            .child(div().font_semibold().child("Emitir em contingência (EPEC)"))
            .child(div().text_sm().child(O_QUE_E_EPEC))
            .child(div().text_xs().text_color(muted).child(
                "Use só quando a SEFAZ da sua UF está mesmo fora do ar. O motivo fica registrado na nota.",
            ));
        if pendentes > 0 {
            c = c.child(
                faixa(Gravidade::Aviso, por_que_nao_ha_epec(pendentes), cx)
                    .id("nfe-sem-epec-com-pendente"),
            );
        } else {
            c = c
                .child(motivo)
                .child(
                    div()
                        .text_xs()
                        .text_color(muted)
                        .child(format!("{n} de {MOTIVO_MIN} a {MOTIVO_MAX} letras.")),
                )
                .child(
                    h_flex().child(
                        Button::new("nfe-emitir-epec")
                            .label("Emitir em contingência (EPEC)")
                            .when(producao, |b| b.danger())
                            .when(!producao, |b| b.primary())
                            .disabled(!pode)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.emitir_em_contingencia(window, cx);
                            })),
                    ),
                );
            if producao && !self.entendo {
                c = c.child(div().text_xs().text_color(muted).child(
                    "Em produção, marque \"Entendo que a nota tem efeito fiscal\" acima para habilitar o botão.",
                ));
            }
        }
        c.into_any_element()
    }

    /// O passo "Conferir e enviar".
    pub(super) fn passo_conferir(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let ambiente = self.ambiente();
        let producao = self.exige_efeito_fiscal();
        let pode = self.pode_enviar();
        let conferindo = self.conferindo;
        let erros = self.conferencia.erros.clone();
        let avisos = self.conferencia.avisos.clone();
        let nao_conferiu = !conferindo && erros.is_empty() && self.conferencia.total.is_empty();
        let muted = cx.theme().muted_foreground;
        let mut col = v_flex().gap_4().child(self.resumo_da_nota(cx));
        if conferindo {
            col = col.child(
                h_flex()
                    .id("nfe-conferindo")
                    .gap_2()
                    .items_center()
                    .child(Icon::new(IconName::LoaderCircle))
                    .child(
                        div()
                            .text_sm()
                            .child("Conferindo a nota com as regras da SEFAZ..."),
                    ),
            );
        }
        if nao_conferiu {
            col = col.child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(div().text_sm().child("A nota ainda não foi conferida."))
                    .child(
                        Button::new("nfe-conferir-de-novo")
                            .label("Conferir agora")
                            .outline()
                            .small()
                            .on_click(cx.listener(|this, _, window, cx| this.conferir(window, cx))),
                    ),
            );
        }
        if !erros.is_empty() {
            let mut lista =
                v_flex()
                    .gap_2()
                    .id("nfe-erros-da-conferencia")
                    .child(div().font_semibold().child(if erros.len() == 1 {
                        "Antes de enviar, falta resolver uma coisa".to_string()
                    } else {
                        format!("Antes de enviar, faltam {} coisas", erros.len())
                    }));
            for e in &erros {
                lista = lista.child(self.linha_de_pendencia(e, cx));
            }
            col = col
                .child(faixa(Gravidade::Erro, "", cx).child(div().flex_1().min_w_0().child(lista)));
        }
        if !avisos.is_empty() {
            let mut lista = v_flex().gap_1().id("nfe-avisos-da-conferencia").child(
                div().font_semibold().child(if avisos.len() == 1 {
                    "Um ponto de atenção (não impede o envio)".to_string()
                } else {
                    format!("{} pontos de atenção (não impedem o envio)", avisos.len())
                }),
            );
            for a in &avisos {
                lista = lista.child(div().text_sm().child(match a.item {
                    Some(i) => format!("Produto {}: {}", i + 1, a.texto),
                    None => a.texto.clone(),
                }));
            }
            col = col.child(
                faixa(Gravidade::Aviso, "", cx).child(div().flex_1().min_w_0().child(lista)),
            );
        }
        let cores = ambiente.cores();
        col = col.child(
            h_flex()
                .id("nfe-ambiente-do-envio")
                .gap_3()
                .items_center()
                .p_3()
                .rounded_md()
                .border_1()
                .border_color(cx.theme().border)
                .child(
                    div()
                        .px_3()
                        .py_1()
                        .rounded_full()
                        .text_sm()
                        .font_semibold()
                        .bg(gpui_kit::rgb(cores.fundo))
                        .text_color(gpui_kit::rgb(cores.texto))
                        .child(crate::modelo::nfe_config::selo_do_ambiente_da_nfe(ambiente)),
                )
                .child(div().text_sm().child(if self.servicos.demonstracao_nfe() {
                    "Demonstração: nada é enviado de verdade."
                } else {
                    ambiente.efeito()
                })),
        );
        if let Some(m) = self.marcacao_de_producao(cx) {
            col = col.child(m);
        }
        col = col
            .child(
                h_flex().gap_2().items_center().child(
                    Button::new("nfe-enviar")
                        .label(if producao {
                            "Enviar a NF-e (vale de verdade)"
                        } else {
                            "Enviar a NF-e (teste)"
                        })
                        .icon(IconName::ArrowRight)
                        .when(producao, |b| b.danger())
                        .when(!producao, |b| b.primary())
                        .disabled(!pode)
                        .on_click(cx.listener(|this, _, window, cx| this.enviar(window, cx))),
                ),
            )
            .when(producao && !self.entendo, |d| {
                d.child(div().text_xs().text_color(muted).child(
                    "Em produção o botão só habilita depois da marcação acima. Nada sai sem ela.",
                ))
            })
            .child(self.bloco_da_sefaz(cx));
        col.into_any_element()
    }

    // ---------------------------------------------------------------- desfechos

    fn titulo_de_desfecho(
        &self,
        icone: IconName,
        cor: gpui_kit::Hsla,
        titulo: String,
        cx: &mut Context<Self>,
    ) -> Div {
        let _ = cx;
        h_flex()
            .gap_3()
            .items_center()
            .child(Icon::new(icone).large().text_color(cor))
            .child(div().text_xl().font_semibold().child(titulo))
    }

    fn quadro_da_chave(&self, chave: &str, cx: &mut Context<Self>) -> AnyElement {
        let muted = cx.theme().muted_foreground;
        let copiada = self.copiada;
        v_flex()
            .gap_1p5()
            .child(div().text_sm().text_color(muted).child("Chave de acesso"))
            .child(
                div()
                    .id("nfe-chave-de-acesso")
                    .p_3()
                    .rounded_md()
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().muted)
                    .text_sm()
                    .child(em_grupos(chave)),
            )
            .child(
                h_flex().child(
                    Button::new("nfe-copiar-chave")
                        .label(if copiada {
                            "Chave copiada"
                        } else {
                            "Copiar a chave"
                        })
                        .icon(if copiada {
                            IconName::Check
                        } else {
                            IconName::Copy
                        })
                        .outline()
                        .small()
                        .on_click(cx.listener(|this, _, _, cx| this.copiar_chave(cx))),
                ),
            )
            .into_any_element()
    }

    fn botoes_de_desfecho(&self, nova: bool, retomar: bool, cx: &mut Context<Self>) -> AnyElement {
        let producao = self.exige_efeito_fiscal();
        let pode_retomar = !producao || self.entendo;
        h_flex()
            .gap_2()
            .flex_wrap()
            .when(retomar, |d| {
                d.child(
                    Button::new("nfe-retomar")
                        .label("Retomar agora")
                        .icon(IconName::RefreshCw)
                        .primary()
                        .disabled(!pode_retomar)
                        .on_click(cx.listener(|this, _, window, cx| this.retomar(window, cx))),
                )
            })
            .child(
                Button::new("nfe-ver-na-lista")
                    .label("Ver na lista de notas")
                    .icon(IconName::Inbox)
                    .when(nova, |b| b.outline())
                    .when(!nova && !retomar, |b| b.primary())
                    .when(!nova && retomar, |b| b.outline())
                    .on_click(cx.listener(|this, _, _, cx| this.ver_na_lista(cx))),
            )
            .when(nova, |d| {
                d.child(
                    Button::new("nfe-nova-nota-limpa")
                        .label("Nova NF-e")
                        .icon(IconName::Plus)
                        .primary()
                        .on_click(cx.listener(|this, _, window, cx| this.nova_nfe(window, cx))),
                )
            })
            .into_any_element()
    }

    /// A tela do desfecho: autorizada, sem resposta, SEFAZ fora, denegada, numero ja usado ou EPEC registrado.
    pub(super) fn tela_do_desfecho(
        &mut self,
        d: DesfechoNfeEmissao,
        titulo: Div,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tema = cx.theme();
        let (sucesso, muted) = (tema.success, tema.muted_foreground);
        let aviso = cor_da(Gravidade::Aviso, cx);
        let erro = cx.theme().danger;
        let producao = self.exige_efeito_fiscal();
        let marcacao = self.marcacao_de_producao(cx);
        let mut caixa = cartao(cx).id("nfe-desfecho").gap_4();
        let (nova, retomar) = (d.concluiu() || !d.deixa_pendente(), d.deixa_pendente());
        match &d {
            DesfechoNfeEmissao::Autorizada {
                chave,
                numero,
                serie,
                protocolo,
                guardada,
                recuperada,
                avisos,
            } => {
                caixa = caixa
                    .child(self.titulo_de_desfecho(
                        IconName::CircleCheck,
                        sucesso,
                        format!("NF-e nº {numero} autorizada"),
                        cx,
                    ))
                    .child(linha_de_resumo("Série", serie.clone(), cx))
                    .child(linha_de_resumo("Protocolo", protocolo.clone(), cx))
                    .child(self.quadro_da_chave(chave, cx));
                if *recuperada {
                    caixa = caixa.child(faixa(
                        Gravidade::Neutro,
                        "A resposta da SEFAZ se perdeu e a consulta achou a nota: é a mesma nota, sem duplicidade.",
                        cx,
                    ));
                }
                caixa = caixa.child(if *guardada {
                    faixa(Gravidade::Ok, "A nota está guardada na pasta de notas. O DANFE abre na lista de notas.", cx)
                } else {
                    faixa(
                        Gravidade::Aviso,
                        "A nota foi autorizada, mas não ficou guardada como autorizada na pasta. Consulte-a na lista de notas.",
                        cx,
                    )
                });
                for a in avisos {
                    caixa = caixa.child(faixa(Gravidade::Aviso, a.clone(), cx));
                }
                if let Some(n) = self.produtos_guardados.filter(|n| *n > 0) {
                    caixa = caixa.child(faixa(
                        Gravidade::Neutro,
                        format!(
                            "{} no cadastro de produtos.",
                            contagem(
                                n,
                                "produto entrou ou foi atualizado",
                                "produtos entraram ou foram atualizados"
                            )
                        ),
                        cx,
                    ));
                }
            }
            DesfechoNfeEmissao::EpecRegistrado {
                chave,
                numero,
                serie,
                protocolo,
            } => {
                caixa = caixa
                    .child(self.titulo_de_desfecho(
                        IconName::TriangleAlert,
                        aviso,
                        format!("NF-e nº {numero} em contingência (EPEC)"),
                        cx,
                    ))
                    .child(linha_de_resumo("Série", serie.clone(), cx))
                    .child(linha_de_resumo("Protocolo do EPEC", protocolo.clone(), cx))
                    .child(self.quadro_da_chave(chave, cx))
                    .child(faixa(Gravidade::Aviso, depois_do_epec(), cx).id("nfe-depois-do-epec"));
            }
            DesfechoNfeEmissao::SemResposta {
                chave,
                numero,
                serie,
            } => {
                caixa = caixa
                    .child(self.titulo_de_desfecho(
                        IconName::TriangleAlert,
                        aviso,
                        "Sem resposta da SEFAZ".to_string(),
                        cx,
                    ))
                    .child(div().text_sm().child(TEXTO_SEM_RESPOSTA))
                    .child(linha_de_resumo(
                        "Nota",
                        format!("nº {numero}, série {serie}"),
                        cx,
                    ))
                    .child(self.quadro_da_chave(chave, cx));
            }
            DesfechoNfeEmissao::SefazFora {
                chave,
                numero,
                serie,
                motivo,
            } => {
                caixa = caixa
                    .child(self.titulo_de_desfecho(
                        IconName::TriangleAlert,
                        aviso,
                        "A SEFAZ está fora do ar".to_string(),
                        cx,
                    ))
                    .child(div().text_sm().child(motivo.clone()))
                    .child(div().text_sm().child(TEXTO_SEFAZ_FORA))
                    .child(linha_de_resumo(
                        "Nota",
                        format!("nº {numero}, série {serie}"),
                        cx,
                    ))
                    .child(self.quadro_da_chave(chave, cx));
            }
            DesfechoNfeEmissao::Denegada { motivo } => {
                caixa = caixa
                    .child(self.titulo_de_desfecho(IconName::CircleAlert, erro, "A SEFAZ negou o uso da nota".to_string(), cx))
                    .child(div().text_sm().child(motivo.clone()))
                    .child(div().text_sm().child(
                        "O número foi gasto e esta nota não vale. Fale com o seu contador sobre a situação do cliente ou do emitente na SEFAZ e emita outra nota depois.",
                    ));
            }
            DesfechoNfeEmissao::NumeroJaUsado { texto } => {
                caixa = caixa
                    .child(self.titulo_de_desfecho(
                        IconName::CircleAlert,
                        erro,
                        "O número já estava usado".to_string(),
                        cx,
                    ))
                    .child(div().text_sm().child(texto.clone()));
            }
            _ => {}
        }
        if retomar {
            if let Some(r) = self.retomada.clone() {
                let g = r.gravidade;
                let mut col = v_flex()
                    .gap_1()
                    .id("nfe-resumo-da-retomada")
                    .child(div().child(r.texto.clone()));
                for l in &r.linhas {
                    col = col.child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .child(format!("Nota nº {}: {}", l.numero, l.texto)),
                    );
                }
                caixa = caixa.child(faixa(g, "", cx).child(div().flex_1().min_w_0().child(col)));
            }
            if producao {
                caixa = caixa.children(marcacao);
            }
        }
        caixa = caixa.child(self.botoes_de_desfecho(nova, retomar, cx));
        let _: Option<PassoNfe> = None;
        div()
            .id("nfe-resultado")
            .size_full()
            .overflow_y_scroll()
            .child(
                v_flex()
                    .w_full()
                    .max_w(px(680.))
                    .mx_auto()
                    .gap_5()
                    .px_6()
                    .py_8()
                    .child(centrada(titulo))
                    .child(caixa),
            )
            .into_any_element()
    }
}
