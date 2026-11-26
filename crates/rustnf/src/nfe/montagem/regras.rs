// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Regras de negocio conferidas ANTES de gastar o numero: as rejeicoes do MOC 7.00 e das Notas Tecnicas que a SEFAZ
//! devolveria, na forma `<cStat>-Rejeicao: <texto>`. O texto e' o do MOC; cada regra leva o `cStat` na frente.

use std::collections::HashSet;

use chrono::{Datelike, NaiveDate};

use super::formato::*;
use super::gtin;
use super::ide::tipos;
use super::modelo::*;
use super::*;

/// Tolerancia de 0,01 (em centavos) que o MOC admite nas contas da nota.
pub(crate) const TOL: i128 = 1;

pub(crate) fn c(v: Dec) -> i128 {
    v.arred(2).m
}

/// 806: CST 60 e CSOSN 500 sem CEST. Fica aqui (lista) e nao no `montar` para nao quebrar quem monta a nota e so'
/// quer saber o que a SEFAZ diria.
fn icms_item(n: &Nota) -> Vec<String> {
    let mut out = Vec::new();
    for (i, it) in n.itens.iter().enumerate() {
        let Some(c) = &it.icms else { continue };
        let (cod, ni) = (c.codigo.trim(), i + 1);
        let sem_cest = it.cest.as_deref().is_none_or(|s| s.trim().is_empty());
        if (cod == "60" || cod == "500") && sem_cest {
            out.push(format!(
                "806-Rejeicao: Operacao com ICMS-ST sem informacao do CEST [nItem: {ni}]"
            ));
        }
    }
    out
}

/// As rejeicoes do MOC e das NT que a nota levaria, na forma `"<cStat>-Rejeicao: <texto>"`, sem enviar nada.
/// Defeito de formato do pedido (o que impede ate montar) vem como `"entrada: <mensagem>"` e encerra a lista.
#[must_use]
pub fn conferir(n: &Nota) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    out.extend(grupo_b25(n));
    out.extend(grupo_vc(n));
    out.extend(modelo_referenciado(n));
    out.extend(gtin(n));
    out.extend(icms_item(n));
    let varejo = super::nfce::regras(n);
    if !varejo.is_empty() {
        out.extend(super::nfce::como_lista(&varejo));
        return out;
    }
    let vnf = match super::montar_interno(n, false) {
        Ok(m) => {
            out.extend(super::nfce::como_lista(&super::nfce::regras_do_total(
                n,
                &m.totais["vNF"],
            )));
            out.extend(grupo_bb(n, &m.chave));
            Dec::parse(&m.totais["vNF"], "vNF").map(c).unwrap_or(0)
        }
        Err(e) => {
            out.push(format!("entrada: {e}"));
            return out;
        }
    };
    let mut tot: i128 = 0;
    for pg in &n.pagamentos {
        tot += Dec::parse(&pg.valor, "vPag").map(c).unwrap_or(0);
    }
    let troco = dec(&n.troco, "vTroco").map(c).unwrap_or(0);
    let tpag = |pg: &Pagamento| format!("{:0>2}", pg.t_pag.trim());
    if n.fin_nfe == 1 || n.fin_nfe == 2 {
        if troco.abs() < TOL && tot - vnf > TOL {
            out.push("866-Rejeicao: Ausencia de troco quando o valor dos pagamentos informados for maior que o total da nota".into());
        }
        if troco > TOL && (vnf - (tot - troco)).abs() > TOL {
            out.push("869-Rejeicao: Valor do troco incorreto".into());
        }
    } else if n.fin_nfe == 4 && n.pagamentos.iter().any(|pg| tpag(pg) != "90") {
        out.push(
            "871-Rejeicao: O campo Meio de Pagamento deve ser preenchido com a opcao Sem Pagamento"
                .into(),
        );
    }
    out
}

/// Dia da emissao no fuso da UF do emitente; None se a UF nao existe (o `montar` acusa).
fn dia_uf(n: &Nota) -> Option<NaiveDate> {
    let uf = n.emit.endereco.uf.trim().to_uppercase();
    uf_codigo(&uf)?;
    Some(na_uf(&n.emissao, &uf).date_naive())
}

fn v153(n: &Nota, dia: NaiveDate) -> bool {
    dia >= vigencia("nt2025_002.v1_53", n.tp_amb)
}

/// (emitente, tipo, uf, modelo) da chave referenciada, ou None se nao decompoe.
fn ref_(c: &str) -> Option<(String, String, String, String)> {
    let k = chave_dfe::decompor(c, false).ok()?;
    let tipo = if k.emitente_cpf { "cpf" } else { "cnpj" };
    Some((k.emitente, tipo.to_string(), k.uf.to_string(), k.modelo))
}

pub(crate) fn grupo_b25(n: &Nota) -> Vec<String> {
    let mut out = Vec::new();
    if n.modelo != 55 {
        return out;
    }
    let dia = dia_uf(n);
    let (Ok((_, cred)), Ok((_, emit))) = (tipos(n), documento(&n.emit.documento, "emit/CNPJ"))
    else {
        return out;
    };
    let cred = cred.as_deref();
    let fin = n.fin_nfe;
    let refs: Vec<_> = n.refs_nfe.iter().filter_map(|c| ref_(c)).collect();
    if (fin == 2 || matches!(cred, Some("01") | Some("03") | Some("04"))) && n.refs_nfe.is_empty() {
        out.push("254-Rejeicao: NF-e nao possui NF referenciada".into());
    }
    if n.refs_nfe.len() > 1
        && (fin == 2
            || matches!(cred, Some("01") | Some("03") | Some("04"))
            || (fin == 4 && dia.is_some_and(|d| v153(n, d))))
    {
        out.push("255-Rejeicao: NF-e possui mais de uma NF referenciada".into());
    }
    if fin == 2 || matches!(cred, Some("03") | Some("04")) {
        if refs.iter().any(|r| r.0 != emit) {
            out.push(
                "269-Rejeicao: CNPJ/CPF Emitente da NF-e difere do CNPJ/CPF da NF Referenciada"
                    .into(),
            );
        }
        let uf = n.emit.endereco.uf.trim().to_uppercase();
        if refs.iter().any(|r| r.2 != uf) {
            out.push("678-Rejeicao: NF referenciada com UF diferente desta NF-e".into());
        }
    }
    if cred == Some("02") && !n.refs_nfe.is_empty() {
        out.push("1027-Rejeicao: NF referenciada informada indevidamente".into());
    }
    if fin == 5
        && refs
            .iter()
            .any(|r| r.3 != "55" && !(cred == Some("03") && r.3 == "65"))
    {
        out.push("1003-Rejeicao: NF-e de credito faz referencia a documento fiscal diferente de NF-e modelo 55".into());
    }
    out
}

pub(crate) fn grupo_bb(n: &Nota, chave: &str) -> Vec<String> {
    let mut out = Vec::new();
    let Some(cg) = n.compra_gov.as_ref().filter(|_| n.modelo == 55) else {
        return out;
    };
    let ano = dia_uf(n).map_or(99, |d| d.year() % 100);
    let Ok((_, emit)) = documento(&n.emit.documento, "emit/CNPJ") else {
        return out;
    };
    let base_emit = if emit.len() == 14 {
        emit[..8].to_string()
    } else {
        emit.clone()
    };
    let msg = "-Rejeicao: DFe referenciado em operacao com ente governamental ";
    let mut vistas: HashSet<String> = HashSet::new();
    for (i, c) in cg.refs.iter().map(|x| chave_dfe::limpar(x)).enumerate() {
        let s = format!(" [nOcor: {}]", i + 1);
        let num = |a: usize, b: usize| c[a..b].parse::<i64>().unwrap_or(-1);
        if chave_dfe::dv(&c[..43]).ok() != c[43..].parse::<u32>().ok() {
            out.push(format!("1204{msg}com Digito Verificador invalido{s}"));
        }
        if uf_sigla(num(0, 2) as u32).is_none() {
            out.push(format!("1205{msg}com UF invalida{s}"));
        }
        if !(6..=i64::from(ano)).contains(&num(2, 4)) {
            out.push(format!("1206{msg}com Ano invalido{s}"));
        }
        if !(1..=12).contains(&num(4, 6)) {
            out.push(format!("1207{msg}com Mes invalido{s}"));
        }
        let (em, tipo) = match chave_dfe::decompor(&c, false) {
            Ok(k) => (
                k.emitente,
                if k.emitente_cpf { "cpf" } else { "cnpj" }.to_string(),
            ),
            Err(_) => (c[6..20].to_string(), "cnpj".to_string()),
        };
        let ok = if tipo == "cpf" {
            chave_dfe::cpf_valido(&em)
        } else {
            chave_dfe::cnpj_valido(&em)
        };
        if !ok {
            out.push(format!("1208{msg}com CNPJ/CPF invalido{s}"));
        }
        if &c[20..22] != "55" {
            out.push(format!("1209{msg}com Modelo invalido{s}"));
        }
        if num(25, 34) == 0 {
            out.push(format!("1210{msg}com Numero invalido{s}"));
        }
        if !vistas.insert(c.clone()) {
            out.push(format!("1211{msg}em duplicidade{s}"));
        }
        if c == chave {
            out.push(format!(
                "1212{msg}com a mesma Chave de Acesso da Nota Fiscal atual{s}"
            ));
        }
        if &c[2..6] > "1104" && !"12345679".contains(&c[34..35]) {
            out.push(format!("1213{msg}com tipo de emissao invalido{s}"));
        }
        let base = if tipo == "cnpj" {
            em[..8.min(em.len())].to_string()
        } else {
            em.clone()
        };
        if base != base_emit {
            out.push(format!(
                "1215{msg}deve possuir o mesmo CNPJ base ou CPF do emitente do documento{s}"
            ));
        }
    }
    out
}

fn limpo_gtin(v: Option<&str>) -> String {
    // a mesma limpeza do `formato::digitos` (espaco . / - ( ))
    v.unwrap_or("")
        .chars()
        .filter(|c| !c.is_whitespace() && !matches!(c, '.' | '/' | '-' | '(' | ')'))
        .collect()
}

fn gtin_dv_errado(g: &str) -> bool {
    g.bytes().all(|b| b.is_ascii_digit())
        && matches!(g.len(), 8 | 12 | 13 | 14)
        && gtin::digito_verificador(&g[..g.len() - 1]).ok()
            != Some(u32::from(g.as_bytes()[g.len() - 1] - b'0'))
}

pub(crate) fn gtin(n: &Nota) -> Vec<String> {
    let mut out = Vec::new();
    for (i, it) in n.itens.iter().enumerate() {
        let i = i + 1;
        let g = limpo_gtin(it.gtin.as_deref());
        let t = match limpo_gtin(it.gtin_trib.as_deref()) {
            t if t.is_empty() => g.clone(),
            t => t,
        };
        if gtin_dv_errado(&g) {
            out.push(format!("611-Rejeicao: GTIN (cEAN) invalido [nItem:{i}]"));
        }
        if !g.is_empty() && !gtin::prefixo_valido(&g) {
            out.push(format!(
                "882-Rejeicao: GTIN (cEAN) com prefixo invalido [nItem:{i}]"
            ));
        }
        if gtin_dv_errado(&t) {
            out.push(format!(
                "612-Rejeicao: GTIN da unidade tributavel (cEANTrib) invalido [nItem:{i}]"
            ));
        }
        if !t.is_empty() && !gtin::prefixo_valido(&t) {
            out.push(format!("884-Rejeicao: GTIN da unidade tributavel (cEANTrib) com prefixo invalido [nItem:{i}]"));
        }
        if g.is_empty() && !t.is_empty() {
            out.push(format!("886-Rejeicao: GTIN da unidade tributavel informado, mas nao informado o GTIN [nItem:{i}]"));
        }
    }
    out
}

pub(crate) fn modelo_referenciado(n: &Nota) -> Vec<String> {
    let mut out = Vec::new();
    if n.modelo != 55 {
        return out;
    }
    let Some(dia) = dia_uf(n) else { return out };
    if dia < vigencia("nt2026_002.v1_11_adiadas", n.tp_amb) {
        return out;
    }
    let deb = tipos(n).ok().and_then(|(d, _)| d);
    let vedado = |m: &str| m == "65" || m == "59";
    let modelos: Vec<Option<String>> = n.refs_nfe.iter().map(|c| ref_(c).map(|r| r.3)).collect();
    if n.tp_nf == 1 && n.fin_nfe != 2 && n.fin_nfe != 4 {
        for (i, m) in modelos.iter().enumerate() {
            if m.as_deref().is_some_and(vedado) {
                out.push(format!(
                    "679-Rejeicao: Chave de Acesso referenciada com Modelo invalido [nOcor: {}]",
                    i + 1
                ));
            }
        }
        if !matches!(deb.as_deref(), Some("03") | Some("04")) {
            for (i, it) in n.itens.iter().enumerate() {
                if it
                    .dfe_ref
                    .as_ref()
                    .and_then(|r| ref_(&r.0))
                    .is_some_and(|r| vedado(&r.3))
                {
                    out.push(format!("679-Rejeicao: Chave de Acesso referenciada com Modelo invalido [nItem: {}]", i + 1));
                }
            }
        }
    }
    if modelos.iter().any(|m| m.as_deref() == Some("65")) {
        for (i, it) in n.itens.iter().enumerate() {
            if matches!(it.cfop.trim().replace('.', "").as_str(), "5929" | "6929") {
                out.push(format!(
                    "375-Rejeicao: NF-e com lancamento relativo a Cupom Fiscal referencia uma NFC-e [nItem: {}]",
                    i + 1
                ));
            }
        }
    }
    out
}

pub(crate) fn grupo_vc(n: &Nota) -> Vec<String> {
    let mut out = Vec::new();
    if n.modelo != 55 {
        return out;
    }
    let Some(dia) = dia_uf(n) else { return out };
    let (deb, cred) = tipos(n).unwrap_or((None, None));
    let (deb, cred) = (deb.as_deref(), cred.as_deref());
    // datas de producao; o VC02-40 daqui e' o 1193 da NT 2025.002, nao o 679 da NT 2026.002
    let vc02_14 = vigencia("nt2025_002.vc02_14", 1);
    let vc02_40 = vigencia("nt2025_002.vc02_40", 1);
    let vc02_04_t2 = vigencia("nt2026_002.vc02_04_tpimp6", 1);
    if tp_imp(n) == 6 && dia >= vc02_04_t2 {
        for (i, it) in n.itens.iter().enumerate() {
            if it.dfe_ref.is_some() {
                out.push(format!(
                    "708-Rejeicao: NFC-e ou NF-e com DANFE Simplificado Tipo 2 nao pode referenciar documento fiscal [nItem: {}]",
                    i + 1
                ));
            }
        }
    }
    if !n.itens.iter().any(|i| i.ibscbs.is_some()) {
        return out;
    }
    let fin = n.fin_nfe;
    if fin == 5 && cred != Some("06") && n.itens.iter().any(|i| i.dfe_ref.is_some()) {
        out.push(
            "1042-Rejeicao: NF-e com referenciamento a nivel de item informado indevidamente"
                .into(),
        );
    }
    if matches!(deb, Some("03") | Some("04")) || cred == Some("06") {
        for (i, it) in n.itens.iter().enumerate() {
            if it.dfe_ref.is_none() {
                out.push(format!(
                    "1038-Rejeicao: DFeReferenciado nao informado [nItem: {}]",
                    i + 1
                ));
            }
        }
    }
    if deb == Some("03") {
        for (i, it) in n.itens.iter().enumerate() {
            if it.dfe_ref.as_ref().is_some_and(|r| r.1.is_some()) {
                out.push(format!(
                    "1039-Rejeicao: nItem do DFeReferenciado informado indevidamente [nItem: {}]",
                    i + 1
                ));
            }
        }
    }
    let nova = v153(n, dia);
    if fin == 4 && nova {
        if n.refs_nfe.is_empty() {
            for (i, it) in n.itens.iter().enumerate() {
                if it.dfe_ref.is_none() && !matches!(it.cfop.trim(), "5921" | "6921") {
                    out.push(format!("321-Rejeicao: NF-e de devolucao de mercadoria nao possui documento fiscal referenciado por item [nItem: {}]", i + 1));
                }
            }
        }
    } else if fin == 4 && dia >= vc02_14 {
        if !n.refs_nfe.is_empty() {
            out.push("321-Rejeicao: NF-e de devolucao de mercadoria nao pode referenciar a nota em refNFe (VC02-14, use DFeReferenciado)".into());
        }
        for (i, it) in n.itens.iter().enumerate() {
            if it.dfe_ref.is_none() {
                out.push(format!("321-Rejeicao: NF-e de devolucao de mercadoria nao possui documento fiscal referenciado por item [nItem: {}]", i + 1));
            }
        }
    }
    let mut vistos: HashSet<(String, Option<i64>)> = HashSet::new();
    for (i, it) in n.itens.iter().enumerate() {
        if let Some((ch, ni)) = &it.dfe_ref {
            if !vistos.insert((ch.clone(), *ni)) {
                out.push(format!(
                    "1072-Rejeicao: DFe/Item referenciado em duplicidade [nItem: {}]",
                    i + 1
                ));
            }
        }
    }
    let chaves: HashSet<&String> = n
        .itens
        .iter()
        .filter_map(|i| i.dfe_ref.as_ref().map(|r| &r.0))
        .collect();
    if chaves.len() > 1 && fin != 4 && !matches!(deb, Some("03") | Some("07")) {
        out.push("1130-Rejeicao: Mais de um documento fiscal referenciado".into());
    }
    if fin == 4 && dia >= vc02_40 {
        let emits: HashSet<String> = chaves
            .iter()
            .filter(|c| c.len() >= 20)
            .map(|c| match (nova, ref_(c)) {
                (true, Some((e, t, _, _))) => {
                    if t == "cnpj" {
                        e[..8.min(e.len())].to_string()
                    } else {
                        e
                    }
                }
                _ => c[6..20].to_string(),
            })
            .collect();
        if emits.len() > 1 {
            out.push(
                "1193-Rejeicao: Emitente das NFe Referenciadas deve ser o mesmo em todos os itens"
                    .into(),
            );
        }
    }
    out
}
