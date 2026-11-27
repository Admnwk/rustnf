// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Os pedidos da NF-e que mudam o que a SEFAZ sabe: cancelar, corrigir (carta de correcao) e inutilizar numeros.
//! O formulario de cada um (com os limites da biblioteca), o desfecho como tipo da tela e o texto de cada desfecho em
//! portugues, com o que fazer. Nenhuma frase aqui leva caminho, senha, XML nem detalhe de rede.

use rustnf::nfe::sefaz::evento::{COND_USO, MAX_SEQ_CARTA};
use rustnf::nfe::sefaz::{DesfechoEvento, DesfechoInutilizacao};
use rustnf_app::erro::Erro;
use rustnf_app::nfe::{Inutilizada, Registrado};

use super::cancelamento::justificativa_normal;
use super::mascara::frase;

/// A justificativa (cancelamento e inutilizacao) tem de 15 a 255 caracteres (MOC 7.00; `texto_livre(.., 15, 255)` da
/// biblioteca, em `crates/rustnf/src/nfe/sefaz/evento/cancelamento.rs`).
pub const JUSTIFICATIVA_MIN: usize = 15;
/// Maior justificativa.
pub const JUSTIFICATIVA_MAX: usize = 255;
/// A carta de correcao tem de 15 a 1000 caracteres.
pub const CORRECAO_MIN: usize = 15;
/// Maior carta de correcao.
pub const CORRECAO_MAX: usize = 1000;
/// Quantos numeros cabem numa faixa de inutilizacao.
pub const FAIXA_MAXIMA: u64 = 10_000;
/// Quantas cartas a nota admite.
pub const CARTAS_MAXIMAS: u32 = MAX_SEQ_CARTA;

/// O texto legal que a biblioteca envia com a carta, para o "ver o texto oficial".
pub const TEXTO_OFICIAL_DA_CARTA: &str = COND_USO;

/// O que a carta de correcao NAO pode corrigir, em palavras simples (resume `COND_USO`, itens I, II e III).
pub const O_QUE_A_CARTA_NAO_CORRIGE: [&str; 3] = [
    "valores e impostos: base de cálculo, alíquota, diferença de preço, quantidade ou valor da operação;",
    "dados que mudem quem vende ou quem compra (troca do emitente ou do destinatário);",
    "a data de emissão ou de saída.",
];

/// Qual pedido o dialogo faz.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PedidoNfe {
    /// Cancelar a nota.
    Cancelar,
    /// Carta de correcao.
    Corrigir,
    /// Inutilizar uma faixa de numeros.
    Inutilizar,
}

impl PedidoNfe {
    /// O titulo do dialogo.
    #[must_use]
    pub fn titulo(self) -> &'static str {
        match self {
            Self::Cancelar => "Cancelar a NF-e",
            Self::Corrigir => "Carta de correção",
            Self::Inutilizar => "Inutilizar números",
        }
    }

    /// O texto do botao de enviar.
    #[must_use]
    pub fn enviar(self, producao: bool) -> &'static str {
        match (self, producao) {
            (Self::Cancelar, true) => "Cancelar a nota em produção",
            (Self::Cancelar, false) => "Cancelar a nota",
            (Self::Corrigir, true) => "Enviar a correção em produção",
            (Self::Corrigir, false) => "Enviar a correção",
            (Self::Inutilizar, true) => "Inutilizar em produção",
            (Self::Inutilizar, false) => "Inutilizar os números",
        }
    }

    /// O texto do que esta acontecendo.
    #[must_use]
    pub fn enviando(self) -> &'static str {
        match self {
            Self::Cancelar => "Enviando o cancelamento à SEFAZ...",
            Self::Corrigir => "Enviando a carta de correção à SEFAZ...",
            Self::Inutilizar => "Enviando a inutilização à SEFAZ...",
        }
    }

    /// A frase de efeito, para quem nao e' contador.
    #[must_use]
    pub fn efeito(self) -> &'static str {
        match self {
            Self::Cancelar => "O cancelamento faz a nota deixar de valer perante a SEFAZ. Não tem volta.",
            Self::Corrigir => "A carta de correção fica registrada junto da nota. Ela corrige só o que a lei permite.",
            Self::Inutilizar => "Inutilizar é avisar a SEFAZ de que estes números foram pulados e não viraram nota. Use quando a numeração pulou (uma nota que deu erro antes de sair, por exemplo). Número já usado por uma nota não se inutiliza.",
        }
    }

    /// A marcacao que a producao exige antes de habilitar o botao.
    #[must_use]
    pub fn marcacao_de_producao(self) -> &'static str {
        match self {
            Self::Cancelar => "Entendo que o cancelamento tem efeito fiscal e não tem volta",
            Self::Corrigir => "Entendo que a carta fica registrada na nota, com efeito fiscal",
            Self::Inutilizar => "Entendo que a inutilização tem efeito fiscal e não tem volta",
        }
    }

    /// O que falta marcar, para o botao desligado explicar.
    #[must_use]
    pub fn falta_marcar(self) -> &'static str {
        match self {
            Self::Cancelar => {
                "Em produção, marque que você entende o efeito fiscal do cancelamento."
            }
            Self::Corrigir => {
                "Em produção, marque que você entende que a carta fica registrada na nota."
            }
            Self::Inutilizar => {
                "Em produção, marque que você entende o efeito fiscal da inutilização."
            }
        }
    }

    /// A mensagem de sucesso.
    #[must_use]
    pub fn registrado(self) -> &'static str {
        match self {
            Self::Cancelar => "A SEFAZ registrou o cancelamento: a nota deixou de valer.",
            Self::Corrigir => "A SEFAZ registrou a carta de correção.",
            Self::Inutilizar => "A SEFAZ inutilizou os números.",
        }
    }

    /// O que a tela diz quando nem a consulta confirmou.
    #[must_use]
    pub fn sem_resposta(self) -> &'static str {
        match self {
            Self::Cancelar => "A SEFAZ não confirmou o cancelamento e a consulta da nota também não o achou. Ele pode ter sido registrado.",
            Self::Corrigir => "A SEFAZ não confirmou a carta e a consulta da nota também não a achou. Ela pode ter sido registrada.",
            Self::Inutilizar => "A SEFAZ não confirmou a inutilização. Ela pode ter sido registrada.",
        }
    }
}

/// A justificativa como a biblioteca a conta: espacos repetidos e das pontas nao valem.
fn contar(texto: &str) -> usize {
    justificativa_normal(texto).chars().count()
}

fn erro_de_tamanho(
    texto: &str,
    min: usize,
    max: usize,
    o_que: &str,
    tocada: bool,
) -> Option<String> {
    let n = contar(texto);
    if n == 0 {
        return tocada
            .then(|| format!("Escreva {o_que}: ela fica registrada no documento fiscal."));
    }
    if n < min {
        return Some(format!(
            "Precisa de pelo menos {min} caracteres (faltam {}).",
            min - n
        ));
    }
    (n > max).then(|| format!("Passa de {max} caracteres (sobram {}).", n - max))
}

/// O cancelamento: a justificativa e, em producao, a marcacao do efeito fiscal.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FormularioDeCancelamentoNfe {
    /// A justificativa, como digitada.
    pub justificativa: String,
    /// Em producao: "Entendo que...".
    pub efeito_fiscal_entendido: bool,
}

impl FormularioDeCancelamentoNfe {
    /// `12 caracteres (de 15 a 255)`.
    #[must_use]
    pub fn contador(&self) -> String {
        format!(
            "{} caracteres (de {JUSTIFICATIVA_MIN} a {JUSTIFICATIVA_MAX})",
            contar(&self.justificativa)
        )
    }

    /// O erro da justificativa, embaixo do campo.
    #[must_use]
    pub fn erro(&self, tocada: bool) -> Option<String> {
        erro_de_tamanho(
            &self.justificativa,
            JUSTIFICATIVA_MIN,
            JUSTIFICATIVA_MAX,
            "a justificativa",
            tocada,
        )
    }

    /// A justificativa que vai no pedido.
    #[must_use]
    pub fn para_enviar(&self) -> String {
        justificativa_normal(&self.justificativa)
    }

    /// Por que o botao esta desligado, ou `None`.
    #[must_use]
    pub fn motivo_de_nao_enviar(&self, producao: bool) -> Option<String> {
        if let Some(e) = self.erro(true) {
            return Some(e);
        }
        (producao && !self.efeito_fiscal_entendido)
            .then(|| PedidoNfe::Cancelar.falta_marcar().to_string())
    }
}

/// A carta de correcao: o texto (15 a 1000), quantas cartas a nota ja tem e a marcacao de producao.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FormularioDeCorrecao {
    /// O texto, como digitado.
    pub texto: String,
    /// Quantas cartas a nota ja tem.
    pub ja_feitas: u32,
    /// Em producao: "Entendo que...".
    pub efeito_fiscal_entendido: bool,
}

impl FormularioDeCorrecao {
    /// O formulario de uma nota que ja tem `ja_feitas` cartas.
    #[must_use]
    pub fn da_nota(ja_feitas: u32) -> Self {
        Self {
            ja_feitas,
            ..Self::default()
        }
    }

    /// `12 caracteres (de 15 a 1000)`.
    #[must_use]
    pub fn contador(&self) -> String {
        format!(
            "{} caracteres (de {CORRECAO_MIN} a {CORRECAO_MAX})",
            contar(&self.texto)
        )
    }

    /// A nota ja usou todas as cartas.
    #[must_use]
    pub fn esgotada(&self) -> bool {
        self.ja_feitas >= CARTAS_MAXIMAS
    }

    /// O que a tela diz de quantas cartas ha, e que a nova substitui as anteriores (`cancelamento.rs:16`).
    #[must_use]
    pub fn aviso_das_cartas(&self) -> String {
        if self.ja_feitas == 0 {
            format!("Esta será a 1ª carta (a nota admite até {CARTAS_MAXIMAS}).")
        } else {
            format!(
                "Esta nota já tem {} (admite até {CARTAS_MAXIMAS}). A nova SUBSTITUI as anteriores: o texto tem de trazer TODAS as correções que ainda valem, as antigas e a nova.",
                super::nfe_resultados::contagem(
                    self.ja_feitas as usize,
                    "carta de correção",
                    "cartas de correção"
                )
            )
        }
    }

    /// O erro do texto, embaixo do campo.
    #[must_use]
    pub fn erro(&self, tocada: bool) -> Option<String> {
        erro_de_tamanho(
            &self.texto,
            CORRECAO_MIN,
            CORRECAO_MAX,
            "a correção",
            tocada,
        )
    }

    /// O texto que vai no pedido.
    #[must_use]
    pub fn para_enviar(&self) -> String {
        justificativa_normal(&self.texto)
    }

    /// Por que o botao esta desligado, ou `None`.
    #[must_use]
    pub fn motivo_de_nao_enviar(&self, producao: bool) -> Option<String> {
        if self.esgotada() {
            return Some(format!(
                "A nota já tem as {CARTAS_MAXIMAS} cartas de correção que a SEFAZ admite."
            ));
        }
        if let Some(e) = self.erro(true) {
            return Some(e);
        }
        (producao && !self.efeito_fiscal_entendido)
            .then(|| PedidoNfe::Corrigir.falta_marcar().to_string())
    }
}

/// A faixa que se pede para inutilizar.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FormularioDeInutilizacao {
    /// A serie (vazio = a da configuracao).
    pub serie: String,
    /// O primeiro numero.
    pub inicio: String,
    /// O ultimo numero (vazio = so' o primeiro).
    pub fim: String,
    /// A justificativa.
    pub justificativa: String,
    /// Em producao: "Entendo que...".
    pub efeito_fiscal_entendido: bool,
}

/// A faixa pedida, ja conferida.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Faixa {
    /// A serie digitada, ou `None` para a da configuracao.
    pub serie: Option<String>,
    /// O primeiro numero.
    pub inicio: u64,
    /// O ultimo numero.
    pub fim: u64,
}

impl FormularioDeInutilizacao {
    /// A faixa digitada, ou o motivo de nao servir.
    ///
    /// # Errors
    ///
    /// A frase que diz o que corrigir.
    pub fn faixa(&self) -> Result<Faixa, String> {
        let numero = |t: &str, o_que: &str| {
            t.trim()
                .parse::<u64>()
                .ok()
                .filter(|n| *n >= 1 && t.trim().chars().all(|c| c.is_ascii_digit()))
                .ok_or_else(|| format!("Digite {o_que} (só números, a partir de 1)."))
        };
        let inicio = numero(&self.inicio, "o primeiro número a inutilizar")?;
        let fim = if self.fim.trim().is_empty() {
            inicio
        } else {
            numero(&self.fim, "o último número a inutilizar")?
        };
        if fim < inicio {
            return Err("O último número não pode ser menor que o primeiro.".into());
        }
        if fim - inicio + 1 > FAIXA_MAXIMA {
            return Err(format!(
                "Uma inutilização cobre no máximo {FAIXA_MAXIMA} números de uma vez."
            ));
        }
        let serie = self.serie.trim();
        if !serie.is_empty() && !(serie.len() <= 3 && serie.chars().all(|c| c.is_ascii_digit())) {
            return Err("A série vai de 0 a 999.".into());
        }
        Ok(Faixa {
            serie: (!serie.is_empty()).then(|| serie.to_string()),
            inicio,
            fim,
        })
    }

    /// `12 caracteres (de 15 a 255)`.
    #[must_use]
    pub fn contador(&self) -> String {
        format!(
            "{} caracteres (de {JUSTIFICATIVA_MIN} a {JUSTIFICATIVA_MAX})",
            contar(&self.justificativa)
        )
    }

    /// O erro da justificativa.
    #[must_use]
    pub fn erro_da_justificativa(&self, tocada: bool) -> Option<String> {
        erro_de_tamanho(
            &self.justificativa,
            JUSTIFICATIVA_MIN,
            JUSTIFICATIVA_MAX,
            "a justificativa",
            tocada,
        )
    }

    /// A justificativa que vai no pedido.
    #[must_use]
    pub fn para_enviar(&self) -> String {
        justificativa_normal(&self.justificativa)
    }

    /// O erro da faixa so' depois de digitar algo.
    #[must_use]
    pub fn erro_da_faixa(&self) -> Option<String> {
        if self.inicio.trim().is_empty() && self.fim.trim().is_empty() {
            return None;
        }
        self.faixa().err()
    }

    /// Por que o botao esta desligado, ou `None`.
    #[must_use]
    pub fn motivo_de_nao_enviar(&self, producao: bool) -> Option<String> {
        if let Err(e) = self.faixa() {
            return Some(e);
        }
        if let Some(e) = self.erro_da_justificativa(true) {
            return Some(e);
        }
        (producao && !self.efeito_fiscal_entendido)
            .then(|| PedidoNfe::Inutilizar.falta_marcar().to_string())
    }
}

/// Como o pedido terminou.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DesfechoNfe {
    /// A SEFAZ registrou.
    Registrado {
        /// O protocolo do evento.
        protocolo: String,
        /// A SEFAZ achou o evento ja registrado (resposta que se perdeu).
        recuperado: bool,
        /// Aviso a mais (fora do prazo, nota fora da pasta...).
        aviso: Option<String>,
    },
    /// A SEFAZ recusou.
    Rejeitado {
        /// O codigo da rejeicao.
        codigo: String,
        /// A descricao da SEFAZ, em destaque.
        descricao: String,
        /// O que fazer.
        orientacao: String,
    },
    /// A SEFAZ confirmou que o pedido NAO foi registrado: nada mudou e e' seguro pedir de novo.
    NaoRegistrado,
    /// Sem resposta: o pedido PODE ter sido registrado.
    SemResposta {
        /// A frase.
        texto: String,
    },
    /// Nada saiu: regra da biblioteca, certificado, senha ou configuracao.
    Recusado {
        /// A frase.
        texto: String,
    },
}

impl DesfechoNfe {
    /// O pedido ficou registrado.
    #[must_use]
    pub fn registrado(&self) -> bool {
        matches!(self, Self::Registrado { .. })
    }

    /// So' se sai deste desfecho conferindo, tentando de novo ou fechando.
    #[must_use]
    pub fn deixa_em_aberto(&self) -> bool {
        matches!(self, Self::SemResposta { .. } | Self::NaoRegistrado)
    }

    /// A SEFAZ recusou o cancelamento por prazo (501).
    #[must_use]
    pub fn recusado_por_prazo(&self) -> bool {
        matches!(self, Self::Rejeitado { codigo, .. } if codigo == "501")
    }

    /// O desfecho do que `cancelar` ou `corrigir` devolveu.
    #[must_use]
    pub fn do_evento(r: Result<Registrado, Erro>, pedido: PedidoNfe) -> Self {
        let r = match r {
            Ok(r) => r,
            Err(e) => return Self::do_erro(&e),
        };
        let sem_pasta = (!r.guardado && r.situacao.is_none())
            .then(|| "A nota não está na pasta de notas: o evento ficou só na SEFAZ.".to_string());
        match &r.resultado.desfecho {
            DesfechoEvento::Registrado {
                protocolo,
                fora_do_prazo,
                ..
            } => Self::Registrado {
                protocolo: protocolo.clone(),
                recuperado: r.resultado.recuperado,
                aviso: if *fora_do_prazo {
                    Some("A SEFAZ aceitou o cancelamento fora do prazo normal (código 155).".into())
                } else {
                    sem_pasta
                },
            },
            DesfechoEvento::SemVinculo { motivo, .. } => Self::Rejeitado {
                codigo: "136".into(),
                descricao: frase(motivo),
                orientacao: "A SEFAZ guardou o pedido, mas a nota não existia lá no momento: a nota não foi alterada. Confira a nota na SEFAZ e peça de novo.".into(),
            },
            DesfechoEvento::PrazoVencido { cstat, motivo } => Self::Rejeitado {
                codigo: cstat.clone(),
                descricao: frase(motivo),
                orientacao: TEXTO_DO_PRAZO.into(),
            },
            DesfechoEvento::Rejeitado { cstat, motivo } => Self::Rejeitado {
                codigo: cstat.clone(),
                descricao: frase(motivo),
                orientacao: "Nada foi registrado e a nota continua como estava. Corrija o que a SEFAZ aponta e peça de novo.".into(),
            },
            DesfechoEvento::EpecBloqueado { motivo, .. } => Self::Rejeitado {
                codigo: "142".into(),
                descricao: frase(motivo),
                orientacao: "Nada foi registrado.".into(),
            },
            DesfechoEvento::NaoRegistrado => Self::NaoRegistrado,
            DesfechoEvento::Indisponivel { motivo, .. } => Self::SemResposta {
                texto: format!(
                    "{} Nada foi registrado: o serviço da SEFAZ está parado. Tente de novo mais tarde.",
                    frase(motivo)
                ),
            },
            _ => Self::SemResposta {
                texto: pedido.sem_resposta().into(),
            },
        }
    }

    /// O desfecho do que `inutilizar` devolveu.
    #[must_use]
    pub fn da_inutilizacao(r: Result<Inutilizada, Erro>) -> Self {
        let r = match r {
            Ok(r) => r,
            Err(e) => return Self::do_erro(&e),
        };
        let rejeitada = |codigo: &str, motivo: &str, o_que_fazer: &str| Self::Rejeitado {
            codigo: codigo.into(),
            descricao: frase(motivo),
            orientacao: o_que_fazer.into(),
        };
        match &r.resultado.desfecho {
            DesfechoInutilizacao::Inutilizada { protocolo, .. } => Self::Registrado {
                protocolo: protocolo.clone(),
                recuperado: false,
                aviso: (!r.guardada)
                    .then(|| "A faixa ficou só na SEFAZ: a pasta de notas não a guardou.".into()),
            },
            DesfechoInutilizacao::JaInutilizada { protocolo, .. } => Self::Registrado {
                protocolo: protocolo.clone(),
                recuperado: true,
                aviso: Some("Estes números já estavam inutilizados na SEFAZ.".into()),
            },
            DesfechoInutilizacao::ServicoDesativado { motivo } => rejeitada(
                "",
                motivo,
                "Neste estado a SEFAZ não aceita mais a inutilização por este serviço (AC, BA e SP o encerraram). Fale com o seu contador.",
            ),
            DesfechoInutilizacao::Rejeitada { cstat, motivo } => rejeitada(
                cstat,
                motivo,
                "Nada foi inutilizado. Corrija o que a SEFAZ aponta e peça de novo.",
            ),
            DesfechoInutilizacao::Indisponivel { motivo, .. } => Self::SemResposta {
                texto: format!(
                    "{} Nada foi inutilizado: o serviço da SEFAZ está parado. Tente de novo mais tarde.",
                    frase(motivo)
                ),
            },
            _ => Self::SemResposta {
                texto: PedidoNfe::Inutilizar.sem_resposta().into(),
            },
        }
    }

    /// Recusa antes da rede.
    #[must_use]
    pub fn do_erro(e: &Erro) -> Self {
        match e {
            Erro::Rede(t) => Self::SemResposta { texto: frase(t) },
            outro => Self::Recusado {
                texto: frase(&outro.mensagem()),
            },
        }
    }
}

/// O que dizer quando o cancelamento volta com 501 (`crates/rustnf-app/src/nfe/eventos.rs:118-124`: a aplicacao nao
/// conta prazo; a SEFAZ da UF decide e responde 501). Nao ha prazo escrito aqui de proposito.
pub const TEXTO_DO_PRAZO: &str = "A SEFAZ recusou porque o prazo para cancelar esta nota já passou. O prazo vem da legislação de cada estado, e só a SEFAZ sabe se ainda aceita. Por aqui a nota não pode mais ser cancelada: fale com o seu contador sobre como regularizar a operação (por exemplo, com uma nota de devolução ou de ajuste).";

#[cfg(test)]
mod testes {
    use super::*;
    use rustnf::nfe::sefaz::ResultadoEvento;

    fn evento(d: DesfechoEvento) -> Result<Registrado, Erro> {
        Ok(Registrado {
            chave: "x".into(),
            resultado: ResultadoEvento {
                desfecho: d,
                chave: "x".into(),
                tp_evento: "110111".into(),
                seq: 1,
                autorizador: String::new(),
                xml_enviado: String::new(),
                xml_retorno: String::new(),
                proc_evento: None,
                recuperado: false,
                frase: "frase".into(),
                detalhe: String::new(),
            },
            guardado: true,
            situacao: None,
            avisos: Vec::new(),
        })
    }

    #[test]
    fn os_limites_da_justificativa_contam_como_a_biblioteca() {
        let mut f = FormularioDeCancelamentoNfe::default();
        assert_eq!(f.erro(false), None, "vazia so' acusa depois de tocada");
        assert!(f.erro(true).unwrap().contains("Escreva a justificativa"));
        f.justificativa = "  curta   demais ".into();
        assert!(f.erro(false).unwrap().contains("faltam"));
        f.justificativa = "a".repeat(15);
        assert_eq!(f.erro(true), None);
        f.justificativa = "a".repeat(256);
        assert!(f.erro(true).unwrap().contains("sobram 1"));
        assert_eq!(f.contador(), "256 caracteres (de 15 a 255)");
    }

    #[test]
    fn os_limites_batem_com_o_que_a_biblioteca_aceita() {
        use rustnf::chave_dfe::{montar, Montar};
        use rustnf::nfe::sefaz::evento::{
            cancelamento, carta_correcao, Cancelamento, CartaCorrecao,
        };
        use rustnf::nfe::vigencias::Ambiente;
        let quando = chrono::DateTime::parse_from_rfc3339("2026-10-03T12:00:00-03:00").unwrap();
        let chave = montar(&Montar {
            cuf: 35,
            aamm: "2610",
            emitente: "11222333000181",
            numero: 1,
            ..Montar::default()
        })
        .unwrap();
        let cancelar = |j: &str| {
            cancelamento(&Cancelamento {
                chave: &chave,
                protocolo: "135260000000001",
                justificativa: j,
                ambiente: Ambiente::Homologacao,
                quando,
            })
            .is_ok()
        };
        assert!(!cancelar(&"a".repeat(JUSTIFICATIVA_MIN - 1)));
        assert!(cancelar(&"a".repeat(JUSTIFICATIVA_MIN)));
        assert!(cancelar(&"a".repeat(JUSTIFICATIVA_MAX)));
        assert!(!cancelar(&"a".repeat(JUSTIFICATIVA_MAX + 1)));
        let corrigir = |t: &str, seq: u32| {
            carta_correcao(&CartaCorrecao {
                chave: &chave,
                correcao: t,
                seq,
                ambiente: Ambiente::Homologacao,
                quando,
            })
            .is_ok()
        };
        assert!(!corrigir(&"a".repeat(CORRECAO_MIN - 1), 1));
        assert!(corrigir(&"a".repeat(CORRECAO_MIN), 1));
        assert!(corrigir(&"a".repeat(CORRECAO_MAX), CARTAS_MAXIMAS));
        assert!(!corrigir(&"a".repeat(CORRECAO_MAX + 1), 1));
        assert!(!corrigir(&"a".repeat(CORRECAO_MIN), CARTAS_MAXIMAS + 1));
    }

    #[test]
    fn a_carta_conta_as_anteriores_e_avisa_que_substitui() {
        let mut f = FormularioDeCorrecao::da_nota(0);
        assert!(f.aviso_das_cartas().contains("1ª carta"));
        f.ja_feitas = 2;
        let a = f.aviso_das_cartas();
        assert!(
            a.contains("já tem 2 cartas de correção") && a.contains("SUBSTITUI"),
            "{a}"
        );
        f.ja_feitas = CARTAS_MAXIMAS;
        assert!(f.esgotada());
        assert!(f.motivo_de_nao_enviar(false).unwrap().contains("20 cartas"));
        f.ja_feitas = 0;
        f.texto = "b".repeat(1001);
        assert!(f.erro(true).unwrap().contains("sobram 1"));
        f.texto = "Corrigir o endereço de entrega no campo de informações adicionais".into();
        assert_eq!(f.motivo_de_nao_enviar(false), None);
        assert!(f
            .motivo_de_nao_enviar(true)
            .unwrap()
            .contains("Em produção"));
    }

    #[test]
    fn a_faixa_de_inutilizacao_se_confere() {
        let mut f = FormularioDeInutilizacao::default();
        assert!(f.faixa().is_err());
        assert_eq!(f.erro_da_faixa(), None, "vazia nao acusa");
        f.inicio = "5".into();
        assert_eq!(
            f.faixa().unwrap(),
            Faixa {
                serie: None,
                inicio: 5,
                fim: 5
            }
        );
        f.fim = "3".into();
        assert!(f.faixa().unwrap_err().contains("menor"));
        f.fim = "10006".into();
        assert!(f.faixa().unwrap_err().contains("10000"));
        f.fim = "9".into();
        f.serie = "2".into();
        assert_eq!(f.faixa().unwrap().serie.as_deref(), Some("2"));
        f.serie = "1234".into();
        assert!(f.faixa().is_err());
        f.inicio = "5x".into();
        assert!(f.faixa().unwrap_err().contains("primeiro número"));
    }

    #[test]
    fn cada_desfecho_do_evento_tem_o_seu_texto() {
        let reg = DesfechoNfe::do_evento(
            evento(DesfechoEvento::Registrado {
                cstat: "135".into(),
                motivo: "ok".into(),
                protocolo: "1351".into(),
                fora_do_prazo: true,
            }),
            PedidoNfe::Cancelar,
        );
        assert!(reg.registrado());
        assert!(
            matches!(&reg, DesfechoNfe::Registrado { aviso: Some(a), .. } if a.contains("155"))
        );
        let prazo = DesfechoNfe::do_evento(
            evento(DesfechoEvento::PrazoVencido {
                cstat: "501".into(),
                motivo: "rejeicao: prazo".into(),
            }),
            PedidoNfe::Cancelar,
        );
        assert!(prazo.recusado_por_prazo());
        assert!(matches!(&prazo, DesfechoNfe::Rejeitado { orientacao, .. }
            if orientacao.contains("legislação de cada estado") && !orientacao.contains("24 horas")));
        let nao =
            DesfechoNfe::do_evento(evento(DesfechoEvento::NaoRegistrado), PedidoNfe::Corrigir);
        assert!(nao.deixa_em_aberto());
        let sem = DesfechoNfe::do_evento(evento(DesfechoEvento::SemResposta), PedidoNfe::Corrigir);
        assert!(matches!(sem, DesfechoNfe::SemResposta { ref texto } if texto.contains("carta")));
        let recusa = DesfechoNfe::do_evento(
            Err(Erro::recusa("chNFe", "so' se cancela nota autorizada")),
            PedidoNfe::Cancelar,
        );
        assert!(
            matches!(recusa, DesfechoNfe::Recusado { ref texto } if texto.contains("So' se cancela"))
        );
    }
}
