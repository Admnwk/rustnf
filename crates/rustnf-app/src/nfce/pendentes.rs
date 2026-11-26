// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! As NFC-e que ainda dependem da SEFAZ ("ao fechar o caixa"): as emitidas em contingencia off-line, que TEM prazo para ser
//! transmitidas, e as enviadas sem resposta, que podem ja ter sido autorizadas. Tudo em palavras para quem nao e' do ramo,
//! com o que fazer e ate quando.
//!
//! O prazo da off-line e' o do MOC 7.00, Anexo IV, itens 2 e 3: o fim do primeiro dia util depois da emissao (a biblioteca
//! o calcula, so' sabado e domingo contam como dia nao util). Depois dele a SEFAZ ainda autoriza (cStat 150, autorizada
//! fora de prazo, NT 2026.002 v1.11, regra B09-40, excecao 2), mas o emitente fica em falta.

use chrono::{DateTime, FixedOffset, Offset, Utc};
use serde_json::{json, Value};

use crate::erro::Erro;
use crate::notas::nfe::{NotasNfe, Registro};

/// Menos que isto para o prazo e a nota entra em destaque.
const A_VENCER_S: i64 = 3600;

/// Onde a nota esta em relacao ao prazo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Prazo {
    /// Sem prazo de lei (a nota foi enviada e a resposta se perdeu).
    Sem,
    /// Falta mais de uma hora (segundos que faltam).
    Folga(i64),
    /// Falta menos de uma hora (segundos que faltam).
    AVencer(i64),
    /// Ja passou (segundos desde o fim do prazo).
    Vencido(i64),
}

/// Uma NFC-e que ainda precisa da SEFAZ.
#[derive(Debug, Clone)]
pub struct Pendente {
    /// A linha da pasta.
    pub registro: Registro,
    /// O prazo.
    pub prazo: Prazo,
    /// O fim do prazo por extenso (`dd/mm/aaaa hh:mm`, no fuso da emissao); vazio sem prazo.
    pub limite: String,
}

fn duracao(s: i64) -> String {
    let m = s.max(0) / 60;
    match (m / 1440, (m / 60) % 24, m % 60) {
        (0, 0, m) => format!("{m} min"),
        (0, h, m) => format!("{h} h {m:02} min"),
        (d, h, _) => format!("{d} dia(s) e {h} h"),
    }
}

impl Pendente {
    fn novo(r: Registro, agora: DateTime<Utc>) -> Self {
        let (prazo, limite) = match r.prazo.filter(|_| r.offline) {
            None => (Prazo::Sem, String::new()),
            Some(fim) => {
                let resta = fim - agora.timestamp();
                let p = if resta < 0 {
                    Prazo::Vencido(-resta)
                } else if resta < A_VENCER_S {
                    Prazo::AVencer(resta)
                } else {
                    Prazo::Folga(resta)
                };
                (p, limite_por_extenso(&r, fim))
            }
        };
        Self {
            registro: r,
            prazo,
            limite,
        }
    }

    /// Prazo vencido ou a vencer em menos de uma hora, ou nota que a SEFAZ recusou.
    #[must_use]
    pub fn destaque(&self) -> bool {
        matches!(self.prazo, Prazo::AVencer(_) | Prazo::Vencido(_))
            || !self.registro.recusa.is_empty()
    }

    /// A frase para a pessoa: o que e' a nota, em que pe esta e o que fazer.
    #[must_use]
    pub fn frase(&self) -> String {
        let r = &self.registro;
        let nota = format!("NFC-e {} (serie {})", r.numero, r.serie);
        let acao = "rode `rustnf nfce pendentes --retomar` com internet";
        if !r.recusa.is_empty() {
            return format!(
                "ATENCAO: a SEFAZ RECUSOU a {nota} emitida sem internet ({}). Ela segue pendente: corrija o que a SEFAZ \
                 apontou e transmita de novo com a MESMA chave ({}); o numero e o codigo (cNF) nao mudam.",
                r.recusa, r.chave
            );
        }
        let prazo = match self.prazo {
            Prazo::Sem => format!(
                "foi enviada e a SEFAZ nao respondeu: ela PODE ter sido autorizada. Nao emita outra no lugar; {acao}."
            ),
            Prazo::Folga(s) => format!(
                "foi emitida SEM INTERNET (contingencia off-line) e ainda nao foi enviada a SEFAZ. Faltam {} para o prazo \
                 (ate {}): {acao}.",
                duracao(s),
                self.limite
            ),
            Prazo::AVencer(s) => format!(
                "foi emitida SEM INTERNET e o prazo para envia-la acaba em {} (ate {}): {acao} AGORA.",
                duracao(s),
                self.limite
            ),
            Prazo::Vencido(s) => format!(
                "foi emitida SEM INTERNET e o prazo para envia-la VENCEU ha {} (era {}). Envie assim mesmo: a SEFAZ costuma \
                 autorizar fora do prazo (situacao 150), mas a falta fica registrada. {acao}.",
                duracao(s),
                self.limite
            ),
        };
        format!("{nota} {prazo}")
    }

    /// O JSON: a nota (chave, serie, numero, valor), `offline`, `prazo` (`sem`, `no_prazo`, `a_vencer`, `vencido`),
    /// `limite`, `segundos` (que faltam; negativo = vencido), `recusa`, `destaque` e a `frase`.
    #[must_use]
    pub fn para_json(&self) -> Value {
        let r = &self.registro;
        let (nome, segundos) = match self.prazo {
            Prazo::Sem => ("sem", None),
            Prazo::Folga(s) => ("no_prazo", Some(s)),
            Prazo::AVencer(s) => ("a_vencer", Some(s)),
            Prazo::Vencido(s) => ("vencido", Some(-s)),
        };
        json!({
            "chave": r.chave,
            "serie": r.serie,
            "numero": r.numero,
            "valor": r.valor,
            "emissao": r.emissao,
            "offline": r.offline,
            "prazo": nome,
            "limite": self.limite,
            "segundos": segundos,
            "recusa": r.recusa,
            "destaque": self.destaque(),
            "frase": self.frase(),
        })
    }
}

pub(super) fn limite_por_extenso(r: &Registro, fim: i64) -> String {
    let brasilia = FixedOffset::west_opt(3 * 3600).unwrap_or_else(|| Utc.fix());
    let fuso = DateTime::parse_from_rfc3339(&r.emissao).map_or(brasilia, |d| *d.offset());
    DateTime::from_timestamp(fim, 0)
        .map(|d| d.with_timezone(&fuso).format("%d/%m/%Y %H:%M").to_string())
        .unwrap_or_default()
}

/// As NFC-e pendentes da pasta, as de prazo mais curto primeiro.
///
/// # Errors
///
/// [`Erro`] se a pasta nao le.
pub fn listar(notas: &NotasNfe, agora: DateTime<Utc>) -> Result<Vec<Pendente>, Erro> {
    let mut v: Vec<Pendente> = notas
        .a_resolver()?
        .into_iter()
        .map(|r| Pendente::novo(r, agora))
        .collect();
    v.sort_by_key(|p| (p.registro.prazo.unwrap_or(i64::MAX), p.registro.desde));
    Ok(v)
}

/// O aviso "ao fechar o caixa": `None` sem pendente; senao o que fazer e ate quando (o prazo mais curto das off-line).
#[must_use]
pub fn aviso_de_fechamento(lista: &[Pendente]) -> Option<String> {
    if lista.is_empty() {
        return None;
    }
    let mais_curto = lista
        .iter()
        .find(|p| p.registro.offline && !p.limite.is_empty());
    let quando = match mais_curto {
        Some(p) if matches!(p.prazo, Prazo::Vencido(_)) => {
            " (o prazo da mais antiga JA VENCEU: faca isso agora)".to_string()
        }
        Some(p) => format!(" antes de {}", p.limite),
        None => String::new(),
    };
    Some(format!(
        "{} NFC-e ainda dependem da SEFAZ: rode `rustnf nfce pendentes --retomar` com internet{quando}",
        lista.len()
    ))
}

/// O erro de quem fecha o caixa com pendente (saida 4): a mesma frase do aviso. `None` sem pendente.
#[must_use]
pub fn erro_de_fechamento(lista: &[Pendente]) -> Option<Erro> {
    aviso_de_fechamento(lista).map(Erro::Rede)
}

/// Como [`listar`], com o relogio do sistema.
///
/// # Errors
///
/// [`Erro`] se a pasta nao le.
pub fn listar_agora(notas: &NotasNfe) -> Result<Vec<Pendente>, Erro> {
    listar(notas, Utc::now())
}
