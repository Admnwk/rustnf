// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Linha de comando: as definicoes do `clap`. A execucao de cada subcomando esta em [`comandos`].
//!
//! Convencoes: dado no stdout, diagnostico no stderr; `--json` onde a saida padrao e' texto; saida 0 ok, 2 erro de
//! uso ou de entrada, 3 recusa de regra, 4 rede ou Sefin, 1 o resto. Onde um arquivo e' esperado, `-` le do stdin.
//! A senha do certificado nunca vem por argumento: so' por variavel de ambiente (`--senha-env`), stdin
//! (`--senha-stdin`) ou, na falta das duas, o cofre do sistema (`rustnf senha guardar`).
//!
//! Configuracao: `--config` (ou `RUSTNF_CONFIG`, ou o arquivo do local padrao). Em todo parametro vale, nesta ordem,
//! a opcao da chamada, a variavel de ambiente `RUSTNF_*`, o arquivo e o padrao.

mod certificado;
mod comandos;
mod configuracao;
mod resolver;
mod servir;

use std::net::IpAddr;
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use rustnf::assinatura::xmldsig::Algoritmo;

pub use comandos::{emitir_valor, executar, executar_com, VARIAVEL_COFRE};
pub use servir::{servir, servir_ate};

/// Emissor de NFS-e nacional: monta a DPS, calcula a previa do IBS/CBS, gera o DANFSe, emite pela Sefin Nacional e
/// serve a mesma coisa por API HTTP e MCP. Nao e' software oficial.
#[derive(Debug, Parser)]
#[command(name = "rustnf", version, propagate_version = true)]
pub struct Cli {
    /// Arquivo de configuracao (TOML). Sem a opcao vale RUSTNF_CONFIG; sem ela, o arquivo do local padrao, se
    /// existir. Precedencia de cada parametro: opcao > variavel de ambiente RUSTNF_* > arquivo > padrao.
    #[arg(long, global = true, value_name = "ARQUIVO")]
    pub config: Option<PathBuf>,
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
    /// Mostra a configuracao efetiva e onde o arquivo mora.
    Config {
        /// O que mostrar.
        #[command(subcommand)]
        acao: AcaoConfig,
    },
    /// Guarda ou apaga a senha do certificado no cofre do sistema.
    Senha {
        /// Guardar ou apagar.
        #[command(subcommand)]
        acao: AcaoSenha,
    },
    /// Notas autorizadas guardadas na pasta de notas.
    Notas {
        /// O que fazer.
        #[command(subcommand)]
        acao: AcaoNotas,
    },
}

/// `rustnf config ...`.
#[derive(Debug, Subcommand)]
pub enum AcaoConfig {
    /// A configuracao efetiva (arquivo e variaveis de ambiente) e de onde veio; nunca mostra segredo.
    Mostrar {
        /// Saida em JSON.
        #[arg(long)]
        json: bool,
    },
    /// O caminho do arquivo de configuracao em uso (ou o que seria usado).
    Caminho,
}

/// `rustnf senha ...`.
#[derive(Debug, Subcommand)]
pub enum AcaoSenha {
    /// Le a senha por --senha-env ou --senha-stdin e grava no cofre do sistema.
    Guardar(ArgGuardar),
    /// Apaga do cofre a senha guardada.
    Apagar(ArgCofre),
}

/// Qual certificado.
#[derive(Debug, Args)]
pub struct ArgCofre {
    /// Arquivo do certificado A1 (.pfx ou .p12); sem ele vale RUSTNF_PFX ou o arquivo de configuracao. Serve para
    /// identificar o item do cofre (com o CNPJ do `[emitente]`, quando houver) e, ao guardar, para conferir a senha.
    #[arg(long, value_name = "ARQUIVO")]
    pub pfx: Option<String>,
}

/// `rustnf senha guardar`: o certificado e de onde vem a senha. Nao existe opcao para passar a senha como argumento.
#[derive(Debug, Args)]
pub struct ArgGuardar {
    /// Qual certificado.
    #[command(flatten)]
    pub cofre: ArgCofre,
    /// De onde ler a senha.
    #[command(flatten)]
    pub senha: ArgSenha,
}

/// `rustnf notas ...`.
#[derive(Debug, Subcommand)]
pub enum AcaoNotas {
    /// Lista as notas da pasta de notas, da mais recente para a mais antiga.
    Listar {
        /// Saida em JSON.
        #[arg(long)]
        json: bool,
    },
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
    /// Sem o canhoto de cientificacao no rodape (sem a opcao vale `[danfse].canhoto`).
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

/// Algoritmo da assinatura da DPS e dos eventos.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Assinatura {
    /// RSA com SHA-256 (o padrao).
    Sha256,
    /// RSA com SHA-1.
    Sha1,
}

impl Assinatura {
    /// O algoritmo da biblioteca.
    #[must_use]
    pub fn algoritmo(self) -> Algoritmo {
        match self {
            Self::Sha256 => Algoritmo::Sha256,
            Self::Sha1 => Algoritmo::Sha1,
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
    /// Arquivo do certificado A1 (.pfx ou .p12). Sem a opcao vale RUSTNF_PFX ou `[sefin].pfx`.
    #[arg(long, value_name = "ARQUIVO")]
    pub pfx: Option<String>,
    /// Senha do certificado. Sem as duas opcoes, vale a senha guardada no cofre do sistema.
    #[command(flatten)]
    pub senha: ArgSenha,
    /// Ambiente da Sefin Nacional. Sem a opcao vale RUSTNF_AMBIENTE ou `[sefin].ambiente`; sem arquivo de
    /// configuracao e obrigatoria.
    #[arg(long, value_enum)]
    pub ambiente: Option<Ambiente>,
    /// Algoritmo da assinatura. Sem a opcao vale RUSTNF_ASSINATURA ou `[sefin].assinatura`; padrao sha256.
    #[arg(long, value_enum)]
    pub assinatura: Option<Assinatura>,
}

/// `rustnf emitir`.
#[derive(Debug, Args)]
pub struct ArgEmitir {
    /// JSON de `rustnf::nfse::Dps`; `-` le do stdin.
    pub pedido: String,
    /// Certificado, senha e ambiente.
    #[command(flatten)]
    pub sefin: ArgSefin,
    /// Segundos subtraidos do relogio ao preencher o `dhEmi` que o pedido nao traz (regra E0008). Padrao: 60.
    #[arg(long, value_name = "SEGUNDOS")]
    pub folga_relogio: Option<u32>,
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

/// `rustnf serve`. Cada parametro: opcao > variavel RUSTNF_* > arquivo de configuracao > padrao.
#[derive(Debug, Args)]
pub struct ArgServe {
    /// Endereco de escuta (padrao 127.0.0.1). Fora de 127.0.0.1 o trafego nao e' cifrado: ponha um proxy com TLS
    /// na frente.
    #[arg(long)]
    pub endereco: Option<IpAddr>,
    /// Porta de escuta, 0 = qualquer porta livre (padrao 8080).
    #[arg(long)]
    pub porta: Option<u16>,
    /// Teto do corpo da requisicao, em bytes (padrao 1048576).
    #[arg(long, value_name = "BYTES")]
    pub limite_corpo: Option<usize>,
    /// Tempo limite de cada requisicao, em segundos (padrao 60).
    #[arg(long, value_name = "SEGUNDOS")]
    pub tempo_limite: Option<u64>,
    /// Origem extra aceita no cabecalho Origin (repetivel), alem do proprio computador.
    #[arg(long, value_name = "ORIGEM")]
    pub origem_permitida: Vec<String>,
    /// Liga as rotas de emissao, consulta e cancelamento (exige certificado, ambiente e token).
    #[arg(long)]
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
    /// Algoritmo da assinatura (padrao sha256).
    #[arg(long, value_enum)]
    pub assinatura: Option<Assinatura>,
    /// Variavel de ambiente com o token Bearer das rotas de emissao (no minimo 32 caracteres).
    #[arg(long, value_name = "VARIAVEL")]
    pub token_env: Option<String>,
    /// Segundos subtraidos do relogio ao preencher o `dhEmi` que o pedido nao traz (padrao 60).
    #[arg(long, value_name = "SEGUNDOS")]
    pub folga_relogio: Option<u32>,
}
