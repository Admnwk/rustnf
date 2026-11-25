// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O que a leitura devolve: a NF-e (modelo 55) em structs, com o nome da tag do leiaute em cada campo.
//!
//! Campo de texto vazio = a tag nao veio. Dinheiro e quantidade sao [`Decimal`], data com hora e' [`DataHora`] (com
//! fuso, como o leiaute exige). Os grupos que variam muito (impostos, totais) vem em [`Grupo`], por nome de tag.

use std::collections::BTreeMap;

use chrono::NaiveDate;
use serde::Serialize;

use super::decimal::{DataHora, Decimal};

/// Folhas de um grupo, por caminho de tag (`a/b`); repeticao ganha `[2]`, `[3]`... no nome.
pub type Folhas = BTreeMap<String, String>;

/// Grupo de imposto ou total lido por inteiro: as folhas numericas em `valores`, as de codigo em `textos`.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct Grupo {
    /// Nome da tag do grupo (`ICMS00`, `PISAliq`, `ICMSTot`...).
    pub nome: String,
    /// Folhas que sao codigo ou texto (`orig`, `CST`, `CSOSN`, `modBC`...), mesmo quando parecem numero.
    pub textos: BTreeMap<String, String>,
    /// Folhas que sao decimal (`vBC`, `pICMS`, `vICMS`...).
    pub valores: BTreeMap<String, Decimal>,
}

impl Grupo {
    /// Decimal da folha `tag`; ausente = zero.
    #[must_use]
    pub fn valor(&self, tag: &str) -> Decimal {
        self.valores.get(tag).copied().unwrap_or(Decimal::ZERO)
    }

    /// Texto da folha `tag`; ausente = vazio.
    #[must_use]
    pub fn texto(&self, tag: &str) -> &str {
        self.textos.get(tag).map_or("", String::as_str)
    }
}

/// Endereco (`enderEmit`, `enderDest`, `retirada`, `entrega`).
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct Endereco {
    /// `xLgr`.
    pub logradouro: String,
    /// `nro`.
    pub numero: String,
    /// `xCpl`.
    pub complemento: String,
    /// `xBairro`.
    pub bairro: String,
    /// `cMun` (codigo IBGE).
    pub cmun: String,
    /// `xMun`.
    pub municipio: String,
    /// `UF`.
    pub uf: String,
    /// `CEP`.
    pub cep: String,
    /// `cPais`.
    pub cpais: String,
    /// `xPais`.
    pub pais: String,
    /// `fone`.
    pub fone: String,
}

/// Emitente, destinatario, local de retirada ou de entrega.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct Participante {
    /// CNPJ, CPF ou `idEstrangeiro` (CNPJ e CPF sem pontuacao, em maiusculas).
    pub documento: String,
    /// `cnpj`, `cpf`, `estrangeiro` ou vazio.
    pub tipo_documento: String,
    /// `xNome`.
    pub nome: String,
    /// `xFant`.
    pub fantasia: String,
    /// `IE`.
    pub ie: String,
    /// `IEST`.
    pub iest: String,
    /// `IM`.
    pub im: String,
    /// `CNAE`.
    pub cnae: String,
    /// `CRT` (regime tributario do emitente).
    pub crt: String,
    /// `indIEDest`.
    pub ind_ie_dest: String,
    /// `ISUF` (inscricao na SUFRAMA).
    pub isuf: String,
    /// `email`.
    pub email: String,
    /// O endereco do grupo, se veio.
    pub endereco: Option<Endereco>,
}

/// Identificacao da nota (`ide`).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Ide {
    /// `cUF`.
    pub cuf: String,
    /// `cNF`.
    pub cnf: String,
    /// `natOp`.
    pub natureza_operacao: String,
    /// `mod`.
    pub modelo: String,
    /// `serie`.
    pub serie: u64,
    /// `nNF`.
    pub numero: u64,
    /// `dhEmi`.
    pub emissao: Option<DataHora>,
    /// `dhSaiEnt`.
    pub saida_entrada: Option<DataHora>,
    /// `tpNF` (0 entrada, 1 saida).
    pub tipo_nf: String,
    /// `idDest`.
    pub destino: String,
    /// `cMunFG`.
    pub cmun_fg: String,
    /// `cMunFGIBS`.
    pub cmun_fg_ibs: String,
    /// `tpImp`.
    pub tipo_impressao: String,
    /// `tpEmis`.
    pub tipo_emissao: String,
    /// `cDV`.
    pub cdv: String,
    /// `tpAmb` (1 producao, 2 homologacao).
    pub ambiente: String,
    /// `finNFe`.
    pub finalidade: String,
    /// `tpNFDebito`.
    pub tp_nf_debito: String,
    /// `tpNFCredito`.
    pub tp_nf_credito: String,
    /// `indFinal`.
    pub consumidor_final: String,
    /// `indPres`.
    pub presenca: String,
    /// `indIntermed`.
    pub intermediador: String,
    /// `procEmi`.
    pub processo_emissao: String,
    /// `verProc`.
    pub versao_processo: String,
    /// `dhCont`.
    pub contingencia_em: Option<DataHora>,
    /// `xJust` da contingencia.
    pub contingencia_justificativa: String,
    /// `dPrevEntrega`.
    pub previsao_entrega: Option<NaiveDate>,
    /// Cada `NFref`, com as folhas por caminho.
    pub referencias: Vec<Folhas>,
    /// `gCompraGov`.
    pub compra_governamental: Folhas,
    /// Cada `refNFe` de `gPagAntecipado`.
    pub pagamento_antecipado: Vec<String>,
}

/// Rastreabilidade do item (`rastro`).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Rastro {
    /// `nLote`.
    pub lote: String,
    /// `qLote`.
    pub quantidade: Option<Decimal>,
    /// `dFab`.
    pub fabricacao: Option<NaiveDate>,
    /// `dVal`.
    pub validade: Option<NaiveDate>,
    /// `cAgreg`.
    pub agregacao: String,
}

/// Impostos do item (`imposto`), cada grupo por inteiro.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct Imposto {
    /// `vTotTrib`.
    pub total_tributos: Option<Decimal>,
    /// O grupo de ICMS que veio (`ICMS00`, `ICMSSN102`...).
    pub icms: Option<Grupo>,
    /// `ICMSUFDest`.
    pub icms_uf_dest: Option<Grupo>,
    /// `IPI` (o `IPITrib` ou `IPINT`, achatado).
    pub ipi: Option<Grupo>,
    /// `II`.
    pub ii: Option<Grupo>,
    /// O grupo de PIS que veio.
    pub pis: Option<Grupo>,
    /// `PISST`.
    pub pis_st: Option<Grupo>,
    /// O grupo de COFINS que veio.
    pub cofins: Option<Grupo>,
    /// `COFINSST`.
    pub cofins_st: Option<Grupo>,
    /// `ISSQN`.
    pub issqn: Option<Grupo>,
    /// `IS` (imposto seletivo).
    pub seletivo: Option<Grupo>,
    /// `IBSCBS`.
    pub ibscbs: Option<Grupo>,
}

impl Imposto {
    /// `orig` + `CST` (3 posicoes) do ICMS, ou a `CSOSN` do Simples Nacional; vazio se nao ha ICMS.
    #[must_use]
    pub fn cst_icms(&self) -> String {
        let Some(g) = &self.icms else {
            return String::new();
        };
        let cst = g.texto("CST");
        if cst.is_empty() {
            g.texto("CSOSN").to_string()
        } else {
            format!("{}{cst}", g.texto("orig"))
        }
    }
}

/// Item da nota (`det`).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Item {
    /// `nItem` (ou a posicao no XML, se o atributo faltar).
    pub n_item: u64,
    /// `cProd`.
    pub codigo: String,
    /// `cEAN`.
    pub gtin: String,
    /// `cBarra`.
    pub codigo_barras: String,
    /// `xProd`.
    pub descricao: String,
    /// `NCM`.
    pub ncm: String,
    /// Cada `NVE`.
    pub nve: Vec<String>,
    /// `CEST`.
    pub cest: String,
    /// `indEscala`.
    pub escala_relevante: String,
    /// `CNPJFab`.
    pub cnpj_fabricante: String,
    /// `cBenef`.
    pub beneficio_fiscal: String,
    /// Cada `gCred`.
    pub credito_presumido: Vec<Folhas>,
    /// `EXTIPI`.
    pub extipi: String,
    /// `CFOP`.
    pub cfop: String,
    /// `uCom`.
    pub unidade: String,
    /// `qCom`.
    pub quantidade: Option<Decimal>,
    /// `vUnCom`.
    pub valor_unitario: Option<Decimal>,
    /// `vProd`.
    pub valor_produto: Option<Decimal>,
    /// `cEANTrib`.
    pub gtin_tributavel: String,
    /// `cBarraTrib`.
    pub codigo_barras_tributavel: String,
    /// `uTrib`.
    pub unidade_tributavel: String,
    /// `qTrib`.
    pub quantidade_tributavel: Option<Decimal>,
    /// `vUnTrib`.
    pub valor_unitario_tributavel: Option<Decimal>,
    /// `vFrete`.
    pub frete: Option<Decimal>,
    /// `vSeg`.
    pub seguro: Option<Decimal>,
    /// `vDesc`.
    pub desconto: Option<Decimal>,
    /// `vOutro`.
    pub outros: Option<Decimal>,
    /// `indTot` (0: o item nao entra no total da nota).
    pub compoe_total: String,
    /// `xPed`.
    pub pedido: String,
    /// `nItemPed`.
    pub item_pedido: String,
    /// `nFCI`.
    pub fci: String,
    /// `nRECOPI`.
    pub recopi: String,
    /// Cada `rastro`.
    pub rastros: Vec<Rastro>,
    /// Cada `DI` (declaracao de importacao).
    pub importacao: Vec<Folhas>,
    /// Cada `detExport`.
    pub exportacao: Vec<Folhas>,
    /// `veicProd`.
    pub veiculo: Folhas,
    /// Cada `med`.
    pub medicamentos: Vec<Folhas>,
    /// Cada `arma`.
    pub armas: Vec<Folhas>,
    /// `comb`.
    pub combustivel: Folhas,
    /// `imposto`.
    pub imposto: Imposto,
    /// `impostoDevol`.
    pub devolucao: Folhas,
    /// `infAdProd`.
    pub informacao_adicional: String,
    /// `obsItem`.
    pub observacoes: Folhas,
    /// `vItem` (valor total do item, NT 2026.008).
    pub valor_item: Option<Decimal>,
    /// `DFeReferenciado`.
    pub dfe_referenciado: Folhas,
}

/// Volume transportado (`vol`).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Volume {
    /// `qVol`.
    pub quantidade: Option<u64>,
    /// `esp`.
    pub especie: String,
    /// `marca`.
    pub marca: String,
    /// `nVol`.
    pub numeracao: String,
    /// `pesoL`.
    pub peso_liquido: Option<Decimal>,
    /// `pesoB`.
    pub peso_bruto: Option<Decimal>,
    /// Cada `nLacre`.
    pub lacres: Vec<String>,
}

/// Transporte (`transp`).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Transporte {
    /// `modFrete`.
    pub modalidade_frete: String,
    /// `transporta`.
    pub transportadora: Option<Participante>,
    /// `retTransp`.
    pub retencao_icms: Folhas,
    /// `veicTransp`.
    pub veiculo: Folhas,
    /// Cada `reboque`.
    pub reboques: Vec<Folhas>,
    /// `vagao`.
    pub vagao: String,
    /// `balsa`.
    pub balsa: String,
    /// Cada `vol`.
    pub volumes: Vec<Volume>,
}

/// Duplicata (`dup`).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Duplicata {
    /// `nDup`.
    pub numero: String,
    /// `dVenc`.
    pub vencimento: Option<NaiveDate>,
    /// `vDup`.
    pub valor: Option<Decimal>,
}

/// Cobranca (`cobr`).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Cobranca {
    /// `fat/nFat`.
    pub fatura_numero: String,
    /// `fat/vOrig`.
    pub fatura_original: Option<Decimal>,
    /// `fat/vDesc`.
    pub fatura_desconto: Option<Decimal>,
    /// `fat/vLiq`.
    pub fatura_liquido: Option<Decimal>,
    /// Cada `dup`.
    pub duplicatas: Vec<Duplicata>,
}

/// Forma de pagamento (`detPag`).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct FormaPagamento {
    /// `indPag`.
    pub indicador: String,
    /// `tPag`.
    pub meio: String,
    /// `xPag`.
    pub descricao: String,
    /// `vPag`.
    pub valor: Option<Decimal>,
    /// `dPag`.
    pub data: Option<NaiveDate>,
    /// `CNPJPag`.
    pub cnpj_pagamento: String,
    /// `UFPag`.
    pub uf_pagamento: String,
    /// `card`.
    pub cartao: Folhas,
}

/// Observacao (`obsCont`, `obsFisco`).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Obs {
    /// Atributo `xCampo`.
    pub campo: String,
    /// `xTexto`.
    pub texto: String,
}

/// Informacoes adicionais (`infAdic`).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct InfAdic {
    /// `infAdFisco`.
    pub fisco: String,
    /// `infCpl`.
    pub complementar: String,
    /// Cada `obsCont`.
    pub obs_contribuinte: Vec<Obs>,
    /// Cada `obsFisco`.
    pub obs_fisco: Vec<Obs>,
    /// Cada `procRef`.
    pub processos: Vec<Folhas>,
}

/// Totais (`total`).
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct Total {
    /// `ICMSTot`.
    pub icms_tot: Option<Grupo>,
    /// `ISSQNtot`.
    pub issqn_tot: Option<Grupo>,
    /// `retTrib`.
    pub retencoes: Option<Grupo>,
    /// `ISTot`.
    pub is_tot: Option<Grupo>,
    /// `IBSCBSTot`.
    pub ibscbs_tot: Option<Grupo>,
    /// `vNFTot`: o `vNF` mais o IBS, a CBS e o IS "por fora"; NAO e' o `vNF`.
    pub valor_total_com_tributos: Option<Decimal>,
}

impl Total {
    /// Campo do `ICMSTot` (`vProd`, `vNF`, `vICMS`...); ausente = zero.
    #[must_use]
    pub fn v(&self, tag: &str) -> Decimal {
        self.icms_tot
            .as_ref()
            .map_or(Decimal::ZERO, |g| g.valor(tag))
    }
}

/// Protocolo de autorizacao (`protNFe/infProt`).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Protocolo {
    /// `tpAmb`.
    pub ambiente: String,
    /// `verAplic`.
    pub versao_aplicativo: String,
    /// `chNFe`, so' os algarismos.
    pub chave: String,
    /// `dhRecbto`.
    pub recebido_em: Option<DataHora>,
    /// `nProt`.
    pub numero: String,
    /// `digVal`.
    pub digest: String,
    /// `cStat`.
    pub cstat: String,
    /// `xMotivo`.
    pub motivo: String,
}

/// A NF-e lida: `NFe` solta (sem protocolo) ou `nfeProc` (com o `protNFe`).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Nfe {
    /// Atributo `versao` do `infNFe`.
    pub versao: String,
    /// Chave de acesso (44 posicoes), do atributo `Id` sem o `NFe`.
    pub chave: String,
    /// Por que a chave nao confere (formato, DV, UF, mes, modelo ou emitente); `None` = confere.
    #[serde(serialize_with = "serializar_erro_da_chave")]
    pub chave_erro: Option<crate::chave_dfe::ErroChave>,
    /// `ide/mod`.
    pub modelo: String,
    /// `ide`.
    pub ide: Ide,
    /// `emit`.
    pub emit: Participante,
    /// `dest`.
    pub dest: Option<Participante>,
    /// `retirada`.
    pub retirada: Option<Participante>,
    /// `entrega`.
    pub entrega: Option<Participante>,
    /// Documento de cada `autXML`.
    pub autorizados_xml: Vec<String>,
    /// Cada `det`.
    pub itens: Vec<Item>,
    /// `total`.
    pub total: Total,
    /// `transp`.
    pub transporte: Option<Transporte>,
    /// `cobr`.
    pub cobranca: Option<Cobranca>,
    /// Cada `pag/detPag`.
    pub pagamentos: Vec<FormaPagamento>,
    /// `pag/vTroco`.
    pub troco: Option<Decimal>,
    /// `infIntermed`.
    pub intermediador: Folhas,
    /// `infAdic`.
    pub inf_adic: Option<InfAdic>,
    /// `exporta`.
    pub exportacao: Folhas,
    /// `compra`.
    pub compra: Folhas,
    /// `cana`.
    pub cana: Folhas,
    /// `infRespTec`.
    pub responsavel_tecnico: Folhas,
    /// `agropecuario`.
    pub agropecuario: Folhas,
    /// `infNFeSupl/qrCode`.
    pub qr_code: String,
    /// `infNFeSupl/urlChave`.
    pub url_chave: String,
    /// `protNFe/infProt`, se a nota veio autorizada (`nfeProc`).
    pub protocolo: Option<Protocolo>,
}

fn serializar_erro_da_chave<S: serde::Serializer>(
    erro: &Option<crate::chave_dfe::ErroChave>,
    s: S,
) -> Result<S::Ok, S::Error> {
    match erro {
        Some(e) => s.serialize_some(&e.to_string()),
        None => s.serialize_none(),
    }
}
