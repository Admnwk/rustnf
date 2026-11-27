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
use gpui_kit::{AppContext as _, Context, Entity, IntoElement, SharedString, Window};
use rustnf_app::config::Config;

use super::raiz::{Raiz, Secao, Tela};
use crate::fora;
use crate::modelo::ambiente::Ambiente;
use crate::modelo::nfe_config::{ambiente_da_nfe, CampoNfe, ResultadoSefaz};
use crate::modelo::nfe_lista::EscolhaDeNotas;
use crate::modelo::passos::configuracao_completa;
use crate::servicos::{testar_sefaz, ErroTela};

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
            |this, _, evento: &super::nfe::EventoNfe, _, cx| match evento {
                super::nfe::EventoNfe::IrParaConfiguracoes => {
                    this.tela = Tela::Configuracoes;
                    this.secao = Secao::Nfe;
                    cx.notify();
                }
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
        c
    }

    /// Mudou alguma coisa (na NFS-e ou na NF-e) desde que foi carregado ou gravado.
    pub(super) fn alterado(&self) -> bool {
        self.formulario.alterado() || self.form_nfe.alterado()
    }

    /// Por que o arquivo nao pode ser gravado agora, ou `None`.
    pub(super) fn motivo_sem_salvar(&self) -> Option<String> {
        if !self.alterado() {
            return Some("Nenhuma alteração para salvar.".into());
        }
        if let Some(m) = self.form_nfe.motivo_sem_salvar(&self.config_para_gravar()) {
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
                if escolha == EscolhaDeNotas::Nfe {
                    this.nfe.update(cx, |n, cx| n.recarregar(cx));
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
        let alvo = if escolha == EscolhaDeNotas::Nfe {
            Some(self.nfe.clone())
        } else {
            None
        };
        if let Some(nfe) = alvo {
            nfe.update(cx, |n, cx| n.recarregar(cx));
        } else {
            self.notas.update(cx, |n, cx| n.recarregar(cx));
        }
        cx.notify();
    }

    /// O seletor "NFS-e | NF-e" do alto da tela Notas.
    pub(super) fn seletor_de_notas(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let atual = self.escolha_notas;
        ToggleGroup::new("notas-documento")
            .segmented()
            .single()
            .children(EscolhaDeNotas::TODAS.iter().enumerate().map(|(i, e)| {
                Toggle::new(SharedString::from(format!("notas-documento-{i}")))
                    .label(e.rotulo())
                    .checked(atual == *e)
            }))
            .on_click(cx.listener(|this, estados: &Vec<bool>, _, cx| {
                if let Some(e) = estados
                    .iter()
                    .position(|e| *e)
                    .and_then(|i| EscolhaDeNotas::TODAS.get(i))
                {
                    this.escolher_notas(*e, cx);
                }
            }))
    }

    /// A tela Notas esta mostrando a NF-e.
    pub(super) fn mostrando_a_nfe(&self) -> bool {
        self.tela == Tela::Notas && self.escolha_notas == EscolhaDeNotas::Nfe
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
