// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Montagem da NF-e (modelo 55): `tests/casos/nfe_montar*.json`. Cada caso traz o pedido, o XML esperado, a chave, os
//! totais e a lista de rejeicoes do `conferir`; cada erro traz o campo (e a rejeicao, quando ha) que a montagem tem de
//! apontar. O XML esperado dos casos foi validado no XSD oficial da NF-e 4.00 (pacote PL_010h v1.03) fora do
//! repositorio.

use std::collections::BTreeMap;
use std::path::PathBuf;

use rustnf::nfe::montagem::{
    campos_conhecidos, campos_desconhecidos, conferir, montar, nota_de_json, nota_de_json_estrito,
};
use rustnf::nfe::{conferir as conferir_lida, ler};
use serde_json::{json, Value};

fn pasta_casos() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/casos")
}

/// Todos os arquivos `nfe_montar*.json`, em ordem.
fn arquivos() -> Vec<(String, Value)> {
    let mut v: Vec<(String, Value)> = std::fs::read_dir(pasta_casos())
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            let n = p.file_name().unwrap().to_string_lossy();
            n.starts_with("nfe_montar") && n.ends_with(".json")
        })
        .map(|p| {
            let nome = p.file_name().unwrap().to_string_lossy().to_string();
            let c: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
            (nome, c)
        })
        .collect();
    v.sort_by(|a, b| a.0.cmp(&b.0));
    v
}

fn secao<'a>(c: &'a Value, nome: &str) -> &'a [Value] {
    c[nome].as_array().map_or(&[], Vec::as_slice)
}

fn lista(v: &Value) -> Vec<String> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap().to_string())
        .collect()
}

#[test]
fn os_casos_montam_o_xml_a_chave_e_os_totais_esperados() {
    let mut n = 0;
    for (arq, c) in arquivos() {
        for caso in secao(&c, "casos") {
            let nome = format!("{arq}:{}", caso["nome"].as_str().unwrap());
            let nota = nota_de_json(&caso["entrada"]).unwrap_or_else(|e| panic!("{nome}: {e}"));
            let m = montar(&nota).unwrap_or_else(|e| panic!("{nome}: {e} [{}]", e.campo));
            assert_eq!(m.xml, caso["xml"].as_str().unwrap(), "{nome}: xml");
            assert_eq!(m.chave, caso["chave"].as_str().unwrap(), "{nome}: chave");
            let totais: BTreeMap<String, String> =
                serde_json::from_value(caso["totais"].clone()).unwrap();
            assert_eq!(m.totais, totais, "{nome}: totais");
            assert_eq!(
                conferir(&nota),
                lista(&caso["conferir"]),
                "{nome}: conferir"
            );
            if let Some(esperados) = caso.get("avisos") {
                let obtidos: Vec<String> = m
                    .avisos
                    .iter()
                    .map(|a| {
                        let item = a.item.map_or("-".to_string(), |i| i.to_string());
                        format!("{}:{item}", a.tipo.codigo())
                    })
                    .collect();
                assert_eq!(obtidos, lista(esperados), "{nome}: avisos");
            }
            n += 1;
        }
    }
    assert_eq!(n, 202);
}

#[test]
fn as_regras_devolvem_a_lista_de_rejeicoes_esperada() {
    let mut n = 0;
    for (arq, c) in arquivos() {
        for caso in secao(&c, "regras") {
            let nome = format!("{arq}:{}", caso["nome"].as_str().unwrap());
            let nota = nota_de_json(&caso["entrada"]).unwrap_or_else(|e| panic!("{nome}: {e}"));
            assert_eq!(conferir(&nota), lista(&caso["conferir"]), "{nome}");
            n += 1;
        }
    }
    assert_eq!(n, 7);
}

#[test]
fn cada_erro_aponta_o_campo_e_a_rejeicao_esperados() {
    let mut n = 0;
    for (arq, c) in arquivos() {
        for caso in secao(&c, "erros") {
            let nome = format!("{arq}:{}", caso["nome"].as_str().unwrap());
            let r = nota_de_json(&caso["entrada"]).and_then(|nota| montar(&nota));
            let Err(e) = r else {
                panic!("{nome}: a montagem devia recusar");
            };
            assert_eq!(e.campo, caso["campo"].as_str().unwrap(), "{nome}: {e}");
            if let Some(regra) = caso["regra"].as_str() {
                assert_eq!(e.regra, regra, "{nome}: {e}");
            }
            n += 1;
        }
    }
    assert_eq!(n, 331);
}

/// O XML montado volta pela leitura que ja existe: a chave, os itens, os totais e as partes conferem, e a conferencia
/// interna (chave x `ide`, totais x soma dos itens) nao acha divergencia.
#[test]
fn o_xml_montado_volta_pela_leitura() {
    let mut n = 0;
    for (arq, c) in arquivos() {
        for caso in secao(&c, "casos") {
            let nome = format!("{arq}:{}", caso["nome"].as_str().unwrap());
            let nota = nota_de_json(&caso["entrada"]).unwrap();
            let m = montar(&nota).unwrap();
            let lida = ler(m.xml.as_bytes()).unwrap_or_else(|e| panic!("{nome}: {e}"));
            assert_eq!(lida.chave, m.chave, "{nome}");
            assert!(lida.chave_erro.is_none(), "{nome}: chave");
            assert_eq!(lida.itens.len(), nota.itens.len(), "{nome}: itens");
            assert_eq!(lida.emit.documento, nota.emit.documento, "{nome}: emit");
            assert_eq!(
                lida.total.v("vNF").to_string(),
                m.totais["vNF"],
                "{nome}: vNF"
            );
            assert_eq!(lida.ide.numero as i64, nota.numero, "{nome}: nNF");
            assert_eq!(lida.versao, "4.00", "{nome}");
            // a unica divergencia que a leitura acusa e' o aviso da nota de homologacao
            let mut divergencias = conferir_lida(&lida);
            divergencias.retain(|d| !d.starts_with("nota de homologacao"));
            assert!(divergencias.is_empty(), "{nome}: {divergencias:?}");
            n += 1;
        }
    }
    assert_eq!(n, 202);
}

fn pedido_basico() -> Value {
    json!({
        "modelo": 55, "serie": 1, "numero": 10, "emissao": "2026-10-03T09:00:00-03:00",
        "nat_op": "VENDA", "cmun_fg": "3550308", "codigo": 12345670,
        "emit": {"documento": "11222333000181", "nome": "LOJA DE TESTE LTDA", "crt": 1,
                 "ie": "111111111111",
                 "endereco": {"logradouro": "Rua das Flores", "numero": "100", "bairro": "Centro",
                              "cod_municipio": "3550308", "municipio": "Sao Paulo", "uf": "SP",
                              "cep": "01001000"}},
        "dest": {"documento": "52998224725", "nome": "CLIENTE TESTE", "ind_ie_dest": 9,
                 "endereco": {"logradouro": "Rua A", "numero": "1", "bairro": "Centro",
                              "cod_municipio": "3550308", "municipio": "Sao Paulo", "uf": "SP"}},
        "ind_final": 1, "ind_pres": 1,
        "itens": [{"codigo": "1", "descricao": "Cafe torrado 500g", "ncm": "09012100", "cfop": "5102",
                   "unidade": "UN", "quantidade": "2", "valor_unitario": "15.90",
                   "icms": {"codigo": "102"}, "pis": {"cst": "49"}, "cofins": {"cst": "49"}}],
        "pagamentos": [{"t_pag": "01", "valor": "31.80"}]
    })
}

#[test]
fn a_mesma_entrada_da_o_mesmo_xml() {
    let nota = nota_de_json(&pedido_basico()).unwrap();
    let a = montar(&nota).unwrap();
    let b = montar(&nota).unwrap();
    assert_eq!(a.xml, b.xml);
    assert_eq!(a.chave, b.chave);
    assert_eq!(&a.chave[35..43], "12345670");
}

#[test]
fn sem_cnf_no_pedido_a_montagem_recusa_em_vez_de_sortear() {
    let mut p = pedido_basico();
    p.as_object_mut().unwrap().remove("codigo");
    let e = montar(&nota_de_json(&p).unwrap()).unwrap_err();
    assert_eq!(e.campo, "cNF");
}

#[test]
fn o_ver_proc_padrao_e_o_da_biblioteca() {
    let m = montar(&nota_de_json(&pedido_basico()).unwrap()).unwrap();
    let esperado = format!("<verProc>rustnf {}</verProc>", env!("CARGO_PKG_VERSION"));
    assert!(m.xml.contains(&esperado), "{}", m.xml);
}

#[test]
fn modelo_65_e_recusado_com_erro_claro() {
    let mut p = pedido_basico();
    p["modelo"] = json!(65);
    let e = montar(&nota_de_json(&p).unwrap()).unwrap_err();
    assert_eq!(e.campo, "mod");
    assert!(e.mensagem.contains("NFC-e"), "{e}");
    // o `conferir` nao inventa regra de NFC-e: devolve so' o defeito de entrada
    let lista = conferir(&nota_de_json(&p).unwrap());
    assert_eq!(lista.len(), 1);
    assert!(lista[0].starts_with("entrada: modelo 65"), "{lista:?}");
}

#[test]
fn campo_desconhecido_vira_aviso_e_no_modo_estrito_recusa() {
    let mut p = pedido_basico();
    p["itens"][0]["icms"]["cstt"] = json!("00");
    p["emitt"] = json!({});
    // o pedido ainda monta (o campo e' ignorado) e a lista de avisos aponta cada um
    assert!(montar(&nota_de_json(&p).unwrap()).is_ok());
    assert_eq!(
        campos_desconhecidos(&p),
        vec!["emitt".to_string(), "itens[1].icms.cstt".to_string()]
    );
    let e = nota_de_json_estrito(&p).unwrap_err();
    assert_eq!(e.campo, "emitt");
    assert!(e.mensagem.contains("itens[1].icms.cstt"), "{e}");
    assert!(nota_de_json_estrito(&pedido_basico()).is_ok());
}

/// A documentacao do pedido cita todo campo que o pedido conhece (e o esquema nao some campo sem a doc acompanhar).
#[test]
fn a_documentacao_do_pedido_cita_todo_campo_conhecido() {
    let doc = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/NFE_PEDIDO.md"),
    )
    .unwrap();
    let faltam: Vec<String> = campos_conhecidos()
        .into_iter()
        .filter(|c| !doc.contains(&format!("`{c}`")))
        .collect();
    assert!(faltam.is_empty(), "campos sem documentacao: {faltam:?}");
}

/// Simples Nacional sem regime regular (CRT 1, sem `tpRegimeIBSCBS`): pela tabela da secao 2.3 da NT 2026.005 o item vai
/// SEM `gIBSCBS` e COM `gTribSN` (`gIBSSN` e `gCBSSN`), mesmo que o CST exija o `gIBSCBS`.
#[test]
fn simples_sem_regime_regular_leva_gtribsn_e_nao_leva_gibscbs() {
    let mut p = pedido_basico();
    p["emissao"] = json!("2026-11-20T10:00:00-03:00");
    p["itens"][0]["c_ativ_sn"] = json!("01");
    p["itens"][0]["ibscbs"] = json!({
        "cst": "000", "cclass_trib": "000001", "vbc": "31.80", "data": "2026-11-20",
        "trib_sn": {"ibs": {"p": "0.50", "v": "0.16"}, "cbs": {"p": "1.20", "v": "0.38"}}
    });
    let m = montar(&nota_de_json(&p).unwrap()).unwrap();
    assert!(!m.xml.contains("<gIBSCBS>"), "{}", m.xml);
    assert!(m.xml.contains(
        "<IBSCBS><CST>000</CST><cClassTrib>000001</cClassTrib><gTribSN><gIBSSN><pIBSSN>0.5000</pIBSSN>\
         <vIBSSN>0.16</vIBSSN></gIBSSN><gCBSSN><pCBSSN>1.2000</pCBSSN><vCBSSN>0.38</vCBSSN></gCBSSN></gTribSN></IBSCBS>"
    ));
    assert!(m.xml.contains(
        "<IBSCBSTot><vBCIBSCBS>0.00</vBCIBSCBS><gTribSN><vIBSSN>0.16</vIBSSN><vCBSSN>0.38</vCBSSN></gTribSN></IBSCBSTot>"
    ));
    // mesmo emitente, mas com o regime regular (tpRegimeIBSCBS 1): o gIBSCBS volta e o gTribSN sai
    let mut regular = p.clone();
    regular["emit"]["tp_regime_ibscbs"] = json!(1);
    regular["itens"][0]["ibscbs"]
        .as_object_mut()
        .unwrap()
        .remove("trib_sn");
    let m2 = montar(&nota_de_json(&regular).unwrap()).unwrap();
    assert!(m2.xml.contains("<gIBSCBS>"));
    assert!(!m2.xml.contains("<gTribSN>"));
}

/// Quem monta o item pela API, sem a marca do Simples sem regime regular, nao consegue gerar o XML errado: a montagem
/// recusa o `gIBSCBS` nesse regime.
#[test]
fn a_montagem_recusa_o_gibscbs_no_simples_sem_regime_regular() {
    use rustnf::reforma::calculo::{ibscbs_do_item, EntradaItem, PValor, TribSn};
    let mut p = pedido_basico();
    p["emissao"] = json!("2026-11-20T10:00:00-03:00");
    let mut nota = nota_de_json(&p).unwrap();
    let e = EntradaItem {
        cst: "000".into(),
        cclass_trib: "000001".into(),
        vbc: "31.80".into(),
        data: "2026-11-20".into(),
        trib_sn: Some(TribSn {
            ibs: Some(PValor {
                p: Some("0.50".into()),
                v: Some("0.16".into()),
            }),
            cbs: Some(PValor {
                p: Some("1.20".into()),
                v: Some("0.38".into()),
            }),
        }),
        ..EntradaItem::default()
    };
    nota.itens[0].ibscbs = Some(ibscbs_do_item(&e).unwrap());
    let err = montar(&nota).unwrap_err();
    assert_eq!(err.campo, "gIBSCBS", "{err}");
}
