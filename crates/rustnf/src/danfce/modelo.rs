// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O conteudo do DANFC-e, por divisao do Manual (I a IX), ja formatado e sem I/O. E' o que o PDF e o ESC/POS
//! imprimem; a serializacao em JSON e' estavel (mesma nota, mesmo JSON).

use serde::Serialize;

/// Titulo do documento (Manual, 3.1.1, divisao I).
pub const TITULO: &str = "Documento Auxiliar da Nota Fiscal de Consumidor Eletrônica";
/// Chamada da consulta pela chave de acesso (Manual, 3.1.4, divisao IV).
pub const CHAMADA_CONSULTA: &str = "Consulte pela Chave de Acesso em";
/// Rotulo do protocolo (Manual, 3.1.7, divisao VII).
pub const ROTULO_PROTOCOLO: &str = "Protocolo de autorização:";
/// Rotulo da data da autorizacao (Manual, 3.1.7, figuras 4 e 5).
pub const ROTULO_AUTORIZACAO: &str = "Data de autorização";
/// Texto da nota emitida em homologacao (Manual, 3.1.8).
pub const HOMOLOGACAO: &str = "EMITIDA EM AMBIENTE DE HOMOLOGAÇÃO – SEM VALOR FISCAL";
/// Primeira linha do aviso da contingencia off-line (Manual, 3.1.8).
pub const CONTINGENCIA: &str = "EMITIDA EM CONTINGÊNCIA";
/// Segunda linha do aviso da contingencia off-line (Manual, 3.1.8).
pub const PENDENTE: &str = "Pendente de autorização";
/// Marca da nota cancelada (o Manual 6.0 nao traz o leiaute da cancelada).
pub const CANCELADA: &str = "NFC-e CANCELADA";
/// Via do consumidor, na contingencia (Manual, 3.1.8, figura 6).
pub const VIA_CONSUMIDOR: &str = "Via consumidor";
/// Via do estabelecimento, na contingencia (Manual, 3.1.8).
pub const VIA_ESTABELECIMENTO: &str = "Via do Estabelecimento";
/// Frase da Lei 12.741/2012 (Manual, 3.1.9.1).
pub const TRIBUTOS: &str = "Tributos Totais Incidentes (Lei Federal 12.741/2012):";

/// Rotulos da divisao II (Manual, 3.1.2, figura 2).
pub const COLUNAS_ITENS: [&str; 6] = ["Código", "Descrição", "Qtde", "UN", "Vl Unit", "Vl Total"];
/// Rotulos da divisao III (Manual, 3.1.3, figura C).
pub const ROTULO_QTDE_ITENS: &str = "Qtde. total de itens";
/// Idem.
pub const ROTULO_VALOR_TOTAL: &str = "Valor total R$";
/// Idem.
pub const ROTULO_ACRESCIMOS: &str = "Acréscimos (frete, seguro e outras despesas) R$";
/// Idem.
pub const ROTULO_DESCONTO: &str = "Desconto R$";
/// Idem.
pub const ROTULO_VALOR_A_PAGAR: &str = "Valor a Pagar R$";
/// Idem.
pub const ROTULO_FORMA_PAGAMENTO: &str = "FORMA PAGAMENTO";
/// Idem.
pub const ROTULO_VALOR_PAGO: &str = "VALOR PAGO R$";
/// Idem.
pub const ROTULO_TROCO: &str = "Troco R$";

/// Em que situacao o cupom sai.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SituacaoCupom {
    /// Nota autorizada (protocolo 100 ou 150).
    Autorizada,
    /// Nota cancelada.
    Cancelada,
    /// Emitida em contingencia off-line e ainda pendente de autorizacao.
    ContingenciaPendente,
}

/// Divisao I: cabecalho (emitente).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Cabecalho {
    /// `CNPJ` ou `CPF`.
    pub rotulo_documento: String,
    /// Documento do emitente com a mascara.
    pub documento: String,
    /// Razao social ou nome.
    pub nome: String,
    /// Endereco completo, sem o pais.
    pub endereco: String,
    /// "Documento Auxiliar da Nota Fiscal de Consumidor Eletronica".
    pub titulo: String,
}

/// Destaque em duas linhas (contingencia) ou uma (cancelada), abaixo do cabecalho e abaixo da identificacao.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Destaque {
    /// Linha em destaque.
    pub linha1: String,
    /// Segunda linha, se houver.
    pub linha2: Option<String>,
}

/// Divisao II: uma linha de item.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct LinhaItem {
    /// `cProd`.
    pub codigo: String,
    /// `xProd`.
    pub descricao: String,
    /// `qCom`.
    pub quantidade: String,
    /// `uCom`.
    pub unidade: String,
    /// `vUnCom`.
    pub valor_unitario: String,
    /// `vProd`.
    pub valor_total: String,
}

/// Divisao III: totais.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Totais {
    /// Quantidade de itens distintos.
    pub quantidade_itens: String,
    /// Soma dos valores dos itens (`vProd`).
    pub valor_total: String,
    /// Frete, seguro e outras despesas (so' se houver).
    pub acrescimos: Option<String>,
    /// Desconto (so' se houver), sem sinal.
    pub desconto: Option<String>,
    /// `vNF` (so' com acrescimo ou desconto).
    pub valor_a_pagar: Option<String>,
}

/// Divisao III: uma forma de pagamento.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Pagamento {
    /// Nome da forma (`tPag`).
    pub forma: String,
    /// `vPag`.
    pub valor: String,
}

/// Divisao IV: consulta pela chave de acesso.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Consulta {
    /// `urlChave` do `infNFeSupl`.
    pub url: String,
    /// A chave em 11 blocos de 4 algarismos.
    pub chave: String,
}

/// Divisao VI: o consumidor.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Consumidor {
    /// `CONSUMIDOR CPF:`, `CONSUMIDOR CNPJ:`, `CONSUMIDOR Id. Estrangeiro:`, `CONSUMIDOR` ou
    /// `CONSUMIDOR NÃO IDENTIFICADO`.
    pub rotulo: String,
    /// Documento, quando o XML o traz.
    pub documento: Option<String>,
    /// Nome, quando impresso.
    pub nome: Option<String>,
    /// Endereco, quando impresso.
    pub endereco: Option<String>,
}

/// Divisao VII: identificacao da NFC-e e protocolo.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Identificacao {
    /// `nNF`, com 9 algarismos.
    pub numero: String,
    /// `serie`, com 3 algarismos.
    pub serie: String,
    /// `dhEmi` no horario da UF do emitente.
    pub emissao: String,
    /// "Via consumidor" ou "Via do Estabelecimento" (contingencia).
    pub via: Option<String>,
    /// `nProt` (suprimido na contingencia).
    pub protocolo: Option<String>,
    /// `dhRecbto` no horario da UF do emitente.
    pub autorizacao: Option<String>,
}

/// O DANFC-e, por divisao do Manual.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Danfce {
    /// Chave de acesso (44 algarismos).
    pub chave: String,
    /// `tpAmb` 2.
    pub homologacao: bool,
    /// Situacao do cupom.
    pub situacao: SituacaoCupom,
    /// Divisao I.
    pub cabecalho: Cabecalho,
    /// Aviso de contingencia ou de nota cancelada.
    pub destaque: Option<Destaque>,
    /// Divisao II (`None` no cupom resumido).
    pub itens: Option<Vec<LinhaItem>>,
    /// Divisao III.
    pub totais: Totais,
    /// Divisao III: formas de pagamento.
    pub pagamentos: Vec<Pagamento>,
    /// Divisao III: troco.
    pub troco: Option<String>,
    /// Divisao IV.
    pub consulta: Consulta,
    /// Divisao V: o `qrCode` do XML.
    pub qr_code: String,
    /// Divisao VI.
    pub consumidor: Consumidor,
    /// Divisao VII.
    pub identificacao: Identificacao,
    /// Divisao VIII: `infAdFisco`.
    pub mensagem_fiscal: Option<String>,
    /// Divisao VIII: texto da homologacao.
    pub homologacao_texto: Option<String>,
    /// Divisao IX: `infCpl`.
    pub interesse_do_contribuinte: Option<String>,
    /// Divisao IX: tributos da Lei 12.741/2012 (`vTotTrib`).
    pub tributos: Option<String>,
    /// Mensagem que o contribuinte imprime depois da divisao IX (nao consta do XML).
    pub mensagem_institucional: Option<String>,
}
