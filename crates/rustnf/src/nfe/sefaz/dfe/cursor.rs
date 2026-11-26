// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O NSU e o cursor da distribuicao: o que a aplicacao guarda entre uma chamada e outra.
//!
//! O Ambiente Nacional numera, por CNPJ ou CPF, tudo o que distribui (NSU, 15 digitos) e exige que o interessado
//! siga a sequencia: pedir sempre a partir do `ultNSU` que ele mesmo devolveu e esperar uma hora quando a fila
//! acaba, sob pena da rejeicao 656 e do bloqueio do CNPJ por uma hora (NT 2014.002 v1.02d, item 3.11.4). O [`Cursor`]
//! carrega esse estado: a biblioteca DECIDE o proximo NSU e quando esperar, e a aplicacao so' PERSISTE o cursor (em
//! texto, com [`Cursor::para_json`]) na mesma gravacao dos documentos do lote.

use chrono::{DateTime, Duration, Utc};
use serde_json::{json, Value};

use super::super::erro::ErroSefaz;

/// Quanto se espera depois do 137, do `ultNSU` igual ao `maxNSU` e do 656: uma hora (NT 2014.002 v1.02d, itens 3.5 e
/// 3.11.4).
pub const ESPERA: Duration = Duration::hours(1);

/// O numero sequencial unico da distribuicao: de 1 a 15 digitos, escrito sempre com 15 (`000000000000042`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Nsu(u64);

impl Nsu {
    /// O NSU zero: "ainda nao recebi nada".
    pub const ZERO: Nsu = Nsu(0);

    /// Maior NSU que cabe nos 15 digitos do leiaute.
    const MAXIMO: u64 = 999_999_999_999_999;

    /// Le o NSU: de 1 a 15 digitos, so' algarismos.
    ///
    /// # Errors
    ///
    /// [`ErroSefaz`] (campo `NSU`) se vier vazio, comprido demais ou com algo que nao e' algarismo.
    pub fn ler(texto: &str) -> Result<Nsu, ErroSefaz> {
        let s = texto.trim();
        if s.is_empty() || s.len() > 15 || !s.bytes().all(|b| b.is_ascii_digit()) {
            let cortado: String = s.chars().take(20).collect();
            return Err(ErroSefaz::novo(
                format!("NSU '{cortado}' invalido: de 1 a 15 digitos"),
                "NSU",
            ));
        }
        s.parse::<u64>()
            .map(Nsu)
            .map_err(|_| ErroSefaz::novo("NSU invalido: de 1 a 15 digitos", "NSU"))
    }

    /// O valor numerico.
    #[must_use]
    pub fn valor(self) -> u64 {
        self.0
    }

    /// O NSU seguinte (para no maximo do leiaute).
    #[must_use]
    pub fn proximo(self) -> Nsu {
        Nsu((self.0 + 1).min(Self::MAXIMO))
    }

    /// O NSU anterior (para em zero).
    #[must_use]
    pub fn anterior(self) -> Nsu {
        Nsu(self.0.saturating_sub(1))
    }
}

impl std::fmt::Display for Nsu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:015}", self.0)
    }
}

/// Uma faixa de NSU, com as duas pontas dentro.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FaixaNsu {
    /// O primeiro NSU da faixa.
    pub inicio: Nsu,
    /// O ultimo NSU da faixa.
    pub fim: Nsu,
}

impl FaixaNsu {
    /// Quantos NSU a faixa tem.
    #[must_use]
    pub fn tamanho(&self) -> u64 {
        self.fim.valor().saturating_sub(self.inicio.valor()) + 1
    }
}

/// Por que a aplicacao tem de esperar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MotivoEspera {
    /// A fila acabou (137, ou `ultNSU` igual ao `maxNSU`): nova distribuicao so' depois de uma hora. A consulta de um
    /// NSU ou de uma chave nao e' a distribuicao e continua liberada.
    FilaEmDia,
    /// 656: o CNPJ ou CPF esta bloqueado por uma hora para QUALQUER consulta.
    ConsumoIndevido,
}

/// A espera que o Ambiente Nacional impos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Espera {
    /// So' depois deste instante se pode chamar de novo.
    pub ate: DateTime<Utc>,
    /// Por que.
    pub motivo: MotivoEspera,
}

/// O estado da distribuicao de UM interessado (CNPJ ou CPF) em UM ambiente. Todas as aplicacoes do mesmo interessado
/// tem de usar o MESMO cursor (NT 2014.002 v1.02d, item 3.11.4).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Cursor {
    /// O ultimo NSU que o Ambiente Nacional devolveu. So' anda para a frente, e so' para o que ele devolveu.
    pub ult_nsu: Nsu,
    /// O maior NSU que o Ambiente Nacional disse existir, se ja disse.
    pub max_nsu: Option<Nsu>,
    /// A espera em vigor, se houver.
    pub espera: Option<Espera>,
    /// NSU que faltaram no meio dos lotes: a biblioteca os busca um a um (`consNSU`) e os tira daqui.
    pub lacunas: Vec<FaixaNsu>,
}

impl Cursor {
    /// O cursor de quem nunca consultou.
    #[must_use]
    pub fn novo() -> Cursor {
        Cursor::default()
    }

    /// A espera que vale AGORA para a consulta pedida: a de consumo indevido vale para todas; a da fila em dia, so' para
    /// a distribuicao a partir do ultimo NSU.
    #[must_use]
    pub fn espera_para(&self, distribuicao: bool, agora: DateTime<Utc>) -> Option<Espera> {
        self.espera
            .filter(|e| e.ate > agora)
            .filter(|e| distribuicao || e.motivo == MotivoEspera::ConsumoIndevido)
    }

    /// Ha NSU a buscar por `consNSU`.
    #[must_use]
    pub fn tem_lacunas(&self) -> bool {
        !self.lacunas.is_empty()
    }

    /// O primeiro NSU que falta, se algum.
    #[must_use]
    pub fn proxima_lacuna(&self) -> Option<Nsu> {
        self.lacunas.first().map(|f| f.inicio)
    }

    /// Tira o NSU das lacunas (achado, ou o Ambiente Nacional disse que ele nao existe). NSU que nao era lacuna nao muda
    /// nada.
    pub fn resolver(&mut self, nsu: Nsu) {
        let mut novas = Vec::with_capacity(self.lacunas.len() + 1);
        for f in self.lacunas.drain(..) {
            if nsu < f.inicio || nsu > f.fim {
                novas.push(f);
                continue;
            }
            if nsu > f.inicio {
                novas.push(FaixaNsu {
                    inicio: f.inicio,
                    fim: nsu.anterior(),
                });
            }
            if nsu < f.fim {
                novas.push(FaixaNsu {
                    inicio: nsu.proximo(),
                    fim: f.fim,
                });
            }
        }
        self.lacunas = novas;
    }

    /// O cursor em texto JSON, para a aplicacao guardar.
    #[must_use]
    pub fn para_json(&self) -> String {
        let motivo = |m: MotivoEspera| match m {
            MotivoEspera::FilaEmDia => "fila_em_dia",
            MotivoEspera::ConsumoIndevido => "consumo_indevido",
        };
        json!({
            "ult_nsu": self.ult_nsu.to_string(),
            "max_nsu": self.max_nsu.map(|n| n.to_string()),
            "espera": self.espera.map(|e| json!({
                "ate": e.ate.to_rfc3339(),
                "motivo": motivo(e.motivo),
            })),
            "lacunas": self.lacunas.iter()
                .map(|f| json!([f.inicio.to_string(), f.fim.to_string()]))
                .collect::<Vec<_>>(),
        })
        .to_string()
    }

    /// Le o cursor que [`Cursor::para_json`] escreveu. Texto vazio e' o cursor de quem nunca consultou.
    ///
    /// # Errors
    ///
    /// [`ErroSefaz`] se o texto nao e' o cursor: NSU torto (campo `NSU`), data da espera torta (campo `espera`),
    /// lacuna invertida ou JSON que nao e' deste cursor. Cursor torto NAO vira "recomece do zero": recomecar e' o
    /// caminho do bloqueio 656.
    pub fn de_json(texto: &str) -> Result<Cursor, ErroSefaz> {
        if texto.trim().is_empty() {
            return Ok(Cursor::novo());
        }
        let v: Value = serde_json::from_str(texto)
            .map_err(|_| ErroSefaz::novo("cursor da distribuicao: JSON ilegivel", "cursor"))?;
        let nsu_de = |x: Option<&Value>| -> Result<Option<Nsu>, ErroSefaz> {
            match x {
                None | Some(Value::Null) => Ok(None),
                Some(Value::String(s)) => Nsu::ler(s).map(Some),
                Some(_) => Err(ErroSefaz::novo("cursor: NSU deve ser texto", "NSU")),
            }
        };
        let espera = match v.get("espera") {
            None | Some(Value::Null) => None,
            Some(e) => {
                let ate = e
                    .get("ate")
                    .and_then(Value::as_str)
                    .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                    .map(|d| d.with_timezone(&Utc))
                    .ok_or_else(|| {
                        ErroSefaz::novo("cursor: data da espera em ISO 8601 com fuso", "espera")
                    })?;
                let motivo = match e.get("motivo").and_then(Value::as_str) {
                    Some("fila_em_dia") => MotivoEspera::FilaEmDia,
                    Some("consumo_indevido") => MotivoEspera::ConsumoIndevido,
                    _ => {
                        return Err(ErroSefaz::novo(
                            "cursor: motivo da espera desconhecido",
                            "espera",
                        ))
                    }
                };
                Some(Espera { ate, motivo })
            }
        };
        let mut lacunas = Vec::new();
        if let Some(Value::Array(fs)) = v.get("lacunas") {
            for f in fs {
                let Some(par) = f.as_array().filter(|p| p.len() == 2) else {
                    return Err(ErroSefaz::novo(
                        "cursor: lacuna deve ser [inicio, fim]",
                        "NSU",
                    ));
                };
                let (Some(i), Some(f2)) = (nsu_de(par.first())?, nsu_de(par.get(1))?) else {
                    return Err(ErroSefaz::novo("cursor: lacuna sem NSU", "NSU"));
                };
                if i > f2 {
                    return Err(ErroSefaz::novo(
                        "cursor: lacuna com o inicio depois do fim",
                        "NSU",
                    ));
                }
                lacunas.push(FaixaNsu { inicio: i, fim: f2 });
            }
        }
        Ok(Cursor {
            ult_nsu: nsu_de(v.get("ult_nsu"))?.unwrap_or(Nsu::ZERO),
            max_nsu: nsu_de(v.get("max_nsu"))?,
            espera,
            lacunas,
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod testes {
    use super::*;

    fn faixa(a: u64, b: u64) -> FaixaNsu {
        FaixaNsu {
            inicio: Nsu(a),
            fim: Nsu(b),
        }
    }

    #[test]
    fn nsu_de_1_a_15_digitos_e_escreve_com_15() {
        assert_eq!(Nsu::ler(" 42 ").unwrap().to_string(), "000000000000042");
        assert_eq!(
            Nsu::ler("999999999999999").unwrap().valor(),
            999_999_999_999_999
        );
        for ruim in ["", "1234567890123456", "12a", "-1", "1.5", "+1"] {
            assert_eq!(Nsu::ler(ruim).unwrap_err().campo, "NSU", "{ruim}");
        }
    }

    #[test]
    fn resolver_parte_a_faixa_sem_perder_nsu() {
        let mut c = Cursor::novo();
        c.lacunas = vec![faixa(5, 9)];
        c.resolver(Nsu(7));
        assert_eq!(c.lacunas, vec![faixa(5, 6), faixa(8, 9)]);
        c.resolver(Nsu(5));
        c.resolver(Nsu(9));
        assert_eq!(c.lacunas, vec![faixa(6, 6), faixa(8, 8)]);
        c.resolver(Nsu(6));
        c.resolver(Nsu(8));
        c.resolver(Nsu(8));
        assert!(!c.tem_lacunas());
    }

    #[test]
    fn a_espera_da_fila_nao_barra_consulta_pontual_mas_a_do_656_barra() {
        let agora = Utc::now();
        let ate = agora + ESPERA;
        let mut c = Cursor::novo();
        c.espera = Some(Espera {
            ate,
            motivo: MotivoEspera::FilaEmDia,
        });
        assert!(c.espera_para(true, agora).is_some());
        assert!(c.espera_para(false, agora).is_none());
        c.espera = Some(Espera {
            ate,
            motivo: MotivoEspera::ConsumoIndevido,
        });
        assert!(c.espera_para(false, agora).is_some());
        // vencida nao conta
        assert!(c.espera_para(true, ate).is_none());
    }

    #[test]
    fn json_vai_e_volta() {
        let c = Cursor {
            ult_nsu: Nsu(77),
            max_nsu: Some(Nsu(90)),
            espera: Some(Espera {
                ate: Utc::now(),
                motivo: MotivoEspera::ConsumoIndevido,
            }),
            lacunas: vec![faixa(3, 4)],
        };
        let volta = Cursor::de_json(&c.para_json()).unwrap();
        assert_eq!(volta.ult_nsu, c.ult_nsu);
        assert_eq!(volta.max_nsu, c.max_nsu);
        assert_eq!(volta.lacunas, c.lacunas);
        assert_eq!(volta.espera.unwrap().motivo, MotivoEspera::ConsumoIndevido);
        assert_eq!(Cursor::de_json("").unwrap(), Cursor::novo());
        assert!(Cursor::de_json("{\"ult_nsu\":\"abc\"}").is_err());
        assert!(Cursor::de_json("{\"ult_nsu\":7}").is_err());
        assert!(Cursor::de_json("{\"lacunas\":[[\"9\",\"3\"]]}").is_err());
        assert!(
            Cursor::de_json("{\"espera\":{\"ate\":\"ontem\",\"motivo\":\"fila_em_dia\"}}").is_err()
        );
    }
}
