// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O resumo de valores (servico, descontos, retencoes e valor liquido) e a previa do IBS/CBS que o painel mostra.
//! Dinheiro sempre em centavos inteiros e em texto decimal da biblioteca: nunca `f64`.

use rustnf::reforma::aliquotas::{self, Publicadas};
use rustnf::reforma::tabela;
use rustnf_app::nucleo::{self, Operacao};
use serde_json::{json, Value};

use super::mascara::{
    centavos_para_dinheiro, data_br_para_iso, decimal_para_dinheiro, dinheiro_para_centavos, frase,
};
use super::nota::DadosNota;
use super::pedido::{percentual, Contexto};
use rustnf_app::config::{aliquota_iss_vai_na_dps, Emitente};

/// O dinheiro da nota, em centavos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Totais {
    /// Valor do servico, se foi digitado.
    pub servico: Option<i64>,
    /// Desconto incondicionado.
    pub desc_incondicionado: i64,
    /// Desconto condicionado.
    pub desc_condicionado: i64,
    /// IRRF, CSLL e contribuicao previdenciaria retidos.
    pub retencoes: i64,
    /// Servico menos descontos e retencoes, se ha valor de servico.
    pub liquido: Option<i64>,
}

/// Os totais do que esta preenchido. Campo que nao e' dinheiro valido conta zero (a conferencia ja o aponta).
#[must_use]
pub fn totais(d: &DadosNota) -> Totais {
    let c = |t: &str| dinheiro_para_centavos(t).unwrap_or(0);
    let servico = dinheiro_para_centavos(&d.valor_servico);
    let retencoes = c(&d.irrf)
        .saturating_add(c(&d.csll))
        .saturating_add(c(&d.previdencia));
    let (inc, cond) = (c(&d.desc_incondicionado), c(&d.desc_condicionado));
    Totais {
        servico,
        desc_incondicionado: inc,
        desc_condicionado: cond,
        retencoes,
        liquido: servico.map(|s| {
            s.saturating_sub(inc)
                .saturating_sub(cond)
                .saturating_sub(retencoes)
        }),
    }
}

/// O ISS que a nota deve ter, estimado na tela.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssEstimado {
    /// A aliquota usada, em percentual (`2,00`).
    pub aliquota: String,
    /// Veio da configuracao (alíquota padrao do emitente), nao do que foi digitado.
    pub padrao: bool,
    /// O valor, em centavos: (servico - desconto incondicionado) x aliquota.
    pub valor: i64,
    /// Retido pelo tomador: sai do valor liquido.
    pub retido: bool,
}

/// O ISS estimado: com a aliquota digitada ou, sem ela, com a padrao do `[emitente]` quando ela vai na DPS. `None`
/// quando nao ha aliquota conhecida (a Sefin calcula com a do municipio) ou quando a nota nao tem ISS destacado
/// (MEI, regime especial, servico sem incidencia). A base ignora deducoes e reducoes: o valor final e' o da Sefin.
#[must_use]
pub fn iss_estimado(d: &DadosNota, emitente: &Emitente) -> Option<IssEstimado> {
    if !aliquota_iss_vai_na_dps(
        emitente.op_simples,
        emitente.regime_especial,
        d.tributacao,
        d.retencao,
    ) && d.aliquota_iss.trim().is_empty()
    {
        return None;
    }
    if emitente.op_simples == 2 || emitente.regime_especial != 0 || d.tributacao != 1 {
        return None;
    }
    let (texto, padrao) = if d.aliquota_iss.trim().is_empty() {
        (emitente.aliquota_iss.trim().replace('.', ","), true)
    } else {
        (d.aliquota_iss.trim().to_string(), false)
    };
    let pct = dinheiro_para_centavos(&texto).filter(|p| (0..=500).contains(p))?;
    let servico = dinheiro_para_centavos(&d.valor_servico)?;
    let base = servico.saturating_sub(dinheiro_para_centavos(&d.desc_incondicionado).unwrap_or(0));
    let valor = (i128::from(base) * i128::from(pct) + 5_000) / 10_000;
    Some(IssEstimado {
        aliquota: centavos_para_dinheiro(pct),
        padrao,
        valor: i64::try_from(valor).unwrap_or(0),
        retido: matches!(d.retencao, 2 | 3),
    })
}

/// Uma parcela do IBS/CBS (estadual, municipal ou CBS), ja em texto da tela.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parcela {
    /// Aliquota efetiva, em percentual (`0,10`).
    pub aliquota: String,
    /// Valor devido (`1,23`).
    pub valor: String,
}

/// A previa do IBS/CBS calculada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValoresIbsCbs {
    /// O ano das aliquotas.
    pub ano: i32,
    /// Base de calculo.
    pub base: String,
    /// IBS estadual.
    pub ibs_uf: Parcela,
    /// IBS municipal.
    pub ibs_mun: Parcela,
    /// CBS.
    pub cbs: Parcela,
    /// IBS total (estadual mais municipal).
    pub ibs_total: String,
    /// IBS total mais CBS.
    pub total: String,
    /// Observacoes da biblioteca (por exemplo, aliquota de teste de 2026).
    pub avisos: Vec<String>,
}

/// O que o painel mostra do IBS/CBS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreviaIbsCbs {
    /// O grupo esta desligado nesta nota.
    Desligada,
    /// Falta alguma coisa para calcular: a frase diz o que.
    Incompleta(String),
    /// A conta nao se aplica a esta nota (MEI, competencia anterior a 2026).
    NaoSeAplica(String),
    /// Nao ha aliquota publicada para o ano: a tela aponta a aba Aliquotas das Configuracoes.
    SemAliquota {
        /// O ano.
        ano: i32,
    },
    /// A biblioteca recusou a conta.
    Recusada(String),
    /// Calculada.
    Calculada(Box<ValoresIbsCbs>),
}

/// O valor do ISS em centavos: `base` x `aliquota` (percentual, ate 4 casas), arredondado para cima na metade.
fn iss_em_centavos(base: i64, aliquota: &str) -> Option<i64> {
    let p = percentual(aliquota)?;
    let (inteira, fracao) = p.split_once('.').unwrap_or((&p, ""));
    if fracao.len() > 4 {
        return None;
    }
    let x10000: i128 = i128::from(inteira.parse::<u64>().ok()?) * 10_000
        + i128::from(format!("{fracao:0<4}").parse::<u64>().ok()?);
    let produto = i128::from(base) * x10000;
    let q = (produto + 500_000) / 1_000_000;
    i64::try_from(q).ok()
}

fn decimal(centavos: i64) -> String {
    format!("{}.{:02}", centavos / 100, centavos % 100)
}

/// O valor de um tributo da DPS informado para a base do IBS/CBS (ISSQN ate 2032; PIS e COFINS so' em 2026).
fn tributos_da_base(d: &DadosNota, base_sem_issqn: i64, ctx: &Contexto) -> (i64, i64) {
    let issqn = if ctx.regime().mostra_aliquota_iss(d.tributacao) {
        iss_em_centavos(base_sem_issqn, &d.aliquota_iss).unwrap_or(0)
    } else {
        0
    };
    let pis_cofins = match dinheiro_para_centavos(&d.pis_base) {
        Some(b) => {
            iss_em_centavos(b, &d.pis_aliquota_pis).unwrap_or(0)
                + iss_em_centavos(b, &d.pis_aliquota_cofins).unwrap_or(0)
        }
        None => 0,
    };
    (issqn, pis_cofins)
}

fn texto_json(v: &Value, ponteiro: &str) -> Option<String> {
    v.pointer(ponteiro)
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn parcela(v: &Value, nome: &str) -> Option<Parcela> {
    let aliquota = texto_json(v, &format!("/tributos/{nome}/aliquota_efetiva"))?;
    let valor = texto_json(v, &format!("/tributos/{nome}/devido"))?;
    Some(Parcela {
        aliquota: decimal_para_dinheiro(&aliquota),
        valor: decimal_para_dinheiro(&valor),
    })
}

fn ler_resposta(v: &Value, ano: i32) -> Option<ValoresIbsCbs> {
    let ibs = texto_json(v, "/tributos/ibs")?;
    let cbs = texto_json(v, "/tributos/cbs/devido")?;
    let total = dinheiro_para_centavos(&ibs)?.checked_add(dinheiro_para_centavos(&cbs)?)?;
    Some(ValoresIbsCbs {
        ano,
        base: decimal_para_dinheiro(&texto_json(v, "/tributos/base")?),
        ibs_uf: parcela(v, "ibs_uf")?,
        ibs_mun: parcela(v, "ibs_mun")?,
        cbs: parcela(v, "cbs")?,
        ibs_total: decimal_para_dinheiro(&ibs),
        total: centavos_para_dinheiro(total),
        avisos: v
            .pointer("/tributos/avisos")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(|x| x.as_str().map(frase)).collect())
            .unwrap_or_default(),
    })
}

/// A previa do IBS/CBS da nota, com as aliquotas publicadas da configuracao. Quem calcula e' a biblioteca
/// (`nucleo::calcular_com`); aqui so' se monta a entrada e se le a resposta.
#[must_use]
pub fn ibscbs(d: &DadosNota, ctx: &Contexto) -> PreviaIbsCbs {
    if !d.ibscbs_ativo {
        return PreviaIbsCbs::Desligada;
    }
    let Some(data) = data_br_para_iso(&d.competencia) else {
        return PreviaIbsCbs::Incompleta("Informe a competência.".into());
    };
    let ano: i32 = data[..4].parse().unwrap_or(0);
    let t = totais(d);
    let Some(servico) = t.servico.filter(|s| *s > 0) else {
        return PreviaIbsCbs::Incompleta("Informe o valor do serviço.".into());
    };
    let classe = match tabela::classificacao(&d.cclass_trib) {
        Ok(c) if d.cclass_trib.len() == 6 => c,
        _ => {
            return PreviaIbsCbs::Incompleta(
                "Informe a classificação tributária (cClassTrib).".into(),
            )
        }
    };
    if ano < 2026 {
        return PreviaIbsCbs::NaoSeAplica("O IBS e a CBS começam em 2026.".into());
    }
    let op = ctx.emitente.op_simples;
    if op == 2 {
        return PreviaIbsCbs::NaoSeAplica(
            "MEI: a base de cálculo do IBS e da CBS não se aplica.".into(),
        );
    }
    let mut base = json!({
        "ano": ano,
        "op_simples": op,
        "servico": decimal(servico),
        "desconto_incondicionado": decimal(t.desc_incondicionado),
    });
    if op == 3 {
        if ctx.emitente.regime_apuracao == 0 {
            return PreviaIbsCbs::Incompleta(
                "Escolha o regime de apuração do Simples Nacional nas Configurações.".into(),
            );
        }
        base["regime_apuracao"] = json!(ctx.emitente.regime_apuracao);
    }
    let (issqn, pis_cofins) =
        tributos_da_base(d, servico.saturating_sub(t.desc_incondicionado), ctx);
    base["issqn"] = json!(decimal(issqn));
    base["pis"] = json!(decimal(pis_cofins));
    conta(
        base,
        &Tributos {
            data,
            cst: d.cst(),
            cclass_trib: classe.codigo.clone(),
            op_simples: op,
            ano,
        },
        &ctx.publicadas,
    )
}

/// O que a conta do IBS/CBS usa da data, da classificacao e do regime.
pub(super) struct Tributos {
    /// O dia do fato gerador, `AAAA-MM-DD`.
    pub(super) data: String,
    /// O CST.
    pub(super) cst: String,
    /// O `cClassTrib`.
    pub(super) cclass_trib: String,
    /// O regime: 1 nao optante, 2 MEI, 3 ME/EPP.
    pub(super) op_simples: u8,
    /// O ano de `data`.
    pub(super) ano: i32,
}

/// A conta do IBS/CBS: a `base` (entrada de `reforma::previa::bases`) e os tributos, pela biblioteca, com as aliquotas
/// publicadas. E' o que a Nova nota e a Previa avulsa compartilham.
pub(super) fn conta(base: Value, t: &Tributos, publicadas: &Publicadas) -> PreviaIbsCbs {
    let (data, ano, op) = (&t.data, t.ano, t.op_simples);
    let entrada = json!({
        "base": base,
        "tributos": {
            "data": data,
            "cst": t.cst,
            "cclass_trib": t.cclass_trib,
            "simples_nacional": op == 3,
        },
    });
    let Ok(operacao) = serde_json::from_value::<Operacao>(entrada) else {
        return PreviaIbsCbs::Recusada("Não foi possível montar a conta.".into());
    };
    match nucleo::calcular_com(&operacao, publicadas) {
        Ok(v) => {
            if v["recusado"].as_bool() == Some(true) {
                let motivo = texto_json(&v, "/bases/erros/0/mensagem")
                    .unwrap_or_else(|| "A base de cálculo foi recusada.".into());
                return PreviaIbsCbs::Recusada(frase(&motivo));
            }
            match ler_resposta(&v, ano) {
                Some(valores) => PreviaIbsCbs::Calculada(Box::new(valores)),
                None => PreviaIbsCbs::Recusada("A conta não devolveu os valores esperados.".into()),
            }
        }
        Err(e) => {
            let sem_numero =
                aliquotas::aliquotas(data, Some(publicadas), op == 3).is_ok_and(|a| !a.confirmado);
            if sem_numero {
                PreviaIbsCbs::SemAliquota { ano }
            } else {
                PreviaIbsCbs::Recusada(frase(&e.mensagem()))
            }
        }
    }
}

/// O valor em centavos como a tela o mostra.
#[must_use]
pub fn mostrar(centavos: i64) -> String {
    centavos_para_dinheiro(centavos)
}

#[cfg(test)]
mod testes_do_iss {
    use super::*;

    fn emitente(op: u8, padrao: &str) -> Emitente {
        Emitente {
            op_simples: op,
            aliquota_iss: padrao.into(),
            ..Emitente::default()
        }
    }

    fn nota(valor: &str, aliquota: &str, retencao: u8) -> DadosNota {
        let mut d = DadosNota::novo("21/11/2026");
        d.valor_servico = valor.into();
        d.aliquota_iss = aliquota.into();
        d.retencao = retencao;
        d
    }

    #[test]
    fn iss_com_a_aliquota_digitada_ou_a_padrao() {
        // digitada: 1.000,00 x 2,5% = 25,00
        let i = iss_estimado(&nota("1.000,00", "2,5", 1), &emitente(1, "")).unwrap();
        assert_eq!(
            (i.aliquota.as_str(), i.valor, i.padrao, i.retido),
            ("2,50", 2500, false, false)
        );
        // padrao do emitente (nao optante): 1.000,00 x 2% = 20,00
        let i = iss_estimado(&nota("1.000,00", "", 1), &emitente(1, "2.00")).unwrap();
        assert_eq!((i.valor, i.padrao), (2000, true));
        // desconto incondicionado sai da base
        let mut d = nota("1.000,00", "2", 2);
        d.desc_incondicionado = "100,00".into();
        let i = iss_estimado(&d, &emitente(1, "")).unwrap();
        assert_eq!((i.valor, i.retido), (1800, true));
    }

    #[test]
    fn sem_aliquota_conhecida_ou_sem_iss_destacado_nao_ha_estimativa() {
        // sem digitar e sem padrao: a Sefin calcula
        assert!(iss_estimado(&nota("1.000,00", "", 1), &emitente(1, "")).is_none());
        // ME/EPP sem retencao: a padrao nao vale (E0625)
        assert!(iss_estimado(&nota("1.000,00", "", 1), &emitente(3, "2")).is_none());
        // ME/EPP com retencao: vale
        assert_eq!(
            iss_estimado(&nota("1.000,00", "", 2), &emitente(3, "2")).map(|i| i.valor),
            Some(2000)
        );
        // MEI: nunca
        assert!(iss_estimado(&nota("1.000,00", "2", 1), &emitente(2, "2")).is_none());
        // sem valor de servico
        assert!(iss_estimado(&nota("", "2", 1), &emitente(1, "")).is_none());
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::modelo::nota::CampoNota;
    use crate::modelo::pedido::testes::{config_do_caso, ctx_do_caso, nota_do_caso};
    use rustnf_app::config::{Aliquota, Config};

    #[test]
    fn valor_liquido_e_servico_menos_descontos_e_retencoes() {
        let mut d = nota_do_caso();
        let t = totais(&d);
        assert_eq!(t.servico, Some(123_456));
        assert_eq!(t.desc_incondicionado, 3_456);
        assert_eq!(t.retencoes, 0);
        assert_eq!(t.liquido, Some(120_000));
        d.definir(CampoNota::DescontoCondicionado, "100");
        d.definir(CampoNota::Irrf, "18,5");
        d.definir(CampoNota::Csll, "12,5");
        d.definir(CampoNota::Previdencia, "69");
        let t = totais(&d);
        assert_eq!(t.desc_condicionado, 10_000);
        assert_eq!(t.retencoes, 10_000);
        assert_eq!(t.liquido, Some(100_000));
        assert_eq!(mostrar(100_000), "1.000,00");
        // sem valor de servico nao ha liquido
        assert_eq!(totais(&DadosNota::novo("21/11/2026")).liquido, None);
    }

    #[test]
    fn o_iss_em_centavos_arredonda_na_metade_para_cima() {
        assert_eq!(iss_em_centavos(10_000, "2,5"), Some(250));
        assert_eq!(iss_em_centavos(12_345, "5"), Some(617)); // 617,25 -> 617
        assert_eq!(iss_em_centavos(1_010, "5"), Some(51)); // 50,5 -> 51
        assert_eq!(iss_em_centavos(10_000, ""), None);
        assert_eq!(iss_em_centavos(10_000, "1,23456"), None);
    }

    fn ctx_nao_optante() -> Contexto {
        let mut config = config_do_caso();
        config.emitente.op_simples = 1;
        config.emitente.regime_apuracao = 0;
        Contexto::de_config(&config, "2026-11-21T15:00:00Z".parse().unwrap())
    }

    #[test]
    fn previa_com_aliquota_do_ano_calcula_pela_biblioteca() {
        // 2026 tem aliquota no calendario da lei (CBS 0,9% e IBS estadual 0,1%): nao precisa de publicada
        let p = ibscbs(&nota_do_caso(), &ctx_nao_optante());
        let PreviaIbsCbs::Calculada(v) = p else {
            panic!("esperava calculada: {p:?}");
        };
        assert_eq!(v.ano, 2026);
        assert_eq!(v.base, "1.200,00");
        assert_eq!(v.cbs.aliquota, "0,90");
        assert_eq!(v.cbs.valor, "10,80");
        assert_eq!(v.ibs_uf.aliquota, "0,10");
        assert_eq!(v.ibs_uf.valor, "1,20");
        assert_eq!(v.ibs_mun.valor, "0,00");
        assert_eq!(v.ibs_total, "1,20");
    }

    #[test]
    fn optante_do_simples_em_2026_calcula_zero_e_a_biblioteca_explica() {
        let p = ibscbs(&nota_do_caso(), &ctx_do_caso());
        let PreviaIbsCbs::Calculada(v) = p else {
            panic!("esperava calculada: {p:?}");
        };
        assert_eq!(v.cbs.valor, "0,00");
        assert!(
            v.avisos.iter().any(|a| a.contains("Simples")),
            "{:?}",
            v.avisos
        );
    }

    #[test]
    fn previa_sem_aliquota_para_o_ano_aponta_as_configuracoes() {
        let mut d = nota_do_caso();
        d.definir(CampoNota::Competencia, "21/03/2029");
        let p = ibscbs(&d, &ctx_nao_optante());
        assert_eq!(p, PreviaIbsCbs::SemAliquota { ano: 2029 });
    }

    #[test]
    fn previa_com_aliquota_publicada_na_configuracao() {
        let mut config: Config = config_do_caso();
        config.aliquotas.push(Aliquota {
            ano: 2029,
            cbs: "8,8".into(),
            ibs_uf: "10".into(),
            ibs_mun: "4".into(),
            fonte: "norma de teste".into(),
            ..Aliquota::default()
        });
        config.emitente.op_simples = 1;
        config.emitente.regime_apuracao = 0;
        let ctx = Contexto::de_config(&config, "2026-11-21T15:00:00Z".parse().unwrap());
        let mut d = nota_do_caso();
        d.definir(CampoNota::Competencia, "21/03/2029");
        let p = ibscbs(&d, &ctx);
        let PreviaIbsCbs::Calculada(v) = p else {
            panic!("esperava calculada: {p:?}");
        };
        assert_eq!(v.ano, 2029);
        assert_eq!(v.cbs.aliquota, "8,80");
        assert_eq!(v.ibs_uf.aliquota, "10,00");
        assert_eq!(v.ibs_mun.aliquota, "4,00");
    }

    #[test]
    fn previa_diz_o_que_falta_ou_que_nao_se_aplica() {
        let ctx = ctx_do_caso();
        let mut d = nota_do_caso();
        d.ibscbs_ativo = false;
        assert_eq!(ibscbs(&d, &ctx), PreviaIbsCbs::Desligada);
        d.ibscbs_ativo = true;
        d.definir(CampoNota::ValorServico, "");
        assert!(matches!(ibscbs(&d, &ctx), PreviaIbsCbs::Incompleta(m) if m.contains("valor")));
        let mut d = nota_do_caso();
        d.definir(CampoNota::ClassTrib, "");
        assert!(
            matches!(ibscbs(&d, &ctx), PreviaIbsCbs::Incompleta(m) if m.contains("cClassTrib"))
        );
        let mut d = nota_do_caso();
        d.definir(CampoNota::Competencia, "21/11/2025");
        assert!(matches!(ibscbs(&d, &ctx), PreviaIbsCbs::NaoSeAplica(_)));
        let mut config = config_do_caso();
        config.emitente.op_simples = 2;
        let mei = Contexto::de_config(&config, "2026-11-21T15:00:00Z".parse().unwrap());
        assert!(matches!(
            ibscbs(&nota_do_caso(), &mei),
            PreviaIbsCbs::NaoSeAplica(m) if m.contains("MEI")
        ));
        config.emitente.op_simples = 3;
        config.emitente.regime_apuracao = 0;
        let sem = Contexto::de_config(&config, "2026-11-21T15:00:00Z".parse().unwrap());
        assert!(matches!(
            ibscbs(&nota_do_caso(), &sem),
            PreviaIbsCbs::Incompleta(m) if m.contains("regime de apuração")
        ));
    }

    #[test]
    fn o_iss_informado_entra_na_base_do_ibscbs_ate_2032() {
        let mut d = nota_do_caso();
        d.definir(CampoNota::AliquotaIss, "5");
        let PreviaIbsCbs::Calculada(v) = ibscbs(&d, &ctx_nao_optante()) else {
            panic!("calculada");
        };
        // base: 1.200,00 menos 5% de ISS (60,00)
        assert_eq!(v.base, "1.140,00");
    }
}
