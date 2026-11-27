// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A distribuicao de DF-e de mentira: o que a SEFAZ de mentira da NF-e responde quando a janela busca as notas RECEBIDAS
//! (`NFeDistribuicaoDFe`), consulta uma nota pela chave e registra a resposta do destinatario. Sem rede.
//!
//! O "Ambiente Nacional" de mentira tem uma lista fixa de documentos (os NSU sao a posicao na lista, de 1 em diante). A
//! pasta de ensaio ganha os primeiros (a SEMENTE); o resto so' aparece quando a janela busca de verdade, e e' o que a
//! demonstracao chama de "notas novas". Como o de verdade, so' entrega a nota COMPLETA a quem ja respondeu sobre ela
//! (NT 2014.002 v1.02d, item 3); antes disso, so' o resumo. O desconhecimento nao libera a nota.

use std::collections::BTreeSet;
use std::io::Write as _;

use base64::Engine as _;
use chrono::{FixedOffset, Utc};
use flate2::{write::GzEncoder, Compression};
use rustnf::chave_dfe::{self, Montar};
use rustnf::nfe::sefaz::FalhaRede;

use crate::demonstracao_nfe::{entre, NS};

/// O nome do servico no roteiro do transporte de mentira.
pub(crate) const DIST: &str = "NFeDistribuicaoDFe";

/// O que o Ambiente Nacional de mentira responde a "buscar notas novas".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RoteiroRecebidas {
    /// Entrega as notas novas (o padrao): tres, e depois a fila esta em dia.
    #[default]
    Normal,
    /// Bloqueia por consultar demais (656): a tela mostra ate que horas esperar.
    Espera,
    /// Nao ha nada de novo (137).
    Vazio,
}

/// O roteiro de `--nfe-recebidas`.
#[must_use]
pub fn roteiro_recebidas_do_nome(nome: &str) -> Option<RoteiroRecebidas> {
    match nome {
        "normal" => Some(RoteiroRecebidas::Normal),
        "espera" => Some(RoteiroRecebidas::Espera),
        "vazio" => Some(RoteiroRecebidas::Vazio),
        _ => None,
    }
}

/// Um documento da fila do Ambiente Nacional de mentira.
#[derive(Debug, Clone)]
pub(crate) struct DocFalso {
    pub(crate) chave: String,
    /// O `resNFe`.
    pub(crate) resumo: String,
    /// O `procNFe` (so' as notas que a demonstracao tem completas).
    pub(crate) proc: Option<String>,
}

/// O estado da distribuicao de mentira.
#[derive(Debug, Default)]
pub(crate) struct EstadoRecebidas {
    pub(crate) roteiro: RoteiroRecebidas,
    pub(crate) docs: Vec<DocFalso>,
    /// Quantos documentos a pasta de ensaio recebe na semeadura (os NSU de 1 a `semente`).
    pub(crate) semente: usize,
    /// As chaves a que a empresa ja respondeu (ciencia, confirmacao ou nao realizada): so' elas recebem a nota completa.
    manifestadas: BTreeSet<String>,
}

fn gz_b64(xml: &str) -> String {
    let mut e = GzEncoder::new(Vec::new(), Compression::default());
    // escrever num `Vec` nao falha
    let _ = e.write_all(xml.as_bytes());
    let bytes = e.finish().unwrap_or_default();
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn doc_zip(nsu: usize, esquema: &str, xml: &str) -> String {
    format!(
        "<docZip NSU=\"{nsu:015}\" schema=\"{esquema}\">{}</docZip>",
        gz_b64(xml)
    )
}

fn resposta(
    cstat: &str,
    motivo: &str,
    ult: usize,
    max: usize,
    docs: &[String],
) -> Result<(u16, Vec<u8>), FalhaRede> {
    let lote = if docs.is_empty() {
        String::new()
    } else {
        format!("<loteDistDFeInt>{}</loteDistDFeInt>", docs.concat())
    };
    let ret = format!(
        "<retDistDFeInt xmlns=\"{NS}\" versao=\"1.01\"><tpAmb>2</tpAmb><verAplic>DEMO_DFE_1</verAplic><cStat>{cstat}</cStat>\
         <xMotivo>{motivo}</xMotivo><dhResp>2026-10-03T09:00:00-03:00</dhResp><ultNSU>{ult:015}</ultNSU>\
         <maxNSU>{max:015}</maxNSU>{lote}</retDistDFeInt>"
    );
    Ok((200, envelope(&ret)))
}

/// O envelope do servico: o `nfeDistDFeInteresseResult` do WSDL da distribuicao.
fn envelope(ret: &str) -> Vec<u8> {
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?><soap:Envelope xmlns:soap=\"http://www.w3.org/2003/05/soap-envelope\">\
         <soap:Body><nfeDistDFeInteresseResponse xmlns=\"{NS}/wsdl/{DIST}\">\
         <nfeDistDFeInteresseResult>{ret}</nfeDistDFeInteresseResult></nfeDistDFeInteresseResponse>\
         </soap:Body></soap:Envelope>"
    )
    .into_bytes()
}

impl EstadoRecebidas {
    /// A empresa respondeu a nota (ciencia, confirmacao, desconhecimento ou nao realizada). So' as tres primeiras liberam a
    /// nota completa.
    pub(crate) fn manifestou(&mut self, chave: &str, tp_evento: &str) {
        if matches!(tp_evento, "210200" | "210210" | "210240") {
            self.manifestadas.insert(chave.to_string());
        }
    }

    /// Quantos documentos o Ambiente Nacional de mentira mostra agora.
    fn visiveis(&self, semeando: bool) -> usize {
        if semeando || self.roteiro == RoteiroRecebidas::Vazio {
            self.semente.min(self.docs.len())
        } else {
            self.docs.len()
        }
    }

    /// Responde a um pedido `distDFeInt`: `distNSU`, `consNSU` ou `consChNFe`.
    pub(crate) fn responder(
        &self,
        corpo: &str,
        semeando: bool,
    ) -> Result<(u16, Vec<u8>), FalhaRede> {
        let visiveis = self.visiveis(semeando);
        if corpo.contains("<consChNFe>") {
            let chave = entre(corpo, "<chNFe>", "</chNFe>");
            let Some((i, d)) = self.docs.iter().enumerate().find(|(_, d)| d.chave == chave) else {
                return resposta("137", "Nenhum documento localizado", 0, 0, &[]);
            };
            let nsu = i + 1;
            let doc = match (&d.proc, self.manifestadas.contains(chave)) {
                (Some(proc), true) => doc_zip(nsu, "procNFe_v4.00.xsd", proc),
                _ => doc_zip(nsu, "resNFe_v1.01.xsd", &d.resumo),
            };
            return resposta("138", "Documento localizado", nsu, nsu, &[doc]);
        }
        if corpo.contains("<consNSU>") {
            let nsu: usize = entre(corpo, "<NSU>", "</NSU>").trim().parse().unwrap_or(0);
            return match nsu.checked_sub(1).and_then(|i| self.docs.get(i)) {
                Some(d) if nsu <= visiveis => resposta(
                    "138",
                    "Documento localizado",
                    nsu,
                    nsu,
                    &[doc_zip(nsu, "resNFe_v1.01.xsd", &d.resumo)],
                ),
                _ => resposta("137", "Nenhum documento localizado", 0, 0, &[]),
            };
        }
        let ult: usize = entre(corpo, "<ultNSU>", "</ultNSU>")
            .trim()
            .parse()
            .unwrap_or(0);
        if self.roteiro == RoteiroRecebidas::Espera && !semeando {
            return resposta("656", "Consumo Indevido", ult, ult, &[]);
        }
        let lote: Vec<(usize, &DocFalso)> = self
            .docs
            .iter()
            .enumerate()
            .map(|(i, d)| (i + 1, d))
            .filter(|(nsu, _)| *nsu > ult && *nsu <= visiveis)
            .take(50)
            .collect();
        let Some((ultimo, _)) = lote.last() else {
            return resposta("137", "Nenhum documento localizado", ult, ult, &[]);
        };
        // na semeadura a fila "tem mais" (maxNSU adiante): o cursor fica sem espera, e a janela busca o resto de verdade
        let max = if semeando { visiveis + 1 } else { visiveis };
        let docs: Vec<String> = lote
            .iter()
            .map(|(nsu, d)| doc_zip(*nsu, "resNFe_v1.01.xsd", &d.resumo))
            .collect();
        resposta("138", "Documento(s) localizado(s)", *ultimo, max, &docs)
    }
}

/// O `resNFe` de um documento de ensaio. `situacao`: `cSitNFe` (1 autorizada, 3 cancelada).
pub(crate) fn res_nfe(
    chave: &str,
    emitente: &str,
    nome: &str,
    emissao: &str,
    valor: &str,
    situacao: u8,
) -> String {
    let tag = if emitente.len() == 11 { "CPF" } else { "CNPJ" };
    format!(
        "<resNFe xmlns=\"{NS}\" versao=\"1.01\"><chNFe>{chave}</chNFe><{tag}>{emitente}</{tag}>\
         <xNome>{nome}</xNome><IE>111111111111</IE><dhEmi>{emissao}</dhEmi><tpNF>1</tpNF><vNF>{valor}</vNF>\
         <digVal>qZk+NkcGgWq6PiVxeFDCbJzQ2J0=</digVal><dhRecbto>{emissao}</dhRecbto>\
         <nProt>135260000000001</nProt><cSitNFe>{situacao}</cSitNFe></resNFe>"
    )
}

/// A chave de uma nota de fornecedor de ensaio (serie 9, para nao colidir com as notas emitidas pela propria empresa).
pub(crate) fn chave_de(
    emitente: &str,
    dias_atras: i64,
    numero: u64,
) -> Result<(String, String), String> {
    let quando = Utc::now() - chrono::Duration::days(dias_atras) - chrono::Duration::minutes(5);
    let brasilia = FixedOffset::west_opt(3 * 3600).ok_or("fuso")?;
    let local = quando.with_timezone(&brasilia);
    let chave = chave_dfe::montar(&Montar {
        cuf: 35,
        aamm: &local.format("%y%m").to_string(),
        emitente,
        modelo: 55,
        serie: 9,
        numero,
        ..Montar::default()
    })
    .map_err(|e| e.to_string())?;
    Ok((
        chave,
        local.to_rfc3339_opts(chrono::SecondsFormat::Secs, false),
    ))
}

/// As chaves das notas recebidas do ensaio.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecebidasDoEnsaio {
    /// So' o resumo, sem resposta da empresa.
    pub resumo: String,
    /// A nota completa, com a ciencia da empresa.
    pub completa: String,
    /// Cancelada pelo emitente.
    pub cancelada: String,
    /// As notas novas, que so' aparecem quando a janela busca.
    pub novas: Vec<String>,
}

/// Um documento do ensaio: quem emitiu, de quando, quanto.
struct Doc<'a> {
    emitente: &'a str,
    nome: &'a str,
    dias: i64,
    numero: u64,
    valor: &'a str,
    situacao: u8,
}

fn doc_de_resumo(d: &Doc<'_>) -> Result<DocFalso, String> {
    let (chave, quando) = chave_de(d.emitente, d.dias, d.numero)?;
    Ok(DocFalso {
        resumo: res_nfe(&chave, d.emitente, d.nome, &quando, d.valor, d.situacao),
        chave,
        proc: None,
    })
}

/// Semeia a pasta de ensaio com as NF-e RECEBIDAS, pela mesma camada de aplicacao que a janela usa (a distribuicao e' a de
/// mentira e o certificado, um de teste gerado na hora): um resumo sem resposta, uma nota completa com a ciencia da empresa e
/// uma cancelada pelo emitente. Outras tres notas ficam para a janela buscar ("notas novas"). Pasta que ja tem recebidas fica
/// como esta; o Ambiente Nacional de mentira aprende os documentos de qualquer jeito.
///
/// # Errors
///
/// A frase do que falhou (configuracao da NF-e incompleta, pasta que nao abre...).
pub fn semear_recebidas(
    config: &rustnf_app::config::Config,
    transporte: &crate::demonstracao_nfe::TransporteNfeDemonstracao,
) -> Result<RecebidasDoEnsaio, String> {
    use std::sync::Arc;
    use std::time::Duration;

    use rustnf::assinatura::cert_a1;
    use rustnf::nfe::sefaz::ClienteSefaz;
    use rustnf_app::nfe::{self as app, Contexto};
    use rustnf_app::notas::nfe::NotasNfe;

    use crate::demonstracao_nfe::{pedido, Passo};
    use crate::servicos::TransporteDin;

    let documento = config.emitente.cnpj.clone();
    let pasta = config.notas.pasta.trim();
    let notas = NotasNfe::abrir(pasta).map_err(|_| "a pasta de notas nao abre")?;
    let ja_semeada = !notas
        .recebidas()
        .and_then(|r| r.listar())
        .map_err(|_| "a pasta de recebidas nao le")?
        .is_empty();
    // o fornecedor de ensaio emite numa pasta a parte, so' para produzir a NF-e completa que a distribuicao entrega
    let a_parte = std::path::Path::new(pasta).join(".fornecedor-de-ensaio");
    let mut do_fornecedor = config.clone();
    do_fornecedor.emitente.nome = "DISTRIBUIDORA DE CAFE ENSAIO LTDA".into();
    do_fornecedor.nfe.serie = "9".into();
    do_fornecedor.notas.pasta = a_parte.to_string_lossy().into_owned();
    let pfx = cert_a1::gerar_teste(
        &documento,
        "FORNECEDOR DE ENSAIO",
        "ensaio",
        30,
        true,
        false,
        chrono::Utc::now(),
    )
    .map_err(|e| e.to_string())?;
    let certificado = cert_a1::carregar(&pfx, "ensaio").map_err(|e| e.to_string())?;
    let cliente = |c: &rustnf_app::config::Config| -> Result<_, String> {
        let sefaz = app::config_sefaz(c).map_err(|e| e.to_string())?;
        Ok(
            ClienteSefaz::com_transporte(sefaz, TransporteDin::de(Arc::new(transporte.clone())))
                .map_err(|e| e.to_string())?
                .com_dormir(|_: Duration| Box::pin(async {})),
        )
    };
    let (cliente_do_fornecedor, cliente_da_empresa) = (cliente(&do_fornecedor)?, cliente(config)?);
    let notas_do_fornecedor =
        NotasNfe::abrir(&a_parte).map_err(|_| "a pasta do fornecedor nao abre")?;
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    let feito = rt.block_on(async {
        transporte.passo_do_ensaio(Passo::Autorizar);
        // as notas completas: o fornecedor as emite PARA a empresa e a SEFAZ de mentira as autoriza
        let mut completas = Vec::new();
        for (nome, dias, item, valor) in [
            (
                "EMBALAGENS PAULISTA ENSAIO LTDA",
                4,
                ("Embalagem kraft 40x30", 50, 2500),
                "1250.00",
            ),
            (
                "DISTRIBUIDORA DE CAFE ENSAIO LTDA",
                3,
                ("Cafe torrado 500g", 20, 1590),
                "318.00",
            ),
        ] {
            let mut deste = do_fornecedor.clone();
            deste.emitente.nome = nome.into();
            let ctx = Contexto::novo(
                &cliente_do_fornecedor,
                &certificado,
                &deste,
                Some(&notas_do_fornecedor),
            );
            let nota = app::emitir(
                &ctx,
                pedido(dias, (&documento, &config.emitente.nome), item),
                false,
            )
            .await
            .map_err(|e| e.to_string())?;
            let proc = notas_do_fornecedor
                .ler_xml(&nota.chave)
                .map_err(|_| "o XML do fornecedor nao le")?;
            let quando = entre(&proc, "<dhEmi>", "</dhEmi>").to_string();
            completas.push(DocFalso {
                resumo: res_nfe(&nota.chave, &documento, nome, &quando, valor, 1),
                chave: nota.chave,
                proc: Some(proc),
            });
        }
        let mut completas = completas.into_iter();
        let (doc_a, doc_b) = (
            completas.next().ok_or("sem a primeira nota")?,
            completas.next().ok_or("sem a segunda nota")?,
        );
        let docs = vec![
            doc_a,
            doc_b,
            doc_de_resumo(&Doc {
                emitente: "11222333000181",
                nome: "PADARIA E CONFEITARIA ENSAIO LTDA",
                dias: 4,
                numero: 77,
                valor: "480.00",
                situacao: 3,
            })?,
            doc_de_resumo(&Doc {
                emitente: "11222333000181",
                nome: "GRAFICA RAPIDA ENSAIO LTDA",
                dias: 1,
                numero: 2002,
                valor: "89.90",
                situacao: 1,
            })?,
            doc_de_resumo(&Doc {
                emitente: "11222333000181",
                nome: "SITIO BOA VISTA ENSAIO LTDA",
                dias: 0,
                numero: 15,
                valor: "640.00",
                situacao: 1,
            })?,
            doc_de_resumo(&Doc {
                emitente: "11222333000181",
                nome: "TRANSPORTES RAPIDO ENSAIO LTDA",
                dias: 0,
                numero: 3003,
                valor: "2310.45",
                situacao: 1,
            })?,
        ];
        let achado = RecebidasDoEnsaio {
            resumo: docs[0].chave.clone(),
            completa: docs[1].chave.clone(),
            cancelada: docs[2].chave.clone(),
            novas: docs[3..].iter().map(|d| d.chave.clone()).collect(),
        };
        let _ = transporte.recebidas(|e| {
            e.docs = docs;
            e.semente = 3;
        });
        if !ja_semeada {
            let ctx = Contexto::novo(&cliente_da_empresa, &certificado, config, Some(&notas));
            // um lote so': a fila "tem mais" e a pasta fica sem espera; o resto e' para a janela buscar
            app::sincronizar(&ctx, Some(1))
                .await
                .map_err(|e| e.to_string())?;
            // so' a segunda ganha a ciencia da empresa: a primeira fica so' no resumo, para a janela baixar
            app::baixar(&ctx, &achado.completa, true)
                .await
                .map_err(|e| e.to_string())?;
        }
        Ok::<_, String>(achado)
    });
    transporte.passo_do_ensaio(Passo::AoVivo);
    let _ = std::fs::remove_dir_all(&a_parte);
    transporte.zerar_contagens();
    feito
}

#[cfg(test)]
mod testes {
    use super::*;

    fn estado() -> EstadoRecebidas {
        let doc = |n: u64| {
            let (chave, quando) = chave_de("11222333000181", 1, n).unwrap();
            DocFalso {
                resumo: res_nfe(&chave, "11222333000181", "FORNECEDOR", &quando, "10.00", 1),
                chave,
                proc: None,
            }
        };
        EstadoRecebidas {
            roteiro: RoteiroRecebidas::Normal,
            docs: vec![doc(1), doc(2), doc(3), doc(4)],
            semente: 2,
            manifestadas: BTreeSet::new(),
        }
    }

    fn pedido(ult: usize) -> String {
        format!("<distDFeInt><distNSU><ultNSU>{ult:015}</ultNSU></distNSU></distDFeInt>")
    }

    fn texto(r: Result<(u16, Vec<u8>), FalhaRede>) -> String {
        String::from_utf8(r.unwrap().1).unwrap()
    }

    #[test]
    fn na_semeadura_so_aparece_a_semente_e_a_fila_diz_que_tem_mais() {
        let e = estado();
        let t = texto(e.responder(&pedido(0), true));
        assert!(t.contains("<cStat>138</cStat>"), "{t}");
        assert!(t.contains("<ultNSU>000000000000002</ultNSU>"), "{t}");
        assert!(t.contains("<maxNSU>000000000000003</maxNSU>"), "{t}");
    }

    #[test]
    fn ao_vivo_entrega_o_resto_e_depois_137() {
        let e = estado();
        let t = texto(e.responder(&pedido(2), false));
        assert!(
            t.contains("<ultNSU>000000000000004</ultNSU>")
                && t.contains("<maxNSU>000000000000004</maxNSU>"),
            "{t}"
        );
        let depois = texto(e.responder(&pedido(4), false));
        assert!(depois.contains("<cStat>137</cStat>"), "{depois}");
    }

    #[test]
    fn os_roteiros_espera_e_vazio() {
        let mut e = estado();
        e.roteiro = RoteiroRecebidas::Espera;
        assert!(texto(e.responder(&pedido(2), false)).contains("<cStat>656</cStat>"));
        e.roteiro = RoteiroRecebidas::Vazio;
        assert!(texto(e.responder(&pedido(2), false)).contains("<cStat>137</cStat>"));
        assert_eq!(
            roteiro_recebidas_do_nome("espera"),
            Some(RoteiroRecebidas::Espera)
        );
        assert_eq!(roteiro_recebidas_do_nome("x"), None);
    }

    #[test]
    fn so_quem_respondeu_recebe_a_nota_completa_e_o_desconhecimento_nao_libera() {
        let mut e = estado();
        let chave = e.docs[0].chave.clone();
        e.docs[0].proc = Some("<nfeProc/>".into());
        let pedido =
            format!("<distDFeInt><consChNFe><chNFe>{chave}</chNFe></consChNFe></distDFeInt>");
        assert!(texto(e.responder(&pedido, false)).contains("resNFe_v1.01.xsd"));
        e.manifestou(&chave, "210220");
        assert!(texto(e.responder(&pedido, false)).contains("resNFe_v1.01.xsd"));
        e.manifestou(&chave, "210210");
        assert!(texto(e.responder(&pedido, false)).contains("procNFe_v4.00.xsd"));
    }
}
