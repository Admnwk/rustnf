// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Emitente, destinatario, retirada e entrega, e o que o regime do emitente (CRT e `tpRegimeIBSCBS`) permite no item
//! do IBS/CBS.

use chrono::NaiveDate;

use super::formato::{digitos, documento, erro, erro_r, txt, txt_ou_none, uf_codigo, Res};
use super::modelo::{Endereco, Item, Local, Nota};
use super::{vigencia, X};

/// Dominio de comprimento da inscricao estadual (2 a 14 digitos).
fn tam_ie() -> Vec<usize> {
    (2..15).collect()
}

pub(crate) fn endereco(x: &mut X, tag: &str, e: &Endereco, ra: bool, cep_obrig: bool) -> Res<()> {
    x.abre(tag);
    campos_endereco(x, tag, e, ra, cep_obrig, true)?;
    x.fecha(tag);
    Ok(())
}

/// Retirada ou entrega (`TLocal`): CNPJ ou CPF, `xNome`, endereco, CEP, pais, fone, `email` e `IE`, na ordem do XSD.
pub(crate) fn local(x: &mut X, tag: &str, lc: Option<&Local>, ra: bool) -> Res<()> {
    let Some(lc) = lc else { return Ok(()) };
    x.abre(tag);
    let (tipo, doc) = documento(&lc.documento, &format!("{tag}/CNPJ"))?;
    x.t(tipo, &doc);
    x.o(
        "xNome",
        &txt_ou_none(&lc.nome, &format!("{tag}/xNome"), 60, 2, ra)?,
    );
    campos_endereco(x, tag, &lc.endereco, ra, false, false)?;
    x.o(
        "email",
        &txt_ou_none(&lc.email, &format!("{tag}/email"), 60, 1, false)?,
    );
    if let Some(ie) = lc
        .ie
        .as_ref()
        .map(|s| s.trim().to_uppercase())
        .filter(|s| !s.is_empty())
    {
        let v = if ie == "ISENTO" {
            ie
        } else {
            digitos(&ie, &format!("{tag}/IE"), &tam_ie())?
        };
        x.t("IE", &v);
    }
    x.fecha(tag);
    Ok(())
}

/// Campos do endereco na ordem do XSD. Emitente e destinatario levam o pais (1058, BRASIL) mesmo sem ele no pedido;
/// retirada e entrega so' o levam se informado.
fn campos_endereco(
    x: &mut X,
    tag: &str,
    e: &Endereco,
    ra: bool,
    cep_obrig: bool,
    pais_padrao: bool,
) -> Res<()> {
    x.t(
        "xLgr",
        &txt(&e.logradouro, &format!("{tag}/xLgr"), 60, 2, ra)?,
    );
    x.t("nro", &txt(&e.numero, &format!("{tag}/nro"), 60, 1, ra)?);
    x.o(
        "xCpl",
        &txt_ou_none(&e.complemento, &format!("{tag}/xCpl"), 60, 1, ra)?,
    );
    x.t(
        "xBairro",
        &txt(&e.bairro, &format!("{tag}/xBairro"), 60, 2, ra)?,
    );
    x.t(
        "cMun",
        &digitos(&e.cod_municipio, &format!("{tag}/cMun"), &[7])?,
    );
    x.t(
        "xMun",
        &txt(&e.municipio, &format!("{tag}/xMun"), 60, 2, ra)?,
    );
    let uf = e.uf.trim().to_uppercase();
    if uf_codigo(&uf).is_none() && uf != "EX" {
        return Err(erro(
            format!("{tag}/UF invalida: {:?}", e.uf),
            &format!("{tag}/UF"),
        ));
    }
    x.t("UF", &uf);
    match &e.cep {
        None if cep_obrig => {
            return Err(erro(
                format!("{tag}/CEP obrigatorio"),
                &format!("{tag}/CEP"),
            ))
        }
        None => {}
        Some(c) => x.t("CEP", &digitos(c, &format!("{tag}/CEP"), &[8])?),
    }
    let (mut cod_pais, mut pais) = (
        e.cod_pais.clone().filter(|s| !s.is_empty()),
        e.pais.clone().filter(|s| !s.is_empty()),
    );
    if uf != "EX" {
        if pais_padrao {
            cod_pais = cod_pais.or_else(|| Some("1058".into()));
            pais = pais.or_else(|| Some("BRASIL".into()));
        }
    } else if cod_pais.is_none() || pais.is_none() {
        return Err(erro(
            format!("{tag}: UF EX (exterior) exige cPais e xPais"),
            &format!("{tag}/cPais"),
        ));
    }
    if let Some(c) = &cod_pais {
        x.t("cPais", &digitos(c, &format!("{tag}/cPais"), &[4])?);
    }
    x.o(
        "xPais",
        &txt_ou_none(&pais, &format!("{tag}/xPais"), 60, 1, ra)?,
    );
    if let Some(f) = e.fone.as_ref().filter(|s| !s.is_empty()) {
        x.t(
            "fone",
            &digitos(f, &format!("{tag}/fone"), &(6..15).collect::<Vec<_>>())?,
        );
    }
    Ok(())
}

/// C22-10 (NT 2025.002): municipios da Zona Franca de Manaus e das Areas de Livre Comercio.
pub(crate) const MUNICIPIOS_SUFRAMA: [&str; 12] = [
    "1302603", "1303569", "1301902", "1304062", "1600303", "1600600", "1100106", "1400100",
    "1400159", "1200104", "1200252", "1200203",
];

/// Inscricao na Suframa: no minimo 9 digitos, o ultimo e' o DV (modulo 11, pesos 2 a 9, dos 8 primeiros).
pub(crate) fn suframa_valida(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() < 9 || !b.iter().all(u8::is_ascii_digit) {
        return false;
    }
    let mut soma = 0u32;
    for (i, c) in b[..8].iter().rev().enumerate() {
        soma += u32::from(c - b'0') * (2 + (i as u32 % 8));
    }
    let r = soma % 11;
    let dv = if r < 2 { 0 } else { 11 - r };
    u32::from(b[8] - b'0') == dv
}

pub(crate) fn emit(x: &mut X, n: &Nota) -> Res<()> {
    let e = &n.emit;
    let ra = n.remover_acento;
    x.abre("emit");
    let (tipo, doc) = documento(&e.documento, "emit/CNPJ")?;
    x.t(tipo, &doc);
    x.t("xNome", &txt(&e.nome, "emit/xNome", 60, 2, ra)?);
    x.o("xFant", &txt_ou_none(&e.fantasia, "emit/xFant", 60, 1, ra)?);
    endereco(x, "enderEmit", &e.endereco, ra, true)?;
    if let Some(ie) = &e.ie {
        let ie = ie.trim().to_uppercase();
        let v = if ie == "ISENTO" {
            ie
        } else {
            digitos(&ie, "emit/IE", &tam_ie())?
        };
        x.t("IE", &v);
    }
    if let Some(s) = e.iest.as_ref().filter(|s| !s.is_empty()) {
        x.t("IEST", &digitos(s, "emit/IEST", &tam_ie())?);
    }
    if let Some(im) = e.im.as_ref().filter(|s| !s.is_empty()) {
        x.t("IM", &txt(im, "emit/IM", 15, 1, true)?);
        if let Some(c) = e.cnae.as_ref().filter(|s| !s.is_empty()) {
            x.t("CNAE", &digitos(c, "emit/CNAE", &[7])?);
        }
    }
    if !(1..=4).contains(&e.crt) {
        return Err(erro(format!("CRT invalido: {}", e.crt), "CRT"));
    }
    x.t("CRT", &e.crt.to_string());
    if let Some(tp) = e.tp_regime_ibscbs {
        // PL_010h: logo depois do CRT
        x.t("tpRegimeIBSCBS", &tp.to_string());
    }
    if let Some(s) = e.isuf.as_ref().filter(|s| !s.trim().is_empty()) {
        // PL_010h: ultimo filho do emit
        let s = digitos(s, "ISUFEmit", &[8, 9])?;
        let cmun = digitos(&e.endereco.cod_municipio, "enderEmit/cMun", &[7])?;
        if !MUNICIPIOS_SUFRAMA.contains(&cmun.as_str()) {
            return Err(erro_r(
                format!(
                    "ISUFEmit com emitente fora da ZFM/ALC (cMun {}) -- C22-10",
                    e.endereco.cod_municipio
                ),
                "ISUFEmit",
                "1185",
            ));
        }
        if !suframa_valida(&s) {
            return Err(erro_r(
                format!("ISUFEmit {s} com digito verificador invalido -- C22-20"),
                "ISUFEmit",
                "1186",
            ));
        }
        x.t("ISUFEmit", &s);
    }
    x.fecha("emit");
    Ok(())
}

/// Qual `gTribSN` o regime do emitente leva no item.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Sn {
    /// Nenhum.
    Nao,
    /// So' o `gCBSSN`.
    Cbs,
    /// `gIBSSN` e `gCBSSN`.
    IbsCbs,
}

/// O que o regime do emitente permite no item, pela tabela da secao 2.3 da NT 2026.005 (colunas `cAtivSN`, `gIBSCBS`
/// e `gTribSN`, por CRT e `tpRegimeIBSCBS`).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Regime {
    /// O item informa o `cAtivSN`.
    pub(crate) ativ_sn: bool,
    /// O `gTribSN` do item.
    pub(crate) trib_sn: Sn,
    /// O item leva o `gIBSCBS` (grupo de valores) quando o CST o exige. O Simples sem regime regular (CRT 1 e
    /// `tpRegimeIBSCBS` ausente) nao leva: a tabela diz "nao informar".
    pub(crate) g_ibscbs: bool,
    /// O `gCBS` do item vai com a aliquota zerada (CRT 2 sem regime regular: a CBS da operacao e' a do `gCBSSN`).
    pub(crate) cbs_zerada: bool,
}

impl Regime {
    const fn novo(ativ_sn: bool, trib_sn: Sn) -> Regime {
        Regime {
            ativ_sn,
            trib_sn,
            g_ibscbs: true,
            cbs_zerada: false,
        }
    }
}

/// Resolve o regime do emitente. Combinacao fora da tabela (por exemplo CRT 1 com `tpRegimeIBSCBS` 2) e' erro: a NT
/// manda nao informar a tag nas situacoes diversas das dela.
pub(crate) fn regime_sn(n: &Nota) -> Res<Regime> {
    let tp = n.emit.tp_regime_ibscbs;
    if let Some(t) = tp {
        if !(1..=4).contains(&t) {
            return Err(erro(
                format!("tpRegimeIBSCBS invalido: {t} (1 a 4)"),
                "tpRegimeIBSCBS",
            ));
        }
    }
    Ok(match (n.emit.crt, tp) {
        (1, None) => Regime {
            g_ibscbs: false,
            ..Regime::novo(true, Sn::IbsCbs)
        },
        (2, None) => Regime {
            cbs_zerada: true,
            ..Regime::novo(true, Sn::Cbs)
        },
        (1, Some(1)) | (2, Some(1)) => Regime::novo(true, Sn::Nao),
        (3, Some(2)) => Regime::novo(true, Sn::IbsCbs),
        (3, Some(3)) => Regime::novo(true, Sn::Cbs),
        (3, Some(4)) => Regime::novo(true, Sn::Nao),
        (3, None) | (4, None) => Regime::novo(false, Sn::Nao),
        (crt, t) => {
            return Err(erro(
                format!(
                    "tpRegimeIBSCBS {} nao combina com CRT {crt}: 1 = Simples (CRT 1 ou 2) no regime regular do \
                     IBS/CBS; 2, 3 e 4 = regime normal (CRT 3) com a opcao pelo Simples pendente (NT 2026.005, \
                     secao 2.1: a tag nao deve ser informada nas situacoes diversas)",
                    t.map_or("ausente".into(), |v| v.to_string())
                ),
                "tpRegimeIBSCBS",
            ))
        }
    })
}

/// PL_010h: antes da vigencia o schema da SEFAZ nao conhece as tags novas (rejeicao 225). A data e' a do `dhEmi` no
/// fuso da UF.
pub(crate) fn vigencia_pl010h(n: &Nota, dia: NaiveDate) -> Res<()> {
    let usa = if n.emit.tp_regime_ibscbs.is_some() {
        Some("tpRegimeIBSCBS")
    } else if n.itens.iter().any(|i| i.c_ativ_sn.is_some()) {
        Some("cAtivSN")
    } else if n
        .itens
        .iter()
        .any(|i| i.valor_unitario_liquido.is_some() || i.valor_produto_liquido.is_some())
    {
        Some("vUnComLiq")
    } else if n
        .itens
        .iter()
        .any(|i| i.ibscbs.as_ref().is_some_and(|b| b.tem_trib_sn))
    {
        Some("gTribSN")
    } else {
        None
    };
    let Some(usa) = usa else { return Ok(()) };
    let inicio = vigencia("pl_010h", n.tp_amb);
    let amb = if n.tp_amb == 1 {
        "producao"
    } else {
        "homologacao"
    };
    if dia < inicio {
        return Err(erro_r(
            format!(
                "{usa} so' existe no schema da SEFAZ a partir de {} ({amb}; NT 2026.005 e NT 2026.008, pacote \
                 PL_010h): antes disso e' a rejeicao 225 (falha de schema)",
                inicio.format("%d/%m/%Y")
            ),
            usa,
            "225",
        ));
    }
    Ok(())
}

/// O item conferido contra o regime do emitente (NT 2026.005): `cAtivSN`, MEI sem IBSCBS, o `gIBSCBS` onde a tabela
/// manda nao informar, o `gCBS` zerado e o `gTribSN` que o regime leva.
pub(crate) fn conferir_sn_item(n: &Nota, it: &Item, k: &str, regime: Regime) -> Res<()> {
    if let Some(c) = &it.c_ativ_sn {
        if !regime.ativ_sn {
            return Err(erro(
                format!(
                    "{k}cAtivSN so' para emitente do Simples (CRT 1 ou 2) ou com a opcao pendente (CRT 3 com \
                     tpRegimeIBSCBS 2, 3 ou 4) -- NT 2026.005 I17d"
                ),
                "cAtivSN",
            ));
        }
        if !matches!(c.trim(), "01" | "02" | "03" | "04" | "90") {
            return Err(erro(
                format!("{k}cAtivSN invalido: \"{c}\" (01, 02, 03, 04 ou 90)"),
                "cAtivSN",
            ));
        }
    }
    let Some(ib) = &it.ibscbs else { return Ok(()) };
    if n.emit.crt == 4 {
        return Err(erro(
            format!("{k}MEI (CRT 4) nao informa o grupo IBSCBS -- NT 2026.005 secao 2.3"),
            "IBSCBS",
        ));
    }
    if ib.tem_trib_sn {
        match regime.trib_sn {
            Sn::Nao => {
                return Err(erro(
                    format!(
                        "{k}gTribSN nao vai com CRT {} e tpRegimeIBSCBS {} (NT 2026.005 secao 2.3: so' no Simples sem regime regular do IBS/CBS ou no regime normal com tpRegimeIBSCBS 2 ou 3)",
                        n.emit.crt,
                        n.emit.tp_regime_ibscbs.map_or("ausente".into(), |v| v.to_string())
                    ),
                    "gTribSN",
                ))
            }
            Sn::Cbs if ib.tem_ibs_sn => {
                return Err(erro(
                    format!(
                        "{k}neste regime (CRT 2 sem regime regular, ou tpRegimeIBSCBS 3) o gTribSN leva so' o gCBSSN: tire o gIBSSN -- NT 2026.005 secao 2.3"
                    ),
                    "gTribSN/gIBSSN",
                ))
            }
            Sn::IbsCbs if !ib.tem_ibs_sn => {
                return Err(erro(
                    format!("{k}neste regime o gTribSN leva gIBSSN e gCBSSN -- NT 2026.005 secao 2.3"),
                    "gTribSN/gIBSSN",
                ))
            }
            _ => {}
        }
    }
    let com_valores = ib.grupo == Some("gIBSCBS");
    if com_valores && !regime.g_ibscbs {
        return Err(erro(
            format!(
                "{k}o Simples sem regime regular do IBS/CBS (CRT 1 sem tpRegimeIBSCBS) nao informa o gIBSCBS: o item leva so' o gTribSN -- NT 2026.005 secao 2.3, tabela \"Como Preencher\""
            ),
            "gIBSCBS",
        ));
    }
    if com_valores && regime.cbs_zerada && ib.cbs.p != 0 {
        return Err(erro(
            format!(
                "{k}CRT 2 sem regime regular do IBS/CBS: o gCBS vai com a aliquota zerada (pCBS 0) e a CBS do Simples vai no gCBSSN -- NT 2026.005 secao 2.3"
            ),
            "gIBSCBS/gCBS",
        ));
    }
    if !ib.tem_trib_sn && !regime.g_ibscbs && ib.grupo != Some("gIBSCBSMono") {
        return Err(erro(
            format!(
                "{k}o Simples sem regime regular do IBS/CBS informa o gTribSN (gIBSSN e gCBSSN) no item -- NT 2026.005 secao 2.3"
            ),
            "gTribSN",
        ));
    }
    Ok(())
}

pub(crate) fn dest(x: &mut X, n: &Nota) -> Res<()> {
    let ra = n.remover_acento;
    let Some(d) = &n.dest else {
        // a NFC-e e' emitida sem destinatario (o XSD o deixa opcional); a NF-e exige
        if n.modelo == 65 {
            return Ok(());
        }
        return Err(erro("NF-e (55) exige destinatario", "dest"));
    };
    x.abre("dest");
    if let Some(ie) = &d.id_estrangeiro {
        let v = if ie.is_empty() {
            String::new()
        } else {
            txt(ie, "idEstrangeiro", 20, 0, true)?
        };
        x.t("idEstrangeiro", &v);
    } else if let Some(doc) = d.documento.as_ref().filter(|s| !s.is_empty()) {
        let (tipo, doc) = documento(doc, "dest/CNPJ")?;
        x.t(tipo, &doc);
    } else {
        return Err(erro(
            "destinatario sem CPF/CNPJ/idEstrangeiro (o XSD exige um deles)",
            "dest/CNPJ",
        ));
    }
    let nome = if n.tp_amb == 2 {
        super::HOM_NOME_DEST.to_string()
    } else {
        d.nome.clone().unwrap_or_default()
    };
    // na NFC-e o nome e o endereco do consumidor sao opcionais (XSD)
    if n.modelo != 65 || !nome.trim().is_empty() {
        x.t("xNome", &txt(&nome, "dest/xNome", 60, 2, ra)?);
    }
    match &d.endereco {
        Some(e) => endereco(x, "enderDest", e, ra, false)?,
        None if n.modelo == 65 => {}
        None => return Err(erro("NF-e (55): enderDest obrigatorio", "enderDest")),
    }
    if ![1, 2, 9].contains(&d.ind_ie_dest) {
        return Err(erro(
            format!("indIEDest invalido: {}", d.ind_ie_dest),
            "indIEDest",
        ));
    }
    x.t("indIEDest", &d.ind_ie_dest.to_string());
    if let Some(ie) = d.ie.as_ref().filter(|s| !s.is_empty()) {
        x.t("IE", &digitos(ie, "dest/IE", &tam_ie())?);
    }
    if let Some(s) = d.isuf.as_ref().filter(|s| !s.is_empty()) {
        x.t("ISUF", &digitos(s, "dest/ISUF", &[8, 9])?);
    }
    if let Some(s) = d.im.as_ref().filter(|s| !s.is_empty()) {
        x.t("IM", &txt(s, "dest/IM", 15, 1, true)?);
    }
    x.o("email", &txt_ou_none(&d.email, "dest/email", 60, 1, false)?);
    x.fecha("dest");
    Ok(())
}
