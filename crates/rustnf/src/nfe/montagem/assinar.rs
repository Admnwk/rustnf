// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Assinatura do `infNFe` com o certificado A1.

use crate::assinatura::cert_a1::Certificado;
use crate::assinatura::xmldsig::{self, Algoritmo};

use super::formato::{erro, erro_r, Res};
use super::modelo::Montada;

/// Assina o `infNFe` da nota montada e devolve o XML da `NFe` com a `Signature` como ultimo filho.
///
/// O XSD da NF-e 4.00 (`xmldsig-core-schema_v1.01.xsd`) fixa `rsa-sha1` como `SignatureMethod` e `sha1` como
/// `DigestMethod`; por isso o algoritmo e' SHA-1, nao o SHA-256 que a NFS-e Nacional usa. A canonicalizacao e' a
/// C14N 1.0 do mesmo XSD.
///
/// # Errors
///
/// [`super::ErroMontagem`] (campo `Signature`) se o XML nao puder ser assinado.
pub fn assinar(m: &Montada, cert: &Certificado) -> Res<String> {
    if m.xml.contains("<tpImp>6</tpImp>") {
        return Err(erro_r(
            "tpImp 6 (DANFE simplificado) exige o QR Code (infNFeSupl), que ainda nao e' montado: a nota nao foi assinada",
            "tpImp",
            "",
        ));
    }
    xmldsig::assinar(
        &m.xml,
        &cert.chave,
        &cert.cert_der,
        "infNFe",
        Algoritmo::Sha1,
        None,
    )
    .map_err(|e| erro(e.0, "Signature"))
}
