// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Calendario da transicao 2026-2033 e as aliquotas de IBS/CBS do FATO GERADOR .
//!
//! 2026: IBS UF 0,1% (LC 214/2025 art. 343, "aliquota ESTADUAL"), IBS Mun 0%, CBS 0,9% (art. 346); Simples nao
//! aplica (art. 348, III, c). 2027-2028: IBS 0,05% + 0,05% FIXADO EM LEI (art. 344); CBS = referencia do Senado
//! menos 0,1 p.p. (art. 347). 2029+: referencias do Senado (art. 349). Sem numero publicado: `confirmado=false`
//! com motivo -- nunca zero calado.

use std::collections::BTreeMap;

use serde_json::{json, Value};

use super::{ano_de, data_iso, escala, fmt_norm4, ErroReforma, Resultado};

pub const ANO_INICIO: i32 = 2026;
pub const FONTE_2026: &str =
    "LC 214/2025 art. 343 (IBS estadual 0,1%), art. 346 (CBS 0,9%) e art. 348; NT 2025.002 UB18-10/UB37-10/UB56-10";
pub const FONTE_2027: &str =
    "LC 214/2025 art. 344 (IBS 0,05% estadual + 0,05% municipal) e art. 347 (CBS = aliquota \
     do art. 14 reduzida em 0,1 ponto percentual); NT 2025.002 UB18-10/UB37-10/UB56-20";
pub const FONTE_SENADO: &str = "LC 214/2025 art. 349 caput e par. 1o II (resolucao do Senado fixa as aliquotas de \
     referencia ate 31/10 do ano anterior; para 2027 prazo prorrogado em 45 dias, art. 353 par. 2o; depois de 22/12, as do \
     TCU, par. 2o)";
/// 0,1 ponto percentual (x10000), art. 347.
pub const REDUTOR_CBS_2027_2028: i64 = 1_000;

/// O que o Senado/ente publicou (texto decimal). 2027-2028: `cbs_referencia` (o crate tira 0,1 p.p.) ou `cbs`;
/// 2029+: `cbs`, `ibs_uf`, `ibs_mun`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Publicada {
    pub cbs: Option<String>,
    pub cbs_referencia: Option<String>,
    pub ibs_uf: Option<String>,
    pub ibs_mun: Option<String>,
    pub fonte: Option<String>,
    /// Vigencia DENTRO do ano: a partir de `inicio` os campos da troca substituem os da base.
    pub trocas: Vec<Troca>,
}

/// Uma troca de aliquota no meio do ano (texto decimal; `inicio` AAAA-MM-DD, inclusive).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Troca {
    pub inicio: String,
    pub cbs: Option<String>,
    pub cbs_referencia: Option<String>,
    pub ibs_uf: Option<String>,
    pub ibs_mun: Option<String>,
    pub fonte: Option<String>,
}

pub type Publicadas = BTreeMap<i32, Publicada>;

/// O que vale no `dia` (base + trocas ja vigentes, em ordem de data) e a proxima troca ainda nao vigente ("").
pub fn vigente(p: &Publicada, ano: i32, dia: &str) -> Resultado<(Publicada, String)> {
    let mut ordenadas: Vec<(String, &Troca)> = Vec::new();
    for t in &p.trocas {
        let ini = t.inicio.trim();
        let iso = if ini.len() == 10 {
            data_iso(ini).unwrap_or_default()
        } else {
            String::new()
        };
        if iso.is_empty() {
            return Err(ErroReforma::novo(
                format!("troca de aliquota de {ano} sem 'inicio' AAAA-MM-DD valido ('{ini}')"),
                "vigencia_invalida",
            ));
        }
        if iso[..4].parse::<i32>().ok() != Some(ano) {
            return Err(ErroReforma::novo(
                format!("troca com inicio {iso} cadastrada no ano {ano}: cadastre no ano dela"),
                "vigencia_fora_do_ano",
            ));
        }
        if ordenadas.iter().any(|(i, _)| *i == iso) {
            return Err(ErroReforma::novo(
                format!("duas trocas de aliquota de {ano} com o mesmo inicio {iso}"),
                "vigencia_repetida",
            ));
        }
        ordenadas.push((iso, t));
    }
    ordenadas.sort_by(|a, b| a.0.cmp(&b.0));
    let mut base = Publicada {
        trocas: Vec::new(),
        ..p.clone()
    };
    let mut proxima = String::new();
    for (ini, t) in ordenadas {
        if ini.as_str() > dia {
            if proxima.is_empty() {
                proxima = ini;
            }
            continue;
        }
        if t.cbs.is_some() {
            base.cbs.clone_from(&t.cbs);
            base.cbs_referencia = None;
        }
        if t.cbs_referencia.is_some() {
            base.cbs_referencia.clone_from(&t.cbs_referencia);
            base.cbs = None;
        }
        if t.ibs_uf.is_some() {
            base.ibs_uf.clone_from(&t.ibs_uf);
        }
        if t.ibs_mun.is_some() {
            base.ibs_mun.clone_from(&t.ibs_mun);
        }
        if t.fonte.as_deref().is_some_and(|f| !f.trim().is_empty()) {
            base.fonte.clone_from(&t.fonte);
        }
    }
    Ok((base, proxima))
}

fn txt(v: &Value) -> Option<String> {
    match v {
        Value::Null => None,
        Value::String(s) => Some(s.clone()),
        o => Some(o.to_string()),
    }
}

/// `{"2027": {"cbs_referencia": "8.8"}, ...}` -> [`Publicadas`].
pub fn publicadas_de_json(v: &Value) -> Publicadas {
    let mut out = Publicadas::new();
    if let Some(m) = v.as_object() {
        for (ano, p) in m {
            if let Ok(a) = ano.parse::<i32>() {
                out.insert(
                    a,
                    Publicada {
                        cbs: p.get("cbs").and_then(txt),
                        cbs_referencia: p.get("cbs_referencia").and_then(txt),
                        ibs_uf: p.get("ibs_uf").and_then(txt),
                        ibs_mun: p.get("ibs_mun").and_then(txt),
                        fonte: p.get("fonte").and_then(txt),
                        trocas: p
                            .get("trocas")
                            .and_then(Value::as_array)
                            .map(|l| {
                                l.iter()
                                    .map(|t| Troca {
                                        inicio: t.get("inicio").and_then(txt).unwrap_or_default(),
                                        cbs: t.get("cbs").and_then(txt),
                                        cbs_referencia: t.get("cbs_referencia").and_then(txt),
                                        ibs_uf: t.get("ibs_uf").and_then(txt),
                                        ibs_mun: t.get("ibs_mun").and_then(txt),
                                        fonte: t.get("fonte").and_then(txt),
                                    })
                                    .collect()
                            })
                            .unwrap_or_default(),
                    },
                );
            }
        }
    }
    out
}

/// Aliquotas (percentual x10000) do IBS UF, IBS municipal e CBS no ano do fato gerador.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aliquotas {
    pub ano: i32,
    pub ibs_uf: i64,
    pub ibs_mun: i64,
    pub cbs: i64,
    pub confirmado: bool,
    pub aplica: bool,
    pub motivo: String,
    pub fonte: String,
    pub avisos: Vec<String>,
}

impl Aliquotas {
    pub fn ibs(&self) -> i64 {
        self.ibs_uf + self.ibs_mun
    }

    pub fn total(&self) -> i64 {
        self.ibs_uf + self.ibs_mun + self.cbs
    }

    /// Representacao em JSON (usada pelo golden).
    pub fn para_json(&self) -> Value {
        json!({"ano": self.ano, "ibs_uf": fmt_norm4(self.ibs_uf), "ibs_mun": fmt_norm4(self.ibs_mun),
               "cbs": fmt_norm4(self.cbs), "confirmado": self.confirmado, "aplica": self.aplica,
               "motivo": self.motivo})
    }
}

fn p4(s: &str, campo: &str) -> Resultado<i64> {
    Ok(escala(s, 4, campo)? as i64)
}

/// O que o crate embarca de numero publicado (resolucao do Senado / TCU), com `fonte` OBRIGATORIA (ato + data do
/// DOU). Vazio = nada publicado (PRS 50/2026 na CAE ao escrever esta tabela; TCU acordao 2.933/2026 = 11,3468% so'
/// vale se nao sair resolucao ate 22/12, art. 349 par. 2o). Golden: `tests/casos/reforma_embarcadas.json`.
pub fn embarcadas() -> Publicadas {
    Publicadas::new()
}

/// As aliquotas que valem na `data` (do fato gerador). Ver o modulo para a regra de cada ano. Ano que o integrador
/// nao informa cai no que o crate embarcou ([`embarcadas`]); ano informado vale o do integrador, com aviso se diverge.
pub fn aliquotas(
    data: &str,
    publicadas: Option<&Publicadas>,
    simples_nacional: bool,
) -> Resultado<Aliquotas> {
    aliquotas_com(data, publicadas, &embarcadas(), simples_nacional)
}

fn txt_aliq(a: &Aliquotas) -> String {
    format!(
        "IBS UF {}%, IBS Mun {}%, CBS {}%",
        fmt_norm4(a.ibs_uf),
        fmt_norm4(a.ibs_mun),
        fmt_norm4(a.cbs)
    )
}

/// [`aliquotas`] com a tabela embarcada trocada (teste e golden).
pub fn aliquotas_com(
    data: &str,
    publicadas: Option<&Publicadas>,
    embarcadas: &Publicadas,
    simples_nacional: bool,
) -> Resultado<Aliquotas> {
    let ano = ano_de(data)?;
    let Some(e_ano) = embarcadas.get(&ano) else {
        return aliquotas_base(data, publicadas, simples_nacional);
    };
    let so_emb: Publicadas = [(ano, e_ano.clone())].into_iter().collect();
    if publicadas.and_then(|m| m.get(&ano)).is_none() {
        return aliquotas_base(data, Some(&so_emb), simples_nacional);
    }
    let mut a = aliquotas_base(data, publicadas, simples_nacional)?;
    let b = aliquotas_base(data, Some(&so_emb), simples_nacional)?;
    if b.confirmado
        && (!a.confirmado || (a.ibs_uf, a.ibs_mun, a.cbs) != (b.ibs_uf, b.ibs_mun, b.cbs))
    {
        let info = if a.confirmado {
            txt_aliq(&a)
        } else {
            "sem numero vigente na data".to_string()
        };
        a.avisos.push(format!(
            "aliquota informada para {ano} ({info}) difere da publicada que a lib embarcou ({}: {}) -- vale a informada; \
             confira se e' lei propria do ente ou cadastro antigo",
            txt_aliq(&b),
            b.fonte
        ));
    }
    Ok(a)
}

fn aliquotas_base(
    data: &str,
    publicadas: Option<&Publicadas>,
    simples_nacional: bool,
) -> Resultado<Aliquotas> {
    let ano = ano_de(data)?;
    let mk = |ibs_uf,
              ibs_mun,
              cbs,
              confirmado,
              aplica,
              motivo: String,
              fonte: String,
              avisos: Vec<String>| Aliquotas {
        ano,
        ibs_uf,
        ibs_mun,
        cbs,
        confirmado,
        aplica,
        motivo,
        fonte,
        avisos,
    };
    if ano < ANO_INICIO {
        return Ok(mk(
            0,
            0,
            0,
            true,
            false,
            format!("em {ano} o IBS e a CBS ainda nao existem (comecam em 2026: LC 214/2025 arts. 343 e 346)"),
            FONTE_2026.into(),
            vec![],
        ));
    }
    let (p, proxima) = vigente(
        &publicadas
            .and_then(|m| m.get(&ano))
            .cloned()
            .unwrap_or_default(),
        ano,
        &data_iso(data)?,
    )?;
    let fonte_pub = p.fonte.clone().unwrap_or_default().trim().to_string();
    let depois = if proxima.is_empty() {
        String::new()
    } else {
        format!(" (ha vigencia cadastrada a partir de {proxima})")
    };
    if ano == 2026 {
        if simples_nacional {
            return Ok(mk(
                0,
                0,
                0,
                true,
                false,
                "as aliquotas de 2026 nao se aplicam as operacoes do optante do Simples Nacional (LC 214/2025 art. \
                 348, III, c)"
                    .into(),
                FONTE_2026.into(),
                vec![],
            ));
        }
        return Ok(mk(
            1_000,
            0,
            9_000,
            true,
            true,
            String::new(),
            FONTE_2026.into(),
            vec![],
        ));
    }
    let avisos: Vec<String> = if simples_nacional {
        vec!["optante do Simples sem a opcao do art. 41 par. 3o segue as regras do Simples (LC 214/2025 art. 41 par. 2o: \
              IBS e CBS dentro do DAS, LC 123/2006); o adquirente credita o valor pago no DAS (art. 47 par. 9o, II)"
            .into()]
    } else {
        vec![]
    };
    if ano == 2027 || ano == 2028 {
        let (ibs_uf, ibs_mun) = (500, 500);
        for (k, v, lei) in [
            ("ibs_uf", &p.ibs_uf, ibs_uf),
            ("ibs_mun", &p.ibs_mun, ibs_mun),
        ] {
            if let Some(s) = v {
                let x = p4(s, k)?;
                if x != lei {
                    return Err(ErroReforma::novo(
                        format!(
                            "{k} de {ano} e' fixado em LEI ({}%, LC 214/2025 art. 344) -- nao se cadastra outro valor ({}%)",
                            fmt_norm4(lei),
                            fmt_norm4(x)
                        ),
                        "fixado_em_lei",
                    ));
                }
            }
        }
        let cbs = if let Some(s) = &p.cbs {
            p4(s, "cbs")?
        } else if let Some(s) = &p.cbs_referencia {
            p4(s, "cbs_referencia")? - REDUTOR_CBS_2027_2028
        } else {
            return Ok(mk(
                ibs_uf,
                ibs_mun,
                0,
                false,
                true,
                format!(
                    "a CBS de {ano} e' a aliquota de referencia fixada pelo Senado menos 0,1 p.p. (LC 214/2025 arts. \
                     347 e 349) e ela nao foi informada{depois} -- cadastre o numero publicado antes de emitir ou apurar"
                ),
                format!("{FONTE_2027}; {FONTE_SENADO}"),
                avisos,
            ));
        };
        if cbs < 0 {
            return Err(ErroReforma::novo(
                format!(
                    "CBS de {ano} negativa ({}%): confira a aliquota de referencia informada",
                    fmt_norm4(cbs)
                ),
                "negativa",
            ));
        }
        let fonte = if fonte_pub.is_empty() {
            FONTE_2027.to_string()
        } else {
            format!("{FONTE_2027}; CBS: {fonte_pub}")
        };
        return Ok(mk(
            ibs_uf,
            ibs_mun,
            cbs,
            true,
            true,
            String::new(),
            fonte,
            avisos,
        ));
    }
    let falta: Vec<&str> = [
        ("cbs", &p.cbs),
        ("ibs_uf", &p.ibs_uf),
        ("ibs_mun", &p.ibs_mun),
    ]
    .iter()
    .filter(|(_, v)| v.is_none())
    .map(|(k, _)| *k)
    .collect();
    let val = |v: &Option<String>, k: &str| -> Resultado<i64> {
        v.as_deref().map_or(Ok(0), |s| p4(s, k))
    };
    let (cbs, ibs_uf, ibs_mun) = (
        val(&p.cbs, "cbs")?,
        val(&p.ibs_uf, "ibs_uf")?,
        val(&p.ibs_mun, "ibs_mun")?,
    );
    if !falta.is_empty() {
        return Ok(mk(
            ibs_uf,
            ibs_mun,
            cbs,
            false,
            true,
            format!(
                "as aliquotas de {ano} ({}) sao as de referencia fixadas pelo Senado (ou as proprias do ente) e nao \
                 foram informadas{depois} -- cadastre o numero publicado antes de emitir ou apurar",
                falta.join(", ")
            ),
            FONTE_SENADO.into(),
            avisos,
        ));
    }
    if cbs < 0 || ibs_uf < 0 || ibs_mun < 0 {
        return Err(ErroReforma::novo(
            format!("aliquota negativa informada para {ano}"),
            "negativa",
        ));
    }
    let fonte = if fonte_pub.is_empty() {
        FONTE_SENADO.to_string()
    } else {
        format!("{FONTE_SENADO}; {fonte_pub}")
    };
    Ok(mk(
        ibs_uf,
        ibs_mun,
        cbs,
        true,
        true,
        String::new(),
        fonte,
        avisos,
    ))
}

/// As aliquotas, ou erro com o motivo. Use ANTES de emitir ou fechar apuracao.
pub fn exigir_aliquotas(
    data: &str,
    publicadas: Option<&Publicadas>,
    simples_nacional: bool,
) -> Resultado<Aliquotas> {
    let a = aliquotas(data, publicadas, simples_nacional)?;
    if !a.confirmado {
        return Err(ErroReforma::novo(a.motivo, "aliquota_nao_publicada"));
    }
    Ok(a)
}
