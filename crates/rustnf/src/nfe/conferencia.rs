// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O que a nota lida diz de si: a situacao pelo protocolo e as divergencias entre as partes do proprio XML.
//!
//! A conferencia e' interna: compara a chave com o `ide`, os totais com a soma dos itens e as duplicatas com a
//! fatura. Nao fala com a SEFAZ e nao substitui a consulta pela chave.

use super::cstat::{AUTORIZADA, CANCELADA, DENEGADA};
use super::decimal::Decimal;
use super::modelo::{Grupo, Imposto, Item, Nfe};
use crate::chave_dfe;

/// Situacao da nota pelo protocolo que o arquivo traz.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Situacao {
    /// `cStat` 100 ou 150.
    Autorizada,
    /// `cStat` 101 ou 151.
    Cancelada,
    /// `cStat` 110, 301, 302 ou 303.
    Denegada,
    /// Protocolo com outro `cStat`: a nota nao foi autorizada.
    Rejeitada,
    /// O arquivo e' a `NFe` solta, sem `protNFe`: nao esta autorizada.
    SemProtocolo,
    /// O protocolo e' de outra chave: nao vale para esta nota.
    ProtocoloDivergente,
}

impl Situacao {
    /// Nome estavel (`autorizada`, `cancelada`, `denegada`, `rejeitada`, `sem_protocolo`, `protocolo_divergente`).
    #[must_use]
    pub fn como_texto(self) -> &'static str {
        match self {
            Self::Autorizada => "autorizada",
            Self::Cancelada => "cancelada",
            Self::Denegada => "denegada",
            Self::Rejeitada => "rejeitada",
            Self::SemProtocolo => "sem_protocolo",
            Self::ProtocoloDivergente => "protocolo_divergente",
        }
    }
}

impl Nfe {
    /// A situacao pelo protocolo. So' o protocolo decide: sem `protNFe` a nota NAO esta autorizada, e protocolo de
    /// outra chave nao vale.
    #[must_use]
    pub fn situacao(&self) -> Situacao {
        let Some(p) = &self.protocolo else {
            return Situacao::SemProtocolo;
        };
        if !p.chave.is_empty() && p.chave != self.chave {
            return Situacao::ProtocoloDivergente;
        }
        let c = p.cstat.as_str();
        if AUTORIZADA.contains(&c) {
            Situacao::Autorizada
        } else if CANCELADA.contains(&c) {
            Situacao::Cancelada
        } else if DENEGADA.contains(&c) {
            Situacao::Denegada
        } else {
            Situacao::Rejeitada
        }
    }

    /// A nota esta autorizada (protocolo 100 ou 150, da mesma chave).
    #[must_use]
    pub fn autorizada(&self) -> bool {
        self.situacao() == Situacao::Autorizada
    }

    /// A chave de acesso passa em [`chave_dfe::validar`] (com o emitente).
    #[must_use]
    pub fn chave_valida(&self) -> bool {
        self.chave_erro.is_none()
    }

    /// Nota de homologacao (`tpAmb` 2): sem valor fiscal.
    #[must_use]
    pub fn homologacao(&self) -> bool {
        self.ide.ambiente == "2"
    }
}

/// Divergencias que a nota carrega (vazio = consistente). Cada frase diz o que difere.
#[must_use]
pub fn conferir(nfe: &Nfe) -> Vec<String> {
    let mut out = Vec::new();
    match &nfe.chave_erro {
        Some(e) => out.push(format!("chave de acesso invalida: {e}")),
        None => out.extend(chave_contra_ide(nfe)),
    }
    if nfe.situacao() == Situacao::ProtocoloDivergente {
        out.push("o protocolo e de outra chave".into());
    }
    if nfe
        .protocolo
        .as_ref()
        .is_some_and(|p| p.ambiente.is_empty())
    {
        out.push("protocolo sem tpAmb (campo obrigatorio do leiaute)".into());
    }
    if nfe.homologacao() {
        out.push("nota de homologacao (tpAmb=2): sem valor fiscal".into());
    }
    out.extend(totais_contra_itens(nfe));
    out.extend(duplicatas_contra_fatura(nfe));
    out
}

/// Cada campo da chave contra o mesmo campo do `ide`.
fn chave_contra_ide(nfe: &Nfe) -> Vec<String> {
    let mut out = Vec::new();
    let Ok(k) = chave_dfe::decompor(&nfe.chave, true) else {
        return out;
    };
    let i = &nfe.ide;
    let aamm = i
        .emissao
        .map(|d| d.0.format("%y%m").to_string())
        .unwrap_or_default();
    let cnf = i.cnf.trim_start_matches('0');
    if i.cnf.is_empty() {
        out.push("cNF ausente no XML (campo obrigatorio do leiaute)".to_string());
    }
    let pares: [(&str, String, String); 9] = [
        ("cUF", i.cuf.clone(), format!("{:02}", k.cuf)),
        ("AAMM da emissao", aamm, k.aamm.clone()),
        ("emitente", nfe.emit.documento.clone(), k.emitente.clone()),
        ("modelo", i.modelo.clone(), k.modelo.clone()),
        ("serie", i.serie.to_string(), k.serie.to_string()),
        ("numero", i.numero.to_string(), k.numero.to_string()),
        ("tpEmis", i.tipo_emissao.clone(), k.tp_emis.to_string()),
        (
            "cNF",
            if i.cnf.is_empty() {
                String::new()
            } else if cnf.is_empty() {
                "0".into()
            } else {
                cnf.to_string()
            },
            k.codigo.to_string(),
        ),
        ("cDV", i.cdv.clone(), k.dv.to_string()),
    ];
    for (nome, no_xml, na_chave) in pares {
        if !no_xml.is_empty() && no_xml != na_chave {
            out.push(format!(
                "{nome} do XML ({no_xml}) difere da chave ({na_chave})"
            ));
        }
    }
    out
}

/// Soma de um campo de item (`vProd`, `vDesc`...), ignorando o item que nao compoe o total.
fn soma_do_item(itens: &[Item], f: impl Fn(&Item) -> Option<Decimal>) -> Decimal {
    itens.iter().filter_map(f).sum()
}

/// Soma de um campo de um grupo de imposto dos itens.
fn soma_do_imposto(
    itens: &[Item],
    grupo: impl Fn(&Imposto) -> Option<&Grupo>,
    tag: &str,
) -> Decimal {
    itens
        .iter()
        .filter_map(|x| grupo(&x.imposto).and_then(|g| g.valores.get(tag).copied()))
        .sum()
}

/// O `ICMSTot` contra a soma dos itens (`vProd`, `vDesc`, `vFrete`, `vSeg`, `vOutro`, ICMS, ST, IPI, PIS e COFINS).
fn totais_contra_itens(nfe: &Nfe) -> Vec<String> {
    let Some(tot) = &nfe.total.icms_tot else {
        return Vec::new();
    };
    let it = &nfe.itens;
    let somas: [(&str, Decimal); 10] = [
        (
            "vProd",
            soma_do_item(it, |x| {
                if x.compoe_total == "0" {
                    None
                } else {
                    x.valor_produto
                }
            }),
        ),
        ("vDesc", soma_do_item(it, |x| x.desconto)),
        ("vFrete", soma_do_item(it, |x| x.frete)),
        ("vSeg", soma_do_item(it, |x| x.seguro)),
        ("vOutro", soma_do_item(it, |x| x.outros)),
        ("vICMS", soma_do_imposto(it, |m| m.icms.as_ref(), "vICMS")),
        ("vST", soma_do_imposto(it, |m| m.icms.as_ref(), "vICMSST")),
        ("vIPI", soma_do_imposto(it, |m| m.ipi.as_ref(), "vIPI")),
        ("vPIS", soma_do_imposto(it, |m| m.pis.as_ref(), "vPIS")),
        (
            "vCOFINS",
            soma_do_imposto(it, |m| m.cofins.as_ref(), "vCOFINS"),
        ),
    ];
    somas
        .into_iter()
        .filter_map(|(tag, soma)| {
            let total = tot.valores.get(tag)?;
            (soma != *total)
                .then(|| format!("soma dos itens em {tag} ({soma}) difere do total ({total})"))
        })
        .collect()
}

/// As duplicatas contra o liquido da fatura.
fn duplicatas_contra_fatura(nfe: &Nfe) -> Option<String> {
    let c = nfe.cobranca.as_ref()?;
    let liquido = c.fatura_liquido?;
    if c.duplicatas.is_empty() {
        return None;
    }
    let soma: Decimal = c.duplicatas.iter().filter_map(|d| d.valor).sum();
    (soma != liquido)
        .then(|| format!("soma das duplicatas ({soma}) difere do liquido da fatura ({liquido})"))
}
