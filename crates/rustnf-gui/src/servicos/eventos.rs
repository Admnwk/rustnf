// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O que as telas pedem a Sefin alem de emitir, cancelar e consultar pela chave: conferir a situacao das notas em
//! lote, ler o historico de eventos de uma nota, registrar os eventos que o contribuinte pede (analise fiscal,
//! confirmar, recusar), conferir se um deles foi registrado e consultar pela DPS. Tudo fala com a rede (e com o disco,
//! na pasta de notas): quem chama roda fora da linha da tela, com o emissor da fabrica dos `Servicos`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use rustnf::nfse::eventos::TIPOS;
use rustnf::nfse::leitura::ler_evento;
use rustnf::nfse::sefin::Status;
use rustnf_app::atualizacao::atualizar_lista;
use rustnf_app::config::Config;
use rustnf_app::emissao::consultar_completa;
use rustnf_app::eventos::{marcar_analise_fiscal, registrar_evento_completo};
use rustnf_app::notas::{Notas, Registro};
use tokio::sync::mpsc::UnboundedSender;

use super::{FabricaDeEmissor, Preparo, Servicos};
use crate::modelo::cancelamento::DesfechoDoCancelamento;
use crate::modelo::conferir::{Andamento, DesfechoDoLote, ResultadoDoLote};
use crate::modelo::consulta::DesfechoDaConsulta;
use crate::modelo::consulta_dps::{conferir_dps, EstadoDaDps};
use crate::modelo::evento::TipoDeEvento;
use crate::modelo::historico::{
    ordenar, DesfechoDoHistorico, ItemDoHistorico, TEXTO_SEM_RESPOSTA as SEM_RESPOSTA_DO_HISTORICO,
};
use crate::modelo::mascara::frase;

/// A pessoa apertou "Parar": o lote para antes da proxima nota.
pub type ParadaDoLote = Arc<AtomicBool>;

/// Quantas sequencias de um mesmo tipo de evento se leem, no maximo (a mesma conta da conferencia em lote).
const SEQUENCIAS_MAXIMAS: u32 = 20;

impl Servicos {
    /// O intervalo entre duas consultas a Sefin num lote ou no historico (padrao: 200 ms).
    #[must_use]
    pub fn intervalo(&self) -> Duration {
        self.intervalo
    }

    /// Troca o intervalo entre as consultas (os testes e a demonstracao usam zero).
    #[must_use]
    pub fn com_intervalo(mut self, intervalo: Duration) -> Self {
        self.intervalo = intervalo;
        self
    }
}

fn recusado(texto: impl Into<String>) -> DesfechoDoCancelamento {
    DesfechoDoCancelamento::Recusado {
        texto: texto.into(),
    }
}

/// Confere na Sefin, uma nota e uma consulta por vez, a situacao das notas de `registros` (as que a lista mostra) e
/// grava na pasta de notas o que mudou. `parada` interrompe antes da proxima nota; `andamento` recebe onde o lote esta, a cada nota. Roda
/// fora da linha da tela (`fora::rede`).
pub async fn conferir_lote(
    fabrica: Arc<dyn FabricaDeEmissor>,
    preparo: Preparo,
    config: Config,
    registros: Vec<Registro>,
    intervalo: Duration,
    parada: ParadaDoLote,
    andamento: UnboundedSender<Andamento>,
) -> DesfechoDoLote {
    let emissor = match fabrica.criar(preparo) {
        Ok(e) => e,
        Err(e) => return DesfechoDoLote::Recusado { texto: e.texto() },
    };
    let notas = match Notas::abrir(config.notas.pasta.trim()) {
        Ok(n) => n,
        Err(_) => {
            return DesfechoDoLote::Recusado {
                texto: super::ErroTela::Pasta.texto(),
            }
        }
    };
    let total = registros.len();
    let interrompido = Arc::clone(&parada);
    let mut acompanhar = move |feitas: usize, total: usize| {
        // a tela pode ter fechado o outro lado: o lote segue mesmo assim
        let _ = andamento.send(Andamento { feitas, total });
        !interrompido.load(Ordering::SeqCst)
    };
    let relatorio =
        atualizar_lista(&*emissor, &notas, &registros, intervalo, &mut acompanhar).await;
    let parado = relatorio.conferidas < total && parada.load(Ordering::SeqCst);
    DesfechoDoLote::Feito(Box::new(ResultadoDoLote::de(
        &relatorio, parado, &registros,
    )))
}

/// Le os eventos da nota na Sefin (todos os tipos que a biblioteca conhece, cada um ate a primeira sequencia que nao
/// existe), uma consulta por vez, com `intervalo` entre elas. Se a Sefin deixar de responder no meio, devolve o que
/// ja achou e diz que ficou incompleto.
pub async fn historico_da_nota(
    fabrica: Arc<dyn FabricaDeEmissor>,
    preparo: Preparo,
    chave: String,
    intervalo: Duration,
) -> DesfechoDoHistorico {
    let emissor = match fabrica.criar(preparo) {
        Ok(e) => e,
        Err(e) => return DesfechoDoHistorico::Recusado { texto: e.texto() },
    };
    let mut itens: Vec<ItemDoHistorico> = Vec::new();
    let mut primeira = true;
    for (tipo, _, _) in TIPOS {
        for sequencia in 1..=SEQUENCIAS_MAXIMAS {
            if !primeira && !intervalo.is_zero() {
                tokio::time::sleep(intervalo).await;
            }
            primeira = false;
            let r = emissor.consultar_evento(&chave, tipo, sequencia).await;
            match r.status {
                Status::Ok => {
                    if let Ok(e) = ler_evento(&r.xml_retorno) {
                        itens.push(ItemDoHistorico::do_evento(&e, sequencia));
                    }
                }
                Status::NaoEncontrada => break,
                // recusa local da biblioteca (chave de tamanho errado): nada saiu
                Status::Erro if r.http == 0 && r.detalhe.is_empty() => {
                    return DesfechoDoHistorico::Recusado {
                        texto: frase(&r.frase),
                    };
                }
                _ => {
                    ordenar(&mut itens);
                    return DesfechoDoHistorico::Incompleto {
                        itens,
                        texto: SEM_RESPOSTA_DO_HISTORICO.into(),
                    };
                }
            }
        }
    }
    ordenar(&mut itens);
    DesfechoDoHistorico::Eventos(itens)
}

/// Registra o evento que a pessoa pediu e devolve como terminou. O cancelamento segue o fluxo dele; os outros passam
/// pela camada de aplicacao (que recusa antes de enviar o que o contribuinte nao pede e marca a pasta de notas no
/// pedido de analise fiscal registrado). Roda fora da linha da tela.
pub async fn registrar_evento_nota(
    fabrica: Arc<dyn FabricaDeEmissor>,
    preparo: Preparo,
    config: Config,
    tipo: TipoDeEvento,
    chave: String,
    motivo: String,
    justificativa: String,
) -> DesfechoDoCancelamento {
    if tipo == TipoDeEvento::Cancelar {
        return super::cancelar_nota(fabrica, preparo, config, chave, motivo, justificativa).await;
    }
    let emissor = match fabrica.criar(preparo) {
        Ok(e) => e,
        Err(e) => return recusado(e.texto()),
    };
    let notas = Notas::abrir(config.notas.pasta.trim()).ok();
    match registrar_evento_completo(
        &*emissor,
        tipo.codigo(),
        &chave,
        &motivo,
        &justificativa,
        notas.as_ref(),
    )
    .await
    {
        Ok(r) => {
            DesfechoDoCancelamento::do_resultado(&r.resultado, r.marcada, &tipo.sem_resposta())
        }
        Err(e) => recusado(frase(&e.mensagem())),
    }
}

/// Depois de um pedido sem resposta: a Sefin tem o evento? Achando o pedido de analise fiscal, a nota da pasta passa a
/// em analise fiscal. So' consulta: nunca reenvia o pedido.
pub async fn conferir_evento(
    fabrica: Arc<dyn FabricaDeEmissor>,
    preparo: Preparo,
    config: Config,
    tipo: TipoDeEvento,
    chave: String,
) -> DesfechoDoCancelamento {
    if tipo == TipoDeEvento::Cancelar {
        return super::conferir_cancelamento(fabrica, preparo, config, chave).await;
    }
    let emissor = match fabrica.criar(preparo) {
        Ok(e) => e,
        Err(e) => return recusado(e.texto()),
    };
    let r = emissor.consultar_evento(&chave, tipo.codigo(), 1).await;
    let marcada = (r.ok() && tipo == TipoDeEvento::AnaliseFiscal)
        .then(|| {
            Notas::abrir(config.notas.pasta.trim())
                .ok()
                .map(|n| marcar_analise_fiscal(&n, &chave))
        })
        .flatten();
    DesfechoDoCancelamento::da_conferencia(r.status, marcada)
}

/// Consulta a nota pela serie e pelo numero da DPS: monta o Id com o `[emitente]`, pergunta a Sefin que nota a DPS
/// gerou e consulta essa nota (com a conferencia do cancelamento). A nota devolvida so' vale se for a DPS pedida.
/// Roda fora da linha da tela.
pub async fn consultar_dps_nota(
    fabrica: Arc<dyn FabricaDeEmissor>,
    preparo: Preparo,
    config: Config,
    serie: String,
    numero: String,
) -> DesfechoDaConsulta {
    let id = match conferir_dps(&config.emitente, &serie, &numero) {
        EstadoDaDps::Valido(id) => id,
        EstadoDaDps::Invalido(texto) => return DesfechoDaConsulta::Recusada { texto },
        EstadoDaDps::Vazio => {
            return DesfechoDaConsulta::Recusada {
                texto: "Digite o número da DPS.".into(),
            }
        }
    };
    let emissor = match fabrica.criar(preparo) {
        Ok(e) => e,
        Err(e) => return DesfechoDaConsulta::Recusada { texto: e.texto() },
    };
    let da_dps = emissor.consultar_dps(&id, false).await;
    match da_dps.status {
        Status::Autorizada | Status::Ok if !da_dps.chave.is_empty() => {
            let chave = da_dps.chave.clone();
            let c = consultar_completa(&*emissor, &chave).await;
            // a nota achada pela chave tem de ser a da DPS pedida
            let outra_dps = c
                .resultado
                .nfse
                .as_ref()
                .is_some_and(|n| !n.id_dps.is_empty() && n.id_dps != id);
            if outra_dps {
                return DesfechoDaConsulta::Recusada {
                    texto: "A Sefin respondeu com uma nota de outra DPS, então ela não é mostrada. Não use esta resposta.".into(),
                };
            }
            DesfechoDaConsulta::de(c, &chave)
        }
        Status::NaoEncontrada => DesfechoDaConsulta::NaoEncontrada,
        Status::Erro if da_dps.http == 0 && da_dps.detalhe.is_empty() => {
            DesfechoDaConsulta::Recusada {
                texto: frase(&da_dps.frase),
            }
        }
        _ => DesfechoDaConsulta::SemResposta {
            texto: crate::modelo::consulta::TEXTO_SEM_RESPOSTA.into(),
        },
    }
}
