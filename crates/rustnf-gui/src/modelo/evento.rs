// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Os eventos que a pessoa pede a Sefin pela tela: cancelar a nota, pedir a analise fiscal do cancelamento e, para a
//! nota recebida, confirmar ou recusar. Cada um tem o texto para quem nao e' contador (o codigo do evento fica em
//! segundo plano), os motivos que a biblioteca aceita e a regra de quando a tela oferece a acao. Sem janela.

use rustnf::nfse::eventos::{self, MOTIVOS_CANCELAMENTO, MOTIVOS_REJEICAO};
use rustnf_app::notas::Situacao;

use super::ambiente::Ambiente;

/// O que a pessoa pede a Sefin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TipoDeEvento {
    /// Cancelar a nota (101101).
    #[default]
    Cancelar,
    /// Pedir ao fisco que analise o cancelamento (101103).
    AnaliseFiscal,
    /// O tomador confirma que o servico foi prestado (203202).
    Confirmar,
    /// O tomador recusa a nota (203206).
    Recusar,
}

impl TipoDeEvento {
    /// O `tpEvento` da biblioteca.
    #[must_use]
    pub fn codigo(self) -> &'static str {
        match self {
            Self::Cancelar => eventos::CANCELAMENTO,
            Self::AnaliseFiscal => eventos::ANALISE_FISCAL,
            Self::Confirmar => "203202",
            Self::Recusar => "203206",
        }
    }

    /// Os motivos que a biblioteca aceita para este evento (`nfse::eventos`); vazio quando ele nao tem motivo.
    #[must_use]
    pub fn motivos(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Self::Cancelar | Self::AnaliseFiscal => MOTIVOS_CANCELAMENTO,
            Self::Recusar => MOTIVOS_REJEICAO,
            Self::Confirmar => &[],
        }
    }

    /// Os motivos como a tela os escreve (com acento), na mesma ordem e com os mesmos codigos de [`Self::motivos`].
    #[must_use]
    pub fn rotulos_dos_motivos(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Self::Cancelar | Self::AnaliseFiscal => &[
                ("1", "Erro na emissão"),
                ("2", "Serviço não prestado"),
                ("9", "Outros"),
            ],
            Self::Recusar => &[
                ("1", "Nota duplicada"),
                ("2", "Já emitida pelo tomador"),
                (
                    "3",
                    "O serviço não foi prestado (não ocorreu o fato gerador)",
                ),
                ("4", "Erro quanto à responsabilidade tributária"),
                ("5", "Erro quanto ao valor, ao serviço ou à data"),
                ("9", "Outros"),
            ],
            Self::Confirmar => &[],
        }
    }

    /// O motivo `codigo` exige a justificativa escrita: no cancelamento e na analise fiscal sempre; na recusa so' com
    /// o motivo 9 (Outros); a confirmacao nao tem.
    #[must_use]
    pub fn exige_justificativa(self, codigo: Option<&str>) -> bool {
        match self {
            Self::Cancelar | Self::AnaliseFiscal => true,
            Self::Recusar => codigo == Some("9"),
            Self::Confirmar => false,
        }
    }

    /// O evento tem justificativa para escrever (obrigatoria ou nao).
    #[must_use]
    pub fn tem_justificativa(self) -> bool {
        self != Self::Confirmar
    }

    /// O titulo do dialogo.
    #[must_use]
    pub fn titulo(self, numero: &str) -> String {
        match self {
            Self::Cancelar => format!("Cancelar a NFS-e nº {numero}"),
            Self::AnaliseFiscal => {
                format!("Pedir análise fiscal do cancelamento da NFS-e nº {numero}")
            }
            Self::Confirmar => format!("Confirmar o serviço da NFS-e nº {numero}"),
            Self::Recusar => format!("Recusar a NFS-e nº {numero}"),
        }
    }

    /// O botao que abre o dialogo, no detalhe da nota.
    #[must_use]
    pub fn rotulo_da_acao(self) -> &'static str {
        match self {
            Self::Cancelar => "Cancelar a nota",
            Self::AnaliseFiscal => "Pedir análise fiscal do cancelamento",
            Self::Confirmar => "Confirmar que o serviço foi prestado",
            Self::Recusar => "Recusar esta nota",
        }
    }

    /// O botao que envia o pedido.
    #[must_use]
    pub fn rotulo_de_enviar(self, ambiente: Ambiente) -> &'static str {
        let producao = ambiente == Ambiente::Producao;
        match (self, producao) {
            (Self::Cancelar, true) => "Cancelar a nota em produção",
            (Self::Cancelar, false) => "Cancelar a nota",
            (Self::AnaliseFiscal, true) => "Pedir a análise fiscal em produção",
            (Self::AnaliseFiscal, false) => "Pedir a análise fiscal",
            (Self::Confirmar, true) => "Confirmar o serviço em produção",
            (Self::Confirmar, false) => "Confirmar o serviço",
            (Self::Recusar, true) => "Recusar a nota em produção",
            (Self::Recusar, false) => "Recusar a nota",
        }
    }

    /// Para quem le o dialogo em producao: o que o pedido faz de verdade.
    #[must_use]
    pub fn efeito(self) -> &'static str {
        match self {
            Self::Cancelar => "Em produção, o cancelamento de uma nota tem efeito fiscal.",
            Self::AnaliseFiscal => "Isto não cancela a nota agora: o fisco vai analisar o pedido e pode aceitar ou recusar o cancelamento. Enquanto isso, a nota fica «em análise fiscal».",
            Self::Confirmar => "Você declara à Sefin que recebeu esta nota e que o serviço foi prestado como ela descreve. A confirmação fica registrada na nota; ela não muda o valor nem a situação da nota.",
            Self::Recusar => "Você declara à Sefin que recusa esta nota. A recusa fica registrada na nota, com o motivo. É uma manifestação de quem recebeu a nota: o cancelamento continua sendo do emitente.",
        }
    }

    /// O aviso que a marcacao de producao acompanha, quando o ambiente e' de testes.
    #[must_use]
    pub fn efeito_nos_testes(self) -> &'static str {
        match self {
            Self::Cancelar => "Ambiente de testes: o cancelamento não tem efeito fiscal.",
            Self::AnaliseFiscal => "Ambiente de testes: o pedido não tem efeito fiscal.",
            Self::Confirmar => "Ambiente de testes: a confirmação não tem efeito fiscal.",
            Self::Recusar => "Ambiente de testes: a recusa não tem efeito fiscal.",
        }
    }

    /// A marcacao que se faz em producao antes de o botao de enviar habilitar.
    #[must_use]
    pub fn marcacao_de_producao(self) -> &'static str {
        match self {
            Self::Cancelar => "Entendo que o cancelamento tem efeito fiscal",
            Self::AnaliseFiscal => {
                "Entendo que este pedido tem efeito fiscal e será analisado pelo fisco"
            }
            Self::Confirmar => "Entendo que esta confirmação fica registrada na nota",
            Self::Recusar => "Entendo que esta recusa fica registrada na nota",
        }
    }

    /// A frase de quando falta a marcacao de producao.
    #[must_use]
    pub fn falta_marcar(self) -> &'static str {
        match self {
            Self::Cancelar => "Marque que você entende que o cancelamento tem efeito fiscal.",
            Self::AnaliseFiscal => "Marque que você entende que este pedido tem efeito fiscal.",
            Self::Confirmar => "Marque que você entende que a confirmação fica registrada na nota.",
            Self::Recusar => "Marque que você entende que a recusa fica registrada na nota.",
        }
    }

    /// O texto de apoio do campo da justificativa.
    #[must_use]
    pub fn dica_da_justificativa(self) -> &'static str {
        match self {
            Self::Cancelar => "Por que a nota está sendo cancelada (fica no documento fiscal)",
            Self::AnaliseFiscal => "Por que a nota precisa ser cancelada (o fisco lê isto)",
            Self::Recusar => "Explique a recusa (obrigatório no motivo Outros)",
            Self::Confirmar => "",
        }
    }

    /// A frase de quando o pedido foi registrado.
    #[must_use]
    pub fn registrado(self) -> &'static str {
        match self {
            Self::Cancelar => "Cancelamento registrado na Sefin Nacional. A nota está cancelada.",
            Self::AnaliseFiscal => "Pedido de análise fiscal registrado na Sefin Nacional. A nota está em análise fiscal: o fisco vai decidir sobre o cancelamento.",
            Self::Confirmar => "Confirmação registrada na Sefin Nacional. A nota segue como está.",
            Self::Recusar => "Recusa registrada na Sefin Nacional. A nota segue como está: o cancelamento continua sendo do emitente.",
        }
    }

    /// "A Sefin recusou o cancelamento", no titulo da rejeicao.
    #[must_use]
    pub fn recusou(self) -> &'static str {
        match self {
            Self::Cancelar => "A Sefin recusou o cancelamento",
            Self::AnaliseFiscal => "A Sefin recusou o pedido de análise fiscal",
            Self::Confirmar => "A Sefin recusou a confirmação",
            Self::Recusar => "A Sefin recusou a recusa",
        }
    }

    /// O que acontece com a nota depois de uma rejeicao.
    #[must_use]
    pub fn depois_de_recusar(self) -> &'static str {
        match self {
            Self::Cancelar => "A nota continua valendo. Seus dados estão como você deixou: corrija o que a Sefin apontou, se for o caso, e tente de novo.",
            Self::AnaliseFiscal => "Nada mudou na nota. Seus dados estão como você deixou: corrija o que a Sefin apontou, se for o caso, e tente de novo.",
            Self::Confirmar | Self::Recusar => "Nada mudou na nota. Leia o que a Sefin apontou e, se for o caso, tente de novo.",
        }
    }

    /// O que se diz do envio em andamento.
    #[must_use]
    pub fn enviando(self) -> &'static str {
        match self {
            Self::Cancelar => "Enviando o pedido de cancelamento à Sefin Nacional...",
            Self::AnaliseFiscal => "Enviando o pedido de análise fiscal à Sefin Nacional...",
            Self::Confirmar => "Enviando a confirmação à Sefin Nacional...",
            Self::Recusar => "Enviando a recusa à Sefin Nacional...",
        }
    }

    /// O que se diz da conferencia em andamento.
    #[must_use]
    pub fn conferindo(self) -> &'static str {
        match self {
            Self::Cancelar => "Conferindo na Sefin se o cancelamento foi registrado...",
            Self::AnaliseFiscal => "Conferindo na Sefin se o pedido foi registrado...",
            Self::Confirmar => "Conferindo na Sefin se a confirmação foi registrada...",
            Self::Recusar => "Conferindo na Sefin se a recusa foi registrada...",
        }
    }

    /// Frase de quando nao houve resposta ao pedido.
    #[must_use]
    pub fn sem_resposta(self) -> String {
        let o_que = match self {
            Self::Cancelar => "o cancelamento",
            Self::AnaliseFiscal => "o pedido de análise fiscal",
            Self::Confirmar => "a confirmação",
            Self::Recusar => "a recusa",
        };
        format!(
            "A Sefin Nacional não confirmou {o_que}, e a consulta do evento também não o achou. Ele pode ter sido registrado mesmo assim."
        )
    }

    /// Explica o que "Tentar de novo" faz. So' o cancelamento tem pedido idempotente (mesmo identificador); os outros
    /// eventos nao reenviam: conferem primeiro.
    #[must_use]
    pub fn explicacao_de_tentar_de_novo(self) -> &'static str {
        match self {
            Self::Cancelar => super::cancelamento::EXPLICACAO_DE_TENTAR_DE_NOVO,
            _ => "Este pedido não é reenviado às cegas: a Sefin pode ter registrado o primeiro e perdido só a resposta, e um segundo pedido poderia valer como outro. Use Conferir na Sefin, que só consulta. Se a Sefin disser que não há registro, aí sim o pedido pode ser feito de novo.",
        }
    }
}

/// O que a tela oferece para uma nota, alem do cancelamento.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AcoesDeEvento {
    /// Pedir a analise fiscal do cancelamento.
    pub analise_fiscal: bool,
    /// Confirmar a nota recebida.
    pub confirmar: bool,
    /// Recusar a nota recebida.
    pub recusar: bool,
}

impl AcoesDeEvento {
    /// As acoes de uma nota: a analise fiscal e' do emitente e so' cabe em nota autorizada; confirmar e recusar sao de
    /// quem recebeu a nota (o tomador e' o documento do certificado, e a nota e' de outro emitente) e tambem so' em
    /// nota autorizada. Nota de outro emitente sem ser o tomador: nenhuma.
    #[must_use]
    pub fn da_nota(situacao: Situacao, e_do_emitente: bool, e_tomador: bool) -> Self {
        let autorizada = situacao == Situacao::Autorizada;
        let recebida = !e_do_emitente && e_tomador;
        Self {
            analise_fiscal: autorizada && e_do_emitente,
            confirmar: autorizada && recebida,
            recusar: autorizada && recebida,
        }
    }

    /// Nao ha acao de evento.
    #[must_use]
    pub fn nenhuma(self) -> bool {
        self == Self::default()
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn os_codigos_e_os_motivos_sao_os_da_biblioteca() {
        for t in [
            TipoDeEvento::Cancelar,
            TipoDeEvento::AnaliseFiscal,
            TipoDeEvento::Confirmar,
            TipoDeEvento::Recusar,
        ] {
            // o contribuinte pode pedir cada um (o cancelamento tem comando proprio, mas e' dele)
            assert!(eventos::TIPOS.iter().any(|x| x.0 == t.codigo()));
        }
        assert_eq!(TipoDeEvento::Confirmar.codigo(), "203202");
        assert_eq!(TipoDeEvento::Recusar.codigo(), "203206");
        assert_eq!(TipoDeEvento::AnaliseFiscal.motivos(), MOTIVOS_CANCELAMENTO);
        assert_eq!(TipoDeEvento::Recusar.motivos(), MOTIVOS_REJEICAO);
        assert!(TipoDeEvento::Confirmar.motivos().is_empty());
    }

    #[test]
    fn os_rotulos_da_tela_tem_os_codigos_da_biblioteca_na_mesma_ordem() {
        for t in [
            TipoDeEvento::Cancelar,
            TipoDeEvento::AnaliseFiscal,
            TipoDeEvento::Confirmar,
            TipoDeEvento::Recusar,
        ] {
            let tela: Vec<&str> = t.rotulos_dos_motivos().iter().map(|m| m.0).collect();
            let lib: Vec<&str> = t.motivos().iter().map(|m| m.0).collect();
            assert_eq!(tela, lib, "{t:?}");
        }
    }

    #[test]
    fn quem_pede_cada_evento_confere_com_a_biblioteca() {
        for (t, autor) in [
            (TipoDeEvento::Cancelar, "emitente"),
            (TipoDeEvento::AnaliseFiscal, "emitente"),
            (TipoDeEvento::Confirmar, "tomador"),
            (TipoDeEvento::Recusar, "tomador"),
        ] {
            let da_biblioteca = eventos::TIPOS.iter().find(|x| x.0 == t.codigo()).unwrap();
            assert_eq!(da_biblioteca.2, autor, "{t:?}");
        }
    }

    #[test]
    fn so_o_recusar_com_outros_exige_justificativa_entre_os_opcionais() {
        assert!(TipoDeEvento::Cancelar.exige_justificativa(Some("1")));
        assert!(TipoDeEvento::AnaliseFiscal.exige_justificativa(None));
        assert!(!TipoDeEvento::Recusar.exige_justificativa(Some("3")));
        assert!(TipoDeEvento::Recusar.exige_justificativa(Some("9")));
        assert!(!TipoDeEvento::Confirmar.exige_justificativa(Some("9")));
        assert!(!TipoDeEvento::Confirmar.tem_justificativa());
    }

    #[test]
    fn o_texto_que_a_pessoa_le_nao_traz_o_codigo_do_evento() {
        for t in [
            TipoDeEvento::Cancelar,
            TipoDeEvento::AnaliseFiscal,
            TipoDeEvento::Confirmar,
            TipoDeEvento::Recusar,
        ] {
            let textos = [
                t.titulo("12"),
                t.rotulo_da_acao().into(),
                t.efeito().into(),
                t.registrado().into(),
                t.marcacao_de_producao().into(),
                t.sem_resposta(),
            ];
            for texto in textos {
                assert!(
                    !texto.contains(t.codigo()),
                    "o codigo {} aparece em: {texto}",
                    t.codigo()
                );
            }
        }
    }

    #[test]
    fn analise_fiscal_so_do_emitente_e_em_nota_autorizada() {
        let a = AcoesDeEvento::da_nota(Situacao::Autorizada, true, false);
        assert!(a.analise_fiscal && !a.confirmar && !a.recusar);
        for s in [
            Situacao::Cancelada,
            Situacao::Substituida,
            Situacao::AnaliseFiscal,
        ] {
            assert!(
                AcoesDeEvento::da_nota(s, true, false).nenhuma(),
                "{s:?}: ja saiu de autorizada"
            );
        }
    }

    #[test]
    fn confirmar_e_recusar_so_na_nota_recebida() {
        let recebida = AcoesDeEvento::da_nota(Situacao::Autorizada, false, true);
        assert!(recebida.confirmar && recebida.recusar && !recebida.analise_fiscal);
        // de outro emitente, sem ser o tomador: nada
        assert!(AcoesDeEvento::da_nota(Situacao::Autorizada, false, false).nenhuma());
        // emitente e tomador ao mesmo tempo (auto-emissao): e' nota propria, nao recebida
        let propria = AcoesDeEvento::da_nota(Situacao::Autorizada, true, true);
        assert!(propria.analise_fiscal && !propria.confirmar && !propria.recusar);
        // recebida mas ja cancelada: nada
        assert!(AcoesDeEvento::da_nota(Situacao::Cancelada, false, true).nenhuma());
    }
}
