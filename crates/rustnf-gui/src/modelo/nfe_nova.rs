// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Os dados da Nova NF-e (o assistente de quem vende) e o pedido JSON que eles viram. O assistente pergunta em
//! linguagem de balcao (para quem, o que vendeu, como recebeu, como entrega); o termo fiscal sai da tabela de
//! `nfe_fiscal`, e o que a pessoa editar a mao em "Detalhes fiscais" nunca e' sobrescrito. Este modulo so' junta as
//! respostas num pedido: completar com a configuracao, numerar, conferir as regras do leiaute e enviar e' da camada de
//! aplicacao (`rustnf_app::nfe`). As pendencias de ca' sao so' o que falta para MONTAR o pedido.

use chrono::{Months, NaiveDate};
use rustnf::danfse::municipio;
use serde::{Deserialize, Serialize};

use super::mascara::{data_br_para_iso, dinheiro_para_centavos, so_digitos};
use super::nfe_fiscal::{
    cfops_da_venda, opcoes_de_icms, Contribuinte, Destino, Entrega, Icms, Origem, Pagamento,
    Presenca, RegimePisCofins,
};

/// A natureza da operacao de toda venda feita por este assistente.
pub const NATUREZA: &str = "VENDA DE MERCADORIA";

/// Quantas parcelas o assistente admite (a biblioteca admite 120: aqui e' uma venda de balcao).
pub const MAX_PARCELAS: u32 = 24;

/// Os cinco passos do assistente.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum PassoNfe {
    /// Para quem se vende.
    ParaQuem,
    /// O que se vendeu.
    OQueVendeu,
    /// Como o cliente pagou.
    ComoRecebeu,
    /// Como a mercadoria chega ao cliente.
    ComoEntrega,
    /// Conferir e enviar.
    Conferir,
}

impl PassoNfe {
    /// Os passos, na ordem.
    pub const TODOS: [PassoNfe; 5] = [
        Self::ParaQuem,
        Self::OQueVendeu,
        Self::ComoRecebeu,
        Self::ComoEntrega,
        Self::Conferir,
    ];

    /// A posicao, a partir de 0.
    #[must_use]
    pub fn indice(self) -> usize {
        Self::TODOS.iter().position(|p| *p == self).unwrap_or(0)
    }

    /// O titulo curto.
    #[must_use]
    pub fn titulo(self) -> &'static str {
        match self {
            Self::ParaQuem => "Para quem",
            Self::OQueVendeu => "O que vendeu",
            Self::ComoRecebeu => "Como recebeu",
            Self::ComoEntrega => "Como entrega",
            Self::Conferir => "Conferir e enviar",
        }
    }

    /// O seguinte, se houver.
    #[must_use]
    pub fn proximo(self) -> Option<Self> {
        Self::TODOS.get(self.indice() + 1).copied()
    }

    /// O anterior, se houver.
    #[must_use]
    pub fn anterior(self) -> Option<Self> {
        self.indice()
            .checked_sub(1)
            .and_then(|i| Self::TODOS.get(i).copied())
    }

    /// `Passo 2 de 5`.
    #[must_use]
    pub fn indicador(self) -> String {
        format!("Passo {} de {}", self.indice() + 1, Self::TODOS.len())
    }
}

/// O cliente compra para o uso dele ou para revender: `indFinal` (B25).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Consumo {
    /// Consumidor final (1).
    Final,
    /// Revenda ou industrializacao (0).
    Revenda,
}

impl Consumo {
    /// As duas, na ordem da tela.
    pub const TODOS: [Consumo; 2] = [Self::Final, Self::Revenda];

    /// `indFinal`.
    #[must_use]
    pub fn ind_final(self) -> u8 {
        u8::from(self == Self::Final)
    }

    /// O texto da escolha.
    #[must_use]
    pub fn rotulo(self) -> &'static str {
        match self {
            Self::Final => "Para uso dele (consumidor final)",
            Self::Revenda => "Para revender ou usar na produção",
        }
    }
}

/// Um produto da nota, com o que a pessoa disse e o que ela fixou.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ItemNfe {
    /// O codigo do produto na loja.
    pub codigo: String,
    /// O que e'.
    pub descricao: String,
    /// A unidade (UN, KG, CX...).
    pub unidade: String,
    /// A quantidade, como digitada.
    pub quantidade: String,
    /// O valor de uma unidade, como digitado.
    pub valor: String,
    /// O desconto no item, em dinheiro.
    pub desconto: String,
    /// O NCM (8 digitos).
    pub ncm: String,
    /// O CEST, quando a mercadoria tem.
    pub cest: String,
    /// A origem da mercadoria (0 nacional... 8), `orig` do ICMS.
    pub origem_mercadoria: u8,
    /// Fabricou ou revende.
    pub origem: Origem,
    /// Como o ICMS se comporta; `None` = a primeira opcao do regime (a tela a mostra marcada).
    pub icms: Option<Icms>,
    /// A aliquota do ICMS em percentual (so' quando paga ICMS na venda).
    pub aliquota_icms: String,
    /// ICMS ja cobrado antes: a base retida, em dinheiro.
    pub st_base: String,
    /// ICMS ja cobrado antes: a aliquota, em percentual.
    pub st_aliquota: String,
    /// ICMS ja cobrado antes: o valor retido, em dinheiro.
    pub st_valor: String,
    /// CFOP escolhido a mao (vale no lugar do da tabela).
    pub cfop: Option<String>,
    /// O CFOP que a pessoa escolheu entre as opcoes da tabela, quando ela mostra mais de uma (cliente isento, ST a nao
    /// contribuinte). So' vale enquanto continua entre as opcoes do cliente de agora.
    pub cfop_escolhido: Option<String>,
    /// DIFAL: a aliquota interna do ICMS da UF do cliente em percentual, como digitada (so' na venda interestadual a
    /// consumidor final nao contribuinte, quando a biblioteca exige o grupo `ICMSUFDest`).
    pub difal_aliquota: String,
    /// DIFAL: o adicional do Fundo de Combate a Pobreza da UF do cliente em percentual, como digitado (vazio = 0).
    pub difal_fcp: String,
    /// CST ou CSOSN do ICMS escolhido a mao.
    pub icms_codigo: Option<String>,
    /// CST do PIS e da COFINS escolhido a mao.
    pub pis_cst: Option<String>,
    /// O produto veio do cadastro.
    pub do_cadastro: bool,
    /// Guardar este produto no cadastro quando a nota for autorizada.
    pub guardar: bool,
}

impl Default for ItemNfe {
    fn default() -> Self {
        Self {
            codigo: String::new(),
            descricao: String::new(),
            unidade: "UN".into(),
            quantidade: "1".into(),
            valor: String::new(),
            desconto: String::new(),
            ncm: String::new(),
            cest: String::new(),
            origem_mercadoria: 0,
            origem: Origem::default(),
            icms: None,
            aliquota_icms: String::new(),
            st_base: String::new(),
            st_aliquota: String::new(),
            st_valor: String::new(),
            cfop: None,
            cfop_escolhido: None,
            difal_aliquota: String::new(),
            difal_fcp: String::new(),
            icms_codigo: None,
            pis_cst: None,
            do_cadastro: false,
            guardar: true,
        }
    }
}

impl ItemNfe {
    /// O ICMS que vale: o escolhido ou a primeira opcao do regime.
    #[must_use]
    pub fn icms_efetivo(&self, crt: u8) -> Option<Icms> {
        self.icms.or_else(|| opcoes_de_icms(crt).first().copied())
    }

    /// A quantidade em milesimos (3 casas), se for um numero maior que zero.
    #[must_use]
    pub fn quantidade_milesimos(&self) -> Option<i64> {
        milesimos(&self.quantidade).filter(|q| *q > 0)
    }

    /// O valor de uma unidade em centavos, se for maior que zero.
    #[must_use]
    pub fn valor_centavos(&self) -> Option<i64> {
        dinheiro_para_centavos(&self.valor).filter(|v| *v > 0)
    }

    /// O desconto em centavos (vazio = 0).
    #[must_use]
    pub fn desconto_centavos(&self) -> Option<i64> {
        if self.desconto.trim().is_empty() {
            Some(0)
        } else {
            dinheiro_para_centavos(&self.desconto)
        }
    }

    /// O valor do produto (quantidade x valor), em centavos, arredondado para cima a partir de meio centavo.
    #[must_use]
    pub fn produto_centavos(&self) -> Option<i64> {
        let q = self.quantidade_milesimos()?;
        let v = self.valor_centavos()?;
        q.checked_mul(v)?.checked_add(500).map(|n| n / 1000)
    }

    /// O que a nota cobra pelo item: o produto menos o desconto, em centavos.
    #[must_use]
    pub fn liquido_centavos(&self) -> Option<i64> {
        Some(self.produto_centavos()? - self.desconto_centavos()?)
    }

    /// As opcoes de CFOP da tabela para este item, neste destino e para este cliente.
    #[must_use]
    pub fn cfops_da_tabela(
        &self,
        destino: Destino,
        crt: u8,
        cliente: Option<Contribuinte>,
    ) -> &'static [&'static str] {
        self.icms_efetivo(crt).map_or(&[], |icms| {
            cfops_da_venda(destino, self.origem, icms, cliente)
        })
    }

    /// O CFOP que vale para este item neste destino: o digitado a mao, o da tabela (quando ela tem um so') ou o que a
    /// pessoa escolheu entre as opcoes. `None` = falta escolher (ou a combinacao nao existe).
    #[must_use]
    pub fn cfop_efetivo(
        &self,
        destino: Destino,
        crt: u8,
        cliente: Option<Contribuinte>,
    ) -> Option<String> {
        if let Some(c) = self.cfop.as_ref().filter(|c| !c.trim().is_empty()) {
            return Some(c.trim().to_string());
        }
        match self.cfops_da_tabela(destino, crt, cliente) {
            [unico] => Some((*unico).to_string()),
            varios => self
                .cfop_escolhido
                .as_ref()
                .filter(|e| varios.contains(&e.as_str()))
                .cloned(),
        }
    }

    /// O CST ou CSOSN que vale: o escolhido a mao ou o da tabela.
    #[must_use]
    pub fn icms_codigo_efetivo(&self, crt: u8) -> Option<String> {
        if let Some(c) = self.icms_codigo.as_ref().filter(|c| !c.trim().is_empty()) {
            return Some(c.trim().to_string());
        }
        self.icms_efetivo(crt)?.codigo(crt).map(str::to_string)
    }
}

/// Os dados da NF-e em andamento.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DadosNfe {
    /// CPF ou CNPJ de quem compra (so' digitos e letras).
    pub documento: String,
    /// Nome ou razao social.
    pub nome: String,
    /// Inscricao estadual.
    pub ie: String,
    /// E-mail (opcional).
    pub email: String,
    /// CEP.
    pub cep: String,
    /// Logradouro.
    pub logradouro: String,
    /// Numero.
    pub numero: String,
    /// Complemento.
    pub complemento: String,
    /// Bairro.
    pub bairro: String,
    /// Codigo IBGE do municipio (o nome e a UF saem da tabela).
    pub cod_municipio: String,
    /// O cliente e' do exterior.
    pub exterior: bool,
    /// Uso proprio ou revenda.
    pub consumo: Option<Consumo>,
    /// Contribuinte do ICMS ou nao.
    pub contribuinte: Option<Contribuinte>,
    /// Como o cliente comprou.
    pub presenca: Presenca,
    /// O que se vendeu.
    pub itens: Vec<ItemNfe>,
    /// O regime do PIS e da COFINS, quando a configuracao nao diz.
    pub regime_pis: Option<RegimePisCofins>,
    /// Como o cliente pagou.
    pub pagamento: Pagamento,
    /// Quantas parcelas (a prazo).
    pub parcelas: String,
    /// O vencimento da primeira parcela, `DD/MM/AAAA` (a prazo).
    pub primeiro_vencimento: String,
    /// Como a mercadoria chega ao cliente.
    pub entrega: Entrega,
    /// CPF ou CNPJ da transportadora.
    pub transp_documento: String,
    /// Nome da transportadora.
    pub transp_nome: String,
    /// Inscricao estadual da transportadora.
    pub transp_ie: String,
    /// Endereco da transportadora.
    pub transp_endereco: String,
    /// Municipio (nome) da transportadora.
    pub transp_municipio: String,
    /// UF da transportadora.
    pub transp_uf: String,
    /// Quantos volumes.
    pub volumes: String,
    /// A especie dos volumes (caixa, fardo...).
    pub especie: String,
    /// O peso bruto em kg.
    pub peso_bruto: String,
}

impl Default for DadosNfe {
    fn default() -> Self {
        Self {
            documento: String::new(),
            nome: String::new(),
            ie: String::new(),
            email: String::new(),
            cep: String::new(),
            logradouro: String::new(),
            numero: String::new(),
            complemento: String::new(),
            bairro: String::new(),
            cod_municipio: String::new(),
            exterior: false,
            consumo: None,
            contribuinte: None,
            presenca: Presenca::default(),
            itens: Vec::new(),
            regime_pis: None,
            pagamento: Pagamento::default(),
            parcelas: "2".into(),
            primeiro_vencimento: String::new(),
            entrega: Entrega::default(),
            transp_documento: String::new(),
            transp_nome: String::new(),
            transp_ie: String::new(),
            transp_endereco: String::new(),
            transp_municipio: String::new(),
            transp_uf: String::new(),
            volumes: String::new(),
            especie: String::new(),
            peso_bruto: String::new(),
        }
    }
}

/// O que o assistente precisa saber da configuracao e do relogio.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextoNfe {
    /// O `CRT` da `[nfe]`: 1 Simples, 2 Simples com excesso de sublimite, 3 regime normal, 4 MEI.
    pub crt: u8,
    /// A UF do emitente.
    pub uf_emitente: String,
    /// O `[nfe].regime_pis_cofins`, quando a configuracao diz.
    pub regime_pis: Option<RegimePisCofins>,
    /// Hoje.
    pub hoje: NaiveDate,
    /// O proximo numero da serie (so' para a fatura; quem numera de verdade e' a pasta de notas).
    pub numero_provisorio: u64,
}

impl ContextoNfe {
    /// O contexto da configuracao (`[nfe]`) para hoje e o numero dado.
    #[must_use]
    pub fn de_config(config: &rustnf_app::config::Config, hoje: NaiveDate, numero: u64) -> Self {
        Self {
            crt: config.nfe.crt,
            uf_emitente: config.nfe.uf.trim().to_uppercase(),
            regime_pis: RegimePisCofins::da_config(&config.nfe.regime_pis_cofins),
            hoje,
            numero_provisorio: numero,
        }
    }

    /// O regime do PIS e da COFINS que vale: o da configuracao ou o que a pessoa escolheu.
    #[must_use]
    pub fn regime_pis_efetivo(&self, d: &DadosNfe) -> Option<RegimePisCofins> {
        self.regime_pis.or(d.regime_pis)
    }

    /// Precisa perguntar o regime do PIS e da COFINS: regime normal sem a informacao na configuracao.
    #[must_use]
    pub fn pergunta_o_regime_pis(&self) -> bool {
        !matches!(self.crt, 1 | 4) && self.regime_pis.is_none()
    }
}

/// O que falta ou esta errado, em portugues comum, com o que fazer e o passo onde se resolve.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendenciaNfe {
    /// O passo onde se corrige.
    pub passo: PassoNfe,
    /// O produto (posicao na lista), quando o problema e' de um deles.
    pub item: Option<usize>,
    /// O que esta errado.
    pub mensagem: String,
    /// O que fazer.
    pub como_resolver: String,
}

impl PendenciaNfe {
    pub(super) fn nova(
        passo: PassoNfe,
        mensagem: impl Into<String>,
        como: impl Into<String>,
    ) -> Self {
        Self {
            passo,
            item: None,
            mensagem: mensagem.into(),
            como_resolver: como.into(),
        }
    }

    pub(super) fn do_item(mut self, i: usize) -> Self {
        self.item = Some(i);
        self
    }
}

/// Os totais que a tela mostra, em centavos.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Totais {
    /// A soma dos produtos.
    pub produtos: i64,
    /// A soma dos descontos.
    pub desconto: i64,
    /// O que o cliente paga.
    pub total: i64,
}

/// Milesimos de um numero digitado (`2`, `1,5`, `0.250`): ate 3 casas, um so' separador.
#[must_use]
pub fn milesimos(texto: &str) -> Option<i64> {
    let t = texto.trim();
    if t.is_empty() {
        return None;
    }
    let (inteira, fracao) = match t.split_once([',', '.']) {
        Some((i, f)) => (i, f),
        None => (t, ""),
    };
    if !inteira.bytes().all(|b| b.is_ascii_digit())
        || !fracao.bytes().all(|b| b.is_ascii_digit())
        || fracao.len() > 3
        || (inteira.is_empty() && fracao.is_empty())
    {
        return None;
    }
    let inteira: i64 = if inteira.is_empty() {
        0
    } else {
        inteira.parse().ok()?
    };
    let fracao: i64 = format!("{fracao:0<3}").parse().ok()?;
    inteira.checked_mul(1000)?.checked_add(fracao)
}

/// Centesimos de percentual (`18` -> 1800; `7,6` -> 760): um percentual de ate 2 casas.
#[must_use]
pub fn centesimos_de_percentual(texto: &str) -> Option<i64> {
    let t = texto.trim();
    let (inteira, fracao) = match t.split_once([',', '.']) {
        Some((i, f)) => (i, f),
        None => (t, ""),
    };
    if t.is_empty()
        || !inteira.bytes().all(|b| b.is_ascii_digit())
        || !fracao.bytes().all(|b| b.is_ascii_digit())
        || fracao.len() > 2
        || (inteira.is_empty() && fracao.is_empty())
    {
        return None;
    }
    let i: i64 = if inteira.is_empty() {
        0
    } else {
        inteira.parse().ok()?
    };
    let f: i64 = format!("{fracao:0<2}").parse().ok()?;
    i.checked_mul(100)?.checked_add(f)
}

/// `base x percentual`, em centavos, arredondado para cima a partir de meio centavo.
pub(super) fn imposto_centavos(base: i64, percentual_centesimos: i64) -> i64 {
    (base * percentual_centesimos + 5_000) / 10_000
}

/// Centavos como decimal com ponto, no formato do pedido (`1234.50`).
pub(super) fn decimal(centavos: i64) -> String {
    format!("{}.{:02}", centavos / 100, centavos.abs() % 100)
}

/// Milesimos como decimal com ponto, sem zeros a mais (`1.5`, `2`).
pub(super) fn decimal_de_milesimos(m: i64) -> String {
    let (i, f) = (m / 1000, m % 1000);
    if f == 0 {
        i.to_string()
    } else {
        format!("{i}.{f:03}").trim_end_matches('0').to_string()
    }
}

/// Percentual (centesimos) como decimal com ponto (`18`, `7.6`).
pub(super) fn decimal_de_percentual(c: i64) -> String {
    let (i, f) = (c / 100, c % 100);
    if f == 0 {
        i.to_string()
    } else {
        format!("{i}.{f:02}").trim_end_matches('0').to_string()
    }
}

/// O municipio pelo codigo IBGE: nome e UF, da tabela da biblioteca.
#[must_use]
pub fn municipio_do_codigo(ibge: &str) -> Option<(String, String)> {
    let d = so_digitos(ibge, 7);
    (d.len() == 7).then(|| municipio::buscar(&d)).flatten()
}

impl DadosNfe {
    /// Nada foi preenchido (o rascunho nao precisa existir).
    #[must_use]
    pub fn vazio(&self) -> bool {
        *self == Self::default()
    }

    /// O documento e' CPF (11 digitos) ou CNPJ (14).
    #[must_use]
    pub fn documento_limpo(&self) -> String {
        self.documento
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .map(|c| c.to_ascii_uppercase())
            .collect()
    }

    /// O destino da venda pelas UFs; `None` = falta o municipio do cliente.
    #[must_use]
    pub fn destino(&self, c: &ContextoNfe) -> Option<Destino> {
        let uf = municipio_do_codigo(&self.cod_municipio)
            .map(|(_, uf)| uf)
            .unwrap_or_default();
        Destino::pelas_ufs(&c.uf_emitente, &uf, self.exterior)
    }

    /// Os totais da nota (itens que ainda nao estao completos nao entram).
    #[must_use]
    pub fn totais(&self) -> Totais {
        let mut t = Totais::default();
        for i in &self.itens {
            if let Some(p) = i.produto_centavos() {
                t.produtos += p;
                t.desconto += i.desconto_centavos().unwrap_or(0).clamp(0, p);
            }
        }
        t.total = t.produtos - t.desconto;
        t
    }

    /// As parcelas: valor em centavos e vencimento. O resto da divisao vai na ultima parcela.
    #[must_use]
    pub fn parcelas_da_nota(&self) -> Vec<(i64, NaiveDate)> {
        let Some(primeira) = data_br_para_iso(&self.primeiro_vencimento)
            .and_then(|iso| NaiveDate::parse_from_str(&iso, "%Y-%m-%d").ok())
        else {
            return Vec::new();
        };
        let n = self.parcelas.trim().parse::<u32>().unwrap_or(0);
        if n == 0 || n > MAX_PARCELAS {
            return Vec::new();
        }
        let total = self.totais().total;
        let base = total / i64::from(n);
        let resto = total - base * i64::from(n);
        (0..n)
            .map(|k| {
                let valor = if k + 1 == n { base + resto } else { base };
                let venc = primeira
                    .checked_add_months(Months::new(k))
                    .unwrap_or(primeira);
                (valor, venc)
            })
            .collect()
    }
}

pub use super::nfe_pedido::pedido;
pub use super::nfe_pendencias::pendencias;

#[cfg(test)]
pub(crate) mod testes;

#[cfg(test)]
mod testes_difal;
