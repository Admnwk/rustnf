// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A Previa avulsa: a calculadora do IBS e da CBS, sem emitir nada. O que se digita vira a entrada de
//! `reforma::previa::bases` e `tributos` (a mesma da linha de comando, `rustnf calcular`); quem calcula e' a
//! biblioteca, com as aliquotas publicadas da configuracao. A conta e o quadro de resultado sao os da Nova nota
//! (`modelo::previa`).

use rustnf::reforma::aliquotas::Publicadas;
use rustnf::reforma::tabela;
use serde_json::{json, Value};

use super::lista::sem_acento;
use super::mascara::{
    data_br_para_iso, dinheiro_para_centavos, dinheiro_para_decimal, formatar_data_br,
    formatar_dinheiro, so_digitos,
};
use super::pedido::Contexto;
use super::previa::{conta, PreviaIbsCbs, Tributos};

/// Quantas classificacoes a busca mostra.
pub const LIMITE_DA_BUSCA: usize = 12;

/// Um campo da calculadora.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CampoPrevia {
    /// Valor do servico.
    Valor,
    /// Desconto incondicionado.
    Desconto,
    /// Data do fato gerador (decide o ano das aliquotas).
    Data,
    /// `cClassTrib`.
    ClassTrib,
    /// ISSQN que sai da base.
    Issqn,
    /// PIS que sai da base.
    Pis,
    /// COFINS que sai da base.
    Cofins,
}

impl CampoPrevia {
    /// Todos os campos, na ordem da tela.
    pub const TODOS: [CampoPrevia; 7] = [
        Self::Valor,
        Self::Desconto,
        Self::Data,
        Self::ClassTrib,
        Self::Issqn,
        Self::Pis,
        Self::Cofins,
    ];

    /// O rotulo do campo.
    #[must_use]
    pub fn rotulo(self) -> &'static str {
        match self {
            Self::Valor => "Valor do serviço",
            Self::Desconto => "Desconto incondicionado",
            Self::Data => "Data do fato gerador",
            Self::ClassTrib => "Classificação tributária (cClassTrib)",
            Self::Issqn => "ISSQN",
            Self::Pis => "PIS",
            Self::Cofins => "COFINS",
        }
    }
}

/// O que a pessoa digitou na calculadora, em texto como na tela.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DadosDaPrevia {
    /// Valor do servico.
    pub valor: String,
    /// Desconto incondicionado.
    pub desconto: String,
    /// Data, `DD/MM/AAAA`.
    pub data: String,
    /// `cClassTrib` (6 digitos).
    pub cclass_trib: String,
    /// ISSQN que sai da base (em reais).
    pub issqn: String,
    /// PIS que sai da base (em reais).
    pub pis: String,
    /// COFINS que sai da base (em reais).
    pub cofins: String,
    /// O regime simulado: 1 nao optante, 2 MEI, 3 ME/EPP. Comeca pelo do emitente.
    pub op_simples: u8,
    /// O regime de apuracao do IBS/CBS do optante (1 a 3); 0 = nenhum escolhido.
    pub regime_apuracao: u8,
}

impl DadosDaPrevia {
    /// A calculadora vazia: data de hoje e o regime do emitente.
    #[must_use]
    pub fn nova(ctx: &Contexto) -> Self {
        Self {
            valor: String::new(),
            desconto: String::new(),
            data: ctx.hoje(),
            cclass_trib: String::new(),
            issqn: String::new(),
            pis: String::new(),
            cofins: String::new(),
            op_simples: ctx.emitente.op_simples.clamp(1, 3),
            regime_apuracao: ctx.emitente.regime_apuracao,
        }
    }

    /// O texto como a tela mostra o campo depois de digitado (a mascara de cada um).
    #[must_use]
    pub fn normalizar(campo: CampoPrevia, texto: &str) -> String {
        match campo {
            CampoPrevia::Valor
            | CampoPrevia::Desconto
            | CampoPrevia::Issqn
            | CampoPrevia::Pis
            | CampoPrevia::Cofins => formatar_dinheiro(texto),
            CampoPrevia::Data => formatar_data_br(texto),
            CampoPrevia::ClassTrib => so_digitos(texto, 6),
        }
    }

    /// Guarda o texto digitado, ja com a mascara, e o devolve.
    pub fn definir(&mut self, campo: CampoPrevia, texto: &str) -> String {
        let normal = Self::normalizar(campo, texto);
        match campo {
            CampoPrevia::Valor => self.valor.clone_from(&normal),
            CampoPrevia::Desconto => self.desconto.clone_from(&normal),
            CampoPrevia::Data => self.data.clone_from(&normal),
            CampoPrevia::ClassTrib => self.cclass_trib.clone_from(&normal),
            CampoPrevia::Issqn => self.issqn.clone_from(&normal),
            CampoPrevia::Pis => self.pis.clone_from(&normal),
            CampoPrevia::Cofins => self.cofins.clone_from(&normal),
        }
        normal
    }

    /// O texto atual de um campo.
    #[must_use]
    pub fn texto(&self, campo: CampoPrevia) -> &str {
        match campo {
            CampoPrevia::Valor => &self.valor,
            CampoPrevia::Desconto => &self.desconto,
            CampoPrevia::Data => &self.data,
            CampoPrevia::ClassTrib => &self.cclass_trib,
            CampoPrevia::Issqn => &self.issqn,
            CampoPrevia::Pis => &self.pis,
            CampoPrevia::Cofins => &self.cofins,
        }
    }

    /// O dia do fato gerador, `AAAA-MM-DD`, se a data digitada existe.
    #[must_use]
    pub fn data_iso(&self) -> Option<String> {
        data_br_para_iso(&self.data)
    }

    /// O erro do campo (embaixo dele), ou `None`. Campo vazio nao e' erro aqui: o quadro de resultado diz o que falta.
    #[must_use]
    pub fn erro_do(&self, campo: CampoPrevia) -> Option<String> {
        match campo {
            CampoPrevia::Data if self.data.len() == 10 && self.data_iso().is_none() => {
                Some("Data inválida (use DD/MM/AAAA, de 2000 a 2099).".into())
            }
            CampoPrevia::ClassTrib if self.cclass_trib.len() == 6 => {
                tabela::classificacao(&self.cclass_trib)
                    .err()
                    .map(|_| "Esta classificação não consta da tabela oficial.".into())
            }
            _ => None,
        }
    }

    /// A entrada de `reforma::previa::bases`: ano, regime e os valores que o campo traz. O que esta vazio nao entra.
    #[must_use]
    pub fn entrada_da_base(&self, ano: i32) -> Value {
        let mut base = serde_json::Map::new();
        base.insert("ano".into(), json!(ano));
        base.insert("op_simples".into(), json!(self.op_simples));
        if self.op_simples == 3 && self.regime_apuracao != 0 {
            base.insert("regime_apuracao".into(), json!(self.regime_apuracao));
        }
        for (chave, campo) in [
            ("servico", CampoPrevia::Valor),
            ("desconto_incondicionado", CampoPrevia::Desconto),
            ("issqn", CampoPrevia::Issqn),
            ("pis", CampoPrevia::Pis),
            ("cofins", CampoPrevia::Cofins),
        ] {
            if let Some(d) = dinheiro_para_decimal(self.texto(campo)) {
                base.insert(chave.into(), json!(d));
            }
        }
        Value::Object(base)
    }

    /// A conta, pela biblioteca, com as aliquotas publicadas. Diz o que falta, o que nao se aplica, a recusa da
    /// biblioteca ou que nao ha aliquota para o ano; nunca inventa numero.
    #[must_use]
    pub fn calcular(&self, publicadas: &Publicadas) -> PreviaIbsCbs {
        let Some(data) = self.data_iso() else {
            return PreviaIbsCbs::Incompleta("Informe a data (DD/MM/AAAA).".into());
        };
        let ano: i32 = data.get(..4).and_then(|a| a.parse().ok()).unwrap_or(0);
        if !dinheiro_para_centavos(&self.valor).is_some_and(|v| v > 0) {
            return PreviaIbsCbs::Incompleta("Informe o valor do serviço.".into());
        }
        let classe = match tabela::classificacao(&self.cclass_trib) {
            Ok(c) if self.cclass_trib.len() == 6 => c,
            _ => {
                return PreviaIbsCbs::Incompleta(
                    "Informe a classificação tributária (cClassTrib).".into(),
                )
            }
        };
        if ano < 2026 {
            return PreviaIbsCbs::NaoSeAplica("O IBS e a CBS começam em 2026.".into());
        }
        if self.op_simples == 2 {
            return PreviaIbsCbs::NaoSeAplica(
                "MEI: a base de cálculo do IBS e da CBS não se aplica.".into(),
            );
        }
        if self.op_simples == 3 && self.regime_apuracao == 0 {
            return PreviaIbsCbs::Incompleta(
                "Escolha o regime de apuração do Simples Nacional.".into(),
            );
        }
        conta(
            self.entrada_da_base(ano),
            &Tributos {
                data,
                cst: classe.cst.clone(),
                cclass_trib: classe.codigo.clone(),
                op_simples: self.op_simples,
                ano,
            },
            publicadas,
        )
    }
}

/// Uma classificacao tributaria da tabela oficial, para a lista de resultados da busca.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Classificacao {
    /// Os 6 digitos.
    pub codigo: String,
    /// O nome oficial.
    pub nome: String,
    /// A descricao oficial.
    pub descricao: String,
}

/// Busca na tabela de `cClassTrib` vigentes na `data` (`AAAA-MM-DD`) para a NFS-e, por codigo ou por texto (sem
/// acento, todas as palavras). Termo vazio: nada.
#[must_use]
pub fn buscar_classificacao(termo: &str, data: Option<&str>) -> Vec<Classificacao> {
    let palavras: Vec<String> = sem_acento(termo)
        .split_whitespace()
        .map(str::to_string)
        .collect();
    if palavras.is_empty() {
        return Vec::new();
    }
    let mut achadas: Vec<&tabela::ClassTrib> = tabela::classificacoes(None, Some("nfse"), data)
        .into_iter()
        .filter(|c| {
            let palheiro = format!(
                "{} {} {}",
                c.codigo,
                sem_acento(&c.nome),
                sem_acento(&c.descricao)
            );
            palavras.iter().all(|p| palheiro.contains(p.as_str()))
        })
        .collect();
    achadas.sort_by(|a, b| a.codigo.cmp(&b.codigo));
    achadas
        .into_iter()
        .take(LIMITE_DA_BUSCA)
        .map(|c| Classificacao {
            codigo: c.codigo.clone(),
            nome: c.nome.clone(),
            descricao: c.descricao.clone(),
        })
        .collect()
}

#[cfg(test)]
mod testes {
    use rustnf_app::config::Aliquota;

    use super::*;
    use crate::modelo::pedido::testes::{config_do_caso, ctx_do_caso};
    use crate::modelo::previa::ValoresIbsCbs;

    fn ctx_nao_optante() -> Contexto {
        let mut config = config_do_caso();
        config.emitente.op_simples = 1;
        config.emitente.regime_apuracao = 0;
        Contexto::de_config(&config, "2026-11-21T15:00:00Z".parse().unwrap())
    }

    fn preenchida() -> DadosDaPrevia {
        let mut d = DadosDaPrevia::nova(&ctx_nao_optante());
        d.definir(CampoPrevia::Valor, "1000");
        d.definir(CampoPrevia::ClassTrib, "000001");
        d
    }

    fn calculada(p: PreviaIbsCbs) -> ValoresIbsCbs {
        match p {
            PreviaIbsCbs::Calculada(v) => *v,
            outro => panic!("esperava calculada: {outro:?}"),
        }
    }

    #[test]
    fn nasce_com_a_data_de_hoje_e_o_regime_do_emitente() {
        let d = DadosDaPrevia::nova(&ctx_do_caso());
        assert_eq!(d.data, "21/11/2026");
        assert_eq!((d.op_simples, d.regime_apuracao), (3, 1));
        let d = DadosDaPrevia::nova(&ctx_nao_optante());
        assert_eq!((d.op_simples, d.regime_apuracao), (1, 0));
    }

    #[test]
    fn cada_campo_aplica_a_mascara_e_guarda_no_lugar_certo() {
        let mut d = DadosDaPrevia::nova(&ctx_nao_optante());
        for (campo, digitado, esperado) in [
            (CampoPrevia::Valor, "1234,5", "1.234,5"),
            (CampoPrevia::Desconto, "10", "10"),
            (CampoPrevia::Data, "02032027", "02/03/2027"),
            (CampoPrevia::ClassTrib, "00000123", "000001"),
            (CampoPrevia::Issqn, "50,25", "50,25"),
            (CampoPrevia::Pis, "6,5", "6,5"),
            (CampoPrevia::Cofins, "30", "30"),
        ] {
            assert_eq!(d.definir(campo, digitado), esperado, "{campo:?}");
            assert_eq!(d.texto(campo), esperado);
        }
    }

    #[test]
    fn cada_campo_vai_para_o_lugar_certo_da_entrada_da_base() {
        let mut d = DadosDaPrevia::nova(&ctx_nao_optante());
        d.definir(CampoPrevia::Valor, "1.000,00");
        d.definir(CampoPrevia::Desconto, "50");
        d.definir(CampoPrevia::Issqn, "47,5");
        d.definir(CampoPrevia::Pis, "6,18");
        d.definir(CampoPrevia::Cofins, "28,5");
        assert_eq!(
            d.entrada_da_base(2026),
            json!({
                "ano": 2026,
                "op_simples": 1,
                "servico": "1000.00",
                "desconto_incondicionado": "50",
                "issqn": "47.50",
                "pis": "6.18",
                "cofins": "28.50",
            })
        );
        // campo vazio nao entra; o regime de apuracao so' entra para ME/EPP
        let vazia = DadosDaPrevia::nova(&ctx_nao_optante());
        assert_eq!(
            vazia.entrada_da_base(2027),
            json!({"ano": 2027, "op_simples": 1})
        );
        let mut me = DadosDaPrevia::nova(&ctx_do_caso());
        me.definir(CampoPrevia::Valor, "10");
        assert_eq!(
            me.entrada_da_base(2026),
            json!({"ano": 2026, "op_simples": 3, "regime_apuracao": 1, "servico": "10"})
        );
    }

    #[test]
    fn a_entrada_montada_e_aceita_pela_biblioteca() {
        let mut d = preenchida();
        d.definir(CampoPrevia::Issqn, "5");
        d.definir(CampoPrevia::Pis, "6,5");
        d.definir(CampoPrevia::Cofins, "30");
        let v = calculada(d.calcular(&Publicadas::new()));
        // o ISSQN, o PIS e a COFINS informados saem da base (o que cada um deles reduz e' da biblioteca)
        assert_eq!(v.ano, 2026);
        assert_ne!(v.base, "1.000,00");
    }

    #[test]
    fn com_aliquota_do_ano_calcula_pela_biblioteca() {
        // 2026 tem aliquota no calendario da lei: CBS 0,9% e IBS estadual 0,1% sobre a base de 1.000,00
        let v = calculada(preenchida().calcular(&Publicadas::new()));
        assert_eq!(v.base, "1.000,00");
        assert_eq!(
            (v.cbs.aliquota.as_str(), v.cbs.valor.as_str()),
            ("0,90", "9,00")
        );
        assert_eq!(
            (v.ibs_uf.aliquota.as_str(), v.ibs_uf.valor.as_str()),
            ("0,10", "1,00")
        );
        assert_eq!(v.ibs_mun.valor, "0,00");
        assert_eq!(v.ibs_total, "1,00");
        assert_eq!(v.total, "10,00");
    }

    #[test]
    fn o_desconto_incondicionado_reduz_a_base() {
        let mut d = preenchida();
        d.definir(CampoPrevia::Desconto, "100");
        let v = calculada(d.calcular(&Publicadas::new()));
        assert_eq!(v.base, "900,00");
        assert_eq!(v.cbs.valor, "8,10");
    }

    #[test]
    fn sem_aliquota_para_o_ano_diz_isso_e_nao_inventa_numero() {
        let mut d = preenchida();
        d.definir(CampoPrevia::Data, "21/03/2029");
        assert_eq!(
            d.calcular(&Publicadas::new()),
            PreviaIbsCbs::SemAliquota { ano: 2029 }
        );
        // com a aliquota publicada na configuracao, calcula
        let mut config = config_do_caso();
        config.aliquotas.push(Aliquota {
            ano: 2029,
            cbs: "8,8".into(),
            ibs_uf: "10".into(),
            ibs_mun: "4".into(),
            fonte: "norma de teste".into(),
            ..Aliquota::default()
        });
        let v = calculada(d.calcular(&config.publicadas()));
        assert_eq!(v.ano, 2029);
        assert_eq!(v.cbs.aliquota, "8,80");
        assert_eq!(v.ibs_uf.aliquota, "10,00");
        assert_eq!(v.ibs_mun.aliquota, "4,00");
    }

    #[test]
    fn diz_o_que_falta_e_o_que_nao_se_aplica() {
        let p = Publicadas::new();
        let mut d = DadosDaPrevia::nova(&ctx_nao_optante());
        assert!(matches!(d.calcular(&p), PreviaIbsCbs::Incompleta(m) if m.contains("valor")));
        d.definir(CampoPrevia::Valor, "100");
        assert!(matches!(d.calcular(&p), PreviaIbsCbs::Incompleta(m) if m.contains("cClassTrib")));
        d.definir(CampoPrevia::ClassTrib, "000001");
        d.definir(CampoPrevia::Data, "21/11/2025");
        assert!(matches!(d.calcular(&p), PreviaIbsCbs::NaoSeAplica(_)));
        d.definir(CampoPrevia::Data, "31/02/2026");
        assert!(matches!(d.calcular(&p), PreviaIbsCbs::Incompleta(m) if m.contains("data")));
        d.definir(CampoPrevia::Data, "21/11/2026");
        d.op_simples = 2;
        assert!(matches!(d.calcular(&p), PreviaIbsCbs::NaoSeAplica(m) if m.contains("MEI")));
        d.op_simples = 3;
        d.regime_apuracao = 0;
        assert!(
            matches!(d.calcular(&p), PreviaIbsCbs::Incompleta(m) if m.contains("regime de apuração"))
        );
        d.regime_apuracao = 1;
        // optante em 2026: calcula, e a biblioteca explica por que o valor e' zero
        let v = calculada(d.calcular(&p));
        assert_eq!(v.cbs.valor, "0,00");
        assert!(
            v.avisos.iter().any(|a| a.contains("Simples")),
            "{:?}",
            v.avisos
        );
        // classificacao que nao existe e' incompleta, nao um erro de conta
        d.definir(CampoPrevia::ClassTrib, "999999");
        assert!(matches!(d.calcular(&p), PreviaIbsCbs::Incompleta(_)));
    }

    #[test]
    fn a_recusa_da_biblioteca_aparece_com_o_motivo() {
        // desconto maior que o servico: a base ficaria negativa
        let mut d = preenchida();
        d.definir(CampoPrevia::Desconto, "2000");
        match d.calcular(&Publicadas::new()) {
            PreviaIbsCbs::Recusada(m) => assert!(!m.is_empty()),
            outro => panic!("esperava recusada: {outro:?}"),
        }
    }

    #[test]
    fn o_erro_do_campo_so_aparece_com_o_valor_completo() {
        let mut d = DadosDaPrevia::nova(&ctx_nao_optante());
        assert_eq!(d.erro_do(CampoPrevia::Data), None);
        d.definir(CampoPrevia::Data, "31/02/2026");
        assert!(d.erro_do(CampoPrevia::Data).unwrap().contains("inválida"));
        d.definir(CampoPrevia::Data, "28/02/2026");
        assert_eq!(d.erro_do(CampoPrevia::Data), None);
        d.definir(CampoPrevia::ClassTrib, "00000");
        assert_eq!(d.erro_do(CampoPrevia::ClassTrib), None, "ainda digitando");
        d.definir(CampoPrevia::ClassTrib, "999999");
        assert!(d
            .erro_do(CampoPrevia::ClassTrib)
            .unwrap()
            .contains("tabela oficial"));
        d.definir(CampoPrevia::ClassTrib, "000001");
        assert_eq!(d.erro_do(CampoPrevia::ClassTrib), None);
    }

    #[test]
    fn a_busca_de_classificacao_acha_por_codigo_e_por_texto_sem_acento() {
        let por_codigo = buscar_classificacao("000001", Some("2026-11-21"));
        assert_eq!(por_codigo.len(), 1);
        assert_eq!(por_codigo[0].codigo, "000001");
        assert!(!por_codigo[0].nome.is_empty());
        let por_texto = buscar_classificacao("tributadas INTEGRALMENTE", Some("2026-11-21"));
        assert!(
            por_texto.iter().any(|c| c.codigo == "000001"),
            "{por_texto:?}"
        );
        assert!(buscar_classificacao("", None).is_empty());
        assert!(buscar_classificacao("zzzzzzzz", None).is_empty());
        assert!(buscar_classificacao("a", None).len() <= LIMITE_DA_BUSCA);
        // ordenado pelo codigo
        let varias = buscar_classificacao("tributacao", None);
        let mut ordenadas = varias.clone();
        ordenadas.sort_by(|a, b| a.codigo.cmp(&b.codigo));
        assert_eq!(varias, ordenadas);
    }
}
