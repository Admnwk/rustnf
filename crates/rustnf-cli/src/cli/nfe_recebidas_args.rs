// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! As definicoes do `clap` de `rustnf nfe recebidas`: os documentos recebidos (distribuicao de DF-e) e a manifestacao do
//! destinatario. A execucao esta em `nfe_recebidas`.

use clap::{Args, Subcommand, ValueEnum};

use super::ArgCertNfe;

/// `rustnf nfe recebidas ...`: as NF-e em que a empresa e' destinataria. `sincronizar`, `baixar` e `manifestar` falam com o
/// Ambiente Nacional; `listar` e `cursor` so' leem a pasta. Codigos de saida: 0 ok (inclusive "ainda nao e' hora de
/// consultar"), 2 uso ou entrada, 3 recusa (bloqueio 656, rejeicao, nota nao localizada, manifestacao recusada), 4 sem
/// resposta do Ambiente Nacional ou servico parado, 1 o resto.
#[derive(Debug, Subcommand)]
pub enum AcaoRecebidas {
    /// Baixa o que ha de novo, do ultimo NSU ate o fim da fila: resumos, NF-e completas e eventos, gravados em
    /// `nfe/recebidas` na pasta de notas. Respeita a espera de uma hora (fila em dia) e o bloqueio de uma hora (cStat 656):
    /// antes da hora diz quando tentar, sem chamar a rede.
    Sincronizar(ArgSincronizarNfe),
    /// Lista as NF-e recebidas da pasta (so' le a pasta), da mais recente para a mais antiga.
    Listar(ArgListarRecebidas),
    /// Baixa a NF-e de uma chave. O Ambiente Nacional so' entrega a nota completa a quem ja manifestou; com `--ciencia` a
    /// ciencia da operacao (um evento fiscal) e' registrada antes, se a nota ainda nao tem manifestacao.
    Baixar(ArgBaixarNfe),
    /// Registra a manifestacao do destinatario (um EVENTO FISCAL no Ambiente Nacional, assinado com o certificado da
    /// empresa): ciencia, confirmacao, desconhecimento ou operacao nao realizada.
    Manifestar(ArgManifestarNfe),
    /// Mostra o cursor da distribuicao (ultimo NSU, quando se pode consultar de novo, documentos que nao abriram). So' le a
    /// pasta.
    Cursor {
        /// Saida em JSON.
        #[arg(long)]
        json: bool,
    },
}

/// `rustnf nfe recebidas sincronizar`.
#[derive(Debug, Args)]
pub struct ArgSincronizarNfe {
    /// No maximo quantos lotes (de ate 50 documentos) ler nesta chamada, de 1 a 200. O padrao e' 20.
    #[arg(long, value_name = "N")]
    pub lotes: Option<usize>,
    /// Certificado, senha e ambiente.
    #[command(flatten)]
    pub cert: ArgCertNfe,
    /// Saida em JSON.
    #[arg(long)]
    pub json: bool,
}

/// Situacao de uma NF-e recebida.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum SituacaoRecebidaArg {
    /// So' o resumo.
    Resumo,
    /// A NF-e completa, sem manifestacao.
    Completa,
    /// Ciencia da operacao.
    Ciencia,
    /// Operacao confirmada.
    Confirmada,
    /// Operacao desconhecida.
    Desconhecida,
    /// Operacao nao realizada.
    NaoRealizada,
    /// Cancelada pelo emitente.
    Cancelada,
    /// Denegada.
    Denegada,
}

/// `rustnf nfe recebidas listar`.
#[derive(Debug, Args)]
pub struct ArgListarRecebidas {
    /// So' as notas nesta situacao.
    #[arg(long, value_enum)]
    pub situacao: Option<SituacaoRecebidaArg>,
    /// So' as que casam com o texto: nome do emitente (sem acento), CNPJ/CPF dele ou chave.
    #[arg(long, value_name = "TEXTO")]
    pub busca: Option<String>,
    /// Saida em JSON.
    #[arg(long)]
    pub json: bool,
}

/// `rustnf nfe recebidas baixar`.
#[derive(Debug, Args)]
pub struct ArgBaixarNfe {
    /// Chave de acesso da NF-e (44 posicoes).
    #[arg(long)]
    pub chave: String,
    /// Registra a ciencia da operacao (evento 210210) antes, se a nota ainda nao tem manifestacao: sem ela o Ambiente
    /// Nacional entrega so' o resumo.
    #[arg(long)]
    pub ciencia: bool,
    /// Certificado, senha e ambiente.
    #[command(flatten)]
    pub cert: ArgCertNfe,
    /// Saida em JSON.
    #[arg(long)]
    pub json: bool,
}

/// A manifestacao do destinatario.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum TipoManifestacaoArg {
    /// 210210: tem ciencia da NF-e (libera a nota completa); ainda nao e' a manifestacao conclusiva.
    Ciencia,
    /// 210200: a operacao ocorreu como a NF-e diz.
    Confirmacao,
    /// 210220: a operacao nao foi solicitada pela empresa. Nao libera a nota completa.
    Desconhecimento,
    /// 210240: a empresa participou, mas a operacao nao ocorreu como a NF-e diz (exige `--justificativa`).
    NaoRealizada,
}

/// `rustnf nfe recebidas manifestar`.
#[derive(Debug, Args)]
pub struct ArgManifestarNfe {
    /// Chave de acesso da NF-e (44 posicoes).
    #[arg(long)]
    pub chave: String,
    /// Qual manifestacao registrar.
    #[arg(long, value_enum)]
    pub tipo: TipoManifestacaoArg,
    /// Justificativa, de 15 a 255 caracteres: obrigatoria na operacao nao realizada, opcional no desconhecimento, proibida
    /// nas outras duas. Fica no documento fiscal.
    #[arg(long)]
    pub justificativa: Option<String>,
    /// Certificado, senha e ambiente.
    #[command(flatten)]
    pub cert: ArgCertNfe,
    /// Saida em JSON.
    #[arg(long)]
    pub json: bool,
}

impl AcaoRecebidas {
    /// O certificado e o ambiente da acao, se ela fala com o Ambiente Nacional.
    #[must_use]
    pub fn cert(&self) -> Option<&ArgCertNfe> {
        match self {
            Self::Sincronizar(a) => Some(&a.cert),
            Self::Baixar(a) => Some(&a.cert),
            Self::Manifestar(a) => Some(&a.cert),
            Self::Listar(_) | Self::Cursor { .. } => None,
        }
    }
}
