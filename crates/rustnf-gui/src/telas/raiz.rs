// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A janela inteira: o estado compartilhado pelas telas (Inicio, primeiro uso e Configuracoes) e o que acontece
//! quando a pessoa digita, escolhe, salva ou testa. Tudo que pode demorar roda fora da linha da tela.

use std::collections::BTreeMap;
use std::sync::Arc;

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::select::{SearchableVec, SelectEvent, SelectState};
use gpui_kit::component::{IndexPath, Theme, ThemeMode, WindowExt as _};
use gpui_kit::{
    AppContext as _, Context, Entity, FocusHandle, PathPromptOptions, ScrollHandle, Subscription,
    Window,
};
use rustnf_app::config::Config;
use rustnf_app::notas::Situacao;
use rustnf_app::segredo::Segredo;

use super::caixa::TelaCaixa;
use super::cancelar::{Cancelamento, EventoCancelamento};
use super::consultar::{EventoConsulta, TelaConsultar};
use super::nfe::TelaNfe;
use super::nfe_dialogo::DialogoNfe;
use super::nfe_nova::NovaNfe;
use super::nota::{EventoNota, Modelo, NovaNota};
use super::notas::{EventoNotas, TelaNotas};
use super::previa::{EventoPrevia, TelaPrevia};
use super::produtos::TelaProdutos;
use super::raiz_nfce::EntradasNfce;
use super::raiz_nfe::EntradasNfe;
use super::raiz_recebidas::TelasRecebidas;
use crate::modelo::ambiente::{Ambiente, Tema};
use crate::modelo::campo::{Campo, CampoAliquota, Erros};
use crate::modelo::certificado::{EstadoCertificado, InfoCertificado};
use crate::modelo::evento::TipoDeEvento;
use crate::modelo::formulario::{Formulario, OPCOES_APURACAO, OPCOES_REGIME_ESPECIAL};
use crate::modelo::nfce_config::{EstadoCsc, FormularioNfce};
use crate::modelo::nfe_config::{FormularioNfe, ResultadoSefaz};
use crate::modelo::nfe_lista::EscolhaDeNotas;
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
    /// O caixa: a venda rapida (NFC-e).
    Caixa,
    /// A lista das notas da pasta, com o detalhe.
    Notas,
    /// Consultar uma nota pela chave de acesso.
    Consultar,
    /// O cadastro de produtos da NF-e.
    Produtos,
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
    /// NF-e (venda de mercadoria).
    Nfe,
    /// NFC-e (cupom do caixa).
    Nfce,
    /// Tabela de aliquotas.
    Aliquotas,
    /// Tema.
    Aparencia,
}

impl Secao {
    /// As secoes, na ordem das abas.
    pub const TODAS: [Secao; 7] = [
        Secao::Emitente,
        Secao::Sefin,
        Secao::Notas,
        Secao::Nfe,
        Secao::Nfce,
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
            Self::Nfe => "NF-e",
            Self::Nfce => "NFC-e",
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
    /// Testando a conexao com a SEFAZ (NF-e).
    pub(super) sefaz: bool,
    /// Guardando ou apagando o CSC do cofre.
    pub(super) csc: bool,
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
    pub(super) aliquota_iss: Entity<InputState>,
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
    pub(super) nova_nfe: Entity<NovaNfe>,
    pub(super) produtos: Entity<TelaProdutos>,
    pub(super) cancelamento: Entity<Cancelamento>,
    pub(super) notas: Entity<TelaNotas>,
    pub(super) consultar: Entity<TelaConsultar>,
    pub(super) previa: Entity<TelaPrevia>,
    pub(super) nfe: Entity<TelaNfe>,
    pub(super) dialogo_nfe: Entity<DialogoNfe>,
    pub(super) recebidas: TelasRecebidas,
    pub(super) caixa: Entity<TelaCaixa>,
    pub(super) nfce: Entity<TelaNfe>,
    pub(super) dialogo_nfce: Entity<DialogoNfe>,
    pub(super) form_nfce: FormularioNfce,
    pub(super) entradas_nfce: EntradasNfce,
    pub(super) csc: EstadoCsc,
    pub(super) resultado_csc: Option<(bool, String)>,
    /// A tela Notas mostra a NFS-e, a NF-e ou a NFC-e.
    pub(super) escolha_notas: EscolhaDeNotas,
    pub(super) form_nfe: FormularioNfe,
    pub(super) entradas_nfe: EntradasNfe,
    pub(super) resultado_sefaz: Option<ResultadoSefaz>,
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
    /// O foco dos dois botoes do Inicio (o Tab os traz para a vista).
    pub(super) foco_do_inicio: [FocusHandle; 2],
    pub(super) fixas: Vec<Subscription>,
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
            aliquota_iss: entrada(window, cx, "Vazio: não informar"),
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
        let (nova_nfe, produtos) = Self::criar_telas_nfe(&servicos, window, cx);
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
        let (dialogo_nfe, nfe) = Self::criar_lista_nfe(&servicos, window, cx);
        let recebidas = Self::criar_telas_recebidas(&servicos, window, cx);
        let entradas_nfe = Self::criar_entradas_nfe(window, cx);
        let telas_nfce = Self::criar_telas_nfce(&servicos, window, cx);
        let entradas_nfce = Self::criar_entradas_nfce(window, cx);
        let mut raiz = Self {
            servicos,
            nova,
            nova_nfe,
            produtos,
            cancelamento,
            notas,
            consultar,
            previa,
            nfe,
            dialogo_nfe,
            recebidas,
            caixa: telas_nfce.caixa,
            nfce: telas_nfce.lista,
            dialogo_nfce: telas_nfce.dialogo,
            form_nfce: FormularioNfce::de_config(&Config::default()),
            entradas_nfce,
            csc: EstadoCsc::Verificando,
            resultado_csc: None,
            escolha_notas: EscolhaDeNotas::default(),
            form_nfe: FormularioNfe::de_config(&Config::default()),
            entradas_nfe,
            resultado_sefaz: None,
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
            foco_do_inicio: [cx.focus_handle(), cx.focus_handle()],
            fixas: Vec::new(),
            das_aliquotas: Vec::new(),
        };
        raiz.ligar_tudo(window, cx);
        raiz.ligar_nfe(window, cx);
        raiz.ligar_nfce(window, cx);
        raiz.ligar_recebidas(window, cx);
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
            (Campo::AliquotaIssPadrao, self.entradas.aliquota_iss.clone()),
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
                EventoCancelamento::Registrado { chave, tipo } => {
                    // o que o evento faz com a situacao da nota (confirmar e recusar nao mexem nela)
                    let situacao = match tipo {
                        TipoDeEvento::Cancelar => Some(Situacao::Cancelada),
                        TipoDeEvento::AnaliseFiscal => Some(Situacao::AnaliseFiscal),
                        TipoDeEvento::Confirmar | TipoDeEvento::Recusar => None,
                    };
                    let (a, b) = (chave.clone(), chave.clone());
                    this.notas
                        .update(cx, |n, cx| n.evento_registrado(&a, situacao, cx));
                    this.consultar
                        .update(cx, |c, cx| c.evento_registrado(&b, situacao, cx));
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
        self.form_nfe = FormularioNfe::de_config(&carga.config);
        self.form_nfce = FormularioNfce::de_config(&carga.config);
        self.salvo = carga.config;
        self.estado = Estado::Pronto;
        self.passo = pendentes(&self.salvo)
            .first()
            .copied()
            .unwrap_or(Passo::Emitente);
        self.sincronizar(window, cx);
        self.sincronizar_nfe(window, cx);
        self.sincronizar_nfce(window, cx);
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
        let pronta = self.nfe_pronta(&efetiva);
        self.entregar_config_nova_nfe(&efetiva, pronta, completa, window, cx);
        let e = efetiva.clone();
        self.dialogo_nfe.update(cx, |d, cx| d.definir_config(e, cx));
        let e = efetiva.clone();
        self.nfe.update(cx, |n, cx| n.definir_config(e, pronta, cx));
        self.entregar_config_nfce(&efetiva, pronta, window, cx);
        self.entregar_config_recebidas(&efetiva, pronta, cx);
        self.carregar_escolha(efetiva.clone(), cx);
        self.atualizar_proximo_dps(efetiva, window, cx);
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
        self.ao_abrir_tela_nfe(tela, cx);
        self.ao_abrir_tela_nfce(tela, cx);
        cx.notify();
    }

    /// A rolagem da tela aberta; `None` nas Notas, onde as setas andam pela lista.
    pub(super) fn rolagem_da_tela(&self, cx: &gpui_kit::App) -> Option<ScrollHandle> {
        match self.tela {
            Tela::Inicio | Tela::Configuracoes => Some(self.rolagem.clone()),
            Tela::NovaNota => Some(self.rolagem_da_nova_nota(cx)),
            Tela::Caixa => Some(self.caixa.read(cx).rolagem.clone()),
            Tela::Consultar => Some(self.consultar.read(cx).rolagem.clone()),
            Tela::Previa => Some(self.previa.read(cx).rolagem.clone()),
            Tela::Notas | Tela::Produtos => None,
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
            Tela::NovaNota => self.notificar_nova_nota(cx),
            Tela::Caixa => self.caixa.update(cx, |_, cx| cx.notify()),
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
        // a nota de servico vai para a Nova nota de servico: a escolha volta para NFS-e
        self.escolher_notas(EscolhaDeNotas::Nfse, cx);
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
            (Campo::AliquotaIssPadrao, self.entradas.aliquota_iss.clone()),
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

    // ---------------------------------------------------------------- salvar e descartar

    pub(super) fn salvar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.ocupado.salvando || self.motivo_sem_salvar().is_some() {
            return;
        }
        let config = self.config_para_gravar();
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
                        this.form_nfe.marcar_salvo();
                        this.form_nfce.marcar_salvo();
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
        if !self.alterado() {
            return;
        }
        self.formulario.descartar();
        self.form_nfe.descartar();
        self.form_nfce.descartar();
        self.sincronizar(window, cx);
        self.sincronizar_nfe(window, cx);
        self.sincronizar_nfce(window, cx);
        self.aplicar_tema(window, cx);
        self.aviso = None;
        self.atualizar_certificado(cx);
        window.push_notification(Notification::info("As alterações foram descartadas."), cx);
        cx.notify();
    }
}
