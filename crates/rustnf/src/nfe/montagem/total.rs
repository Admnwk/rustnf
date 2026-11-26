// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Totais da nota (`total`), pagamento, informacoes adicionais e responsavel tecnico, e o acumulador
//! que os itens alimentam.

use sha1::{Digest, Sha1};

use super::formato::*;
use super::icms_mono::TOT_MONO;
use super::modelo::*;
use super::*;

/// Acumulador dos totais: cada item soma os seus valores, o `total` escreve a soma.
#[derive(Default)]
pub(crate) struct Ac {
    pub(crate) v: BTreeMap<&'static str, Dec>,
    /// Soma do `vProdLiq` (NT 2026.008, W01a); `None` = nenhum item com valor liquido.
    pub(crate) liq: Option<Dec>,
}

impl Ac {
    pub(crate) fn add(&mut self, k: &'static str, d: Dec) {
        let a = *self.v.get(k).unwrap_or(&Dec::ZERO);
        self.v.insert(k, a.mais(d));
    }
    pub(crate) fn get(&self, k: &str) -> Dec {
        *self.v.get(k).unwrap_or(&Dec::ZERO)
    }
}

/// Chave interna do acumulador: `vICMSDeson` dos itens com `indDeduzDeson` 1 (nao sai nos totais, so' deduz do `vNF`).
pub(crate) const DESON_DEDUZ: &str = "_deson_deduz";

/// Totais que o `ICMSTot` repete na resposta de `montar` (alem do `vNF` e dos opcionais).
pub(crate) const ACS: [&str; 12] = [
    "vBC", "vICMS", "vFCP", "vProd", "vFrete", "vSeg", "vDesc", "vOutro", "vIPI", "vPIS",
    "vCOFINS", "vTotTrib",
];

/// Faturamento direto de veiculo novo (tpOp 2) em algum item: o `vNF` nao soma o ST (W16-10, excecao 1).
fn direto(n: &Nota) -> bool {
    n.itens
        .iter()
        .any(|it| it.veiculo.as_ref().is_some_and(|v| v.tp_op.trim() == "2"))
}

/// Grupo `total`: `ICMSTot`, `IBSCBSTot` e `vNFTot`. O `vNF` segue a W16-10: + vST + vFCPST + vICMSMonoReten (fora o faturamento
/// direto de veiculo), - vICMSDeson dos itens com `indDeduzDeson` 1 (excecao 4).
pub(crate) fn total(x: &mut X, n: &Nota, ac: &Ac, uf: &str) -> Res<BTreeMap<String, String>> {
    let st_no_vnf = if direto(n) {
        Dec::ZERO
    } else {
        ac.get("vST")
            .mais(ac.get("vFCPST"))
            .mais(ac.get("vICMSMonoReten"))
    };
    let vnf = ac
        .get("vProd")
        .menos(ac.get("vDesc"))
        .menos(ac.get(DESON_DEDUZ))
        .mais(st_no_vnf)
        .mais(ac.get("vFrete"))
        .mais(ac.get("vSeg"))
        .mais(ac.get("vOutro"))
        .mais(ac.get("vIPI"));
    let com_st = ac.v.contains_key("vST");
    x.abre("total");
    if let Some(liq) = ac.liq {
        // NT 2026.008, W01a-10 (1282) e W01a-20 (1283); PL_010h: primeiro filho do total
        x.t("vProdLiqTot", &num(liq, 2, "vProdLiqTot")?);
    }
    x.abre("ICMSTot");
    let z = Dec::ZERO;
    let difal = ac.v.contains_key("vICMSUFDest");
    // W04a (795): soma do vICMSDeson de TODOS os itens
    let mut pre = vec![
        ("vBC", ac.get("vBC")),
        ("vICMS", ac.get("vICMS")),
        ("vICMSDeson", ac.get("vICMSDeson")),
    ];
    if difal {
        // W04c, W04e e W04g (798, 799 e 800): so' com item de DIFAL; sao a soma dos itens
        pre.extend([
            ("vFCPUFDest", ac.get("vFCPUFDest")),
            ("vICMSUFDest", ac.get("vICMSUFDest")),
            ("vICMSUFRemet", ac.get("vICMSUFRemet")),
        ]);
    }
    for (tag, v) in pre {
        x.t(tag, &num(v, 2, tag)?);
    }
    for (tag, v) in [
        ("vFCP", ac.get("vFCP")),
        ("vBCST", ac.get("vBCST")),
        ("vST", ac.get("vST")),
        ("vFCPST", ac.get("vFCPST")),
        ("vFCPSTRet", ac.get("vFCPSTRet")),
    ] {
        x.t(tag, &num(v, 2, tag)?);
    }
    for tag in TOT_MONO {
        // W06b.1 a W06e: ocorrencia 0-1, so' com valor; 2 casas (`TDec_1302`)
        let v = ac.get(tag).arred(2);
        if !v.eh_zero() {
            x.t(tag, &num(v, 2, tag)?);
        }
    }
    for (tag, v) in [
        ("vProd", ac.get("vProd")),
        ("vFrete", ac.get("vFrete")),
        ("vSeg", ac.get("vSeg")),
        ("vDesc", ac.get("vDesc")),
        ("vII", z),
        ("vIPI", ac.get("vIPI")),
        ("vIPIDevol", z),
        ("vPIS", ac.get("vPIS")),
        ("vCOFINS", ac.get("vCOFINS")),
        ("vOutro", ac.get("vOutro")),
        ("vNF", vnf),
    ] {
        x.t(tag, &num(v, 2, tag)?);
    }
    if !ac.get("vTotTrib").eh_zero() {
        x.t("vTotTrib", &num(ac.get("vTotTrib"), 2, "vTotTrib")?);
    }
    x.fecha("ICMSTot");
    let mut out: BTreeMap<String, String> = ACS
        .iter()
        .map(|k| (k.to_string(), ac.get(k).fixo(2)))
        .collect();
    out.insert("vNF".into(), vnf.fixo(2));
    if difal {
        for k in ["vFCPUFDest", "vICMSUFDest", "vICMSUFRemet"] {
            out.insert(k.into(), ac.get(k).fixo(2));
        }
    }
    if com_st {
        for k in ["vBCST", "vST", "vFCPST"] {
            out.insert(k.into(), ac.get(k).fixo(2));
        }
    }
    for k in ["vICMSDeson", "vFCPSTRet"] {
        if ac.v.contains_key(k) {
            out.insert(k.into(), ac.get(k).fixo(2));
        }
    }
    for k in TOT_MONO {
        if !ac.get(k).arred(2).eh_zero() {
            out.insert(k.into(), ac.get(k).fixo(2));
        }
    }
    if let Some(liq) = ac.liq {
        out.insert("vProdLiqTot".into(), liq.fixo(2));
    }
    let itens: Vec<&ItemIbsCbs> = n.itens.iter().filter_map(|i| i.ibscbs.as_ref()).collect();
    if let Some(tt) = if itens.is_empty() {
        None
    } else {
        totalizar(&itens)
    } {
        x.0.push_str(&tt.xml());
        let data = na_uf(&n.emissao, uf).format("%Y-%m-%d").to_string();
        let v = vnftot(&vnf.fixo(2), Some(&tt), &data, "0")
            .map_err(|e| erro(e.to_string(), "vNFTot"))?;
        let v = Dec::centavos(v);
        x.t("vNFTot", &num(v, 2, "vNFTot")?);
        out.insert("vNFTot".into(), v.fixo(2));
    }
    x.fecha("total");
    Ok(out)
}

/// Grupo `pag`: de 1 a 100 `detPag`, com o cartao quando informado, e o `vTroco`.
pub(crate) fn pag(x: &mut X, n: &Nota) -> Res<()> {
    if n.pagamentos.is_empty() {
        return Err(erro(
            "informe ao menos um pagamento (tPag 90 = sem pagamento)",
            "pag",
        ));
    }
    if n.pagamentos.len() > 100 {
        return Err(erro("pag: maximo 100 detPag", "detPag"));
    }
    x.abre("pag");
    for p in &n.pagamentos {
        x.abre("detPag");
        if let Some(i) = p.ind_pag {
            x.t("indPag", &i.to_string());
        }
        let t = p.t_pag.trim();
        let tpag = format!("{t:0>2}");
        if tpag.len() != 2 || !tpag.bytes().all(|b| b.is_ascii_digit()) {
            return Err(erro(format!("tPag invalido: {:?}", p.t_pag), "tPag"));
        }
        x.t("tPag", &tpag);
        if tpag == "99" && p.x_pag.as_ref().is_none_or(|s| s.is_empty()) {
            return Err(erro(
                "tPag 99 exige xPag (descricao do meio de pagamento)",
                "xPag",
            ));
        }
        x.o(
            "xPag",
            &txt_ou_none(&p.x_pag, "xPag", 60, 2, n.remover_acento)?,
        );
        x.t("vPag", &num(Dec::parse(&p.valor, "vPag")?, 2, "vPag")?);
        if let Some(c) = &p.cartao {
            x.abre("card");
            x.t("tpIntegra", &c.tp_integra.to_string());
            if let Some(s) = c.cnpj.as_ref().filter(|s| !s.is_empty()) {
                x.t("CNPJ", &documento(s, "card/CNPJ")?.1);
            }
            if let Some(s) = c.t_band.as_ref().filter(|s| !s.is_empty()) {
                x.t("tBand", &digitos(s, "tBand", &[2])?);
            }
            x.o("cAut", &txt_ou_none(&c.c_aut, "cAut", 128, 1, false)?);
            x.fecha("card");
        }
        x.fecha("detPag");
    }
    x.o("vTroco", &num_ou_none(&n.troco, 2, "vTroco")?);
    x.fecha("pag");
    Ok(())
}

/// Grupo `infAdic`: so' sai com `infAdFisco` ou `infCpl`.
pub(crate) fn inf_adic(x: &mut X, n: &Nota) -> Res<()> {
    let fisco = txt_ou_none(&n.inf_ad_fisco, "infAdFisco", 2000, 1, n.remover_acento)?;
    let cpl = txt_ou_none(&n.inf_cpl, "infCpl", 5000, 1, n.remover_acento)?;
    if fisco.is_none() && cpl.is_none() {
        return Ok(());
    }
    x.abre("infAdic");
    x.o("infAdFisco", &fisco);
    x.o("infCpl", &cpl);
    x.fecha("infAdic");
    Ok(())
}

pub(crate) fn b64(b: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(b)
}

/// Grupo `infRespTec`. Com `idCSRT` e o codigo de seguranca, o `hashCSRT` e' o SHA-1 em base64 de codigo + chave de
/// acesso, como manda a NT 2018.005.
pub(crate) fn resp_tec(x: &mut X, n: &Nota, chave: &str) -> Res<()> {
    let Some(r) = &n.resp_tec else { return Ok(()) };
    x.abre("infRespTec");
    x.t("CNPJ", &digitos(&r.cnpj, "infRespTec/CNPJ", &[14])?);
    x.t(
        "xContato",
        &txt(&r.contato, "xContato", 60, 2, n.remover_acento)?,
    );
    x.t("email", &txt(&r.email, "infRespTec/email", 60, 6, false)?);
    x.t(
        "fone",
        &digitos(&r.fone, "infRespTec/fone", &(7..13).collect::<Vec<_>>())?,
    );
    if let (Some(id), Some(csrt)) = (
        r.id_csrt.filter(|v| *v != 0),
        r.csrt.as_ref().filter(|s| !s.is_empty()),
    ) {
        x.t("idCSRT", &format!("{id:02}"));
        x.t(
            "hashCSRT",
            &b64(&Sha1::digest(format!("{csrt}{chave}").as_bytes())),
        );
    }
    x.fecha("infRespTec");
    Ok(())
}
