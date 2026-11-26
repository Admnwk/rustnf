// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! EPEC, o Evento Previo de Emissao em Contingencia (110140).
//!
//! MOC 7.00, Visao Geral, 3.3 (visao geral) e 5.12 (leiaute, tabela 5-43, e regras, tabela 5-45). O EPEC e' enviado ao
//! **Ambiente Nacional**, nao a SEFAZ da UF (item 3.3, primeiro paragrafo): o `cOrgao` do `infEvento` e' 91 ("Ambiente
//! Nacional", tabela 5-32, P08) e o `cOrgaoAutor`, a UF do emitente. Os dados saem da propria NF-e (que tem `tpEmis` 4
//! na chave e ainda nao foi transmitida): a SEFAZ confere o EPEC contra ela quando a nota chega e o EPEC e' dado como
//! conciliado (item 3.3.11); quem registra o EPEC tem de transmitir a MESMA NF-e depois (item 3.3.6, 168 horas).
//!
//! O EPEC so' existe para NF-e de emitente com CNPJ (rejeicao 408 para CPF), serie de 0 a 889, `nSeqEvento` 1, e a
//! NF-e emitida no maximo 1 dia antes do evento (228). O registro devolve **136** (registrado, sem NF-e para vincular;
//! o MOC exclui o 135, item 5.12.4) ou **142** com as chaves dos EPEC pendentes (`chNFePend`).

use chrono::{DateTime, FixedOffset};

use super::super::erro::ErroSefaz;
use super::super::pedidos::{chave_valida, sem_declaracao};
use super::super::tabela::uf_valida;
use super::comum::{escapar, montar, texto_curto, Cabecalho, Montado};
use crate::nfe::vigencias::Ambiente;
use crate::nfe::NS;

/// `tpEvento` do EPEC.
pub const EPEC: &str = "110140";

/// `cOrgao` do Ambiente Nacional, que recebe o EPEC.
pub const ORGAO_AMBIENTE_NACIONAL: u32 = 91;

/// Os campos do EPEC, como estao na NF-e.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CamposEpec {
    /// A chave de 44 posicoes (`infNFe/@Id` sem o `NFe`).
    pub chave: String,
    /// `ide/dhEmi`, como esta na NF-e.
    pub dh_emi: String,
    /// `ide/tpNF`.
    pub tp_nf: String,
    /// `emit/IE`.
    pub ie: String,
    /// `dest/enderDest/UF` (`EX` para o exterior).
    pub dest_uf: String,
    /// `CNPJ`, `CPF` ou `idEstrangeiro`.
    pub dest_tag: String,
    /// O documento do destinatario.
    pub dest_doc: String,
    /// `dest/IE`; vazio = destinatario sem IE.
    pub dest_ie: String,
    /// `total/ICMSTot/vNF`.
    pub v_nf: String,
    /// `total/ICMSTot/vICMS`.
    pub v_icms: String,
    /// `total/ICMSTot/vST`.
    pub v_st: String,
}

type No<'a, 'i> = roxmltree::Node<'a, 'i>;

fn filho<'a, 'i>(n: No<'a, 'i>, nome: &str) -> Option<No<'a, 'i>> {
    n.children().find(|c| {
        c.is_element() && c.tag_name().name() == nome && c.tag_name().namespace() == Some(NS)
    })
}

fn texto(n: Option<No<'_, '_>>) -> String {
    n.and_then(|x| x.text()).unwrap_or("").trim().to_string()
}

fn caminho(n: No<'_, '_>, partes: &[&str]) -> String {
    let mut atual = Some(n);
    for p in partes {
        atual = atual.and_then(|x| filho(x, p));
    }
    texto(atual)
}

/// Le da NF-e (`NFe` ou `nfeProc`) o que o EPEC leva.
///
/// # Errors
///
/// [`ErroSefaz`] se o texto nao e' XML, a raiz nao e' `NFe` nem `nfeProc` do leiaute 4.00, ou falta o destinatario.
pub fn campos_epec(nfe: &str) -> Result<CamposEpec, ErroSefaz> {
    let doc = roxmltree::Document::parse(sem_declaracao(nfe))
        .map_err(|_| ErroSefaz::novo("a NF-e nao e' um XML bem formado", "NFe"))?;
    let raiz = doc.root_element();
    let e_ns = |n: &No<'_, '_>, nome: &str| {
        n.tag_name().name() == nome && n.tag_name().namespace() == Some(NS)
    };
    let nfe_el = if e_ns(&raiz, "NFe") {
        Some(raiz)
    } else if e_ns(&raiz, "nfeProc") {
        filho(raiz, "NFe")
    } else {
        None
    };
    let Some(inf) = nfe_el.and_then(|n| filho(n, "infNFe")) else {
        return Err(ErroSefaz::novo(
            "o EPEC sai da NF-e: a raiz tem de ser <NFe> ou <nfeProc> do leiaute 4.00",
            "NFe",
        ));
    };
    let chave = inf
        .attribute("Id")
        .and_then(|i| i.strip_prefix("NFe"))
        .unwrap_or("")
        .to_string();
    let Some(dest) = filho(inf, "dest") else {
        return Err(ErroSefaz::novo(
            "o EPEC exige o destinatario da NF-e (grupo dest)",
            "dest",
        ));
    };
    let Some((tag, doc_dest)) = ["CNPJ", "CPF", "idEstrangeiro"]
        .iter()
        .find_map(|t| filho(dest, t).map(|el| ((*t).to_string(), texto(Some(el)))))
    else {
        return Err(ErroSefaz::novo(
            "o destinatario nao tem CNPJ, CPF nem idEstrangeiro",
            "dest",
        ));
    };
    let mut uf = caminho(dest, &["enderDest", "UF"]);
    if uf.is_empty() && tag == "idEstrangeiro" {
        uf = "EX".into();
    }
    let tot = filho(inf, "total").and_then(|t| filho(t, "ICMSTot"));
    let tv = |nome: &str| tot.map(|t| caminho(t, &[nome])).unwrap_or_default();
    Ok(CamposEpec {
        chave,
        dh_emi: caminho(inf, &["ide", "dhEmi"]),
        tp_nf: caminho(inf, &["ide", "tpNF"]),
        ie: caminho(inf, &["emit", "IE"]),
        dest_uf: uf,
        dest_tag: tag,
        dest_doc: doc_dest,
        dest_ie: caminho(dest, &["IE"]),
        v_nf: tv("vNF"),
        v_icms: tv("vICMS"),
        v_st: tv("vST"),
    })
}

/// Os dados do pedido de EPEC.
#[derive(Debug, Clone)]
pub struct Epec<'a> {
    /// A NF-e com `tpEmis` 4, assinada ou nao, que o EPEC antecipa.
    pub nfe: &'a str,
    /// Ambiente.
    pub ambiente: Ambiente,
    /// Data e hora do evento.
    pub quando: DateTime<FixedOffset>,
    /// Versao do aplicativo do autor (`verAplic`, ate 20 caracteres).
    pub ver_aplic: &'a str,
}

/// `IE` do leiaute do EPEC: 2 a 14 digitos, nao zerada.
fn ie_valida(s: &str) -> bool {
    (2..=14).contains(&s.len())
        && s.bytes().all(|b| b.is_ascii_digit())
        && s.bytes().any(|b| b != b'0')
}

/// `TDec_1302` do XSD: `0`, `0.dd` ou inteiro de ate 13 digitos com no maximo 2 casas.
fn dec_1302(s: &str, campo: &str) -> Result<(), ErroSefaz> {
    let (int, frac) = s.split_once('.').map_or((s, ""), |(a, b)| (a, b));
    let ok = !int.is_empty()
        && int.len() <= 13
        && int.bytes().all(|b| b.is_ascii_digit())
        && (int == "0" || !int.starts_with('0'))
        && (!s.contains('.') || frac.len() == 2)
        && frac.bytes().all(|b| b.is_ascii_digit());
    if ok {
        Ok(())
    } else {
        Err(ErroSefaz::novo(
            format!("{campo}: '{s}' nao e' valor com ate 2 casas decimais"),
            campo,
        ))
    }
}

/// Monta o EPEC (sem assinatura), com as conferencias da tabela 5-45 que dependem so' da NF-e e do relogio.
///
/// # Errors
///
/// [`ErroSefaz`] com o campo e, no texto, a rejeicao que a SEFAZ devolveria.
pub fn epec(e: &Epec<'_>) -> Result<Montado, ErroSefaz> {
    let k = campos_epec(e.nfe)?;
    let c = chave_valida(&k.chave)?;
    if c.emitente_cpf {
        return Err(ErroSefaz::novo(
            "EPEC nao existe para emitente pessoa fisica (rejeicao 408)",
            "CPF",
        ));
    }
    if c.serie > 889 {
        return Err(ErroSefaz::novo(
            format!(
                "serie {} fora da faixa do EPEC, de 0 a 889 (rejeicao 266)",
                c.serie
            ),
            "serie",
        ));
    }
    if c.tp_emis != 4 {
        return Err(ErroSefaz::novo(
            "a NF-e do EPEC tem tpEmis 4 na chave, posicao 35 (rejeicao 484)",
            "tpEmis",
        ));
    }
    let emissao = DateTime::parse_from_rfc3339(&k.dh_emi).map_err(|_| {
        ErroSefaz::novo(
            "dhEmi da NF-e ausente ou fora do formato UTC com fuso",
            "dhEmi",
        )
    })?;
    if emissao > e.quando {
        return Err(ErroSefaz::novo(
            "dhEmi da NF-e depois do evento (rejeicoes 212 e 577)",
            "dhEmi",
        ));
    }
    if e.quando.signed_duration_since(emissao) > chrono::Duration::days(1) {
        return Err(ErroSefaz::novo(
            "dhEmi da NF-e ha mais de 1 dia (rejeicao 228): o EPEC se registra na emissao",
            "dhEmi",
        ));
    }
    let aamm_emissao = format!(
        "{}{}",
        k.dh_emi.get(2..4).unwrap_or(""),
        k.dh_emi.get(5..7).unwrap_or("")
    );
    if aamm_emissao != c.aamm {
        return Err(ErroSefaz::novo(
            "ano e mes do dhEmi divergem da chave (rejeicao 659)",
            "dhEmi",
        ));
    }
    if !ie_valida(&k.ie) {
        return Err(ErroSefaz::novo(
            "IE do emitente: 2 a 14 digitos e nao zerada (rejeicoes 229 e 209)",
            "IE",
        ));
    }
    if k.tp_nf != "0" && k.tp_nf != "1" {
        return Err(ErroSefaz::novo("tpNF da NF-e: 0 ou 1", "tpNF"));
    }
    conferir_destinatario(&k, c.uf)?;
    for (v, campo) in [(&k.v_nf, "vNF"), (&k.v_icms, "vICMS"), (&k.v_st, "vST")] {
        dec_1302(v, campo)?;
    }
    let ie_dest = if k.dest_ie.is_empty() {
        String::new()
    } else {
        format!("<IE>{}</IE>", k.dest_ie)
    };
    let tag = &k.dest_tag;
    let doc_dest = escapar(&k.dest_doc);
    let det = format!(
        "<descEvento>EPEC</descEvento><cOrgaoAutor>{}</cOrgaoAutor><tpAutor>1</tpAutor><verAplic>{}</verAplic>\
         <dhEmi>{}</dhEmi><tpNF>{}</tpNF><IE>{}</IE><dest><UF>{}</UF><{tag}>{doc_dest}</{tag}>{ie_dest}\
         <vNF>{}</vNF><vICMS>{}</vICMS><vST>{}</vST></dest>",
        c.cuf,
        texto_curto(e.ver_aplic, "verAplic", 20)?,
        k.dh_emi,
        k.tp_nf,
        k.ie,
        k.dest_uf,
        k.v_nf,
        k.v_icms,
        k.v_st
    );
    Ok(montar(
        &Cabecalho {
            c_orgao: ORGAO_AMBIENTE_NACIONAL,
            ambiente: e.ambiente,
            autor_tag: "CNPJ",
            autor: &c.emitente,
            chave: &c.chave,
            tp_evento: EPEC,
            seq: 1,
            uf_fuso: c.uf,
            quando: &e.quando,
        },
        &det,
    ))
}

fn conferir_destinatario(k: &CamposEpec, uf_emitente: &str) -> Result<(), ErroSefaz> {
    let uf = k.dest_uf.as_str();
    if uf == "EX" && k.dest_tag != "idEstrangeiro" {
        return Err(ErroSefaz::novo(
            "destinatario no exterior (UF EX): informe idEstrangeiro (rejeicao 720)",
            "idEstrangeiro",
        ));
    }
    if k.dest_tag == "idEstrangeiro" && uf != "EX" && uf != uf_emitente {
        return Err(ErroSefaz::novo(
            "operacao interestadual: informe CNPJ ou CPF do destinatario, nao idEstrangeiro (rejeicao 721)",
            "idEstrangeiro",
        ));
    }
    if uf != "EX" {
        uf_valida(uf)?;
    }
    if !k.dest_ie.is_empty() {
        if uf == "EX" {
            return Err(ErroSefaz::novo(
                "IE do destinatario no exterior (rejeicao 792)",
                "dest/IE",
            ));
        }
        if !ie_valida(&k.dest_ie) {
            return Err(ErroSefaz::novo(
                "IE do destinatario: 2 a 14 digitos e nao zerada (rejeicao 210)",
                "dest/IE",
            ));
        }
    }
    Ok(())
}
