// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Leiaute da Reforma Tributaria (NT 2026.010 v1.00): o que muda no DANFE com o IBS, a CBS e o Imposto Seletivo.
//!
//! * item 4.1: bloco "Total do IBS/CBS/IS" logo depois do "Total do ICMS/IPI";
//! * item 4.2: o CRT no quadro do emitente e o campo reservado ao tipo de regime de apuracao (sem conteudo: a tag
//!   sera publicada em NT futura);
//! * item 4.3: no quadro dos produtos, a classificacao tributaria e as bases, aliquotas e valores por item; com o
//!   grupo `gRed` (reducao de aliquota ou compra governamental) imprime-se a aliquota efetiva (`pAliqEfet`), sem ele
//!   a aliquota vigente; o Imposto Seletivo fica com `pIS`;
//! * item 4.4: quadros e campos facultativos (FCP, DIFAL, monofasia, ISSQN, transporte, fatura) so' aparecem quando o
//!   XML traz a informacao;
//! * item 4.5: QR Code do `infNFeSupl`, quando existe.

use serde_json::{json, Value};

use super::formato::{brl, casas_unitario, pct, qtd};
use super::{conteudo, ErroDanfe};
use crate::nfe::modelo::{Item, Nfe};
use crate::nfe::{Decimal, Grupo};

/// Descricao do CRT no quadro do emitente (NT 2026.010, item 4.2; tabela do `CRT` do Anexo I).
fn crt_descricao(crt: &str) -> String {
    match crt {
        "1" => "1 - SIMPLES NACIONAL".into(),
        "2" => "2 - SIMPLES NACIONAL - EXCESSO DE SUBLIMITE DE RECEITA BRUTA".into(),
        "3" => "3 - REGIME NORMAL".into(),
        "4" => "4 - SIMPLES NACIONAL - MEI".into(),
        outro => outro.into(),
    }
}

fn v(g: Option<&Grupo>, tag: &str) -> Option<Decimal> {
    Some(
        g.and_then(|g| g.valores.get(tag).copied())
            .unwrap_or(Decimal::ZERO),
    )
}

fn positivo(d: Option<Decimal>) -> bool {
    d.is_some_and(|d| d != Decimal::ZERO)
}

/// NT 2026.010, item 4.3: com o grupo `gRed` informado imprime-se `pAliqEfet`; sem ele, a aliquota vigente.
fn aliquota(ib: Option<&Grupo>, sub: &str, tag: &str) -> Option<Decimal> {
    let Some(g) = ib else {
        return Some(Decimal::ZERO);
    };
    let base = format!("gIBSCBS/{sub}/");
    if g.valores
        .keys()
        .chain(g.textos.keys())
        .any(|k| k.starts_with(&format!("{base}gRed/")))
    {
        return v(ib, &format!("{base}gRed/pAliqEfet"));
    }
    v(ib, &format!("{base}{tag}"))
}

fn par(r1: &str, v1: String, r2: &str, v2: String) -> Value {
    json!([[r1, v1], [r2, v2]])
}

/// Bases (4), aliquotas (3 pares) e valores (3 pares) do item, ja formatados, e a classificacao
/// `[NCM x] [cClassTrib y]` (NT 2026.010, item 4.3). Grupo ausente = zero.
fn tributos_item(it: &Item) -> Value {
    let imp = &it.imposto;
    let (icms, ipi, is, ib) = (
        imp.icms.as_ref(),
        imp.ipi.as_ref(),
        imp.seletivo.as_ref(),
        imp.ibscbs.as_ref(),
    );
    let mut clas = Vec::new();
    if !it.ncm.is_empty() {
        clas.push(format!("[NCM {}]", it.ncm));
    }
    if let Some(c) = ib.map(|g| g.texto("cClassTrib")).filter(|c| !c.is_empty()) {
        clas.push(format!("[cClassTrib {c}]"));
    }
    let d = |g: Option<&Grupo>, tag: &str| brl(v(g, tag), 2);
    json!({
        "bases": [
            ["ICMS", d(icms, "vBC")], ["IBS / CBS", d(ib, "gIBSCBS/vBC")],
            ["IS", d(is, "vBCIS")], ["IPI", d(ipi, "vBC")]
        ],
        "aliquotas": [
            par("ICMS", pct(v(icms, "pICMS")), "CBS", pct(aliquota(ib, "gCBS", "pCBS"))),
            par("IBS UF", pct(aliquota(ib, "gIBSUF", "pIBSUF")), "IPI", pct(v(ipi, "pIPI"))),
            par("IBS MUN", pct(aliquota(ib, "gIBSMun", "pIBSMun")), "IS", pct(v(is, "pIS")))
        ],
        "valores": [
            par("ICMS", d(icms, "vICMS"), "CBS", d(ib, "gIBSCBS/gCBS/vCBS")),
            par("IBS UF", d(ib, "gIBSCBS/gIBSUF/vIBSUF"), "IPI", d(ipi, "vIPI")),
            par("IBS MUN", d(ib, "gIBSCBS/gIBSMun/vIBSMun"), "IS", d(is, "vIS"))
        ],
        "classificacao": clas.join(" "),
    })
}

fn campo(rotulo: &str, valor: String) -> Value {
    json!([rotulo, valor])
}

/// O bloco do leiaute RTC: CRT, totais em tres blocos (linhas facultativas so' com dado), condicionais e itens.
pub(super) fn dados(nfe: &Nfe) -> Result<Value, ErroDanfe> {
    let t = &nfe.total;
    let pos = |k: &str| positivo(Some(t.v(k)));
    let tib = t.ibscbs_tot.as_ref();
    let tv = |k: &str| brl(v(tib, k), 2);
    let b = |k: &str| brl(Some(t.v(k)), 2);
    let ver_fcp = ["vFCP", "vFCPST", "vICMSUFDest", "vFCPUFDest"]
        .iter()
        .any(|k| pos(k));
    let ver_mono_icms = ["qBCMono", "vICMSMono", "qBCMonoReten", "vICMSMonoReten"]
        .iter()
        .any(|k| pos(k));
    let ver_mono_ibs = [
        "gMono/vIBSMono",
        "gMono/vCBSMono",
        "gMono/vIBSMonoReten",
        "gMono/vCBSMonoReten",
    ]
    .iter()
    .any(|k| positivo(v(tib, k)));
    let ver_issqn = t.issqn_tot.as_ref().is_some_and(|g| {
        ["vServ", "vBC", "vISS"]
            .iter()
            .any(|k| g.valor(k) != Decimal::ZERO)
    });
    let tr = nfe.transporte.as_ref();
    let tp = tr.and_then(|x| x.transportadora.as_ref());
    let ver_transporte = tr.is_some_and(|x| {
        tp.is_some_and(|p| !p.documento.is_empty() || !p.nome.is_empty())
            || x.veiculo.get("placa").is_some_and(|s| !s.is_empty())
            || !x.volumes.is_empty()
    });
    let itens: Vec<Value> = nfe
        .itens
        .iter()
        .map(|it| {
            json!({
                "desc": it.descricao, "adic": it.informacao_adicional,
                "cst": it.imposto.cst_icms(), "cfop": it.cfop,
                "qtd": qtd(it.quantidade), "un": it.unidade,
                "vun": brl(it.valor_unitario, casas_unitario(it.valor_unitario)),
                "vtot": brl(it.valor_produto, 2), "trib": tributos_item(it),
            })
        })
        .collect();
    let facultativa = |ver: bool, campos: Vec<Value>| if ver { json!(campos) } else { Value::Null };
    Ok(json!({
        "crt": crt_descricao(&nfe.emit.crt),
        "totais": [
            campo("Valor total dos produtos", b("vProd")), campo("Valor do frete", b("vFrete")),
            campo("Valor do seguro", b("vSeg")), campo("Desconto", b("vDesc")),
            campo("Outras despesas acessórias", b("vOutro"))
        ],
        "vnf": b("vNF"),
        "icmsipi": [
            campo("Base de cálculo do ICMS", b("vBC")), campo("Valor do ICMS", b("vICMS")),
            campo("Base de cálculo do ICMS ST", b("vBCST")), campo("Valor do ICMS ST", b("vST")),
            campo("Valor do IPI", b("vIPI"))
        ],
        "fcp": facultativa(ver_fcp, vec![
            campo("Valor do FCP", b("vFCP")), campo("Valor do FCP retido por ST", b("vFCPST")),
            campo("Valor do DIFAL na UF de destino", b("vICMSUFDest")),
            campo("Valor do FCP na UF de destino", b("vFCPUFDest"))
        ]),
        "mono_icms": facultativa(ver_mono_icms, vec![
            campo("BC do ICMS monofásico", qtd(Some(t.v("qBCMono")))),
            campo("Valor do ICMS monofásico", b("vICMSMono")),
            campo("BC do ICMS monofásico por retenção", qtd(Some(t.v("qBCMonoReten")))),
            campo("Valor do ICMS monofásico por retenção", b("vICMSMonoReten"))
        ]),
        "ibscbs": [
            campo("Valor da CBS", tv("gCBS/vCBS")), campo("Valor do IBS UF", tv("gIBS/gIBSUF/vIBSUF")),
            campo("Valor do IBS município", tv("gIBS/gIBSMun/vIBSMun")),
            campo("Valor do imposto seletivo", brl(v(t.is_tot.as_ref(), "vIS"), 2))
        ],
        "mono_ibs": facultativa(ver_mono_ibs, vec![
            campo("Valor do IBS monofásico", tv("gMono/vIBSMono")),
            campo("Valor da CBS monofásica", tv("gMono/vCBSMono")),
            campo("Valor do IBS monofásico por retenção", tv("gMono/vIBSMonoReten")),
            campo("Valor da CBS monofásica por retenção", tv("gMono/vCBSMonoReten"))
        ]),
        "issqn": ver_issqn,
        "transporte": ver_transporte,
        "fatura": nfe.cobranca.as_ref().is_some_and(|c| !c.duplicatas.is_empty()),
        "itens": itens,
    }))
}

/// O leiaute RTC vale para a emissao a partir da vigencia da NT 2026.010 (`nt2026_010.danfe_rtc`), na data da
/// emissao no fuso da UF do emitente e no ambiente da nota.
pub(super) fn vigente(nfe: &Nfe) -> bool {
    use crate::nfe::montagem::na_uf;
    use crate::nfe::vigencias::{self, Ambiente};
    let Some(em) = nfe.ide.emissao.as_ref() else {
        return false;
    };
    let ambiente = if nfe.homologacao() {
        Ambiente::Homologacao
    } else {
        Ambiente::Producao
    };
    let dia = na_uf(&em.0, conteudo::uf_emitente(nfe)).date_naive();
    vigencias::inicio("nt2026_010.danfe_rtc", ambiente).is_some_and(|inicio| dia >= inicio)
}
