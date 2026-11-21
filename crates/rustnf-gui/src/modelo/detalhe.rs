// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O detalhe de uma nota: os dados lidos do XML da NFS-e (guardado na pasta ou devolvido pela consulta), ja com o texto
//! da tela. Aqui o documento do tomador aparece inteiro e formatado. Sem janela.

use rustnf::nfse::leitura::{ler_nfse, Nfse};
use rustnf_app::notas::Situacao;

use super::chave;
use super::mascara::{
    centavos_para_dinheiro, dinheiro_para_centavos, formatar_ctribnac, formatar_documento,
    iso_para_data_br, limpar_documento,
};

/// Um valor em dinheiro como a tela mostra; o que nao e' dinheiro legivel fica como veio, e vazio vira `-`.
fn dinheiro(texto: &str) -> String {
    let t = texto.trim();
    if t.is_empty() {
        return "-".into();
    }
    dinheiro_para_centavos(t).map_or_else(|| t.to_string(), centavos_para_dinheiro)
}

fn data(iso: &str) -> String {
    let t = iso.trim();
    t.get(..10).map_or_else(|| t.to_string(), iso_para_data_br)
}

/// A data e a hora do processamento (`DD/MM/AAAA HH:MM`), se o XML as traz.
fn data_e_hora(iso: &str) -> String {
    let t = iso.trim();
    match (t.get(..10), t.get(11..16)) {
        (Some(d), Some(h)) if t.get(10..11) == Some("T") => {
            format!("{} {h}", iso_para_data_br(d))
        }
        _ => data(t),
    }
}

/// O que a tela mostra de uma nota.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetalheDaNota {
    /// A chave de acesso (50 posicoes), sem separador.
    pub chave: String,
    /// O numero da NFS-e.
    pub numero: String,
    /// A situacao da nota na pasta ou na consulta.
    pub situacao: Situacao,
    /// A emissao (processamento), `DD/MM/AAAA HH:MM`.
    pub emissao: String,
    /// A competencia, `DD/MM/AAAA`.
    pub competencia: String,
    /// A DPS, `serie/numero`.
    pub dps: String,
    /// O nome do emitente.
    pub emitente: String,
    /// O documento do emitente, formatado.
    pub emitente_documento: String,
    /// O nome do tomador (vazio se a nota nao identifica o tomador).
    pub tomador: String,
    /// O documento do tomador, inteiro e formatado (vazio se a nota nao o traz).
    pub tomador_documento: String,
    /// O codigo de tributacao nacional, com mascara.
    pub servico_codigo: String,
    /// A descricao oficial do codigo (`xTribNac`).
    pub servico_tributacao: String,
    /// A descricao do servico que o emitente escreveu.
    pub servico_descricao: String,
    /// O valor do servico.
    pub valor_servico: String,
    /// A base de calculo do ISS.
    pub base_calculo: String,
    /// A aliquota do ISS aplicada, em percentual (`5,00`); vazia se o XML nao a traz.
    pub aliquota_iss: String,
    /// O ISS.
    pub iss: String,
    /// O total retido.
    pub retido: String,
    /// O valor liquido.
    pub liquido: String,
    /// O ambiente em que a nota foi gerada, se o XML diz: `1` producao, `2` producao restrita.
    pub ambiente: Option<u8>,
}

impl DetalheDaNota {
    /// O detalhe de uma NFS-e lida, com a situacao que a pasta ou a consulta diz.
    #[must_use]
    pub fn da_nfse(n: &Nfse, situacao: Situacao) -> Self {
        let dps = match (n.serie_dps.trim(), n.numero_dps.trim()) {
            ("", "") => String::new(),
            (s, d) => format!("{s}/{d}"),
        };
        let documento = |d: &str| {
            if limpar_documento(d).is_empty() {
                String::new()
            } else {
                formatar_documento(d)
            }
        };
        Self {
            chave: chave::limpar(&n.chave),
            numero: n.numero.trim().to_string(),
            situacao,
            emissao: data_e_hora(&n.processada_em),
            competencia: data(&n.competencia),
            dps,
            emitente: n.emitente_nome.trim().to_string(),
            emitente_documento: documento(&n.emitente_documento),
            tomador: n.tomador_nome.trim().to_string(),
            tomador_documento: documento(&n.tomador_documento),
            servico_codigo: formatar_ctribnac(&n.ctribnac),
            servico_tributacao: n.descricao_tributacao.trim().to_string(),
            servico_descricao: n.descricao_servico.trim().to_string(),
            valor_servico: dinheiro(&n.valor_servico),
            base_calculo: dinheiro(&n.base_calculo),
            aliquota_iss: n.aliquota_aplicada.trim().replace('.', ","),
            iss: dinheiro(&n.iss),
            retido: dinheiro(&n.total_retido),
            liquido: dinheiro(&n.valor_liquido),
            ambiente: match n.ambiente.trim() {
                "1" => Some(1),
                "2" => Some(2),
                _ => None,
            },
        }
    }

    /// A chave em grupos de 5, para ler e para quebrar a linha.
    #[must_use]
    pub fn chave_em_grupos(&self) -> String {
        chave::em_grupos(&self.chave)
    }

    /// O emitente da nota e' o documento dado (letras e digitos, sem mascara, sem diferenca de caixa).
    #[must_use]
    pub fn e_do_emitente(&self, documento: &str) -> bool {
        let meu = limpar_documento(documento);
        !meu.is_empty() && limpar_documento(&self.emitente_documento) == meu
    }
}

/// O que a leitura do XML guardado deu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LeituraDoXml {
    /// Leu.
    Lida(Box<DetalheDaNota>),
    /// A nota consta do indice, mas o arquivo nao esta na pasta (ou nao pode ser aberto).
    Ausente,
    /// O arquivo esta na pasta, mas nao e' uma NFS-e legivel.
    Ilegivel,
}

impl LeituraDoXml {
    /// Le o XML guardado: `xml` e' o resultado de abrir o arquivo da pasta (`None`: nao foi possivel abri-lo).
    #[must_use]
    pub fn do_xml(xml: Option<&str>, situacao: Situacao) -> Self {
        match xml {
            None => Self::Ausente,
            Some(texto) => ler_nfse(texto).map_or(Self::Ilegivel, |n| {
                Self::Lida(Box::new(DetalheDaNota::da_nfse(&n, situacao)))
            }),
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    const XML: &str = include_str!("../../../rustnf-cli/exemplos/nfse.xml");

    fn nfse() -> Nfse {
        ler_nfse(XML).unwrap()
    }

    #[test]
    fn o_detalhe_traz_os_dados_do_xml_com_o_texto_da_tela() {
        let d = DetalheDaNota::da_nfse(&nfse(), Situacao::Autorizada);
        assert_eq!(d.numero, "25");
        assert_eq!(d.chave.len(), 50);
        assert_eq!(d.chave_em_grupos().split(' ').count(), 10);
        assert_eq!(d.situacao, Situacao::Autorizada);
        assert_eq!(d.competencia, "23/09/2026");
        assert_eq!(d.emissao, "23/09/2026 10:20");
        assert_eq!(d.dps, "900/1559999999");
        assert_eq!(d.tomador, "CLIENTE EXEMPLO SA");
        assert_eq!(d.emitente, "OFICINA DE TESTE LTDA");
        assert_eq!(d.servico_codigo, "01.03.01");
        assert_eq!(d.servico_tributacao, "Processamento de dados");
        assert_eq!(d.servico_descricao, "Descrição do Serviço");
        assert_eq!(
            (
                d.valor_servico.as_str(),
                d.base_calculo.as_str(),
                d.iss.as_str()
            ),
            ("12,45", "12,45", "0,62")
        );
        assert_eq!((d.retido.as_str(), d.liquido.as_str()), ("0,00", "12,45"));
        assert_eq!(d.aliquota_iss, "5,00");
        assert_eq!(d.ambiente, Some(2));
    }

    #[test]
    fn o_documento_do_tomador_aparece_inteiro_e_formatado() {
        let d = DetalheDaNota::da_nfse(&nfse(), Situacao::Autorizada);
        assert_eq!(d.tomador_documento, "33.444.555/0001-72");
        assert_eq!(d.emitente_documento, "11.222.333/0001-81");
        let mut n = nfse();
        n.tomador_documento = "52998224725".into();
        let d = DetalheDaNota::da_nfse(&n, Situacao::Cancelada);
        assert_eq!(d.tomador_documento, "529.982.247-25");
        assert_eq!(d.situacao, Situacao::Cancelada);
        n.tomador_documento.clear();
        n.tomador_nome.clear();
        let d = DetalheDaNota::da_nfse(&n, Situacao::Autorizada);
        assert_eq!(d.tomador_documento, "");
        assert_eq!(d.tomador, "");
    }

    #[test]
    fn valor_ausente_vira_traco_e_valor_legivel_vira_dinheiro() {
        assert_eq!(dinheiro(""), "-");
        assert_eq!(dinheiro("1234.5"), "1.234,50");
        assert_eq!(dinheiro("0.00"), "0,00");
        assert_eq!(dinheiro("abc"), "abc");
    }

    #[test]
    fn a_nota_e_do_emitente_so_quando_o_documento_confere() {
        let d = DetalheDaNota::da_nfse(&nfse(), Situacao::Autorizada);
        assert!(d.e_do_emitente("11222333000181"));
        assert!(d.e_do_emitente("11.222.333/0001-81"));
        assert!(!d.e_do_emitente("52998224725"));
        assert!(!d.e_do_emitente(""));
    }

    #[test]
    fn xml_ausente_ou_ilegivel_nao_derruba() {
        assert_eq!(
            LeituraDoXml::do_xml(None, Situacao::Autorizada),
            LeituraDoXml::Ausente
        );
        assert_eq!(
            LeituraDoXml::do_xml(Some("isto nao e' xml"), Situacao::Autorizada),
            LeituraDoXml::Ilegivel
        );
        assert_eq!(
            LeituraDoXml::do_xml(Some("<outra/>"), Situacao::Autorizada),
            LeituraDoXml::Ilegivel
        );
        assert!(matches!(
            LeituraDoXml::do_xml(Some(XML), Situacao::Autorizada),
            LeituraDoXml::Lida(_)
        ));
    }

    #[test]
    fn data_e_hora_aceitam_so_a_data() {
        assert_eq!(data_e_hora("2026-09-23T10:15:00-03:00"), "23/09/2026 10:15");
        assert_eq!(data_e_hora("2026-09-23"), "23/09/2026");
        assert_eq!(data_e_hora(""), "");
    }
}
