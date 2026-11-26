// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Varredura das notas "no limbo": enviadas sem resultado (resposta perdida) ou com o lote ainda em processamento.
//!
//! Quem guarda a lista e' a aplicacao. Aqui estao so' as regras e a funcao [`varrer`], que recebe a lista, o instante
//! e a politica, consulta as notas que ja podem ser consultadas e devolve, para cada uma, o estado novo e um aviso se
//! for o caso. Nota que a SEFAZ diz que nao consta e cuja NF-e assinada foi guardada e' reenviada (a MESMA, entao
//! nunca vira nota gemea); sem a NF-e guardada, so' um aviso: nada se reenvia no escuro.
//!
//! Instante = segundos Unix.

use async_trait::async_trait;

use super::cliente::ClienteSefaz;
use super::erro::ErroSefaz;
use super::resultado::{Desfecho, Resultado};
use super::transporte::Transporte;

/// Espera crescente entre as consultas e idade do aviso. Os numeros padrao sao escolha desta biblioteca, nao norma.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Politica {
    /// Segundos depois do envio para a primeira consulta.
    pub primeira_apos: i64,
    /// Segundos entre a primeira e a segunda consulta.
    pub espera_inicial: i64,
    /// Quanto a espera cresce a cada consulta (1 = nao cresce).
    pub fator: i64,
    /// Teto da espera, em segundos.
    pub espera_max: i64,
    /// Depois de quantos segundos sem confirmacao a nota gera aviso.
    pub aviso_apos: i64,
    /// Quantas notas uma rodada consulta, no maximo (a SEFAZ limita o consumo: rejeicao 656).
    pub max_por_rodada: usize,
}

impl Default for Politica {
    fn default() -> Self {
        Politica {
            primeira_apos: 60,
            espera_inicial: 120,
            fator: 2,
            espera_max: 3600,
            aviso_apos: 1800,
            max_por_rodada: 20,
        }
    }
}

impl Politica {
    /// Confere a politica.
    ///
    /// # Errors
    ///
    /// Texto com o que esta fora da regra: tempos e limite maiores que zero, fator de 1 para cima e teto que nao e'
    /// menor que a espera inicial.
    pub fn validar(&self) -> Result<(), String> {
        if self.primeira_apos <= 0
            || self.espera_inicial <= 0
            || self.espera_max <= 0
            || self.aviso_apos <= 0
            || self.max_por_rodada == 0
        {
            return Err("politica: tempos e max_por_rodada precisam ser maiores que zero".into());
        }
        if self.fator < 1 || self.espera_max < self.espera_inicial {
            return Err("politica: fator >= 1 e espera_max >= espera_inicial".into());
        }
        Ok(())
    }
}

/// Em que pe' a nota esta, do ponto de vista de quem guarda a lista.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Estado {
    /// Enviada, sem resposta (ou com o servico parado): precisa de consulta.
    Pendente,
    /// O lote assincrono segue em processamento na SEFAZ.
    Processando,
    /// Autorizada (final).
    Autorizada,
    /// Uso denegado (final).
    Denegada,
    /// Cancelada (final).
    Cancelada,
    /// Rejeitada, ou numero usado por outra chave (final: esta nota nao vale).
    Rejeitada,
    /// A SEFAZ diz que nao consta e a NF-e assinada nao foi guardada: a pessoa confere antes de emitir.
    SemXml,
}

impl Estado {
    /// A nota esta no limbo (a varredura cuida dela).
    #[must_use]
    pub fn no_limbo(self) -> bool {
        matches!(self, Estado::Pendente | Estado::Processando)
    }
}

/// Uma nota que a aplicacao guardou.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotaLocal {
    /// A chave da NF-e.
    pub chave: String,
    /// O estado guardado.
    pub estado: Estado,
    /// Quando foi enviada.
    pub desde: i64,
    /// Quantas consultas ja foram feitas.
    pub tentativas: i64,
    /// Quando foi a ultima.
    pub ultima: Option<i64>,
    /// A NF-e assinada: monta o `nfeProc` na consulta e e' o que se reenvia. Vazia = nao guardada.
    pub nfe_assinada: String,
}

/// O que a varredura fez com uma nota.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Acao {
    /// Consultou a chave.
    Consultou,
    /// A SEFAZ disse que nao consta e a MESMA NF-e foi reenviada.
    Reenviou,
    /// Nao foi a rede (erro de uso ou nota sem o que a varredura precisa).
    Nada,
}

/// O desfecho da varredura para uma nota.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Passo {
    /// A chave.
    pub chave: String,
    /// O que se fez.
    pub acao: Acao,
    /// O estado novo, para a aplicacao guardar.
    pub estado: Estado,
    /// Quantas consultas, contando esta.
    pub tentativas: i64,
    /// O instante desta rodada.
    pub ultima: i64,
    /// Aviso para a pessoa, se houver.
    pub aviso: Option<String>,
    /// O resultado do cliente (com o `nfeProc` a guardar); `None` se nao foi a rede.
    pub resultado: Option<Resultado>,
}

/// Segundos entre a consulta `n` (de 1 em diante) e a seguinte; menos de 1 = `primeira_apos`.
#[must_use]
pub fn espera(tentativas: i64, pol: &Politica) -> i64 {
    if tentativas < 1 {
        return pol.primeira_apos;
    }
    // sem crescimento a espera e' a inicial para sempre (e o laco de baixo nao pode rodar `tentativas` vezes)
    if pol.fator <= 1 {
        return pol.espera_inicial.min(pol.espera_max);
    }
    let mut s = pol.espera_inicial;
    for _ in 1..tentativas {
        s = s.saturating_mul(pol.fator);
        if s >= pol.espera_max {
            return pol.espera_max;
        }
    }
    s.min(pol.espera_max)
}

/// O instante da proxima consulta da nota.
#[must_use]
pub fn proxima(n: &NotaLocal, pol: &Politica) -> i64 {
    match n.ultima {
        Some(u) if n.tentativas >= 1 => u.saturating_add(espera(n.tentativas, pol)),
        _ => n.desde.saturating_add(pol.primeira_apos),
    }
}

/// As notas do limbo com a proxima consulta vencida, a mais atrasada primeiro (empate: a mais antiga, depois a chave),
/// no maximo `max_por_rodada`.
#[must_use]
pub fn devidas<'a>(notas: &'a [NotaLocal], agora: i64, pol: &Politica) -> Vec<&'a NotaLocal> {
    let mut v: Vec<&NotaLocal> = notas
        .iter()
        .filter(|n| n.estado.no_limbo() && proxima(n, pol) <= agora)
        .collect();
    v.sort_by(|a, b| {
        (proxima(a, pol), a.desde, &a.chave).cmp(&(proxima(b, pol), b.desde, &b.chave))
    });
    v.truncate(pol.max_por_rodada);
    v
}

/// O estado novo da nota depois do desfecho que o cliente deu.
#[must_use]
pub fn decidir(estado: Estado, desfecho: &Desfecho, tem_nfe: bool) -> Estado {
    match desfecho {
        Desfecho::Autorizada { .. } => Estado::Autorizada,
        Desfecho::Denegada { .. } => Estado::Denegada,
        Desfecho::Cancelada { .. } => Estado::Cancelada,
        Desfecho::Rejeitada { .. } | Desfecho::NumeroJaUsado { .. } => Estado::Rejeitada,
        Desfecho::EmProcessamento { .. } => Estado::Processando,
        Desfecho::NaoRecebida if tem_nfe => Estado::Pendente,
        Desfecho::NaoRecebida => Estado::SemXml,
        Desfecho::SoEpec { .. } | Desfecho::Indisponivel { .. } | Desfecho::SemResposta => estado,
    }
}

/// O aviso para a pessoa: nota sem a NF-e guardada, ou no limbo ha mais de `aviso_apos` segundos.
#[must_use]
pub fn aviso(n: &NotaLocal, agora: i64, pol: &Politica, estado: Estado) -> Option<String> {
    if estado == Estado::SemXml {
        return Some(
            "A SEFAZ diz que a nota nao consta e a NF-e assinada nao foi guardada: confira no fiscal antes de emitir."
                .into(),
        );
    }
    if estado.no_limbo() && agora.saturating_sub(n.desde) >= pol.aviso_apos {
        return Some(format!(
            "Nota {} ha {} min sem confirmacao da SEFAZ.",
            n.chave,
            agora.saturating_sub(n.desde) / 60
        ));
    }
    None
}

/// A porta da varredura: consultar pela chave e reenviar a MESMA NF-e (sincrono). O [`ClienteSefaz`] a implementa; o
/// teste usa um falso.
#[async_trait]
pub trait Sefaz: Send + Sync {
    /// Consulta a chave (com a NF-e, a nota autorizada volta com o `nfeProc`).
    async fn consultar(
        &self,
        chave: &str,
        nfe_assinada: Option<&str>,
    ) -> Result<Resultado, ErroSefaz>;
    /// Reenvia a NF-e assinada em lote sincrono.
    async fn autorizar(&self, nfe_assinada: &str) -> Result<Resultado, ErroSefaz>;
}

#[async_trait]
impl<T: Transporte> Sefaz for ClienteSefaz<T> {
    async fn consultar(
        &self,
        chave: &str,
        nfe_assinada: Option<&str>,
    ) -> Result<Resultado, ErroSefaz> {
        ClienteSefaz::consultar(self, chave, nfe_assinada).await
    }

    async fn autorizar(&self, nfe_assinada: &str) -> Result<Resultado, ErroSefaz> {
        ClienteSefaz::autorizar(self, nfe_assinada).await
    }
}

/// Uma rodada: consulta as notas [`devidas`]; a que a SEFAZ diz que nao consta e tem a NF-e guardada e' reenviada.
/// Erro de uso (`ErroSefaz`) vira passo `Nada` com aviso e a rodada segue.
pub async fn varrer<S: Sefaz>(
    cliente: &S,
    notas: &[NotaLocal],
    agora: i64,
    pol: &Politica,
) -> Vec<Passo> {
    let mut out = Vec::new();
    for n in devidas(notas, agora, pol) {
        let t = n.tentativas.saturating_add(1);
        let nfe = Some(n.nfe_assinada.as_str()).filter(|s| !s.is_empty());
        let consulta = cliente.consultar(&n.chave, nfe).await;
        let feito = match (consulta, nfe) {
            (Ok(r), Some(x)) if r.desfecho == Desfecho::NaoRecebida => {
                cliente.autorizar(x).await.map(|r| (Acao::Reenviou, r))
            }
            (c, _) => c.map(|r| (Acao::Consultou, r)),
        };
        let (acao, r) = match feito {
            Ok(x) => x,
            Err(e) => {
                out.push(Passo {
                    chave: n.chave.clone(),
                    acao: Acao::Nada,
                    estado: n.estado,
                    tentativas: t,
                    ultima: agora,
                    aviso: Some(format!("Nota {}: {e}", n.chave)),
                    resultado: None,
                });
                continue;
            }
        };
        let est = decidir(n.estado, &r.desfecho, nfe.is_some());
        let mut av = aviso(n, agora, pol, est);
        if av.is_none() {
            if let Desfecho::Rejeitada { cstat, motivo } = &r.desfecho {
                av = Some(format!("Nota {}: recusada ({cstat}): {motivo}", n.chave));
            }
        }
        out.push(Passo {
            chave: n.chave.clone(),
            acao,
            estado: est,
            tentativas: t,
            ultima: agora,
            aviso: av,
            resultado: Some(r),
        });
    }
    out
}
