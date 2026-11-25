// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O que a interface faz fora da tela, sem depender de janela: carregar e gravar o arquivo de configuracao, abrir o
//! certificado, guardar a senha no cofre e testar a conexao com a Sefin. Tudo aqui bloqueia (disco, cofre, calculo
//! do certificado): quem chama roda em segundo plano. A conexao com a Sefin e' uma chamada de rede que a pessoa
//! pede ao clicar; os testes usam um transporte falso.

use std::ffi::OsString;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use rustnf::assinatura::cert_a1::Certificado;
use rustnf::assinatura::xmldsig::Algoritmo;
use rustnf_app::atualizacao::INTERVALO_PADRAO;
use rustnf_app::certificado;
use rustnf_app::cofre::{
    chave_do_certificado, cofre_do_ambiente, Cofre, ErroCofre, VARIAVEL_COFRE,
};
use rustnf_app::config::{caminho_do_arquivo, Config, Efetiva, ErroConfig, Indicado};
use rustnf_app::erro::Erro;
use rustnf_app::notas::Notas;
use rustnf_app::segredo::Segredo;

use crate::modelo::ambiente::Ambiente;
use crate::modelo::campo::Campo;
use crate::modelo::certificado::{avaliar, testar, EstadoCertificado, InfoCertificado};
use crate::modelo::mascara::frase;
use crate::modelo::nota::DadosNota;
use crate::modelo::rascunho;

mod emitir;
mod eventos;
mod notas;

pub use emitir::{emitir, FabricaDeEmissor, FabricaReal, FabricaSemEmissor};
pub use eventos::{
    conferir_evento, conferir_lote, consultar_dps_nota, historico_da_nota, registrar_evento_nota,
    ParadaDoLote,
};
pub use notas::{
    cancelar_nota, conferir_cancelamento, consultar_nota, AbrirArquivos, Destino, DoSistema,
    NotaDaPasta,
};

/// Leitura de variavel de ambiente (os testes trocam o ambiente).
pub type LerAmbiente = Arc<dyn Fn(&str) -> Option<OsString> + Send + Sync>;

/// O relogio (os testes fixam o instante).
pub type Relogio = Arc<dyn Fn() -> DateTime<Utc> + Send + Sync>;

/// Por que uma operacao nao deu certo, ja em frase para a tela. Nunca leva senha, caminho de arquivo nem XML.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErroTela {
    /// O arquivo de configuracao nao pode ser lido (ou esta com erro de sintaxe).
    Configuracao(String),
    /// Nao ha onde gravar o arquivo (sem `APPDATA` e sem `--config`).
    SemLocalDeConfiguracao,
    /// O arquivo nao pode ser gravado.
    Gravacao,
    /// A configuracao tem um valor que nao serve.
    Valor(String),
    /// Nenhum certificado escolhido.
    SemCertificado,
    /// Nao ha senha digitada nem guardada.
    SemSenha,
    /// O certificado nao abriu (arquivo ou senha).
    Certificado(String),
    /// A senha digitada nao abre o certificado.
    SenhaIncorreta,
    /// O cofre nao respondeu.
    Cofre(ErroCofre),
    /// Falta o CNPJ/CPF ou o arquivo do certificado para identificar o item do cofre.
    SemChaveDoCofre,
    /// A pasta das notas nao pode ser usada.
    Pasta,
    /// A chamada de rede nao pode ser feita.
    Rede(String),
    /// A pasta das notas nao pode ser lida (o indice esta ilegivel ou em uso).
    LeituraDaPasta,
    /// A nota nao consta da pasta de notas.
    NotaAusente,
    /// O arquivo XML da nota nao esta na pasta ou nao pode ser lido.
    XmlAusente,
    /// O DANFSe nao pode ser gerado: a frase diz por que.
    Danfse(String),
    /// O arquivo nao pode ser gravado.
    GravacaoDoArquivo,
}

impl ErroTela {
    /// A frase para a tela.
    #[must_use]
    pub fn texto(&self) -> String {
        match self {
            Self::Configuracao(t) | Self::Valor(t) | Self::Certificado(t) | Self::Rede(t) => {
                t.clone()
            }
            Self::SemLocalDeConfiguracao => {
                "Não achei a pasta de configuração do usuário. Abra o programa com --config <arquivo>."
                    .into()
            }
            Self::Gravacao => "Não foi possível gravar o arquivo de configuração.".into(),
            Self::SemCertificado => "Escolha o arquivo do certificado.".into(),
            Self::SemSenha => {
                "Digite a senha do certificado (ou guarde-a no cofre) para continuar.".into()
            }
            Self::SenhaIncorreta => "Senha do certificado incorreta.".into(),
            Self::Cofre(e) => texto_do_cofre(*e),
            Self::SemChaveDoCofre => {
                "Informe o CNPJ ou o CPF, ou escolha o certificado, para identificar a senha no cofre."
                    .into()
            }
            Self::Pasta => "Não foi possível usar essa pasta para guardar as notas.".into(),
            Self::LeituraDaPasta => {
                "Não foi possível ler a pasta de notas. O índice pode estar em uso por outro programa ou danificado."
                    .into()
            }
            Self::NotaAusente => "Esta nota não consta da pasta de notas.".into(),
            Self::XmlAusente => {
                "O arquivo XML desta nota não está na pasta de notas ou não pôde ser lido.".into()
            }
            Self::Danfse(t) => t.clone(),
            Self::GravacaoDoArquivo => "Não foi possível gravar o arquivo.".into(),
        }
    }

    /// O campo da tela que originou o erro, se houver um.
    #[must_use]
    pub fn campo(&self) -> Option<Campo> {
        match self {
            Self::SemCertificado | Self::Certificado(_) => Some(Campo::Pfx),
            Self::SemSenha | Self::SenhaIncorreta => Some(Campo::Senha),
            Self::Pasta => Some(Campo::NotasPasta),
            _ => None,
        }
    }
}

impl std::fmt::Display for ErroTela {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.texto())
    }
}

impl std::error::Error for ErroTela {}

fn texto_do_cofre(e: ErroCofre) -> String {
    match e {
        ErroCofre::Desligado => {
            "O cofre de senhas está desligado (RUSTNF_COFRE=desligado): a senha não será guardada."
                .into()
        }
        ErroCofre::NaoSuportado => "Este sistema não tem cofre de senhas ligado ao rustnf.".into(),
        ErroCofre::ChaveInvalida => {
            "Informe o CNPJ ou o CPF, ou escolha o certificado, para identificar a senha no cofre."
                .into()
        }
        _ => "O cofre de senhas do Windows não respondeu.".into(),
    }
}

fn do_certificado(e: &Erro) -> ErroTela {
    let texto = e.to_string();
    if texto.starts_with("Senha do certificado incorreta") {
        ErroTela::SenhaIncorreta
    } else {
        ErroTela::Certificado(frase(&texto))
    }
}

/// O que foi lido do disco ao abrir a interface.
#[derive(Debug, Clone)]
pub struct Carga {
    /// A configuracao do arquivo (sem as variaveis de ambiente), o que a tela edita e grava. Pode ter valores que a
    /// conferencia recusa: a tela mostra o erro no campo.
    pub config: Config,
    /// O arquivo lido, se existia.
    pub arquivo: Option<PathBuf>,
    /// O arquivo ainda nao existe: sera criado ao salvar.
    pub arquivo_novo: bool,
    /// As variaveis `RUSTNF_*` que mudam algum valor em relacao ao arquivo.
    pub variaveis: Vec<&'static str>,
    /// O ambiente em vigor (arquivo com as variaveis por cima), o que a linha de comando usaria.
    pub ambiente: Ambiente,
}

/// Tudo para abrir o certificado e falar com a Sefin.
pub struct Preparo {
    certificado: Certificado,
    /// 1 producao, 2 producao restrita.
    pub ambiente: u8,
    /// Algoritmo da assinatura.
    pub algoritmo: Algoritmo,
    /// Tempo limite da chamada.
    pub tempo_limite: Duration,
}

/// Futuro do teste de conexao.
pub type FuturoConexao = Pin<Box<dyn Future<Output = (bool, String)> + Send + 'static>>;

/// Quem testa a conexao com a Sefin Nacional. O real chama `ClienteNfse::testar` (rede de verdade); os testes
/// trocam por um com transporte falso.
pub trait Conexao: Send + Sync {
    /// Confere o certificado e uma consulta de chave inexistente: resposta de negocio da Sefin vale como "ok".
    fn testar(&self, preparo: Preparo) -> FuturoConexao;
}

/// Futuro da busca de endereco pelo CEP.
pub type FuturoCep = Pin<
    Box<
        dyn Future<Output = Result<rustnf_app::cep::EnderecoDoCep, rustnf_app::cep::ErroCep>>
            + Send
            + 'static,
    >,
>;

/// Quem busca o endereco pelo CEP. O real chama o ViaCEP (e a BrasilAPI de reserva); os testes trocam por um falso.
pub trait BuscaCep: Send + Sync {
    /// Busca o CEP (8 digitos).
    fn buscar(&self, cep: String) -> FuturoCep;
}

/// A busca de CEP real, pela rede.
#[derive(Debug, Clone, Copy, Default)]
pub struct CepReal;

impl BuscaCep for CepReal {
    fn buscar(&self, cep: String) -> FuturoCep {
        Box::pin(async move { rustnf_app::cep::buscar(&cep, Duration::from_secs(8)).await })
    }
}

/// O teste de conexao real, com TLS mutuo.
#[derive(Debug, Clone, Copy, Default)]
pub struct ConexaoReal;

impl Conexao for ConexaoReal {
    fn testar(&self, preparo: Preparo) -> FuturoConexao {
        Box::pin(async move {
            match certificado::cliente(
                preparo.certificado,
                preparo.ambiente,
                preparo.algoritmo,
                preparo.tempo_limite,
            ) {
                Ok(cliente) => cliente.testar().await,
                Err(e) => (false, frase(&e.to_string())),
            }
        })
    }
}

impl Preparo {
    /// Monta o preparo com o transporte que o chamador quiser (os testes usam um falso).
    #[must_use]
    pub fn novo(
        certificado: Certificado,
        ambiente: u8,
        algoritmo: Algoritmo,
        tempo_limite: Duration,
    ) -> Self {
        Self {
            certificado,
            ambiente,
            algoritmo,
            tempo_limite,
        }
    }

    /// O certificado aberto.
    #[must_use]
    pub fn certificado(&self) -> &Certificado {
        &self.certificado
    }

    /// Entrega o certificado a quem vai montar o cliente.
    #[must_use]
    pub fn into_certificado(self) -> Certificado {
        self.certificado
    }
}

/// Os servicos da interface.
pub struct Servicos {
    explicito: Option<PathBuf>,
    env: LerAmbiente,
    cofre: Arc<dyn Cofre>,
    cofre_desligado: bool,
    conexao: Arc<dyn Conexao>,
    relogio: Relogio,
    fabrica: Arc<dyn FabricaDeEmissor>,
    abridor: Arc<dyn AbrirArquivos>,
    cep: Arc<dyn BuscaCep>,
    intervalo: Duration,
}

impl Servicos {
    /// Os servicos de verdade: ambiente do processo, cofre do sistema (ou nenhum, com `RUSTNF_COFRE=desligado`),
    /// relogio do sistema e conexao real.
    #[must_use]
    pub fn do_sistema(config: Option<PathBuf>) -> Self {
        let desligado = std::env::var(VARIAVEL_COFRE).is_ok_and(|v| v == "desligado");
        Self {
            explicito: config,
            env: Arc::new(|nome| std::env::var_os(nome)),
            cofre: Arc::from(cofre_do_ambiente()),
            cofre_desligado: desligado,
            conexao: Arc::new(ConexaoReal),
            relogio: Arc::new(Utc::now),
            fabrica: Arc::new(FabricaReal),
            abridor: Arc::new(DoSistema),
            cep: Arc::new(CepReal),
            intervalo: INTERVALO_PADRAO,
        }
    }

    /// Troca a busca de CEP (os testes usam uma falsa, sem rede).
    #[must_use]
    pub fn com_busca_cep(mut self, cep: Arc<dyn BuscaCep>) -> Self {
        self.cep = cep;
        self
    }

    /// Quem busca o endereco pelo CEP.
    #[must_use]
    pub fn busca_cep(&self) -> Arc<dyn BuscaCep> {
        Arc::clone(&self.cep)
    }

    /// Os servicos com tudo trocavel, para os testes.
    #[must_use]
    pub fn com(
        config: Option<PathBuf>,
        env: LerAmbiente,
        cofre: Arc<dyn Cofre>,
        cofre_desligado: bool,
        conexao: Arc<dyn Conexao>,
        relogio: Relogio,
    ) -> Self {
        Self {
            explicito: config,
            env,
            cofre,
            cofre_desligado,
            conexao,
            relogio,
            fabrica: Arc::new(FabricaSemEmissor),
            abridor: Arc::new(DoSistema),
            cep: Arc::new(CepReal),
            intervalo: INTERVALO_PADRAO,
        }
    }

    /// O cofre esta desligado por `RUSTNF_COFRE=desligado`: a senha nao e' guardada.
    #[must_use]
    pub fn cofre_desligado(&self) -> bool {
        self.cofre_desligado
    }

    /// O instante de agora.
    #[must_use]
    pub fn agora(&self) -> DateTime<Utc> {
        (self.relogio)()
    }

    /// Quem testa a conexao.
    #[must_use]
    pub fn conexao(&self) -> Arc<dyn Conexao> {
        Arc::clone(&self.conexao)
    }

    /// O arquivo de configuracao em uso: `--config`, `RUSTNF_CONFIG` ou o local padrao.
    #[must_use]
    pub fn local_do_arquivo(&self) -> Option<PathBuf> {
        caminho_do_arquivo(self.explicito.as_deref(), &*self.env)
    }

    /// Le o arquivo de configuracao. Arquivo indicado que ainda nao existe nao e' erro: a tela comeca vazia e o
    /// cria ao salvar. Sintaxe errada e arquivo ilegivel sao erro.
    ///
    /// # Errors
    ///
    /// [`ErroTela::Configuracao`] se o arquivo existe e nao pode ser lido.
    pub fn carregar(&self) -> Result<Carga, ErroTela> {
        let env = &*self.env;
        let (config, arquivo, novo) = match Config::carregar_com(self.explicito.as_deref(), env) {
            Ok((c, a)) => {
                let novo = a.is_none();
                (c, a, novo)
            }
            Err(ErroConfig::NaoEncontrado(Indicado::Opcao | Indicado::Variavel)) => {
                (Config::default(), None, true)
            }
            Err(e) => return Err(ErroTela::Configuracao(texto_da_configuracao(&e))),
        };
        let (ambiente, variaveis) = match Efetiva::de(config.clone(), arquivo.clone(), env) {
            Ok(e) => (Ambiente::de_config(&e.config.sefin.ambiente), e.variaveis),
            Err(_) => (Ambiente::de_config(&config.sefin.ambiente), Vec::new()),
        };
        Ok(Carga {
            config,
            arquivo,
            arquivo_novo: novo,
            variaveis,
            ambiente,
        })
    }

    /// Confere e grava a configuracao (de forma atomica) no arquivo em uso.
    ///
    /// # Errors
    ///
    /// [`ErroTela::SemLocalDeConfiguracao`], [`ErroTela::Valor`] ou [`ErroTela::Gravacao`].
    pub fn salvar(&self, config: &Config) -> Result<(), ErroTela> {
        let destino = self
            .local_do_arquivo()
            .ok_or(ErroTela::SemLocalDeConfiguracao)?;
        config.salvar(&destino).map_err(|e| match e {
            ErroConfig::Invalido { .. } => ErroTela::Valor(texto_da_configuracao(&e)),
            _ => ErroTela::Gravacao,
        })
    }

    /// A configuracao que vale de verdade: o arquivo com as variaveis `RUSTNF_*` por cima, como a linha de comando a
    /// usaria. Se alguma variavel nao serve, vale o arquivo.
    #[must_use]
    pub fn efetiva(&self, config: &Config) -> Config {
        Efetiva::de(config.clone(), self.local_do_arquivo(), &*self.env)
            .map_or_else(|_| config.clone(), |e| e.config)
    }

    /// Confere que a pasta das notas serve (cria se faltar).
    ///
    /// # Errors
    ///
    /// [`ErroTela::Pasta`].
    pub fn conferir_pasta(&self, pasta: &str) -> Result<(), ErroTela> {
        Notas::abrir(pasta).map(|_| ()).map_err(|_| ErroTela::Pasta)
    }

    fn chave(config: &Config) -> Option<String> {
        chave_do_certificado(&config.emitente, &config.sefin.pfx)
    }

    /// `Some(true)` se ha senha no cofre para o certificado, `Some(false)` se nao ha, `None` se o cofre nao
    /// respondeu (ou esta desligado).
    #[must_use]
    pub fn senha_guardada(&self, config: &Config) -> Option<bool> {
        let chave = Self::chave(config)?;
        match self.cofre.ler(&chave) {
            Ok(s) => Some(s.is_some()),
            Err(_) => None,
        }
    }

    /// A senha para abrir o certificado: a digitada, ou a do cofre.
    fn senha(&self, config: &Config, digitada: Option<&Segredo>) -> Result<Segredo, ErroTela> {
        if let Some(s) = digitada.filter(|s| !s.expor().is_empty()) {
            return Ok(Segredo::novo(s.expor().to_string()));
        }
        Self::chave(config)
            .and_then(|chave| self.cofre.ler(&chave).ok().flatten())
            .ok_or(ErroTela::SemSenha)
    }

    fn abrir(&self, config: &Config, digitada: Option<&Segredo>) -> Result<Certificado, ErroTela> {
        let pfx = config.sefin.pfx.trim();
        if pfx.is_empty() {
            return Err(ErroTela::SemCertificado);
        }
        let senha = self.senha(config, digitada)?;
        certificado::abrir(Path::new(pfx), &senha).map_err(|e| do_certificado(&e))
    }

    /// O estado do certificado configurado, para o rodape: arquivo, senha no cofre e validade.
    #[must_use]
    pub fn estado_do_certificado(&self, config: &Config) -> EstadoCertificado {
        let pfx = config.sefin.pfx.trim();
        if pfx.is_empty() {
            return EstadoCertificado::SemCertificado;
        }
        if !Path::new(pfx).is_file() {
            return EstadoCertificado::ArquivoNaoEncontrado;
        }
        match self.abrir(config, None) {
            Ok(c) => avaliar(&c, self.agora()),
            Err(ErroTela::SemSenha) => EstadoCertificado::SenhaNaoGuardada,
            Err(ErroTela::SenhaIncorreta) => EstadoCertificado::SenhaNaoConfere,
            Err(_) => EstadoCertificado::Ilegivel,
        }
    }

    /// O botao "Testar certificado": abre o `.pfx` com a senha digitada (ou a do cofre) e diz titular, documento e
    /// validade. Nao fala com a rede.
    ///
    /// # Errors
    ///
    /// [`ErroTela::SemCertificado`], [`ErroTela::SemSenha`], [`ErroTela::SenhaIncorreta`] ou
    /// [`ErroTela::Certificado`].
    pub fn testar_certificado(
        &self,
        config: &Config,
        digitada: Option<&Segredo>,
    ) -> Result<InfoCertificado, ErroTela> {
        let c = self.abrir(config, digitada)?;
        let doc = if config.emitente.cnpj.is_empty() {
            config.emitente.cpf.as_str()
        } else {
            config.emitente.cnpj.as_str()
        };
        Ok(testar(
            &c,
            Some(doc).filter(|d| !d.is_empty()),
            self.agora(),
        ))
    }

    /// Confere que a senha abre o certificado e a guarda no cofre.
    ///
    /// # Errors
    ///
    /// [`ErroTela::Cofre`] se o cofre estiver desligado ou nao responder; os de [`Servicos::testar_certificado`].
    pub fn guardar_senha(&self, config: &Config, senha: &Segredo) -> Result<(), ErroTela> {
        if self.cofre_desligado {
            return Err(ErroTela::Cofre(ErroCofre::Desligado));
        }
        let chave = Self::chave(config).ok_or(ErroTela::SemChaveDoCofre)?;
        if senha.expor().is_empty() {
            return Err(ErroTela::SemSenha);
        }
        self.abrir(config, Some(senha))?;
        self.cofre.guardar(&chave, senha).map_err(ErroTela::Cofre)
    }

    /// Apaga a senha do cofre.
    ///
    /// # Errors
    ///
    /// [`ErroTela::SemChaveDoCofre`] ou [`ErroTela::Cofre`].
    pub fn apagar_senha(&self, config: &Config) -> Result<(), ErroTela> {
        let chave = Self::chave(config).ok_or(ErroTela::SemChaveDoCofre)?;
        self.cofre.apagar(&chave).map_err(ErroTela::Cofre)
    }

    /// Le o rascunho da Nova nota, na pasta de notas. Sem pasta, sem arquivo, arquivo ilegivel ou de versao
    /// desconhecida: `None` (a tela comeca vazia, sem erro).
    #[must_use]
    pub fn ler_rascunho(&self, config: &Config) -> Option<DadosNota> {
        use std::io::Read as _;
        let pasta = config.notas.pasta.trim();
        if pasta.is_empty() {
            return None;
        }
        let arquivo = std::fs::File::open(Path::new(pasta).join(rascunho::ARQUIVO)).ok()?;
        let mut bytes = Vec::new();
        arquivo
            .take(rascunho::LIMITE + 1)
            .read_to_end(&mut bytes)
            .ok()?;
        if bytes.len() as u64 > rascunho::LIMITE {
            return None;
        }
        rascunho::decodificar(&bytes)
    }

    /// Grava o rascunho da Nova nota na pasta de notas, de forma atomica.
    ///
    /// # Errors
    ///
    /// [`ErroTela::Pasta`] se nao ha pasta ou nao foi possivel gravar.
    pub fn gravar_rascunho(&self, config: &Config, dados: &DadosNota) -> Result<(), ErroTela> {
        let pasta = config.notas.pasta.trim();
        if pasta.is_empty() {
            return Err(ErroTela::Pasta);
        }
        rustnf_app::atomico::gravar(
            &Path::new(pasta).join(rascunho::ARQUIVO),
            &rascunho::codificar(dados),
        )
        .map_err(|_| ErroTela::Pasta)
    }

    /// Apaga o rascunho (nao e' erro se ele nao existe).
    pub fn apagar_rascunho(&self, config: &Config) {
        let pasta = config.notas.pasta.trim();
        if !pasta.is_empty() {
            let _ = std::fs::remove_file(Path::new(pasta).join(rascunho::ARQUIVO));
        }
    }

    /// Abre o certificado e reune o que o teste de conexao precisa.
    ///
    /// # Errors
    ///
    /// Os de [`Servicos::testar_certificado`].
    pub fn preparar_conexao(
        &self,
        config: &Config,
        digitada: Option<&Segredo>,
    ) -> Result<Preparo, ErroTela> {
        let c = self.abrir(config, digitada)?;
        Ok(Preparo::novo(
            c,
            Ambiente::de_config(&config.sefin.ambiente).codigo(),
            config.sefin.algoritmo(),
            Duration::from_secs(config.sefin.tempo_limite_s),
        ))
    }
}

/// Texto da configuracao para a tela, sem o prefixo "configuracao:" das mensagens da biblioteca.
fn texto_da_configuracao(e: &ErroConfig) -> String {
    match e {
        ErroConfig::Leitura => {
            "Não foi possível ler o arquivo de configuração (ou ele passa de 1 MiB).".into()
        }
        ErroConfig::Sintaxe {
            linha: Some(l),
            mensagem,
        } => format!("O arquivo de configuração tem um erro na linha {l}: {mensagem}."),
        ErroConfig::Sintaxe {
            linha: None,
            mensagem,
        } => format!("O arquivo de configuração tem um erro: {mensagem}."),
        ErroConfig::Invalido {
            secao,
            campo,
            motivo,
        } => format!("Valor inválido em [{secao}] {campo}: {motivo}."),
        ErroConfig::Variavel { nome, motivo } => format!("Variável {nome}: {motivo}."),
        ErroConfig::Gravacao => "Não foi possível gravar o arquivo de configuração.".into(),
        _ => frase(&e.to_string()),
    }
}

#[cfg(test)]
mod testes;
#[cfg(test)]
mod testes_emissao;
