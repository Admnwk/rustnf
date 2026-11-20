// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Pedido de registro de evento (`pedRegEvento` v1.01): cancelamento, analise fiscal e manifestacoes.
//! Contrato: `tests/casos/nfse_nacional.json`, secao `eventos`. O
//! contribuinte so' monta o `PRE` (59); 105102 nasce da DPS de substituicao; os do fisco sao recusados.

use super::base::{
    casa_chave_evento, data_hora, documento, so_alnum, texto, ErroNfse, No, Txt, NS, R, VERSAO,
};
use crate::chave_dfe;

pub const CANCELAMENTO: &str = "101101";
pub const SUBSTITUICAO: &str = "105102";
pub const ANALISE_FISCAL: &str = "101103";
pub const MOTIVO_MIN: usize = 15;
pub const MOTIVO_MAX: usize = 255;

/// (codigo, xDesc EXATO do XSD, autor no Anexo II).
pub const TIPOS: &[(&str, &str, &str)] = &[
    ("101101", "Cancelamento de NFS-e", "emitente"),
    (
        "105102",
        "Cancelamento de NFS-e por Substituição",
        "sistema",
    ),
    (
        "101103",
        "Solicitação de Análise Fiscal para Cancelamento de NFS-e",
        "emitente",
    ),
    (
        "105104",
        "Cancelamento de NFS-e Deferido por Análise Fiscal",
        "fisco",
    ),
    (
        "105105",
        "Cancelamento de NFS-e Indeferido por Análise Fiscal",
        "fisco",
    ),
    (
        "202201",
        "Manifestação de NFS-e - Confirmação do Prestador",
        "prestador",
    ),
    (
        "203202",
        "Manifestação de NFS-e - Confirmação do Tomador",
        "tomador",
    ),
    (
        "204203",
        "Manifestação de NFS-e - Confirmação do Intermediário",
        "intermediario",
    ),
    (
        "205204",
        "Manifestação de NFS-e - Confirmação Tácita",
        "fisco",
    ),
    (
        "202205",
        "Manifestação de NFS-e - Rejeição do Prestador",
        "prestador",
    ),
    (
        "203206",
        "Manifestação de NFS-e - Rejeição do Tomador",
        "tomador",
    ),
    (
        "204207",
        "Manifestação de NFS-e - Rejeição do Intermediário",
        "intermediario",
    ),
    (
        "205208",
        "Manifestação de NFS-e - Anulação da Rejeição",
        "fisco",
    ),
    ("305101", "Cancelamento de NFS-e por Ofício", "fisco"),
    ("305102", "Bloqueio de NFS-e por Ofício", "fisco"),
    ("305103", "Desbloqueio de NFS-e por Ofício", "fisco"),
];

pub const MOTIVOS_CANCELAMENTO: &[(&str, &str)] = &[
    ("1", "Erro na emissao"),
    ("2", "Servico nao prestado"),
    ("9", "Outros"),
];
pub const MOTIVOS_REJEICAO: &[(&str, &str)] = &[
    ("1", "Duplicidade de nota"),
    ("2", "Ja emitida pelo tomador"),
    ("3", "Nao ocorrencia do fato gerador"),
    ("4", "Erro quanto a responsabilidade tributaria"),
    ("5", "Erro quanto ao valor, servico ou data"),
    ("9", "Outros"),
];
const REJEICOES: &[&str] = &["202205", "203206", "204207"];

/// Chave de 50 que o pedRegEvento aceita (intersecao TSChaveNFSe x TSIdPedRegEvt) e com DV conferido.
pub fn chave_do_evento(chave: &str) -> R<String> {
    let ch = so_alnum(chave);
    if ch.len() != 50 {
        return Err(ErroNfse::novo(
            "infPedReg/chNFSe",
            "a chave da NFS-e Nacional tem 50 posicoes",
        ));
    }
    if !casa_chave_evento(&ch) {
        return Err(ErroNfse::novo(
            "infPedReg/chNFSe",
            "chave fora do layout: municipio(7) + ambiente(1) + tipo de \
            inscricao (1 CPF / 2 CNPJ) + inscricao(14) + digitos; letra so' no CNPJ",
        ));
    }
    if chave_dfe::conferir(&ch).is_err() {
        return Err(ErroNfse::novo(
            "infPedReg/chNFSe",
            "digito verificador da chave nao confere",
        ));
    }
    Ok(ch)
}

/// Pedido pronto para assinar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pedido {
    pub tipo: String,
    pub chave: String,
    pub id: String,
    pub xml: String,
}

fn lista(m: &[(&str, &str)]) -> String {
    m.iter()
        .map(|(k, v)| format!("{k} ({v})"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// O que vai no pedido. `autor` = CPF/CNPJ do titular do certificado que vai assinar (E0812/E0815);
/// `dh_evento` em ISO 8601 (None = agora, -03:00).
#[derive(Debug, Clone, Copy)]
pub struct DadosEvento<'a> {
    pub tipo: &'a str,
    pub chave: &'a str,
    pub autor: &'a str,
    pub motivo: &'a str,
    pub justificativa: &'a str,
    pub ambiente: u8,
    pub dh_evento: Option<&'a str>,
    pub ver_aplic: &'a str,
}

impl Default for DadosEvento<'_> {
    fn default() -> Self {
        DadosEvento {
            tipo: CANCELAMENTO,
            chave: "",
            autor: "",
            motivo: "",
            justificativa: "",
            ambiente: 2,
            dh_evento: None,
            ver_aplic: "rustnf",
        }
    }
}

/// Monta o pedRegEvento (NAO assinado).
pub fn montar(d: &DadosEvento) -> R<Pedido> {
    let DadosEvento {
        tipo,
        chave,
        autor,
        motivo,
        justificativa: texto_motivo,
        ambiente,
        dh_evento,
        ver_aplic,
    } = *d;
    let (_, desc, quem) = TIPOS.iter().find(|t| t.0 == tipo).ok_or_else(|| {
        ErroNfse::novo("infPedReg", format!("tipo de evento desconhecido: {tipo}"))
    })?;
    if *quem == "sistema" {
        return Err(ErroNfse::novo(
            "infPedReg",
            "substituicao nao e' pedido de evento: emita uma DPS nova com o grupo de \
            substituicao (Substituicao); o SEFIN registra o 105102 sozinho",
        ));
    }
    if *quem == "fisco" {
        return Err(ErroNfse::novo(
            "infPedReg",
            format!("o evento {tipo} ({desc}) e' registrado pelo fisco, nao pelo contribuinte"),
        ));
    }
    let ch = chave_do_evento(chave)?;
    let (tp_autor, doc) = documento(autor)
        .map_err(|_| ErroNfse::novo("infPedReg/CNPJAutor", "autor sem CPF/CNPJ valido"))?;
    let mot = motivo.trim();
    let just = texto(
        texto_motivo,
        "infPedReg/xMotivo",
        super::base::Txt {
            minimo: MOTIVO_MIN,
            ..Txt::opc(MOTIVO_MAX)
        },
    )?;
    let ident = format!("PRE{ch}{tipo}");
    let mut raiz = No::new("pedRegEvento").attr("versao", VERSAO);
    let mut inf = No::new("infPedReg").attr("Id", &ident);
    inf.add(
        "tpAmb",
        if ambiente == 1 || ambiente == 2 {
            ambiente.to_string()
        } else {
            "2".into()
        },
    );
    inf.add(
        "verAplic",
        texto(ver_aplic, "infPedReg/verAplic", Txt::max(20))?,
    );
    inf.add("dhEvento", data_hora(dh_evento, "infPedReg/dhEvento")?);
    inf.add(
        if tp_autor == "CPF" {
            "CPFAutor"
        } else {
            "CNPJAutor"
        },
        doc,
    );
    inf.add("chNFSe", ch.clone());
    let ev = inf.grupo(&format!("e{tipo}"));
    ev.add("xDesc", *desc);
    if tipo == CANCELAMENTO || tipo == ANALISE_FISCAL {
        if !MOTIVOS_CANCELAMENTO.iter().any(|m| m.0 == mot) {
            return Err(ErroNfse::novo(
                format!("infPedReg/e{tipo}/cMotivo"),
                format!("motivo: {}", lista(MOTIVOS_CANCELAMENTO)),
            ));
        }
        if just.is_empty() {
            return Err(ErroNfse::novo(format!("infPedReg/e{tipo}/xMotivo"), format!(
                "a justificativa e' obrigatoria, de {MOTIVO_MIN} a {MOTIVO_MAX} caracteres -- ela fica no documento fiscal")));
        }
        ev.add("cMotivo", mot);
        ev.add("xMotivo", just);
    } else if REJEICOES.contains(&tipo) {
        if !MOTIVOS_REJEICAO.iter().any(|m| m.0 == mot) {
            return Err(ErroNfse::novo(
                format!("infPedReg/e{tipo}/cMotivo"),
                format!("motivo: {}", lista(MOTIVOS_REJEICAO)),
            ));
        }
        if mot == "9" && just.is_empty() {
            return Err(ErroNfse::novo(format!("infPedReg/e{tipo}/xMotivo"),
                "com motivo 9 (Outros) a justificativa e' obrigatoria (regra E1944/E1949/E1954 do SEFIN)"));
        }
        ev.add("cMotivo", mot);
        if !just.is_empty() {
            ev.add("xMotivo", just);
        }
    }
    raiz.anexar(inf);
    Ok(Pedido {
        tipo: tipo.into(),
        chave: ch,
        id: ident,
        xml: format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>{}",
            raiz.xml(Some(NS))
        ),
    })
}

/// Atalho do 101101.
pub fn cancelamento(
    chave: &str,
    autor: &str,
    motivo: &str,
    justificativa: &str,
    ambiente: u8,
) -> R<Pedido> {
    montar(&DadosEvento {
        chave,
        autor,
        motivo,
        justificativa,
        ambiente,
        ..Default::default()
    })
}
