// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Cabecalho da nota (`ide`) e as regras que olham o conjunto da nota: finalidade, nota de credito e de debito
//! (B25), compra governamental (BB), recebimento antecipado (NB01) e os grupos de tributo que cada tipo de nota leva.

use super::formato::*;
use super::modelo::*;
use super::*;

/// Grupo `ide`, na ordem do XSD PL_010h. A chave ja' vem pronta: `cNF` e `cDV` saem dela.
pub(crate) fn ide(x: &mut X, n: &Nota, chave: &str, uf: &str) -> Res<()> {
    let ra = n.remover_acento;
    x.abre("ide");
    x.t("cUF", &format!("{:02}", uf_codigo(uf).unwrap_or(0)));
    x.t("cNF", &chave[35..43]);
    x.t("natOp", &txt(&n.nat_op, "natOp", 60, 1, ra)?);
    x.t("mod", &n.modelo.to_string());
    x.t("serie", &n.serie.to_string());
    x.t("nNF", &n.numero.to_string());
    x.t("dhEmi", &dh(&n.emissao, uf));
    if let Some(s) = &n.saida {
        x.t("dhSaiEnt", &dh(s, uf));
    }
    x.t("tpNF", &n.tp_nf.to_string());
    x.t("idDest", &n.id_dest.to_string());
    x.t("cMunFG", &digitos(&n.cmun_fg, "cMunFG", &[7])?);
    if let Some(c) = n.cmun_fg_ibs.as_ref().filter(|s| !s.is_empty()) {
        x.t("cMunFGIBS", &digitos(c, "cMunFGIBS", &[7])?);
    }
    x.t("tpImp", &tp_imp(n).to_string());
    x.t("tpEmis", &n.tp_emis.to_string());
    x.t("cDV", &chave[43..44]);
    x.t("tpAmb", &n.tp_amb.to_string());
    x.t("finNFe", &n.fin_nfe.to_string());
    let (deb, cred) = tipos(n)?;
    if let Some(d) = &deb {
        // PL_010h: logo depois do finNFe
        x.t("tpNFDebito", d);
    }
    if let Some(c) = &cred {
        x.t("tpNFCredito", c);
    }
    x.t("indFinal", &ind_final(n).to_string());
    x.t("indPres", &ind_pres(n).to_string());
    if let Some(i) = n.ind_intermed {
        x.t("indIntermed", &i.to_string());
    }
    if let Some(c) = &n.c_ind_op {
        // B25d: depois do indIntermed (XSD PL_010h)
        let c = digitos(c, "cIndOp", &[6])?;
        if let Some((cstat, msg)) =
            crate::reforma::indop::erro_nfe(&c, n.modelo, n.retirada.is_some())
        {
            return Err(erro_r(msg, "cIndOp", cstat));
        }
        x.t("cIndOp", &c);
    }
    x.t("procEmi", &n.proc_emi.to_string());
    x.t("verProc", &txt(&n.ver_proc, "verProc", 20, 1, true)?);
    let xj = n.x_just.as_ref().filter(|s| !s.is_empty());
    if n.tp_emis == 1 && (n.dh_cont.is_some() || xj.is_some()) {
        return Err(erro_r(
            "dhCont/xJust nao se informam na emissao normal (tpEmis 1)",
            "dhCont",
            "556",
        ));
    }
    if [2, 4, 5, 9].contains(&n.tp_emis) && (n.dh_cont.is_none() || xj.is_none()) {
        return Err(erro_r(
            format!(
                "contingencia (tpEmis {}): informe dhCont e xJust",
                n.tp_emis
            ),
            "dhCont",
            "557",
        ));
    }
    // SVC: o MOC 7.00, Anexo III, 2.1.3.5, manda gerar a NF-e com o motivo (xJust) e o inicio (dhCont) da contingencia, que
    // saem no DANFE. O Anexo I nao tem rejeicao para a falta deles nos tpEmis 6 e 7 (a 557 vale so' para 2, 4, 5 e 9):
    // por isso o erro nao leva codigo de rejeicao.
    if [6, 7].contains(&n.tp_emis) && (n.dh_cont.is_none() || xj.is_none()) {
        return Err(erro(
            format!(
                "contingencia SVC (tpEmis {}): informe dhCont e xJust, que saem no DANFE (MOC 7.00, Anexo III, 2.1.3.5)",
                n.tp_emis
            ),
            "dhCont",
        ));
    }
    if n.dh_cont.is_some() || xj.is_some() {
        let (Some(dc), Some(j)) = (&n.dh_cont, xj) else {
            return Err(erro(
                "contingencia: informe dhCont e xJust juntos",
                "dhCont",
            ));
        };
        x.t("dhCont", &dh(dc, uf));
        x.t("xJust", &txt(j, "xJust", 256, 15, ra)?);
    }
    if n.refs_nfe.len() > 999 {
        return Err(erro("NFref: maximo 999", "NFref"));
    }
    for c in &n.refs_nfe {
        if let Some(e) = erro_chave(c) {
            return Err(erro(format!("NFref/refNFe invalida: {e}"), "refNFe"));
        }
        x.abre("NFref");
        x.t("refNFe", &chave_dfe::limpar(c));
        x.fecha("NFref");
    }
    if let Some(cg) = compra_gov(n)? {
        // XSD PL_010h: depois do NFref (BB01)
        x.abre("gCompraGov");
        x.t("tpEnteGov", &cg.tp_ente.to_string());
        x.t(
            "pRedutor",
            &num(Dec::parse(&cg.p_redutor, "pRedutor")?, 4, "pRedutor")?,
        );
        x.t("tpOperGov", &cg.tp_oper.to_string());
        for c in &cg.refs {
            x.t("refDFeAnt", c);
        }
        x.fecha("gCompraGov");
    }
    if !n.pag_antecipado.is_empty() {
        // XSD PL_010h: ultimo filho do ide (BC01)
        if n.pag_antecipado.len() > 99 {
            return Err(erro("gPagAntecipado: maximo 99 refNFe", "gPagAntecipado"));
        }
        x.abre("gPagAntecipado");
        for c in &n.pag_antecipado {
            if let Some(e) = erro_chave(c) {
                return Err(erro(
                    format!("gPagAntecipado/refNFe invalida: {e}"),
                    "gPagAntecipado",
                ));
            }
            x.t("refNFe", &chave_dfe::limpar(c));
        }
        x.fecha("gPagAntecipado");
    }
    x.fecha("ide");
    Ok(())
}

/// Dominio do `tpNFDebito` (XSD `TTpNFDebito`) e do `tpNFCredito` (`TTpNFCredito`).
pub(crate) const TP_NF_DEBITO: [&str; 8] = ["01", "02", "03", "04", "05", "06", "07", "08"];
pub(crate) const TP_NF_CREDITO: [&str; 6] = ["01", "02", "03", "04", "05", "06"];

/// (`tpNFDebito`, `tpNFCredito`) com 2 digitos, conferidos contra o `finNFe` (B25.1-10 e -20, B25.2-10 e -20).
pub(crate) fn tipos(n: &Nota) -> Res<(Option<String>, Option<String>)> {
    let um = |v: &Option<String>, nome: &str, dom: &[&str]| -> Res<Option<String>> {
        let Some(s) = v.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) else {
            return Ok(None);
        };
        let t = format!("{s:0>2}");
        if !dom.contains(&t.as_str()) {
            return Err(erro(
                format!("{nome} invalido: '{s}' ({})", dom.join(", ")),
                nome,
            ));
        }
        Ok(Some(t))
    };
    let deb = um(&n.tp_nf_debito, "tpNFDebito", &TP_NF_DEBITO)?;
    let cred = um(&n.tp_nf_credito, "tpNFCredito", &TP_NF_CREDITO)?;
    if let Some(d) = &deb {
        if n.fin_nfe != 6 {
            return Err(erro_r(
                format!("tpNFDebito {d} so' na nota de debito (finNFe 6) -- B25.1-10"),
                "tpNFDebito",
                "1139",
            ));
        }
    }
    if n.fin_nfe == 6 && deb.is_none() {
        return Err(erro_r(
            "nota de debito (finNFe 6) sem tpNFDebito -- B25.1-20",
            "tpNFDebito",
            "1009",
        ));
    }
    if let Some(c) = &cred {
        if n.fin_nfe != 5 {
            return Err(erro_r(
                format!("tpNFCredito {c} so' na nota de credito (finNFe 5) -- B25.2-10"),
                "tpNFCredito",
                "1163",
            ));
        }
    }
    if n.fin_nfe == 5 && cred.is_none() {
        return Err(erro_r(
            "nota de credito (finNFe 5) sem tpNFCredito -- B25.2-20",
            "tpNFCredito",
            "1164",
        ));
    }
    Ok((deb, cred))
}

/// `gCompraGov` conferido: dominios do XSD (`TEnteGov` 1 a 6, `TOperCompraGov` 1 a 4, `refDFeAnt` de 0 a 99 chaves
/// de 44 digitos) e as regras BB05-10 a BB05-60 (NT 2025.002). O resto do BB05 (DV, UF, ano) e' do `conferir`.
pub(crate) fn compra_gov(n: &Nota) -> Res<Option<CompraGov>> {
    let Some(cg) = &n.compra_gov else {
        return Ok(None);
    };
    let (ente, oper) = (cg.tp_ente, cg.tp_oper);
    if !(1..=6).contains(&ente) {
        return Err(erro(
            format!("tpEnteGov invalido: {ente} (1 a 6)"),
            "tpEnteGov",
        ));
    }
    if !(1..=4).contains(&oper) {
        return Err(erro(
            format!("tpOperGov invalido: {oper} (1 a 4)"),
            "tpOperGov",
        ));
    }
    let red = Dec::parse(&cg.p_redutor, "pRedutor")?;
    if red.negativo() || !Dec::parse("100", "pRedutor")?.maior(red) {
        return Err(erro(
            format!(
                "pRedutor {} fora de 0 <= pRedutor < 100 (art. 472 LC 214/2025)",
                cg.p_redutor.trim()
            ),
            "pRedutor",
        ));
    }
    let refs: Vec<String> = cg.refs.iter().map(|c| chave_dfe::limpar(c)).collect();
    if refs.len() > 99 {
        return Err(erro("gCompraGov: maximo 99 refDFeAnt", "refDFeAnt"));
    }
    for (i, c) in refs.iter().enumerate() {
        if c.len() != 44 || !c.bytes().all(|b| b.is_ascii_digit()) {
            return Err(erro_r(
                format!(
                    "refDFeAnt {} com tamanho/formato invalido (44 digitos) -- BB05-60",
                    i + 1
                ),
                "refDFeAnt",
                "1203",
            ));
        }
    }
    if (oper == 1 || oper == 4) && !refs.is_empty() {
        let (rv, rej) = if oper == 1 {
            ("10", "1195")
        } else {
            ("50", "1199")
        };
        return Err(erro_r(
            format!("tpOperGov {oper} nao referencia DF-e anterior -- BB05-{rv}"),
            "refDFeAnt",
            rej,
        ));
    }
    if (oper == 2 || oper == 3) && refs.is_empty() {
        let (rv, rej) = if oper == 2 {
            ("20", "1196")
        } else {
            ("40", "1198")
        };
        return Err(erro_r(
            format!("tpOperGov {oper} exige o DF-e anterior (refDFeAnt) -- BB05-{rv}"),
            "refDFeAnt",
            rej,
        ));
    }
    if oper == 2 && refs.len() > 1 {
        return Err(erro_r(
            "tpOperGov 2 referencia UM DF-e anterior -- BB05-30",
            "refDFeAnt",
            "1197",
        ));
    }
    Ok(Some(CompraGov {
        tp_ente: ente,
        p_redutor: cg.p_redutor.clone(),
        tp_oper: oper,
        refs,
    }))
}

pub(crate) fn tp_oper(n: &Nota) -> Option<i64> {
    n.compra_gov.as_ref().map(|c| c.tp_oper)
}

/// BB02-10: com compra governamental, a aliquota dos OUTROS entes no `gIBSCBS` e' zero (arts. 372 e 473, par. 1o,
/// da LC 214/2025): de 2027 a 2032 so' o IBS; de 2033 em diante tambem a CBS. Consorcio publico (5) e CGIBS (6) ficam fora.
fn bb02_zeros(ente: i64, ano: i32) -> &'static [&'static str] {
    match (ano <= 2032, ente) {
        (_, 1) => &["ibs_uf", "ibs_mun"],
        (true, 2) | (true, 3) => &["ibs_mun"],
        (true, 4) => &["ibs_uf"],
        (false, 2) | (false, 3) => &["cbs", "ibs_mun"],
        (false, 4) => &["ibs_uf", "cbs"],
        _ => &[],
    }
}

/// UB82a-10 (1141), UB82a-30 (1144), o mesmo `pRedutor` na nota e no item (UB28-10, 1035) e BB02-10 (1008).
pub(crate) fn compra_gov_item(n: &Nota, it: &Item, k: &str, ano: i32) -> Res<()> {
    let Some(ib) = &it.ibscbs else { return Ok(()) };
    let tem_trib = ib.trib_compra_gov.is_some();
    let Some(cg) = &n.compra_gov else {
        if tem_trib || ib.p_redutor.is_some() {
            return Err(erro_r(format!("{k}item com compra governamental (gTribCompraGov/pRedutor) e a nota sem gCompraGov -- \
                                       UB82a-30"), "gTribCompraGov", "1144"));
        }
        return Ok(());
    };
    let com_grupo = ib.grupo == Some("gIBSCBS");
    if com_grupo && !tem_trib {
        return Err(erro_r(
            format!(
                "{k}compra governamental: o item com gIBSCBS leva o gTribCompraGov -- informe o \
                                   compra_gov do item (p_redutor e trib) -- UB82a-10"
            ),
            "gTribCompraGov",
            "1141",
        ));
    }
    let red_nota = Dec::parse(&cg.p_redutor, "pRedutor")?;
    let red_item = Dec {
        m: i128::from(ib.p_redutor.unwrap_or(0)),
        e: 4,
    };
    if com_grupo && !red_item.menos(red_nota).eh_zero() {
        return Err(erro_r(format!("{k}pRedutor do item ({}) difere do gCompraGov ({}): o pAliqEfet sai errado -- UB28-10",
                                  red_item.fixo(4), cg.p_redutor.trim()),
                          "pRedutor", "1035"));
    }
    if com_grupo && ano >= 2027 {
        for e in bb02_zeros(cg.tp_ente, ano) {
            let (p, tag) = match *e {
                "ibs_uf" => (ib.ibs_uf.p, "pIBSUF"),
                "ibs_mun" => (ib.ibs_mun.p, "pIBSMun"),
                _ => (ib.cbs.p, "pCBS"),
            };
            if p > 0 {
                return Err(erro_r(format!("{k}compra governamental (tpEnteGov {}) em {ano}: a aliquota {tag} do outro ente \
                                           tem de ser zero (art. 473 LC 214/2025) -- BB02-10", cg.tp_ente),
                                  "gCompraGov", "1008"));
            }
        }
    }
    Ok(())
}

/// Regras do `finNFe` 5 e 6 que nao dependem do documento referenciado (NT 2025.002 v1.53): B25-110, B25-120 e
/// B25.2-30.
pub(crate) fn finalidade(n: &Nota, ano: i32) -> Res<()> {
    let (_, cred) = tipos(n)?;
    if n.fin_nfe == 5 && n.tp_nf != 0 {
        // B25-110; o B25.2-40/1152 (credito 03 de saida) cai aqui antes
        return Err(erro_r(
            "nota de credito (finNFe 5) e' de entrada (tpNF 0) -- B25-110",
            "tpNF",
            "1161",
        ));
    }
    if n.fin_nfe == 6 && n.tp_nf != 1 {
        return Err(erro_r(
            "nota de debito (finNFe 6) e' de saida (tpNF 1) -- B25-120",
            "tpNF",
            "1162",
        ));
    }
    if cred.as_deref() == Some("02") && ano < 2029 {
        return Err(erro_r("tpNFCredito 02 (credito presumido de IBS na ZFM) so' a partir de janeiro/2029 -- B25.2-30",
                          "tpNFCredito", "1145"));
    }
    Ok(())
}

/// B25-80 (1001): na nota de credito ou de debito (e no recebimento do pagamento da compra governamental) os grupos
/// de tributo classico que o item PODE levar; `None` = sem restricao. Segue as excecoes 1 a 4 da regra (NT 2025.002 e
/// NT 2026.008).
pub(crate) fn classicos_permitidos(n: &Nota, ano: i32) -> Res<Option<Vec<&'static str>>> {
    if n.fin_nfe != 5 && n.fin_nfe != 6 && tp_oper(n) != Some(2) {
        return Ok(None);
    }
    let (deb, cred) = tipos(n)?;
    let (deb, cred) = (deb.as_deref(), cred.as_deref());
    if matches!(cred, Some("03") | Some("04") | Some("06")) || deb == Some("07") {
        return Ok(None); // excecao 1
    }
    if deb == Some("06") {
        let mut v = vec!["IPI", "ICMSPrevistoPagtoAntecip"]; // excecoes 3 e 4
        if ano == 2026 {
            v.extend(["PIS", "COFINS"]); // excecao 2
        }
        return Ok(Some(v));
    }
    Ok(Some(Vec::new()))
}

pub(crate) fn cclass_deb_cred(ct: &str) -> Option<(Option<&'static str>, Option<&'static str>)> {
    Some(match ct {
        "410030" => (Some("07"), None),
        "800001" => (Some("05"), Some("05")),
        "800002" => (Some("01"), None),
        "810001" => (None, Some("02")),
        "811001" => (Some("02"), None),
        "811002" => (Some("03"), None),
        "811003" => (Some("08"), None),
        _ => return None,
    })
}

pub(crate) fn deb_cclass(d: &str) -> Option<&'static str> {
    Some(match d {
        "01" => "800002",
        "02" => "811001",
        "03" => "811002",
        "05" => "800001",
        "07" => "410030",
        "08" => "811003",
        _ => return None,
    })
}

pub(crate) fn cred_cclass(c: &str) -> Option<&'static str> {
    Some(match c {
        "02" => "810001",
        "05" => "800001",
        _ => return None,
    })
}

/// UB14-70/1200, UB14-80/1201, UB14-60/1202. O UB13-30/UB13-40 (debito 07 sem gIBSCBS) cai por construcao: o UB14-70
/// forca o 410030, CST 410, sem grupo.
pub(crate) fn tipo_x_ibscbs(n: &Nota, it: &Item, k: &str) -> Res<()> {
    let Some(ib) = &it.ibscbs else { return Ok(()) };
    let (deb, cred) = tipos(n)?;
    let ct = ib.cclass_trib.as_str();
    if let Some(d) = &deb {
        if let Some(q) = deb_cclass(d).filter(|q| *q != ct) {
            return Err(erro_r(
                format!("{k}tpNFDebito {d} pede o cClassTrib {q} (veio {ct}) -- UB14-70"),
                "cClassTrib",
                "1200",
            ));
        }
    }
    if let Some(c) = &cred {
        if let Some(q) = cred_cclass(c).filter(|q| *q != ct) {
            return Err(erro_r(
                format!("{k}tpNFCredito {c} pede o cClassTrib {q} (veio {ct}) -- UB14-80"),
                "cClassTrib",
                "1201",
            ));
        }
    }
    if let Some((d, c)) = cclass_deb_cred(ct) {
        let ok = (deb.is_some() && deb.as_deref() == d) || (cred.is_some() && cred.as_deref() == c);
        if !ok {
            let quer: Vec<String> = [
                d.map(|v| format!("tpNFDebito {v}")),
                c.map(|v| format!("tpNFCredito {v}")),
            ]
            .into_iter()
            .flatten()
            .collect();
            return Err(erro_r(
                format!(
                    "{k}cClassTrib {ct} so' em nota com {} -- UB14-60",
                    quer.join(" ou ")
                ),
                "cClassTrib",
                "1202",
            ));
        }
    }
    Ok(())
}

/// NB01-30 (NT 2026.008): homologacao a partir de 01/02/2027 e producao a partir de 01/03/2027.
fn nb01_30(tp_amb: i64) -> chrono::NaiveDate {
    vigencia("nt2026_008.nb01_30", tp_amb)
}

/// Quais grupos de tributo classico o item leva: NB01-10/-20/-30, B25-80/1001, B25-90/1002, Q01-20/745, S01-20/748.
pub(crate) fn classicos(n: &Nota, it: &Item, k: &str, ano: i32) -> Res<()> {
    let (deb, _) = tipos(n)?;
    let oper = tp_oper(n);
    let prev = it.v_icms_previsto.is_some();
    let deb06 = deb.as_deref() == Some("06");
    let antecipado = deb06 || oper == Some(4);
    if prev && it.icms.is_some() {
        return Err(erro(
            format!("{k}ICMS e vICMSPrevisto juntos: o XSD aceita um so' (choice do grupo ICMS)"),
            "ICMS",
        ));
    }
    if prev && !antecipado {
        return Err(erro_r(
            format!("{k}ICMSPrevistoPagtoAntecip so' na nota de debito 06 (pagamento antecipado) ou no recebimento \
                     antecipado da compra governamental (tpOperGov 4) -- NB01-20"),
            "vICMSPrevisto",
            "1280",
        ));
    }
    if antecipado && !prev {
        let uf = uf_nota(n)?;
        if na_uf(&n.emissao, &uf).date_naive() >= nb01_30(n.tp_amb) {
            let quem = if deb06 {
                "nota de debito 06"
            } else {
                "tpOperGov 4"
            };
            return Err(erro_r(
                format!("{k}{quem} sem ICMSPrevistoPagtoAntecip (vICMSPrevisto; 0 se nao havera ICMS no fornecimento) -- \
                         NB01-30"),
                "vICMSPrevisto",
                "1281",
            ));
        }
    }
    if n.fin_nfe == 5 || n.fin_nfe == 6 || oper == Some(2) {
        if let Some(pode) = classicos_permitidos(n, ano)? {
            let onde = if n.fin_nfe == 5 || n.fin_nfe == 6 {
                format!(
                    "na nota de {} deste tipo",
                    if n.fin_nfe == 5 { "credito" } else { "debito" }
                )
            } else {
                "no recebimento do pagamento da compra governamental (tpOperGov 2)".to_string()
            };
            for (nome, tem) in [
                ("ICMS", it.icms.is_some()),
                ("ICMSPrevistoPagtoAntecip", prev),
                ("IPI", it.ipi.is_some()),
                ("PIS", it.pis.is_some()),
                ("COFINS", it.cofins.is_some()),
            ] {
                if tem && !pode.contains(&nome) {
                    return Err(erro_r(
                        format!("{k}{nome} nao vai {onde}: ela e' so' de IBS/CBS -- B25-80"),
                        nome,
                        "1001",
                    ));
                }
            }
        }
        return Ok(());
    }
    if it.icms.is_none() && !prev {
        return Err(erro_r(
            format!("{k}sem ICMS: so' a nota de credito/debito (finNFe 5/6) dispensa -- B25-90"),
            "ICMS",
            "1002",
        ));
    }
    for (nome, falta, rej, rv) in [
        ("PIS", it.pis.is_none(), "745", "Q01-20"),
        ("COFINS", it.cofins.is_none(), "748", "S01-20"),
    ] {
        if falta {
            return Err(erro_r(
                format!(
                    "{k}sem {nome}: so' a nota de credito/debito (finNFe 5/6) dispensa -- {rv}"
                ),
                nome,
                rej,
            ));
        }
    }
    Ok(())
}
