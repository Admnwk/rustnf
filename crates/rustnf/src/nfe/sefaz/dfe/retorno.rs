// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A leitura do `retDistDFeInt`: o que o Ambiente Nacional respondeu, antes de abrir os documentos.
//!
//! Fonte: NT 2014.002 v1.02d, item 3.2 (leiaute da mensagem de retorno) e `retDistDFeInt_v1.01.xsd`.

use super::super::resposta::{abrir, filhos, texto_filho, ErroResposta};

/// Um `docZip` como veio: o NSU, o esquema que o descreve e o conteudo (gzip em base64), sem abrir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DocZip {
    /// O atributo `NSU`, como veio.
    pub(crate) nsu: String,
    /// O atributo `schema` (por exemplo `resNFe_v1.01.xsd`).
    pub(crate) esquema: String,
    /// O conteudo, em base64, como veio.
    pub(crate) base64: String,
}

/// O retorno da distribuicao.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RetDistDfe {
    /// `cStat`.
    pub(crate) cstat: String,
    /// `xMotivo`.
    pub(crate) motivo: String,
    /// `ultNSU`, como veio (vazio se faltou).
    pub(crate) ult_nsu: String,
    /// `maxNSU`, como veio (vazio se faltou).
    pub(crate) max_nsu: String,
    /// Os `docZip`, na ordem da resposta.
    pub(crate) docs: Vec<DocZip>,
}

/// Le o `retDistDFeInt` (o texto que a SEFAZ mandou, ja tirado do envelope SOAP).
///
/// # Errors
///
/// [`ErroResposta`] se o texto nao e' um `retDistDFeInt`.
pub(crate) fn ler_ret_dist_dfe(texto: &str) -> Result<RetDistDfe, ErroResposta> {
    let doc = abrir(texto, "retDistDFeInt")?;
    let raiz = doc.root_element();
    let docs = filhos(raiz, "loteDistDFeInt")
        .flat_map(|lote| filhos(lote, "docZip"))
        .map(|z| DocZip {
            nsu: z.attribute("NSU").unwrap_or("").trim().to_string(),
            esquema: z.attribute("schema").unwrap_or("").trim().to_string(),
            base64: z.text().unwrap_or("").to_string(),
        })
        .collect();
    Ok(RetDistDfe {
        cstat: texto_filho(raiz, "cStat"),
        motivo: texto_filho(raiz, "xMotivo"),
        ult_nsu: texto_filho(raiz, "ultNSU"),
        max_nsu: texto_filho(raiz, "maxNSU"),
        docs,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod testes {
    use super::*;

    #[test]
    fn le_o_status_os_nsu_e_os_docs() {
        let x = "<retDistDFeInt xmlns=\"http://www.portalfiscal.inf.br/nfe\" versao=\"1.01\"><tpAmb>2</tpAmb>\
                 <verAplic>AN_1.0</verAplic><cStat>138</cStat><xMotivo>Documento(s) localizado(s)</xMotivo>\
                 <dhResp>2026-11-26T09:00:00-03:00</dhResp><ultNSU>000000000000002</ultNSU>\
                 <maxNSU>000000000000005</maxNSU><loteDistDFeInt>\
                 <docZip NSU=\"000000000000001\" schema=\"resNFe_v1.01.xsd\">QUJD</docZip>\
                 <docZip NSU=\"000000000000002\" schema=\"resEvento_v1.01.xsd\">REVG</docZip>\
                 </loteDistDFeInt></retDistDFeInt>";
        let r = ler_ret_dist_dfe(x).unwrap();
        assert_eq!(
            (r.cstat.as_str(), r.ult_nsu.as_str(), r.max_nsu.as_str()),
            ("138", "000000000000002", "000000000000005")
        );
        assert_eq!(r.docs.len(), 2);
        assert_eq!(r.docs[1].esquema, "resEvento_v1.01.xsd");
        assert_eq!(r.docs[0].base64, "QUJD");
        assert!(ler_ret_dist_dfe("<outra/>").is_err());
    }
}
