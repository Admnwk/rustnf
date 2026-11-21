// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A chave de acesso digitada ou colada: limpeza, mascara em grupos, conferencia com o motivo em portugues e a
//! comparacao com a chave da nota que a Sefin devolveu.

use rustnf::chave_dfe::{self, ErroChave};

/// Posicoes da chave de acesso da NFS-e Nacional.
pub const TAMANHO: usize = 50;

/// Quantas posicoes tem cada grupo da chave mostrada.
const GRUPO: usize = 5;

/// A chave "crua" do texto: so' letras e digitos, em maiusculas, a partir do primeiro par de digitos (o `Id` da
/// NFS-e traz um prefixo de letras, `NFS`), no maximo 50 posicoes. E' a chave que se consulta e se compara.
#[must_use]
pub fn limpar(texto: &str) -> String {
    let v: Vec<char> = texto
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_uppercase())
        .collect();
    // a chave comeca por digito (municipio): so' o prefixo de letras do `Id` fica de fora. Um unico digito digitado
    // ainda e' o comeco da chave.
    let inicio = v.iter().position(char::is_ascii_digit).unwrap_or(v.len());
    v[inicio..].iter().take(TAMANHO).collect()
}

/// A chave em grupos de 5 separados por espaco, para ler, conferir a olho e quebrar a linha. Aceita a chave inteira
/// ou o que ja foi digitado dela.
#[must_use]
pub fn em_grupos(texto: &str) -> String {
    limpar(texto)
        .as_bytes()
        .chunks(GRUPO)
        .map(|c| String::from_utf8_lossy(c).into_owned())
        .collect::<Vec<_>>()
        .join(" ")
}

/// O que a tela diz da chave digitada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EstadoDaChave {
    /// Nada digitado.
    Vazia,
    /// Ainda faltam posicoes.
    Incompleta {
        /// Quantas posicoes faltam.
        faltam: usize,
    },
    /// As 50 posicoes, mas a chave nao confere: o motivo, em frase.
    Invalida(String),
    /// Confere.
    Valida,
}

impl EstadoDaChave {
    /// A chave pode ser consultada.
    #[must_use]
    pub fn valida(&self) -> bool {
        matches!(self, Self::Valida)
    }

    /// A frase que vai embaixo do campo, se ha o que dizer (chave vazia nao acusa erro).
    #[must_use]
    pub fn mensagem(&self) -> Option<String> {
        match self {
            Self::Vazia | Self::Valida => None,
            Self::Incompleta { faltam } => Some(format!(
                "A chave tem {TAMANHO} posições: {} {faltam}.",
                if *faltam == 1 { "falta" } else { "faltam" }
            )),
            Self::Invalida(m) => Some(m.clone()),
        }
    }
}

fn motivo(e: ErroChave) -> String {
    match e {
        ErroChave::Tamanho => format!("A chave tem {TAMANHO} posições."),
        ErroChave::Formato => {
            "A chave não está no formato da NFS-e Nacional (50 posições; letra só no CNPJ alfanumérico).".into()
        }
        ErroChave::DigitoVerificador => {
            "O dígito verificador não confere: confira se a chave foi copiada inteira e sem troca de posição."
                .into()
        }
        ErroChave::UfInexistente => "Os dois primeiros dígitos não são o código de um estado.".into(),
        ErroChave::AmbienteGerador => {
            "O ambiente gerador da chave deve ser 1 (prefeitura) ou 2 (Sefin Nacional).".into()
        }
        ErroChave::Mes => "O mês da chave (AAMM) é inválido.".into(),
        ErroChave::Emitente => "O emitente da chave não é um CPF nem um CNPJ.".into(),
        _ => "A chave de acesso não confere.".into(),
    }
}

/// Confere a chave digitada (a limpeza ja e' feita aqui): tamanho, formato e digitos verificadores, pela biblioteca.
#[must_use]
pub fn conferir(texto: &str) -> EstadoDaChave {
    let chave = limpar(texto);
    if chave.is_empty() {
        return EstadoDaChave::Vazia;
    }
    if chave.len() < TAMANHO {
        return EstadoDaChave::Incompleta {
            faltam: TAMANHO - chave.len(),
        };
    }
    match chave_dfe::conferir(&chave) {
        Ok(()) => EstadoDaChave::Valida,
        Err(e) => EstadoDaChave::Invalida(motivo(e)),
    }
}

/// As duas chaves sao a mesma (ignorando pontuacao, espacos e caixa). Chave vazia nunca e' igual a nada.
#[must_use]
pub fn sao_a_mesma(a: &str, b: &str) -> bool {
    let (a, b) = (limpar(a), limpar(b));
    !a.is_empty() && a == b
}

#[cfg(test)]
mod testes {
    use super::*;

    fn chave() -> String {
        chave_dfe::montar_nfse(3_550_308, "11222333000181", 42, "2610", Some(1), 2).unwrap()
    }

    #[test]
    fn limpar_aceita_espacos_pontuacao_prefixo_e_minusculas() {
        let c = chave();
        assert_eq!(c.len(), 50);
        let agrupada = em_grupos(&c);
        assert_eq!(agrupada.split(' ').count(), 10);
        assert_eq!(limpar(&agrupada), c);
        assert_eq!(limpar(&format!("NFS{c}")), c);
        assert_eq!(limpar(&format!("  nfs-{}.{} ", &c[..20], &c[20..])), c);
        assert_eq!(limpar(&format!("{c}999999")), c, "so' 50 posicoes");
        assert_eq!(limpar(""), "");
        assert_eq!(limpar("---"), "");
    }

    #[test]
    fn a_mascara_acompanha_a_digitacao() {
        // digitando a chave caractere a caractere, o primeiro digito nao some
        assert_eq!(em_grupos("3"), "3");
        assert_eq!(em_grupos("NFS3"), "3");
        assert_eq!(em_grupos("3550308"), "35503 08");
        assert_eq!(em_grupos("35503 08 12"), "35503 0812");
        assert_eq!(em_grupos(""), "");
    }

    #[test]
    fn conferir_diz_o_que_falta_ou_o_motivo() {
        let c = chave();
        assert_eq!(conferir(""), EstadoDaChave::Vazia);
        assert_eq!(conferir(&c[..30]), EstadoDaChave::Incompleta { faltam: 20 });
        assert_eq!(
            conferir(&c[..30]).mensagem().unwrap(),
            "A chave tem 50 posições: faltam 20."
        );
        assert_eq!(
            conferir(&c[..49]).mensagem().unwrap(),
            "A chave tem 50 posições: falta 1."
        );
        assert!(conferir(&c).valida());
        assert!(conferir(&em_grupos(&c)).valida());
        assert_eq!(conferir(&c).mensagem(), None);
        // um digito trocado: o verificador acusa
        let mut errada = c.clone();
        errada.replace_range(30..31, if &c[30..31] == "1" { "2" } else { "1" });
        let m = conferir(&errada).mensagem().unwrap();
        assert!(m.contains("dígito verificador"), "{m}");
        assert!(!conferir(&errada).valida());
    }

    #[test]
    fn comparar_ignora_a_forma_e_nunca_casa_chave_vazia() {
        let c = chave();
        assert!(sao_a_mesma(&c, &em_grupos(&c)));
        assert!(sao_a_mesma(&c.to_lowercase(), &format!("NFS{c}")));
        let outra =
            chave_dfe::montar_nfse(3_550_308, "11222333000181", 43, "2610", Some(1), 2).unwrap();
        assert!(!sao_a_mesma(&c, &outra));
        assert!(!sao_a_mesma("", ""));
        assert!(!sao_a_mesma(&c, ""));
    }
}
