// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A lista das notas da pasta de notas: uma linha por nota ja com o texto da tela (data, numero, DPS, tomador com o
//! CPF mascarado, valor e situacao), a busca por texto e o filtro por situacao. Sem janela: a tela so' desenha o que
//! `visiveis` devolve.

use rustnf_app::notas::{chave_da_busca, Registro, Situacao};

use super::mascara::{
    centavos_para_dinheiro, dinheiro_para_centavos, documento_para_resumo, iso_para_data_br,
    limpar_documento,
};

/// O filtro por situacao.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FiltroDeSituacao {
    /// Todas as notas.
    #[default]
    Todas,
    /// So' as autorizadas.
    Autorizadas,
    /// So' as canceladas.
    Canceladas,
}

impl FiltroDeSituacao {
    /// Os filtros, na ordem em que a tela os mostra.
    pub const TODOS: [FiltroDeSituacao; 3] = [Self::Todas, Self::Autorizadas, Self::Canceladas];

    /// O rotulo do filtro.
    #[must_use]
    pub fn rotulo(self) -> &'static str {
        match self {
            Self::Todas => "Todas",
            Self::Autorizadas => "Autorizadas",
            Self::Canceladas => "Canceladas",
        }
    }

    fn deixa_passar(self, situacao: Situacao) -> bool {
        match self {
            Self::Todas => true,
            Self::Autorizadas => situacao == Situacao::Autorizada,
            Self::Canceladas => situacao == Situacao::Cancelada,
        }
    }
}

/// O rotulo da situacao de uma nota.
#[must_use]
pub fn rotulo_da_situacao(situacao: Situacao) -> &'static str {
    match situacao {
        Situacao::Autorizada => "Autorizada",
        Situacao::Cancelada => "Cancelada",
    }
}

/// Uma nota da lista, com os textos prontos para a tela.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinhaDeNota {
    /// A chave de acesso (50 posicoes).
    pub chave: String,
    /// A data de emissao, `DD/MM/AAAA`.
    pub data: String,
    /// O numero da NFS-e.
    pub numero: String,
    /// A DPS, `serie/numero`.
    pub dps: String,
    /// O nome do tomador (vazio se a nota nao o identifica).
    pub tomador: String,
    /// O documento do tomador para a lista: CPF mascarado (`***.***.***-NN`), CNPJ inteiro e formatado.
    pub documento: String,
    /// O valor do servico em dinheiro brasileiro (`1.234,56`).
    pub valor: String,
    /// A situacao.
    pub situacao: Situacao,
    /// O que a busca procura: nome sem acento e em minusculas, numeros e documento so' com digitos.
    palheiro: Palheiro,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Palheiro {
    nome: String,
    numeros: String,
    documento: String,
}

/// Minusculas e sem acento: a mesma regra da busca da pasta de notas na linha de comando.
pub use rustnf_app::notas::sem_acento;

fn so_digitos(texto: &str) -> String {
    texto.chars().filter(char::is_ascii_digit).collect()
}

impl LinhaDeNota {
    /// A linha de um registro do indice.
    #[must_use]
    pub fn do_registro(r: &Registro) -> Self {
        let data = r
            .emissao
            .get(..10)
            .map_or_else(|| r.emissao.clone(), iso_para_data_br);
        let valor = dinheiro_para_centavos(&r.valor_servico)
            .map_or_else(|| r.valor_servico.clone(), centavos_para_dinheiro);
        let documento = if limpar_documento(&r.tomador_documento).is_empty() {
            String::new()
        } else {
            documento_para_resumo(&r.tomador_documento)
        };
        let dps = match (r.serie.trim(), r.numero_dps.trim()) {
            ("", "") => String::new(),
            (s, n) => format!("{s}/{n}"),
        };
        Self {
            chave: r.chave.clone(),
            data,
            numero: r.numero.clone(),
            dps,
            tomador: r.tomador_nome.trim().to_string(),
            documento,
            valor,
            situacao: r.situacao,
            palheiro: Palheiro {
                nome: sem_acento(&r.tomador_nome),
                numeros: format!("{} {}", so_digitos(&r.numero), so_digitos(&r.numero_dps)),
                documento: so_digitos(&r.tomador_documento),
            },
        }
    }

    /// A linha casa com o termo (todas as palavras dele, em qualquer ordem, no nome, no documento ou nos numeros).
    fn casa(&self, termo: &Termo) -> bool {
        if let Some(chave) = &termo.chave_inteira {
            return self.chave.contains(chave.as_str());
        }
        termo.palavras.iter().all(|p| {
            self.palheiro.nome.contains(p.texto.as_str())
                || (!p.digitos.is_empty()
                    && (self.palheiro.documento.contains(&p.digitos)
                        || self.palheiro.numeros.contains(&p.digitos)))
        })
    }
}

struct Palavra {
    texto: String,
    digitos: String,
}

/// O texto da busca, ja separado em palavras. Uma chave de acesso colada (inteira ou o comeco dela, com 20 posicoes ou
/// mais) acha a nota pela chave; a regra e' a de `rustnf_app::notas::chave_da_busca`, a mesma da linha de comando.
struct Termo {
    palavras: Vec<Palavra>,
    chave_inteira: Option<String>,
}

fn termo(busca: &str) -> Termo {
    let chave_inteira = chave_da_busca(busca);
    let palavras = sem_acento(busca)
        .split_whitespace()
        .map(|p| Palavra {
            digitos: so_digitos(p),
            texto: p.to_string(),
        })
        .collect();
    Termo {
        palavras,
        chave_inteira,
    }
}

/// A lista das notas da pasta, com a busca e o filtro. Guarda os indices das linhas que passam, para nao refazer a
/// conta a cada desenho.
#[derive(Debug, Clone, Default)]
pub struct ListaDeNotas {
    linhas: Vec<LinhaDeNota>,
    busca: String,
    filtro: FiltroDeSituacao,
    visiveis: Vec<usize>,
}

impl ListaDeNotas {
    /// A lista dos registros do indice, da nota mais recente para a mais antiga (data, depois chave).
    #[must_use]
    pub fn nova(registros: &[Registro]) -> Self {
        let mut ordem: Vec<&Registro> = registros.iter().collect();
        ordem.sort_by(|a, b| (&b.emissao, &b.chave).cmp(&(&a.emissao, &a.chave)));
        let mut lista = Self {
            linhas: ordem.into_iter().map(LinhaDeNota::do_registro).collect(),
            ..Self::default()
        };
        lista.refazer();
        lista
    }

    /// A lista nova com a busca e o filtro que a antiga tinha (a tela recarrega sem perder o que a pessoa escolheu).
    #[must_use]
    pub fn recarregada(&self, registros: &[Registro]) -> Self {
        let mut nova = Self::nova(registros);
        nova.busca.clone_from(&self.busca);
        nova.filtro = self.filtro;
        nova.refazer();
        nova
    }

    fn refazer(&mut self) {
        let termo = termo(&self.busca);
        self.visiveis = self
            .linhas
            .iter()
            .enumerate()
            .filter(|(_, l)| self.filtro.deixa_passar(l.situacao) && l.casa(&termo))
            .map(|(i, _)| i)
            .collect();
    }

    /// Muda o texto da busca.
    pub fn definir_busca(&mut self, busca: &str) {
        if self.busca != busca {
            self.busca = busca.to_string();
            self.refazer();
        }
    }

    /// Muda o filtro de situacao.
    pub fn definir_filtro(&mut self, filtro: FiltroDeSituacao) {
        if self.filtro != filtro {
            self.filtro = filtro;
            self.refazer();
        }
    }

    /// O texto da busca.
    #[must_use]
    pub fn busca(&self) -> &str {
        &self.busca
    }

    /// O filtro de situacao.
    #[must_use]
    pub fn filtro(&self) -> FiltroDeSituacao {
        self.filtro
    }

    /// Quantas notas ha na pasta, sem busca nem filtro.
    #[must_use]
    pub fn total(&self) -> usize {
        self.linhas.len()
    }

    /// Quantas notas passam pela busca e pelo filtro.
    #[must_use]
    pub fn quantas(&self) -> usize {
        self.visiveis.len()
    }

    /// A pasta nao tem nota nenhuma.
    #[must_use]
    pub fn vazia(&self) -> bool {
        self.linhas.is_empty()
    }

    /// Ha notas, mas nenhuma passa pela busca e pelo filtro.
    #[must_use]
    pub fn sem_resultado(&self) -> bool {
        !self.linhas.is_empty() && self.visiveis.is_empty()
    }

    /// A linha `i` entre as que passam (a ordem e' a da lista).
    #[must_use]
    pub fn linha(&self, i: usize) -> Option<&LinhaDeNota> {
        self.visiveis.get(i).and_then(|k| self.linhas.get(*k))
    }

    /// A posicao, entre as que passam, da nota com a chave dada.
    #[must_use]
    pub fn posicao_da_chave(&self, chave: &str) -> Option<usize> {
        self.visiveis
            .iter()
            .position(|k| self.linhas.get(*k).is_some_and(|l| l.chave == chave))
    }

    /// Quantas notas de cada situacao ha na pasta.
    #[must_use]
    pub fn contagem(&self, situacao: Situacao) -> usize {
        self.linhas
            .iter()
            .filter(|l| l.situacao == situacao)
            .count()
    }
}

/// O texto do contador da lista: `1 nota`, `9 notas`, `1 de 9 notas`.
#[must_use]
pub fn contagem_de_notas(mostradas: usize, total: usize) -> String {
    let nome = if total == 1 { "nota" } else { "notas" };
    if mostradas == total {
        format!("{total} {nome}")
    } else {
        format!("{mostradas} de {total} {nome}")
    }
}

#[cfg(test)]
mod testes {
    use std::time::Instant;

    use rustnf::chave_dfe::montar_nfse;

    use super::*;
    use crate::modelo::chave;

    #[test]
    fn contador_concorda_em_numero() {
        assert_eq!(contagem_de_notas(1, 1), "1 nota");
        assert_eq!(contagem_de_notas(9, 9), "9 notas");
        assert_eq!(contagem_de_notas(1, 9), "1 de 9 notas");
        assert_eq!(contagem_de_notas(0, 1), "0 de 1 nota");
    }

    fn registro(n: u64, emissao: &str, nome: &str, doc: &str, situacao: Situacao) -> Registro {
        Registro {
            chave: montar_nfse(3_550_308, "11222333000181", n, "2610", Some(1), 2).unwrap(),
            numero: n.to_string(),
            serie: "1".into(),
            numero_dps: n.to_string(),
            emissao: emissao.into(),
            valor_servico: "1234.5".into(),
            situacao,
            tomador_documento: doc.into(),
            tomador_nome: nome.into(),
        }
    }

    fn amostra() -> Vec<Registro> {
        vec![
            registro(
                1,
                "2026-10-02T10:00:00-03:00",
                "Maria de Teste",
                "52998224725",
                Situacao::Autorizada,
            ),
            registro(
                2,
                "2026-11-05T09:30:00-03:00",
                "Oficina Exemplo Ltda",
                "11222333000181",
                Situacao::Cancelada,
            ),
            registro(
                3,
                "2026-11-05T11:00:00-03:00",
                "João da Silva",
                "12345678909",
                Situacao::Autorizada,
            ),
            registro(
                4,
                "2026-09-20T08:00:00-03:00",
                "Sem documento",
                "",
                Situacao::Autorizada,
            ),
        ]
    }

    #[test]
    fn a_linha_traz_o_texto_da_tela_com_o_cpf_mascarado_e_o_cnpj_inteiro() {
        let l = ListaDeNotas::nova(&amostra());
        let maria = l
            .linha(l.posicao_da_chave(&amostra()[0].chave).unwrap())
            .unwrap();
        assert_eq!(maria.data, "02/10/2026");
        assert_eq!(maria.numero, "1");
        assert_eq!(maria.dps, "1/1");
        assert_eq!(maria.documento, "***.***.***-25");
        assert_eq!(maria.valor, "1.234,50");
        assert!(
            !maria.documento.contains("529"),
            "o CPF nao aparece inteiro"
        );
        let oficina = l
            .linha(l.posicao_da_chave(&amostra()[1].chave).unwrap())
            .unwrap();
        assert_eq!(oficina.documento, "11.222.333/0001-81");
        assert_eq!(oficina.situacao, Situacao::Cancelada);
        let sem = l
            .linha(l.posicao_da_chave(&amostra()[3].chave).unwrap())
            .unwrap();
        assert_eq!(sem.documento, "");
    }

    #[test]
    fn a_ordem_e_da_mais_recente_para_a_mais_antiga_e_desempata_pela_chave() {
        let l = ListaDeNotas::nova(&amostra());
        let numeros: Vec<&str> = (0..l.quantas())
            .map(|i| l.linha(i).unwrap().numero.as_str())
            .collect();
        // dia 05 (duas notas; a chave maior vem primeiro), depois 02/10, depois 20/09
        assert_eq!(l.linha(0).unwrap().data, "05/11/2026");
        assert_eq!(l.linha(1).unwrap().data, "05/11/2026");
        assert!(l.linha(0).unwrap().chave > l.linha(1).unwrap().chave);
        assert_eq!(numeros[2], "1");
        assert_eq!(numeros[3], "4");
    }

    #[test]
    fn a_busca_acha_por_nome_sem_acento_documento_e_numero() {
        let mut l = ListaDeNotas::nova(&amostra());
        l.definir_busca("joao");
        assert_eq!(l.quantas(), 1);
        assert_eq!(l.linha(0).unwrap().tomador, "João da Silva");
        l.definir_busca("JOÃO silva");
        assert_eq!(l.quantas(), 1);
        l.definir_busca("silva joao");
        assert_eq!(l.quantas(), 1, "a ordem das palavras nao importa");
        // pelo CPF inteiro, com a mascara que a pessoa digitar
        l.definir_busca("529.982.247-25");
        assert_eq!(l.quantas(), 1);
        assert_eq!(l.linha(0).unwrap().tomador, "Maria de Teste");
        // pelo CNPJ
        l.definir_busca("11222333");
        assert_eq!(l.quantas(), 1);
        // pelo numero da nota
        l.definir_busca("3");
        assert!(l.linha(0).is_some());
        // o que nao existe
        l.definir_busca("zzz");
        assert_eq!(l.quantas(), 0);
        assert!(l.sem_resultado());
        assert!(!l.vazia());
        l.definir_busca("");
        assert_eq!(l.quantas(), 4);
        assert!(!l.sem_resultado());
    }

    #[test]
    fn uma_chave_colada_acha_a_nota_e_um_numero_curto_nao_acha_chaves_por_engano() {
        let reg = amostra();
        let mut l = ListaDeNotas::nova(&reg);
        l.definir_busca(&chave::em_grupos(&reg[2].chave));
        assert_eq!(l.quantas(), 1);
        assert_eq!(l.linha(0).unwrap().numero, "3");
        // "55" aparece em quase toda chave, mas a busca curta nao olha a chave
        l.definir_busca("55");
        assert_eq!(l.quantas(), 0);
    }

    #[test]
    fn nome_comprido_com_numero_no_meio_continua_busca_por_nome() {
        let reg = vec![
            registro(
                1,
                "2026-10-02T10:00:00-03:00",
                "Clinica 24 Horas Sao Paulo Centro Ltda",
                "11222333000181",
                Situacao::Autorizada,
            ),
            registro(
                2,
                "2026-10-03T10:00:00-03:00",
                "Maria de Teste",
                "52998224725",
                Situacao::Autorizada,
            ),
        ];
        let mut l = ListaDeNotas::nova(&reg);
        // 25 letras e digitos a partir do "24": nao e' uma chave de acesso, e' o nome
        l.definir_busca("24 horas sao paulo centro ltda");
        assert_eq!(l.quantas(), 1);
        assert_eq!(l.linha(0).unwrap().numero, "1");
        l.definir_busca("Clinica 24 Horas Sao Paulo Centro");
        assert_eq!(l.quantas(), 1);
        // a chave colada (inteira, com ou sem grupos) continua achando a nota
        l.definir_busca(&reg[1].chave);
        assert_eq!(l.quantas(), 1);
        assert_eq!(l.linha(0).unwrap().numero, "2");
    }

    #[test]
    fn o_filtro_de_situacao_combina_com_a_busca() {
        let mut l = ListaDeNotas::nova(&amostra());
        l.definir_filtro(FiltroDeSituacao::Canceladas);
        assert_eq!(l.quantas(), 1);
        assert_eq!(l.linha(0).unwrap().numero, "2");
        l.definir_filtro(FiltroDeSituacao::Autorizadas);
        assert_eq!(l.quantas(), 3);
        l.definir_busca("maria");
        assert_eq!(l.quantas(), 1);
        l.definir_filtro(FiltroDeSituacao::Canceladas);
        assert_eq!(l.quantas(), 0);
        assert!(l.sem_resultado());
        l.definir_filtro(FiltroDeSituacao::Todas);
        l.definir_busca("");
        assert_eq!(l.quantas(), 4);
        assert_eq!(l.contagem(Situacao::Cancelada), 1);
        assert_eq!(l.contagem(Situacao::Autorizada), 3);
        assert_eq!(FiltroDeSituacao::Canceladas.rotulo(), "Canceladas");
        assert_eq!(rotulo_da_situacao(Situacao::Autorizada), "Autorizada");
    }

    #[test]
    fn recarregar_guarda_a_busca_e_o_filtro() {
        let mut l = ListaDeNotas::nova(&amostra());
        l.definir_busca("maria");
        l.definir_filtro(FiltroDeSituacao::Autorizadas);
        let mut mais = amostra();
        mais.push(registro(
            5,
            "2026-11-20T10:00:00-03:00",
            "Maria Nova",
            "52998224725",
            Situacao::Autorizada,
        ));
        let nova = l.recarregada(&mais);
        assert_eq!(nova.busca(), "maria");
        assert_eq!(nova.filtro(), FiltroDeSituacao::Autorizadas);
        assert_eq!(nova.quantas(), 2);
        assert_eq!(nova.total(), 5);
    }

    #[test]
    fn pasta_vazia_e_registro_estranho_nao_derrubam_a_lista() {
        let l = ListaDeNotas::nova(&[]);
        assert!(l.vazia());
        assert_eq!(l.quantas(), 0);
        assert!(l.linha(0).is_none());
        let mut r = registro(9, "x", "Nome", "123", Situacao::Autorizada);
        r.valor_servico = "abc".into();
        let l = ListaDeNotas::nova(&[r]);
        let linha = l.linha(0).unwrap();
        assert_eq!(linha.data, "x");
        assert_eq!(linha.valor, "abc");
        assert_eq!(linha.documento, "***");
    }

    #[test]
    fn duas_mil_notas_buscam_e_filtram_sem_custo_absurdo() {
        let nomes = ["Maria", "Joao", "Ana", "Oficina", "Padaria", "Clinica"];
        let registros: Vec<Registro> = (1..=2000_u64)
            .map(|n| {
                registro(
                    n,
                    &format!("2026-{:02}-{:02}T10:00:00-03:00", 1 + n % 11, 1 + n % 28),
                    &format!("{} {n}", nomes[usize::try_from(n % 6).unwrap()]),
                    "52998224725",
                    if n % 7 == 0 {
                        Situacao::Cancelada
                    } else {
                        Situacao::Autorizada
                    },
                )
            })
            .collect();
        let t0 = Instant::now();
        let mut l = ListaDeNotas::nova(&registros);
        let montar = t0.elapsed();
        assert_eq!(l.total(), 2000);
        let t1 = Instant::now();
        l.definir_busca("maria 12");
        l.definir_filtro(FiltroDeSituacao::Autorizadas);
        l.definir_busca("");
        l.definir_busca("clinica");
        let buscas = t1.elapsed();
        assert!(l.quantas() > 200, "{}", l.quantas());
        // medido em depuracao: bem abaixo do limite; o limite e' folgado de proposito
        assert!(montar.as_millis() < 1500, "montar levou {montar:?}");
        assert!(buscas.as_millis() < 1500, "4 buscas levaram {buscas:?}");
    }
}
