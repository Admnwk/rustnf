// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O modelo da DPS: os campos entram direto do JSON do contrato (serde).

use super::*;

pub const AMBIENTE_PRODUCAO: u8 = 1;
pub const AMBIENTE_HOMOLOGACAO: u8 = 2;
pub const EMITENTE_PRESTADOR: u8 = 1;

/// Nacional (`cmun` + `cep`) ou exterior (`pais` ISO, `cod_postal`, `cidade`, `estado`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Endereco {
    pub logradouro: String,
    pub numero: String,
    pub bairro: String,
    pub cmun: String,
    pub cep: String,
    pub complemento: String,
    pub pais: String,
    pub cod_postal: String,
    pub cidade: String,
    pub estado: String,
}

/// Endereco de obra/evento/imovel: CEP (nacional) ou exterior.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct EnderecoSimples {
    pub logradouro: String,
    pub numero: String,
    pub bairro: String,
    pub cep: String,
    pub complemento: String,
    pub cod_postal: String,
    pub cidade: String,
    pub estado: String,
}

/// Tomador, intermediario, fornecedor ou destinatario: `documento` (CPF/CNPJ) OU `nif` OU `sem_nif` (0/1/2).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Pessoa {
    pub nome: String,
    pub documento: String,
    pub nif: String,
    pub sem_nif: Option<u8>,
    pub caepf: String,
    pub im: String,
    pub endereco: Option<Endereco>,
    pub fone: String,
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Prestador {
    #[serde(flatten)]
    pub pessoa: Pessoa,
    /// 1 nao optante, 2 MEI, 3 ME/EPP.
    pub op_simples: u8,
    /// So' ME/EPP (E0162/E0166).
    pub apuracao_sn: Option<u8>,
    pub regime_especial: u8,
}

impl Default for Prestador {
    fn default() -> Self {
        Prestador {
            pessoa: Pessoa::default(),
            op_simples: 1,
            apuracao_sn: None,
            regime_especial: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Substituicao {
    pub chave: String,
    pub motivo: Valor,
    pub texto: String,
}

impl Default for Substituicao {
    fn default() -> Self {
        Substituicao {
            chave: String::new(),
            motivo: Valor::from("99"),
            texto: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ComercioExterior {
    pub modo: u8,
    pub vinculo: u8,
    pub moeda: Valor,
    pub valor_moeda: Valor,
    pub mec_prestador: Valor,
    pub mec_tomador: Valor,
    pub mov_temp_bens: u8,
    pub mdic: u8,
    pub num_di: String,
    pub num_re: String,
}

impl Default for ComercioExterior {
    fn default() -> Self {
        ComercioExterior {
            modo: 0,
            vinculo: 0,
            moeda: Valor::default(),
            valor_moeda: Valor::default(),
            mec_prestador: Valor::from("00"),
            mec_tomador: Valor::from("00"),
            mov_temp_bens: 0,
            mdic: 0,
            num_di: String::new(),
            num_re: String::new(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Obra {
    pub codigo: String,
    pub cib: String,
    pub endereco: Option<EnderecoSimples>,
    pub inscricao_imobiliaria: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AtividadeEvento {
    pub nome: String,
    pub inicio: String,
    pub fim: String,
    pub identificacao: String,
    pub endereco: Option<EnderecoSimples>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct InfoComplementar {
    pub id_doc_tecnico: String,
    pub doc_referencia: String,
    pub pedido: String,
    pub itens_pedido: Vec<String>,
    pub texto: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Servico {
    pub ctribnac: Valor,
    pub descricao: String,
    pub cmun_prestacao: String,
    pub pais_prestacao: String,
    pub ctribmun: String,
    pub cnbs: String,
    pub cod_interno: String,
    pub comercio_exterior: Option<ComercioExterior>,
    pub obra: Option<Obra>,
    pub evento: Option<AtividadeEvento>,
    pub complemento: Option<InfoComplementar>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DocDeducao {
    pub tipo: u8,
    pub emissao: String,
    pub valor_dedutivel: Valor,
    pub valor_deducao: Valor,
    pub chave_nfse: String,
    pub chave_nfe: String,
    pub nfse_mun_cmun: String,
    pub nfse_mun_numero: String,
    pub nfse_mun_verif: String,
    pub nfnfs_numero: String,
    pub nfnfs_modelo: String,
    pub nfnfs_serie: String,
    pub doc_fiscal: String,
    pub doc: String,
    pub descricao_outras: String,
    pub fornecedor: Option<Pessoa>,
}

/// Leiaute nt009: documento de ajuste (deducao/reducao) da base -- ISSQN (tpAjusteBC 2-99) ou repasse/reembolso
/// que tira da base do IBS/CBS (101-105, 199; substitui o gReeRepRes). Identificacao: UM de (dfe_tipo + dfe_chave),
/// (outro_cmun + outro_numero + outro_descricao), (doc_numero + doc_descricao).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DocAjusteBC {
    pub tipo: Valor,
    pub valor_total: Valor,
    pub valor_aplicado: Valor,
    pub emissao: String,
    pub competencia: String,
    pub tipo_descricao: String,
    pub dfe_tipo: Option<u8>,
    pub dfe_tipo_descricao: String,
    pub dfe_chave: String,
    pub outro_cmun: String,
    pub outro_numero: String,
    pub outro_descricao: String,
    pub doc_numero: String,
    pub doc_descricao: String,
    pub fornecedor: Option<Pessoa>,
}

/// UMA forma: percentual OU valor OU documentos (leiaute producao, docDedRed) / ajustes (leiaute nt009, docAjusteBC).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Deducao {
    pub percentual: Option<Valor>,
    pub valor: Option<Valor>,
    pub documentos: Vec<DocDeducao>,
    pub ajustes: Vec<DocAjusteBC>,
}

/// Leiaute nt009, gAjusteBCLocImoveis (so' cTribNac 99.03, E0939). `valor` cheio, sem a copropriedade.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AjusteLocImovel {
    pub tipo: Valor,
    pub valor: Valor,
    pub descricao: String,
}

/// Leiaute nt009, gUnidImob: `cib` OU `endereco` (CEP obrigatorio; bairro opcional).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct UnidadeImobiliaria {
    pub cib: String,
    pub endereco: Option<EnderecoSimples>,
    pub inscricao: String,
    pub ajustes: Vec<AjusteLocImovel>,
}

/// Leiaute nt009, gLocacao. Em 99.03 o vServ e os descontos sao estes totais x copropriedade (E0433/E0434/E0443).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LocacaoImovel {
    pub copropriedade: Valor,
    pub valor_total: Valor,
    pub desconto_incondicionado: Option<Valor>,
    pub desconto_condicionado: Option<Valor>,
    pub vencimento: String,
}

/// Leiaute nt009, grupo imovel do IBSCBS (exceto obra).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Imovel {
    pub cmun: String,
    pub locacao: Option<LocacaoImovel>,
    pub unidades: Vec<UnidadeImobiliaria>,
}

/// Leiaute nt009, bensMoveis (obrigatorio so' em cTribNac 99.04.01, E0963/E0967).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct BemMovel {
    pub ncm: String,
    pub descricao: String,
    pub quantidade: Valor,
}

/// Leiaute de producao (XSD `nfse-esquemas_xsd-v1-01-20260209`) -- o padrao.
pub const LEIAUTE_PRODUCAO: &str = "producao";
/// XSD da Producao Restrita de 20/10/2026 (`esquemas_prod_restrita_20261020`), parte da NT 009 v1.01: finNFSe antes
/// de cLocEmi, vAjusteBC, CST/cClassTrib direto em trib, imovel com gLocacao/gUnidImob, bensMoveis, xDescServ no fim
/// do cServ. Regras: Anexo I PRODREST `anexo_i-sefin_adn-dps_nfse-snnfse_prodrest-v1-01-20261020.xlsx`.
pub const LEIAUTE_NT009: &str = "nt009";
pub const LEIAUTES: [&str; 2] = [LEIAUTE_PRODUCAO, LEIAUTE_NT009];
pub(super) const TIPO_AJUSTE_BC: &[&str] = &[
    "2", "5", "6", "7", "8", "9", "99", "101", "102", "103", "104", "105", "199",
];
/// tpAjusteBC de repasse/reembolso (base do IBS/CBS; liberam o ajuste em E0435/E0445).
pub const TIPO_AJUSTE_BC_REPASSE: [&str; 6] = ["101", "102", "103", "104", "105", "199"];
/// tpAjusteBC so' do ISSQN: vedados nos documentos quando ja' ha' pAjusteBCISSQN/vAjusteBCISSQN (E0462, Anexo I PRODREST
/// AI:433 -- a coluna REGRA lista 2/5/6/7/8/9/99; a MENSAGEM acrescenta o 105, vale a regra).
pub const TIPO_AJUSTE_BC_SO_ISSQN: [&str; 7] = ["2", "5", "6", "7", "8", "9", "99"];
pub(super) const TIPO_AJUSTE_LOC_IMOVEL: &[&str] = &["01", "02", "03", "04", "99"];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TributacaoMunicipal {
    pub tributacao: u8,
    pub retencao: u8,
    /// pAliq (max 5%, E0595): quando vai depende do regime e do convenio do municipio (Anexo I E0617/E0619 nao
    /// optante: so' com convenio NAO ativo; E0621/E0625/E0628 Simples ME/EPP: conforme apuracao e retencao).
    pub aliquota: Option<Valor>,
    pub pais_resultado: String,
    pub imunidade: Option<u8>,
    pub suspensao_tipo: Option<u8>,
    pub suspensao_processo: String,
    pub beneficio_numero: String,
    pub beneficio_reducao_valor: Option<Valor>,
    pub beneficio_reducao_percentual: Option<Valor>,
}

impl Default for TributacaoMunicipal {
    fn default() -> Self {
        TributacaoMunicipal {
            tributacao: 1,
            retencao: 1,
            aliquota: None,
            pais_resultado: String::new(),
            imunidade: None,
            suspensao_tipo: None,
            suspensao_processo: String::new(),
            beneficio_numero: String::new(),
            beneficio_reducao_valor: None,
            beneficio_reducao_percentual: None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PisCofins {
    pub cst: Valor,
    pub base: Option<Valor>,
    pub aliquota_pis: Option<Valor>,
    pub aliquota_cofins: Option<Valor>,
    pub valor_pis: Option<Valor>,
    pub valor_cofins: Option<Valor>,
    pub tipo_retencao: Option<u8>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TributacaoFederal {
    pub pis_cofins: Option<PisCofins>,
    pub retencao_cp: Option<Valor>,
    pub retencao_irrf: Option<Valor>,
    pub retencao_csll: Option<Valor>,
}

/// UMA forma (Lei 12.741): valores OU percentuais OU percentual do Simples OU nada (indTotTrib 0).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TotalTributos {
    pub valor_federal: Option<Valor>,
    pub valor_estadual: Option<Valor>,
    pub valor_municipal: Option<Valor>,
    pub percentual_federal: Option<Valor>,
    pub percentual_estadual: Option<Valor>,
    pub percentual_municipal: Option<Valor>,
    pub percentual_sn: Option<Valor>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Valores {
    pub servico: Valor,
    pub recebido: Option<Valor>,
    pub desconto_incondicionado: Option<Valor>,
    pub desconto_condicionado: Option<Valor>,
    pub deducao: Option<Deducao>,
    pub municipal: TributacaoMunicipal,
    pub federal: Option<TributacaoFederal>,
    pub total: TotalTributos,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DocReembolso {
    pub tipo: Valor,
    pub emissao: String,
    pub competencia: String,
    pub valor: Valor,
    pub dfe_tipo: Option<u8>,
    pub dfe_tipo_descricao: String,
    pub dfe_chave: String,
    pub outro_cmun: String,
    pub outro_numero: String,
    pub outro_descricao: String,
    pub doc_numero: String,
    pub doc_descricao: String,
    pub tipo_descricao: String,
    pub fornecedor: Option<Pessoa>,
}

/// Grupo da Reforma. Leiaute producao = 1.01 vigente (NT 004 + `tpRetPisCofins` da NT 007); leiaute nt009 = XSD da
/// Producao Restrita de 20/10/2026 (`LEIAUTE_NT009`): `imovel` e `bens_moveis` so' la; `imovel_*` e `reembolsos` so'
/// na producao (no nt009 o reembolso vai em `Valores.deducao.ajustes`, tpAjusteBC 101-105/199). Prazos de destaque por servico: Ato
/// Conjunto RFB/CGIBS no 4/2026 (fica com o integrador). O emissor manda a CLASSIFICACAO (codigos do contador); o SEFIN calcula.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct IbsCbs {
    pub cind_op: String,
    pub cst: String,
    pub cclass_trib: String,
    pub ind_dest: u8,
    pub fin: u8,
    pub ind_final: Option<u8>,
    pub tp_oper: Option<u8>,
    pub refs_nfse: Vec<String>,
    pub tp_ente_gov: Option<u8>,
    pub destinatario: Option<Pessoa>,
    pub imovel_cib: String,
    pub imovel_endereco: Option<EnderecoSimples>,
    pub imovel_inscricao: String,
    pub reembolsos: Vec<DocReembolso>,
    /// So' leiaute nt009.
    pub imovel: Option<Imovel>,
    /// So' leiaute nt009.
    pub bens_moveis: Vec<BemMovel>,
    pub ccred_pres: String,
    pub cst_regular: String,
    pub cclass_trib_regular: String,
    pub dif_uf: Option<Valor>,
    pub dif_mun: Option<Valor>,
    pub dif_cbs: Option<Valor>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Dps {
    pub prestador: Prestador,
    pub servico: Servico,
    pub valores: Valores,
    /// cLocEmi (IBGE 7).
    pub cmun_emissor: Valor,
    /// nDPS: sem zero a esquerda, ate 15 digitos; NUNCA reaproveitar.
    pub numero: Valor,
    pub serie: Valor,
    /// Data do fato gerador (default: hoje).
    pub competencia: Option<String>,
    pub ambiente: u8,
    /// 1 prestador, 2 tomador, 3 intermediario.
    pub emitente: u8,
    pub tomador: Option<Pessoa>,
    pub intermediario: Option<Pessoa>,
    pub substituicao: Option<Substituicao>,
    pub ibscbs: Option<IbsCbs>,
    pub motivo_emissao_ti: Option<u8>,
    pub chave_rejeitada: String,
    /// dhEmi em ISO 8601 (default: agora, -03:00; sem fuso = Brasilia).
    pub emissao: Option<String>,
    pub ver_aplic: String,
    pub ctribnac_estrito: bool,
    /// "producao" (padrao) ou "nt009" (XSD da Producao Restrita de 20/10/2026).
    pub leiaute: String,
}

impl Default for Dps {
    fn default() -> Self {
        Dps {
            prestador: Prestador::default(),
            servico: Servico::default(),
            valores: Valores::default(),
            cmun_emissor: Valor::default(),
            numero: Valor::default(),
            serie: Valor::from(1u32),
            competencia: None,
            ambiente: AMBIENTE_HOMOLOGACAO,
            emitente: EMITENTE_PRESTADOR,
            tomador: None,
            intermediario: None,
            substituicao: None,
            ibscbs: None,
            motivo_emissao_ti: None,
            chave_rejeitada: String::new(),
            emissao: None,
            ver_aplic: "rustnf".into(),
            ctribnac_estrito: true,
            leiaute: LEIAUTE_PRODUCAO.into(),
        }
    }
}
