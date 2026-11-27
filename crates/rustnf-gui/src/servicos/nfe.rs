// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A NF-e (modelo 55) fora da tela: ler a pasta de notas, gerar o DANFE e falar com a SEFAZ. O que a SEFAZ responde
//! passa pela camada de aplicacao (`rustnf_app::nfe`), a mesma da linha de comando; aqui so' se monta o cliente (com o
//! transporte que a fabrica der: o de verdade, ou o roteirizado da demonstracao e dos testes), se abre o certificado e
//! se traduz o resultado em tipos da tela. Tudo bloqueia (disco, PDF) ou fala com a rede: quem chama roda fora da linha
//! da tela. Nenhum teste toca a rede, o cofre nem a pasta do usuario.

use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use rustnf::assinatura::cert_a1::Certificado;
use rustnf::nfe::montagem::OpcoesQr;
use rustnf::nfe::sefaz::{ClienteSefaz, FalhaRede, Transporte, TransporteReqwest};
use rustnf_app::config::Config;
use rustnf_app::nfe::{self as app, Contexto};
use rustnf_app::notas::nfe::{NotasNfe, Registro};

use super::{Documento, ErroTela, Preparo, Servicos};
use crate::modelo::mascara::frase;
use crate::modelo::nfe_config::ResultadoSefaz;
use crate::modelo::nfe_detalhe::LeituraNfe;
use crate::modelo::nfe_eventos::{DesfechoNfe, Faixa, PedidoNfe};
use crate::modelo::nfe_lista::{
    codificar_escolha, decodificar_escolha, EscolhaDeNotas, ARQUIVO_DE_PREFERENCIAS,
    LIMITE_DAS_PREFERENCIAS,
};
use crate::modelo::nfe_resultados::{ConsultaNfe, ResumoDaRetomada};
use rustnf_app::notas::nfe::Situacao;

/// O transporte que a fabrica entregou, como o tipo que o cliente da SEFAZ pede.
#[derive(Clone)]
pub struct TransporteDin(Arc<dyn Transporte>);

#[async_trait]
impl Transporte for TransporteDin {
    async fn enviar(
        &self,
        url: &str,
        corpo: Vec<u8>,
        content_type: &str,
    ) -> Result<(u16, Vec<u8>), FalhaRede> {
        self.0.enviar(url, corpo, content_type).await
    }
}

impl TransporteDin {
    /// O transporte que a fabrica entregou.
    #[must_use]
    pub fn de(transporte: Arc<dyn Transporte>) -> Self {
        Self(transporte)
    }
}

/// O cliente da SEFAZ que as telas usam.
pub type ClienteNfe = ClienteSefaz<TransporteDin>;

/// Quem entrega o transporte da NF-e. O real e' o `reqwest` com TLS mutuo pelo certificado; os testes e a demonstracao
/// trocam por um roteirizado, que nunca toca a rede.
pub trait FabricaNfe: Send + Sync {
    /// O transporte para o certificado e a configuracao.
    ///
    /// # Errors
    ///
    /// [`ErroTela::Rede`] se o certificado ou a raiz de confianca nao servem.
    fn transporte(
        &self,
        certificado: &Certificado,
        config: &Config,
    ) -> Result<Arc<dyn Transporte>, ErroTela>;

    /// Ajusta o cliente montado (a demonstracao e os testes tiram as esperas); o de verdade nao mexe.
    fn ajustar(&self, cliente: ClienteNfe) -> ClienteNfe {
        cliente
    }

    /// E' a demonstracao: nada e' enviado de verdade.
    fn demonstracao(&self) -> bool {
        false
    }
}

/// A fabrica de verdade.
#[derive(Debug, Clone, Copy, Default)]
pub struct FabricaNfeReal;

impl FabricaNfe for FabricaNfeReal {
    fn transporte(
        &self,
        certificado: &Certificado,
        config: &Config,
    ) -> Result<Arc<dyn Transporte>, ErroTela> {
        let ca = app::ler_ca_pem(config).map_err(|e| ErroTela::Rede(frase(&e.to_string())))?;
        let t = TransporteReqwest::novo(
            certificado,
            ca.as_deref(),
            Duration::from_secs(config.nfe.tempo_limite_s),
        )
        .map_err(|e| ErroTela::Rede(frase(&e)))?;
        Ok(Arc::new(t))
    }
}

/// A fabrica de quem nao fala com a SEFAZ: toda tentativa volta com um erro. E' o que `Servicos::com` usa ate alguem
/// trocar.
#[derive(Debug, Clone, Copy, Default)]
pub struct FabricaNfeSemTransporte;

impl FabricaNfe for FabricaNfeSemTransporte {
    fn transporte(
        &self,
        _certificado: &Certificado,
        _config: &Config,
    ) -> Result<Arc<dyn Transporte>, ErroTela> {
        Err(ErroTela::Rede(
            "Este conjunto de serviços não fala com a SEFAZ.".into(),
        ))
    }
}

/// Uma NF-e da pasta, com o que se leu do XML guardado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotaNfeDaPasta {
    /// A linha do indice.
    pub registro: Registro,
    /// O que a leitura do XML deu.
    pub leitura: LeituraNfe,
}

impl Servicos {
    /// Troca a fabrica do transporte da NF-e (os testes e a demonstracao).
    #[must_use]
    pub fn com_fabrica_nfe(mut self, fabrica: Arc<dyn FabricaNfe>) -> Self {
        self.fabrica_nfe = fabrica;
        self
    }

    /// A fabrica do transporte da NF-e.
    #[must_use]
    pub fn fabrica_nfe(&self) -> Arc<dyn FabricaNfe> {
        Arc::clone(&self.fabrica_nfe)
    }

    /// A NF-e esta na demonstracao: nada e' enviado de verdade.
    #[must_use]
    pub fn demonstracao_nfe(&self) -> bool {
        self.fabrica_nfe.demonstracao()
    }

    pub(super) fn notas_nfe(&self, config: &Config) -> Result<NotasNfe, ErroTela> {
        NotasNfe::abrir(config.notas.pasta.trim()).map_err(|_| ErroTela::Pasta)
    }

    /// As NF-e do indice da pasta, da mais recente para a mais antiga.
    ///
    /// # Errors
    ///
    /// [`ErroTela::Pasta`] ou [`ErroTela::LeituraDaPasta`].
    pub fn listar_nfe(&self, config: &Config) -> Result<Vec<Registro>, ErroTela> {
        self.notas_nfe(config)?
            .listar()
            .map_err(|_| ErroTela::LeituraDaPasta)
    }

    /// A NF-e da pasta com o que o XML guardado traz. XML ausente ou ilegivel nao e' erro: a leitura diz isso.
    ///
    /// # Errors
    ///
    /// [`ErroTela::Pasta`] ou [`ErroTela::NotaAusente`].
    pub fn ler_nfe(&self, config: &Config, chave: &str) -> Result<NotaNfeDaPasta, ErroTela> {
        let notas = self.notas_nfe(config)?;
        let registro = notas.registro(chave).map_err(|_| ErroTela::NotaAusente)?;
        let xml = notas.ler_xml(chave).ok();
        let leitura = LeituraNfe::do_xml(xml.as_deref(), &registro);
        Ok(NotaNfeDaPasta { registro, leitura })
    }

    /// Gera o DANFE da nota da pasta (com a marca de cancelada e o canhoto da configuracao) e o grava ao lado do XML
    /// (de forma atomica). Devolve onde ficou.
    ///
    /// # Errors
    ///
    /// [`ErroTela::NotaAusente`], [`ErroTela::Danfse`] ou [`ErroTela::GravacaoDoArquivo`].
    pub fn danfe_da_pasta(&self, config: &Config, chave: &str) -> Result<PathBuf, ErroTela> {
        let notas = self.notas_nfe(config)?;
        let pdf = app::danfe_da_pasta(&notas, config, chave).map_err(|e| match e {
            rustnf_app::erro::Erro::NaoEncontrada(_) => ErroTela::NotaAusente,
            outro => ErroTela::Danfse(frase(&outro.to_string())),
        })?;
        notas
            .guardar_danfe(chave, &pdf)
            .map_err(|_| ErroTela::GravacaoDoArquivo)
    }

    /// Onde esta o `nfeProc` guardado de uma nota (para "Mostrar na pasta").
    ///
    /// # Errors
    ///
    /// [`ErroTela::NotaAusente`] ou [`ErroTela::XmlAusente`] (a nota sem resposta so' tem a NF-e assinada).
    pub fn caminho_do_xml_nfe(&self, config: &Config, chave: &str) -> Result<PathBuf, ErroTela> {
        let caminho = self
            .notas_nfe(config)?
            .caminho_do_proc(chave)
            .map_err(|_| ErroTela::NotaAusente)?;
        if caminho.is_file() {
            Ok(caminho)
        } else {
            Err(ErroTela::XmlAusente)
        }
    }

    /// Abre o certificado e reune o que as chamadas a SEFAZ precisam.
    ///
    /// # Errors
    ///
    /// Os de [`Servicos::testar_certificado`].
    pub fn preparar_nfe(
        &self,
        config: &Config,
        digitada: Option<&rustnf_app::segredo::Segredo>,
    ) -> Result<Preparo, ErroTela> {
        self.preparar_conexao(config, digitada)
    }

    /// A escolha "NFS-e | NF-e" que a pessoa fez da ultima vez. Sem pasta, sem arquivo ou arquivo ruim: o padrao.
    #[must_use]
    pub fn ler_escolha_de_notas(&self, config: &Config) -> EscolhaDeNotas {
        let pasta = config.notas.pasta.trim();
        if pasta.is_empty() {
            return EscolhaDeNotas::default();
        }
        let Ok(arquivo) = std::fs::File::open(Path::new(pasta).join(ARQUIVO_DE_PREFERENCIAS))
        else {
            return EscolhaDeNotas::default();
        };
        let mut bytes = Vec::new();
        if arquivo
            .take(LIMITE_DAS_PREFERENCIAS + 1)
            .read_to_end(&mut bytes)
            .is_err()
            || bytes.len() as u64 > LIMITE_DAS_PREFERENCIAS
        {
            return EscolhaDeNotas::default();
        }
        decodificar_escolha(&bytes).unwrap_or_default()
    }

    /// Guarda a escolha na pasta de notas (de forma atomica). Falhar nao e' erro para a pessoa: a escolha so' nao e'
    /// lembrada.
    pub fn gravar_escolha_de_notas(&self, config: &Config, escolha: EscolhaDeNotas) {
        let pasta = config.notas.pasta.trim();
        if !pasta.is_empty() {
            let _ = rustnf_app::atomico::gravar(
                &Path::new(pasta).join(ARQUIVO_DE_PREFERENCIAS),
                &codificar_escolha(escolha),
            );
        }
    }
}

/// O cliente, o certificado e a pasta de uma chamada.
pub(super) struct Sessao {
    pub(super) cliente: ClienteNfe,
    pub(super) certificado: Certificado,
    pub(super) config: Config,
    pub(super) notas: Option<NotasNfe>,
    pub(super) qr: OpcoesQr,
}

impl Sessao {
    pub(super) fn contexto(&self) -> Contexto<'_, TransporteDin> {
        Contexto::novo(
            &self.cliente,
            &self.certificado,
            &self.config,
            self.notas.as_ref(),
        )
        .com_qr(self.qr.clone())
    }
}

pub(super) fn montar(
    fabrica: &Arc<dyn FabricaNfe>,
    preparo: Preparo,
    config: Config,
) -> Result<Sessao, ErroTela> {
    montar_doc(Documento::Nfe, fabrica, preparo, config, None)
}

/// A sessao do documento: a NFC-e usa a configuracao de `Config::como_nfce` (ambiente e serie dela), a pasta `nfce/` e,
/// na emissao, as opcoes do QR Code (versao e CSC).
pub(super) fn montar_doc(
    doc: Documento,
    fabrica: &Arc<dyn FabricaNfe>,
    preparo: Preparo,
    config: Config,
    qr: Option<OpcoesQr>,
) -> Result<Sessao, ErroTela> {
    let config = match doc {
        Documento::Nfe => config,
        Documento::Nfce => config.como_nfce(),
    };
    let certificado = preparo.into_certificado();
    let transporte = fabrica.transporte(&certificado, &config)?;
    let sefaz = app::config_sefaz(&config).map_err(|e| ErroTela::Valor(frase(&e.to_string())))?;
    let cliente = ClienteSefaz::com_transporte(sefaz, TransporteDin(transporte))
        .map_err(|e| ErroTela::Rede(frase(&e.to_string())))?;
    let pasta = config.notas.pasta.trim();
    let notas = match doc {
        Documento::Nfe => NotasNfe::abrir(pasta),
        Documento::Nfce => NotasNfe::abrir_nfce(pasta),
    }
    .ok();
    Ok(Sessao {
        cliente: fabrica.ajustar(cliente),
        certificado,
        config,
        notas,
        qr: qr.unwrap_or_default(),
    })
}

/// "Testar conexao com a SEFAZ": pergunta o estado do servico. So' pergunta: nada e' emitido.
pub async fn testar_sefaz(
    fabrica: Arc<dyn FabricaNfe>,
    preparo: Preparo,
    config: Config,
) -> ResultadoSefaz {
    let s = match montar(&fabrica, preparo, config) {
        Ok(s) => s,
        Err(e) => {
            return ResultadoSefaz::Recusado { texto: e.texto() };
        }
    };
    let producao = s.config.nfe.ambiente == "producao";
    match app::status(&s.cliente, s.config.nfe.em_contingencia()).await {
        Ok(st) => ResultadoSefaz::do_status(&st, s.cliente.uf(), producao),
        Err(rustnf_app::erro::Erro::Rede(_)) => ResultadoSefaz::SemResposta,
        Err(e) => ResultadoSefaz::Recusado {
            texto: frase(&e.mensagem()),
        },
    }
}

/// "Conferir na SEFAZ": consulta a nota pela chave e poe a pasta em dia com o que a SEFAZ diz.
pub async fn consultar_nfe(
    fabrica: Arc<dyn FabricaNfe>,
    preparo: Preparo,
    config: Config,
    chave: String,
) -> ConsultaNfe {
    consultar_doc(Documento::Nfe, fabrica, preparo, config, chave).await
}

/// "Conferir na SEFAZ" da NF-e ou da NFC-e.
pub async fn consultar_doc(
    doc: Documento,
    fabrica: Arc<dyn FabricaNfe>,
    preparo: Preparo,
    config: Config,
    chave: String,
) -> ConsultaNfe {
    let s = match montar_doc(doc, &fabrica, preparo, config, None) {
        Ok(s) => s,
        Err(e) => {
            return ConsultaNfe::do_resultado(Err(rustnf_app::erro::Erro::Uso(e.texto())));
        }
    };
    ConsultaNfe::do_resultado(app::consultar(&s.contexto(), &chave).await)
}

/// Cancela a NF-e.
pub async fn cancelar_nfe(
    fabrica: Arc<dyn FabricaNfe>,
    preparo: Preparo,
    config: Config,
    chave: String,
    justificativa: String,
) -> DesfechoNfe {
    cancelar_doc(
        Documento::Nfe,
        fabrica,
        preparo,
        config,
        chave,
        justificativa,
    )
    .await
}

/// Cancela a NF-e ou a NFC-e (o aviso do prazo da NFC-e e' dado pela tela ANTES de chamar, sem bloquear).
pub async fn cancelar_doc(
    doc: Documento,
    fabrica: Arc<dyn FabricaNfe>,
    preparo: Preparo,
    config: Config,
    chave: String,
    justificativa: String,
) -> DesfechoNfe {
    let s = match montar_doc(doc, &fabrica, preparo, config, None) {
        Ok(s) => s,
        Err(e) => return recusado(&e),
    };
    DesfechoNfe::do_evento(
        app::cancelar(&s.contexto(), &chave, &justificativa, None).await,
        PedidoNfe::Cancelar,
    )
}

/// Registra a carta de correcao.
pub async fn corrigir_nfe(
    fabrica: Arc<dyn FabricaNfe>,
    preparo: Preparo,
    config: Config,
    chave: String,
    correcao: String,
) -> DesfechoNfe {
    let s = match montar(&fabrica, preparo, config) {
        Ok(s) => s,
        Err(e) => return recusado(&e),
    };
    DesfechoNfe::do_evento(
        app::corrigir(&s.contexto(), &chave, &correcao, None).await,
        PedidoNfe::Corrigir,
    )
}

/// Inutiliza a faixa de numeros.
pub async fn inutilizar_nfe(
    fabrica: Arc<dyn FabricaNfe>,
    preparo: Preparo,
    config: Config,
    faixa: Faixa,
    justificativa: String,
) -> DesfechoNfe {
    let s = match montar(&fabrica, preparo, config) {
        Ok(s) => s,
        Err(e) => return recusado(&e),
    };
    DesfechoNfe::da_inutilizacao(
        app::inutilizar(
            &s.contexto(),
            faixa.serie.as_deref(),
            faixa.inicio,
            faixa.fim,
            &justificativa,
        )
        .await,
    )
}

/// Depois de um pedido sem resposta: consulta a nota e diz se o cancelamento esta registrado. So' o cancelamento tem
/// uma resposta definitiva na consulta (a nota cancelada); a carta de correcao nao aparece nela.
pub async fn conferir_cancelamento_nfe(
    fabrica: Arc<dyn FabricaNfe>,
    preparo: Preparo,
    config: Config,
    chave: String,
) -> DesfechoNfe {
    conferir_cancelamento_doc(Documento::Nfe, fabrica, preparo, config, chave).await
}

/// Confere o cancelamento sem resposta da NF-e ou da NFC-e.
pub async fn conferir_cancelamento_doc(
    doc: Documento,
    fabrica: Arc<dyn FabricaNfe>,
    preparo: Preparo,
    config: Config,
    chave: String,
) -> DesfechoNfe {
    let s = match montar_doc(doc, &fabrica, preparo, config, None) {
        Ok(s) => s,
        Err(e) => return recusado(&e),
    };
    match app::consultar(&s.contexto(), &chave).await {
        Ok(c) if c.situacao == Some(Situacao::Cancelada) => DesfechoNfe::Registrado {
            protocolo: String::new(),
            recuperado: true,
            aviso: None,
        },
        Ok(c) if c.situacao == Some(Situacao::Autorizada) => DesfechoNfe::NaoRegistrado,
        Ok(_) | Err(rustnf_app::erro::Erro::Rede(_)) => DesfechoNfe::SemResposta {
            texto: "A SEFAZ não respondeu à conferência. Tente de novo daqui a pouco.".into(),
        },
        Err(e) => DesfechoNfe::do_erro(&e),
    }
}

/// "Retomar": retoma as notas enviadas sem resposta (consulta a mesma chave; so' reenvia a mesma NF-e assinada).
pub async fn retomar_nfe(
    fabrica: Arc<dyn FabricaNfe>,
    preparo: Preparo,
    config: Config,
) -> ResumoDaRetomada {
    retomar_doc(Documento::Nfe, fabrica, preparo, config).await
}

/// "Retomar" da NF-e ou da NFC-e: a NFC-e off-line tambem e' transmitida aqui, com a MESMA chave.
pub async fn retomar_doc(
    doc: Documento,
    fabrica: Arc<dyn FabricaNfe>,
    preparo: Preparo,
    config: Config,
) -> ResumoDaRetomada {
    let s = match montar_doc(doc, &fabrica, preparo, config, None) {
        Ok(s) => s,
        Err(e) => {
            return ResumoDaRetomada::do_resultado(Err(rustnf_app::erro::Erro::Uso(e.texto())));
        }
    };
    ResumoDaRetomada::do_resultado(app::retomar(&s.contexto()).await)
}

pub(super) fn recusado(e: &ErroTela) -> DesfechoNfe {
    DesfechoNfe::Recusado { texto: e.texto() }
}
