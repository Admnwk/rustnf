// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! As regras do dialogo de responder a uma NF-e recebida (a manifestacao do destinatario): as quatro respostas em palavras
//! de quem compra e quando usar cada uma, o que o Ambiente Nacional recusaria (decidido ANTES de enviar, pela mesma funcao
//! da biblioteca que a camada de aplicacao usa), a justificativa e a marcacao de producao. Sem janela.
//!
//! Fontes (nenhuma regra nova aqui, so' o que a biblioteca ja decide):
//! * os quatro eventos, a sequencia 1 e "vale o ultimo": `crates/rustnf/src/nfe/sefaz/evento/manifestacao.rs:5-13`
//!   (MOC 7.00, Visao Geral, 3.1.2 e 3.2; 5.11);
//! * a justificativa de 15 a 255 caracteres, obrigatoria so' na operacao nao realizada: `manifestacao.rs:14-19` e
//!   `:155-176`;
//! * as combinacoes recusadas (650, 651, 655, 573): `manifestacao.rs:112-141` (`conferir_combinacao`);
//! * o PRAZO da resposta nao e' conferido por este programa (`manifestacao.rs:20-22`): nenhum prazo aparece aqui.

use rustnf::nfe::sefaz::dfe::SituacaoResumo;
use rustnf::nfe::sefaz::evento::{conferir_combinacao, TipoManifestacao};
use rustnf_app::notas::nfe::Recebida;

use super::cancelamento::justificativa_normal;
use super::nfe_eventos::{JUSTIFICATIVA_MAX, JUSTIFICATIVA_MIN};
use super::nfe_recebidas::{LinhaRecebida, RespostaDada, SituacaoDaNota};

/// As quatro respostas, na ordem do dialogo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resposta {
    /// "Recebi a mercadoria, esta certo": confirmacao da operacao (210200).
    Confirmar,
    /// "Vi a nota, ainda vou conferir": ciencia da operacao (210210).
    Ciencia,
    /// "Nao comprei isto": desconhecimento da operacao (210220).
    Desconhecer,
    /// "Comprei, mas a venda nao aconteceu": operacao nao realizada (210240).
    NaoRealizada,
}

impl Resposta {
    /// As respostas, na ordem do dialogo.
    pub const TODAS: [Resposta; 4] = [
        Self::Confirmar,
        Self::Ciencia,
        Self::Desconhecer,
        Self::NaoRealizada,
    ];

    /// O evento da biblioteca.
    #[must_use]
    pub fn tipo(self) -> TipoManifestacao {
        match self {
            Self::Confirmar => TipoManifestacao::Confirmacao,
            Self::Ciencia => TipoManifestacao::Ciencia,
            Self::Desconhecer => TipoManifestacao::Desconhecimento,
            Self::NaoRealizada => TipoManifestacao::NaoRealizada,
        }
    }

    /// A resposta de um evento da biblioteca.
    #[must_use]
    pub fn do_tipo(t: TipoManifestacao) -> Option<Self> {
        Self::TODAS.into_iter().find(|r| r.tipo() == t)
    }

    /// A frase da opcao, nas palavras de quem compra.
    #[must_use]
    pub fn rotulo(self) -> &'static str {
        match self {
            Self::Confirmar => "Recebi a mercadoria, está certo",
            Self::Ciencia => "Vi a nota, ainda vou conferir",
            Self::Desconhecer => "Não comprei isto",
            Self::NaoRealizada => "Comprei, mas a venda não aconteceu",
        }
    }

    /// Quando usar esta resposta.
    #[must_use]
    pub fn quando_usar(self) -> &'static str {
        match self {
            Self::Confirmar => "Use quando a mercadoria chegou (ou o serviço foi feito) e confere com a nota. Chama-se confirmação da operação.",
            Self::Ciencia => "Use quando você já viu que a nota existe, mas ainda não conferiu a compra. É só um aviso de que você a viu (ciência da operação); depois ainda dá para dar a resposta final. É também o que libera a nota completa para baixar.",
            Self::Desconhecer => "Use quando a sua empresa não fez esta compra: nota em engano ou indevida (desconhecimento da operação). A SEFAZ não entrega a nota completa depois dessa resposta.",
            Self::NaoRealizada => "Use quando a compra foi combinada, mas a venda não aconteceu: a mercadoria não chegou ou foi devolvida (operação não realizada). Pede uma justificativa.",
        }
    }

    /// Esta resposta pede justificativa (so' a operacao nao realizada).
    #[must_use]
    pub fn pede_justificativa(self) -> bool {
        self == Self::NaoRealizada
    }

    /// O texto do botao de enviar.
    #[must_use]
    pub fn enviar(self, producao: bool) -> &'static str {
        match (self, producao) {
            (Self::Confirmar, false) => "Confirmar a operação",
            (Self::Confirmar, true) => "Confirmar a operação em produção",
            (Self::Ciencia, false) => "Dar ciência",
            (Self::Ciencia, true) => "Dar ciência em produção",
            (Self::Desconhecer, false) => "Desconhecer a operação",
            (Self::Desconhecer, true) => "Desconhecer em produção",
            (Self::NaoRealizada, false) => "Registrar operação não realizada",
            (Self::NaoRealizada, true) => "Registrar não realizada em produção",
        }
    }

    /// O texto de "enviando".
    #[must_use]
    pub fn enviando(self) -> &'static str {
        match self {
            Self::Confirmar => "Enviando a confirmação à SEFAZ...",
            Self::Ciencia => "Enviando a ciência à SEFAZ...",
            Self::Desconhecer => "Enviando o desconhecimento à SEFAZ...",
            Self::NaoRealizada => "Enviando a operação não realizada à SEFAZ...",
        }
    }

    /// A mensagem de sucesso.
    #[must_use]
    pub fn registrada(self) -> &'static str {
        match self {
            Self::Confirmar => "A SEFAZ registrou a confirmação da operação.",
            Self::Ciencia => {
                "A SEFAZ registrou a ciência da operação. Agora a nota completa pode ser baixada."
            }
            Self::Desconhecer => "A SEFAZ registrou o desconhecimento da operação.",
            Self::NaoRealizada => "A SEFAZ registrou que a operação não foi realizada.",
        }
    }
}

/// O efeito fiscal, em uma frase para quem nao e' contador.
pub const EFEITO_DA_RESPOSTA: &str = "A resposta fica registrada na SEFAZ, em nome da sua empresa (é um evento fiscal) e não se apaga. Se mudar de ideia, dá para registrar outra resposta final: vale a última.";

/// A marcacao que a producao exige antes de habilitar o botao.
pub const MARCACAO_DE_PRODUCAO: &str = "Entendo que a resposta tem efeito fiscal e não tem volta";

/// O que o botao desligado diz quando falta a marcacao de producao.
pub const FALTA_MARCAR: &str = "Em produção, marque que você entende o efeito fiscal da resposta.";

/// A nota de que o dialogo trata, com o que ele mostra e o que a aplicacao precisa saber para recusar antes de enviar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlvoDaResposta {
    /// A chave de acesso.
    pub chave: String,
    /// O fornecedor.
    pub fornecedor: String,
    /// O valor, em dinheiro brasileiro.
    pub valor: String,
    /// A data de emissao, `DD/MM/AAAA`.
    pub data: String,
    /// O que aconteceu com a nota.
    pub nota: SituacaoDaNota,
    /// O que a empresa ja respondeu.
    pub resposta_dada: RespostaDada,
    /// A NF-e completa esta na pasta.
    pub completa: bool,
}

impl AlvoDaResposta {
    /// O alvo de uma NF-e recebida do indice.
    #[must_use]
    pub fn da_recebida(r: &Recebida) -> Self {
        let l = LinhaRecebida::do_registro(r);
        Self {
            chave: l.chave,
            fornecedor: l.fornecedor,
            valor: l.valor,
            data: l.data,
            nota: l.nota,
            resposta_dada: l.resposta,
            completa: l.completa,
        }
    }

    fn anterior(&self) -> Option<TipoManifestacao> {
        match self.resposta_dada {
            RespostaDada::Nenhuma => None,
            RespostaDada::Ciencia => Some(TipoManifestacao::Ciencia),
            RespostaDada::Confirmada => Some(TipoManifestacao::Confirmacao),
            RespostaDada::Desconhecida => Some(TipoManifestacao::Desconhecimento),
            RespostaDada::NaoRealizada => Some(TipoManifestacao::NaoRealizada),
        }
    }

    fn situacao_nfe(&self) -> Option<SituacaoResumo> {
        match self.nota {
            SituacaoDaNota::Autorizada => None,
            SituacaoDaNota::Cancelada => Some(SituacaoResumo::Cancelada),
            SituacaoDaNota::Denegada => Some(SituacaoResumo::Denegada),
        }
    }

    /// Por que o Ambiente Nacional recusaria esta resposta para esta nota, em palavras (e `None` se aceitaria). A decisao
    /// e' da biblioteca (`conferir_combinacao`, a mesma que a camada de aplicacao aplica antes de enviar); o texto so'
    /// diz qual das regras pegou.
    #[must_use]
    pub fn recusada(&self, r: Resposta) -> Option<&'static str> {
        let tipo = r.tipo();
        conferir_combinacao(tipo, self.situacao_nfe().as_ref(), self.anterior()).err()?;
        let encerrada = self.nota != SituacaoDaNota::Autorizada;
        Some(match r {
            Resposta::Ciencia | Resposta::Desconhecer if encerrada => {
                "A nota foi cancelada pelo emitente (ou o uso foi negado): para ela não vale dar ciência nem desconhecer."
            }
            Resposta::Ciencia if self.resposta_dada.conclusiva() => {
                "Você já deu a resposta final a esta nota: a ciência vem antes dela, não depois."
            }
            _ => "Esta resposta já está registrada para esta nota.",
        })
    }

    /// A resposta que a pessoa tem, de saida, para sugerir: a ciencia se ainda nao respondeu nada, senao nenhuma.
    #[must_use]
    pub fn respostas_possiveis(&self) -> Vec<Resposta> {
        Resposta::TODAS
            .into_iter()
            .filter(|r| self.recusada(*r).is_none())
            .collect()
    }
}

/// O que a pessoa preencheu no dialogo.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FormularioDaResposta {
    /// A resposta escolhida.
    pub escolha: Option<Resposta>,
    /// A justificativa, como digitada.
    pub justificativa: String,
    /// Em producao: "Entendo que...".
    pub efeito_fiscal_entendido: bool,
}

fn contar(texto: &str) -> usize {
    justificativa_normal(texto).chars().count()
}

impl FormularioDaResposta {
    /// `12 caracteres (de 15 a 255)`.
    #[must_use]
    pub fn contador(&self) -> String {
        format!(
            "{} caracteres (de {JUSTIFICATIVA_MIN} a {JUSTIFICATIVA_MAX})",
            contar(&self.justificativa)
        )
    }

    /// O erro da justificativa, embaixo do campo (so' quando a resposta escolhida a pede).
    #[must_use]
    pub fn erro_da_justificativa(&self, tocada: bool) -> Option<String> {
        if !self.escolha.is_some_and(Resposta::pede_justificativa) {
            return None;
        }
        let n = contar(&self.justificativa);
        if n == 0 {
            return tocada.then(|| {
                "Escreva a justificativa: ela fica registrada no documento fiscal.".to_string()
            });
        }
        if n < JUSTIFICATIVA_MIN {
            return Some(format!(
                "Precisa de pelo menos {JUSTIFICATIVA_MIN} caracteres (faltam {}).",
                JUSTIFICATIVA_MIN - n
            ));
        }
        (n > JUSTIFICATIVA_MAX).then(|| {
            format!(
                "Passa de {JUSTIFICATIVA_MAX} caracteres (sobram {}).",
                n - JUSTIFICATIVA_MAX
            )
        })
    }

    /// A justificativa que vai no pedido: so' a operacao nao realizada a leva.
    #[must_use]
    pub fn para_enviar(&self) -> Option<String> {
        self.escolha
            .is_some_and(Resposta::pede_justificativa)
            .then(|| justificativa_normal(&self.justificativa))
    }

    /// Por que o botao esta desligado, ou `None` se pode enviar.
    #[must_use]
    pub fn motivo_de_nao_enviar(&self, alvo: &AlvoDaResposta, producao: bool) -> Option<String> {
        let Some(r) = self.escolha else {
            return Some("Escolha uma das respostas.".into());
        };
        if let Some(m) = alvo.recusada(r) {
            return Some(m.to_string());
        }
        if let Some(e) = self.erro_da_justificativa(true) {
            return Some(e);
        }
        (producao && !self.efeito_fiscal_entendido).then(|| FALTA_MARCAR.to_string())
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn alvo(nota: SituacaoDaNota, resposta: RespostaDada) -> AlvoDaResposta {
        AlvoDaResposta {
            chave: "1".repeat(44),
            fornecedor: "FORNECEDOR DE ENSAIO".into(),
            valor: "31,80".into(),
            data: "03/10/2026".into(),
            nota,
            resposta_dada: resposta,
            completa: false,
        }
    }

    const NOTAS: [SituacaoDaNota; 3] = [
        SituacaoDaNota::Autorizada,
        SituacaoDaNota::Cancelada,
        SituacaoDaNota::Denegada,
    ];
    const DADAS: [RespostaDada; 5] = [
        RespostaDada::Nenhuma,
        RespostaDada::Ciencia,
        RespostaDada::Confirmada,
        RespostaDada::Desconhecida,
        RespostaDada::NaoRealizada,
    ];

    #[test]
    fn as_quatro_respostas_tem_as_palavras_de_quem_compra() {
        let rotulos: Vec<&str> = Resposta::TODAS.iter().map(|r| r.rotulo()).collect();
        assert_eq!(
            rotulos,
            [
                "Recebi a mercadoria, está certo",
                "Vi a nota, ainda vou conferir",
                "Não comprei isto",
                "Comprei, mas a venda não aconteceu"
            ]
        );
        let tipos: Vec<&str> = Resposta::TODAS.iter().map(|r| r.tipo().codigo()).collect();
        assert_eq!(tipos, ["210200", "210210", "210220", "210240"]);
        for r in Resposta::TODAS {
            assert_eq!(Resposta::do_tipo(r.tipo()), Some(r));
            assert!(!r.quando_usar().is_empty());
        }
    }

    #[test]
    fn o_modelo_recusa_exatamente_o_que_a_biblioteca_recusa() {
        // todas as combinacoes de nota, resposta anterior e resposta nova: o modelo e a biblioteca dizem o mesmo
        for nota in NOTAS {
            for dada in DADAS {
                let a = alvo(nota, dada);
                for r in Resposta::TODAS {
                    let da_biblioteca =
                        conferir_combinacao(r.tipo(), a.situacao_nfe().as_ref(), a.anterior())
                            .is_err();
                    assert_eq!(
                        a.recusada(r).is_some(),
                        da_biblioteca,
                        "{nota:?} {dada:?} {r:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn as_regras_conhecidas_650_651_655_e_573() {
        let nova = alvo(SituacaoDaNota::Autorizada, RespostaDada::Nenhuma);
        assert_eq!(nova.respostas_possiveis(), Resposta::TODAS);
        let cancelada = alvo(SituacaoDaNota::Cancelada, RespostaDada::Nenhuma);
        assert!(cancelada
            .recusada(Resposta::Ciencia)
            .unwrap()
            .contains("cancelada"));
        assert!(cancelada.recusada(Resposta::Desconhecer).is_some());
        assert!(cancelada.recusada(Resposta::Confirmar).is_none());
        let respondida = alvo(SituacaoDaNota::Autorizada, RespostaDada::Confirmada);
        assert!(respondida
            .recusada(Resposta::Ciencia)
            .unwrap()
            .contains("antes dela"));
        assert!(respondida
            .recusada(Resposta::Confirmar)
            .unwrap()
            .contains("já está registrada"));
        assert!(
            respondida.recusada(Resposta::Desconhecer).is_none(),
            "mudar de ideia vale a ultima"
        );
        let so_ciencia = alvo(SituacaoDaNota::Autorizada, RespostaDada::Ciencia);
        assert!(so_ciencia.recusada(Resposta::Ciencia).is_some());
        assert!(so_ciencia.recusada(Resposta::Confirmar).is_none());
    }

    #[test]
    fn justificativa_so_na_operacao_nao_realizada() {
        let a = alvo(SituacaoDaNota::Autorizada, RespostaDada::Nenhuma);
        let mut f = FormularioDaResposta {
            escolha: Some(Resposta::Confirmar),
            ..FormularioDaResposta::default()
        };
        assert!(f.para_enviar().is_none());
        assert!(f.motivo_de_nao_enviar(&a, false).is_none());
        f.justificativa = "texto que sera ignorado aqui".into();
        assert!(
            f.para_enviar().is_none(),
            "a confirmacao nao leva justificativa"
        );
        f.escolha = Some(Resposta::NaoRealizada);
        f.justificativa = String::new();
        assert!(f
            .motivo_de_nao_enviar(&a, false)
            .unwrap()
            .contains("justificativa"));
        f.justificativa = "curta".into();
        assert!(f
            .erro_da_justificativa(false)
            .unwrap()
            .contains("faltam 10"));
        f.justificativa = "x".repeat(256);
        assert!(f.erro_da_justificativa(false).unwrap().contains("sobram 1"));
        f.justificativa = "  Mercadoria   nao chegou ao deposito  ".into();
        assert!(f.motivo_de_nao_enviar(&a, false).is_none());
        assert_eq!(
            f.para_enviar().unwrap(),
            "Mercadoria nao chegou ao deposito"
        );
        assert_eq!(f.contador(), "33 caracteres (de 15 a 255)");
    }

    #[test]
    fn em_producao_sem_a_marcacao_o_botao_fica_desligado_e_diz_por_que() {
        let a = alvo(SituacaoDaNota::Autorizada, RespostaDada::Nenhuma);
        let mut f = FormularioDaResposta {
            escolha: Some(Resposta::Ciencia),
            ..FormularioDaResposta::default()
        };
        assert!(
            f.motivo_de_nao_enviar(&a, false).is_none(),
            "homologacao nao exige"
        );
        assert_eq!(
            f.motivo_de_nao_enviar(&a, true).as_deref(),
            Some(FALTA_MARCAR)
        );
        f.efeito_fiscal_entendido = true;
        assert!(f.motivo_de_nao_enviar(&a, true).is_none());
        // sem escolha nao ha o que enviar
        f.escolha = None;
        assert_eq!(
            f.motivo_de_nao_enviar(&a, false).as_deref(),
            Some("Escolha uma das respostas.")
        );
    }

    #[test]
    fn nenhum_texto_de_resposta_inventa_prazo() {
        for r in Resposta::TODAS {
            let t = format!("{} {}", r.quando_usar(), r.registrada());
            assert!(!t.to_lowercase().contains("dias"), "{t}");
            assert!(!t.to_lowercase().contains("prazo"), "{t}");
        }
        assert!(!EFEITO_DA_RESPOSTA.to_lowercase().contains("prazo"));
    }
}
