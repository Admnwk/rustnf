// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Linha de comando: as definicoes do `clap`. A execucao de cada subcomando esta em [`comandos`].
//!
//! Convencoes: dado no stdout, diagnostico no stderr; `--json` onde a saida padrao e' texto; saida 0 ok, 2 erro de
//! uso ou de entrada, 3 recusa de regra, 4 rede ou Sefin, 1 o resto. Onde um arquivo e' esperado, `-` le do stdin.
//! A senha do certificado nunca vem por argumento: so' por variavel de ambiente (`--senha-env`) ou stdin
//! (`--senha-stdin`).

mod certificado;
mod comandos;
mod servir;

use std::net::IpAddr;

use clap::{Args, Parser, Subcommand, ValueEnum};

pub use comandos::executar;
pub use servir::{servir, servir_ate};

/// Emissor de NFS-e nacional: monta a DPS, calcula a previa do IBS/CBS, gera o DANFSe, emite pela Sefin Nacional e
/// serve a mesma coisa por API HTTP e MCP. Nao e' software oficial.
#[derive(Debug, Parser)]
#[command(name = "rustnf", version, propagate_version = true)]
pub struct Cli {
    /// O que fazer.
    #[command(subcommand)]
    pub comando: Comando,
}

/// Subcomandos.
#[derive(Debug, Subcommand)]
pub enum Comando {
    /// Monta ou confere a DPS (Declaracao de Prestacao de Servicos).
    Dps {
        /// Montar ou conferir.
        #[command(subcommand)]
        acao: AcaoDps,
    },
    /// Previa do IBS/CBS: base de calculo e tributos, como a calculadora da Receita Federal.
    Calcular(ArgCalcular),
    /// Gera o DANFSe em PDF a partir do XML da NFS-e autorizada.
    Danfse(ArgDanfse),
    /// Monta, assina com o certificado A1 e envia a DPS a Sefin Nacional.
    Emitir(ArgEmitir),
    /// Consulta uma NFS-e pela chave de acesso.
    Consultar(ArgConsultar),
    /// Cancela uma NFS-e (evento 101101).
    Cancelar(ArgCancelar),
    /// Sobe a API HTTP e o servidor MCP.
    Serve(ArgServe),
}

/// `rustnf dps ...`.
#[derive(Debug, Subcommand)]
pub enum AcaoDps {
    /// Le o JSON da DPS e escreve o XML (nao assinado) no stdout.
    Montar {
        /// JSON de `rustnf::nfse::Dps`; `-` le do stdin.
        pedido: String,
    },
    /// So' confere as regras: diz "ok" ou a recusa (campo e motivo).
    Conferir {
        /// JSON de `rustnf::nfse::Dps`; `-` le do stdin.
        pedido: String,
        /// Saida em JSON.
        #[arg(long)]
        json: bool,
    },
}

/// `rustnf calcular`.
#[derive(Debug, Args)]
pub struct ArgCalcular {
    /// JSON com `base` e/ou `tributos`; `-` le do stdin.
    pub operacao: String,
}

/// Situacao da NFS-e no DANFSe.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum SituacaoArg {
    /// Nota valida.
    Valida,
    /// Marca d'agua de cancelada.
    Cancelada,
    /// Marca d'agua de substituida.
    Substituida,
}

/// `rustnf danfse`.
#[derive(Debug, Args)]
pub struct ArgDanfse {
    /// XML da NFS-e autorizada; `-` le do stdin.
    pub nfse: String,
    /// PDF de saida; `-` escreve no stdout.
    #[arg(short = 'o', long = "saida", value_name = "ARQUIVO.pdf")]
    pub saida: String,
    /// Situacao da nota (marca d'agua).
    #[arg(long, value_enum, default_value = "valida")]
    pub situacao: SituacaoArg,
    /// Sem o canhoto de cientificacao no rodape.
    #[arg(long)]
    pub sem_canhoto: bool,
}

/// Ambiente da Sefin Nacional.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Ambiente {
    /// Producao (tpAmb 1): a nota vale no fisco.
    Producao,
    /// Producao restrita (tpAmb 2): ambiente de testes da Sefin.
    Restrita,
}

impl Ambiente {
    /// O `tpAmb` do leiaute.
    #[must_use]
    pub fn codigo(self) -> u8 {
        match self {
            Self::Producao => 1,
            Self::Restrita => 2,
        }
    }
}

/// De onde vem a senha do certificado. Nao existe opcao para passar a senha como argumento.
#[derive(Debug, Args)]
pub struct ArgSenha {
    /// Variavel de ambiente que guarda a senha do .pfx.
    #[arg(long, value_name = "VARIAVEL", conflicts_with = "senha_stdin")]
    pub senha_env: Option<String>,
    /// Le a senha do .pfx da primeira linha do stdin.
    #[arg(long)]
    pub senha_stdin: bool,
}

/// Certificado e ambiente das operacoes que falam com a Sefin.
#[derive(Debug, Args)]
pub struct ArgSefin {
    /// Arquivo do certificado A1 (.pfx ou .p12).
    #[arg(long, value_name = "ARQUIVO")]
    pub pfx: String,
    /// Senha do certificado.
    #[command(flatten)]
    pub senha: ArgSenha,
    /// Ambiente da Sefin Nacional.
    #[arg(long, value_enum)]
    pub ambiente: Ambiente,
}

/// `rustnf emitir`.
#[derive(Debug, Args)]
pub struct ArgEmitir {
    /// JSON de `rustnf::nfse::Dps`; `-` le do stdin.
    pub pedido: String,
    /// Certificado, senha e ambiente.
    #[command(flatten)]
    pub sefin: ArgSefin,
    /// Segundos subtraidos do relogio ao preencher o `dhEmi` que o pedido nao traz (regra E0008).
    #[arg(long, default_value_t = 60, value_name = "SEGUNDOS")]
    pub folga_relogio: u32,
    /// Saida em JSON.
    #[arg(long)]
    pub json: bool,
}

/// `rustnf consultar`.
#[derive(Debug, Args)]
pub struct ArgConsultar {
    /// Chave de acesso da NFS-e (50 posicoes).
    #[arg(long)]
    pub chave: String,
    /// Certificado, senha e ambiente.
    #[command(flatten)]
    pub sefin: ArgSefin,
    /// Saida em JSON.
    #[arg(long)]
    pub json: bool,
}

/// `rustnf cancelar`.
#[derive(Debug, Args)]
pub struct ArgCancelar {
    /// Chave de acesso da NFS-e (50 posicoes).
    #[arg(long)]
    pub chave: String,
    /// Codigo do motivo: 1 erro na emissao, 2 servico nao prestado, 9 outros.
    #[arg(long)]
    pub motivo: String,
    /// Justificativa (fica no documento fiscal).
    #[arg(long)]
    pub justificativa: String,
    /// Certificado, senha e ambiente.
    #[command(flatten)]
    pub sefin: ArgSefin,
    /// Saida em JSON.
    #[arg(long)]
    pub json: bool,
}

/// `rustnf serve`.
#[derive(Debug, Args)]
pub struct ArgServe {
    /// Endereco de escuta. Fora de 127.0.0.1 o trafego nao e' cifrado: ponha um proxy com TLS na frente.
    #[arg(long, default_value = "127.0.0.1")]
    pub endereco: IpAddr,
    /// Porta de escuta (0 = qualquer porta livre).
    #[arg(long, default_value_t = 8080)]
    pub porta: u16,
    /// Teto do corpo da requisicao, em bytes.
    #[arg(long, default_value_t = 1024 * 1024, value_name = "BYTES")]
    pub limite_corpo: usize,
    /// Tempo limite de cada requisicao, em segundos.
    #[arg(long, default_value_t = 60, value_name = "SEGUNDOS")]
    pub tempo_limite: u64,
    /// Origem extra aceita no cabecalho Origin (repetivel), alem do proprio computador.
    #[arg(long, value_name = "ORIGEM")]
    pub origem_permitida: Vec<String>,
    /// Liga as rotas de emissao, consulta e cancelamento (exige certificado, ambiente e token).
    #[arg(long, requires_all = ["pfx", "ambiente", "token_env"])]
    pub permitir_emissao: bool,
    /// Arquivo do certificado A1 (.pfx ou .p12).
    #[arg(long, value_name = "ARQUIVO")]
    pub pfx: Option<String>,
    /// Senha do certificado.
    #[command(flatten)]
    pub senha: ArgSenha,
    /// Ambiente da Sefin Nacional.
    #[arg(long, value_enum)]
    pub ambiente: Option<Ambiente>,
    /// Variavel de ambiente com o token Bearer das rotas de emissao (no minimo 32 caracteres).
    #[arg(long, value_name = "VARIAVEL")]
    pub token_env: Option<String>,
    /// Segundos subtraidos do relogio ao preencher o `dhEmi` que o pedido nao traz.
    #[arg(long, default_value_t = 60, value_name = "SEGUNDOS")]
    pub folga_relogio: u32,
}
