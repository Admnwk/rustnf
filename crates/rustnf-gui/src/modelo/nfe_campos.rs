// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Os campos de texto da Nova NF-e: qual e' cada um, a mascara de digitacao e como o texto entra nos dados. A tela so'
//! liga o campo ao texto; aqui mora o que e' normalizado (documento, CEP, dinheiro, NCM...). Sem janela.

use super::mascara::{
    filtrar_decimal, formatar_cep, formatar_data_br, formatar_dinheiro, formatar_documento,
    limpar_documento, so_digitos,
};
use super::nfe_nova::{DadosNfe, ItemNfe};

/// Os campos de texto fixos do assistente.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CampoNova {
    /// CPF ou CNPJ do cliente.
    Documento,
    /// Nome do cliente.
    Nome,
    /// Inscricao estadual do cliente.
    Ie,
    /// E-mail do cliente.
    Email,
    /// CEP.
    Cep,
    /// Rua.
    Logradouro,
    /// Numero.
    Numero,
    /// Complemento.
    Complemento,
    /// Bairro.
    Bairro,
    /// Codigo IBGE do municipio.
    CodMunicipio,
    /// Numero de parcelas.
    Parcelas,
    /// Vencimento da primeira parcela.
    Vencimento,
    /// Documento da transportadora.
    TranspDocumento,
    /// Nome da transportadora.
    TranspNome,
    /// IE da transportadora.
    TranspIe,
    /// Endereco da transportadora.
    TranspEndereco,
    /// Municipio da transportadora.
    TranspMunicipio,
    /// UF da transportadora.
    TranspUf,
    /// Quantos volumes.
    Volumes,
    /// Especie dos volumes.
    Especie,
    /// Peso bruto.
    PesoBruto,
}

impl CampoNova {
    /// Todos, na ordem da tela.
    pub const TODOS: [CampoNova; 21] = [
        Self::Documento,
        Self::Nome,
        Self::Ie,
        Self::Email,
        Self::Cep,
        Self::Logradouro,
        Self::Numero,
        Self::Complemento,
        Self::Bairro,
        Self::CodMunicipio,
        Self::Parcelas,
        Self::Vencimento,
        Self::TranspDocumento,
        Self::TranspNome,
        Self::TranspIe,
        Self::TranspEndereco,
        Self::TranspMunicipio,
        Self::TranspUf,
        Self::Volumes,
        Self::Especie,
        Self::PesoBruto,
    ];

    /// A dica do campo vazio.
    #[must_use]
    pub fn dica(self) -> &'static str {
        match self {
            Self::Documento | Self::TranspDocumento => "CPF ou CNPJ",
            Self::Nome => "Nome ou razão social",
            Self::Ie | Self::TranspIe => "Só' os números",
            Self::Email => "nome@exemplo.com.br",
            Self::Cep => "00000-000",
            Self::Logradouro | Self::TranspEndereco => "Rua, avenida...",
            Self::Numero => "S/N se não tem",
            Self::Complemento => "Opcional",
            Self::Bairro => "Bairro",
            Self::CodMunicipio => "Ex.: 3550308",
            Self::Parcelas => "2",
            Self::Vencimento => "DD/MM/AAAA",
            Self::TranspNome => "Nome da transportadora",
            Self::TranspMunicipio => "Cidade",
            Self::TranspUf => "SP",
            Self::Volumes => "Ex.: 3",
            Self::Especie => "Caixa, fardo...",
            Self::PesoBruto => "Em kg, ex.: 12,5",
        }
    }

    /// O texto do campo nos dados.
    #[must_use]
    pub fn texto(self, d: &DadosNfe) -> String {
        match self {
            Self::Documento => formatar_documento(&d.documento),
            Self::Nome => d.nome.clone(),
            Self::Ie => d.ie.clone(),
            Self::Email => d.email.clone(),
            Self::Cep => formatar_cep(&d.cep),
            Self::Logradouro => d.logradouro.clone(),
            Self::Numero => d.numero.clone(),
            Self::Complemento => d.complemento.clone(),
            Self::Bairro => d.bairro.clone(),
            Self::CodMunicipio => d.cod_municipio.clone(),
            Self::Parcelas => d.parcelas.clone(),
            Self::Vencimento => d.primeiro_vencimento.clone(),
            Self::TranspDocumento => formatar_documento(&d.transp_documento),
            Self::TranspNome => d.transp_nome.clone(),
            Self::TranspIe => d.transp_ie.clone(),
            Self::TranspEndereco => d.transp_endereco.clone(),
            Self::TranspMunicipio => d.transp_municipio.clone(),
            Self::TranspUf => d.transp_uf.clone(),
            Self::Volumes => d.volumes.clone(),
            Self::Especie => d.especie.clone(),
            Self::PesoBruto => d.peso_bruto.clone(),
        }
    }

    /// Poe o texto digitado nos dados e devolve o texto normalizado (a tela o poe de volta no campo se for outro).
    pub fn definir(self, d: &mut DadosNfe, texto: &str) -> String {
        match self {
            Self::Documento => {
                d.documento = limpar_documento(texto);
                formatar_documento(&d.documento)
            }
            Self::TranspDocumento => {
                d.transp_documento = limpar_documento(texto);
                formatar_documento(&d.transp_documento)
            }
            Self::Cep => {
                d.cep = so_digitos(texto, 8);
                formatar_cep(&d.cep)
            }
            Self::CodMunicipio => {
                d.cod_municipio = so_digitos(texto, 7);
                d.cod_municipio.clone()
            }
            Self::Parcelas => {
                d.parcelas = so_digitos(texto, 2);
                d.parcelas.clone()
            }
            Self::Volumes => {
                d.volumes = so_digitos(texto, 6);
                d.volumes.clone()
            }
            Self::Vencimento => {
                d.primeiro_vencimento = formatar_data_br(texto);
                d.primeiro_vencimento.clone()
            }
            Self::PesoBruto => {
                d.peso_bruto = filtrar_decimal(texto);
                d.peso_bruto.clone()
            }
            Self::TranspUf => {
                d.transp_uf = texto
                    .chars()
                    .filter(char::is_ascii_alphabetic)
                    .map(|c| c.to_ascii_uppercase())
                    .take(2)
                    .collect();
                d.transp_uf.clone()
            }
            Self::Ie | Self::TranspIe => {
                let limpo: String = texto
                    .chars()
                    .filter(char::is_ascii_alphanumeric)
                    .map(|c| c.to_ascii_uppercase())
                    .take(14)
                    .collect();
                if self == Self::Ie {
                    d.ie.clone_from(&limpo);
                } else {
                    d.transp_ie.clone_from(&limpo);
                }
                limpo
            }
            Self::Nome => livre(&mut d.nome, texto, 60),
            Self::Email => livre(&mut d.email, texto, 60),
            Self::Logradouro => livre(&mut d.logradouro, texto, 60),
            Self::Numero => livre(&mut d.numero, texto, 60),
            Self::Complemento => livre(&mut d.complemento, texto, 60),
            Self::Bairro => livre(&mut d.bairro, texto, 60),
            Self::TranspNome => livre(&mut d.transp_nome, texto, 60),
            Self::TranspEndereco => livre(&mut d.transp_endereco, texto, 60),
            Self::TranspMunicipio => livre(&mut d.transp_municipio, texto, 60),
            Self::Especie => livre(&mut d.especie, texto, 60),
        }
    }
}

fn livre(destino: &mut String, texto: &str, limite: usize) -> String {
    let t: String = texto
        .chars()
        .filter(|c| !c.is_control())
        .take(limite)
        .collect();
    destino.clone_from(&t);
    t
}

/// O NCM com mascara (`0901.21.00`), no maximo 8 digitos.
#[must_use]
pub fn formatar_ncm(texto: &str) -> String {
    let d = so_digitos(texto, 8);
    let mut s = String::new();
    for (i, c) in d.chars().enumerate() {
        if i == 4 || i == 6 {
            s.push('.');
        }
        s.push(c);
    }
    s
}

/// Os campos de texto de cada produto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CampoItem {
    /// Codigo na loja.
    Codigo,
    /// Descricao.
    Descricao,
    /// Unidade.
    Unidade,
    /// Quantidade.
    Quantidade,
    /// Valor de uma unidade.
    Valor,
    /// Desconto.
    Desconto,
    /// NCM.
    Ncm,
    /// CEST.
    Cest,
    /// Aliquota do ICMS.
    AliquotaIcms,
    /// ICMS ja cobrado antes: base.
    StBase,
    /// ICMS ja cobrado antes: aliquota.
    StAliquota,
    /// ICMS ja cobrado antes: valor.
    StValor,
    /// CFOP escolhido a mao.
    Cfop,
    /// CST ou CSOSN escolhido a mao.
    IcmsCodigo,
    /// CST do PIS e da COFINS escolhido a mao.
    PisCst,
    /// DIFAL: aliquota interna do ICMS da UF do cliente.
    DifalAliquota,
    /// DIFAL: adicional do Fundo de Combate a Pobreza da UF do cliente.
    DifalFcp,
}

impl CampoItem {
    /// Todos, na ordem da tela.
    pub const TODOS: [CampoItem; 17] = [
        Self::Codigo,
        Self::Descricao,
        Self::Unidade,
        Self::Quantidade,
        Self::Valor,
        Self::Desconto,
        Self::Ncm,
        Self::Cest,
        Self::AliquotaIcms,
        Self::StBase,
        Self::StAliquota,
        Self::StValor,
        Self::Cfop,
        Self::IcmsCodigo,
        Self::PisCst,
        Self::DifalAliquota,
        Self::DifalFcp,
    ];

    /// A dica do campo vazio.
    #[must_use]
    pub fn dica(self) -> &'static str {
        match self {
            Self::Codigo => "Código na sua loja",
            Self::Descricao => "O que foi vendido",
            Self::Unidade => "UN",
            Self::Quantidade => "1",
            Self::Valor | Self::Desconto | Self::StBase | Self::StValor => "0,00",
            Self::Ncm => "0000.00.00",
            Self::Cest => "7 dígitos, se tem",
            Self::AliquotaIcms | Self::StAliquota | Self::DifalAliquota => "Ex.: 18",
            Self::DifalFcp => "Ex.: 2 (vazio = não tem)",
            Self::Cfop => "4 dígitos",
            Self::IcmsCodigo => "Ex.: 102 ou 00",
            Self::PisCst => "Ex.: 49",
        }
    }

    /// O texto do campo no item.
    #[must_use]
    pub fn texto(self, i: &ItemNfe) -> String {
        match self {
            Self::Codigo => i.codigo.clone(),
            Self::Descricao => i.descricao.clone(),
            Self::Unidade => i.unidade.clone(),
            Self::Quantidade => i.quantidade.clone(),
            Self::Valor => i.valor.clone(),
            Self::Desconto => i.desconto.clone(),
            Self::Ncm => formatar_ncm(&i.ncm),
            Self::Cest => i.cest.clone(),
            Self::AliquotaIcms => i.aliquota_icms.clone(),
            Self::StBase => i.st_base.clone(),
            Self::StAliquota => i.st_aliquota.clone(),
            Self::StValor => i.st_valor.clone(),
            Self::Cfop => i.cfop.clone().unwrap_or_default(),
            Self::IcmsCodigo => i.icms_codigo.clone().unwrap_or_default(),
            Self::PisCst => i.pis_cst.clone().unwrap_or_default(),
            Self::DifalAliquota => i.difal_aliquota.clone(),
            Self::DifalFcp => i.difal_fcp.clone(),
        }
    }

    /// Poe o texto digitado no item e devolve o texto normalizado.
    pub fn definir(self, i: &mut ItemNfe, texto: &str) -> String {
        let manual = |campo: &mut Option<String>, limite: usize| {
            let d = so_digitos(texto, limite);
            *campo = (!d.is_empty()).then(|| d.clone());
            d
        };
        match self {
            Self::Codigo => livre(&mut i.codigo, texto, 60),
            Self::Descricao => livre(&mut i.descricao, texto, 120),
            Self::Unidade => {
                let u: String = texto
                    .chars()
                    .filter(|c| c.is_ascii_alphanumeric())
                    .map(|c| c.to_ascii_uppercase())
                    .take(6)
                    .collect();
                i.unidade.clone_from(&u);
                u
            }
            Self::Quantidade => {
                i.quantidade = filtrar_decimal(texto);
                i.quantidade.clone()
            }
            Self::Valor => {
                i.valor = formatar_dinheiro(texto);
                i.valor.clone()
            }
            Self::Desconto => {
                i.desconto = formatar_dinheiro(texto);
                i.desconto.clone()
            }
            Self::StBase => {
                i.st_base = formatar_dinheiro(texto);
                i.st_base.clone()
            }
            Self::StValor => {
                i.st_valor = formatar_dinheiro(texto);
                i.st_valor.clone()
            }
            Self::Ncm => {
                i.ncm = so_digitos(texto, 8);
                formatar_ncm(&i.ncm)
            }
            Self::Cest => {
                i.cest = so_digitos(texto, 7);
                i.cest.clone()
            }
            Self::AliquotaIcms => {
                i.aliquota_icms = filtrar_decimal(texto);
                i.aliquota_icms.clone()
            }
            Self::StAliquota => {
                i.st_aliquota = filtrar_decimal(texto);
                i.st_aliquota.clone()
            }
            Self::DifalAliquota => {
                i.difal_aliquota = filtrar_decimal(texto);
                i.difal_aliquota.clone()
            }
            Self::DifalFcp => {
                i.difal_fcp = filtrar_decimal(texto);
                i.difal_fcp.clone()
            }
            Self::Cfop => manual(&mut i.cfop, 4),
            Self::IcmsCodigo => manual(&mut i.icms_codigo, 3),
            Self::PisCst => manual(&mut i.pis_cst, 2),
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn cada_campo_poe_o_texto_normalizado_nos_dados_e_le_de_volta() {
        let mut d = DadosNfe::default();
        assert_eq!(
            CampoNova::Documento.definir(&mut d, "529.982.247-25"),
            "529.982.247-25"
        );
        assert_eq!(d.documento, "52998224725");
        assert_eq!(CampoNova::Cep.definir(&mut d, "01310100"), "01310-100");
        assert_eq!(d.cep, "01310100");
        assert_eq!(
            CampoNova::CodMunicipio.definir(&mut d, "35.50308"),
            "3550308"
        );
        assert_eq!(
            CampoNova::Vencimento.definir(&mut d, "15122026"),
            "15/12/2026"
        );
        assert_eq!(CampoNova::TranspUf.definir(&mut d, "s1p"), "SP");
        assert_eq!(CampoNova::Parcelas.definir(&mut d, "a12b"), "12");
        assert_eq!(CampoNova::Ie.definir(&mut d, "111.222-x"), "111222X");
        assert_eq!(CampoNova::PesoBruto.definir(&mut d, "12,5kg"), "12,5");
        assert_eq!(CampoNova::Nome.definir(&mut d, "Maria\n"), "Maria");
        for c in CampoNova::TODOS {
            // ler o que se acabou de definir da o mesmo texto normalizado
            let t = c.texto(&d);
            let mut outro = d.clone();
            assert_eq!(c.definir(&mut outro, &t), t, "{c:?}");
        }
        // todo campo tem dica
        assert!(CampoNova::TODOS.iter().all(|c| !c.dica().is_empty()));
    }

    #[test]
    fn os_campos_do_item_normalizam_e_o_manual_vazio_volta_a_ser_automatico() {
        let mut i = ItemNfe::default();
        assert_eq!(CampoItem::Ncm.definir(&mut i, "0901.21.00"), "0901.21.00");
        assert_eq!(i.ncm, "09012100");
        assert_eq!(CampoItem::Valor.definir(&mut i, "1234,5"), "1.234,5");
        assert_eq!(CampoItem::Unidade.definir(&mut i, "kg!"), "KG");
        assert_eq!(CampoItem::Quantidade.definir(&mut i, "1,5x"), "1,5");
        // o campo manual vazio e' "nada escolhido": a tabela volta a valer
        assert_eq!(CampoItem::Cfop.definir(&mut i, "5.949"), "5949");
        assert_eq!(i.cfop.as_deref(), Some("5949"));
        CampoItem::Cfop.definir(&mut i, "");
        assert_eq!(i.cfop, None);
        assert_eq!(CampoItem::IcmsCodigo.definir(&mut i, "900"), "900");
        assert_eq!(i.icms_codigo.as_deref(), Some("900"));
        assert_eq!(CampoItem::PisCst.definir(&mut i, "9"), "9");
        for c in CampoItem::TODOS {
            let t = c.texto(&i);
            let mut outro = i.clone();
            assert_eq!(c.definir(&mut outro, &t), t, "{c:?}");
            assert!(!c.dica().is_empty());
        }
    }

    #[test]
    fn o_ncm_tem_mascara() {
        assert_eq!(formatar_ncm("09012100"), "0901.21.00");
        assert_eq!(formatar_ncm("0901"), "0901");
        assert_eq!(formatar_ncm("09012"), "0901.2");
        assert_eq!(formatar_ncm("090121009999"), "0901.21.00");
    }
}
