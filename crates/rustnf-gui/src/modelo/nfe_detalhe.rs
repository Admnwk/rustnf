// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O detalhe de uma NF-e: o que a tela mostra do XML guardado na pasta (emitente, destinatario, itens resumidos,
//! totais, protocolo) e o que o indice sabe (situacao, eventos, tentativas). O destinatario aparece inteiro, como
//! no DANFE. Sem janela.

use rustnf::nfe::{self, Decimal, Item, Nfe};
use rustnf_app::notas::nfe::{Registro, Situacao};

use super::chave;
use super::mascara::{decimal_para_dinheiro, formatar_documento, iso_para_data_br};
use super::nfe_lista::aviso_da_situacao;

/// Quantos itens o detalhe lista antes de resumir o resto.
pub const ITENS_VISIVEIS: usize = 5;

/// `tpEvento` do cancelamento e da carta de correcao, como o indice os guarda.
const CANCELAMENTO: &str = "110111";
const CARTA: &str = "110110";

/// Um item da nota, resumido.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemNfe {
    /// A descricao do produto.
    pub descricao: String,
    /// Quantidade e unidade (`2 UN`).
    pub quantidade: String,
    /// O valor do item, em dinheiro brasileiro.
    pub valor: String,
}

/// O que a tela mostra de uma NF-e.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetalheNfe {
    /// A chave de acesso (44 posicoes), sem separador.
    pub chave: String,
    /// A chave em grupos de 4, para ler e copiar.
    pub chave_em_grupos: String,
    /// O numero e a serie (`nº 12, série 1`).
    pub numero: String,
    /// A situacao da nota na pasta.
    pub situacao: Situacao,
    /// A emissao, `DD/MM/AAAA HH:MM`.
    pub emissao: String,
    /// A natureza da operacao.
    pub natureza: String,
    /// O nome do emitente.
    pub emitente: String,
    /// O documento do emitente, formatado.
    pub emitente_documento: String,
    /// O nome do destinatario (`-` se a nota nao o identifica).
    pub destinatario: String,
    /// O documento do destinatario, formatado (vazio se nao ha).
    pub destinatario_documento: String,
    /// Os primeiros itens.
    pub itens: Vec<ItemNfe>,
    /// Quantos itens ficaram de fora do resumo.
    pub itens_a_mais: usize,
    /// O total dos produtos.
    pub total_produtos: String,
    /// O desconto (vazio se nao ha).
    pub desconto: String,
    /// O frete (vazio se nao ha).
    pub frete: String,
    /// O total da nota.
    pub total: String,
    /// O protocolo de autorizacao (vazio enquanto pendente).
    pub protocolo: String,
    /// O protocolo do cancelamento, se a nota foi cancelada.
    pub protocolo_do_cancelamento: String,
    /// Quantas cartas de correcao a nota ja tem.
    pub correcoes: usize,
    /// A nota e' de producao.
    pub producao: bool,
    /// O que aconteceu e o que fazer (so' pendente, EPEC e denegada).
    pub aviso: Option<String>,
}

/// O que se leu do XML guardado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LeituraNfe {
    /// Leu.
    Lida(Box<DetalheNfe>),
    /// O XML nao esta na pasta: so' o que o indice sabe.
    Ausente(Box<DetalheNfe>),
    /// O XML esta na pasta e nao pode ser lido.
    Ilegivel(Box<DetalheNfe>),
}

fn dinheiro(d: Decimal) -> String {
    decimal_para_dinheiro(&d.to_string())
}

fn opcional(d: Decimal) -> String {
    if d == Decimal::ZERO {
        String::new()
    } else {
        dinheiro(d)
    }
}

fn data_e_hora(iso: &str) -> String {
    match (iso.get(..10), iso.get(11..16)) {
        (Some(d), Some(h)) => format!("{} {h}", iso_para_data_br(d)),
        _ => iso.to_string(),
    }
}

fn item(i: &Item) -> ItemNfe {
    let quantidade = i.quantidade.map_or_else(String::new, |q| q.to_string());
    ItemNfe {
        descricao: i.descricao.clone(),
        quantidade: format!("{quantidade} {}", i.unidade).trim().to_string(),
        valor: i
            .valor_produto
            .or(i.valor_item)
            .map_or_else(|| "-".into(), dinheiro),
    }
}

/// Os dados que so' o indice sabe, para a nota sem XML legivel.
fn do_registro(r: &Registro) -> DetalheNfe {
    DetalheNfe {
        chave: r.chave.clone(),
        chave_em_grupos: em_grupos_de_4(&r.chave),
        numero: format!("nº {}, série {}", r.numero, r.serie),
        situacao: r.situacao,
        emissao: data_e_hora(&r.emissao),
        natureza: String::new(),
        emitente: String::new(),
        emitente_documento: String::new(),
        destinatario: if r.destinatario_nome.is_empty() {
            "-".into()
        } else {
            r.destinatario_nome.clone()
        },
        destinatario_documento: formatar_documento(&r.destinatario_documento),
        itens: Vec::new(),
        itens_a_mais: 0,
        total_produtos: String::new(),
        desconto: String::new(),
        frete: String::new(),
        total: decimal_para_dinheiro(&r.valor),
        protocolo: r.protocolo.clone(),
        protocolo_do_cancelamento: r
            .eventos
            .iter()
            .find(|e| e.tipo == CANCELAMENTO)
            .map(|e| e.protocolo.clone())
            .unwrap_or_default(),
        correcoes: r.eventos.iter().filter(|e| e.tipo == CARTA).count(),
        producao: r.ambiente == "producao",
        aviso: aviso_da_situacao(r),
    }
}

fn em_grupos_de_4(chave: &str) -> String {
    chave
        .as_bytes()
        .chunks(4)
        .map(|c| String::from_utf8_lossy(c).into_owned())
        .collect::<Vec<_>>()
        .join(" ")
}

impl LeituraNfe {
    /// A leitura do `xml` guardado (`None`: nao esta na pasta) para o registro do indice.
    #[must_use]
    pub fn do_xml(xml: Option<&str>, r: &Registro) -> Self {
        let base = do_registro(r);
        let Some(xml) = xml else {
            return Self::Ausente(Box::new(base));
        };
        let Ok(n) = nfe::ler(xml.as_bytes()) else {
            return Self::Ilegivel(Box::new(base));
        };
        Self::Lida(Box::new(completar(base, &n)))
    }

    /// O detalhe, lido ou so' do indice.
    #[must_use]
    pub fn detalhe(&self) -> &DetalheNfe {
        match self {
            Self::Lida(d) | Self::Ausente(d) | Self::Ilegivel(d) => d,
        }
    }
}

fn completar(mut d: DetalheNfe, n: &Nfe) -> DetalheNfe {
    d.natureza.clone_from(&n.ide.natureza_operacao);
    d.emitente.clone_from(&n.emit.nome);
    d.emitente_documento = formatar_documento(&n.emit.documento);
    if let Some(dest) = &n.dest {
        d.destinatario = if dest.nome.is_empty() {
            "-".into()
        } else {
            dest.nome.clone()
        };
        d.destinatario_documento = formatar_documento(&dest.documento);
    }
    d.itens = n.itens.iter().take(ITENS_VISIVEIS).map(item).collect();
    d.itens_a_mais = n.itens.len().saturating_sub(ITENS_VISIVEIS);
    d.total_produtos = dinheiro(n.total.v("vProd"));
    d.desconto = opcional(n.total.v("vDesc"));
    d.frete = opcional(n.total.v("vFrete"));
    d.total = dinheiro(n.total.v("vNF"));
    if let Some(p) = &n.protocolo {
        if d.protocolo.is_empty() {
            d.protocolo.clone_from(&p.numero);
        }
    }
    if n.chave.len() == 44 && chave::limpar(&n.chave).len() == 44 {
        d.chave.clone_from(&n.chave);
        d.chave_em_grupos = em_grupos_de_4(&n.chave);
    }
    d
}

#[cfg(test)]
mod testes {
    use super::*;
    use rustnf_app::notas::nfe::EventoGuardado;

    fn registro() -> Registro {
        Registro {
            chave: "35261011222333000181550010000000011000000011".into(),
            serie: "1".into(),
            numero: "1".into(),
            emissao: "2026-10-03T09:00:00-03:00".into(),
            valor: "31.80".into(),
            ambiente: "producao".into(),
            situacao: Situacao::Cancelada,
            protocolo: "135260000000001".into(),
            destinatario_documento: "52998224725".into(),
            destinatario_nome: "CLIENTE TESTE".into(),
            eventos: vec![
                EventoGuardado {
                    tipo: CARTA.into(),
                    sequencia: 1,
                    protocolo: "1".into(),
                },
                EventoGuardado {
                    tipo: CANCELAMENTO.into(),
                    sequencia: 1,
                    protocolo: "135260000000009".into(),
                },
            ],
            ..Registro::default()
        }
    }

    #[test]
    fn sem_xml_o_detalhe_mostra_o_que_o_indice_sabe() {
        let l = LeituraNfe::do_xml(None, &registro());
        assert!(matches!(l, LeituraNfe::Ausente(_)));
        let d = l.detalhe();
        assert_eq!(d.numero, "nº 1, série 1");
        assert_eq!(d.emissao, "03/10/2026 09:00");
        assert_eq!(d.total, "31,80");
        assert_eq!(d.destinatario_documento, "529.982.247-25");
        assert_eq!(d.protocolo_do_cancelamento, "135260000000009");
        assert_eq!(d.correcoes, 1);
        assert!(d.producao);
        assert!(d.chave_em_grupos.starts_with("3526 1011 2223"));
    }

    #[test]
    fn xml_que_nao_le_nao_derruba_a_tela() {
        let l = LeituraNfe::do_xml(Some("<isto nao e uma nota"), &registro());
        assert!(matches!(l, LeituraNfe::Ilegivel(_)));
        assert_eq!(l.detalhe().total, "31,80");
    }
}
