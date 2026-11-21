// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A busca do codigo de tributacao nacional e a sugestao de NBS, `cIndOp` e `cClassTrib` pela correlacao do Anexo
//! VIII. A sugestao e' uma PROPOSTA: so' preenche campo vazio, nunca sobrescreve o que a pessoa digitou, e nunca
//! escolhe calada quando ha mais de uma opcao (a biblioteca recusa e a tela mostra as opcoes).

use rustnf::nfse::correlacao::{self, Filtro};
use rustnf::nfse::ctribnac;
use rustnf::reforma::{indop, tabela};

use super::mascara::{formatar_ctribnac, formatar_nbs};
use super::nota::{CampoNota, DadosNota};

/// Um servico da Lista Nacional, para a lista de resultados.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Servico {
    /// O codigo de 6 digitos.
    pub codigo: String,
    /// O codigo com mascara (`14.01.01`).
    pub mascarado: String,
    /// A descricao oficial.
    pub descricao: String,
}

/// Quantos servicos a busca mostra.
pub const LIMITE_DA_BUSCA: usize = 12;

/// Busca na Lista Nacional por texto ou por codigo. Vazio: nada.
#[must_use]
pub fn buscar_servico(termo: &str) -> Vec<Servico> {
    if termo.trim().is_empty() {
        return Vec::new();
    }
    ctribnac::buscar(termo, LIMITE_DA_BUSCA)
        .into_iter()
        .map(|c| Servico {
            codigo: c.ctribnac.clone(),
            mascarado: formatar_ctribnac(&c.ctribnac),
            descricao: c.descricao.clone(),
        })
        .collect()
}

/// A descricao oficial de um codigo de tributacao nacional ja digitado (`14.01.01` ou `140101`), se existir.
#[must_use]
pub fn descricao_do_servico(ctribnac_digitado: &str) -> Option<&'static str> {
    let digitos: String = ctribnac_digitado
        .chars()
        .filter(char::is_ascii_digit)
        .collect();
    ctribnac::achar(&digitos).map(|c| c.descricao.as_str())
}

/// Um NBS que o Anexo VIII traz para o servico.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpcaoNbs {
    /// Os 9 digitos.
    pub codigo: String,
    /// Com mascara (`1.2001.10.00`).
    pub mascarado: String,
    /// A descricao, quando o anexo traz.
    pub descricao: String,
}

/// A classificacao proposta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proposta {
    /// O NBS, quando e' um so'.
    pub nbs: Option<String>,
    /// O `cIndOp`.
    pub cind_op: String,
    /// O `cClassTrib`.
    pub cclass_trib: String,
    /// Observacoes do anexo (por exemplo, um codigo que o Anexo VII marca como nao sendo de NFS-e).
    pub avisos: Vec<String>,
}

/// O que a correlacao diz de um servico.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Sugestao {
    /// A proposta, quando a combinacao e' unica.
    pub proposta: Option<Proposta>,
    /// Os NBS que o servico tem no anexo, quando ha mais de um.
    pub nbs: Vec<OpcaoNbs>,
    /// Por que nao ha proposta (varias classificacoes, item fora do anexo...).
    pub aviso: Option<String>,
}

fn opcoes_de_nbs(codigo: &str) -> Vec<OpcaoNbs> {
    let mut vistos = Vec::new();
    if let Ok(linhas) = correlacao::candidatos(codigo, Filtro::default()) {
        for l in linhas {
            if let Some(n) = &l.nbs {
                if !vistos.contains(n) {
                    vistos.push(n.clone());
                }
            }
        }
    }
    vistos
        .into_iter()
        .map(|n| OpcaoNbs {
            mascarado: formatar_nbs(&n),
            descricao: correlacao::descricao_nbs(&n).to_string(),
            codigo: n,
        })
        .collect()
}

fn proposta_de(c: &correlacao::Correlacao, nbs_unico: Option<String>) -> Option<Proposta> {
    Some(Proposta {
        nbs: nbs_unico,
        cind_op: c.cind_op.clone()?,
        cclass_trib: c.cclasstrib.clone()?,
        avisos: c.avisos.clone(),
    })
}

/// A sugestao para o servico (`ctribnac`, 6 digitos) e o NBS ja escolhido (9 digitos, ou vazio).
#[must_use]
pub fn sugerir(ctribnac: &str, nbs: &str) -> Sugestao {
    if ctribnac.len() != 6 {
        return Sugestao::default();
    }
    let opcoes = opcoes_de_nbs(ctribnac);
    if let Err(e) = correlacao::candidatos(ctribnac, Filtro::default()) {
        return Sugestao {
            aviso: Some(e.mensagem),
            ..Sugestao::default()
        };
    }
    let escolhido: Option<String> = if nbs.len() == 9 {
        Some(nbs.to_string())
    } else if opcoes.len() == 1 {
        Some(opcoes[0].codigo.clone())
    } else {
        None
    };
    let filtro = Filtro {
        nbs: escolhido.as_deref(),
        ..Filtro::default()
    };
    // o NBS so' vira opcao a escolher enquanto a pessoa nao escolheu um
    let a_escolher = if nbs.len() == 9 { Vec::new() } else { opcoes };
    match correlacao::sugerir(ctribnac, filtro) {
        Ok(c) => Sugestao {
            proposta: proposta_de(c, escolhido.clone()),
            nbs: a_escolher,
            aviso: None,
        },
        Err(e) => Sugestao {
            proposta: None,
            nbs: a_escolher,
            aviso: Some(e.mensagem),
        },
    }
}

/// O estado de um item da proposta frente ao que ja esta digitado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstadoDoItem {
    /// O campo esta vazio: aceitar preenche.
    Preencher,
    /// O campo ja tem exatamente este valor.
    Igual,
    /// A pessoa digitou outro valor: aceitar NAO o troca.
    Diferente,
}

/// Um item da proposta, com o que aceitar faria.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemDaProposta {
    /// O campo.
    pub campo: CampoNota,
    /// O valor proposto, como aparece na tela.
    pub valor: String,
    /// Uma descricao curta do valor, quando a tabela oficial tem.
    pub descricao: String,
    /// Frente ao digitado.
    pub estado: EstadoDoItem,
}

fn estado(atual: &str, proposto: &str) -> EstadoDoItem {
    if atual.trim().is_empty() {
        EstadoDoItem::Preencher
    } else if atual == proposto {
        EstadoDoItem::Igual
    } else {
        EstadoDoItem::Diferente
    }
}

impl Proposta {
    /// Os itens da proposta frente aos dados, na ordem da tela (NBS, `cIndOp`, `cClassTrib`).
    #[must_use]
    pub fn itens(&self, dados: &DadosNota) -> Vec<ItemDaProposta> {
        let mut itens = Vec::new();
        if let Some(n) = &self.nbs {
            let m = formatar_nbs(n);
            itens.push(ItemDaProposta {
                campo: CampoNota::Nbs,
                estado: estado(&dados.nbs, &m),
                descricao: correlacao::descricao_nbs(n).to_string(),
                valor: m,
            });
        }
        itens.push(ItemDaProposta {
            campo: CampoNota::CindOp,
            estado: estado(&dados.cind_op, &self.cind_op),
            descricao: descricao_do_cind_op(&self.cind_op),
            valor: self.cind_op.clone(),
        });
        itens.push(ItemDaProposta {
            campo: CampoNota::ClassTrib,
            estado: estado(&dados.cclass_trib, &self.cclass_trib),
            descricao: descricao_da_classe(&self.cclass_trib),
            valor: self.cclass_trib.clone(),
        });
        itens
    }

    /// Ha algo que aceitar preencheria.
    #[must_use]
    pub fn ha_o_que_preencher(&self, dados: &DadosNota) -> bool {
        self.itens(dados)
            .iter()
            .any(|i| i.estado == EstadoDoItem::Preencher)
    }

    /// Aceita a proposta: preenche so' os campos VAZIOS e devolve quais preencheu. O que a pessoa digitou fica.
    pub fn aceitar(&self, dados: &mut DadosNota) -> Vec<CampoNota> {
        let mut preenchidos = Vec::new();
        for item in self.itens(dados) {
            if item.estado == EstadoDoItem::Preencher {
                dados.definir(item.campo, &item.valor);
                preenchidos.push(item.campo);
            }
        }
        preenchidos
    }
}

/// A descricao do NBS no Anexo VIII, quando ja' tem os 9 digitos e consta.
#[must_use]
pub fn descricao_do_nbs(nbs: &str) -> Option<String> {
    let digitos: String = nbs.chars().filter(char::is_ascii_digit).collect();
    if digitos.len() != 9 {
        return None;
    }
    let texto = correlacao::descricao_nbs(&digitos);
    (!texto.is_empty()).then(|| texto.to_string())
}

/// A descricao curta do `cIndOp` (tipo de operacao do Anexo VII), ou vazio.
#[must_use]
pub fn descricao_do_cind_op(codigo: &str) -> String {
    indop::registro(codigo).map_or_else(String::new, |r| r.tipo_operacao.clone())
}

/// O nome oficial de um `cClassTrib`, ou vazio.
#[must_use]
pub fn descricao_da_classe(codigo: &str) -> String {
    tabela::classificacao(codigo).map_or_else(|_| String::new(), |c| c.nome.clone())
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn busca_por_texto_e_por_codigo_sem_acento() {
        let por_texto = buscar_servico("programacao");
        assert!(
            por_texto.iter().any(|s| s.codigo == "010201"),
            "{por_texto:?}"
        );
        let por_codigo = buscar_servico("140101");
        assert_eq!(por_codigo.len(), 1);
        assert_eq!(por_codigo[0].mascarado, "14.01.01");
        assert!(por_codigo[0].descricao.starts_with("Lubrificação"));
        assert!(buscar_servico("").is_empty());
        assert!(buscar_servico("   ").is_empty());
        assert!(buscar_servico("zzzzzzzz").is_empty());
        assert!(buscar_servico("a").len() <= LIMITE_DA_BUSCA);
    }

    #[test]
    fn descricao_do_codigo_digitado() {
        assert!(descricao_do_servico("14.01.01")
            .unwrap()
            .starts_with("Lubrificação"));
        assert!(descricao_do_servico("140101").is_some());
        assert_eq!(descricao_do_servico("990000"), None);
        assert_eq!(descricao_do_servico(""), None);
    }

    #[test]
    fn servico_com_um_so_par_e_varios_nbs_propoe_e_lista_os_nbs() {
        // 01.03.01: uma unica combinacao no anexo (100301 + 000001), com varios NBS
        let s = sugerir("010301", "");
        let p = s.proposta.expect("proposta");
        assert_eq!(p.cind_op, "100301");
        assert_eq!(p.cclass_trib, "000001");
        assert_eq!(p.nbs, None, "varios NBS: a pessoa escolhe");
        assert!(s.nbs.len() > 1);
        assert!(s.nbs.iter().all(|n| n.codigo.len() == 9));
        assert!(s.aviso.is_none());
    }

    #[test]
    fn servico_com_varias_classificacoes_nao_escolhe_calado() {
        // 14.01.01: duas opcoes de cIndOp por NBS
        let s = sugerir("140101", "");
        assert!(s.proposta.is_none());
        assert!(!s.nbs.is_empty());
        assert!(s.aviso.as_deref().unwrap().contains("chute fiscal"));
        // com o NBS escolhido ainda ha duas (050101 e 050102)
        let s = sugerir("140101", "120011000");
        assert!(s.proposta.is_none());
        assert!(s.nbs.is_empty(), "NBS ja escolhido nao volta como opcao");
        assert!(s.aviso.is_some());
    }

    #[test]
    fn servico_com_um_nbs_e_um_par_propoe_tudo() {
        // 03.04.01: um NBS e um par no anexo
        let s = sugerir("030401", "");
        let p = s.proposta.expect("proposta");
        assert_eq!(p.nbs.as_deref(), Some("110011210"));
        assert_eq!(p.cind_op, "020401");
        assert_eq!(p.cclass_trib, "200027");
        assert!(s.nbs.len() == 1);
    }

    #[test]
    fn item_fora_do_anexo_ou_codigo_incompleto() {
        assert_eq!(sugerir("", ""), Sugestao::default());
        assert_eq!(sugerir("1401", ""), Sugestao::default());
        let s = sugerir("999999", "");
        assert!(s.proposta.is_none());
        assert!(s.aviso.is_some());
    }

    #[test]
    fn aceitar_so_preenche_o_que_esta_vazio() {
        let p = sugerir("030401", "").proposta.unwrap();
        let mut d = DadosNota::novo("21/11/2026");
        d.definir(CampoNota::CindOp, "050101");
        let itens = p.itens(&d);
        assert_eq!(itens[0].campo, CampoNota::Nbs);
        assert_eq!(itens[0].estado, EstadoDoItem::Preencher);
        assert_eq!(itens[1].estado, EstadoDoItem::Diferente);
        assert_eq!(itens[2].estado, EstadoDoItem::Preencher);
        assert!(p.ha_o_que_preencher(&d));
        let feitos = p.aceitar(&mut d);
        assert_eq!(feitos, vec![CampoNota::Nbs, CampoNota::ClassTrib]);
        assert_eq!(d.cind_op, "050101", "o digitado fica");
        assert_eq!(d.cclass_trib, "200027");
        assert_eq!(d.nbs, "1.1001.12.10");
        assert!(!p.ha_o_que_preencher(&d));
        // aceitar de novo nao muda nada
        assert!(p.aceitar(&mut d).is_empty());
        // valores iguais aos da proposta contam como iguais
        let mut igual = DadosNota::novo("21/11/2026");
        p.aceitar(&mut igual);
        assert!(p
            .itens(&igual)
            .iter()
            .all(|i| i.estado == EstadoDoItem::Igual));
    }

    #[test]
    fn descricoes_vem_das_tabelas_oficiais() {
        assert!(!descricao_da_classe("000001").is_empty());
        assert_eq!(descricao_da_classe("999999"), "");
        assert_eq!(descricao_do_cind_op("999999"), "");
        let proposta = sugerir("030401", "").proposta.unwrap();
        let itens = proposta.itens(&DadosNota::novo("21/11/2026"));
        assert!(itens.iter().all(|i| !i.valor.is_empty()));
    }
}
