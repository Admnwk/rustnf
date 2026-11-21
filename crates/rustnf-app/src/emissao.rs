// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Emissao, consulta e cancelamento pela Sefin Nacional, atras de um trait para a API e os testes poderem trocar o
//! transporte. A idempotencia (resposta perdida, 5xx, E0014) e' do cliente da biblioteca.

use async_trait::async_trait;
use rustnf::nfse::leitura::Mensagem;
use rustnf::nfse::sefin::{ClienteNfse, Resultado, Status, Transporte};
use rustnf::nfse::Dps;
use serde_json::{json, Value};

use crate::erro::Erro;
use crate::nucleo;

/// Quem fala com a Sefin Nacional.
#[async_trait]
pub trait Emissor: Send + Sync {
    /// 1 producao, 2 producao restrita.
    fn ambiente(&self) -> u8;
    /// Monta, assina uma vez e envia.
    async fn emitir(&self, dps: &Dps) -> Resultado;
    /// `GET /nfse/{chave}`.
    async fn consultar(&self, chave: &str) -> Resultado;
    /// Evento 101101.
    async fn cancelar(&self, chave: &str, motivo: &str, justificativa: &str) -> Resultado;
    /// A nota tem o evento de cancelamento registrado? `Ok` = sim, `NaoEncontrada` = nao; outro resultado = nao deu
    /// para saber.
    async fn cancelada(&self, chave: &str) -> Resultado;
}

#[async_trait]
impl<T: Transporte + 'static> Emissor for ClienteNfse<T> {
    fn ambiente(&self) -> u8 {
        self.ambiente
    }

    async fn emitir(&self, dps: &Dps) -> Resultado {
        ClienteNfse::emitir(self, dps).await
    }

    async fn consultar(&self, chave: &str) -> Resultado {
        self.consultar_nfse(chave).await
    }

    async fn cancelar(&self, chave: &str, motivo: &str, justificativa: &str) -> Resultado {
        ClienteNfse::cancelar(self, chave, motivo, justificativa).await
    }

    async fn cancelada(&self, chave: &str) -> Resultado {
        ClienteNfse::cancelada(self, chave).await
    }
}

fn nome_do_status(s: Status) -> &'static str {
    match s {
        Status::Autorizada => "autorizada",
        Status::Rejeitada => "rejeitada",
        Status::Pendente => "pendente",
        Status::Indisponivel => "indisponivel",
        Status::Erro => "erro",
        Status::Ok => "ok",
        Status::NaoEncontrada => "nao_encontrada",
        Status::Registrado => "registrado",
    }
}

fn mensagens(lista: &[Mensagem]) -> Vec<Value> {
    lista
        .iter()
        .map(
            |m| json!({"codigo": m.codigo, "descricao": m.descricao, "complemento": m.complemento}),
        )
        .collect()
}

fn texto_da_rejeicao(r: &Resultado) -> String {
    let itens: Vec<String> = r.erros.iter().map(ToString::to_string).collect();
    if itens.is_empty() {
        r.frase.clone()
    } else {
        format!("{} {}", r.frase, itens.join("; "))
    }
}

/// Resultado do cliente -> JSON de sucesso ou [`Erro`]. Nunca leva o XML assinado nem o detalhe de rede.
///
/// # Errors
///
/// [`Erro::Recusa`] para rejeicao ou recusa local, [`Erro::NaoEncontrada`] e [`Erro::Rede`] para o resto.
pub fn resultado_json(r: &Resultado) -> Result<Value, Erro> {
    match r.status {
        Status::Autorizada | Status::Ok | Status::Registrado => Ok(json!({
            "status": nome_do_status(r.status),
            "frase": r.frase,
            "chave": r.chave,
            "id_dps": r.id_dps,
            "recuperada": r.recuperada,
            "alertas": mensagens(&r.alertas),
            "nfse": match &r.nfse {
                Some(n) => serde_json::to_value(n)?,
                None => Value::Null,
            },
        })),
        Status::Rejeitada => Err(Erro::recusa("sefin", texto_da_rejeicao(r))),
        // recusa local da biblioteca (chave de tamanho errado, evento fora da regra): nada saiu para a rede
        Status::Erro if r.http == 0 && r.detalhe.is_empty() => {
            Err(Erro::recusa("", r.frase.clone()))
        }
        Status::NaoEncontrada => Err(Erro::NaoEncontrada(r.frase.clone())),
        Status::Pendente | Status::Indisponivel | Status::Erro => {
            Err(Erro::Rede(texto_da_rejeicao(r)))
        }
    }
}

/// Emite a DPS: ambiente do emissor, `dhEmi` com folga se faltar, regras conferidas antes de assinar e enviar.
///
/// # Errors
///
/// Como [`resultado_json`], mais a recusa da propria montagem.
pub async fn emitir(emissor: &dyn Emissor, dps: Dps, folga_segundos: u32) -> Result<Value, Erro> {
    resultado_json(&enviar(emissor, dps, folga_segundos).await?)
}

/// Como [`emitir`], devolvendo o resultado inteiro do cliente (com o XML da nota autorizada em `xml_retorno`), para
/// quem precisa guardar a nota.
///
/// # Errors
///
/// So' a recusa da propria montagem; o que a Sefin responde vem em [`Resultado::status`].
pub async fn enviar(
    emissor: &dyn Emissor,
    mut dps: Dps,
    folga_segundos: u32,
) -> Result<Resultado, Erro> {
    nucleo::preparar_emissao(&mut dps, emissor.ambiente(), folga_segundos);
    nucleo::conferir_dps(&dps)?;
    Ok(emissor.emitir(&dps).await)
}

/// Consulta a NFS-e pela chave de acesso.
///
/// # Errors
///
/// Como [`resultado_json`].
pub async fn consultar(emissor: &dyn Emissor, chave: &str) -> Result<Value, Erro> {
    resultado_json(&emissor.consultar(chave).await)
}

/// A consulta de uma NFS-e com o que se sabe do cancelamento dela.
#[derive(Debug, Clone)]
pub struct Consultada {
    /// O resultado da consulta pela chave (com a nota e o XML, se foi achada).
    pub resultado: Resultado,
    /// A nota achada tem o cancelamento registrado? `None`: nao e' uma nota achada, ou nao deu para saber.
    pub cancelada: Option<bool>,
}

/// Consulta a NFS-e pela chave e, achando-a, confere se ha cancelamento registrado (o XML da nota nao diz). E' o
/// que uma interface usa para mostrar a situacao da nota; [`consultar`] continua devolvendo o JSON de sempre.
pub async fn consultar_completa(emissor: &dyn Emissor, chave: &str) -> Consultada {
    let resultado = emissor.consultar(chave).await;
    let cancelada = if resultado.status == Status::Autorizada && resultado.nfse.is_some() {
        match emissor.cancelada(chave).await.status {
            Status::Ok => Some(true),
            Status::NaoEncontrada => Some(false),
            _ => None,
        }
    } else {
        None
    };
    Consultada {
        resultado,
        cancelada,
    }
}

/// Cancela a NFS-e (evento 101101).
///
/// # Errors
///
/// Como [`resultado_json`].
pub async fn cancelar(
    emissor: &dyn Emissor,
    chave: &str,
    motivo: &str,
    justificativa: &str,
) -> Result<Value, Erro> {
    resultado_json(&emissor.cancelar(chave, motivo, justificativa).await)
}
