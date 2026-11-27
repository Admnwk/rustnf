// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O que a conferencia e a emissao da NF-e dizem, como tipo da tela e em portugues comum: os erros e avisos da montagem
//! (`rustnf_app::nfe::montar_pedido`) viram pendencias com o que fazer e o passo onde se corrige, e o desfecho do envio
//! (autorizada, rejeitada, sem resposta, SEFAZ fora do ar, recusada antes de enviar, EPEC) vira texto com o que fazer.
//! Nenhuma frase leva caminho de arquivo, senha, XML assinado nem detalhe de rede. Sem janela.

use rustnf::nfe::montagem::{Aviso, TipoAviso};
use rustnf::nfe::sefaz::{Desfecho, DesfechoEvento};
use rustnf_app::erro::Erro;
use rustnf_app::nfe::{Emitida, Montado, Registrado};
use rustnf_app::notas::nfe::Situacao;

use super::mascara::{decimal_para_dinheiro, frase};
use super::nfe_nova::{PassoNfe, PendenciaNfe};

/// Quanto tempo a NF-e em EPEC tem para ser transmitida a SEFAZ da UF depois do EPEC: 168 horas (MOC 7.00, Visao Geral,
/// 3.3.6, como a retomada da aplicacao anota).
pub const HORAS_PARA_TRANSMITIR_O_EPEC: u32 = 168;

/// Um aviso da montagem: nao impede a nota.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvisoNfe {
    /// O produto (posicao na lista, a partir de 0), se o aviso e' de um deles.
    pub item: Option<usize>,
    /// O passo onde se ajusta.
    pub passo: PassoNfe,
    /// O que e'.
    pub texto: String,
}

/// O que a conferencia achou.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConferenciaNfe {
    /// O que impede o envio.
    pub erros: Vec<PendenciaNfe>,
    /// O que merece atencao.
    pub avisos: Vec<AvisoNfe>,
    /// O total da nota, em dinheiro brasileiro (vazio se nao montou).
    pub total: String,
    /// Os impostos que a montagem calculou: nome e valor, so' os que tem valor.
    pub impostos: Vec<(String, String)>,
    /// O numero da nota (o da pasta de notas, ainda nao gasto).
    pub numero: u64,
    /// A serie.
    pub serie: String,
}

impl ConferenciaNfe {
    /// Pode enviar: montou e nao ha erro.
    #[must_use]
    pub fn limpa(&self) -> bool {
        self.erros.is_empty() && !self.total.is_empty()
    }
}

/// Os impostos do `ICMSTot` que a tela mostra, com o nome de quem vende.
const IMPOSTOS: [(&str, &str); 8] = [
    ("vICMS", "ICMS"),
    ("vICMSUFDest", "ICMS para o estado do cliente (DIFAL)"),
    (
        "vFCPUFDest",
        "Fundo de combate à pobreza do estado do cliente",
    ),
    ("vST", "ICMS cobrado por substituição"),
    ("vIPI", "IPI"),
    ("vPIS", "PIS"),
    ("vCOFINS", "COFINS"),
    ("vTotTrib", "Tributos aproximados"),
];

/// A conferencia de uma montagem.
#[must_use]
pub fn conferencia(r: Result<Montado, Erro>) -> ConferenciaNfe {
    match r {
        Ok(m) => {
            let impostos = IMPOSTOS
                .iter()
                .filter_map(|(tag, nome)| {
                    let v = m.totais.get(*tag)?;
                    if v.trim_matches(['0', '.']).is_empty() {
                        None
                    } else {
                        Some(((*nome).to_string(), decimal_para_dinheiro(v)))
                    }
                })
                .collect();
            ConferenciaNfe {
                erros: Vec::new(),
                avisos: m.avisos.iter().map(aviso).collect(),
                total: m
                    .totais
                    .get("vNF")
                    .map(|v| decimal_para_dinheiro(v))
                    .unwrap_or_default(),
                impostos,
                numero: m.numero,
                serie: m.serie,
            }
        }
        Err(e) => ConferenciaNfe {
            erros: erros_da_recusa(&e),
            ..ConferenciaNfe::default()
        },
    }
}

fn aviso(a: &Aviso) -> AvisoNfe {
    let item = a.item.map(|i| i.saturating_sub(1));
    let (passo, texto) = match a.tipo {
        TipoAviso::PisCofinsRegime => (
            PassoNfe::OQueVendeu,
            format!(
                "O PIS ou a COFINS não combina com o regime da empresa: {} Confira nos detalhes fiscais do produto.",
                frase(&a.mensagem)
            ),
        ),
        TipoAviso::SimplesSemGIbscbs => (
            PassoNfe::OQueVendeu,
            "A SEFAZ pode recusar esta nota por causa do IBS e da CBS (a regra do Simples ainda não esta fechada entre as duas notas tecnicas). Se recusar, o motivo aparece aqui e o número continua livre.".to_string(),
        ),
        TipoAviso::CfopForaDaTabela => (
            PassoNfe::OQueVendeu,
            format!(
                "O CFOP não está na tabela do Portal da NF-e ou está fora de vigência: {} A SEFAZ da sua UF decide se aceita.",
                frase(&a.mensagem)
            ),
        ),
        _ => (PassoNfe::OQueVendeu, frase(&a.mensagem)),
    };
    AvisoNfe { item, passo, texto }
}

/// O erro de uma recusa (antes de enviar) em pendencias: cada rejeicao que a biblioteca lista vira uma.
#[must_use]
pub fn erros_da_recusa(e: &Erro) -> Vec<PendenciaNfe> {
    let texto = match e {
        Erro::Recusa { mensagem, .. } => mensagem.clone(),
        outro => outro.mensagem(),
    };
    let mut lista: Vec<PendenciaNfe> = texto
        .split("; ")
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(traduzir)
        .collect();
    lista.dedup();
    if lista.is_empty() {
        lista.push(PendenciaNfe {
            passo: PassoNfe::Conferir,
            item: None,
            mensagem: "A conferência não aceitou o pedido.".into(),
            como_resolver: "Volte e confira cada passo.".into(),
        });
    }
    lista
}

/// O codigo da rejeicao no comeco do texto (`741-Rejeicao: ...`).
fn codigo_do(texto: &str) -> Option<&str> {
    let (cod, resto) = texto.split_once('-')?;
    (cod.len() >= 3
        && cod.len() <= 4
        && cod.bytes().all(|b| b.is_ascii_digit())
        && resto.starts_with("Rejei"))
    .then_some(cod)
}

/// O texto sem o codigo e sem o "Rejeicao:".
fn sem_codigo(texto: &str) -> String {
    let t = match codigo_do(texto) {
        Some(c) => &texto[c.len() + 1..],
        None => texto,
    };
    let t = t
        .strip_prefix("Rejeicao:")
        .or_else(|| t.strip_prefix("Rejeição:"))
        .unwrap_or(t);
    frase(t)
}

/// O passo onde o assunto se resolve, pelas palavras do texto.
fn passo_do(texto: &str) -> PassoNfe {
    let t = texto.to_lowercase();
    let tem = |ps: &[&str]| ps.iter().any(|p| t.contains(p));
    if tem(&["emit", "[nfe]", "configura"]) {
        PassoNfe::Conferir
    } else if tem(&[
        "dest",
        "indiedest",
        "indfinal",
        "inscricao estadual",
        "cpf",
        "cnpj do dest",
    ]) {
        PassoNfe::ParaQuem
    } else if tem(&["pag", "dup", "cobr", "fatura", "troco"]) {
        PassoNfe::ComoRecebeu
    } else if tem(&["transp", "frete", "veic", "vol"]) {
        PassoNfe::ComoEntrega
    } else {
        PassoNfe::OQueVendeu
    }
}

fn traduzir(texto: &str) -> PendenciaNfe {
    let t = texto.to_lowercase();
    let passo = passo_do(texto);
    let mensagem = sem_codigo(texto);
    let como: String = if t.contains("pendente") {
        "Retome as notas pendentes (Notas, aba NF-e, Retomar) para saber o que a SEFAZ fez antes de emitir outra.".into()
    } else if t.contains("ja foi gasto") || t.contains("foi inutilizado") {
        "O número será o próximo livre: tente de novo.".into()
    } else if t.contains("ncm") {
        "Confira o NCM do produto (8 dígitos) na tabela da Receita.".into()
    } else if t.contains("cfop") {
        "Abra os detalhes fiscais do produto e confira o CFOP e o ICMS.".into()
    } else if t.contains("ie") && t.contains("dest") {
        "Confira a inscrição estadual do cliente e se ele é contribuinte.".into()
    } else if let Some(c) = codigo_do(texto) {
        format!(
            "Corrija o ponto indicado no passo \"{}\" e confira de novo (regra {c} da SEFAZ).",
            passo.titulo()
        )
    } else {
        format!(
            "Corrija o ponto indicado no passo \"{}\" e confira de novo.",
            passo.titulo()
        )
    };
    PendenciaNfe {
        passo,
        item: None,
        mensagem,
        como_resolver: como,
    }
}

/// Como terminou o envio (ou o EPEC) de uma NF-e.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DesfechoNfeEmissao {
    /// A SEFAZ autorizou.
    Autorizada {
        /// A chave de acesso.
        chave: String,
        /// O numero.
        numero: String,
        /// A serie.
        serie: String,
        /// O protocolo de autorizacao.
        protocolo: String,
        /// A nota ficou guardada na pasta como autorizada.
        guardada: bool,
        /// A resposta se perdeu e a consulta achou a nota (sem nota nova).
        recuperada: bool,
        /// Os avisos da montagem.
        avisos: Vec<String>,
    },
    /// A SEFAZ recusou. O numero voltou a ficar livre.
    Rejeitada {
        /// O codigo da SEFAZ.
        cstat: String,
        /// O motivo, como a SEFAZ escreveu.
        motivo: String,
        /// O que fazer.
        o_que_fazer: String,
        /// O passo onde se corrige, pelo motivo.
        passo: PassoNfe,
        /// O numero que continua livre.
        numero: String,
    },
    /// A SEFAZ negou o uso da nota (denegada): o numero esta gasto e a nota nao vale.
    Denegada {
        /// O motivo.
        motivo: String,
    },
    /// O numero ja estava autorizado com outra chave.
    NumeroJaUsado {
        /// A frase.
        texto: String,
    },
    /// Sem resposta: a nota PODE ter sido autorizada. Fica pendente na pasta e o caminho e' o Retomar.
    SemResposta {
        /// A chave da nota pendente.
        chave: String,
        /// O numero.
        numero: String,
        /// A serie.
        serie: String,
    },
    /// A SEFAZ disse que o servico esta parado (108 ou 109): nada foi processado. A nota fica pendente na pasta.
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
    /// Recusada antes de enviar (regra da biblioteca, pasta, certificado): nada saiu.
    Recusada {
        /// As pendencias.
        pendencias: Vec<PendenciaNfe>,
    },
    /// O EPEC foi registrado no Ambiente Nacional: a nota fica em contingencia e tem de ser transmitida depois.
    EpecRegistrado {
        /// A chave.
        chave: String,
        /// O numero.
        numero: String,
        /// A serie.
        serie: String,
        /// O protocolo do EPEC.
        protocolo: String,
    },
    /// O EPEC foi recusado: a nota nao saiu e o numero ficou livre.
    EpecRecusado {
        /// O que aconteceu e o que fazer.
        texto: String,
    },
}

impl DesfechoNfeEmissao {
    /// O desfecho do que `emitir` devolveu.
    #[must_use]
    pub fn de(r: Result<Emitida, Erro>) -> Self {
        let e = match r {
            Ok(e) => e,
            Err(erro) => return Self::do_erro(&erro),
        };
        let (numero, serie) = (e.numero.to_string(), e.serie.clone());
        match &e.resultado.desfecho {
            Desfecho::Autorizada { protocolo, .. } => Self::Autorizada {
                chave: e.chave.clone(),
                numero,
                serie,
                protocolo: protocolo.clone(),
                guardada: e.situacao == Some(Situacao::Autorizada),
                recuperada: e.resultado.recuperada,
                avisos: e.avisos.iter().map(|a| aviso(a).texto).collect(),
            },
            Desfecho::Rejeitada { cstat, motivo } => {
                let passo = passo_do(motivo);
                Self::Rejeitada {
                    cstat: cstat.clone(),
                    motivo: sem_codigo(motivo),
                    o_que_fazer: format!(
                        "A SEFAZ não aceitou a nota. Corrija o que ela apontou (passo \"{}\") e envie de novo: o número {numero} continua livre.",
                        passo.titulo()
                    ),
                    passo,
                    numero,
                }
            }
            Desfecho::Denegada { motivo, .. } => Self::Denegada {
                motivo: frase(motivo),
            },
            Desfecho::NumeroJaUsado { motivo, .. } => Self::NumeroJaUsado {
                texto: format!(
                    "{} Esta nota não vale; a que vale é a outra. Confira na lista de notas.",
                    frase(motivo)
                ),
            },
            Desfecho::Indisponivel { motivo, .. } => Self::SefazFora {
                chave: e.chave.clone(),
                numero,
                serie,
                motivo: frase(motivo),
            },
            _ => Self::SemResposta {
                chave: e.chave.clone(),
                numero,
                serie,
            },
        }
    }

    /// O desfecho do que `epec` devolveu.
    #[must_use]
    pub fn do_epec(r: Result<Registrado, Erro>) -> Self {
        let reg = match r {
            Ok(r) => r,
            Err(erro) => return Self::do_erro(&erro),
        };
        let chave = reg.chave.clone();
        let (numero, serie) = numero_e_serie(&chave);
        match &reg.resultado.desfecho {
            DesfechoEvento::Registrado { protocolo, .. } => Self::EpecRegistrado {
                chave,
                numero,
                serie,
                protocolo: protocolo.clone(),
            },
            DesfechoEvento::EpecBloqueado { motivo, pendentes } => Self::EpecRecusado {
                texto: format!(
                    "{} Há {} nota(s) em contingência ainda não transmitidas: transmita-as (Notas, aba NF-e, Retomar) antes de emitir outra em contingência.",
                    frase(motivo),
                    pendentes.len()
                ),
            },
            DesfechoEvento::Rejeitado { motivo, .. } | DesfechoEvento::PrazoVencido { motivo, .. } => {
                Self::EpecRecusado {
                    texto: format!(
                        "{} A nota não saiu e o número {numero} continua livre.",
                        frase(motivo)
                    ),
                }
            }
            DesfechoEvento::Indisponivel { motivo, .. } => Self::EpecRecusado {
                texto: format!(
                    "{} A nota em contingência não foi registrada: tente de novo daqui a pouco.",
                    frase(motivo)
                ),
            },
            _ => Self::SemResposta {
                chave,
                numero,
                serie,
            },
        }
    }

    fn do_erro(e: &Erro) -> Self {
        Self::Recusada {
            pendencias: erros_da_recusa(e),
        }
    }

    /// O desfecho deixa uma nota PENDENTE na pasta: o formulario nao volta a editar e o caminho e' o Retomar.
    #[must_use]
    pub fn deixa_pendente(&self) -> bool {
        matches!(self, Self::SemResposta { .. } | Self::SefazFora { .. })
    }

    /// A nota esta feita (autorizada ou em EPEC): o assistente termina.
    #[must_use]
    pub fn concluiu(&self) -> bool {
        matches!(self, Self::Autorizada { .. } | Self::EpecRegistrado { .. })
    }
}

/// O numero e a serie que a chave de 44 posicoes traz (serie nas posicoes 22 a 24, numero de 25 a 33).
fn numero_e_serie(chave: &str) -> (String, String) {
    let pega = |de: usize, ate: usize| -> String {
        chave
            .get(de..ate)
            .map(|s| s.trim_start_matches('0').to_string())
            .map(|s| if s.is_empty() { "0".into() } else { s })
            .unwrap_or_default()
    };
    (pega(25, 34), pega(22, 25))
}

/// O que a tela diz quando o envio ficou sem resposta (a nota esta pendente na pasta).
pub const TEXTO_SEM_RESPOSTA: &str = "A SEFAZ não respondeu, e não foi possível confirmar se a nota foi autorizada: ela PODE ter sido. A nota ficou guardada como pendente. Use Retomar: o programa consulta a SEFAZ pela MESMA chave e, só se ela não tiver recebido, reenvia a MESMA nota. Não emita outra no lugar.";

/// O que a tela diz quando a SEFAZ respondeu que o servico esta parado.
pub const TEXTO_SEFAZ_FORA: &str = "A SEFAZ informou que o serviço está parado e não processou nada. A nota ficou guardada como pendente: quando o serviço voltar, use Retomar para enviar a MESMA nota. Não emita outra no lugar.";

/// O que a contingencia EPEC e', em uma frase.
pub const O_QUE_E_EPEC: &str = "O EPEC (Evento Prévio de Emissão em Contingência) é o caminho para emitir quando a SEFAZ da sua UF está fora do ar: a nota é registrada no Ambiente Nacional e a venda segue; depois a mesma nota tem de ser transmitida a SEFAZ da sua UF.";

/// O que fazer depois do EPEC registrado.
#[must_use]
pub fn depois_do_epec() -> String {
    format!(
        "A nota está em contingência (EPEC). Quando a SEFAZ da sua UF voltar, transmita a MESMA nota em até {HORAS_PARA_TRANSMITIR_O_EPEC} horas (7 dias): abra Notas, aba NF-e, e use Retomar. O DANFE já pode acompanhar a mercadoria."
    )
}

/// Por que a contingencia nao e' oferecida enquanto ha nota pendente: seria outra nota para a mesma venda.
#[must_use]
pub fn por_que_nao_ha_epec(pendentes: usize) -> String {
    let notas =
        super::nfe_resultados::contagem(pendentes, "nota sem resposta", "notas sem resposta");
    format!(
        "Há {notas} na pasta. Emitir em contingência agora criaria OUTRA nota (outro número e outra chave) enquanto a SEFAZ pode já ter recebido a primeira. Use Retomar primeiro: quando as pendentes se resolverem, a contingência volta a ser oferecida."
    )
}

#[cfg(test)]
mod testes;
