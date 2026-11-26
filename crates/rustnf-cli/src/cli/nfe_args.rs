// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! As definicoes do `clap` de `rustnf nfe`: a NF-e (modelo 55). A execucao esta em `nfe`.

use clap::{Args, Subcommand, ValueEnum};

use super::nfe_recebidas_args::AcaoRecebidas;
use super::ArgSenha;

/// `rustnf nfe ...`. Codigos de saida: 0 ok (nota autorizada, evento registrado, faixa inutilizada), 2 uso ou entrada,
/// 3 recusa de regra (rejeitada, denegada, prazo vencido, pedido que a conferencia recusa), 4 sem resposta da SEFAZ ou
/// servico parado (a nota PODE ter sido autorizada: use `rustnf nfe pendentes --retomar`, nunca emita outra), 1 o resto.
#[derive(Debug, Subcommand)]
pub enum AcaoNfe {
    /// Monta o XML da NF-e a partir do pedido em JSON (sem assinar, sem rede e sem gastar numero) e o escreve no stdout.
    /// O que o pedido nao traz vem do arquivo de configuracao (`[emitente]`, `[nfe]`).
    Montar(ArgPedidoNfe),
    /// So' confere as regras do MOC e das Notas Tecnicas: diz "ok" ou a recusa (campo e codigo da rejeicao), com os
    /// avisos. Nao usa a rede e nao grava nada.
    Conferir(ArgConferirNfe),
    /// Completa, confere, monta, assina com o certificado A1, GRAVA como pendente na pasta de notas e so' entao envia a
    /// SEFAZ; o resultado fecha a nota na pasta. Se a resposta se perde, a nota fica pendente (nunca vira outra nota).
    Emitir(ArgEmitirNfe),
    /// Consulta a NF-e pela chave de acesso e poe a nota da pasta em dia com o que a SEFAZ diz.
    Consultar(ArgConsultarNfe),
    /// Cancela a NF-e (evento 110111).
    Cancelar(ArgCancelarNfe),
    /// Registra uma carta de correcao (evento 110110). A sequencia (1, 2, 3...) sai da pasta de notas; a carta nova
    /// substitui as anteriores, entao o texto leva todas as correcoes que ainda valem.
    Corrigir(ArgCorrigirNfe),
    /// Inutiliza uma faixa de numeracao (numero pulado que nao vai mais ser usado).
    Inutilizar(ArgInutilizarNfe),
    /// O estado do servico de autorizacao da SEFAZ (ou da SVC, com `--svc`). Nao altera nada.
    Status(ArgStatusNfe),
    /// Emite em contingencia EPEC: assina a NF-e (`tpEmis` 4), grava como pendente e registra o EPEC no Ambiente
    /// Nacional. Depois, a MESMA NF-e se transmite a SEFAZ da UF com `pendentes --retomar`.
    Epec(ArgEmitirNfe),
    /// Lista as notas pendentes e em EPEC da pasta e, com `--retomar`, as retoma: consulta a MESMA chave e, se a SEFAZ
    /// diz que nao recebeu, reenvia a MESMA NF-e; a nota em EPEC e' transmitida.
    Pendentes(ArgPendentesNfe),
    /// Gera o DANFE em PDF a partir do XML (`nfeProc`, ou a `NFe` solta como previa) ou de uma nota da pasta (`--chave`).
    Danfe(ArgDanfeNfe),
    /// Notas da NF-e guardadas na pasta de notas (`nfe/` dentro da `[notas].pasta`).
    Notas {
        /// O que fazer.
        #[command(subcommand)]
        acao: AcaoNotasNfe,
    },
    /// Documentos RECEBIDOS: baixa as NF-e em que a empresa e destinataria (distribuicao de DF-e do Ambiente Nacional) e
    /// registra a manifestacao do destinatario (ciencia, confirmacao, desconhecimento, operacao nao realizada).
    Recebidas {
        /// O que fazer.
        #[command(subcommand)]
        acao: AcaoRecebidas,
    },
}

/// Ambiente da NF-e.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum AmbienteNfe {
    /// Homologacao (`tpAmb` 2): a nota nao vale.
    Homologacao,
    /// Producao (`tpAmb` 1): a nota vale no fisco.
    Producao,
}

impl AmbienteNfe {
    /// O texto de `[nfe].ambiente`.
    #[must_use]
    pub fn texto(self) -> &'static str {
        match self {
            Self::Homologacao => "homologacao",
            Self::Producao => "producao",
        }
    }
}

/// Certificado e ambiente das acoes que falam com a SEFAZ.
#[derive(Debug, Args)]
pub struct ArgCertNfe {
    /// Arquivo do certificado A1 (.pfx ou .p12). Sem a opcao vale RUSTNF_PFX ou `[sefin].pfx` (o mesmo da NFS-e).
    #[arg(long, value_name = "ARQUIVO")]
    pub pfx: Option<String>,
    /// Senha do certificado. Sem as duas opcoes, vale a senha guardada no cofre do sistema.
    #[command(flatten)]
    pub senha: ArgSenha,
    /// Ambiente da SEFAZ. Sem a opcao vale RUSTNF_NFE_AMBIENTE ou `[nfe].ambiente`; o padrao e' homologacao.
    #[arg(long, value_enum)]
    pub ambiente: Option<AmbienteNfe>,
}

/// `rustnf nfe montar`.
#[derive(Debug, Args)]
pub struct ArgPedidoNfe {
    /// JSON do pedido da NF-e (docs/NFE_PEDIDO.md); `-` le do stdin.
    pub pedido: String,
    /// Aceita o campo que o leiaute nao conhece: ele e' ignorado (nao entra no XML) e aparece num aviso no stderr. Sem a
    /// opcao, o pedido com campo desconhecido e' recusado (saida 3).
    #[arg(long)]
    pub ignorar_desconhecidos: bool,
}

/// `rustnf nfe conferir`.
#[derive(Debug, Args)]
pub struct ArgConferirNfe {
    /// JSON do pedido da NF-e; `-` le do stdin.
    pub pedido: String,
    /// Aceita o campo desconhecido, com aviso.
    #[arg(long)]
    pub ignorar_desconhecidos: bool,
    /// Saida em JSON.
    #[arg(long)]
    pub json: bool,
}

/// `rustnf nfe emitir` e `rustnf nfe epec`.
#[derive(Debug, Args)]
pub struct ArgEmitirNfe {
    /// JSON do pedido da NF-e; `-` le do stdin.
    pub pedido: String,
    /// Certificado, senha e ambiente.
    #[command(flatten)]
    pub cert: ArgCertNfe,
    /// Aceita o campo desconhecido, com aviso. Sem a opcao, ele e' recusado antes de qualquer envio.
    #[arg(long)]
    pub ignorar_desconhecidos: bool,
    /// Saida em JSON.
    #[arg(long)]
    pub json: bool,
}

/// `rustnf nfe consultar`.
#[derive(Debug, Args)]
pub struct ArgConsultarNfe {
    /// Chave de acesso da NF-e (44 posicoes).
    #[arg(long)]
    pub chave: String,
    /// Certificado, senha e ambiente.
    #[command(flatten)]
    pub cert: ArgCertNfe,
    /// Saida em JSON.
    #[arg(long)]
    pub json: bool,
}

/// `rustnf nfe cancelar`.
#[derive(Debug, Args)]
pub struct ArgCancelarNfe {
    /// Chave de acesso da NF-e (44 posicoes).
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

/// `rustnf nfe corrigir`.
#[derive(Debug, Args)]
pub struct ArgCorrigirNfe {
    /// Chave de acesso da NF-e (44 posicoes).
    #[arg(long)]
    pub chave: String,
    /// O texto da correcao, de 15 a 1000 caracteres: TODAS as correcoes que ainda valem.
    #[arg(long)]
    pub texto: String,
    /// Sequencia da carta (1 a 20): so' para a nota que nao esta na pasta de notas.
    #[arg(long, value_name = "N")]
    pub sequencia: Option<u32>,
    /// Certificado, senha e ambiente.
    #[command(flatten)]
    pub cert: ArgCertNfe,
    /// Saida em JSON.
    #[arg(long)]
    pub json: bool,
}

/// `rustnf nfe inutilizar`.
#[derive(Debug, Args)]
pub struct ArgInutilizarNfe {
    /// Primeiro numero da faixa.
    #[arg(long, value_name = "N")]
    pub inicio: u64,
    /// Ultimo numero da faixa (no maximo 10.000 numeros).
    #[arg(long, value_name = "N")]
    pub fim: u64,
    /// Justificativa, de 15 a 255 caracteres.
    #[arg(long)]
    pub justificativa: String,
    /// A serie; sem a opcao, `[nfe].serie`.
    #[arg(long, value_name = "SERIE")]
    pub serie: Option<String>,
    /// Certificado, senha e ambiente.
    #[command(flatten)]
    pub cert: ArgCertNfe,
    /// Saida em JSON.
    #[arg(long)]
    pub json: bool,
}

/// `rustnf nfe status`.
#[derive(Debug, Args)]
pub struct ArgStatusNfe {
    /// Pergunta a SVC que atende a UF, nao a SEFAZ dela.
    #[arg(long)]
    pub svc: bool,
    /// Certificado, senha e ambiente.
    #[command(flatten)]
    pub cert: ArgCertNfe,
    /// Saida em JSON.
    #[arg(long)]
    pub json: bool,
}

/// `rustnf nfe pendentes`.
#[derive(Debug, Args)]
pub struct ArgPendentesNfe {
    /// Retoma as notas: consulta a SEFAZ (precisa do certificado). Sem a opcao, so' lista.
    #[arg(long)]
    pub retomar: bool,
    /// Certificado, senha e ambiente (so' com `--retomar`).
    #[command(flatten)]
    pub cert: ArgCertNfe,
    /// Saida em JSON.
    #[arg(long)]
    pub json: bool,
}

/// `rustnf nfe danfe`.
#[derive(Debug, Args)]
pub struct ArgDanfeNfe {
    /// XML da NF-e (`nfeProc`, ou `NFe` como previa); `-` le do stdin. Dispensado com `--chave`.
    #[arg(required_unless_present = "chave", conflicts_with = "chave")]
    pub nfe: Option<String>,
    /// Chave de acesso de uma nota da pasta de notas: o DANFE sai do XML guardado nela, com a marca de cancelada.
    #[arg(long, value_name = "CHAVE")]
    pub chave: Option<String>,
    /// PDF de saida; `-` escreve no stdout. Com `--chave` e sem esta opcao, o PDF fica ao lado do XML, na pasta de notas,
    /// e o caminho dele sai no stdout.
    #[arg(
        short = 'o',
        long = "saida",
        value_name = "ARQUIVO.pdf",
        required_unless_present = "chave"
    )]
    pub saida: Option<String>,
    /// Sem o canhoto de recebimento (sem a opcao vale `[nfe].canhoto`).
    #[arg(long)]
    pub sem_canhoto: bool,
    /// Marca d'agua de nota cancelada (com `--chave` ela vem da pasta).
    #[arg(long, conflicts_with = "chave")]
    pub cancelada: bool,
}

/// Situacao de uma NF-e (ou faixa) na pasta de notas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum SituacaoNfe {
    /// Autorizada.
    Autorizada,
    /// Denegada.
    Denegada,
    /// Cancelada.
    Cancelada,
    /// Enviada sem resultado.
    Pendente,
    /// Faixa de numeracao inutilizada.
    Inutilizada,
    /// EPEC registrado, NF-e por transmitir.
    Epec,
}

/// `rustnf nfe notas ...`.
#[derive(Debug, Subcommand)]
pub enum AcaoNotasNfe {
    /// Lista as notas da pasta, da mais recente para a mais antiga. Com `--situacao inutilizada`, lista as faixas
    /// inutilizadas.
    Listar {
        /// Saida em JSON.
        #[arg(long)]
        json: bool,
        /// Saida em CSV para planilha: separador `;`, valor com virgula decimal e uma linha de titulos.
        #[arg(long, conflicts_with = "json")]
        csv: bool,
        /// So' as notas emitidas a partir deste dia (inclusive).
        #[arg(long, value_name = "AAAA-MM-DD")]
        de: Option<String>,
        /// So' as notas emitidas ate este dia (inclusive).
        #[arg(long, value_name = "AAAA-MM-DD")]
        ate: Option<String>,
        /// So' as notas nesta situacao.
        #[arg(long, value_enum)]
        situacao: Option<SituacaoNfe>,
        /// So' as notas que casam com o texto: nome do destinatario (sem acento), CPF/CNPJ dele, numero ou chave.
        #[arg(long, value_name = "TEXTO")]
        busca: Option<String>,
    },
    /// Copia para uma pasta os XML das notas de um mes (e os eventos delas) e a lista em CSV (`notas.csv`): o que se manda
    /// para a contabilidade. Nada sai da pasta de notas.
    Exportar {
        /// O mes das notas.
        #[arg(long, value_name = "AAAA-MM")]
        mes: String,
        /// A pasta de destino (criada se faltar).
        #[arg(long, value_name = "PASTA")]
        para: String,
    },
    /// Refaz o indice da pasta da NF-e a partir dos arquivos guardados. O indice anterior fica copiado ao lado; a numeracao
    /// nao muda.
    Reindexar,
    /// Mostra o proximo numero da serie e os numeros PULADOS (os que se inutilizam) ou, com `--proximo`, faz a serie
    /// continuar nele: para quem ja emitia por outro sistema. So' anda para a frente.
    Numero {
        /// A serie; sem a opcao, `[nfe].serie`.
        #[arg(long, value_name = "SERIE")]
        serie: Option<String>,
        /// O numero da proxima NF-e da serie.
        #[arg(long, value_name = "N")]
        proximo: Option<u64>,
    },
}

impl AcaoNfe {
    /// O certificado e o ambiente da acao, se ela fala com a SEFAZ.
    #[must_use]
    pub fn cert(&self) -> Option<&ArgCertNfe> {
        match self {
            Self::Emitir(a) | Self::Epec(a) => Some(&a.cert),
            Self::Consultar(a) => Some(&a.cert),
            Self::Cancelar(a) => Some(&a.cert),
            Self::Corrigir(a) => Some(&a.cert),
            Self::Inutilizar(a) => Some(&a.cert),
            Self::Status(a) => Some(&a.cert),
            Self::Pendentes(a) if a.retomar => Some(&a.cert),
            Self::Recebidas { acao } => acao.cert(),
            _ => None,
        }
    }

    /// A acao le o pedido do stdin (`-`): a senha nao pode vir do stdin tambem.
    #[must_use]
    pub fn pedido_do_stdin(&self) -> bool {
        matches!(self, Self::Emitir(a) | Self::Epec(a) if a.pedido == "-")
    }
}
