// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Os pedidos do servico `RecepcaoEvento` (`envEvento` 1.00): cancelamento, carta de correcao, EPEC, os eventos da
//! apuracao do IBS e da CBS e a manifestacao do destinatario (210200, 210210, 210220 e 210240). Parte pura: nada aqui
//! faz rede.
//!
//! Cada funcao devolve um [`Montado`] (o `<evento>` sem assinatura, comparavel byte a byte) e [`Montado::assinar`] o
//! assina com o A1 e o embrulha no `envEvento` ([`PedidoEvento`]). Fonte: MOC 7.00, Visao Geral, 5.8 a 5.12, e a NT
//! 2025.002-RTC v1.53, item 8; os schemas estao em `docs/FONTES.md`.

mod cancelamento;
pub(crate) mod comum;
mod epec;
mod manifestacao;
mod rtc;

pub use cancelamento::{
    cancelamento, cancelamento_nfce, carta_correcao, Cancelamento, CartaCorrecao, CANCELAMENTO,
    CARTA_CORRECAO, COND_USO, MAX_SEQ_CARTA,
};
pub use comum::{Montado, PedidoEvento, VERSAO_EVENTO};
pub use epec::{campos_epec, epec, CamposEpec, Epec, EPEC, ORGAO_AMBIENTE_NACIONAL};
pub use manifestacao::{
    conferir_combinacao, manifestacao, Manifestacao, TipoManifestacao, CIENCIA, CONFIRMACAO,
    DESCONHECIMENTO, NAO_REALIZADA,
};
pub use rtc::{
    evento_rtc, tipo_rtc, EventoRtc, ItemRtc, TipoRtc, CANCELAMENTO_DE_EVENTO, CANCELAVEIS, EVENTOS,
};
