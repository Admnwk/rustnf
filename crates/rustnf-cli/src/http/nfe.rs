// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A NF-e (modelo 55) dentro do servidor: o estado que as rotas de `/nfe/` e as ferramentas do MCP compartilham. Cada
//! metodo chama a MESMA funcao de `rustnf_app::nfe` que a linha de comando chama; nenhuma regra fiscal mora aqui. O
//! resultado vem como [`Saida`]: o JSON completo da SEFAZ e, se a nota nao ficou autorizada, o erro em palavras.

use std::sync::Arc;

use async_trait::async_trait;
use rustnf::assinatura::cert_a1::Certificado;
use rustnf::nfe::sefaz::{ClienteSefaz, FalhaRede, Transporte, TransporteReqwest};
use rustnf_app::config::Config;
use rustnf_app::nfe::{self, Contexto};
use rustnf_app::notas::nfe::NotasNfe;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::erro::Erro;

/// O transporte do servidor: o real (TLS mutuo pelo A1) ou, nos testes, um falso. Fica atras de um ponteiro para o
/// servidor nao depender de qual e'.
pub struct TransporteDin(Arc<dyn Transporte>);

/// Poe um transporte qualquer no formato do servidor.
pub fn dinamico<T: Transporte + 'static>(transporte: T) -> TransporteDin {
    TransporteDin(Arc::new(transporte))
}

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

/// O cliente real da SEFAZ para a `config` (a da NF-e ou a vista pela NFC-e): TLS mutuo pelo A1, raizes da ICP-Brasil e a CA
/// extra de `[nfe].ca_pem`.
///
/// # Errors
///
/// [`Erro::Entrada`] se `ca_pem` nao le; [`Erro::Recusa`] se o certificado nao serve para TLS; os da configuracao do cliente.
pub(super) fn cliente_real(
    config: &Config,
    certificado: &Certificado,
) -> Result<ClienteSefaz<TransporteDin>, Erro> {
    let sefaz = nfe::config_sefaz(config)?;
    let ca = nfe::ler_ca_pem(config)?;
    let transporte = TransporteReqwest::novo(certificado, ca.as_deref(), sefaz.timeout)
        .map_err(|_| Erro::recusa("certificado", "o certificado nao serve para TLS"))?;
    Ok(ClienteSefaz::com_transporte(sefaz, dinamico(transporte))?)
}

/// O que uma acao da NF-e devolveu: o JSON completo e, se a nota nao ficou como o pedido queria, o erro (recusa de regra
/// ou falha de rede) com a frase da SEFAZ.
#[derive(Debug)]
pub struct Saida {
    /// O resultado em JSON (chave, cStat, protocolo, situacao na pasta...).
    pub corpo: Value,
    /// O erro do resultado; `None` = deu certo.
    pub erro: Option<Erro>,
}

struct Motor {
    cliente: ClienteSefaz<TransporteDin>,
    certificado: Certificado,
}

/// A NF-e do servidor: a configuracao (`[emitente]`, `[nfe]`, `[notas]`), a pasta de notas e, quando ha emissao, o cliente
/// da SEFAZ com o certificado A1.
pub struct ServidorNfe {
    pub(super) config: Config,
    pub(super) notas: Option<NotasNfe>,
    motor: Result<Motor, String>,
    /// Quem numera ou reserva numero entra um de cada vez: duas emissoes simultaneas nao gastam o mesmo numero (a pasta
    /// de notas tambem trava, mas entre ler o proximo numero e grava-lo ha um intervalo).
    pub(super) fila: tokio::sync::Mutex<()>,
}

impl ServidorNfe {
    /// Sem emissao: so' conferir, montar e gerar o DANFE; as acoes que usam a SEFAZ respondem `motivo`.
    #[must_use]
    pub fn sem_emissao(config: Config, notas: Option<NotasNfe>, motivo: impl Into<String>) -> Self {
        Self {
            config,
            notas,
            motor: Err(motivo.into()),
            fila: tokio::sync::Mutex::new(()),
        }
    }

    /// Com o cliente da SEFAZ e o certificado que assina.
    #[must_use]
    pub fn com_cliente(
        config: Config,
        notas: Option<NotasNfe>,
        cliente: ClienteSefaz<TransporteDin>,
        certificado: Certificado,
    ) -> Self {
        Self {
            config,
            notas,
            motor: Ok(Motor {
                cliente,
                certificado,
            }),
            fila: tokio::sync::Mutex::new(()),
        }
    }

    /// O cliente real: TLS mutuo pelo A1, raizes da ICP-Brasil e a CA extra de `[nfe].ca_pem`. Sem `[nfe].uf` a NF-e
    /// fica sem emissao e diz o que configurar; o resto da configuracao errada impede o servidor de subir.
    ///
    /// # Errors
    ///
    /// [`Erro::Entrada`] se `ca_pem` nao le; [`Erro::Recusa`] se o certificado nao serve para TLS ou a configuracao do
    /// cliente e' invalida.
    pub fn real(
        config: Config,
        notas: Option<NotasNfe>,
        certificado: Certificado,
    ) -> Result<Self, Erro> {
        if config.nfe.uf.is_empty() {
            return Ok(Self::sem_emissao(config, notas, SEM_UF));
        }
        let cliente = cliente_real(&config, &certificado)?;
        Ok(Self::com_cliente(config, notas, cliente, certificado))
    }

    /// A NF-e pode falar com a SEFAZ neste servidor?
    #[must_use]
    pub fn pronta(&self) -> bool {
        self.motor.is_ok()
    }

    fn motor(&self) -> Result<&Motor, Erro> {
        self.motor
            .as_ref()
            .map_err(|motivo| Erro::Uso(format!("a NF-e nao esta pronta neste servidor: {motivo}")))
    }

    pub(super) fn contexto(&self) -> Result<Contexto<'_, TransporteDin>, Erro> {
        let m = self.motor()?;
        Ok(Contexto::novo(
            &m.cliente,
            &m.certificado,
            &self.config,
            self.notas.as_ref(),
        ))
    }

    /// Confere o pedido (regras do MOC e das NT) sem rede e sem gastar numero. A recusa vai dentro do JSON, com
    /// `ok: false`, e o erro vem em [`Saida::erro`].
    #[must_use]
    pub fn conferir(&self, pedido: Value, estrito: bool) -> Saida {
        let r = nfe::montar_pedido(&self.config, self.notas.as_ref(), pedido, estrito);
        Saida {
            corpo: nfe::conferencia_json(&r),
            erro: r.err(),
        }
    }

    /// Monta o XML da NF-e. Nunca assina (a assinatura e' da emissao, que tem o certificado): serve com ou sem certificado.
    ///
    /// # Errors
    ///
    /// Os de [`nfe::montar_pedido`].
    pub fn montar(&self, pedido: Value, estrito: bool) -> Result<Value, Erro> {
        let m = nfe::montar_pedido(&self.config, self.notas.as_ref(), pedido, estrito)?;
        let mut v = json!({
            "xml": m.xml,
            "assinada": false,
            "chave": m.chave,
            "serie": m.serie,
            "numero": m.numero,
            "totais": m.totais,
            "avisos": nfe::avisos_json(&m.avisos),
        });
        if !m.desconhecidos.is_empty() {
            v["campos_desconhecidos"] = json!(m.desconhecidos);
        }
        Ok(v)
    }

    /// O DANFE em PDF do `nfeProc`. `canhoto`: sem ele, vale `[nfe].canhoto`.
    ///
    /// # Errors
    ///
    /// Os de [`nfe::danfe_do_xml`].
    pub async fn danfe(
        &self,
        xml: String,
        canhoto: Option<bool>,
        cancelada: bool,
    ) -> Result<Vec<u8>, Erro> {
        let canhoto = canhoto.unwrap_or(self.config.nfe.canhoto);
        tokio::task::spawn_blocking(move || nfe::danfe_do_xml(&xml, canhoto, cancelada, None))
            .await
            .map_err(|_| Erro::Interno)?
    }

    /// Emite a NF-e (ou, com `epec`, a registra em contingencia EPEC). No EPEC o pedido pode trazer `substitui`, a chave da
    /// NF-e pendente cuja venda ele registra.
    ///
    /// # Errors
    ///
    /// Os de [`nfe::emitir`] e [`nfe::epec_substituindo`].
    pub async fn emitir(&self, pedido: Value, estrito: bool, epec: bool) -> Result<Saida, Erro> {
        let ctx = self.contexto()?;
        let _vez = self.fila.lock().await;
        if epec {
            let mut pedido = pedido;
            let substitui = nfe::separar_substitui(&mut pedido)?;
            let r = nfe::epec_substituindo(&ctx, pedido, estrito, substitui.as_deref()).await?;
            return Ok(Saida {
                corpo: r.para_json(),
                erro: r.erro(),
            });
        }
        let e = nfe::emitir(&ctx, pedido, estrito).await?;
        Ok(Saida {
            corpo: e.para_json(),
            erro: e.erro(),
        })
    }

    /// Consulta a NF-e pela chave e atualiza a pasta.
    ///
    /// # Errors
    ///
    /// Os de [`nfe::consultar`].
    pub async fn consultar(&self, chave: &str) -> Result<Saida, Erro> {
        let c = nfe::consultar(&self.contexto()?, chave).await?;
        Ok(Saida {
            corpo: c.para_json(),
            erro: c.erro(),
        })
    }

    /// Cancela a NF-e (evento 110111).
    ///
    /// # Errors
    ///
    /// Os de [`nfe::cancelar`].
    pub async fn cancelar(&self, p: &PedidoCancelar) -> Result<Saida, Erro> {
        let ctx = self.contexto()?;
        let r = nfe::cancelar(&ctx, &p.chave, &p.justificativa, p.protocolo.as_deref()).await?;
        Ok(Saida {
            corpo: r.para_json(),
            erro: r.erro(),
        })
    }

    /// Registra a carta de correcao (evento 110110).
    ///
    /// # Errors
    ///
    /// Os de [`nfe::corrigir`].
    pub async fn corrigir(&self, p: &PedidoCorrigir) -> Result<Saida, Erro> {
        let ctx = self.contexto()?;
        let r = nfe::corrigir(&ctx, &p.chave, &p.texto, p.sequencia).await?;
        Ok(Saida {
            corpo: r.para_json(),
            erro: r.erro(),
        })
    }

    /// Inutiliza uma faixa de numeros.
    ///
    /// # Errors
    ///
    /// Os de [`nfe::inutilizar`].
    pub async fn inutilizar(&self, p: &PedidoInutilizar) -> Result<Saida, Erro> {
        let ctx = self.contexto()?;
        let _vez = self.fila.lock().await;
        let r =
            nfe::inutilizar(&ctx, p.serie.as_deref(), p.inicio, p.fim, &p.justificativa).await?;
        Ok(Saida {
            corpo: r.para_json(),
            erro: r.erro(),
        })
    }

    /// O estado do servico da SEFAZ (ou da SVC).
    ///
    /// # Errors
    ///
    /// Os de [`nfe::status`].
    pub async fn status(&self, svc: bool) -> Result<Saida, Erro> {
        let m = self.motor()?;
        let s = nfe::status(&m.cliente, svc).await?;
        let mut v = nfe::estado_do_servico(&m.cliente, &s);
        v["svc"] = json!(svc);
        Ok(Saida {
            corpo: v,
            erro: (!s.em_operacao()).then(|| Erro::Rede(s.frase.clone())),
        })
    }

    /// As notas que seguem sem resultado (pendentes e em EPEC), sem rede.
    ///
    /// # Errors
    ///
    /// [`Erro::Uso`] sem pasta de notas.
    pub fn pendentes(&self) -> Result<Value, Erro> {
        let notas = self.notas.as_ref().ok_or_else(|| {
            Erro::Uso(
                "sem pasta de notas: configure [notas].pasta no arquivo ou RUSTNF_NOTAS_PASTA"
                    .into(),
            )
        })?;
        Ok(json!({"notas": nfe::pendentes_json(&notas.a_resolver()?)?}))
    }

    /// Retoma as notas pendentes: consulta a SEFAZ pela MESMA chave, nunca gera outra nota.
    ///
    /// # Errors
    ///
    /// Os de [`nfe::retomar`].
    pub async fn retomar(&self) -> Result<Saida, Erro> {
        let ctx = self.contexto()?;
        let _vez = self.fila.lock().await;
        let feitos = nfe::retomar(&ctx).await?;
        Ok(Saida {
            corpo: json!({"notas": feitos.iter().map(nfe::Retomada::para_json).collect::<Vec<_>>()}),
            erro: nfe::erro_da_retomada(&feitos),
        })
    }
}

impl Saida {
    /// O corpo para a resposta: o resultado e, se houver erro, o envelope `erro` junto, para a pessoa ver a chave e o
    /// cStat mesmo quando a nota nao foi autorizada.
    #[must_use]
    pub fn corpo_com_erro(&self) -> Value {
        let mut v = self.corpo.clone();
        if let Some(e) = &self.erro {
            v["erro"] = e.envelope()["erro"].clone();
        }
        v
    }
}

/// O que falta para a NF-e falar com a SEFAZ quando `[nfe].uf` esta vazia.
const SEM_UF: &str =
    "informe a UF do emitente em [nfe].uf do arquivo de configuracao (e [notas].pasta para numerar e guardar as notas)";

/// `cancelar_nfe` / `POST /nfe/cancelar`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PedidoCancelar {
    /// Chave de acesso (44 posicoes).
    pub chave: String,
    /// Justificativa (15 a 255 caracteres).
    pub justificativa: String,
    /// Protocolo de autorizacao, se a nota nao esta na pasta.
    #[serde(default)]
    pub protocolo: Option<String>,
}

/// `corrigir_nfe` / `POST /nfe/corrigir`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PedidoCorrigir {
    /// Chave de acesso.
    pub chave: String,
    /// O texto da correcao (15 a 1000 caracteres).
    pub texto: String,
    /// Sequencia do evento; sem ela a pasta numera.
    #[serde(default)]
    pub sequencia: Option<u32>,
}

/// `inutilizar_nfe` / `POST /nfe/inutilizar`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PedidoInutilizar {
    /// Serie; sem ela vale `[nfe].serie`.
    #[serde(default)]
    pub serie: Option<String>,
    /// Primeiro numero da faixa.
    pub inicio: u64,
    /// Ultimo numero da faixa.
    pub fim: u64,
    /// Justificativa (15 a 255 caracteres).
    pub justificativa: String,
}
