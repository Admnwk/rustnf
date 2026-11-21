// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! "Tomador ja usado": ao digitar o CPF/CNPJ de quem ja recebeu nota, a Nova nota completa o nome, o contato e o
//! endereco a partir da nota mais recente desse tomador na pasta de notas. So' preenche campo vazio: o que a pessoa
//! digitou fica.

use rustnf::chave_dfe::{cnpj_valido, cpf_valido};
use rustnf::nfse::leitura::Nfse;

use super::mascara::iso_para_data_br;
use super::nota::{CampoNota, DadosNota};

/// O que se sabe do tomador pela nota mais recente dele.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TomadorConhecido {
    /// O numero da NFS-e de onde os dados vieram.
    pub numero_da_nota: String,
    /// A data dessa nota, `DD/MM/AAAA`.
    pub data_da_nota: String,
    nome: String,
    im: String,
    email: String,
    fone: String,
    cep: String,
    logradouro: String,
    numero: String,
    complemento: String,
    bairro: String,
    municipio: String,
}

impl TomadorConhecido {
    /// O tomador de uma NFS-e lida. `emissao` e' a data do indice da pasta (`AAAA-MM-DD`, pode seguir a hora).
    #[must_use]
    pub fn da_nfse(n: &Nfse, emissao: &str) -> Self {
        Self {
            numero_da_nota: n.numero.clone(),
            data_da_nota: emissao.get(..10).map_or_else(String::new, iso_para_data_br),
            nome: n.tomador_nome.clone(),
            im: n.tomador_im.clone(),
            email: n.tomador_email.clone(),
            fone: n.tomador_fone.clone(),
            cep: n.tomador_cep.clone(),
            logradouro: n.tomador_logradouro.clone(),
            numero: n.tomador_numero.clone(),
            complemento: n.tomador_complemento.clone(),
            bairro: n.tomador_bairro.clone(),
            municipio: n.tomador_municipio.clone(),
        }
    }

    fn pares(&self) -> [(CampoNota, &str); 10] {
        [
            (CampoNota::TomadorNome, &self.nome),
            (CampoNota::TomadorIm, &self.im),
            (CampoNota::TomadorEmail, &self.email),
            (CampoNota::TomadorFone, &self.fone),
            (CampoNota::EnderecoCep, &self.cep),
            (CampoNota::EnderecoLogradouro, &self.logradouro),
            (CampoNota::EnderecoNumero, &self.numero),
            (CampoNota::EnderecoComplemento, &self.complemento),
            (CampoNota::EnderecoBairro, &self.bairro),
            (CampoNota::EnderecoMunicipio, &self.municipio),
        ]
    }

    /// Preenche, em `d`, so' os campos do tomador e do endereco que estao vazios e que a nota traz. Devolve os campos
    /// preenchidos (vazio: nada mudou).
    ///
    /// O endereco vai inteiro ou nao vai: com qualquer campo do endereco ja digitado, ele fica como a pessoa o deixou
    /// (misturar o CEP de um com a rua de outro daria um endereco que nao existe).
    pub fn completar(&self, d: &mut DadosNota) -> Vec<CampoNota> {
        let endereco_digitado = d.tem_endereco();
        let mut feitos = Vec::new();
        for (campo, valor) in self.pares() {
            let do_endereco = !matches!(
                campo,
                CampoNota::TomadorNome
                    | CampoNota::TomadorIm
                    | CampoNota::TomadorEmail
                    | CampoNota::TomadorFone
            );
            if valor.trim().is_empty()
                || !d.texto(campo).trim().is_empty()
                || (do_endereco && endereco_digitado)
            {
                continue;
            }
            d.definir(campo, valor);
            feitos.push(campo);
        }
        feitos
    }

    /// A frase que a tela mostra depois de completar.
    #[must_use]
    pub fn aviso(&self) -> String {
        if self.data_da_nota.is_empty() {
            format!(
                "Dados do tomador preenchidos a partir da NFS-e nº {}. Confira antes de emitir.",
                self.numero_da_nota
            )
        } else {
            format!(
                "Dados do tomador preenchidos a partir da NFS-e nº {}, de {}. Confira antes de emitir.",
                self.numero_da_nota, self.data_da_nota
            )
        }
    }
}

/// O CPF ou CNPJ digitado, so' com letras e digitos, quando esta completo e confere; `None` enquanto a pessoa digita
/// (ou se nao confere): so' entao vale procurar o tomador na pasta.
#[must_use]
pub fn documento_completo(d: &DadosNota) -> Option<String> {
    let doc = d.documento_do_tomador().to_ascii_uppercase();
    let ok = match doc.len() {
        11 => doc.bytes().all(|b| b.is_ascii_digit()) && cpf_valido(&doc),
        14 => cnpj_valido(&doc),
        _ => false,
    };
    (ok && !d.sem_tomador).then_some(doc)
}

#[cfg(test)]
mod testes {
    use rustnf::nfse::leitura::ler_nfse;

    use super::*;
    use crate::modelo::pedido::testes::ctx_do_caso;

    /// A NFS-e de exemplo, com o tomador completo (o exemplo traz o endereco; o contato vem acrescentado aqui).
    fn conhecido() -> TomadorConhecido {
        let mut n = ler_nfse(include_str!("../../../rustnf-cli/exemplos/nfse.xml")).unwrap();
        n.tomador_im = "998877".into();
        n.tomador_email = "contato@exemplo.com.br".into();
        n.tomador_fone = "11988887777".into();
        TomadorConhecido::da_nfse(&n, "2026-09-23T10:00:00-03:00")
    }

    #[test]
    fn formulario_vazio_recebe_nome_contato_e_endereco_com_a_mascara_da_tela() {
        let t = conhecido();
        let mut d = ctx_do_caso().nota_nova();
        d.definir(CampoNota::TomadorDocumento, "33444555000172");
        let feitos = t.completar(&mut d);
        assert_eq!(feitos.len(), 10, "{feitos:?}");
        assert_eq!(d.tomador_nome, "CLIENTE EXEMPLO SA");
        assert_eq!(d.tomador_im, "998877");
        assert_eq!(d.tomador_email, "contato@exemplo.com.br");
        assert_eq!(d.tomador_fone, "(11) 98888-7777");
        assert_eq!(d.end_cep, "05422-030");
        assert_eq!(d.end_logradouro, "Rua");
        assert_eq!(d.end_numero, "2176");
        assert_eq!(d.end_complemento, "COMPLEMENTO");
        assert_eq!(d.end_bairro, "Centro");
        assert_eq!(d.end_municipio, "3503208");
        assert_eq!(
            t.aviso(),
            "Dados do tomador preenchidos a partir da NFS-e nº 25, de 23/09/2026. Confira antes de emitir."
        );
    }

    #[test]
    fn o_que_a_pessoa_digitou_fica() {
        let t = conhecido();
        let mut d = ctx_do_caso().nota_nova();
        d.definir(CampoNota::TomadorNome, "Nome que a pessoa digitou");
        d.definir(CampoNota::EnderecoBairro, "Outro bairro");
        let feitos = t.completar(&mut d);
        assert_eq!(d.tomador_nome, "Nome que a pessoa digitou");
        // com parte do endereco digitada, o endereco fica inteiro como esta
        assert_eq!(d.end_bairro, "Outro bairro");
        assert_eq!(d.end_cep, "");
        assert_eq!(d.end_logradouro, "");
        // o contato, que estava vazio, vem
        assert_eq!(d.tomador_email, "contato@exemplo.com.br");
        assert_eq!(
            feitos,
            [
                CampoNota::TomadorIm,
                CampoNota::TomadorEmail,
                CampoNota::TomadorFone
            ]
        );
        // completar de novo nao muda mais nada
        assert!(t.completar(&mut d).is_empty());
    }

    #[test]
    fn so_procura_com_o_documento_completo_e_valido() {
        let mut d = ctx_do_caso().nota_nova();
        for (digitado, achado) in [
            ("", None),
            ("11.222.333/0001", None),
            ("11.222.333/0001-82", None),
            ("11.222.333/0001-81", Some("11222333000181")),
            // o CNPJ do XML de exemplo e' ficticio e nao confere: nao procura
            ("33.444.555/0001-72", None),
            ("529.982.247-2", None),
            ("529.982.247-25", Some("52998224725")),
        ] {
            d.definir(CampoNota::TomadorDocumento, digitado);
            assert_eq!(documento_completo(&d).as_deref(), achado, "{digitado}");
        }
        // nota sem tomador: nao procura
        d.sem_tomador = true;
        assert_eq!(documento_completo(&d), None);
    }
}
