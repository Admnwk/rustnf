// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Mascaras e filtros de digitacao: o texto que a tela mostra e o valor limpo que vai para a configuracao.

use rustnf::danfse::municipio;

/// A frase com maiuscula na primeira letra: as mensagens da biblioteca comecam em minuscula, e a tela nao.
#[must_use]
pub fn frase(texto: &str) -> String {
    let mut letras = texto.trim().chars();
    match letras.next() {
        Some(primeira) => primeira.to_uppercase().chain(letras).collect(),
        None => String::new(),
    }
}

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

/// Separa o texto digitado de um valor em dinheiro em (parte inteira, centavos). Os centavos so' existem se houver
/// virgula, ou um unico ponto seguido de 1 ou 2 digitos (`12.5`, colado de outro programa); o ponto de milhar
/// (`1.234`, depois de 3 digitos) e' descartado.
fn partes_do_dinheiro(texto: &str) -> (String, Option<String>) {
    let limpo: String = texto
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == ',' || *c == '.')
        .collect();
    let (inteira, centavos) = if let Some((i, c)) = limpo.split_once(',') {
        (i.to_string(), Some(c.to_string()))
    } else if let Some((i, c)) = limpo.rsplit_once('.') {
        if (1..=2).contains(&c.len()) && !i.is_empty() {
            (i.to_string(), Some(c.to_string()))
        } else {
            (limpo.clone(), None)
        }
    } else {
        (limpo.clone(), None)
    };
    let inteira: String = inteira.chars().filter(char::is_ascii_digit).collect();
    let inteira = inteira.trim_start_matches('0').to_string();
    let centavos = centavos.map(|c| {
        c.chars()
            .filter(char::is_ascii_digit)
            .take(2)
            .collect::<String>()
    });
    (inteira.chars().take(13).collect(), centavos)
}

/// O dinheiro como a tela o mostra enquanto se digita: milhar com ponto e centavos com virgula (`1.234,56`). Ate 13
/// digitos na parte inteira e 2 nos centavos.
#[must_use]
pub fn formatar_dinheiro(texto: &str) -> String {
    let (inteira, centavos) = partes_do_dinheiro(texto);
    if inteira.is_empty() && centavos.is_none() {
        return String::new();
    }
    let inteira = if inteira.is_empty() {
        "0".to_string()
    } else {
        inteira
    };
    let mut agrupada = String::new();
    for (i, c) in inteira.chars().enumerate() {
        if i > 0 && (inteira.len() - i) % 3 == 0 {
            agrupada.push('.');
        }
        agrupada.push(c);
    }
    match centavos {
        Some(c) => format!("{agrupada},{c}"),
        None => agrupada,
    }
}

/// O dinheiro digitado (`1.234,56`) como decimal com ponto (`1234.56`), no formato do pedido. Vazio ou sem digito
/// nenhum: `None`.
#[must_use]
pub fn dinheiro_para_decimal(texto: &str) -> Option<String> {
    let (inteira, centavos) = partes_do_dinheiro(texto);
    let centavos = centavos.filter(|c| !c.is_empty());
    if inteira.is_empty() && centavos.is_none() {
        return None;
    }
    let inteira = if inteira.is_empty() {
        "0".to_string()
    } else {
        inteira
    };
    Some(match centavos {
        Some(c) => format!("{inteira}.{c:0<2}"),
        None => inteira,
    })
}

/// O dinheiro digitado (`1.234,56`) em centavos inteiros (`123456`). Vazio ou invalido: `None`.
#[must_use]
pub fn dinheiro_para_centavos(texto: &str) -> Option<i64> {
    let decimal = dinheiro_para_decimal(texto)?;
    let (inteira, centavos) = decimal.split_once('.').unwrap_or((&decimal, "00"));
    let inteira: i64 = inteira.parse().ok()?;
    let centavos: i64 = centavos.parse().ok()?;
    inteira.checked_mul(100)?.checked_add(centavos)
}

/// Centavos inteiros como a tela mostra o dinheiro, sempre com 2 casas (`-1.234,50`).
#[must_use]
pub fn centavos_para_dinheiro(centavos: i64) -> String {
    let sinal = if centavos < 0 { "-" } else { "" };
    let abs = centavos.unsigned_abs();
    let inteira = formatar_dinheiro(&(abs / 100).to_string());
    let inteira = if inteira.is_empty() {
        "0".into()
    } else {
        inteira
    };
    format!("{sinal}{inteira},{:02}", abs % 100)
}

/// Um decimal com ponto (`1234.56`) como a tela o mostra (`1.234,56`).
#[must_use]
pub fn decimal_para_dinheiro(decimal: &str) -> String {
    formatar_dinheiro(&decimal.replace('.', ","))
}

/// O CEP com mascara (`01310-100`), no maximo 8 digitos.
#[must_use]
pub fn formatar_cep(texto: &str) -> String {
    com_separadores(&so_digitos(texto, 8), &[(5, '-')])
}

/// A data digitada como `DD/MM/AAAA`: so' digitos, barras sozinhas.
#[must_use]
pub fn formatar_data_br(texto: &str) -> String {
    com_separadores(&so_digitos(texto, 8), &[(2, '/'), (4, '/')])
}

/// `DD/MM/AAAA` -> `AAAA-MM-DD`, se for uma data que existe (ano de 2000 a 2099, como a DPS aceita).
#[must_use]
pub fn data_br_para_iso(texto: &str) -> Option<String> {
    let texto = texto.trim();
    if texto.len() != 10 {
        return None;
    }
    let data = chrono::NaiveDate::parse_from_str(texto, "%d/%m/%Y").ok()?;
    if !(2000..=2099).contains(&chrono::Datelike::year(&data)) {
        return None;
    }
    Some(data.format("%Y-%m-%d").to_string())
}

/// `AAAA-MM-DD` -> `DD/MM/AAAA`; texto que nao e' uma data volta como veio.
#[must_use]
pub fn iso_para_data_br(texto: &str) -> String {
    chrono::NaiveDate::parse_from_str(texto.trim(), "%Y-%m-%d")
        .map_or_else(|_| texto.to_string(), |d| d.format("%d/%m/%Y").to_string())
}

/// O telefone com mascara: `(11) 98888-7777` (11 digitos) ou `(11) 8888-7777` (10); menos que isso aparece por
/// partes.
#[must_use]
pub fn formatar_telefone(texto: &str) -> String {
    let d = so_digitos(texto, 11);
    match d.len() {
        0 => String::new(),
        1..=2 => format!("({d}"),
        3..=6 => format!("({}) {}", &d[..2], &d[2..]),
        7..=10 => format!("({}) {}-{}", &d[..2], &d[2..d.len() - 4], &d[d.len() - 4..]),
        _ => format!("({}) {}-{}", &d[..2], &d[2..7], &d[7..]),
    }
}

/// O NBS com mascara (`1.2603.00.00`), no maximo 9 digitos.
#[must_use]
pub fn formatar_nbs(texto: &str) -> String {
    com_separadores(&so_digitos(texto, 9), &[(1, '.'), (5, '.'), (7, '.')])
}

/// O codigo de tributacao nacional com mascara (`14.01.01`), no maximo 6 digitos.
#[must_use]
pub fn formatar_ctribnac(texto: &str) -> String {
    com_separadores(&so_digitos(texto, 6), &[(2, '.'), (4, '.')])
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
    fn frase_ganha_maiuscula_na_primeira_letra() {
        assert_eq!(frase("a competencia nao pode"), "A competencia nao pode");
        assert_eq!(frase("  órgão sem resposta "), "Órgão sem resposta");
        assert_eq!(frase("Já certa."), "Já certa.");
        assert_eq!(frase(""), "");
    }

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
    fn dinheiro_ganha_milhar_e_virgula_enquanto_se_digita() {
        assert_eq!(formatar_dinheiro(""), "");
        assert_eq!(formatar_dinheiro("0"), "");
        assert_eq!(formatar_dinheiro("7"), "7");
        assert_eq!(formatar_dinheiro("1234"), "1.234");
        assert_eq!(formatar_dinheiro("1234567"), "1.234.567");
        assert_eq!(formatar_dinheiro("1234,5"), "1.234,5");
        assert_eq!(formatar_dinheiro("1234,567"), "1.234,56");
        assert_eq!(formatar_dinheiro(","), "0,");
        assert_eq!(formatar_dinheiro(",5"), "0,5");
        assert_eq!(formatar_dinheiro("00012"), "12");
        assert_eq!(formatar_dinheiro("R$ 1.234,56abc"), "1.234,56");
        // o que a mascara devolve a si mesma nao muda
        for t in ["1.234", "1.234,56", "0,5", "12", "999.999.999,99"] {
            assert_eq!(formatar_dinheiro(t), t);
        }
    }

    #[test]
    fn ponto_com_um_ou_dois_digitos_no_fim_e_decimal_e_com_tres_e_milhar() {
        assert_eq!(formatar_dinheiro("12.5"), "12,5");
        assert_eq!(formatar_dinheiro("1234.56"), "1.234,56");
        assert_eq!(formatar_dinheiro("1.234"), "1.234");
        assert_eq!(formatar_dinheiro("1.234.567"), "1.234.567");
    }

    #[test]
    fn dinheiro_vira_decimal_com_ponto_e_volta() {
        assert_eq!(
            dinheiro_para_decimal("1.234,56").as_deref(),
            Some("1234.56")
        );
        assert_eq!(dinheiro_para_decimal("150").as_deref(), Some("150"));
        assert_eq!(dinheiro_para_decimal("150,5").as_deref(), Some("150.50"));
        assert_eq!(dinheiro_para_decimal("0,5").as_deref(), Some("0.50"));
        assert_eq!(dinheiro_para_decimal("150,").as_deref(), Some("150"));
        assert_eq!(dinheiro_para_decimal(""), None);
        assert_eq!(dinheiro_para_decimal("abc"), None);
        assert_eq!(dinheiro_para_decimal(","), None);
        assert_eq!(decimal_para_dinheiro("1234.56"), "1.234,56");
        assert_eq!(decimal_para_dinheiro("150"), "150");
        assert_eq!(decimal_para_dinheiro("0.50"), "0,50");
    }

    #[test]
    fn centavos_inteiros_ida_e_volta() {
        assert_eq!(dinheiro_para_centavos("1.234,56"), Some(123_456));
        assert_eq!(dinheiro_para_centavos("150"), Some(15_000));
        assert_eq!(dinheiro_para_centavos("0,5"), Some(50));
        assert_eq!(dinheiro_para_centavos(""), None);
        assert_eq!(centavos_para_dinheiro(123_456), "1.234,56");
        assert_eq!(centavos_para_dinheiro(5), "0,05");
        assert_eq!(centavos_para_dinheiro(0), "0,00");
        assert_eq!(centavos_para_dinheiro(-150_050), "-1.500,50");
    }

    #[test]
    fn cep_data_telefone_nbs_e_codigo_de_servico_ganham_mascara() {
        assert_eq!(formatar_cep("01310100"), "01310-100");
        assert_eq!(formatar_cep("0131"), "0131");
        assert_eq!(formatar_cep("01310-100123"), "01310-100");
        assert_eq!(formatar_data_br("21112026"), "21/11/2026");
        assert_eq!(formatar_data_br("2111"), "21/11");
        assert_eq!(formatar_data_br("21/11/2026"), "21/11/2026");
        assert_eq!(formatar_telefone("11988887777"), "(11) 98888-7777");
        assert_eq!(formatar_telefone("1188887777"), "(11) 8888-7777");
        assert_eq!(formatar_telefone("119"), "(11) 9");
        assert_eq!(formatar_telefone("1"), "(1");
        assert_eq!(formatar_telefone(""), "");
        assert_eq!(formatar_nbs("126030000"), "1.2603.00.00");
        assert_eq!(formatar_nbs("1.2603"), "1.2603");
        assert_eq!(formatar_ctribnac("140101"), "14.01.01");
        assert_eq!(formatar_ctribnac("1401"), "14.01");
    }

    #[test]
    fn data_br_e_iso_conferem_a_existencia_da_data() {
        assert_eq!(
            data_br_para_iso("21/11/2026").as_deref(),
            Some("2026-11-21")
        );
        assert_eq!(
            data_br_para_iso("29/02/2028").as_deref(),
            Some("2028-02-29")
        );
        assert_eq!(data_br_para_iso("29/02/2027"), None);
        assert_eq!(data_br_para_iso("31/04/2026"), None);
        assert_eq!(data_br_para_iso("21/11"), None);
        assert_eq!(data_br_para_iso("1/1/2026"), None);
        assert_eq!(data_br_para_iso("21/11/1999"), None);
        assert_eq!(iso_para_data_br("2026-11-21"), "21/11/2026");
        assert_eq!(iso_para_data_br("lixo"), "lixo");
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
