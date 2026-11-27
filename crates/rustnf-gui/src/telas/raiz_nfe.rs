// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O que a `Raiz` sabe da NF-e: a escolha "NFS-e | NF-e" da tela Notas (lembrada de uma vez para outra), os campos da
//! secao NF-e das Configuracoes (que gravam pela mesma via da NFS-e: `Servicos::salvar`, num arquivo so'), o "Testar
//! conexao com a SEFAZ" e a conta de "o que mudou" que junta o formulario da NFS-e com o da NF-e.

use std::collections::BTreeMap;
use std::sync::Arc;

use gpui_kit::component::button::{Toggle, ToggleGroup};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::WindowExt as _;
use gpui_kit::{AppContext as _, Context, Entity, IntoElement, ScrollHandle, SharedString, Window};
use rustnf_app::config::Config;

use super::nfe::TelaNfe;
use super::nfe_dialogo::DialogoNfe;
use super::nfe_nova::{EventoNovaNfe, ModeloRecebido, NovaNfe};
use super::produtos::TelaProdutos;
use super::raiz::{Raiz, Secao, Tela};
use crate::fora;
use crate::modelo::ambiente::Ambiente;
use crate::modelo::nfe_config::{ambiente_da_nfe, CampoNfe, ResultadoSefaz};
use crate::modelo::nfe_lista::EscolhaDeNotas;
use crate::modelo::nfe_modelo::ModeloNfe;
use crate::modelo::passos::configuracao_completa;
use crate::servicos::{testar_sefaz, Documento, ErroTela, Servicos};

/// Os campos de texto da secao NF-e.
pub(super) struct EntradasNfe {
    pub(super) por_campo: BTreeMap<CampoNfe, Entity<InputState>>,
}

fn dica(c: CampoNfe) -> &'static str {
    match c {
        CampoNfe::Uf => "SP",
        CampoNfe::Ie => "Só dígitos",
        CampoNfe::Serie => "1",
        CampoNfe::Fantasia => "Nome da loja (opcional)",
        CampoNfe::Logradouro => "Rua, avenida...",
        CampoNfe::Numero => "S/N se não tem",
        CampoNfe::Complemento => "Opcional",
        CampoNfe::Bairro => "Bairro",
        CampoNfe::CodMunicipio => "Vazio: o do Emitente",
        CampoNfe::Municipio => "Nome do município",
        CampoNfe::Cep => "00000-000",
        CampoNfe::Fone => "(11) 3333-4444",
        CampoNfe::MotivoContingencia => "Por que está emitindo em contingência",
        CampoNfe::Tempo => "60",
        CampoNfe::Tentativas => "5",
        CampoNfe::Intervalo => "3",
    }
}

impl Raiz {
    /// Os campos da secao NF-e, ainda vazios.
    pub(super) fn criar_entradas_nfe(window: &mut Window, cx: &mut Context<Self>) -> EntradasNfe {
        let por_campo = CampoNfe::TODOS
            .into_iter()
            .map(|c| {
                (
                    c,
                    cx.new(|cx| InputState::new(window, cx).placeholder(dica(c))),
                )
            })
            .collect();
        EntradasNfe { por_campo }
    }

    /// O campo de texto da secao NF-e.
    pub(super) fn entrada_nfe(&self, c: CampoNfe) -> Option<&Entity<InputState>> {
        self.entradas_nfe.por_campo.get(&c)
    }

    /// A lista de NF-e da tela Notas e o dialogo dela.
    pub(super) fn criar_lista_nfe(
        servicos: &Arc<Servicos>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> (Entity<DialogoNfe>, Entity<TelaNfe>) {
        let s = Arc::clone(servicos);
        let dialogo = cx.new(|cx| DialogoNfe::new(s, Documento::Nfe, window, cx));
        let (s, d) = (Arc::clone(servicos), dialogo.clone());
        let lista = cx.new(|cx| TelaNfe::new(s, d, Documento::Nfe, window, cx));
        (dialogo, lista)
    }

    /// As telas da emissao da NF-e: a Nova NF-e (assistente) e o cadastro de produtos.
    pub(super) fn criar_telas_nfe(
        servicos: &Arc<Servicos>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> (Entity<NovaNfe>, Entity<TelaProdutos>) {
        let (a, b) = (Arc::clone(servicos), Arc::clone(servicos));
        (
            cx.new(|cx| NovaNfe::new(a, window, cx)),
            cx.new(|cx| TelaProdutos::new(b, window, cx)),
        )
    }

    /// Entrega a configuracao que vale a Nova NF-e e ao cadastro de produtos.
    pub(super) fn entregar_config_nova_nfe(
        &mut self,
        efetiva: &Config,
        pronta: bool,
        completa: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let e = efetiva.clone();
        self.produtos
            .update(cx, |p, cx| p.definir_config(e, completa, cx));
        let e = efetiva.clone();
        self.nova_nfe
            .update(cx, |n, cx| n.definir_config(e, pronta, window, cx));
    }

    /// O que a tela Produtos faz ao abrir: le o cadastro de novo.
    pub(super) fn ao_abrir_tela_nfe(&mut self, tela: Tela, cx: &mut Context<Self>) {
        if tela == Tela::Produtos {
            self.produtos.update(cx, |p, cx| p.recarregar(cx));
        }
    }

    /// A rolagem da Nova nota aberta: a do assistente da NF-e ou a da NFS-e, pela escolha lembrada.
    pub(super) fn rolagem_da_nova_nota(&self, cx: &gpui_kit::App) -> ScrollHandle {
        match self.escolha_notas.da_emissao() {
            EscolhaDeNotas::Nfe => self.nova_nfe.read(cx).rolagem.clone(),
            EscolhaDeNotas::Nfce => self.caixa.read(cx).rolagem.clone(),
            _ => self.nova.read(cx).rolagem.clone(),
        }
    }

    /// Redesenha a Nova nota aberta (depois de rolar pelo teclado).
    pub(super) fn notificar_nova_nota(&mut self, cx: &mut Context<Self>) {
        match self.escolha_notas.da_emissao() {
            EscolhaDeNotas::Nfe => self.nova_nfe.update(cx, |_, cx| cx.notify()),
            EscolhaDeNotas::Nfce => self.caixa.update(cx, |_, cx| cx.notify()),
            _ => self.nova.update(cx, |_, cx| cx.notify()),
        }
    }

    /// Liga os campos da secao NF-e e a tela NF-e (chamar uma vez, depois de criar a janela).
    pub(super) fn ligar_nfe(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let pares: Vec<(CampoNfe, Entity<InputState>)> = self
            .entradas_nfe
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
                        let normal = this.form_nfe.definir(campo, &texto);
                        if normal != texto {
                            entrada.update(cx, |estado, cx| estado.set_value(normal, window, cx));
                        }
                        this.resultado_sefaz = None;
                        cx.notify();
                    }
                },
            );
            self.fixas.push(s);
        }
        let s = cx.subscribe_in(
            &self.nfe.clone(),
            window,
            |this, _, evento: &super::nfe::EventoNfe, window, cx| match evento {
                super::nfe::EventoNfe::IrParaConfiguracoes => {
                    this.tela = Tela::Configuracoes;
                    this.secao = Secao::Nfe;
                    cx.notify();
                }
                super::nfe::EventoNfe::UsarComoModelo(chave) => {
                    this.usar_nfe_como_modelo(chave.clone(), window, cx);
                }
            },
        );
        self.fixas.push(s);
        let s = cx.subscribe_in(
            &self.nova_nfe.clone(),
            window,
            |this, _, evento: &EventoNovaNfe, _, cx| {
                match evento {
                    EventoNovaNfe::NotaMudou => this.nfe.update(cx, |n, cx| n.recarregar(cx)),
                    EventoNovaNfe::IrParaConfiguracoes => {
                        this.tela = Tela::Configuracoes;
                        this.secao = Secao::Nfe;
                    }
                    EventoNovaNfe::IrParaNotas => {
                        this.escolher_notas(EscolhaDeNotas::Nfe, cx);
                        this.ir_para(Tela::Notas, cx);
                    }
                }
                cx.notify();
            },
        );
        self.fixas.push(s);
    }

    /// Poe nos campos da tela o texto do formulario da NF-e (sem disparar `Change` de volta).
    pub(super) fn sincronizar_nfe(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for c in CampoNfe::TODOS {
            let texto = self.form_nfe.dados.texto(c).to_string();
            if let Some(e) = self.entrada_nfe(c) {
                e.update(cx, |estado, cx| estado.set_value(texto, window, cx));
            }
        }
        self.resultado_sefaz = None;
    }

    // ---------------------------------------------------------------- o que mudou, junto

    /// A configuracao que o formulario inteiro representa: a da NFS-e mais a secao NF-e.
    pub(super) fn config_para_gravar(&self) -> Config {
        let mut c = self.formulario.para_config();
        self.form_nfe.aplicar(&mut c);
        self.form_nfce.aplicar(&mut c);
        c
    }

    /// Mudou alguma coisa (na NFS-e ou na NF-e) desde que foi carregado ou gravado.
    pub(super) fn alterado(&self) -> bool {
        self.formulario.alterado() || self.form_nfe.alterado() || self.form_nfce.alterado()
    }

    /// Por que o arquivo nao pode ser gravado agora, ou `None`.
    pub(super) fn motivo_sem_salvar(&self) -> Option<String> {
        if !self.alterado() {
            return Some("Nenhuma alteração para salvar.".into());
        }
        if let Some(m) = self.form_nfe.motivo_sem_salvar(&self.config_para_gravar()) {
            return Some(m);
        }
        if let Some(m) = self.form_nfce.motivo_sem_salvar(&self.config_para_gravar()) {
            return Some(m);
        }
        if self.formulario.alterado() {
            return self.formulario.motivo_sem_salvar();
        }
        None
    }

    // ---------------------------------------------------------------- a escolha NFS-e | NF-e

    /// Le a escolha lembrada da pasta de notas, fora da tela.
    pub(super) fn carregar_escolha(&mut self, efetiva: Config, cx: &mut Context<Self>) {
        let servicos = Arc::clone(&self.servicos);
        cx.spawn(async move |this, cx| {
            let escolha = cx
                .background_spawn(async move { servicos.ler_escolha_de_notas(&efetiva) })
                .await;
            this.update(cx, |this, cx| {
                this.escolha_notas = escolha;
                match escolha {
                    EscolhaDeNotas::Nfe => this.nfe.update(cx, |n, cx| n.recarregar(cx)),
                    EscolhaDeNotas::Nfce => this.nfce.update(cx, |n, cx| n.recarregar(cx)),
                    EscolhaDeNotas::Recebidas => this.recarregar_recebidas(cx),
                    EscolhaDeNotas::Nfse => {}
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// A pessoa escolheu NFS-e ou NF-e: troca a lista e lembra a escolha.
    pub(super) fn escolher_notas(&mut self, escolha: EscolhaDeNotas, cx: &mut Context<Self>) {
        if self.escolha_notas == escolha {
            return;
        }
        self.escolha_notas = escolha;
        let (servicos, efetiva) = (
            Arc::clone(&self.servicos),
            self.servicos.efetiva(&self.salvo),
        );
        cx.background_spawn(async move { servicos.gravar_escolha_de_notas(&efetiva, escolha) })
            .detach();
        match escolha {
            EscolhaDeNotas::Nfe => self.nfe.update(cx, |n, cx| n.recarregar(cx)),
            EscolhaDeNotas::Nfce => self.nfce.update(cx, |n, cx| n.recarregar(cx)),
            EscolhaDeNotas::Recebidas => self.recarregar_recebidas(cx),
            EscolhaDeNotas::Nfse => self.notas.update(cx, |n, cx| n.recarregar(cx)),
        }
        cx.notify();
    }

    /// O seletor "NFS-e | NF-e | NFC-e | Recebidas" do alto da tela Notas. Na Nova nota (`de_emissao`) so' ha o que se
    /// emite: as recebidas nao se emitem.
    pub(super) fn seletor_de_notas(
        &self,
        de_emissao: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let opcoes: &'static [EscolhaDeNotas] = if de_emissao {
            &EscolhaDeNotas::DE_EMISSAO
        } else {
            &EscolhaDeNotas::TODAS
        };
        let atual = if de_emissao {
            self.escolha_notas.da_emissao()
        } else {
            self.escolha_notas
        };
        ToggleGroup::new("notas-documento")
            .segmented()
            .single()
            .children(opcoes.iter().enumerate().map(|(i, e)| {
                Toggle::new(SharedString::from(format!("notas-documento-{i}")))
                    .label(e.rotulo())
                    .checked(atual == *e)
            }))
            .on_click(cx.listener(move |this, estados: &Vec<bool>, _, cx| {
                if let Some(e) = estados.iter().position(|e| *e).and_then(|i| opcoes.get(i)) {
                    this.escolher_notas(*e, cx);
                }
            }))
    }

    /// A tela Notas esta mostrando a NF-e.
    pub(super) fn mostrando_a_nfe(&self) -> bool {
        // as recebidas tem o ambiente da NF-e (`[nfe].ambiente`): a distribuicao e a resposta usam esse
        self.tela == Tela::Notas
            && matches!(
                self.escolha_notas,
                EscolhaDeNotas::Nfe | EscolhaDeNotas::Recebidas
            )
    }

    /// "Usar como modelo" na NF-e: le a nota guardada fora da tela e leva os dados para a Nova NF-e. Com algo digitado,
    /// pergunta antes de substituir; com nota sem resposta ou envio em curso, nao substitui.
    pub(super) fn usar_nfe_como_modelo(
        &mut self,
        chave: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let servicos = Arc::clone(&self.servicos);
        let efetiva = self.servicos.efetiva(&self.salvo);
        cx.spawn_in(window, async move |this, cx| {
            let r = cx
                .background_spawn(async move { servicos.modelo_da_nfe(&efetiva, &chave) })
                .await;
            this.update_in(cx, |this, window, cx| match r {
                Ok(m) => this.aplicar_modelo_nfe(m, window, cx),
                Err(e) => window.push_notification(
                    Notification::warning(e.texto()).title("Não foi possível usar como modelo"),
                    cx,
                ),
            })
            .ok();
        })
        .detach();
    }

    fn aplicar_modelo_nfe(&mut self, m: ModeloNfe, window: &mut Window, cx: &mut Context<Self>) {
        let resposta = self
            .nova_nfe
            .update(cx, |n, cx| n.receber_modelo(m, window, cx));
        match resposta {
            ModeloRecebido::Aplicado => {
                self.escolher_notas(EscolhaDeNotas::Nfe, cx);
                self.ir_para(Tela::NovaNota, cx);
            }
            ModeloRecebido::Recusado(texto) => window.push_notification(
                Notification::warning(texto).title("Não foi possível usar como modelo"),
                cx,
            ),
            ModeloRecebido::PedirConfirmacao(modelo) => {
                let raiz = cx.entity();
                window.open_alert_dialog(cx, move |alerta, _, _| {
                    let (raiz, modelo) = (raiz.clone(), modelo.clone());
                    alerta
                        .title("Substituir o que você preencheu?")
                        .description(
                            "A Nova NF-e tem dados preenchidos. Usar a nota escolhida como modelo apaga o que está no assistente e o rascunho salvo.",
                        )
                        .confirm()
                        .ok_text("Substituir")
                        .cancel_text("Manter o assistente")
                        .on_ok(move |_, window, cx| {
                            let modelo = (*modelo).clone();
                            raiz.update(cx, |r, cx| {
                                r.nova_nfe.update(cx, |n, cx| n.usar_modelo(modelo, window, cx));
                                r.escolher_notas(EscolhaDeNotas::Nfe, cx);
                                r.ir_para(Tela::NovaNota, cx);
                            });
                            true
                        })
                });
            }
        }
    }

    /// O ambiente da NF-e em vigor (arquivo com as variaveis por cima).
    pub(super) fn ambiente_da_nfe_em_vigor(&self) -> Ambiente {
        ambiente_da_nfe(&self.servicos.efetiva(&self.salvo).nfe)
    }

    /// A NF-e pode ser usada: a configuracao comum completa mais a UF.
    pub(super) fn nfe_pronta(&self, efetiva: &Config) -> bool {
        configuracao_completa(&self.salvo) && !efetiva.nfe.uf.trim().is_empty()
    }

    // ---------------------------------------------------------------- testar conexao

    /// "Testar conexao com a SEFAZ": so' pergunta se o servico esta no ar, com o certificado, no ambiente da NF-e.
    pub(super) fn testar_sefaz(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.ocupado.sefaz {
            return;
        }
        let config = self.config_para_gravar();
        let senha = self.senha_digitada(cx);
        let servicos = Arc::clone(&self.servicos);
        let fabrica = servicos.fabrica_nfe();
        self.ocupado.sefaz = true;
        self.resultado_sefaz = None;
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let preparo = cx
                .background_spawn(async move {
                    servicos
                        .preparar_nfe(&config, senha.as_ref())
                        .map(|p| (p, config))
                })
                .await;
            let resultado = match preparo {
                Ok((p, config)) => match fora::rede(testar_sefaz(fabrica, p, config)) {
                    Some(tarefa) => tarefa.await.unwrap_or(ResultadoSefaz::SemResposta),
                    None => ResultadoSefaz::Recusado {
                        texto: "Não foi possível preparar a chamada de rede.".into(),
                    },
                },
                Err(e) => ResultadoSefaz::Recusado {
                    texto: texto_do_erro(&e),
                },
            };
            this.update_in(cx, |this, _, cx| {
                this.ocupado.sefaz = false;
                this.resultado_sefaz = Some(resultado);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

/// A frase de um erro de certificado ou senha, com o aviso de onde se resolve (o certificado e' o da NFS-e).
fn texto_do_erro(e: &ErroTela) -> String {
    match e {
        ErroTela::SemSenha => {
            "Falta a senha do certificado: digite-a ou guarde-a no cofre, na seção Sefin Nacional (o certificado é o mesmo da NFS-e).".into()
        }
        outro => outro.texto(),
    }
}
