// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O que o DANFSe mostra, ja formatado e sem posicao: dados do XML da NFS-e organizados pelos blocos da NT 008.

use serde_json::{json, Value};

use super::formato::*;
use super::tabelas::*;
use super::xml::{documento as xml_documento, el, filho, filhos, g, int, No};
use super::{municipio, ErroDanfse, Opcoes, Situacao, CONSULTA, NS};

fn resolver(op: &Opcoes, codigo: &str) -> Option<(String, String)> {
    let c: String = codigo.chars().filter(char::is_ascii_digit).collect();
    if c.is_empty() {
        return None;
    }
    match op.municipio {
        Some(f) => f(&c),
        None => municipio::buscar(&c),
    }
}

// ------------------------------------------------------------------------------------------------ conteudo
#[allow(clippy::too_many_arguments)]
fn parte(
    r: Option<No>,
    nac: Option<No>,
    ext: Option<No>,
    base: Option<No>,
    op: &Opcoes,
    titulo: &str,
    im: bool,
    xml_mun: &str,
) -> Value {
    let (mut mun, mut uf, mut cmun, mut cep) =
        (String::new(), String::new(), String::new(), String::new());
    let tem_end = nac.is_some() || ext.is_some();
    if nac.is_some() {
        cmun = g(nac, "cMun");
        cep = g(nac, "CEP");
        let achou = resolver(op, &cmun);
        mun = if xml_mun.is_empty() {
            achou.as_ref().map_or(String::new(), |a| a.0.clone())
        } else {
            xml_mun.into()
        };
        uf = achou
            .map(|a| a.1)
            .filter(|x| !x.is_empty())
            .unwrap_or_else(|| g(nac, "UF"));
    } else if ext.is_some() {
        mun = g(ext, "xCidade");
        cep = g(ext, "cEndPost");
    }
    json!({
        "titulo": titulo, "visivel": true, "mensagem": "",
        "documento": traco(&documento(&primeiro(&[g(r, "CNPJ"), g(r, "CPF"), g(r, "NIF")]))),
        "im": if im { traco(&g(r, "IM")) } else { String::new() },
        "telefone": traco(&telefone(&g(r, "fone"))),
        "nome": traco(&g(r, "xNome")),
        "municipio_uf": if tem_end { municipio_uf(&mun, &uf) } else { "-".into() },
        "ibge_cep": if tem_end { ibge_cep(&cmun, &cep) } else { "-".into() },
        "endereco": if base.is_some() {
            endereco(&[g(base, "xLgr"), g(base, "nro"), g(base, "xCpl"), g(base, "xBairro")])
        } else { "-".into() },
        "email": traco(&g(r, "email")),
    })
}

fn parte_dps(e: Option<No>, op: &Opcoes, titulo: &str, im: bool) -> Value {
    let end = filho(e, "end");
    parte(
        e,
        filho(end, "endNac"),
        filho(end, "endExt"),
        end,
        op,
        titulo,
        im,
        "",
    )
}

fn suprimida(titulo: &str, msg: &str) -> Value {
    json!({"titulo": titulo, "visivel": false, "mensagem": msg})
}

fn texto_complementar(dps: Option<No>) -> String {
    let serv = filho(dps, "serv");
    let ic = filho(serv, "infoCompl");
    let mut insc = g(serv, "obra/inscImobFisc");
    if insc.is_empty() {
        insc = g(dps, "IBSCBS/imovel/inscImobFisc");
    }
    let itens: Vec<String> = filhos(ic, "gItemPed")
        .into_iter()
        .map(|e| g(Some(e), "xItemPed"))
        .filter(|x| !x.is_empty())
        .collect();
    let grupos: Vec<(&str, String)> = vec![
        ("Inf. Cont.:", g(ic, "xInfComp")),
        ("NFS-e Subst.:", g(dps, "subst/chSubstda")),
        ("Doc. Ref.:", g(ic, "docRef")),
        ("Cod. Obra:", g(serv, "obra/cObra")),
        ("Insc. Imob.:", insc),
        ("Cod. Evt.:", g(serv, "atvEvento/idAtvEvt")),
        ("Doc. Tec.:", g(ic, "idDocTec")),
        ("Núm. Ped.:", g(ic, "xPed")),
        ("Item Ped.:", itens.join(", ")),
        ("Inf. A. T. Mun.:", g(ic, "xOutInf")),
    ];
    grupos
        .into_iter()
        .filter(|(_, v)| !v.is_empty())
        .map(|(r, v)| format!("{r} {v}"))
        .collect::<Vec<_>>()
        .join(" | ")
}

fn linha_tributos(dps: Option<No>) -> String {
    let tt = el(dps, "valores/trib/totTrib");
    let v = |x: &str| z(d(tt, &format!("vTotTrib/vTotTrib{x}")));
    let (vf, ve, vm) = (v("Fed"), v("Est"), v("Mun"));
    if vf.0 > 0 || ve.0 > 0 || vm.0 > 0 {
        return format!(
            "{LEI_TRIBUTOS}Federais: {}; Estaduais: {}; Municipais: {};",
            moeda(Some(vf)),
            moeda(Some(ve)),
            moeda(Some(vm))
        );
    }
    let p = |x: &str| z(d(tt, &format!("pTotTrib/pTotTrib{x}")));
    let (pf, pe, pm) = (p("Fed"), p("Est"), p("Mun"));
    if pf.0 > 0 || pe.0 > 0 || pm.0 > 0 {
        return format!(
            "{LEI_TRIBUTOS}Federais: {}; Estaduais: {}; Municipais: {};",
            percentual(Some(pf), true),
            percentual(Some(pe), true),
            percentual(Some(pm), true)
        );
    }
    format!("{LEI_TRIBUTOS}Federais: R$ 0,00; Estaduais: R$ 0,00; Municipais: R$ 0,00;")
}

fn compl_com_totais(base: &str, linha: &str) -> String {
    if base.is_empty() {
        return linha.into();
    }
    let lim = MAX_COMPLEMENTAR - 3;
    let n = base.chars().count();
    let corpo = if n <= lim {
        base.to_string()
    } else {
        let p: String = base.chars().take(lim).collect();
        format!("{}...", rstrip_esp_linha(&p))
    };
    if linha.is_empty() {
        corpo
    } else {
        format!("{corpo} | {linha}")
    }
}

fn exibir_pis_cofins(dcompet: &str) -> bool {
    !(eh_data(dcompet) && dcompet > "2026-12-31")
}

fn add(a: Dec, b: Dec) -> Dec {
    Dec(a.0 + b.0)
}

/// O que o DANFSe mostra, ja' formatado (sem posicao).
pub(super) fn conteudo(xml: &str, op: &Opcoes) -> Result<Value, ErroDanfse> {
    let marca = match op.situacao {
        Situacao::Valida => "",
        Situacao::Cancelada => "CANCELADA",
        Situacao::Substituida => "SUBSTITUÍDA",
    };
    let doc = xml_documento(xml, NS)?;
    let raiz = doc.root_element();
    if raiz.tag_name().name() != "NFSe" {
        let n: String = raiz.tag_name().name().chars().take(20).collect();
        return Err(ErroDanfse::NaoENfse(n));
    }
    let inf = filho(Some(raiz), "infNFSe");
    let ident = inf
        .and_then(|e| e.attribute("Id"))
        .unwrap_or("")
        .trim()
        .to_string();
    if ident.is_empty() {
        return Err(ErroDanfse::SemChave);
    }
    let dps = el(inf, "DPS/infDPS");
    let chave = ident.replace("NFS", "");
    let emit = filho(inf, "emit");
    let enac = filho(emit, "enderNac");
    let achou_emit = resolver(op, &g(enac, "cMun"));
    let x_loc_emi = g(inf, "xLocEmi");
    let mun_emit = if x_loc_emi.is_empty() {
        achou_emit.as_ref().map_or(String::new(), |a| a.0.clone())
    } else {
        x_loc_emi.clone()
    };
    let uf_emit = achou_emit
        .as_ref()
        .map(|a| a.1.clone())
        .filter(|x| !x.is_empty())
        .unwrap_or_else(|| g(enac, "UF"));
    let ctrib = g(dps, "serv/cServ/cTribNac");
    let tpamb = int(dps, "tpAmb");

    // partes
    let mut prest = parte(
        emit,
        enac,
        filho(emit, "enderExt"),
        enac,
        op,
        "PRESTADOR / FORNECEDOR",
        true,
        &x_loc_emi,
    );
    let tem_prest = filho(dps, "prest").is_some();
    prest["simples_nacional"] = json!(if tem_prest {
        tab(&OP_SIMPLES, int(dps, "prest/regTrib/opSimpNac"))
    } else {
        "-".into()
    });
    prest["regime_sn"] = json!(if tem_prest {
        tab(&REG_AP_SN, int(dps, "prest/regTrib/regApTribSN"))
    } else {
        "-".into()
    });
    let toma = filho(dps, "toma");
    let tomador = if toma.is_some() {
        parte_dps(toma, op, "TOMADOR / ADQUIRENTE", true)
    } else {
        suprimida("TOMADOR / ADQUIRENTE", SEM_TOMADOR)
    };
    let dest = el(dps, "IBSCBS/dest");
    let destinatario = if dest.is_none() {
        suprimida("DESTINATÁRIO DA OPERAÇÃO", SEM_DESTINATARIO)
    } else if toma.is_some()
        && ["CNPJ", "CPF", "NIF"]
            .iter()
            .any(|k| !g(toma, k).is_empty() && g(toma, k) == g(dest, k))
    {
        suprimida("DESTINATÁRIO DA OPERAÇÃO", DEST_TOMADOR)
    } else {
        parte_dps(dest, op, "DESTINATÁRIO DA OPERAÇÃO", false)
    };
    let interm = filho(dps, "interm");
    let intermediario = if interm.is_some() {
        parte_dps(interm, op, "INTERMEDIÁRIO DA OPERAÇÃO", true)
    } else {
        suprimida("INTERMEDIÁRIO DA OPERAÇÃO", SEM_INTERMEDIARIO)
    };

    // servico
    let cmun_trib = g(dps, "serv/cServ/cTribMun");
    let cod_trib = if ctrib.is_empty() {
        cmun_trib.clone()
    } else if cmun_trib.is_empty() {
        ctribnac(&ctrib)
    } else {
        format!("{} / {cmun_trib}", ctribnac(&ctrib))
    };
    let achou_lp = resolver(op, &g(dps, "serv/locPrest/cLocPrestacao"));
    let servico = json!({
        "codigo_trib": if cod_trib.is_empty() { "-".into() } else { cod_trib },
        "nbs": traco(&nbs(&g(dps, "serv/cServ/cNBS"))),
        "local_prestacao": com_barra(&[g(inf, "xLocPrestacao"), achou_lp.map_or(String::new(), |a| a.1),
                                       pais(&g(dps, "serv/locPrest/cPaisPrestacao"))]),
        "descricao_trib": traco(&primeiro(&[g(inf, "xTribMun"), g(inf, "xTribNac")])),
        "descricao": truncar(&g(dps, "serv/cServ/xDescServ"), MAX_DESCRICAO - 3),
    });

    // tributacao municipal
    let tm = el(dps, "valores/trib/tribMun");
    let vinf = filho(inf, "valores");
    let calc_bm = d(vinf, "vCalcBM").or_else(|| d(tm, "BM/vRedBCBM"));
    let reg_esp = int(dps, "prest/regTrib/regEspTrib");
    let tp_bm = int(vinf, "tpBM");
    let v_dr = d(dps, "valores/vDedRed/vDR");
    let v_desc_inc = d(dps, "valores/vDescCondIncond/vDescIncond");
    let opcionais =
        tp_bm > 0 || z(d(vinf, "vCalcBM")).0 > 0 || z(v_dr).0 > 0 || z(v_desc_inc).0 > 0;
    let achou_inc = resolver(op, &g(inf, "cLocIncid"));
    let x_li = g(inf, "xLocIncid");
    let x_loc_inc = if x_li.is_empty() {
        achou_inc.as_ref().map_or(String::new(), |a| a.0.clone())
    } else {
        x_li
    };
    let uf_inc = achou_inc
        .as_ref()
        .map_or_else(|| g(inf, "xUFIncid"), |a| a.1.clone());
    let tem_mun = tm.is_some()
        && (int(tm, "tribISSQN") != 0
            || !x_loc_inc.trim().is_empty()
            || !uf_inc.trim().is_empty()
            || !g(tm, "cPaisResult").is_empty()
            || reg_esp != 0
            || int(tm, "tpImunidade") != 0
            || int(tm, "exigSusp/tpSusp") != 0
            || !g(tm, "exigSusp/nProcesso").is_empty()
            || tp_bm != 0
            || z(calc_bm).0 != 0
            || z(v_dr).0 != 0
            || z(v_desc_inc).0 != 0
            || z(d(vinf, "vBC")).0 != 0
            || d(vinf, "pAliqAplic").is_some()
            || int(tm, "tpRetISSQN") != 0
            || z(d(vinf, "vISSQN")).0 != 0);
    let municipal = if tem_mun {
        let ou = |s: String| if opcionais { s } else { "-".into() };
        json!({
            "visivel": true, "mensagem": "",
            "trib_issqn": tab(&TRIB_ISSQN, int(tm, "tribISSQN")),
            "local_incidencia": com_barra(&[x_loc_inc, uf_inc, pais(&g(tm, "cPaisResult"))]),
            "reg_esp": tab(&REG_ESP, reg_esp),
            "imunidade": tab(&IMUNIDADE, int(tm, "tpImunidade")),
            "suspensao": tab(&SUSPENSAO, int(tm, "exigSusp/tpSusp")),
            "processo": traco(&g(tm, "exigSusp/nProcesso")),
            "beneficio": ou(tab(&BENEFICIO, tp_bm)),
            "calculo_bm": ou(moeda_ou_traco(calc_bm)),
            "deducoes": ou(moeda_ou_traco(v_dr)),
            "desconto_incond": ou(moeda_ou_traco(v_desc_inc)),
            "bc": moeda_ou_traco(d(vinf, "vBC")),
            "aliquota": percentual_ou_traco(d(vinf, "pAliqAplic"), false),
            "retencao": tab(&RET_ISSQN, int(tm, "tpRetISSQN")),
            "issqn": moeda_ou_traco(d(vinf, "vISSQN")),
        })
    } else {
        json!({"visivel": false, "mensagem": SEM_ISSQN})
    };

    // tributacao federal
    let tf = el(dps, "valores/trib/tribFed");
    let pc = filho(tf, "piscofins");
    let mut federal = json!({
        "irrf": moeda_ou_traco(d(tf, "vRetIRRF")), "cp": moeda_ou_traco(d(tf, "vRetCP")),
        "csll_ret": moeda_ou_traco(d(tf, "vRetCSLL")), "pis": "-", "cofins": "-", "desc_ret": "-",
        "exibir_pis_cofins": exibir_pis_cofins(&g(dps, "dCompet")),
    });
    if pc.is_some() {
        let tp = int(pc, "tpRetPisCofins");
        if tp == 1 {
            federal["pis"] = json!(moeda(Some(Dec(0))));
            federal["cofins"] = json!(moeda(Some(Dec(0))));
            let s = add(
                add(z(d(tf, "vRetCSLL")), z(d(pc, "vPis"))),
                z(d(pc, "vCofins")),
            );
            federal["csll_ret"] = json!(moeda(Some(s)));
        } else {
            federal["pis"] = json!(moeda_ou_traco(d(pc, "vPis")));
            federal["cofins"] = json!(moeda_ou_traco(d(pc, "vCofins")));
        }
        federal["desc_ret"] = json!(tab(&RET_PISCOFINS, tp));
    }

    // IBS/CBS
    let ib = filho(inf, "IBSCBS");
    let gib = el(dps, "IBSCBS/valores/trib/gIBSCBS");
    let (mut cst, mut cclass, mut cindop) = ("-".to_string(), "-".to_string(), "-".to_string());
    if ib.is_some() && filho(dps, "IBSCBS").is_some() {
        cst = traco(&g(gib, "CST"));
        cclass = traco(&g(gib, "cClassTrib"));
        cindop = traco(&g(dps, "IBSCBS/cIndOp"));
    }
    let achou_loc = resolver(op, &g(ib, "cLocalidadeIncid"));
    let xl = g(ib, "xLocalidadeIncid");
    let x_loc = if xl.is_empty() {
        achou_loc.as_ref().map_or(String::new(), |a| a.0.clone())
    } else {
        xl
    };
    let uf_loc = achou_loc
        .as_ref()
        .map(|a| a.1.clone())
        .filter(|x| !x.is_empty())
        .unwrap_or_else(|| g(ib, "uf"));
    let excl = [
        z(v_desc_inc),
        z(d(vinf, "vISSQN")),
        z(d(vinf, "vCalcDR")),
        z(d(ib, "valores/vCalcReeRepRes")),
        z(d(pc, "vPis")),
        z(d(pc, "vCofins")),
    ]
    .into_iter()
    .fold(Dec(0), add);
    let p = |c: &str| percentual_ou_traco(d(ib, c), true);
    let ibscbs = json!({
        "cst_cclasstrib": valores_com_barra(&[cst, cclass]),
        "indop_local": valores_com_barra(&[cindop, traco(&g(ib, "cLocalidadeIncid")), traco(&x_loc), traco(&uf_loc)]),
        "exclusoes": moeda_ou_traco(Some(excl)),
        "bc": moeda_ou_traco(d(ib, "valores/vBC")),
        "red_aliq": valores_com_barra(&[p("valores/uf/pRedAliqUF"), p("valores/mun/pRedAliqMun"), p("valores/fed/pRedAliqCBS")]),
        "aliq_uf_mun": valores_com_barra(&[p("valores/uf/pIBSUF"), p("valores/mun/pIBSMun")]),
        "aliq_efet_mun": p("valores/mun/pAliqEfetMun"),
        "valor_mun": moeda_ou_traco(d(ib, "totCIBS/gIBS/gIBSMunTot/vIBSMun")),
        "aliq_efet_uf": p("valores/uf/pAliqEfetUF"),
        "valor_uf": moeda_ou_traco(d(ib, "totCIBS/gIBS/gIBSUFTot/vIBSUF")),
        "valor_ibs": moeda_ou_traco(d(ib, "totCIBS/gIBS/vIBSTot")),
        "aliq_cbs": p("valores/fed/pCBS"),
        "aliq_efet_cbs": p("valores/fed/pAliqEfetCBS"),
        "valor_cbs": moeda_ou_traco(d(ib, "totCIBS/gCBS/vCBS")),
    });

    // totais
    let soma = add(
        z(d(ib, "totCIBS/gIBS/vIBSTot")),
        z(d(ib, "totCIBS/gCBS/vCBS")),
    );
    let totais = json!({
        "valor_servico": moeda(d(dps, "valores/vServPrest/vServ")),
        "desconto_incond": moeda_ou_traco(v_desc_inc),
        "desconto_cond": moeda_ou_traco(d(dps, "valores/vDescCondIncond/vDescCond")),
        "retencoes": moeda_ou_traco(d(vinf, "vTotalRet")),
        "liquido": moeda(d(vinf, "vLiq")),
        "ibscbs": if ib.is_none() || soma.0 == 0 { "-".into() } else { moeda(Some(soma)) },
        "liquido_ibscbs": if ib.is_none() { "-".into() } else { moeda_ou_traco(d(ib, "totCIBS/vTotNF")) },
    });

    let base = texto_complementar(dps);
    let linha = linha_tributos(dps);
    let fin = g(dps, "IBSCBS/finNFSe");
    let item99 = ctrib.starts_with("99");
    Ok(json!({
        "marca_dagua": marca,
        "cabecalho": {
            "municipio": if item99 { String::new() } else { mun_emit },
            "uf": if item99 { String::new() } else { uf_emit },
            "ambiente_gerador": amb_ger(&g(inf, "ambGer")),
            "tipo_ambiente": tab(&TP_AMB, tpamb),
            "sem_validade": tpamb == 2,
        },
        "acesso": {
            "chave": traco(&chave), "qr_url": format!("{CONSULTA}{}", url_quote(&chave)),
            "numero": traco(&g(inf, "nNFSe")), "competencia": data(&g(dps, "dCompet")),
            "emissao_nfse": data_hora(&g(inf, "dhProc")), "numero_dps": traco(&g(dps, "nDPS")),
            "serie_dps": traco(&g(dps, "serie")), "emissao_dps": data_hora(&g(dps, "dhEmi")),
            "emitente": tab(&TP_EMIT, int(dps, "tpEmit")),
            "situacao": cstat(&g(inf, "cStat")),
            "finalidade": if !fin.is_empty() && fin.chars().all(|c| c.is_ascii_digit()) {
                tab(&FIN_NFSE, fin.parse().unwrap_or(-1))
            } else { "-".into() },
        },
        "partes": {"prestador": prest, "tomador": tomador, "destinatario": destinatario, "intermediario": intermediario},
        "servico": servico, "municipal": municipal, "federal": federal, "ibscbs": ibscbs, "totais": totais,
        "complementares": {"texto": compl_com_totais(&base, &linha), "linha_tributos": linha},
    }))
}

/// Codifica em percentual (RFC 3986) tudo que nao e' letra, digito ou `_.-~`.
fn url_quote(s: &str) -> String {
    let mut o = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"_.-~".contains(&b) {
            o.push(b as char);
        } else {
            o.push_str(&format!("%{b:02X}"));
        }
    }
    o
}
