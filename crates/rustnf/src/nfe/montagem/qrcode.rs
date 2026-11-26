// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! QR Code e URL de consulta por chave (`infNFeSupl`) da NFC-e e da NF-e com DANFE Simplificado Tipo 2 (`tpImp` 6).
//!
//! Duas versoes do leiaute, as duas na URL `<endereco da UF>?p=<parametros separados por "|">`:
//!
//! * versao 2 (Manual de Padroes Tecnicos do DANFE NFC-e e QR Code 6.0, item 4.3): leva o identificador do CSC e um
//!   hash SHA-1 dos parametros mais o proprio CSC. Na contingencia off-line leva ainda o dia da emissao, o valor da
//!   nota e o `DigestValue` da assinatura em hexadecimal (o texto base64 do digest, byte a byte);
//! * versao 3 (NT 2025.001, Manual 6.0, item 4.4): sem CSC. On-line sao so' chave, versao e ambiente; off-line leva
//!   tambem dia, valor, tipo e numero do destinatario e a assinatura RSA SHA-1 (base64) desses parametros, feita com o
//!   certificado que assina a nota.
//!
//! A NF-e com `tpImp` 6 usa SEMPRE a versao 3 (NT 2026.002 v1.11, regra ZX02-220, rejeicao 672) e a URL da NFC-e (regra
//! ZX02-20, observacao 3). O endereco por UF e ambiente sai de `dados/nfce_qrcode.json`, com a fonte de cada linha.
//!
//! O CSC e' segredo: so' o [`Csc`] o guarda, o `Debug` o esconde e nenhuma mensagem de erro o repete.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::OnceLock;

use base64::Engine;
use serde::Deserialize;
use sha1::{Digest, Sha1};

use super::formato::{erro, erro_r, Res};
use crate::nfe::vigencias::Ambiente;

const TABELA: &str = include_str!("../../../dados/nfce_qrcode.json");

#[derive(Deserialize)]
struct Linha {
    url: String,
}

#[derive(Deserialize)]
struct PorAmbiente {
    producao: Linha,
    homologacao: Linha,
}

#[derive(Deserialize)]
struct Arquivo {
    qrcode: BTreeMap<String, PorAmbiente>,
    url_chave: BTreeMap<String, PorAmbiente>,
    fora_da_tabela: BTreeMap<String, String>,
}

fn arquivo() -> &'static Arquivo {
    static T: OnceLock<Arquivo> = OnceLock::new();
    T.get_or_init(|| {
        serde_json::from_str(TABELA).unwrap_or_else(|_| Arquivo {
            // o JSON e' embutido e conferido por teste: este ramo nao roda
            qrcode: BTreeMap::new(),
            url_chave: BTreeMap::new(),
            fora_da_tabela: BTreeMap::new(),
        })
    })
}

fn do_ambiente(p: &PorAmbiente, amb: Ambiente) -> &str {
    match amb {
        Ambiente::Producao => &p.producao.url,
        Ambiente::Homologacao => &p.homologacao.url,
    }
}

/// O endereco do QR Code da UF no ambiente, sem o `?p=` (a montagem acrescenta).
///
/// # Errors
///
/// [`super::ErroMontagem`] (campo `qrCode`) se a tabela oficial nao tem a UF: a mensagem diz por que ela ficou fora.
pub fn url_qrcode(uf: &str, amb: Ambiente) -> Res<&'static str> {
    let a = arquivo();
    match a.qrcode.get(uf) {
        Some(p) => Ok(do_ambiente(p, amb)),
        None => Err(erro(
            match a.fora_da_tabela.get(uf) {
                Some(porque) => format!("sem endereco do QR Code para {uf}: {porque}"),
                None => format!("sem endereco do QR Code para a UF {uf}"),
            },
            "qrCode",
        )),
    }
}

/// O endereco da consulta por chave de acesso (`urlChave`) da UF no ambiente, como o Portal Nacional da NFC-e o publica.
///
/// # Errors
///
/// [`super::ErroMontagem`] (campo `urlChave`) se a tabela nao tem a UF.
pub fn url_chave(uf: &str, amb: Ambiente) -> Res<&'static str> {
    arquivo()
        .url_chave
        .get(uf)
        .map(|p| do_ambiente(p, amb))
        .ok_or_else(|| {
            erro(
                format!("sem endereco de consulta por chave para a UF {uf}"),
                "urlChave",
            )
        })
}

/// As UF que a tabela do QR Code cobre.
pub fn ufs_com_qrcode() -> impl Iterator<Item = &'static str> {
    arquivo().qrcode.keys().map(String::as_str)
}

/// Codigo de Seguranca do Contribuinte (CSC) e o identificador dele (`idCSC`), que a SEFAZ da UF fornece pelo portal
/// dela. So' a versao 2 do QR Code usa. O valor nao aparece no `Debug` nem em mensagem de erro.
#[derive(Clone)]
pub struct Csc {
    id: String,
    valor: String,
}

impl fmt::Debug for Csc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Csc")
            .field("id", &self.id)
            .field("valor", &"<oculto>")
            .finish()
    }
}

impl Csc {
    /// Confere o identificador (1 a 6 digitos; os zeros a esquerda saem na URL) e o codigo (16 a 36 caracteres
    /// alfanumericos, Manual 6.0, item 4.6).
    ///
    /// # Errors
    ///
    /// [`super::ErroMontagem`] (campo `idCSC` ou `CSC`), sem repetir o codigo.
    pub fn novo(id: &str, valor: &str) -> Res<Csc> {
        let i = id.trim();
        if i.is_empty() || i.len() > 6 || !i.bytes().all(|b| b.is_ascii_digit()) {
            return Err(erro("idCSC invalido: de 1 a 6 digitos", "idCSC"));
        }
        let v = valor.trim();
        if !(16..=36).contains(&v.len()) || !v.bytes().all(|b| b.is_ascii_alphanumeric()) {
            return Err(erro(
                "CSC invalido: de 16 a 36 caracteres alfanumericos (o que a SEFAZ da UF forneceu)",
                "CSC",
            ));
        }
        let sem_zeros = i.trim_start_matches('0');
        Ok(Csc {
            id: if sem_zeros.is_empty() { "0" } else { sem_zeros }.to_string(),
            valor: v.to_string(),
        })
    }

    /// O `idCSC` como vai na URL (sem zeros a esquerda).
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }
}

/// Como o `assinar_com` monta o QR Code. O padrao (sem CSC) gera a versao 3.
#[derive(Debug, Clone, Default)]
pub struct OpcoesQr {
    /// Versao do QR Code (2 ou 3). Vazio: 2 se ha CSC, senao 3. A NF-e com `tpImp` 6 so' aceita a 3.
    pub versao: Option<u8>,
    /// O CSC, obrigatorio na versao 2.
    pub csc: Option<Csc>,
}

/// O que o QR Code le da nota montada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Dados {
    pub(crate) chave: String,
    pub(crate) modelo: String,
    pub(crate) tp_imp: String,
    pub(crate) tp_emis: String,
    pub(crate) tp_amb: String,
    pub(crate) dia: String,
    pub(crate) v_nf: String,
    pub(crate) emitente_cpf: bool,
    /// (tipo 1, 2 ou 3, numero) do destinatario; vazio se a nota nao o identifica.
    pub(crate) destino: (String, String),
}

fn texto<'a>(pai: roxmltree::Node<'a, '_>, nome: &str) -> &'a str {
    pai.children()
        .find(|c| c.is_element() && c.tag_name().name() == nome)
        .and_then(|c| c.text())
        .unwrap_or("")
        .trim()
}

fn filho<'a, 'i>(pai: roxmltree::Node<'a, 'i>, nome: &str) -> Option<roxmltree::Node<'a, 'i>> {
    pai.children()
        .find(|c| c.is_element() && c.tag_name().name() == nome)
}

/// Le da nota (montada ou assinada) o que o QR Code precisa.
pub(crate) fn ler_dados(xml: &str) -> Res<Dados> {
    let doc = roxmltree::Document::parse(xml)
        .map_err(|_| erro("a nota nao e' um XML bem formado", "NFe"))?;
    let inf = filho(doc.root_element(), "infNFe")
        .ok_or_else(|| erro("a nota nao tem o grupo infNFe", "infNFe"))?;
    let ide = filho(inf, "ide").ok_or_else(|| erro("a nota nao tem o grupo ide", "ide"))?;
    let chave = inf
        .attribute("Id")
        .and_then(|i| i.strip_prefix("NFe"))
        .unwrap_or("")
        .to_string();
    let v_nf = filho(inf, "total")
        .and_then(|t| filho(t, "ICMSTot"))
        .map(|t| texto(t, "vNF"))
        .unwrap_or("")
        .to_string();
    let destino = match filho(inf, "dest") {
        Some(d) if !texto(d, "CNPJ").is_empty() => ("1".into(), texto(d, "CNPJ").to_string()),
        Some(d) if !texto(d, "CPF").is_empty() => ("2".into(), texto(d, "CPF").to_string()),
        Some(d) if filho(d, "idEstrangeiro").is_some() => ("3".into(), String::new()),
        _ => (String::new(), String::new()),
    };
    let emitente_cpf = filho(inf, "emit").is_some_and(|e| filho(e, "CPF").is_some());
    let dh = texto(ide, "dhEmi");
    let dia = dh.get(8..10).unwrap_or("").to_string();
    if chave.len() != 44 || v_nf.is_empty() || dia.len() != 2 {
        return Err(erro(
            "a nota nao tem chave, dhEmi ou vNF legiveis para o QR Code",
            "qrCode",
        ));
    }
    Ok(Dados {
        chave,
        modelo: texto(ide, "mod").to_string(),
        tp_imp: texto(ide, "tpImp").to_string(),
        tp_emis: texto(ide, "tpEmis").to_string(),
        tp_amb: texto(ide, "tpAmb").to_string(),
        dia,
        v_nf,
        emitente_cpf,
        destino,
    })
}

/// A nota leva `infNFeSupl`: a NFC-e e a NF-e com `tpImp` 6.
pub(crate) fn exige_qr(d: &Dados) -> bool {
    d.modelo == "65" || (d.modelo == "55" && d.tp_imp == "6")
}

fn hex_maiusculo(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02X}")).collect()
}

fn hex_minusculo(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// O parametro de hash da versao 2: SHA-1 de `<parametros>` seguido do CSC, em hexadecimal maiusculo (Manual 6.0,
/// itens 4.3.4 e 4.3.5).
#[must_use]
pub fn hash_v2(parametros: &str, csc: &str) -> String {
    hex_maiusculo(&Sha1::digest(format!("{parametros}{csc}").as_bytes()))
}

fn qr_v2(d: &Dados, csc: &Csc, dig_val: &str) -> String {
    let base = if d.tp_emis == "9" {
        format!(
            "{}|2|{}|{}|{}|{}|{}",
            d.chave,
            d.tp_amb,
            d.dia,
            d.v_nf,
            hex_minusculo(dig_val.as_bytes()),
            csc.id
        )
    } else {
        format!("{}|2|{}|{}", d.chave, d.tp_amb, csc.id)
    };
    let hash = hash_v2(&base, &csc.valor);
    format!("{base}|{hash}")
}

fn qr_v3(d: &Dados, assinar: &dyn Fn(&[u8]) -> Vec<u8>) -> String {
    let online = format!("{}|3|{}", d.chave, d.tp_amb);
    if d.tp_emis != "9" {
        return online;
    }
    let corpo = format!(
        "{online}|{}|{}|{}|{}",
        d.dia, d.v_nf, d.destino.0, d.destino.1
    );
    let sig = base64::engine::general_purpose::STANDARD.encode(assinar(corpo.as_bytes()));
    format!("{corpo}|{sig}")
}

/// A versao do QR Code que a nota usa e o que falta para gera-la.
pub(crate) fn versao(d: &Dados, op: &OpcoesQr) -> Res<u8> {
    let v = op
        .versao
        .unwrap_or(if op.csc.is_some() && d.modelo == "65" { 2 } else { 3 });
    if !(2..=3).contains(&v) {
        return Err(erro(format!("versao do QR Code {v}: use 2 ou 3"), "qrCode"));
    }
    if d.modelo == "55" && v != 3 {
        return Err(erro_r(
            "a NF-e com DANFE Simplificado Tipo 2 (tpImp 6) so' aceita o QR Code versao 3 (ZX02-220)",
            "qrCode",
            "672",
        ));
    }
    if v == 2 && d.emitente_cpf {
        return Err(erro_r(
            "o emitente pessoa fisica (produtor rural, CPF) usa o QR Code versao 3: a versao 2 e' rejeitada (ZX02-222)",
            "qrCode",
            "444",
        ));
    }
    if v == 2 && op.csc.is_none() {
        return Err(erro(
            "o QR Code versao 2 exige o CSC e o idCSC que a SEFAZ da UF fornece",
            "CSC",
        ));
    }
    Ok(v)
}

/// O texto do `qrCode`. `dig_val` e' o `DigestValue` da assinatura (so' a versao 2 off-line o usa); `assinar` assina
/// os parametros da versao 3 off-line.
pub(crate) fn texto_do_qr(
    d: &Dados,
    versao: u8,
    endereco: &str,
    csc: Option<&Csc>,
    dig_val: &str,
    assinar: &dyn Fn(&[u8]) -> Vec<u8>,
) -> Res<String> {
    let parametros = match (versao, csc) {
        (2, Some(c)) => qr_v2(d, c, dig_val),
        (2, None) => return Err(erro("QR Code versao 2 sem CSC", "CSC")),
        _ => qr_v3(d, assinar),
    };
    Ok(format!("{endereco}?p={parametros}"))
}

/// O grupo `infNFeSupl` com o `qrCode` e a `urlChave`, dentro dos limites do XSD (60 a 1000 e 21 a 85 caracteres).
pub(crate) fn infnfesupl(qr: &str, url_chave: &str) -> Res<String> {
    if !(60..=1000).contains(&qr.len()) {
        return Err(erro("o qrCode sai fora de 60 a 1000 caracteres", "qrCode"));
    }
    if !(21..=85).contains(&url_chave.len()) {
        return Err(erro(
            "a urlChave da tabela sai fora de 21 a 85 caracteres",
            "urlChave",
        ));
    }
    Ok(format!(
        "<infNFeSupl><qrCode>{}</qrCode><urlChave>{}</urlChave></infNFeSupl>",
        super::formato::esc(qr),
        super::formato::esc(url_chave)
    ))
}
