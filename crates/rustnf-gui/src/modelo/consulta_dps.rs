// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A consulta pela serie e numero da DPS: de quem e' a nota (o `[emitente]` da configuracao), o Id de 45 posicoes
//! montado e validado pela biblioteca, e o que a tela diz enquanto a pessoa digita. Sem janela.

use rustnf_app::config::Emitente;
use rustnf_app::eventos::id_da_dps;

use super::mascara::frase;

/// Como a pessoa quer achar a nota.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ModoDeConsulta {
    /// Pela chave de acesso (50 posicoes).
    #[default]
    Chave,
    /// Pela serie e pelo numero da DPS.
    Dps,
}

impl ModoDeConsulta {
    /// Os modos, na ordem em que a tela os mostra.
    pub const TODOS: [ModoDeConsulta; 2] = [Self::Chave, Self::Dps];

    /// O rotulo do modo.
    #[must_use]
    pub fn rotulo(self) -> &'static str {
        match self {
            Self::Chave => "Pela chave de acesso",
            Self::Dps => "Pela série e número da DPS",
        }
    }
}

/// O que a tela diz da serie e do numero digitados.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EstadoDaDps {
    /// Nada digitado no numero.
    Vazio,
    /// Nao da para montar o Id: o motivo, em frase.
    Invalido(String),
    /// O Id da DPS (45 posicoes) que se vai consultar.
    Valido(String),
}

impl EstadoDaDps {
    /// Pode consultar.
    #[must_use]
    pub fn valido(&self) -> bool {
        matches!(self, Self::Valido(_))
    }

    /// O motivo, se nao serve e ja ha o que dizer.
    #[must_use]
    pub fn mensagem(&self) -> Option<String> {
        match self {
            Self::Invalido(m) => Some(m.clone()),
            _ => None,
        }
    }
}

/// So' os digitos de `texto` (a pessoa cola "1.234" ou " 12 ").
#[must_use]
pub fn so_digitos(texto: &str) -> String {
    texto.chars().filter(char::is_ascii_digit).collect()
}

/// A serie que vale: a digitada, ou a do `[emitente]` na falta dela, ou 1 (como a emissao).
#[must_use]
pub fn serie_efetiva(emitente: &Emitente, digitada: &str) -> String {
    let d = so_digitos(digitada);
    if !d.is_empty() {
        return d;
    }
    let da_config = so_digitos(&emitente.serie);
    if da_config.is_empty() {
        "1".into()
    } else {
        da_config
    }
}

/// Confere a serie e o numero digitados e monta o Id da DPS com o `[emitente]`.
#[must_use]
pub fn conferir_dps(emitente: &Emitente, serie: &str, numero: &str) -> EstadoDaDps {
    let numero = so_digitos(numero);
    if numero.is_empty() {
        return EstadoDaDps::Vazio;
    }
    if emitente.municipio.trim().is_empty()
        || (emitente.cnpj.trim().is_empty() && emitente.cpf.trim().is_empty())
    {
        return EstadoDaDps::Invalido(
            "Para achar a nota pela DPS, preencha o município e o CNPJ do emitente nas Configurações: o Id da DPS é montado com eles.".into(),
        );
    }
    let serie = serie_efetiva(emitente, serie);
    if serie.len() > 5 {
        return EstadoDaDps::Invalido("A série tem de 1 a 5 dígitos.".into());
    }
    if numero.len() > 15 {
        return EstadoDaDps::Invalido("O número da DPS tem no máximo 15 dígitos.".into());
    }
    if numero.starts_with('0') {
        return EstadoDaDps::Invalido("O número da DPS não começa com zero.".into());
    }
    match id_da_dps(emitente, Some(&serie), &numero) {
        Ok(id) => EstadoDaDps::Valido(id),
        Err(e) => EstadoDaDps::Invalido(frase(&e.mensagem())),
    }
}

/// O que a tela diz quando a Sefin nao tem nota para a DPS.
pub const TEXTO_NAO_ENCONTRADA: &str = "A Sefin Nacional não tem nota gerada por esta DPS neste ambiente. Confira a série e o número e se o ambiente das Configurações (produção ou produção restrita) é o da nota. A consulta usa o município e o CNPJ do emitente das Configurações.";

#[cfg(test)]
mod testes {
    use super::*;

    fn emitente() -> Emitente {
        Emitente {
            cnpj: "11222333000181".into(),
            municipio: "3550308".into(),
            serie: "7".into(),
            ..Emitente::default()
        }
    }

    #[test]
    fn o_id_sai_do_emitente_da_configuracao_com_45_posicoes() {
        let EstadoDaDps::Valido(id) = conferir_dps(&emitente(), "900", "1234") else {
            panic!("esperava valido");
        };
        assert_eq!(id.len(), 45);
        assert_eq!(id, "DPS355030821122233300018100900000000000001234");
        assert_eq!(
            id,
            rustnf::nfse::id_dps("3550308", "11222333000181", "900", "1234").unwrap()
        );
    }

    #[test]
    fn sem_serie_vale_a_do_emitente_e_na_falta_dela_a_1() {
        let EstadoDaDps::Valido(id) = conferir_dps(&emitente(), "", "5") else {
            panic!("esperava valido");
        };
        assert!(id.contains("00007000000000000005"), "{id}");
        let mut sem = emitente();
        sem.serie.clear();
        let EstadoDaDps::Valido(id) = conferir_dps(&sem, "  ", "5") else {
            panic!("esperava valido");
        };
        assert!(id.contains("00001000000000000005"), "{id}");
    }

    #[test]
    fn o_numero_colado_com_ponto_ou_espaco_vale_pelos_digitos() {
        assert_eq!(so_digitos(" 1.234 "), "1234");
        assert!(conferir_dps(&emitente(), "1", "1.234").valido());
    }

    #[test]
    fn nada_digitado_nao_e_erro_ainda() {
        assert_eq!(conferir_dps(&emitente(), "1", ""), EstadoDaDps::Vazio);
        assert_eq!(conferir_dps(&emitente(), "1", "abc"), EstadoDaDps::Vazio);
        assert_eq!(EstadoDaDps::Vazio.mensagem(), None);
    }

    #[test]
    fn cada_recusa_diz_o_motivo_em_portugues() {
        let motivo = |e: &Emitente, s: &str, n: &str| conferir_dps(e, s, n).mensagem().unwrap();
        assert!(motivo(&emitente(), "123456", "1").contains("1 a 5 dígitos"));
        assert!(motivo(&emitente(), "1", "1234567890123456").contains("15 dígitos"));
        assert!(motivo(&emitente(), "1", "012").contains("zero"));
        let mut sem = emitente();
        sem.municipio.clear();
        assert!(motivo(&sem, "1", "1").contains("Configurações"));
        let mut sem = emitente();
        sem.cnpj.clear();
        assert!(motivo(&sem, "1", "1").contains("Configurações"));
        // a biblioteca recusa o que a tela nao previu (serie reservada) e a tela repete a frase dela
        assert!(!motivo(&emitente(), "90001", "1").is_empty());
    }

    #[test]
    fn emitente_com_cpf_monta_o_id_de_cpf() {
        let mut e = emitente();
        e.cnpj.clear();
        e.cpf = "52998224725".into();
        let EstadoDaDps::Valido(id) = conferir_dps(&e, "1", "9") else {
            panic!("esperava valido");
        };
        assert!(id.starts_with("DPS35503081"), "{id}");
    }
}
