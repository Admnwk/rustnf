// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! ICMS monofasico sobre combustiveis: os grupos `ICMS02`, `ICMS15`, `ICMS53` e `ICMS61` do item (CST 02, 15, 53 e
//! 61) e os totais `qBCMono`, `vICMSMono`, `qBCMonoReten`, `vICMSMonoReten`, `qBCMonoRet` e `vICMSMonoRet` do
//! `ICMSTot`. Fonte: NT 2023.001 v1.60 (grupos N02a, N03a, N07a, N08a e W, regras N12-20, N12-70, N12-100, N37a-10,
//! N39-10, N39a-10, N41-10, N41-20, N43a-10, W06b.1-10 a W06e-10 e W16-10) e o XSD PL_010h v1.03 (ordem, ocorrencia e
//! tipo de cada campo). O MOC 7.00 (novembro de 2020) e' anterior a esta tributacao e nao a traz.
//!
//! As regras N38-10, N40-10 e N44-10 (rejeicoes 961, 963 e 965) e N45-10 (966) estao revogadas na NT (tachadas no
//! PDF) e nao sao conferidas; `qBCMonoDif` e `adRemICMSDif` do CST 53, tambem revogados, nao saem. A comparacao do
//! produto (`cProdANP`) com a Tabela de Combustiveis Sujeitos a Tributacao Monofasica (rejeicoes 959 e 960) depende
//! de tabela publicada a parte: aqui se confere so' a presenca do grupo `comb`.

use std::collections::BTreeMap;

use super::formato::*;
use super::modelo::*;
use super::total::Ac;
use super::X;

/// CST do ICMS monofasico.
pub(crate) const CST_MONO: [&str; 4] = ["02", "15", "53", "61"];

/// Codigos ANP do biodiesel e do diesel B de que trata a regra N41-20: a retencao e' multiplicada por 0,3333.
const ANP_BIO_3333: [&str; 12] = [
    "420102004",
    "420102005",
    "420105001",
    "820101003",
    "820101011",
    "820101012",
    "820101013",
    "820101026",
    "820101027",
    "820101032",
    "820101033",
    "820101034",
];

/// `motRedAdRem` (N48): 1 transporte coletivo de passageiros, 9 outros.
const MOT_RED_AD_REM: [&str; 2] = ["1", "9"];

/// Totais do `ICMSTot` (W06b.1 a W06e), na ordem do XSD.
pub(crate) const TOT_MONO: [&str; 6] = [
    "qBCMono",
    "vICMSMono",
    "qBCMonoReten",
    "vICMSMonoReten",
    "qBCMonoRet",
    "vICMSMonoRet",
];

/// O que a regra de cada item precisa saber da nota e do item.
pub(crate) struct Contexto<'a> {
    pub(crate) cod: &'a str,
    pub(crate) item: &'a Item,
    /// `finNFe` da nota.
    pub(crate) fin: i64,
    /// Saida a nao contribuinte (`indIEDest` 9) a que a regra N12-70 se aplica.
    pub(crate) nao_contrib: bool,
    /// `idDest` da nota.
    pub(crate) id_dest: i64,
    /// Prefixo das mensagens (`item N: `).
    pub(crate) k: &'a str,
}

/// O grupo e' monofasico?
pub(crate) fn eh_grupo(grupo: &str) -> bool {
    matches!(grupo, "ICMS02" | "ICMS15" | "ICMS53" | "ICMS61")
}

/// Campos do pedido com a tag do XML, na ordem do leiaute.
fn campos(c: &Icms) -> [(&'static str, &Option<String>); 13] {
    [
        ("qBCMono", &c.q_bc_mono),
        ("adRemICMS", &c.ad_rem_icms),
        ("vICMSMono", &c.v_icms_mono),
        ("qBCMonoReten", &c.q_bc_mono_reten),
        ("adRemICMSReten", &c.ad_rem_icms_reten),
        ("vICMSMonoReten", &c.v_icms_mono_reten),
        ("pRedAdRem", &c.p_red_ad_rem),
        ("motRedAdRem", &c.mot_red_ad_rem),
        ("vICMSMonoOp", &c.v_icms_mono_op),
        ("vICMSMonoDif", &c.v_icms_mono_dif),
        ("qBCMonoRet", &c.q_bc_mono_ret),
        ("adRemICMSRet", &c.ad_rem_icms_ret),
        ("vICMSMonoRet", &c.v_icms_mono_ret),
    ]
}

/// Tags que cada grupo leva (alem de `orig`, `CST` e, no 53, `pDif`).
fn do_grupo(grupo: &str) -> &'static [&'static str] {
    match grupo {
        "ICMS02" => &["qBCMono", "adRemICMS", "vICMSMono"],
        "ICMS15" => &[
            "qBCMono",
            "adRemICMS",
            "vICMSMono",
            "qBCMonoReten",
            "adRemICMSReten",
            "vICMSMonoReten",
            "pRedAdRem",
            "motRedAdRem",
        ],
        "ICMS53" => &[
            "qBCMono",
            "adRemICMS",
            "vICMSMonoOp",
            "vICMSMonoDif",
            "vICMSMono",
        ],
        "ICMS61" => &["qBCMonoRet", "adRemICMSRet", "vICMSMonoRet"],
        _ => &[],
    }
}

/// Casas decimais do tipo do XSD: quantidade e aliquota ad rem 4 (`TDec_1104v`, `TDec_0302a04`), percentual 2
/// (`TDec_0302Max100`) ou 4 (`TDec_0302a04Max100`, o `pDif`), valor 2 (`TDec_1302`).
fn casas(tag: &str) -> u32 {
    if tag.starts_with("qBC") || tag.starts_with("adRem") || tag == "pDif" {
        4
    } else {
        2
    }
}

/// Digitos antes da virgula que o tipo do XSD comporta: 11 na quantidade, 3 na aliquota e no percentual, 13 no valor.
fn inteiros(tag: &str) -> usize {
    if tag.starts_with("qBC") {
        11
    } else if tag.starts_with("adRem") || tag.starts_with('p') {
        3
    } else {
        13
    }
}

/// Campos do ICMS por valor que o monofasico nao leva (tag, valor).
fn classicos(c: &Icms) -> [(&'static str, &Option<String>); 18] {
    [
        ("vBC", &c.vbc),
        ("pRedBC", &c.p_red_bc),
        ("pICMS", &c.p_icms),
        ("vICMS", &c.v_icms),
        ("vBCFCP", &c.vbc_fcp),
        ("pFCP", &c.p_fcp),
        ("vFCP", &c.v_fcp),
        ("vBCSTRet", &c.vbc_st_ret),
        ("pST", &c.p_st),
        ("vICMSSubstituto", &c.v_icms_substituto),
        ("vICMSSTRet", &c.v_icms_st_ret),
        ("pCredSN", &c.p_cred_sn),
        ("vCredICMSSN", &c.v_cred_icms_sn),
        ("vICMSOp", &c.v_icms_op),
        ("vICMSDif", &c.v_icms_dif),
        ("pFCPDif", &c.p_fcp_dif),
        ("vFCPDif", &c.v_fcp_dif),
        ("vFCPEfet", &c.v_fcp_efet),
    ]
}

/// Recusa o campo do monofasico em grupo que nao o leva (inclusive o de outro grupo monofasico).
pub(crate) fn conferir_fora(c: &Icms, grupo: &str, tipo: &str, cod: &str, k: &str) -> Res<()> {
    let usados = do_grupo(grupo);
    for (tag, v) in campos(c) {
        if !usados.contains(&tag) && v.is_some() {
            return Err(erro(
                format!(
                    "{k}{tipo} {cod} nao leva {tag} (monofasico: CST 02, 15, 53 e 61, cada um com os seus campos)"
                ),
                tag,
            ));
        }
    }
    Ok(())
}

fn obrigatorio(v: &BTreeMap<&'static str, Dec>, tag: &'static str, cx: &Contexto) -> Res<()> {
    if v.contains_key(tag) {
        return Ok(());
    }
    let (cod, k) = (cx.cod, cx.k);
    let r = match tag {
        "qBCMono" => Some(("767", "N37a-10")),
        "qBCMonoReten" => Some(("768", "N39a-10")),
        "qBCMonoRet" => Some(("769", "N43a-10")),
        _ => None,
    };
    match r {
        Some((rej, regra)) => Err(erro_r(
            format!("{k}CST {cod} sem {tag} -- {regra}"),
            tag,
            rej,
        )),
        None => Err(erro(
            format!("{k}CST {cod} sem {tag} (obrigatorio no XSD)"),
            tag,
        )),
    }
}

/// Confere o valor com a quantidade x aliquota (arredondado a 2 casas, tolerancia de 0,01: nota (*4) da NT) quando
/// a nota e' normal (`finNFe` 1).
fn conta_confere(valor: Dec, q: Dec, ad_rem: Dec, fator: Option<Dec>, normal: bool) -> (bool, Dec) {
    let mut conta = q.vezes(ad_rem);
    if let Some(f) = fator {
        conta = conta.vezes(f);
    }
    let conta = conta.arred(2);
    (
        normal && valor.menos(conta).abs().maior(Dec::centavos(1)),
        conta,
    )
}

fn conferir_15(
    c: &Icms,
    v: &mut BTreeMap<&'static str, Dec>,
    cx: &Contexto,
    normal: bool,
) -> Res<()> {
    let k = cx.k;
    obrigatorio(v, "qBCMonoReten", cx)?;
    obrigatorio(v, "adRemICMSReten", cx)?;
    obrigatorio(v, "vICMSMonoReten", cx)?;
    let bio = cx
        .item
        .combustivel
        .as_ref()
        .is_some_and(|cb| ANP_BIO_3333.contains(&cb.c_prod_anp.trim()));
    let fator = bio.then_some(Dec { m: 3333, e: 4 });
    let vr = v["vICMSMonoReten"];
    let (errado, conta) = conta_confere(vr, v["qBCMonoReten"], v["adRemICMSReten"], fator, normal);
    if errado {
        return Err(erro_r(
            format!(
                "{k}vICMSMonoReten {} difere de qBCMonoReten x adRemICMSReten{} ({}) -- {}",
                vr.fixo(2),
                if bio { " x 0,3333" } else { "" },
                conta.fixo(2),
                if bio { "N41-20" } else { "N41-10" }
            ),
            "vICMSMonoReten",
            "964",
        ));
    }
    if c.p_red_ad_rem.is_none() != c.mot_red_ad_rem.is_none() {
        let falta = if c.mot_red_ad_rem.is_none() {
            "motRedAdRem"
        } else {
            "pRedAdRem"
        };
        return Err(erro(
            format!(
                "{k}pRedAdRem e motRedAdRem vao juntos (XSD ICMS15, sequencia N46): falta {falta}"
            ),
            falta,
        ));
    }
    if let (Some(p), Some(m)) = (v.get("pRedAdRem").copied(), c.mot_red_ad_rem.as_deref()) {
        if p.maior(Dec::de_int(100)) {
            return Err(erro(
                format!(
                    "{k}pRedAdRem {} acima de 100 (XSD TDec_0302Max100)",
                    p.fixo(p.e)
                ),
                "pRedAdRem",
            ));
        }
        if !MOT_RED_AD_REM.contains(&m.trim()) {
            return Err(erro(
                format!(
                    "{k}motRedAdRem '{}' deve ser 1 (transporte coletivo de passageiros) ou 9 (outros)",
                    m.trim()
                ),
                "motRedAdRem",
            ));
        }
    }
    Ok(())
}

/// Confere o grupo monofasico ANTES de sair e devolve `{tag: valor}` so' dos campos informados.
pub(crate) fn conferir(c: &Icms, grupo: &str, cx: &Contexto) -> Res<BTreeMap<&'static str, Dec>> {
    let (cod, k) = (cx.cod, cx.k);
    if cx.item.combustivel.is_none() {
        return Err(erro_r(
            format!(
                "{k}CST {cod} (monofasico sobre combustiveis) sem o grupo comb: sem cProdANP o produto nao esta na \
                 Tabela de Combustiveis Sujeitos a Tributacao Monofasica -- N12-100"
            ),
            "CST",
            "959",
        ));
    }
    if cx.nao_contrib
        && (grupo == "ICMS15" || grupo == "ICMS53" || (grupo == "ICMS02" && cx.id_dest != 1))
    {
        return Err(erro_r(
            format!(
                "{k}CST {cod} a nao contribuinte (indIEDest 9): a N12-70 aceita o 61 (e o 02 so' na operacao \
                 interna, exc. 8, a criterio da UF)"
            ),
            "CST",
            "508",
        ));
    }
    for (tag, v) in classicos(c) {
        if v.is_some() {
            return Err(erro(
                format!(
                    "{k}CST {cod} (monofasico) nao leva {tag}: o imposto vai por quantidade x aliquota ad rem"
                ),
                tag,
            ));
        }
    }
    let usados = do_grupo(grupo);
    let mut v: BTreeMap<&'static str, Dec> = BTreeMap::new();
    for (tag, valor) in campos(c) {
        if tag == "motRedAdRem" || !usados.contains(&tag) {
            continue;
        }
        if let Some(s) = valor {
            let d = Dec::parse(s, tag)?;
            num_lim(d, inteiros(tag), casas(tag), tag)?;
            v.insert(tag, d);
        }
    }
    let normal = cx.fin == 1;
    if grupo != "ICMS61" {
        obrigatorio(&v, "qBCMono", cx)?;
    }
    if grupo == "ICMS02" || grupo == "ICMS15" {
        obrigatorio(&v, "adRemICMS", cx)?;
        obrigatorio(&v, "vICMSMono", cx)?;
        let vm = v["vICMSMono"];
        let (errado, conta) = conta_confere(vm, v["qBCMono"], v["adRemICMS"], None, normal);
        if errado {
            return Err(erro_r(
                format!(
                    "{k}vICMSMono {} difere de qBCMono x adRemICMS ({}) -- N39-10",
                    vm.fixo(2),
                    conta.fixo(2)
                ),
                "vICMSMono",
                "962",
            ));
        }
    }
    match grupo {
        "ICMS15" => conferir_15(c, &mut v, cx, normal)?,
        "ICMS53" => {
            if let Some(s) = &c.p_dif {
                let d = Dec::parse(s, "pDif")?;
                num_lim(d, inteiros("pDif"), casas("pDif"), "pDif")?;
                if d.maior(Dec::de_int(100)) {
                    return Err(erro(
                        format!("{k}pDif acima de 100 (XSD TDec_0302a04Max100)"),
                        "pDif",
                    ));
                }
                v.insert("pDif", d);
            }
        }
        "ICMS61" => {
            obrigatorio(&v, "qBCMonoRet", cx)?;
            obrigatorio(&v, "adRemICMSRet", cx)?;
            obrigatorio(&v, "vICMSMonoRet", cx)?;
        }
        _ => {}
    }
    Ok(v)
}

fn escreve_tag(x: &mut X, v: &BTreeMap<&'static str, Dec>, tag: &'static str) -> Res<()> {
    let d = v.get(tag).copied().unwrap_or(Dec::ZERO);
    x.t(tag, &num_lim(d, inteiros(tag), casas(tag), tag)?);
    Ok(())
}

/// Escreve o grupo na ordem do XSD e soma os valores no acumulador dos totais. No 02, 15 e 61 tudo sai; no 53 so'
/// o `qBCMono` sai sempre e o resto so' com valor (ocorrencia 0-1).
pub(crate) fn escreve(
    x: &mut X,
    ac: &mut Ac,
    grupo: &str,
    c: &Icms,
    v: &BTreeMap<&'static str, Dec>,
) -> Res<()> {
    let val = |t: &str| v.get(t).copied().unwrap_or(Dec::ZERO);
    if grupo == "ICMS61" {
        for tag in ["qBCMonoRet", "adRemICMSRet", "vICMSMonoRet"] {
            escreve_tag(x, v, tag)?;
        }
        ac.add("qBCMonoRet", val("qBCMonoRet"));
        ac.add("vICMSMonoRet", val("vICMSMonoRet"));
        return Ok(());
    }
    escreve_tag(x, v, "qBCMono")?;
    if grupo == "ICMS53" {
        for tag in [
            "adRemICMS",
            "vICMSMonoOp",
            "pDif",
            "vICMSMonoDif",
            "vICMSMono",
        ] {
            if !val(tag).eh_zero() {
                escreve_tag(x, v, tag)?;
            }
        }
    } else {
        escreve_tag(x, v, "adRemICMS")?;
        escreve_tag(x, v, "vICMSMono")?;
    }
    ac.add("qBCMono", val("qBCMono"));
    ac.add("vICMSMono", val("vICMSMono"));
    if grupo != "ICMS15" {
        return Ok(());
    }
    for tag in ["qBCMonoReten", "adRemICMSReten", "vICMSMonoReten"] {
        escreve_tag(x, v, tag)?;
    }
    ac.add("qBCMonoReten", val("qBCMonoReten"));
    ac.add("vICMSMonoReten", val("vICMSMonoReten"));
    if v.contains_key("pRedAdRem") {
        escreve_tag(x, v, "pRedAdRem")?;
        x.t(
            "motRedAdRem",
            c.mot_red_ad_rem.as_deref().unwrap_or_default().trim(),
        );
    }
    Ok(())
}
