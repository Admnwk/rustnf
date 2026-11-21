// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Operacoes sem I/O que a linha de comando, a API HTTP e o MCP compartilham: cada uma le a entrada ja em memoria,
//! chama a biblioteca e devolve o resultado. Nenhuma regra fiscal mora aqui.

use rustnf::danfse::{self, Opcoes, Situacao};
use rustnf::nfse::{self, base, Dps};
use rustnf::reforma::aliquotas::Publicadas;
use rustnf::reforma::previa::{self, EntradaBase, EntradaTributos};
use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::erro::Erro;

/// Pedido de `calcular`: a base de calculo e/ou os tributos do IBS/CBS.
///
/// `tributos` fica como JSON porque, quando `base` tambem vem, o campo `tributos.base` pode faltar e entao vale a
/// base do regime regular calculada.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Operacao {
    /// Entrada de `reforma::previa::bases`.
    #[serde(default)]
    pub base: Option<EntradaBase>,
    /// Entrada de `reforma::previa::tributos`.
    #[serde(default)]
    pub tributos: Option<Value>,
}

/// Pedido de cancelamento (evento 101101).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PedidoCancelamento {
    /// Chave de acesso da NFS-e (50 posicoes).
    pub chave: String,
    /// Codigo do motivo: 1, 2 ou 9.
    pub motivo: String,
    /// Justificativa.
    pub justificativa: String,
}

/// Le o JSON de uma DPS.
///
/// # Errors
///
/// [`Erro::Entrada`] se o JSON nao servir.
pub fn ler_dps(texto: &str) -> Result<Dps, Erro> {
    Ok(serde_json::from_str(texto)?)
}

fn comparar(dado: &Value, modelo: &Value, caminho: &str, saida: &mut Vec<String>) {
    match (dado, modelo) {
        (Value::Object(d), Value::Object(m)) => {
            for (nome, valor) in d {
                let aqui = if caminho.is_empty() {
                    nome.clone()
                } else {
                    format!("{caminho}.{nome}")
                };
                match m.get(nome) {
                    Some(conhecido) => comparar(valor, conhecido, &aqui, saida),
                    None => saida.push(aqui),
                }
            }
        }
        (Value::Array(d), Value::Array(m)) => {
            for (i, (valor, conhecido)) in d.iter().zip(m).enumerate() {
                comparar(valor, conhecido, &format!("{caminho}[{i}]"), saida);
            }
        }
        _ => {}
    }
}

/// Os campos do JSON do `pedido` que a DPS nao le, em ordem alfabetica: nome que nao existe naquele lugar (erro de
/// digitacao, campo de outro grupo). A leitura da DPS os ignora em silencio; aqui eles sao apontados pelo caminho
/// (`tomador.endereco.cepp`, `valores.deducao.documentos[1].valor`). Pedido que nem chega a ser lido como DPS
/// devolve a lista vazia: quem diz esse erro e' a leitura.
#[must_use]
pub fn campos_desconhecidos(pedido: &Value) -> Vec<String> {
    // a DPS lida e escrita de volta traz todos os campos que ela conhece: o que sobra no pedido e' desconhecido
    let Ok(conhecido) =
        serde_json::from_value::<Dps>(pedido.clone()).and_then(|dps| serde_json::to_value(&dps))
    else {
        return Vec::new();
    };
    let mut saida = Vec::new();
    comparar(pedido, &conhecido, "", &mut saida);
    saida.sort();
    saida
}

/// Recusa o pedido que tem campo desconhecido, com o primeiro no `campo` e os demais na mensagem.
///
/// # Errors
///
/// [`Erro::Recusa`] quando [`campos_desconhecidos`] acha algum.
pub fn recusar_desconhecidos(pedido: &Value) -> Result<(), Erro> {
    let achados = campos_desconhecidos(pedido);
    let Some(primeiro) = achados.first() else {
        return Ok(());
    };
    let mensagem = if achados.len() == 1 {
        "campo desconhecido no pedido: confira o nome".to_string()
    } else {
        format!(
            "campo desconhecido no pedido: confira o nome (tambem: {})",
            achados[1..].join(", ")
        )
    };
    Err(Erro::recusa(primeiro.as_str(), mensagem))
}

/// Le o JSON de `calcular`.
///
/// # Errors
///
/// [`Erro::Entrada`] se o JSON nao servir.
pub fn ler_operacao(texto: &str) -> Result<Operacao, Erro> {
    Ok(serde_json::from_str(texto)?)
}

/// XML da DPS, nao assinado. As regras do Anexo I rodam na montagem.
///
/// # Errors
///
/// [`Erro::Recusa`] com o campo e o motivo.
pub fn montar_dps(dps: &Dps) -> Result<String, Erro> {
    Ok(nfse::montar(dps)?)
}

/// So' confere: monta e descarta o XML.
///
/// # Errors
///
/// [`Erro::Recusa`] com o campo e o motivo.
pub fn conferir_dps(dps: &Dps) -> Result<(), Erro> {
    montar_dps(dps).map(|_| ())
}

fn completar_base(tributos: &mut Value, bases: Option<&previa::Bases>) -> Result<(), Erro> {
    let Some(objeto) = tributos.as_object_mut() else {
        return Err(Erro::Entrada("\"tributos\" deve ser um objeto".into()));
    };
    if objeto.contains_key("base") {
        return Ok(());
    }
    let regular = bases.and_then(|b| b.regime_regular).ok_or_else(|| {
        Erro::recusa(
            "tributos.base",
            "ausente, e nao ha base do regime regular calculada para usar no lugar",
        )
    })?;
    objeto.insert("base".into(), serde_json::to_value(regular)?);
    Ok(())
}

/// Previa do IBS/CBS. A resposta traz `bases` e/ou `tributos` e o campo `recusado`: `true` quando a base calculada
/// registrou recusas de regra (`bases.erros`); a conta sai mesmo assim, para mostrar de onde vem o problema.
///
/// # Errors
///
/// [`Erro::Entrada`] se nao vier nem `base` nem `tributos`; [`Erro::Recusa`] se a biblioteca nao puder calcular.
pub fn calcular(op: &Operacao) -> Result<Value, Erro> {
    calcular_com(op, &Publicadas::new())
}

/// Como [`calcular`], com `padrao` como as aliquotas publicadas de `tributos` quando o pedido nao traz as suas
/// (`publicadas`). O que o pedido traz vale sobre o `padrao`.
///
/// # Errors
///
/// Os mesmos de [`calcular`].
pub fn calcular_com(op: &Operacao, padrao: &Publicadas) -> Result<Value, Erro> {
    if op.base.is_none() && op.tributos.is_none() {
        return Err(Erro::Entrada(
            "informe \"base\" e/ou \"tributos\" no JSON".into(),
        ));
    }
    let mut saida = Map::new();
    let bases = op.base.as_ref().map(previa::bases).transpose()?;
    let mut recusado = false;
    if let Some(b) = &bases {
        recusado = !b.erros.is_empty();
        saida.insert("bases".into(), serde_json::to_value(b)?);
    }
    if let Some(t) = &op.tributos {
        let mut t = t.clone();
        completar_base(&mut t, bases.as_ref())?;
        let mut entrada: EntradaTributos = serde_json::from_value(t)?;
        if entrada.publicadas.is_none() && !padrao.is_empty() {
            entrada.publicadas = Some(padrao.clone());
        }
        saida.insert(
            "tributos".into(),
            serde_json::to_value(previa::tributos(&entrada)?)?,
        );
    }
    saida.insert("recusado".into(), Value::Bool(recusado));
    Ok(Value::Object(saida))
}

/// Situacao da NFS-e para o DANFSe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SituacaoNota {
    /// Nota valida.
    #[default]
    Valida,
    /// Marca d'agua de cancelada.
    Cancelada,
    /// Marca d'agua de substituida.
    Substituida,
}

impl SituacaoNota {
    /// Le `valida`, `cancelada` ou `substituida`.
    ///
    /// # Errors
    ///
    /// [`Erro::Entrada`] para qualquer outro texto.
    pub fn ler(texto: &str) -> Result<Self, Erro> {
        match texto {
            "valida" => Ok(Self::Valida),
            "cancelada" => Ok(Self::Cancelada),
            "substituida" => Ok(Self::Substituida),
            _ => Err(Erro::Entrada(
                "situacao: use valida, cancelada ou substituida".into(),
            )),
        }
    }
}

/// DANFSe em PDF a partir do XML da NFS-e autorizada. Pesado em CPU: dentro de servidor assincrono, rode em
/// `spawn_blocking`.
///
/// # Errors
///
/// [`Erro::Entrada`] se o XML nao for uma NFS-e legivel; [`Erro::Interno`] se o PDF nao sair.
pub fn gerar_danfse(xml: &str, situacao: SituacaoNota, canhoto: bool) -> Result<Vec<u8>, Erro> {
    let situacao = match situacao {
        SituacaoNota::Valida => Situacao::Valida,
        SituacaoNota::Cancelada => Situacao::Cancelada,
        SituacaoNota::Substituida => Situacao::Substituida,
    };
    let opcoes = Opcoes {
        situacao,
        canhoto,
        ..Opcoes::default()
    };
    Ok(danfse::pdf(xml, &opcoes)?)
}

/// Prepara a DPS para emitir: ambiente do servico e `dhEmi` com a folga do relogio quando o pedido nao traz.
pub fn preparar_emissao(dps: &mut Dps, ambiente: u8, folga_segundos: u32) {
    dps.ambiente = ambiente;
    if dps.emissao.as_deref().is_none_or(|e| e.trim().is_empty()) {
        dps.emissao = Some(base::agora_com_folga(folga_segundos));
    }
}

/// Resumo curto, para a tela, do resultado de uma emissao/consulta/cancelamento (`Value` de `emissao`).
#[must_use]
pub fn resumo_texto(v: &Value) -> String {
    let campo = |nome: &str| v.get(nome).and_then(Value::as_str).unwrap_or("");
    let mut linhas = vec![campo("status").to_string()];
    let mut item = |rotulo: &str, valor: &str| {
        if !valor.is_empty() {
            linhas.push(format!("{rotulo}: {valor}"));
        }
    };
    item("chave", campo("chave"));
    item("dps", campo("id_dps"));
    if let Some(numero) = v.pointer("/nfse/numero").and_then(Value::as_str) {
        item("numero", numero);
    }
    item("mensagem", campo("frase"));
    if v.get("recuperada").and_then(Value::as_bool) == Some(true) {
        linhas.push("recuperada: a nota ja existia na Sefin".into());
    }
    linhas.join("\n")
}

/// A resposta `{"ok": true}`.
#[must_use]
pub fn ok_simples() -> Value {
    json!({"ok": true})
}
