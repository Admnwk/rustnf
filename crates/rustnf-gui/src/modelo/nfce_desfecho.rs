// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Como terminou a venda do caixa: autorizada (o cupom sai), rejeitada (o motivo em palavras; o numero volta livre), sem
//! resposta (fica pendente e se retoma, nunca "emitir de novo"), SEFAZ parada, recusada antes de enviar e emitida em
//! contingencia off-line (o cupom vale e a nota tem prazo para ser transmitida). Sem janela.
//!
//! Prazo da off-line: o fim do primeiro dia util depois da emissao (MOC 7.00, Anexo IV, itens 2 e 3); a aplicacao o calcula
//! (`rustnf_app::nfce::Gravada::prazo`) e esta tela so' o repete. Depois do prazo a SEFAZ ainda autoriza (cStat 150), mas o
//! emitente fica em falta (`rustnf_app::nfce::pendentes`).

use rustnf::nfe::sefaz::Desfecho;
use rustnf_app::erro::Erro;
use rustnf_app::nfce::{Emissao, Gravada, Pendente, Prazo};
use rustnf_app::nfe::Emitida;
use rustnf_app::notas::nfe::Situacao;

use super::mascara::frase;
use super::nfce_caixa::PendenciaCaixa;
use super::nfe_emissao::erros_da_recusa;
use super::nfe_resultados::contagem;

/// Como terminou a venda.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DesfechoCaixa {
    /// A SEFAZ autorizou: o cupom ja pode sair.
    Autorizada {
        /// A chave de acesso.
        chave: String,
        /// O numero.
        numero: String,
        /// A serie.
        serie: String,
        /// O protocolo.
        protocolo: String,
        /// A resposta se perdeu e a consulta achou a nota (sem nota nova).
        recuperada: bool,
    },
    /// A SEFAZ recusou: o numero voltou a ficar livre.
    Rejeitada {
        /// O codigo da SEFAZ.
        cstat: String,
        /// O motivo, em palavras.
        motivo: String,
        /// O numero que continua livre.
        numero: String,
    },
    /// A SEFAZ negou o uso: o numero foi gasto e a nota nao vale.
    Denegada {
        /// O motivo.
        motivo: String,
    },
    /// O numero ja estava autorizado com outra chave.
    NumeroJaUsado {
        /// A frase.
        texto: String,
    },
    /// Enviada e sem resposta: a nota PODE ter sido autorizada. Fica pendente; o caminho e' o Retomar.
    SemResposta {
        /// A chave da nota pendente.
        chave: String,
        /// O numero.
        numero: String,
        /// A serie.
        serie: String,
    },
    /// A SEFAZ respondeu que o servico esta parado depois do envio: nada foi processado e a nota fica pendente.
    SefazFora {
        /// A chave da nota pendente.
        chave: String,
        /// O numero.
        numero: String,
        /// A serie.
        serie: String,
        /// O que a SEFAZ disse.
        motivo: String,
    },
    /// Emitida em contingencia off-line: o cupom vale e a nota tem de ser transmitida ate o prazo.
    Offline {
        /// A chave.
        chave: String,
        /// O numero.
        numero: String,
        /// A serie.
        serie: String,
        /// O fim do prazo, `dd/mm/aaaa hh:mm`.
        prazo: String,
    },
    /// Recusada antes de enviar (regra da biblioteca, pasta, certificado, CSC): nada saiu e nenhum numero foi gasto.
    Recusada {
        /// As pendencias.
        pendencias: Vec<PendenciaCaixa>,
    },
}

impl DesfechoCaixa {
    /// O desfecho do que `rustnf_app::nfce::emitir` devolveu.
    #[must_use]
    pub fn de(r: Result<Emissao, Erro>) -> Self {
        match r {
            Ok(Emissao::Enviada(e)) => Self::da_enviada(&e),
            Ok(Emissao::Offline(g)) => Self::da_offline(&g),
            Err(e) => Self::do_erro(&e),
        }
    }

    fn da_offline(g: &Gravada) -> Self {
        Self::Offline {
            chave: g.chave.clone(),
            numero: g.numero.to_string(),
            serie: g.serie.clone(),
            prazo: g.prazo.clone(),
        }
    }

    fn da_enviada(e: &Emitida) -> Self {
        let (numero, serie, chave) = (e.numero.to_string(), e.serie.clone(), e.chave.clone());
        match &e.resultado.desfecho {
            Desfecho::Autorizada { protocolo, .. } => Self::Autorizada {
                chave,
                numero,
                serie,
                protocolo: protocolo.clone(),
                recuperada: e.resultado.recuperada,
            },
            Desfecho::Rejeitada { cstat, motivo } => Self::Rejeitada {
                cstat: cstat.clone(),
                motivo: sem_codigo(motivo),
                numero,
            },
            Desfecho::Denegada { motivo, .. } => Self::Denegada {
                motivo: frase(motivo),
            },
            Desfecho::NumeroJaUsado { motivo, .. } => Self::NumeroJaUsado {
                texto: format!(
                    "{} Esta venda não saiu com este número: olhe a lista de notas antes de tentar de novo.",
                    frase(motivo)
                ),
            },
            Desfecho::Indisponivel { motivo, .. } => Self::SefazFora {
                chave,
                numero,
                serie,
                motivo: frase(motivo),
            },
            _ => Self::SemResposta {
                chave,
                numero,
                serie,
            },
        }
    }

    /// Recusa antes de enviar: o texto da aplicacao em pendencias, com o que fazer no caixa (nada de passo de assistente).
    #[must_use]
    pub fn do_erro(e: &Erro) -> Self {
        let pendencias = erros_da_recusa(e)
            .into_iter()
            .map(|p| {
                let m = p.mensagem.to_lowercase();
                let como = if m.contains("pendente") {
                    "Abra \"Fechar o caixa\" e transmita as notas por transmitir antes de vender de novo."
                } else if m.contains("csc") || m.contains("qr code") {
                    "Guarde o CSC em Configurações, NFC-e (ou use o QR Code versão 3, que não usa CSC)."
                } else if m.contains("ncm") {
                    "Corrija o NCM do produto em Produtos e coloque-o de novo no cupom."
                } else if m.contains("ja foi gasto") || m.contains("foi inutilizado") {
                    "O número será o próximo livre: tente de novo."
                } else {
                    "Corrija o ponto indicado e emita de novo: nada saiu da sua máquina e nenhum número foi gasto."
                };
                PendenciaCaixa {
                    mensagem: p.mensagem,
                    como_resolver: como.into(),
                }
            })
            .collect();
        Self::Recusada { pendencias }
    }

    /// Fica uma nota na pasta que ainda depende da SEFAZ (pendente ou off-line): a tela nao deixa o cupom ser "emitido de novo".
    #[must_use]
    pub fn deixa_pendente(&self) -> bool {
        matches!(
            self,
            Self::SemResposta { .. } | Self::SefazFora { .. } | Self::Offline { .. }
        )
    }

    /// A venda esta feita e o cupom existe (autorizada ou off-line).
    #[must_use]
    pub fn tem_cupom(&self) -> bool {
        matches!(self, Self::Autorizada { .. } | Self::Offline { .. })
    }

    /// A chave da nota, se ha nota na pasta.
    #[must_use]
    pub fn chave(&self) -> Option<&str> {
        match self {
            Self::Autorizada { chave, .. }
            | Self::SemResposta { chave, .. }
            | Self::SefazFora { chave, .. }
            | Self::Offline { chave, .. } => Some(chave),
            _ => None,
        }
    }

    /// O titulo curto.
    #[must_use]
    pub fn titulo(&self) -> String {
        match self {
            Self::Autorizada { numero, .. } => format!("NFC-e nº {numero} autorizada"),
            Self::Rejeitada { .. } => "A SEFAZ não aceitou a venda".into(),
            Self::Denegada { .. } => "Uso denegado".into(),
            Self::NumeroJaUsado { .. } => "Número já usado".into(),
            Self::SemResposta { numero, .. } => format!("NFC-e nº {numero} sem resposta da SEFAZ"),
            Self::SefazFora { numero, .. } => format!("NFC-e nº {numero}: SEFAZ parada"),
            Self::Offline { numero, .. } => format!("NFC-e nº {numero} emitida sem internet"),
            Self::Recusada { .. } => "A venda ainda não pode ser emitida".into(),
        }
    }

    /// O que aconteceu e o que fazer, em uma ou duas frases.
    #[must_use]
    pub fn texto(&self) -> String {
        match self {
            Self::Autorizada { recuperada, .. } => if *recuperada {
                "A resposta do envio tinha se perdido e a consulta achou a nota autorizada: nenhuma nota nova foi criada. O cupom já pode sair."
            } else {
                "A SEFAZ autorizou. O cupom já pode sair: abra-o para imprimir ou salve-o."
            }
            .into(),
            Self::Rejeitada { cstat, motivo, numero } => format!(
                "{motivo} (código {cstat}). Nada foi vendido fiscalmente e o número {numero} continua livre. Corrija o que a SEFAZ apontou e emita de novo: a venda continua no cupom."
            ),
            Self::Denegada { motivo } => format!("{motivo} A nota não vale e o número foi gasto."),
            Self::NumeroJaUsado { texto } => texto.clone(),
            Self::SemResposta { .. } => "A SEFAZ não respondeu e não foi possível confirmar se a nota foi autorizada: ela PODE ter sido. A nota ficou guardada como pendente. Use Retomar: o programa consulta a SEFAZ pela MESMA chave e, só se ela não tiver recebido, reenvia a MESMA nota. Não emita outro cupom para esta venda.".into(),
            Self::SefazFora { motivo, .. } => format!(
                "{motivo} A SEFAZ informou que o serviço está parado e não processou nada. A nota ficou guardada como pendente: use Retomar quando o serviço voltar. Não emita outro cupom para esta venda."
            ),
            Self::Offline { prazo, .. } => format!(
                "A SEFAZ não respondeu antes do envio, então a venda foi emitida em contingência (sem internet). O cupom vale: entregue-o ao cliente. A nota ainda não foi enviada à SEFAZ: transmita-a até {prazo} (use \"Fechar o caixa\")."
            ),
            Self::Recusada { pendencias } => pendencias
                .first()
                .map(|p| format!("{} {}", p.mensagem, p.como_resolver))
                .unwrap_or_default(),
        }
    }
}

/// O texto sem o codigo e sem o "Rejeicao:".
fn sem_codigo(texto: &str) -> String {
    let t = texto
        .split_once('-')
        .filter(|(c, r)| {
            c.len() <= 4 && c.bytes().all(|b| b.is_ascii_digit()) && r.starts_with("Rejei")
        })
        .map_or(texto, |(_, r)| r);
    let t = t
        .strip_prefix("Rejeicao:")
        .or_else(|| t.strip_prefix("Rejeição:"))
        .unwrap_or(t);
    frase(t)
}

/// A nota pendente do fechamento, em linha de tela: o que e' e o que fazer, com o prazo da off-line em palavras.
#[must_use]
pub fn frase_do_pendente(p: &Pendente) -> String {
    let r = &p.registro;
    let nota = format!("NFC-e nº {} (série {})", r.numero, r.serie);
    if !r.recusa.is_empty() {
        return format!(
            "{nota}: a SEFAZ recusou a transmissão ({}). A nota segue pendente com a mesma chave: corrija o que ela apontou e transmita de novo.",
            r.recusa
        );
    }
    match p.prazo {
        Prazo::Sem => format!(
            "{nota}: foi enviada e a SEFAZ não respondeu; ela PODE ter sido autorizada. Transmitir consulta a mesma chave: não emita outro cupom."
        ),
        Prazo::Folga(_) => format!(
            "{nota}: emitida sem internet; transmita até {}.",
            p.limite
        ),
        Prazo::AVencer(_) => format!(
            "{nota}: emitida sem internet e o prazo acaba em breve (até {}): transmita agora.",
            p.limite
        ),
        Prazo::Vencido(_) => format!(
            "{nota}: emitida sem internet e o prazo VENCEU em {}. Transmita assim mesmo: a SEFAZ costuma autorizar fora do prazo, mas a falta fica registrada.",
            p.limite
        ),
    }
}

/// O aviso de fechamento do caixa: o da aplicacao, trocado para a linguagem da janela. `None` sem nota por transmitir.
#[must_use]
pub fn aviso_do_fechamento(lista: &[Pendente]) -> Option<String> {
    let n = lista.len();
    if n == 0 {
        return None;
    }
    let mais_curto = lista
        .iter()
        .find(|p| p.registro.offline && !p.limite.is_empty());
    let quando = match mais_curto {
        Some(p) if matches!(p.prazo, Prazo::Vencido(_)) => {
            " O prazo da mais antiga JÁ VENCEU: faça isso agora.".to_string()
        }
        Some(p) => format!(" Transmita antes de {}.", p.limite),
        None => String::new(),
    };
    Some(format!(
        "{} para transmitir à SEFAZ.{quando} Você pode fechar o caixa assim, mas a nota só fica completa depois de transmitida.",
        contagem(n, "NFC-e", "NFC-e")
    ))
}

/// A situacao que o fechamento mostra para a nota.
#[must_use]
pub fn rotulo_do_pendente(p: &Pendente) -> &'static str {
    if p.registro.situacao != Situacao::Pendente {
        return "Aguardando";
    }
    if p.registro.offline {
        "Sem internet (off-line)"
    } else {
        "Sem resposta"
    }
}

#[cfg(test)]
mod testes;
