// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A distribuicao de DF-e (`NFeDistribuicaoDFe`, Ambiente Nacional): baixar as notas e os eventos em que um CNPJ ou
//! CPF e' interessado (destinatario, transportador, terceiro do `autXML`), com a sequencia de NSU que a NT exige.
//!
//! * [`pedido`]: o `distDFeInt` (`distNSU`, `consNSU`, `consChNFe`), puro;
//! * [`Cursor`], [`Nsu`]: o estado que a aplicacao PERSISTE (ultimo NSU, espera de uma hora, lacunas);
//! * [`Doc`], [`Conteudo`]: cada `docZip` aberto (gzip com limite de tamanho) e lido (`resNFe`, `resEvento`, `procNFe`,
//!   `procEventoNFe`); o que nao e' reconhecido ou nao abre volta cru, nunca descartado;
//! * [`ResultadoDfe`], [`DesfechoDfe`]: o desfecho em palavras;
//! * `ClienteSefaz::distribuir`, `distribuir_nsu`, `distribuir_chave` e `sincronizar`: a chamada, sobre o mesmo
//!   [`Transporte`](super::Transporte) do resto do cliente.
//!
//! # A regra que o cliente nao quebra
//!
//! O cursor so' anda para o `ultNSU` que o Ambiente Nacional devolveu; nunca pula um NSU (o que falta no meio do lote
//! vira lacuna e se busca por `consNSU`), nunca entrega o mesmo NSU duas vezes e nunca volta. Depois do 137 e do
//! `ultNSU` igual ao `maxNSU` o cliente nao chama a distribuicao por uma hora; depois do 656 nao chama nada por uma
//! hora. A biblioteca decide o proximo NSU e quando esperar; a aplicacao grava, a cada lote, os documentos e o cursor
//! novo na mesma transacao.
//!
//! # Fontes
//!
//! NT 2014.002 v1.02d (`NFeDistribuicaoDFe`): itens 3.1 e 3.2 (leiautes), 3.3 (gzip), 3.5 a 3.7 (as tres consultas, lote
//! de 50, espera de uma hora, `consNSU` para lacuna), 3.10.2 (regras), 3.11 (resumos e uso indevido, 656) e 4 (codigos);
//! esquemas `distDFeInt`, `retDistDFeInt`, `resNFe` e `resEvento` v1.01 do pacote de liberacao da distribuicao v1.04.
//! A lista completa esta em `docs/FONTES.md`.

mod cliente;
mod cursor;
mod documento;
pub mod pedido;
mod resultado;
mod retorno;

pub use cliente::{ErroSincronizacao, LACUNAS_POR_RODADA, LACUNA_MAXIMA};
pub use cursor::{Cursor, Espera, FaixaNsu, MotivoEspera, Nsu, ESPERA};
pub use documento::{Conteudo, Doc, ResumoEvento, ResumoNfe, SituacaoResumo};
pub use resultado::{DesfechoDfe, ResultadoDfe};
