// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Emitir a nota: a fabrica do emissor (trocavel em teste, no mesmo desenho do `Conexao`) e a emissao em si, que
//! abre a pasta de notas, envia o pedido congelado pela camada de aplicacao e devolve o desfecho tipado.

use std::sync::Arc;

use chrono::Duration;
use rustnf::nfse::base::{brasilia, de_datahora};
use rustnf_app::certificado;
use rustnf_app::config::Config;
use rustnf_app::emissao::Emissor;
use rustnf_app::notas::Notas;
use rustnf_app::pedido::emitir_pedido_completo;
use rustnf_app::segredo::Segredo;

use super::{frase, ErroTela, Preparo, Servicos};
use crate::modelo::conferencia::Pendencia;
use crate::modelo::emissao::DesfechoEmissao;
use crate::modelo::nota::DadosNota;
use crate::modelo::pedido::{congelar, Contexto, PedidoCongelado};

/// Quem monta o emissor a partir do certificado aberto. O real monta o cliente da Sefin Nacional (TLS mutuo); os
/// testes e a demonstracao trocam por um que nao fala com a rede.
pub trait FabricaDeEmissor: Send + Sync {
    /// Monta o emissor com o certificado, o ambiente, a assinatura e o tempo limite do `preparo`.
    ///
    /// # Errors
    ///
    /// [`ErroTela::Rede`] se o certificado nao serve para a conexao.
    fn criar(&self, preparo: Preparo) -> Result<Arc<dyn Emissor>, ErroTela>;

    /// E' a demonstracao: nada e' enviado de verdade. So' a fabrica da demonstracao diz que sim.
    fn demonstracao(&self) -> bool {
        false
    }
}

/// A fabrica de verdade: o cliente real da Sefin Nacional.
#[derive(Debug, Clone, Copy, Default)]
pub struct FabricaReal;

impl FabricaDeEmissor for FabricaReal {
    fn criar(&self, preparo: Preparo) -> Result<Arc<dyn Emissor>, ErroTela> {
        let (ambiente, algoritmo, tempo) =
            (preparo.ambiente, preparo.algoritmo, preparo.tempo_limite);
        let cliente = certificado::cliente(preparo.into_certificado(), ambiente, algoritmo, tempo)
            .map_err(|e| ErroTela::Rede(frase(&e.to_string())))?;
        Ok(Arc::new(cliente))
    }
}

/// A fabrica de quem nao emite: toda tentativa volta com um erro e nada e' enviado. E' o que `Servicos::com` usa
/// ate alguem trocar.
#[derive(Debug, Clone, Copy, Default)]
pub struct FabricaSemEmissor;

impl FabricaDeEmissor for FabricaSemEmissor {
    fn criar(&self, _preparo: Preparo) -> Result<Arc<dyn Emissor>, ErroTela> {
        Err(ErroTela::Rede(
            "Este conjunto de serviços não envia notas.".into(),
        ))
    }
}

impl Servicos {
    /// Troca a fabrica do emissor (os testes e a demonstracao).
    #[must_use]
    pub fn com_fabrica(mut self, fabrica: Arc<dyn FabricaDeEmissor>) -> Self {
        self.fabrica = fabrica;
        self
    }

    /// A fabrica do emissor.
    #[must_use]
    pub fn fabrica(&self) -> Arc<dyn FabricaDeEmissor> {
        Arc::clone(&self.fabrica)
    }

    /// Os servicos estao na demonstracao: nada e' enviado de verdade.
    #[must_use]
    pub fn demonstracao(&self) -> bool {
        self.fabrica.demonstracao()
    }

    /// O contexto da nota para a configuracao, no instante de agora.
    #[must_use]
    pub fn contexto_da_nota(&self, config: &Config) -> Contexto {
        Contexto::de_config(config, self.agora())
    }

    /// O proximo numero de DPS da serie do emitente, pela pasta de notas.
    ///
    /// # Errors
    ///
    /// [`ErroTela::Pasta`] se a pasta nao serve ou o numero nao pode ser lido.
    pub fn proximo_numero(&self, config: &Config) -> Result<u64, ErroTela> {
        let serie = self.contexto_da_nota(config).serie();
        Notas::abrir(&config.notas.pasta)
            .and_then(|n| n.proximo_numero(&serie))
            .map_err(|_| ErroTela::Pasta)
    }

    /// Congela o pedido da nota: o proximo numero da pasta de notas e a data e hora de emissao de agora (menos a
    /// folga do relogio). Cada reenvio da mesma tentativa usa este mesmo pedido.
    ///
    /// # Errors
    ///
    /// [`ErroTela::Pasta`].
    pub fn congelar_pedido(
        &self,
        config: &Config,
        dados: &DadosNota,
    ) -> Result<PedidoCongelado, ErroTela> {
        let ctx = self.contexto_da_nota(config);
        let numero = self.proximo_numero(config)?;
        let agora = (self.agora() - Duration::seconds(i64::from(config.sefin.folga_relogio_s)))
            .with_timezone(&brasilia());
        let emissao = de_datahora(&agora, "infDPS/dhEmi").map_err(|_| ErroTela::Pasta)?;
        Ok(congelar(dados, &ctx, numero, &emissao))
    }

    /// Abre o certificado e reune o que a emissao precisa (o mesmo da conexao: certificado, ambiente, assinatura e
    /// tempo limite).
    ///
    /// # Errors
    ///
    /// Os de [`Servicos::testar_certificado`]; sem senha guardada nem digitada, [`ErroTela::SemSenha`].
    pub fn preparar_emissao(
        &self,
        config: &Config,
        digitada: Option<&Segredo>,
    ) -> Result<Preparo, ErroTela> {
        self.preparar_conexao(config, digitada)
    }
}

/// Envia o pedido congelado e devolve como terminou. Roda fora da linha da tela (`fora::rede`): monta o emissor, abre
/// a pasta de notas e usa a camada de aplicacao, que numera so' o que a pasta confirma, guarda a nota autorizada e
/// recupera a que ja existe.
pub async fn emitir(
    fabrica: Arc<dyn FabricaDeEmissor>,
    preparo: Preparo,
    config: Config,
    tentativa: PedidoCongelado,
) -> DesfechoEmissao {
    let op = config.emitente.op_simples;
    let emissor = match fabrica.criar(preparo) {
        Ok(e) => e,
        Err(e) => return antes_de_enviar(e.texto()),
    };
    let notas = match Notas::abrir(&config.notas.pasta) {
        Ok(n) => n,
        Err(_) => return antes_de_enviar(ErroTela::Pasta.texto()),
    };
    let r = emitir_pedido_completo(
        &*emissor,
        tentativa.pedido,
        &config.emitente,
        config.sefin.folga_relogio_s,
        Some(&notas),
    )
    .await;
    DesfechoEmissao::de(r, op)
}

fn antes_de_enviar(texto: String) -> DesfechoEmissao {
    DesfechoEmissao::Recusada {
        pendencia: Pendencia::geral(texto),
    }
}
