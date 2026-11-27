// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Linha de comando: as definicoes do `clap`. A execucao de cada subcomando esta em `comandos`.
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
mod nfce;
mod nfce_args;
mod nfe;
mod nfe_args;
mod nfe_recebidas;
mod nfe_recebidas_args;
mod resolver;
mod servir;

use std::net::IpAddr;
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use rustnf::assinatura::xmldsig::Algoritmo;

pub use comandos::{
    atualizar_com, consultar_com, emitir_valor, evento_consultar_com, evento_registrar_com,
    executar, executar_com, testar_conexao, VARIAVEL_COFRE,
};
pub use nfce::nfce_com;
pub use nfce_args::{
    AcaoCsc, AcaoNfce, ArgCancelarNfce, ArgConsultarNfce, ArgCscGuardar, ArgCscSo, ArgDanfce,
    ArgEmitirNfce, ArgPendentesNfce, ArgStatusNfce,
};
pub use nfe::nfe_com;
pub use nfe_args::{
    AcaoNfe, AcaoNotasNfe, AmbienteNfe, ArgCancelarNfe, ArgCertNfe, ArgConferirNfe,
    ArgConsultarNfe, ArgCorrigirNfe, ArgDanfeNfe, ArgEmitirNfe, ArgInutilizarNfe, ArgPedidoNfe,
    ArgPendentesNfe, ArgStatusNfe, SituacaoNfe,
};
pub use nfe_recebidas_args::{
    AcaoRecebidas, ArgBaixarNfe, ArgListarRecebidas, ArgManifestarNfe, ArgSincronizarNfe,
    SituacaoRecebidaArg, TipoManifestacaoArg,
};
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
    /// Gera o DANFSe em PDF a partir do XML da NFS-e autorizada ou de uma nota da pasta de notas (`--chave`).
    Danfse(ArgDanfse),
    /// Monta, assina com o certificado A1 e envia a DPS a Sefin Nacional.
    Emitir(ArgEmitir),
    /// Consulta uma NFS-e pela chave de acesso, ou a chave que uma DPS gerou (pelo Id ou por serie e numero).
    Consultar(ArgConsultar),
    /// Cancela uma NFS-e (evento 101101).
    Cancelar(ArgCancelar),
    /// Eventos da NFS-e: consultar um evento, registrar um que o contribuinte pede e listar os tipos.
    Evento {
        /// O que fazer.
        #[command(subcommand)]
        acao: AcaoEvento,
    },
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
    /// NF-e (modelo 55): montar, conferir, emitir, consultar, cancelar, corrigir, inutilizar, status, EPEC, retomar
    /// pendentes, DANFE e a pasta de notas da NF-e. Nao e' software oficial: o XML e as regras foram conferidos contra a
    /// documentacao oficial; comece pela homologacao, onde a nota nao tem efeito fiscal (veja "Estado" no README).
    Nfe {
        /// O que fazer.
        #[command(subcommand)]
        acao: AcaoNfe,
    },
    /// NFC-e (modelo 65): o CSC no cofre, emitir (inclusive em contingencia off-line), consultar, cancelar, status, os
    /// pendentes e o aviso "ao fechar o caixa", e o DANFC-e como arquivo (PDF ou ESC/POS, pronto para a impressora). Nao e'
    /// software oficial: comece pela homologacao, com o CSC de homologacao da UF (veja "Estado" no README).
    Nfce {
        /// O que fazer.
        #[command(subcommand)]
        acao: AcaoNfce,
    },
    /// Notas autorizadas guardadas na pasta de notas.
    Notas {
        /// O que fazer.
        #[command(subcommand)]
        acao: AcaoNotas,
    },
    /// O certificado A1: de quem e', ate quando vale e se assina hoje.
    Certificado {
        /// O que fazer.
        #[command(subcommand)]
        acao: AcaoCertificado,
    },
    /// A Sefin Nacional: teste da conexao com o certificado.
    Sefin {
        /// O que fazer.
        #[command(subcommand)]
        acao: AcaoSefin,
    },
    /// Imprime o script que completa os comandos com Tab no shell. PowerShell:
    /// `rustnf completar powershell | Out-String | Invoke-Expression` (ponha no `$PROFILE` para valer sempre);
    /// bash: `source <(rustnf completar bash)`.
    Completar {
        /// O shell: powershell, bash, zsh, fish ou elvish.
        shell: clap_complete::Shell,
    },
}

/// `rustnf evento ...`.
#[derive(Debug, Subcommand)]
pub enum AcaoEvento {
    /// Consulta um evento da NFS-e na Sefin Nacional. Nao altera nada. Saida 4 se o evento nao existe ou a Sefin nao
    /// respondeu.
    Consultar(ArgEventoConsultar),
    /// Registra um evento que o contribuinte pede (por exemplo, 101103: pedido de analise fiscal para
    /// cancelamento). O cancelamento tem o comando `cancelar`; os eventos do fisco nao se pedem. Operacao com efeito
    /// fiscal.
    Registrar(ArgEventoRegistrar),
    /// Lista os tipos de evento que o contribuinte registra, com a descricao e o autor. Nao usa a rede.
    Tipos {
        /// Saida em JSON.
        #[arg(long)]
        json: bool,
    },
}

/// `rustnf evento consultar`.
#[derive(Debug, Args)]
pub struct ArgEventoConsultar {
    /// Chave de acesso da NFS-e (50 posicoes).
    #[arg(long)]
    pub chave: String,
    /// Tipo do evento (`tpEvento`, 6 digitos): veja `rustnf evento tipos`.
    #[arg(long, value_name = "TIPO")]
    pub tipo: String,
    /// Sequencia do evento do tipo na nota (comeca em 1).
    #[arg(long, value_name = "N", default_value_t = 1)]
    pub sequencia: u32,
    /// Certificado, senha e ambiente.
    #[command(flatten)]
    pub sefin: ArgSefin,
    /// Saida em JSON.
    #[arg(long)]
    pub json: bool,
}

/// `rustnf evento registrar`.
#[derive(Debug, Args)]
pub struct ArgEventoRegistrar {
    /// Chave de acesso da NFS-e (50 posicoes).
    #[arg(long)]
    pub chave: String,
    /// Tipo do evento (`tpEvento`, 6 digitos): veja `rustnf evento tipos`.
    #[arg(long, value_name = "TIPO")]
    pub tipo: String,
    /// Codigo do motivo, quando o evento tem (101103: 1 erro na emissao, 2 servico nao prestado, 9 outros).
    #[arg(long)]
    pub motivo: Option<String>,
    /// Justificativa, quando o evento tem (101103: obrigatoria, fica no documento fiscal).
    #[arg(long)]
    pub justificativa: Option<String>,
    /// Certificado, senha e ambiente.
    #[command(flatten)]
    pub sefin: ArgSefin,
    /// Saida em JSON.
    #[arg(long)]
    pub json: bool,
}

/// `rustnf certificado ...`.
#[derive(Debug, Subcommand)]
pub enum AcaoCertificado {
    /// Abre o certificado e mostra titular, documento, emissor, validade e os avisos. Nao usa a rede. Saida 3 se o
    /// certificado nao assina hoje (vencido ou ainda nao valido).
    Mostrar(ArgCertificado),
}

/// `rustnf certificado mostrar`.
#[derive(Debug, Args)]
pub struct ArgCertificado {
    /// Arquivo do certificado A1 (.pfx ou .p12). Sem a opcao vale RUSTNF_PFX ou `[sefin].pfx`.
    #[arg(long, value_name = "ARQUIVO")]
    pub pfx: Option<String>,
    /// Senha do certificado. Sem as duas opcoes, vale a senha guardada no cofre do sistema.
    #[command(flatten)]
    pub senha: ArgSenha,
    /// Saida em JSON.
    #[arg(long)]
    pub json: bool,
}

/// `rustnf sefin ...`.
#[derive(Debug, Subcommand)]
pub enum AcaoSefin {
    /// Confere o certificado e faz uma chamada real a Sefin Nacional (a consulta de uma chave que nao existe): diz
    /// se a conexao e o certificado foram aceitos. Nao emite nem altera nada. Saida 3 se o certificado nao assina
    /// hoje, 4 se a Sefin nao respondeu ou recusou o certificado.
    Testar(ArgTestar),
}

/// `rustnf sefin testar`.
#[derive(Debug, Args)]
pub struct ArgTestar {
    /// Certificado, senha e ambiente.
    #[command(flatten)]
    pub sefin: ArgSefin,
    /// Saida em JSON.
    #[arg(long)]
    pub json: bool,
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
        situacao: Option<SituacaoDaNota>,
        /// So' as notas que casam com o texto: nome do tomador (sem acento), CPF/CNPJ dele, numero da NFS-e ou da
        /// DPS, ou uma chave de acesso.
        #[arg(long, value_name = "TEXTO")]
        busca: Option<String>,
    },
    /// Copia para uma pasta os XML das notas de um mes e a lista delas em CSV (`notas.csv`): o que se manda para a
    /// contabilidade. Nada sai da pasta de notas.
    Exportar {
        /// O mes das notas.
        #[arg(long, value_name = "AAAA-MM")]
        mes: String,
        /// A pasta de destino (criada se faltar).
        #[arg(long, value_name = "PASTA")]
        para: String,
    },
    /// Refaz o indice da pasta de notas a partir dos XML guardados (indice perdido ou ilegivel). O indice anterior
    /// fica copiado ao lado; a numeracao da DPS nao muda.
    Reindexar,
    /// Confere na Sefin Nacional a situacao das notas da pasta (cancelada, substituida, em analise fiscal) e atualiza
    /// o indice. Uma nota e uma consulta por vez, com um intervalo entre elas. Nota que a pasta da como cancelada ou
    /// substituida e a Sefin nao mostra assim nunca volta para autorizada: vai para as falhas. Saida 4 se alguma
    /// nota nao pode ser conferida.
    Atualizar(ArgAtualizar),
    /// Mostra o proximo numero de DPS da serie ou, com `--proximo`, faz a serie continuar nele: para quem ja emitia
    /// por outro sistema, use o numero seguinte a ultima DPS de la. So' anda para a frente.
    Numero {
        /// A serie; sem a opcao, a do `[emitente]` ou 1.
        #[arg(long, value_name = "SERIE")]
        serie: Option<String>,
        /// O numero da proxima DPS da serie.
        #[arg(long, value_name = "N")]
        proximo: Option<u64>,
    },
}

impl From<SituacaoDaNota> for rustnf_app::notas::Situacao {
    fn from(s: SituacaoDaNota) -> Self {
        match s {
            SituacaoDaNota::Autorizada => Self::Autorizada,
            SituacaoDaNota::Cancelada => Self::Cancelada,
            SituacaoDaNota::Substituida => Self::Substituida,
            SituacaoDaNota::AnaliseFiscal => Self::AnaliseFiscal,
        }
    }
}

/// `rustnf notas atualizar`.
#[derive(Debug, Args)]
pub struct ArgAtualizar {
    /// So' as notas emitidas a partir deste dia (inclusive).
    #[arg(long, value_name = "AAAA-MM-DD")]
    pub de: Option<String>,
    /// So' as notas emitidas ate este dia (inclusive).
    #[arg(long, value_name = "AAAA-MM-DD")]
    pub ate: Option<String>,
    /// So' as notas que a pasta tem nesta situacao.
    #[arg(long, value_enum)]
    pub situacao: Option<SituacaoDaNota>,
    /// So' as notas que casam com o texto: nome do tomador (sem acento), CPF/CNPJ dele, numero da NFS-e ou da DPS,
    /// ou uma chave de acesso.
    #[arg(long, value_name = "TEXTO")]
    pub busca: Option<String>,
    /// Intervalo entre duas consultas a Sefin, em milissegundos (cortesia com o servico; padrao 200).
    #[arg(long, value_name = "MS")]
    pub intervalo_ms: Option<u64>,
    /// Certificado, senha e ambiente.
    #[command(flatten)]
    pub sefin: ArgSefin,
    /// Saida em JSON.
    #[arg(long)]
    pub json: bool,
}

/// Situacao de uma nota na pasta de notas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum SituacaoDaNota {
    /// Autorizada.
    Autorizada,
    /// Cancelada.
    Cancelada,
    /// Substituida por outra nota.
    Substituida,
    /// Com pedido de analise fiscal em aberto.
    AnaliseFiscal,
}

/// `rustnf dps ...`.
#[derive(Debug, Subcommand)]
pub enum AcaoDps {
    /// Le o JSON da DPS e escreve o XML (nao assinado) no stdout.
    Montar {
        /// JSON de `rustnf::nfse::Dps`; `-` le do stdin.
        pedido: String,
        /// Recusa (saida 3) o pedido com campo desconhecido; sem a opcao, ele e' ignorado com um aviso no stderr.
        #[arg(long)]
        estrito: bool,
    },
    /// So' confere as regras: diz "ok" ou a recusa (campo e motivo).
    Conferir {
        /// JSON de `rustnf::nfse::Dps`; `-` le do stdin.
        pedido: String,
        /// Saida em JSON.
        #[arg(long)]
        json: bool,
        /// Recusa (saida 3) o pedido com campo desconhecido; sem a opcao, ele e' ignorado com um aviso no stderr.
        #[arg(long)]
        estrito: bool,
    },
    /// Imprime o JSON Schema do pedido da DPS: os campos que existem, com a descricao de cada um.
    Esquema {
        /// O schema do pedido de emissao: nao exige o prestador, o municipio emissor nem o numero, que a
        /// configuracao pode completar.
        #[arg(long)]
        emissao: bool,
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
    /// XML da NFS-e autorizada; `-` le do stdin. Dispensado com `--chave`.
    #[arg(required_unless_present = "chave", conflicts_with = "chave")]
    pub nfse: Option<String>,
    /// Chave de acesso de uma nota da pasta de notas: o DANFSe sai do XML guardado nela.
    #[arg(long, value_name = "CHAVE")]
    pub chave: Option<String>,
    /// PDF de saida; `-` escreve no stdout. Com `--chave` e sem esta opcao, o PDF fica ao lado do XML, na pasta de
    /// notas, e o caminho dele sai no stdout.
    #[arg(
        short = 'o',
        long = "saida",
        value_name = "ARQUIVO.pdf",
        required_unless_present = "chave"
    )]
    pub saida: Option<String>,
    /// Situacao da nota (marca d'agua). Sem a opcao: valida; com `--chave`, a situacao da nota na pasta.
    #[arg(long, value_enum)]
    pub situacao: Option<SituacaoArg>,
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
    /// Recusa (saida 3), antes de qualquer envio, o pedido com campo desconhecido. E' o padrao; a opcao continua
    /// aceita por compatibilidade.
    #[arg(long, conflicts_with = "ignorar_desconhecidos")]
    pub estrito: bool,
    /// Emite mesmo com campo desconhecido no pedido: ele e' ignorado (nao entra no XML) e aparece num aviso no stderr.
    #[arg(long)]
    pub ignorar_desconhecidos: bool,
    /// Saida em JSON.
    #[arg(long)]
    pub json: bool,
}

/// `rustnf consultar`: uma NFS-e pela chave, ou a DPS pelo Id ou por serie e numero.
#[derive(Debug, Args)]
pub struct ArgConsultar {
    /// Chave de acesso da NFS-e (50 posicoes).
    #[arg(
        long,
        required_unless_present_any = ["dps", "numero"],
        conflicts_with_all = ["dps", "serie", "numero", "com_nota"]
    )]
    pub chave: Option<String>,
    /// Id da DPS (45 posicoes): devolve a chave da NFS-e que ela gerou.
    #[arg(long, value_name = "ID", conflicts_with_all = ["serie", "numero"])]
    pub dps: Option<String>,
    /// Serie da DPS (sem a opcao, a do `[emitente]` ou 1). O Id da DPS sai do `[emitente]`, da serie e do numero.
    #[arg(long, requires = "numero")]
    pub serie: Option<String>,
    /// Numero da DPS: devolve a chave da NFS-e que ela gerou.
    #[arg(long)]
    pub numero: Option<String>,
    /// Com `--dps` ou `--numero`: busca tambem a nota inteira, nao so' a chave.
    #[arg(long)]
    pub com_nota: bool,
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
