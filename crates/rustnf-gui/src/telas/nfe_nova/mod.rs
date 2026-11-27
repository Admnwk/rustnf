// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A Nova NF-e: o assistente de quem vende, em cinco passos (para quem, o que vendeu, como recebeu, como entrega, conferir
//! e enviar). So' desenho e ligacao de eventos: as respostas simples viram o termo fiscal em `modelo::nfe_fiscal`, o
//! pedido sai de `modelo::nfe_nova`, a conferencia e o envio passam pela camada de aplicacao (`servicos::nfe_emissao`) e
//! tudo que pode demorar roda fora da linha da tela. O fluxo e' o da Nova nota da NFS-e (confirmacao explicita em
//! producao, sem clique duplo, senha so' se nao esta no cofre, rascunho que sobrevive ao fechar a janela), com a
//! diferenca que o que a NF-e manda e' o `Retomar` (a mesma chave, nunca outra nota) e nao o "tentar de novo".

mod envio;
mod itens;
mod linhas;
mod passos;
mod resultado;

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::{FixedOffset, Offset as _};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{
    AppContext as _, Context, Entity, EventEmitter, ScrollHandle, Subscription, Window,
};
use rustnf_app::config::Config;

use crate::modelo::nfe_campos::{CampoItem, CampoNova};
use crate::modelo::nfe_config::ResultadoSefaz;
use crate::modelo::nfe_emissao::{ConferenciaNfe, DesfechoNfeEmissao};
use crate::modelo::nfe_modelo::{ClienteConhecido, ModeloNfe};
use crate::modelo::nfe_nova::{pendencias, ContextoNfe, DadosNfe, PassoNfe, PendenciaNfe};
use crate::modelo::nfe_produtos::Cadastro;
use crate::modelo::nfe_resultados::ResumoDaRetomada;
use crate::servicos::Servicos;
use crate::telas::senha::DialogoSenha;

/// O que a tela pede a janela.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EventoNovaNfe {
    /// A SEFAZ autorizou uma nota (ou ela entrou em contingencia): a lista de notas deve ser lida de novo.
    NotaMudou,
    /// Abrir as Configuracoes, na secao da NF-e.
    IrParaConfiguracoes,
    /// Abrir a lista de NF-e (para ver ou retomar a nota).
    IrParaNotas,
}

/// O que acontece quando outra tela pede para usar uma NF-e como modelo.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum ModeloRecebido {
    /// Os dados da nota viraram o assistente.
    Aplicado,
    /// O assistente tem dados digitados: a tela dona pergunta antes de substituir.
    PedirConfirmacao(Box<ModeloNfe>),
    /// Nao da para substituir agora: a frase diz por que.
    Recusado(String),
}

/// Quanto tempo depois da ultima mudanca o rascunho e' gravado.
pub(super) const ESPERA_DO_RASCUNHO: Duration = Duration::from_millis(700);

/// A etapa do envio que esta acontecendo de verdade.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Etapa {
    /// Abrindo o certificado (disco e senha).
    Certificado,
    /// Falando com a SEFAZ.
    Enviando,
    /// Registrando o EPEC.
    Epec,
    /// Retomando a nota sem resposta.
    Retomando,
}

/// Onde o assistente esta.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Fase {
    /// Preenchendo.
    Editando,
    /// Enviando.
    Enviando(Etapa),
    /// A SEFAZ respondeu (ou nao): o que aconteceu e o que fazer.
    Desfecho(Box<DesfechoNfeEmissao>),
}

/// O que a senha pedida vai destravar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Acao {
    /// Enviar a nota.
    Enviar,
    /// Emitir em contingencia.
    Epec,
    /// Retomar a nota sem resposta.
    Retomar,
    /// Verificar se a SEFAZ esta no ar.
    Verificar,
}

/// O estado do rascunho, para o indicador discreto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EstadoDoRascunho {
    /// Nada a guardar.
    Nenhum,
    /// Mudou, ainda nao foi gravado.
    Alterado,
    /// Gravado.
    Salvo,
}

/// Uma linha de produto na tela: os campos de texto e se os detalhes fiscais estao abertos.
pub(super) struct LinhaItem {
    pub(super) id: u64,
    pub(super) entradas: BTreeMap<CampoItem, Entity<InputState>>,
    pub(super) detalhes: bool,
    _subs: Vec<Subscription>,
}

/// A tela.
pub(super) struct NovaNfe {
    pub(super) servicos: Arc<Servicos>,
    pub(super) config: Config,
    pub(super) ctx: ContextoNfe,
    /// A configuracao ja chegou e a NF-e pode ser usada (configuracao comum completa mais a UF).
    pub(super) pronta: bool,
    pub(super) dados: DadosNfe,
    pub(super) passo: PassoNfe,
    pub(super) entradas: BTreeMap<CampoNova, Entity<InputState>>,
    pub(super) linhas: Vec<LinhaItem>,
    proximo_id: u64,
    pub(super) busca: Entity<InputState>,
    pub(super) cadastro: Cadastro,
    pub(super) aviso_cadastro: Option<String>,
    pub(super) conferencia: ConferenciaNfe,
    /// A conferencia esta rodando (ou ainda nao rodou para os dados de agora).
    pub(super) conferindo: bool,
    versao_da_conferencia: u64,
    /// A pessoa tentou continuar neste passo: os erros dele aparecem.
    pub(super) tentou: bool,
    pub(super) fase: Fase,
    /// Em producao: marcou "Entendo que a nota tem efeito fiscal".
    pub(super) entendo: bool,
    /// O que a SEFAZ recusou na ultima tentativa (some quando a pessoa mexe).
    pub(super) rejeicao: Option<DesfechoNfeEmissao>,
    cliente_buscado: Option<String>,
    pub(super) cliente_preenchido: Option<String>,
    pub(super) buscando_cep: bool,
    pub(super) aviso_cep: Option<String>,
    pub(super) rascunho: EstadoDoRascunho,
    pub(super) recuperado: bool,
    versao: u64,
    /// O assistente veio de uma nota anterior: o que nao deu para ler.
    pub(super) aviso_do_modelo: Vec<String>,
    /// O que a verificacao da SEFAZ disse (para oferecer a contingencia).
    pub(super) status: Option<ResultadoSefaz>,
    pub(super) verificando: bool,
    /// O motivo da contingencia.
    pub(super) motivo: Entity<InputState>,
    pub(super) epec_aberto: bool,
    /// Quantas notas estao sem resposta na pasta.
    pub(super) pendentes: usize,
    pub(super) retomada: Option<ResumoDaRetomada>,
    pub(super) produtos_guardados: Option<usize>,
    pub(super) senha_pedida: Option<(Entity<DialogoSenha>, Acao)>,
    pub(super) sub_senha: Option<Subscription>,
    pub(super) rolagem: ScrollHandle,
    pub(super) copiada: bool,
    carregada: bool,
    subs: Vec<Subscription>,
}

impl EventEmitter<EventoNovaNfe> for NovaNfe {}

/// Hoje, em Brasilia, a partir do instante.
fn hoje_em_brasilia(agora: chrono::DateTime<chrono::Utc>) -> chrono::NaiveDate {
    let brasilia = FixedOffset::west_opt(3 * 3600).unwrap_or(chrono::Utc.fix());
    agora.with_timezone(&brasilia).date_naive()
}

impl NovaNfe {
    /// Cria a tela (ainda sem configuracao: `definir_config` a poe para funcionar).
    pub(super) fn new(
        servicos: Arc<Servicos>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let config = Config::default();
        let ctx = ContextoNfe::de_config(&config, hoje_em_brasilia(servicos.agora()), 1);
        let mut entradas = BTreeMap::new();
        for campo in CampoNova::TODOS {
            let e = cx.new(|cx| InputState::new(window, cx).placeholder(campo.dica()));
            entradas.insert(campo, e);
        }
        let busca = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Buscar no cadastro por nome, código ou NCM")
        });
        let motivo = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Por que está emitindo em contingência (15 a 255 letras)")
        });
        let mut nova = Self {
            servicos,
            config,
            ctx,
            pronta: false,
            dados: DadosNfe::default(),
            passo: PassoNfe::ParaQuem,
            entradas,
            linhas: Vec::new(),
            proximo_id: 0,
            busca,
            cadastro: Cadastro::default(),
            aviso_cadastro: None,
            conferencia: ConferenciaNfe::default(),
            conferindo: false,
            versao_da_conferencia: 0,
            tentou: false,
            fase: Fase::Editando,
            entendo: false,
            rejeicao: None,
            cliente_buscado: None,
            cliente_preenchido: None,
            buscando_cep: false,
            aviso_cep: None,
            rascunho: EstadoDoRascunho::Nenhum,
            recuperado: false,
            versao: 0,
            aviso_do_modelo: Vec::new(),
            status: None,
            verificando: false,
            motivo,
            epec_aberto: false,
            pendentes: 0,
            retomada: None,
            produtos_guardados: None,
            senha_pedida: None,
            sub_senha: None,
            rolagem: ScrollHandle::new(),
            copiada: false,
            carregada: false,
            subs: Vec::new(),
        };
        nova.ligar(window, cx);
        nova
    }

    fn ligar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let pares: Vec<(CampoNova, Entity<InputState>)> =
            self.entradas.iter().map(|(c, e)| (*c, e.clone())).collect();
        for (campo, e) in pares {
            let s = cx.subscribe_in(
                &e,
                window,
                move |this, entrada, evento: &InputEvent, window, cx| {
                    if matches!(evento, InputEvent::Change) {
                        let texto = entrada.read(cx).value().to_string();
                        this.ao_digitar(campo, &texto, window, cx);
                    }
                },
            );
            self.subs.push(s);
        }
        let busca = self.busca.clone();
        let s = cx.subscribe_in(&busca, window, |_, _, evento: &InputEvent, _, cx| {
            if matches!(evento, InputEvent::Change) {
                cx.notify();
            }
        });
        self.subs.push(s);
    }

    /// A janela entrega a configuracao (a efetiva) quando ela chega ou muda. `pronta`: a NF-e pode ser usada.
    pub(super) fn definir_config(
        &mut self,
        config: Config,
        pronta: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let primeira = !self.carregada && pronta;
        let pasta_mudou = self.config.notas.pasta != config.notas.pasta;
        self.ctx = ContextoNfe::de_config(
            &config,
            hoje_em_brasilia(self.servicos.agora()),
            self.ctx.numero_provisorio,
        );
        self.config = config;
        self.pronta = pronta;
        if primeira || (pronta && pasta_mudou) {
            self.carregada = true;
            self.carregar(primeira, window, cx);
        }
        self.atualizar_o_contexto(cx);
        cx.notify();
    }

    /// Le fora da tela o que mora na pasta: o rascunho (so' na primeira vez), o cadastro de produtos e o contexto.
    fn carregar(&mut self, rascunho: bool, window: &mut Window, cx: &mut Context<Self>) {
        let servicos = Arc::clone(&self.servicos);
        let config = self.config.clone();
        cx.spawn_in(window, async move |this, cx| {
            let (s2, c2) = (Arc::clone(&servicos), config.clone());
            let (lido, rasc) = cx
                .background_spawn(async move {
                    (
                        s2.ler_produtos(&c2),
                        if rascunho {
                            s2.ler_rascunho_nfe(&c2)
                        } else {
                            None
                        },
                    )
                })
                .await;
            this.update_in(cx, |this, window, cx| {
                this.cadastro = lido.cadastro;
                this.aviso_cadastro = lido.aviso;
                if let Some(r) = rasc {
                    if matches!(this.fase, Fase::Editando) && this.dados.vazio() {
                        this.passo = r.passo;
                        this.aplicar_dados(r.dados, window, cx);
                        this.recuperado = true;
                        this.rascunho = EstadoDoRascunho::Salvo;
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Le de novo, fora da tela, o proximo numero e quantas notas estao sem resposta.
    pub(super) fn atualizar_o_contexto(&mut self, cx: &mut Context<Self>) {
        if !self.pronta {
            return;
        }
        let servicos = Arc::clone(&self.servicos);
        let config = self.config.clone();
        cx.spawn(async move |this, cx| {
            let (ctx, pendentes) = cx
                .background_spawn(async move {
                    (
                        servicos.contexto_da_nfe(&config),
                        servicos.pendentes_nfe(&config),
                    )
                })
                .await;
            this.update(cx, |this, cx| {
                this.ctx = ctx;
                this.pendentes = pendentes;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Outra tela pede para usar uma NF-e como modelo. Envio em curso ou nota sem resposta: nao substitui. Com algo
    /// digitado: pede confirmacao. Assistente vazio (ou tela de resultado): aplica na hora.
    pub(super) fn receber_modelo(
        &mut self,
        modelo: ModeloNfe,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> ModeloRecebido {
        if !self.pronta {
            return ModeloRecebido::Recusado(
                "Complete a configuração da NF-e (emitente, certificado, pasta das notas e a UF) para emitir.".into(),
            );
        }
        match &self.fase {
            Fase::Enviando(_) => {
                return ModeloRecebido::Recusado(
                    "Há uma nota sendo enviada agora. Espere o resultado antes de usar outra como modelo.".into(),
                );
            }
            Fase::Desfecho(d) if d.deixa_pendente() => {
                return ModeloRecebido::Recusado(
                    "A nota em andamento ficou sem resposta da SEFAZ. Use Retomar antes de usar outra nota como modelo.".into(),
                );
            }
            Fase::Desfecho(d) => {
                if d.concluiu() {
                    self.dados = DadosNfe::default();
                }
                self.fase = Fase::Editando;
            }
            Fase::Editando => {}
        }
        if self.dados.vazio() {
            self.usar_modelo(modelo, window, cx);
            ModeloRecebido::Aplicado
        } else {
            ModeloRecebido::PedirConfirmacao(Box::new(modelo))
        }
    }

    /// Poe a nota escolhida no assistente, no lugar do que havia (a pessoa ja confirmou).
    pub(super) fn usar_modelo(
        &mut self,
        modelo: ModeloNfe,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !matches!(self.fase, Fase::Editando) {
            return;
        }
        self.passo = PassoNfe::ParaQuem;
        self.tentou = false;
        self.aviso_do_modelo = modelo.avisos;
        self.aplicar_dados(modelo.dados, window, cx);
        self.mudou(cx);
    }

    /// Poe todos os dados nos campos da tela.
    pub(super) fn aplicar_dados(
        &mut self,
        dados: DadosNfe,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.dados = dados;
        self.cliente_buscado = None;
        self.cliente_preenchido = None;
        self.aviso_do_modelo.clear();
        for campo in CampoNova::TODOS {
            let texto = campo.texto(&self.dados);
            self.por_no_campo(campo, &texto, window, cx);
        }
        self.reconstruir_linhas(window, cx);
        self.busca.update(cx, |e, cx| e.set_value("", window, cx));
    }

    /// Poe o texto no campo da tela (sem disparar `Change`).
    pub(super) fn por_no_campo(
        &mut self,
        campo: CampoNova,
        texto: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(e) = self.entradas.get(&campo) {
            let t = texto.to_string();
            e.update(cx, |e, cx| e.set_value(t, window, cx));
        }
    }

    /// O assistente esta travado para edicao (enviando ou com desfecho na tela).
    pub(super) fn travado(&self) -> bool {
        !matches!(self.fase, Fase::Editando)
    }

    /// Os dados mudaram: a conferencia e o aviso da SEFAZ ficam velhos, o rascunho e' agendado.
    pub(super) fn mudou(&mut self, cx: &mut Context<Self>) {
        self.conferencia = ConferenciaNfe::default();
        self.conferindo = false;
        self.versao_da_conferencia += 1;
        self.rejeicao = None;
        self.recuperado = false;
        self.status = None;
        self.epec_aberto = false;
        self.agendar_rascunho(cx);
        cx.notify();
    }

    fn ao_digitar(
        &mut self,
        campo: CampoNova,
        texto: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.travado() {
            return;
        }
        let normal = campo.definir(&mut self.dados, texto);
        if normal != texto {
            self.por_no_campo(campo, &normal, window, cx);
        }
        self.mudou(cx);
        if campo == CampoNova::Documento {
            self.procurar_cliente(window, cx);
        }
    }

    /// O documento esta completo e confere: procura na pasta (fora da tela) a NF-e mais recente desse cliente e completa
    /// os campos vazios. O que a pessoa digitou fica.
    fn procurar_cliente(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let doc = self.dados.documento_limpo();
        let confere = match doc.len() {
            11 => rustnf::chave_dfe::cpf_valido(&doc),
            14 => rustnf::chave_dfe::cnpj_valido(&doc),
            _ => false,
        };
        if !confere {
            self.cliente_buscado = None;
            self.cliente_preenchido = None;
            return;
        }
        if self.cliente_buscado.as_deref() == Some(doc.as_str()) {
            return;
        }
        self.cliente_buscado = Some(doc.clone());
        self.cliente_preenchido = None;
        let servicos = Arc::clone(&self.servicos);
        let config = self.config.clone();
        let procurado = doc.clone();
        cx.spawn_in(window, async move |this, cx| {
            let achado: Option<ClienteConhecido> = cx
                .background_spawn(
                    async move { servicos.cliente_conhecido_nfe(&config, &procurado) },
                )
                .await;
            this.update_in(cx, |this, window, cx| {
                let Some(c) = achado else {
                    return;
                };
                if this.travado() || this.dados.documento_limpo() != doc {
                    return;
                }
                let feitos = c.completar(&mut this.dados);
                if feitos.is_empty() {
                    return;
                }
                for campo in CampoNova::TODOS {
                    let texto = campo.texto(&this.dados);
                    this.por_no_campo(campo, &texto, window, cx);
                }
                this.mudou(cx);
                this.cliente_preenchido = Some(c.aviso(&feitos));
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// "Buscar endereco pelo CEP": so' no clique (o CEP sai da maquina). Preenche logradouro, bairro e municipio que
    /// vierem; o que nao vier fica como estava.
    pub(super) fn buscar_cep(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.buscando_cep || self.travado() {
            return;
        }
        let Ok(cep) = rustnf_app::cep::cep_limpo(&self.dados.cep) else {
            self.aviso_cep = Some("Digite o CEP com 8 dígitos para buscar o endereço.".into());
            cx.notify();
            return;
        };
        let busca = self.servicos.busca_cep();
        self.buscando_cep = true;
        self.aviso_cep = None;
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let r = match crate::fora::rede(busca.buscar(cep)) {
                Some(tarefa) => tarefa
                    .await
                    .unwrap_or(Err(rustnf_app::cep::ErroCep::Indisponivel)),
                None => Err(rustnf_app::cep::ErroCep::Indisponivel),
            };
            this.update_in(cx, |this, window, cx| {
                this.buscando_cep = false;
                match r {
                    Ok(e) => {
                        for (campo, valor) in [
                            (CampoNova::Logradouro, e.logradouro.as_str()),
                            (CampoNova::Bairro, e.bairro.as_str()),
                            (CampoNova::CodMunicipio, e.municipio.as_str()),
                        ] {
                            if !valor.is_empty() {
                                let normal = campo.definir(&mut this.dados, valor);
                                this.por_no_campo(campo, &normal, window, cx);
                            }
                        }
                        this.mudou(cx);
                    }
                    Err(rustnf_app::cep::ErroCep::NaoEncontrado) => {
                        this.aviso_cep = Some(
                            "CEP não encontrado. Confira os dígitos ou preencha o endereço a mão."
                                .into(),
                        );
                    }
                    Err(_) => {
                        this.aviso_cep = Some(
                            "A busca do CEP não respondeu agora. Preencha o endereço a mão.".into(),
                        );
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    // ---------------------------------------------------------------- escolhas

    /// Aplica uma escolha simples aos dados (se o assistente nao esta travado) e refaz o que depende dela.
    pub(super) fn escolher(&mut self, f: impl FnOnce(&mut DadosNfe), cx: &mut Context<Self>) {
        if self.travado() {
            return;
        }
        f(&mut self.dados);
        self.mudou(cx);
    }

    // ---------------------------------------------------------------- navegacao

    /// O que falta neste passo (so' o que impede de seguir).
    pub(super) fn pendencias_do_passo(&self, passo: PassoNfe) -> Vec<PendenciaNfe> {
        pendencias(&self.dados, &self.ctx)
            .into_iter()
            .filter(|p| p.passo == passo)
            .collect()
    }

    /// Todas as pendencias locais.
    pub(super) fn todas_as_pendencias(&self) -> Vec<PendenciaNfe> {
        pendencias(&self.dados, &self.ctx)
    }

    /// "Continuar": confere o passo e segue; com pendencia, ela aparece e o passo nao muda.
    pub(super) fn continuar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.travado() || !self.pronta {
            return;
        }
        if !self.pendencias_do_passo(self.passo).is_empty() {
            self.tentou = true;
            cx.notify();
            return;
        }
        if let Some(proximo) = self.passo.proximo() {
            self.ir_para_o_passo(proximo, window, cx);
        }
    }

    /// "Voltar".
    pub(super) fn voltar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.travado() {
            return;
        }
        if let Some(anterior) = self.passo.anterior() {
            self.ir_para_o_passo(anterior, window, cx);
        }
    }

    /// Vai ao passo (o que a lista de passos e os botoes que levam ao problema usam).
    pub(super) fn ir_para_o_passo(
        &mut self,
        passo: PassoNfe,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.travado() {
            return;
        }
        self.passo = passo;
        self.tentou = false;
        self.rolagem.set_offset(gpui_kit::Point::default());
        if passo == PassoNfe::Conferir {
            self.conferir(window, cx);
        }
        self.agendar_rascunho(cx);
        cx.notify();
    }

    /// Confere o pedido pela camada de aplicacao (as regras do leiaute e da SEFAZ), fora da tela e sem rede.
    pub(super) fn conferir(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let locais = self.todas_as_pendencias();
        self.versao_da_conferencia += 1;
        if !locais.is_empty() {
            self.conferencia = ConferenciaNfe {
                erros: locais,
                ..ConferenciaNfe::default()
            };
            self.conferindo = false;
            return;
        }
        let Ok(pedido) = crate::modelo::nfe_nova::pedido(&self.dados, &self.ctx) else {
            return;
        };
        self.conferindo = true;
        let versao = self.versao_da_conferencia;
        let servicos = Arc::clone(&self.servicos);
        let config = self.config.clone();
        cx.spawn_in(window, async move |this, cx| {
            let c = cx
                .background_spawn(async move { servicos.conferir_pedido_nfe(&config, pedido) })
                .await;
            this.update(cx, |this, cx| {
                if this.versao_da_conferencia == versao {
                    this.conferencia = c;
                    this.conferindo = false;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// Esvazia o assistente (e apaga o rascunho): a próxima nota.
    pub(super) fn limpar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.fase = Fase::Editando;
        self.passo = PassoNfe::ParaQuem;
        self.tentou = false;
        self.entendo = false;
        self.retomada = None;
        self.produtos_guardados = None;
        self.copiada = false;
        self.aplicar_dados(DadosNfe::default(), window, cx);
        self.mudou(cx);
        self.apagar_rascunho(cx);
        self.atualizar_o_contexto(cx);
    }
}
