// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O pedido JSON da NF-e que o assistente monta com as respostas simples (a aplicacao o completa com a configuracao, numera,
//! confere e envia).

use chrono::Datelike;
use serde_json::{json, Map, Value};

use super::mascara::{dinheiro_para_centavos, so_digitos};
use super::nfe_fiscal::{
    cst_pis_cofins, Contribuinte, Destino, Entrega, Icms, Pagamento, RegimePisCofins,
};
use super::nfe_nova::{
    centesimos_de_percentual, decimal, decimal_de_milesimos, decimal_de_percentual,
    imposto_centavos, milesimos, municipio_do_codigo, ContextoNfe, DadosNfe, ItemNfe, PendenciaNfe,
    NATUREZA,
};
use super::nfe_pendencias::{pendencias, so_letras_e_digitos};

fn percentual_texto(c: i64) -> Value {
    json!(decimal_de_percentual(c))
}

fn item_json(
    i: &ItemNfe,
    destino: Destino,
    c: &ContextoNfe,
    regime: Option<RegimePisCofins>,
) -> Value {
    let produto = i.produto_centavos().unwrap_or(0);
    let desconto = i.desconto_centavos().unwrap_or(0).clamp(0, produto);
    let base = produto - desconto;
    let mut m = Map::new();
    let codigo = if i.codigo.trim().is_empty() {
        "S/C".to_string()
    } else {
        i.codigo.trim().to_string()
    };
    m.insert("codigo".into(), json!(codigo));
    m.insert("descricao".into(), json!(i.descricao.trim()));
    m.insert("ncm".into(), json!(so_digitos(&i.ncm, 8)));
    if !i.cest.trim().is_empty() {
        m.insert("cest".into(), json!(so_digitos(&i.cest, 7)));
    }
    m.insert(
        "cfop".into(),
        json!(i.cfop_efetivo(destino, c.crt).unwrap_or_default()),
    );
    m.insert("unidade".into(), json!(i.unidade.trim().to_uppercase()));
    m.insert(
        "quantidade".into(),
        json!(decimal_de_milesimos(i.quantidade_milesimos().unwrap_or(0))),
    );
    m.insert(
        "valor_unitario".into(),
        json!(decimal(i.valor_centavos().unwrap_or(0))),
    );
    m.insert("valor_produto".into(), json!(decimal(produto)));
    if desconto > 0 {
        m.insert("desconto".into(), json!(decimal(desconto)));
    }
    m.insert("icms".into(), icms_json(i, base, c));
    let (pis, cofins) = pis_cofins_json(i, base, c, regime);
    m.insert("pis".into(), pis);
    m.insert("cofins".into(), cofins);
    Value::Object(m)
}

fn icms_json(i: &ItemNfe, base: i64, c: &ContextoNfe) -> Value {
    let codigo = i.icms_codigo_efetivo(c.crt).unwrap_or_default();
    let mut m = Map::new();
    m.insert("codigo".into(), json!(codigo));
    m.insert("orig".into(), json!(i.origem_mercadoria));
    let manual = i.icms_codigo.as_ref().is_some_and(|c| !c.trim().is_empty());
    match (i.icms_efetivo(c.crt), manual) {
        (Some(Icms::Tributado), false) => {
            let a = centesimos_de_percentual(&i.aliquota_icms).unwrap_or(0);
            m.insert("vbc".into(), json!(decimal(base)));
            m.insert("p_icms".into(), percentual_texto(a));
            m.insert("v_icms".into(), json!(decimal(imposto_centavos(base, a))));
        }
        (Some(Icms::JaCobradoAntes), false) => {
            let a = centesimos_de_percentual(&i.st_aliquota).unwrap_or(0);
            m.insert(
                "vbc_st_ret".into(),
                json!(decimal(dinheiro_para_centavos(&i.st_base).unwrap_or(0))),
            );
            m.insert("p_st".into(), percentual_texto(a));
            m.insert(
                "v_icms_st_ret".into(),
                json!(decimal(dinheiro_para_centavos(&i.st_valor).unwrap_or(0))),
            );
        }
        _ => {}
    }
    Value::Object(m)
}

fn pis_cofins_json(
    i: &ItemNfe,
    base: i64,
    c: &ContextoNfe,
    regime: Option<RegimePisCofins>,
) -> (Value, Value) {
    let cst = i
        .pis_cst
        .as_ref()
        .filter(|s| !s.trim().is_empty())
        .map_or_else(
            || cst_pis_cofins(c.crt).to_string(),
            |s| s.trim().to_string(),
        );
    let tributa = cst == "01" || cst == "02";
    if !tributa {
        return (json!({"cst": cst}), json!({"cst": cst}));
    }
    let (ap, ac) = regime.map_or(("0", "0"), RegimePisCofins::aliquotas);
    let bp = centesimos_de_percentual(ap).unwrap_or(0);
    let bc = centesimos_de_percentual(ac).unwrap_or(0);
    (
        json!({"cst": cst, "vbc": decimal(base), "aliquota": ap, "valor": decimal(imposto_centavos(base, bp))}),
        json!({"cst": cst, "vbc": decimal(base), "aliquota": ac, "valor": decimal(imposto_centavos(base, bc))}),
    )
}

fn dest_json(d: &DadosNfe, c: &ContextoNfe) -> Value {
    let (municipio, uf) = municipio_do_codigo(&d.cod_municipio).unwrap_or_default();
    let mut endereco = Map::new();
    for (k, v) in [
        ("logradouro", d.logradouro.trim().to_string()),
        ("numero", d.numero.trim().to_string()),
        ("complemento", d.complemento.trim().to_string()),
        ("bairro", d.bairro.trim().to_string()),
        ("cod_municipio", so_digitos(&d.cod_municipio, 7)),
        ("municipio", municipio),
        ("uf", uf),
        ("cep", so_digitos(&d.cep, 8)),
    ] {
        if !v.is_empty() {
            endereco.insert(k.into(), json!(v));
        }
    }
    let contribuinte = d.contribuinte.unwrap_or(Contribuinte::Nao);
    let mut m = Map::new();
    m.insert("documento".into(), json!(d.documento_limpo()));
    m.insert("nome".into(), json!(d.nome.trim()));
    m.insert("ind_ie_dest".into(), json!(contribuinte.ind_ie_dest()));
    if contribuinte == Contribuinte::ComIe {
        m.insert("ie".into(), json!(so_letras_e_digitos(&d.ie)));
    }
    if !d.email.trim().is_empty() {
        m.insert("email".into(), json!(d.email.trim()));
    }
    m.insert("endereco".into(), Value::Object(endereco));
    let _ = c;
    Value::Object(m)
}

fn pagamento_json(d: &DadosNfe, total: i64) -> Value {
    let mut p = Map::new();
    p.insert("t_pag".into(), json!(d.pagamento.t_pag()));
    p.insert("valor".into(), json!(decimal(total)));
    p.insert("ind_pag".into(), json!(d.pagamento.ind_pag()));
    if d.pagamento.e_cartao() {
        p.insert("cartao".into(), json!({"tp_integra": 2}));
    }
    json!([Value::Object(p)])
}

fn cobranca_json(d: &DadosNfe, c: &ContextoNfe) -> Option<Value> {
    if d.pagamento != Pagamento::APrazo {
        return None;
    }
    let parcelas = d.parcelas_da_nota();
    if parcelas.is_empty() {
        return None;
    }
    let total = d.totais().total;
    let dups: Vec<Value> = parcelas
        .iter()
        .enumerate()
        .map(|(k, (valor, venc))| {
            json!({
                "n_dup": format!("{:03}", k + 1),
                "d_venc": format!("{:04}-{:02}-{:02}", venc.year(), venc.month(), venc.day()),
                "v_dup": decimal(*valor),
            })
        })
        .collect();
    Some(json!({
        "fat": {"n_fat": c.numero_provisorio.max(1).to_string(), "v_orig": decimal(total), "v_liq": decimal(total)},
        "dup": dups,
    }))
}

fn transporte_json(d: &DadosNfe) -> Option<Value> {
    let mut t = Map::new();
    if d.entrega.com_transportadora() {
        let mut x = Map::new();
        let doc = so_letras_e_digitos(&d.transp_documento);
        for (k, v) in [
            ("documento", doc),
            ("nome", d.transp_nome.trim().to_string()),
            ("ie", so_letras_e_digitos(&d.transp_ie)),
            ("endereco", d.transp_endereco.trim().to_string()),
            ("municipio", d.transp_municipio.trim().to_string()),
            ("uf", d.transp_uf.trim().to_uppercase()),
        ] {
            if !v.is_empty() {
                x.insert(k.into(), json!(v));
            }
        }
        t.insert("transporta".into(), Value::Object(x));
    }
    let volumes = d.volumes.trim().parse::<u64>().unwrap_or(0);
    if volumes > 0 && d.entrega != Entrega::Balcao {
        let mut v = Map::new();
        v.insert("q_vol".into(), json!(volumes.to_string()));
        if !d.especie.trim().is_empty() {
            v.insert("esp".into(), json!(d.especie.trim()));
        }
        if let Some(kg) = milesimos(&d.peso_bruto).filter(|k| *k > 0) {
            v.insert("peso_b".into(), json!(decimal_de_milesimos(kg)));
        }
        t.insert("vol".into(), json!([Value::Object(v)]));
    }
    (!t.is_empty()).then(|| Value::Object(t))
}

/// O pedido JSON da NF-e (o que `rustnf_app::nfe` completa com a configuracao e confere). Com pendencia, devolve a lista.
///
/// # Errors
///
/// As pendencias, quando algo falta para montar o pedido.
pub fn pedido(d: &DadosNfe, c: &ContextoNfe) -> Result<Value, Vec<PendenciaNfe>> {
    let pend = pendencias(d, c);
    if !pend.is_empty() {
        return Err(pend);
    }
    let destino = d.destino(c).ok_or_else(Vec::new)?;
    let consumo = d.consumo.ok_or_else(Vec::new)?;
    let regime = c.regime_pis_efetivo(d);
    let itens: Vec<Value> = d
        .itens
        .iter()
        .map(|i| item_json(i, destino, c, regime))
        .collect();
    let total = d.totais().total;
    let mut m = Map::new();
    m.insert("nat_op".into(), json!(NATUREZA));
    m.insert("id_dest".into(), json!(destino.id_dest()));
    m.insert("ind_final".into(), json!(consumo.ind_final()));
    m.insert("ind_pres".into(), json!(d.presenca.ind_pres()));
    if let Some(i) = d.presenca.ind_intermed() {
        m.insert("ind_intermed".into(), json!(i));
    }
    m.insert("mod_frete".into(), json!(d.entrega.mod_frete()));
    m.insert("dest".into(), dest_json(d, c));
    m.insert("itens".into(), Value::Array(itens));
    m.insert("pagamentos".into(), pagamento_json(d, total));
    if let Some(cobr) = cobranca_json(d, c) {
        m.insert("cobr".into(), cobr);
    }
    if let Some(t) = transporte_json(d) {
        m.insert("transp".into(), t);
    }
    Ok(Value::Object(m))
}
