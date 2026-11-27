// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O que cada pedido das NF-e recebidas respondeu, ja em palavras para a tela: a busca de notas novas, a baixa da nota
//! completa e a resposta (manifestacao) do destinatario. Nenhuma frase aqui leva caminho, senha, XML nem detalhe de
//! rede. Sem janela.
//!
//! De onde vem cada regra (a tela so' conta o que a camada de aplicacao e a biblioteca ja decidem):
//! * a ESPERA entre buscas e o bloqueio por consultar demais (656) sao da biblioteca
//!   (`crates/rustnf/src/nfe/sefaz/dfe/cursor.rs:97-103`, `resultado.rs:19-36`): a tela mostra o horario e nunca contorna;
//! * a NOTA COMPLETA so' vem a quem ja manifestou, e o desconhecimento nao a libera
//!   (`crates/rustnf-app/src/nfe/recebidas.rs:10-12`, NT 2014.002 v1.02d, item 3);
//! * o PRAZO da manifestacao NAO e' calculado por este programa: quem confere e' o Ambiente Nacional (rejeicao 596,
//!   `crates/rustnf/src/nfe/sefaz/evento/manifestacao.rs:20-22`), e a tela so' repete o que ele responde.

use chrono::{DateTime, Utc};
use rustnf::nfe::sefaz::dfe::{DesfechoDfe, ResultadoDfe};
use rustnf::nfe::sefaz::evento::TipoManifestacao;
use rustnf::nfe::sefaz::DesfechoEvento;
use rustnf_app::erro::Erro;
use rustnf_app::nfe::{Baixada, Contagem, Manifestada, Sincronizada};

use super::certificado::Gravidade;
use super::mascara::frase;
use super::nfe_eventos::DesfechoNfe;
use super::nfe_recebidas::{hora_de_brasilia, RespostaDada};
use super::nfe_resultados::contagem;

/// A espera que o resultado deixou no cursor, se houver.
fn espera_do(r: &ResultadoDfe) -> Option<DateTime<Utc>> {
    r.cursor.espera.map(|e| e.ate)
}

/// A frase de "tente depois das HH:MM", ou a de "tente de novo daqui a pouco" se a hora nao veio.
fn depois_das(ate: Option<DateTime<Utc>>) -> String {
    ate.map_or_else(
        || "Tente de novo mais tarde.".to_string(),
        |t| {
            format!(
                "Tente de novo depois das {} (horário de Brasília).",
                hora_de_brasilia(t)
            )
        },
    )
}

/// Como a busca de notas novas terminou.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DesfechoBusca {
    /// Chegaram notas novas.
    Novas {
        /// Quantas notas que a pasta ainda nao tinha.
        novas: usize,
        /// Ainda ha mais na fila: buscar de novo continua de onde parou.
        mais_na_fila: bool,
        /// A espera ate a proxima busca (so' quando a fila esta em dia).
        proxima: Option<DateTime<Utc>>,
        /// O que mais veio (cancelamentos e eventos de notas que a pasta ja tinha, notas que nao abriram...).
        extra: String,
    },
    /// A fila esta em dia: nada de novo.
    NadaNovo {
        /// A espera ate a proxima busca.
        proxima: Option<DateTime<Utc>>,
        /// O que mais veio.
        extra: String,
    },
    /// Nenhuma chamada saiu: ainda e' cedo (o Ambiente Nacional pediu para esperar).
    Espera {
        /// Ate quando.
        ate: DateTime<Utc>,
    },
    /// O Ambiente Nacional bloqueou o CNPJ por consultar demais (656).
    Bloqueada {
        /// Ate quando.
        ate: Option<DateTime<Utc>>,
    },
    /// O Ambiente Nacional recusou a busca.
    Recusada {
        /// O codigo.
        codigo: String,
        /// O motivo, como ele escreveu.
        descricao: String,
    },
    /// Sem resposta que sirva (rede, servico parado): nada foi perdido, o cursor nao andou.
    SemResposta {
        /// A frase.
        texto: String,
    },
    /// Nada saiu: configuracao, certificado ou senha.
    Recusado {
        /// A frase.
        texto: String,
    },
}

impl DesfechoBusca {
    /// O desfecho do que `sincronizar` devolveu. `novas`: quantas notas a pasta ganhou (a conta e' de quem chama: compara
    /// a pasta antes e depois, porque o documento que chega pode ser de uma nota que a pasta ja tinha).
    #[must_use]
    pub fn da_busca(r: Result<Sincronizada, Erro>, novas: usize) -> Self {
        match r {
            Ok(s) => Self::de_partes(
                &s.resultado.desfecho,
                espera_do(&s.resultado),
                s.contagem,
                !s.resultado.irrecuperaveis.is_empty(),
                &s.resultado.frase,
                novas,
            ),
            Err(Erro::Rede(t)) => Self::SemResposta { texto: frase(&t) },
            Err(e) => Self::Recusado {
                texto: frase(&e.mensagem()),
            },
        }
    }

    /// O desfecho a partir das partes do resultado: o que a biblioteca disse, a espera que ficou no cursor, quantos
    /// documentos de cada tipo vieram, se ha NSU que ja nao se recupera e a frase dela.
    #[must_use]
    pub fn de_partes(
        desfecho: &DesfechoDfe,
        espera: Option<DateTime<Utc>>,
        contagem: Contagem,
        irrecuperaveis: bool,
        frase_da_biblioteca: &str,
        novas: usize,
    ) -> Self {
        let extra = Self::extra(contagem, irrecuperaveis, novas);
        match desfecho {
            DesfechoDfe::Aguarde { ate, .. } => Self::Espera { ate: *ate },
            DesfechoDfe::ConsumoIndevido { ate } => Self::Bloqueada { ate: Some(*ate) },
            DesfechoDfe::Rejeitada { cstat, motivo } => Self::Recusada {
                codigo: cstat.clone(),
                descricao: frase(motivo),
            },
            DesfechoDfe::Parcial if novas > 0 => Self::Novas {
                novas,
                mais_na_fila: true,
                proxima: None,
                extra,
            },
            DesfechoDfe::Parcial
            | DesfechoDfe::EmDia
            | DesfechoDfe::Localizado
            | DesfechoDfe::NaoLocalizado => {
                if novas > 0 {
                    Self::Novas {
                        novas,
                        mais_na_fila: false,
                        proxima: espera,
                        extra,
                    }
                } else {
                    Self::NadaNovo {
                        proxima: espera,
                        extra,
                    }
                }
            }
            DesfechoDfe::Indisponivel => Self::SemResposta {
                texto: "O Ambiente Nacional não respondeu. Nada foi perdido: a próxima busca continua de onde parou.".into(),
            },
            _ => Self::SemResposta {
                texto: frase(frase_da_biblioteca),
            },
        }
    }

    fn extra(c: Contagem, irrecuperaveis: bool, novas: usize) -> String {
        let mut partes = Vec::new();
        let abertos = c.total().saturating_sub(c.ilegiveis);
        if abertos > novas {
            partes.push(format!(
                "{} de notas que você já tinha (cancelamentos, cartas de correção ou a nota completa).",
                contagem(abertos - novas, "aviso", "avisos")
            ));
        }
        if c.ilegiveis > 0 {
            partes.push(format!(
                "{} não abriu e será lido de novo na próxima busca.",
                contagem(c.ilegiveis, "documento", "documentos")
            ));
        }
        if irrecuperaveis {
            partes.push(
                "Houve notas que o Ambiente Nacional já não guarda (ele só guarda 90 dias): elas não podem mais ser baixadas por aqui."
                    .into(),
            );
        }
        partes.join(" ")
    }

    /// A cor do aviso.
    #[must_use]
    pub fn gravidade(&self) -> Gravidade {
        match self {
            Self::Novas { .. } => Gravidade::Ok,
            Self::NadaNovo { .. } => Gravidade::Neutro,
            Self::Espera { .. } | Self::Bloqueada { .. } | Self::SemResposta { .. } => {
                Gravidade::Aviso
            }
            Self::Recusada { .. } | Self::Recusado { .. } => Gravidade::Erro,
        }
    }

    /// O titulo, em uma linha.
    #[must_use]
    pub fn titulo(&self) -> String {
        match self {
            Self::Novas { novas, .. } => contagem(*novas, "nota nova", "notas novas"),
            Self::NadaNovo { .. } => "Nada novo".into(),
            Self::Espera { ate } => format!(
                "O Ambiente Nacional pediu para esperar até {}",
                hora_de_brasilia(*ate)
            ),
            Self::Bloqueada { .. } => "Busca bloqueada por uma hora".into(),
            Self::Recusada { .. } => "O Ambiente Nacional recusou a busca".into(),
            Self::SemResposta { .. } => "Sem resposta do Ambiente Nacional".into(),
            Self::Recusado { .. } => "Nada foi enviado".into(),
        }
    }

    /// O que aconteceu e o que fazer.
    #[must_use]
    pub fn texto(&self) -> String {
        match self {
            Self::Novas {
                mais_na_fila,
                proxima,
                extra,
                ..
            } => {
                let resto = if *mais_na_fila {
                    "Ainda há mais na fila: busque de novo para continuar.".to_string()
                } else {
                    proxima.map_or_else(String::new, |t| {
                        format!(
                            "A fila está em dia. A próxima busca pode ser feita a partir das {} (horário de Brasília).",
                            hora_de_brasilia(t)
                        )
                    })
                };
                [extra.as_str(), resto.as_str()]
                    .iter()
                    .filter(|t| !t.is_empty())
                    .copied()
                    .collect::<Vec<_>>()
                    .join(" ")
            }
            Self::NadaNovo { proxima, extra } => {
                let em_dia = proxima.map_or_else(
                    || "Não há notas novas para você agora.".to_string(),
                    |t| {
                        format!(
                            "Não há notas novas para você agora. A próxima busca pode ser feita a partir das {} (horário de Brasília): o Ambiente Nacional pede uma hora entre as buscas.",
                            hora_de_brasilia(t)
                        )
                    },
                );
                if extra.is_empty() {
                    em_dia
                } else {
                    format!("{extra} {em_dia}")
                }
            }
            Self::Espera { ate } => format!(
                "Para não consultar demais, o programa não busca antes disso. O botão volta a funcionar às {} (horário de Brasília).",
                hora_de_brasilia(*ate)
            ),
            Self::Bloqueada { ate } => format!(
                "O Ambiente Nacional bloqueou novas buscas deste CNPJ por consultar demais. Nada foi perdido. {}",
                depois_das(*ate)
            ),
            Self::Recusada { codigo, descricao } => {
                format!("{descricao} (código {codigo}). Nada foi baixado e nada mudou na pasta.")
            }
            Self::SemResposta { texto } | Self::Recusado { texto } => texto.clone(),
        }
    }

    /// A pasta pode ter mudado (a lista le de novo).
    #[must_use]
    pub fn mexeu_na_pasta(&self) -> bool {
        matches!(self, Self::Novas { .. } | Self::NadaNovo { .. })
    }
}

/// Como a baixa da nota completa terminou.
#[derive(Debug, Clone, PartialEq)]
pub enum DesfechoBaixa {
    /// A nota completa esta na pasta.
    Completa {
        /// A ciencia foi registrada por este pedido.
        ciencia_registrada: bool,
    },
    /// Veio so' o resumo (a SEFAZ ainda nao liberou a nota).
    SoResumo {
        /// O que aconteceu e o que fazer.
        texto: String,
    },
    /// A ciencia que liberaria a nota nao foi registrada: o desfecho dela diz por que.
    CienciaNaoRegistrada(Box<DesfechoNfe>),
    /// Ainda e' cedo (ou o CNPJ esta bloqueado): ate quando.
    Espera {
        /// Ate quando; `None` se a hora nao veio.
        ate: Option<DateTime<Utc>>,
    },
    /// O Ambiente Nacional nao tem a nota para este CNPJ.
    NaoLocalizada,
    /// O Ambiente Nacional recusou.
    Recusada {
        /// A frase.
        texto: String,
    },
    /// Sem resposta que sirva.
    SemResposta {
        /// A frase.
        texto: String,
    },
    /// Nada saiu.
    Recusado {
        /// A frase.
        texto: String,
    },
}

impl DesfechoBaixa {
    /// O desfecho do que `baixar` devolveu.
    #[must_use]
    pub fn da_baixa(r: Result<Baixada, Erro>) -> Self {
        let b = match r {
            Ok(b) => b,
            Err(Erro::Rede(t)) => return Self::SemResposta { texto: frase(&t) },
            Err(e) => {
                return Self::Recusado {
                    texto: frase(&e.mensagem()),
                }
            }
        };
        if let Some(c) = b.ciencia.as_ref().filter(|c| !c.resultado.registrado()) {
            return Self::CienciaNaoRegistrada(Box::new(DesfechoNfe::da_manifestacao(
                Ok(c.clone()),
                TipoManifestacao::Ciencia,
            )));
        }
        let ciencia_registrada = b.ciencia.is_some();
        match &b.resultado.desfecho {
            DesfechoDfe::Localizado if b.completa => Self::Completa { ciencia_registrada },
            DesfechoDfe::Localizado => Self::SoResumo {
                texto: if ciencia_registrada {
                    "A ciência foi registrada, mas a SEFAZ ainda não liberou a nota completa. Isso pode levar alguns minutos: tente baixar de novo daqui a pouco, ou busque notas novas.".into()
                } else {
                    "A SEFAZ só entrega a nota completa a quem já respondeu sobre ela. Dê a ciência (ou outra resposta) para liberar a nota.".into()
                },
            },
            DesfechoDfe::NaoLocalizado => Self::NaoLocalizada,
            DesfechoDfe::Aguarde { ate, .. } | DesfechoDfe::ConsumoIndevido { ate } => {
                Self::Espera { ate: Some(*ate) }
            }
            DesfechoDfe::Rejeitada { cstat, motivo } => Self::Recusada {
                texto: format!("{} (código {cstat}).", frase(motivo)),
            },
            _ => Self::SemResposta {
                texto: "O Ambiente Nacional não respondeu. Nada foi perdido: tente de novo daqui a pouco.".into(),
            },
        }
    }

    /// A cor do aviso.
    #[must_use]
    pub fn gravidade(&self) -> Gravidade {
        match self {
            Self::Completa { .. } => Gravidade::Ok,
            Self::SoResumo { .. } | Self::Espera { .. } | Self::SemResposta { .. } => {
                Gravidade::Aviso
            }
            Self::NaoLocalizada | Self::Recusada { .. } | Self::Recusado { .. } => Gravidade::Erro,
            Self::CienciaNaoRegistrada(d) => match **d {
                DesfechoNfe::SemResposta { .. } | DesfechoNfe::NaoRegistrado => Gravidade::Aviso,
                _ => Gravidade::Erro,
            },
        }
    }

    /// O titulo.
    #[must_use]
    pub fn titulo(&self) -> &'static str {
        match self {
            Self::Completa { .. } => "Nota completa baixada",
            Self::SoResumo { .. } => "Ainda só o resumo",
            Self::CienciaNaoRegistrada(_) => "A ciência não foi registrada",
            Self::Espera { .. } => "É cedo para buscar",
            Self::NaoLocalizada => "Nota não encontrada",
            Self::Recusada { .. } => "O Ambiente Nacional recusou",
            Self::SemResposta { .. } => "Sem resposta do Ambiente Nacional",
            Self::Recusado { .. } => "Nada foi enviado",
        }
    }

    /// O que aconteceu e o que fazer.
    #[must_use]
    pub fn texto(&self) -> String {
        match self {
            Self::Completa { ciencia_registrada } => if *ciencia_registrada {
                "A ciência da operação foi registrada na SEFAZ e a nota completa está na pasta: já dá para ver o detalhe e abrir o DANFE."
            } else {
                "A nota completa está na pasta: já dá para ver o detalhe e abrir o DANFE."
            }
            .into(),
            Self::SoResumo { texto }
            | Self::Recusada { texto }
            | Self::SemResposta { texto }
            | Self::Recusado { texto } => texto.clone(),
            Self::CienciaNaoRegistrada(d) => match &**d {
                DesfechoNfe::Rejeitado {
                    descricao,
                    orientacao,
                    ..
                } => format!("{descricao} {orientacao}"),
                DesfechoNfe::SemResposta { texto } | DesfechoNfe::Recusado { texto } => {
                    texto.clone()
                }
                _ => "A SEFAZ não registrou a ciência. Nada mudou: tente de novo.".into(),
            },
            Self::Espera { ate } => format!(
                "O Ambiente Nacional pediu para esperar. {}",
                depois_das(*ate)
            ),
            Self::NaoLocalizada => {
                "O Ambiente Nacional não achou esta nota para o CNPJ da empresa. Confira a chave e o ambiente (testes ou produção).".into()
            }
        }
    }

    /// A pasta pode ter mudado.
    #[must_use]
    pub fn mexeu_na_pasta(&self) -> bool {
        matches!(
            self,
            Self::Completa { .. } | Self::SoResumo { .. } | Self::CienciaNaoRegistrada(_)
        )
    }
}

/// O texto do que a SEFAZ responde quando recusa uma manifestacao: o que fazer, por codigo.
fn orientacao_da_recusa(codigo: &str) -> &'static str {
    match codigo {
        // o prazo e' da legislacao; so' o Ambiente Nacional o confere (`manifestacao.rs:20-22`)
        "596" => "O prazo para responder a esta nota pode ter passado. O prazo vem da legislação e só o Ambiente Nacional o confere: este programa não o calcula. Fale com o seu contador.",
        "573" => "A SEFAZ diz que esta resposta já estava registrada para a nota. Nada mudou por causa deste envio.",
        _ => "Nada foi registrado e a nota continua como estava. Confira o que a SEFAZ aponta e responda de novo.",
    }
}

impl DesfechoNfe {
    /// O desfecho do que `manifestar` devolveu. Os mesmos cinco desfechos do cancelamento: registrada, recusada pela SEFAZ,
    /// confirmada como NAO registrada, sem resposta e recusada antes de enviar.
    #[must_use]
    pub fn da_manifestacao(r: Result<Manifestada, Erro>, tipo: TipoManifestacao) -> Self {
        let m = match r {
            Ok(m) => m,
            Err(e) => return Self::do_erro(&e),
        };
        let sem_pasta = (!m.guardado && m.situacao.is_none()).then(|| {
            "A resposta ficou só na SEFAZ: a nota não está na pasta de notas recebidas.".to_string()
        });
        match &m.resultado.desfecho {
            DesfechoEvento::Registrado { protocolo, .. } => Self::Registrado {
                protocolo: protocolo.clone(),
                recuperado: m.resultado.recuperado,
                aviso: sem_pasta,
            },
            DesfechoEvento::Rejeitado { cstat, motivo }
            | DesfechoEvento::PrazoVencido { cstat, motivo } => Self::Rejeitado {
                codigo: cstat.clone(),
                descricao: frase(motivo),
                orientacao: orientacao_da_recusa(cstat).into(),
            },
            DesfechoEvento::SemVinculo { motivo, .. } => Self::Rejeitado {
                codigo: "136".into(),
                descricao: frase(motivo),
                orientacao: "A SEFAZ guardou a resposta, mas a nota não existia lá no momento. Confira a chave e tente de novo.".into(),
            },
            DesfechoEvento::NaoRegistrado => Self::NaoRegistrado,
            DesfechoEvento::Indisponivel { motivo, .. } => Self::SemResposta {
                texto: format!(
                    "{} Nada foi registrado: o serviço está parado. Tente de novo mais tarde.",
                    frase(motivo)
                ),
            },
            _ => Self::SemResposta {
                texto: format!(
                    "A SEFAZ não confirmou {}. Ela pode ter sido registrada: tente de novo (se já estiver registrada, a SEFAZ avisa que é repetida e nada muda).",
                    texto_da_resposta(tipo)
                ),
            },
        }
    }
}

/// "a ciência", "a confirmação"...: o que o texto de "sem resposta" diz que nao foi confirmado.
fn texto_da_resposta(tipo: TipoManifestacao) -> &'static str {
    match tipo {
        TipoManifestacao::Confirmacao => "a confirmação",
        TipoManifestacao::Ciencia => "a ciência",
        TipoManifestacao::Desconhecimento => "o desconhecimento",
        _ => "a resposta",
    }
}

/// A resposta que ficou na pasta depois de um pedido registrado.
#[must_use]
pub fn resposta_registrada(tipo: TipoManifestacao) -> RespostaDada {
    RespostaDada::do_codigo(tipo.codigo())
}

#[cfg(test)]
mod testes {
    use super::*;
    use rustnf::nfe::sefaz::dfe::MotivoEspera;

    fn instante(texto: &str) -> DateTime<Utc> {
        texto.parse().unwrap()
    }

    fn busca(desfecho: &DesfechoDfe, espera: Option<DateTime<Utc>>, novas: usize) -> DesfechoBusca {
        DesfechoBusca::de_partes(
            desfecho,
            espera,
            Contagem {
                resumos: novas,
                ..Contagem::default()
            },
            false,
            "frase da biblioteca",
            novas,
        )
    }

    #[test]
    fn notas_novas_com_a_fila_em_dia_dizem_a_hora_da_proxima_busca() {
        let d = busca(
            &DesfechoDfe::EmDia,
            Some(instante("2026-10-03T13:00:00Z")),
            3,
        );
        assert_eq!(d.titulo(), "3 notas novas");
        assert!(d.texto().contains("10:00"), "{}", d.texto());
        assert!(d.texto().contains("em dia"));
        assert_eq!(d.gravidade(), Gravidade::Ok);
    }

    #[test]
    fn uma_nota_nova_no_singular_e_fila_parcial_manda_buscar_de_novo() {
        let d = busca(&DesfechoDfe::Parcial, None, 1);
        assert_eq!(d.titulo(), "1 nota nova");
        assert!(d.texto().contains("busque de novo"), "{}", d.texto());
    }

    #[test]
    fn nada_novo_diz_que_a_fila_esta_em_dia() {
        let d = busca(
            &DesfechoDfe::EmDia,
            Some(instante("2026-10-03T13:00:00Z")),
            0,
        );
        assert_eq!(d.titulo(), "Nada novo");
        assert!(d.texto().contains("10:00"));
        assert_eq!(d.gravidade(), Gravidade::Neutro);
    }

    #[test]
    fn a_espera_e_o_bloqueio_dizem_o_horario_e_nao_sao_falha() {
        let ate = instante("2026-10-03T13:00:00Z");
        let espera = busca(
            &DesfechoDfe::Aguarde {
                ate,
                motivo: MotivoEspera::FilaEmDia,
            },
            Some(ate),
            0,
        );
        assert!(espera.titulo().contains("10:00"), "{}", espera.titulo());
        assert_eq!(espera.gravidade(), Gravidade::Aviso);
        let bloqueio = busca(&DesfechoDfe::ConsumoIndevido { ate }, Some(ate), 0);
        assert!(bloqueio.texto().contains("10:00"), "{}", bloqueio.texto());
        assert!(bloqueio.texto().contains("Nada foi perdido"));
    }

    #[test]
    fn recusa_do_ambiente_nacional_sem_resposta_e_erro_antes_de_enviar() {
        let rec = busca(
            &DesfechoDfe::Rejeitada {
                cstat: "999".into(),
                motivo: "rejeicao de teste".into(),
            },
            None,
            0,
        );
        assert!(rec.texto().contains("código 999"), "{}", rec.texto());
        assert_eq!(rec.gravidade(), Gravidade::Erro);
        let sem = busca(&DesfechoDfe::Indisponivel, None, 0);
        assert!(sem.texto().contains("Nada foi perdido"));
        let rede = DesfechoBusca::da_busca(Err(Erro::Rede("tempo esgotado".into())), 0);
        assert!(matches!(rede, DesfechoBusca::SemResposta { .. }));
        let uso = DesfechoBusca::da_busca(Err(Erro::Uso("informe o CNPJ".into())), 0);
        assert!(matches!(uso, DesfechoBusca::Recusado { .. }));
        assert_eq!(uso.titulo(), "Nada foi enviado");
    }

    #[test]
    fn a_recusa_de_prazo_nao_inventa_prazo() {
        let t = orientacao_da_recusa("596");
        assert!(t.contains("não o calcula"));
        assert!(!t.chars().any(|c| c.is_ascii_digit()), "{t}");
    }

    #[test]
    fn a_resposta_registrada_vira_a_resposta_da_lista() {
        assert_eq!(
            resposta_registrada(TipoManifestacao::Confirmacao),
            RespostaDada::Confirmada
        );
        assert_eq!(
            resposta_registrada(TipoManifestacao::NaoRealizada),
            RespostaDada::NaoRealizada
        );
    }
}
