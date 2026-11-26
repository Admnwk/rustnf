// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Os pedidos dos webservices da NF-e 4.00 e o envelope SOAP 1.2. Parte pura: nada aqui faz rede.
//!
//! * `consStatServ` (status do servico), `enviNFe` (lote de NF-e), `consReciNFe` (resultado do lote pelo recibo) e
//!   `consSitNFe` (situacao pela chave): MOC 7.00, Visao Geral, 5.5.1, 5.1.1, 5.2.1 e 5.4.1; os XSD do pacote de
//!   liberacao conferem a ordem e os tipos.
//! * A NF-e entra no `enviNFe` EXATAMENTE como veio assinada: o texto cru, sem reserializar. Qualquer byte mudado
//!   (ordem de atributo, `<X/>` no lugar de `<X></X>`, quebra de linha) quebra a assinatura do `infNFe`. A unica coisa
//!   que sai e' a declaracao `<?xml ...?>` do inicio, que nao pode ficar no meio do envelope.
//! * Envelope: SOAP 1.2, mensagem no `nfeDadosMsg` com o espaco de nomes do WSDL do servico, sem o cabecalho
//!   `nfeCabecMsg` (eliminado no leiaute 4.00; MOC 7.00, Visao Geral, 4.4.1) e a acao no `Content-Type`.

use super::erro::ErroSefaz;
use super::tabela::{codigo_da_uf, Servico};
use crate::chave_dfe::{self, Chave};
use crate::nfe::vigencias::Ambiente;
use crate::nfe::NS;

/// Versao do leiaute das mensagens.
pub const VERSAO: &str = "4.00";
/// Maximo de NF-e por lote (MOC 7.00, Visao Geral, 5.1.1).
pub const MAX_NFE_POR_LOTE: usize = 50;

const SOAP12: &str = "http://www.w3.org/2003/05/soap-envelope";

/// `1` producao, `2` homologacao (o `tpAmb` do leiaute).
#[must_use]
pub fn tp_amb(ambiente: Ambiente) -> char {
    match ambiente {
        Ambiente::Producao => '1',
        Ambiente::Homologacao => '2',
    }
}

/// O XML sem o BOM e sem a declaracao `<?xml ...?>`, e sem espaco nas pontas.
#[must_use]
pub fn sem_declaracao(xml: &str) -> &str {
    let s = xml.trim_start_matches('\u{feff}').trim_start();
    let s = match s.strip_prefix("<?xml") {
        Some(_) => match s.find("?>") {
            Some(i) => s[i + 2..].trim_start(),
            None => s,
        },
        None => s,
    };
    s.trim_end()
}

/// `consStatServ`: o status do servico da UF.
///
/// # Errors
///
/// [`ErroSefaz`] se a UF e' desconhecida.
pub fn cons_stat(ambiente: Ambiente, uf: &str) -> Result<String, ErroSefaz> {
    let cuf = codigo_da_uf(uf)?;
    Ok(format!(
        "<consStatServ xmlns=\"{NS}\" versao=\"{VERSAO}\"><tpAmb>{}</tpAmb><cUF>{cuf}</cUF>\
         <xServ>STATUS</xServ></consStatServ>",
        tp_amb(ambiente)
    ))
}

/// `consReciNFe`: o resultado do lote pelo recibo (`nRec` de 15 digitos).
///
/// # Errors
///
/// [`ErroSefaz`] se o recibo nao tem 15 digitos.
pub fn cons_reci(ambiente: Ambiente, recibo: &str) -> Result<String, ErroSefaz> {
    let r = recibo.trim();
    if r.len() != 15 || !r.bytes().all(|b| b.is_ascii_digit()) {
        return Err(ErroSefaz::novo("o recibo (nRec) tem 15 digitos", "nRec"));
    }
    Ok(format!(
        "<consReciNFe xmlns=\"{NS}\" versao=\"{VERSAO}\"><tpAmb>{}</tpAmb><nRec>{r}</nRec></consReciNFe>",
        tp_amb(ambiente)
    ))
}

/// A chave de uma NF-e (modelo 55) valida: 44 posicoes, digito verificador e CPF/CNPJ do emitente conferidos.
///
/// # Errors
///
/// [`ErroSefaz`] (campo `chNFe`) com o defeito achado.
pub fn chave_valida(chave: &str) -> Result<Chave, ErroSefaz> {
    let c = chave_dfe::decompor(chave.trim(), true)
        .map_err(|e| ErroSefaz::novo(format!("chave de acesso invalida: {e}"), "chNFe"))?;
    if let Some(e) = &c.erro {
        return Err(ErroSefaz::novo(
            format!("chave de acesso invalida: {e}"),
            "chNFe",
        ));
    }
    if c.modelo != "55" {
        return Err(ErroSefaz::novo(
            format!(
                "a chave e' do modelo {}: este cliente e' da NF-e (modelo 55)",
                c.modelo
            ),
            "chNFe",
        ));
    }
    Ok(c)
}

/// `consSitNFe`: a situacao da NF-e pela chave.
///
/// # Errors
///
/// [`ErroSefaz`] se a chave nao e' de uma NF-e valida.
pub fn cons_sit(ambiente: Ambiente, chave: &str) -> Result<String, ErroSefaz> {
    let c = chave_valida(chave)?.chave;
    Ok(format!(
        "<consSitNFe xmlns=\"{NS}\" versao=\"{VERSAO}\"><tpAmb>{}</tpAmb><xServ>CONSULTAR</xServ>\
         <chNFe>{c}</chNFe></consSitNFe>",
        tp_amb(ambiente)
    ))
}

/// A NF-e que vai ser enviada, conferida: o texto cru (sem a declaracao `<?xml ...?>`) e o que a chave e o `ide` dizem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NfeConferida<'a> {
    /// O XML da `NFe` como veio, sem a declaracao.
    pub texto: &'a str,
    /// A chave do `infNFe/@Id`, ja decomposta e valida.
    pub chave: Chave,
    /// `ide/tpAmb` da nota (`1` ou `2`).
    pub tp_amb: String,
}

fn filho<'a, 'i>(
    pai: roxmltree::Node<'a, 'i>,
    nome: &str,
    ns: &str,
) -> Option<roxmltree::Node<'a, 'i>> {
    pai.children().find(|c| {
        c.is_element() && c.tag_name().name() == nome && c.tag_name().namespace() == Some(ns)
    })
}

/// Confere a NF-e assinada antes de qualquer envio: e' um XML bem formado sem DTD, cuja raiz e' a `NFe` do leiaute
/// 4.00 e nada mais (nenhum comentario ou elemento solto antes ou depois), tem o `infNFe` com `Id` de chave valida do
/// modelo 55 e a `Signature` no fim. Nao reescreve nada: devolve o proprio texto.
///
/// Sem esta conferencia o texto cru iria para dentro do envelope como estivesse, e um XML com elemento a mais depois
/// da `NFe` passaria a fazer parte da mensagem.
///
/// # Errors
///
/// [`ErroSefaz`] (campo `NFe`, `infNFe`, `Signature` ou `chNFe`) com o que falta ou sobra.
pub fn conferir_nfe(nfe: &str) -> Result<NfeConferida<'_>, ErroSefaz> {
    let texto = sem_declaracao(nfe);
    let doc = roxmltree::Document::parse(texto)
        .map_err(|_| ErroSefaz::novo("a NF-e nao e' um XML bem formado", "NFe"))?;
    let raiz = doc.root_element();
    if raiz.tag_name().name() != "NFe"
        || raiz.tag_name().namespace() != Some(NS)
        || raiz.range() != (0..texto.len())
    {
        return Err(ErroSefaz::novo(
            "o lote recebe o XML da NFe assinada: a raiz tem de ser <NFe> do leiaute 4.00 e nada mais",
            "NFe",
        ));
    }
    let inf = filho(raiz, "infNFe", NS)
        .ok_or_else(|| ErroSefaz::novo("a NF-e nao tem o grupo infNFe", "infNFe"))?;
    let id = inf.attribute("Id").unwrap_or("");
    let chave = id
        .strip_prefix("NFe")
        .ok_or_else(|| ErroSefaz::novo("infNFe sem o atributo Id (NFe + chave)", "infNFe"))?;
    let chave = chave_valida(chave)?;
    if filho(raiz, "Signature", crate::assinatura::xmldsig::DS).is_none() {
        return Err(ErroSefaz::novo(
            "a NF-e precisa estar assinada",
            "Signature",
        ));
    }
    let tp_amb = filho(inf, "ide", NS)
        .and_then(|ide| filho(ide, "tpAmb", NS))
        .and_then(|t| t.text())
        .unwrap_or("")
        .trim()
        .to_string();
    Ok(NfeConferida {
        texto,
        chave,
        tp_amb,
    })
}

/// `enviNFe`: o lote com as NF-e assinadas, cada uma no texto em que veio.
///
/// Sincrono (`indSinc` 1, o padrao) leva UMA nota: a SEFAZ rejeita o lote sincrono com mais de uma (rejeicao 764,
/// MOC 7.00, Anexo I, regra GAP03a-1). O lote tem de 1 a 50 notas (Visao Geral, 5.1.1). O `idLote` e' o numero de
/// controle do contribuinte (1 a 15 digitos).
///
/// # Errors
///
/// [`ErroSefaz`] se o lote esta vazio ou passa de 50, se o sincrono tem mais de uma nota, se o `idLote` esta fora da
/// forma ou se alguma NF-e nao passa em [`conferir_nfe`].
pub fn envi_nfe(nfes: &[&str], id_lote: &str, sincrono: bool) -> Result<String, ErroSefaz> {
    if nfes.is_empty() {
        return Err(ErroSefaz::novo("lote vazio", "NFe"));
    }
    if nfes.len() > MAX_NFE_POR_LOTE {
        return Err(ErroSefaz::novo(
            format!("lote com mais de {MAX_NFE_POR_LOTE} NF-e"),
            "NFe",
        ));
    }
    if sincrono && nfes.len() > 1 {
        return Err(ErroSefaz::novo(
            "o envio sincrono leva uma nota por lote (rejeicao 764)",
            "indSinc",
        ));
    }
    let id = id_lote.trim();
    if id.is_empty() || id.len() > 15 || !id.bytes().all(|b| b.is_ascii_digit()) {
        return Err(ErroSefaz::novo("idLote: de 1 a 15 digitos", "idLote"));
    }
    let mut corpo = String::new();
    for n in nfes {
        corpo.push_str(conferir_nfe(n)?.texto);
    }
    Ok(format!(
        "<enviNFe xmlns=\"{NS}\" versao=\"{VERSAO}\"><idLote>{id}</idLote><indSinc>{}</indSinc>{corpo}</enviNFe>",
        u8::from(sincrono)
    ))
}

/// A mensagem SOAP pronta para sair: o corpo e o `Content-Type`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Envelope {
    /// O envelope SOAP 1.2, em UTF-8.
    pub corpo: Vec<u8>,
    /// `application/soap+xml; charset=utf-8; action="..."`.
    pub content_type: String,
}

/// O envelope SOAP 1.2 do servico: `<nfeDadosMsg xmlns="<WSDL do servico>">` com o pedido dentro, sem cabecalho.
/// O pedido entra como esta (sem a declaracao `<?xml ...?>`).
///
/// A distribuicao de DF-e e' a excecao: o WSDL dela declara a operacao `nfeDistDFeInteresse` e o `nfeDadosMsg` vai
/// DENTRO dela (NT 2014.002, item 3), entao o pedido sai embrulhado nessa operacao.
#[must_use]
pub fn envelope(dados: &str, servico: Servico) -> Envelope {
    let ns = servico.namespace();
    let pedido = sem_declaracao(dados);
    let miolo = if servico == Servico::DistribuicaoDfe {
        format!(
            "<nfeDistDFeInteresse xmlns=\"{ns}\"><nfeDadosMsg>{pedido}</nfeDadosMsg></nfeDistDFeInteresse>"
        )
    } else {
        format!("<nfeDadosMsg xmlns=\"{ns}\">{pedido}</nfeDadosMsg>")
    };
    let corpo = format!(
        "<soap12:Envelope xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" \
         xmlns:xsd=\"http://www.w3.org/2001/XMLSchema\" xmlns:soap12=\"{SOAP12}\"><soap12:Body>\
         {miolo}</soap12:Body></soap12:Envelope>"
    );
    Envelope {
        corpo: corpo.into_bytes(),
        content_type: format!(
            "application/soap+xml; charset=utf-8; action=\"{}\"",
            servico.acao()
        ),
    }
}
