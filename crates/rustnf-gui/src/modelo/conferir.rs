// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! "Conferir situacao na Sefin": o lote que confere as notas que a lista mostra. Aqui ficam a conta do tempo (e a
//! regra de pedir confirmacao com muitas notas), o texto do andamento e o resultado em linguagem simples: o que
//! mudou (de -> para), quantas estavam iguais e, de cada falha, o que aconteceu e o que fazer. Sem janela.

use std::time::Duration;

use rustnf_app::atualizacao::Relatorio;
use rustnf_app::notas::{Registro, Situacao};

use super::lista::{rotulo_da_situacao, LinhaDeNota};
use super::mascara::frase;

/// Acima de quantas notas a tela diz quanto tempo leva e pede confirmacao antes de comecar.
pub const LIMITE_SEM_CONFIRMAR: usize = 50;

/// Quantas consultas a Sefin recebe, no minimo, por nota: os quatro eventos que decidem a situacao (cancelamento,
/// cancelamento deferido, substituicao e pedido de analise fiscal) e a consulta da nota. O que a Sefin demora para
/// responder nao entra na conta.
pub const CONSULTAS_POR_NOTA: u32 = 5;

/// Com `total` notas e este intervalo entre as consultas, e' preciso confirmar antes de comecar.
#[must_use]
pub fn precisa_confirmar(total: usize) -> bool {
    total > LIMITE_SEM_CONFIRMAR
}

/// O tempo minimo do lote: so' as esperas entre as consultas.
#[must_use]
pub fn tempo_minimo(total: usize, intervalo: Duration) -> Duration {
    let consultas = u32::try_from(total)
        .unwrap_or(u32::MAX)
        .saturating_mul(CONSULTAS_POR_NOTA);
    intervalo.saturating_mul(consultas.saturating_sub(1))
}

/// O tempo, em palavras: "menos de 1 minuto", "cerca de 4 minutos".
#[must_use]
pub fn texto_do_tempo(tempo: Duration) -> String {
    let minutos = tempo.as_secs().div_ceil(60);
    match minutos {
        0 | 1 => "menos de 1 minuto".into(),
        n => format!("cerca de {n} minutos"),
    }
}

/// A frase que pede a confirmacao de um lote grande.
#[must_use]
pub fn pergunta_de_confirmacao(total: usize, intervalo: Duration) -> String {
    format!(
        "Vou conferir {total} notas, uma de cada vez, com {} ms entre as consultas. Isso leva pelo menos {}, e mais se a Sefin demorar a responder. Pode parar quando quiser: o que já foi conferido fica gravado.",
        intervalo.as_millis(),
        texto_do_tempo(tempo_minimo(total, intervalo))
    )
}

/// Onde o lote esta: quantas notas ja foram conferidas e quantas ha.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Andamento {
    /// Quantas notas ja foram conferidas.
    pub feitas: usize,
    /// Quantas notas o lote tem.
    pub total: usize,
}

impl Andamento {
    /// "Conferindo 12 de 40..." (a nota que esta sendo conferida e' a de numero `feitas + 1`).
    #[must_use]
    pub fn texto(self) -> String {
        format!(
            "Conferindo {} de {}...",
            (self.feitas + 1).min(self.total),
            self.total
        )
    }

    /// A fracao concluida, de 0 a 1, para a barra.
    #[must_use]
    pub fn fracao(self) -> f32 {
        if self.total == 0 {
            return 0.0;
        }
        let feitas = u32::try_from(self.feitas.min(self.total)).unwrap_or(u32::MAX);
        let total = u32::try_from(self.total).unwrap_or(u32::MAX);
        feitas as f32 / total as f32
    }
}

/// Uma nota que mudou de situacao.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotaQueMudou {
    /// A chave de acesso.
    pub chave: String,
    /// O numero da NFS-e.
    pub numero: String,
    /// O tomador.
    pub tomador: String,
    /// A situacao que a pasta tinha.
    pub de: Situacao,
    /// A que a Sefin mostra.
    pub para: Situacao,
}

impl NotaQueMudou {
    /// "nº 12, Maria Souza: de Autorizada para Cancelada".
    #[must_use]
    pub fn texto(&self) -> String {
        let quem = if self.tomador.is_empty() {
            String::new()
        } else {
            format!(", {}", self.tomador)
        };
        format!(
            "nº {}{quem}: de {} para {}",
            self.numero,
            rotulo_da_situacao(self.de),
            rotulo_da_situacao(self.para)
        )
    }
}

/// Uma nota que nao deu para conferir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotaComFalha {
    /// O numero da NFS-e.
    pub numero: String,
    /// O tomador.
    pub tomador: String,
    /// O que aconteceu, em portugues simples.
    pub o_que_houve: String,
    /// O que fazer.
    pub o_que_fazer: String,
}

/// O que aconteceu com uma nota e o que fazer, a partir do motivo que a camada de aplicacao deu.
#[must_use]
pub fn explicar_falha(motivo: &str) -> (String, String) {
    let m = motivo.trim();
    if let Some(resto) = m.strip_prefix("divergencia: a pasta diz ") {
        let na_pasta = resto.split(" e a Sefin").next().unwrap_or("");
        let na_pasta = match na_pasta {
            "substituida" => "substituída",
            outra => outra,
        };
        let falta = if resto.contains("o pedido") {
            "o pedido de análise fiscal"
        } else {
            "o cancelamento nem a substituição"
        };
        return (
            format!("A pasta diz que a nota está {na_pasta}, e a Sefin não mostra {falta}."),
            "Abra a nota e confira antes de mudar qualquer coisa: a pasta não foi alterada.".into(),
        );
    }
    if m.starts_with("a Sefin nao encontrou a nota") {
        return (
            "A Sefin não encontrou esta nota neste ambiente.".into(),
            "Confira se o ambiente das Configurações (produção ou produção restrita) é o da nota."
                .into(),
        );
    }
    if m.starts_with("a Sefin respondeu com a nota de outra chave") {
        return (
            "A Sefin respondeu com uma nota diferente da pedida.".into(),
            "Não use esta resposta: abra a nota e confira a chave.".into(),
        );
    }
    if m.starts_with("nao foi possivel gravar") {
        return (
            "A Sefin respondeu, mas a pasta de notas não pôde ser atualizada.".into(),
            "Veja se a pasta está acessível e confira de novo.".into(),
        );
    }
    (
        "A Sefin não respondeu a esta nota.".into(),
        "Tente de novo mais tarde: conferir só consulta, não altera nada na Sefin.".into(),
    )
}

/// O que o lote fez, como a tela mostra.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResultadoDoLote {
    /// Quantas notas o lote tinha.
    pub total: usize,
    /// Quantas foram conferidas (menos que `total` se parou antes).
    pub conferidas: usize,
    /// A pessoa parou o lote.
    pub parado: bool,
    /// As que mudaram.
    pub mudaram: Vec<NotaQueMudou>,
    /// Quantas estavam como a Sefin mostra.
    pub iguais: usize,
    /// As que nao deu para conferir.
    pub falhas: Vec<NotaComFalha>,
}

impl ResultadoDoLote {
    /// O resultado de um `Relatorio`; `registros` sao as notas do lote (para pôr numero e tomador nas linhas).
    #[must_use]
    pub fn de(relatorio: &Relatorio, parado: bool, registros: &[Registro]) -> Self {
        let linha = |chave: &str| {
            registros
                .iter()
                .find(|r| r.chave == chave)
                .map(LinhaDeNota::do_registro)
        };
        let da_nota = |chave: &str| {
            linha(chave).map_or_else(
                || (chave.chars().take(12).collect::<String>(), String::new()),
                |l| (l.numero, l.tomador),
            )
        };
        Self {
            total: registros.len(),
            conferidas: relatorio.conferidas,
            parado,
            mudaram: relatorio
                .mudaram
                .iter()
                .map(|m| {
                    let (numero, tomador) = da_nota(&m.chave);
                    NotaQueMudou {
                        chave: m.chave.clone(),
                        numero,
                        tomador,
                        de: m.de,
                        para: m.para,
                    }
                })
                .collect(),
            iguais: relatorio.iguais,
            falhas: relatorio
                .falhas
                .iter()
                .map(|f| {
                    let (numero, tomador) = da_nota(&f.chave);
                    let (o_que_houve, o_que_fazer) = explicar_falha(&f.motivo);
                    NotaComFalha {
                        numero,
                        tomador,
                        o_que_houve,
                        o_que_fazer,
                    }
                })
                .collect(),
        }
    }

    /// A frase de cima: quantas mudaram, quantas estavam iguais, quantas falharam.
    #[must_use]
    pub fn resumo(&self) -> String {
        let notas = |n: usize| {
            if n == 1 {
                "1 nota".to_string()
            } else {
                format!("{n} notas")
            }
        };
        let mut partes = Vec::new();
        partes.push(match self.mudaram.len() {
            0 => "nenhuma mudou de situação".to_string(),
            1 => "1 mudou de situação".to_string(),
            n => format!("{n} mudaram de situação"),
        });
        partes.push(match self.iguais {
            0 => "nenhuma estava igual".to_string(),
            1 => "1 já estava igual à Sefin".to_string(),
            n => format!("{n} já estavam iguais à Sefin"),
        });
        if !self.falhas.is_empty() {
            partes.push(match self.falhas.len() {
                1 => "1 não deu para conferir".to_string(),
                n => format!("{n} não deu para conferir"),
            });
        }
        let inicio = if self.parado {
            format!(
                "Parei depois de {} de {}. O que já foi conferido ficou gravado.",
                self.conferidas,
                notas(self.total)
            )
        } else {
            format!("Conferi {}.", notas(self.conferidas))
        };
        format!("{inicio} {}.", frase(&partes.join("; ")))
    }

    /// Alguma coisa mudou na pasta: a lista deve ser lida de novo.
    #[must_use]
    pub fn mudou_a_pasta(&self) -> bool {
        !self.mudaram.is_empty()
    }
}

/// Como o lote terminou.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DesfechoDoLote {
    /// O lote rodou (inteiro, ou ate a pessoa parar).
    Feito(Box<ResultadoDoLote>),
    /// Nao deu para comecar (certificado, senha, pasta): nada foi consultado.
    Recusado {
        /// A frase para a tela.
        texto: String,
    },
}

/// O que a tela diz quando nao ha nota na lista para conferir.
pub const TEXTO_SEM_NOTAS: &str = "Não há nota na lista para conferir. Mude a busca ou o filtro.";

#[cfg(test)]
mod testes {
    use rustnf_app::atualizacao::{Falha, Mudanca};

    use super::*;

    fn registro(chave: &str, numero: &str, tomador: &str) -> Registro {
        Registro {
            chave: chave.into(),
            numero: numero.into(),
            serie: "1".into(),
            numero_dps: numero.into(),
            emissao: "2026-11-20".into(),
            valor_servico: "10.00".into(),
            situacao: Situacao::Autorizada,
            tomador_documento: String::new(),
            tomador_nome: tomador.into(),
        }
    }

    #[test]
    fn mais_de_cinquenta_notas_pedem_confirmacao() {
        assert!(!precisa_confirmar(50));
        assert!(precisa_confirmar(51));
        assert!(!precisa_confirmar(0));
    }

    #[test]
    fn o_tempo_minimo_e_so_a_espera_entre_as_consultas() {
        let i = Duration::from_millis(200);
        // 1 nota: 5 consultas, 4 esperas
        assert_eq!(tempo_minimo(1, i), Duration::from_millis(800));
        // 60 notas: 300 consultas, 299 esperas de 200 ms
        assert_eq!(tempo_minimo(60, i), Duration::from_millis(59_800));
        assert_eq!(tempo_minimo(0, i), Duration::ZERO);
        assert_eq!(tempo_minimo(200, Duration::ZERO), Duration::ZERO);
    }

    #[test]
    fn o_tempo_em_palavras() {
        assert_eq!(texto_do_tempo(Duration::from_secs(5)), "menos de 1 minuto");
        assert_eq!(texto_do_tempo(Duration::from_secs(60)), "menos de 1 minuto");
        assert_eq!(
            texto_do_tempo(Duration::from_secs(61)),
            "cerca de 2 minutos"
        );
        assert_eq!(
            texto_do_tempo(Duration::from_secs(600)),
            "cerca de 10 minutos"
        );
    }

    #[test]
    fn a_pergunta_diz_quantas_notas_quanto_tempo_e_que_da_para_parar() {
        let t = pergunta_de_confirmacao(120, Duration::from_millis(200));
        assert!(t.contains("120 notas"), "{t}");
        assert!(t.contains("200 ms"), "{t}");
        assert!(t.contains("cerca de 2 minutos"), "{t}");
        assert!(t.contains("Pode parar"), "{t}");
    }

    #[test]
    fn o_andamento_conta_a_nota_que_esta_sendo_conferida() {
        assert_eq!(
            Andamento {
                feitas: 11,
                total: 40
            }
            .texto(),
            "Conferindo 12 de 40..."
        );
        assert_eq!(
            Andamento {
                feitas: 40,
                total: 40
            }
            .texto(),
            "Conferindo 40 de 40..."
        );
        let meio = Andamento {
            feitas: 10,
            total: 40,
        };
        assert!((meio.fracao() - 0.25).abs() < 1e-6);
        assert!(Andamento::default().fracao().abs() < 1e-6);
    }

    #[test]
    fn cada_tipo_de_falha_tem_o_texto_certo() {
        let (houve, fazer) = explicar_falha(
            "divergencia: a pasta diz cancelada e a Sefin nao mostra o cancelamento nem a substituicao; confira a nota",
        );
        assert_eq!(
            houve,
            "A pasta diz que a nota está cancelada, e a Sefin não mostra o cancelamento nem a substituição."
        );
        assert!(fazer.contains("Abra a nota e confira"));
        let (houve, _) = explicar_falha(
            "divergencia: a pasta diz em analise fiscal e a Sefin nao mostra o pedido; confira a nota",
        );
        assert!(
            houve.contains("em analise fiscal") && houve.contains("o pedido de análise fiscal")
        );
        let (houve, _) = explicar_falha(
            "divergencia: a pasta diz substituida e a Sefin nao mostra o cancelamento nem a substituicao; confira a nota",
        );
        assert!(houve.contains("substituída"), "{houve}");
        let (houve, fazer) = explicar_falha("a Sefin nao encontrou a nota");
        assert!(houve.contains("não encontrou") && fazer.contains("ambiente"));
        let (houve, _) = explicar_falha("a Sefin respondeu com a nota de outra chave");
        assert!(houve.contains("diferente"));
        let (houve, _) = explicar_falha("nao foi possivel gravar a situacao na pasta de notas");
        assert!(houve.contains("pasta de notas"));
        let (houve, fazer) = explicar_falha("evento 101101: Sem resposta");
        assert_eq!(houve, "A Sefin não respondeu a esta nota.");
        assert!(fazer.contains("mais tarde"));
        // nenhum texto da tela leva o codigo do evento nem a palavra "divergencia" crua
        let (houve, fazer) = explicar_falha("consulta da nota: Falha");
        assert!(!houve.contains("101") && !fazer.contains("consulta da nota"));
    }

    #[test]
    fn o_resultado_diz_o_que_mudou_de_para_quantas_estavam_iguais_e_cada_falha() {
        let regs = vec![
            registro("A", "10", "Maria Souza"),
            registro("B", "11", "Joao Lima"),
            registro("C", "12", ""),
            registro("D", "13", "Ana"),
        ];
        let relatorio = Relatorio {
            conferidas: 4,
            mudaram: vec![Mudanca {
                chave: "A".into(),
                de: Situacao::Autorizada,
                para: Situacao::Cancelada,
            }],
            iguais: 2,
            falhas: vec![Falha {
                chave: "C".into(),
                motivo: "a Sefin nao encontrou a nota".into(),
            }],
        };
        let r = ResultadoDoLote::de(&relatorio, false, &regs);
        assert_eq!(
            r.mudaram[0].texto(),
            "nº 10, Maria Souza: de Autorizada para Cancelada"
        );
        assert_eq!(r.iguais, 2);
        assert_eq!(r.falhas[0].numero, "12");
        assert!(r.falhas[0].o_que_houve.contains("não encontrou"));
        assert_eq!(
            r.resumo(),
            "Conferi 4 notas. 1 mudou de situação; 2 já estavam iguais à Sefin; 1 não deu para conferir."
        );
        assert!(r.mudou_a_pasta());
    }

    #[test]
    fn lote_parado_diz_onde_parou_e_que_o_conferido_ficou_gravado() {
        let regs: Vec<Registro> = (0..40)
            .map(|i| registro(&format!("K{i}"), &i.to_string(), "T"))
            .collect();
        let relatorio = Relatorio {
            conferidas: 12,
            iguais: 12,
            ..Relatorio::default()
        };
        let r = ResultadoDoLote::de(&relatorio, true, &regs);
        assert!(r.parado);
        assert_eq!(
            r.resumo(),
            "Parei depois de 12 de 40 notas. O que já foi conferido ficou gravado. Nenhuma mudou de situação; 12 já estavam iguais à Sefin."
        );
        assert!(!r.mudou_a_pasta());
    }
}
