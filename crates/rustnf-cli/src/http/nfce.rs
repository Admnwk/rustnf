// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A NFC-e (modelo 65) dentro do servidor: o estado que as rotas de `/nfce/` e as ferramentas `*_nfce` do MCP compartilham.
//! Cada metodo chama a MESMA funcao de `rustnf_app::nfce` (e de `rustnf_app::nfe`, no que a NFC-e compartilha) que a linha
//! de comando `rustnf nfce` chama; nenhuma regra fiscal mora aqui.
//!
//! O CSC (Codigo de Seguranca do Contribuinte) NAO passa por aqui: o servidor o le do cofre UMA vez, ao subir, para o QR Code
//! versao 2, e nenhuma rota nem ferramenta o grava, le ou apaga. Pedido com campo de CSC e' recusado antes de qualquer envio.

use rustnf::assinatura::cert_a1::Certificado;
use rustnf::nfe::montagem::OpcoesQr;
use rustnf::nfe::sefaz::ClienteSefaz;
use rustnf_app::config::Config;
use rustnf_app::nfce::{self, danfce, Modo, Pendente};
use rustnf_app::nfe::{self, Contexto};
use rustnf_app::notas::nfe::NotasNfe;
use serde_json::{json, Value};

use super::nfe::{cliente_real, PedidoCancelar, Saida, TransporteDin};
use crate::erro::Erro;

struct Motor {
    cliente: ClienteSefaz<TransporteDin>,
    certificado: Certificado,
    qr: OpcoesQr,
}

/// A NFC-e do servidor: a configuracao vista pela NFC-e (`[nfce]` sobre o `[emitente]` e o `[nfe]`), a pasta `nfce/` e, quando ha
/// emissao, o cliente da SEFAZ, o certificado A1 e o QR Code.
pub struct ServidorNfce {
    pub(super) config: Config,
    pub(super) notas: Option<NotasNfe>,
    motor: Result<Motor, String>,
    /// Quem numera entra um de cada vez: duas emissoes simultaneas nao gastam o mesmo numero. A fila e' PROPRIA da NFC-e: a
    /// numeracao do 65 e' outra que a do 55, e uma nao espera a outra.
    pub(super) fila: tokio::sync::Mutex<()>,
}

/// O que falta para a NFC-e falar com a SEFAZ quando `[nfe].uf` esta vazia.
const SEM_UF: &str =
    "informe a UF do emitente em [nfe].uf do arquivo de configuracao (a NFC-e usa o mesmo emitente, certificado e UF da NF-e; [notas].pasta guarda e numera as notas)";

/// Pedido que traz o CSC por HTTP ou MCP: nunca.
const CSC_SO_PELA_CLI: &str =
    "o CSC nao se informa por HTTP nem MCP: guarde-o na maquina do servidor com `rustnf nfce csc guardar`";

impl ServidorNfce {
    /// Sem emissao: so' o DANFC-e; as acoes que usam a SEFAZ respondem `motivo`.
    #[must_use]
    pub fn sem_emissao(
        config: &Config,
        notas: Option<NotasNfe>,
        motivo: impl Into<String>,
    ) -> Self {
        Self {
            config: config.como_nfce(),
            notas,
            motor: Err(motivo.into()),
            fila: tokio::sync::Mutex::new(()),
        }
    }

    /// Com o cliente da SEFAZ, o certificado que assina e as opcoes do QR Code (com o CSC, se a versao e' a 2).
    #[must_use]
    pub fn com_cliente(
        config: &Config,
        notas: Option<NotasNfe>,
        cliente: ClienteSefaz<TransporteDin>,
        certificado: Certificado,
        qr: OpcoesQr,
    ) -> Self {
        Self {
            config: config.como_nfce(),
            notas,
            motor: Ok(Motor {
                cliente,
                certificado,
                qr,
            }),
            fila: tokio::sync::Mutex::new(()),
        }
    }

    /// O cliente real, como o da NF-e. Sem `[nfe].uf` a NFC-e fica sem emissao e diz o que configurar.
    ///
    /// # Errors
    ///
    /// Os de [`cliente_real`].
    pub fn real(
        config: &Config,
        notas: Option<NotasNfe>,
        certificado: Certificado,
        qr: OpcoesQr,
    ) -> Result<Self, Erro> {
        let cfg = config.como_nfce();
        if cfg.nfe.uf.is_empty() {
            return Ok(Self::sem_emissao(config, notas, SEM_UF));
        }
        let cliente = cliente_real(&cfg, &certificado)?;
        Ok(Self::com_cliente(config, notas, cliente, certificado, qr))
    }

    /// A NFC-e pode falar com a SEFAZ neste servidor?
    #[must_use]
    pub fn pronta(&self) -> bool {
        self.motor.is_ok()
    }

    /// O QR Code versao 2 esta configurado E o CSC foi lido do cofre ao subir? So' um booleano: o valor nunca sai daqui.
    #[must_use]
    pub fn csc_guardado(&self) -> bool {
        self.motor.as_ref().is_ok_and(|m| m.qr.csc.is_some())
    }

    fn motor(&self) -> Result<&Motor, Erro> {
        self.motor.as_ref().map_err(|motivo| {
            Erro::Uso(format!("a NFC-e nao esta pronta neste servidor: {motivo}"))
        })
    }

    fn contexto(&self) -> Result<Contexto<'_, TransporteDin>, Erro> {
        let m = self.motor()?;
        Ok(Contexto::novo(
            &m.cliente,
            &m.certificado,
            &self.config,
            self.notas.as_ref(),
        )
        .com_qr(m.qr.clone()))
    }

    fn pasta(&self) -> Result<&NotasNfe, Erro> {
        self.notas.as_ref().ok_or_else(|| {
            Erro::Uso(
                "sem pasta de notas: configure [notas].pasta no arquivo ou RUSTNF_NOTAS_PASTA"
                    .into(),
            )
        })
    }

    /// O DANFC-e do `nfeProc` (ou da NFe de contingencia off-line) em PDF de bobina ou, com `escpos`, em bytes ESC/POS. O que
    /// o pedido nao traz vem de `[nfce]`.
    ///
    /// # Errors
    ///
    /// Os de [`danfce::opcoes`], [`danfce::pdf`] e [`danfce::escpos`].
    pub async fn danfce(
        &self,
        xml: String,
        pedido: danfce::Pedido,
        cancelada: bool,
        escpos: bool,
    ) -> Result<Vec<u8>, Erro> {
        let opcoes = danfce::opcoes(&self.config, pedido, cancelada)?;
        tokio::task::spawn_blocking(move || {
            if escpos {
                danfce::escpos(&xml, &opcoes)
            } else {
                danfce::pdf(&xml, &opcoes)
            }
        })
        .await
        .map_err(|_| Erro::Interno)?
    }

    /// Emite a NFC-e (normal ou em contingencia off-line). O pedido com campo de CSC e' recusado ANTES de tudo.
    ///
    /// # Errors
    ///
    /// [`Erro::Recusa`] com CSC no pedido; os de [`nfce::emitir`].
    pub async fn emitir(&self, pedido: Value, estrito: bool, modo: Modo) -> Result<Saida, Erro> {
        recusar_csc(&pedido)?;
        let ctx = self.contexto()?;
        let _vez = self.fila.lock().await;
        let e = nfce::emitir(&ctx, pedido, estrito, modo).await?;
        Ok(Saida {
            corpo: e.para_json(),
            erro: e.erro(),
        })
    }

    /// Consulta a NFC-e pela chave e atualiza a pasta.
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

    /// Cancela a NFC-e (evento 110111). Se o prazo nacional de 24 horas ja acabou, o aviso vai em `aviso_prazo` no JSON de
    /// resposta; ele nunca bloqueia o envio (a SEFAZ decide).
    ///
    /// # Errors
    ///
    /// Os de [`nfe::cancelar`].
    pub async fn cancelar(&self, p: &PedidoCancelar) -> Result<Saida, Erro> {
        let ctx = self.contexto()?;
        let aviso = ctx
            .notas
            .and_then(|n| nfce::aviso_de_prazo(n, &p.chave, ctx.agora));
        let r = nfe::cancelar(&ctx, &p.chave, &p.justificativa, p.protocolo.as_deref()).await?;
        let mut corpo = r.para_json();
        if let Some(a) = aviso {
            corpo["aviso_prazo"] = json!(a);
        }
        Ok(Saida {
            corpo,
            erro: r.erro(),
        })
    }

    /// O estado do servico de autorizacao da NFC-e da UF (a NFC-e nao tem SVC).
    ///
    /// # Errors
    ///
    /// Os de `status_nfce`.
    pub async fn status(&self) -> Result<Saida, Erro> {
        let m = self.motor()?;
        let s = m.cliente.status_nfce().await?;
        Ok(Saida {
            corpo: nfe::estado_do_servico(&m.cliente, &s),
            erro: (!s.em_operacao()).then(|| Erro::Rede(s.frase.clone())),
        })
    }

    /// As NFC-e que ainda dependem da SEFAZ, sem rede: a lista com o prazo, `pode_fechar_caixa` e o aviso.
    ///
    /// # Errors
    ///
    /// [`Erro::Uso`] sem pasta de notas.
    pub fn pendentes(&self) -> Result<Value, Erro> {
        Ok(resumo(&nfce::listar_pendentes_agora(self.pasta()?)?))
    }

    /// Retoma as pendentes (consulta a SEFAZ pela MESMA chave; transmite as off-line) e diz o que sobrou.
    ///
    /// # Errors
    ///
    /// Os de [`nfe::retomar`].
    pub async fn retomar(&self) -> Result<Saida, Erro> {
        let ctx = self.contexto()?;
        let _vez = self.fila.lock().await;
        let feitos = nfe::retomar(&ctx).await?;
        let mut corpo = match ctx.notas {
            Some(notas) => resumo(&nfce::listar_pendentes(notas, ctx.agora)?),
            None => json!({"notas": [], "pode_fechar_caixa": true, "aviso": null}),
        };
        corpo["retomadas"] = json!(feitos
            .iter()
            .map(nfe::Retomada::para_json)
            .collect::<Vec<_>>());
        Ok(Saida {
            corpo,
            erro: nfe::erro_da_retomada(&feitos),
        })
    }
}

/// O JSON da lista de pendentes: as notas, se o caixa pode fechar (nada pendente) e o aviso do que fazer e ate quando.
fn resumo(lista: &[Pendente]) -> Value {
    json!({
        "notas": lista.iter().map(Pendente::para_json).collect::<Vec<_>>(),
        "pode_fechar_caixa": lista.is_empty(),
        "aviso": nfce::aviso_de_fechamento(lista),
    })
}

/// Tira do pedido `offline` e `offline_se_necessario` (que nao sao campos da nota) e diz o modo de emitir.
///
/// # Errors
///
/// [`Erro::Entrada`] se o pedido nao e' um objeto ou o campo nao e' booleano; [`Erro::Uso`] se os dois vem ligados.
pub fn separar_modo(mut pedido: Value) -> Result<(Value, Modo), Erro> {
    let Some(raiz) = pedido.as_object_mut() else {
        return Err(Erro::Entrada(
            "o pedido da NFC-e tem de ser um objeto JSON".into(),
        ));
    };
    let mut ligado = |nome: &str| -> Result<bool, Erro> {
        match raiz.remove(nome) {
            None => Ok(false),
            Some(Value::Bool(b)) => Ok(b),
            Some(_) => Err(Erro::Entrada(format!("{nome}: use true ou false"))),
        }
    };
    let (offline, se_necessario) = (ligado("offline")?, ligado("offline_se_necessario")?);
    let modo = match (offline, se_necessario) {
        (true, true) => {
            return Err(Erro::Uso(
                "offline e offline_se_necessario nao andam juntos: escolha um".into(),
            ))
        }
        (true, false) => Modo::Offline,
        (false, true) => Modo::OfflineSeNecessario,
        (false, false) => Modo::Normal,
    };
    Ok((pedido, modo))
}

/// Recusa o pedido que tem, em qualquer nivel, um campo cujo nome diz CSC (`csc`, `id_csc`, `idCSC`...). Vale mesmo com
/// `estrito=false`: o CSC nunca entra por aqui, e a mensagem nao repete o valor.
///
/// # Errors
///
/// [`Erro::Recusa`] com o nome do campo.
pub fn recusar_csc(pedido: &Value) -> Result<(), Erro> {
    match pedido {
        Value::Object(m) => {
            for (nome, v) in m {
                if nome.to_ascii_lowercase().contains("csc") {
                    return Err(Erro::recusa(nome.as_str(), CSC_SO_PELA_CLI));
                }
                recusar_csc(v)?;
            }
            Ok(())
        }
        Value::Array(a) => a.iter().try_for_each(recusar_csc),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn modo_sai_do_pedido_e_os_dois_juntos_nao_valem() {
        let (p, m) = separar_modo(json!({"a": 1, "offline": true})).unwrap();
        assert_eq!((m, p), (Modo::Offline, json!({"a": 1})));
        let (_, m) = separar_modo(json!({"offline_se_necessario": true})).unwrap();
        assert_eq!(m, Modo::OfflineSeNecessario);
        let (_, m) = separar_modo(json!({"offline": false})).unwrap();
        assert_eq!(m, Modo::Normal);
        assert!(separar_modo(json!({"offline": true, "offline_se_necessario": true})).is_err());
        assert!(separar_modo(json!({"offline": "sim"})).is_err());
        assert!(separar_modo(json!([1])).is_err());
    }

    #[test]
    fn campo_de_csc_em_qualquer_nivel_e_recusado_sem_repetir_o_valor() {
        assert!(recusar_csc(&json!({"itens": [], "nat_op": "VENDA"})).is_ok());
        for ruim in [
            json!({"csc": "ZZVALORSECRETO"}),
            json!({"id_csc": "1"}),
            json!({"emit": {"idCSC": "1"}}),
            json!({"itens": [{"CSC_valor": "ZZVALORSECRETO"}]}),
        ] {
            let e = recusar_csc(&ruim).unwrap_err().to_string();
            assert!(!e.contains("ZZVALORSECRETO"), "{e}");
            assert!(e.contains("nfce csc guardar"), "{e}");
        }
    }
}
