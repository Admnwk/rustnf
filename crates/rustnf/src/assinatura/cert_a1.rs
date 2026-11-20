// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Certificado A1 ICP-Brasil (.pfx/.p12): carregar, titular pelo campo oficial, vencimento, Testar e a identidade
//! mTLS EM MEMORIA (PEM para `reqwest::Identity::from_pem`, nada no disco).
//!
//! PKCS#12 pelo `p12-keystore` 0.3 (PBES2/AES, 3DES e RC2). CNPJ/CPF pelo otherName 2.16.76.1.3.3 / 2.16.76.1.3.1
//! (DOC-ICP-04); o CN "NOME:DOCUMENTO" fica de reserva.
//!
//! Norma em mudanca (Res. CG/ICP-Brasil 211/2024): o tipo A1 foi extinto (art. 3;
//! emissao na cadeia V5 ate 02/03/2029, art. 13); o selo eletronico SE-S e' o sucessor para PJ (art. 2); CPF/CNPJ vao
//! para o `serialNumber` 2.5.4.5 do DN (art. 8); os otherName 2.16.76.1.3.1/.3.3 sao recomendados so' ate 31/12/2028
//! e o 2.16.76.1.3.4 foi extinto (art. 7). A leitura do serialNumber ainda nao esta implementada.

use chrono::{DateTime, TimeZone, Utc};
use rsa::pkcs8::{DecodePrivateKey, EncodePrivateKey, LineEnding};
use rsa::PrivateKey as RsaPrivateKey;
use zeroize::Zeroizing;

pub const OID_CNPJ: &str = "2.16.76.1.3.3";
pub const OID_DADOS_PF: &str = "2.16.76.1.3.1";
/// Extinto pela Res. CG/ICP-Brasil 211/2024 art. 7 (certificado novo pode nao trazer).
pub const OID_DADOS_RESPONSAVEL: &str = "2.16.76.1.3.4";
pub const AVISO_DIAS: i64 = 30;
/// Emissor do `gerar_teste(.., icp_brasil=true, ..)`.
pub const AC_TESTE: &str = "AC TESTE RUSTNF (sem validade)";

/// Certificado que nao abre. Mensagem para a tela; o detalhe tecnico nao sai daqui.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErroCertificado(pub String);

impl std::fmt::Display for ErroCertificado {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for ErroCertificado {}

/// Resultado do botao Testar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Teste {
    pub ok: bool,
    pub frase: String,
    pub avisos: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Situacao {
    Valido,
    VenceEmBreve,
    Vencido,
    AindaNaoValido,
}

#[derive(Clone)]
pub struct Certificado {
    pub chave: RsaPrivateKey,
    pub cert_der: Vec<u8>,
    /// Intermediarias que vieram no .pfx (vao junto no mTLS).
    pub cadeia_der: Vec<Vec<u8>>,
    pub titular: String,
    pub cnpj: String,
    pub cpf: String,
    pub cpf_responsavel: String,
    pub emissor: String,
    pub icp_brasil: bool,
    pub serie: String,
    pub validade_inicio: DateTime<Utc>,
    pub validade_fim: DateTime<Utc>,
    pub documento_pelo_cn: bool,
}

impl std::fmt::Debug for Certificado {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Certificado")
            .field("titular", &self.titular)
            .field("cnpj", &self.cnpj)
            .field("cpf", &self.cpf)
            .field("validade_fim", &self.validade_fim)
            .finish_non_exhaustive()
    }
}

/// Conteudo de string DER (OCTET/UTF8/Printable/IA5), com ou sem o [0] explicito do otherName.
fn der_texto(mut b: &[u8]) -> String {
    for _ in 0..3 {
        if b.len() >= 2 && matches!(b[0], 0x04 | 0x0C | 0x13 | 0x16 | 0xA0) {
            let (n, i) = if b[1] & 0x80 != 0 {
                let k = (b[1] & 0x7F) as usize;
                (
                    b.get(2..2 + k)
                        .map_or(0, |x| x.iter().fold(0usize, |a, &y| (a << 8) | y as usize)),
                    2 + k,
                )
            } else {
                (b[1] as usize, 2)
            };
            b = b.get(i..(i + n).min(b.len())).unwrap_or(&[]);
            continue;
        }
        break;
    }
    b.iter().map(|&c| c as char).collect()
}

fn alnum_up(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

/// CPF '123.***.***-09', CNPJ '11.***.***/****-81'; outro = '***'.
fn mascarar_doc(doc: &str) -> String {
    let c = alnum_up(doc);
    match c.len() {
        11 => format!("{}.***.***-{}", &c[..3], &c[9..]),
        14 => format!("{}.***.***/****-{}", &c[..2], &c[12..]),
        _ => "***".into(),
    }
}

fn digitos(s: &str) -> String {
    s.chars().filter(char::is_ascii_digit).collect()
}

fn fatia(s: &str, a: usize, b: usize) -> &str {
    s.get(a..b.min(s.len())).unwrap_or("")
}

fn doc_do_cn(cn: &str) -> String {
    let Some((_, d)) = cn.rsplit_once(':') else {
        return String::new();
    };
    let d = alnum_up(d);
    if (d.len() == 11 && d.chars().all(|c| c.is_ascii_digit())) || d.len() == 14 {
        d
    } else {
        String::new()
    }
}

fn pem(rotulo: &str, der: &[u8]) -> String {
    use base64::Engine;
    let b = base64::engine::general_purpose::STANDARD.encode(der);
    let mut s = format!("-----BEGIN {rotulo}-----\n");
    for l in b.as_bytes().chunks(64) {
        s.push_str(std::str::from_utf8(l).unwrap_or_default());
        s.push('\n');
    }
    s.push_str(&format!("-----END {rotulo}-----\n"));
    s
}

impl Certificado {
    pub fn documento(&self) -> &str {
        if self.cnpj.is_empty() {
            &self.cpf
        } else {
            &self.cnpj
        }
    }

    pub fn dias_restantes(&self, agora: DateTime<Utc>) -> i64 {
        (self.validade_fim - agora).num_days()
    }

    pub fn situacao(&self, agora: DateTime<Utc>, aviso_dias: i64) -> Situacao {
        if agora < self.validade_inicio {
            Situacao::AindaNaoValido
        } else if agora >= self.validade_fim {
            Situacao::Vencido
        } else if self.dias_restantes(agora) <= aviso_dias {
            Situacao::VenceEmBreve
        } else {
            Situacao::Valido
        }
    }

    /// Botao Testar: pode assinar hoje? e' desta empresa? Nao fala com a rede. Frase de tela: documento MASCARADO.
    pub fn testar(
        &self,
        cnpj_empresa: Option<&str>,
        agora: DateTime<Utc>,
        aviso_dias: i64,
    ) -> Teste {
        let fim = self
            .validade_fim
            .with_timezone(&crate_brasilia())
            .format("%d/%m/%Y")
            .to_string();
        match self.situacao(agora, aviso_dias) {
            Situacao::Vencido => return Teste { ok: false, frase: format!("Certificado VENCIDO em {fim}. Renove o A1 com a certificadora."), avisos: vec![] },
            Situacao::AindaNaoValido => return Teste { ok: false, frase: "O certificado ainda nao comecou a valer (confira a data e a hora do computador).".into(), avisos: vec![] },
            _ => {}
        }
        let mut avisos = Vec::new();
        if self.situacao(agora, aviso_dias) == Situacao::VenceEmBreve {
            avisos.push(format!("O certificado vence em {} dia(s), em {fim}. Providencie a renovacao antes: vencido, ele nao assina.",
                                self.dias_restantes(agora)));
        }
        if !self.icp_brasil {
            avisos.push("Este certificado nao e' da ICP-Brasil: serve so' para teste (sem validade juridica; o fisco recusa).".into());
        }
        let alvo = alnum_up(cnpj_empresa.unwrap_or(""));
        if !alvo.is_empty() && !self.documento().is_empty() && self.documento() != alvo {
            // a assinatura vale pela RAIZ do CNPJ (Manual Integrado do Sistema Nacional NFS-e v1.00.02,
            // item 6.1.3 b, E0718)
            if alvo.len() == 14 && self.cnpj.len() == 14 && self.cnpj[..8] == alvo[..8] {
                avisos.push(
                    "O certificado e' de outra filial da mesma empresa (raiz do CNPJ igual): a NFS-e aceita, a regra \
                     e' a raiz."
                        .into(),
                );
            } else {
                avisos.push(
                    "O certificado e' de outro documento (CNPJ/CPF) que nao o da empresa configurada: a nota assinada \
                     com ele volta recusada (NFS-e E0718)."
                        .into(),
                );
            }
        }
        let quem = if self.titular.is_empty() {
            "titular sem nome"
        } else {
            &self.titular
        };
        // a frase vai para a TELA: documento mascarado
        let doc = if self.documento().is_empty() {
            String::new()
        } else {
            format!(" ({})", mascarar_doc(self.documento()))
        };
        Teste {
            ok: true,
            frase: format!("Certificado OK: {quem}{doc}, valido ate {fim}."),
            avisos,
        }
    }

    /// Titular + cadeia em PEM (publico).
    pub fn pem_certificados(&self) -> String {
        std::iter::once(&self.cert_der)
            .chain(&self.cadeia_der)
            .map(|d| pem("CERTIFICATE", d))
            .collect()
    }

    /// Chave (PKCS#8) + certificados em PEM, EM MEMORIA, para `reqwest::Identity::from_pem` (rustls). Nada vai
    /// para o disco -- nunca grave a chave sem senha em arquivo temporario. O retorno e' zerado ao ser solto.
    pub fn identidade_pem(&self) -> Zeroizing<Vec<u8>> {
        let certificados = self.pem_certificados();
        let chave = self.chave.to_pkcs8_pem(LineEnding::LF).ok();
        let tamanho = chave.as_ref().map_or(0, |c| c.len()) + certificados.len();
        // capacidade exata: o vetor nao realoca, entao nao sobra copia da chave para tras
        let mut pem = Zeroizing::new(Vec::with_capacity(tamanho));
        if let Some(c) = &chave {
            pem.extend_from_slice(c.as_bytes());
        }
        pem.extend_from_slice(certificados.as_bytes());
        pem
    }
}

fn cn_de(n: &x509_parser::x509::X509Name) -> String {
    let v = n
        .iter_common_name()
        .next()
        .and_then(|a| a.as_str().ok())
        .unwrap_or("")
        .to_string();
    v
}

fn crate_brasilia() -> chrono::FixedOffset {
    chrono::FixedOffset::west_opt(3 * 3600).expect("fuso fixo")
}

/// Abre o A1 a partir dos BYTES do .pfx/.p12 (leia o arquivo antes). Erro = frase de tela.
pub fn carregar(pfx: &[u8], senha: &str) -> Result<Certificado, ErroCertificado> {
    use p12_keystore::{KeyStore, Pkcs12ImportPolicy};
    if pfx.is_empty() {
        return Err(ErroCertificado(
            "O arquivo do certificado esta vazio.".into(),
        ));
    }
    let ks = KeyStore::from_pkcs12(pfx, senha, Pkcs12ImportPolicy::Relaxed).map_err(|e| {
        ErroCertificado(match e {
            p12_keystore::error::Error::MacError(_) => "Senha do certificado incorreta.".into(),
            _ => "O arquivo nao e' um certificado A1 valido (.pfx/.p12).".into(),
        })
    })?;
    let (_, cadeia) = ks.private_key_chain().ok_or_else(|| {
        ErroCertificado(
            "O arquivo nao traz a chave privada junto do certificado (exporte o A1 com a chave)."
                .into(),
        )
    })?;
    let chave = RsaPrivateKey::from_pkcs8_der(cadeia.key().as_der()).map_err(|_| {
        ErroCertificado("A chave do certificado nao e' RSA (o padrao do A1 ICP-Brasil).".into())
    })?;
    let certs = cadeia.certs();
    let titular_der = certs
        .first()
        .ok_or_else(|| ErroCertificado("O arquivo nao traz o certificado do titular.".into()))?
        .as_der()
        .to_vec();
    montar(
        chave,
        titular_der,
        certs.iter().skip(1).map(|x| x.as_der().to_vec()).collect(),
    )
}

/// Abre o A1 guardado em PEM (cofre que salva chave + certificados em texto): chave `PRIVATE KEY` (PKCS#8) ou
/// `RSA PRIVATE KEY` (PKCS#1) + um ou mais `CERTIFICATE`, em qualquer ordem. O titular e' o certificado da chave
/// (mesmo modulo RSA); os outros viram a cadeia. CNPJ/CPF pelo otherName, igual ao `carregar`. Chave cifrada
/// (`ENCRYPTED PRIVATE KEY`) nao abre aqui: decifre no cofre. Erro = frase de tela.
pub fn de_pem(pem: &str) -> Result<Certificado, ErroCertificado> {
    use base64::Engine;
    use rsa::pkcs1::DecodeRsaPrivateKey;
    use rsa::traits::PublicKeyParts;
    let ilegivel = || ErroCertificado("O texto PEM do certificado esta ilegivel.".into());
    let mut chave = None;
    let mut ders = Vec::new();
    let mut resto = pem;
    while let Some(i) = resto.find("-----BEGIN ") {
        let r = &resto[i + 11..];
        let f = r.find("-----").ok_or_else(ilegivel)?;
        let rotulo = r[..f].trim().to_string();
        let fim = format!("-----END {rotulo}-----");
        let j = r.find(&fim).ok_or_else(ilegivel)?;
        let corpo: String = r[f + 5..j].chars().filter(|c| !c.is_whitespace()).collect();
        resto = &r[j + fim.len()..];
        let der = base64::engine::general_purpose::STANDARD
            .decode(corpo.as_bytes())
            .map_err(|_| ilegivel())?;
        match rotulo.as_str() {
            "CERTIFICATE" => ders.push(der),
            "PRIVATE KEY" => {
                chave = Some(RsaPrivateKey::from_pkcs8_der(&der).map_err(|_| {
                    ErroCertificado(
                        "A chave do certificado nao e' RSA (o padrao do A1 ICP-Brasil).".into(),
                    )
                })?)
            }
            "RSA PRIVATE KEY" => {
                chave = Some(RsaPrivateKey::from_pkcs1_der(&der).map_err(|_| ilegivel())?)
            }
            "ENCRYPTED PRIVATE KEY" => {
                return Err(ErroCertificado(
                    "A chave do certificado esta cifrada no PEM: decifre antes de abrir.".into(),
                ))
            }
            _ => {}
        }
    }
    let chave = chave
        .ok_or_else(|| ErroCertificado("O PEM nao traz a chave privada do certificado.".into()))?;
    if ders.is_empty() {
        return Err(ErroCertificado(
            "O PEM nao traz o certificado do titular.".into(),
        ));
    }
    let n = chave.n().as_ref().to_be_bytes();
    let da_chave = |der: &Vec<u8>| {
        x509_parser::parse_x509_certificate(der)
            .ok()
            .is_some_and(|(_, c)| match c.public_key().parsed() {
                Ok(x509_parser::public_key::PublicKey::RSA(k)) => k
                    .modulus
                    .iter()
                    .skip_while(|b| **b == 0)
                    .eq(n.iter().skip_while(|b| **b == 0)),
                _ => false,
            })
    };
    let i = ders.iter().position(da_chave).ok_or_else(|| {
        ErroCertificado("Nenhum certificado do PEM e' o da chave privada.".into())
    })?;
    let titular_der = ders.remove(i);
    montar(chave, titular_der, ders)
}

fn montar(
    chave: RsaPrivateKey,
    titular_der: Vec<u8>,
    cadeia_der: Vec<Vec<u8>>,
) -> Result<Certificado, ErroCertificado> {
    let (_, c) = x509_parser::parse_x509_certificate(&titular_der)
        .map_err(|_| ErroCertificado("O certificado do titular esta ilegivel.".into()))?;
    let mut nomes = std::collections::BTreeMap::new();
    if let Ok(Some(san)) = c.subject_alternative_name() {
        for g in &san.value.general_names {
            if let x509_parser::extensions::GeneralName::OtherName(oid, bytes) = g {
                nomes.insert(oid.to_id_string(), der_texto(bytes));
            }
        }
    }
    let cn = cn_de(c.subject());
    let mut cnpj: String = alnum_up(nomes.get(OID_CNPJ).map_or("", String::as_str))
        .chars()
        .take(14)
        .collect();
    if cnpj.len() != 14 {
        cnpj.clear();
    }
    let pf = digitos(fatia(
        nomes.get(OID_DADOS_PF).map_or("", String::as_str),
        8,
        19,
    ));
    let resp = digitos(fatia(
        nomes.get(OID_DADOS_RESPONSAVEL).map_or("", String::as_str),
        8,
        19,
    ));
    let mut cpf = if pf.len() == 11 && cnpj.is_empty() {
        pf
    } else {
        String::new()
    };
    let mut pelo_cn = false;
    if cnpj.is_empty() && cpf.is_empty() {
        let d = doc_do_cn(&cn);
        if !d.is_empty() {
            pelo_cn = true;
            if d.len() == 14 {
                cnpj = d
            } else {
                cpf = d
            }
        }
    }
    let orgs: Vec<String> = c
        .subject()
        .iter_organization()
        .chain(c.issuer().iter_organization())
        .filter_map(|a| a.as_str().ok().map(String::from))
        .collect();
    let ts = |t: i64| Utc.timestamp_opt(t, 0).single().unwrap_or_default();
    Ok(Certificado {
        chave,
        cadeia_der,
        titular: cn.split(':').next().unwrap_or("").trim().to_string(),
        cnpj,
        cpf,
        cpf_responsavel: if resp.len() == 11 {
            resp
        } else {
            String::new()
        },
        emissor: cn_de(c.issuer()),
        // carimbo, nao cadeia: autoassinado (emissor == titular, bytes do DER) nunca e' ICP
        // (um carimbo escrito a mao no titular nao basta)
        icp_brasil: orgs.iter().any(|o| o == "ICP-Brasil")
            && c.subject().as_raw() != c.issuer().as_raw(),
        serie: c.raw_serial_as_string().replace(':', ""),
        validade_inicio: ts(c.validity().not_before.timestamp()),
        validade_fim: ts(c.validity().not_after.timestamp()),
        documento_pelo_cn: pelo_cn,
        cert_der: titular_der.clone(),
    })
}

/// Testar da tela sem levantar.
pub fn testar(pfx: &[u8], senha: &str, cnpj_empresa: Option<&str>, agora: DateTime<Utc>) -> Teste {
    match carregar(pfx, senha) {
        Ok(c) => c.testar(cnpj_empresa, agora, AVISO_DIAS),
        Err(e) => Teste {
            ok: false,
            frase: e.0,
            avisos: vec![],
        },
    }
}

/// Perfil de folha autoassinada do gerador de teste: o que o `Profile::Leaf` do x509-cert 0.2.5 montava
/// (`x509-cert-0.2.5/src/builder.rs:126-213`: SKI, AKI, BasicConstraints ca=false, KeyUsage
/// digitalSignature|nonRepudiation|keyEncipherment). O 0.3 nao traz mais perfil de folha generico.
#[cfg(feature = "cert_a1_teste")]
struct Folha {
    titular: x509_cert::name::Name,
    emissor: x509_cert::name::Name,
}

#[cfg(feature = "cert_a1_teste")]
impl x509_cert::builder::profile::BuilderProfile for Folha {
    fn get_issuer(&self, _subject: &x509_cert::name::Name) -> x509_cert::name::Name {
        self.emissor.clone()
    }

    fn get_subject(&self) -> x509_cert::name::Name {
        self.titular.clone()
    }

    fn build_extensions(
        &self,
        spk: x509_cert::spki::SubjectPublicKeyInfoRef<'_>,
        issuer_spk: x509_cert::spki::SubjectPublicKeyInfoRef<'_>,
        tbs: &x509_cert::certificate::TbsCertificate,
    ) -> x509_cert::builder::Result<Vec<x509_cert::ext::Extension>> {
        use x509_cert::ext::pkix::{
            AuthorityKeyIdentifier, BasicConstraints, KeyUsage, KeyUsages, SubjectKeyIdentifier,
        };
        use x509_cert::ext::ToExtension;
        let sujeito = tbs.subject();
        let mut ext = Vec::new();
        ext.push(SubjectKeyIdentifier::try_from(spk)?.to_extension(sujeito, &ext)?);
        ext.push(AuthorityKeyIdentifier::try_from(issuer_spk)?.to_extension(sujeito, &ext)?);
        ext.push(
            BasicConstraints {
                ca: false,
                path_len_constraint: None,
            }
            .to_extension(sujeito, &ext)?,
        );
        let uso =
            KeyUsages::DigitalSignature | KeyUsages::NonRepudiation | KeyUsages::KeyEncipherment;
        ext.push(KeyUsage(uso).to_extension(sujeito, &ext)?);
        Ok(ext)
    }
}

/// .pfx AUTOASSINADO para teste e simulador (nunca vale no fisco). `documento` 14 = PJ (otherName 2.16.76.1.3.3),
/// 11 = PF (2.16.76.1.3.1). `icp_brasil` carimba O=ICP-Brasil no titular e o assina por uma "AC de teste" descartavel
/// ([`AC_TESTE`], O=ICP-Brasil, chave jogada fora; o certificado dela vai na cadeia do .pfx) para exercitar a
/// leitura -- autoassinado nao seria
/// lido como ICP. Continua sem cadeia valida: o fisco recusa.
#[cfg(feature = "cert_a1_teste")]
pub fn gerar_teste(
    documento: &str,
    nome: &str,
    senha: &str,
    dias: i64,
    icp_brasil: bool,
    cn_com_documento: bool,
    agora: DateTime<Utc>,
) -> Result<Vec<u8>, ErroCertificado> {
    use p12_keystore::{Certificate, KeyStore, KeyStoreEntry, PrivateKey, PrivateKeyChain};
    use rsa::pkcs1v15::SigningKey;
    use sha2::Sha256;
    use std::str::FromStr;
    use x509_cert::builder::{Builder, CertificateBuilder};
    use x509_cert::der::asn1::{Any, ObjectIdentifier, OctetString};
    use x509_cert::der::{Encode, Tag};
    use x509_cert::ext::pkix::name::{GeneralName, OtherName};
    use x509_cert::ext::pkix::SubjectAltName;
    use x509_cert::name::Name;
    use x509_cert::serial_number::SerialNumber;
    use x509_cert::time::{Time, Validity};

    let falha = |m: &str| ErroCertificado(format!("gerar certificado de teste: {m}"));
    let doc = alnum_up(documento);
    let chave =
        RsaPrivateKey::new(&mut crate::aleatorio::sistema(), 2048).map_err(|_| falha("chave"))?;
    let cn = if cn_com_documento {
        format!("{nome}:{doc}")
    } else {
        nome.to_string()
    };
    let titular = Name::from_str(&format!(
        "CN={cn},{}C=BR",
        if icp_brasil { "O=ICP-Brasil," } else { "" }
    ))
    .map_err(|_| falha("titular"))?;
    let t = |d: DateTime<Utc>| {
        Time::try_from(
            std::time::UNIX_EPOCH + std::time::Duration::from_secs(d.timestamp().max(0) as u64),
        )
    };
    let validade = Validity::new(
        t(agora - chrono::Duration::days(1)).map_err(|_| falha("validade"))?,
        t(agora + chrono::Duration::days(dias)).map_err(|_| falha("validade"))?,
    );
    let pub_der = rsa::pkcs8::EncodePublicKey::to_public_key_der(&chave.to_public_key())
        .map_err(|_| falha("spki"))?;
    let spki = x509_cert::spki::SubjectPublicKeyInfoOwned::try_from(pub_der.as_bytes())
        .map_err(|_| falha("spki"))?;
    // icp_brasil: a AC de teste (raiz propria, vai na cadeia do .pfx -- `pem_certificados()` serve de CA no mTLS do
    // simulador, como antes com o autoassinado)
    let (emissor, chave_emissor, ac_der) = if icp_brasil {
        let ac = RsaPrivateKey::new(&mut crate::aleatorio::sistema(), 2048)
            .map_err(|_| falha("chave da AC"))?;
        let nome_ac = Name::from_str(&format!("CN={AC_TESTE},O=ICP-Brasil,C=BR"))
            .map_err(|_| falha("emissor"))?;
        let ac_pub = rsa::pkcs8::EncodePublicKey::to_public_key_der(&ac.to_public_key())
            .map_err(|_| falha("spki da AC"))?;
        let ac_spki = x509_cert::spki::SubjectPublicKeyInfoOwned::try_from(ac_pub.as_bytes())
            .map_err(|_| falha("spki da AC"))?;
        let perfil = x509_cert::builder::profile::cabf::Root::new(false, nome_ac.clone())
            .map_err(|_| falha("perfil da AC"))?;
        let ac_cert = CertificateBuilder::new(
            perfil,
            SerialNumber::from(agora.timestamp_subsec_nanos().max(1) ^ 0x5a5a),
            validade,
            ac_spki,
        )
        .map_err(|_| falha("montar AC"))?
        .build::<_, rsa::pkcs1v15::Signature>(&SigningKey::<Sha256>::new(ac.clone()))
        .map_err(|_| falha("assinar AC"))?;
        (
            nome_ac,
            ac,
            Some(ac_cert.to_der().map_err(|_| falha("codificar AC"))?),
        )
    } else {
        (titular.clone(), chave.clone(), None)
    };
    let signer = SigningKey::<Sha256>::new(chave_emissor);
    let (oid, conteudo) = if doc.len() == 14 {
        (OID_CNPJ, doc.clone())
    } else {
        (
            OID_DADOS_PF,
            format!("01011980{doc}{}{}SSPSP ", "0".repeat(11), "0".repeat(15)),
        )
    };
    let valor = OctetString::new(conteudo.into_bytes()).map_err(|_| falha("otherName"))?;
    let any = Any::new(Tag::OctetString, valor.as_bytes()).map_err(|_| falha("otherName"))?;
    let san = SubjectAltName(vec![GeneralName::OtherName(OtherName {
        type_id: ObjectIdentifier::new(oid).map_err(|_| falha("oid"))?,
        value: any,
    })]);
    let mut b = CertificateBuilder::new(
        Folha { titular, emissor },
        SerialNumber::from(agora.timestamp_subsec_nanos().max(1)),
        validade,
        spki,
    )
    .map_err(|_| falha("montar"))?;
    b.add_extension(&san).map_err(|_| falha("SAN"))?;
    let cert = b
        .build::<_, rsa::pkcs1v15::Signature>(&signer)
        .map_err(|_| falha("assinar"))?;
    let der = cert.to_der().map_err(|_| falha("codificar"))?;
    let chave_der = chave.to_pkcs8_der().map_err(|_| falha("chave"))?;
    let entrada = PrivateKeyChain::new(
        b"rustnf-teste".to_vec(),
        PrivateKey::from_der(chave_der.as_bytes()).map_err(|_| falha("pkcs8"))?,
        std::iter::once(der.as_slice())
            .chain(ac_der.as_deref())
            .map(|d| Certificate::from_der(d).map_err(|_| falha("certificado")))
            .collect::<Result<Vec<_>, _>>()?,
    );
    let mut ks = KeyStore::new();
    ks.add_entry("rustnf-teste", KeyStoreEntry::PrivateKeyChain(entrada));
    ks.writer(senha).write().map_err(|_| falha("pkcs12"))
}
