// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O cancelamento de uma nota e os outros eventos que a pessoa pede pelo mesmo dialogo (analise fiscal, confirmar e
//! recusar a nota recebida): o formulario (motivo, justificativa e a marcacao de producao), os limites da biblioteca
//! e o desfecho como tipo da tela. Nenhuma frase aqui leva caminho, senha, XML nem detalhe de rede.

use rustnf::nfse::eventos::{MOTIVO_MAX, MOTIVO_MIN};
use rustnf::nfse::sefin::{Resultado, Status};
use rustnf_app::pedido::Cancelada;

use super::ambiente::Ambiente;
use super::emissao::MensagemSefin;
use super::evento::TipoDeEvento;
use super::mascara::frase;

/// Os motivos que o cancelamento aceita, com o rotulo da tela. Os codigos sao os da biblioteca
/// (`nfse::eventos::MOTIVOS_CANCELAMENTO`); um teste confere que continuam os mesmos.
pub const MOTIVOS: [(&str, &str); 3] = [
    ("1", "Erro na emissão"),
    ("2", "Serviço não prestado"),
    ("9", "Outros"),
];

/// A justificativa como a biblioteca a conta: espacos repetidos e das pontas nao valem.
#[must_use]
pub fn justificativa_normal(texto: &str) -> String {
    texto.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// O que a pessoa preencheu para cancelar (ou para o outro evento que o dialogo pede: `tipo`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FormularioDeCancelamento {
    /// O evento que se pede; o cancelamento e' o padrao.
    pub tipo: TipoDeEvento,
    /// O codigo do motivo escolhido.
    pub motivo: Option<&'static str>,
    /// A justificativa, como digitada.
    pub justificativa: String,
    /// Em producao: "Entendo que o cancelamento tem efeito fiscal".
    pub efeito_fiscal_entendido: bool,
}

impl FormularioDeCancelamento {
    /// O formulario vazio de um evento.
    #[must_use]
    pub fn de(tipo: TipoDeEvento) -> Self {
        Self {
            tipo,
            ..Self::default()
        }
    }

    /// Escolhe o motivo pelo codigo (os do evento: `1`, `2` ou `9` no cancelamento); codigo que nao existe e'
    /// ignorado.
    pub fn escolher_motivo(&mut self, codigo: &str) {
        self.motivo = self
            .tipo
            .motivos()
            .iter()
            .map(|(c, _)| *c)
            .find(|c| *c == codigo);
    }

    /// Quantos caracteres a justificativa tem, contados como a biblioteca conta.
    #[must_use]
    pub fn contagem(&self) -> usize {
        justificativa_normal(&self.justificativa).chars().count()
    }

    /// O contador da tela: `12 de 15 a 255 caracteres`.
    #[must_use]
    pub fn contador(&self) -> String {
        format!(
            "{} caracteres (de {MOTIVO_MIN} a {MOTIVO_MAX})",
            self.contagem()
        )
    }

    /// O erro da justificativa, embaixo do campo. Vazia so' acusa erro depois de tocada (`tocada`).
    #[must_use]
    pub fn erro_da_justificativa(&self, tocada: bool) -> Option<String> {
        let n = self.contagem();
        if !self.tipo.tem_justificativa() {
            return None;
        }
        if n == 0 {
            if !self.tipo.exige_justificativa(self.motivo) {
                return None;
            }
            return tocada.then(|| {
                "Escreva a justificativa: ela fica registrada no documento fiscal.".into()
            });
        }
        if n < MOTIVO_MIN {
            return Some(format!(
                "A justificativa precisa de pelo menos {MOTIVO_MIN} caracteres (faltam {}).",
                MOTIVO_MIN - n
            ));
        }
        if n > MOTIVO_MAX {
            return Some(format!(
                "A justificativa passa de {MOTIVO_MAX} caracteres (sobram {}).",
                n - MOTIVO_MAX
            ));
        }
        None
    }

    /// A justificativa que vai no pedido.
    #[must_use]
    pub fn justificativa_para_enviar(&self) -> String {
        justificativa_normal(&self.justificativa)
    }

    /// Por que o botao Cancelar esta desligado, ou `None` se pode enviar. Em producao falta marcar o efeito fiscal.
    #[must_use]
    pub fn motivo_de_nao_enviar(&self, ambiente: Ambiente) -> Option<String> {
        if self.motivo.is_none() && !self.tipo.motivos().is_empty() {
            return Some(
                match self.tipo {
                    TipoDeEvento::Cancelar => "Escolha o motivo do cancelamento.",
                    TipoDeEvento::AnaliseFiscal => "Escolha o motivo do cancelamento pedido.",
                    _ => "Escolha o motivo da recusa.",
                }
                .into(),
            );
        }
        if let Some(e) = self.erro_da_justificativa(true) {
            return Some(e);
        }
        if ambiente == Ambiente::Producao && !self.efeito_fiscal_entendido {
            return Some(self.tipo.falta_marcar().into());
        }
        None
    }

    /// Pode enviar.
    #[must_use]
    pub fn pode_enviar(&self, ambiente: Ambiente) -> bool {
        self.motivo_de_nao_enviar(ambiente).is_none()
    }
}

/// Como o cancelamento terminou.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DesfechoDoCancelamento {
    /// A Sefin registrou o cancelamento (ou o achou registrado, depois de a resposta se perder).
    Registrado {
        /// O evento ja estava registrado e foi achado pela consulta.
        recuperada: bool,
        /// A pasta de notas marcou a nota como cancelada. `Some(false)`: nao marcou (a nota nao esta na pasta ou a
        /// pasta nao pode ser atualizada); `None`: nao havia pasta em uso.
        marcada: Option<bool>,
    },
    /// A Sefin recusou o pedido.
    Rejeitado {
        /// As mensagens da Sefin.
        mensagens: Vec<MensagemSefin>,
    },
    /// Sem resposta: o cancelamento PODE ter sido registrado.
    SemResposta {
        /// A frase para a tela.
        texto: String,
    },
    /// A conferencia na Sefin respondeu que nao ha cancelamento registrado: a nota continua valida.
    NaoRegistrado,
    /// Recusado antes de enviar (regra da biblioteca, certificado, senha): nada saiu.
    Recusado {
        /// A frase para a tela.
        texto: String,
    },
}

/// O que a tela diz quando nao houve resposta ao pedido de cancelamento. A biblioteca, depois de um envio sem
/// resposta, ainda consulta o evento na Sefin; so' devolve "sem confirmacao" se nem assim o achou.
pub const TEXTO_SEM_RESPOSTA: &str = "A Sefin Nacional não confirmou o cancelamento, e a consulta do evento também não o achou. Ele pode ter sido registrado mesmo assim.";

/// Explica o que "Tentar de novo" faz no cancelamento.
pub const EXPLICACAO_DE_TENTAR_DE_NOVO: &str = "Tentar de novo reenvia o mesmo pedido: a mesma nota, o mesmo motivo e a mesma justificativa. O pedido de cancelamento tem sempre o mesmo identificador (a chave da nota mais o tipo do evento), então ele não cria um segundo cancelamento. Se o primeiro envio já tiver sido registrado, a Sefin recusa o reenvio por repetição; nesse caso a tela confere a nota e mostra que ela já está cancelada. Conferir na Sefin só consulta, sem reenviar.";

/// O que a tela diz quando a conferencia achou o cancelamento ja registrado.
pub const TEXTO_DA_RECUPERADA: &str = "O cancelamento já estava registrado na Sefin (de um envio anterior cuja resposta se perdeu, ou feito em outro lugar) e foi achado pela consulta do evento.";

/// O aviso quando o cancelamento foi registrado mas a pasta nao foi atualizada.
pub const AVISO_DA_PASTA: &str = "O cancelamento foi registrado na Sefin, mas a pasta de notas não pôde marcar esta nota como cancelada. Use Atualizar ou consulte a nota pela chave.";

impl DesfechoDoCancelamento {
    /// O desfecho do que `cancelar_pedido_completo` devolveu.
    #[must_use]
    pub fn de(c: Cancelada) -> Self {
        Self::do_resultado(&c.resultado, c.marcada, TEXTO_SEM_RESPOSTA)
    }

    /// O desfecho de um resultado do cliente da Sefin para um pedido de evento; `sem_resposta` e' a frase de quando
    /// nem a consulta do evento o achou.
    #[must_use]
    pub fn do_resultado(r: &Resultado, marcada: Option<bool>, sem_resposta: &str) -> Self {
        match r.status {
            Status::Registrado | Status::Ok | Status::Autorizada => Self::Registrado {
                recuperada: r.recuperada,
                marcada,
            },
            Status::Rejeitada => {
                let mut mensagens: Vec<MensagemSefin> =
                    r.erros.iter().map(MensagemSefin::from).collect();
                if mensagens.is_empty() {
                    mensagens.push(MensagemSefin {
                        codigo: String::new(),
                        descricao: frase(&r.frase),
                        complemento: String::new(),
                        dica: String::new(),
                        campo: None,
                    });
                }
                Self::Rejeitado { mensagens }
            }
            // recusa local da biblioteca: nada saiu para a rede
            Status::Erro if r.http == 0 && r.detalhe.is_empty() => Self::Recusado {
                texto: frase(&r.frase),
            },
            Status::Pendente | Status::Indisponivel | Status::Erro | Status::NaoEncontrada => {
                Self::SemResposta {
                    texto: sem_resposta.into(),
                }
            }
        }
    }

    /// O desfecho da conferencia "esta cancelada na Sefin?": `status` e' o do resultado de `Emissor::cancelada`.
    #[must_use]
    pub fn da_conferencia(status: Status, marcada: Option<bool>) -> Self {
        match status {
            Status::Ok | Status::Registrado => Self::Registrado {
                recuperada: true,
                marcada,
            },
            Status::NaoEncontrada => Self::NaoRegistrado,
            _ => Self::SemResposta {
                texto: "A Sefin Nacional não respondeu à conferência. Tente de novo daqui a pouco."
                    .into(),
            },
        }
    }

    /// O desfecho deixa o cancelamento em aberto: so' se sai dele tentando de novo, conferindo ou fechando.
    #[must_use]
    pub fn deixa_em_aberto(&self) -> bool {
        matches!(self, Self::SemResposta { .. })
    }

    /// O cancelamento esta registrado.
    #[must_use]
    pub fn registrado(&self) -> bool {
        matches!(self, Self::Registrado { .. })
    }
}

#[cfg(test)]
mod testes {
    use rustnf::nfse::eventos::MOTIVOS_CANCELAMENTO;
    use rustnf::nfse::leitura::Mensagem;
    use rustnf::nfse::sefin::Resultado;

    use super::*;

    fn resultado(status: Status) -> Resultado {
        Resultado {
            status,
            frase: "frase da biblioteca".into(),
            chave: String::new(),
            id_dps: String::new(),
            nfse: None,
            xml_assinado: String::new(),
            xml_retorno: String::new(),
            erros: vec![],
            alertas: vec![],
            http: 200,
            recuperada: false,
            corpo: None,
            detalhe: String::new(),
        }
    }

    fn cancelada(r: Resultado, marcada: Option<bool>) -> Cancelada {
        Cancelada {
            resultado: r,
            marcada,
        }
    }

    #[test]
    fn os_motivos_sao_os_da_biblioteca() {
        let da_biblioteca: Vec<&str> = MOTIVOS_CANCELAMENTO.iter().map(|(c, _)| *c).collect();
        let da_tela: Vec<&str> = MOTIVOS.iter().map(|(c, _)| *c).collect();
        assert_eq!(da_tela, da_biblioteca);
    }

    #[test]
    fn a_justificativa_segue_os_limites_da_biblioteca() {
        let mut f = FormularioDeCancelamento::default();
        assert_eq!(f.contagem(), 0);
        assert_eq!(
            f.erro_da_justificativa(false),
            None,
            "vazia so' acusa depois de tocada"
        );
        assert!(f
            .erro_da_justificativa(true)
            .unwrap()
            .contains("Escreva a justificativa"));
        f.justificativa = "curta".into();
        let e = f.erro_da_justificativa(false).unwrap();
        assert!(
            e.contains("15 caracteres") && e.contains("faltam 10"),
            "{e}"
        );
        f.justificativa = "x".repeat(MOTIVO_MIN);
        assert_eq!(f.erro_da_justificativa(true), None);
        assert_eq!(f.contador(), "15 caracteres (de 15 a 255)");
        f.justificativa = "x".repeat(MOTIVO_MAX);
        assert_eq!(f.erro_da_justificativa(true), None);
        f.justificativa = "x".repeat(MOTIVO_MAX + 3);
        let e = f.erro_da_justificativa(true).unwrap();
        assert!(e.contains("255") && e.contains("sobram 3"), "{e}");
    }

    #[test]
    fn a_contagem_ignora_espaco_repetido_e_das_pontas_como_a_biblioteca() {
        let mut f = FormularioDeCancelamento {
            justificativa: "  erro   na\n\nemissao  da   nota  ".into(),
            ..FormularioDeCancelamento::default()
        };
        assert_eq!(f.justificativa_para_enviar(), "erro na emissao da nota");
        assert_eq!(f.contagem(), 23);
        // 14 letras com muito espaco continuam sendo 14
        f.justificativa = "a  b  c  d  e  f  g".into();
        assert_eq!(f.contagem(), 13);
        assert!(f.erro_da_justificativa(true).is_some());
        // o que acentua conta por caractere, nao por byte
        f.justificativa = "servico nao prestado".replace('c', "ç");
        assert_eq!(f.contagem(), "servico nao prestado".chars().count());
    }

    #[test]
    fn o_botao_so_liga_com_motivo_justificativa_e_em_producao_a_marcacao() {
        let mut f = FormularioDeCancelamento::default();
        assert_eq!(
            f.motivo_de_nao_enviar(Ambiente::Restrita).unwrap(),
            "Escolha o motivo do cancelamento."
        );
        f.escolher_motivo("1");
        assert_eq!(f.motivo, Some("1"));
        assert!(f
            .motivo_de_nao_enviar(Ambiente::Restrita)
            .unwrap()
            .contains("justificativa"));
        f.justificativa = "Nota emitida com o valor errado".into();
        assert!(f.pode_enviar(Ambiente::Restrita));
        // em producao falta a marcacao explicita
        assert!(!f.pode_enviar(Ambiente::Producao));
        assert!(f
            .motivo_de_nao_enviar(Ambiente::Producao)
            .unwrap()
            .contains("efeito fiscal"));
        f.efeito_fiscal_entendido = true;
        assert!(f.pode_enviar(Ambiente::Producao));
        // motivo que a biblioteca nao conhece e' ignorado
        f.escolher_motivo("7");
        assert_eq!(f.motivo, None);
        assert!(!f.pode_enviar(Ambiente::Restrita));
    }

    #[test]
    fn registrado_leva_a_marca_da_pasta() {
        let d = DesfechoDoCancelamento::de(cancelada(resultado(Status::Registrado), Some(true)));
        assert_eq!(
            d,
            DesfechoDoCancelamento::Registrado {
                recuperada: false,
                marcada: Some(true)
            }
        );
        assert!(d.registrado() && !d.deixa_em_aberto());
        let mut r = resultado(Status::Registrado);
        r.recuperada = true;
        let d = DesfechoDoCancelamento::de(cancelada(r, Some(false)));
        assert_eq!(
            d,
            DesfechoDoCancelamento::Registrado {
                recuperada: true,
                marcada: Some(false)
            }
        );
    }

    #[test]
    fn rejeitado_traz_as_mensagens_com_codigo_e_complemento() {
        let mut r = resultado(Status::Rejeitada);
        r.erros = vec![Mensagem {
            codigo: "E0001".into(),
            descricao: " nota ja cancelada ".into(),
            complemento: "evento".into(),
        }];
        let DesfechoDoCancelamento::Rejeitado { mensagens } =
            DesfechoDoCancelamento::de(cancelada(r, None))
        else {
            panic!("esperava rejeitado");
        };
        assert_eq!(mensagens.len(), 1);
        assert_eq!(mensagens[0].codigo, "E0001");
        assert_eq!(mensagens[0].descricao, "nota ja cancelada");
        assert_eq!(mensagens[0].complemento, "evento");
        // sem mensagens estruturadas, a frase da biblioteca (com maiuscula)
        let DesfechoDoCancelamento::Rejeitado { mensagens } =
            DesfechoDoCancelamento::de(cancelada(resultado(Status::Rejeitada), None))
        else {
            panic!("esperava rejeitado");
        };
        assert_eq!(mensagens[0].descricao, "Frase da biblioteca");
    }

    #[test]
    fn sem_resposta_deixa_em_aberto_e_recusa_local_nao() {
        for status in [
            Status::Pendente,
            Status::Indisponivel,
            Status::NaoEncontrada,
        ] {
            let d = DesfechoDoCancelamento::de(cancelada(resultado(status), None));
            assert!(d.deixa_em_aberto(), "{status:?}");
            assert!(
                matches!(d, DesfechoDoCancelamento::SemResposta { texto } if texto == TEXTO_SEM_RESPOSTA)
            );
        }
        let mut r = resultado(Status::Erro);
        r.http = 500;
        assert!(DesfechoDoCancelamento::de(cancelada(r, None)).deixa_em_aberto());
        // recusa da biblioteca antes de enviar: http 0 e sem detalhe de rede
        let mut r = resultado(Status::Erro);
        r.http = 0;
        r.frase = "infPedReg/xMotivo: precisa de pelo menos 15 caracteres".into();
        let d = DesfechoDoCancelamento::de(cancelada(r, None));
        assert!(!d.deixa_em_aberto());
        assert!(
            matches!(&d, DesfechoDoCancelamento::Recusado { texto } if texto.starts_with("InfPedReg"))
        );
    }

    #[test]
    fn a_conferencia_na_sefin_tem_tres_respostas() {
        assert_eq!(
            DesfechoDoCancelamento::da_conferencia(Status::Ok, Some(true)),
            DesfechoDoCancelamento::Registrado {
                recuperada: true,
                marcada: Some(true)
            }
        );
        assert_eq!(
            DesfechoDoCancelamento::da_conferencia(Status::NaoEncontrada, None),
            DesfechoDoCancelamento::NaoRegistrado
        );
        assert!(DesfechoDoCancelamento::da_conferencia(Status::Pendente, None).deixa_em_aberto());
        assert!(DesfechoDoCancelamento::da_conferencia(Status::Erro, None).deixa_em_aberto());
    }

    #[test]
    fn os_textos_dizem_o_que_o_codigo_faz() {
        assert!(EXPLICACAO_DE_TENTAR_DE_NOVO.contains("mesmo identificador"));
        assert!(EXPLICACAO_DE_TENTAR_DE_NOVO.contains("recusa o reenvio por repetição"));
        assert!(EXPLICACAO_DE_TENTAR_DE_NOVO.contains("já está cancelada"));
        assert!(TEXTO_DA_RECUPERADA.contains("já estava registrado"));
        assert!(TEXTO_SEM_RESPOSTA.contains("pode ter sido registrado"));
        assert!(AVISO_DA_PASTA.contains("marcar esta nota como cancelada"));
    }
}
