// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O caixa: a NFC-e (modelo 65) como a venda rapida do balcao. Busca o produto pelo codigo ou pelo nome (o cadastro de
//! produtos da NF-e), lista o cupom com o total grande, aceita desconto, CPF opcional e uma ou mais formas de pagamento com
//! troco, e emite com o teclado (Enter na busca poe o produto; Enter no valor recebido lanca o dinheiro; Ctrl+Enter emite).
//! Depois da autorizacao o cupom sai como arquivo e a tela ja esta pronta para a proxima venda.
//!
//! Marcacao de producao: em producao, a PRIMEIRA emissao da sessao do caixa pede "Entendo que a nota tem efeito fiscal" e
//! a marcacao vale ate a janela fechar ou o ambiente mudar. Por cupom o caixa ficaria lento; o que protege a pessoa de vender
//! sem querer em producao e' o selo vermelho do ambiente sempre a vista e o botao que so' habilita depois da marcacao. Sem
//! ela, nenhuma chamada sai. Sem clique duplo: com o envio em curso o botao nao faz nada.
//!
//! O que decide mora em `modelo::nfce_caixa` e `modelo::nfce_desfecho`; o que fala com disco, cofre e SEFAZ, em
//! `servicos::nfce`. Aqui so' o estado da tela e a ligacao dos eventos.

use std::sync::Arc;

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{
    AppContext as _, Context, Entity, EventEmitter, FocusHandle, ScrollHandle, Subscription, Window,
};
use rustnf_app::config::Config;
use rustnf_app::nfce::Pendente;

use super::nfe_dialogo::{DialogoNfe, EventoDialogoNfe, Operacao};
use super::nfe_nova::ESPERA_DO_RASCUNHO;
use super::senha::DialogoSenha;
use crate::modelo::ambiente::Ambiente;
use crate::modelo::mascara::{
    centavos_para_dinheiro, dinheiro_para_centavos, formatar_dinheiro, formatar_documento,
    so_digitos,
};
use crate::modelo::nfce_caixa::{item_do_produto, quantidade_digitada, Cupom};
use crate::modelo::nfce_config::{ambiente_da_nfce, por_que_o_csc_trava_o_caixa, EstadoCsc};
use crate::modelo::nfce_desfecho::DesfechoCaixa;
use crate::modelo::nfe_fiscal::Pagamento;
use crate::modelo::nfe_nova::ContextoNfe;
use crate::modelo::nfe_produtos::{Cadastro, Produto};
use crate::servicos::Servicos;

mod desenho;
mod envio;

/// O que a tela pede a janela.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EventoCaixa {
    /// Abrir as Configuracoes, na secao NFC-e.
    IrParaConfiguracoes,
    /// A pasta mudou (nota emitida ou transmitida): a lista de Notas le de novo.
    NotaMudou,
    /// Abrir a lista de Notas, na aba NFC-e.
    IrParaNotas,
}

/// A etapa do envio que esta acontecendo de verdade.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Etapa {
    /// Abrindo o certificado.
    Certificado,
    /// Falando com a SEFAZ.
    Enviando,
}

/// Onde o caixa esta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Fase {
    /// Vendendo.
    Vendendo,
    /// Enviando o cupom.
    Enviando(Etapa),
}

/// O que a tela mostra: a venda ou o fechamento do caixa.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Vista {
    /// A venda.
    Venda,
    /// "Fechar o caixa": as notas por transmitir.
    Fechamento,
}

/// O cupom (PDF) da ultima venda.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EstadoDoCupom {
    /// Nada pedido.
    Parado,
    /// Gerando o arquivo.
    Gerando,
}

/// Os campos de texto do caixa.
pub(super) struct Entradas {
    pub(super) busca: Entity<InputState>,
    pub(super) quantidade: Entity<InputState>,
    pub(super) desconto: Entity<InputState>,
    pub(super) cpf: Entity<InputState>,
    pub(super) recebido: Entity<InputState>,
    pub(super) np_descricao: Entity<InputState>,
    pub(super) np_valor: Entity<InputState>,
    pub(super) np_ncm: Entity<InputState>,
}

/// A tela do caixa.
pub(super) struct TelaCaixa {
    pub(super) servicos: Arc<Servicos>,
    pub(super) config: Config,
    /// A NFC-e pode ser usada: a configuracao comum completa mais a UF.
    pub(super) pronta: bool,
    pub(super) csc: EstadoCsc,
    pub(super) cupom: Cupom,
    pub(super) cadastro: Cadastro,
    pub(super) aviso_cadastro: Option<String>,
    pub(super) contexto: ContextoNfe,
    pub(super) e: Entradas,
    pub(super) novo_aberto: bool,
    pub(super) erro_do_novo: Option<String>,
    pub(super) fase: Fase,
    pub(super) vista: Vista,
    pub(super) desfecho: Option<DesfechoCaixa>,
    pub(super) cupom_do_desfecho: EstadoDoCupom,
    pub(super) erro: Option<String>,
    pub(super) chave_copiada: bool,
    pub(super) entendo: bool,
    pub(super) pendentes: Vec<Pendente>,
    pub(super) pendentes_lidos: bool,
    pub(super) dialogo: Entity<DialogoNfe>,
    pub(super) rolagem: ScrollHandle,
    pub(super) foco: FocusHandle,
    versao: u64,
    carregado: bool,
    senha_pedida: Option<Entity<DialogoSenha>>,
    sub_senha: Option<Subscription>,
    _subs: Vec<Subscription>,
}

impl EventEmitter<EventoCaixa> for TelaCaixa {}

fn entrada(
    window: &mut Window,
    cx: &mut Context<TelaCaixa>,
    dica: &'static str,
) -> Entity<InputState> {
    cx.new(|cx| InputState::new(window, cx).placeholder(dica))
}

impl TelaCaixa {
    pub(super) fn new(
        servicos: Arc<Servicos>,
        dialogo: Entity<DialogoNfe>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let e = Entradas {
            busca: entrada(window, cx, "Código ou nome do produto"),
            quantidade: entrada(window, cx, "Qtd"),
            desconto: entrada(window, cx, "0,00"),
            cpf: entrada(window, cx, "000.000.000-00"),
            recebido: entrada(window, cx, "Valor recebido"),
            np_descricao: entrada(window, cx, "Nome do produto"),
            np_valor: entrada(window, cx, "0,00"),
            np_ncm: entrada(window, cx, "8 dígitos"),
        };
        let mut subs = Vec::new();
        subs.push(cx.subscribe_in(
            &e.busca,
            window,
            |this, _, ev: &InputEvent, window, cx| match ev {
                InputEvent::Change => cx.notify(),
                InputEvent::PressEnter { .. } => this.confirmar_a_busca(window, cx),
                _ => {}
            },
        ));
        subs.push(cx.subscribe_in(
            &e.desconto,
            window,
            |this, entrada, ev: &InputEvent, window, cx| {
                if matches!(ev, InputEvent::Change) {
                    let t = entrada.read(cx).value().to_string();
                    let normal = formatar_dinheiro(&t);
                    if normal != t {
                        entrada.update(cx, |s, cx| s.set_value(normal.clone(), window, cx));
                    }
                    this.cupom.desconto = normal;
                    this.mudou(cx);
                }
            },
        ));
        subs.push(cx.subscribe_in(
            &e.cpf,
            window,
            |this, entrada, ev: &InputEvent, window, cx| {
                if matches!(ev, InputEvent::Change) {
                    let t = entrada.read(cx).value().to_string();
                    let digitos = so_digitos(&t, 11);
                    let normal = formatar_documento(&digitos);
                    if normal != t {
                        entrada.update(cx, |s, cx| s.set_value(normal, window, cx));
                    }
                    this.cupom.cpf = digitos;
                    this.mudou(cx);
                }
            },
        ));
        subs.push(cx.subscribe_in(
            &e.recebido,
            window,
            |this, entrada, ev: &InputEvent, window, cx| match ev {
                InputEvent::Change => {
                    let t = entrada.read(cx).value().to_string();
                    let normal = formatar_dinheiro(&t);
                    if normal != t {
                        entrada.update(cx, |s, cx| s.set_value(normal, window, cx));
                    }
                    cx.notify();
                }
                InputEvent::PressEnter { .. } => this.pagar(Pagamento::Dinheiro, window, cx),
                _ => {}
            },
        ));
        subs.push(cx.subscribe_in(
            &e.np_valor,
            window,
            |_, entrada, ev: &InputEvent, window, cx| {
                if matches!(ev, InputEvent::Change) {
                    let t = entrada.read(cx).value().to_string();
                    let normal = formatar_dinheiro(&t);
                    if normal != t {
                        entrada.update(cx, |s, cx| s.set_value(normal, window, cx));
                    }
                }
            },
        ));
        subs.push(cx.subscribe_in(
            &e.np_ncm,
            window,
            |this, entrada, ev: &InputEvent, window, cx| match ev {
                InputEvent::Change => {
                    let t = entrada.read(cx).value().to_string();
                    let digitos = so_digitos(&t, 8);
                    if digitos != t {
                        entrada.update(cx, |s, cx| s.set_value(digitos, window, cx));
                    }
                }
                InputEvent::PressEnter { .. } => this.guardar_o_produto_novo(window, cx),
                _ => {}
            },
        ));
        // o que a retomada fez muda a pasta: o fechamento e a lista leem de novo
        subs.push(
            cx.subscribe_in(&dialogo, window, |this, _, _: &EventoDialogoNfe, _, cx| {
                this.recarregar_pendentes(cx);
                cx.emit(EventoCaixa::NotaMudou);
            }),
        );
        Self {
            servicos,
            config: Config::default(),
            pronta: false,
            csc: EstadoCsc::Verificando,
            cupom: Cupom::default(),
            cadastro: Cadastro::default(),
            aviso_cadastro: None,
            contexto: ContextoNfe::de_config(&Config::default(), chrono::NaiveDate::default(), 1),
            e,
            novo_aberto: false,
            erro_do_novo: None,
            fase: Fase::Vendendo,
            vista: Vista::Venda,
            desfecho: None,
            cupom_do_desfecho: EstadoDoCupom::Parado,
            erro: None,
            chave_copiada: false,
            entendo: false,
            pendentes: Vec::new(),
            pendentes_lidos: false,
            dialogo,
            rolagem: ScrollHandle::new(),
            foco: cx.focus_handle(),
            versao: 0,
            carregado: false,
            senha_pedida: None,
            sub_senha: None,
            _subs: subs,
        }
    }

    /// O ambiente da NFC-e em vigor.
    pub(super) fn ambiente(&self) -> Ambiente {
        ambiente_da_nfce(&self.config.nfce)
    }

    /// Em producao o envio pede a marcacao (uma vez por sessao do caixa).
    pub(super) fn exige_efeito_fiscal(&self) -> bool {
        self.ambiente() == Ambiente::Producao
    }

    /// A janela entrega a configuracao que vale (a efetiva) quando ela chega ou muda. A tela nasce ANTES dela: nada que dependa
    /// do emitente, da pasta ou do ambiente e' decidido no construtor.
    pub(super) fn definir_config(
        &mut self,
        config: Config,
        pronta: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mudou_o_ambiente = self.config.nfce.ambiente != config.nfce.ambiente;
        let mudou_a_pasta = self.config.notas.pasta != config.notas.pasta;
        self.config = config;
        self.pronta = pronta;
        if mudou_o_ambiente {
            self.entendo = false;
        }
        if pronta && (!self.carregado || mudou_a_pasta) {
            self.carregar(window, cx);
        } else if pronta {
            self.atualizar_o_contexto(cx);
        }
        cx.notify();
    }

    /// Le fora da tela o que mora na pasta e no cofre: o cadastro de produtos, o rascunho (so' na primeira vez), o contexto, o
    /// CSC e as notas por transmitir.
    fn carregar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.carregado = true;
        let (servicos, config) = (Arc::clone(&self.servicos), self.config.clone());
        cx.spawn_in(window, async move |this, cx| {
            let (s, c) = (Arc::clone(&servicos), config.clone());
            let lido = cx
                .background_spawn(async move {
                    (
                        s.ler_produtos(&c),
                        s.ler_rascunho_caixa(&c),
                        s.contexto_do_caixa(&c),
                        s.estado_do_csc(&c),
                        s.pendentes_nfce(&c).ok(),
                    )
                })
                .await;
            this.update_in(cx, |this, window, cx| {
                let (produtos, rascunho, contexto, csc, pendentes) = lido;
                this.cadastro = produtos.cadastro;
                this.aviso_cadastro = produtos.aviso;
                this.contexto = contexto;
                this.csc = csc;
                if let Some(p) = pendentes {
                    this.pendentes = p;
                    this.pendentes_lidos = true;
                }
                if let Some(r) = rascunho.filter(|_| this.cupom.vazio()) {
                    this.cupom = r;
                    this.sincronizar_os_campos(window, cx);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Poe nos campos de texto o que o cupom guarda (depois de restaurar o rascunho ou de limpar).
    pub(super) fn sincronizar_os_campos(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let desconto = self.cupom.desconto.clone();
        let cpf = formatar_documento(&self.cupom.cpf);
        let cpf = if self.cupom.cpf.is_empty() {
            String::new()
        } else {
            cpf
        };
        self.e
            .desconto
            .update(cx, |s, cx| s.set_value(desconto, window, cx));
        self.e.cpf.update(cx, |s, cx| s.set_value(cpf, window, cx));
        for e in [&self.e.recebido, &self.e.busca, &self.e.quantidade] {
            e.update(cx, |s, cx| s.set_value("", window, cx));
        }
    }

    /// O contexto (regime, numero, hoje) e o CSC de novo, fora da tela.
    pub(super) fn atualizar_o_contexto(&mut self, cx: &mut Context<Self>) {
        let (servicos, config) = (Arc::clone(&self.servicos), self.config.clone());
        cx.spawn(async move |this, cx| {
            let r = cx
                .background_spawn(async move {
                    (
                        servicos.contexto_do_caixa(&config),
                        servicos.estado_do_csc(&config),
                    )
                })
                .await;
            this.update(cx, |this, cx| {
                this.contexto = r.0;
                this.csc = r.1;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Le de novo as notas por transmitir, fora da tela.
    pub(super) fn recarregar_pendentes(&mut self, cx: &mut Context<Self>) {
        if !self.pronta {
            return;
        }
        let (servicos, config) = (Arc::clone(&self.servicos), self.config.clone());
        cx.spawn(async move |this, cx| {
            let r = cx
                .background_spawn(async move { servicos.pendentes_nfce(&config) })
                .await;
            this.update(cx, |this, cx| {
                if let Ok(p) = r {
                    this.pendentes = p;
                    this.pendentes_lidos = true;
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    // ---------------------------------------------------------------- o cupom

    /// O cupom mudou: o rascunho e' agendado.
    pub(super) fn mudou(&mut self, cx: &mut Context<Self>) {
        self.agendar_rascunho(cx);
        cx.notify();
    }

    fn agendar_rascunho(&mut self, cx: &mut Context<Self>) {
        self.versao += 1;
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

    /// Grava (ou apaga, se o cupom voltou a ficar vazio) o rascunho, fora da tela.
    fn gravar_rascunho(&mut self, cx: &mut Context<Self>) {
        if !self.pronta {
            return;
        }
        let (servicos, config, cupom) = (
            Arc::clone(&self.servicos),
            self.config.clone(),
            self.cupom.clone(),
        );
        cx.background_spawn(async move {
            if cupom.vazio() {
                servicos.apagar_rascunho_caixa(&config);
            } else {
                let _ = servicos.gravar_rascunho_caixa(&config, &cupom);
            }
        })
        .detach();
    }

    /// Enter na busca: poe o primeiro produto; sem achar nada, oferece cadastrar o que foi digitado.
    fn confirmar_a_busca(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !matches!(self.fase, Fase::Vendendo) {
            return;
        }
        let texto = self.e.busca.read(cx).value().trim().to_string();
        if texto.is_empty() {
            return;
        }
        // o codigo exato ganha da busca por nome
        let exato = self
            .cadastro
            .produtos
            .iter()
            .position(|p| p.codigo.eq_ignore_ascii_case(&texto));
        let primeiro = exato.or_else(|| self.cadastro.indices(&texto, 1).first().copied());
        match primeiro {
            Some(i) => self.adicionar_do_cadastro(i, window, cx),
            None => self.abrir_o_cadastro_novo(texto, window, cx),
        }
    }

    /// Poe o produto `i` do cadastro no cupom, na quantidade do campo (vazio = 1).
    pub(super) fn adicionar_do_cadastro(
        &mut self,
        i: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(p) = self.cadastro.produtos.get(i).cloned() else {
            return;
        };
        let q = self.e.quantidade.read(cx).value().to_string();
        let Some(q) = quantidade_digitada(&q) else {
            self.erro =
                Some("A quantidade tem de ser um número maior que zero (ex.: 1 ou 0,5).".into());
            cx.notify();
            return;
        };
        self.erro = None;
        self.desfecho_some_se_nao_pendente();
        self.cupom.adicionar(item_do_produto(&p), q);
        for e in [&self.e.busca, &self.e.quantidade] {
            e.update(cx, |s, cx| s.set_value("", window, cx));
        }
        self.mudou(cx);
    }

    /// Uma venda nova apaga o aviso da anterior, menos o que ainda pede acao (nota sem resposta ou off-line).
    fn desfecho_some_se_nao_pendente(&mut self) {
        if self.desfecho.as_ref().is_some_and(|d| !d.deixa_pendente()) {
            self.desfecho = None;
        }
    }

    pub(super) fn mudar_quantidade(&mut self, i: usize, delta: i64, cx: &mut Context<Self>) {
        self.cupom.mudar_quantidade(i, delta);
        self.mudou(cx);
    }

    pub(super) fn tirar_item(&mut self, i: usize, cx: &mut Context<Self>) {
        self.cupom.tirar(i);
        self.mudou(cx);
    }

    /// Lanca um pagamento: o valor do campo "recebido" ou, vazio, o que falta.
    pub(super) fn pagar(&mut self, forma: Pagamento, window: &mut Window, cx: &mut Context<Self>) {
        if !matches!(self.fase, Fase::Vendendo) {
            return;
        }
        let texto = self.e.recebido.read(cx).value().to_string();
        let centavos = dinheiro_para_centavos(&texto).unwrap_or(0);
        if self.cupom.pagar(forma, centavos) {
            self.e
                .recebido
                .update(cx, |s, cx| s.set_value("", window, cx));
            self.mudou(cx);
        }
    }

    pub(super) fn tirar_pagamento(&mut self, i: usize, cx: &mut Context<Self>) {
        self.cupom.tirar_pagamento(i);
        self.mudou(cx);
    }

    /// Esvazia o cupom (e apaga o rascunho): a proxima venda.
    pub(super) fn limpar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.cupom = Cupom::default();
        self.sincronizar_os_campos(window, cx);
        self.mudou(cx);
    }

    // ---------------------------------------------------------------- produto novo

    fn abrir_o_cadastro_novo(
        &mut self,
        texto: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.novo_aberto = true;
        self.erro_do_novo = None;
        self.e
            .np_descricao
            .update(cx, |s, cx| s.set_value(texto, window, cx));
        for e in [&self.e.np_valor, &self.e.np_ncm] {
            e.update(cx, |s, cx| s.set_value("", window, cx));
        }
        cx.notify();
    }

    pub(super) fn abrir_o_cadastro(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let texto = self.e.busca.read(cx).value().trim().to_string();
        self.abrir_o_cadastro_novo(texto, window, cx);
    }

    pub(super) fn fechar_o_cadastro(&mut self, cx: &mut Context<Self>) {
        self.novo_aberto = false;
        self.erro_do_novo = None;
        cx.notify();
    }

    /// Guarda o produto novo no cadastro (o mesmo arquivo da NF-e) e o poe no cupom. O NCM de 8 digitos e' obrigatorio.
    pub(super) fn guardar_o_produto_novo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let descricao = self.e.np_descricao.read(cx).value().trim().to_string();
        let valor = self.e.np_valor.read(cx).value().to_string();
        let ncm = so_digitos(&self.e.np_ncm.read(cx).value(), 8);
        let erro = if descricao.chars().count() < 2 {
            Some("Escreva o nome do produto.")
        } else if dinheiro_para_centavos(&valor).filter(|v| *v > 0).is_none() {
            Some("Informe o preço (maior que zero).")
        } else if ncm.len() != 8 {
            Some("O NCM de 8 dígitos é obrigatório na nota. Confira na tabela da Receita.")
        } else {
            None
        };
        if let Some(e) = erro {
            self.erro_do_novo = Some(e.into());
            cx.notify();
            return;
        }
        // o codigo e' o proximo numero livre do cadastro: o produto nao fica sem codigo (a busca e o cupom o usam)
        let codigo = (1..)
            .map(|n| format!("P{n}"))
            .find(|c| !self.cadastro.tem(c))
            .unwrap_or_default();
        let produto = Produto {
            codigo,
            descricao,
            valor: centavos_para_dinheiro(dinheiro_para_centavos(&valor).unwrap_or(0)),
            ncm,
            ..Produto::default()
        };
        let (servicos, config, novo) = (
            Arc::clone(&self.servicos),
            self.config.clone(),
            produto.clone(),
        );
        self.novo_aberto = false;
        cx.spawn_in(window, async move |this, cx| {
            let (s, c) = (Arc::clone(&servicos), config.clone());
            let r = cx
                .background_spawn(async move { s.guardar_produtos(&c, vec![novo]) })
                .await;
            let lido = cx
                .background_spawn(async move { servicos.ler_produtos(&config) })
                .await;
            this.update_in(cx, |this, window, cx| {
                this.cadastro = lido.cadastro;
                this.aviso_cadastro = lido.aviso;
                match r {
                    Ok(_) => {
                        if let Some(i) = this
                            .cadastro
                            .produtos
                            .iter()
                            .position(|p| p.codigo == produto.codigo)
                        {
                            this.adicionar_do_cadastro(i, window, cx);
                        }
                    }
                    Err(e) => this.erro = Some(e.texto()),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    // ---------------------------------------------------------------- pode emitir?

    /// Por que a venda nao pode ser emitida agora, em uma frase; `None` = pode.
    pub(super) fn por_que_nao_emite(&self) -> Option<String> {
        if !self.pronta {
            return Some("Falta configurar a NFC-e.".into());
        }
        if !matches!(self.fase, Fase::Vendendo) {
            return Some("Enviando a venda...".into());
        }
        if let Some(t) = por_que_o_csc_trava_o_caixa(self.config.nfce.qr_versao, self.csc) {
            return Some(t);
        }
        if let Some(p) = self.cupom.pendencias(&self.contexto).first() {
            return Some(format!("{} {}", p.mensagem, p.como_resolver));
        }
        if self.exige_efeito_fiscal() && !self.entendo {
            return Some(
                "Em produção, marque \"Entendo que a nota tem efeito fiscal\" para emitir.".into(),
            );
        }
        None
    }

    /// O cupom pode ser emitido agora.
    pub(super) fn pode_emitir(&self) -> bool {
        self.por_que_nao_emite().is_none()
    }

    // ---------------------------------------------------------------- fechamento

    pub(super) fn abrir_o_fechamento(&mut self, cx: &mut Context<Self>) {
        self.vista = Vista::Fechamento;
        self.recarregar_pendentes(cx);
        cx.notify();
    }

    pub(super) fn voltar_a_venda(&mut self, cx: &mut Context<Self>) {
        self.vista = Vista::Venda;
        cx.notify();
    }

    /// "Transmitir agora": retoma as NFC-e por transmitir pelo dialogo da lista (a mesma retomada: consulta a MESMA chave e so'
    /// reenvia a MESMA nota; nunca cria outra).
    pub(super) fn transmitir_agora(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let n = self.pendentes.len();
        if n == 0 {
            return;
        }
        self.dialogo.update(cx, |d, cx| {
            d.iniciar(Operacao::Retomar, None, n, window, cx);
        });
    }

    /// As notas por transmitir, para quem usa o Retomar do desfecho.
    pub(super) fn retomar_do_desfecho(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let n = self.pendentes.len().max(1);
        self.dialogo.update(cx, |d, cx| {
            d.iniciar(Operacao::Retomar, None, n, window, cx);
        });
    }

    // ---------------------------------------------------------------- o cupom como arquivo

    fn pedir_ao_sistema(
        &self,
        destino: crate::servicos::Destino,
        caminho: &std::path::Path,
        cx: &mut gpui_kit::App,
    ) {
        if self.servicos.abridor().pedir(destino, caminho) {
            match destino {
                crate::servicos::Destino::Abrir => cx.open_with_system(caminho),
                crate::servicos::Destino::Mostrar => cx.reveal_path(caminho),
            }
        }
    }

    /// "Abrir o cupom": gera o DANFC-e em PDF e pede ao sistema que o abra.
    pub(super) fn abrir_o_cupom(&mut self, cx: &mut Context<Self>) {
        let Some(chave) = self
            .desfecho
            .as_ref()
            .and_then(|d| d.chave())
            .map(str::to_string)
        else {
            return;
        };
        if self.cupom_do_desfecho == EstadoDoCupom::Gerando {
            return;
        }
        self.cupom_do_desfecho = EstadoDoCupom::Gerando;
        self.erro = None;
        cx.notify();
        let (servicos, config) = (Arc::clone(&self.servicos), self.config.clone());
        cx.spawn(async move |this, cx| {
            let r = cx
                .background_spawn(async move { servicos.danfce_da_pasta(&config, &chave) })
                .await;
            this.update(cx, |this, cx| {
                this.cupom_do_desfecho = EstadoDoCupom::Parado;
                match r {
                    Ok(c) => this.pedir_ao_sistema(crate::servicos::Destino::Abrir, &c, cx),
                    Err(e) => this.erro = Some(e.texto()),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// "Salvar para a impressora (ESC/POS)": grava os bytes do cupom e mostra a pasta. O programa nao fala com a impressora.
    pub(super) fn salvar_escpos(&mut self, cx: &mut Context<Self>) {
        let Some(chave) = self
            .desfecho
            .as_ref()
            .and_then(|d| d.chave())
            .map(str::to_string)
        else {
            return;
        };
        self.erro = None;
        let (servicos, config) = (Arc::clone(&self.servicos), self.config.clone());
        cx.spawn(async move |this, cx| {
            let r = cx
                .background_spawn(async move { servicos.escpos_da_pasta(&config, &chave) })
                .await;
            this.update(cx, |this, cx| {
                match r {
                    Ok(c) => this.pedir_ao_sistema(crate::servicos::Destino::Mostrar, &c, cx),
                    Err(e) => this.erro = Some(e.texto()),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn copiar_a_chave(&mut self, cx: &mut Context<Self>) {
        if let Some(chave) = self.desfecho.as_ref().and_then(|d| d.chave()) {
            cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(chave.to_string()));
            self.chave_copiada = true;
            cx.notify();
        }
    }

    pub(super) fn fechar_o_aviso(&mut self, cx: &mut Context<Self>) {
        self.desfecho = None;
        self.erro = None;
        cx.notify();
    }

    pub(super) fn ver_nas_notas(&mut self, cx: &mut Context<Self>) {
        cx.emit(EventoCaixa::IrParaNotas);
    }
}
