// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O cupom do caixa (NFC-e, modelo 65): os itens, o desconto, o CPF opcional e as formas de pagamento, com o total, o
//! troco e o pedido JSON que a aplicacao completa, numera, confere e envia. A venda e' sempre presencial, para consumidor
//! final e dentro do estado: o que a NF-e (modelo 55) pergunta em cinco passos aqui ja vem decidido.
//!
//! Fontes das regras que a tela decide (a conferencia final e' da aplicacao):
//!
//! * idDest 1, finNFe 1, indFinal 1 e indPres 1 (presencial): rejeicoes 707, 715, 716 e 717 da NT 2026.002 v1.11 para a
//!   NFC-e (`crates/rustnf/src/nfe/montagem/nfce.rs`, `fn ide`). CPF so' com `indIEDest` 9 (789, mesmo arquivo);
//! * pagamentos e troco: a soma dos pagamentos menos o troco tem de dar o `vNF` (rejeicoes 866 e 869,
//!   `crates/rustnf/src/nfe/montagem/regras.rs`). O troco so' sai do dinheiro: cartao e Pix vao pelo valor exato (decisao do
//!   projeto, nao do MOC: devolver troco de cartao nao existe no balcao);
//! * CFOP da NFC-e: so' 5.xxx (lista do `CFOP_NOTA`, `nfce.rs`); um CFOP de outro estado fixado no cadastro do produto (de
//!   uma NF-e) nao vale no caixa e a tabela fiscal escolhe o da venda interna (5.102, 5.101 ou 5.405).
//!
//! Sem janela.

use rustnf::chave_dfe;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use super::mascara::{centavos_para_dinheiro, dinheiro_para_centavos, so_digitos};
use super::nfe_fiscal::{Destino, Icms, Pagamento};
use super::nfe_nova::{
    centesimos_de_percentual, decimal, decimal_de_milesimos, milesimos, ContextoNfe, DadosNfe,
    ItemNfe, NATUREZA,
};
use super::nfe_pedido::item_json;
use super::nfe_produtos::Produto;

/// As formas de pagamento do caixa, na ordem da tela.
pub const FORMAS: [Pagamento; 4] = [
    Pagamento::Dinheiro,
    Pagamento::Pix,
    Pagamento::CartaoCredito,
    Pagamento::CartaoDebito,
];

/// Quantas unidades de uma linha o caixa admite (3 casas): 99.999,999.
const QUANTIDADE_MAXIMA: i64 = 99_999_999;

/// Um pagamento do cupom.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PagamentoCupom {
    /// A forma.
    pub forma: Pagamento,
    /// Quanto o cliente entregou nesta forma, em centavos.
    pub centavos: i64,
}

/// O que falta ou esta errado, em portugues comum, com o que fazer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendenciaCaixa {
    /// O que esta errado.
    pub mensagem: String,
    /// O que fazer.
    pub como_resolver: String,
}

impl PendenciaCaixa {
    fn nova(m: impl Into<String>, c: impl Into<String>) -> Self {
        Self {
            mensagem: m.into(),
            como_resolver: c.into(),
        }
    }
}

/// Os totais do cupom, em centavos.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TotaisCupom {
    /// A soma dos produtos.
    pub produtos: i64,
    /// O desconto do cupom (nunca mais que os produtos).
    pub desconto: i64,
    /// O que o cliente paga.
    pub total: i64,
}

/// O cupom em andamento.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Cupom {
    /// Os itens (o mesmo tipo da NF-e: o caixa reusa a tabela fiscal dela).
    pub itens: Vec<ItemNfe>,
    /// O desconto do cupom em dinheiro, como digitado.
    pub desconto: String,
    /// O CPF na nota, so' digitos (vazio = sem CPF).
    pub cpf: String,
    /// Os pagamentos ja lancados.
    pub pagamentos: Vec<PagamentoCupom>,
}

impl Cupom {
    /// Nada foi lancado (o rascunho nao precisa existir).
    #[must_use]
    pub fn vazio(&self) -> bool {
        *self == Self::default()
    }

    /// Poe um produto do cadastro (ou um item pronto) no cupom, na quantidade dada em milesimos. O mesmo codigo, com o mesmo
    /// valor, soma na linha que ja existe.
    pub fn adicionar(&mut self, mut item: ItemNfe, quantidade_milesimos: i64) {
        let q = quantidade_milesimos.clamp(1, QUANTIDADE_MAXIMA);
        if !item.codigo.trim().is_empty() {
            if let Some(i) = self
                .itens
                .iter_mut()
                .find(|i| i.codigo == item.codigo && i.valor == item.valor)
            {
                let nova = i.quantidade_milesimos().unwrap_or(0) + q;
                i.quantidade = decimal_de_milesimos(nova.min(QUANTIDADE_MAXIMA));
                return;
            }
        }
        item.quantidade = decimal_de_milesimos(q);
        item.guardar = false;
        self.itens.push(item);
    }

    /// Soma `delta` milesimos a quantidade da linha `i`; nunca deixa a linha em zero (para tirar, use [`Cupom::tirar`]).
    pub fn mudar_quantidade(&mut self, i: usize, delta: i64) {
        if let Some(l) = self.itens.get_mut(i) {
            let nova = l.quantidade_milesimos().unwrap_or(1000) + delta;
            if nova >= 1 {
                l.quantidade = decimal_de_milesimos(nova.min(QUANTIDADE_MAXIMA));
            }
        }
    }

    /// Tira a linha `i`.
    pub fn tirar(&mut self, i: usize) {
        if i < self.itens.len() {
            self.itens.remove(i);
        }
    }

    /// O desconto do cupom em centavos (vazio ou ilegivel = 0).
    #[must_use]
    pub fn desconto_centavos(&self) -> i64 {
        dinheiro_para_centavos(&self.desconto).unwrap_or(0).max(0)
    }

    /// Os totais.
    #[must_use]
    pub fn totais(&self) -> TotaisCupom {
        let produtos: i64 = self
            .itens
            .iter()
            .filter_map(ItemNfe::produto_centavos)
            .sum();
        let desconto = self.desconto_centavos().min(produtos);
        TotaisCupom {
            produtos,
            desconto,
            total: produtos - desconto,
        }
    }

    /// Quanto o cliente ja entregou, somando todas as formas.
    #[must_use]
    pub fn pago(&self) -> i64 {
        self.pagamentos.iter().map(|p| p.centavos).sum()
    }

    fn em_dinheiro(&self) -> i64 {
        self.pagamentos
            .iter()
            .filter(|p| p.forma == Pagamento::Dinheiro)
            .map(|p| p.centavos)
            .sum()
    }

    /// Quanto ainda falta receber (nunca negativo).
    #[must_use]
    pub fn falta(&self) -> i64 {
        (self.totais().total - self.pago()).max(0)
    }

    /// O troco: o que passou do total, e so' o dinheiro devolve troco. Se o excesso e' maior que o dinheiro recebido (cartao ou
    /// Pix acima do total), nao ha troco possivel: a conferencia pede para acertar o valor.
    #[must_use]
    pub fn troco(&self) -> i64 {
        let excesso = self.pago() - self.totais().total;
        if excesso > 0 && excesso <= self.em_dinheiro() {
            excesso
        } else {
            0
        }
    }

    /// Lanca um pagamento. Valor zero lanca o que falta (a pessoa so' escolhe a forma). Devolve `false` se nao ha o que lancar.
    pub fn pagar(&mut self, forma: Pagamento, centavos: i64) -> bool {
        if self.totais().total <= 0 {
            return false;
        }
        let valor = if centavos > 0 { centavos } else { self.falta() };
        if valor <= 0 {
            return false;
        }
        self.pagamentos.push(PagamentoCupom {
            forma,
            centavos: valor,
        });
        true
    }

    /// Tira o pagamento `i`.
    pub fn tirar_pagamento(&mut self, i: usize) {
        if i < self.pagamentos.len() {
            self.pagamentos.remove(i);
        }
    }

    /// O CPF na nota confere (vazio tambem: o CPF e' opcional).
    #[must_use]
    pub fn cpf_confere(&self) -> bool {
        self.cpf.is_empty() || (self.cpf.len() == 11 && chave_dfe::cpf_valido(&self.cpf))
    }

    /// O desconto do cupom rateado entre os itens, proporcional ao valor de cada um; o que sobra de centavos vai um a um para os
    /// primeiros itens. O desconto do cupom vira desconto de item porque a NFC-e o declara por item (`vDesc`).
    #[must_use]
    pub fn itens_com_rateio(&self) -> Vec<ItemNfe> {
        let t = self.totais();
        let mut itens: Vec<ItemNfe> = self.itens.iter().map(item_do_caixa).collect();
        if t.desconto == 0 || t.produtos == 0 {
            return itens;
        }
        let produtos: Vec<i64> = itens
            .iter()
            .map(|i| i.produto_centavos().unwrap_or(0))
            .collect();
        let mut partes: Vec<i64> = produtos
            .iter()
            .map(|p| t.desconto * p / t.produtos)
            .collect();
        let mut resto = t.desconto - partes.iter().sum::<i64>();
        for (parte, produto) in partes.iter_mut().zip(&produtos) {
            if resto == 0 {
                break;
            }
            if *parte < *produto {
                *parte += 1;
                resto -= 1;
            }
        }
        for (i, parte) in itens.iter_mut().zip(partes) {
            i.desconto = if parte > 0 {
                centavos_para_dinheiro(parte)
            } else {
                String::new()
            };
        }
        itens
    }

    /// O que falta ou esta errado para emitir, em portugues comum.
    #[must_use]
    pub fn pendencias(&self, c: &ContextoNfe) -> Vec<PendenciaCaixa> {
        let mut p = Vec::new();
        if self.itens.is_empty() {
            p.push(PendenciaCaixa::nova(
                "O cupom está vazio.",
                "Procure o produto pelo código ou pelo nome e aperte Enter.",
            ));
            return p;
        }
        for (k, i) in self.itens.iter().enumerate() {
            pendencias_do_item(k + 1, i, c, &mut p);
        }
        if !self.cpf_confere() {
            p.push(PendenciaCaixa::nova(
                "O CPF não confere.",
                "Confira os números (o dígito final não bate) ou apague o CPF: ele é opcional.",
            ));
        }
        let t = self.totais();
        if t.total <= 0 {
            p.push(PendenciaCaixa::nova(
                "O total do cupom é zero.",
                "O desconto não pode ser igual ou maior que a soma dos produtos.",
            ));
        } else if self.pagamentos.is_empty() {
            p.push(PendenciaCaixa::nova(
                "Falta lançar o pagamento.",
                "Escolha a forma de pagamento: sem digitar valor, ela recebe o total.",
            ));
        } else if self.falta() > 0 {
            p.push(PendenciaCaixa::nova(
                format!(
                    "Faltam R$ {} para fechar o cupom.",
                    centavos_para_dinheiro(self.falta())
                ),
                "Lance mais um pagamento com o que falta.",
            ));
        } else if self.pago() > t.total && self.troco() == 0 {
            p.push(PendenciaCaixa::nova(
                "O Pix ou o cartão passa do total e não há dinheiro para dar troco.",
                "Tire o pagamento e lance o valor exato: só o dinheiro devolve troco.",
            ));
        }
        p
    }

    /// O pedido JSON da NFC-e (o que `rustnf_app::nfce` completa com a configuracao e confere). Com pendencia, devolve a lista.
    ///
    /// # Errors
    ///
    /// As pendencias, quando algo falta para montar o pedido.
    pub fn pedido(&self, c: &ContextoNfe) -> Result<Value, Vec<PendenciaCaixa>> {
        let pend = self.pendencias(c);
        if !pend.is_empty() {
            return Err(pend);
        }
        let d = DadosNfe::default();
        let regime = c.regime_pis_efetivo(&d);
        let itens: Vec<Value> = self
            .itens_com_rateio()
            .iter()
            .map(|i| item_json(&d, i, Destino::DentroDoEstado, c, regime))
            .collect();
        let mut m = Map::new();
        m.insert("nat_op".into(), json!(NATUREZA));
        m.insert("id_dest".into(), json!(1));
        m.insert("ind_final".into(), json!(1));
        m.insert("ind_pres".into(), json!(1));
        m.insert("mod_frete".into(), json!(9));
        m.insert("itens".into(), Value::Array(itens));
        let pagamentos: Vec<Value> = self
            .pagamentos
            .iter()
            .map(|p| {
                let mut x = Map::new();
                x.insert("t_pag".into(), json!(p.forma.t_pag()));
                x.insert("valor".into(), json!(decimal(p.centavos)));
                x.insert("ind_pag".into(), json!(p.forma.ind_pag()));
                if p.forma.leva_card() {
                    x.insert("cartao".into(), json!({"tp_integra": 2}));
                }
                Value::Object(x)
            })
            .collect();
        m.insert("pagamentos".into(), Value::Array(pagamentos));
        if self.troco() > 0 {
            m.insert("troco".into(), json!(decimal(self.troco())));
        }
        if !self.cpf.is_empty() {
            m.insert(
                "dest".into(),
                json!({"documento": self.cpf, "ind_ie_dest": 9}),
            );
        }
        Ok(Value::Object(m))
    }
}

/// O item como o caixa o emite: o CFOP fixado no cadastro so' vale se for da venda interna (5.xxx).
fn item_do_caixa(i: &ItemNfe) -> ItemNfe {
    let mut x = i.clone();
    if x.cfop
        .as_deref()
        .is_some_and(|c| !c.trim().starts_with('5'))
    {
        x.cfop = None;
    }
    x
}

fn pendencias_do_item(n: usize, i: &ItemNfe, c: &ContextoNfe, p: &mut Vec<PendenciaCaixa>) {
    let nome = if i.descricao.trim().is_empty() {
        format!("O item {n}")
    } else {
        format!("\"{}\"", i.descricao.trim())
    };
    let como_cadastro = "Corrija o produto em Produtos e coloque-o de novo no cupom.";
    if i.descricao.trim().is_empty() {
        p.push(PendenciaCaixa::nova(
            format!("{nome} está sem descrição."),
            como_cadastro,
        ));
    }
    if so_digitos(&i.ncm, 8).len() != 8 {
        p.push(PendenciaCaixa::nova(
            format!("{nome} está sem o NCM de 8 dígitos."),
            "O NCM é obrigatório na nota. Confira na tabela da Receita e corrija o produto em Produtos.",
        ));
    }
    if i.valor_centavos().is_none() || i.quantidade_milesimos().is_none() {
        p.push(PendenciaCaixa::nova(
            format!("{nome} está sem preço ou sem quantidade."),
            como_cadastro,
        ));
    }
    if i.icms_efetivo(c.crt).is_none() {
        p.push(PendenciaCaixa::nova(
            format!("{nome}: o regime da empresa não está nas Configurações."),
            "Escolha o regime em Configurações, seção NF-e.",
        ));
    }
    let tributado = i.icms_efetivo(c.crt) == Some(Icms::Tributado)
        && !i.icms_codigo.as_ref().is_some_and(|c| !c.trim().is_empty());
    if tributado && centesimos_de_percentual(&i.aliquota_icms).is_none() {
        p.push(PendenciaCaixa::nova(
            format!("{nome} paga ICMS e está sem a alíquota."),
            "Informe a alíquota do ICMS nos detalhes fiscais do produto, em Produtos.",
        ));
    }
    let x = item_do_caixa(i);
    if x.cfop_efetivo(Destino::DentroDoEstado, c.crt, None)
        .is_none()
        && x.icms_efetivo(c.crt).is_some()
    {
        p.push(PendenciaCaixa::nova(
            format!("{nome}: não achei o CFOP da venda dentro do estado."),
            "Confira a origem e o ICMS do produto em Produtos.",
        ));
    }
}

/// O produto do cadastro como item do cupom (quantidade 1; a tela a muda).
#[must_use]
pub fn item_do_produto(p: &Produto) -> ItemNfe {
    p.para_item()
}

/// O que a pessoa digitou como quantidade, em milesimos; vazio = 1.
#[must_use]
pub fn quantidade_digitada(texto: &str) -> Option<i64> {
    if texto.trim().is_empty() {
        return Some(1000);
    }
    milesimos(texto).filter(|q| *q > 0 && *q <= QUANTIDADE_MAXIMA)
}

/// O texto de uma quantidade como a tela mostra (`1`, `0,5`).
#[must_use]
pub fn quantidade_em_texto(item: &ItemNfe) -> String {
    item.quantidade.replace('.', ",")
}

#[cfg(test)]
mod testes;
