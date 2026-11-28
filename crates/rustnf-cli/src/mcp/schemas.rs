// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! `inputSchema` (JSON Schema 2020-12, o padrao do MCP) de cada ferramenta, escrito a mao a partir das structs da
//! biblioteca. Os testes comparam as chaves de cada schema com as chaves que a biblioteca serializa, para o
//! schema nao se afastar dela.

use serde_json::{json, Map, Value};

fn texto(descricao: &str) -> Value {
    json!({"type": "string", "description": descricao})
}

fn valor(descricao: &str) -> Value {
    json!({"type": ["string", "number"], "description": descricao})
}

fn inteiro(descricao: &str) -> Value {
    json!({"type": "integer", "description": descricao})
}

fn objeto(descricao: &str) -> Value {
    json!({"type": "object", "description": descricao})
}

fn objeto_com(descricao: &str, propriedades: Vec<(&str, Value)>) -> Value {
    let props: Map<String, Value> = propriedades
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
    json!({"type": "object", "description": descricao, "properties": props})
}

fn endereco() -> Value {
    objeto_com(
        "Endereco nacional (cmun e cep) ou do exterior (pais, cod_postal, cidade, estado).",
        vec![
            ("logradouro", texto("Logradouro.")),
            ("numero", texto("Numero.")),
            ("bairro", texto("Bairro.")),
            ("cmun", texto("Codigo IBGE do municipio (7 digitos).")),
            ("cep", texto("CEP, 8 digitos.")),
            ("complemento", texto("Complemento.")),
            ("pais", texto("Codigo ISO do pais (endereco no exterior).")),
            ("cod_postal", texto("Codigo postal (exterior).")),
            ("cidade", texto("Cidade (exterior).")),
            ("estado", texto("Estado ou provincia (exterior).")),
        ],
    )
}

fn pessoa_props() -> Vec<(&'static str, Value)> {
    vec![
        ("nome", texto("Nome ou razao social.")),
        (
            "documento",
            texto("CPF ou CNPJ, so' digitos (ou 12 alfanumericos + 2 digitos)."),
        ),
        ("nif", texto("NIF (estrangeiro sem CPF/CNPJ).")),
        (
            "sem_nif",
            inteiro("0, 1 ou 2: motivo de nao informar o NIF."),
        ),
        ("caepf", texto("CAEPF.")),
        ("im", texto("Inscricao municipal.")),
        ("endereco", endereco()),
        ("fone", texto("Telefone.")),
        ("email", texto("E-mail.")),
    ]
}

fn pessoa(descricao: &str) -> Value {
    objeto_com(descricao, pessoa_props())
}

fn prestador() -> Value {
    let mut props = pessoa_props();
    props.push(("op_simples", inteiro("1 nao optante, 2 MEI, 3 ME/EPP.")));
    props.push((
        "apuracao_sn",
        inteiro("Apuracao do Simples Nacional (so' ME/EPP)."),
    ));
    props.push(("regime_especial", inteiro("Regime especial de tributacao.")));
    objeto_com(
        "Prestador: pessoa mais o regime no Simples Nacional.",
        props,
    )
}

fn servico() -> Value {
    objeto_com(
        "Servico prestado.",
        vec![
            (
                "ctribnac",
                valor("Codigo de tributacao nacional (6 digitos, Lista Nacional)."),
            ),
            ("descricao", texto("Descricao do servico.")),
            (
                "cmun_prestacao",
                texto("Municipio da prestacao (IBGE); vazio = o do emissor."),
            ),
            (
                "pais_prestacao",
                texto("Pais da prestacao (servico no exterior)."),
            ),
            ("ctribmun", texto("Codigo de tributacao municipal.")),
            ("cnbs", texto("Codigo NBS.")),
            ("cod_interno", texto("Codigo interno do contribuinte.")),
            (
                "comercio_exterior",
                objeto("Comercio exterior: modo, vinculo, moeda, mec_prestador, mec_tomador, mov_temp_bens, mdic; no leiaute de producao valor_moeda, num_di e num_re; no nt009 num_fatura, valor_fatura, num_duimp, num_due e contrato_cambio."),
            ),
            (
                "obra",
                objeto("Obra: codigo, cib, endereco, inscricao_imobiliaria."),
            ),
            (
                "evento",
                objeto("Atividade de evento: nome, inicio, fim, identificacao, endereco; no leiaute nt009 tambem cmun (municipio do evento, so' nos subitens 12.13 e 17.10)."),
            ),
            ("complemento", objeto("Informacoes complementares.")),
        ],
    )
}

fn valores() -> Value {
    let municipal = objeto_com(
        "Tributacao do ISSQN.",
        vec![
            ("tributacao", inteiro("tribISSQN: 1 a 4.")),
            ("retencao", inteiro("tpRetISSQN: 1 a 3.")),
            ("aliquota", valor("pAliq, em pontos percentuais.")),
            ("pais_resultado", texto("Pais do resultado (exportacao).")),
            ("imunidade", inteiro("tpImunidade.")),
            ("suspensao_tipo", inteiro("tpSusp.")),
            (
                "suspensao_processo",
                texto("Numero do processo da suspensao."),
            ),
            ("beneficio_numero", texto("Numero do beneficio municipal.")),
            (
                "beneficio_reducao_valor",
                valor("Reducao do beneficio, em valor."),
            ),
            (
                "beneficio_reducao_percentual",
                valor("Reducao do beneficio, em percentual."),
            ),
        ],
    );
    let total = objeto_com(
        "Total aproximado de tributos (Lei 12.741): UMA forma.",
        vec![
            ("valor_federal", valor("Valor federal.")),
            ("valor_estadual", valor("Valor estadual.")),
            ("valor_municipal", valor("Valor municipal.")),
            ("percentual_federal", valor("Percentual federal.")),
            ("percentual_estadual", valor("Percentual estadual.")),
            ("percentual_municipal", valor("Percentual municipal.")),
            (
                "percentual_sn",
                valor("Percentual do Simples Nacional (pTotTribSN)."),
            ),
        ],
    );
    objeto_com(
        "Valores e tributacao.",
        vec![
            ("servico", valor("Valor do servico (vServ).")),
            ("recebido", valor("Valor recebido pelo intermediario.")),
            ("desconto_incondicionado", valor("Desconto incondicionado.")),
            ("desconto_condicionado", valor("Desconto condicionado.")),
            (
                "deducao",
                objeto("Deducao/reducao: percentual OU valor OU documentos/ajustes."),
            ),
            ("municipal", municipal),
            (
                "federal",
                objeto(
                    "Tributacao federal: pis_cofins, retencao_cp, retencao_irrf, retencao_csll.",
                ),
            ),
            ("total", total),
        ],
    )
}

fn ibscbs() -> Value {
    objeto_com(
        "Classificacao do IBS/CBS (a Sefin Nacional calcula os valores).",
        vec![
            ("cind_op", texto("cIndOp (Anexo VII).")),
            ("cst", texto("CST do IBS/CBS.")),
            ("cclass_trib", texto("cClassTrib.")),
            ("ind_dest", inteiro("Indicador de destinatario.")),
            ("fin", inteiro("finNFSe.")),
            ("ind_final", inteiro("indFinal.")),
            ("tp_oper", inteiro("tpOper.")),
            (
                "refs_nfse",
                json!({"type": "array", "items": {"type": "string"}, "description": "Chaves de NFS-e referenciadas."}),
            ),
            ("tp_ente_gov", inteiro("tpEnteGov.")),
            (
                "destinatario",
                pessoa("Destinatario, quando difere do tomador."),
            ),
            ("imovel_cib", texto("CIB do imovel (leiaute de producao).")),
            (
                "imovel_endereco",
                objeto("Endereco do imovel (leiaute de producao)."),
            ),
            ("imovel_inscricao", texto("Inscricao imobiliaria.")),
            (
                "reembolsos",
                json!({"type": "array", "items": {"type": "object"}, "description": "Documentos de reembolso (leiaute de producao)."}),
            ),
            ("imovel", objeto("Grupo imovel (leiaute nt009).")),
            (
                "bens_moveis",
                json!({"type": "array", "items": {"type": "object"}, "description": "Bens moveis (leiaute nt009)."}),
            ),
            ("ccred_pres", texto("cCredPres.")),
            (
                "cst_regular",
                texto("CST do regime regular (optante do Simples)."),
            ),
            (
                "cclass_trib_regular",
                texto("cClassTrib do regime regular."),
            ),
            (
                "dif_uf",
                valor("Diferimento do IBS estadual, em percentual."),
            ),
            (
                "dif_mun",
                valor("Diferimento do IBS municipal, em percentual."),
            ),
            ("dif_cbs", valor("Diferimento da CBS, em percentual.")),
        ],
    )
}

/// Schema da DPS (`rustnf::nfse::Dps`).
#[must_use]
pub fn dps() -> Value {
    let Value::Object(mut s) = objeto_com(
        "Pedido de emissao: a DPS. Os valores aceitam numero ou texto decimal.",
        vec![
            ("prestador", prestador()),
            ("servico", servico()),
            ("valores", valores()),
            ("cmun_emissor", texto("Codigo IBGE do municipio emissor (cLocEmi).")),
            ("numero", valor("nDPS: sem zero a esquerda, ate 15 digitos; nunca reaproveitar.")),
            ("serie", valor("Serie da DPS, 1 a 5 digitos (padrao 1).")),
            ("competencia", texto("Data do fato gerador, AAAA-MM-DD.")),
            ("ambiente", inteiro("1 producao, 2 producao restrita. Na emissao vale o do servidor.")),
            ("emitente", inteiro("1 prestador, 2 tomador, 3 intermediario.")),
            ("tomador", pessoa("Tomador do servico.")),
            ("intermediario", pessoa("Intermediario.")),
            ("substituicao", objeto("Substituicao de NFS-e: chave, motivo, texto.")),
            ("ibscbs", ibscbs()),
            ("motivo_emissao_ti", inteiro("cMotivoEmisTI (emissao pelo tomador/intermediario).")),
            ("chave_rejeitada", texto("Chave da NFS-e rejeitada que esta DPS refaz.")),
            ("emissao", texto("dhEmi em ISO 8601; sem fuso vale Brasilia. Vazio = agora, com folga do relogio.")),
            ("ver_aplic", texto("Versao do aplicativo emissor.")),
            ("ctribnac_estrito", json!({"type": "boolean", "description": "Confere o cTribNac contra a Lista Nacional."})),
            ("leiaute", json!({"type": "string", "enum": ["producao", "nt009"], "description": "Leiaute do XML: producao (o vigente, padrao) ou nt009 (Releases 1 e 2 da NT 009, o da Producao Restrita de 24/11/2026)."})),
        ],
    ) else {
        return Value::Null;
    };
    s.insert(
        "required".into(),
        json!(["prestador", "servico", "valores", "cmun_emissor", "numero"]),
    );
    Value::Object(s)
}

/// Schema de `emitir_nfse`: a DPS, sem exigir o que a configuracao do servidor pode completar (os dados do
/// prestador e o municipio emissor, pelo emitente; o numero, pela pasta de notas). O que faltar de verdade e'
/// recusado pela montagem da DPS, com o campo.
#[must_use]
pub fn dps_de_emissao() -> Value {
    let mut s = dps();
    s["required"] = json!(["servico", "valores"]);
    s
}

/// Schema de `calcular_cbs_ibs` (`nucleo::Operacao`).
#[must_use]
pub fn operacao() -> Value {
    let base = json!({
        "type": "object",
        "description": "Base de calculo do IBS/CBS (reforma::previa::bases).",
        "properties": {
            "ano": {"type": "integer", "minimum": 2026, "description": "Ano do fato gerador."},
            "op_simples": {"type": "integer", "enum": [1, 2, 3, 4], "description": "1 nao optante, 2 MEI, 3 optante, 4 optante pendente."},
            "situacao": {"type": "string", "enum": ["comum", "salao_parceiro", "locacao9903"]},
            "regime_apuracao": {"type": "integer", "enum": [1, 2, 3], "description": "Apuracao do IBS/CBS do optante."},
            "servico": valor("Valor do servico."),
            "desconto_incondicionado": valor("Desconto incondicionado."),
            "issqn": valor("Valor do ISSQN."),
            "pis": valor("Valor do PIS."),
            "cofins": valor("Valor da COFINS."),
            "documentos": {"type": "array", "description": "Documentos de ajuste da base.", "items": {
                "type": "object",
                "properties": {"tipo": texto("tpAjusteBC: 101 a 105, 199 ou 9."), "valor": valor("Valor do ajuste."), "descricao": texto("Obrigatoria no tipo 199.")},
                "additionalProperties": false
            }},
            "locacao": {"type": "object", "description": "Obrigatoria na situacao locacao9903.", "properties": {
                "copropriedade": valor("Percentual de copropriedade."),
                "total_operacao": valor("Valor total da operacao."),
                "desconto_total": valor("Desconto incondicionado total."),
                "ajustes": valor("Soma dos ajustes da locacao.")
            }, "additionalProperties": false}
        },
        "required": ["ano", "op_simples"],
        "additionalProperties": false
    });
    let tributos = json!({
        "type": "object",
        "description": "IBS e CBS (reforma::previa::tributos). Se \"base\" faltar e \"base\" da operacao vier, vale a base do regime regular calculada.",
        "properties": {
            "data": texto("Data do fato gerador, AAAA-MM-DD."),
            "cst": texto("CST do IBS/CBS."),
            "cclass_trib": texto("cClassTrib."),
            "base": valor("Base de calculo."),
            "aliquotas": {"type": "object", "description": "Aliquotas em pontos percentuais; vazio = calendario da LC 214/2025.", "properties": {
                "ibs_uf": valor("IBS estadual."), "ibs_mun": valor("IBS municipal."), "cbs": valor("CBS.")
            }, "additionalProperties": false},
            "publicadas": {"type": "object", "description": "Aliquotas publicadas para os anos que o calendario nao fixa."},
            "simples_nacional": {"type": "boolean", "description": "O prestador e' optante do Simples Nacional."},
            "diferimento": {"type": "object", "description": "Percentuais de diferimento (gDif).", "properties": {
                "ibs_uf": valor("pDifUF."), "ibs_mun": valor("pDifMun."), "cbs": valor("pDifCBS.")
            }, "additionalProperties": false}
        },
        "required": ["data", "cst", "cclass_trib"],
        "additionalProperties": false
    });
    json!({
        "type": "object",
        "properties": {"base": base, "tributos": tributos},
        "anyOf": [{"required": ["base"]}, {"required": ["tributos"]}],
        "additionalProperties": false
    })
}

/// Schema de `gerar_danfse`.
#[must_use]
pub fn danfse() -> Value {
    json!({
        "type": "object",
        "properties": {
            "xml": texto("XML da NFS-e autorizada."),
            "situacao": {"type": "string", "enum": ["valida", "cancelada", "substituida"], "description": "Marca d'agua; padrao valida."},
            "canhoto": {"type": "boolean", "description": "Com o canhoto de cientificacao; padrao true."}
        },
        "required": ["xml"],
        "additionalProperties": false
    })
}

/// Schema de `consultar_nfse`.
#[must_use]
pub fn consulta() -> Value {
    json!({
        "type": "object",
        "properties": {
            "chave": texto("Chave de acesso da NFS-e (50 posicoes).")
        },
        "required": ["chave"],
        "additionalProperties": false
    })
}

/// Schema de `consultar_dps`: o Id da DPS, ou o numero (e a serie, se nao for a do emitente do servidor).
#[must_use]
pub fn consulta_dps() -> Value {
    json!({
        "type": "object",
        "properties": {
            "id_dps": texto("Id da DPS (45 posicoes). Sem ele, informe o numero."),
            "serie": texto("Serie da DPS; sem ela vale a do emitente do servidor, ou 1."),
            "numero": valor("Numero da DPS; o Id sai do emitente do servidor, da serie e deste numero."),
            "com_nota": {
                "type": "boolean",
                "description": "Busca tambem a nota inteira, nao so' a chave (padrao: so' a chave)."
            }
        },
        "anyOf": [{"required": ["id_dps"]}, {"required": ["numero"]}],
        "additionalProperties": false
    })
}

/// Schema de `consultar_evento`.
#[must_use]
pub fn consulta_evento() -> Value {
    json!({
        "type": "object",
        "properties": {
            "chave": texto("Chave de acesso da NFS-e (50 posicoes)."),
            "tipo": texto("Tipo do evento (tpEvento, 6 digitos), por exemplo 101103."),
            "sequencia": {
                "type": "integer",
                "minimum": 1,
                "description": "Sequencia do evento do tipo na nota (padrao 1)."
            }
        },
        "required": ["chave", "tipo"],
        "additionalProperties": false
    })
}

/// Schema de `registrar_evento`.
#[must_use]
pub fn registro_de_evento() -> Value {
    json!({
        "type": "object",
        "properties": {
            "chave": texto("Chave de acesso da NFS-e (50 posicoes)."),
            "tipo": texto("Tipo do evento (tpEvento, 6 digitos) que o contribuinte registra, por exemplo 101103 (pedido de analise fiscal). O cancelamento (101101) tem a ferramenta cancelar_nfse."),
            "motivo": texto("Codigo do motivo, nos eventos que o tem (101103: 1 erro na emissao, 2 servico nao prestado, 9 outros)."),
            "justificativa": texto("Justificativa, nos eventos que a tem (101103: obrigatoria, de 15 a 255 caracteres; fica no documento fiscal).")
        },
        "required": ["chave", "tipo"],
        "additionalProperties": false
    })
}

/// Schema de `atualizar_situacoes`.
#[must_use]
pub fn atualizacao() -> Value {
    json!({
        "type": "object",
        "properties": {
            "de": texto("So' as notas emitidas a partir deste dia, AAAA-MM-DD (inclusive)."),
            "ate": texto("So' as notas emitidas ate este dia, AAAA-MM-DD (inclusive)."),
            "situacao": {
                "type": "string",
                "enum": ["autorizada", "cancelada", "substituida", "analise_fiscal"],
                "description": "So' as notas que a pasta tem nesta situacao."
            },
            "busca": texto("So' as notas que casam com o texto: nome do tomador, CPF/CNPJ dele, numero da NFS-e ou da DPS, ou uma chave de acesso."),
            "intervalo_ms": {
                "type": "integer",
                "minimum": 0,
                "description": "Intervalo entre duas consultas a Sefin, em milissegundos (padrao 200)."
            }
        },
        "additionalProperties": false
    })
}

/// Schema de `cancelar_nfse`.
#[must_use]
pub fn cancelamento() -> Value {
    json!({
        "type": "object",
        "properties": {
            "chave": texto("Chave de acesso da NFS-e (50 posicoes)."),
            "motivo": texto("Codigo do motivo: 1 erro na emissao, 2 servico nao prestado, 9 outros."),
            "justificativa": texto("Justificativa (obrigatoria), fica no documento fiscal.")
        },
        "required": ["chave", "motivo", "justificativa"],
        "additionalProperties": false
    })
}

#[cfg(test)]
mod testes {
    use std::collections::BTreeSet;

    use rustnf::nfse::{Dps, Endereco, IbsCbs, Pessoa};
    use serde_json::json;

    use super::*;

    fn chaves(v: &Value) -> BTreeSet<String> {
        v.as_object()
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default()
    }

    fn propriedades(schema: &Value, ponteiro: &str) -> BTreeSet<String> {
        let mut atual = schema;
        for parte in ponteiro.split('/').filter(|p| !p.is_empty()) {
            atual = &atual["properties"][parte];
        }
        chaves(&atual["properties"])
    }

    fn confere(schema: &Value, ponteiro: &str, serializado: &Value) {
        assert_eq!(
            propriedades(schema, ponteiro),
            chaves(serializado),
            "schema de '{ponteiro}' difere do que a biblioteca serializa"
        );
    }

    #[test]
    fn o_schema_da_dps_tem_as_mesmas_chaves_da_biblioteca() {
        let s = dps();
        let d = serde_json::to_value(Dps::default()).unwrap();
        confere(&s, "", &d);
        confere(&s, "prestador", &d["prestador"]);
        confere(&s, "servico", &d["servico"]);
        confere(&s, "valores", &d["valores"]);
        confere(&s, "valores/municipal", &d["valores"]["municipal"]);
        confere(&s, "valores/total", &d["valores"]["total"]);
        confere(
            &s,
            "tomador",
            &serde_json::to_value(Pessoa::default()).unwrap(),
        );
        confere(
            &s,
            "ibscbs",
            &serde_json::to_value(IbsCbs::default()).unwrap(),
        );
        confere(
            &s,
            "tomador/endereco",
            &serde_json::to_value(Endereco::default()).unwrap(),
        );
    }

    #[test]
    fn os_casos_da_previa_cabem_no_schema() {
        let caminho = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../rustnf/tests/casos/reforma_previa.json");
        let casos: Value =
            serde_json::from_str(&std::fs::read_to_string(caminho).unwrap()).unwrap();
        let s = operacao();
        let base = propriedades(&s, "base");
        let tributos = propriedades(&s, "tributos");
        for c in casos["bases"].as_array().unwrap() {
            for k in chaves(&c["entrada"]) {
                assert!(base.contains(&k), "base: '{k}' fora do schema");
            }
        }
        for c in casos["tributos"].as_array().unwrap() {
            for k in chaves(&c["entrada"]) {
                assert!(tributos.contains(&k), "tributos: '{k}' fora do schema");
            }
        }
    }

    #[test]
    fn todo_schema_e_objeto_com_tipo() {
        for s in [
            dps(),
            dps_de_emissao(),
            operacao(),
            danfse(),
            consulta(),
            cancelamento(),
        ] {
            assert_eq!(s["type"], json!("object"));
        }
    }

    #[test]
    fn a_emissao_nao_exige_o_que_a_configuracao_completa() {
        let (conferir, emitir) = (dps(), dps_de_emissao());
        assert_eq!(conferir["properties"], emitir["properties"]);
        let exigidos = |s: &Value| -> BTreeSet<String> {
            s["required"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap().to_string())
                .collect()
        };
        for campo in ["prestador", "cmun_emissor", "numero"] {
            assert!(exigidos(&conferir).contains(campo), "{campo}");
            assert!(!exigidos(&emitir).contains(campo), "{campo}");
        }
        assert!(exigidos(&emitir).contains("servico") && exigidos(&emitir).contains("valores"));
    }
}
