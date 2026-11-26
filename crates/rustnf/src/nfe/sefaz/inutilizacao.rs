// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Inutilizacao de numeracao (`inutNFe` 4.00, servico `NfeInutilizacao`).
//!
//! MOC 7.00, Visao Geral, 5.3: pedido assinado pelo emitente com `tpAmb`, `xServ` INUTILIZAR, UF, ano (2 digitos),
//! CNPJ, modelo, serie, numero inicial e final e justificativa de 15 a 255 caracteres; `Id` de 43 posicoes (`ID` + UF +
//! ano + CNPJ + modelo + serie + inicial + final). O schema nacional so' tem `CNPJ`: nao ha `CPF` no `inutNFe`.
//!
//! Regras que o cliente confere antes da rede (tabela 5-12): ano nao superior ao atual (453) nem inferior a 2006 (454),
//! faixa inicial nao maior que a final (224) e no maximo 10.000 numeros (201).
//!
//! # O servico acabou em AC, BA e SP
//!
//! A NT 2023.002 v1.02, item 2.5 e regra I01a, elimina o webservice de inutilizacao dessas tres UF em **11/11/2026**,
//! nos dois ambientes, "nos termos dos Ajustes SINIEF 38/2026 e 40/2026" (citados pela NT): a SEFAZ passa a responder
//! HTTP 404 ou a rejeicao 998 "Servico Desativado". A NT nao diz o que fazer no lugar. [`disponivel`] diz se a UF ainda
//! tem o servico num dia; o cliente nem chama a rede quando nao tem.

use chrono::{Datelike, NaiveDate};

use super::erro::ErroSefaz;
use super::evento::comum::texto_livre;
use super::pedidos::VERSAO;
use super::tabela::{codigo_da_uf, uf_valida};
use crate::assinatura::cert_a1::Certificado;
use crate::assinatura::xmldsig::{self, Algoritmo};
use crate::nfe::montagem::documento_cnpj_cpf;
use crate::nfe::vigencias::{self, Ambiente};
use crate::nfe::NS;

use super::pedidos::tp_amb;

/// Maximo de numeros numa inutilizacao (regra I04, rejeicao 201).
pub const MAX_NUMEROS: u64 = 10_000;

/// A UF ainda tem o servico de inutilizacao no dia e ambiente?
///
/// # Errors
///
/// [`ErroSefaz`] se a UF e' desconhecida.
pub fn disponivel(uf: &str, ambiente: Ambiente, dia: NaiveDate) -> Result<bool, ErroSefaz> {
    let sigla = uf_valida(uf)?;
    if !matches!(sigla, "AC" | "BA" | "SP") {
        return Ok(true);
    }
    Ok(!vigencias::vigente("nt2023_002.inutilizacao_ac_ba_sp", dia, ambiente).unwrap_or(false))
}

/// O dia em que a UF perdeu o servico de inutilizacao no ambiente (`None` = ainda tem).
#[must_use]
pub fn eliminada_em(uf: &str, ambiente: Ambiente) -> Option<NaiveDate> {
    let sigla = uf_valida(uf).ok()?;
    if matches!(sigla, "AC" | "BA" | "SP") {
        vigencias::inicio("nt2023_002.inutilizacao_ac_ba_sp", ambiente)
    } else {
        None
    }
}

/// O que o cliente pede para inutilizar: a faixa de uma serie. A UF e o ambiente sao os do cliente.
#[derive(Debug, Clone)]
pub struct PedidoInutilizar<'a> {
    /// CNPJ do emitente.
    pub cnpj: &'a str,
    /// Serie, de 0 a 999.
    pub serie: u32,
    /// Primeiro numero da faixa.
    pub inicio: u64,
    /// Ultimo numero da faixa.
    pub fim: u64,
    /// Justificativa, de 15 a 255 caracteres.
    pub justificativa: &'a str,
    /// Ano (4 digitos); `None` = o ano do relogio do cliente.
    pub ano: Option<i32>,
}

/// Os dados de uma inutilizacao.
#[derive(Debug, Clone)]
pub struct Inutilizacao<'a> {
    /// UF do emitente (sigla ou codigo IBGE).
    pub uf: &'a str,
    /// Ambiente.
    pub ambiente: Ambiente,
    /// CNPJ do emitente.
    pub cnpj: &'a str,
    /// Serie, de 0 a 999.
    pub serie: u32,
    /// Primeiro numero da faixa.
    pub inicio: u64,
    /// Ultimo numero da faixa.
    pub fim: u64,
    /// Justificativa, de 15 a 255 caracteres.
    pub justificativa: &'a str,
    /// Ano (4 digitos) da inutilizacao; o `ano` do leiaute leva os 2 ultimos.
    pub ano: i32,
}

/// O pedido montado e ainda sem assinatura.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Montada {
    /// `<inutNFe>` sem `Signature`.
    pub xml: String,
    /// `Id` do `infInut`.
    pub id: String,
}

/// O pedido assinado, pronto para o `nfeDadosMsg`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PedidoInutilizacao {
    /// `Id` do `infInut`.
    pub id: String,
    /// `<inutNFe>` assinado: vai como esta no `ProcInutNFe`.
    pub xml: String,
}

impl Montada {
    /// Assina o `infInut` com o A1 (RSA-SHA1, C14N 1.0, como o XSD fixa).
    ///
    /// # Errors
    ///
    /// [`ErroSefaz`] se o certificado nao assina.
    pub fn assinar(self, cert: &Certificado) -> Result<PedidoInutilizacao, ErroSefaz> {
        let xml = xmldsig::assinar(
            &self.xml,
            &cert.chave,
            &cert.cert_der,
            "infInut",
            Algoritmo::Sha1,
            None,
        )
        .map_err(|_| {
            ErroSefaz::novo(
                "nao foi possivel assinar a inutilizacao com o certificado",
                "Signature",
            )
        })?;
        Ok(PedidoInutilizacao { id: self.id, xml })
    }
}

/// Monta o `inutNFe` do modelo 55 (sem assinatura). `hoje` e' o dia do relogio, para as regras 453 e 454.
///
/// # Errors
///
/// [`ErroSefaz`] com o campo e a rejeicao que a SEFAZ devolveria.
pub fn montar(p: &Inutilizacao<'_>, hoje: NaiveDate) -> Result<Montada, ErroSefaz> {
    let cuf = codigo_da_uf(p.uf)?;
    let (tag, cnpj) =
        documento_cnpj_cpf(p.cnpj, "CNPJ").map_err(|e| ErroSefaz::novo(e.mensagem, "CNPJ"))?;
    if tag != "CNPJ" {
        return Err(ErroSefaz::novo(
            "o inutNFe nacional so' tem CNPJ do emitente (nao ha CPF no schema)",
            "CNPJ",
        ));
    }
    if p.serie > 999 {
        return Err(ErroSefaz::novo("serie: de 0 a 999", "serie"));
    }
    if p.ano < 2006 {
        return Err(ErroSefaz::novo(
            "ano da inutilizacao inferior a 2006 (rejeicao 454)",
            "ano",
        ));
    }
    if p.ano > hoje.year() {
        return Err(ErroSefaz::novo(
            "ano da inutilizacao superior ao ano atual (rejeicao 453)",
            "ano",
        ));
    }
    if p.inicio < 1 || p.fim > 999_999_999 {
        return Err(ErroSefaz::novo(
            "numeros da faixa: de 1 a 999999999",
            "nNFIni",
        ));
    }
    if p.inicio > p.fim {
        return Err(ErroSefaz::novo(
            "a faixa inicial e' maior que a final (rejeicao 224)",
            "nNFIni",
        ));
    }
    if p.fim - p.inicio + 1 > MAX_NUMEROS {
        return Err(ErroSefaz::novo(
            "a faixa passa de 10.000 numeros (rejeicao 201)",
            "nNFFin",
        ));
    }
    let aa = p.ano.rem_euclid(100);
    let id = format!(
        "ID{cuf:02}{aa:02}{cnpj}55{:03}{:09}{:09}",
        p.serie, p.inicio, p.fim
    );
    let xml = format!(
        "<inutNFe xmlns=\"{NS}\" versao=\"{VERSAO}\"><infInut Id=\"{id}\"><tpAmb>{}</tpAmb>\
         <xServ>INUTILIZAR</xServ><cUF>{cuf}</cUF><ano>{aa:02}</ano><CNPJ>{cnpj}</CNPJ><mod>55</mod>\
         <serie>{}</serie><nNFIni>{}</nNFIni><nNFFin>{}</nNFFin><xJust>{}</xJust></infInut></inutNFe>",
        tp_amb(p.ambiente),
        p.serie,
        p.inicio,
        p.fim,
        texto_livre(p.justificativa, "xJust", 15, 255)?
    );
    Ok(Montada { xml, id })
}
