// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! As definicoes do `clap` de `rustnf nfce`: a NFC-e (modelo 65). A execucao esta em `nfce`.

use clap::{Args, Subcommand};

use super::nfe_args::ArgCertNfe;

/// `rustnf nfce ...`. Codigos de saida: 0 ok (nota autorizada ou off-line gravada, evento registrado, nada pendente),
/// 2 uso ou entrada, 3 recusa de regra (rejeitada, denegada, prazo vencido), 4 sem resposta da SEFAZ ou servico parado (a
/// nota PODE ter sido autorizada: use `rustnf nfce pendentes --retomar`, nunca emita outra) e, com `--fechar-caixa`,
/// nota ainda pendente.
#[derive(Debug, Subcommand)]
pub enum AcaoNfce {
    /// O CSC (Codigo de Seguranca do Contribuinte) no cofre do sistema: so' o QR Code versao 2 o usa. O valor nunca fica no
    /// arquivo de configuracao, na tela nem em arquivo da pasta de notas.
    Csc {
        /// O que fazer.
        #[command(subcommand)]
        acao: AcaoCsc,
    },
    /// Completa, confere, monta, assina (com o QR Code da configuracao), GRAVA como pendente e so' entao envia a SEFAZ; o
    /// resultado fecha a nota na pasta. O envio da NFC-e e' sempre sincrono. Se a resposta se perde, a nota fica pendente
    /// (nunca vira outra nota); sem internet, `--offline` emite em contingencia off-line.
    Emitir(ArgEmitirNfce),
    /// Consulta a NFC-e pela chave e poe a nota da pasta em dia com o que a SEFAZ diz.
    Consultar(ArgConsultarNfce),
    /// Cancela a NFC-e (evento 110111). Antes de enviar, avisa se o prazo nacional de 24 horas ja acabou; a SEFAZ decide.
    Cancelar(ArgCancelarNfce),
    /// Lista as NFC-e que ainda dependem da SEFAZ (off-line por transmitir e enviadas sem resposta) com o prazo que resta e,
    /// com `--retomar`, as retoma. Com `--fechar-caixa` sai com codigo 4 se sobrar alguma.
    Pendentes(ArgPendentesNfce),
    /// O estado do servico de autorizacao da NFC-e da UF. Nao altera nada.
    Status(ArgStatusNfce),
    /// Gera o DANFC-e como ARQUIVO (PDF de bobina ou bytes ESC/POS) da nota da pasta (pela chave) ou de um XML. Nao fala com
    /// impressora: quem imprime e' outro programa.
    Danfce(ArgDanfce),
}

/// `rustnf nfce csc ...`.
#[derive(Debug, Subcommand)]
pub enum AcaoCsc {
    /// Guarda o CSC no cofre (chave propria: emitente, ambiente e `[nfce].id_csc`). Le uma linha do stdin ou da variavel de
    /// `--csc-env`; nunca de argumento de linha de comando.
    Guardar(ArgCscGuardar),
    /// Diz se ha CSC guardado para o ambiente e o `idCSC` da configuracao. Nunca mostra o valor.
    Testar(ArgCscSo),
    /// Apaga o CSC do cofre.
    Apagar(ArgCscSo),
}

/// Ambiente de `rustnf nfce csc`.
#[derive(Debug, Args)]
pub struct ArgCscSo {
    /// Ambiente do CSC. Sem a opcao vale `[nfce].ambiente`; o padrao e' homologacao.
    #[arg(long, value_enum)]
    pub ambiente: Option<super::AmbienteNfe>,
    /// O `idCSC`; sem a opcao vale `[nfce].id_csc`.
    #[arg(long, value_name = "ID")]
    pub id_csc: Option<String>,
}

/// `rustnf nfce csc guardar`.
#[derive(Debug, Args)]
pub struct ArgCscGuardar {
    /// Ambiente e `idCSC`.
    #[command(flatten)]
    pub so: ArgCscSo,
    /// Le o CSC desta variavel de ambiente. Sem a opcao, le uma linha do stdin.
    #[arg(long, value_name = "VARIAVEL")]
    pub csc_env: Option<String>,
}

/// `rustnf nfce emitir`.
#[derive(Debug, Args)]
pub struct ArgEmitirNfce {
    /// JSON do pedido da NFC-e (docs/NFE_PEDIDO.md; o modelo 65 vem sozinho); `-` le do stdin.
    pub pedido: String,
    /// Certificado, senha e ambiente.
    #[command(flatten)]
    pub cert: ArgCertNfe,
    /// Contingencia off-line (`tpEmis` 9): assina e grava a nota como pendente de transmissao, SEM falar com a SEFAZ.
    /// Entregue o DANFC-e ao consumidor e transmita no prazo com `rustnf nfce pendentes --retomar`.
    #[arg(long, conflicts_with = "offline_se_necessario")]
    pub offline: bool,
    /// Pergunta ao servico de status da NFC-e antes de enviar: se a SEFAZ nao responde (ou esta paralisada), emite off-line;
    /// se responde, envia normalmente. Depois de enviar sem resposta NUNCA vira off-line (seria nota gemea): fica pendente.
    #[arg(long)]
    pub offline_se_necessario: bool,
    /// Grava o DANFC-e em PDF ao lado da nota na pasta de notas (se a nota foi autorizada ou ficou off-line).
    #[arg(long)]
    pub danfce: bool,
    /// Aceita o campo que o leiaute nao conhece, com aviso. Sem a opcao, ele e' recusado antes de qualquer envio.
    #[arg(long)]
    pub ignorar_desconhecidos: bool,
    /// Saida em JSON.
    #[arg(long)]
    pub json: bool,
}

/// `rustnf nfce consultar`.
#[derive(Debug, Args)]
pub struct ArgConsultarNfce {
    /// Chave de acesso da NFC-e (44 posicoes).
    #[arg(long)]
    pub chave: String,
    /// Certificado, senha e ambiente.
    #[command(flatten)]
    pub cert: ArgCertNfe,
    /// Saida em JSON.
    #[arg(long)]
    pub json: bool,
}

/// `rustnf nfce cancelar`.
#[derive(Debug, Args)]
pub struct ArgCancelarNfce {
    /// Chave de acesso da NFC-e (44 posicoes).
    #[arg(long)]
    pub chave: String,
    /// Justificativa, de 15 a 255 caracteres (fica no documento fiscal).
    #[arg(long)]
    pub justificativa: String,
    /// Protocolo da AUTORIZACAO (`nProt`): so' para a nota que nao esta na pasta de notas.
    #[arg(long, value_name = "NPROT")]
    pub protocolo: Option<String>,
    /// Certificado, senha e ambiente.
    #[command(flatten)]
    pub cert: ArgCertNfe,
    /// Saida em JSON.
    #[arg(long)]
    pub json: bool,
}

/// `rustnf nfce pendentes`.
#[derive(Debug, Args)]
pub struct ArgPendentesNfce {
    /// Retoma as notas: consulta a SEFAZ e transmite as off-line (precisa do certificado). Sem a opcao, so' lista.
    #[arg(long)]
    pub retomar: bool,
    /// Ao fechar o caixa: sai com codigo 4 se sobrar nota pendente, e diz o que fazer e ate quando.
    #[arg(long)]
    pub fechar_caixa: bool,
    /// Certificado, senha e ambiente (so' com `--retomar`).
    #[command(flatten)]
    pub cert: ArgCertNfe,
    /// Saida em JSON.
    #[arg(long)]
    pub json: bool,
}

/// `rustnf nfce status`.
#[derive(Debug, Args)]
pub struct ArgStatusNfce {
    /// Certificado, senha e ambiente.
    #[command(flatten)]
    pub cert: ArgCertNfe,
    /// Saida em JSON.
    #[arg(long)]
    pub json: bool,
}

/// `rustnf nfce danfce`.
#[derive(Debug, Args)]
pub struct ArgDanfce {
    /// A chave de acesso (44 posicoes) de uma nota da pasta de notas, ou o arquivo XML (`nfeProc` ou NFe de contingencia);
    /// `-` le o XML do stdin.
    pub alvo: String,
    /// Grava o PDF de bobina neste arquivo (`-` = stdout).
    #[arg(
        long,
        value_name = "ARQUIVO",
        conflicts_with = "escpos",
        required_unless_present = "escpos"
    )]
    pub pdf: Option<String>,
    /// Grava os bytes ESC/POS neste arquivo (`-` = stdout); quem envia a impressora e' outro programa.
    #[arg(long, value_name = "ARQUIVO")]
    pub escpos: Option<String>,
    /// Largura da bobina em mm: 80 ou 58. Sem a opcao vale `[nfce].largura_danfce`.
    #[arg(long, value_name = "MM", value_parser = ["80", "58"])]
    pub largura: Option<String>,
    /// Colunas do ESC/POS: 48, 42 ou 32. Sem a opcao vale `[nfce].colunas_escpos`.
    #[arg(long, value_name = "N", value_parser = ["48", "42", "32"])]
    pub colunas: Option<String>,
    /// Cupom resumido, sem a divisao II (so' onde a UF permite).
    #[arg(long)]
    pub resumido: bool,
    /// A via do estabelecimento (so' existe na contingencia off-line).
    #[arg(long)]
    pub via_estabelecimento: bool,
    /// Marca de nota cancelada num XML solto (com a chave da pasta ela vem sozinha).
    #[arg(long)]
    pub cancelada: bool,
}

impl AcaoNfce {
    /// O certificado e o ambiente da acao, se ela fala com a SEFAZ.
    #[must_use]
    pub fn cert(&self) -> Option<&ArgCertNfe> {
        match self {
            Self::Emitir(a) => Some(&a.cert),
            Self::Consultar(a) => Some(&a.cert),
            Self::Cancelar(a) => Some(&a.cert),
            Self::Status(a) => Some(&a.cert),
            Self::Pendentes(a) if a.retomar => Some(&a.cert),
            _ => None,
        }
    }

    /// A acao le o pedido do stdin (`-`): a senha nao pode vir do stdin tambem.
    #[must_use]
    pub fn pedido_do_stdin(&self) -> bool {
        matches!(self, Self::Emitir(a) if a.pedido == "-")
    }
}
