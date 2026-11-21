// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O que a tela diz do certificado: o estado do rodape e o resultado do botao "Testar certificado". Nunca leva o
//! caminho do arquivo nem a senha.

use chrono::{DateTime, Duration, Utc};
use rustnf::assinatura::cert_a1::{Certificado, Situacao, AVISO_DIAS};

use super::mascara::documento_para_resumo;

/// `DD/MM/AAAA` no horario de Brasilia (UTC-3, sem horario de verao desde 2019).
#[must_use]
pub fn formatar_data(data: DateTime<Utc>) -> String {
    (data - Duration::hours(3)).format("%d/%m/%Y").to_string()
}

/// Gravidade do estado, para escolher a cor do texto no rodape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gravidade {
    /// Tudo certo.
    Ok,
    /// Precisa de atencao, mas ainda funciona.
    Aviso,
    /// Nao funciona.
    Erro,
    /// Ainda nao ha o que dizer.
    Neutro,
}

/// O estado do certificado configurado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EstadoCertificado {
    /// Conferindo (a leitura do arquivo roda fora da tela).
    Verificando,
    /// `[sefin].pfx` vazio.
    SemCertificado,
    /// O arquivo indicado nao existe.
    ArquivoNaoEncontrado,
    /// Nao ha senha no cofre para este certificado.
    SenhaNaoGuardada,
    /// A senha do cofre nao abre o certificado.
    SenhaNaoConfere,
    /// O arquivo nao e' um certificado A1 legivel.
    Ilegivel,
    /// Valido, com mais de 30 dias pela frente.
    Valido {
        /// `DD/MM/AAAA`.
        ate: String,
        /// Dias que faltam.
        dias: i64,
    },
    /// Vence em 30 dias ou menos.
    VenceEmBreve {
        /// `DD/MM/AAAA`.
        ate: String,
        /// Dias que faltam.
        dias: i64,
    },
    /// Ja venceu.
    Vencido {
        /// `DD/MM/AAAA`.
        ate: String,
    },
    /// A validade ainda nao comecou (relogio errado ou certificado novo).
    AindaNaoValido,
}

impl EstadoCertificado {
    /// Texto curto do rodape.
    #[must_use]
    pub fn texto(&self) -> String {
        match self {
            Self::Verificando => "Certificado: conferindo...".into(),
            Self::SemCertificado => "Sem certificado configurado".into(),
            Self::ArquivoNaoEncontrado => "Certificado: arquivo não encontrado".into(),
            Self::SenhaNaoGuardada => "Certificado: senha não guardada".into(),
            Self::SenhaNaoConfere => "Certificado: a senha guardada não confere".into(),
            Self::Ilegivel => "Certificado: arquivo ilegível".into(),
            Self::Valido { ate, dias } => {
                format!("Certificado válido até {ate} ({})", dias_texto(*dias))
            }
            Self::VenceEmBreve { dias, .. } => {
                format!("Certificado vence em {}", dias_texto(*dias))
            }
            Self::Vencido { ate } => format!("Certificado vencido em {ate}"),
            Self::AindaNaoValido => "Certificado ainda não é válido".into(),
        }
    }

    /// Gravidade do estado.
    #[must_use]
    pub fn gravidade(&self) -> Gravidade {
        match self {
            Self::Verificando => Gravidade::Neutro,
            Self::Valido { .. } => Gravidade::Ok,
            Self::SemCertificado | Self::SenhaNaoGuardada | Self::VenceEmBreve { .. } => {
                Gravidade::Aviso
            }
            Self::ArquivoNaoEncontrado
            | Self::SenhaNaoConfere
            | Self::Ilegivel
            | Self::Vencido { .. }
            | Self::AindaNaoValido => Gravidade::Erro,
        }
    }
}

fn dias_texto(dias: i64) -> String {
    if dias == 1 {
        "1 dia".into()
    } else {
        format!("{dias} dias")
    }
}

/// O estado de um certificado ja aberto, no instante `agora`.
#[must_use]
pub fn avaliar(certificado: &Certificado, agora: DateTime<Utc>) -> EstadoCertificado {
    let ate = formatar_data(certificado.validade_fim);
    let dias = certificado.dias_restantes(agora);
    match certificado.situacao(agora, AVISO_DIAS) {
        Situacao::Valido => EstadoCertificado::Valido { ate, dias },
        Situacao::VenceEmBreve => EstadoCertificado::VenceEmBreve { ate, dias },
        Situacao::Vencido => EstadoCertificado::Vencido { ate },
        Situacao::AindaNaoValido => EstadoCertificado::AindaNaoValido,
    }
}

/// O que o botao "Testar certificado" mostra quando o arquivo abre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InfoCertificado {
    /// O certificado esta apto a assinar hoje.
    pub apto: bool,
    /// Nome do titular, como esta no certificado.
    pub titular: String,
    /// CNPJ (inteiro) ou CPF (mascarado) do certificado; vazio se ele nao traz documento.
    pub documento: String,
    /// `DD/MM/AAAA`.
    pub validade: String,
    /// Estado para o rodape.
    pub estado: EstadoCertificado,
    /// Avisos da biblioteca (outro CNPJ, vence em breve, certificado de teste).
    pub avisos: Vec<String>,
    /// Frase pronta quando o certificado nao serve (vencido, ainda nao valido).
    pub problema: Option<String>,
}

/// Monta o resultado do teste a partir do certificado aberto. `cnpj_empresa` e' o documento do `[emitente]`, para
/// avisar quando o certificado e' de outro.
#[must_use]
pub fn testar(
    certificado: &Certificado,
    cnpj_empresa: Option<&str>,
    agora: DateTime<Utc>,
) -> InfoCertificado {
    let teste = certificado.testar(cnpj_empresa, agora, AVISO_DIAS);
    let documento = certificado.documento();
    InfoCertificado {
        apto: teste.ok,
        titular: if certificado.titular.is_empty() {
            "(titular sem nome)".into()
        } else {
            certificado.titular.clone()
        },
        documento: if documento.is_empty() {
            String::new()
        } else {
            documento_para_resumo(documento)
        },
        validade: formatar_data(certificado.validade_fim),
        estado: avaliar(certificado, agora),
        avisos: teste.avisos,
        problema: (!teste.ok).then_some(teste.frase),
    }
}

#[cfg(test)]
mod testes {
    use rustnf::assinatura::cert_a1::{carregar, gerar_teste};

    use super::*;

    fn certificado(dias: i64, agora: DateTime<Utc>) -> Certificado {
        let pfx = gerar_teste(
            "11222333000181",
            "EMPRESA TESTE",
            "s",
            dias,
            false,
            false,
            agora,
        )
        .unwrap();
        carregar(&pfx, "s").unwrap()
    }

    #[test]
    fn data_em_dia_mes_ano_no_horario_de_brasilia() {
        let t = "2026-12-31T01:30:00Z".parse::<DateTime<Utc>>().unwrap();
        // 01:30 UTC ainda e' 30/12 em Brasilia
        assert_eq!(formatar_data(t), "30/12/2026");
        let t = "2027-03-05T15:00:00Z".parse::<DateTime<Utc>>().unwrap();
        assert_eq!(formatar_data(t), "05/03/2027");
    }

    #[test]
    fn estado_valido_aviso_e_vencido() {
        let agora = Utc::now();
        let longo = certificado(365, agora);
        match avaliar(&longo, agora) {
            EstadoCertificado::Valido { dias, ate } => {
                assert!((363..=365).contains(&dias), "{dias}");
                assert_eq!(ate.len(), 10);
            }
            outro => panic!("esperava Valido, veio {outro:?}"),
        }
        let curto = certificado(10, agora);
        let e = avaliar(&curto, agora);
        assert!(matches!(e, EstadoCertificado::VenceEmBreve { .. }), "{e:?}");
        assert_eq!(e.gravidade(), Gravidade::Aviso);
        assert!(e.texto().starts_with("Certificado vence em "));
        // 400 dias depois, o de 365 esta vencido
        let e = avaliar(&longo, agora + Duration::days(400));
        assert!(matches!(e, EstadoCertificado::Vencido { .. }), "{e:?}");
        assert_eq!(e.gravidade(), Gravidade::Erro);
        // antes de comecar
        let e = avaliar(&longo, agora - Duration::days(400));
        assert_eq!(e, EstadoCertificado::AindaNaoValido);
    }

    #[test]
    fn textos_do_rodape_nao_levam_caminho_nem_senha() {
        let estados = [
            EstadoCertificado::Verificando,
            EstadoCertificado::SemCertificado,
            EstadoCertificado::ArquivoNaoEncontrado,
            EstadoCertificado::SenhaNaoGuardada,
            EstadoCertificado::SenhaNaoConfere,
            EstadoCertificado::Ilegivel,
            EstadoCertificado::Valido {
                ate: "01/02/2027".into(),
                dias: 120,
            },
            EstadoCertificado::VenceEmBreve {
                ate: "01/02/2027".into(),
                dias: 1,
            },
            EstadoCertificado::Vencido {
                ate: "01/02/2026".into(),
            },
            EstadoCertificado::AindaNaoValido,
        ];
        for e in estados {
            let t = e.texto();
            assert!(!t.contains('\\'), "{t}");
            assert!(!t.contains(".pfx"), "{t}");
        }
        assert_eq!(
            EstadoCertificado::Valido {
                ate: "01/02/2027".into(),
                dias: 120
            }
            .texto(),
            "Certificado válido até 01/02/2027 (120 dias)"
        );
        assert_eq!(
            EstadoCertificado::VenceEmBreve {
                ate: "x".into(),
                dias: 1
            }
            .texto(),
            "Certificado vence em 1 dia"
        );
    }

    #[test]
    fn teste_mostra_titular_documento_e_validade() {
        let agora = Utc::now();
        let c = certificado(365, agora);
        let info = testar(&c, Some("11222333000181"), agora);
        assert!(info.apto);
        assert_eq!(info.documento, "11.222.333/0001-81");
        assert!(info.titular.contains("EMPRESA TESTE"));
        assert!(info.problema.is_none());
        // de outro documento: aviso, mas ainda abre
        let info = testar(&c, Some("52998224725"), agora);
        assert!(
            info.avisos.iter().any(|a| a.contains("outro documento")),
            "{:?}",
            info.avisos
        );
        // vencido: nao apto e com o problema
        let info = testar(&c, None, agora + Duration::days(500));
        assert!(!info.apto);
        assert!(info.problema.is_some());
    }
}
