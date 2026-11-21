// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Do que foi preenchido para o PEDIDO em JSON, no mesmo formato que a linha de comando aceita (`rustnf::nfse::Dps`),
//! sem o prestador (quem o completa e' `rustnf_app::config::aplicar_emitente`). A tela nao monta `Dps` na mao nem
//! repete regra fiscal: campo vazio nao entra no JSON, e a biblioteca confere o resto.

use chrono::{DateTime, Duration, Utc};
use rustnf::reforma::aliquotas::Publicadas;
use rustnf_app::config::{Config, Emitente};
use serde_json::{json, Map, Value};

use super::ambiente::Ambiente;
use super::mascara::{data_br_para_iso, dinheiro_para_decimal, iso_para_data_br};
use super::nota::{DadosNota, Regime};

/// O que a nota precisa saber do mundo: o emitente, as aliquotas publicadas, a folga do relogio, o ambiente e o dia
/// de hoje (em Brasilia).
#[derive(Debug, Clone)]
pub struct Contexto {
    /// O `[emitente]` da configuracao.
    pub emitente: Emitente,
    /// As `[[aliquotas]]` da configuracao.
    pub publicadas: Publicadas,
    /// Folga do relogio, em segundos (`[sefin].folga_relogio_s`).
    pub folga_s: u32,
    /// O ambiente em vigor.
    pub ambiente: Ambiente,
    /// Hoje, `AAAA-MM-DD`.
    pub hoje_iso: String,
}

impl Contexto {
    /// O contexto de uma configuracao, no instante `agora`.
    #[must_use]
    pub fn de_config(config: &Config, agora: DateTime<Utc>) -> Self {
        Self {
            emitente: config.emitente.clone(),
            publicadas: config.publicadas(),
            folga_s: config.sefin.folga_relogio_s,
            ambiente: Ambiente::de_config(&config.sefin.ambiente),
            hoje_iso: (agora - Duration::hours(3)).format("%Y-%m-%d").to_string(),
        }
    }

    /// Hoje, `DD/MM/AAAA`.
    #[must_use]
    pub fn hoje(&self) -> String {
        iso_para_data_br(&self.hoje_iso)
    }

    /// O regime do emitente.
    #[must_use]
    pub fn regime(&self) -> Regime {
        Regime::de(&self.emitente)
    }

    /// A serie da DPS: a do `[emitente]`, ou 1.
    #[must_use]
    pub fn serie(&self) -> String {
        let s = self.emitente.serie.trim();
        if s.is_empty() {
            "1".into()
        } else {
            s.to_string()
        }
    }

    /// O documento do emitente (CNPJ, ou CPF na falta dele).
    #[must_use]
    pub fn documento_do_emitente(&self) -> &str {
        if self.emitente.cnpj.is_empty() {
            &self.emitente.cpf
        } else {
            &self.emitente.cnpj
        }
    }
}

/// Poe `valor` (sem espacos nas pontas) em `mapa[chave]`, se nao for vazio.
fn texto(mapa: &mut Map<String, Value>, chave: &str, valor: &str) {
    let v = valor.trim();
    if !v.is_empty() {
        mapa.insert(chave.into(), json!(v));
    }
}

/// Dinheiro (`1.234,56`) como decimal com ponto no JSON, se houver.
fn dinheiro(mapa: &mut Map<String, Value>, chave: &str, valor: &str) {
    if let Some(d) = dinheiro_para_decimal(valor) {
        mapa.insert(chave.into(), json!(d));
    }
}

/// Percentual digitado (`2,5`) como decimal com ponto no JSON, se houver.
pub(super) fn percentual(valor: &str) -> Option<String> {
    let t = valor.trim().replace(',', ".");
    let t = t.trim_end_matches('.');
    if t.is_empty() {
        None
    } else if t.starts_with('.') {
        Some(format!("0{t}"))
    } else {
        Some(t.to_string())
    }
}

fn percentual_em(mapa: &mut Map<String, Value>, chave: &str, valor: &str) {
    if let Some(p) = percentual(valor) {
        mapa.insert(chave.into(), json!(p));
    }
}

fn so_alfanumericos(t: &str) -> String {
    t.chars().filter(char::is_ascii_alphanumeric).collect()
}

fn so_digitos(t: &str) -> String {
    t.chars().filter(char::is_ascii_digit).collect()
}

fn tomador(d: &DadosNota) -> Option<Value> {
    if d.sem_tomador {
        return None;
    }
    let mut t = Map::new();
    texto(&mut t, "documento", &so_alfanumericos(&d.tomador_documento));
    texto(&mut t, "nome", &d.tomador_nome);
    texto(&mut t, "im", &d.tomador_im);
    texto(&mut t, "email", &d.tomador_email);
    texto(&mut t, "fone", &so_digitos(&d.tomador_fone));
    if d.tem_endereco() {
        let mut e = Map::new();
        texto(&mut e, "cmun", &d.end_municipio);
        texto(&mut e, "cep", &so_digitos(&d.end_cep));
        texto(&mut e, "logradouro", &d.end_logradouro);
        texto(&mut e, "numero", &d.end_numero);
        texto(&mut e, "complemento", &d.end_complemento);
        texto(&mut e, "bairro", &d.end_bairro);
        t.insert("endereco".into(), Value::Object(e));
    }
    (!t.is_empty()).then_some(Value::Object(t))
}

fn servico(d: &DadosNota) -> Value {
    let mut s = Map::new();
    texto(&mut s, "ctribnac", &d.ctribnac_digitos());
    texto(&mut s, "descricao", &d.descricao);
    texto(&mut s, "cmun_prestacao", &d.municipio_prestacao);
    texto(&mut s, "ctribmun", &d.ctribmun);
    texto(&mut s, "cnbs", &d.nbs_digitos());
    texto(&mut s, "cod_interno", &d.cod_interno);
    Value::Object(s)
}

fn federal(d: &DadosNota) -> Option<Value> {
    let mut f = Map::new();
    dinheiro(&mut f, "retencao_irrf", &d.irrf);
    dinheiro(&mut f, "retencao_csll", &d.csll);
    dinheiro(&mut f, "retencao_cp", &d.previdencia);
    let mut p = Map::new();
    texto(&mut p, "cst", &d.pis_cst);
    dinheiro(&mut p, "base", &d.pis_base);
    percentual_em(&mut p, "aliquota_pis", &d.pis_aliquota_pis);
    percentual_em(&mut p, "aliquota_cofins", &d.pis_aliquota_cofins);
    if let Ok(tipo) = d.pis_tipo_retencao.trim().parse::<u8>() {
        p.insert("tipo_retencao".into(), json!(tipo));
    }
    if !p.is_empty() {
        f.insert("pis_cofins".into(), Value::Object(p));
    }
    (!f.is_empty()).then_some(Value::Object(f))
}

fn total(d: &DadosNota, regime: Regime) -> Value {
    let mut t = Map::new();
    if regime.pede_percentual_sn() {
        percentual_em(&mut t, "percentual_sn", &d.percentual_sn);
    }
    if regime.pede_totais_aproximados() {
        percentual_em(&mut t, "percentual_federal", &d.percentual_federal);
        percentual_em(&mut t, "percentual_estadual", &d.percentual_estadual);
        percentual_em(&mut t, "percentual_municipal", &d.percentual_municipal);
    }
    Value::Object(t)
}

fn valores(d: &DadosNota, regime: Regime) -> Value {
    let mut v = Map::new();
    dinheiro(&mut v, "servico", &d.valor_servico);
    dinheiro(&mut v, "desconto_incondicionado", &d.desc_incondicionado);
    dinheiro(&mut v, "desconto_condicionado", &d.desc_condicionado);
    let mut m = Map::new();
    m.insert("tributacao".into(), json!(d.tributacao));
    m.insert("retencao".into(), json!(d.retencao));
    if regime.mostra_aliquota_iss(d.tributacao) {
        percentual_em(&mut m, "aliquota", &d.aliquota_iss);
    }
    v.insert("municipal".into(), Value::Object(m));
    if let Some(f) = federal(d) {
        v.insert("federal".into(), f);
    }
    v.insert("total".into(), total(d, regime));
    Value::Object(v)
}

fn ibscbs(d: &DadosNota) -> Value {
    let mut g = Map::new();
    texto(&mut g, "cind_op", &d.cind_op);
    texto(&mut g, "cst", &d.cst());
    texto(&mut g, "cclass_trib", &d.cclass_trib);
    // o `finNFSe` so' aceita 0 (emissao regular) e o destinatario e' sempre o tomador (`indDest` 0)
    g.insert("fin".into(), json!(0));
    g.insert("ind_dest".into(), json!(0));
    if let Some(f) = d.ind_final {
        g.insert("ind_final".into(), json!(f));
    }
    Value::Object(g)
}

/// O pedido em JSON do que foi preenchido, sem o prestador, o numero e a data/hora de emissao. Campo vazio nao entra;
/// o que o regime do emitente nao pede tambem nao.
#[must_use]
pub fn montar_pedido(d: &DadosNota, ctx: &Contexto) -> Value {
    let regime = ctx.regime();
    let mut p = Map::new();
    if let Some(iso) = data_br_para_iso(&d.competencia) {
        p.insert("competencia".into(), json!(iso));
    }
    if let Some(t) = tomador(d) {
        p.insert("tomador".into(), t);
    }
    p.insert("servico".into(), servico(d));
    p.insert("valores".into(), valores(d, regime));
    if d.ibscbs_ativo {
        p.insert("ibscbs".into(), ibscbs(d));
    }
    Value::Object(p)
}

/// O pedido de uma tentativa de emissao, congelado: o mesmo numero, a mesma serie, a mesma data e hora de emissao
/// e os mesmos dados, em cada reenvio.
#[derive(Debug, Clone, PartialEq)]
pub struct PedidoCongelado {
    /// O JSON completo (com `numero`, `serie` e `emissao`), como vai para `emitir_pedido_completo`.
    pub pedido: Value,
    /// O numero da DPS.
    pub numero: u64,
    /// A serie da DPS.
    pub serie: String,
    /// `dhEmi`.
    pub emissao: String,
}

/// Congela o pedido da nota com o `numero` (da pasta de notas) e o `dhEmi` dados.
#[must_use]
pub fn congelar(d: &DadosNota, ctx: &Contexto, numero: u64, emissao: &str) -> PedidoCongelado {
    let mut pedido = montar_pedido(d, ctx);
    let serie = ctx.serie();
    if let Some(raiz) = pedido.as_object_mut() {
        raiz.insert("numero".into(), json!(numero));
        raiz.insert("serie".into(), json!(serie));
        raiz.insert("emissao".into(), json!(emissao));
    }
    PedidoCongelado {
        pedido,
        numero,
        serie,
        emissao: emissao.to_string(),
    }
}

#[cfg(test)]
pub(crate) mod testes {
    use super::*;
    use crate::modelo::nota::CampoNota;

    pub(crate) fn config_do_caso() -> Config {
        let mut c = Config::default();
        c.emitente.cnpj = "11222333000181".into();
        c.emitente.nome = "OFICINA DE TESTE LTDA".into();
        c.emitente.inscricao_municipal = "123456".into();
        c.emitente.municipio = "3550308".into();
        c.emitente.op_simples = 3;
        c.emitente.regime_apuracao = 1;
        c
    }

    pub(crate) fn ctx_do_caso() -> Contexto {
        let agora = "2026-11-21T15:00:00Z".parse().unwrap();
        Contexto::de_config(&config_do_caso(), agora)
    }

    /// A nota do caso comum: ME/EPP, tomador pessoa fisica com endereco, servico de informatica, IBS/CBS ligado.
    pub(crate) fn nota_do_caso() -> DadosNota {
        let mut d = DadosNota::novo("21/11/2026");
        for (campo, texto) in [
            (CampoNota::TomadorDocumento, "52998224725"),
            (CampoNota::TomadorNome, "Maria de Teste"),
            (CampoNota::TomadorEmail, "maria@exemplo.com.br"),
            (CampoNota::TomadorFone, "11988887777"),
            (CampoNota::EnderecoCep, "01310100"),
            (CampoNota::EnderecoLogradouro, "Avenida de Teste"),
            (CampoNota::EnderecoNumero, "100"),
            (CampoNota::EnderecoBairro, "Centro"),
            (CampoNota::EnderecoMunicipio, "3550308"),
            (CampoNota::Ctribnac, "140101"),
            (CampoNota::Descricao, "Revisão geral do veículo"),
            (CampoNota::Nbs, "120011000"),
            (CampoNota::ValorServico, "1234,56"),
            (CampoNota::DescontoIncondicionado, "34,56"),
            (CampoNota::PercentualSn, "4,5"),
            (CampoNota::CindOp, "050101"),
            (CampoNota::ClassTrib, "000001"),
        ] {
            d.definir(campo, texto);
        }
        d
    }

    #[test]
    fn o_caso_comum_vira_o_json_esperado() {
        let esperado: Value =
            serde_json::from_str(include_str!("../../tests/casos/pedido_caso_comum.json")).unwrap();
        let obtido = montar_pedido(&nota_do_caso(), &ctx_do_caso());
        assert_eq!(obtido, esperado, "{obtido:#}");
    }

    #[test]
    fn campo_vazio_nao_entra_no_json() {
        let d = DadosNota::novo("21/11/2026");
        let p = montar_pedido(&d, &ctx_do_caso());
        assert_eq!(
            p,
            json!({
                "competencia": "2026-11-21",
                "servico": {},
                "valores": {"municipal": {"tributacao": 1, "retencao": 1}, "total": {}},
                "ibscbs": {"fin": 0, "ind_dest": 0}
            })
        );
        let mut sem = d;
        sem.ibscbs_ativo = false;
        sem.competencia.clear();
        let p = montar_pedido(&sem, &ctx_do_caso());
        assert!(p.get("ibscbs").is_none());
        assert!(p.get("competencia").is_none());
        assert!(p.get("tomador").is_none());
        assert!(p["valores"].get("federal").is_none());
    }

    #[test]
    fn cada_campo_vai_para_o_lugar_certo_do_json() {
        let ctx = ctx_do_caso();
        let casos: [(CampoNota, &str, &str, Value); 22] = [
            (
                CampoNota::TomadorDocumento,
                "52998224725",
                "/tomador/documento",
                json!("52998224725"),
            ),
            (CampoNota::TomadorNome, "Ana", "/tomador/nome", json!("Ana")),
            (CampoNota::TomadorIm, "998", "/tomador/im", json!("998")),
            (
                CampoNota::TomadorEmail,
                "a@b.com",
                "/tomador/email",
                json!("a@b.com"),
            ),
            (
                CampoNota::TomadorFone,
                "1188887777",
                "/tomador/fone",
                json!("1188887777"),
            ),
            (
                CampoNota::EnderecoCep,
                "01310100",
                "/tomador/endereco/cep",
                json!("01310100"),
            ),
            (
                CampoNota::EnderecoLogradouro,
                "Rua A",
                "/tomador/endereco/logradouro",
                json!("Rua A"),
            ),
            (
                CampoNota::EnderecoNumero,
                "7",
                "/tomador/endereco/numero",
                json!("7"),
            ),
            (
                CampoNota::EnderecoComplemento,
                "ap 2",
                "/tomador/endereco/complemento",
                json!("ap 2"),
            ),
            (
                CampoNota::EnderecoBairro,
                "Centro",
                "/tomador/endereco/bairro",
                json!("Centro"),
            ),
            (
                CampoNota::EnderecoMunicipio,
                "3550308",
                "/tomador/endereco/cmun",
                json!("3550308"),
            ),
            (
                CampoNota::Ctribnac,
                "140101",
                "/servico/ctribnac",
                json!("140101"),
            ),
            (
                CampoNota::Descricao,
                "Serviço",
                "/servico/descricao",
                json!("Serviço"),
            ),
            (
                CampoNota::MunicipioPrestacao,
                "4106902",
                "/servico/cmun_prestacao",
                json!("4106902"),
            ),
            (
                CampoNota::Nbs,
                "120011000",
                "/servico/cnbs",
                json!("120011000"),
            ),
            (
                CampoNota::CtribMun,
                "001",
                "/servico/ctribmun",
                json!("001"),
            ),
            (
                CampoNota::CodInterno,
                "A-1",
                "/servico/cod_interno",
                json!("A-1"),
            ),
            (
                CampoNota::ValorServico,
                "1.500,5",
                "/valores/servico",
                json!("1500.50"),
            ),
            (
                CampoNota::DescontoCondicionado,
                "10",
                "/valores/desconto_condicionado",
                json!("10"),
            ),
            (
                CampoNota::Irrf,
                "15,5",
                "/valores/federal/retencao_irrf",
                json!("15.50"),
            ),
            (
                CampoNota::Csll,
                "1",
                "/valores/federal/retencao_csll",
                json!("1"),
            ),
            (
                CampoNota::Previdencia,
                "2",
                "/valores/federal/retencao_cp",
                json!("2"),
            ),
        ];
        for (campo, digitado, ponteiro, esperado) in casos {
            let mut d = DadosNota::novo("21/11/2026");
            d.definir(campo, digitado);
            let p = montar_pedido(&d, &ctx);
            assert_eq!(
                p.pointer(ponteiro),
                Some(&esperado),
                "{campo:?} em {ponteiro}"
            );
        }
    }

    #[test]
    fn campos_do_pis_cofins_e_do_ibscbs_vao_para_o_lugar_certo() {
        let mut d = DadosNota::novo("21/11/2026");
        for (c, t) in [
            (CampoNota::PisCst, "01"),
            (CampoNota::PisBase, "100"),
            (CampoNota::PisAliquotaPis, "0,65"),
            (CampoNota::PisAliquotaCofins, "3"),
            (CampoNota::PisTipoRetencao, "1"),
            (CampoNota::CindOp, "030101"),
            (CampoNota::ClassTrib, "200003"),
        ] {
            d.definir(c, t);
        }
        d.ind_final = Some(1);
        let p = montar_pedido(&d, &ctx_do_caso());
        let pis = &p["valores"]["federal"]["pis_cofins"];
        assert_eq!(pis["cst"], "01");
        assert_eq!(pis["base"], "100");
        assert_eq!(pis["aliquota_pis"], "0.65");
        assert_eq!(pis["aliquota_cofins"], "3");
        assert_eq!(pis["tipo_retencao"], 1);
        assert_eq!(p["ibscbs"]["cind_op"], "030101");
        assert_eq!(p["ibscbs"]["cclass_trib"], "200003");
        assert_eq!(p["ibscbs"]["cst"], "200");
        assert_eq!(p["ibscbs"]["ind_final"], 1);
    }

    #[test]
    fn escolhas_de_iss_e_o_regime_decidem_o_que_entra() {
        let mut d = DadosNota::novo("21/11/2026");
        d.tributacao = 4;
        d.retencao = 2;
        d.definir(CampoNota::AliquotaIss, "2,5");
        d.definir(CampoNota::PercentualSn, "4,5");
        d.definir(CampoNota::PercentualFederal, "13,45");
        // ME/EPP: so' o percentual do Simples; sem aliquota porque nao ha incidencia
        let p = montar_pedido(&d, &ctx_do_caso());
        assert_eq!(
            p["valores"]["municipal"],
            json!({"tributacao": 4, "retencao": 2})
        );
        assert_eq!(p["valores"]["total"], json!({"percentual_sn": "4.5"}));
        // nao optante: so' os percentuais por esfera, e a aliquota aparece quando ha incidencia
        let mut config = config_do_caso();
        config.emitente.op_simples = 1;
        config.emitente.regime_apuracao = 0;
        let ctx = Contexto::de_config(&config, "2026-11-21T15:00:00Z".parse().unwrap());
        d.tributacao = 1;
        let p = montar_pedido(&d, &ctx);
        assert_eq!(p["valores"]["municipal"]["aliquota"], "2.5");
        assert_eq!(
            p["valores"]["total"],
            json!({"percentual_federal": "13.45"})
        );
        // MEI: nem aliquota nem totais
        config.emitente.op_simples = 2;
        let ctx = Contexto::de_config(&config, "2026-11-21T15:00:00Z".parse().unwrap());
        let p = montar_pedido(&d, &ctx);
        assert!(p["valores"]["municipal"].get("aliquota").is_none());
        assert_eq!(p["valores"]["total"], json!({}));
    }

    #[test]
    fn sem_tomador_nao_leva_o_grupo_nem_o_endereco() {
        let mut d = nota_do_caso();
        d.sem_tomador = true;
        let p = montar_pedido(&d, &ctx_do_caso());
        assert!(p.get("tomador").is_none());
    }

    #[test]
    fn contexto_usa_o_dia_de_brasilia_e_a_serie_do_emitente() {
        // 01:00 UTC ainda e' o dia anterior em Brasilia
        let c = Contexto::de_config(&config_do_caso(), "2026-11-22T01:00:00Z".parse().unwrap());
        assert_eq!(c.hoje_iso, "2026-11-21");
        assert_eq!(c.hoje(), "21/11/2026");
        assert_eq!(c.serie(), "1");
        let mut config = config_do_caso();
        config.emitente.serie = "7".into();
        assert_eq!(Contexto::de_config(&config, Utc::now()).serie(), "7");
        assert_eq!(c.documento_do_emitente(), "11222333000181");
    }

    #[test]
    fn congelar_fixa_numero_serie_e_data_hora() {
        let c = congelar(
            &nota_do_caso(),
            &ctx_do_caso(),
            42,
            "2026-11-21T12:00:00-03:00",
        );
        assert_eq!(c.numero, 42);
        assert_eq!(c.serie, "1");
        assert_eq!(c.pedido["numero"], 42);
        assert_eq!(c.pedido["serie"], "1");
        assert_eq!(c.pedido["emissao"], "2026-11-21T12:00:00-03:00");
        assert_eq!(c.pedido["tomador"]["nome"], "Maria de Teste");
    }

    #[test]
    fn percentual_digitado_vira_decimal_com_ponto() {
        assert_eq!(percentual("2,5").as_deref(), Some("2.5"));
        assert_eq!(percentual("4.50").as_deref(), Some("4.50"));
        assert_eq!(percentual("3,").as_deref(), Some("3"));
        assert_eq!(percentual(",5").as_deref(), Some("0.5"));
        assert_eq!(percentual(""), None);
        assert_eq!(percentual(","), None);
    }
}
