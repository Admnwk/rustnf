// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A NFC-e (modelo 65, o cupom do caixa) fora da tela: o CSC no cofre, a pasta `nfce/`, o cupom (DANFC-e) como arquivo, o
//! caixa (contexto, rascunho, pendentes) e a emissao. O que a SEFAZ responde passa pela camada de aplicacao
//! (`rustnf_app::nfce` e `rustnf_app::nfe`), a mesma da linha de comando; aqui se monta o cliente, se abre o certificado e se
//! traduz o resultado em tipos da tela. Tudo bloqueia (disco, cofre, PDF) ou fala com a rede: quem chama roda fora da linha da
//! tela. O VALOR do CSC so' passa daqui para o cofre e para o QR Code: nunca para arquivo, log, tela ou erro. Nenhum teste toca
//! a rede, o cofre do sistema nem a pasta do usuario.

use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::{FixedOffset, Offset as _};
use rustnf::nfe::montagem::OpcoesQr;
use rustnf_app::config::Config;
use rustnf_app::erro::Erro;
use rustnf_app::nfce::{self, danfce, Modo, Pendente};
use rustnf_app::notas::nfe::NotasNfe;
use rustnf_app::segredo::Segredo;
use serde_json::Value;

use super::nfe::{montar_doc, NotaNfeDaPasta};
use super::{ErroTela, FabricaNfe, Preparo, Servicos};
use crate::modelo::mascara::frase;
use crate::modelo::nfce_caixa::Cupom;
use crate::modelo::nfce_config::{csc_parece_valido, EstadoCsc};
use crate::modelo::nfce_desfecho::DesfechoCaixa;
use crate::modelo::nfce_rascunho;
use crate::modelo::nfe_detalhe::LeituraNfe;
use crate::modelo::nfe_nova::ContextoNfe;

/// Qual documento do modelo 55/65 uma chamada trata: a NF-e (pasta `nfe/`) ou a NFC-e (pasta `nfce/`, configuracao de
/// `Config::como_nfce`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Documento {
    /// NF-e, modelo 55.
    #[default]
    Nfe,
    /// NFC-e, modelo 65.
    Nfce,
}

impl Documento {
    /// O nome do documento na tela.
    #[must_use]
    pub fn nome(self) -> &'static str {
        match self {
            Self::Nfe => "NF-e",
            Self::Nfce => "NFC-e",
        }
    }
}

fn do_erro(e: &Erro) -> ErroTela {
    match e {
        Erro::NaoEncontrada(_) => ErroTela::NotaAusente,
        outro => ErroTela::Valor(frase(&outro.mensagem())),
    }
}

impl Servicos {
    // ---------------------------------------------------------------- CSC

    /// O que se sabe do CSC no cofre (nunca o valor). Com o cofre desligado, `SemCofre`.
    #[must_use]
    pub fn estado_do_csc(&self, config: &Config) -> EstadoCsc {
        if self.cofre_desligado {
            return EstadoCsc::SemCofre;
        }
        match nfce::csc::existe(&*self.cofre, config) {
            Ok(true) => EstadoCsc::Guardado,
            Ok(false) => EstadoCsc::NaoGuardado,
            Err(Erro::Uso(_)) => EstadoCsc::SemIdCsc,
            Err(_) => EstadoCsc::CofreFora,
        }
    }

    /// Confere o CSC e o guarda no cofre, para o emitente, o ambiente e o `idCSC` da configuracao.
    ///
    /// # Errors
    ///
    /// [`ErroTela::Cofre`] com o cofre desligado ou fora; [`ErroTela::Valor`] se o CSC ou o `idCSC` nao servem (a frase nao
    /// repete o CSC).
    pub fn guardar_csc(&self, config: &Config, csc: &Segredo) -> Result<(), ErroTela> {
        if self.cofre_desligado {
            return Err(ErroTela::Cofre(rustnf_app::cofre::ErroCofre::Desligado));
        }
        if !csc_parece_valido(csc.expor()) {
            return Err(ErroTela::Valor(
                "O CSC tem de ter de 16 a 36 letras e números, como a SEFAZ o entregou.".into(),
            ));
        }
        nfce::csc::guardar(&*self.cofre, config, csc).map_err(|e| do_erro(&e))
    }

    /// Apaga o CSC do cofre (CSC ausente nao e' erro).
    ///
    /// # Errors
    ///
    /// [`ErroTela::Valor`] se faltam o documento do emitente ou o `idCSC`.
    pub fn apagar_csc(&self, config: &Config) -> Result<(), ErroTela> {
        nfce::csc::apagar(&*self.cofre, config).map_err(|e| do_erro(&e))
    }

    /// As opcoes do QR Code para emitir: a versao 3 (padrao) dispensa o CSC; a versao 2 o le do cofre.
    ///
    /// # Errors
    ///
    /// [`ErroTela::Valor`] com a versao 2 sem CSC guardado.
    pub fn opcoes_qr_nfce(&self, config: &Config) -> Result<OpcoesQr, ErroTela> {
        nfce::opcoes_qr(&*self.cofre, &config.como_nfce()).map_err(|e| match e {
            // a frase da aplicacao manda rodar um comando: na janela o caminho e' a tela de Configuracoes
            Erro::Uso(_) => ErroTela::Valor(
                "O QR Code versão 2 precisa do CSC e ele não está guardado neste computador: guarde-o em Configurações, NFC-e (ou use o QR Code versão 3, que não usa CSC).".into(),
            ),
            outro => do_erro(&outro),
        })
    }

    // ---------------------------------------------------------------- pasta e cupom

    fn notas_nfce(&self, config: &Config) -> Result<NotasNfe, ErroTela> {
        nfce::abrir_pasta(config.notas.pasta.trim()).map_err(|_| ErroTela::Pasta)
    }

    /// As NFC-e do indice da pasta `nfce/`, da mais recente para a mais antiga.
    ///
    /// # Errors
    ///
    /// [`ErroTela::Pasta`] ou [`ErroTela::LeituraDaPasta`].
    pub fn listar_nfce(
        &self,
        config: &Config,
    ) -> Result<Vec<rustnf_app::notas::nfe::Registro>, ErroTela> {
        self.notas_nfce(config)?
            .listar()
            .map_err(|_| ErroTela::LeituraDaPasta)
    }

    /// A NFC-e da pasta com o que o XML guardado traz.
    ///
    /// # Errors
    ///
    /// [`ErroTela::Pasta`] ou [`ErroTela::NotaAusente`].
    pub fn ler_nfce(&self, config: &Config, chave: &str) -> Result<NotaNfeDaPasta, ErroTela> {
        let notas = self.notas_nfce(config)?;
        let registro = notas.registro(chave).map_err(|_| ErroTela::NotaAusente)?;
        let xml = notas.ler_xml(chave).ok();
        let leitura = LeituraNfe::do_xml(xml.as_deref(), &registro);
        Ok(NotaNfeDaPasta { registro, leitura })
    }

    /// Gera o cupom (DANFC-e em PDF de bobina, com a marca de cancelada se for o caso) e o grava ao lado do XML da nota. Devolve
    /// onde ficou. A aplicacao nao abre impressora nem porta: quem imprime e' outro programa.
    ///
    /// # Errors
    ///
    /// [`ErroTela::NotaAusente`], [`ErroTela::Danfse`] ou [`ErroTela::GravacaoDoArquivo`].
    pub fn danfce_da_pasta(&self, config: &Config, chave: &str) -> Result<PathBuf, ErroTela> {
        let notas = self.notas_nfce(config)?;
        let (xml, cancelada) = danfce::xml_da_pasta(&notas, chave).map_err(|e| match e {
            Erro::NaoEncontrada(_) => ErroTela::NotaAusente,
            outro => ErroTela::Danfse(frase(&outro.mensagem())),
        })?;
        let op = danfce::opcoes(config, danfce::Pedido::default(), cancelada)
            .map_err(|e| ErroTela::Danfse(frase(&e.mensagem())))?;
        let pdf = danfce::pdf(&xml, &op).map_err(|e| ErroTela::Danfse(frase(&e.mensagem())))?;
        danfce::guardar(&notas, chave, &pdf).map_err(|_| ErroTela::GravacaoDoArquivo)
    }

    /// Salva o cupom em bytes ESC/POS ao lado do PDF (`<chave>-...escpos`), para quem tem impressora termica que le o arquivo.
    ///
    /// # Errors
    ///
    /// Como em [`Servicos::danfce_da_pasta`].
    pub fn escpos_da_pasta(&self, config: &Config, chave: &str) -> Result<PathBuf, ErroTela> {
        let pdf = self.danfce_da_pasta(config, chave)?;
        let notas = self.notas_nfce(config)?;
        let (xml, cancelada) =
            danfce::xml_da_pasta(&notas, chave).map_err(|_| ErroTela::NotaAusente)?;
        let op = danfce::opcoes(config, danfce::Pedido::default(), cancelada)
            .map_err(|e| ErroTela::Danfse(frase(&e.mensagem())))?;
        let bytes =
            danfce::escpos(&xml, &op).map_err(|e| ErroTela::Danfse(frase(&e.mensagem())))?;
        let destino = pdf.with_extension("escpos");
        rustnf_app::atomico::gravar(&destino, &bytes).map_err(|_| ErroTela::GravacaoDoArquivo)?;
        Ok(destino)
    }

    /// Onde esta o XML guardado da NFC-e (para "Mostrar na pasta").
    ///
    /// # Errors
    ///
    /// [`ErroTela::NotaAusente`] ou [`ErroTela::XmlAusente`].
    pub fn caminho_do_xml_nfce(&self, config: &Config, chave: &str) -> Result<PathBuf, ErroTela> {
        let c = self
            .notas_nfce(config)?
            .caminho_do_proc(chave)
            .map_err(|_| ErroTela::NotaAusente)?;
        if c.is_file() {
            Ok(c)
        } else {
            Err(ErroTela::XmlAusente)
        }
    }

    /// O aviso do prazo nacional do cancelamento da NFC-e (24 horas), se ja acabou; `None` dentro do prazo. Nunca bloqueia: quem
    /// decide e' a SEFAZ.
    #[must_use]
    pub fn aviso_do_prazo_nfce(&self, config: &Config, chave: &str) -> Option<String> {
        let notas = self.notas_nfce(config).ok()?;
        nfce::aviso_de_prazo(&notas, chave, self.agora()).map(|t| frase(&t))
    }

    // ---------------------------------------------------------------- o caixa

    /// O que o caixa precisa da configuracao e do relogio: o regime, a UF e a hora (Brasilia).
    #[must_use]
    pub fn contexto_do_caixa(&self, config: &Config) -> ContextoNfe {
        let brasilia = FixedOffset::west_opt(3 * 3600).unwrap_or(chrono::Utc.fix());
        let hoje = self.agora().with_timezone(&brasilia).date_naive();
        let c = config.como_nfce();
        let numero = self
            .notas_nfce(&c)
            .ok()
            .and_then(|n| n.proximo_numero(c.nfe.serie.trim()).ok())
            .unwrap_or(1);
        ContextoNfe::de_config(&c, hoje, numero)
    }

    /// As NFC-e que ainda dependem da SEFAZ (off-line e sem resposta), as de prazo mais curto primeiro, com o relogio dos
    /// servicos.
    ///
    /// # Errors
    ///
    /// [`ErroTela::Pasta`] ou [`ErroTela::LeituraDaPasta`].
    pub fn pendentes_nfce(&self, config: &Config) -> Result<Vec<Pendente>, ErroTela> {
        nfce::listar_pendentes(&self.notas_nfce(config)?, self.agora())
            .map_err(|_| ErroTela::LeituraDaPasta)
    }

    fn ler_do_disco(pasta: &str, nome: &str, limite: u64) -> Option<Vec<u8>> {
        let pasta = pasta.trim();
        if pasta.is_empty() {
            return None;
        }
        let arquivo = std::fs::File::open(Path::new(pasta).join(nome)).ok()?;
        let mut bytes = Vec::new();
        arquivo.take(limite + 1).read_to_end(&mut bytes).ok()?;
        (bytes.len() as u64 <= limite).then_some(bytes)
    }

    /// Le o rascunho do caixa. Sem pasta, sem arquivo, ilegivel ou de outra versao: `None`.
    #[must_use]
    pub fn ler_rascunho_caixa(&self, config: &Config) -> Option<Cupom> {
        let bytes = Self::ler_do_disco(
            &config.notas.pasta,
            nfce_rascunho::ARQUIVO,
            nfce_rascunho::LIMITE,
        )?;
        nfce_rascunho::decodificar(&bytes)
    }

    /// Grava o rascunho do caixa, de forma atomica.
    ///
    /// # Errors
    ///
    /// [`ErroTela::Pasta`] se nao ha pasta ou nao foi possivel gravar.
    pub fn gravar_rascunho_caixa(&self, config: &Config, cupom: &Cupom) -> Result<(), ErroTela> {
        let pasta = config.notas.pasta.trim();
        if pasta.is_empty() {
            return Err(ErroTela::Pasta);
        }
        rustnf_app::atomico::gravar(
            &Path::new(pasta).join(nfce_rascunho::ARQUIVO),
            &nfce_rascunho::codificar(cupom),
        )
        .map_err(|_| ErroTela::Pasta)
    }

    /// Apaga o rascunho do caixa (nao e' erro se ele nao existe).
    pub fn apagar_rascunho_caixa(&self, config: &Config) {
        let pasta = config.notas.pasta.trim();
        if !pasta.is_empty() {
            let _ = std::fs::remove_file(Path::new(pasta).join(nfce_rascunho::ARQUIVO));
        }
    }
}

/// Emite a NFC-e do cupom: o pedido e' completado, conferido, assinado, GRAVADO como pendente e so' entao enviado. Com
/// `Modo::OfflineSeNecessario`, a SEFAZ que nao responde ANTES do envio vira contingencia off-line; depois de enviar sem
/// resposta a nota fica pendente (nunca vira uma segunda NFC-e para o mesmo cupom, ver `rustnf_app::nfce::emitir`).
pub async fn emitir_nfce(
    fabrica: Arc<dyn FabricaNfe>,
    preparo: Preparo,
    config: Config,
    qr: OpcoesQr,
    pedido: Value,
    modo: Modo,
) -> DesfechoCaixa {
    let s = match montar_doc(Documento::Nfce, &fabrica, preparo, config, Some(qr)) {
        Ok(s) => s,
        Err(e) => return DesfechoCaixa::do_erro(&Erro::Uso(e.texto())),
    };
    DesfechoCaixa::de(nfce::emitir(&s.contexto(), pedido, true, modo).await)
}
