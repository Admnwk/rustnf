// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O desfecho de uma emissao como tipo da tela, a partir do que `emitir_pedido_completo` devolve. Nenhuma frase aqui
//! leva caminho de arquivo, senha, XML assinado nem detalhe de rede.

use rustnf::nfse::leitura::Mensagem;
use rustnf::nfse::sefin::Status;
use rustnf_app::erro::Erro;
use rustnf_app::pedido::Emitida;

use super::conferencia::{campo_do_codigo, pendencia_da_recusa, Pendencia};
use super::mascara::frase;
use super::nota::CampoNota;

/// Uma mensagem da Sefin: a descricao em destaque, o codigo e o complemento em segundo plano.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MensagemSefin {
    /// O codigo (`E0204`).
    pub codigo: String,
    /// A descricao.
    pub descricao: String,
    /// O complemento, quando ha.
    pub complemento: String,
    /// A orientacao da biblioteca para os retornos mais comuns, quando ha.
    pub dica: String,
    /// O campo da tela que a mensagem aponta, quando o mapa conhece o codigo.
    pub campo: Option<CampoNota>,
}

impl From<&Mensagem> for MensagemSefin {
    fn from(m: &Mensagem) -> Self {
        Self {
            codigo: m.codigo.trim().to_string(),
            descricao: m.descricao.trim().to_string(),
            complemento: m.complemento.trim().to_string(),
            dica: frase(m.dica()),
            campo: campo_do_codigo(&m.codigo),
        }
    }
}

/// Como a emissao terminou.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DesfechoEmissao {
    /// A Sefin autorizou (ou a nota ja existia e foi recuperada).
    Autorizada {
        /// O numero da NFS-e.
        numero: String,
        /// A chave de acesso, 50 posicoes.
        chave: String,
        /// O XML foi guardado na pasta de notas.
        guardada: bool,
        /// O aviso, quando a pasta nao pode ser atualizada por completo.
        aviso: Option<String>,
        /// A nota ja existia na Sefin com este numero de DPS e foi achada pela consulta.
        recuperada: bool,
        /// Nota recuperada: em que ela difere do formulario. Com diferenca, a nota que existe NAO e' a que se
        /// quis emitir (sobrou de uma tentativa anterior com outros dados).
        diferencas: Vec<String>,
        /// Alertas da Sefin que acompanham a autorizacao.
        alertas: Vec<MensagemSefin>,
    },
    /// A Sefin recusou a DPS.
    Rejeitada {
        /// As mensagens da Sefin.
        mensagens: Vec<MensagemSefin>,
    },
    /// Sem resposta: rede, Sefin indisponivel ou pendente. A nota PODE ter sido emitida.
    SemResposta {
        /// A frase para a tela.
        texto: String,
    },
    /// Recusada antes de enviar (regra da biblioteca ou pasta de notas): nada saiu.
    Recusada {
        /// A pendencia, no campo da tela quando o mapa conhece o campo.
        pendencia: Pendencia,
    },
}

impl DesfechoEmissao {
    /// O desfecho do que `emitir_pedido_completo` devolveu. `op_simples` e' o regime do emitente (decide o campo de
    /// algumas recusas).
    #[must_use]
    pub fn de(resultado: Result<Emitida, Erro>, op_simples: u8) -> Self {
        match resultado {
            Ok(emitida) => Self::do_resultado(emitida),
            Err(Erro::Recusa { campo, mensagem }) => Self::Recusada {
                pendencia: pendencia_da_recusa(&campo, &mensagem, op_simples),
            },
            Err(outro) => Self::Recusada {
                pendencia: Pendencia::geral(outro.mensagem()),
            },
        }
    }

    fn do_resultado(emitida: Emitida) -> Self {
        let r = &emitida.resultado;
        match r.status {
            Status::Autorizada | Status::Ok | Status::Registrado => {
                let nfse = r.nfse.as_ref();
                Self::Autorizada {
                    numero: nfse.map(|n| n.numero.clone()).unwrap_or_default(),
                    chave: if r.chave.is_empty() {
                        nfse.map(|n| n.chave.clone()).unwrap_or_default()
                    } else {
                        r.chave.clone()
                    },
                    guardada: emitida.guardada.unwrap_or(false),
                    aviso: emitida.aviso.map(|_| AVISO_DA_PASTA.to_string()),
                    recuperada: r.recuperada,
                    diferencas: emitida
                        .diferencas
                        .iter()
                        .map(|d| rotulo_da_diferenca(d).to_string())
                        .collect(),
                    alertas: r.alertas.iter().map(MensagemSefin::from).collect(),
                }
            }
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
                Self::Rejeitada { mensagens }
            }
            // recusa local da biblioteca (nada saiu para a rede)
            Status::Erro if r.http == 0 && r.detalhe.is_empty() => Self::Recusada {
                pendencia: Pendencia::geral(frase(&r.frase)),
            },
            Status::Pendente | Status::Indisponivel | Status::Erro | Status::NaoEncontrada => {
                Self::SemResposta {
                    texto: TEXTO_SEM_RESPOSTA.into(),
                }
            }
        }
    }

    /// O desfecho deixa a nota PENDENTE: o pedido continua congelado e so' se sai dele tentando de novo ou desistindo.
    #[must_use]
    pub fn deixa_pendente(&self) -> bool {
        matches!(self, Self::SemResposta { .. })
    }
}

/// O aviso quando a nota foi autorizada mas a pasta nao pode ser atualizada por completo.
pub const AVISO_DA_PASTA: &str = "A nota foi autorizada, mas a pasta de notas não pôde ser atualizada por completo. Consulte a nota pela chave de acesso e confira a numeração.";

/// O que a tela diz quando nao houve resposta. A biblioteca assina a DPS UMA vez e, se a resposta se perde, consulta
/// a Sefin pelo Id da DPS: achando, a nota vem como autorizada e recuperada; nao achando, o mesmo pedido pode ser
/// reenviado, e a Sefin deduplica pelo Id.
pub const TEXTO_SEM_RESPOSTA: &str = "A Sefin Nacional não respondeu (ou está indisponível), e não foi possível confirmar se a nota foi emitida. Ela pode ter sido autorizada.";

/// Explica o que "Tentar de novo" faz.
pub const EXPLICACAO_DE_TENTAR_DE_NOVO: &str = "Tentar de novo reenvia exatamente o mesmo pedido (mesmo número de DPS e mesma data e hora de emissão). Se a nota já existir na Sefin, ela é recuperada, sem duplicar.";

/// O aviso de quem desiste de uma tentativa sem desfecho. O numero da DPS nao avanca na desistencia: a proxima
/// emissao usa o mesmo, e a Sefin devolve a nota anterior se ela existir.
#[must_use]
pub fn aviso_de_desistir(serie: &str, numero: u64) -> String {
    format!(
        "A nota PODE ter sido autorizada (DPS número {numero}, série {serie}). A próxima emissão usa esse mesmo número: se a nota anterior existir na Sefin, é ela que volta, e a tela avisa quando os dados dela não são os do formulário."
    )
}

/// O nome, para a tela, de cada diferenca que a camada de aplicacao aponta numa nota recuperada.
fn rotulo_da_diferenca(diferenca: &str) -> &str {
    match diferenca {
        "valor do servico" => "valor do serviço",
        "codigo do servico" => "código do serviço",
        "competencia" => "competência",
        outra => outra,
    }
}

/// O que a tela diz da nota recuperada cujos dados conferem com os do formulario.
pub const TEXTO_DA_RECUPERADA: &str = "A nota já existia na Sefin com este número de DPS (a resposta de um envio anterior se perdeu) e foi recuperada pela consulta. Os dados dela conferem com os do formulário. Não houve duplicidade.";

/// O que a tela diz da nota recuperada que NAO e' a do formulario.
#[must_use]
pub fn texto_da_recuperada_diferente(diferencas: &[String]) -> String {
    format!(
        "Já existia na Sefin uma nota com este número de DPS, de uma tentativa anterior, e ela NÃO é a do formulário: difere em {}. Ela foi autorizada naquela tentativa e é a nota mostrada abaixo. A nota do formulário ainda não foi emitida: volte ao formulário, que está como você deixou, e emita de novo (o número da DPS já avançou).",
        diferencas.join(", ")
    )
}

#[cfg(test)]
mod testes {
    use rustnf::nfse::leitura::ler_nfse;
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

    fn emitida(r: Resultado) -> Emitida {
        Emitida {
            resultado: r,
            numero_dps: None,
            guardada: None,
            aviso: None,
            diferencas: Vec::new(),
        }
    }

    fn msg(codigo: &str, desc: &str, compl: &str) -> Mensagem {
        Mensagem {
            codigo: codigo.into(),
            descricao: desc.into(),
            complemento: compl.into(),
        }
    }

    #[test]
    fn autorizada_leva_numero_chave_guarda_e_alertas() {
        let xml = include_str!("../../../rustnf-cli/exemplos/nfse.xml");
        let nfse = ler_nfse(xml).unwrap();
        let mut r = resultado(Status::Autorizada);
        r.chave = nfse.chave.clone();
        r.nfse = Some(nfse);
        r.alertas = vec![msg("A001", " atencao ", "")];
        r.recuperada = true;
        let mut e = emitida(r);
        e.guardada = Some(true);
        match DesfechoEmissao::de(Ok(e), 3) {
            DesfechoEmissao::Autorizada {
                numero,
                chave,
                guardada,
                aviso,
                recuperada,
                diferencas,
                alertas,
            } => {
                assert!(diferencas.is_empty());
                assert_eq!(numero, "25");
                assert_eq!(chave.len(), 50);
                assert!(guardada);
                assert_eq!(aviso, None);
                assert!(recuperada);
                assert_eq!(alertas.len(), 1);
                assert_eq!(alertas[0].descricao, "atencao");
            }
            outro => panic!("{outro:?}"),
        }
    }

    #[test]
    fn recuperada_de_outra_tentativa_leva_as_diferencas_com_o_nome_da_tela() {
        let mut r = resultado(Status::Autorizada);
        r.recuperada = true;
        let mut e = emitida(r);
        e.diferencas = vec![
            "tomador",
            "valor do servico",
            "codigo do servico",
            "competencia",
        ];
        let DesfechoEmissao::Autorizada { diferencas, .. } = DesfechoEmissao::de(Ok(e), 3) else {
            panic!("esperava autorizada");
        };
        assert_eq!(
            diferencas,
            [
                "tomador",
                "valor do serviço",
                "código do serviço",
                "competência"
            ]
        );
        let texto = texto_da_recuperada_diferente(&diferencas);
        assert!(texto.contains("NÃO é a do formulário"));
        assert!(texto.contains("tomador, valor do serviço"));
        assert!(texto.contains("ainda não foi emitida"));
    }

    #[test]
    fn autorizada_com_a_pasta_falhando_leva_o_aviso() {
        let mut e = emitida(resultado(Status::Autorizada));
        e.guardada = Some(false);
        e.aviso = Some("qualquer");
        match DesfechoEmissao::de(Ok(e), 3) {
            DesfechoEmissao::Autorizada {
                guardada, aviso, ..
            } => {
                assert!(!guardada);
                assert_eq!(aviso.as_deref(), Some(AVISO_DA_PASTA));
            }
            outro => panic!("{outro:?}"),
        }
    }

    #[test]
    fn rejeitada_leva_as_mensagens_com_o_campo_quando_o_mapa_conhece() {
        let mut r = resultado(Status::Rejeitada);
        r.erros = vec![
            msg("E0204", "Tomador sem documento", "toma"),
            msg("E9999", "Outra coisa", ""),
        ];
        match DesfechoEmissao::de(Ok(emitida(r)), 3) {
            DesfechoEmissao::Rejeitada { mensagens } => {
                assert_eq!(mensagens.len(), 2);
                assert_eq!(mensagens[0].codigo, "E0204");
                assert_eq!(mensagens[0].campo, Some(CampoNota::TomadorDocumento));
                assert_eq!(mensagens[0].complemento, "toma");
                assert_eq!(mensagens[1].campo, None);
            }
            outro => panic!("{outro:?}"),
        }
        // sem mensagens estruturadas, a frase da biblioteca
        let r = resultado(Status::Rejeitada);
        match DesfechoEmissao::de(Ok(emitida(r)), 3) {
            DesfechoEmissao::Rejeitada { mensagens } => {
                assert_eq!(mensagens.len(), 1);
                assert_eq!(mensagens[0].descricao, "Frase da biblioteca");
            }
            outro => panic!("{outro:?}"),
        }
    }

    #[test]
    fn sem_resposta_e_pendente_deixa_o_pedido_travado() {
        for status in [
            Status::Pendente,
            Status::Indisponivel,
            Status::NaoEncontrada,
        ] {
            let d = DesfechoEmissao::de(Ok(emitida(resultado(status))), 3);
            assert!(d.deixa_pendente(), "{status:?}");
            assert!(
                matches!(d, DesfechoEmissao::SemResposta { texto } if texto == TEXTO_SEM_RESPOSTA)
            );
        }
        // erro com resposta HTTP tambem nao confirma nada
        let mut r = resultado(Status::Erro);
        r.http = 500;
        assert!(DesfechoEmissao::de(Ok(emitida(r)), 3).deixa_pendente());
    }

    #[test]
    fn recusa_local_nao_deixa_pendente() {
        let mut r = resultado(Status::Erro);
        r.http = 0;
        let d = DesfechoEmissao::de(Ok(emitida(r)), 3);
        assert!(!d.deixa_pendente());
        assert!(matches!(d, DesfechoEmissao::Recusada { .. }));
        let d = DesfechoEmissao::de(
            Err(Erro::recusa(
                "infDPS/dCompet",
                "feio (regra E0015 do SEFIN)",
            )),
            3,
        );
        match d {
            DesfechoEmissao::Recusada { pendencia } => {
                assert_eq!(pendencia.campo, Some(CampoNota::Competencia));
                assert_eq!(pendencia.regra.as_deref(), Some("E0015"));
            }
            outro => panic!("{outro:?}"),
        }
        let d = DesfechoEmissao::de(Err(Erro::Entrada("notas: pasta".into())), 3);
        assert!(matches!(d, DesfechoEmissao::Recusada { pendencia } if pendencia.campo.is_none()));
    }

    #[test]
    fn os_textos_dizem_o_que_o_codigo_faz() {
        assert!(TEXTO_SEM_RESPOSTA.contains("pode ter sido autorizada"));
        assert!(EXPLICACAO_DE_TENTAR_DE_NOVO.contains("mesmo número de DPS"));
        assert!(EXPLICACAO_DE_TENTAR_DE_NOVO.contains("recuperada"));
        let a = aviso_de_desistir("1", 42);
        assert!(a.contains("PODE ter sido autorizada"));
        assert!(a.contains("número 42"));
        assert!(a.contains("série 1"));
        assert!(a.contains("mesmo número"));
    }
}
