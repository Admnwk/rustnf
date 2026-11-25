// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A janela inteira: o estado compartilhado pelas telas (Inicio, primeiro uso e Configuracoes) e o que acontece
//! quando a pessoa digita, escolhe, salva ou testa. Tudo que pode demorar roda fora da linha da tela.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::select::{SearchableVec, SelectEvent, SelectState};
use gpui_kit::component::{IndexPath, Theme, ThemeMode, WindowExt as _};
use gpui_kit::{
    AppContext as _, Context, Entity, PathPromptOptions, ScrollHandle, Subscription, Window,
};
use rustnf_app::config::Config;
use rustnf_app::segredo::Segredo;

use super::cancelar::{Cancelamento, EventoCancelamento};
use super::consultar::{EventoConsulta, TelaConsultar};
use super::nota::{EventoNota, Modelo, NovaNota};
use super::notas::{EventoNotas, TelaNotas};
use super::previa::{EventoPrevia, TelaPrevia};
use crate::fora;
use crate::modelo::ambiente::{Ambiente, Tema};
use crate::modelo::campo::{Campo, CampoAliquota, Erros};
use crate::modelo::certificado::{EstadoCertificado, InfoCertificado};
use crate::modelo::formulario::{Formulario, OPCOES_APURACAO, OPCOES_REGIME_ESPECIAL};
use crate::modelo::nota::DadosNota;
use crate::modelo::passos::{configuracao_completa, pendentes, Passo};
use crate::servicos::{Carga, ErroTela, Servicos};

/// Qual tela esta aberta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tela {
    /// O resumo (ou o primeiro uso, se a configuracao estiver incompleta).
    Inicio,
    /// Montar e emitir uma nota.
    NovaNota,
    /// A lista das notas da pasta, com o detalhe.
    Notas,
    /// Consultar uma nota pela chave de acesso.
    Consultar,
    /// A calculadora avulsa do IBS e da CBS.
    Previa,
    /// Todas as opcoes.
    Configuracoes,
}

/// Secao aberta em Configuracoes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Secao {
    /// Dados do emitente.
    Emitente,
    /// Ambiente, certificado e conexao.
    Sefin,
    /// Pasta das notas e DANFSe.
    Notas,
    /// Tabela de aliquotas.
    Aliquotas,
    /// Tema.
    Aparencia,
}

impl Secao {
    /// As secoes, na ordem das abas.
    pub const TODAS: [Secao; 5] = [
        Secao::Emitente,
        Secao::Sefin,
        Secao::Notas,
        Secao::Aliquotas,
        Secao::Aparencia,
    ];

    /// Titulo da aba.
    #[must_use]
    pub fn titulo(self) -> &'static str {
        match self {
            Self::Emitente => "Emitente",
            Self::Sefin => "Sefin Nacional",
            Self::Notas => "Notas e DANFSe",
            Self::Aliquotas => "Alíquotas",
            Self::Aparencia => "Aparência",
        }
    }
}

/// O que ja foi lido do disco.
pub(super) enum Estado {
    /// Lendo o arquivo.
    Carregando,
    /// O arquivo nao pode ser lido: a frase para a tela.
    Falha(String),
    /// Pronto.
    Pronto,
}

/// O que esta rodando fora da tela (o botao correspondente mostra que esta trabalhando).
#[derive(Debug, Default, Clone, Copy)]
pub(super) struct Ocupado {
    /// Gravando o arquivo.
    pub(super) salvando: bool,
    /// Testando o certificado.
    pub(super) certificado: bool,
    /// Testando a conexao com a Sefin.
    pub(super) conexao: bool,
    /// Guardando ou apagando a senha do cofre.
    pub(super) senha: bool,
    /// Conferindo o passo do primeiro uso.
    pub(super) passo: bool,
    /// Mudando o proximo numero da DPS.
    pub(super) numero: bool,
}

/// Aviso de pagina (erro que nao e' de campo, ou confirmacao).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Aviso {
    /// Falha ou confirmacao.
    pub(super) erro: bool,
    /// O texto.
    pub(super) texto: String,
}

/// O que a carga disse sobre o arquivo.
pub(super) struct InfoDoArquivo {
    /// O arquivo ainda nao existe.
    pub(super) novo: bool,
    /// Nome do arquivo (sem a pasta).
    pub(super) nome: String,
    /// Variaveis `RUSTNF_*` que sobrepoem o arquivo.
    pub(super) variaveis: Vec<&'static str>,
    /// Ambiente em vigor.
    pub(super) ambiente: Ambiente,
}

/// Todos os campos de texto da janela.
pub(super) struct Entradas {
    pub(super) documento: Entity<InputState>,
    pub(super) nome: Entity<InputState>,
    pub(super) inscricao: Entity<InputState>,
    pub(super) municipio: Entity<InputState>,
    pub(super) serie: Entity<InputState>,
    pub(super) pfx: Entity<InputState>,
    pub(super) senha: Entity<InputState>,
    pub(super) tempo: Entity<InputState>,
    pub(super) folga: Entity<InputState>,
    pub(super) notas: Entity<InputState>,
    /// O numero em que a serie deve continuar (quem ja emitia por outro sistema).
    pub(super) proximo: Entity<InputState>,
    /// Uma linha por aliquota, uma entrada por coluna (na ordem de `CampoAliquota::TODOS`).
    pub(super) aliquotas: Vec<Vec<Entity<InputState>>>,
}

/// A janela.
pub struct Raiz {
    pub(super) servicos: Arc<Servicos>,
    pub(super) nova: Entity<NovaNota>,
    pub(super) cancelamento: Entity<Cancelamento>,
    pub(super) notas: Entity<TelaNotas>,
    pub(super) consultar: Entity<TelaConsultar>,
    pub(super) previa: Entity<TelaPrevia>,
    pub(super) estado: Estado,
    pub(super) tela: Tela,
    pub(super) secao: Secao,
    pub(super) formulario: Formulario,
    pub(super) salvo: Config,
    pub(super) info: Option<InfoDoArquivo>,
    pub(super) entradas: Entradas,
    pub(super) apuracao: Entity<SelectState<SearchableVec<&'static str>>>,
    pub(super) especial: Entity<SelectState<SearchableVec<&'static str>>>,
    pub(super) cert: EstadoCertificado,
    pub(super) senha_guardada: Option<bool>,
    pub(super) passo: Passo,
    pub(super) tentou_continuar: bool,
    pub(super) ocupado: Ocupado,
    pub(super) resultado_cert: Option<Result<InfoCertificado, ErroTela>>,
    pub(super) resultado_conexao: Option<(bool, String)>,
    pub(super) erros_externos: BTreeMap<Campo, String>,
    pub(super) aviso: Option<Aviso>,
    /// O proximo numero de DPS da serie, pela pasta de notas gravada; `None` = sem pasta ou ainda lendo.
    pub(super) proximo_dps: Option<u64>,
    /// A rolagem das telas da propria janela (Inicio, primeiro uso, Configuracoes, nota sem configuracao).
    pub(super) rolagem: ScrollHandle,
    fixas: Vec<Subscription>,
    das_aliquotas: Vec<Subscription>,
}

fn entrada(window: &mut Window, cx: &mut Context<Raiz>, dica: &'static str) -> Entity<InputState> {
    cx.new(|cx| InputState::new(window, cx).placeholder(dica))
}

fn selecao(
    window: &mut Window,
    cx: &mut Context<Raiz>,
    opcoes: &[(u8, &'static str)],
) -> Entity<SelectState<SearchableVec<&'static str>>> {
    let rotulos: Vec<&'static str> = opcoes.iter().map(|(_, r)| *r).collect();
    cx.new(|cx| {
        SelectState::new(
            SearchableVec::new(rotulos),
            Some(IndexPath::new(0)),
            window,
            cx,
        )
    })
}

impl Raiz {
    /// Abre a janela e comeca a ler o arquivo de configuracao fora da tela.
    pub fn new(servicos: Arc<Servicos>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let entradas = Entradas {
            documento: entrada(window, cx, "CNPJ ou CPF"),
            nome: entrada(window, cx, "Nome ou razão social"),
            inscricao: entrada(window, cx, "Inscrição municipal"),
            municipio: entrada(window, cx, "Ex.: 3550308"),
            serie: entrada(window, cx, "Ex.: 1"),
            pfx: entrada(window, cx, "Arquivo .pfx ou .p12"),
            senha: cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("Senha do certificado")
                    .masked(true)
            }),
            tempo: entrada(window, cx, "60"),
            folga: entrada(window, cx, "60"),
            notas: entrada(window, cx, "Pasta das notas"),
            proximo: entrada(window, cx, "Ex.: 1501"),
            aliquotas: Vec::new(),
        };
        let apuracao = selecao(window, cx, &OPCOES_APURACAO);
        let especial = selecao(window, cx, &OPCOES_REGIME_ESPECIAL);
        let nova = {
            let servicos = Arc::clone(&servicos);
            cx.new(|cx| NovaNota::new(servicos, window, cx))
        };
        let cancelamento = {
            let servicos = Arc::clone(&servicos);
            cx.new(|cx| Cancelamento::new(servicos, window, cx))
        };
        let notas = {
            let (servicos, cancelamento) = (Arc::clone(&servicos), cancelamento.clone());
            cx.new(|cx| TelaNotas::new(servicos, cancelamento, window, cx))
        };
        let consultar = {
            let (servicos, cancelamento) = (Arc::clone(&servicos), cancelamento.clone());
            cx.new(|cx| TelaConsultar::new(servicos, cancelamento, window, cx))
        };
        let previa = {
            let servicos = Arc::clone(&servicos);
            cx.new(|cx| TelaPrevia::new(servicos, window, cx))
        };
        let mut raiz = Self {
            servicos,
            nova,
            cancelamento,
            notas,
            consultar,
            previa,
            estado: Estado::Carregando,
            tela: Tela::Inicio,
            secao: Secao::Emitente,
            formulario: Formulario::de_config(&Config::default()),
            salvo: Config::default(),
            info: None,
            entradas,
            apuracao,
            especial,
            cert: EstadoCertificado::Verificando,
            senha_guardada: None,
            passo: Passo::Emitente,
            tentou_continuar: false,
            ocupado: Ocupado::default(),
            resultado_cert: None,
            resultado_conexao: None,
            erros_externos: BTreeMap::new(),
            aviso: None,
            proximo_dps: None,
            rolagem: ScrollHandle::new(),
            fixas: Vec::new(),
            das_aliquotas: Vec::new(),
        };
        raiz.ligar_tudo(window, cx);
        raiz.carregar(window, cx);
        raiz
    }

    fn ligar_tudo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let pares = [
            (Campo::Documento, self.entradas.documento.clone()),
            (Campo::Nome, self.entradas.nome.clone()),
            (Campo::InscricaoMunicipal, self.entradas.inscricao.clone()),
            (Campo::Municipio, self.entradas.municipio.clone()),
            (Campo::Serie, self.entradas.serie.clone()),
            (Campo::Pfx, self.entradas.pfx.clone()),
            (Campo::Senha, self.entradas.senha.clone()),
            (Campo::TempoLimite, self.entradas.tempo.clone()),
            (Campo::FolgaRelogio, self.entradas.folga.clone()),
            (Campo::NotasPasta, self.entradas.notas.clone()),
        ];
        for (campo, e) in pares {
            let s = self.ligar(campo, &e, window, cx);
            self.fixas.push(s);
        }
        let s = cx.subscribe_in(
            &self.apuracao.clone(),
            window,
            |this, _, ev: &SelectEvent<SearchableVec<&'static str>>, _, cx| {
                if let SelectEvent::Confirm(Some(rotulo)) = ev {
                    if let Some((codigo, _)) = OPCOES_APURACAO.iter().find(|(_, r)| r == rotulo) {
                        this.formulario.dados.regime_apuracao = *codigo;
                        cx.notify();
                    }
                }
            },
        );
        self.fixas.push(s);
        let s = cx.subscribe_in(
            &self.especial.clone(),
            window,
            |this, _, ev: &SelectEvent<SearchableVec<&'static str>>, _, cx| {
                if let SelectEvent::Confirm(Some(rotulo)) = ev {
                    if let Some((codigo, _)) =
                        OPCOES_REGIME_ESPECIAL.iter().find(|(_, r)| r == rotulo)
                    {
                        this.formulario.dados.regime_especial = *codigo;
                        cx.notify();
                    }
                }
            },
        );
        self.fixas.push(s);
        let s = cx.subscribe_in(
            &self.nova.clone(),
            window,
            |this, _, evento: &EventoNota, _, cx| {
                match evento {
                    EventoNota::IrParaAliquotas => this.ir_para_aliquotas(),
                    EventoNota::NotaAutorizada => {
                        this.notas.update(cx, |n, cx| n.recarregar(cx));
                    }
                }
                cx.notify();
            },
        );
        self.fixas.push(s);
        let s = cx.subscribe_in(
            &self.notas.clone(),
            window,
            |this, _, evento: &EventoNotas, window, cx| match evento {
                EventoNotas::UsarComoModelo(dados) => {
                    this.usar_como_modelo((**dados).clone(), window, cx)
                }
                EventoNotas::IrParaNovaNota => this.ir_para(Tela::NovaNota, cx),
            },
        );
        self.fixas.push(s);
        let s = cx.subscribe_in(
            &self.consultar.clone(),
            window,
            |this, _, evento: &EventoConsulta, window, cx| match evento {
                EventoConsulta::Guardou => {
                    this.notas.update(cx, |n, cx| n.recarregar(cx));
                }
                EventoConsulta::UsarComoModelo(dados) => {
                    this.usar_como_modelo((**dados).clone(), window, cx);
                }
            },
        );
        self.fixas.push(s);
        let s = cx.subscribe_in(
            &self.cancelamento.clone(),
            window,
            |this, _, evento: &EventoCancelamento, _, cx| match evento {
                EventoCancelamento::Registrado(chave) => {
                    let (a, b) = (chave.clone(), chave.clone());
                    this.notas.update(cx, |n, cx| n.cancelada(&a, cx));
                    this.consultar.update(cx, |c, cx| c.cancelada(&b, cx));
                }
            },
        );
        self.fixas.push(s);
        let s = cx.subscribe_in(
            &self.previa.clone(),
            window,
            |this, _, evento: &EventoPrevia, _, cx| {
                match evento {
                    EventoPrevia::IrParaAliquotas => this.ir_para_aliquotas(),
                }
                cx.notify();
            },
        );
        self.fixas.push(s);
        // com o tema "sistema", acompanha o Windows quando ele muda
        let s = cx.observe_window_appearance(window, |this, window, cx| {
            if this.formulario.dados.tema == Tema::Sistema {
                Theme::sync_system_appearance(Some(window), cx);
            }
        });
        self.fixas.push(s);
    }

    fn ligar(
        &mut self,
        campo: Campo,
        entrada: &Entity<InputState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Subscription {
        cx.subscribe_in(
            entrada,
            window,
            move |this, entrada, evento: &InputEvent, window, cx| match evento {
                InputEvent::Change => this.ao_digitar(campo, entrada, window, cx),
                InputEvent::PressEnter { .. } => this.ao_enter(window, cx),
                InputEvent::Blur => this.ao_sair(campo, cx),
                InputEvent::Focus => {}
            },
        )
    }

    fn ao_digitar(
        &mut self,
        campo: Campo,
        entrada: &Entity<InputState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.erros_externos.remove(&campo);
        if campo == Campo::Senha {
            self.resultado_cert = None;
            cx.notify();
            return;
        }
        let texto = entrada.read(cx).value().to_string();
        let normal = self.formulario.definir(campo, &texto);
        if normal != texto {
            entrada.update(cx, |estado, cx| estado.set_value(normal, window, cx));
        }
        if matches!(campo, Campo::Pfx | Campo::Documento) {
            self.resultado_cert = None;
            self.resultado_conexao = None;
        }
        cx.notify();
    }

    fn ao_enter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.tela == Tela::Inicio && !configuracao_completa(&self.salvo) {
            self.continuar(window, cx);
        }
    }

    fn ao_sair(&mut self, campo: Campo, cx: &mut Context<Self>) {
        if matches!(campo, Campo::Pfx | Campo::Documento) {
            self.atualizar_certificado(cx);
        }
    }

    // ---------------------------------------------------------------- carga e sincronia

    pub(super) fn carregar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.estado = Estado::Carregando;
        cx.notify();
        let servicos = Arc::clone(&self.servicos);
        cx.spawn_in(window, async move |this, cx| {
            let resultado = cx
                .background_spawn(async move { servicos.carregar() })
                .await;
            this.update_in(cx, |this, window, cx| match resultado {
                Ok(carga) => this.aplicar_carga(carga, window, cx),
                Err(erro) => {
                    this.estado = Estado::Falha(erro.texto());
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn aplicar_carga(&mut self, carga: Carga, window: &mut Window, cx: &mut Context<Self>) {
        let nome = carga
            .arquivo
            .as_deref()
            .or(self.servicos.local_do_arquivo().as_deref())
            .and_then(|p| p.file_name())
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        self.info = Some(InfoDoArquivo {
            novo: carga.arquivo_novo,
            nome,
            variaveis: carga.variaveis,
            ambiente: carga.ambiente,
        });
        self.formulario = Formulario::de_config(&carga.config);
        self.salvo = carga.config;
        self.estado = Estado::Pronto;
        self.passo = pendentes(&self.salvo)
            .first()
            .copied()
            .unwrap_or(Passo::Emitente);
        self.sincronizar(window, cx);
        self.aplicar_tema(window, cx);
        self.atualizar_certificado(cx);
        self.entregar_config(window, cx);
        cx.notify();
    }

    /// Entrega a configuracao que vale (a gravada, com as variaveis de ambiente por cima) as telas que a usam: Nova
    /// nota, Notas, Consultar, o cancelamento e a Previa.
    pub(super) fn entregar_config(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let efetiva = self.servicos.efetiva(&self.salvo);
        let completa = configuracao_completa(&self.salvo);
        let e = efetiva.clone();
        self.nova
            .update(cx, |n, cx| n.definir_config(e, completa, window, cx));
        let e = efetiva.clone();
        self.cancelamento
            .update(cx, |c, cx| c.definir_config(e, cx));
        let e = efetiva.clone();
        self.notas
            .update(cx, |n, cx| n.definir_config(e, completa, cx));
        let e = efetiva.clone();
        self.consultar.update(cx, |c, cx| c.definir_config(e, cx));
        self.previa
            .update(cx, |p, cx| p.definir_config(&efetiva, window, cx));
        self.atualizar_proximo_dps(efetiva, window, cx);
    }

    /// Le de novo o proximo numero de DPS da serie (para a secao Notas das Configuracoes).
    fn atualizar_proximo_dps(
        &mut self,
        efetiva: Config,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.proximo_dps = None;
        if efetiva.notas.pasta.trim().is_empty() {
            return;
        }
        let servicos = Arc::clone(&self.servicos);
        cx.spawn_in(window, async move |this, cx| {
            let r = cx
                .background_spawn(async move { servicos.proximo_numero(&efetiva) })
                .await;
            this.update(cx, |this, cx| {
                this.proximo_dps = r.ok();
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// "Continuar deste numero": confere o que foi digitado e pergunta antes de mudar a numeracao.
    pub(super) fn pedir_proximo_dps(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let digitado = self.entradas.proximo.read(cx).value().trim().to_string();
        let Some(proximo) = digitado
            .parse::<u64>()
            .ok()
            .filter(|n| *n > 0 && digitado.chars().all(|c| c.is_ascii_digit()))
        else {
            self.erros_externos.insert(
                Campo::ProximoDps,
                "Digite só o número da próxima DPS, por exemplo 1501.".into(),
            );
            cx.notify();
            return;
        };
        self.erros_externos.remove(&Campo::ProximoDps);
        let serie = self
            .servicos
            .contexto_da_nota(&self.servicos.efetiva(&self.salvo))
            .serie();
        let raiz = cx.entity();
        window.open_alert_dialog(cx, move |alerta, _, _| {
            let raiz = raiz.clone();
            alerta
                .title(format!("A próxima DPS da série {serie} será a nº {proximo}?"))
                .description(
                    "Use o número seguinte ao da última DPS emitida no outro sistema. Um número já usado faz a Sefin devolver a nota antiga em vez de autorizar a nova. A numeração não volta atrás depois.",
                )
                .confirm()
                .ok_text("Continuar deste número")
                .cancel_text("Cancelar")
                .on_ok(move |_, window, cx| {
                    raiz.update(cx, |this, cx| this.definir_proximo_dps(proximo, window, cx));
                    true
                })
        });
    }

    fn definir_proximo_dps(&mut self, proximo: u64, window: &mut Window, cx: &mut Context<Self>) {
        if self.ocupado.numero {
            return;
        }
        let efetiva = self.servicos.efetiva(&self.salvo);
        let servicos = Arc::clone(&self.servicos);
        self.ocupado.numero = true;
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let r = cx
                .background_spawn(async move { servicos.definir_proximo(&efetiva, proximo) })
                .await;
            this.update_in(cx, |this, window, cx| {
                this.ocupado.numero = false;
                match r {
                    Ok(n) => {
                        this.entradas
                            .proximo
                            .update(cx, |e, cx| e.set_value("", window, cx));
                        window.push_notification(
                            Notification::success(format!("A próxima DPS será a nº {n}."))
                                .title("Numeração alterada"),
                            cx,
                        );
                        this.proximo_dps = Some(n);
                        // a Nova nota mostra o numero da proxima DPS: le de novo
                        this.nova.update(cx, |nova, cx| nova.atualizar_numero(cx));
                    }
                    Err(erro) => {
                        this.erros_externos.insert(Campo::ProximoDps, erro.texto());
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Muda a tela. Notas le a pasta de novo ao abrir, para a lista nao ficar atras do que a Nova nota e os
    /// cancelamentos fizeram.
    pub(super) fn ir_para(&mut self, tela: Tela, cx: &mut Context<Self>) {
        if self.tela != tela {
            // cada tela da janela abre no topo
            self.rolagem.set_offset(gpui_kit::Point::default());
        }
        self.tela = tela;
        if tela == Tela::Notas {
            self.notas.update(cx, |n, cx| n.recarregar(cx));
        }
        cx.notify();
    }

    /// A rolagem da tela aberta; `None` nas Notas, onde as setas andam pela lista.
    pub(super) fn rolagem_da_tela(&self, cx: &gpui_kit::App) -> Option<ScrollHandle> {
        match self.tela {
            Tela::Inicio | Tela::Configuracoes => Some(self.rolagem.clone()),
            Tela::NovaNota => Some(self.nova.read(cx).rolagem.clone()),
            Tela::Consultar => Some(self.consultar.read(cx).rolagem.clone()),
            Tela::Previa => Some(self.previa.read(cx).rolagem.clone()),
            Tela::Notas => None,
        }
    }

    /// Rola a tela aberta pelo teclado; diz se rolou (se nao, a tecla segue para quem estiver abaixo).
    pub(super) fn rolar_tela(
        &mut self,
        passo: super::rolagem::Passo,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(rolagem) = self.rolagem_da_tela(cx) else {
            return false;
        };
        if !super::rolagem::rolar(&rolagem, passo) {
            return false;
        }
        match self.tela {
            Tela::NovaNota => self.nova.update(cx, |_, cx| cx.notify()),
            Tela::Consultar => self.consultar.update(cx, |_, cx| cx.notify()),
            Tela::Previa => self.previa.update(cx, |_, cx| cx.notify()),
            _ => {}
        }
        cx.notify();
        true
    }

    fn ir_para_aliquotas(&mut self) {
        self.tela = Tela::Configuracoes;
        self.secao = Secao::Aliquotas;
    }

    /// "Usar como modelo": leva os dados da nota para a Nova nota. Com algo digitado, pergunta antes de substituir;
    /// com uma tentativa de emissao pendente, nao substitui.
    pub(super) fn usar_como_modelo(
        &mut self,
        dados: DadosNota,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let resposta = self
            .nova
            .update(cx, |n, cx| n.receber_modelo(dados, window, cx));
        match resposta {
            Modelo::Aplicado => self.ir_para(Tela::NovaNota, cx),
            Modelo::Recusado(texto) => {
                window.push_notification(
                    Notification::warning(texto).title("Não foi possível usar como modelo"),
                    cx,
                );
            }
            Modelo::PedirConfirmacao(dados) => {
                let raiz = cx.entity();
                window.open_alert_dialog(cx, move |alerta, _, _| {
                    let (raiz, dados) = (raiz.clone(), dados.clone());
                    alerta
                        .title("Substituir o que você preencheu?")
                        .description(
                            "A Nova nota tem dados preenchidos. Usar a nota escolhida como modelo apaga o que está no formulário e o rascunho salvo.",
                        )
                        .confirm()
                        .ok_text("Substituir")
                        .cancel_text("Manter o formulário")
                        .on_ok(move |_, window, cx| {
                            let dados = (*dados).clone();
                            raiz.update(cx, |r, cx| {
                                r.nova.update(cx, |n, cx| n.usar_modelo(dados, window, cx));
                                r.ir_para(Tela::NovaNota, cx);
                            });
                            true
                        })
                });
            }
        }
    }

    /// Poe nos campos da tela o texto do formulario (sem disparar `Change`).
    pub(super) fn sincronizar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let pares = [
            (Campo::Documento, self.entradas.documento.clone()),
            (Campo::Nome, self.entradas.nome.clone()),
            (Campo::InscricaoMunicipal, self.entradas.inscricao.clone()),
            (Campo::Municipio, self.entradas.municipio.clone()),
            (Campo::Serie, self.entradas.serie.clone()),
            (Campo::Pfx, self.entradas.pfx.clone()),
            (Campo::TempoLimite, self.entradas.tempo.clone()),
            (Campo::FolgaRelogio, self.entradas.folga.clone()),
            (Campo::NotasPasta, self.entradas.notas.clone()),
        ];
        for (campo, e) in pares {
            let texto = self.formulario.texto(campo);
            e.update(cx, |estado, cx| estado.set_value(texto, window, cx));
        }
        self.entradas
            .senha
            .update(cx, |estado, cx| estado.set_value("", window, cx));
        self.reconstruir_aliquotas(window, cx);
        self.sincronizar_selecoes(window, cx);
        self.erros_externos.clear();
        self.resultado_cert = None;
        self.resultado_conexao = None;
    }

    pub(super) fn sincronizar_selecoes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let d = &self.formulario.dados;
        let apuracao = OPCOES_APURACAO
            .iter()
            .position(|(c, _)| *c == d.regime_apuracao);
        let especial = OPCOES_REGIME_ESPECIAL
            .iter()
            .position(|(c, _)| *c == d.regime_especial)
            .unwrap_or(0);
        self.apuracao.update(cx, |s, cx| {
            if let Some(i) = apuracao {
                s.set_selected_index(Some(IndexPath::new(i)), window, cx);
            } else {
                s.set_selected_index(None, window, cx);
            }
        });
        self.especial.update(cx, |s, cx| {
            s.set_selected_index(Some(IndexPath::new(especial)), window, cx);
        });
    }

    /// Refaz os campos da tabela de aliquotas a partir do formulario.
    pub(super) fn reconstruir_aliquotas(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.das_aliquotas.clear();
        self.entradas.aliquotas.clear();
        let linhas = self.formulario.dados.aliquotas.len();
        for i in 0..linhas {
            let mut linha = Vec::new();
            for coluna in CampoAliquota::TODOS {
                let texto = self.formulario.texto(Campo::Aliquota(i, coluna));
                let e = cx.new(|cx| {
                    let mut s = InputState::new(window, cx);
                    s.set_value(texto, window, cx);
                    s
                });
                let sub = self.ligar(Campo::Aliquota(i, coluna), &e, window, cx);
                self.das_aliquotas.push(sub);
                linha.push(e);
            }
            self.entradas.aliquotas.push(linha);
        }
    }

    pub(super) fn adicionar_aliquota(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.formulario.adicionar_aliquota();
        self.reconstruir_aliquotas(window, cx);
        cx.notify();
    }

    pub(super) fn remover_aliquota(
        &mut self,
        indice: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.formulario.remover_aliquota(indice);
        self.reconstruir_aliquotas(window, cx);
        cx.notify();
    }

    pub(super) fn aplicar_tema(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.formulario.dados.tema {
            Tema::Sistema => Theme::sync_system_appearance(Some(window), cx),
            Tema::Claro => Theme::change(ThemeMode::Light, Some(window), cx),
            Tema::Escuro => Theme::change(ThemeMode::Dark, Some(window), cx),
        }
    }

    // ---------------------------------------------------------------- leitura dos campos

    /// A senha digitada, se ha uma. Nunca sai daqui como texto solto.
    pub(super) fn senha_digitada(&self, cx: &gpui_kit::App) -> Option<Segredo> {
        let texto = self.entradas.senha.read(cx).value();
        (!texto.is_empty()).then(|| Segredo::novo(texto.to_string()))
    }

    /// Os erros que a tela mostra agora. Fora do primeiro uso so' aparece o que esta errado; no primeiro uso, o que
    /// falta so' aparece depois de tentar continuar.
    pub(super) fn erros_visiveis(&self) -> Erros {
        let mut erros = if self.pergunta_o_primeiro_uso() {
            if self.tentou_continuar {
                self.formulario.erros_do_passo(self.passo)
            } else {
                self.formulario.erros().restrito_a(self.passo.campos())
            }
        } else {
            self.formulario.erros()
        };
        for (campo, texto) in &self.erros_externos {
            erros.campos.insert(*campo, texto.clone());
        }
        erros
    }

    /// A configuracao gravada esta incompleta: o Inicio mostra o primeiro uso.
    pub(super) fn pergunta_o_primeiro_uso(&self) -> bool {
        !configuracao_completa(&self.salvo)
    }

    // ---------------------------------------------------------------- selecao de arquivo e pasta

    pub(super) fn escolher(
        &mut self,
        campo: Campo,
        pasta: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let recebe = cx.prompt_for_paths(PathPromptOptions {
            files: !pasta,
            directories: pasta,
            multiple: false,
            prompt: Some(
                if pasta {
                    "Escolher a pasta"
                } else {
                    "Escolher o certificado"
                }
                .into(),
            ),
        });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(caminhos))) = recebe.await {
                if let Some(caminho) = caminhos.into_iter().next() {
                    this.update_in(cx, |this, window, cx| {
                        this.definir_caminho(campo, &caminho, window, cx);
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    /// Poe um caminho escolhido no campo.
    pub(super) fn definir_caminho(
        &mut self,
        campo: Campo,
        caminho: &std::path::Path,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let texto = caminho.to_string_lossy().into_owned();
        let entrada = match campo {
            Campo::NotasPasta => self.entradas.notas.clone(),
            _ => self.entradas.pfx.clone(),
        };
        let normal = self.formulario.definir(campo, &texto);
        entrada.update(cx, |estado, cx| estado.set_value(normal, window, cx));
        self.erros_externos.remove(&campo);
        self.resultado_cert = None;
        self.resultado_conexao = None;
        if campo == Campo::Pfx {
            self.atualizar_certificado(cx);
        }
        cx.notify();
    }

    // ---------------------------------------------------------------- certificado e cofre

    /// Confere o estado do certificado fora da tela e atualiza o rodape.
    pub(super) fn atualizar_certificado(&mut self, cx: &mut Context<Self>) {
        let config = self.formulario.para_config();
        let servicos = Arc::clone(&self.servicos);
        let servicos2 = Arc::clone(&self.servicos);
        let config2 = config.clone();
        cx.spawn(async move |this, cx| {
            let (estado, guardada) = cx
                .background_spawn(async move {
                    (
                        servicos.estado_do_certificado(&config),
                        servicos2.senha_guardada(&config2),
                    )
                })
                .await;
            this.update(cx, |this, cx| {
                this.cert = estado;
                this.senha_guardada = guardada;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn testar_certificado(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.ocupado.certificado {
            return;
        }
        let config = self.formulario.para_config();
        let senha = self.senha_digitada(cx);
        let servicos = Arc::clone(&self.servicos);
        self.ocupado.certificado = true;
        self.resultado_cert = None;
        self.erros_externos.remove(&Campo::Pfx);
        self.erros_externos.remove(&Campo::Senha);
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let r = cx
                .background_spawn(
                    async move { servicos.testar_certificado(&config, senha.as_ref()) },
                )
                .await;
            this.update_in(cx, |this, _, cx| {
                this.ocupado.certificado = false;
                if let Err(erro) = &r {
                    this.erro_no_campo(erro);
                }
                this.resultado_cert = Some(r);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Mostra o erro embaixo do campo que o originou (ou como aviso, se nao for de campo nenhum).
    pub(super) fn erro_no_campo(&mut self, erro: &ErroTela) {
        match erro.campo() {
            Some(campo) => {
                self.erros_externos.insert(campo, erro.texto());
            }
            None => {
                self.aviso = Some(Aviso {
                    erro: true,
                    texto: erro.texto(),
                });
            }
        }
    }

    pub(super) fn guardar_senha(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.ocupado.senha {
            return;
        }
        let Some(senha) = self.senha_digitada(cx) else {
            self.erros_externos
                .insert(Campo::Senha, ErroTela::SemSenha.texto());
            cx.notify();
            return;
        };
        let config = self.formulario.para_config();
        let servicos = Arc::clone(&self.servicos);
        self.ocupado.senha = true;
        self.erros_externos.remove(&Campo::Senha);
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let r = cx
                .background_spawn(async move { servicos.guardar_senha(&config, &senha) })
                .await;
            this.update_in(cx, |this, window, cx| {
                this.ocupado.senha = false;
                match r {
                    Ok(()) => {
                        this.entradas
                            .senha
                            .update(cx, |e, cx| e.set_value("", window, cx));
                        this.aviso = None;
                        window.push_notification(
                            Notification::success("Senha guardada no cofre do Windows.")
                                .title("Senha guardada"),
                            cx,
                        );
                        this.atualizar_certificado(cx);
                    }
                    Err(erro) => this.erro_no_campo(&erro),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn apagar_senha(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.ocupado.senha {
            return;
        }
        let config = self.formulario.para_config();
        let servicos = Arc::clone(&self.servicos);
        self.ocupado.senha = true;
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let r = cx
                .background_spawn(async move { servicos.apagar_senha(&config) })
                .await;
            this.update_in(cx, |this, window, cx| {
                this.ocupado.senha = false;
                match r {
                    Ok(()) => {
                        window.push_notification(
                            Notification::success("A senha foi apagada do cofre do Windows.")
                                .title("Senha apagada"),
                            cx,
                        );
                        this.resultado_cert = None;
                        this.atualizar_certificado(cx);
                    }
                    Err(erro) => this.erro_no_campo(&erro),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// O botao "Testar conexao com a Sefin": uma chamada de rede de verdade, feita so' quando a pessoa clica.
    pub(super) fn testar_conexao(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.ocupado.conexao {
            return;
        }
        let config = self.formulario.para_config();
        let senha = self.senha_digitada(cx);
        let servicos = Arc::clone(&self.servicos);
        let conexao = servicos.conexao();
        self.ocupado.conexao = true;
        self.resultado_conexao = None;
        self.erros_externos.remove(&Campo::Pfx);
        self.erros_externos.remove(&Campo::Senha);
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            // 1) abre o certificado (calculo e disco) no pool de segundo plano
            let preparo = cx
                .background_spawn(async move { servicos.preparar_conexao(&config, senha.as_ref()) })
                .await;
            // 2) a chamada de rede roda no runtime proprio; aqui so' se espera o resultado
            let resultado: Result<(bool, String), ErroTela> = match preparo {
                Ok(p) => match fora::rede(conexao.testar(p)) {
                    Some(tarefa) => tarefa
                        .await
                        .map_err(|_| ErroTela::Rede("A chamada à Sefin foi interrompida.".into())),
                    None => Err(ErroTela::Rede(
                        "Não foi possível preparar a chamada de rede.".into(),
                    )),
                },
                Err(e) => Err(e),
            };
            this.update_in(cx, |this, _, cx| {
                this.ocupado.conexao = false;
                match resultado {
                    Ok(r) => this.resultado_conexao = Some(r),
                    Err(erro) => {
                        this.erro_no_campo(&erro);
                        this.resultado_conexao = Some((false, erro.texto()));
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    // ---------------------------------------------------------------- salvar e descartar

    pub(super) fn salvar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.ocupado.salvando || self.formulario.motivo_sem_salvar().is_some() {
            return;
        }
        let config = self.formulario.para_config();
        self.gravar(config, window, cx);
    }

    fn gravar(&mut self, config: Config, window: &mut Window, cx: &mut Context<Self>) {
        let servicos = Arc::clone(&self.servicos);
        let a_gravar = config.clone();
        self.ocupado.salvando = true;
        self.aviso = None;
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let r = cx
                .background_spawn(async move { servicos.salvar(&a_gravar) })
                .await;
            this.update_in(cx, |this, window, cx| {
                this.ocupado.salvando = false;
                match r {
                    Ok(()) => {
                        this.formulario.marcar_salvo(&config);
                        this.salvo = config;
                        this.aviso = None;
                        window.push_notification(
                            Notification::success("As configurações foram gravadas.")
                                .title("Configuração salva"),
                            cx,
                        );
                        this.atualizar_certificado(cx);
                        this.entregar_config(window, cx);
                    }
                    Err(erro) => {
                        this.aviso = Some(Aviso {
                            erro: true,
                            texto: erro.texto(),
                        });
                        window.push_notification(
                            Notification::error(erro.texto()).title("Não foi possível salvar"),
                            cx,
                        );
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn descartar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.formulario.alterado() {
            return;
        }
        self.formulario.descartar();
        self.sincronizar(window, cx);
        self.aplicar_tema(window, cx);
        self.aviso = None;
        self.atualizar_certificado(cx);
        window.push_notification(Notification::info("As alterações foram descartadas."), cx);
        cx.notify();
    }

    // ---------------------------------------------------------------- primeiro uso

    pub(super) fn voltar(&mut self, cx: &mut Context<Self>) {
        if let Some(anterior) = self.passo.anterior() {
            self.passo = anterior;
            self.tentou_continuar = false;
            self.aviso = None;
            cx.notify();
        }
    }

    /// "Continuar" (ou "Concluir", no ultimo passo): confere o passo e segue.
    pub(super) fn continuar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.ocupado.passo {
            return;
        }
        self.tentou_continuar = true;
        self.aviso = None;
        if !self.formulario.erros_do_passo(self.passo).vazio() || !self.erros_externos.is_empty() {
            cx.notify();
            return;
        }
        match self.passo {
            Passo::Emitente => self.seguir(cx),
            Passo::Certificado => self.conferir_certificado_do_passo(window, cx),
            Passo::Notas => self.concluir(window, cx),
        }
    }

    fn seguir(&mut self, cx: &mut Context<Self>) {
        if let Some(proximo) = self.passo.proximo() {
            self.passo = proximo;
            self.tentou_continuar = false;
        }
        cx.notify();
    }

    /// Passo 2: a senha tem de abrir o certificado; e vai para o cofre.
    fn conferir_certificado_do_passo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let config = self.formulario.para_config();
        let digitada = self.senha_digitada(cx);
        let servicos = Arc::clone(&self.servicos);
        self.ocupado.passo = true;
        self.erros_externos.remove(&Campo::Pfx);
        self.erros_externos.remove(&Campo::Senha);
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let r: Result<Option<ErroTela>, ErroTela> = cx
                .background_spawn(async move {
                    match digitada {
                        Some(senha) => {
                            // so' guarda a senha que de fato abre o certificado
                            servicos.testar_certificado(&config, Some(&senha))?;
                            if servicos.cofre_desligado() {
                                Ok(Some(ErroTela::Cofre(
                                    rustnf_app::cofre::ErroCofre::Desligado,
                                )))
                            } else {
                                servicos.guardar_senha(&config, &senha)?;
                                Ok(None)
                            }
                        }
                        None => {
                            servicos.testar_certificado(&config, None)?;
                            Ok(None)
                        }
                    }
                })
                .await;
            this.update_in(cx, |this, window, cx| {
                this.ocupado.passo = false;
                match r {
                    Ok(aviso) => {
                        if let Some(a) = aviso {
                            window.push_notification(
                                Notification::warning(a.texto()).title("Senha não guardada"),
                                cx,
                            );
                        }
                        this.entradas
                            .senha
                            .update(cx, |e, cx| e.set_value("", window, cx));
                        this.atualizar_certificado(cx);
                        this.seguir(cx);
                    }
                    Err(erro) => {
                        this.erro_no_campo(&erro);
                        cx.notify();
                    }
                }
            })
            .ok();
        })
        .detach();
    }

    /// Passo 3: confere a pasta, grava o arquivo e termina.
    fn concluir(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let erros = self.formulario.erros();
        if let Some(primeiro) = erros.primeira() {
            self.aviso = Some(Aviso {
                erro: true,
                texto: format!("Corrija antes de concluir: {primeiro}"),
            });
            cx.notify();
            return;
        }
        let config = self.formulario.para_config();
        let servicos = Arc::clone(&self.servicos);
        let pasta = config.notas.pasta.clone();
        let a_gravar = config.clone();
        self.ocupado.passo = true;
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let r = cx
                .background_spawn(async move {
                    servicos.conferir_pasta(&pasta)?;
                    servicos.salvar(&a_gravar)
                })
                .await;
            this.update_in(cx, |this, window, cx| {
                this.ocupado.passo = false;
                match r {
                    Ok(()) => {
                        this.formulario.marcar_salvo(&config);
                        this.salvo = config;
                        window.push_notification(
                            Notification::success("O rustnf está configurado e pronto para uso.")
                                .title("Configuração salva"),
                            cx,
                        );
                        this.atualizar_certificado(cx);
                        this.entregar_config(window, cx);
                    }
                    Err(erro) => this.erro_no_campo(&erro),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Caminho sugerido para as notas: uma pasta `notas` ao lado do arquivo de configuracao.
    pub(super) fn pasta_sugerida(&self) -> Option<PathBuf> {
        self.servicos
            .local_do_arquivo()
            .and_then(|p| p.parent().map(|d| d.join("notas")))
    }
}
