// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Regras do Anexo I e do Anexo VI conferidas sem rede, antes do envio.

use super::*;

/// Itens da LC 116/2003 em que o tpOper e' obrigatorio (E0903); fora deles, so' com tpEnteGov (E0904). Fonte:
/// AnexoVI NT009 v1.04.01, aba "RN DPS_NFS-e", linhas 705/706. So' pelo cTribNac (4 primeiros digitos = o item); a
/// "correspondente na NBS" do texto nao tem tabela publicada para conferir.
pub const ITENS_COM_TP_OPER: [&str; 4] = ["2505", "1509", "1712", "1005"];
/// Subitens de OBRA: o imovel vai no grupo serv/obra, nunca no IBSCBS/imovel (E0931). Fonte: AnexoVI NT009 v1.04.01,
/// aba "RN DPS_NFS-e", linhas 721-723 (v1.04.00 :684-686); a planilha escreve "07.05,01" = 07.05.01.
pub const SUBITENS_OBRA: [&str; 11] = [
    "070201", "070202", "070401", "070501", "070502", "070601", "070602", "070701", "070801",
    "071701", "071901",
];
/// cIndOp de operacao com imovel: sem eles o IBSCBS/imovel e' proibido (E0928); com eles e fora da obra, obrigatorio
/// (E0932). Mesma fonte do `SUBITENS_OBRA`.
pub const CINDOP_IMOVEL: [&str; 5] = ["020101", "020201", "020202", "020301", "020401"];

/// E0964/E0965: gTribRegular so' (e sempre) com cClassTrib de `ind_gTribRegular` (AnexoVI NT009 v1.04.01, "RN
/// DPS_NFS-e" linhas 795/796). A 796 diz "NAO deve ser informado quando ... for igual a true" -- erro de digitacao
/// da planilha: a mensagem do mesmo codigo e' "Grupo de tributacao regular DEVE ser informado".
pub(super) fn regras_trib_regular(g: &IbsCbs, campo: &str) -> R<()> {
    let cct = so_digitos(&g.cclass_trib);
    let tem = !g.cst_regular.is_empty() || !g.cclass_trib_regular.is_empty();
    let exige = crate::reforma::tabela::classificacao(&cct)
        .map_err(|e| ErroNfse::novo(campo, e.to_string()))?
        .ind_g_trib_regular;
    regra(
        tem && !exige,
        "E0964",
        campo,
        &format!("cClassTrib {cct} nao leva tributacao regular (gTribRegular)"),
    )?;
    regra(
        exige && !tem,
        "E0965",
        campo,
        &format!("cClassTrib {cct} exige a tributacao regular (cst_regular + cclass_trib_regular)"),
    )
}

/// cTribNac de 6 digitos para as regras; formato ruim fica para o `servico` recusar no campo dele.
pub(super) fn ctribnac_ou_digitos(ctribnac: &str) -> String {
    ctribnac::normalizar(ctribnac, false).unwrap_or_else(|_| so_digitos(ctribnac))
}

/// E0850, E0901, E0903-E0906, E0910, E0928, E0931, E0932 (AnexoVI NT009 v1.04.01, aba "RN DPS_NFS-e", linha ao lado
/// de cada uma).
pub(super) fn regras_ibscbs(g: &IbsCbs, ctribnac: &str, competencia: &str) -> R<()> {
    use crate::reforma::indop;
    let c = "infDPS/IBSCBS";
    let comp = data(competencia, "infDPS/dCompet")?;
    regra(
        comp.as_str() < "2026-01-01",
        "E0850",
        c,
        "IBS/CBS so' a partir da competencia 01/01/2026",
    )?; // linha 693
    let cind = so_digitos(&g.cind_op);
    regra(
        cind.len() == 6 && !indop::permitido(&cind, "nfse").unwrap_or(false),
        "E0901",
        &format!("{c}/cIndOp"),
        &format!(
            "cIndOp {cind} nao vale para prestacao de servico (Anexo VII {}, coluna indNFSe)",
            indop::versao()
        ),
    )?;
    let d = so_digitos(ctribnac);
    let no_item = d.len() >= 4 && ITENS_COM_TP_OPER.contains(&&d[..4]);
    let tp = g.tp_oper;
    let campo_tp = format!("{c}/tpOper");
    regra(tp.is_none() && (g.tp_ente_gov.is_some() || no_item), "E0903", &campo_tp,
          "informe o tipo de operacao (tpOper): compra governamental ou servico 25.05/15.09/17.12/10.05")?;
    regra(
        tp.is_some() && g.tp_ente_gov.is_none() && !no_item,
        "E0904",
        &campo_tp,
        "tpOper so' em compra governamental ou nos servicos 25.05/15.09/17.12/10.05",
    )?;
    let campo_ref = format!("{c}/gRefNFSe");
    let com_ref = matches!(tp, Some(2 | 3));
    regra(
        com_ref && g.refs_nfse.is_empty(),
        "E0905",
        &campo_ref,
        "com tpOper 2 ou 3, informe as notas referenciadas (gRefNFSe)",
    )?;
    regra(
        !g.refs_nfse.is_empty() && !com_ref,
        "E0906",
        &campo_ref,
        "notas referenciadas (gRefNFSe) so' com tpOper 2 ou 3",
    )?;
    // v1.04.01 :372 = obrigatorio com indDest 1; v1.04.00 :659 = so' com indDest 1. Mesmo codigo, os dois lados.
    regra(g.destinatario.is_some() != (g.ind_dest == 1), "E0910", &format!("{c}/dest"),
          "o destinatario (dest) vai quando, e so' quando, indDest = 1 (destinatario diferente do tomador)")?;
    let tem_imovel = g.imovel.is_some() || !g.imovel_cib.is_empty() || g.imovel_endereco.is_some();
    let ct = ctribnac_ou_digitos(ctribnac);
    let ci = format!("{c}/imovel");
    if SUBITENS_OBRA.contains(&ct.as_str()) {
        regra(tem_imovel, "E0931", &ci, &format!(  // linha 721
            "no subitem de obra {}.{}.{} o imovel vai no grupo da obra (serv/obra), nao no IBSCBS/imovel",
            &ct[..2], &ct[2..4], &ct[4..]))?;
    } else if CINDOP_IMOVEL.contains(&cind.as_str()) {
        regra(
            !tem_imovel,
            "E0932",
            &ci,
            &format!("cIndOp {cind} e' operacao com imovel: informe o imovel"),
        )?; // 722
    } else {
        regra(
            tem_imovel,
            "E0928",
            &ci,
            &format!(
                // linha 723
                "imovel so' com cIndOp de operacao com imovel ({}); cIndOp {cind} nao e'",
                CINDOP_IMOVEL.join(", ")
            ),
        )?;
    }
    Ok(())
}

/// O leiaute pedido e os campos que so' existem no OUTRO leiaute.
pub(super) fn conferir_leiaute(dps: &Dps) -> R<&'static str> {
    let lei = if dps.leiaute.trim() == LEIAUTE_NT009 {
        LEIAUTE_NT009
    } else {
        LEIAUTE_PRODUCAO
    };
    en(dps.leiaute.as_str(), &LEIAUTES, "leiaute")?;
    let (dd, g) = (dps.valores.deducao.as_ref(), dps.ibscbs.as_ref());
    if lei == LEIAUTE_PRODUCAO {
        if dd.is_some_and(|d| !d.ajustes.is_empty()) {
            return Err(ErroNfse::novo("infDPS/valores/vDedRed/documentos",
                                      "ajustes (docAjusteBC) sao do leiaute nt009; no de producao use documentos (docDedRed)"));
        }
        if g.is_some_and(|g| g.imovel.is_some()) {
            return Err(ErroNfse::novo("infDPS/IBSCBS/imovel",
                                      "imovel=Imovel(...) e' do leiaute nt009; no de producao use imovel_cib/imovel_endereco"));
        }
        if g.is_some_and(|g| !g.bens_moveis.is_empty()) {
            return Err(ErroNfse::novo(
                "infDPS/IBSCBS/bensMoveis",
                "bensMoveis so' existe no leiaute nt009",
            ));
        }
    } else {
        if dd.is_some_and(|d| !d.documentos.is_empty()) {
            return Err(ErroNfse::novo(
                "infDPS/valores/vAjusteBC/documentos",
                "no leiaute nt009 os documentos vao em ajustes (DocAjusteBC, com tpAjusteBC)",
            ));
        }
        if g.is_some_and(|g| !g.reembolsos.is_empty()) {
            return Err(ErroNfse::novo("infDPS/IBSCBS/valores/gReeRepRes",
                                      "no leiaute nt009 o reembolso/repasse vai em valores.deducao.ajustes com tpAjusteBC 101-105/199"));
        }
        if g.is_some_and(|g| !g.imovel_cib.is_empty() || g.imovel_endereco.is_some()) {
            return Err(ErroNfse::novo(
                "infDPS/IBSCBS/imovel",
                "no leiaute nt009 o imovel vai em imovel=Imovel(cmun, locacao, unidades)",
            ));
        }
    }
    Ok(lei)
}

/// Centavos de total x pct/100, arredondado meio-para-par ("half even com tolerancia de R$0,01", Anexo I PRODREST,
/// aba LEIAUTE DPS_NFS-e 1.01, vServ).
pub(super) fn proporcional(total: &str, pct: &str) -> R<u128> {
    let (t, p) = (centavos(total)?, centavos(pct)?);
    let (mut q, r) = ((t * p) / 10000, (t * p) % 10000);
    if r * 2 > 10000 || (r * 2 == 10000 && q % 2 == 1) {
        q += 1;
    }
    Ok(q)
}

/// Regras do leiaute nt009 sem rede (Anexo I PRODREST `anexo_i-sefin_adn-dps_nfse-snnfse_prodrest-v1-01-20261020.xlsx`,
/// aba "RN DPS_NFS-e 1.01", linha ao lado). Ficam com o SEFIN: E0444, E0448,
/// E0933, E0973.
pub(super) fn regras_nt009(dps: &Dps) -> R<()> {
    let (pr, s, v, m) = (
        &dps.prestador,
        &dps.servico,
        &dps.valores,
        &dps.valores.municipal,
    );
    let ct = so_digitos(s.ctribnac.as_str());
    let ct4 = ct.get(..4).unwrap_or(&ct);
    let vazio: Vec<DocAjusteBC> = Vec::new();
    let ajustes = v.deducao.as_ref().map_or(&vazio, |d| &d.ajustes);
    let tem_repasse = ajustes
        .iter()
        .any(|a| TIPO_AJUSTE_BC_REPASSE.contains(&a.tipo.as_str().trim()));
    let padrao = v
        .deducao
        .as_ref()
        .is_some_and(|d| d.percentual.is_some() || d.valor.is_some());
    let v_serv = centavos(v.servico.as_str())?;
    let c = "infDPS/valores/vAjusteBC";
    if v.deducao.is_some() {
        regra(m.tributacao != 1 && !tem_repasse, "E0435", c, // linha 419
              "servico imune, exportado ou sem incidencia so' ajusta a base por documento de repasse/reembolso (tpAjusteBC 101-105 ou 199)")?;
        regra(
            pr.op_simples == 2,
            "E0436",
            c,
            "MEI nao informa ajuste de base de calculo",
        )?; // linha 420
        regra(pr.regime_especial != 0 && !tem_repasse, "E0445", c, // linha 421
              "prestador com regime especial so' ajusta a base por documento de repasse/reembolso (tpAjusteBC 101-105 ou 199)")?;
    }
    for (i, a) in ajustes.iter().enumerate() {
        let ca = format!("{c}/documentos/docAjusteBC[{i}]");
        let tp = a.tipo.as_str().trim();
        let desc = !a.tipo_descricao.is_empty();
        regra(padrao && TIPO_AJUSTE_BC_SO_ISSQN.contains(&tp), "E0462", &format!("{ca}/tpAjusteBC"), // linha 433
              "com o ajuste do ISSQN por percentual ou valor, os documentos nao podem ser dos tipos do ISSQN (2, 5, 6, 7, 8, 9, 99) -- so' repasse/reembolso (101-105, 199)")?;
        regra((ct4 == "0601" || ct4 == "0602") && tp != "9" && tp != "199", "E0463", &format!("{ca}/tpAjusteBC"), // 434
              "nos servicos 6.01/6.02 o ajuste e' profissional parceiro (9) ou outros reembolsos (199)")?;
        regra(
            desc && tp != "99" && tp != "199",
            "E0464",
            &format!("{ca}/xTpAjusteBC"), // linha 435
            "descricao do ajuste so' com tpAjusteBC 99 ou 199",
        )?;
        regra(
            !desc && (tp == "99" || tp == "199"),
            "E0465",
            &format!("{ca}/xTpAjusteBC"), // linha 436
            "com tpAjusteBC 99 ou 199 descreva o ajuste (xTpAjusteBC)",
        )?;
        let emi = if a.emissao.trim().is_empty() {
            String::new()
        } else {
            data(&a.emissao, &format!("{ca}/dtEmiDoc"))?
        };
        let comp = if a.competencia.trim().is_empty() {
            String::new()
        } else {
            data(&a.competencia, &format!("{ca}/dtCompDoc"))?
        };
        regra(!emi.is_empty() && !comp.is_empty() && emi < comp, "E0472", &format!("{ca}/dtEmiDoc"), // linha 437
              "a emissao do documento (dtEmiDoc) nao pode ser anterior a competencia dele (dtCompDoc)")?;
        let aplic = centavos(a.valor_aplicado.as_str())?;
        regra(
            aplic > v_serv,
            "E0467",
            &format!("{ca}/vAjusteAplic"), // linha 440
            "o ajuste aplicado nao pode passar do valor do servico (vServ)",
        )?;
        regra(
            aplic > centavos(a.valor_total.as_str())?,
            "E0469",
            &format!("{ca}/vAjusteAplic"), // linha 441
            "o ajuste aplicado nao pode passar do valor do documento (vTotDoc)",
        )?;
        if !a.dfe_chave.is_empty() {
            let cd = format!("{ca}/dFeNacional");
            // AI:444 escreve "99 - Outro"; no XSD (TSRTCTipoChaveDFe) o "Outro" e' o 9
            regra(
                a.dfe_tipo == Some(9) && a.dfe_tipo_descricao.is_empty(),
                "E0470",
                &format!("{cd}/xTipoChaveDFe"), // 444
                "com tipoChaveDFe 9 (outro DF-e) descreva o documento (xTipoChaveDFe)",
            )?;
            let tam = match a.dfe_tipo {
                Some(1) => Some(50),
                Some(2 | 3) => Some(44),
                _ => None,
            };
            let n = a.dfe_chave.trim().chars().count();
            regra(tam.is_some_and(|t| n != t), "E0460", &format!("{cd}/chaveDFe"), // linha 445
                  &format!("chave DF-e com tamanho errado para o tipo (NFS-e 50, NF-e e CT-e 44): veio {n}"))?;
        } else if !a.outro_numero.is_empty() {
            // sem dtCompDoc a regra nao tem o que comparar e passa (interpretacao da regra)
            regra(!comp.is_empty() && comp.as_str() >= "2026-01-01", "E0476", &format!("{ca}/dtCompDoc"), // linha 446
                  "documento fiscal nao eletronico (docFiscalOutro) so' com competencia ate' 31/12/2025")?;
        }
        regra(["101", "103", "104", "199"].contains(&tp) && a.dfe_chave.is_empty() && a.outro_numero.is_empty(), "E0479",
              &ca, "para este tipo de ajuste informe um documento fiscal (chave DF-e ou documento fiscal), nao docOutro")?;
        // 450
    }
    let Some(g) = &dps.ibscbs else { return Ok(()) };
    let ci = "infDPS/IBSCBS/imovel";
    let im = g.imovel.as_ref();
    let tem_loc = im.is_some_and(|i| i.locacao.is_some());
    regra(
        tem_loc && ct4 != "9903",
        "E0936",
        &format!("{ci}/gLocacao"), // linha 609
        "o grupo gLocacao so' vai na locacao de imovel (cTribNac 99.03)",
    )?;
    regra(
        !tem_loc && ct4 == "9903",
        "E0937",
        &format!("{ci}/gLocacao"), // linha 610
        "na locacao de imovel (cTribNac 99.03) informe o grupo gLocacao",
    )?;
    regra(
        ["020101", "020201", "020301"].contains(&so_digitos(&g.cind_op).as_str())
            && im.is_none_or(|i| i.unidades.is_empty()),
        "E0938",
        &format!("{ci}/gUnidImob"),
        "com cIndOp 020101/020201/020301 informe as unidades imobiliarias",
    )?; // 616
    let lo = im.and_then(|i| i.locacao.as_ref());
    if let Some(lo) = lo {
        let pc = centavos(lo.copropriedade.as_str())?;
        regra(
            pc > 10000,
            "E0940",
            &format!("{ci}/gLocacao/pCopropriedade"), // linha 611
            "percentual de copropriedade entre 0 e 100",
        )?;
    }
    for (i, u) in im.map_or(&[][..], |i| &i.unidades[..]).iter().enumerate() {
        for (j, a) in u.ajustes.iter().enumerate() {
            let ca = format!("{ci}/gUnidImob[{i}]/gAjusteBCLocImoveis[{j}]");
            let tp = format!("{:0>2}", a.tipo.as_str().trim());
            regra(
                ct4 != "9903",
                "E0939",
                &ca,
                "ajuste da base do IBS/CBS de imovel so' na locacao (cTribNac 99.03)",
            )?; // 625
            regra(
                tp == "04" && so_digitos(&s.cnbs) != "110021000",
                "E0944",
                &format!("{ca}/tpAjusteBCLocImoveis"), // 626
                "redutor social (04) so' na locacao residencial (NBS 1.1002.10.00)",
            )?;
            regra(
                tp == "99" && a.descricao.is_empty(),
                "E0949",
                &format!("{ca}/xTpAjusteBCLocImoveis"), // linha 627
                "com tpAjusteBCLocImoveis 99 descreva a parcela",
            )?;
            if let Some(lo) = lo {
                regra(
                    centavos(a.valor.as_str())? > centavos(lo.valor_total.as_str())?,
                    "E0966",
                    &format!("{ca}/vAjusteBCLocImoveis"), // linha 629
                    "o ajuste nao pode passar do valor total da operacao (vTotOper)",
                )?;
            }
        }
    }
    regra(
        !g.bens_moveis.is_empty() && ct != "990401",
        "E0967",
        "infDPS/IBSCBS/bensMoveis", // linha 630
        "bensMoveis so' na locacao de bem movel (cTribNac 99.04.01)",
    )?;
    regra(
        g.bens_moveis.is_empty() && ct == "990401",
        "E0963",
        "infDPS/IBSCBS/bensMoveis", // linha 631
        "na locacao de bem movel (cTribNac 99.04.01) informe os bens (bensMoveis)",
    )?;
    for (i, b) in g.bens_moveis.iter().enumerate() {
        let q = so_digitos(b.quantidade.as_str());
        regra(
            !q.is_empty() && q.chars().all(|c| c == '0'),
            "E0974",
            &format!("infDPS/IBSCBS/bensMoveis[{i}]/qtdNCMBemMovel"),
            "a quantidade de bens moveis tem de ser maior que zero",
        )?; // linha 634
    }
    if let (true, Some(lo)) = (ct4 == "9903", lo) {
        let cv = "infDPS/valores";
        let pares = [
            (
                "E0433",
                format!("{cv}/vServPrest/vServ"),
                Some(&v.servico),
                Some(&lo.valor_total),
                true,
                "o valor do servico",
            ), // 413
            (
                "E0434",
                format!("{cv}/vDescCondIncond/vDescIncond"),
                v.desconto_incondicionado.as_ref(),
                lo.desconto_incondicionado.as_ref(),
                false,
                "o desconto incondicionado",
            ), // linha 416
            (
                "E0443",
                format!("{cv}/vDescCondIncond/vDescCond"),
                v.desconto_condicionado.as_ref(),
                lo.desconto_condicionado.as_ref(),
                false,
                "o desconto condicionado",
            ), // linha 418
        ];
        for (cod, campo, valor, total, sempre, nome) in pares {
            if sempre || valor.is_some() || total.is_some() {
                let esperado =
                    proporcional(total.map_or("0", |x| x.as_str()), lo.copropriedade.as_str())?;
                let tem = centavos(valor.map_or("0", |x| x.as_str()))?;
                regra(tem.abs_diff(esperado) > 1, cod, &campo,
                      &format!("na locacao (99.03) {nome} e' o total da operacao x a copropriedade ({}.{:02})",
                               esperado / 100, esperado % 100))?;
            }
        }
    }
    Ok(())
}

/// Regras do Anexo I que cruzam grupos e nao dependem do cadastro do SEFIN.
pub(super) fn regras_cruzadas(dps: &Dps, ident: &str, dh_emi: &str) -> R<()> {
    let p = &dps.prestador.pessoa;
    let (t, v, m) = (dps.tomador.as_ref(), &dps.valores, &dps.valores.municipal);
    let (trib, ret) = (m.tributacao, m.retencao);
    if let Some(t) = t {
        if !t.documento.is_empty() && !p.documento.is_empty() {
            regra(
                so_alnum(&t.documento) == so_alnum(&p.documento)
                    && so_alnum(&p.documento).len() == 14,
                "E0202",
                "infDPS/toma/CNPJ",
                "o tomador nao pode ser o proprio prestador",
            )?;
        }
    }
    regra(
        ret == 2 && t.is_none_or(|t| t.documento.is_empty()),
        "E0204",
        "infDPS/toma",
        "com ISS retido pelo tomador, o tomador precisa de CPF/CNPJ",
    )?;
    regra(
        ret == 3
            && dps
                .intermediario
                .as_ref()
                .is_none_or(|i| i.documento.is_empty()),
        "E0264",
        "infDPS/interm",
        "com ISS retido pelo intermediario, o intermediario precisa de CPF/CNPJ",
    )?;
    regra(
        dps.emitente == 2 && t.is_some_and(|t| !t.nif.is_empty()),
        "E0222",
        "infDPS/toma/NIF",
        "quando o tomador emite, ele nao se identifica por NIF",
    )?;
    let s = &dps.servico;
    regra(
        !s.pais_prestacao.is_empty() && s.pais_prestacao.to_uppercase() == "BR",
        "E0304",
        "infDPS/serv/locPrest",
        "servico no Brasil vai pelo municipio (cLocPrestacao), nao pelo pais",
    )?;
    regra(
        !s.ctribmun.is_empty() && so_digitos(&s.ctribmun) == "000",
        "E0315",
        "infDPS/serv/cServ/cTribMun",
        "000 nao e' aceito como codigo municipal",
    )?;
    regra(
        dps.ibscbs.is_some() && s.cnbs.is_empty(),
        "E0322",
        "infDPS/serv/cServ/cNBS",
        "com o grupo IBS/CBS a NBS do servico e' obrigatoria",
    )?;
    if let Some(g) = &dps.ibscbs {
        regras_ibscbs(g, s.ctribnac.as_str(), &competencia(dps))?;
    }
    if let Some(r) = &v.recebido {
        regra(
            centavos(r.as_str())? < centavos(v.servico.as_str())?,
            "E0425",
            "infDPS/valores/vServPrest/vReceb",
            "o valor recebido nao pode ser menor que o valor do servico",
        )?;
    }
    let nt009 = dps.leiaute.trim() == LEIAUTE_NT009;
    if let Some(p) = v.deducao.as_ref().and_then(|d| d.percentual.as_ref()) {
        let c = centavos(p.as_str())?;
        // nt009: mesma regra no pAjusteBCISSQN (Anexo I PRODREST, RN DPS_NFS-e 1.01, linha 428)
        let campo = if nt009 {
            "infDPS/valores/vAjusteBC/pAjusteBCISSQN"
        } else {
            "infDPS/valores/vDedRed/pDR"
        };
        regra(
            !(c > 0 && c <= 10000),
            "E0453",
            campo,
            "percentual de deducao entre 0 e 100",
        )?;
    }
    if nt009 {
        regras_nt009(dps)?;
    }
    regra(
        trib != 1 && (ret == 2 || ret == 3),
        "E0580",
        "infDPS/valores/trib/tribMun/tpRetISSQN",
        "servico imune, exportado ou sem incidencia nao tem ISS retido",
    )?;
    regra(
        trib == 2 && m.imunidade.is_none(),
        "E0592",
        "infDPS/valores/trib/tribMun/tpImunidade",
        "com imunidade, informe o tipo de imunidade",
    )?;
    if m.aliquota.is_some() {
        regra(
            dps.prestador.op_simples == 2,
            "E0600",
            "infDPS/valores/trib/tribMun/pAliq",
            "MEI nao informa aliquota",
        )?;
        regra(
            trib != 1,
            "E0602",
            "infDPS/valores/trib/tribMun/pAliq",
            "servico imune, exportado ou sem incidencia nao informa aliquota",
        )?;
        regra(
            dps.prestador.regime_especial != 0,
            "E0604",
            "infDPS/valores/trib/tribMun/pAliq",
            "prestador com regime especial nao informa aliquota",
        )?;
    }
    let comp = data(&competencia(dps), "infDPS/dCompet")?;
    regra(
        comp.as_str() > &dh_emi[..10],
        "E0015",
        "infDPS/dCompet",
        "a competencia nao pode ser depois da data de emissao",
    )?;
    if let Some(sb) = &dps.substituicao {
        let ch = so_alnum(&sb.chave);
        regra(
            ch.len() != 50 || ch[..7] != ident[3..10] || ch[8..23] != ident[10..25],
            "E0042",
            "infDPS/subst/chSubstda",
            "a nota substituida tem de ser do mesmo municipio e da mesma inscricao desta DPS",
        )?;
        regra(
            chave_dfe::conferir(&ch).is_err(),
            "E0042",
            "infDPS/subst/chSubstda",
            "digito verificador da chave substituida nao confere",
        )?;
    }
    Ok(())
}

/// Forma do `totTrib` x situacao do prestador no Simples Nacional (Anexo I v1.01, aba "RN DPS_NFS-e": E0712, E0713 e
/// E0710, portal da NFS-e, <https://www.gov.br/nfse>):
///
/// * MEI (`op_simples` 2): `pTotTribSN` nunca (E0710); sem forma informada sai `indTotTrib` 0;
/// * ME/EPP (3): `indTotTrib` nunca (E0712); sem forma informada a DPS nao fecha, pois so' restam valores,
///   percentuais ou o percentual do Simples (`percentual_sn`) e nao ha numero padrao;
/// * nao optante (1): `indTotTrib` e `pTotTribSN` nunca (E0713); informe valores ou percentuais.
///
/// Roda depois das demais conferencias, para que cada recusa mantenha a ordem de sempre.
pub(super) fn regras_tot_trib(dps: &Dps) -> R<()> {
    let t = &dps.valores.total;
    let tem_val =
        t.valor_federal.is_some() || t.valor_estadual.is_some() || t.valor_municipal.is_some();
    let tem_per = t.percentual_federal.is_some()
        || t.percentual_estadual.is_some()
        || t.percentual_municipal.is_some();
    let sn = t.percentual_sn.is_some();
    let sem_forma = !tem_val && !tem_per && !sn;
    let base = "infDPS/valores/trib/totTrib";
    match dps.prestador.op_simples {
        2 => regra(
            sn,
            "E0710",
            &format!("{base}/pTotTribSN"),
            "MEI nao informa o percentual aproximado dos tributos do Simples Nacional (pTotTribSN); informe valores ou percentuais, ou nenhuma forma",
        ),
        3 => regra(
            sem_forma,
            "E0712",
            &format!("{base}/indTotTrib"),
            "ME/EPP nao pode usar o indicador indTotTrib: informe valores (vTotTrib), percentuais (pTotTrib) ou o percentual do Simples Nacional (pTotTribSN)",
        ),
        1 => {
            regra(
                sn,
                "E0713",
                &format!("{base}/pTotTribSN"),
                "nao optante pelo Simples Nacional nao informa pTotTribSN: informe valores (vTotTrib) ou percentuais (pTotTrib)",
            )?;
            regra(
                sem_forma,
                "E0713",
                &format!("{base}/indTotTrib"),
                "nao optante pelo Simples Nacional nao pode usar o indicador indTotTrib: informe valores (vTotTrib) ou percentuais (pTotTrib)",
            )
        }
        _ => Ok(()),
    }
}

/// CST x cClassTrib pela tabela oficial (`reforma::tabela::validar`): existe, e' do CST e vale no documento --
/// alem do FORMATO, confere a existencia na tabela. `documento` None = so' existencia e CST (gTribRegular).
/// Codigos da NFS-e (AnexoVI NT009 v1.04.01, aba "RN DPS_NFS-e"): os 3 primeiros digitos do cClassTrib = o CST
/// (E0959, linha 778; no gTribRegular E0970, linha 801) e o cClassTrib vale na NFS-e (E0958, linha 777).
pub(super) fn par_da_tabela(
    cst: &str,
    cclass_trib: &str,
    campo: &str,
    documento: Option<&str>,
) -> R<()> {
    use crate::reforma::tabela;
    let prefixo = cclass_trib.get(..3).unwrap_or(cclass_trib);
    regra(prefixo != cst, if documento.is_some() { "E0959" } else { "E0970" }, campo,
          &format!("cClassTrib {cclass_trib} e' do CST {prefixo}, nao do {cst}: os 3 primeiros digitos sao o CST"))?;
    let k = tabela::classificacao(cclass_trib).map_err(|e| ErroNfse::novo(campo, e.to_string()))?;
    if let Some(d) = documento {
        let docs = if k.documentos.is_empty() {
            "nenhum documento".to_string()
        } else {
            k.documentos.join(", ")
        };
        regra(
            !k.vale_em(d),
            "E0958",
            campo,
            &format!(
                "cClassTrib {} nao vale na {} pela tabela oficial (vale em: {docs})",
                k.codigo,
                d.to_uppercase()
            ),
        )?;
        tabela::validar(cst, cclass_trib, d, None)
            .map_err(|e| ErroNfse::novo(campo, e.to_string()))?;
    }
    Ok(())
}
