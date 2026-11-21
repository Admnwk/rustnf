// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O arquivo de configuracao (TOML), comum a linha de comando e a qualquer interface.
//!
//! * [`Config`] -- a forma do arquivo, com padrao em tudo e campo desconhecido recusado;
//! * [`Config::carregar`] e [`Config::salvar`] -- achar, ler e gravar de forma atomica;
//! * [`Config::validar`] -- erro tipado com a secao e o campo;
//! * [`Efetiva`] -- o arquivo com as variaveis de ambiente `RUSTNF_*` por cima; a opcao da chamada vence tudo;
//! * [`aplicar_emitente`] -- completa o pedido da DPS com o `[emitente]`.
//!
//! Nao existe campo de senha nem de token: segredo nunca fica no arquivo (veja [`crate::cofre`]).

mod arquivo;
mod efetiva;
mod emitente;
mod erro;
mod modelo;
mod validar;

use std::collections::BTreeMap;

use rustnf::reforma::aliquotas::{Publicada, Publicadas};

pub use arquivo::{caminho_do_arquivo, caminho_padrao, LerAmbiente, VARIAVEL_CONFIG};
pub use efetiva::{Efetiva, VARIAVEIS};
pub use emitente::aplicar_emitente;
pub use erro::{ErroConfig, Indicado};
pub use modelo::{Aliquota, Config, Danfse, Emitente, Notas, Sefin, Servidor};

fn opcional(s: &str) -> Option<String> {
    let t = s.trim();
    (!t.is_empty()).then(|| t.to_string())
}

impl Config {
    /// As `[[aliquotas]]` no formato da previa do IBS/CBS.
    #[must_use]
    pub fn publicadas(&self) -> Publicadas {
        let mut mapa = BTreeMap::new();
        for a in &self.aliquotas {
            mapa.insert(
                a.ano,
                Publicada {
                    cbs: opcional(&a.cbs),
                    cbs_referencia: opcional(&a.cbs_referencia),
                    ibs_uf: opcional(&a.ibs_uf),
                    ibs_mun: opcional(&a.ibs_mun),
                    fonte: opcional(&a.fonte),
                    trocas: Vec::new(),
                },
            );
        }
        mapa
    }
}
