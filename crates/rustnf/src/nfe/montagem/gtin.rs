// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! GTIN (cEAN e cEANTrib): digito verificador GS1 e prefixo da Tabela Prefixo GS1, para as rejeicoes 611, 612, 882
//! e 884 (NT 2021.003 v1.50, que cita a Tabela Prefixo GS1 do Portal Nacional da NF-e).

/// Faixas de prefixo GS1 (3 digitos) que a Tabela Prefixo GS1 do Portal Nacional da NF-e admite, em ordem crescente.
const FAIXAS: [(u32, u32); 127] = [
    (0, 19),
    (20, 29),
    (30, 39),
    (40, 49),
    (50, 59),
    (60, 139),
    (200, 299),
    (300, 379),
    (380, 380),
    (383, 383),
    (385, 385),
    (387, 387),
    (389, 389),
    (400, 440),
    (450, 459),
    (460, 469),
    (470, 470),
    (471, 471),
    (474, 474),
    (475, 475),
    (476, 476),
    (477, 477),
    (478, 478),
    (479, 479),
    (480, 480),
    (481, 481),
    (482, 482),
    (483, 483),
    (484, 484),
    (485, 485),
    (486, 486),
    (487, 487),
    (488, 488),
    (489, 489),
    (490, 499),
    (500, 509),
    (520, 521),
    (528, 528),
    (529, 529),
    (530, 530),
    (531, 531),
    (535, 535),
    (539, 539),
    (540, 549),
    (560, 560),
    (569, 569),
    (570, 579),
    (590, 590),
    (594, 594),
    (599, 599),
    (600, 601),
    (603, 603),
    (604, 604),
    (608, 608),
    (609, 609),
    (611, 611),
    (613, 613),
    (615, 615),
    (616, 616),
    (618, 618),
    (619, 619),
    (620, 620),
    (621, 621),
    (622, 622),
    (623, 623),
    (624, 624),
    (625, 625),
    (626, 626),
    (627, 627),
    (628, 628),
    (629, 629),
    (640, 649),
    (690, 699),
    (700, 709),
    (729, 729),
    (730, 739),
    (740, 740),
    (741, 741),
    (742, 742),
    (743, 743),
    (744, 744),
    (745, 745),
    (746, 746),
    (750, 750),
    (754, 755),
    (759, 759),
    (760, 769),
    (770, 771),
    (773, 773),
    (775, 775),
    (777, 777),
    (778, 779),
    (780, 780),
    (784, 784),
    (786, 786),
    (789, 790),
    (800, 839),
    (840, 849),
    (850, 850),
    (858, 858),
    (859, 859),
    (860, 860),
    (865, 865),
    (867, 867),
    (868, 869),
    (870, 879),
    (880, 880),
    (884, 884),
    (885, 885),
    (888, 888),
    (890, 890),
    (893, 893),
    (896, 896),
    (899, 899),
    (900, 919),
    (930, 939),
    (940, 949),
    (950, 950),
    (951, 951),
    (955, 955),
    (958, 958),
    (960, 969),
    (977, 977),
    (978, 979),
    (980, 980),
    (981, 984),
    (990, 999),
];

/// Digito verificador GS1 (modulo 10, pesos 3 e 1 da direita para a esquerda) do corpo do GTIN, sem o ultimo digito.
pub(crate) fn digito_verificador(numero: &str) -> Result<u32, String> {
    if numero.is_empty() || !numero.bytes().all(|b| b.is_ascii_digit()) {
        return Err("digito verificador so' de digitos".into());
    }
    let (mut soma, mut peso) = (0u32, 3u32);
    for b in numero.bytes().rev() {
        soma += peso * u32::from(b - b'0');
        peso = 4 - peso;
    }
    Ok((10 - soma % 10) % 10)
}

/// O prefixo GS1 do GTIN: 14 posicoes; com 6 zeros na frente (GTIN-8) o prefixo esta nas posicoes 7 a 9, senao nas
/// posicoes 2 a 4. `None` quando nao e' GTIN de 8, 12, 13 ou 14 digitos.
fn prefixo(gtin: &str) -> Option<u32> {
    let g = gtin.trim();
    if !matches!(g.len(), 8 | 12 | 13 | 14) || !g.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let n = format!("{g:0>14}");
    let p = if &n[..6] == "000000" {
        &n[6..9]
    } else {
        &n[1..4]
    };
    p.parse().ok()
}

/// O prefixo cai numa faixa valida da Tabela Prefixo GS1 (regras 882 e 884)?
pub(crate) fn prefixo_valido(gtin: &str) -> bool {
    prefixo(gtin).is_some_and(|p| FAIXAS.iter().any(|&(a, b)| a <= p && p <= b))
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn digito_e_prefixo() {
        // 789 e' a faixa do Brasil (GS1 Brasil); DV do GTIN-13 7891000100103
        assert_eq!(digito_verificador("789100010010"), Ok(3));
        assert!(prefixo_valido("7891000100103"));
        assert!(prefixo_valido("96385074"));
        assert!(!prefixo_valido("1401234567892"));
        assert!(!prefixo_valido("abc"));
        assert_eq!(prefixo("96385074"), Some(963));
        assert!(digito_verificador("12a").is_err());
    }
}
