// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O cadastro de produtos da NF-e: o que a loja vende, com o NCM, a origem e o detalhe fiscal que a pessoa fixou, para
//! a proxima nota nao pedir tudo de novo. Fica num arquivo JSON proprio na pasta de notas, lido com tolerancia (arquivo
//! ruim = lista vazia e um aviso, nunca panico) e gravado de forma atomica. Sem janela.

use rustnf_app::notas::sem_acento;
use serde::{Deserialize, Serialize};

use super::nfe_fiscal::{Icms, Origem};
use super::nfe_nova::ItemNfe;

/// O nome do arquivo, na pasta de notas.
pub const ARQUIVO: &str = "produtos.json";

/// A versao do formato do arquivo.
pub const VERSAO: u32 = 1;

/// Teto de leitura do arquivo: 8 MiB (uns 20 mil produtos).
pub const LIMITE: u64 = 8 * 1024 * 1024;

/// Quantos produtos o cadastro guarda.
pub const MAXIMO: usize = 20_000;

/// O detalhe fiscal que a pessoa fixou no produto: vale em toda nota e nunca e' trocado pela tabela.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Fixado {
    /// Como o ICMS se comporta.
    pub icms: Option<Icms>,
    /// A aliquota do ICMS, em percentual.
    pub aliquota_icms: String,
    /// CFOP escolhido a mao.
    pub cfop: Option<String>,
    /// CST ou CSOSN escolhido a mao.
    pub icms_codigo: Option<String>,
    /// CST do PIS e da COFINS escolhido a mao.
    pub pis_cst: Option<String>,
}

impl Fixado {
    /// Nada foi fixado.
    #[must_use]
    pub fn vazio(&self) -> bool {
        *self == Self::default()
    }
}

/// Um produto do cadastro.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Produto {
    /// O codigo na loja (a chave do produto no cadastro).
    pub codigo: String,
    /// O que e'.
    pub descricao: String,
    /// A unidade.
    pub unidade: String,
    /// O valor de uma unidade, como a tela mostra (`15,90`).
    pub valor: String,
    /// O NCM.
    pub ncm: String,
    /// O CEST, se tem.
    pub cest: String,
    /// A origem da mercadoria (0 a 8).
    pub origem_mercadoria: u8,
    /// Fabrico ou revendo.
    pub origem: Origem,
    /// O detalhe fiscal fixado.
    pub fixado: Fixado,
}

impl Default for Produto {
    fn default() -> Self {
        Self {
            codigo: String::new(),
            descricao: String::new(),
            unidade: "UN".into(),
            valor: String::new(),
            ncm: String::new(),
            cest: String::new(),
            origem_mercadoria: 0,
            origem: Origem::default(),
            fixado: Fixado::default(),
        }
    }
}

impl Produto {
    /// O produto serve para ser guardado: tem descricao e NCM de 8 digitos.
    #[must_use]
    pub fn completo(&self) -> bool {
        !self.descricao.trim().is_empty()
            && self.ncm.chars().filter(char::is_ascii_digit).count() == 8
    }

    /// O item da nota que o produto vira: quantidade 1, e o detalhe fixado.
    #[must_use]
    pub fn para_item(&self) -> ItemNfe {
        ItemNfe {
            codigo: self.codigo.clone(),
            descricao: self.descricao.clone(),
            unidade: self.unidade.clone(),
            valor: self.valor.clone(),
            ncm: self.ncm.clone(),
            cest: self.cest.clone(),
            origem_mercadoria: self.origem_mercadoria,
            origem: self.origem,
            icms: self.fixado.icms,
            aliquota_icms: self.fixado.aliquota_icms.clone(),
            cfop: self.fixado.cfop.clone(),
            icms_codigo: self.fixado.icms_codigo.clone(),
            pis_cst: self.fixado.pis_cst.clone(),
            do_cadastro: true,
            ..ItemNfe::default()
        }
    }

    /// O produto que o item da nota diz.
    #[must_use]
    pub fn do_item(i: &ItemNfe) -> Self {
        let fixado = Fixado {
            icms: i.icms,
            aliquota_icms: i.aliquota_icms.clone(),
            cfop: i.cfop.clone(),
            icms_codigo: i.icms_codigo.clone(),
            pis_cst: i.pis_cst.clone(),
        };
        Self {
            codigo: i.codigo.trim().to_string(),
            descricao: i.descricao.trim().to_string(),
            unidade: i.unidade.trim().to_uppercase(),
            valor: i.valor.trim().to_string(),
            ncm: i.ncm.chars().filter(char::is_ascii_digit).collect(),
            cest: i.cest.chars().filter(char::is_ascii_digit).collect(),
            origem_mercadoria: i.origem_mercadoria,
            origem: i.origem,
            fixado,
        }
    }
}

#[derive(Serialize)]
struct Gravacao<'a> {
    versao: u32,
    produtos: &'a [Produto],
}

#[derive(Deserialize)]
struct Leitura {
    versao: u32,
    #[serde(default)]
    produtos: Vec<Produto>,
}

/// O cadastro.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Cadastro {
    /// Os produtos, na ordem em que entraram.
    pub produtos: Vec<Produto>,
}

/// Como o cadastro foi lido.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lido {
    /// O cadastro (vazio se o arquivo nao serviu).
    pub cadastro: Cadastro,
    /// O que dizer a pessoa quando o arquivo existia e nao serviu.
    pub aviso: Option<String>,
}

/// O aviso do arquivo que nao serviu.
pub const AVISO_ARQUIVO_RUIM: &str = "O arquivo de produtos está ilegível ou é de outra versao do programa: a lista abriu vazia e o arquivo não foi mexido. Ele só é trocado quando você guardar um produto.";

impl Cadastro {
    /// Le os bytes do arquivo. Arquivo ilegivel, de outra versao ou grande demais: lista vazia e um aviso.
    #[must_use]
    pub fn ler(bytes: &[u8]) -> Lido {
        let ruim = || Lido {
            cadastro: Self::default(),
            aviso: Some(AVISO_ARQUIVO_RUIM.into()),
        };
        if bytes.len() as u64 > LIMITE {
            return ruim();
        }
        match serde_json::from_slice::<Leitura>(bytes) {
            Ok(l) if l.versao == VERSAO => {
                let mut produtos = l.produtos;
                produtos.truncate(MAXIMO);
                Lido {
                    cadastro: Self { produtos },
                    aviso: None,
                }
            }
            _ => ruim(),
        }
    }

    /// Os bytes do arquivo.
    #[must_use]
    pub fn codificar(&self) -> Vec<u8> {
        let mut texto = serde_json::to_string_pretty(&Gravacao {
            versao: VERSAO,
            produtos: &self.produtos,
        })
        .unwrap_or_default();
        texto.push('\n');
        texto.into_bytes()
    }

    /// As posicoes dos produtos que casam com o texto (descricao sem acento, codigo ou NCM), no maximo `limite`. Texto
    /// vazio: os primeiros.
    #[must_use]
    pub fn indices(&self, texto: &str, limite: usize) -> Vec<usize> {
        let palavras: Vec<String> = sem_acento(texto)
            .split_whitespace()
            .map(str::to_string)
            .collect();
        self.produtos
            .iter()
            .enumerate()
            .filter(|(_, p)| {
                let alvo = format!(
                    "{} {} {}",
                    sem_acento(&p.descricao),
                    sem_acento(&p.codigo),
                    p.ncm
                );
                palavras.iter().all(|w| alvo.contains(w.as_str()))
            })
            .map(|(i, _)| i)
            .take(limite)
            .collect()
    }

    /// Os produtos que casam com o texto, no maximo `limite`.
    #[must_use]
    pub fn buscar(&self, texto: &str, limite: usize) -> Vec<&Produto> {
        self.indices(texto, limite)
            .into_iter()
            .filter_map(|i| self.produtos.get(i))
            .collect()
    }

    /// Guarda o produto: troca o que tem o mesmo codigo (ou, sem codigo, a mesma descricao) e acrescenta o que e' novo.
    /// Devolve `true` se acrescentou. O cadastro cheio recusa o novo.
    pub fn guardar(&mut self, produto: Produto) -> bool {
        let mesmo = |p: &Produto| {
            if produto.codigo.is_empty() {
                p.codigo.is_empty() && sem_acento(&p.descricao) == sem_acento(&produto.descricao)
            } else {
                p.codigo == produto.codigo
            }
        };
        if let Some(existente) = self.produtos.iter_mut().find(|p| mesmo(p)) {
            *existente = produto;
            return false;
        }
        if self.produtos.len() >= MAXIMO {
            return false;
        }
        self.produtos.push(produto);
        true
    }

    /// Apaga o produto da posicao `i`.
    pub fn apagar(&mut self, i: usize) -> Option<Produto> {
        (i < self.produtos.len()).then(|| self.produtos.remove(i))
    }

    /// Tem o produto de codigo `codigo`.
    #[must_use]
    pub fn tem(&self, codigo: &str) -> bool {
        !codigo.is_empty() && self.produtos.iter().any(|p| p.codigo == codigo)
    }

    /// Os produtos desta nota que entram no cadastro ao autorizar: os que a pessoa deixou marcados e que ainda nao
    /// estao nele (o que ja esta, so' se atualiza se a pessoa o editou na nota) e que estao completos.
    #[must_use]
    pub fn novos_da_nota(&self, itens: &[ItemNfe]) -> Vec<Produto> {
        itens
            .iter()
            .filter(|i| i.guardar)
            .map(Produto::do_item)
            .filter(Produto::completo)
            .filter(|p| !self.tem(&p.codigo) || !self.iguais(p))
            .collect()
    }

    fn iguais(&self, p: &Produto) -> bool {
        self.produtos.iter().any(|q| q == p)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn cafe() -> Produto {
        Produto {
            codigo: "C1".into(),
            descricao: "Cafe torrado 500g".into(),
            valor: "15,90".into(),
            ncm: "09012100".into(),
            ..Produto::default()
        }
    }

    #[test]
    fn ida_e_volta_guarda_tudo_inclusive_o_detalhe_fixado() {
        let mut c = Cadastro::default();
        let mut p = cafe();
        p.fixado = Fixado {
            icms: Some(Icms::Tributado),
            aliquota_icms: "18".into(),
            cfop: Some("5102".into()),
            icms_codigo: None,
            pis_cst: Some("01".into()),
        };
        assert!(c.guardar(p));
        let lido = Cadastro::ler(&c.codificar());
        assert_eq!(lido.aviso, None);
        assert_eq!(lido.cadastro, c);
        assert!(String::from_utf8(c.codificar())
            .unwrap()
            .contains("\"versao\": 1"));
    }

    #[test]
    fn arquivo_ruim_vira_lista_vazia_com_aviso_e_nunca_panico() {
        for ruim in [
            &b""[..],
            b"{ nao e json",
            b"[1,2]",
            b"{\"produtos\":[]}",
            b"{\"versao\":2,\"produtos\":[]}",
            &[0xff, 0xfe, 0x00],
            b"{\"versao\":1,\"produtos\":[1,2]}",
        ] {
            let l = Cadastro::ler(ruim);
            assert!(l.cadastro.produtos.is_empty());
            assert_eq!(l.aviso.as_deref(), Some(AVISO_ARQUIVO_RUIM));
        }
        let enorme = vec![b' '; (LIMITE + 1) as usize];
        assert!(Cadastro::ler(&enorme).aviso.is_some());
        // versao certa com produto parcial: o que falta fica no padrao
        let l = Cadastro::ler(b"{\"versao\":1,\"produtos\":[{\"descricao\":\"x\"}]}");
        assert_eq!(l.aviso, None);
        assert_eq!(l.cadastro.produtos[0].unidade, "UN");
    }

    #[test]
    fn a_busca_acha_por_descricao_sem_acento_codigo_e_ncm() {
        let mut c = Cadastro::default();
        c.guardar(cafe());
        c.guardar(Produto {
            codigo: "F9".into(),
            descricao: "Filtro de papel 103".into(),
            ncm: "48119090".into(),
            ..Produto::default()
        });
        assert_eq!(c.buscar("CAFÉ", 10).len(), 1);
        assert_eq!(c.buscar("cafe 500", 10)[0].codigo, "C1");
        assert_eq!(c.buscar("f9", 10)[0].descricao, "Filtro de papel 103");
        assert_eq!(c.buscar("4811", 10).len(), 1);
        assert_eq!(c.buscar("", 10).len(), 2);
        assert_eq!(c.buscar("", 1).len(), 1);
        assert!(c.buscar("nada disso", 10).is_empty());
    }

    #[test]
    fn guardar_troca_o_do_mesmo_codigo_e_acrescenta_o_novo() {
        let mut c = Cadastro::default();
        assert!(c.guardar(cafe()));
        let mut mais_caro = cafe();
        mais_caro.valor = "17,00".into();
        assert!(!c.guardar(mais_caro));
        assert_eq!(c.produtos.len(), 1);
        assert_eq!(c.produtos[0].valor, "17,00");
        // sem codigo: a mesma descricao e' o mesmo produto
        let sem_codigo = Produto {
            descricao: "Sacola".into(),
            ncm: "39232100".into(),
            ..Produto::default()
        };
        assert!(c.guardar(sem_codigo.clone()));
        assert!(!c.guardar(sem_codigo));
        assert_eq!(c.produtos.len(), 2);
        assert!(c.apagar(5).is_none());
        assert_eq!(c.apagar(0).unwrap().codigo, "C1");
        assert_eq!(c.produtos.len(), 1);
    }

    #[test]
    fn o_produto_vira_item_e_o_item_volta_a_ser_produto() {
        let mut p = cafe();
        p.fixado.cfop = Some("5949".into());
        let item = p.para_item();
        assert!(item.do_cadastro);
        assert_eq!(item.quantidade, "1");
        assert_eq!(item.cfop.as_deref(), Some("5949"));
        assert_eq!(item.valor, "15,90");
        assert_eq!(Produto::do_item(&item), p);
    }

    #[test]
    fn produto_novo_da_nota_entra_se_marcado_e_completo() {
        let mut c = Cadastro::default();
        c.guardar(cafe());
        let novo = ItemNfe {
            codigo: "N1".into(),
            descricao: "Moedor".into(),
            ncm: "8210.00.00".into(),
            valor: "50,00".into(),
            ..ItemNfe::default()
        };
        let desmarcado = ItemNfe {
            guardar: false,
            ..novo.clone()
        };
        let sem_ncm = ItemNfe {
            ncm: String::new(),
            ..novo.clone()
        };
        let igual = cafe().para_item();
        let novos = c.novos_da_nota(&[novo, desmarcado, sem_ncm, igual]);
        assert_eq!(novos.len(), 1);
        assert_eq!(novos[0].codigo, "N1");
        assert_eq!(novos[0].ncm, "82100000", "o NCM guarda só os dígitos");
        // o do cadastro que a pessoa editou na nota (outro preco) entra para atualizar
        let mut editado = cafe().para_item();
        editado.valor = "19,90".into();
        assert_eq!(c.novos_da_nota(&[editado]).len(), 1);
    }
}
