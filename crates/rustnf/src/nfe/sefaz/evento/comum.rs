// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O que todo evento tem em comum: o grupo `infEvento`, o `Id`, o texto livre e a assinatura com o A1.
//!
//! Fonte: MOC 7.00, Visao Geral, 5.8.1 (leiaute da parte geral do `envEvento`, tabela 5-32) e 5.8.4 (regras
//! genericas, tabela 5-35: `Id` = `ID` + `tpEvento` + chave + `nSeqEvento`, rejeicao 572).

use chrono::{DateTime, FixedOffset};

use crate::assinatura::cert_a1::Certificado;
use crate::assinatura::xmldsig::{self, Algoritmo};
use crate::nfe::montagem::{dh, esc, filtrar};
use crate::nfe::NS;

use super::super::erro::ErroSefaz;
use super::super::pedidos::tp_amb;
use crate::nfe::vigencias::Ambiente;

/// Versao do leiaute do evento (`verEvento`, `versao` do `evento` e do `detEvento`).
pub const VERSAO_EVENTO: &str = "1.00";

/// Escapa o texto de um elemento e os atributos.
pub(crate) fn escapar(s: &str) -> String {
    esc(s).replace('"', "&quot;").replace('\'', "&apos;")
}

/// Texto livre do leiaute (`xJust`, `xCorrecao`): so' ASCII (acento sai), espacos repetidos viram um, quebra de linha
/// vira `;`, sem espaco nas pontas, com o tamanho de `minimo` a `maximo` caracteres, escapado.
pub(crate) fn texto_livre(
    valor: &str,
    campo: &str,
    minimo: usize,
    maximo: usize,
) -> Result<String, ErroSefaz> {
    let s = filtrar(valor, true);
    let n = s.chars().count();
    if n < minimo {
        return Err(ErroSefaz::novo(
            format!("{campo}: minimo de {minimo} caracteres (veio {n})"),
            campo,
        ));
    }
    if n > maximo {
        return Err(ErroSefaz::novo(
            format!("{campo}: {n} caracteres, maximo {maximo}"),
            campo,
        ));
    }
    Ok(escapar(&s))
}

/// Texto livre que o leiaute deixa com acento (`verAplic`, unidades): sem controle nem espaco nas pontas.
pub(crate) fn texto_curto(valor: &str, campo: &str, maximo: usize) -> Result<String, ErroSefaz> {
    let s = filtrar(valor, false);
    let n = s.chars().count();
    if n == 0 {
        return Err(ErroSefaz::novo(format!("{campo}: obrigatorio"), campo));
    }
    if n > maximo {
        return Err(ErroSefaz::novo(
            format!("{campo}: {n} caracteres, maximo {maximo}"),
            campo,
        ));
    }
    if s.chars().any(|c| (c as u32) > 0xFF) {
        return Err(ErroSefaz::novo(
            format!("{campo}: so' aceita letras do alfabeto latino (ISO 8859-1)"),
            campo,
        ));
    }
    Ok(escapar(&s))
}

/// `idLote` do `envEvento`: 1 a 15 digitos.
pub(crate) fn id_lote(valor: &str) -> Result<String, ErroSefaz> {
    let s = valor.trim();
    if s.is_empty() || s.len() > 15 || !s.bytes().all(|b| b.is_ascii_digit()) {
        return Err(ErroSefaz::novo("idLote: de 1 a 15 digitos", "idLote"));
    }
    Ok(s.to_string())
}

/// `nProt` do leiaute: 15 ou 17 digitos (`TProt` do XSD).
pub(crate) fn protocolo(valor: &str, campo: &str) -> Result<String, ErroSefaz> {
    let p = valor.trim();
    if !(p.len() == 15 || p.len() == 17) || !p.bytes().all(|b| b.is_ascii_digit()) {
        return Err(ErroSefaz::novo(
            format!("{campo}: o protocolo tem 15 ou 17 digitos"),
            campo,
        ));
    }
    Ok(p.to_string())
}

/// O que identifica o evento no `infEvento`.
pub(crate) struct Cabecalho<'a> {
    pub c_orgao: u32,
    pub ambiente: Ambiente,
    /// `CNPJ` ou `CPF`.
    pub autor_tag: &'static str,
    pub autor: &'a str,
    pub chave: &'a str,
    pub tp_evento: &'a str,
    pub seq: u32,
    /// UF para o fuso do `dhEvento`.
    pub uf_fuso: &'a str,
    pub quando: &'a DateTime<FixedOffset>,
}

/// Um evento montado e ainda SEM assinatura.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Montado {
    /// `<evento>` sem a `Signature`.
    pub xml: String,
    /// `ID` + `tpEvento` + chave + `nSeqEvento`.
    pub id: String,
    /// A chave da NF-e.
    pub chave: String,
    /// `tpEvento`.
    pub tp_evento: String,
    /// `nSeqEvento`.
    pub seq: u32,
}

pub(crate) fn montar(cab: &Cabecalho<'_>, det: &str) -> Montado {
    let id = format!("ID{}{}{:02}", cab.tp_evento, cab.chave, cab.seq);
    let xml = format!(
        "<evento xmlns=\"{NS}\" versao=\"{VERSAO_EVENTO}\"><infEvento Id=\"{id}\"><cOrgao>{}</cOrgao>\
         <tpAmb>{}</tpAmb><{t}>{}</{t}><chNFe>{}</chNFe><dhEvento>{}</dhEvento><tpEvento>{}</tpEvento>\
         <nSeqEvento>{}</nSeqEvento><verEvento>{VERSAO_EVENTO}</verEvento>\
         <detEvento versao=\"{VERSAO_EVENTO}\">{det}</detEvento></infEvento></evento>",
        cab.c_orgao,
        tp_amb(cab.ambiente),
        cab.autor,
        cab.chave,
        dh(cab.quando, cab.uf_fuso),
        cab.tp_evento,
        cab.seq,
        t = cab.autor_tag,
    );
    Montado {
        xml,
        id,
        chave: cab.chave.to_string(),
        tp_evento: cab.tp_evento.to_string(),
        seq: cab.seq,
    }
}

/// O `envEvento` pronto para o webservice: UM evento assinado dentro do lote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PedidoEvento {
    /// `ID` do evento.
    pub id: String,
    /// A chave da NF-e.
    pub chave: String,
    /// `tpEvento`.
    pub tp_evento: String,
    /// `nSeqEvento`.
    pub seq: u32,
    /// `<evento>` assinado: e' ele que vai, como esta, dentro do `procEventoNFe`.
    pub evento: String,
    /// `<envEvento>` com o evento assinado: o que vai no `nfeDadosMsg`.
    pub env_evento: String,
}

impl Montado {
    /// Assina o `infEvento` com o A1 (RSA-SHA1, C14N 1.0, como o XSD do leiaute fixa) e embrulha no `envEvento`.
    ///
    /// Um evento por lote: o MOC admite ate 20, e a NT 2025.002 orienta a nao compor lote nos eventos novos (item 8.2).
    ///
    /// # Errors
    ///
    /// [`ErroSefaz`] se o `idLote` esta fora da forma ou o certificado nao assina.
    pub fn assinar(self, cert: &Certificado, id_lote_: &str) -> Result<PedidoEvento, ErroSefaz> {
        let lote = id_lote(id_lote_)?;
        let evento = xmldsig::assinar(
            &self.xml,
            &cert.chave,
            &cert.cert_der,
            "infEvento",
            Algoritmo::Sha1,
            None,
        )
        .map_err(|_| {
            ErroSefaz::novo(
                "nao foi possivel assinar o evento com o certificado",
                "Signature",
            )
        })?;
        let env_evento = format!(
            "<envEvento xmlns=\"{NS}\" versao=\"{VERSAO_EVENTO}\"><idLote>{lote}</idLote>{evento}</envEvento>"
        );
        Ok(PedidoEvento {
            id: self.id,
            chave: self.chave,
            tp_evento: self.tp_evento,
            seq: self.seq,
            evento,
            env_evento,
        })
    }
}
