// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O pedido ja' lido: a [`Nota`] e as partes dela. Os campos seguem o leiaute 4.00 (nomes e ordem do Anexo I do MOC
//! 7.00 e das NT); valor monetario e percentual vao como texto decimal (`"10.50"`), nunca como ponto flutuante.

use std::collections::BTreeMap;

use chrono::{DateTime, FixedOffset};

use crate::reforma::calculo::ItemIbsCbs;

use super::cobr::Cobranca;
use super::comb::Combustivel;
use super::difal;
use super::exporta::{Compra, Exportacao};
use super::produto::{Medicamento, Rastro};
use super::transp::Transporte;

/// Endereco do emitente, do destinatario, da retirada ou da entrega.
#[derive(Debug, Clone, Default)]
pub struct Endereco {
    /// `xLgr`.
    pub logradouro: String,
    /// `nro`.
    pub numero: String,
    /// `xBairro`.
    pub bairro: String,
    /// `cMun`: codigo IBGE, 7 digitos.
    pub cod_municipio: String,
    /// `xMun`.
    pub municipio: String,
    /// `UF`; `EX` para o exterior.
    pub uf: String,
    /// `CEP`, 8 digitos.
    pub cep: Option<String>,
    /// `xCpl`.
    pub complemento: Option<String>,
    /// `cPais`; vazio = 1058 (Brasil) no emitente e no destinatario.
    pub cod_pais: Option<String>,
    /// `xPais`; vazio = BRASIL no emitente e no destinatario.
    pub pais: Option<String>,
    /// `fone`.
    pub fone: Option<String>,
}

/// Emitente (grupo C).
#[derive(Debug, Clone, Default)]
pub struct Emitente {
    /// CNPJ (numerico ou alfanumerico) ou CPF.
    pub documento: String,
    /// `xNome`.
    pub nome: String,
    /// `enderEmit`.
    pub endereco: Endereco,
    /// `CRT`: 1 Simples Nacional, 2 Simples com excesso de sublimite, 3 regime normal, 4 MEI.
    pub crt: i64,
    /// `IE`.
    pub ie: Option<String>,
    /// `xFant`.
    pub fantasia: Option<String>,
    /// `IEST`.
    pub iest: Option<String>,
    /// `IM`.
    pub im: Option<String>,
    /// `CNAE`, so' sai com a inscricao municipal.
    pub cnae: Option<String>,
    /// C21a, NT 2026.005: 1 Simples com regime regular do IBS/CBS; 2, 3 e 4 regime normal (CRT 3) com a opcao pelo
    /// Simples pendente. Ausente para os demais.
    pub tp_regime_ibscbs: Option<i64>,
    /// C22 `ISUFEmit` (NT 2025.002): inscricao na Suframa do emitente da ZFM ou das ALC.
    pub isuf: Option<String>,
}

/// Destinatario (grupo E).
#[derive(Debug, Clone)]
pub struct Destinatario {
    /// CNPJ ou CPF.
    pub documento: Option<String>,
    /// `idEstrangeiro`.
    pub id_estrangeiro: Option<String>,
    /// `xNome`.
    pub nome: Option<String>,
    /// `enderDest`.
    pub endereco: Option<Endereco>,
    /// `indIEDest`: 1, 2 ou 9.
    pub ind_ie_dest: i64,
    /// `IE`.
    pub ie: Option<String>,
    /// `ISUF`.
    pub isuf: Option<String>,
    /// `IM`.
    pub im: Option<String>,
    /// `email`.
    pub email: Option<String>,
}

/// Retirada (F01) ou entrega (G01), tipo `TLocal` do XSD: so' quando difere do endereco do emitente ou do
/// destinatario. CNPJ ou CPF e' obrigatorio; o pais so' sai se informado.
#[derive(Debug, Clone, Default)]
pub struct Local {
    /// CNPJ ou CPF do local.
    pub documento: String,
    /// Endereco.
    pub endereco: Endereco,
    /// `xNome`.
    pub nome: Option<String>,
    /// `email`.
    pub email: Option<String>,
    /// `IE`.
    pub ie: Option<String>,
}

/// Grupo J `veicProd`: veiculo novo. Todos os campos sao obrigatorios no XSD e vao como texto; os codigos de marca,
/// modelo e cor sao os da tabela do Denatran.
#[derive(Debug, Clone, Default)]
pub struct Veiculo {
    /// `tpOp`: 0 outros, 1 concessionaria, 2 faturamento direto, 3 venda direta a grande consumidor. No faturamento
    /// direto o `vNF` e o `vItem` nao somam o ICMS-ST (W16-10, excecao 1) e a nota exige o local de entrega
    /// (rejeicao 478).
    pub tp_op: String,
    /// `chassi`: 17 posicoes.
    pub chassi: String,
    /// `cCor`.
    pub c_cor: String,
    /// `xCor`.
    pub x_cor: String,
    /// `pot`.
    pub pot: String,
    /// `cilin`.
    pub cilin: String,
    /// `pesoL`.
    pub peso_l: String,
    /// `pesoB`.
    pub peso_b: String,
    /// `nSerie`.
    pub n_serie: String,
    /// `tpComb`.
    pub tp_comb: String,
    /// `nMotor`.
    pub n_motor: String,
    /// `CMT`.
    pub cmt: String,
    /// `dist`.
    pub dist: String,
    /// `anoMod`.
    pub ano_mod: String,
    /// `anoFab`.
    pub ano_fab: String,
    /// `tpPint`.
    pub tp_pint: String,
    /// `tpVeic`.
    pub tp_veic: String,
    /// `espVeic`.
    pub esp_veic: String,
    /// `VIN`: `R` remarcado ou `N` normal.
    pub vin: String,
    /// `condVeic`.
    pub cond_veic: String,
    /// `cMod`.
    pub c_mod: String,
    /// `cCorDENATRAN`.
    pub c_cor_denatran: String,
    /// `lota`.
    pub lota: String,
    /// `tpRest`.
    pub tp_rest: String,
}

/// ICMS do item (grupo N): o CST (regime normal) ou o CSOSN (Simples) e os valores que o grupo leva. Os valores sao
/// PRONTOS: a montagem confere as contas que o MOC manda conferir, mas a base, a aliquota e a margem de cada UF
/// vem do pedido.
#[derive(Debug, Clone, Default)]
pub struct Icms {
    /// CST (2 digitos) ou CSOSN (3 digitos), conforme o CRT do emitente.
    pub codigo: String,
    /// `orig`: 0 a 8.
    pub orig: i64,
    /// `modBC`.
    pub mod_bc: i64,
    /// `vBC`.
    pub vbc: Option<String>,
    /// `pRedBC`.
    pub p_red_bc: Option<String>,
    /// `pICMS`.
    pub p_icms: Option<String>,
    /// `vICMS`.
    pub v_icms: Option<String>,
    /// `vBCFCP`.
    pub vbc_fcp: Option<String>,
    /// `pFCP`.
    pub p_fcp: Option<String>,
    /// `vFCP`.
    pub v_fcp: Option<String>,
    /// `vBCSTRet`.
    pub vbc_st_ret: Option<String>,
    /// `pST`.
    pub p_st: Option<String>,
    /// `vICMSSubstituto`.
    pub v_icms_substituto: Option<String>,
    /// `vICMSSTRet`.
    pub v_icms_st_ret: Option<String>,
    /// `pCredSN`.
    pub p_cred_sn: Option<String>,
    /// `vCredICMSSN`.
    pub v_cred_icms_sn: Option<String>,
    /// ST propria: CST 10, 30 e 70 e CSOSN 201, 202 e 203 exigem; CST 90 e CSOSN 900 levam se informada. N18: 0
    /// tabelado, 1 lista negativa, 2 positiva, 3 neutra, 4 MVA, 5 pauta, 6 valor da operacao.
    pub mod_bc_st: Option<i64>,
    /// N19 `pMVAST`: so' com `modBCST` 4 (rejeicoes 932 e 933).
    pub p_mva_st: Option<String>,
    /// `pRedBCST`.
    pub p_red_bc_st: Option<String>,
    /// `vBCST`.
    pub vbc_st: Option<String>,
    /// N22 `pICMSST`, sem o FCP.
    pub p_icms_st: Option<String>,
    /// `vICMSST`.
    pub v_icms_st: Option<String>,
    /// N23a `vBCFCPST`: FCP retido por ST (grupo opcional).
    pub vbc_fcp_st: Option<String>,
    /// N23b `pFCPST` (zero e' a rejeicao 881).
    pub p_fcp_st: Option<String>,
    /// N23d `vFCPST`.
    pub v_fcp_st: Option<String>,
    /// Diferimento (CST 51), todos opcionais: N16a `vICMSOp`, o ICMS sem o diferimento.
    pub v_icms_op: Option<String>,
    /// N16b `pDif`: percentual do diferimento (100 = total).
    pub p_dif: Option<String>,
    /// N16c `vICMSDif`: `vICMSOp` x `pDif`.
    pub v_icms_dif: Option<String>,
    /// N17d `pFCPDif`.
    pub p_fcp_dif: Option<String>,
    /// N17e `vFCPDif`.
    pub v_fcp_dif: Option<String>,
    /// N17f `vFCPEfet`.
    pub v_fcp_efet: Option<String>,
    /// N28a `vICMSDeson`: CST 20, 30, 40, 41, 50, 70 e 90; sai so' com valor maior que zero.
    pub v_icms_deson: Option<String>,
    /// N28 `motDesICMS`: codigo do motivo, na enumeracao do XSD de cada grupo.
    pub mot_des_icms: Option<String>,
    /// N28b `indDeduzDeson`: 1 = o `vICMSDeson` sai do `vItem` e do `vNF` (W16-10, excecao 4).
    pub ind_deduz_deson: Option<i64>,
    /// N33a `vICMSSTDeson`: ICMS-ST desonerado (CST 10, 70 e 90); nao tem total no `ICMSTot`.
    pub v_icms_st_deson: Option<String>,
    /// N33b `motDesICMSST`.
    pub mot_des_icms_st: Option<String>,
    /// Partilha do ICMS entre a UF de origem e a do destino (grupo `ICMSPart`): CST 10 e 90. Exige o ICMS proprio
    /// e a ST, sem FCP proprio.
    pub partilha: bool,
    /// N25 `pBCOp`.
    pub p_bc_op: Option<String>,
    /// N24 `UFST` (`EX` = exterior).
    pub uf_st: Option<String>,
    /// Repasse do ICMS-ST retido (grupo `ICMSST`): CST 41 e 60.
    pub repasse: bool,
    /// N31 `vBCSTDest`.
    pub vbc_st_dest: Option<String>,
    /// N32 `vICMSSTDest`.
    pub v_icms_st_dest: Option<String>,
    /// N27a `vBCFCPSTRet`: FCP retido anteriormente por ST (os tres campos ou nenhum).
    pub vbc_fcp_st_ret: Option<String>,
    /// N27b `pFCPSTRet`.
    pub p_fcp_st_ret: Option<String>,
    /// N27d `vFCPSTRet`.
    pub v_fcp_st_ret: Option<String>,
    /// N34 `pRedBCEfet`: ICMS efetivo (os quatro campos ou nenhum).
    pub p_red_bc_efet: Option<String>,
    /// N35 `vBCEfet`.
    pub vbc_efet: Option<String>,
    /// N36 `pICMSEfet`.
    pub p_icms_efet: Option<String>,
    /// N37 `vICMSEfet`.
    pub v_icms_efet: Option<String>,
    /// N14a `cBenefRBC` (NT 2019.001): beneficio fiscal da reducao de base quando acumula com o diferimento (CST 51)
    /// ou no CST 90. Confere-se o formato do XSD; se o codigo existe na tabela da UF e' assunto da UF.
    pub c_benef_rbc: Option<String>,
}

/// PIS ou COFINS do item (grupos Q e S).
#[derive(Debug, Clone, Default)]
pub struct PisCofins {
    /// CST: 01 e 02 (aliquota), 04 a 09 (nao tributado), 49 a 99 (outras operacoes).
    pub cst: String,
    /// `vBC`.
    pub vbc: Option<String>,
    /// `pPIS` ou `pCOFINS`.
    pub aliquota: Option<String>,
    /// `vPIS` ou `vCOFINS`.
    pub valor: Option<String>,
}

/// IPI do item (grupo O).
#[derive(Debug, Clone, Default)]
pub struct Ipi {
    /// CST: 00, 49, 50 e 99 tributam; 01 a 05 e 51 a 55 nao.
    pub cst: String,
    /// `cEnq`: 3 digitos.
    pub c_enq: String,
    /// `vBC`.
    pub vbc: Option<String>,
    /// `pIPI`.
    pub aliquota: Option<String>,
    /// `vIPI`.
    pub valor: Option<String>,
}

/// Item da nota (`det`).
#[derive(Debug, Clone, Default)]
pub struct Item {
    /// `cProd`.
    pub codigo: String,
    /// `xProd`.
    pub descricao: String,
    /// `NCM`: 2 ou 8 digitos.
    pub ncm: String,
    /// `CFOP`.
    pub cfop: String,
    /// `uCom`.
    pub unidade: String,
    /// `qCom`.
    pub quantidade: String,
    /// `vUnCom`.
    pub valor_unitario: String,
    /// ICMS. Ausente so' na nota de credito ou de debito (`finNFe` 5 ou 6, B25-80); nas demais e' a rejeicao 1002.
    pub icms: Option<Icms>,
    /// PIS. Ausente so' na nota de credito ou de debito; nas demais e' a rejeicao 745.
    pub pis: Option<PisCofins>,
    /// COFINS. Ausente so' na nota de credito ou de debito; nas demais e' a rejeicao 748.
    pub cofins: Option<PisCofins>,
    /// `vProd`; vazio = `qCom` x `vUnCom` arredondado a 2 casas.
    pub valor_produto: Option<String>,
    /// `cEAN`; vazio = `SEM GTIN`.
    pub gtin: Option<String>,
    /// `cEANTrib`; vazio = o `cEAN`.
    pub gtin_trib: Option<String>,
    /// `uTrib`; vazio = `uCom`.
    pub unidade_trib: Option<String>,
    /// `qTrib`; vazio = `qCom`.
    pub quantidade_trib: Option<String>,
    /// `vUnTrib`; vazio = `vUnCom`.
    pub valor_unitario_trib: Option<String>,
    /// `CEST`.
    pub cest: Option<String>,
    /// `cBenef`.
    pub cbenef: Option<String>,
    /// `EXTIPI`.
    pub extipi: Option<String>,
    /// `vFrete`.
    pub frete: Option<String>,
    /// `vSeg`.
    pub seguro: Option<String>,
    /// `vDesc`.
    pub desconto: Option<String>,
    /// `vOutro`.
    pub outros: Option<String>,
    /// `indTot`: 0 ou 1.
    pub ind_tot: i64,
    /// IPI.
    pub ipi: Option<Ipi>,
    /// IBS/CBS do item, ja' calculado ([`crate::reforma::calculo::ibscbs_do_item`]).
    pub ibscbs: Option<ItemIbsCbs>,
    /// `vTotTrib`.
    pub v_tot_trib: Option<String>,
    /// `infAdProd`.
    pub inf_ad_prod: Option<String>,
    /// `xPed`.
    pub x_ped: Option<String>,
    /// `nItemPed`.
    pub n_item_ped: Option<String>,
    /// `DFeReferenciado`: chave do documento e `nItem` dele (devolucao, VC02-14).
    pub dfe_ref: Option<(String, Option<i64>)>,
    /// I11b `vUnComLiq` (NT 2026.008): valor unitario sem tributo; abre a sequencia I11a.
    pub valor_unitario_liquido: Option<String>,
    /// I11c `vProdLiq`; vazio = `qCom` x `vUnComLiq` arredondado a 2 casas.
    pub valor_produto_liquido: Option<String>,
    /// I17d `cAtivSN` (NT 2026.005): 01, 02, 03, 04 ou 90; so' emitente do Simples ou com a opcao pendente.
    pub c_ativ_sn: Option<String>,
    /// NB02 `vICMSPrevisto` (NT 2026.008): ICMS do fornecimento futuro, no lugar do grupo de ICMS; so' com
    /// `tpNFDebito` 06 ou recebimento antecipado de compra governamental.
    pub v_icms_previsto: Option<String>,
    /// NA01 `ICMSUFDest` (DIFAL), calculado por [`difal::calcular`].
    pub icms_uf_dest: Option<difal::Grupo>,
    /// Dispensa do DIFAL pelas excecoes da rejeicao 694: `cfop_retorno` ou `cfop_remessa`.
    pub difal_dispensado: Option<String>,
    /// J `veicProd`.
    pub veiculo: Option<Veiculo>,
    /// LA `comb`: combustivel liquido e lubrificante. Um grupo especifico so' por item (`veicProd`, `med` ou `comb`).
    pub combustivel: Option<Combustivel>,
    /// K `med`: medicamento. Exige o `rastro` (rejeicao 873), salvo as excecoes da regra K01-20.
    pub med: Option<Medicamento>,
    /// I80 `rastro`: lotes do item, de 0 a 500.
    pub rastro: Vec<Rastro>,
}

/// Dados do cartao de um pagamento (`card`).
#[derive(Debug, Clone, Default)]
pub struct Cartao {
    /// `tpIntegra`.
    pub tp_integra: i64,
    /// CNPJ da credenciadora.
    pub cnpj: Option<String>,
    /// `tBand`.
    pub t_band: Option<String>,
    /// `cAut`.
    pub c_aut: Option<String>,
}

/// Um pagamento (`detPag`).
#[derive(Debug, Clone, Default)]
pub struct Pagamento {
    /// `tPag`.
    pub t_pag: String,
    /// `vPag`.
    pub valor: String,
    /// `indPag`.
    pub ind_pag: Option<i64>,
    /// `xPag`: obrigatorio com `tPag` 99.
    pub x_pag: Option<String>,
    /// `card`.
    pub cartao: Option<Cartao>,
}

/// Responsavel tecnico (`infRespTec`).
#[derive(Debug, Clone, Default)]
pub struct RespTec {
    /// CNPJ do responsavel.
    pub cnpj: String,
    /// `xContato`.
    pub contato: String,
    /// `email`.
    pub email: String,
    /// `fone`.
    pub fone: String,
    /// `idCSRT`.
    pub id_csrt: Option<i64>,
    /// Codigo de seguranca do responsavel tecnico; com ele a nota leva `idCSRT` e `hashCSRT`.
    pub csrt: Option<String>,
}

/// `gCompraGov` (BB01, NT 2025.002): compra por ente governamental.
#[derive(Debug, Clone, Default)]
pub struct CompraGov {
    /// BB02: 1 Uniao, 2 Estado, 3 Distrito Federal, 4 Municipio, 5 consorcio publico, 6 CGIBS.
    pub tp_ente: i64,
    /// BB03: redutor do art. 472 da LC 214/2025.
    pub p_redutor: String,
    /// BB04: 1 fornecimento com pagamento posterior, 2 recebimento do pagamento (fornecimento feito), 3 fornecimento
    /// com pagamento feito, 4 recebimento do pagamento (fornecimento posterior).
    pub tp_oper: i64,
    /// BB05 `refDFeAnt`: chaves dos DF-e anteriores (so' com `tpOperGov` 2 e 3; de 0 a 99).
    pub refs: Vec<String>,
}

/// A NF-e (modelo 55) pedida.
#[derive(Debug, Clone)]
pub struct Nota {
    /// Modelo: so' o 55 e' montado.
    pub modelo: i64,
    /// `serie`.
    pub serie: i64,
    /// `nNF`.
    pub numero: i64,
    /// `dhEmi`, com fuso.
    pub emissao: DateTime<FixedOffset>,
    /// `natOp`.
    pub nat_op: String,
    /// `cMunFG`.
    pub cmun_fg: String,
    /// Emitente.
    pub emit: Emitente,
    /// Itens (de 1 a 990).
    pub itens: Vec<Item>,
    /// Pagamentos (de 1 a 100).
    pub pagamentos: Vec<Pagamento>,
    /// Destinatario.
    pub dest: Option<Destinatario>,
    /// `tpAmb`: 1 producao, 2 homologacao.
    pub tp_amb: i64,
    /// `tpEmis`.
    pub tp_emis: i64,
    /// `tpNF`: 0 entrada, 1 saida.
    pub tp_nf: i64,
    /// `idDest`.
    pub id_dest: i64,
    /// `finNFe`.
    pub fin_nfe: i64,
    /// `indFinal`; vazio = 0.
    pub ind_final: Option<i64>,
    /// `indPres`; vazio = 9.
    pub ind_pres: Option<i64>,
    /// `indIntermed`.
    pub ind_intermed: Option<i64>,
    /// `tpImp`; vazio = 1. O 6 (DANFE simplificado) pede o QR Code: a nota monta, mas nao e' assinada.
    pub tp_imp: Option<i64>,
    /// `procEmi`.
    pub proc_emi: i64,
    /// `verProc`; o pedido JSON sem ele leva `rustnf <versao>`.
    pub ver_proc: String,
    /// `cNF` (8 digitos). Obrigatorio na montagem: ela nao sorteia, para a mesma entrada dar o mesmo XML (veja
    /// `chave_dfe::gerar_codigo`).
    pub codigo: Option<i64>,
    /// `dhSaiEnt`.
    pub saida: Option<DateTime<FixedOffset>>,
    /// `cMunFGIBS`.
    pub cmun_fg_ibs: Option<String>,
    /// `dhCont`.
    pub dh_cont: Option<DateTime<FixedOffset>>,
    /// `xJust`.
    pub x_just: Option<String>,
    /// `NFref/refNFe`.
    pub refs_nfe: Vec<String>,
    /// `vTroco`.
    pub troco: Option<String>,
    /// `modFrete`.
    pub mod_frete: i64,
    /// `infAdFisco`.
    pub inf_ad_fisco: Option<String>,
    /// `infCpl`.
    pub inf_cpl: Option<String>,
    /// `infRespTec`.
    pub resp_tec: Option<RespTec>,
    /// Tira acento e caractere fora do ASCII dos textos (padrao do pedido JSON: sim).
    pub remover_acento: bool,
    /// B25.1 `tpNFDebito` (NT 2025.002): "01" a "08", so' com `finNFe` 6.
    pub tp_nf_debito: Option<String>,
    /// B25.2 `tpNFCredito`: "01" a "06", so' com `finNFe` 5.
    pub tp_nf_credito: Option<String>,
    /// BC02 `gPagAntecipado/refNFe`: chaves das notas de debito 06 a abater (de 1 a 99).
    pub pag_antecipado: Vec<String>,
    /// BB01 `gCompraGov`. O item leva o `gTribCompraGov` com o mesmo `pRedutor`.
    pub compra_gov: Option<CompraGov>,
    /// B25d `cIndOp` (NT 2025.002): so' codigo que o Anexo VII admite na NF-e ([`crate::reforma::indop`]).
    pub c_ind_op: Option<String>,
    /// F01: local de retirada (obrigatorio com `cIndOp` 010104 ou 010105, B25d-30).
    pub retirada: Option<Local>,
    /// G01: local de entrega.
    pub entrega: Option<Local>,
    /// X03 a X34: transportadora, retencao do ICMS do transporte, veiculo, reboques, vagao, balsa e volumes (o
    /// `modFrete` e' [`Nota::mod_frete`]).
    pub transp: Option<Transporte>,
    /// Y01 `cobr`: fatura e duplicatas.
    pub cobr: Option<Cobranca>,
    /// ZA01 `exporta`: obrigatorio na exportacao (`tpNF` 1 e `idDest` 3, rejeicao 355) e proibido fora dela (356).
    pub exporta: Option<Exportacao>,
    /// ZB01 `compra`: nota de empenho, pedido e contrato.
    pub compra: Option<Compra>,
}

/// A nota montada, ainda sem assinatura.
#[derive(Debug, Clone)]
pub struct Montada {
    /// O XML da `NFe` (sem `Signature`).
    pub xml: String,
    /// A chave de acesso de 44 posicoes.
    pub chave: String,
    /// Os totais do `ICMSTot` (e `vNFTot`, `vProdLiqTot`) em texto com 2 casas.
    pub totais: BTreeMap<String, String>,
}
