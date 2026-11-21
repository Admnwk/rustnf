// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! XMLDSig ENVELOPED da NFS-e Nacional (DPS e pedido de evento): assinar e conferir.
//!
//! A C14N e' PROPRIA (sem libxml2), sobre o `roxmltree`. Os casos `c14n` de `tests/casos/nfse_nacional.json` fixam a
//! saida octeto a octeto (inclusiva e exclusiva, com e sem comentario, namespace herdado, `xmlns=""`, CDATA,
//! `&#13;`, atributos fora de ordem), conforme a recomendacao do W3C
//! (<https://www.w3.org/TR/2001/REC-xml-c14n-20010315>). O que decide os octetos:
//! - a C14N 1.0 INCLUSIVA renderiza no apice TODAS as declaracoes de namespace em escopo -- por isso o digest e o
//!   SignedInfo se calculam com o elemento POSICIONADO no documento;
//! - texto so' escapa `& < >` (+ `\r`): quem assina a string literal com `&quot;`/`&apos;` erra o digest
//!   (a C14N escapa so' esses);
//! - assinatura sem prefixo (`<Signature xmlns=...>`); `sha256` e' o padrao; `sha1` (o das NFS-e autorizadas pela
//!   Sefin Nacional) por parametro. O XSD da NFS-e Nacional declara o algoritmo como anyURI, sem fixar um.
//!
//! Limite declarado: o prefixo de um elemento sai do `lookup_prefix` da URI (documento que liga a MESMA URI a dois
//! prefixos fica ambiguo); `xml:lang`/`xml:space` de ancestral fora do subconjunto nao sao herdados.
//! `conferir` confere integridade e a chave do certificado embutido; a cadeia ICP-Brasil e' do servidor que recebe.

use std::collections::BTreeMap;

use base64::Engine;
use roxmltree::{Document, Node, NodeId};
use rsa::pkcs1v15::{Signature, SigningKey, VerifyingKey};
use rsa::pkcs8::DecodePublicKey;
use rsa::signature::{RandomizedSigner, SignatureEncoding, Verifier};
use rsa::{PrivateKey as RsaPrivateKey, PublicKey as RsaPublicKey};
use sha1::Sha1;
use sha2::{Digest, Sha256, Sha512};

pub const DS: &str = "http://www.w3.org/2000/09/xmldsig#";
pub const C14N: &str = "http://www.w3.org/TR/2001/REC-xml-c14n-20010315";
const C14N_COM: &str = "http://www.w3.org/TR/2001/REC-xml-c14n-20010315#WithComments";
const EXC: &str = "http://www.w3.org/2001/10/xml-exc-c14n#";
const EXC_COM: &str = "http://www.w3.org/2001/10/xml-exc-c14n#WithComments";
pub const ENVELOPED: &str = "http://www.w3.org/2000/09/xmldsig#enveloped-signature";

/// Algoritmo da assinatura: (SignatureMethod, DigestMethod). O padrao e' o SHA-256.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Algoritmo {
    Sha1,
    #[default]
    Sha256,
}

impl Algoritmo {
    fn uris(self) -> (&'static str, &'static str) {
        match self {
            Algoritmo::Sha1 => (
                "http://www.w3.org/2000/09/xmldsig#rsa-sha1",
                "http://www.w3.org/2000/09/xmldsig#sha1",
            ),
            Algoritmo::Sha256 => (
                "http://www.w3.org/2001/04/xmldsig-more#rsa-sha256",
                "http://www.w3.org/2001/04/xmlenc#sha256",
            ),
        }
    }
    pub fn nome(self) -> &'static str {
        match self {
            Algoritmo::Sha1 => "sha1",
            Algoritmo::Sha256 => "sha256",
        }
    }

    /// O algoritmo pelo nome (`sha1` ou `sha256`); `None` para qualquer outro texto.
    pub fn de_nome(nome: &str) -> Option<Self> {
        match nome {
            "sha1" => Some(Algoritmo::Sha1),
            "sha256" => Some(Algoritmo::Sha256),
            _ => None,
        }
    }
}

/// XML que nao da para assinar/conferir. Mensagem segura para a tela.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErroAssinatura(pub String);

impl std::fmt::Display for ErroAssinatura {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for ErroAssinatura {}

fn erro(m: impl Into<String>) -> ErroAssinatura {
    ErroAssinatura(m.into())
}

pub fn ler(xml: &str) -> Result<Document<'_>, ErroAssinatura> {
    Document::parse(xml.strip_prefix('\u{feff}').unwrap_or(xml))
        .map_err(|e| erro(format!("XML mal formado ou com DTD ({})", e.pos())))
}

// -- C14N ---------------------------------------------------------------------------------------------------------
fn esc_texto(s: &str, out: &mut String) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\r' => out.push_str("&#xD;"),
            _ => out.push(c),
        }
    }
}

fn esc_attr(s: &str, out: &mut String) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '"' => out.push_str("&quot;"),
            '\t' => out.push_str("&#x9;"),
            '\n' => out.push_str("&#xA;"),
            '\r' => out.push_str("&#xD;"),
            _ => out.push(c),
        }
    }
}

fn prefixo(n: Node, uri: Option<&str>) -> String {
    uri.and_then(|u| n.lookup_prefix(u))
        .unwrap_or("")
        .to_string()
}

fn qname(p: &str, local: &str) -> String {
    if p.is_empty() {
        local.to_string()
    } else {
        format!("{p}:{local}")
    }
}

type Mapa = BTreeMap<String, String>;

struct C14n {
    exclusiva: bool,
    comentarios: bool,
    excluir: Option<NodeId>,
}

impl C14n {
    fn elemento(&self, n: Node, render: &Mapa, out: &mut String) {
        let em_escopo: Mapa = n
            .namespaces()
            .filter(|ns| ns.name() != Some("xml"))
            .map(|ns| (ns.name().unwrap_or("").to_string(), ns.uri().to_string()))
            .collect();
        let pfx = prefixo(n, n.tag_name().namespace());
        // candidatos: inclusiva = tudo em escopo (+ o default, para o xmlns=""); exclusiva = so' o que o elemento e
        // seus atributos USAM ("visibly utilized")
        let mut candidatos: Vec<String> = if self.exclusiva {
            let mut u = vec![pfx.clone()];
            u.extend(
                n.attributes()
                    .filter(|a| a.namespace().is_some())
                    .map(|a| prefixo(n, a.namespace())),
            );
            u
        } else {
            let mut u: Vec<String> = em_escopo.keys().cloned().collect();
            u.push(String::new());
            u
        };
        candidatos.sort();
        candidatos.dedup();
        let mut decl: Vec<(String, String)> = Vec::new();
        let mut novo = render.clone();
        for p in candidatos {
            let atual = em_escopo.get(&p).cloned().unwrap_or_default();
            // default: compara com o default ja renderizado (ausente = ""); prefixado: so' se mudou ou nunca saiu
            let precisa = if p.is_empty() {
                atual != render.get("").cloned().unwrap_or_default()
            } else {
                em_escopo.contains_key(&p) && render.get(&p) != Some(&atual)
            };
            if precisa {
                novo.insert(p.clone(), atual.clone());
                decl.push((p, atual));
            }
        }
        let nome = qname(&pfx, n.tag_name().name());
        out.push('<');
        out.push_str(&nome);
        for (p, u) in &decl {
            out.push_str(if p.is_empty() { " xmlns=\"" } else { " xmlns:" });
            if !p.is_empty() {
                out.push_str(p);
                out.push_str("=\"");
            }
            esc_attr(u, out);
            out.push('"');
        }
        let mut attrs: Vec<(String, String, String)> = n
            .attributes()
            .map(|a| {
                (
                    a.namespace().unwrap_or("").to_string(),
                    a.name().to_string(),
                    a.value().to_string(),
                )
            })
            .collect();
        attrs.sort();
        for (ns, local, v) in &attrs {
            let p = if ns.is_empty() {
                String::new()
            } else {
                prefixo(n, Some(ns))
            };
            out.push(' ');
            out.push_str(&qname(&p, local));
            out.push_str("=\"");
            esc_attr(v, out);
            out.push('"');
        }
        out.push('>');
        for c in n.children() {
            if Some(c.id()) == self.excluir {
                continue;
            }
            if c.is_element() {
                self.elemento(c, &novo, out);
            } else if c.is_text() {
                esc_texto(c.text().unwrap_or(""), out);
            } else if c.is_comment() {
                if self.comentarios {
                    out.push_str("<!--");
                    out.push_str(c.text().unwrap_or(""));
                    out.push_str("-->");
                }
            } else if let Some(pi) = c.pi() {
                out.push_str("<?");
                out.push_str(pi.target);
                if let Some(v) = pi.value.filter(|v| !v.is_empty()) {
                    out.push(' ');
                    out.push_str(v);
                }
                out.push_str("?>");
            }
        }
        out.push_str("</");
        out.push_str(&nome);
        out.push('>');
    }
}

/// C14N 1.0 de `no` JA POSICIONADO no documento (inclusiva ou exclusiva, com ou sem comentarios), opcionalmente
/// sem o no `excluir` (a propria Signature, no enveloped).
pub fn c14n(no: Node, exclusiva: bool, comentarios: bool, excluir: Option<NodeId>) -> String {
    let mut out = String::new();
    C14n {
        exclusiva,
        comentarios,
        excluir,
    }
    .elemento(no, &Mapa::new(), &mut out);
    out
}

/// Primeiro elemento de nome local `tag`.
pub fn achar<'a, 'i>(doc: &'a Document<'i>, tag: &str) -> Option<Node<'a, 'i>> {
    doc.descendants()
        .find(|n| n.is_element() && n.tag_name().name() == tag)
}

fn b64(b: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(b)
}

fn resumo(alg: &str, dados: &[u8]) -> Option<Vec<u8>> {
    Some(match alg {
        "sha1" => Sha1::digest(dados).to_vec(),
        "sha256" => Sha256::digest(dados).to_vec(),
        "sha512" => Sha512::digest(dados).to_vec(),
        _ => return None,
    })
}

// -- assinar ------------------------------------------------------------------------------------------------------
/// Assina o elemento `tag` (infDPS, infPedReg, infNFe...). A `Signature` vai como ULTIMO filho de `pai` (default: o
/// pai do elemento assinado). O texto de entrada e' preservado; so' a assinatura e' inserida.
pub fn assinar(
    xml: &str,
    chave: &RsaPrivateKey,
    cert_der: &[u8],
    tag: &str,
    algoritmo: Algoritmo,
    pai: Option<&str>,
) -> Result<String, ErroAssinatura> {
    let xml = xml.strip_prefix('\u{feff}').unwrap_or(xml);
    let (sig_uri, dig_uri) = algoritmo.uris();
    let (pos, assinatura_sem_valor, ref_id) = {
        let doc = ler(xml)?;
        let alvo = achar(&doc, tag)
            .ok_or_else(|| erro(format!("elemento <{tag}> nao encontrado no XML")))?;
        let ref_id = alvo
            .attribute("Id")
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                erro(format!(
                    "<{tag}> sem o atributo Id: a Reference da assinatura aponta para ele"
                ))
            })?;
        if doc
            .descendants()
            .filter(|n| n.attribute("Id") == Some(ref_id))
            .count()
            != 1
        {
            return Err(erro(format!(
                "Id {ref_id:?} repetido no documento: a Reference ficaria ambigua"
            )));
        }
        let ancora = match pai {
            None => alvo
                .parent_element()
                .ok_or_else(|| erro(format!("<{tag}> e' a raiz: precisa de um elemento pai")))?,
            Some(p) => achar(&doc, p)
                .ok_or_else(|| erro(format!("elemento <{p}> nao encontrado no XML")))?,
        };
        if ancora.children().any(|c| {
            c.is_element()
                && c.tag_name().name() == "Signature"
                && c.tag_name().namespace() == Some(DS)
        }) {
            return Err(erro("o documento ja tem assinatura neste ponto"));
        }
        let fim = ancora.range().end;
        let pos = xml[..fim]
            .rfind("</")
            .filter(|&p| p > ancora.range().start)
            .ok_or_else(|| erro("o elemento que recebe a assinatura nao tem tag de fechamento"))?;
        let digest = b64(
            &resumo(algoritmo.nome(), c14n(alvo, false, false, None).as_bytes())
                .expect("algoritmo conhecido"),
        );
        let s = format!(
            "<Signature xmlns=\"{DS}\"><SignedInfo><CanonicalizationMethod Algorithm=\"{C14N}\"/>\
             <SignatureMethod Algorithm=\"{sig_uri}\"/><Reference URI=\"#{ref_id}\"><Transforms>\
             <Transform Algorithm=\"{ENVELOPED}\"/><Transform Algorithm=\"{C14N}\"/></Transforms>\
             <DigestMethod Algorithm=\"{dig_uri}\"/><DigestValue>{digest}</DigestValue></Reference></SignedInfo>\
             <SignatureValue></SignatureValue><KeyInfo><X509Data><X509Certificate>{}</X509Certificate></X509Data>\
             </KeyInfo></Signature>", b64(cert_der));
        (pos, s, ref_id.to_string())
    };
    let _ = ref_id;
    let montado = format!("{}{assinatura_sem_valor}{}", &xml[..pos], &xml[pos..]);
    // o SignedInfo se assina EM CONTEXTO (ja posicionado no documento)
    let si_c14n = {
        let doc = ler(&montado)?;
        let sig = doc
            .descendants()
            .find(|n| n.is_element() && n.range().start == pos)
            .ok_or_else(|| erro("assinatura inserida nao encontrada"))?;
        let si = sig
            .children()
            .find(|c| c.is_element() && c.tag_name().name() == "SignedInfo")
            .ok_or_else(|| erro("SignedInfo ausente"))?;
        c14n(si, false, false, None)
    };
    let valor = match algoritmo {
        // COM rng: o `sign` sem rng do rsa 0.9 nao faz blinding (`pkcs1v15/signing_key.rs:161` passa None;
        // `algorithms/rsa.rs:49`). RUSTSEC-2023-0071 (Marvin) sem versao corrigida, e o rsa-sha1 continua
        // disponivel por parametro (ring/aws-lc-rs nao assinam SHA-1): blinding e' a mitigacao. PKCS#1 v1.5 e'
        // deterministico, a assinatura sai IGUAL byte a byte (o digest e a assinatura conferem em
        // `tests/nfse_assinatura.rs`).
        Algoritmo::Sha1 => SigningKey::<Sha1>::new(chave.clone())
            .sign_with_rng(&mut crate::aleatorio::sistema(), si_c14n.as_bytes())
            .to_vec(),
        Algoritmo::Sha256 => SigningKey::<Sha256>::new(chave.clone())
            .sign_with_rng(&mut crate::aleatorio::sistema(), si_c14n.as_bytes())
            .to_vec(),
    };
    let marca = "<SignatureValue></SignatureValue>";
    let i = pos
        + assinatura_sem_valor
            .find(marca)
            .expect("marca no bloco montado");
    Ok(format!(
        "{}<SignatureValue>{}</SignatureValue>{}",
        &montado[..i],
        b64(&valor),
        &montado[i + marca.len()..]
    ))
}

// -- conferir -----------------------------------------------------------------------------------------------------
/// Resultado de conferir UMA assinatura do documento.
#[derive(Debug, Clone, Default)]
pub struct Conferencia {
    pub referencia: String,
    pub ok: bool,
    pub digest_ok: bool,
    pub assinatura_ok: bool,
    /// sha1 / sha256 / sha512 da SignatureMethod.
    pub algoritmo: String,
    /// Subject do certificado embutido.
    pub titular: String,
    pub motivo: String,
    pub certificado_der: Vec<u8>,
}

fn filho<'a, 'i>(n: Node<'a, 'i>, nome: &str) -> Option<Node<'a, 'i>> {
    n.children().find(|c| {
        c.is_element() && c.tag_name().name() == nome && c.tag_name().namespace() == Some(DS)
    })
}

fn c14n_de(uri: &str) -> Option<(bool, bool)> {
    match uri {
        C14N => Some((false, false)),
        C14N_COM => Some((false, true)),
        EXC => Some((true, false)),
        EXC_COM => Some((true, true)),
        _ => None,
    }
}

fn alg_assinatura(uri: &str) -> &'static str {
    match uri {
        "http://www.w3.org/2000/09/xmldsig#rsa-sha1" => "sha1",
        "http://www.w3.org/2001/04/xmldsig-more#rsa-sha256" => "sha256",
        "http://www.w3.org/2001/04/xmldsig-more#rsa-sha512" => "sha512",
        _ => "",
    }
}

fn alg_digest(uri: &str) -> &'static str {
    match uri {
        "http://www.w3.org/2000/09/xmldsig#sha1" => "sha1",
        "http://www.w3.org/2001/04/xmlenc#sha256" => "sha256",
        "http://www.w3.org/2001/04/xmlenc#sha512" => "sha512",
        _ => "",
    }
}

fn verificar(chave: &RsaPublicKey, alg: &str, dados: &[u8], valor: &[u8]) -> bool {
    let Ok(s) = Signature::try_from(valor) else {
        return false;
    };
    match alg {
        "sha1" => VerifyingKey::<Sha1>::new(chave.clone())
            .verify(dados, &s)
            .is_ok(),
        "sha256" => VerifyingKey::<Sha256>::new(chave.clone())
            .verify(dados, &s)
            .is_ok(),
        "sha512" => VerifyingKey::<Sha512>::new(chave.clone())
            .verify(dados, &s)
            .is_ok(),
        _ => false,
    }
}

fn texto_de(n: Option<Node>) -> String {
    n.and_then(|x| x.text())
        .map(|t| t.split_whitespace().collect())
        .unwrap_or_default()
}

fn conferir_uma(doc: &Document, sig: Node) -> Conferencia {
    let mut r = Conferencia::default();
    let falha = |mut r: Conferencia, m: &str| -> Conferencia {
        r.motivo = m.into();
        r
    };
    let Some(si) = filho(sig, "SignedInfo") else {
        return falha(r, "Signature sem SignedInfo");
    };
    let (Some(rf), Some(cm), Some(sm)) = (
        filho(si, "Reference"),
        filho(si, "CanonicalizationMethod"),
        filho(si, "SignatureMethod"),
    ) else {
        return falha(r, "SignedInfo incompleto");
    };
    let uri = rf.attribute("URI").unwrap_or("");
    r.referencia = uri.trim_start_matches('#').to_string();
    r.algoritmo = alg_assinatura(sm.attribute("Algorithm").unwrap_or("")).to_string();
    let cert_b64 = texto_de(
        sig.descendants()
            .find(|n| n.is_element() && n.tag_name().name() == "X509Certificate"),
    );
    let Ok(der) = base64::engine::general_purpose::STANDARD.decode(cert_b64) else {
        return falha(r, "certificado ausente ou ilegivel no KeyInfo");
    };
    let chave = match x509_parser::parse_x509_certificate(&der) {
        Ok((_, c)) => {
            r.titular = c.subject().to_string();
            RsaPublicKey::from_public_key_der(c.tbs_certificate.subject_pki.raw).ok()
        }
        Err(_) => None,
    };
    r.certificado_der = der;
    let Some(chave) = chave else {
        return falha(r, "certificado ausente ou ilegivel no KeyInfo");
    };
    let alvo = if uri.is_empty() {
        Some(doc.root_element())
    } else if let Some(id) = uri.strip_prefix('#') {
        let achados: Vec<Node> = doc
            .descendants()
            .filter(|n| {
                n.attribute("Id") == Some(id)
                    || n.attribute("id") == Some(id)
                    || n.attribute("ID") == Some(id)
            })
            .collect();
        if achados.len() != 1 {
            return falha(r, "Reference aponta para Id inexistente ou repetido");
        }
        Some(achados[0])
    } else {
        return falha(r, "Reference externa nao e' aceita");
    };
    let alvo = alvo.expect("definido acima");
    let mut c14n_ref = (false, false);
    if let Some(tr) = filho(rf, "Transforms") {
        for t in tr.children().filter(|c| c.is_element()) {
            let a = t.attribute("Algorithm").unwrap_or("");
            if a == ENVELOPED {
                continue;
            }
            match c14n_de(a) {
                Some(x) => c14n_ref = x,
                None => return falha(r, &format!("transform nao suportado: {a}")),
            }
        }
    }
    let dig_alg = alg_digest(
        filho(rf, "DigestMethod")
            .and_then(|d| d.attribute("Algorithm"))
            .unwrap_or(""),
    );
    if dig_alg.is_empty() || r.algoritmo.is_empty() {
        return falha(r, "algoritmo nao suportado");
    }
    let dentro = sig.ancestors().any(|a| a.id() == alvo.id()) || sig.id() == alvo.id();
    let canon = c14n(
        alvo,
        c14n_ref.0,
        c14n_ref.1,
        if dentro { Some(sig.id()) } else { None },
    );
    let calculado = b64(&resumo(dig_alg, canon.as_bytes()).expect("algoritmo conhecido"));
    r.digest_ok = calculado == texto_de(filho(rf, "DigestValue"));
    let Some(c14n_si) = c14n_de(cm.attribute("Algorithm").unwrap_or("")) else {
        return falha(r, "CanonicalizationMethod nao suportado");
    };
    let valor = base64::engine::general_purpose::STANDARD
        .decode(texto_de(filho(sig, "SignatureValue")))
        .unwrap_or_default();
    r.assinatura_ok = verificar(
        &chave,
        &r.algoritmo,
        c14n(si, c14n_si.0, c14n_si.1, None).as_bytes(),
        &valor,
    );
    r.ok = r.digest_ok && r.assinatura_ok;
    if !r.ok {
        r.motivo = if !r.digest_ok {
            "o conteudo assinado foi alterado (digest nao confere)"
        } else {
            "a assinatura nao confere com o certificado embutido"
        }
        .into();
    }
    r
}

/// Confere TODAS as assinaturas do documento. Vazio = sem assinatura (ou XML ilegivel/com DTD).
pub fn conferir(xml: &str) -> Vec<Conferencia> {
    let Ok(doc) = ler(xml) else { return Vec::new() };
    doc.descendants()
        .filter(|n| {
            n.is_element()
                && n.tag_name().name() == "Signature"
                && n.tag_name().namespace() == Some(DS)
        })
        .map(|s| conferir_uma(&doc, s))
        .collect()
}

/// True se ha assinatura e TODAS conferem.
pub fn conferido(xml: &str) -> bool {
    let r = conferir(xml);
    !r.is_empty() && r.iter().all(|c| c.ok)
}
