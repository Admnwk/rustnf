// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O ambiente da Sefin Nacional e o selo que a tela mostra o tempo todo, e o tema da interface.

/// Ambiente da Sefin Nacional (`[sefin].ambiente`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ambiente {
    /// Producao: a nota tem efeito fiscal.
    Producao,
    /// Producao restrita: ambiente de testes, sem efeito fiscal.
    Restrita,
}

/// As cores proprias do selo de ambiente, em RGB `0xRRGGBB`. E' o unico lugar da interface com cor fixa: o resto
/// vem do tema.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoresDoSelo {
    /// Fundo do selo.
    pub fundo: u32,
    /// Texto do selo.
    pub texto: u32,
}

impl Ambiente {
    /// Le o valor de `[sefin].ambiente`. Qualquer texto fora de `producao` conta como restrita, que e' o lado
    /// seguro (a conferencia da configuracao ja recusa o resto).
    #[must_use]
    pub fn de_config(texto: &str) -> Self {
        if texto == "producao" {
            Self::Producao
        } else {
            Self::Restrita
        }
    }

    /// O valor gravado em `[sefin].ambiente`.
    #[must_use]
    pub fn para_config(self) -> &'static str {
        match self {
            Self::Producao => "producao",
            Self::Restrita => "restrita",
        }
    }

    /// O codigo do ambiente no cliente da Sefin: 1 producao, 2 producao restrita.
    #[must_use]
    pub fn codigo(self) -> u8 {
        match self {
            Self::Producao => 1,
            Self::Restrita => 2,
        }
    }

    /// Texto do selo.
    #[must_use]
    pub fn selo(self) -> &'static str {
        match self {
            Self::Producao => "Produção",
            Self::Restrita => "Produção restrita (testes)",
        }
    }

    /// Frase curta sobre o efeito das notas neste ambiente.
    #[must_use]
    pub fn efeito(self) -> &'static str {
        match self {
            Self::Producao => "As notas emitidas têm efeito fiscal.",
            Self::Restrita => "Ambiente de testes: as notas não têm efeito fiscal.",
        }
    }

    /// Cores do selo: vermelho de alerta para producao, verde calmo para a producao restrita. Texto branco nos dois,
    /// com contraste acima de 5:1 sobre o fundo.
    #[must_use]
    pub fn cores(self) -> CoresDoSelo {
        match self {
            Self::Producao => CoresDoSelo {
                fundo: 0x00B4_2318,
                texto: 0x00FF_FFFF,
            },
            Self::Restrita => CoresDoSelo {
                fundo: 0x000F_6B4F,
                texto: 0x00FF_FFFF,
            },
        }
    }
}

/// Preferencia de tema (`[interface].tema`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tema {
    /// Segue o tema do Windows e muda junto com ele.
    Sistema,
    /// Sempre claro.
    Claro,
    /// Sempre escuro.
    Escuro,
}

impl Tema {
    /// Os tres, na ordem em que aparecem na tela.
    pub const TODOS: [Tema; 3] = [Tema::Sistema, Tema::Claro, Tema::Escuro];

    /// Le o valor de `[interface].tema`; texto desconhecido segue o sistema.
    #[must_use]
    pub fn de_config(texto: &str) -> Self {
        match texto {
            "claro" => Self::Claro,
            "escuro" => Self::Escuro,
            _ => Self::Sistema,
        }
    }

    /// O valor gravado em `[interface].tema`.
    #[must_use]
    pub fn para_config(self) -> &'static str {
        match self {
            Self::Sistema => "sistema",
            Self::Claro => "claro",
            Self::Escuro => "escuro",
        }
    }

    /// Rotulo da tela.
    #[must_use]
    pub fn rotulo(self) -> &'static str {
        match self {
            Self::Sistema => "Sistema",
            Self::Claro => "Claro",
            Self::Escuro => "Escuro",
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn selo_diz_producao_ou_producao_restrita_com_cores_diferentes() {
        assert_eq!(Ambiente::de_config("producao").selo(), "Produção");
        assert_eq!(
            Ambiente::de_config("restrita").selo(),
            "Produção restrita (testes)"
        );
        assert_ne!(
            Ambiente::Producao.cores().fundo,
            Ambiente::Restrita.cores().fundo
        );
        // texto desconhecido cai no lado seguro
        assert_eq!(Ambiente::de_config("qualquer"), Ambiente::Restrita);
        assert_eq!(Ambiente::de_config(""), Ambiente::Restrita);
    }

    #[test]
    fn ida_e_volta_do_texto_da_configuracao() {
        for a in [Ambiente::Producao, Ambiente::Restrita] {
            assert_eq!(Ambiente::de_config(a.para_config()), a);
        }
        for t in Tema::TODOS {
            assert_eq!(Tema::de_config(t.para_config()), t);
        }
        assert_eq!(Tema::de_config("azul"), Tema::Sistema);
    }

    /// Contraste WCAG entre duas cores `0xRRGGBB`.
    fn contraste(a: u32, b: u32) -> f64 {
        fn luminancia(c: u32) -> f64 {
            let canal = |v: u32| {
                let s = f64::from(v) / 255.0;
                if s <= 0.039_28 {
                    s / 12.92
                } else {
                    ((s + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * canal((c >> 16) & 0xFF)
                + 0.7152 * canal((c >> 8) & 0xFF)
                + 0.0722 * canal(c & 0xFF)
        }
        let (la, lb) = (luminancia(a), luminancia(b));
        (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
    }

    #[test]
    fn o_texto_do_selo_tem_contraste_de_leitura() {
        for a in [Ambiente::Producao, Ambiente::Restrita] {
            let c = a.cores();
            assert!(contraste(c.fundo, c.texto) >= 5.0, "{a:?}");
        }
    }
}
