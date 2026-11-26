// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Eventos da apuracao do IBS e da CBS (Reforma Tributaria).
//!
//! NT 2025.002-RTC v1.53, item 8: 13 eventos de NF-e modelo 55 mais o cancelamento generico `110001`, todos recebidos
//! na SVRS (SEFAZ Virtual do Rio Grande do Sul), de um evento por envio (item 8.2). O leiaute de cada um confere com o
//! pacote "Schema dos eventos da NT 2025.002 v1.40 - RTC" do Portal Nacional da NF-e (publicado em 27/07/2026), mais o
//! `e110001` do pacote 010h v1.03 (que inclui os eventos novos na lista de cancelaveis).
//!
//! O `cOrgao` do `infEvento` e' a UF da chave (P08: "Informar o codigo da UF para este evento").
//!
//! Ficaram **fora**: `211120` (eliminado pela propria NT, versao 1.40), `412120` e `412130` (autor Fisco) e `110300`
//! (vinculacao de pagamento, NT 2026.006, evento que a NF-e de hoje nao usa aqui): nenhum e' evento que o contribuinte
//! emitente ou destinatario registre pelos eventos desta NT.

use chrono::{DateTime, Datelike, FixedOffset, NaiveDate};

use super::super::erro::ErroSefaz;
use super::super::pedidos::chave_valida;
use super::super::tabela::uf_valida;
use super::comum::{montar, protocolo, texto_curto, Cabecalho, Montado};
use crate::nfe::montagem::{documento_cnpj_cpf, uf_codigo, Dec};
use crate::nfe::vigencias::Ambiente;

/// `tpEvento` do cancelamento de evento (generico).
pub const CANCELAMENTO_DE_EVENTO: &str = "110001";

/// Um tipo de evento da apuracao.
#[derive(Debug, Clone, Copy)]
pub struct TipoRtc {
    /// `tpEvento`.
    pub tipo: &'static str,
    /// `descEvento`, como o enumerado do XSD (com acento).
    pub descricao: &'static str,
    /// `tpAutor` aceitos (1 emitente, 2 destinatario, 8 sucessora).
    pub autores: &'static [&'static str],
    /// O grupo repetido por item, quando ha.
    pub grupo: Option<&'static str>,
    /// Nomes da tag de quantidade e da de unidade do grupo, quando ha.
    pub quantidade: Option<(&'static str, &'static str)>,
}

const fn t(
    tipo: &'static str,
    descricao: &'static str,
    autores: &'static [&'static str],
    grupo: Option<&'static str>,
    quantidade: Option<(&'static str, &'static str)>,
) -> TipoRtc {
    TipoRtc {
        tipo,
        descricao,
        autores,
        grupo,
        quantidade,
    }
}

/// Os 13 eventos de apuracao e o cancelamento generico, com a descricao do XSD.
pub const EVENTOS: &[TipoRtc] = &[
    t("110001", "Cancelamento de Evento", &[], None, None),
    t(
        "112110",
        "Informação de efetivo pagamento integral para liberar crédito presumido do adquirente",
        &["1"],
        None,
        None,
    ),
    t(
        "112120",
        "Importação em ALC/ZFM não convertida em isenção",
        &["1"],
        Some("gConsumo"),
        Some(("qtde", "unidade")),
    ),
    t(
        "112130",
        "Perecimento, perda, roubo ou furto durante o transporte contratado pelo fornecedor",
        &["1"],
        Some("gPerecimento"),
        Some(("qPerecimento", "uPerecimento")),
    ),
    t(
        "112140",
        "Fornecimento não realizado com pagamento antecipado",
        &["1"],
        Some("gItemNaoFornecido"),
        Some(("qNaoFornecida", "uNaoFornecida")),
    ),
    t(
        "112150",
        "Atualização da Data de Previsão de Entrega",
        &["1"],
        None,
        None,
    ),
    t(
        "211110",
        "Solicitação de Apropriação de crédito presumido",
        &["1", "2"],
        Some("gCredPresOper"),
        None,
    ),
    t(
        "211124",
        "Perecimento, perda, roubo ou furto durante o transporte contratado pelo adquirente",
        &["2"],
        Some("gPerecimento"),
        Some(("qPerecimento", "uPerecimento")),
    ),
    t(
        "211128",
        "Aceite de débito na apuração por emissão de nota de crédito",
        &["2"],
        None,
        None,
    ),
    t(
        "211130",
        "Imobilização de Item",
        &["2"],
        Some("gImobilizacao"),
        Some(("qImobilizado", "uImobilizado")),
    ),
    t(
        "211140",
        "Solicitação de Apropriação de Crédito de Combustível",
        &["2"],
        Some("gConsumoComb"),
        Some(("qComb", "uComb")),
    ),
    t(
        "211150",
        "Solicitação de Apropriação de Crédito para bens e serviços que dependem de atividade do adquirente",
        &["2"],
        Some("gCredito"),
        None,
    ),
    t(
        "212110",
        "Manifestação sobre Pedido de Transferência de Crédito de IBS em Operação de Sucessão",
        &["8"],
        None,
        None,
    ),
    t(
        "212120",
        "Manifestação sobre Pedido de Transferência de Crédito de CBS em Operação de Sucessão",
        &["8"],
        None,
        None,
    ),
];

/// `tpEventoAut` aceitos pelo `110001`: o enumerado do `e110001_v1.00.xsd` do pacote 010h v1.03.
pub const CANCELAVEIS: &[&str] = &[
    "110300", "112110", "112120", "112130", "112140", "112150", "211110", "211120", "211124",
    "211128", "211130", "211140", "211150", "212110", "212120", "412120", "412130", "412140",
    "420110", "420120", "420130", "420140", "420150", "420160", "420170",
];

/// O tipo de evento de apuracao, se `tipo` e' um.
#[must_use]
pub fn tipo_rtc(tipo: &str) -> Option<&'static TipoRtc> {
    EVENTOS.iter().find(|e| e.tipo == tipo)
}

/// Um item (`det/@nItem` da NF-e) de um evento por item. Os valores vao em texto decimal (`10.50` ou `10,50`). O que
/// nao existe no grupo do tipo tem de ficar `None`: informado e' erro, porque costuma ser campo trocado.
#[derive(Debug, Clone, Default)]
pub struct ItemRtc {
    /// `nItem`: o item da NF-e, de 1 a 990.
    pub item: u32,
    /// `vIBS` (ou `vCredIBS` no 211150).
    pub v_ibs: Option<String>,
    /// `vCBS` (ou `vCredCBS` no 211150).
    pub v_cbs: Option<String>,
    /// Quantidade (`qtde`, `qPerecimento`...), ate 11 inteiros e 4 casas.
    pub quantidade: Option<String>,
    /// Unidade (ate 6 caracteres).
    pub unidade: Option<String>,
    /// So' no 112130: `vIBS` do estorno do credito.
    pub v_ibs_estorno: Option<String>,
    /// So' no 112130: `vCBS` do estorno do credito.
    pub v_cbs_estorno: Option<String>,
    /// So' no 211110: `vBCCredPres`.
    pub v_bc: Option<String>,
    /// So' no 211110: `cCredPres`, 2 digitos.
    pub c_cred_pres: Option<String>,
    /// So' no 211110: `pCredPres` do IBS.
    pub p_cred_ibs: Option<String>,
    /// So' no 211110: `vCredPres` do IBS.
    pub v_cred_ibs: Option<String>,
    /// So' no 211110: `pCredPres` da CBS.
    pub p_cred_cbs: Option<String>,
    /// So' no 211110: `vCredPres` da CBS.
    pub v_cred_cbs: Option<String>,
}

/// Os dados de um evento de apuracao.
#[derive(Debug, Clone)]
pub struct EventoRtc<'a> {
    /// A chave da NF-e (modelo 55).
    pub chave: &'a str,
    /// `tpEvento` (um de [`EVENTOS`]).
    pub tipo: &'a str,
    /// `nSeqEvento`, de 1 a 99. No `110001` e' a sequencia do evento que se cancela.
    pub seq: u32,
    /// Ambiente.
    pub ambiente: Ambiente,
    /// Data e hora do evento.
    pub quando: DateTime<FixedOffset>,
    /// Versao do aplicativo do autor (ate 20 caracteres).
    pub ver_aplic: &'a str,
    /// CNPJ ou CPF de quem registra; `None` = o emitente da chave.
    pub autor: Option<&'a str>,
    /// UF (sigla ou codigo) de quem registra; `None` = a da chave, so' se o autor e' o emitente.
    pub uf_autor: Option<&'a str>,
    /// So' o `211110` escolhe (1 ou 2); `None` = sai do autor.
    pub tp_autor: Option<&'a str>,
    /// Os itens, nos eventos por item.
    pub itens: Vec<ItemRtc>,
    /// `indAceitacao` do 211128, 212110 e 212120.
    pub aceite: Option<bool>,
    /// `dPrevEntrega` do 112150.
    pub data_entrega: Option<NaiveDate>,
    /// `tpEventoAut` do 110001: o evento que se cancela.
    pub evento_cancelado: &'a str,
    /// `nProtEvento` do 110001: o protocolo do evento que se cancela.
    pub protocolo_evento: &'a str,
}

impl<'a> EventoRtc<'a> {
    /// Evento sem autor distinto do emitente, sem itens e sem os campos de tipo; preencha o que o tipo pede.
    #[must_use]
    pub fn novo(
        chave: &'a str,
        tipo: &'a str,
        seq: u32,
        ambiente: Ambiente,
        quando: DateTime<FixedOffset>,
        ver_aplic: &'a str,
    ) -> Self {
        EventoRtc {
            chave,
            tipo,
            seq,
            ambiente,
            quando,
            ver_aplic,
            autor: None,
            uf_autor: None,
            tp_autor: None,
            itens: Vec::new(),
            aceite: None,
            data_entrega: None,
            evento_cancelado: "",
            protocolo_evento: "",
        }
    }
}

/// Numero do XSD: ate `inteiros` digitos inteiros e `casas` casas fixas (ou de `casas` a `casas_max`).
fn valor(
    v: Option<&String>,
    campo: &str,
    inteiros: usize,
    casas: u32,
    casas_max: Option<u32>,
) -> Result<String, ErroSefaz> {
    let Some(s) = v.filter(|s| !s.trim().is_empty()) else {
        return Err(ErroSefaz::novo(format!("{campo}: obrigatorio"), campo));
    };
    let d = Dec::parse(s, campo).map_err(|e| ErroSefaz::novo(e.mensagem, campo))?;
    if d.negativo() {
        return Err(ErroSefaz::novo(
            format!("{campo}: valor negativo ({})", d.fixo(d.casas())),
            campo,
        ));
    }
    let teto = casas_max.unwrap_or(casas);
    if d.casas() > teto {
        return Err(ErroSefaz::novo(
            format!(
                "{campo}: {} tem mais de {teto} casas decimais",
                d.fixo(d.casas())
            ),
            campo,
        ));
    }
    let saida = d.fixo(if casas_max.is_some() {
        casas.max(d.casas())
    } else {
        casas
    });
    if saida.split('.').next().unwrap_or("").len() > inteiros {
        return Err(ErroSefaz::novo(
            format!("{campo}: {saida} passa de {inteiros} digitos inteiros"),
            campo,
        ));
    }
    Ok(saida)
}

/// Os campos de `it` que o grupo nao tem.
fn campos_que_sobram(tipo: &str, grupo: &str, it: &ItemRtc) -> Vec<&'static str> {
    let presentes: [(&'static str, bool); 12] = [
        ("c_cred_pres", it.c_cred_pres.is_some()),
        ("p_cred_cbs", it.p_cred_cbs.is_some()),
        ("p_cred_ibs", it.p_cred_ibs.is_some()),
        ("quantidade", it.quantidade.is_some()),
        ("unidade", it.unidade.is_some()),
        ("v_bc", it.v_bc.is_some()),
        ("v_cbs", it.v_cbs.is_some()),
        ("v_cbs_estorno", it.v_cbs_estorno.is_some()),
        ("v_cred_cbs", it.v_cred_cbs.is_some()),
        ("v_cred_ibs", it.v_cred_ibs.is_some()),
        ("v_ibs", it.v_ibs.is_some()),
        ("v_ibs_estorno", it.v_ibs_estorno.is_some()),
    ];
    presentes
        .into_iter()
        .filter(|(k, tem)| {
            let aceita = match grupo {
                "gCredPresOper" => matches!(
                    *k,
                    "v_bc"
                        | "c_cred_pres"
                        | "p_cred_ibs"
                        | "v_cred_ibs"
                        | "p_cred_cbs"
                        | "v_cred_cbs"
                ),
                "gCredito" => matches!(*k, "v_ibs" | "v_cbs"),
                _ => {
                    matches!(*k, "v_ibs" | "v_cbs" | "quantidade" | "unidade")
                        || (tipo == "112130" && matches!(*k, "v_ibs_estorno" | "v_cbs_estorno"))
                }
            };
            *tem && !aceita
        })
        .map(|(k, _)| k)
        .collect()
}

fn credito_presumido(it: &ItemRtc) -> Result<String, ErroSefaz> {
    let c = it.c_cred_pres.as_deref().unwrap_or("").trim().to_string();
    if c.len() != 2 || !c.bytes().all(|b| b.is_ascii_digit()) {
        return Err(ErroSefaz::novo(
            "cCredPres: 2 digitos (tabela cCredPres, Anexo IV da NT)",
            "cCredPres",
        ));
    }
    let mut m = format!(
        "<vBCCredPres>{}</vBCCredPres><cCredPres>{c}</cCredPres>",
        valor(it.v_bc.as_ref(), "vBCCredPres", 13, 2, None)?
    );
    for (g, p, v) in [
        ("gIBSCredPres", &it.p_cred_ibs, &it.v_cred_ibs),
        ("gCBSCredPres", &it.p_cred_cbs, &it.v_cred_cbs),
    ] {
        if p.is_none() && v.is_none() {
            continue;
        }
        m.push_str(&format!(
            "<{g}><pCredPres>{}</pCredPres><vCredPres>{}</vCredPres></{g}>",
            valor(p.as_ref(), "pCredPres", 3, 2, Some(4))?,
            valor(v.as_ref(), "vCredPres", 13, 2, None)?
        ));
    }
    Ok(m)
}

fn com_estoque(
    tipo: &str,
    quantidade: Option<(&str, &str)>,
    it: &ItemRtc,
) -> Result<String, ErroSefaz> {
    let (tq, tu) = quantidade.unwrap_or(("qtde", "unidade"));
    let mut estoque = format!(
        "<{tq}>{}</{tq}><{tu}>{}</{tu}>",
        valor(it.quantidade.as_ref(), tq, 11, 4, None)?,
        texto_curto(it.unidade.as_deref().unwrap_or(""), tu, 6)?
    );
    if tipo == "112130" {
        estoque.push_str(&format!(
            "<vIBS>{}</vIBS><vCBS>{}</vCBS>",
            valor(it.v_ibs_estorno.as_ref(), "vIBS", 13, 2, None)?,
            valor(it.v_cbs_estorno.as_ref(), "vCBS", 13, 2, None)?
        ));
    }
    Ok(format!(
        "<vIBS>{}</vIBS><vCBS>{}</vCBS><gControleEstoque>{estoque}</gControleEstoque>",
        valor(it.v_ibs.as_ref(), "vIBS", 13, 2, None)?,
        valor(it.v_cbs.as_ref(), "vCBS", 13, 2, None)?
    ))
}

fn itens(
    tipo: &str,
    grupo: &str,
    quantidade: Option<(&str, &str)>,
    lista: &[ItemRtc],
) -> Result<String, ErroSefaz> {
    if lista.is_empty() {
        return Err(ErroSefaz::novo(
            format!("evento {tipo}: informe ao menos 1 item ({grupo})"),
            grupo,
        ));
    }
    if lista.len() > 990 {
        return Err(ErroSefaz::novo(
            format!("{grupo}: {} itens, maximo 990", lista.len()),
            grupo,
        ));
    }
    let mut vistos = std::collections::BTreeSet::new();
    let mut out = String::new();
    for it in lista {
        let sobra = campos_que_sobram(tipo, grupo, it);
        if !sobra.is_empty() {
            return Err(ErroSefaz::novo(
                format!(
                    "{grupo}: campo(s) {} nao existe(m) no evento {tipo}",
                    sobra.join(", ")
                ),
                grupo,
            ));
        }
        if !(1..=990).contains(&it.item) {
            return Err(ErroSefaz::novo(
                format!("{grupo}: nItem de 1 a 990 (det/@nItem da NF-e)"),
                "nItem",
            ));
        }
        if !vistos.insert(it.item) {
            return Err(ErroSefaz::novo(
                format!("{grupo}: nItem {} repetido", it.item),
                "nItem",
            ));
        }
        let miolo = match grupo {
            "gCredPresOper" => credito_presumido(it)?,
            "gCredito" => format!(
                "<vCredIBS>{}</vCredIBS><vCredCBS>{}</vCredCBS>",
                valor(it.v_ibs.as_ref(), "vCredIBS", 13, 2, None)?,
                valor(it.v_cbs.as_ref(), "vCredCBS", 13, 2, None)?
            ),
            _ => com_estoque(tipo, quantidade, it)?,
        };
        out.push_str(&format!("<{grupo} nItem=\"{}\">{miolo}</{grupo}>", it.item));
    }
    Ok(out)
}

/// De quem e' o evento: (`CNPJ` ou `CPF`, documento, autor e' o emitente).
fn autor_do_evento(
    e: &EventoRtc<'_>,
    c: &crate::chave_dfe::Chave,
) -> Result<(&'static str, String, bool), ErroSefaz> {
    let tag_emitente = if c.emitente_cpf { "CPF" } else { "CNPJ" };
    let informado = e.autor.map(str::trim).filter(|s| !s.is_empty());
    let (tag, doc) = match informado {
        None => (tag_emitente, c.emitente.clone()),
        Some(a) => {
            documento_cnpj_cpf(a, "CNPJ").map_err(|x| ErroSefaz::novo(x.mensagem, "CNPJ"))?
        }
    };
    let e_emitente = doc == c.emitente;
    Ok((tag, doc, e_emitente))
}

/// O `tpAutor` do evento: o unico do tipo, ou o escolhido, ou 1 se o autor e' o emitente e 2 se nao.
fn tp_autor(e: &EventoRtc<'_>, autores: &[&str], e_emitente: bool) -> Result<String, ErroSefaz> {
    match e.tp_autor.map(str::trim).filter(|s| !s.is_empty()) {
        None if autores.len() == 1 => Ok(autores[0].to_string()),
        None => Ok(if e_emitente { "1" } else { "2" }.to_string()),
        Some(t) if autores.contains(&t) => Ok(t.to_string()),
        Some(_) => Err(ErroSefaz::novo(
            format!("tpAutor do evento {}: {}", e.tipo, autores.join(" ou ")),
            "tpAutor",
        )),
    }
}

fn c_orgao_autor(
    e: &EventoRtc<'_>,
    c: &crate::chave_dfe::Chave,
    e_emitente: bool,
) -> Result<u32, ErroSefaz> {
    match e.uf_autor.map(str::trim).filter(|s| !s.is_empty()) {
        None if e_emitente => Ok(c.cuf),
        None => Err(ErroSefaz::novo(
            "cOrgaoAutor: informe a UF do autor do evento",
            "cOrgaoAutor",
        )),
        Some(u) => {
            let sigla = uf_valida(u)
                .map_err(|_| ErroSefaz::novo("cOrgaoAutor: UF invalida", "cOrgaoAutor"))?;
            Ok(uf_codigo(sigla).unwrap_or(0))
        }
    }
}

fn detalhe_do_cancelamento(e: &EventoRtc<'_>, cab: &str, va: &str) -> Result<String, ErroSefaz> {
    let ev = e.evento_cancelado.trim();
    if !CANCELAVEIS.contains(&ev) {
        return Err(ErroSefaz::novo(
            "tpEventoAut: informe o evento a cancelar (um da lista do e110001; o proprio 110001 nao se cancela)",
            "tpEventoAut",
        ));
    }
    let p = protocolo(e.protocolo_evento, "nProtEvento")?;
    Ok(format!(
        "{cab}<verAplic>{va}</verAplic><tpEventoAut>{ev}</tpEventoAut><nProtEvento>{p}</nProtEvento>"
    ))
}

fn detalhe_do_evento(
    e: &EventoRtc<'_>,
    tipo: &TipoRtc,
    cab: &str,
    tp: &str,
    va: &str,
) -> Result<String, ErroSefaz> {
    let mut d = format!("{cab}<tpAutor>{tp}</tpAutor><verAplic>{va}</verAplic>");
    match e.tipo {
        "112110" => d.push_str("<indQuitacao>1</indQuitacao>"),
        "112150" => {
            let Some(data) = e.data_entrega else {
                return Err(ErroSefaz::novo(
                    "dPrevEntrega: informe a data",
                    "dPrevEntrega",
                ));
            };
            if !(2000..=2099).contains(&data.year()) {
                return Err(ErroSefaz::novo(
                    "dPrevEntrega: ano de 2000 a 2099",
                    "dPrevEntrega",
                ));
            }
            d.push_str(&format!(
                "<dPrevEntrega>{}</dPrevEntrega>",
                data.format("%Y-%m-%d")
            ));
        }
        "211128" | "212110" | "212120" => {
            let Some(a) = e.aceite else {
                return Err(ErroSefaz::novo(
                    "indAceitacao: informe aceite (true) ou nao aceite (false)",
                    "indAceitacao",
                ));
            };
            d.push_str(&format!("<indAceitacao>{}</indAceitacao>", u8::from(a)));
        }
        _ => d.push_str(&itens(
            e.tipo,
            tipo.grupo.unwrap_or(""),
            tipo.quantidade,
            &e.itens,
        )?),
    }
    Ok(d)
}

/// Monta o evento de apuracao (sem assinatura).
///
/// # Errors
///
/// [`ErroSefaz`] com o campo e o defeito (e a rejeicao 574 da NT quando o autor nao bate com o emitente).
pub fn evento_rtc(e: &EventoRtc<'_>) -> Result<Montado, ErroSefaz> {
    let c = chave_valida(e.chave)?;
    let Some(tipo) = tipo_rtc(e.tipo) else {
        return Err(ErroSefaz::novo(
            format!(
                "evento {} nao e' evento de apuracao do IBS/CBS da NT 2025.002",
                e.tipo
            ),
            "tpEvento",
        ));
    };
    if !(1..=99).contains(&e.seq) {
        return Err(ErroSefaz::novo("nSeqEvento: de 1 a 99", "nSeqEvento"));
    }
    let (autor_tag, autor_doc, e_emitente) = autor_do_evento(e, &c)?;
    let cancelamento = e.tipo == CANCELAMENTO_DE_EVENTO;
    if cancelamento && e.tp_autor.map(str::trim).is_some_and(|s| !s.is_empty()) {
        return Err(ErroSefaz::novo(
            "tpAutor nao existe no cancelamento de evento (110001)",
            "tpAutor",
        ));
    }
    let tp = if cancelamento {
        String::new()
    } else {
        tp_autor(e, tipo.autores, e_emitente)?
    };
    if tp == "1" && !e_emitente {
        return Err(ErroSefaz::novo(
            "autor do evento diverge do emitente da NF-e (rejeicao 574)",
            "CNPJ",
        ));
    }
    if (tp == "2" || tp == "8") && e_emitente && e.autor.is_none() {
        let quem = if tp == "2" {
            "destinatario"
        } else {
            "sucessora"
        };
        return Err(ErroSefaz::novo(
            format!("evento {}: informe o CNPJ ou CPF do autor ({quem})", e.tipo),
            "CNPJ",
        ));
    }
    let orgao_autor = c_orgao_autor(e, &c, e_emitente)?;
    let va = texto_curto(e.ver_aplic, "verAplic", 20)?;
    let cab = format!(
        "<descEvento>{}</descEvento><cOrgaoAutor>{orgao_autor}</cOrgaoAutor>",
        tipo.descricao
    );
    let det = if cancelamento {
        detalhe_do_cancelamento(e, &cab, &va)?
    } else {
        detalhe_do_evento(e, tipo, &cab, &tp, &va)?
    };
    Ok(montar(
        &Cabecalho {
            c_orgao: c.cuf,
            ambiente: e.ambiente,
            autor_tag,
            autor: &autor_doc,
            chave: &c.chave,
            tp_evento: e.tipo,
            seq: e.seq,
            uf_fuso: c.uf,
            quando: &e.quando,
        },
        &det,
    ))
}
