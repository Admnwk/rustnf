// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Assinatura do `infNFe` com o certificado A1 e o QR Code (`infNFeSupl`) da NFC-e.

use crate::assinatura::cert_a1::Certificado;
use crate::assinatura::xmldsig::{self, Algoritmo};
use crate::nfe::vigencias::Ambiente;

use super::formato::{erro, uf_sigla, Res};
use super::modelo::Montada;
use super::qrcode::{self, OpcoesQr};

/// Assina o `infNFe` da nota montada e devolve o XML da `NFe` com a `Signature` como ultimo filho.
///
/// A NFC-e (modelo 65) e a NF-e com `tpImp` 6 saem com o QR Code (`infNFeSupl`) entre o `infNFe` e a `Signature`;
/// sem CSC o QR Code e' o da versao 3. Para a versao 2, com CSC, use [`assinar_com`].
///
/// O XSD da NF-e 4.00 (`xmldsig-core-schema_v1.01.xsd`) fixa `rsa-sha1` como `SignatureMethod` e `sha1` como
/// `DigestMethod`; por isso o algoritmo e' SHA-1, nao o SHA-256 que a NFS-e Nacional usa. A canonicalizacao e' a
/// C14N 1.0 do mesmo XSD.
///
/// # Errors
///
/// [`super::ErroMontagem`] (campo `Signature`, ou do QR Code) se o XML nao puder ser assinado.
pub fn assinar(m: &Montada, cert: &Certificado) -> Res<String> {
    assinar_com(m, cert, &OpcoesQr::default())
}

/// Como [`assinar`], com a versao do QR Code e o CSC.
///
/// # Errors
///
/// [`super::ErroMontagem`] (campo `Signature`, `qrCode`, `CSC` ou `urlChave`) se a nota nao puder ser assinada ou o
/// QR Code nao puder ser montado (UF fora da tabela, CSC ausente na versao 2, versao nao aceita pela nota).
pub fn assinar_com(m: &Montada, cert: &Certificado, op: &OpcoesQr) -> Res<String> {
    let assinada = xmldsig::assinar(
        &m.xml,
        &cert.chave,
        &cert.cert_der,
        "infNFe",
        Algoritmo::Sha1,
        None,
    )
    .map_err(|e| erro(e.0, "Signature"))?;
    if !m.xml.contains("<mod>65</mod>") && !m.xml.contains("<tpImp>6</tpImp>") {
        return Ok(assinada);
    }
    let d = qrcode::ler_dados(&m.xml)?;
    if !qrcode::exige_qr(&d) {
        return Ok(assinada);
    }
    let versao = qrcode::versao(&d, op)?;
    let uf = d
        .chave
        .get(..2)
        .and_then(|c| c.parse::<u32>().ok())
        .and_then(uf_sigla)
        .ok_or_else(|| erro("cUF da chave desconhecido", "cUF"))?;
    let amb = if d.tp_amb == "1" {
        Ambiente::Producao
    } else {
        Ambiente::Homologacao
    };
    let endereco = qrcode::url_qrcode(uf, amb)?;
    let url_chave = qrcode::url_chave(uf, amb)?;
    let dig_val = digest_value(&assinada)?;
    let assina = |b: &[u8]| xmldsig::assinar_sha1(&cert.chave, b);
    let qr = qrcode::texto_do_qr(&d, versao, endereco, op.csc.as_ref(), &dig_val, &assina)?;
    let supl = qrcode::infnfesupl(&qr, url_chave)?;
    // a Signature e' o ultimo filho da NFe, logo depois do fechamento do infNFe: o infNFeSupl entra antes dela
    let ponto = assinada
        .find("</infNFe>")
        .map(|i| i + "</infNFe>".len())
        .filter(|&i| assinada[i..].starts_with("<Signature"))
        .ok_or_else(|| {
            erro(
                "a nota assinada nao tem a Signature depois do infNFe",
                "Signature",
            )
        })?;
    Ok(format!(
        "{}{supl}{}",
        &assinada[..ponto],
        &assinada[ponto..]
    ))
}

/// O texto do `DigestValue` da assinatura (base64, 28 caracteres para o SHA-1).
fn digest_value(xml: &str) -> Res<String> {
    let ini = xml
        .find("<DigestValue>")
        .map(|i| i + "<DigestValue>".len())
        .ok_or_else(|| erro("a assinatura nao tem o DigestValue", "Signature"))?;
    let fim = xml[ini..]
        .find("</DigestValue>")
        .map(|i| ini + i)
        .ok_or_else(|| erro("a assinatura nao tem o DigestValue", "Signature"))?;
    Ok(xml[ini..fim].trim().to_string())
}
