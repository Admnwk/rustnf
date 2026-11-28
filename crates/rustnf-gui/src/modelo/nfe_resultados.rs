// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O que a SEFAZ respondeu a "Conferir na SEFAZ" (consulta pela chave) e a "Retomar" (a nota enviada sem resposta),
//! como tipo da tela, com o texto em portugues e o que fazer. Sem janela.

use rustnf::nfe::sefaz::Desfecho;
use rustnf_app::erro::Erro;
use rustnf_app::nfe::{Consultada, Retomada};
use rustnf_app::notas::nfe::Situacao;

use super::certificado::Gravidade;
use super::mascara::frase;

/// "1 nota segue" / "3 notas seguem": o numero e a forma certa, sem o "(s)".
#[must_use]
pub fn contagem(n: usize, um: &str, varios: &str) -> String {
    if n == 1 {
        format!("1 {um}")
    } else {
        format!("{n} {varios}")
    }
}

/// O que a consulta achou.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsultaNfe {
    /// A cor do aviso.
    pub gravidade: Gravidade,
    /// Uma linha.
    pub titulo: String,
    /// O que aconteceu e o que fazer.
    pub texto: String,
    /// O codigo da SEFAZ, em segundo plano (vazio se nao ha).
    pub codigo: String,
    /// O protocolo, quando a nota esta autorizada.
    pub protocolo: String,
    /// A situacao da nota na pasta depois da consulta; `None` = a nota nao esta na pasta ou saiu dela.
    pub situacao: Option<Situacao>,
    /// A consulta mudou ou pode ter mudado a situacao da nota na pasta (a tela le a pasta de novo).
    pub mexeu_na_pasta: bool,
}

impl ConsultaNfe {
    fn nova(g: Gravidade, titulo: &str, texto: impl Into<String>) -> Self {
        Self {
            gravidade: g,
            titulo: titulo.into(),
            texto: texto.into(),
            codigo: String::new(),
            protocolo: String::new(),
            situacao: None,
            mexeu_na_pasta: false,
        }
    }

    /// O resultado de `consultar`.
    #[must_use]
    pub fn do_resultado(r: Result<Consultada, Erro>) -> Self {
        let c = match r {
            Ok(c) => c,
            Err(Erro::Rede(t)) => {
                return Self::nova(
                    Gravidade::Aviso,
                    "Sem resposta da SEFAZ",
                    format!(
                        "{} Não emita outra nota no lugar: consulte de novo daqui a pouco.",
                        frase(&t)
                    ),
                )
            }
            Err(e) => {
                return Self::nova(
                    Gravidade::Erro,
                    "Nada foi consultado",
                    format!("Nada foi enviado. {}", frase(&e.mensagem())),
                )
            }
        };
        let mut s = match &c.resultado.desfecho {
            Desfecho::Autorizada {
                cstat, protocolo, ..
            } => {
                let mut s = Self::nova(
                    Gravidade::Ok,
                    "A nota está autorizada",
                    if c.resultado.recuperada {
                        "A SEFAZ confirma que a nota vale. A resposta do envio tinha se perdido e a consulta a achou: nenhuma nota nova foi criada."
                    } else {
                        "A SEFAZ confirma que a nota vale."
                    },
                );
                s.codigo.clone_from(cstat);
                s.protocolo.clone_from(protocolo);
                s
            }
            Desfecho::Cancelada { cstat, .. } => {
                let mut s = Self::nova(
                    Gravidade::Aviso,
                    "A nota está cancelada",
                    "A SEFAZ informa que esta nota foi cancelada: ela não vale mais.",
                );
                s.codigo.clone_from(cstat);
                s
            }
            Desfecho::Denegada { cstat, motivo, .. } => {
                let mut s = Self::nova(
                    Gravidade::Erro,
                    "Uso denegado",
                    format!("{} A nota não vale e o número foi gasto.", frase(motivo)),
                );
                s.codigo.clone_from(cstat);
                s
            }
            Desfecho::Rejeitada { cstat, motivo } => {
                let mut s = Self::nova(
                    Gravidade::Erro,
                    "A SEFAZ recusou a nota",
                    format!("{} A nota não existe fiscalmente. Se ela estava como sem resposta, saiu da lista e o número volta a ficar livre.", frase(motivo)),
                );
                s.codigo.clone_from(cstat);
                s
            }
            Desfecho::NumeroJaUsado { motivo, .. } => Self::nova(
                Gravidade::Erro,
                "Número já usado",
                frase(motivo),
            ),
            Desfecho::EmProcessamento { .. } => Self::nova(
                Gravidade::Aviso,
                "A SEFAZ ainda está processando",
                "Consulte de novo em instantes.",
            ),
            Desfecho::NaoRecebida => Self::nova(
                Gravidade::Aviso,
                "A SEFAZ não recebeu esta nota",
                "A SEFAZ não tem esta chave. Se a nota está como sem resposta na lista, use Retomar: o programa reenvia a mesma nota, nunca uma nova.",
            ),
            Desfecho::SoEpec { motivo } => Self::nova(
                Gravidade::Aviso,
                "Só o EPEC está registrado",
                frase(motivo),
            ),
            Desfecho::Indisponivel { motivo, .. } => Self::nova(
                Gravidade::Aviso,
                "Serviço da SEFAZ parado",
                format!("{} Tente de novo mais tarde.", frase(motivo)),
            ),
            Desfecho::ConsultaRecusada { cstat, motivo } => {
                let mut s = Self::nova(
                    Gravidade::Aviso,
                    "A SEFAZ não aceitou a consulta",
                    format!(
                        "{} Isso não diz nada da nota: ela continua como estava na lista. Consulte de novo mais tarde e não emita outra no lugar.",
                        frase(motivo)
                    ),
                );
                s.codigo.clone_from(cstat);
                s
            }
            _ => Self::nova(
                Gravidade::Aviso,
                "Sem resposta da SEFAZ",
                "A SEFAZ não respondeu. Não emita outra nota no lugar: consulte de novo daqui a pouco.",
            ),
        };
        s.situacao = c.situacao;
        s.mexeu_na_pasta = true;
        s
    }
}

/// O que a retomada fez com uma nota.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinhaDaRetomada {
    /// O numero da nota.
    pub numero: String,
    /// O que aconteceu.
    pub texto: String,
    /// A cor.
    pub gravidade: Gravidade,
    /// A situacao depois; `None` = saiu da pasta (recusada).
    pub situacao: Option<Situacao>,
}

/// O que a retomada fez com o lote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumoDaRetomada {
    /// A cor do resumo.
    pub gravidade: Gravidade,
    /// A frase do resumo.
    pub texto: String,
    /// Uma linha por nota.
    pub linhas: Vec<LinhaDaRetomada>,
}

impl ResumoDaRetomada {
    /// O resultado de `retomar`.
    #[must_use]
    pub fn do_resultado(r: Result<Vec<Retomada>, Erro>) -> Self {
        let feitos = match r {
            Ok(f) => f,
            Err(e) => {
                return Self {
                    gravidade: Gravidade::Erro,
                    texto: format!("Nada foi enviado. {}", frase(&e.mensagem())),
                    linhas: Vec::new(),
                }
            }
        };
        if feitos.is_empty() {
            return Self {
                gravidade: Gravidade::Neutro,
                texto: "Não há nota sem resposta para retomar.".into(),
                linhas: Vec::new(),
            };
        }
        let linhas: Vec<LinhaDaRetomada> = feitos.iter().map(linha).collect();
        let sem = linhas
            .iter()
            .filter(|l| matches!(l.situacao, Some(Situacao::Pendente | Situacao::Epec)))
            .count();
        let recusadas = linhas
            .iter()
            .filter(|l| l.gravidade == Gravidade::Erro)
            .count();
        let (gravidade, texto) = if sem > 0 {
            (
                Gravidade::Aviso,
                format!(
                    "{} sem resposta da SEFAZ: tente de novo mais tarde e não emita outra nota no lugar.",
                    contagem(sem, "nota segue", "notas seguem")
                ),
            )
        } else if recusadas > 0 {
            (
                Gravidade::Erro,
                format!(
                    "{} pela SEFAZ.",
                    contagem(
                        recusadas,
                        "nota foi recusada ou denegada",
                        "notas foram recusadas ou denegadas"
                    )
                ),
            )
        } else {
            (
                Gravidade::Ok,
                "Todas as notas foram resolvidas pela SEFAZ.".to_string(),
            )
        };
        Self {
            gravidade,
            texto,
            linhas,
        }
    }
}

fn linha(f: &Retomada) -> LinhaDaRetomada {
    let (gravidade, texto) = match (f.status, f.situacao) {
        ("autorizada", _) => (
            Gravidade::Ok,
            if f.acao == "reenviou" || f.acao == "transmitiu" {
                "a SEFAZ não a tinha recebido: a mesma nota foi reenviada e autorizada"
            } else {
                "a SEFAZ já a tinha autorizado: agora a pasta está em dia"
            },
        ),
        ("cancelada", _) => (Gravidade::Aviso, "a SEFAZ informa que foi cancelada"),
        ("consulta_recusada", _) => (
            Gravidade::Aviso,
            "a SEFAZ não aceitou a consulta agora: a nota segue como estava; tente de novo mais tarde, sem emitir outra",
        ),
        ("denegada", _) => (Gravidade::Erro, "a SEFAZ negou o uso desta nota"),
        // `situacao` None = a recusa tirou a nota da pasta (`rustnf-app/src/nfe/retomada.rs:34`); a NFC-e off-line
        // recusada continua na pasta, pendente com a recusa anotada
        ("rejeitada" | "numero_ja_usado", None) => (
            Gravidade::Erro,
            "a SEFAZ recusou: a nota saiu da lista e o número voltou a ficar livre",
        ),
        ("rejeitada" | "numero_ja_usado", Some(_)) => (
            Gravidade::Erro,
            "a SEFAZ recusou a transmissão: a nota segue pendente com a mesma chave; corrija o que ela apontou e transmita de novo",
        ),
        (_, Some(Situacao::Pendente | Situacao::Epec)) => (
            Gravidade::Aviso,
            "segue sem resposta: tente de novo mais tarde, sem emitir outra nota",
        ),
        _ => (Gravidade::Neutro, "sem mudança"),
    };
    LinhaDaRetomada {
        numero: f.numero.clone(),
        texto: texto.to_string(),
        gravidade,
        situacao: f.situacao,
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use serde_json::Value;

    #[test]
    fn contagem_usa_o_singular_so_para_um() {
        assert_eq!(contagem(1, "nota segue", "notas seguem"), "1 nota segue");
        assert_eq!(contagem(0, "nota segue", "notas seguem"), "0 notas seguem");
        assert_eq!(contagem(3, "nota segue", "notas seguem"), "3 notas seguem");
    }

    fn feita(status: &'static str, acao: &'static str, s: Option<Situacao>) -> Retomada {
        Retomada {
            chave: "k".into(),
            serie: "1".into(),
            numero: "7".into(),
            acao,
            resultado: Value::Null,
            situacao: s,
            aviso: None,
            status,
        }
    }

    #[test]
    fn recusa_que_deixa_a_nota_na_pasta_nao_diz_que_o_numero_ficou_livre() {
        let saiu = linha(&feita("rejeitada", "consultou", None));
        assert!(
            saiu.texto.contains("número voltou a ficar livre"),
            "{saiu:?}"
        );
        let ficou = linha(&feita("rejeitada", "transmitiu", Some(Situacao::Pendente)));
        assert!(
            ficou.texto.contains("segue pendente com a mesma chave"),
            "{ficou:?}"
        );
        assert!(!ficou.texto.contains("livre"), "{ficou:?}");
    }

    #[test]
    fn consulta_recusada_na_retomada_diz_que_a_nota_segue_e_nunca_que_o_numero_ficou_livre() {
        let l = linha(&feita(
            "consulta_recusada",
            "consultou",
            Some(Situacao::Pendente),
        ));
        assert_eq!(l.gravidade, Gravidade::Aviso);
        assert!(l.texto.contains("segue como estava"), "{l:?}");
        assert!(!l.texto.contains("livre"), "{l:?}");
    }

    #[test]
    fn retomar_diz_o_que_aconteceu_com_cada_nota() {
        let r = ResumoDaRetomada::do_resultado(Ok(vec![feita(
            "autorizada",
            "consultou",
            Some(Situacao::Autorizada),
        )]));
        assert_eq!(r.gravidade, Gravidade::Ok);
        assert!(r.linhas[0].texto.contains("já a tinha autorizado"));
        let r = ResumoDaRetomada::do_resultado(Ok(vec![feita(
            "sem_resposta",
            "consultou",
            Some(Situacao::Pendente),
        )]));
        assert_eq!(r.gravidade, Gravidade::Aviso);
        assert!(r.texto.contains("não emita outra nota"), "{}", r.texto);
        let r = ResumoDaRetomada::do_resultado(Ok(Vec::new()));
        assert!(r.texto.contains("Não há nota"));
        let r = ResumoDaRetomada::do_resultado(Err(Erro::Uso("sem pasta".into())));
        assert_eq!(r.gravidade, Gravidade::Erro);
    }

    #[test]
    fn consulta_com_erro_de_uso_nao_diz_que_a_sefaz_respondeu() {
        let c = ConsultaNfe::do_resultado(Err(Erro::recusa("chNFe", "chave invalida")));
        assert_eq!(c.gravidade, Gravidade::Erro);
        assert!(c.texto.starts_with("Nada foi enviado"));
        let c = ConsultaNfe::do_resultado(Err(Erro::Rede("tempo esgotado".into())));
        assert_eq!(c.gravidade, Gravidade::Aviso);
        assert!(c.texto.contains("Não emita outra nota"));
    }
}
