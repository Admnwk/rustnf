// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A tela Nova nota: so' desenho e ligacao de eventos. O estado do formulario, a conferencia, a previa, a sugestao, o
//! rascunho e o desfecho da emissao moram em `modelo`; o que mexe em disco, no certificado e na rede mora em
//! `servicos`. Tudo que pode demorar roda fora da linha da tela.

mod dialogos;
mod emissao;
mod formulario;
mod painel;
mod resultado;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Duration;

use gpui_kit::component::input::{InputEvent, InputState, TextareaState};
use gpui_kit::{
    AppContext as _, Context, Entity, EventEmitter, ScrollHandle, Subscription, Window,
};
use rustnf_app::config::Config;

use crate::modelo::conferencia::{self, Conferencia, Pendencia};
use crate::modelo::emissao::MensagemSefin;
use crate::modelo::nota::{CampoNota, DadosNota, SecaoNota};
use crate::modelo::pedido::{Contexto, PedidoCongelado};
use crate::modelo::previa::{self, PreviaIbsCbs};
use crate::modelo::sugestao::{self, Servico, Sugestao};
use crate::modelo::tomador;
use crate::servicos::Servicos;
use crate::telas::senha::DialogoSenha;

/// O que a tela pede a janela.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EventoNota {
    /// Ir as Configuracoes, na aba das aliquotas.
    IrParaAliquotas,
    /// A Sefin autorizou uma nota (ou a achou ja emitida): a lista de notas deve ser lida de novo.
    NotaAutorizada,
}

/// O que acontece quando outra tela pede para usar uma nota como modelo.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Modelo {
    /// Os dados da nota viraram o formulario.
    Aplicado,
    /// O formulario tem dados digitados: a tela dona pergunta antes de substituir.
    PedirConfirmacao(Box<DadosNota>),
    /// Nao da para substituir agora: a frase diz por que.
    Recusado(String),
}

/// Quanto tempo depois da ultima mudanca o rascunho e' gravado.
const ESPERA_DO_RASCUNHO: Duration = Duration::from_millis(700);

/// A etapa do envio que esta acontecendo de verdade.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Etapa {
    /// Abrindo o certificado (disco e senha).
    AbrindoCertificado,
    /// Enviando para a Sefin Nacional.
    Enviando,
}

/// O resultado de uma emissao autorizada, para a tela de resultado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Concluida {
    pub(super) numero: String,
    pub(super) chave: String,
    pub(super) guardada: bool,
    pub(super) aviso: Option<String>,
    pub(super) recuperada: bool,
    /// Nota recuperada que nao e' a do formulario: em que difere.
    pub(super) diferencas: Vec<String>,
    pub(super) alertas: Vec<MensagemSefin>,
}

/// Onde a emissao esta.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Fase {
    /// Editando o formulario.
    Editando,
    /// Enviando o pedido congelado.
    Enviando {
        etapa: Etapa,
        /// Ainda nao ha pedido congelado enquanto se abre o certificado pela primeira vez.
        tentativa: Option<PedidoCongelado>,
    },
    /// Sem resposta: o pedido continua congelado e o formulario, travado.
    Pendente {
        tentativa: PedidoCongelado,
        texto: String,
    },
    /// Autorizada.
    Concluida(Concluida),
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

/// A tela.
pub(super) struct NovaNota {
    pub(super) servicos: Arc<Servicos>,
    pub(super) config: Config,
    pub(super) ctx: Contexto,
    /// A configuracao ja chegou e esta completa.
    pub(super) pronta: bool,
    pub(super) dados: DadosNota,
    pub(super) entradas: BTreeMap<CampoNota, Entity<InputState>>,
    pub(super) descricao: Entity<TextareaState>,
    pub(super) busca: Entity<InputState>,
    pub(super) resultados: Vec<Servico>,
    pub(super) conferencia: Conferencia,
    pub(super) previa_ibs: PreviaIbsCbs,
    pub(super) sugestao: Sugestao,
    /// Campos em que a pessoa ja passou (so' esses mostram erro antes de tentar emitir).
    pub(super) tocados: BTreeSet<CampoNota>,
    /// A pessoa tentou emitir ou clicou numa pendencia: todos os erros aparecem.
    pub(super) mostrar_tudo: bool,
    pub(super) federais_abertas: bool,
    pub(super) numero_provisorio: u64,
    pub(super) fase: Fase,
    /// Campos que a Sefin ou a biblioteca apontaram, com a frase, ate a pessoa mexer neles.
    pub(super) marcas: BTreeMap<CampoNota, String>,
    pub(super) rejeicao: Option<Vec<MensagemSefin>>,
    pub(super) recusa: Option<Pendencia>,
    pub(super) desistencia: Option<String>,
    pub(super) rascunho: EstadoDoRascunho,
    pub(super) recuperado: bool,
    versao: u64,
    pub(super) valor_fiscal_entendido: bool,
    /// O dialogo da senha em uso e a tentativa que espera por ela.
    pub(super) senha_pedida: Option<(Entity<DialogoSenha>, PedidoCongelado)>,
    pub(super) sub_senha: Option<Subscription>,
    pub(super) rolagem: ScrollHandle,
    pub(super) copiada: bool,
    /// Buscando o endereco pelo CEP.
    pub(super) buscando_cep: bool,
    /// O que a busca do CEP disse (CEP invalido, nao encontrado, servico fora).
    pub(super) aviso_cep: Option<String>,
    /// O ultimo documento de tomador procurado na pasta de notas (para nao procurar de novo a cada tecla).
    tomador_buscado: Option<String>,
    /// Os dados do tomador vieram de uma nota anterior: a frase diz de qual.
    pub(super) tomador_preenchido: Option<String>,
    carregada: bool,
    subs: Vec<Subscription>,
}

impl EventEmitter<EventoNota> for NovaNota {}

/// O campo e' um campo de uma linha (nao e' a descricao nem uma escolha).
pub(super) fn eh_entrada(campo: CampoNota) -> bool {
    !matches!(
        campo,
        CampoNota::Descricao | CampoNota::Tributacao | CampoNota::Retencao | CampoNota::IndFinal
    )
}

fn dica(campo: CampoNota) -> &'static str {
    match campo {
        CampoNota::TomadorDocumento => "CNPJ ou CPF",
        CampoNota::TomadorNome => "Nome ou razão social",
        CampoNota::TomadorIm => "Opcional",
        CampoNota::TomadorEmail => "nome@exemplo.com.br",
        CampoNota::TomadorFone => "(11) 98888-7777",
        CampoNota::EnderecoCep => "00000-000",
        CampoNota::EnderecoLogradouro => "Rua, avenida...",
        CampoNota::EnderecoNumero => "S/N se não houver",
        CampoNota::EnderecoComplemento => "Opcional",
        CampoNota::EnderecoBairro => "Bairro",
        CampoNota::EnderecoMunicipio => "Ex.: 3550308",
        CampoNota::Ctribnac => "00.00.00",
        CampoNota::MunicipioPrestacao => "Vazio: o do emitente",
        CampoNota::Nbs => "0.0000.00.00",
        CampoNota::CtribMun => "Opcional",
        CampoNota::CodInterno => "Opcional",
        CampoNota::ValorServico
        | CampoNota::DescontoIncondicionado
        | CampoNota::DescontoCondicionado
        | CampoNota::Irrf
        | CampoNota::Csll
        | CampoNota::Previdencia
        | CampoNota::PisBase => "0,00",
        CampoNota::Competencia => "DD/MM/AAAA",
        CampoNota::AliquotaIss => "Ex.: 2,5",
        CampoNota::PercentualSn
        | CampoNota::PercentualFederal
        | CampoNota::PercentualEstadual
        | CampoNota::PercentualMunicipal
        | CampoNota::PisAliquotaPis
        | CampoNota::PisAliquotaCofins => "0,00",
        CampoNota::PisCst => "Ex.: 01",
        CampoNota::PisTipoRetencao => "0 a 9",
        CampoNota::CindOp => "6 dígitos",
        CampoNota::ClassTrib => "6 dígitos",
        CampoNota::Descricao
        | CampoNota::Tributacao
        | CampoNota::Retencao
        | CampoNota::IndFinal => "",
    }
}

impl NovaNota {
    /// Cria a tela (ainda sem configuracao: `definir_config` a poe para funcionar).
    pub(super) fn new(
        servicos: Arc<Servicos>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let config = Config::default();
        let ctx = servicos.contexto_da_nota(&config);
        let mut entradas = BTreeMap::new();
        for campo in CampoNota::TODOS {
            if eh_entrada(campo) {
                let e = cx.new(|cx| InputState::new(window, cx).placeholder(dica(campo)));
                entradas.insert(campo, e);
            }
        }
        let descricao = cx.new(|cx| {
            TextareaState::new(window, cx)
                .placeholder("O que foi feito, para quem e em que condições")
                .auto_grow(3, 8)
        });
        let busca = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Buscar o serviço por palavra ou código")
        });
        let dados = ctx.nota_nova();
        let mut nova = Self {
            servicos,
            config,
            ctx,
            pronta: false,
            dados,
            entradas,
            descricao,
            busca,
            resultados: Vec::new(),
            conferencia: Conferencia::default(),
            previa_ibs: PreviaIbsCbs::Desligada,
            sugestao: Sugestao::default(),
            tocados: BTreeSet::new(),
            mostrar_tudo: false,
            federais_abertas: false,
            numero_provisorio: 1,
            fase: Fase::Editando,
            marcas: BTreeMap::new(),
            rejeicao: None,
            recusa: None,
            desistencia: None,
            rascunho: EstadoDoRascunho::Nenhum,
            recuperado: false,
            versao: 0,
            valor_fiscal_entendido: false,
            senha_pedida: None,
            sub_senha: None,
            rolagem: ScrollHandle::new(),
            copiada: false,
            buscando_cep: false,
            aviso_cep: None,
            tomador_buscado: None,
            tomador_preenchido: None,
            carregada: false,
            subs: Vec::new(),
        };
        nova.ligar(window, cx);
        nova.recalcular();
        nova
    }

    fn ligar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let pares: Vec<(CampoNota, Entity<InputState>)> =
            self.entradas.iter().map(|(c, e)| (*c, e.clone())).collect();
        for (campo, e) in pares {
            let s = cx.subscribe_in(
                &e,
                window,
                move |this, entrada, evento: &InputEvent, window, cx| match evento {
                    InputEvent::Change => {
                        let texto = entrada.read(cx).value().to_string();
                        this.ao_digitar(campo, &texto, window, cx);
                    }
                    InputEvent::Blur => this.ao_sair(campo, cx),
                    _ => {}
                },
            );
            self.subs.push(s);
        }
        let descricao = self.descricao.clone();
        let s = cx.subscribe_in(
            &descricao,
            window,
            |this, entrada, evento: &InputEvent, window, cx| match evento {
                InputEvent::Change => {
                    let texto = entrada.read(cx).value().to_string();
                    this.ao_digitar(CampoNota::Descricao, &texto, window, cx);
                }
                InputEvent::Blur => this.ao_sair(CampoNota::Descricao, cx),
                _ => {}
            },
        );
        self.subs.push(s);
        let busca = self.busca.clone();
        let s = cx.subscribe_in(
            &busca,
            window,
            |this, entrada, evento: &InputEvent, _, cx| {
                if matches!(evento, InputEvent::Change) {
                    let termo = entrada.read(cx).value().to_string();
                    this.resultados = sugestao::buscar_servico(&termo);
                    cx.notify();
                }
            },
        );
        self.subs.push(s);
    }

    /// Outra tela pede para usar uma nota como modelo. Tentativa de emissao pendente ou em curso: nao substitui. Com
    /// algo digitado: pede confirmacao. Formulario vazio (ou tela de resultado): aplica na hora.
    pub(super) fn receber_modelo(
        &mut self,
        dados: DadosNota,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Modelo {
        if !self.pronta {
            return Modelo::Recusado(
                "Complete a configuração (emitente, certificado e pasta das notas) para emitir uma nota.".into(),
            );
        }
        match &self.fase {
            Fase::Enviando { .. } => {
                return Modelo::Recusado(
                    "Há uma nota sendo enviada agora. Espere o resultado antes de usar outra como modelo.".into(),
                );
            }
            Fase::Pendente { .. } => {
                return Modelo::Recusado(
                    "A Nova nota tem uma tentativa sem resposta da Sefin. Tente de novo ou desista dela antes de usar outra nota como modelo.".into(),
                );
            }
            Fase::Concluida(c) => {
                // a nota que acabou de sair e' a do formulario: nada a perder. Uma recuperada de outra tentativa deixa o
                // formulario como a pessoa o tinha
                if c.diferencas.is_empty() {
                    self.dados = self.ctx.nota_nova();
                }
                self.fase = Fase::Editando;
            }
            Fase::Editando => {}
        }
        if self.dados.vazio(&self.ctx.hoje()) {
            self.usar_modelo(dados, window, cx);
            Modelo::Aplicado
        } else {
            Modelo::PedirConfirmacao(Box::new(dados))
        }
    }

    /// Poe os dados da nota escolhida no formulario, no lugar do que havia (a pessoa ja confirmou).
    pub(super) fn usar_modelo(
        &mut self,
        dados: DadosNota,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !matches!(self.fase, Fase::Editando) {
            return;
        }
        self.aplicar_dados(dados, window, cx);
        self.tocados.clear();
        self.marcas.clear();
        self.mostrar_tudo = false;
        self.mudou(cx);
        self.atualizar_numero(cx);
    }

    /// A janela entrega a configuracao (a efetiva: arquivo mais variaveis de ambiente) quando ela chega ou muda.
    pub(super) fn definir_config(
        &mut self,
        config: Config,
        completa: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let primeira = !self.carregada && completa;
        self.ctx = self.servicos.contexto_da_nota(&config);
        // a aliquota padrao do emitente aparece como dica no campo (vale quando ele fica vazio)
        let padrao = self.ctx.emitente.aliquota_iss.trim().replace('.', ",");
        let dica_iss = if padrao.is_empty() {
            dica(CampoNota::AliquotaIss).to_string()
        } else {
            format!("Padrão: {padrao}")
        };
        if let Some(e) = self.entradas.get(&CampoNota::AliquotaIss).cloned() {
            e.update(cx, |s, cx| s.set_placeholder(dica_iss, window, cx));
        } // formulario ainda sem nada digitado: a chave do grupo IBS/CBS acompanha o regime da configuracao que chegou
        if matches!(self.fase, Fase::Editando) && self.dados.vazio(&self.ctx.hoje()) {
            self.dados.ibscbs_ativo = self.ctx.regime().informa_ibscbs_por_padrao();
        }
        self.config = config;
        self.pronta = completa;
        if primeira {
            self.carregada = true;
            self.carregar_rascunho(window, cx);
            self.atualizar_numero(cx);
        }
        self.recalcular();
        cx.notify();
    }

    /// Le o rascunho fora da tela e o poe nos campos; sem rascunho, formulario novo.
    fn carregar_rascunho(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let servicos = Arc::clone(&self.servicos);
        let config = self.config.clone();
        cx.spawn_in(window, async move |this, cx| {
            let lido = cx
                .background_spawn(async move { servicos.ler_rascunho(&config) })
                .await;
            this.update_in(cx, |this, window, cx| {
                if let Some(dados) = lido {
                    if matches!(this.fase, Fase::Editando) && this.dados.vazio(&this.ctx.hoje()) {
                        this.aplicar_dados(dados, window, cx);
                        this.recuperado = true;
                        this.rascunho = EstadoDoRascunho::Salvo;
                        cx.notify();
                    }
                }
            })
            .ok();
        })
        .detach();
    }

    /// Descobre o proximo numero de DPS na pasta de notas (para a linha "DPS no N" e para a conferencia).
    pub(super) fn atualizar_numero(&mut self, cx: &mut Context<Self>) {
        let servicos = Arc::clone(&self.servicos);
        let config = self.config.clone();
        cx.spawn(async move |this, cx| {
            let numero = cx
                .background_spawn(async move { servicos.proximo_numero(&config) })
                .await;
            this.update(cx, |this, cx| {
                if let Ok(n) = numero {
                    this.numero_provisorio = n;
                    this.recalcular();
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// Refaz a conferencia, a previa e a sugestao com o que esta preenchido.
    pub(super) fn recalcular(&mut self) {
        self.conferencia = conferencia::conferir(&self.dados, &self.ctx, self.numero_provisorio);
        self.previa_ibs = previa::ibscbs(&self.dados, &self.ctx);
        self.sugestao = if self.dados.ibscbs_ativo {
            sugestao::sugerir(&self.dados.ctribnac_digitos(), &self.dados.nbs_digitos())
        } else {
            Sugestao::default()
        };
    }

    /// O formulario esta travado para edicao (enviando, sem resposta ou concluido).
    pub(super) fn travado(&self) -> bool {
        !matches!(self.fase, Fase::Editando)
    }

    // ---------------------------------------------------------------- digitacao

    fn ao_digitar(
        &mut self,
        campo: CampoNota,
        texto: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.travado() {
            return;
        }
        let normal = self.dados.definir(campo, texto);
        if normal != texto {
            self.por_no_campo(campo, &normal, window, cx);
        }
        self.marcas.remove(&campo);
        self.mudou(cx);
        if campo == CampoNota::TomadorDocumento {
            self.procurar_tomador(window, cx);
        }
    }

    /// Com o CPF/CNPJ completo e valido, procura na pasta de notas (fora da tela) a nota mais recente desse tomador e
    /// completa os campos vazios do tomador e do endereco. O que a pessoa digitou fica.
    fn procurar_tomador(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(documento) = tomador::documento_completo(&self.dados) else {
            self.tomador_buscado = None;
            self.tomador_preenchido = None;
            return;
        };
        if self.tomador_buscado.as_deref() == Some(documento.as_str()) {
            return;
        }
        self.tomador_buscado = Some(documento.clone());
        self.tomador_preenchido = None;
        let servicos = Arc::clone(&self.servicos);
        let config = self.config.clone();
        let doc = documento.clone();
        cx.spawn_in(window, async move |this, cx| {
            let achado = cx
                .background_spawn(async move { servicos.tomador_conhecido(&config, &doc) })
                .await;
            this.update_in(cx, |this, window, cx| {
                let Some(conhecido) = achado else {
                    return;
                };
                // a pessoa mudou o documento (ou o formulario travou) enquanto a pasta era lida
                if this.travado()
                    || tomador::documento_completo(&this.dados).as_deref()
                        != Some(documento.as_str())
                {
                    return;
                }
                let feitos = conhecido.completar(&mut this.dados);
                if feitos.is_empty() {
                    return;
                }
                for campo in feitos {
                    let texto = this.dados.texto(campo);
                    this.por_no_campo(campo, &texto, window, cx);
                    this.marcas.remove(&campo);
                }
                this.mudou(cx);
                this.tomador_preenchido = Some(conhecido.aviso());
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn ao_sair(&mut self, campo: CampoNota, cx: &mut Context<Self>) {
        if self.tocados.insert(campo) {
            cx.notify();
        }
    }

    /// "Buscar endereco pelo CEP": so' no clique (o CEP sai da maquina). Preenche logradouro, bairro e municipio que
    /// vierem; o que nao vier fica como estava.
    pub(super) fn buscar_cep(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.buscando_cep {
            return;
        }
        let cep = match rustnf_app::cep::cep_limpo(&self.dados.end_cep) {
            Ok(c) => c,
            Err(_) => {
                self.aviso_cep = Some("Digite o CEP com 8 dígitos para buscar o endereço.".into());
                cx.notify();
                return;
            }
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
                            (CampoNota::EnderecoLogradouro, e.logradouro.as_str()),
                            (CampoNota::EnderecoBairro, e.bairro.as_str()),
                            (CampoNota::EnderecoMunicipio, e.municipio.as_str()),
                        ] {
                            if !valor.is_empty() {
                                let normal = this.dados.definir(campo, valor);
                                this.por_no_campo(campo, &normal, window, cx);
                            }
                        }
                        this.mudou(cx);
                    }
                    Err(rustnf_app::cep::ErroCep::NaoEncontrado) => {
                        this.aviso_cep = Some(
                            "CEP não encontrado. Confira os dígitos ou preencha o endereço à mão."
                                .into(),
                        );
                    }
                    Err(_) => {
                        this.aviso_cep = Some(
                            "A busca do CEP não respondeu agora. Preencha o endereço à mão.".into(),
                        );
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Poe o texto no campo da tela (sem disparar `Change`).
    pub(super) fn por_no_campo(
        &mut self,
        campo: CampoNota,
        texto: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let texto = texto.to_string();
        if campo == CampoNota::Descricao {
            self.descricao
                .update(cx, |e, cx| e.set_value(texto, window, cx));
        } else if let Some(e) = self.entradas.get(&campo) {
            e.update(cx, |e, cx| e.set_value(texto, window, cx));
        }
    }

    /// O formulario mudou: reconfere, agenda o rascunho e redesenha.
    pub(super) fn mudou(&mut self, cx: &mut Context<Self>) {
        self.recalcular();
        self.rejeicao = None;
        self.recusa = None;
        self.desistencia = None;
        self.recuperado = false;
        self.agendar_rascunho(cx);
        cx.notify();
    }

    /// Poe todos os dados nos campos da tela.
    pub(super) fn aplicar_dados(
        &mut self,
        dados: DadosNota,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.dados = dados;
        self.tomador_buscado = None;
        self.tomador_preenchido = None;
        self.sincronizar(window, cx);
        self.federais_abertas = !(self.dados.irrf.is_empty()
            && self.dados.csll.is_empty()
            && self.dados.previdencia.is_empty()
            && self.dados.pis_cst.is_empty()
            && self.dados.pis_base.is_empty());
        self.recalcular();
    }

    /// Poe nos campos da tela o texto de `self.dados` (sem disparar `Change`).
    pub(super) fn sincronizar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for campo in CampoNota::TODOS {
            if eh_entrada(campo) || campo == CampoNota::Descricao {
                let texto = self.dados.texto(campo);
                self.por_no_campo(campo, &texto, window, cx);
            }
        }
        self.busca.update(cx, |e, cx| e.set_value("", window, cx));
        self.resultados.clear();
    }

    // ---------------------------------------------------------------- escolhas

    pub(super) fn escolher_sem_tomador(&mut self, valor: bool, cx: &mut Context<Self>) {
        if self.travado() {
            return;
        }
        self.dados.sem_tomador = valor;
        self.mudou(cx);
    }

    pub(super) fn escolher_tributacao(&mut self, valor: u8, cx: &mut Context<Self>) {
        if self.travado() {
            return;
        }
        self.dados.tributacao = valor;
        self.marcas.remove(&CampoNota::Tributacao);
        self.mudou(cx);
    }

    pub(super) fn escolher_retencao(&mut self, valor: u8, cx: &mut Context<Self>) {
        if self.travado() {
            return;
        }
        self.dados.retencao = valor;
        self.marcas.remove(&CampoNota::Retencao);
        self.mudou(cx);
    }

    pub(super) fn escolher_ibscbs(&mut self, valor: bool, cx: &mut Context<Self>) {
        if self.travado() {
            return;
        }
        self.dados.ibscbs_ativo = valor;
        self.mudou(cx);
    }

    pub(super) fn escolher_consumidor_final(&mut self, valor: Option<u8>, cx: &mut Context<Self>) {
        if self.travado() {
            return;
        }
        self.dados.ind_final = valor;
        self.mudou(cx);
    }

    /// Escolhe um servico da lista de resultados: preenche o codigo (a descricao do servico e' da pessoa).
    pub(super) fn escolher_servico(
        &mut self,
        codigo: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.travado() {
            return;
        }
        let normal = self.dados.definir(CampoNota::Ctribnac, codigo);
        self.por_no_campo(CampoNota::Ctribnac, &normal, window, cx);
        self.busca.update(cx, |e, cx| e.set_value("", window, cx));
        self.resultados.clear();
        self.tocados.insert(CampoNota::Ctribnac);
        self.marcas.remove(&CampoNota::Ctribnac);
        self.mudou(cx);
    }

    /// Escolhe um dos NBS que o anexo traz para o servico.
    pub(super) fn escolher_nbs(&mut self, nbs: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.travado() {
            return;
        }
        let normal = self.dados.definir(CampoNota::Nbs, nbs);
        self.por_no_campo(CampoNota::Nbs, &normal, window, cx);
        self.tocados.insert(CampoNota::Nbs);
        self.mudou(cx);
    }

    /// Aceita a proposta da correlacao: preenche so' os campos vazios.
    pub(super) fn aceitar_sugestao(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.travado() {
            return;
        }
        let Some(proposta) = self.sugestao.proposta.clone() else {
            return;
        };
        for campo in proposta.aceitar(&mut self.dados) {
            let texto = self.dados.texto(campo);
            self.por_no_campo(campo, &texto, window, cx);
            self.marcas.remove(&campo);
        }
        self.mudou(cx);
    }

    pub(super) fn alternar_federais(&mut self, cx: &mut Context<Self>) {
        self.federais_abertas = !self.federais_abertas;
        cx.notify();
    }

    // ---------------------------------------------------------------- erros visiveis

    /// A mensagem de erro do campo, se ja pode aparecer: depois de a pessoa passar por ele, ou depois de tentar emitir.
    pub(super) fn erro_do(&self, campo: CampoNota) -> Option<String> {
        if let Some(m) = self.marcas.get(&campo) {
            return Some(m.clone());
        }
        if !(self.mostrar_tudo || self.tocados.contains(&campo)) {
            return None;
        }
        self.conferencia.do_campo(campo).map(|p| p.mensagem.clone())
    }

    // ---------------------------------------------------------------- rascunho

    fn agendar_rascunho(&mut self, cx: &mut Context<Self>) {
        self.versao += 1;
        self.rascunho = if self.dados.vazio(&self.ctx.hoje()) {
            EstadoDoRascunho::Nenhum
        } else {
            EstadoDoRascunho::Alterado
        };
        let versao = self.versao;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(ESPERA_DO_RASCUNHO).await;
            this.update(cx, |this, cx| {
                if this.versao == versao {
                    this.gravar_rascunho(cx);
                }
            })
            .ok();
        })
        .detach();
    }

    /// Grava (ou apaga, se o formulario voltou a ficar vazio) o rascunho, fora da tela.
    fn gravar_rascunho(&mut self, cx: &mut Context<Self>) {
        if !self.pronta || !matches!(self.fase, Fase::Editando) {
            return;
        }
        let vazio = self.dados.vazio(&self.ctx.hoje());
        let servicos = Arc::clone(&self.servicos);
        let config = self.config.clone();
        let dados = self.dados.clone();
        let versao = self.versao;
        cx.spawn(async move |this, cx| {
            let ok = cx
                .background_spawn(async move {
                    if vazio {
                        servicos.apagar_rascunho(&config);
                        true
                    } else {
                        servicos.gravar_rascunho(&config, &dados).is_ok()
                    }
                })
                .await;
            this.update(cx, |this, cx| {
                if this.versao == versao {
                    this.rascunho = match (vazio, ok) {
                        (true, _) => EstadoDoRascunho::Nenhum,
                        (false, true) => EstadoDoRascunho::Salvo,
                        (false, false) => EstadoDoRascunho::Alterado,
                    };
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// Esvazia o formulario (e apaga o rascunho).
    pub(super) fn limpar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.travado() {
            return;
        }
        self.dados = self.ctx.nota_nova();
        self.tomador_buscado = None;
        self.tomador_preenchido = None;
        self.sincronizar(window, cx);
        self.tocados.clear();
        self.marcas.clear();
        self.mostrar_tudo = false;
        self.federais_abertas = false;
        self.mudou(cx);
        self.versao += 1;
        let servicos = Arc::clone(&self.servicos);
        let config = self.config.clone();
        cx.background_executor()
            .spawn(async move { servicos.apagar_rascunho(&config) })
            .detach();
        self.rascunho = EstadoDoRascunho::Nenhum;
    }

    // ---------------------------------------------------------------- navegacao

    /// Leva a pessoa ao campo: abre a secao recolhida, rola ate ela e poe o foco no campo.
    pub(super) fn ir_ao_campo(
        &mut self,
        campo: CampoNota,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.mostrar_tudo = true;
        if campo.secao() == SecaoNota::Federais {
            self.federais_abertas = true;
        }
        let indice = SecaoNota::TODAS
            .iter()
            .position(|s| *s == campo.secao())
            .map_or(0, |i| i + 1);
        self.rolagem.scroll_to_top_of_item(indice);
        if campo == CampoNota::Descricao {
            let d = self.descricao.clone();
            d.update(cx, |e, cx| e.focus(window, cx));
        } else if let Some(e) = self.entradas.get(&campo).cloned() {
            e.update(cx, |e, cx| e.focus(window, cx));
        }
        cx.notify();
    }

    pub(super) fn pedir_aliquotas(&mut self, cx: &mut Context<Self>) {
        cx.emit(EventoNota::IrParaAliquotas);
    }
}
