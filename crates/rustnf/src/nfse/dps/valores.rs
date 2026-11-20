// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Montagem do grupo `valores` da DPS.

use super::*;

pub(super) fn valores(v: &Valores, leiaute: &str) -> R<No> {
    let c = "infDPS/valores";
    let mut n = No::new("valores");
    {
        let sp = n.grupo("vServPrest");
        if let Some(r) = &v.recebido {
            sp.add(
                "vReceb",
                dinheiro(r.as_str(), &format!("{c}/vServPrest/vReceb"), 15, true)?,
            );
        }
        sp.add(
            "vServ",
            dinheiro(
                v.servico.as_str(),
                &format!("{c}/vServPrest/vServ"),
                15,
                false,
            )?,
        );
    }
    if v.desconto_incondicionado.is_some() || v.desconto_condicionado.is_some() {
        let d = n.grupo("vDescCondIncond");
        if let Some(x) = &v.desconto_incondicionado {
            d.add(
                "vDescIncond",
                dinheiro(
                    x.as_str(),
                    &format!("{c}/vDescCondIncond/vDescIncond"),
                    15,
                    true,
                )?,
            );
        }
        if let Some(x) = &v.desconto_condicionado {
            d.add(
                "vDescCond",
                dinheiro(
                    x.as_str(),
                    &format!("{c}/vDescCondIncond/vDescCond"),
                    15,
                    true,
                )?,
            );
        }
    }
    if let Some(dd) = v.deducao.as_ref().filter(|_| leiaute == LEIAUTE_NT009) {
        n.anexar(ajuste_bc(dd, c)?);
    } else if let Some(dd) = &v.deducao {
        let cd = format!("{c}/vDedRed");
        let formas = dd.percentual.is_some() as u8
            + dd.valor.is_some() as u8
            + (!dd.documentos.is_empty()) as u8;
        if formas != 1 {
            return Err(ErroNfse::novo(
                cd,
                "informe UMA forma: percentual, valor ou documentos",
            ));
        }
        let x = n.grupo("vDedRed");
        if let Some(p) = &dd.percentual {
            x.add("pDR", dinheiro(p.as_str(), &format!("{cd}/pDR"), 3, true)?);
        } else if let Some(p) = &dd.valor {
            x.add("vDR", dinheiro(p.as_str(), &format!("{cd}/vDR"), 15, true)?);
        } else {
            if dd.documentos.len() > 1000 {
                return Err(ErroNfse::novo(
                    format!("{cd}/documentos"),
                    "no maximo 1000 documentos",
                ));
            }
            let lst = x.grupo("documentos");
            for (i, d) in dd.documentos.iter().enumerate() {
                lst.anexar(doc_deducao(d, &format!("{cd}/documentos/docDedRed[{i}]"))?);
            }
        }
    }
    let t = n.grupo("trib");
    let m = &v.municipal;
    let cm = format!("{c}/trib/tribMun");
    {
        let tm = t.grupo("tribMun");
        let trib = en(
            m.tributacao,
            &["1", "2", "3", "4"],
            &format!("{cm}/tribISSQN"),
        )?;
        tm.add("tribISSQN", trib.clone());
        if !m.pais_resultado.is_empty() {
            tm.add(
                "cPaisResult",
                confere(
                    "TSCodPaisISO",
                    &m.pais_resultado.to_uppercase(),
                    &format!("{cm}/cPaisResult"),
                    "",
                )?,
            );
        }
        if let Some(im) = m.imunidade {
            if trib != "2" {
                return Err(ErroNfse::novo(
                    format!("{cm}/tpImunidade"),
                    "so' com tributacao 2 (imunidade)",
                ));
            }
            tm.add(
                "tpImunidade",
                en(im, &refs(&faixa(0, 5)), &format!("{cm}/tpImunidade"))?,
            );
        }
        if let Some(st) = m.suspensao_tipo {
            let s = tm.grupo("exigSusp");
            s.add(
                "tpSusp",
                en(st, &["1", "2"], &format!("{cm}/exigSusp/tpSusp"))?,
            );
            s.add(
                "nProcesso",
                confere(
                    "TSNumProcesso",
                    &so_digitos(&m.suspensao_processo),
                    &format!("{cm}/exigSusp/nProcesso"),
                    "numero do processo com 30 digitos",
                )?,
            );
        }
        if !m.beneficio_numero.is_empty() {
            let b = tm.grupo("BM");
            b.add(
                "nBM",
                confere(
                    "TSNumBM",
                    &so_digitos(&m.beneficio_numero),
                    &format!("{cm}/BM/nBM"),
                    "",
                )?,
            );
            match (&m.beneficio_reducao_valor, &m.beneficio_reducao_percentual) {
                (Some(x), None) => b.add(
                    "vRedBCBM",
                    dinheiro(x.as_str(), &format!("{cm}/BM/vRedBCBM"), 15, true)?,
                ),
                (None, Some(x)) => b.add(
                    "pRedBCBM",
                    dinheiro(x.as_str(), &format!("{cm}/BM/pRedBCBM"), 3, true)?,
                ),
                _ => {
                    return Err(ErroNfse::novo(
                        format!("{cm}/BM"),
                        "informe a reducao da base em valor OU em percentual",
                    ))
                }
            }
        }
        tm.add(
            "tpRetISSQN",
            en(m.retencao, &["1", "2", "3"], &format!("{cm}/tpRetISSQN"))?,
        );
        if let Some(a) = &m.aliquota {
            tm.add("pAliq", aliquota_iss(a.as_str())?);
        }
    }
    if let Some(f) = &v.federal {
        let cf = format!("{c}/trib/tribFed");
        let mut tf = No::new("tribFed");
        if let Some(p) = &f.pis_cofins {
            let cp = format!("{cf}/piscofins");
            let x = tf.grupo("piscofins");
            let validos: Vec<String> = (0..10)
                .chain(49..57)
                .chain(60..68)
                .chain(70..76)
                .chain([98, 99])
                .map(|i| format!("{i:02}"))
                .collect();
            x.add(
                "CST",
                en(
                    format!("{:0>2}", p.cst.as_str()),
                    &refs(&validos),
                    &format!("{cp}/CST"),
                )?,
            );
            for (tag, val, int) in [
                ("vBCPisCofins", &p.base, 15),
                ("pAliqPis", &p.aliquota_pis, 2),
                ("pAliqCofins", &p.aliquota_cofins, 2),
                ("vPis", &p.valor_pis, 15),
                ("vCofins", &p.valor_cofins, 15),
            ] {
                if let Some(v) = val {
                    let txt = dinheiro(v.as_str(), &format!("{cp}/{tag}"), int, true)?;
                    // vPis/vCofins [0..1]: zero = omitir
                    if (tag == "vPis" || tag == "vCofins") && txt == "0.00" {
                        continue;
                    }
                    x.add(tag, txt);
                }
            }
            if let Some(tr) = p.tipo_retencao {
                x.add(
                    "tpRetPisCofins",
                    en(tr, &refs(&faixa(0, 9)), &format!("{cp}/tpRetPisCofins"))?,
                );
            }
        }
        for (tag, val) in [
            ("vRetCP", &f.retencao_cp),
            ("vRetIRRF", &f.retencao_irrf),
            ("vRetCSLL", &f.retencao_csll),
        ] {
            if let Some(v) = val {
                tf.add(tag, dinheiro(v.as_str(), &format!("{cf}/{tag}"), 15, true)?);
            }
        }
        if !tf.filhos.is_empty() {
            t.anexar(tf);
        }
    }
    let tt = &v.total;
    let ct = format!("{c}/trib/totTrib");
    let val = [&tt.valor_federal, &tt.valor_estadual, &tt.valor_municipal];
    let per = [
        &tt.percentual_federal,
        &tt.percentual_estadual,
        &tt.percentual_municipal,
    ];
    let tem_val = val.iter().any(|x| x.is_some());
    let tem_per = per.iter().any(|x| x.is_some());
    if tem_val as u8 + tem_per as u8 + tt.percentual_sn.is_some() as u8 > 1 {
        return Err(ErroNfse::novo(
            ct,
            "informe UMA forma: valores, percentuais ou percentual do Simples",
        ));
    }
    let tot = t.grupo("totTrib");
    if tem_val {
        let x = tot.grupo("vTotTrib");
        for (tag, q) in ["vTotTribFed", "vTotTribEst", "vTotTribMun"]
            .iter()
            .zip(val)
        {
            x.add(
                tag,
                dinheiro(
                    q.as_ref().map_or("0", |v| v.as_str()),
                    &format!("{ct}/vTotTrib/{tag}"),
                    15,
                    true,
                )?,
            );
        }
    } else if tem_per {
        let x = tot.grupo("pTotTrib");
        for (tag, q) in ["pTotTribFed", "pTotTribEst", "pTotTribMun"]
            .iter()
            .zip(per)
        {
            x.add(
                tag,
                dinheiro(
                    q.as_ref().map_or("0", |v| v.as_str()),
                    &format!("{ct}/pTotTrib/{tag}"),
                    3,
                    true,
                )?,
            );
        }
    } else if let Some(p) = &tt.percentual_sn {
        tot.add(
            "pTotTribSN",
            dinheiro(p.as_str(), &format!("{ct}/pTotTribSN"), 2, true)?,
        );
    } else {
        tot.add("indTotTrib", "0");
    }
    Ok(n)
}
