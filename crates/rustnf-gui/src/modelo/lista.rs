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
    /// So' as substituidas.
    Substituidas,
    /// So' as com pedido de analise fiscal em aberto.
    EmAnaliseFiscal,
}

impl FiltroDeSituacao {
    /// Os filtros, na ordem em que a tela os mostra.
    pub const TODOS: [FiltroDeSituacao; 5] = [
        Self::Todas,
        Self::Autorizadas,
        Self::Canceladas,
        Self::Substituidas,
        Self::EmAnaliseFiscal,
    ];

    /// O rotulo do filtro.
    #[must_use]
    pub fn rotulo(self) -> &'static str {
        match self {
            Self::Todas => "Todas",
            Self::Autorizadas => "Autorizadas",
            Self::Canceladas => "Canceladas",
            Self::Substituidas => "Substituidas",
            Self::EmAnaliseFiscal => "Em analise fiscal",
        }
    }

    fn deixa_passar(self, situacao: Situacao) -> bool {
        match self {
            Self::Todas => true,
            Self::Autorizadas => situacao == Situacao::Autorizada,
            Self::Canceladas => situacao == Situacao::Cancelada,
            Self::Substituidas => situacao == Situacao::Substituida,
            Self::EmAnaliseFiscal => situacao == Situacao::AnaliseFiscal,
        }
    }
}

/// O periodo da lista, contado a partir de hoje: o que se pede para fechar o mes com a contabilidade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Periodo {
    /// Sem limite de data.
    #[default]
    Todo,
    /// Do dia 1 do mes de hoje em diante.
    EsteMes,
    /// O mes anterior ao de hoje, inteiro.
    MesPassado,
    /// Do dia 1 de janeiro do ano de hoje em diante.
    EsteAno,
}

impl Periodo {
    /// Os periodos, na ordem em que a tela os mostra.
    pub const TODOS: [Periodo; 4] = [Self::Todo, Self::EsteMes, Self::MesPassado, Self::EsteAno];

    /// O rotulo do periodo.
    #[must_use]
    pub fn rotulo(self) -> &'static str {
        match self {
            Self::Todo => "Todo o período",
            Self::EsteMes => "Este mês",
            Self::MesPassado => "Mês passado",
            Self::EsteAno => "Este ano",
        }
    }

    /// O primeiro e o ultimo dia (`AAAA-MM-DD`, os dois inclusive) do periodo, contado a partir de `hoje`
    /// (`AAAA-MM-DD`). `None` e' sem limite; `hoje` que nao e' uma data deixa o periodo sem limite.
    #[must_use]
    pub fn intervalo(self, hoje: &str) -> (Option<String>, Option<String>) {
        let ano = hoje.get(0..4).and_then(|a| a.parse::<u32>().ok());
        let mes = hoje
            .get(5..7)
            .and_then(|m| m.parse::<u32>().ok())
            .filter(|m| (1..=12).contains(m));
        let (Some(ano), Some(mes)) = (ano, mes) else {
            return (None, None);
        };
        // o fim do mes como dia 31: a comparacao e' de texto, e nenhum dia passa de 31
        let do_mes = |a: u32, m: u32| {
            (
                Some(format!("{a:04}-{m:02}-01")),
                Some(format!("{a:04}-{m:02}-31")),
            )
        };
        match self {
            Self::Todo => (None, None),
            Self::EsteMes => do_mes(ano, mes),
            Self::MesPassado if mes == 1 => do_mes(ano - 1, 12),
            Self::MesPassado => do_mes(ano, mes - 1),
            Self::EsteAno => (
                Some(format!("{ano:04}-01-01")),
                Some(format!("{ano:04}-12-31")),
            ),
        }
    }
}

/// O rotulo da situacao de uma nota.
#[must_use]
pub fn rotulo_da_situacao(situacao: Situacao) -> &'static str {
    match situacao {
        Situacao::Autorizada => "Autorizada",
        Situacao::Cancelada => "Cancelada",
        Situacao::Substituida => "Substituida",
        Situacao::AnaliseFiscal => "Em analise fiscal",
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
    /// O dia da emissao, `AAAA-MM-DD`, para o periodo.
    dia: String,
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
                dia: r.emissao.get(..10).unwrap_or(&r.emissao).to_string(),
            },
        }
    }

    /// O dia da emissao esta no intervalo (as pontas inclusive).
    fn no_periodo(&self, intervalo: &(Option<String>, Option<String>)) -> bool {
        let dia = self.palheiro.dia.as_str();
        intervalo.0.as_deref().is_none_or(|de| dia >= de)
            && intervalo.1.as_deref().is_none_or(|ate| dia <= ate)
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
    /// Os registros do indice, na mesma ordem das linhas (para exportar o que esta na lista).
    registros: Vec<Registro>,
    busca: String,
    filtro: FiltroDeSituacao,
    periodo: Periodo,
    /// Hoje, `AAAA-MM-DD`: a referencia do periodo.
    hoje: String,
    visiveis: Vec<usize>,
}

impl ListaDeNotas {
    /// A lista dos registros do indice, da nota mais recente para a mais antiga (data, depois chave).
    #[must_use]
    pub fn nova(registros: &[Registro]) -> Self {
        let mut ordem: Vec<Registro> = registros.to_vec();
        ordem.sort_by(|a, b| (&b.emissao, &b.chave).cmp(&(&a.emissao, &a.chave)));
        let mut lista = Self {
            linhas: ordem.iter().map(LinhaDeNota::do_registro).collect(),
            registros: ordem,
            ..Self::default()
        };
        lista.refazer();
        lista
    }

    /// A lista nova com a busca, o filtro e o periodo que a antiga tinha (a tela recarrega sem perder o que a pessoa
    /// escolheu).
    #[must_use]
    pub fn recarregada(&self, registros: &[Registro]) -> Self {
        let mut nova = Self::nova(registros);
        nova.busca.clone_from(&self.busca);
        nova.filtro = self.filtro;
        nova.periodo = self.periodo;
        nova.hoje.clone_from(&self.hoje);
        nova.refazer();
        nova
    }

    fn refazer(&mut self) {
        let termo = termo(&self.busca);
        let intervalo = self.periodo.intervalo(&self.hoje);
        self.visiveis = self
            .linhas
            .iter()
            .enumerate()
            .filter(|(_, l)| {
                self.filtro.deixa_passar(l.situacao) && l.no_periodo(&intervalo) && l.casa(&termo)
            })
            .map(|(i, _)| i)
            .collect();
    }

    /// Muda o periodo, contado a partir de `hoje` (`AAAA-MM-DD`).
    pub fn definir_periodo(&mut self, periodo: Periodo, hoje: &str) {
        if self.periodo != periodo || self.hoje != hoje {
            self.periodo = periodo;
            self.hoje = hoje.to_string();
            self.refazer();
        }
    }

    /// O periodo.
    #[must_use]
    pub fn periodo(&self) -> Periodo {
        self.periodo
    }

    /// Os registros do indice das notas que passam pela busca, pelo filtro e pelo periodo, na ordem da lista: o que
    /// "Exportar a lista" grava.
    #[must_use]
    pub fn registros_visiveis(&self) -> Vec<Registro> {
        self.visiveis
            .iter()
            .filter_map(|i| self.registros.get(*i).cloned())
            .collect()
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
    fn as_situacoes_novas_tem_rotulo_e_filtro() {
        let reg = vec![
            registro(1, "2026-10-01", "Ana", "52998224725", Situacao::Autorizada),
            registro(2, "2026-10-02", "Bia", "52998224725", Situacao::Substituida),
            registro(
                3,
                "2026-10-03",
                "Caio",
                "52998224725",
                Situacao::AnaliseFiscal,
            ),
        ];
        let mut l = ListaDeNotas::nova(&reg);
        l.definir_filtro(FiltroDeSituacao::Substituidas);
        assert_eq!((l.quantas(), l.linha(0).unwrap().numero.as_str()), (1, "2"));
        l.definir_filtro(FiltroDeSituacao::EmAnaliseFiscal);
        assert_eq!((l.quantas(), l.linha(0).unwrap().numero.as_str()), (1, "3"));
        l.definir_filtro(FiltroDeSituacao::Autorizadas);
        assert_eq!(l.quantas(), 1);
        assert_eq!(l.contagem(Situacao::Substituida), 1);
        assert_eq!(l.contagem(Situacao::AnaliseFiscal), 1);
        assert_eq!(rotulo_da_situacao(Situacao::Substituida), "Substituida");
        assert_eq!(
            rotulo_da_situacao(Situacao::AnaliseFiscal),
            "Em analise fiscal"
        );
        // todo filtro da tela tem rotulo e a tela mostra todos
        assert_eq!(FiltroDeSituacao::TODOS.len(), 5);
    }

    #[test]
    fn o_periodo_conta_a_partir_de_hoje_e_vira_o_ano() {
        let dia = |s: &str| Some(s.to_string());
        assert_eq!(Periodo::Todo.intervalo("2026-11-21"), (None, None));
        assert_eq!(
            Periodo::EsteMes.intervalo("2026-11-21"),
            (dia("2026-11-01"), dia("2026-11-31"))
        );
        assert_eq!(
            Periodo::MesPassado.intervalo("2026-11-21"),
            (dia("2026-10-01"), dia("2026-10-31"))
        );
        assert_eq!(
            Periodo::MesPassado.intervalo("2027-01-05"),
            (dia("2026-12-01"), dia("2026-12-31")),
            "em janeiro, o mes passado e' dezembro do ano anterior"
        );
        assert_eq!(
            Periodo::EsteAno.intervalo("2026-11-21"),
            (dia("2026-01-01"), dia("2026-12-31"))
        );
        // hoje que nao e' uma data: sem limite, em vez de esconder tudo
        assert_eq!(Periodo::EsteMes.intervalo(""), (None, None));
        assert_eq!(Periodo::EsteMes.intervalo("2026-13-01"), (None, None));
        assert_eq!(Periodo::TODOS[2].rotulo(), "Mês passado");
    }

    #[test]
    fn o_periodo_combina_com_a_busca_e_o_filtro_e_a_exportacao_leva_o_que_passa() {
        // amostra: 02/10 Maria (autorizada), 05/11 Oficina (cancelada), 05/11 Joao (autorizada), 20/09 sem documento
        let mut l = ListaDeNotas::nova(&amostra());
        l.definir_periodo(Periodo::EsteMes, "2026-11-21");
        assert_eq!(l.periodo(), Periodo::EsteMes);
        assert_eq!(l.quantas(), 2);
        l.definir_periodo(Periodo::MesPassado, "2026-11-21");
        assert_eq!(l.quantas(), 1);
        assert_eq!(l.linha(0).unwrap().tomador, "Maria de Teste");
        l.definir_periodo(Periodo::EsteAno, "2026-11-21");
        assert_eq!(l.quantas(), 4);
        // com o filtro e a busca
        l.definir_periodo(Periodo::EsteMes, "2026-11-21");
        l.definir_filtro(FiltroDeSituacao::Autorizadas);
        assert_eq!(l.quantas(), 1);
        assert_eq!(l.linha(0).unwrap().tomador, "João da Silva");
        let exportar = l.registros_visiveis();
        assert_eq!(exportar.len(), 1);
        assert_eq!(exportar[0].tomador_nome, "João da Silva");
        assert_eq!(exportar[0].chave, l.linha(0).unwrap().chave);
        // um mes sem nota: sem resultado, e nada a exportar
        l.definir_filtro(FiltroDeSituacao::Todas);
        l.definir_periodo(Periodo::EsteMes, "2026-12-01");
        assert!(l.sem_resultado());
        assert!(l.registros_visiveis().is_empty());
        // de volta a todo o periodo: tudo, na ordem da lista
        l.definir_periodo(Periodo::Todo, "2026-12-01");
        let todas = l.registros_visiveis();
        assert_eq!(todas.len(), 4);
        for (i, r) in todas.iter().enumerate() {
            assert_eq!(r.chave, l.linha(i).unwrap().chave);
        }
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
        l.definir_periodo(Periodo::EsteAno, "2026-11-21");
        let nova = l.recarregada(&mais);
        assert_eq!(nova.busca(), "maria");
        assert_eq!(nova.filtro(), FiltroDeSituacao::Autorizadas);
        assert_eq!(nova.periodo(), Periodo::EsteAno);
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
