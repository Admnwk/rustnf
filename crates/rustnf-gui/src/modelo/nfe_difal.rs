// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O DIFAL da Nova NF-e: na venda para outro estado a consumidor final que nao e' contribuinte do ICMS, a biblioteca
//! EXIGE o grupo `icms_uf_dest` em cada produto (rejeicao 694) e recusa a nota sem ele. Aqui esta a condicao que a tela
//! usa para pedir ao vendedor, em palavras simples, o que a biblioteca nao sabe (a aliquota interna do ICMS da UF do
//! cliente e o adicional do Fundo de Combate a Pobreza) e o pedaco do pedido que leva esses dados. O resto (a aliquota
//! interestadual de 4, 7 ou 12 %, a partilha do ano, a base e os valores) a tela nao pede: a biblioteca calcula
//! (`rustnf::nfe::montagem::difal`).

use chrono::Datelike;
use rustnf::nfe::montagem::difal;
use serde_json::{json, Value};

use super::nfe_fiscal::{Contribuinte, Destino};
use super::nfe_nova::{
    centesimos_de_percentual, decimal_de_percentual, municipio_do_codigo, Consumo, ContextoNfe,
    DadosNfe, ItemNfe, PassoNfe, PendenciaNfe,
};

/// Os CFOP em que a biblioteca nao exige o grupo (`difal.rs`, `CFOP_FORA`).
const CFOP_SEM_DIFAL: [&str; 3] = ["6552", "6922", "6929"];

/// Os CST/CSOSN em que a biblioteca nao exige o grupo (`difal.rs`, `CST_SEM_DIFAL`): isenta, nao tributada, imunidade,
/// suspensao e o CSOSN 103, 300 e 400 do Simples.
const CST_SEM_DIFAL: [&str; 5] = ["40", "41", "103", "300", "400"];

/// A UF do cliente (a do municipio do endereco).
#[must_use]
pub fn uf_do_cliente(d: &DadosNfe) -> Option<String> {
    municipio_do_codigo(&d.cod_municipio).map(|(_, uf)| uf)
}

/// A biblioteca exige o grupo `icms_uf_dest` neste produto? Espelha `exige_grupo` da biblioteca
/// (`crates/rustnf/src/nfe/montagem/difal.rs:174`): a venda e' para outro estado (`idDest` 2), a consumidor final
/// (`indFinal` 1), nao contribuinte (`indIEDest` 9) e nao e' servico; e nao e' do Simples comum (CRT 1), nem tem CFOP
/// ou CST/CSOSN da lista sem DIFAL. As demais excecoes da biblioteca (partilha, entrada, retorno e remessa, finalidade
/// de complemento ou ajuste, combustivel fora da lista, entrega na UF do emitente) o assistente nao tem: ele so' emite
/// venda normal, sem local de entrega.
///
/// A prova de que a condicao bate esta nos testes (`nfe_nova/testes_difal.rs`): o pedido passa pela montagem estrita da
/// aplicacao com e sem o grupo.
#[must_use]
pub fn exige_o_grupo(d: &DadosNfe, i: &ItemNfe, c: &ContextoNfe) -> bool {
    let Some(destino) = d.destino(c) else {
        return false;
    };
    if destino != Destino::OutroEstado
        || d.consumo != Some(Consumo::Final)
        || d.contribuinte != Some(Contribuinte::Nao)
        || c.crt == 1
    {
        return false;
    }
    let cfop = i.cfop_efetivo(destino, c.crt, d.contribuinte);
    let cfop = cfop.unwrap_or_default().replace('.', "");
    let cst = i.icms_codigo_efetivo(c.crt).unwrap_or_default();
    !(CFOP_SEM_DIFAL.contains(&cfop.as_str()) || CST_SEM_DIFAL.contains(&cst.as_str()))
}

/// Algum produto da nota exige o grupo.
#[must_use]
pub fn nota_exige_o_grupo(d: &DadosNfe, c: &ContextoNfe) -> bool {
    d.itens.iter().any(|i| exige_o_grupo(d, i, c))
}

/// O que falta do DIFAL de um produto: a aliquota interna da UF do cliente e, se digitado, o FCP.
pub(super) fn pendencias_do_item(
    d: &DadosNfe,
    i: &ItemNfe,
    n: usize,
    nome: &str,
    c: &ContextoNfe,
) -> Vec<PendenciaNfe> {
    if !exige_o_grupo(d, i, c) {
        return Vec::new();
    }
    let uf = uf_do_cliente(d).unwrap_or_default();
    let onde = "Procure no site da Secretaria da Fazenda de ".to_string()
        + &uf
        + " ou pergunte ao seu contador: o programa não tem a tabela de alíquotas dos estados.";
    let mut p = Vec::new();
    let aliquota = centesimos_de_percentual(&i.difal_aliquota);
    if i.difal_aliquota.trim().is_empty() {
        p.push(
            PendenciaNfe::nova(
                PassoNfe::OQueVendeu,
                format!("Falta a alíquota do ICMS de {uf} para {nome}."),
                format!("Este cliente é de outro estado e não tem inscrição: digite a alíquota interna do ICMS de {uf} (por exemplo 18). {onde}"),
            )
            .do_item(n),
        );
    } else if aliquota.is_none_or(|a| a == 0 || a > 10_000) {
        p.push(
            PendenciaNfe::nova(
                PassoNfe::OQueVendeu,
                format!("A alíquota do ICMS de {uf} para {nome} não serve."),
                "Digite só o percentual, maior que zero (por exemplo 18 ou 17,5).",
            )
            .do_item(n),
        );
    }
    if !i.difal_fcp.trim().is_empty()
        && centesimos_de_percentual(&i.difal_fcp).is_none_or(|f| f > 200)
    {
        p.push(
            PendenciaNfe::nova(
                PassoNfe::OQueVendeu,
                format!("O adicional do fundo de combate à pobreza de {nome} não serve."),
                "Digite de 0 a 2 (por exemplo 2), ou deixe vazio se o estado não cobra.",
            )
            .do_item(n),
        );
    }
    p
}

/// O grupo `icms_uf_dest` do produto, no formato que a biblioteca le (`json.rs`, `difal_de_json`): a base em centavos,
/// a aliquota interna e o FCP da UF do cliente (digitados), a aliquota interestadual e o ano (da biblioteca). `None` =
/// o produto nao exige o grupo ou faltam os dados (as pendencias dizem o que).
#[must_use]
pub fn grupo_json(d: &DadosNfe, i: &ItemNfe, c: &ContextoNfe, base_centavos: i64) -> Option<Value> {
    if !exige_o_grupo(d, i, c) {
        return None;
    }
    let uf_cliente = uf_do_cliente(d)?;
    let interna = decimal_de_percentual(centesimos_de_percentual(&i.difal_aliquota)?);
    let fcp = if i.difal_fcp.trim().is_empty() {
        "0".to_string()
    } else {
        decimal_de_percentual(centesimos_de_percentual(&i.difal_fcp)?)
    };
    let inter =
        difal::aliquota_interestadual(&c.uf_emitente, &uf_cliente, i64::from(i.origem_mercadoria))
            .ok()?;
    Some(json!({
        "vbc": base_centavos,
        "aliquota_interna": interna,
        "aliquota_inter": inter,
        "ano": c.hoje.year(),
        "fcp": fcp,
    }))
}
