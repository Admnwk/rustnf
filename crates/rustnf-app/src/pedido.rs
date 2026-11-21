// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O fluxo de emissao e de cancelamento com a configuracao: completa o pedido com o `[emitente]`, numera a DPS pela
//! pasta de notas, emite e guarda a nota autorizada.

use rustnf::nfse::sefin::{Resultado, Status};
use rustnf::nfse::Dps;
use serde_json::{json, Value};

use crate::config::{aplicar_emitente, Emitente};
use crate::emissao::{self, Emissor};
use crate::erro::Erro;
use crate::notas::{Notas, Registro};

/// Campo ausente do pedido: nao existe, e' `null` ou e' texto vazio.
fn ausente(pedido: &Value, chave: &str) -> bool {
    match pedido.get(chave) {
        None | Some(Value::Null) => true,
        Some(Value::String(s)) => s.trim().is_empty(),
        Some(_) => false,
    }
}

/// A serie do pedido como texto; sem ela, 1 (o padrao da DPS).
fn serie_do_pedido(pedido: &Value) -> String {
    match pedido.get("serie") {
        Some(Value::String(s)) if !s.trim().is_empty() => s.trim().to_string(),
        Some(Value::Number(n)) => n.to_string(),
        _ => "1".to_string(),
    }
}

/// O que `emitir_pedido_completo` devolve: o resultado inteiro do cliente e o que se fez com a pasta de notas.
#[derive(Debug, Clone)]
pub struct Emitida {
    /// O resultado do cliente da Sefin (com o XML da nota, se autorizada, e as mensagens de rejeicao).
    pub resultado: Resultado,
    /// O numero da DPS, quando veio da pasta de notas (o pedido nao trazia).
    pub numero_dps: Option<u64>,
    /// Nota autorizada ou recuperada e pasta de notas em uso: o XML foi guardado? `None` nos outros casos.
    pub guardada: Option<bool>,
    /// Aviso quando a nota foi autorizada mas a pasta nao pode ser atualizada por completo.
    pub aviso: Option<&'static str>,
}

const AVISO_PASTA: &str = "a nota foi autorizada, mas a pasta de notas nao pode ser atualizada por completo; consulte a nota pela chave e confira a numeracao";

/// Emite a DPS do `pedido` (JSON).
///
/// 1. completa so' os campos ausentes com o `[emitente]`;
/// 2. sem `numero` no pedido e com `notas`, usa o proximo numero da serie;
/// 3. emite (veja [`emissao::emitir`]);
/// 4. nota autorizada ou recuperada: guarda o XML na pasta de notas e confirma o numero da DPS. Se a guarda falhar,
///    a emissao vale (a nota existe na Sefin): a resposta traz `guardada: false` e o `aviso`.
///
/// A resposta e' a de [`emissao::emitir`]; com `notas`, ganha `guardada` (e `numero_dps`, quando o numero veio da
/// pasta).
///
/// # Errors
///
/// Como [`emissao::emitir`], mais o erro de ler o numero da pasta de notas ou da leitura do pedido.
pub async fn emitir_pedido(
    emissor: &dyn Emissor,
    pedido: Value,
    emitente: &Emitente,
    folga_segundos: u32,
    notas: Option<&Notas>,
) -> Result<Value, Erro> {
    let emitida = emitir_pedido_completo(emissor, pedido, emitente, folga_segundos, notas).await?;
    let mut saida = emissao::resultado_json(&emitida.resultado)?;
    if let Some(guardada) = emitida.guardada {
        saida["guardada"] = json!(guardada);
    }
    if let Some(aviso) = emitida.aviso {
        saida["aviso"] = json!(aviso);
    }
    if let Some(n) = emitida.numero_dps {
        saida["numero_dps"] = json!(n);
    }
    Ok(saida)
}

/// Como [`emitir_pedido`], devolvendo o resultado do cliente como dado tipado (a rejeicao da Sefin chega com os
/// codigos e as descricoes, sem virar texto) em vez de JSON. E' o que uma interface usa para mostrar cada desfecho.
///
/// # Errors
///
/// So' a recusa da propria montagem, o erro de ler o numero da pasta de notas e o da leitura do pedido; o que a
/// Sefin responde vem em `Emitida::resultado`.
pub async fn emitir_pedido_completo(
    emissor: &dyn Emissor,
    mut pedido: Value,
    emitente: &Emitente,
    folga_segundos: u32,
    notas: Option<&Notas>,
) -> Result<Emitida, Erro> {
    aplicar_emitente(&mut pedido, emitente);
    let mut numero_da_pasta = None;
    if let (Some(notas), true) = (notas, pedido.is_object() && ausente(&pedido, "numero")) {
        let numero = notas.proximo_numero(&serie_do_pedido(&pedido))?;
        pedido["numero"] = json!(numero);
        numero_da_pasta = Some(numero);
    }
    let dps: Dps = serde_json::from_value(pedido)?;
    let serie = dps.serie.as_str().to_string();
    let numero = dps.numero.as_str().to_string();
    let resultado = emissao::enviar(emissor, dps, folga_segundos).await?;
    let mut emitida = Emitida {
        resultado,
        numero_dps: numero_da_pasta,
        guardada: None,
        aviso: None,
    };
    if let (Some(notas), Status::Autorizada) = (notas, emitida.resultado.status) {
        // o numero foi gasto na Sefin mesmo que a guarda do XML falhe
        let confirmado = confirmar(notas, &serie, &numero);
        let guardada = guardar(notas, &emitida.resultado, &serie, &numero);
        emitida.guardada = Some(guardada);
        if !(guardada && confirmado) {
            emitida.aviso = Some(AVISO_PASTA);
        }
    }
    Ok(emitida)
}

fn guardar(notas: &Notas, r: &Resultado, serie: &str, numero: &str) -> bool {
    let Some(nfse) = &r.nfse else {
        return false;
    };
    if r.xml_retorno.is_empty() {
        return false;
    }
    let mut registro = Registro::da_nfse(nfse);
    if registro.chave.is_empty() {
        registro.chave.clone_from(&r.chave);
    }
    if registro.serie.is_empty() {
        registro.serie = serie.to_string();
    }
    if registro.numero_dps.is_empty() {
        registro.numero_dps = numero.to_string();
    }
    notas.guardar(&registro, &r.xml_retorno).is_ok()
}

fn confirmar(notas: &Notas, serie: &str, numero: &str) -> bool {
    numero
        .parse::<u64>()
        .is_ok_and(|n| notas.confirmar_numero(serie, n).is_ok())
}

/// Cancela a NFS-e (veja [`emissao::cancelar`]) e, com `notas`, marca a nota como cancelada na pasta. Nota que nao
/// esta na pasta nao e' erro.
///
/// # Errors
///
/// Como [`emissao::cancelar`].
pub async fn cancelar_pedido(
    emissor: &dyn Emissor,
    chave: &str,
    motivo: &str,
    justificativa: &str,
    notas: Option<&Notas>,
) -> Result<Value, Erro> {
    let saida = emissao::cancelar(emissor, chave, motivo, justificativa).await?;
    if let Some(notas) = notas {
        // uma nota que nao esta na pasta (emitida em outro lugar) continua cancelada na Sefin
        let _ = notas.marcar_cancelada(chave);
    }
    Ok(saida)
}

#[cfg(test)]
mod testes {
    use std::future::Future;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::task::{Context, Poll, Waker};

    use async_trait::async_trait;
    use rustnf::nfse::leitura::{ler_nfse, Mensagem};

    use super::*;
    use crate::testes_comuns::PastaTemporaria;

    const DPS: &str = include_str!("../../rustnf-cli/exemplos/dps.json");
    const NFSE_BASE: &str = include_str!("../../rustnf-cli/exemplos/nfse.xml");

    /// A NFS-e de exemplo com uma chave de acesso de digito verificador certo (a do exemplo nao tem).
    fn nfse_valida() -> String {
        let chave = rustnf::chave_dfe::montar_nfse(
            1_400_159,
            "11222333000181",
            22,
            "2609",
            Some(97_579_757),
            2,
        )
        .unwrap();
        NFSE_BASE.replace("14001591299999999999999000000000000022097579757684", &chave)
    }

    /// O emissor desses testes responde na hora, sem rede.
    struct Falso {
        status: Status,
        chamadas: AtomicUsize,
    }

    #[async_trait]
    impl Emissor for Falso {
        fn ambiente(&self) -> u8 {
            2
        }

        async fn emitir(&self, _dps: &Dps) -> Resultado {
            self.chamadas.fetch_add(1, Ordering::Relaxed);
            let xml = nfse_valida();
            let nfse = ler_nfse(&xml).unwrap();
            let autorizada = self.status == Status::Autorizada;
            Resultado {
                status: self.status,
                frase: "resposta de teste".into(),
                chave: if autorizada {
                    nfse.chave.clone()
                } else {
                    String::new()
                },
                id_dps: String::new(),
                nfse: autorizada.then_some(nfse),
                xml_assinado: String::new(),
                xml_retorno: if autorizada { xml } else { String::new() },
                erros: if autorizada {
                    vec![]
                } else {
                    vec![Mensagem {
                        codigo: "E0312".into(),
                        descricao: "codigo de tributacao".into(),
                        complemento: "serv".into(),
                    }]
                },
                alertas: vec![],
                http: 200,
                recuperada: false,
                corpo: None,
                detalhe: String::new(),
            }
        }

        async fn consultar(&self, _chave: &str) -> Resultado {
            unreachable!("os testes nao consultam")
        }

        async fn cancelar(&self, _chave: &str, _motivo: &str, _justificativa: &str) -> Resultado {
            unreachable!("os testes nao cancelam")
        }
    }

    fn bloquear<T>(futuro: impl Future<Output = T>) -> T {
        let mut futuro = std::pin::pin!(futuro);
        let mut cx = Context::from_waker(Waker::noop());
        loop {
            if let Poll::Ready(v) = futuro.as_mut().poll(&mut cx) {
                return v;
            }
        }
    }

    fn pedido_sem_numero() -> Value {
        let mut p: Value = serde_json::from_str(DPS).unwrap();
        p.as_object_mut().unwrap().remove("numero");
        p
    }

    #[test]
    fn completo_devolve_a_rejeicao_com_codigo_e_a_forma_json_continua_recusa() {
        let emissor = Falso {
            status: Status::Rejeitada,
            chamadas: AtomicUsize::new(0),
        };
        let emitente = Emitente::default();
        let pasta = PastaTemporaria::nova("pedido-rejeitada");
        let notas = Notas::abrir(pasta.caminho()).unwrap();
        let e = bloquear(emitir_pedido_completo(
            &emissor,
            pedido_sem_numero(),
            &emitente,
            60,
            Some(&notas),
        ))
        .unwrap();
        assert_eq!(e.resultado.status, Status::Rejeitada);
        assert_eq!(e.resultado.erros[0].codigo, "E0312");
        assert_eq!(e.numero_dps, Some(1));
        assert_eq!(e.guardada, None);
        // rejeitada nao gasta numero
        assert_eq!(notas.proximo_numero("1").unwrap(), 1);
        // a funcao de sempre continua devolvendo a recusa em texto
        let erro = bloquear(emitir_pedido(
            &emissor,
            pedido_sem_numero(),
            &emitente,
            60,
            Some(&notas),
        ))
        .unwrap_err();
        assert!(matches!(erro, Erro::Recusa { campo, .. } if campo == "sefin"));
        assert_eq!(emissor.chamadas.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn completo_guarda_a_nota_autorizada_e_a_forma_json_diz_o_mesmo() {
        let emissor = Falso {
            status: Status::Autorizada,
            chamadas: AtomicUsize::new(0),
        };
        let emitente = Emitente::default();
        let pasta = PastaTemporaria::nova("pedido-autorizada");
        let notas = Notas::abrir(pasta.caminho()).unwrap();
        let e = bloquear(emitir_pedido_completo(
            &emissor,
            pedido_sem_numero(),
            &emitente,
            60,
            Some(&notas),
        ))
        .unwrap();
        assert_eq!(e.resultado.status, Status::Autorizada);
        assert_eq!(e.numero_dps, Some(1));
        assert_eq!(e.guardada, Some(true));
        assert_eq!(e.aviso, None);
        assert_eq!(notas.listar().unwrap().len(), 1);
        assert_eq!(notas.proximo_numero("1").unwrap(), 2);
        // a mesma DPS, na forma de sempre: JSON com `guardada` e `numero_dps`
        let json = bloquear(emitir_pedido(
            &emissor,
            pedido_sem_numero(),
            &emitente,
            60,
            Some(&notas),
        ))
        .unwrap();
        assert_eq!(json["guardada"], true);
        assert_eq!(json["numero_dps"], 2);
        assert_eq!(json["status"], "autorizada");
        // sem pasta de notas nao ha o que guardar
        let sem = bloquear(emitir_pedido_completo(
            &emissor,
            serde_json::from_str(DPS).unwrap(),
            &emitente,
            60,
            None,
        ))
        .unwrap();
        assert_eq!(sem.guardada, None);
        assert_eq!(sem.numero_dps, None);
    }
}
