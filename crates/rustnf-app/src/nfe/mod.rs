// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A NF-e (modelo 55) na camada de aplicacao: uma funcao por acao, usada pela linha de comando agora e pela API, pelo
//! MCP e pela interface depois. Nenhuma regra fiscal mora aqui: a biblioteca (`rustnf::nfe`) monta, assina, envia e
//! entende a resposta; esta camada completa o pedido com a configuracao, numera pela pasta de notas, guarda o que
//! a biblioteca devolve e diz o resultado em palavras.
//!
//! * [`emitir`], [`epec`] e [`retomar`] -- a emissao, a emissao em contingencia EPEC e a retomada do que ficou sem
//!   resultado. A NF-e assinada e' GRAVADA como pendente antes de ir a rede: uma queda no meio nunca perde a chave, e a
//!   retomada consulta a MESMA chave (nunca gera outra nota);
//! * [`consultar`], [`status`] e [`danfe_da_pasta`] -- consulta pela chave, estado do servico e o PDF;
//! * [`cancelar`], [`corrigir`] e [`inutilizar`] -- os eventos e a inutilizacao;
//! * [`preparar`] -- o pedido completado, lido e conferido, sem rede e sem gastar numero;
//! * [`sincronizar`], [`baixar`], [`manifestar`], [`listar`] e [`cursor`] -- os documentos RECEBIDOS (distribuicao de DF-e
//!   do Ambiente Nacional) e a manifestacao do destinatario, na pasta `nfe/recebidas`;
//! * [`config_sefaz`] e [`cliente_real`] -- o cliente da SEFAZ a partir da configuracao.
//!
//! A pasta e' a [`NotasNfe`], `nfe/` dentro da `[notas].pasta`.

mod cliente;
mod consulta;
mod emissao;
mod eventos;
mod pedido;
mod recebidas;
mod resposta;
mod retomada;
#[cfg(test)]
mod testes;
#[cfg(test)]
pub(crate) mod testes_comuns;
#[cfg(test)]
pub(crate) mod testes_dfe;
#[cfg(test)]
mod testes_eventos;
#[cfg(test)]
mod testes_recebidas;
#[cfg(test)]
mod testes_retomada;

use chrono::{DateTime, Utc};
use rustnf::assinatura::cert_a1::Certificado;
use rustnf::nfe::montagem::OpcoesQr;
use rustnf::nfe::sefaz::{ClienteSefaz, Transporte};

pub use cliente::{cliente_real, config_sefaz, ler_ca_pem};
pub use consulta::{
    consultar, danfe_da_pasta, danfe_do_xml, estado_do_servico, status, Consultada,
};
pub(crate) use emissao::preparar_e_gravar;
pub use emissao::{emitir, Emitida};
pub use eventos::{cancelar, corrigir, epec, inutilizar, Inutilizada, Registrado};
pub(crate) use pedido::emissao_agora;
pub use pedido::{
    avisos_json, completar_pedido, conferencia_json, montar_pedido, preparar, Montado, Preparada,
};
pub use recebidas::{
    baixar, cursor, danfe_da_recebida, fila, listar, manifestar, sincronizar, Baixada, Contagem,
    Fila, FiltroRecebidas, Manifestada, Sincronizada, LOTES_MAXIMO, LOTES_PADRAO,
};
pub use resposta::{erro_do_evento, erro_do_resultado, json_do_resultado};
pub use retomada::{erro_da_retomada, retomar, Retomada};

use crate::erro::Erro;
use crate::notas::nfe::NotasNfe;

/// Tudo de que uma acao precisa: o cliente da SEFAZ, o certificado que assina, a configuracao, a pasta de notas e o
/// relogio (os testes fixam o relogio).
pub struct Contexto<'a, T: Transporte> {
    /// O cliente dos webservices.
    pub cliente: &'a ClienteSefaz<T>,
    /// O certificado A1 do emitente.
    pub certificado: &'a Certificado,
    /// A configuracao (`[nfe]`, `[emitente]` e o que a NFS-e compartilha).
    pub config: &'a crate::config::Config,
    /// A pasta de notas da NF-e; sem ela, so' as acoes que nao guardam nada.
    pub notas: Option<&'a NotasNfe>,
    /// Agora.
    pub agora: DateTime<Utc>,
    /// O QR Code da NFC-e (versao e CSC); o padrao (3, sem CSC) serve a NF-e, que nao leva QR Code.
    pub qr: OpcoesQr,
}

impl<'a, T: Transporte> Contexto<'a, T> {
    /// O contexto com o relogio do sistema.
    #[must_use]
    pub fn novo(
        cliente: &'a ClienteSefaz<T>,
        certificado: &'a Certificado,
        config: &'a crate::config::Config,
        notas: Option<&'a NotasNfe>,
    ) -> Self {
        Self {
            cliente,
            certificado,
            config,
            notas,
            agora: Utc::now(),
            qr: OpcoesQr::default(),
        }
    }

    /// Troca as opcoes do QR Code (a NFC-e com CSC).
    #[must_use]
    pub fn com_qr(mut self, qr: OpcoesQr) -> Self {
        self.qr = qr;
        self
    }

    /// Troca o relogio (teste).
    #[must_use]
    pub fn com_agora(mut self, agora: DateTime<Utc>) -> Self {
        self.agora = agora;
        self
    }

    /// A pasta de notas, ou o erro que diz como configura-la.
    ///
    /// # Errors
    ///
    /// [`Erro::Uso`] se nao ha `[notas].pasta`.
    pub fn pasta(&self) -> Result<&'a NotasNfe, Erro> {
        self.notas.ok_or_else(precisa_da_pasta)
    }

    pub(crate) fn segundos(&self) -> i64 {
        self.agora.timestamp()
    }
}

/// O erro de quem tenta guardar sem pasta.
pub(crate) fn precisa_da_pasta() -> Erro {
    Erro::Uso(
        "a NF-e usa a pasta de notas: informe [notas].pasta no arquivo de configuracao (ou RUSTNF_NOTAS_PASTA)"
            .into(),
    )
}

/// A chave sem espacos nem pontuacao, como a pessoa a cola.
pub(crate) fn chave_limpa(chave: &str) -> String {
    chave.chars().filter(char::is_ascii_digit).collect()
}
