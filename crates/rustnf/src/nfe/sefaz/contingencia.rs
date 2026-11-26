// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O que fazer com a NFC-e emitida em contingencia off-line (`tpEmis` 9) depois que o cliente tentou transmiti-la.
//!
//! Regra do MOC 7.00, Anexo IV, item 3: na transmissao a nota segue com a MESMA chave de acesso, inclusive o mesmo
//! `cNF`; a numeracao nova e' recomendada so' na entrada em contingencia, para nao colidir com um envio normal que
//! deixou o numero na base.

use super::resultado::{Desfecho, Resultado};

/// O passo seguinte da NFC-e emitida off-line, pelo resultado da transmissao.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PassoOffline {
    /// Autorizada (100): a contingencia acabou para esta nota.
    Concluida,
    /// Autorizada fora de prazo (150): vale, mas passou do primeiro dia util depois da emissao.
    ConcluidaForaDoPrazo,
    /// Rejeitada: corrigir o XML e transmitir de novo com a MESMA chave e o mesmo `cNF`, assinando outra vez.
    CorrigirEReenviar {
        /// O `cStat` da rejeicao.
        cstat: String,
        /// O motivo que a SEFAZ deu.
        motivo: String,
    },
    /// O numero ja existe na base: nao reenvie a mesma nota; a consulta pela chave diz qual nota ocupa o numero.
    NumeroJaUsado,
    /// Sem resposta, em processamento ou servico fora do ar: transmitir de novo a MESMA nota (a consulta pela chave
    /// vem antes de qualquer reenvio), dentro do prazo.
    TransmitirDeNovo,
    /// Cancelada ou denegada: nada a transmitir.
    Encerrada,
}

/// O passo seguinte da nota emitida em contingencia off-line, pelo resultado da transmissao.
#[must_use]
pub fn passo_offline(r: &Resultado) -> PassoOffline {
    match &r.desfecho {
        Desfecho::Autorizada { cstat, .. } if cstat == "150" => PassoOffline::ConcluidaForaDoPrazo,
        Desfecho::Autorizada { .. } => PassoOffline::Concluida,
        Desfecho::Rejeitada { cstat, motivo } => PassoOffline::CorrigirEReenviar {
            cstat: cstat.clone(),
            motivo: motivo.clone(),
        },
        Desfecho::NumeroJaUsado { .. } => PassoOffline::NumeroJaUsado,
        Desfecho::Cancelada { .. } | Desfecho::Denegada { .. } => PassoOffline::Encerrada,
        _ => PassoOffline::TransmitirDeNovo,
    }
}
