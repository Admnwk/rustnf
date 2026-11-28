// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Montagem do grupo `IBSCBS` da DPS e do imovel do leiaute da producao restrita.

use super::*;

/// imovel do leiaute nt009 (TCRTCInfoImovel do XSD prodrest 20/10/2026): cMun, gLocacao, gUnidImob.
pub(super) fn imovel(im: &Imovel, c: &str) -> R<No> {
    let mut n = No::new("imovel");
    n.add(
        "cMun",
        confere(
            "TSCodMunIBGE",
            &so_digitos(&im.cmun),
            &format!("{c}/cMun"),
            "",
        )?,
    );
    if let Some(lo) = &im.locacao {
        let cl = format!("{c}/gLocacao");
        let x = n.grupo("gLocacao");
        x.add(
            "pCopropriedade",
            dinheiro(
                lo.copropriedade.as_str(),
                &format!("{cl}/pCopropriedade"),
                3,
                true,
            )?,
        );
        x.add(
            "vTotOper",
            dinheiro(lo.valor_total.as_str(), &format!("{cl}/vTotOper"), 15, true)?,
        );
        if let Some(d) = &lo.desconto_incondicionado {
            x.add(
                "vDescIncondTot",
                dinheiro(d.as_str(), &format!("{cl}/vDescIncondTot"), 15, true)?,
            );
        }
        if let Some(d) = &lo.desconto_condicionado {
            x.add(
                "vDescCondTot",
                dinheiro(d.as_str(), &format!("{cl}/vDescCondTot"), 15, true)?,
            );
        }
        if !lo.vencimento.trim().is_empty() {
            x.add(
                "dVencOrig",
                data(&lo.vencimento, &format!("{cl}/dVencOrig"))?,
            );
        }
    }
    if im.unidades.len() > 99 {
        return Err(ErroNfse::novo(
            format!("{c}/gUnidImob"),
            "no maximo 99 unidades imobiliarias",
        ));
    }
    for (i, u) in im.unidades.iter().enumerate() {
        let cu = format!("{c}/gUnidImob[{i}]");
        let x = n.grupo("gUnidImob");
        x.opc(
            "inscImobFisc",
            &texto(&u.inscricao, &format!("{cu}/inscImobFisc"), Txt::opc(30))?,
        );
        if !u.cib.is_empty() {
            x.add(
                "cCIB",
                texto(
                    &u.cib,
                    &format!("{cu}/cCIB"),
                    Txt {
                        minimo: 8,
                        ..Txt::max(8)
                    },
                )?,
            );
        } else if let Some(e) = &u.endereco {
            let ce = format!("{cu}/end");
            let y = x.grupo("end");
            y.add(
                "CEP",
                confere(
                    "TSCEP",
                    &so_digitos(&e.cep),
                    &format!("{ce}/CEP"),
                    "CEP nacional (o imovel nao tem endereco no exterior neste leiaute)",
                )?,
            );
            y.add(
                "xLgr",
                texto(&e.logradouro, &format!("{ce}/xLgr"), Txt::max(255))?,
            );
            let nro = if e.numero.is_empty() {
                "S/N"
            } else {
                e.numero.as_str()
            };
            y.add("nro", texto(nro, &format!("{ce}/nro"), Txt::max(60))?);
            y.opc(
                "xCpl",
                &texto(&e.complemento, &format!("{ce}/xCpl"), Txt::opc(156))?,
            );
            y.opc(
                "xBairro",
                &texto(&e.bairro, &format!("{ce}/xBairro"), Txt::opc(60))?,
            );
        } else {
            return Err(ErroNfse::novo(
                cu,
                "identifique a unidade: cCIB ou endereco",
            ));
        }
        if u.ajustes.len() > 1000 {
            return Err(ErroNfse::novo(
                format!("{cu}/gAjusteBCLocImoveis"),
                "no maximo 1000 ajustes",
            ));
        }
        for (j, a) in u.ajustes.iter().enumerate() {
            let ca = format!("{cu}/gAjusteBCLocImoveis[{j}]");
            let y = x.grupo("gAjusteBCLocImoveis");
            y.add(
                "tpAjusteBCLocImoveis",
                en(
                    format!("{:0>2}", a.tipo.as_str().trim()),
                    TIPO_AJUSTE_LOC_IMOVEL,
                    &format!("{ca}/tpAjusteBCLocImoveis"),
                )?,
            );
            y.opc(
                "xTpAjusteBCLocImoveis",
                &texto(
                    &a.descricao,
                    &format!("{ca}/xTpAjusteBCLocImoveis"),
                    Txt::opc(150),
                )?,
            );
            y.add(
                "vAjusteBCLocImoveis",
                dinheiro(
                    a.valor.as_str(),
                    &format!("{ca}/vAjusteBCLocImoveis"),
                    15,
                    true,
                )?,
            );
        }
    }
    Ok(n)
}

/// O grupo `dest` sob `pai` (infDPS/IBSCBS no leiaute de producao, infDPS no nt009). indDest x dest: E0908/E0910 em
/// `regras_ibscbs`. DV: AnexoVI NT009 v1.04.01 "RN DPS_NFS-e" linhas 373/375 (E0911, E0913).
fn destinatario(p: Option<&Pessoa>, pai: &str) -> R<Option<No>> {
    let Some(p) = p else { return Ok(None) };
    let cd = format!("{pai}/dest");
    let mut d = No::new("dest");
    ident_dv(&mut d, p, &cd, Some(("E0911", "E0913")))?;
    d.add(
        "xNome",
        texto(&p.nome, &format!("{cd}/xNome"), Txt::max(150))?,
    );
    contato(&mut d, p, &cd)?;
    Ok(Some(d))
}

/// Leiaute nt009 (XSD prodrest 24/11/2026, TCInfDPS): indDest (obrigatorio, 0 sem o grupo IBSCBS) e dest no infDPS,
/// depois do toma e antes do interm.
pub(super) fn destinatario_infdps(inf: &mut No, g: Option<&IbsCbs>) -> R<()> {
    inf.add(
        "indDest",
        en(g.map_or(0, |g| g.ind_dest), SIM_NAO, "infDPS/indDest")?,
    );
    if let Some(d) = destinatario(g.and_then(|g| g.destinatario.as_ref()), "infDPS")? {
        inf.anexar(d);
    }
    Ok(())
}

pub(super) fn ibscbs(g: &IbsCbs, leiaute: &str) -> R<No> {
    let c = "infDPS/IBSCBS";
    let nt009 = leiaute == LEIAUTE_NT009;
    let mut n = No::new("IBSCBS");
    if !nt009 {
        // no nt009 o finNFSe sobe para o infDPS
        n.add("finNFSe", en(g.fin, &["0"], &format!("{c}/finNFSe"))?);
    }
    if let Some(x) = g.ind_final {
        n.add("indFinal", en(x, SIM_NAO, &format!("{c}/indFinal"))?);
    }
    n.add(
        "cIndOp",
        confere(
            "TSCodIndOp",
            &so_digitos(&g.cind_op),
            &format!("{c}/cIndOp"),
            "codigo de 6 digitos do Anexo VII",
        )?,
    );
    if let Some(x) = g.tp_oper {
        n.add(
            "tpOper",
            en(x, &refs(&faixa(1, 5)), &format!("{c}/tpOper"))?,
        );
    }
    if !g.refs_nfse.is_empty() {
        if g.refs_nfse.len() > 99 {
            return Err(ErroNfse::novo(
                format!("{c}/gRefNFSe"),
                "no maximo 99 notas referenciadas",
            ));
        }
        let r = n.grupo("gRefNFSe");
        for ch in &g.refs_nfse {
            r.add(
                "refNFSe",
                confere(
                    "TSChaveNFSe",
                    &so_alnum(ch),
                    &format!("{c}/gRefNFSe/refNFSe"),
                    "",
                )?,
            );
        }
    }
    if let Some(x) = g.tp_ente_gov {
        n.add(
            "tpEnteGov",
            en(x, &refs(&faixa(1, 4)), &format!("{c}/tpEnteGov"))?,
        );
    }
    if !nt009 {
        // no nt009 (Release 2) indDest e dest sobem para o infDPS: `destinatario_infdps`
        n.add("indDest", en(g.ind_dest, SIM_NAO, &format!("{c}/indDest"))?);
        if let Some(d) = destinatario(g.destinatario.as_ref(), c)? {
            n.anexar(d);
        }
    }
    if nt009 {
        if let Some(im) = &g.imovel {
            n.anexar(imovel(im, &format!("{c}/imovel"))?);
        }
        if g.bens_moveis.len() > 1000 {
            return Err(ErroNfse::novo(
                format!("{c}/bensMoveis"),
                "no maximo 1000 bens moveis",
            ));
        }
        for (i, b) in g.bens_moveis.iter().enumerate() {
            let cb = format!("{c}/bensMoveis[{i}]");
            let x = n.grupo("bensMoveis");
            x.add(
                "cNCMBemMovel",
                confere(
                    "TSNCM",
                    &so_digitos(&b.ncm),
                    &format!("{cb}/cNCMBemMovel"),
                    "NCM de 8 digitos",
                )?,
            );
            x.add(
                "xNCMBemMovel",
                texto(&b.descricao, &format!("{cb}/xNCMBemMovel"), Txt::max(150))?,
            );
            x.add(
                "qtdNCMBemMovel",
                confere(
                    "TSNum3Dig",
                    b.quantidade.as_str().trim(),
                    &format!("{cb}/qtdNCMBemMovel"),
                    "quantidade de 1 a 3 digitos",
                )?,
            );
        }
    } else if !g.imovel_cib.is_empty() || g.imovel_endereco.is_some() {
        let ci = format!("{c}/imovel");
        let im = n.grupo("imovel");
        im.opc(
            "inscImobFisc",
            &texto(
                &g.imovel_inscricao,
                &format!("{ci}/inscImobFisc"),
                Txt::opc(30),
            )?,
        );
        if !g.imovel_cib.is_empty() {
            im.add(
                "cCIB",
                texto(
                    &g.imovel_cib,
                    &format!("{ci}/cCIB"),
                    Txt {
                        minimo: 8,
                        ..Txt::max(8)
                    },
                )?,
            );
        } else if let Some(e) = &g.imovel_endereco {
            im.anexar(endereco_simples(e, &format!("{ci}/end"))?);
        }
    }
    let v = n.grupo("valores");
    if !g.reembolsos.is_empty() {
        if g.reembolsos.len() > 1000 {
            return Err(ErroNfse::novo(
                format!("{c}/valores/gReeRepRes"),
                "no maximo 1000 documentos",
            ));
        }
        let gr = v.grupo("gReeRepRes");
        for (i, d) in g.reembolsos.iter().enumerate() {
            let cr = format!("{c}/valores/gReeRepRes/documentos[{i}]");
            let x = gr.grupo("documentos");
            doc_ref(x, &d.refdoc(), &cr)?;
            if let Some(f) = &d.fornecedor {
                let fo = x.grupo("fornec");
                ident(fo, f, &format!("{cr}/fornec"))?;
                fo.add(
                    "xNome",
                    texto(&f.nome, &format!("{cr}/fornec/xNome"), Txt::max(150))?,
                );
            }
            x.add("dtEmiDoc", data(&d.emissao, &format!("{cr}/dtEmiDoc"))?);
            x.add(
                "dtCompDoc",
                data(&d.competencia, &format!("{cr}/dtCompDoc"))?,
            );
            x.add(
                "tpReeRepRes",
                en(
                    format!("{:0>2}", d.tipo.as_str()),
                    &["01", "02", "03", "04", "99"],
                    &format!("{cr}/tpReeRepRes"),
                )?,
            );
            x.opc(
                "xTpReeRepRes",
                &texto(
                    &d.tipo_descricao,
                    &format!("{cr}/xTpReeRepRes"),
                    Txt::opc(150),
                )?,
            );
            x.add(
                "vlrReeRepRes",
                dinheiro(d.valor.as_str(), &format!("{cr}/vlrReeRepRes"), 15, true)?,
            );
        }
    }
    // producao: trib/gIBSCBS/(CST, cClassTrib, ...); nt009: trib/(CST, cClassTrib, gIBSCBS?) -- NT 009 v1.01 item 2.16
    let t = v.grupo("trib");
    let ct = if nt009 {
        format!("{c}/valores/trib")
    } else {
        format!("{c}/valores/trib/gIBSCBS")
    };
    let cg = format!("{c}/valores/trib/gIBSCBS");
    let cst = confere(
        "TSCST",
        &so_digitos(&g.cst),
        &format!("{ct}/CST"),
        "CST de 3 digitos (do contador)",
    )?;
    let mut gi = No::new("gIBSCBS");
    {
        let alvo: &mut No = if nt009 { &mut *t } else { &mut gi };
        alvo.add("CST", cst);
        alvo.add(
            "cClassTrib",
            confere(
                "TSCClassTrib",
                &so_digitos(&g.cclass_trib),
                &format!("{ct}/cClassTrib"),
                "cClassTrib de 6 digitos (do contador)",
            )?,
        );
    }
    par_da_tabela(
        &so_digitos(&g.cst),
        &so_digitos(&g.cclass_trib),
        &format!("{ct}/cClassTrib"),
        Some("nfse"),
    )?;
    if !g.ccred_pres.is_empty() {
        gi.add(
            "cCredPres",
            confere(
                "TSCCredPres",
                &so_digitos(&g.ccred_pres),
                &format!("{cg}/cCredPres"),
                "",
            )?,
        );
    }
    regras_trib_regular(g, &format!("{cg}/gTribRegular"))?;
    if !g.cst_regular.is_empty() || !g.cclass_trib_regular.is_empty() {
        let r = gi.grupo("gTribRegular");
        r.add(
            "CSTReg",
            confere(
                "TSCST",
                &so_digitos(&g.cst_regular),
                &format!("{cg}/gTribRegular/CSTReg"),
                "",
            )?,
        );
        r.add(
            "cClassTribReg",
            confere(
                "TSCClassTrib",
                &so_digitos(&g.cclass_trib_regular),
                &format!("{cg}/gTribRegular/cClassTribReg"),
                "",
            )?,
        );
        par_da_tabela(
            &so_digitos(&g.cst_regular),
            &so_digitos(&g.cclass_trib_regular),
            &format!("{cg}/gTribRegular/cClassTribReg"),
            None,
        )?;
    }
    let difs = [&g.dif_uf, &g.dif_mun, &g.dif_cbs];
    if difs.iter().any(|x| x.is_some()) {
        let d = gi.grupo("gDif");
        for (tag, q) in ["pDifUF", "pDifMun", "pDifCBS"].iter().zip(difs) {
            d.add(
                tag,
                dinheiro(
                    q.as_ref().map_or("0", |v| v.as_str()),
                    &format!("{cg}/gDif/{tag}"),
                    3,
                    true,
                )?,
            );
        }
    }
    if !nt009 || !gi.filhos.is_empty() {
        t.anexar(gi);
    }
    Ok(n)
}
