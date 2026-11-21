// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O desfecho de uma consulta pela chave de acesso, como tipo da tela. A nota devolvida so' vale como resultado se a
//! chave dela for a que foi pedida.

use rustnf::nfse::leitura::Nfse;
use rustnf::nfse::sefin::Status;
use rustnf_app::emissao::Consultada;
use rustnf_app::notas::{Registro, Situacao};

use super::chave;
use super::detalhe::DetalheDaNota;
use super::mascara::frase;

/// A nota que a Sefin devolveu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotaConsultada {
    /// Os dados da nota, para a tela.
    pub detalhe: DetalheDaNota,
    /// O XML da nota (para o DANFSe e para guardar na pasta). Nunca vai para a tela.
    pub xml: String,
    /// A conferencia do cancelamento deu certo: a situacao do detalhe e' a da Sefin. `false`: nao deu para saber, e a
    /// tela diz isso.
    pub situacao_conferida: bool,
    /// O registro que iria para o indice da pasta de notas.
    pub registro: Registro,
}

/// Como a consulta terminou.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DesfechoDaConsulta {
    /// A Sefin devolveu a nota que foi pedida.
    Encontrada(Box<NotaConsultada>),
    /// A Sefin devolveu uma nota de outra chave: nao e' a que se pediu, e a tela nao a mostra como resultado.
    Divergente {
        /// A chave pedida, em grupos.
        pedida: String,
        /// A chave da nota devolvida, em grupos.
        devolvida: String,
    },
    /// A Sefin nao tem nota com essa chave.
    NaoEncontrada,
    /// Sem resposta: rede ou Sefin indisponivel.
    SemResposta {
        /// A frase para a tela.
        texto: String,
    },
    /// Nao deu para consultar (chave recusada pela biblioteca, certificado, senha, resposta sem a nota): nada saiu ou
    /// nada serve.
    Recusada {
        /// A frase para a tela.
        texto: String,
    },
}

/// O que a tela diz quando nao houve resposta a consulta.
pub const TEXTO_SEM_RESPOSTA: &str = "A Sefin Nacional não respondeu (ou está indisponível). Consultar não altera nada: pode tentar de novo.";

/// O que a tela diz quando a Sefin nao tem a nota.
pub const TEXTO_NAO_ENCONTRADA: &str = "A Sefin Nacional não tem nenhuma nota com esta chave neste ambiente. Confira a chave e se o ambiente das Configurações (produção ou produção restrita) é o da nota.";

fn nota_de(nfse: &Nfse, cancelada: Option<bool>) -> NotaConsultada {
    let situacao = if cancelada == Some(true) {
        Situacao::Cancelada
    } else {
        Situacao::Autorizada
    };
    let mut registro = Registro::da_nfse(nfse);
    registro.situacao = situacao;
    NotaConsultada {
        detalhe: DetalheDaNota::da_nfse(nfse, situacao),
        xml: nfse.xml.clone(),
        situacao_conferida: cancelada.is_some(),
        registro,
    }
}

impl DesfechoDaConsulta {
    /// O desfecho do que `consultar_completa` devolveu para a chave `pedida`.
    #[must_use]
    pub fn de(consultada: Consultada, pedida: &str) -> Self {
        let r = &consultada.resultado;
        match r.status {
            Status::Autorizada | Status::Ok | Status::Registrado => match &r.nfse {
                Some(nfse) if chave::sao_a_mesma(&nfse.chave, pedida) => {
                    Self::Encontrada(Box::new(nota_de(nfse, consultada.cancelada)))
                }
                Some(nfse) => Self::Divergente {
                    pedida: chave::em_grupos(pedida),
                    devolvida: chave::em_grupos(&nfse.chave),
                },
                None => Self::Recusada {
                    texto: "A Sefin respondeu, mas sem os dados da nota. Tente de novo.".into(),
                },
            },
            Status::NaoEncontrada => Self::NaoEncontrada,
            Status::Erro if r.http == 0 && r.detalhe.is_empty() => Self::Recusada {
                texto: frase(&r.frase),
            },
            Status::Rejeitada | Status::Pendente | Status::Indisponivel | Status::Erro => {
                Self::SemResposta {
                    texto: TEXTO_SEM_RESPOSTA.into(),
                }
            }
        }
    }
}

/// O que se pode fazer com a nota consultada em relacao a pasta de notas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Guarda {
    /// Pode guardar: e' nota do emitente da configuracao e ainda nao esta na pasta.
    Guardar,
    /// A nota ja esta na pasta de notas.
    JaEstaNaPasta,
    /// A nota e' de outro emitente: nao e' guardada nesta pasta.
    OutroEmitente,
}

/// O que a tela oferece para a nota consultada: so' guarda nota do emitente da configuracao.
#[must_use]
pub fn guarda(e_do_emitente: bool, na_pasta: bool) -> Guarda {
    if !e_do_emitente {
        Guarda::OutroEmitente
    } else if na_pasta {
        Guarda::JaEstaNaPasta
    } else {
        Guarda::Guardar
    }
}

#[cfg(test)]
mod testes {
    use rustnf::nfse::leitura::ler_nfse;
    use rustnf::nfse::sefin::Resultado;

    use super::*;

    const XML: &str = include_str!("../../../rustnf-cli/exemplos/nfse.xml");

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

    fn achada(cancelada: Option<bool>) -> Consultada {
        let nfse = ler_nfse(XML).unwrap();
        let mut r = resultado(Status::Autorizada);
        r.chave = nfse.chave.clone();
        r.nfse = Some(nfse);
        Consultada {
            resultado: r,
            cancelada,
        }
    }

    fn chave_do_exemplo() -> String {
        ler_nfse(XML).unwrap().chave
    }

    #[test]
    fn a_nota_com_a_chave_pedida_e_o_resultado() {
        let pedida = chave::em_grupos(&chave_do_exemplo());
        let DesfechoDaConsulta::Encontrada(n) =
            DesfechoDaConsulta::de(achada(Some(false)), &pedida)
        else {
            panic!("esperava encontrada");
        };
        assert_eq!(n.detalhe.numero, "25");
        assert_eq!(n.detalhe.situacao, Situacao::Autorizada);
        assert!(n.situacao_conferida);
        assert_eq!(n.registro.chave, chave_do_exemplo());
        assert_eq!(n.registro.situacao, Situacao::Autorizada);
        assert!(n.xml.contains("<NFSe"));
    }

    #[test]
    fn nota_cancelada_na_sefin_leva_a_situacao_para_o_detalhe_e_para_o_registro() {
        let DesfechoDaConsulta::Encontrada(n) =
            DesfechoDaConsulta::de(achada(Some(true)), &chave_do_exemplo())
        else {
            panic!("esperava encontrada");
        };
        assert_eq!(n.detalhe.situacao, Situacao::Cancelada);
        assert_eq!(n.registro.situacao, Situacao::Cancelada);
    }

    #[test]
    fn nao_saber_se_foi_cancelada_nao_e_dizer_que_nao_foi() {
        let DesfechoDaConsulta::Encontrada(n) =
            DesfechoDaConsulta::de(achada(None), &chave_do_exemplo())
        else {
            panic!("esperava encontrada");
        };
        assert!(!n.situacao_conferida);
    }

    #[test]
    fn nota_de_outra_chave_nao_e_o_resultado() {
        let outra =
            rustnf::chave_dfe::montar_nfse(1_400_159, "11222333000181", 99, "2609", Some(1), 2)
                .unwrap();
        let d = DesfechoDaConsulta::de(achada(Some(false)), &outra);
        let DesfechoDaConsulta::Divergente { pedida, devolvida } = d else {
            panic!("esperava divergente");
        };
        assert_eq!(chave::limpar(&pedida), outra);
        assert_eq!(chave::limpar(&devolvida), chave_do_exemplo());
    }

    #[test]
    fn os_outros_desfechos() {
        let c = |r| Consultada {
            resultado: r,
            cancelada: None,
        };
        assert_eq!(
            DesfechoDaConsulta::de(c(resultado(Status::NaoEncontrada)), "x"),
            DesfechoDaConsulta::NaoEncontrada
        );
        for status in [Status::Pendente, Status::Indisponivel] {
            assert!(matches!(
                DesfechoDaConsulta::de(c(resultado(status)), "x"),
                DesfechoDaConsulta::SemResposta { .. }
            ));
        }
        let mut r = resultado(Status::Erro);
        r.http = 500;
        assert!(matches!(
            DesfechoDaConsulta::de(c(r), "x"),
            DesfechoDaConsulta::SemResposta { .. }
        ));
        let mut r = resultado(Status::Erro);
        r.http = 0;
        assert!(matches!(
            DesfechoDaConsulta::de(c(r), "x"),
            DesfechoDaConsulta::Recusada { texto } if texto == "Frase da biblioteca"
        ));
        // autorizada sem a nota: nao ha o que mostrar
        assert!(matches!(
            DesfechoDaConsulta::de(c(resultado(Status::Autorizada)), "x"),
            DesfechoDaConsulta::Recusada { .. }
        ));
    }

    #[test]
    fn so_se_guarda_nota_do_emitente_que_ainda_nao_esta_na_pasta() {
        assert_eq!(guarda(true, false), Guarda::Guardar);
        assert_eq!(guarda(true, true), Guarda::JaEstaNaPasta);
        assert_eq!(guarda(false, false), Guarda::OutroEmitente);
        assert_eq!(guarda(false, true), Guarda::OutroEmitente);
    }
}
