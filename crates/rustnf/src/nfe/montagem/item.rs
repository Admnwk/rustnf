// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O item (`det`): produto, veiculo novo, IPI, PIS e COFINS, e a ordem dos grupos do `imposto` (ICMS, IPI, PIS,
//! COFINS, ICMSUFDest e IBSCBS). Soma no acumulador os valores que vao para os totais.

use super::comb::{combustivel, repasse_do_cst60};
use super::formato::*;
use super::icms::icms;
use super::icms_regras::cod_benef;
use super::ide::{classicos, compra_gov_item, tipo_x_ibscbs};
use super::modelo::*;
use super::partes::{conferir_sn_item, Regime};
use super::produto::{medicamento, rastros, um_grupo_so};
use super::total::{Ac, DESON_DEDUZ};
use super::uf_dest::icms_uf_dest;
use super::*;

/// (`vUnComLiq`, `vProdLiq`) da NT 2026.008 (I11a a I11c), ou `None`. A sequencia do XSD e' opcional, mas os dois
/// campos vao juntos.
fn liquido(n: &Nota, it: &Item, q: Dec, k: &str) -> Res<Option<(Dec, Dec)>> {
    let Some(vul) = &it.valor_unitario_liquido else {
        if it.valor_produto_liquido.is_some() {
            return Err(erro(
                format!("{k}vProdLiq sem vUnComLiq: o XSD (PL_010h, sequencia I11a) exige os dois"),
                "vUnComLiq",
            ));
        }
        return Ok(None);
    };
    let vul = Dec::parse(vul, "vUnComLiq")?;
    let exato = q.vezes(vul);
    let calc = exato.arred(2);
    let vpl = match &it.valor_produto_liquido {
        Some(v) => Dec::parse(v, "vProdLiq")?,
        None => calc,
    };
    if vul.negativo() || vpl.negativo() {
        return Err(erro(format!("{k}vUnComLiq/vProdLiq negativo"), "vProdLiq"));
    }
    // I11c-20: so' com finNFe 1, tolerancia de 0,01
    if n.fin_nfe == 1 && vpl.menos(exato).abs().maior(Dec::centavos(1)) {
        return Err(erro_r(
            format!("{k}vProdLiq {} difere de vUnComLiq x qCom ({}) -- NT 2026.008 I11c-20, rejeicao 1279", vpl.fixo(2),
                    calc.fixo(2)),
            "vProdLiq",
            "1279",
        ));
    }
    Ok(Some((vul, vpl)))
}

/// `tpRest` do veiculo (XSD): restricao.
const TP_REST: [u32; 6] = [0, 1, 2, 3, 4, 9];

pub(crate) fn int_faixa(
    v: &str,
    campo: &str,
    lo: u32,
    hi: u32,
    permitidos: Option<&[u32]>,
) -> Res<String> {
    let s = v.trim();
    let n: Option<u32> = if !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) && s.len() <= 9 {
        s.parse().ok()
    } else {
        None
    };
    match n {
        Some(n) if (lo..=hi).contains(&n) && permitidos.is_none_or(|p| p.contains(&n)) => {
            Ok(n.to_string())
        }
        _ => {
            let faixa = match permitidos {
                Some(p) => p.iter().map(u32::to_string).collect::<Vec<_>>().join(", "),
                None => format!("{lo}..{hi}"),
            };
            Err(erro(format!("{campo}: '{s}' fora de {faixa}"), campo))
        }
    }
}

pub(crate) fn digitos_ate(v: &str, campo: &str, maximo: usize) -> Res<String> {
    let s = v.trim();
    if s.is_empty() || s.len() > maximo || !s.bytes().all(|b| b.is_ascii_digit()) {
        return Err(erro(
            format!("{campo}: '{s}' deve ter de 1 a {maximo} digitos"),
            campo,
        ));
    }
    Ok(s.to_string())
}

/// Grupo `veicProd` (J02 a J26) na ordem do XSD, conferido campo a campo.
fn veiculo(x: &mut X, v: &Veiculo, k: &str, ra: bool) -> Res<()> {
    x.abre("veicProd");
    x.t(
        "tpOp",
        &int_faixa(&v.tp_op, &format!("{k}tpOp"), 0, 3, None)?,
    );
    let ch = v.chassi.trim().to_uppercase();
    if ch.chars().count() != 17 || !ch.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(erro(
            format!("{k}chassi: '{ch}' deve ter 17 posicoes [A-Z0-9]"),
            &format!("{k}chassi"),
        ));
    }
    x.t("chassi", &ch);
    for (tag, val, mx) in [
        ("cCor", &v.c_cor, 4),
        ("xCor", &v.x_cor, 40),
        ("pot", &v.pot, 4),
        ("cilin", &v.cilin, 4),
        ("pesoL", &v.peso_l, 9),
        ("pesoB", &v.peso_b, 9),
        ("nSerie", &v.n_serie, 9),
        ("tpComb", &v.tp_comb, 2),
        ("nMotor", &v.n_motor, 21),
        ("CMT", &v.cmt, 9),
        ("dist", &v.dist, 4),
    ] {
        x.t(tag, &txt(val, &format!("{k}{tag}"), mx, 1, ra)?);
    }
    x.t(
        "anoMod",
        &int_faixa(&v.ano_mod, &format!("{k}anoMod"), 1000, 9999, None)?,
    );
    x.t(
        "anoFab",
        &int_faixa(&v.ano_fab, &format!("{k}anoFab"), 1000, 9999, None)?,
    );
    x.t("tpPint", &txt(&v.tp_pint, &format!("{k}tpPint"), 1, 1, ra)?);
    x.t(
        "tpVeic",
        &int_faixa(&v.tp_veic, &format!("{k}tpVeic"), 0, 99, None)?,
    );
    x.t(
        "espVeic",
        &int_faixa(&v.esp_veic, &format!("{k}espVeic"), 0, 9, None)?,
    );
    let vin = v.vin.trim().to_uppercase();
    if vin != "R" && vin != "N" {
        return Err(erro(
            format!("{k}VIN: '{vin}' deve ser R (remarcado) ou N (normal)"),
            &format!("{k}VIN"),
        ));
    }
    x.t("VIN", &vin);
    x.t(
        "condVeic",
        &int_faixa(&v.cond_veic, &format!("{k}condVeic"), 1, 3, None)?,
    );
    x.t("cMod", &digitos_ate(&v.c_mod, &format!("{k}cMod"), 6)?);
    x.t(
        "cCorDENATRAN",
        &digitos_ate(&v.c_cor_denatran, &format!("{k}cCorDENATRAN"), 2)?,
    );
    x.t(
        "lota",
        &int_faixa(&v.lota, &format!("{k}lota"), 0, 999, None)?,
    );
    x.t(
        "tpRest",
        &int_faixa(&v.tp_rest, &format!("{k}tpRest"), 0, 9, Some(&TP_REST))?,
    );
    x.fecha("veicProd");
    Ok(())
}

/// Grupo `IPI`: CST 00, 49, 50 e 99 tributam (`IPITrib`); 01 a 05 e 51 a 55 nao (`IPINT`).
pub(crate) fn ipi(x: &mut X, it: &Item, ac: &mut Ac, k: &str) -> Res<()> {
    let Some(p) = &it.ipi else { return Ok(()) };
    let cst = p.cst.trim();
    let trib = ["00", "49", "50", "99"].contains(&cst);
    let nt = ["01", "02", "03", "04", "05", "51", "52", "53", "54", "55"].contains(&cst);
    if !trib && !nt {
        return Err(erro(format!("{k}CST do IPI invalido: {cst:?}"), "IPI/CST"));
    }
    x.abre("IPI");
    x.t("cEnq", &digitos(&p.c_enq, "cEnq", &[3])?);
    if trib {
        x.abre("IPITrib");
        x.t("CST", cst);
        x.t(
            "vBC",
            &num_txt(exige(&p.vbc, &format!("{k}IPI/vBC"))?, 2, "IPI/vBC")?,
        );
        x.t(
            "pIPI",
            &num_txt(exige(&p.aliquota, &format!("{k}pIPI"))?, 4, "pIPI")?,
        );
        x.t(
            "vIPI",
            &num_txt(exige(&p.valor, &format!("{k}vIPI"))?, 2, "vIPI")?,
        );
        ac.add("vIPI", dec(&p.valor, "vIPI")?);
        x.fecha("IPITrib");
    } else {
        x.abre("IPINT");
        x.t("CST", cst);
        x.fecha("IPINT");
    }
    x.fecha("IPI");
    Ok(())
}

/// Grupo `PIS` ou `COFINS` (`nome`): CST 01 e 02 pela aliquota, 04 a 09 nao tributado, 49 a 99 outras operacoes.
pub(crate) fn pis_cofins(
    x: &mut X,
    pc: &PisCofins,
    nome: &'static str,
    ac: &mut Ac,
    k: &str,
) -> Res<()> {
    let cst = pc.cst.trim();
    const OUTR: [&str; 24] = [
        "49", "50", "51", "52", "53", "54", "55", "56", "60", "61", "62", "63", "64", "65", "66",
        "67", "70", "71", "72", "73", "74", "75", "98", "99",
    ];
    let (sub, obrig) = if cst == "01" || cst == "02" {
        (format!("{nome}Aliq"), true)
    } else if ["04", "05", "06", "07", "08", "09"].contains(&cst) {
        x.abre(nome);
        x.abre(&format!("{nome}NT"));
        x.t("CST", cst);
        x.fecha(&format!("{nome}NT"));
        x.fecha(nome);
        return Ok(());
    } else if OUTR.contains(&cst) {
        (format!("{nome}Outr"), false)
    } else {
        return Err(erro(format!("{k}CST do {nome} {cst:?} fora do escopo desta montagem (01, 02, 04 a 09 e 49 a 99)"),
                        &format!("{nome}/CST")));
    };
    let z = Some("0".to_string());
    let pega = |v: &Option<String>| -> Option<String> {
        if v.is_some() || obrig {
            v.clone()
        } else {
            z.clone()
        }
    };
    let (vbc, ali, val) = (pega(&pc.vbc), pega(&pc.aliquota), pega(&pc.valor));
    x.abre(nome);
    x.abre(&sub);
    x.t("CST", cst);
    x.t(
        "vBC",
        &num_txt(
            exige(&vbc, &format!("{k}{nome}/vBC"))?,
            2,
            &format!("{nome}/vBC"),
        )?,
    );
    x.t(
        &format!("p{nome}"),
        &num_txt(exige(&ali, &format!("{k}p{nome}"))?, 4, &format!("p{nome}"))?,
    );
    x.t(
        &format!("v{nome}"),
        &num_txt(exige(&val, &format!("{k}v{nome}"))?, 2, &format!("v{nome}"))?,
    );
    x.fecha(&sub);
    x.fecha(nome);
    ac.add(
        if nome == "PIS" { "vPIS" } else { "vCOFINS" },
        dec(&val, "valor")?,
    );
    Ok(())
}

/// Item `i` (a partir de 1) da nota: confere o item contra o regime do emitente e as regras da nota, escreve o
/// `det` e acumula os totais.
#[allow(clippy::too_many_arguments)]
pub(crate) fn det(
    x: &mut X,
    n: &Nota,
    i: usize,
    it: &Item,
    ac: &mut Ac,
    tem_ibscbs: bool,
    ano: i32,
    regime: Regime,
) -> Res<()> {
    let ra = n.remover_acento;
    let k = format!("item {i}: ");
    conferir_sn_item(n, it, &k, regime)?;
    tipo_x_ibscbs(n, it, &k)?;
    compra_gov_item(n, it, &k, ano)?;
    x.0.push_str(&format!("<det nItem=\"{i}\">"));
    x.abre("prod");
    x.t("cProd", &txt(&it.codigo, &format!("{k}cProd"), 60, 1, ra)?);
    let gtin = match it.gtin.as_ref().filter(|s| !s.is_empty()) {
        None => SEM_GTIN.to_string(),
        Some(g) => digitos(g, &format!("{k}cEAN"), &[8, 12, 13, 14])?,
    };
    x.t("cEAN", &gtin);
    // homologacao: o primeiro item do DANFE simplificado leva o texto fixo do MOC
    let xprod = if i == 1 && n.tp_amb == 2 && tp_imp(n) == 6 {
        HOM_XPROD
    } else {
        &it.descricao
    };
    x.t("xProd", &txt(xprod, &format!("{k}xProd"), 120, 1, ra)?);
    x.t("NCM", &digitos(&it.ncm, &format!("{k}NCM"), &[2, 8])?);
    if let Some(c) = it.cest.as_ref().filter(|s| !s.is_empty()) {
        x.t("CEST", &digitos(c, &format!("{k}CEST"), &[7])?);
    }
    if let Some(c) = it.cbenef.as_ref().filter(|s| !s.is_empty()) {
        x.t("cBenef", &cod_benef(c, &format!("{k}cBenef"), true)?);
    }
    if let Some(c) = it.extipi.as_ref().filter(|s| !s.is_empty()) {
        x.t("EXTIPI", &digitos(c, &format!("{k}EXTIPI"), &[2, 3])?);
    }
    x.t("CFOP", &digitos(&it.cfop, &format!("{k}CFOP"), &[4])?);
    x.t("uCom", &txt(&it.unidade, &format!("{k}uCom"), 6, 1, ra)?);
    let q = Dec::parse(&it.quantidade, "qCom")?;
    let vu = Dec::parse(&it.valor_unitario, "vUnCom")?;
    if q.cmp_zero() != std::cmp::Ordering::Greater {
        return Err(erro(format!("{k}qCom deve ser > 0"), "qCom"));
    }
    x.t("qCom", &num(q, 4, "qCom")?);
    x.t("vUnCom", &num(vu, 10, "vUnCom")?);
    let vprod = match &it.valor_produto {
        Some(v) => Dec::parse(v, "vProd")?,
        None => q.vezes(vu).arred(2),
    };
    x.t("vProd", &num(vprod, 2, "vProd")?);
    if let Some((vul, vpl)) = liquido(n, it, q, &k)? {
        // XSD PL_010h: depois do vProd
        x.t("vUnComLiq", &num(vul, 10, "vUnComLiq")?);
        x.t("vProdLiq", &num(vpl, 2, "vProdLiq")?);
        ac.liq = Some(ac.liq.unwrap_or(Dec::ZERO).mais(vpl));
    }
    let gt = it
        .gtin_trib
        .as_ref()
        .filter(|s| !s.is_empty())
        .or(it.gtin.as_ref().filter(|s| !s.is_empty()));
    x.t(
        "cEANTrib",
        &match gt {
            None => SEM_GTIN.to_string(),
            Some(g) => digitos(g, &format!("{k}cEANTrib"), &[8, 12, 13, 14])?,
        },
    );
    x.t(
        "uTrib",
        &txt(
            it.unidade_trib
                .as_deref()
                .filter(|s| !s.is_empty())
                .unwrap_or(&it.unidade),
            &format!("{k}uTrib"),
            6,
            1,
            ra,
        )?,
    );
    x.t(
        "qTrib",
        &num(
            match &it.quantidade_trib {
                Some(v) => Dec::parse(v, "qTrib")?,
                None => q,
            },
            4,
            "qTrib",
        )?,
    );
    x.t(
        "vUnTrib",
        &num(
            match &it.valor_unitario_trib {
                Some(v) => Dec::parse(v, "vUnTrib")?,
                None => vu,
            },
            10,
            "vUnTrib",
        )?,
    );
    let mut parc: BTreeMap<&str, Dec> = BTreeMap::new();
    for (tag, v) in [
        ("vFrete", &it.frete),
        ("vSeg", &it.seguro),
        ("vDesc", &it.desconto),
        ("vOutro", &it.outros),
    ] {
        parc.insert(tag, dec(v, tag)?);
        x.o(tag, &num_ou_none(v, 2, tag)?);
    }
    if it.ind_tot != 0 && it.ind_tot != 1 {
        return Err(erro(format!("{k}indTot 0 ou 1"), "indTot"));
    }
    x.t("indTot", &it.ind_tot.to_string());
    if let Some(c) = &it.c_ativ_sn {
        // PL_010h: depois do indTot, antes de xPed
        x.t("cAtivSN", c.trim());
    }
    x.o(
        "xPed",
        &txt_ou_none(&it.x_ped, &format!("{k}xPed"), 15, 1, true)?,
    );
    x.o(
        "nItemPed",
        &txt_ou_none(&it.n_item_ped, &format!("{k}nItemPed"), 6, 1, true)?,
    );
    um_grupo_so(it, &k)?;
    rastros(x, n, it, &k, ra)?;
    // ultimo filho do prod: o choice veicProd, med, arma, comb ou nRECOPI
    if let Some(m) = &it.med {
        medicamento(x, n, it, m, &k, ra)?;
    }
    if let Some(c) = &it.combustivel {
        combustivel(x, c, it, n, &k, ra)?;
    }
    if let Some(v) = &it.veiculo {
        if v.tp_op.trim() == "2"
            && n.entrega
                .as_ref()
                .is_none_or(|e| e.endereco.uf.trim().is_empty())
        {
            return Err(erro_r(
                format!(
                    "{k}faturamento direto de veiculo novo (tpOp 2) sem o local de entrega \
                                       (entrega/UF)"
                ),
                "entrega",
                "478",
            ));
        }
        veiculo(x, v, &k, ra)?;
    }
    x.fecha("prod");

    x.abre("imposto");
    x.o("vTotTrib", &num_ou_none(&it.v_tot_trib, 2, "vTotTrib")?);
    let v_ipi_antes = ac.get("vIPI");
    let st_antes = ac.get("vST").mais(ac.get("vFCPST"));
    let deduz_antes = ac.get(DESON_DEDUZ);
    classicos(n, it, &k, ano)?;
    repasse_do_cst60(it, &k)?;
    if let Some(v) = &it.v_icms_previsto {
        // NB01: alternativa do choice do ICMS
        let v = Dec::parse(v, "vICMSPrevisto")?;
        if v.negativo() {
            return Err(erro(format!("{k}vICMSPrevisto negativo"), "vICMSPrevisto"));
        }
        x.abre("ICMS");
        x.abre("ICMSPrevistoPagtoAntecip");
        x.t("vICMSPrevisto", &num(v, 2, "vICMSPrevisto")?);
        x.fecha("ICMSPrevistoPagtoAntecip");
        x.fecha("ICMS");
    } else if let Some(c) = &it.icms {
        let isuf = n
            .dest
            .as_ref()
            .and_then(|d| d.isuf.as_deref())
            .is_some_and(|s| !s.trim().is_empty());
        // 508 (N12-70): NF-e de saida a nao contribuinte; a excecao 3 libera a nota com veiculo novo em algum item
        let nao_contrib = n.dest.as_ref().is_some_and(|d| d.ind_ie_dest == 9)
            && n.tp_nf == 1
            && !n.itens.iter().any(|x| x.veiculo.is_some());
        icms(
            x,
            c,
            n.emit.crt,
            ac,
            &k,
            it,
            &uf_nota(n)?,
            isuf,
            n.fin_nfe,
            nao_contrib,
        )?;
    }
    ipi(x, it, ac, &k)?;
    if let Some(p) = &it.pis {
        pis_cofins(x, p, "PIS", ac, &k)?;
    }
    if let Some(c) = &it.cofins {
        pis_cofins(x, c, "COFINS", ac, &k)?;
    }
    // XSD: depois do COFINSST, antes do IS
    icms_uf_dest(x, n, it, ac, &k)?;
    if let Some(ib) = &it.ibscbs {
        x.0.push_str(&ib.xml());
    }
    x.fecha("imposto");
    x.o(
        "infAdProd",
        &txt_ou_none(&it.inf_ad_prod, &format!("{k}infAdProd"), 500, 1, ra)?,
    );

    let v_ipi = ac.get("vIPI").menos(v_ipi_antes);
    // vItem (VB01-10, NT 2025.002): vProd - vDesc - vICMSDeson (indDeduzDeson 1) + vFrete + vSeg + vOutro + vIPI +
    // vICMSST + vFCPST; no faturamento direto de veiculo novo sem o ICMS-ST (VB01-20); de 2027 em diante soma o IBS e a CBS.
    let mut v_st = ac.get("vST").mais(ac.get("vFCPST")).menos(st_antes);
    if it.veiculo.as_ref().is_some_and(|v| v.tp_op.trim() == "2") {
        v_st = Dec::ZERO;
    }
    let v_deduz = ac.get(DESON_DEDUZ).menos(deduz_antes);
    let mut v_item = vprod
        .menos(parc["vDesc"])
        .menos(v_deduz)
        .mais(parc["vFrete"])
        .mais(parc["vSeg"])
        .mais(parc["vOutro"])
        .mais(v_ipi)
        .mais(v_st);
    if let Some(ib) = &it.ibscbs {
        if tributos_somam_no_total(ano) {
            v_item = v_item.mais(Dec::centavos(
                ib.v_ibs + ib.v_cbs + ib.mono.vtot_ibs + ib.mono.vtot_cbs,
            ));
        }
    }
    if tem_ibscbs {
        x.t("vItem", &num(v_item, 2, "vItem")?);
    }
    if let Some((ch, nitem)) = &it.dfe_ref {
        if let Some(e) = erro_chave(ch) {
            return Err(erro(
                format!("{k}DFeReferenciado/chaveAcesso invalida: {e}"),
                "DFeReferenciado",
            ));
        }
        x.abre("DFeReferenciado");
        x.t("chaveAcesso", &chave_dfe::limpar(ch));
        if let Some(ni) = nitem {
            x.t("nItem", &ni.to_string());
        }
        x.fecha("DFeReferenciado");
    }
    x.fecha("det");

    if it.ind_tot == 1 {
        ac.add("vProd", vprod);
    }
    for tag in ["vFrete", "vSeg", "vDesc", "vOutro"] {
        ac.add(tag, parc[tag]);
    }
    ac.add("vTotTrib", dec(&it.v_tot_trib, "vTotTrib")?);
    Ok(())
}
