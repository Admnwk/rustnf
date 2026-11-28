// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A lista das NF-e RECEBIDAS (as compras: notas de fornecedor em que a empresa e' a destinataria): uma linha por nota
//! com o texto da tela (data, fornecedor, valor, a situacao da nota e a da resposta da empresa), a busca e o filtro, e
//! "quando foi a ultima busca". Sem janela.
//!
//! A situacao tem duas partes que a pessoa enxerga separadas: o que aconteceu com a NOTA (autorizada, cancelada pelo
//! emitente, denegada) e o que a EMPRESA ja respondeu (nada, ciencia, confirmacao, desconhecimento, operacao nao
//! realizada). O indice guarda a resposta em `Recebida::manifestacao` (o `tpEvento`).

use chrono::{DateTime, FixedOffset, Utc};
use rustnf::nfe::sefaz::evento::{CIENCIA, CONFIRMACAO, DESCONHECIMENTO, NAO_REALIZADA};
use rustnf_app::notas::nfe::Recebida;
use rustnf_app::notas::sem_acento;

use super::certificado::Gravidade;
use super::mascara::{
    decimal_para_dinheiro, documento_para_resumo, iso_para_data_br, limpar_documento, so_digitos,
};
use super::nfe_resultados::contagem;

/// O que aconteceu com a nota (a resposta da empresa e' outra coisa).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SituacaoDaNota {
    /// Autorizada pela SEFAZ.
    Autorizada,
    /// O emitente (o fornecedor) cancelou a nota.
    Cancelada,
    /// A SEFAZ negou o uso da nota.
    Denegada,
}

impl SituacaoDaNota {
    /// O rotulo, em palavras de quem compra.
    #[must_use]
    pub fn rotulo(self) -> &'static str {
        match self {
            Self::Autorizada => "Autorizada",
            Self::Cancelada => "Cancelada pelo emitente",
            Self::Denegada => "Uso negado",
        }
    }

    /// O rotulo curto do selo da lista: numa nota recebida so' o emitente cancela, e o detalhe diz por extenso. Curto
    /// para a coluna do selo nao tomar a largura que o nome do fornecedor precisa.
    #[must_use]
    pub fn rotulo_da_lista(self) -> &'static str {
        match self {
            Self::Cancelada => "Cancelada",
            outra => outra.rotulo(),
        }
    }

    /// A cor do selo.
    #[must_use]
    pub fn gravidade(self) -> Gravidade {
        match self {
            Self::Autorizada => Gravidade::Ok,
            Self::Cancelada | Self::Denegada => Gravidade::Erro,
        }
    }
}

/// O que a empresa ja respondeu sobre a nota.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RespostaDada {
    /// Nada ainda.
    Nenhuma,
    /// "Vi a nota, ainda vou conferir" (ciencia da operacao, 210210).
    Ciencia,
    /// "Recebi a mercadoria, esta certo" (confirmacao da operacao, 210200).
    Confirmada,
    /// "Nao comprei isto" (desconhecimento da operacao, 210220).
    Desconhecida,
    /// "Comprei, mas a venda nao aconteceu" (operacao nao realizada, 210240).
    NaoRealizada,
}

impl RespostaDada {
    /// A resposta que o indice guarda (`tpEvento`; vazio ou desconhecido = nenhuma).
    #[must_use]
    pub fn do_codigo(tp_evento: &str) -> Self {
        match tp_evento {
            CIENCIA => Self::Ciencia,
            CONFIRMACAO => Self::Confirmada,
            DESCONHECIMENTO => Self::Desconhecida,
            NAO_REALIZADA => Self::NaoRealizada,
            _ => Self::Nenhuma,
        }
    }

    /// O rotulo do selo.
    #[must_use]
    pub fn rotulo(self) -> &'static str {
        match self {
            Self::Nenhuma => "Falta responder",
            Self::Ciencia => "Ciência dada",
            Self::Confirmada => "Confirmada",
            Self::Desconhecida => "Desconhecida",
            Self::NaoRealizada => "Não realizada",
        }
    }

    /// A cor do selo: amarelo enquanto falta responder, verde para a confirmacao, cinza para o resto.
    #[must_use]
    pub fn gravidade(self) -> Gravidade {
        match self {
            Self::Nenhuma => Gravidade::Aviso,
            Self::Confirmada => Gravidade::Ok,
            Self::Ciencia | Self::Desconhecida | Self::NaoRealizada => Gravidade::Neutro,
        }
    }

    /// A resposta e' definitiva (todas menos a ciencia: ela e' so' "vi", e a conclusiva vem depois).
    #[must_use]
    pub fn conclusiva(self) -> bool {
        matches!(
            self,
            Self::Confirmada | Self::Desconhecida | Self::NaoRealizada
        )
    }
}

/// O filtro da lista: o que a pessoa precisa fazer, antes de tudo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FiltroRecebidas {
    /// Todas.
    #[default]
    Todas,
    /// As que esperam uma resposta da empresa (autorizadas e ainda sem nenhuma).
    SemResposta,
    /// As em que a empresa so' deu ciencia (falta a resposta final).
    Ciencia,
    /// As que ja tem a resposta final (confirmada, desconhecida ou nao realizada).
    Respondidas,
    /// As canceladas pelo emitente ou de uso negado: nao ha o que responder.
    SemEfeito,
}

impl FiltroRecebidas {
    /// Os filtros, na ordem da tela.
    pub const TODOS: [FiltroRecebidas; 5] = [
        Self::Todas,
        Self::SemResposta,
        Self::Ciencia,
        Self::Respondidas,
        Self::SemEfeito,
    ];

    /// O rotulo do filtro.
    #[must_use]
    pub fn rotulo(self) -> &'static str {
        match self {
            Self::Todas => "Todas",
            Self::SemResposta => "Sem resposta",
            Self::Ciencia => "Só ciência",
            Self::Respondidas => "Respondidas",
            Self::SemEfeito => "Canceladas ou negadas",
        }
    }

    fn deixa_passar(self, l: &LinhaRecebida) -> bool {
        match self {
            Self::Todas => true,
            Self::SemResposta => l.espera_resposta(),
            Self::Ciencia => l.resposta == RespostaDada::Ciencia,
            Self::Respondidas => l.resposta.conclusiva(),
            Self::SemEfeito => l.nota != SituacaoDaNota::Autorizada,
        }
    }
}

/// Uma NF-e recebida da lista, com os textos prontos para a tela.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinhaRecebida {
    /// A chave de acesso (44 posicoes).
    pub chave: String,
    /// A data de emissao, `DD/MM/AAAA`.
    pub data: String,
    /// O nome do fornecedor (vazio se a nota nao o traz).
    pub fornecedor: String,
    /// O documento do fornecedor: CPF mascarado, CNPJ inteiro e formatado.
    pub documento: String,
    /// O valor da nota, em dinheiro brasileiro.
    pub valor: String,
    /// O que aconteceu com a nota.
    pub nota: SituacaoDaNota,
    /// O que a empresa ja respondeu.
    pub resposta: RespostaDada,
    /// A NF-e completa esta na pasta (senao so' o resumo).
    pub completa: bool,
    /// A nota e' de producao.
    pub producao: bool,
    nome: String,
    doc: String,
    dia: String,
}

impl LinhaRecebida {
    /// A linha de um registro do indice.
    #[must_use]
    pub fn do_registro(r: &Recebida) -> Self {
        let nota = if r.cancelada {
            SituacaoDaNota::Cancelada
        } else if r.denegada {
            SituacaoDaNota::Denegada
        } else {
            SituacaoDaNota::Autorizada
        };
        Self {
            chave: r.chave.clone(),
            data: r
                .emissao
                .get(..10)
                .map_or_else(|| r.emissao.clone(), iso_para_data_br),
            fornecedor: r.emitente_nome.trim().to_string(),
            documento: if limpar_documento(&r.emitente).is_empty() {
                String::new()
            } else {
                documento_para_resumo(&r.emitente)
            },
            valor: decimal_para_dinheiro(&r.valor),
            nota,
            resposta: RespostaDada::do_codigo(&r.manifestacao),
            completa: r.completa,
            producao: r.ambiente == "producao",
            nome: sem_acento(&r.emitente_nome),
            doc: limpar_documento(&r.emitente),
            dia: r.emissao.get(..10).unwrap_or(&r.emissao).to_string(),
        }
    }

    /// O dia da emissao, `AAAA-MM-DD`.
    #[must_use]
    pub fn dia(&self) -> &str {
        &self.dia
    }

    /// O selo da resposta: o rotulo e a cor, ou `None` quando nao ha o que mostrar. Nota cancelada pelo emitente ou de
    /// uso negado, que a empresa nunca respondeu, nao tem selo: nao ha o que responder (o aviso da nota e o filtro
    /// "Canceladas ou negadas" ja' dizem isso), e um "falta responder" ali pediria uma acao impossivel. Se a empresa
    /// respondeu antes da nota ser cancelada, a resposta continua sendo um fato e aparece.
    #[must_use]
    pub fn selo_da_resposta(&self) -> Option<(&'static str, Gravidade)> {
        let sem_o_que_responder =
            self.nota != SituacaoDaNota::Autorizada && self.resposta == RespostaDada::Nenhuma;
        (!sem_o_que_responder).then(|| (self.resposta.rotulo(), self.resposta.gravidade()))
    }

    /// A nota espera uma resposta da empresa: esta autorizada e a empresa ainda nao respondeu nada.
    #[must_use]
    pub fn espera_resposta(&self) -> bool {
        self.nota == SituacaoDaNota::Autorizada && self.resposta == RespostaDada::Nenhuma
    }

    fn casa(&self, palavras: &[String], chave: Option<&str>) -> bool {
        if let Some(c) = chave {
            return self.chave.contains(c);
        }
        palavras.iter().all(|p| {
            let digitos = so_digitos(p, 44);
            self.nome.contains(p.as_str()) || (!digitos.is_empty() && self.doc.contains(&digitos))
        })
    }
}

/// A lista das NF-e recebidas da pasta, com a busca e o filtro.
#[derive(Debug, Clone, Default)]
pub struct ListaRecebidas {
    linhas: Vec<LinhaRecebida>,
    busca: String,
    filtro: FiltroRecebidas,
    visiveis: Vec<usize>,
}

impl ListaRecebidas {
    /// A lista dos registros do indice, da nota mais recente para a mais antiga (data, depois chave).
    #[must_use]
    pub fn nova(registros: &[Recebida]) -> Self {
        let mut ordem: Vec<&Recebida> = registros.iter().collect();
        ordem.sort_by(|a, b| (&b.emissao, &b.chave).cmp(&(&a.emissao, &a.chave)));
        let mut l = Self {
            linhas: ordem.into_iter().map(LinhaRecebida::do_registro).collect(),
            ..Self::default()
        };
        l.refazer();
        l
    }

    /// A lista nova com a busca e o filtro que a antiga tinha.
    #[must_use]
    pub fn recarregada(&self, registros: &[Recebida]) -> Self {
        let mut nova = Self::nova(registros);
        nova.busca.clone_from(&self.busca);
        nova.filtro = self.filtro;
        nova.refazer();
        nova
    }

    fn refazer(&mut self) {
        let palavras: Vec<String> = sem_acento(&self.busca)
            .split_whitespace()
            .map(str::to_string)
            .collect();
        let digitos = so_digitos(&self.busca, 44);
        // uma chave colada (ou o comeco dela, 20 posicoes ou mais) acha a nota pela chave
        let chave = (digitos.len() >= 20).then_some(digitos);
        self.visiveis = self
            .linhas
            .iter()
            .enumerate()
            .filter(|(_, l)| self.filtro.deixa_passar(l) && l.casa(&palavras, chave.as_deref()))
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

    /// Muda o filtro.
    pub fn definir_filtro(&mut self, filtro: FiltroRecebidas) {
        if self.filtro != filtro {
            self.filtro = filtro;
            self.refazer();
        }
    }

    /// O filtro.
    #[must_use]
    pub fn filtro(&self) -> FiltroRecebidas {
        self.filtro
    }

    /// Quantas notas ha na pasta.
    #[must_use]
    pub fn total(&self) -> usize {
        self.linhas.len()
    }

    /// Quantas passam pela busca e pelo filtro.
    #[must_use]
    pub fn quantas(&self) -> usize {
        self.visiveis.len()
    }

    /// A pasta nao tem NF-e recebida.
    #[must_use]
    pub fn vazia(&self) -> bool {
        self.linhas.is_empty()
    }

    /// Ha notas, mas nenhuma passa pela busca e pelo filtro.
    #[must_use]
    pub fn sem_resultado(&self) -> bool {
        !self.linhas.is_empty() && self.visiveis.is_empty()
    }

    /// A linha `i` entre as que passam.
    #[must_use]
    pub fn linha(&self, i: usize) -> Option<&LinhaRecebida> {
        self.visiveis.get(i).and_then(|k| self.linhas.get(*k))
    }

    /// A linha da chave, entre todas (passe ou nao pela busca).
    #[must_use]
    pub fn da_chave(&self, chave: &str) -> Option<&LinhaRecebida> {
        self.linhas.iter().find(|l| l.chave == chave)
    }

    /// Quantas notas esperam uma resposta da empresa.
    #[must_use]
    pub fn a_responder(&self) -> usize {
        self.linhas.iter().filter(|l| l.espera_resposta()).count()
    }

    /// "12 de 30 notas" ou "30 notas".
    #[must_use]
    pub fn contagem(&self) -> String {
        let (q, t) = (self.quantas(), self.total());
        if q == t {
            contagem(t, "nota", "notas")
        } else {
            format!("{q} de {}", contagem(t, "nota", "notas"))
        }
    }
}

/// O fuso de Brasilia: o Ambiente Nacional conta a espera em horas, e a frase e' para quem trabalha no horario do pais
/// (sem horario de verao desde 2019; o mesmo fuso fixo que a camada de aplicacao usa em `recebidas`).
fn brasilia() -> Option<FixedOffset> {
    FixedOffset::west_opt(3 * 3600)
}

/// `HH:MM` (horario de Brasilia) de um instante.
#[must_use]
pub fn hora_de_brasilia(t: DateTime<Utc>) -> String {
    brasilia().map_or_else(
        || t.format("%H:%M UTC").to_string(),
        |f| t.with_timezone(&f).format("%H:%M").to_string(),
    )
}

/// `DD/MM/AAAA HH:MM` (horario de Brasilia) de um instante.
#[must_use]
pub fn data_e_hora_de_brasilia(t: DateTime<Utc>) -> String {
    brasilia().map_or_else(
        || t.format("%d/%m/%Y %H:%M UTC").to_string(),
        |f| t.with_timezone(&f).format("%d/%m/%Y %H:%M").to_string(),
    )
}

/// O que a tela diz da fila de busca: a ultima busca e, se o Ambiente Nacional mandou esperar, ate quando.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FilaDaTela {
    /// Quando foi a ultima busca; `None` se nunca buscou.
    pub ultima_busca: Option<DateTime<Utc>>,
    /// Ate quando esperar; `None` se pode buscar.
    pub espera_ate: Option<DateTime<Utc>>,
}

impl FilaDaTela {
    /// "Última busca: 03/10/2026 09:00" ou "Ainda não buscou."
    #[must_use]
    pub fn texto_da_ultima_busca(&self) -> String {
        match self.ultima_busca {
            Some(t) => format!(
                "Última busca: {} (horário de Brasília).",
                data_e_hora_de_brasilia(t)
            ),
            None => "Ainda não buscou notas neste computador.".into(),
        }
    }

    /// O botao fica desligado enquanto a espera vale; `agora` e' o relogio da janela.
    #[must_use]
    pub fn esperando(&self, agora: DateTime<Utc>) -> Option<DateTime<Utc>> {
        self.espera_ate.filter(|ate| *ate > agora)
    }

    /// A frase da espera, para ficar ao lado do botao desligado.
    #[must_use]
    pub fn texto_da_espera(&self, agora: DateTime<Utc>) -> Option<String> {
        self.esperando(agora).map(|ate| {
            format!(
                "O Ambiente Nacional pediu para esperar até {} (horário de Brasília) antes de buscar de novo.",
                hora_de_brasilia(ate)
            )
        })
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use rustnf_app::notas::nfe::SituacaoRecebida;

    fn rec(chave: &str, nome: &str, doc: &str, emissao: &str, manifestacao: &str) -> Recebida {
        Recebida {
            chave: chave.into(),
            emitente: doc.into(),
            emitente_nome: nome.into(),
            emissao: emissao.into(),
            valor: "1234.50".into(),
            ambiente: "homologacao".into(),
            manifestacao: manifestacao.into(),
            situacao: SituacaoRecebida::Resumo,
            ..Recebida::default()
        }
    }

    fn lista() -> ListaRecebidas {
        let mut cancelada = rec(
            &"3".repeat(44),
            "PADARIA SÃO JOÃO LTDA",
            "11222333000181",
            "2026-10-02T08:00:00-03:00",
            "",
        );
        cancelada.cancelada = true;
        ListaRecebidas::nova(&[
            rec(
                &"1".repeat(44),
                "Fornecedor Alfa Ltda",
                "11222333000181",
                "2026-10-03T09:00:00-03:00",
                "",
            ),
            rec(
                &"2".repeat(44),
                "Beta Embalagens",
                "52998224725",
                "2026-10-01T10:00:00-03:00",
                "210210",
            ),
            cancelada,
            rec(
                &"4".repeat(44),
                "Gama Peças",
                "11222333000181",
                "2026-09-30T10:00:00-03:00",
                "210200",
            ),
        ])
    }

    #[test]
    fn a_linha_traz_os_textos_da_tela() {
        let l = lista();
        let a = l.linha(0).unwrap();
        assert_eq!(a.data, "03/10/2026");
        assert_eq!(a.fornecedor, "Fornecedor Alfa Ltda");
        assert_eq!(a.documento, "11.222.333/0001-81");
        assert_eq!(a.valor, "1.234,50");
        assert_eq!(
            (a.nota, a.resposta),
            (SituacaoDaNota::Autorizada, RespostaDada::Nenhuma)
        );
        // o CPF do fornecedor sai mascarado, como nas outras listas
        assert_eq!(l.linha(2).unwrap().documento, "***.***.***-25");
        assert_eq!(l.linha(2).unwrap().resposta.rotulo(), "Ciência dada");
        assert_eq!(l.linha(1).unwrap().nota.rotulo(), "Cancelada pelo emitente");
    }

    #[test]
    fn o_rotulo_da_lista_e_curto_e_o_por_extenso_fica_no_detalhe() {
        assert_eq!(SituacaoDaNota::Cancelada.rotulo_da_lista(), "Cancelada");
        for s in [
            SituacaoDaNota::Autorizada,
            SituacaoDaNota::Cancelada,
            SituacaoDaNota::Denegada,
        ] {
            // a coluna do selo na lista e' medida para "Autorizada"/"Uso negado" (10 letras)
            assert!(s.rotulo_da_lista().chars().count() <= 10, "{s:?}");
        }
        assert_eq!(SituacaoDaNota::Autorizada.rotulo_da_lista(), "Autorizada");
        assert_eq!(SituacaoDaNota::Denegada.rotulo_da_lista(), "Uso negado");
    }

    #[test]
    fn o_selo_da_resposta_so_aparece_onde_ha_o_que_responder() {
        let mut denegada = rec(
            &"5".repeat(44),
            "Delta Ltda",
            "11222333000181",
            "2026-09-29T10:00:00-03:00",
            "",
        );
        denegada.denegada = true;
        let mut respondida_e_cancelada = rec(
            &"6".repeat(44),
            "Épsilon Ltda",
            "11222333000181",
            "2026-09-28T10:00:00-03:00",
            "210200",
        );
        respondida_e_cancelada.cancelada = true;
        let l = ListaRecebidas::nova(&[
            rec(
                &"1".repeat(44),
                "Alfa",
                "11222333000181",
                "2026-10-03T09:00:00-03:00",
                "",
            ),
            denegada,
            respondida_e_cancelada,
        ]);
        // autorizada sem resposta: rotulo curto, de acao, amarelo
        assert_eq!(
            l.da_chave(&"1".repeat(44)).unwrap().selo_da_resposta(),
            Some(("Falta responder", Gravidade::Aviso))
        );
        // cancelada (lista de teste) e denegada sem resposta: nenhum selo
        assert_eq!(lista().linha(1).unwrap().selo_da_resposta(), None);
        assert_eq!(
            l.da_chave(&"5".repeat(44)).unwrap().selo_da_resposta(),
            None
        );
        // a resposta que a empresa ja deu continua aparecendo, mesmo na nota depois cancelada
        assert_eq!(
            l.da_chave(&"6".repeat(44)).unwrap().selo_da_resposta(),
            Some(("Confirmada", Gravidade::Ok))
        );
        // o aviso "N notas esperam resposta" conta so' as autorizadas
        assert_eq!(l.a_responder(), 1);
    }

    #[test]
    fn mais_recente_primeiro() {
        let l = lista();
        let dias: Vec<&str> = (0..l.quantas())
            .map(|i| l.linha(i).unwrap().dia())
            .collect();
        assert_eq!(
            dias,
            ["2026-10-03", "2026-10-02", "2026-10-01", "2026-09-30"]
        );
    }

    #[test]
    fn o_filtro_separa_o_que_falta_responder() {
        let mut l = lista();
        l.definir_filtro(FiltroRecebidas::SemResposta);
        assert_eq!(l.quantas(), 1, "a cancelada nao espera resposta");
        assert_eq!(l.a_responder(), 1);
        l.definir_filtro(FiltroRecebidas::Ciencia);
        assert_eq!(l.quantas(), 1);
        l.definir_filtro(FiltroRecebidas::Respondidas);
        assert_eq!(l.quantas(), 1);
        l.definir_filtro(FiltroRecebidas::SemEfeito);
        assert_eq!(l.quantas(), 1);
        l.definir_filtro(FiltroRecebidas::Todas);
        assert_eq!(l.quantas(), 4);
    }

    #[test]
    fn a_busca_acha_por_nome_sem_acento_documento_e_chave() {
        let mut l = lista();
        l.definir_busca("sao joao");
        assert_eq!(l.quantas(), 1);
        l.definir_busca("padaria são");
        assert_eq!(l.quantas(), 1);
        l.definir_busca("11.222.333");
        assert_eq!(l.quantas(), 3);
        l.definir_busca(&"2".repeat(30));
        assert_eq!(l.quantas(), 1);
        l.definir_busca("nada disso");
        assert!(l.sem_resultado());
        assert_eq!(l.contagem(), "0 de 4 notas");
        l.definir_busca("");
        assert_eq!(l.contagem(), "4 notas");
    }

    #[test]
    fn contagem_no_singular_e_recarregar_guarda_busca_e_filtro() {
        let r = [rec(
            &"1".repeat(44),
            "Alfa",
            "11222333000181",
            "2026-10-03T09:00:00-03:00",
            "",
        )];
        assert_eq!(ListaRecebidas::nova(&r).contagem(), "1 nota");
        let mut l = lista();
        l.definir_filtro(FiltroRecebidas::Respondidas);
        l.definir_busca("gama");
        let outra = l.recarregada(&[rec(
            &"4".repeat(44),
            "Gama Peças",
            "11222333000181",
            "2026-09-30T10:00:00-03:00",
            "210200",
        )]);
        assert_eq!(outra.filtro(), FiltroRecebidas::Respondidas);
        assert_eq!(outra.quantas(), 1);
    }

    #[test]
    fn pasta_grande_filtra_sem_travar() {
        let registros: Vec<Recebida> = (0..2000)
            .map(|i| {
                rec(
                    &format!("{i:044}"),
                    &format!("FORNECEDOR {i}"),
                    "11222333000181",
                    "2026-10-03T09:00:00-03:00",
                    "",
                )
            })
            .collect();
        let mut l = ListaRecebidas::nova(&registros);
        assert_eq!(l.total(), 2000);
        l.definir_busca("fornecedor 19");
        let esperadas = (0..2000)
            .filter(|i: &u32| i.to_string().contains("19"))
            .count();
        assert_eq!(l.quantas(), esperadas);
    }

    #[test]
    fn a_espera_so_vale_ate_a_hora_e_mostra_o_horario_de_brasilia() {
        let ate: DateTime<Utc> = "2026-10-03T13:00:00Z".parse().unwrap();
        let f = FilaDaTela {
            ultima_busca: Some("2026-10-03T12:00:00Z".parse().unwrap()),
            espera_ate: Some(ate),
        };
        let antes: DateTime<Utc> = "2026-10-03T12:30:00Z".parse().unwrap();
        assert!(f.esperando(antes).is_some());
        assert!(f.texto_da_espera(antes).unwrap().contains("10:00"));
        assert!(f.esperando(ate).is_none(), "na hora exata ja pode");
        assert_eq!(
            f.texto_da_ultima_busca(),
            "Última busca: 03/10/2026 09:00 (horário de Brasília)."
        );
        assert!(FilaDaTela::default()
            .texto_da_ultima_busca()
            .starts_with("Ainda não"));
    }
}
