// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Leitura do que o SEFIN/ADN devolvem: a NFS-e autorizada, o evento e as mensagens (erros e alertas).
//! O roxmltree recusa DTD por padrao (sem entidade externa).

use std::io::{Read, Write};

use base64::Engine;
use serde::{Deserialize, Serialize};

use super::base::NS;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErroLeitura(pub String);

impl std::fmt::Display for ErroLeitura {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for ErroLeitura {}

/// XML -> gzip -> base64 (o formato dos campos `...XmlGZipB64`).
pub fn compactar(xml: &str) -> String {
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gz.write_all(xml.as_bytes()).expect("escrita em memoria");
    base64::engine::general_purpose::STANDARD.encode(gz.finish().expect("gzip em memoria"))
}

/// base64 -> gunzip -> texto UTF-8.
pub fn descompactar(b64: &str) -> Result<String, ErroLeitura> {
    let ilegivel = || ErroLeitura("conteudo compactado ilegivel".into());
    let bruto = base64::engine::general_purpose::STANDARD
        .decode(b64.trim())
        .map_err(|_| ilegivel())?;
    let mut s = String::new();
    flate2::read::GzDecoder::new(bruto.as_slice())
        .read_to_string(&mut s)
        .map_err(|_| ilegivel())?;
    Ok(s)
}

fn doc(xml: &str) -> Result<roxmltree::Document<'_>, ErroLeitura> {
    roxmltree::Document::parse(xml).map_err(|_| ErroLeitura("XML mal formado ou com DTD".into()))
}

fn filho<'a, 'i>(n: roxmltree::Node<'a, 'i>, nome: &str) -> Option<roxmltree::Node<'a, 'i>> {
    n.children().find(|c| {
        c.is_element() && c.tag_name().name() == nome && c.tag_name().namespace() == Some(NS)
    })
}

fn caminho<'a, 'i>(n: Option<roxmltree::Node<'a, 'i>>, c: &str) -> Option<roxmltree::Node<'a, 'i>> {
    c.split('/').try_fold(n?, |n, p| filho(n, p))
}

fn t(n: Option<roxmltree::Node>, c: &str) -> String {
    caminho(n, c)
        .and_then(|x| x.text())
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Nfse {
    pub chave: String,
    pub numero: String,
    /// cStat: 100 gerada, 102 decisao judicial, 103 avulsa, 107 MEI.
    pub situacao: String,
    pub processada_em: String,
    /// ambGer: 1 prefeitura, 2 SEFIN Nacional.
    pub ambiente_gerador: String,
    pub numero_dfe: String,
    /// verCalcIBSCBS: a versao da Calculadora de Tributos que gerou a NFS-e (Release 2 da NT 009, so' em nota gerada
    /// pela Sefin Nacional). Vazio na nota anterior a ele.
    #[serde(default)]
    pub versao_calculadora: String,
    pub emitente_documento: String,
    pub emitente_nome: String,
    pub id_dps: String,
    pub serie_dps: String,
    pub numero_dps: String,
    pub competencia: String,
    pub ambiente: String,
    pub local_emissao: String,
    pub local_incidencia: String,
    pub ctribnac: String,
    pub descricao_tributacao: String,
    pub descricao_servico: String,
    pub tomador_documento: String,
    pub tomador_nome: String,
    /// Inscricao municipal do tomador (`toma/IM`).
    #[serde(default)]
    pub tomador_im: String,
    /// `toma/email`.
    #[serde(default)]
    pub tomador_email: String,
    /// `toma/fone`, so' digitos (como a DPS o escreve).
    #[serde(default)]
    pub tomador_fone: String,
    /// CEP do endereco nacional do tomador (`toma/end/endNac/CEP`).
    #[serde(default)]
    pub tomador_cep: String,
    /// Municipio (IBGE) do endereco nacional do tomador (`toma/end/endNac/cMun`).
    #[serde(default)]
    pub tomador_municipio: String,
    /// `toma/end/xLgr`.
    #[serde(default)]
    pub tomador_logradouro: String,
    /// `toma/end/nro`.
    #[serde(default)]
    pub tomador_numero: String,
    /// `toma/end/xCpl`.
    #[serde(default)]
    pub tomador_complemento: String,
    /// `toma/end/xBairro`.
    #[serde(default)]
    pub tomador_bairro: String,
    pub valor_servico: String,
    pub base_calculo: String,
    pub aliquota_aplicada: String,
    pub iss: String,
    pub total_retido: String,
    pub valor_liquido: String,
    /// Valores IBS/CBS calculados pelo SEFIN (folha -> valor), quando houver.
    pub ibscbs: Vec<(String, String)>,
    #[serde(skip)]
    pub xml: String,
}

impl Nfse {
    /// Consulta publica (o que o QR do DANFSe abre).
    pub fn link_consulta(&self) -> String {
        let base = if self.ambiente == "1" {
            "www.nfse.gov.br"
        } else {
            "www.producaorestrita.nfse.gov.br"
        };
        format!("https://{base}/ConsultaPublica/?tpc=1&chave={}", self.chave)
    }
}

/// A NFS-e autorizada (`<NFSe>`).
pub fn ler_nfse(xml: &str) -> Result<Nfse, ErroLeitura> {
    let d = doc(xml)?;
    let raiz = d.root_element();
    let inf = filho(raiz, "infNFSe")
        .filter(|_| raiz.tag_name().name() == "NFSe")
        .ok_or_else(|| ErroLeitura("o documento nao e' uma NFS-e Nacional".into()))?;
    let ident = inf.attribute("Id").unwrap_or("");
    let dps = caminho(Some(inf), "DPS/infDPS");
    let toma = caminho(dps, "toma");
    let emit = filho(inf, "emit");
    let ibscbs = filho(inf, "IBSCBS")
        .map(|ib| {
            ib.descendants()
                .filter(|n| n.is_element() && !n.children().any(|c| c.is_element()))
                .filter_map(|n| {
                    n.text()
                        .map(|x| (n.tag_name().name().to_string(), x.trim().to_string()))
                })
                .filter(|(_, v)| !v.is_empty())
                .collect()
        })
        .unwrap_or_default();
    let ou = |a: String, b: String| if a.is_empty() { b } else { a };
    Ok(Nfse {
        chave: ident.strip_prefix("NFS").unwrap_or(ident).to_string(),
        numero: t(Some(inf), "nNFSe"),
        situacao: t(Some(inf), "cStat"),
        processada_em: t(Some(inf), "dhProc"),
        ambiente_gerador: t(Some(inf), "ambGer"),
        numero_dfe: t(Some(inf), "nDFSe"),
        versao_calculadora: t(Some(inf), "verCalcIBSCBS"),
        emitente_documento: ou(t(emit, "CNPJ"), t(emit, "CPF")),
        emitente_nome: t(emit, "xNome"),
        id_dps: dps
            .and_then(|n| n.attribute("Id"))
            .unwrap_or("")
            .to_string(),
        serie_dps: t(dps, "serie"),
        numero_dps: t(dps, "nDPS"),
        competencia: t(dps, "dCompet"),
        ambiente: t(dps, "tpAmb"),
        local_emissao: t(Some(inf), "xLocEmi"),
        local_incidencia: t(Some(inf), "cLocIncid"),
        ctribnac: t(dps, "serv/cServ/cTribNac"),
        descricao_tributacao: t(Some(inf), "xTribNac"),
        descricao_servico: t(dps, "serv/cServ/xDescServ"),
        tomador_documento: ou(ou(t(toma, "CNPJ"), t(toma, "CPF")), t(toma, "NIF")),
        tomador_nome: t(toma, "xNome"),
        tomador_im: t(toma, "IM"),
        tomador_email: t(toma, "email"),
        tomador_fone: t(toma, "fone"),
        tomador_cep: t(toma, "end/endNac/CEP"),
        tomador_municipio: t(toma, "end/endNac/cMun"),
        tomador_logradouro: t(toma, "end/xLgr"),
        tomador_numero: t(toma, "end/nro"),
        tomador_complemento: t(toma, "end/xCpl"),
        tomador_bairro: t(toma, "end/xBairro"),
        valor_servico: t(dps, "valores/vServPrest/vServ"),
        base_calculo: t(Some(inf), "valores/vBC"),
        aliquota_aplicada: t(Some(inf), "valores/pAliqAplic"),
        iss: t(Some(inf), "valores/vISSQN"),
        total_retido: t(Some(inf), "valores/vTotalRet"),
        valor_liquido: t(Some(inf), "valores/vLiq"),
        ibscbs,
        xml: xml.to_string(),
    })
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Evento {
    pub tipo: String,
    pub chave: String,
    pub sequencia: String,
    pub processado_em: String,
    /// `dhEvento` do pedido: quando o autor pediu o evento.
    pub data_evento: String,
    pub id: String,
    pub autor: String,
    pub motivo: String,
}

/// O evento registrado (`<evento>` com o pedRegEvento dentro) ou o proprio pedRegEvento.
pub fn ler_evento(xml: &str) -> Result<Evento, ErroLeitura> {
    let d = doc(xml)?;
    let raiz = d.root_element();
    let inf = filho(raiz, "infEvento");
    let ped = match inf {
        Some(i) => caminho(Some(i), "pedRegEvento/infPedReg"),
        None => filho(raiz, "infPedReg"),
    }
    .ok_or_else(|| ErroLeitura("o documento nao e' um evento da NFS-e Nacional".into()))?;
    let grupo = ped.children().find(|c| {
        let n = c.tag_name().name();
        c.is_element()
            && n.len() > 1
            && n.starts_with('e')
            && n[1..].chars().all(|x| x.is_ascii_digit())
    });
    let tipo = grupo
        .map(|g| g.tag_name().name()[1..].to_string())
        .unwrap_or_default();
    let ou = |a: String, b: String| if a.is_empty() { b } else { a };
    Ok(Evento {
        chave: t(Some(ped), "chNFSe"),
        sequencia: t(inf, "nSeqEvento"),
        processado_em: t(inf, "dhProc"),
        data_evento: t(Some(ped), "dhEvento"),
        id: inf
            .or(Some(ped))
            .and_then(|n| n.attribute("Id"))
            .unwrap_or("")
            .to_string(),
        autor: ou(t(Some(ped), "CNPJAutor"), t(Some(ped), "CPFAutor")),
        motivo: ou(t(grupo, "xMotivo"), t(grupo, "cMotivo")),
        tipo,
    })
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mensagem {
    pub codigo: String,
    pub descricao: String,
    pub complemento: String,
}

impl Mensagem {
    /// Orientacao para os retornos mais comuns de configuracao errada, ou vazio. A mensagem oficial nao diz o que
    /// fazer: E0008 (Anexo I, `dhEmi` posterior ao processamento: relogio adiantado), E0037 (`cLocEmi`: municipio fora
    /// do cadastro de convenio) e E0312 (codigo de tributacao nacional nao administrado pelo municipio de incidencia na
    /// competencia).
    pub fn dica(&self) -> &'static str {
        match self.codigo.trim() {
            "E0008" => concat!(
                "a hora de emissao ficou depois da hora da Sefin: o relogio deste computador esta adiantado. Acerte o ",
                "relogio ou aumente a folga (`folga_relogio_s` em [sefin], padrao 60 s); com `dhEmi` no pedido, ",
                "informe uma hora que ja passou"
            ),
            "E0037" => concat!(
                "o municipio emissor (cLocEmi) nao esta ativo no sistema nacional NAQUELE ambiente: producao e ",
                "producao restrita tem cadastros de convenio separados. Confira o ambiente usado e se o municipio ",
                "aderiu ao padrao nacional nele (a lista de municipios fica no portal da NFS-e)"
            ),
            "E0312" => concat!(
                "o municipio de incidencia nao administra esse codigo de tributacao nacional (cTribNac) na ",
                "competencia informada: confira se o municipio tem o servico habilitado nessa data e, se preciso, ",
                "use o codigo com o desdobro que ele administra"
            ),
            _ => "",
        }
    }
}

impl std::fmt::Display for Mensagem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let txt = [self.descricao.as_str(), self.complemento.as_str()]
            .iter()
            .filter(|x| !x.is_empty())
            .copied()
            .collect::<Vec<_>>()
            .join(" -- ");
        let txt = if txt.is_empty() {
            "(sem descricao)".to_string()
        } else {
            txt
        };
        let txt = match self.dica() {
            "" => txt,
            dica => format!("{txt} (dica: {dica})"),
        };
        if self.codigo.is_empty() {
            write!(f, "{txt}")
        } else {
            write!(f, "{}: {txt}", self.codigo)
        }
    }
}

fn msg(e: &serde_json::Value) -> Mensagem {
    let g = |ks: &[&str], max: usize| -> String {
        ks.iter()
            .find_map(|k| {
                e.get(*k).filter(|v| !v.is_null()).map(|v| match v {
                    serde_json::Value::String(s) => s.clone(),
                    o => o.to_string(),
                })
            })
            .filter(|s| !s.is_empty())
            .map(|s| s.chars().take(max).collect())
            .unwrap_or_default()
    };
    if !e.is_object() {
        return Mensagem {
            descricao: e.to_string().chars().take(300).collect(),
            ..Default::default()
        };
    }
    Mensagem {
        codigo: g(&["codigo", "Codigo"], 50),
        descricao: g(&["descricao", "Descricao", "mensagem", "Mensagem"], 500),
        complemento: g(&["complemento", "Complemento"], 500),
    }
}

/// (erros, alertas). Duas formas de erro na API: `erros` (lista, POST /nfse) e `erro` (objeto, consultas e eventos);
/// o ADN usa PascalCase. `erros` em TEXTO tambem e' recusa; `null`, lista vazia e texto em branco nao sao.
pub fn mensagens(corpo: &serde_json::Value) -> (Vec<Mensagem>, Vec<Mensagem>) {
    let lista = |ks: &[&str]| -> Vec<Mensagem> {
        for k in ks {
            match corpo.get(*k) {
                Some(serde_json::Value::Array(a)) => return a.iter().map(msg).collect(),
                Some(o @ serde_json::Value::Object(_)) => return vec![msg(o)],
                // texto = recusa; em branco nao e'
                Some(serde_json::Value::String(t)) if !t.trim().is_empty() => {
                    return vec![Mensagem {
                        descricao: t.trim().chars().take(300).collect(),
                        ..Default::default()
                    }];
                }
                _ => {}
            }
        }
        Vec::new()
    };
    (
        lista(&["erros", "Erros", "erro", "Erro"]),
        lista(&["alertas", "Alertas"]),
    )
}
