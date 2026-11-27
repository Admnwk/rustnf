// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O fluxo de enviar: abrir o certificado, enviar fora da tela, tratar o desfecho, retomar a nota sem resposta, verificar
//! se a SEFAZ esta no ar e emitir em contingencia (EPEC). Nada sai sem o botao; em producao, nem com ele sem a marcacao
//! "Entendo que a nota tem efeito fiscal" (sem ela, nenhuma chamada ao transporte). Sem clique duplo: com o envio em
//! curso (ou com desfecho na tela) o botao nao faz nada.

use std::sync::Arc;

use gpui_kit::{AppContext as _, Context, Window};
use rustnf_app::segredo::Segredo;

use super::{Acao, Etapa, EventoNovaNfe, Fase, NovaNfe};
use crate::fora;
use crate::modelo::ambiente::Ambiente;
use crate::modelo::nfe_config::{ambiente_da_nfe, ResultadoSefaz};
use crate::modelo::nfe_emissao::DesfechoNfeEmissao;
use crate::modelo::nfe_nova::{pedido, PassoNfe};
use crate::modelo::nfe_resultados::ResumoDaRetomada;
use crate::servicos::{emitir_nfe, epec_nfe, retomar_nfe, testar_sefaz, ErroTela};
use crate::telas::senha::{classificar, Credencial, DialogoSenha, EventoSenha};

/// O motivo da contingencia tem de ter de 15 a 255 caracteres (a regra da biblioteca e do MOC).
pub(super) const MOTIVO_MIN: usize = 15;
/// O maximo do motivo.
pub(super) const MOTIVO_MAX: usize = 255;

/// O resultado do trabalho de rede de uma acao.
enum Feito {
    Desfecho(DesfechoNfeEmissao),
    Status(ResultadoSefaz),
    Retomada(ResumoDaRetomada, Option<DesfechoNfeEmissao>),
}

impl NovaNfe {
    /// O ambiente da NF-e em vigor.
    pub(in crate::telas) fn ambiente(&self) -> Ambiente {
        ambiente_da_nfe(&self.config.nfe)
    }

    /// Em producao o envio pede a marcacao explicita.
    pub(super) fn exige_efeito_fiscal(&self) -> bool {
        self.ambiente() == Ambiente::Producao
    }

    /// O botao Enviar: so' habilita com o assistente conferido e limpo e, em producao, depois da marcacao.
    pub(in crate::telas) fn pode_enviar(&self) -> bool {
        matches!(self.fase, Fase::Editando)
            && self.pronta
            && !self.conferindo
            && self.conferencia.limpa()
            && (!self.exige_efeito_fiscal() || self.entendo)
    }

    /// O texto do motivo da contingencia, sem espacos nas pontas.
    pub(super) fn texto_do_motivo(&self, cx: &gpui_kit::App) -> String {
        self.motivo.read(cx).value().trim().to_string()
    }

    /// A contingencia pode ser pedida agora: a SEFAZ nao respondeu ou esta parada, o assistente esta conferido, o motivo
    /// tem de 15 a 255 letras, nao ha nota sem resposta na pasta (seria uma nota gemea) e, em producao, houve a marcacao.
    pub(in crate::telas) fn pode_epec(&self, cx: &gpui_kit::App) -> bool {
        let n = self.texto_do_motivo(cx).chars().count();
        matches!(self.fase, Fase::Editando)
            && self.pronta
            && self.conferencia.limpa()
            && self.pendentes == 0
            && matches!(
                self.status,
                Some(ResultadoSefaz::Parado { .. } | ResultadoSefaz::SemResposta)
            )
            && (MOTIVO_MIN..=MOTIVO_MAX).contains(&n)
            && (!self.exige_efeito_fiscal() || self.entendo)
    }

    /// Enviar (o botao): congela nada, confere de novo que pode, abre o certificado e envia.
    pub(in crate::telas) fn enviar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.pode_enviar() {
            self.executar(Acao::Enviar, None, window, cx);
        }
    }

    /// Emitir em contingencia (o botao).
    pub(in crate::telas) fn emitir_em_contingencia(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.pode_epec(cx) {
            self.executar(Acao::Epec, None, window, cx);
        }
    }

    /// Retomar a nota sem resposta (o botao): consulta a MESMA chave e, so' se a SEFAZ nao a recebeu, reenvia a MESMA nota.
    pub(in crate::telas) fn retomar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let pendente = matches!(&self.fase, Fase::Desfecho(d) if d.deixa_pendente());
        if pendente && (!self.exige_efeito_fiscal() || self.entendo) {
            self.executar(Acao::Retomar, None, window, cx);
        }
    }

    /// "A SEFAZ esta no ar?" (o botao do passo final): so' pergunta; nada e' emitido.
    pub(in crate::telas) fn verificar_a_sefaz(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.verificando || !matches!(self.fase, Fase::Editando) || !self.pronta {
            return;
        }
        self.executar(Acao::Verificar, None, window, cx);
    }

    fn executar(
        &mut self,
        acao: Acao,
        digitada: Option<Segredo>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(self.fase, Fase::Enviando(_)) || (acao == Acao::Verificar && self.verificando) {
            return;
        }
        let anterior = self.fase.clone();
        if acao == Acao::Verificar {
            self.verificando = true;
            self.status = None;
        } else {
            self.fase = Fase::Enviando(Etapa::Certificado);
            self.rejeicao = None;
        }
        cx.notify();
        let pedido_json = match acao {
            Acao::Enviar | Acao::Epec => match pedido(&self.dados, &self.ctx) {
                Ok(mut p) => {
                    if acao == Acao::Epec {
                        p["x_just"] = serde_json::json!(self.texto_do_motivo(cx));
                    }
                    Some(p)
                }
                Err(_) => {
                    self.fase = anterior;
                    cx.notify();
                    return;
                }
            },
            _ => None,
        };
        let chave_pendente = match &anterior {
            Fase::Desfecho(d) => chave_do(d),
            _ => None,
        };
        let servicos = Arc::clone(&self.servicos);
        let fabrica = servicos.fabrica_nfe();
        let config = self.config.clone();
        let pendentes = self.pendentes;
        let digitou = digitada.is_some();
        cx.spawn_in(window, async move |this, cx| {
            let (s2, c2) = (Arc::clone(&servicos), config.clone());
            let preparo = cx
                .background_spawn(async move { s2.preparar_nfe(&c2, digitada.as_ref()) })
                .await;
            let preparo = match classificar(preparo, digitou) {
                Credencial::Pronta(p) => *p,
                Credencial::PedirSenha(erro) => {
                    this.update_in(cx, |this, window, cx| {
                        this.fase = anterior;
                        this.verificando = false;
                        this.pedir_senha(acao, erro, window, cx);
                    })
                    .ok();
                    return;
                }
                Credencial::Falhou(texto) => {
                    this.update_in(cx, |this, _, cx| {
                        this.verificando = false;
                        this.terminou(
                            Feito::Desfecho(DesfechoNfeEmissao::Recusada {
                                pendencias: vec![crate::modelo::nfe_nova::PendenciaNfe {
                                    passo: PassoNfe::Conferir,
                                    item: None,
                                    mensagem: texto,
                                    como_resolver:
                                        "Resolva isso e tente de novo: nada saiu da sua maquina."
                                            .into(),
                                }],
                            }),
                            anterior,
                            cx,
                        );
                    })
                    .ok();
                    return;
                }
            };
            if acao != Acao::Verificar {
                this.update_in(cx, |this, _, cx| {
                    this.fase = Fase::Enviando(match acao {
                        Acao::Epec => Etapa::Epec,
                        Acao::Retomar => Etapa::Retomando,
                        _ => Etapa::Enviando,
                    });
                    cx.notify();
                })
                .ok();
            }
            let (s3, c3) = (Arc::clone(&servicos), config.clone());
            let trabalho = async move {
                match acao {
                    Acao::Enviar => Feito::Desfecho(
                        emitir_nfe(fabrica, preparo, config, pedido_json.unwrap_or_default()).await,
                    ),
                    Acao::Epec => Feito::Desfecho(
                        epec_nfe(
                            fabrica,
                            preparo,
                            config,
                            pedido_json.unwrap_or_default(),
                            pendentes,
                        )
                        .await,
                    ),
                    Acao::Verificar => Feito::Status(testar_sefaz(fabrica, preparo, config).await),
                    Acao::Retomar => {
                        let r = retomar_nfe(fabrica, preparo, c3.clone()).await;
                        Feito::Retomada(r.clone(), depois_da_retomada(&s3, &c3, chave_pendente, &r))
                    }
                }
            };
            let feito = match fora::rede(trabalho) {
                Some(tarefa) => tarefa.await.unwrap_or_else(|_| {
                    Feito::Desfecho(DesfechoNfeEmissao::SemResposta {
                        chave: String::new(),
                        numero: String::new(),
                        serie: String::new(),
                    })
                }),
                None => Feito::Status(ResultadoSefaz::Recusado {
                    texto: "Não foi possível preparar a chamada de rede.".into(),
                }),
            };
            this.update_in(cx, |this, _, cx| {
                this.verificando = false;
                this.terminou(feito, anterior, cx);
            })
            .ok();
        })
        .detach();
    }

    /// Trata o que a rede devolveu.
    fn terminou(&mut self, feito: Feito, anterior: Fase, cx: &mut Context<Self>) {
        match feito {
            Feito::Status(s) => {
                self.epec_aberto = matches!(
                    s,
                    ResultadoSefaz::Parado { .. } | ResultadoSefaz::SemResposta
                );
                self.status = Some(s);
                if matches!(self.fase, Fase::Enviando(_)) {
                    self.fase = anterior;
                }
            }
            Feito::Desfecho(d) => self.aplicar_desfecho(d, cx),
            Feito::Retomada(resumo, novo) => {
                self.retomada = Some(resumo);
                match novo {
                    Some(d) => self.aplicar_desfecho(d, cx),
                    None => self.fase = anterior,
                }
                cx.emit(EventoNovaNfe::NotaMudou);
                self.atualizar_o_contexto(cx);
            }
        }
        cx.notify();
    }

    /// Trata como a emissao terminou.
    pub(super) fn aplicar_desfecho(&mut self, d: DesfechoNfeEmissao, cx: &mut Context<Self>) {
        match &d {
            DesfechoNfeEmissao::Autorizada { .. } => {
                self.fase = Fase::Desfecho(Box::new(d));
                self.copiada = false;
                self.apagar_rascunho(cx);
                self.guardar_os_produtos(cx);
                cx.emit(EventoNovaNfe::NotaMudou);
                self.atualizar_o_contexto(cx);
            }
            DesfechoNfeEmissao::EpecRegistrado { .. }
            | DesfechoNfeEmissao::SemResposta { .. }
            | DesfechoNfeEmissao::SefazFora { .. } => {
                // nota na pasta (pendente ou em contingencia): o assistente fica com o desfecho, sem volta
                let epec = matches!(d, DesfechoNfeEmissao::EpecRegistrado { .. });
                self.fase = Fase::Desfecho(Box::new(d));
                self.copiada = false;
                if epec {
                    self.apagar_rascunho(cx);
                }
                cx.emit(EventoNovaNfe::NotaMudou);
                self.atualizar_o_contexto(cx);
            }
            DesfechoNfeEmissao::Denegada { .. } | DesfechoNfeEmissao::NumeroJaUsado { .. } => {
                self.fase = Fase::Desfecho(Box::new(d));
                cx.emit(EventoNovaNfe::NotaMudou);
                self.atualizar_o_contexto(cx);
            }
            DesfechoNfeEmissao::Rejeitada { passo, .. } => {
                // volta a editar, com os dados intactos, no passo onde o problema se resolve
                self.passo = *passo;
                self.rejeicao = Some(d);
                self.fase = Fase::Editando;
                self.conferencia = Default::default();
                self.atualizar_o_contexto(cx);
            }
            DesfechoNfeEmissao::Recusada { pendencias } => {
                if let Some(p) = pendencias.first() {
                    self.passo = p.passo;
                }
                self.rejeicao = Some(d);
                self.fase = Fase::Editando;
                self.conferencia = Default::default();
            }
            DesfechoNfeEmissao::EpecRecusado { .. } => {
                self.rejeicao = Some(d);
                self.fase = Fase::Editando;
            }
        }
        self.rolagem.set_offset(gpui_kit::Point::default());
    }

    /// Autorizada: os produtos novos da nota entram no cadastro (os que a pessoa deixou marcados), fora da tela.
    fn guardar_os_produtos(&mut self, cx: &mut Context<Self>) {
        let novos = self.cadastro.novos_da_nota(&self.dados.itens);
        self.produtos_guardados = None;
        if novos.is_empty() {
            return;
        }
        let servicos = Arc::clone(&self.servicos);
        let config = self.config.clone();
        cx.spawn(async move |this, cx| {
            let (s2, c2) = (Arc::clone(&servicos), config.clone());
            let r = cx
                .background_spawn(async move { s2.guardar_produtos(&c2, novos) })
                .await;
            let lido = cx
                .background_spawn(async move { servicos.ler_produtos(&config) })
                .await;
            this.update(cx, |this, cx| {
                if let Ok(n) = r {
                    this.produtos_guardados = Some(n);
                }
                this.cadastro = lido.cadastro;
                this.aviso_cadastro = lido.aviso;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    // ---------------------------------------------------------------- senha

    fn pedir_senha(
        &mut self,
        acao: Acao,
        erro: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let desligado = self.servicos.cofre_desligado();
        let dialogo = cx.new(|cx| DialogoSenha::new(desligado, erro, window, cx));
        self.sub_senha = Some(cx.subscribe_in(
            &dialogo,
            window,
            |this, dialogo, evento: &EventoSenha, window, cx| {
                this.sub_senha = None;
                let pedida = this.senha_pedida.take();
                if let (EventoSenha::Usada, Some((_, acao))) = (evento, pedida) {
                    let senha = dialogo.update(cx, |d, _| d.tomar());
                    this.executar(acao, senha, window, cx);
                }
            },
        ));
        self.senha_pedida = Some((dialogo.clone(), acao));
        DialogoSenha::abrir(&dialogo, window, cx);
    }

    // ---------------------------------------------------------------- depois do desfecho

    /// Depois de uma nota autorizada ou em contingencia: o assistente limpo para a proxima.
    pub(super) fn nova_nfe(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let pode = matches!(&self.fase, Fase::Desfecho(d) if !d.deixa_pendente());
        if pode {
            self.limpar(window, cx);
        }
    }

    /// Copia a chave de acesso da nota do desfecho.
    pub(super) fn copiar_chave(&mut self, cx: &mut Context<Self>) {
        if let Fase::Desfecho(d) = &self.fase {
            if let Some(chave) = chave_do(d) {
                cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(chave));
                self.copiada = true;
                cx.notify();
            }
        }
    }

    /// Abre a lista de notas (para ver a nota, abrir o DANFE ou retomar).
    pub(super) fn ver_na_lista(&mut self, cx: &mut Context<Self>) {
        cx.emit(EventoNovaNfe::IrParaNotas);
    }

    /// Leva as Configuracoes, na secao da NF-e.
    pub(super) fn ir_para_configuracoes(&mut self, cx: &mut Context<Self>) {
        cx.emit(EventoNovaNfe::IrParaConfiguracoes);
    }

    /// Fecha o aviso de que a SEFAZ recusou (ou de que nada foi enviado).
    pub(super) fn fechar_rejeicao(&mut self, cx: &mut Context<Self>) {
        self.rejeicao = None;
        cx.notify();
    }
}

/// A chave da nota de um desfecho que tem nota na pasta.
pub(super) fn chave_do(d: &DesfechoNfeEmissao) -> Option<String> {
    match d {
        DesfechoNfeEmissao::Autorizada { chave, .. }
        | DesfechoNfeEmissao::SemResposta { chave, .. }
        | DesfechoNfeEmissao::SefazFora { chave, .. }
        | DesfechoNfeEmissao::EpecRegistrado { chave, .. } => Some(chave.clone()),
        _ => None,
    }
}

/// O que a retomada fez com a nota deste assistente: autorizada (le o protocolo da pasta), recusada (saiu da pasta) ou
/// ainda sem resposta (`None`: o desfecho continua como estava).
fn depois_da_retomada(
    servicos: &crate::servicos::Servicos,
    config: &rustnf_app::config::Config,
    chave: Option<String>,
    resumo: &ResumoDaRetomada,
) -> Option<DesfechoNfeEmissao> {
    use rustnf_app::notas::nfe::Situacao;
    let chave = chave?;
    match servicos.ler_nfe(config, &chave) {
        Ok(n) if n.registro.situacao == Situacao::Autorizada => Some(DesfechoNfeEmissao::Autorizada {
            chave,
            numero: n.registro.numero.clone(),
            serie: n.registro.serie.clone(),
            protocolo: n.registro.protocolo.clone(),
            guardada: true,
            recuperada: true,
            avisos: Vec::new(),
        }),
        Ok(n) if matches!(n.registro.situacao, Situacao::Denegada) => {
            Some(DesfechoNfeEmissao::Denegada {
                motivo: "A SEFAZ negou o uso desta nota.".into(),
            })
        }
        Ok(_) => None,
        // saiu da pasta: a SEFAZ recusou e o numero voltou a ficar livre
        Err(ErroTela::NotaAusente) => Some(DesfechoNfeEmissao::Rejeitada {
            cstat: String::new(),
            motivo: resumo
                .linhas
                .first()
                .map(|l| crate::modelo::mascara::frase(&l.texto))
                .unwrap_or_default(),
            o_que_fazer: "A nota saiu da lista e o número voltou a ficar livre. Corrija o que a SEFAZ apontou e envie de novo.".into(),
            passo: PassoNfe::Conferir,
            numero: String::new(),
        }),
        Err(_) => None,
    }
}
