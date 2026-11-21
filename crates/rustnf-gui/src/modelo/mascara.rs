// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Mascaras e filtros de digitacao: o texto que a tela mostra e o valor limpo que vai para a configuracao.

use rustnf::danfse::municipio;

/// So' os digitos de `texto`, no maximo `limite`.
#[must_use]
pub fn so_digitos(texto: &str, limite: usize) -> String {
    texto
        .chars()
        .filter(char::is_ascii_digit)
        .take(limite)
        .collect()
}

/// Letras e digitos de `texto`, em maiusculas (o CNPJ pode ter letras), no maximo 14.
#[must_use]
pub fn limpar_documento(texto: &str) -> String {
    texto
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_uppercase())
        .take(14)
        .collect()
}

fn com_separadores(limpo: &str, antes_de: &[(usize, char)]) -> String {
    let mut saida = String::with_capacity(limpo.len() + 4);
    for (i, c) in limpo.chars().enumerate() {
        if let Some((_, sep)) = antes_de.iter().find(|(pos, _)| *pos == i) {
            saida.push(*sep);
        }
        saida.push(c);
    }
    saida
}

/// O documento com mascara: ate 11 digitos aparecem como CPF (`999.999.999-99`); a partir do 12o caractere, ou com
/// letra, como CNPJ (`99.999.999/9999-99`).
#[must_use]
pub fn formatar_documento(texto: &str) -> String {
    let limpo = limpar_documento(texto);
    if limpo.len() <= 11 && limpo.bytes().all(|b| b.is_ascii_digit()) {
        com_separadores(&limpo, &[(3, '.'), (6, '.'), (9, '-')])
    } else {
        com_separadores(&limpo, &[(2, '.'), (5, '.'), (8, '/'), (12, '-')])
    }
}

/// O documento de um resumo: CPF mascarado (`***.***.***-NN`) e CNPJ inteiro e formatado. Documento de outro
/// tamanho vira `***`.
#[must_use]
pub fn documento_para_resumo(texto: &str) -> String {
    let limpo = limpar_documento(texto);
    match limpo.len() {
        11 => format!("***.***.***-{}", &limpo[9..]),
        14 => formatar_documento(&limpo),
        _ => "***".into(),
    }
}

/// Mantem so' o que cabe num percentual digitado: digitos e um unico separador decimal (`,` ou `.`).
#[must_use]
pub fn filtrar_decimal(texto: &str) -> String {
    let mut saida = String::new();
    let mut viu_separador = false;
    for c in texto.chars() {
        if c.is_ascii_digit() {
            saida.push(c);
        } else if (c == ',' || c == '.') && !viu_separador {
            viu_separador = true;
            saida.push(c);
        }
        if saida.len() >= 9 {
            break;
        }
    }
    saida
}

/// O que a tela diz ao lado do codigo IBGE do municipio.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RotuloMunicipio {
    /// Nada digitado.
    Vazio,
    /// Menos de 7 digitos.
    Incompleto,
    /// Consta da tabela: nome e UF.
    Encontrado {
        /// Nome em formato de titulo.
        nome: String,
        /// Sigla da UF.
        uf: String,
    },
    /// 7 digitos que nao constam da tabela.
    NaoConsta,
}

impl RotuloMunicipio {
    /// Texto curto para a tela, ou `None` quando nao ha nada a dizer.
    #[must_use]
    pub fn texto(&self) -> Option<String> {
        match self {
            Self::Vazio => None,
            Self::Incompleto => Some("Digite os 7 dígitos do código IBGE.".into()),
            Self::Encontrado { nome, uf } => Some(format!("{nome} - {uf}")),
            Self::NaoConsta => Some("Código não encontrado na tabela de municípios.".into()),
        }
    }
}

/// Procura o municipio pela tabela da biblioteca.
#[must_use]
pub fn rotulo_do_municipio(ibge: &str) -> RotuloMunicipio {
    if ibge.is_empty() {
        return RotuloMunicipio::Vazio;
    }
    if ibge.len() < 7 {
        return RotuloMunicipio::Incompleto;
    }
    match municipio::buscar(ibge) {
        Some((nome, uf)) => RotuloMunicipio::Encontrado {
            nome: nome_proprio(&nome),
            uf,
        },
        None => RotuloMunicipio::NaoConsta,
    }
}

/// `SAO JOSE DO RIO PRETO` -> `Sao Jose do Rio Preto` (preposicoes em minuscula, apos apostrofo em maiuscula).
fn nome_proprio(maiusculas: &str) -> String {
    const MINUSCULAS: [&str; 6] = ["de", "da", "do", "das", "dos", "e"];
    let mut palavras = Vec::new();
    for (i, palavra) in maiusculas.split_whitespace().enumerate() {
        let baixa = palavra.to_lowercase();
        if i > 0 && MINUSCULAS.contains(&baixa.as_str()) {
            palavras.push(baixa);
            continue;
        }
        let mut saida = String::new();
        let mut inicio = true;
        for c in baixa.chars() {
            if inicio {
                saida.extend(c.to_uppercase());
            } else {
                saida.push(c);
            }
            inicio = c == '\'' || c == '-';
        }
        palavras.push(saida);
    }
    palavras.join(" ")
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn cpf_e_cnpj_ganham_mascara_conforme_o_tamanho() {
        assert_eq!(formatar_documento(""), "");
        assert_eq!(formatar_documento("5299"), "529.9");
        assert_eq!(formatar_documento("52998224725"), "529.982.247-25");
        assert_eq!(formatar_documento("529.982.247-25"), "529.982.247-25");
        assert_eq!(formatar_documento("112223330001"), "11.222.333/0001");
        assert_eq!(formatar_documento("11222333000181"), "11.222.333/0001-81");
        assert_eq!(
            formatar_documento("11.222.333/0001-81 sobra"),
            "11.222.333/0001-81"
        );
        // CNPJ alfanumerico: letras entram em maiuscula
        assert_eq!(formatar_documento("12abc345000195"), "12.ABC.345/0001-95");
    }

    #[test]
    fn documento_de_resumo_mascara_o_cpf_e_mostra_o_cnpj() {
        assert_eq!(documento_para_resumo("52998224725"), "***.***.***-25");
        assert_eq!(documento_para_resumo("529.982.247-25"), "***.***.***-25");
        assert_eq!(
            documento_para_resumo("11222333000181"),
            "11.222.333/0001-81"
        );
        assert_eq!(documento_para_resumo("123"), "***");
        assert!(!documento_para_resumo("52998224725").contains("529"));
    }

    #[test]
    fn filtros_de_digitacao() {
        assert_eq!(so_digitos("35-503.08x", 7), "3550308");
        assert_eq!(so_digitos("123456789", 7), "1234567");
        assert_eq!(filtrar_decimal("8,8"), "8,8");
        assert_eq!(filtrar_decimal("8.8.8"), "8.88");
        assert_eq!(filtrar_decimal("abc1,2,3"), "1,23");
        assert_eq!(filtrar_decimal("1234567890123"), "123456789");
    }

    #[test]
    fn municipio_mostra_nome_e_uf_ou_o_que_falta() {
        assert_eq!(rotulo_do_municipio(""), RotuloMunicipio::Vazio);
        assert_eq!(rotulo_do_municipio("3550"), RotuloMunicipio::Incompleto);
        assert_eq!(
            rotulo_do_municipio("4106902"),
            RotuloMunicipio::Encontrado {
                nome: "Curitiba".into(),
                uf: "PR".into()
            }
        );
        assert_eq!(
            rotulo_do_municipio("3550308").texto().as_deref(),
            Some("São Paulo - SP")
        );
        assert_eq!(rotulo_do_municipio("0000000"), RotuloMunicipio::NaoConsta);
        assert_eq!(RotuloMunicipio::Vazio.texto(), None);
    }

    #[test]
    fn nome_proprio_trata_preposicao_e_apostrofo() {
        assert_eq!(
            nome_proprio("SÃO JOSÉ DO RIO PRETO"),
            "São José do Rio Preto"
        );
        assert_eq!(
            nome_proprio("ALTA FLORESTA D'OESTE"),
            "Alta Floresta D'Oeste"
        );
    }
}
