// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O estado do formulario da Nova nota: o TEXTO digitado em cada campo e as escolhas, com a mascara de cada campo.
//! Nao ha regra fiscal aqui: o que o texto significa para a DPS fica em [`super::pedido`], e a conferencia, em
//! [`super::conferencia`].

use rustnf_app::config::Emitente;
use serde::{Deserialize, Serialize};

use super::mascara::{
    filtrar_decimal, formatar_cep, formatar_ctribnac, formatar_data_br, formatar_dinheiro,
    formatar_documento, formatar_nbs, formatar_telefone, so_digitos,
};

/// Tributacao do ISS que a tela oferece (`tribISSQN`): tributavel no municipio ou nao incidencia. Imunidade pede o
/// tipo da imunidade e exportacao e' comercio exterior: ficam para a linha de comando.
pub const OPCOES_TRIBUTACAO: [(u8, &str); 2] =
    [(1, "Tributável no município"), (4, "Não incidência")];

/// Retencao do ISS que a tela oferece (`tpRetISSQN`); a retencao pelo intermediario fica para a linha de comando.
pub const OPCOES_RETENCAO: [(u8, &str); 2] =
    [(1, "ISS não retido"), (2, "ISS retido pelo tomador")];

/// `indFinal`: operacao com consumidor final.
pub const OPCOES_CONSUMIDOR_FINAL: [(Option<u8>, &str); 3] = [
    (None, "Não informar"),
    (Some(1), "Consumidor final"),
    (Some(0), "Não é consumidor final"),
];

/// Uma parte da pagina.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SecaoNota {
    /// Quem recebe o servico.
    Tomador,
    /// O que foi prestado.
    Servico,
    /// Quanto.
    Valores,
    /// ISS e tributos aproximados.
    Iss,
    /// Retencoes federais e PIS/COFINS.
    Federais,
    /// IBS e CBS.
    IbsCbs,
}

impl SecaoNota {
    /// As secoes, na ordem da pagina.
    pub const TODAS: [SecaoNota; 6] = [
        SecaoNota::Tomador,
        SecaoNota::Servico,
        SecaoNota::Valores,
        SecaoNota::Iss,
        SecaoNota::Federais,
        SecaoNota::IbsCbs,
    ];

    /// Titulo da secao.
    #[must_use]
    pub fn titulo(self) -> &'static str {
        match self {
            Self::Tomador => "Tomador",
            Self::Servico => "Serviço",
            Self::Valores => "Valores",
            Self::Iss => "ISS",
            Self::Federais => "Tributos federais",
            Self::IbsCbs => "IBS e CBS",
        }
    }
}

/// Um campo da Nova nota: o que identifica de onde veio cada pendencia.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CampoNota {
    /// CNPJ ou CPF do tomador.
    TomadorDocumento,
    /// Nome ou razao social do tomador.
    TomadorNome,
    /// Inscricao municipal do tomador.
    TomadorIm,
    /// E-mail do tomador.
    TomadorEmail,
    /// Telefone do tomador.
    TomadorFone,
    /// CEP do tomador.
    EnderecoCep,
    /// Logradouro do tomador.
    EnderecoLogradouro,
    /// Numero do endereco do tomador.
    EnderecoNumero,
    /// Complemento do endereco do tomador.
    EnderecoComplemento,
    /// Bairro do tomador.
    EnderecoBairro,
    /// Municipio do tomador (codigo IBGE).
    EnderecoMunicipio,
    /// Codigo de tributacao nacional.
    Ctribnac,
    /// Descricao do servico.
    Descricao,
    /// Municipio da prestacao (codigo IBGE).
    MunicipioPrestacao,
    /// NBS.
    Nbs,
    /// Codigo de tributacao municipal.
    CtribMun,
    /// Codigo interno do contribuinte.
    CodInterno,
    /// Valor do servico.
    ValorServico,
    /// Desconto incondicionado.
    DescontoIncondicionado,
    /// Desconto condicionado.
    DescontoCondicionado,
    /// Competencia.
    Competencia,
    /// Tributacao do ISS.
    Tributacao,
    /// Retencao do ISS.
    Retencao,
    /// Aliquota do ISS.
    AliquotaIss,
    /// Percentual de tributos do Simples Nacional.
    PercentualSn,
    /// Percentual aproximado dos tributos federais.
    PercentualFederal,
    /// Percentual aproximado dos tributos estaduais.
    PercentualEstadual,
    /// Percentual aproximado dos tributos municipais.
    PercentualMunicipal,
    /// Retencao do IRRF.
    Irrf,
    /// Retencao da CSLL.
    Csll,
    /// Retencao da contribuicao previdenciaria.
    Previdencia,
    /// CST do PIS/COFINS.
    PisCst,
    /// Base do PIS/COFINS.
    PisBase,
    /// Aliquota do PIS.
    PisAliquotaPis,
    /// Aliquota da COFINS.
    PisAliquotaCofins,
    /// Tipo de retencao do PIS/COFINS.
    PisTipoRetencao,
    /// `cIndOp`.
    CindOp,
    /// `cClassTrib` (o CST sai dele).
    ClassTrib,
    /// `indFinal`.
    IndFinal,
}

impl CampoNota {
    /// Todos os campos, na ordem da pagina.
    pub const TODOS: [CampoNota; 39] = [
        Self::TomadorDocumento,
        Self::TomadorNome,
        Self::TomadorIm,
        Self::TomadorEmail,
        Self::TomadorFone,
        Self::EnderecoCep,
        Self::EnderecoLogradouro,
        Self::EnderecoNumero,
        Self::EnderecoComplemento,
        Self::EnderecoBairro,
        Self::EnderecoMunicipio,
        Self::Ctribnac,
        Self::Descricao,
        Self::MunicipioPrestacao,
        Self::Nbs,
        Self::CtribMun,
        Self::CodInterno,
        Self::ValorServico,
        Self::DescontoIncondicionado,
        Self::DescontoCondicionado,
        Self::Competencia,
        Self::Tributacao,
        Self::Retencao,
        Self::AliquotaIss,
        Self::PercentualSn,
        Self::PercentualFederal,
        Self::PercentualEstadual,
        Self::PercentualMunicipal,
        Self::Irrf,
        Self::Csll,
        Self::Previdencia,
        Self::PisCst,
        Self::PisBase,
        Self::PisAliquotaPis,
        Self::PisAliquotaCofins,
        Self::PisTipoRetencao,
        Self::CindOp,
        Self::ClassTrib,
        Self::IndFinal,
    ];

    /// A secao em que o campo aparece.
    #[must_use]
    pub fn secao(self) -> SecaoNota {
        match self {
            Self::TomadorDocumento
            | Self::TomadorNome
            | Self::TomadorIm
            | Self::TomadorEmail
            | Self::TomadorFone
            | Self::EnderecoCep
            | Self::EnderecoLogradouro
            | Self::EnderecoNumero
            | Self::EnderecoComplemento
            | Self::EnderecoBairro
            | Self::EnderecoMunicipio => SecaoNota::Tomador,
            Self::Ctribnac
            | Self::Descricao
            | Self::MunicipioPrestacao
            | Self::Nbs
            | Self::CtribMun
            | Self::CodInterno => SecaoNota::Servico,
            Self::ValorServico
            | Self::DescontoIncondicionado
            | Self::DescontoCondicionado
            | Self::Competencia => SecaoNota::Valores,
            Self::Tributacao
            | Self::Retencao
            | Self::AliquotaIss
            | Self::PercentualSn
            | Self::PercentualFederal
            | Self::PercentualEstadual
            | Self::PercentualMunicipal => SecaoNota::Iss,
            Self::Irrf
            | Self::Csll
            | Self::Previdencia
            | Self::PisCst
            | Self::PisBase
            | Self::PisAliquotaPis
            | Self::PisAliquotaCofins
            | Self::PisTipoRetencao => SecaoNota::Federais,
            Self::CindOp | Self::ClassTrib | Self::IndFinal => SecaoNota::IbsCbs,
        }
    }

    /// Nome curto do campo, para as pendencias.
    #[must_use]
    pub fn rotulo(self) -> &'static str {
        match self {
            Self::TomadorDocumento => "CNPJ ou CPF do tomador",
            Self::TomadorNome => "Nome do tomador",
            Self::TomadorIm => "Inscrição municipal do tomador",
            Self::TomadorEmail => "E-mail do tomador",
            Self::TomadorFone => "Telefone do tomador",
            Self::EnderecoCep => "CEP",
            Self::EnderecoLogradouro => "Logradouro",
            Self::EnderecoNumero => "Número do endereço",
            Self::EnderecoComplemento => "Complemento",
            Self::EnderecoBairro => "Bairro",
            Self::EnderecoMunicipio => "Município do tomador",
            Self::Ctribnac => "Código de tributação nacional",
            Self::Descricao => "Descrição do serviço",
            Self::MunicipioPrestacao => "Município da prestação",
            Self::Nbs => "NBS",
            Self::CtribMun => "Código de tributação municipal",
            Self::CodInterno => "Código interno",
            Self::ValorServico => "Valor do serviço",
            Self::DescontoIncondicionado => "Desconto incondicionado",
            Self::DescontoCondicionado => "Desconto condicionado",
            Self::Competencia => "Competência",
            Self::Tributacao => "Tributação do ISS",
            Self::Retencao => "Retenção do ISS",
            Self::AliquotaIss => "Alíquota do ISS",
            Self::PercentualSn => "Tributos do Simples Nacional",
            Self::PercentualFederal => "Tributos federais (aproximado)",
            Self::PercentualEstadual => "Tributos estaduais (aproximado)",
            Self::PercentualMunicipal => "Tributos municipais (aproximado)",
            Self::Irrf => "IRRF retido",
            Self::Csll => "CSLL retida",
            Self::Previdencia => "Contribuição previdenciária retida",
            Self::PisCst => "CST do PIS/COFINS",
            Self::PisBase => "Base do PIS/COFINS",
            Self::PisAliquotaPis => "Alíquota do PIS",
            Self::PisAliquotaCofins => "Alíquota da COFINS",
            Self::PisTipoRetencao => "Tipo de retenção do PIS/COFINS",
            Self::CindOp => "Indicador da operação (cIndOp)",
            Self::ClassTrib => "Classificação tributária (cClassTrib)",
            Self::IndFinal => "Consumidor final",
        }
    }
}

/// O que a pessoa preencheu. Tudo em texto, como se ve na tela (com a mascara); o pedido sai em
/// [`super::pedido`]. E' tambem o que o rascunho guarda.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DadosNota {
    /// A nota nao identifica o tomador.
    pub sem_tomador: bool,
    /// CNPJ ou CPF do tomador.
    pub tomador_documento: String,
    /// Nome ou razao social do tomador.
    pub tomador_nome: String,
    /// Inscricao municipal do tomador.
    pub tomador_im: String,
    /// E-mail do tomador.
    pub tomador_email: String,
    /// Telefone do tomador.
    pub tomador_fone: String,
    /// CEP do tomador.
    pub end_cep: String,
    /// Logradouro do tomador.
    pub end_logradouro: String,
    /// Numero do endereco.
    pub end_numero: String,
    /// Complemento do endereco.
    pub end_complemento: String,
    /// Bairro do tomador.
    pub end_bairro: String,
    /// Municipio do tomador (IBGE).
    pub end_municipio: String,
    /// Codigo de tributacao nacional (`14.01.01`).
    pub ctribnac: String,
    /// Descricao do servico.
    pub descricao: String,
    /// Municipio da prestacao (IBGE); vazio = o do emitente.
    pub municipio_prestacao: String,
    /// NBS (`1.2603.00.00`).
    pub nbs: String,
    /// Codigo de tributacao municipal.
    pub ctribmun: String,
    /// Codigo interno do contribuinte.
    pub cod_interno: String,
    /// Valor do servico.
    pub valor_servico: String,
    /// Desconto incondicionado.
    pub desc_incondicionado: String,
    /// Desconto condicionado.
    pub desc_condicionado: String,
    /// Competencia, `DD/MM/AAAA`.
    pub competencia: String,
    /// `tribISSQN`.
    pub tributacao: u8,
    /// `tpRetISSQN`.
    pub retencao: u8,
    /// Aliquota do ISS.
    pub aliquota_iss: String,
    /// Percentual de tributos do Simples Nacional.
    pub percentual_sn: String,
    /// Percentual aproximado de tributos federais.
    pub percentual_federal: String,
    /// Percentual aproximado de tributos estaduais.
    pub percentual_estadual: String,
    /// Percentual aproximado de tributos municipais.
    pub percentual_municipal: String,
    /// IRRF retido.
    pub irrf: String,
    /// CSLL retida.
    pub csll: String,
    /// Contribuicao previdenciaria retida.
    pub previdencia: String,
    /// CST do PIS/COFINS.
    pub pis_cst: String,
    /// Base do PIS/COFINS.
    pub pis_base: String,
    /// Aliquota do PIS.
    pub pis_aliquota_pis: String,
    /// Aliquota da COFINS.
    pub pis_aliquota_cofins: String,
    /// Tipo de retencao do PIS/COFINS (0 a 9).
    pub pis_tipo_retencao: String,
    /// A nota leva o grupo IBS/CBS.
    pub ibscbs_ativo: bool,
    /// `cIndOp`.
    pub cind_op: String,
    /// `cClassTrib`.
    pub cclass_trib: String,
    /// `indFinal`.
    pub ind_final: Option<u8>,
}

impl Default for DadosNota {
    fn default() -> Self {
        Self {
            sem_tomador: false,
            tomador_documento: String::new(),
            tomador_nome: String::new(),
            tomador_im: String::new(),
            tomador_email: String::new(),
            tomador_fone: String::new(),
            end_cep: String::new(),
            end_logradouro: String::new(),
            end_numero: String::new(),
            end_complemento: String::new(),
            end_bairro: String::new(),
            end_municipio: String::new(),
            ctribnac: String::new(),
            descricao: String::new(),
            municipio_prestacao: String::new(),
            nbs: String::new(),
            ctribmun: String::new(),
            cod_interno: String::new(),
            valor_servico: String::new(),
            desc_incondicionado: String::new(),
            desc_condicionado: String::new(),
            competencia: String::new(),
            tributacao: 1,
            retencao: 1,
            aliquota_iss: String::new(),
            percentual_sn: String::new(),
            percentual_federal: String::new(),
            percentual_estadual: String::new(),
            percentual_municipal: String::new(),
            irrf: String::new(),
            csll: String::new(),
            previdencia: String::new(),
            pis_cst: String::new(),
            pis_base: String::new(),
            pis_aliquota_pis: String::new(),
            pis_aliquota_cofins: String::new(),
            pis_tipo_retencao: String::new(),
            ibscbs_ativo: true,
            cind_op: String::new(),
            cclass_trib: String::new(),
            ind_final: None,
        }
    }
}

fn cortar(texto: &str, limite: usize) -> String {
    texto.chars().take(limite).collect()
}

impl DadosNota {
    /// Formulario vazio, com a competencia de hoje (`DD/MM/AAAA`).
    #[must_use]
    pub fn novo(hoje: &str) -> Self {
        Self {
            competencia: hoje.to_string(),
            ..Self::default()
        }
    }

    /// O texto atual de um campo.
    #[must_use]
    pub fn texto(&self, campo: CampoNota) -> String {
        match campo {
            CampoNota::TomadorDocumento => self.tomador_documento.clone(),
            CampoNota::TomadorNome => self.tomador_nome.clone(),
            CampoNota::TomadorIm => self.tomador_im.clone(),
            CampoNota::TomadorEmail => self.tomador_email.clone(),
            CampoNota::TomadorFone => self.tomador_fone.clone(),
            CampoNota::EnderecoCep => self.end_cep.clone(),
            CampoNota::EnderecoLogradouro => self.end_logradouro.clone(),
            CampoNota::EnderecoNumero => self.end_numero.clone(),
            CampoNota::EnderecoComplemento => self.end_complemento.clone(),
            CampoNota::EnderecoBairro => self.end_bairro.clone(),
            CampoNota::EnderecoMunicipio => self.end_municipio.clone(),
            CampoNota::Ctribnac => self.ctribnac.clone(),
            CampoNota::Descricao => self.descricao.clone(),
            CampoNota::MunicipioPrestacao => self.municipio_prestacao.clone(),
            CampoNota::Nbs => self.nbs.clone(),
            CampoNota::CtribMun => self.ctribmun.clone(),
            CampoNota::CodInterno => self.cod_interno.clone(),
            CampoNota::ValorServico => self.valor_servico.clone(),
            CampoNota::DescontoIncondicionado => self.desc_incondicionado.clone(),
            CampoNota::DescontoCondicionado => self.desc_condicionado.clone(),
            CampoNota::Competencia => self.competencia.clone(),
            CampoNota::AliquotaIss => self.aliquota_iss.clone(),
            CampoNota::PercentualSn => self.percentual_sn.clone(),
            CampoNota::PercentualFederal => self.percentual_federal.clone(),
            CampoNota::PercentualEstadual => self.percentual_estadual.clone(),
            CampoNota::PercentualMunicipal => self.percentual_municipal.clone(),
            CampoNota::Irrf => self.irrf.clone(),
            CampoNota::Csll => self.csll.clone(),
            CampoNota::Previdencia => self.previdencia.clone(),
            CampoNota::PisCst => self.pis_cst.clone(),
            CampoNota::PisBase => self.pis_base.clone(),
            CampoNota::PisAliquotaPis => self.pis_aliquota_pis.clone(),
            CampoNota::PisAliquotaCofins => self.pis_aliquota_cofins.clone(),
            CampoNota::PisTipoRetencao => self.pis_tipo_retencao.clone(),
            CampoNota::CindOp => self.cind_op.clone(),
            CampoNota::ClassTrib => self.cclass_trib.clone(),
            CampoNota::Tributacao | CampoNota::Retencao | CampoNota::IndFinal => String::new(),
        }
    }

    /// O texto como a tela deve mostra-lo depois de digitado (a mascara de cada campo).
    #[must_use]
    pub fn normalizar(campo: CampoNota, texto: &str) -> String {
        match campo {
            CampoNota::TomadorDocumento => formatar_documento(texto),
            CampoNota::TomadorNome => cortar(texto, 300),
            CampoNota::TomadorIm => cortar(texto, 15),
            CampoNota::TomadorEmail => cortar(texto, 80),
            CampoNota::TomadorFone => formatar_telefone(texto),
            CampoNota::EnderecoCep => formatar_cep(texto),
            CampoNota::EnderecoLogradouro => cortar(texto, 255),
            CampoNota::EnderecoNumero | CampoNota::EnderecoBairro => cortar(texto, 60),
            CampoNota::EnderecoComplemento => cortar(texto, 156),
            CampoNota::EnderecoMunicipio | CampoNota::MunicipioPrestacao => so_digitos(texto, 7),
            CampoNota::Ctribnac => formatar_ctribnac(texto),
            CampoNota::Descricao => cortar(texto, 2000),
            CampoNota::Nbs => formatar_nbs(texto),
            CampoNota::CtribMun => so_digitos(texto, 3),
            CampoNota::CodInterno => cortar(texto, 20),
            CampoNota::ValorServico
            | CampoNota::DescontoIncondicionado
            | CampoNota::DescontoCondicionado
            | CampoNota::Irrf
            | CampoNota::Csll
            | CampoNota::Previdencia
            | CampoNota::PisBase => formatar_dinheiro(texto),
            CampoNota::Competencia => formatar_data_br(texto),
            CampoNota::AliquotaIss
            | CampoNota::PercentualSn
            | CampoNota::PercentualFederal
            | CampoNota::PercentualEstadual
            | CampoNota::PercentualMunicipal
            | CampoNota::PisAliquotaPis
            | CampoNota::PisAliquotaCofins => filtrar_decimal(texto),
            CampoNota::PisCst => so_digitos(texto, 2),
            CampoNota::PisTipoRetencao => so_digitos(texto, 1),
            CampoNota::CindOp | CampoNota::ClassTrib => so_digitos(texto, 6),
            CampoNota::Tributacao | CampoNota::Retencao | CampoNota::IndFinal => texto.to_string(),
        }
    }

    /// Guarda o texto digitado de um campo, ja com a mascara, e o devolve (a tela troca o que mostra se for diferente).
    pub fn definir(&mut self, campo: CampoNota, texto: &str) -> String {
        let normal = Self::normalizar(campo, texto);
        let destino = match campo {
            CampoNota::TomadorDocumento => &mut self.tomador_documento,
            CampoNota::TomadorNome => &mut self.tomador_nome,
            CampoNota::TomadorIm => &mut self.tomador_im,
            CampoNota::TomadorEmail => &mut self.tomador_email,
            CampoNota::TomadorFone => &mut self.tomador_fone,
            CampoNota::EnderecoCep => &mut self.end_cep,
            CampoNota::EnderecoLogradouro => &mut self.end_logradouro,
            CampoNota::EnderecoNumero => &mut self.end_numero,
            CampoNota::EnderecoComplemento => &mut self.end_complemento,
            CampoNota::EnderecoBairro => &mut self.end_bairro,
            CampoNota::EnderecoMunicipio => &mut self.end_municipio,
            CampoNota::Ctribnac => &mut self.ctribnac,
            CampoNota::Descricao => &mut self.descricao,
            CampoNota::MunicipioPrestacao => &mut self.municipio_prestacao,
            CampoNota::Nbs => &mut self.nbs,
            CampoNota::CtribMun => &mut self.ctribmun,
            CampoNota::CodInterno => &mut self.cod_interno,
            CampoNota::ValorServico => &mut self.valor_servico,
            CampoNota::DescontoIncondicionado => &mut self.desc_incondicionado,
            CampoNota::DescontoCondicionado => &mut self.desc_condicionado,
            CampoNota::Competencia => &mut self.competencia,
            CampoNota::AliquotaIss => &mut self.aliquota_iss,
            CampoNota::PercentualSn => &mut self.percentual_sn,
            CampoNota::PercentualFederal => &mut self.percentual_federal,
            CampoNota::PercentualEstadual => &mut self.percentual_estadual,
            CampoNota::PercentualMunicipal => &mut self.percentual_municipal,
            CampoNota::Irrf => &mut self.irrf,
            CampoNota::Csll => &mut self.csll,
            CampoNota::Previdencia => &mut self.previdencia,
            CampoNota::PisCst => &mut self.pis_cst,
            CampoNota::PisBase => &mut self.pis_base,
            CampoNota::PisAliquotaPis => &mut self.pis_aliquota_pis,
            CampoNota::PisAliquotaCofins => &mut self.pis_aliquota_cofins,
            CampoNota::PisTipoRetencao => &mut self.pis_tipo_retencao,
            CampoNota::CindOp => &mut self.cind_op,
            CampoNota::ClassTrib => &mut self.cclass_trib,
            CampoNota::Tributacao | CampoNota::Retencao | CampoNota::IndFinal => {
                return normal;
            }
        };
        destino.clone_from(&normal);
        normal
    }

    /// Os dados do tomador, de documento a municipio, nao estao todos vazios.
    #[must_use]
    pub fn tem_endereco(&self) -> bool {
        [
            &self.end_cep,
            &self.end_logradouro,
            &self.end_numero,
            &self.end_complemento,
            &self.end_bairro,
            &self.end_municipio,
        ]
        .iter()
        .any(|t| !t.trim().is_empty())
    }

    /// Nada foi digitado nem escolhido alem do que ja vem de fabrica (so' a competencia, se for a de `hoje`). A
    /// chave do grupo IBS/CBS nao conta: ela nasce conforme o regime do emitente e, sozinha, nao e' dado da nota.
    #[must_use]
    pub fn vazio(&self, hoje: &str) -> bool {
        let de_fabrica = Self {
            ibscbs_ativo: self.ibscbs_ativo,
            ..Self::novo(hoje)
        };
        *self == de_fabrica
    }

    /// O documento do tomador, so' com letras e digitos.
    #[must_use]
    pub fn documento_do_tomador(&self) -> String {
        self.tomador_documento
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .collect()
    }

    /// Os 6 digitos do codigo de tributacao nacional (vazio se faltar).
    #[must_use]
    pub fn ctribnac_digitos(&self) -> String {
        so_digitos(&self.ctribnac, 6)
    }

    /// Os 9 digitos do NBS.
    #[must_use]
    pub fn nbs_digitos(&self) -> String {
        so_digitos(&self.nbs, 9)
    }

    /// O CST do IBS/CBS: os 3 primeiros digitos do `cClassTrib`.
    #[must_use]
    pub fn cst(&self) -> String {
        self.cclass_trib.chars().take(3).collect()
    }
}

/// O que o regime do emitente decide sobre a tela.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Regime {
    /// 1 nao optante, 2 MEI, 3 ME/EPP.
    pub op_simples: u8,
    /// 0 = nenhum.
    pub regime_especial: u8,
}

impl Regime {
    /// O regime do `[emitente]`.
    #[must_use]
    pub fn de(emitente: &Emitente) -> Self {
        Self {
            op_simples: emitente.op_simples,
            regime_especial: emitente.regime_especial,
        }
    }

    /// A nota nasce com o grupo IBS/CBS ligado: so' para quem nao e' optante do Simples Nacional. As aliquotas de
    /// 2026 nao se aplicam ao optante (LC 214/2025, art. 348, III, como a previa da biblioteca diz), entao a nota
    /// dele nasce sem o grupo. E' so' o ponto de partida: a chave da tela liga e desliga nos dois casos.
    #[must_use]
    pub fn informa_ibscbs_por_padrao(self) -> bool {
        self.op_simples == 1
    }

    /// A tela pede o percentual de tributos do Simples Nacional (so' ME/EPP).
    #[must_use]
    pub fn pede_percentual_sn(self) -> bool {
        self.op_simples == 3
    }

    /// A tela pede o total aproximado de tributos por esfera (so' quem nao e' do Simples).
    #[must_use]
    pub fn pede_totais_aproximados(self) -> bool {
        self.op_simples == 1
    }

    /// A tela mostra a aliquota do ISS: a biblioteca recusa aliquota de MEI, de regime especial e de servico sem
    /// incidencia (E0600, E0604 e E0602 do Anexo I).
    #[must_use]
    pub fn mostra_aliquota_iss(self, tributacao: u8) -> bool {
        tributacao == 1 && self.op_simples != 2 && self.regime_especial == 0
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn d() -> DadosNota {
        DadosNota::novo("21/11/2026")
    }

    #[test]
    fn cada_campo_aplica_a_mascara_e_guarda_no_lugar_certo() {
        let casos: [(CampoNota, &str, &str); 27] = [
            (CampoNota::TomadorDocumento, "52998224725", "529.982.247-25"),
            (
                CampoNota::TomadorDocumento,
                "11222333000199",
                "11.222.333/0001-99",
            ),
            (CampoNota::TomadorFone, "11988887777", "(11) 98888-7777"),
            (CampoNota::EnderecoCep, "01310100", "01310-100"),
            (CampoNota::EnderecoMunicipio, "35503089999", "3550308"),
            (CampoNota::MunicipioPrestacao, "4106902x", "4106902"),
            (CampoNota::Ctribnac, "140101", "14.01.01"),
            (CampoNota::Nbs, "126030000", "1.2603.00.00"),
            (CampoNota::CtribMun, "0012", "001"),
            (CampoNota::ValorServico, "1234,5", "1.234,5"),
            (CampoNota::DescontoIncondicionado, "10", "10"),
            (CampoNota::DescontoCondicionado, "0,5", "0,5"),
            (CampoNota::Competencia, "21112026", "21/11/2026"),
            (CampoNota::AliquotaIss, "2,5x", "2,5"),
            (CampoNota::PercentualSn, "4,50", "4,50"),
            (CampoNota::PercentualFederal, "13,45", "13,45"),
            (CampoNota::PercentualEstadual, "1", "1"),
            (CampoNota::PercentualMunicipal, "a2", "2"),
            (CampoNota::Irrf, "1500", "1.500"),
            (CampoNota::Csll, "10,00", "10,00"),
            (CampoNota::Previdencia, "20", "20"),
            (CampoNota::PisCst, "012", "01"),
            (CampoNota::PisBase, "100", "100"),
            (CampoNota::PisAliquotaPis, "0,65", "0,65"),
            (CampoNota::PisAliquotaCofins, "3", "3"),
            (CampoNota::PisTipoRetencao, "12", "1"),
            (CampoNota::ClassTrib, "000001x", "000001"),
        ];
        for (campo, digitado, esperado) in casos {
            let mut dados = d();
            let devolvido = dados.definir(campo, digitado);
            assert_eq!(devolvido, esperado, "{campo:?}");
            assert_eq!(dados.texto(campo), esperado, "{campo:?}");
            // so' o campo tocado mudou
            let mut outro = d();
            outro.definir(campo, esperado);
            assert_eq!(dados, outro, "{campo:?}");
        }
    }

    #[test]
    fn campos_de_texto_livre_cortam_no_limite_da_dps() {
        let mut dados = d();
        let longo = "a".repeat(3000);
        assert_eq!(dados.definir(CampoNota::Descricao, &longo).len(), 2000);
        assert_eq!(dados.definir(CampoNota::TomadorNome, &longo).len(), 300);
        assert_eq!(
            dados.definir(CampoNota::EnderecoLogradouro, &longo).len(),
            255
        );
        assert_eq!(
            dados.definir(CampoNota::EnderecoComplemento, &longo).len(),
            156
        );
        assert_eq!(dados.definir(CampoNota::TomadorEmail, &longo).len(), 80);
        assert_eq!(dados.definir(CampoNota::TomadorIm, &longo).len(), 15);
        assert_eq!(dados.definir(CampoNota::CodInterno, &longo).len(), 20);
        assert_eq!(dados.definir(CampoNota::EnderecoBairro, &longo).len(), 60);
        assert_eq!(dados.definir(CampoNota::EnderecoNumero, &longo).len(), 60);
    }

    #[test]
    fn todo_campo_tem_secao_rotulo_e_aparece_uma_vez() {
        let mut vistos = std::collections::BTreeSet::new();
        for c in CampoNota::TODOS {
            assert!(vistos.insert(c), "repetido: {c:?}");
            assert!(!c.rotulo().is_empty());
            assert!(SecaoNota::TODAS.contains(&c.secao()));
        }
        // a ordem da lista segue a ordem das secoes
        let secoes: Vec<_> = CampoNota::TODOS.iter().map(|c| c.secao()).collect();
        let mut ordenadas = secoes.clone();
        ordenadas.sort();
        assert_eq!(secoes, ordenadas);
    }

    #[test]
    fn formulario_novo_so_tem_a_competencia_e_os_padroes() {
        let dados = d();
        assert!(dados.vazio("21/11/2026"));
        assert!(!dados.vazio("22/11/2026"));
        assert_eq!(dados.tributacao, 1);
        assert_eq!(dados.retencao, 1);
        assert!(dados.ibscbs_ativo);
        let mut mexido = d();
        mexido.definir(CampoNota::Descricao, "x");
        assert!(!mexido.vazio("21/11/2026"));
        // so' a chave do grupo IBS/CBS (que nasce conforme o regime) nao e' dado digitado
        let mut so_a_chave = DadosNota::novo("21/11/2026");
        so_a_chave.ibscbs_ativo = false;
        assert!(so_a_chave.vazio("21/11/2026"));
    }

    #[test]
    fn cst_sai_do_cclasstrib_e_o_endereco_so_conta_se_tiver_algo() {
        let mut dados = d();
        assert_eq!(dados.cst(), "");
        dados.definir(CampoNota::ClassTrib, "200003");
        assert_eq!(dados.cst(), "200");
        assert!(!dados.tem_endereco());
        dados.definir(CampoNota::EnderecoBairro, " ");
        assert!(!dados.tem_endereco());
        dados.definir(CampoNota::EnderecoBairro, "Centro");
        assert!(dados.tem_endereco());
    }

    #[test]
    fn o_regime_decide_que_campos_de_tributo_aparecem() {
        let e = |op, esp| Regime {
            op_simples: op,
            regime_especial: esp,
        };
        assert!(e(3, 0).pede_percentual_sn());
        assert!(!e(1, 0).pede_percentual_sn());
        assert!(e(1, 0).pede_totais_aproximados());
        assert!(!e(2, 0).pede_totais_aproximados());
        assert!(!e(3, 0).pede_totais_aproximados());
        assert!(e(1, 0).mostra_aliquota_iss(1));
        assert!(e(3, 0).mostra_aliquota_iss(1));
        assert!(!e(2, 0).mostra_aliquota_iss(1));
        assert!(!e(1, 3).mostra_aliquota_iss(1));
        assert!(!e(1, 0).mostra_aliquota_iss(4));
    }

    #[test]
    fn os_dados_voltam_iguais_do_json_e_campo_novo_nao_quebra_o_antigo() {
        let mut dados = d();
        dados.definir(CampoNota::Descricao, "Revisão");
        dados.ind_final = Some(1);
        let json = serde_json::to_string(&dados).unwrap();
        assert_eq!(serde_json::from_str::<DadosNota>(&json).unwrap(), dados);
        let parcial: DadosNota = serde_json::from_str(r#"{"descricao":"x"}"#).unwrap();
        assert_eq!(parcial.descricao, "x");
        assert_eq!(parcial.tributacao, 1);
    }
}
