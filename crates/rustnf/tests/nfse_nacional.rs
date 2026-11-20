// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! NFS-e Nacional (parte pura): `tests/casos/nfse_nacional.json`. A montagem tem de gerar o MESMO XML byte a byte,
//! recusar com o MESMO campo e a MESMA regra, e dar o mesmo Id/valor/data/texto/chave.

use rustnf::nfse::base::{data_hora, dinheiro, texto, Txt};
use rustnf::nfse::{ctribnac, eventos, id_dps, ler_evento, ler_nfse, mensagens, montar, Dps};
use serde_json::Value;

fn casos() -> Value {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/casos/nfse_nacional.json");
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn txt(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        o => o.to_string(),
    }
}

#[test]
fn contrato_id_dps() {
    for c in casos()["id_dps"].as_array().unwrap() {
        let r = id_dps(
            &txt(&c["cmun"]),
            &txt(&c["inscricao"]),
            &txt(&c["serie"]),
            &txt(&c["numero"]),
        );
        match c.get("ok") {
            Some(ok) => assert_eq!(r.unwrap(), txt(ok), "{c}"),
            None => assert!(r.is_err(), "{c}"),
        }
    }
}

#[test]
fn contrato_dinheiro() {
    for c in casos()["dinheiro"].as_array().unwrap() {
        let r = dinheiro(
            &txt(&c["entrada"]),
            "x",
            c["inteiros"].as_u64().unwrap() as usize,
            true,
        );
        match c.get("ok") {
            Some(ok) => assert_eq!(r.unwrap(), txt(ok), "{c}"),
            None => assert!(r.is_err(), "{c}"),
        }
    }
}

#[test]
fn contrato_data_hora() {
    for c in casos()["data_hora"].as_array().unwrap() {
        let e = txt(&c["entrada"]);
        let iso = match c["offset_min"].as_i64() {
            None => e,
            Some(m) => format!(
                "{e}{}{:02}:{:02}",
                if m < 0 { '-' } else { '+' },
                m.abs() / 60,
                m.abs() % 60
            ),
        };
        let r = data_hora(Some(&iso), "dhEmi");
        match c.get("ok") {
            Some(ok) => assert_eq!(r.unwrap(), txt(ok), "{c}"),
            None => assert!(r.is_err(), "{c}"),
        }
    }
}

#[test]
fn contrato_latin1() {
    for c in casos()["latin1"].as_array().unwrap() {
        assert_eq!(
            texto(&txt(&c["entrada"]), "x", Txt::max(300)).unwrap(),
            txt(&c["saida"]),
            "{c}"
        );
    }
}

#[test]
fn contrato_chave_evento() {
    for c in casos()["chave_evento"].as_array().unwrap() {
        let r = eventos::chave_do_evento(&txt(&c["chave"]));
        match c.get("ok") {
            Some(ok) => assert_eq!(r.unwrap(), txt(ok), "{c}"),
            None => assert!(r.is_err(), "{c}"),
        }
    }
}

#[test]
fn contrato_ctribnac() {
    for c in casos()["ctribnac"].as_array().unwrap() {
        let r = ctribnac::normalizar(&txt(&c["entrada"]), true);
        match c.get("ok") {
            Some(ok) => assert_eq!(r.unwrap(), txt(ok), "{c}"),
            None => assert!(r.is_err(), "{c}"),
        }
    }
    assert_eq!(ctribnac::codigos().len(), 338); // Anexo B v1.01-20260122 (gov.br/nfse)
    assert!(ctribnac::descricao("250101").starts_with("Funerais"));
    assert_eq!(
        ctribnac::buscar("manutencao veiculos", 5)[0].ctribnac,
        "140101"
    );
    assert_eq!(ctribnac::normalizar("999998", false).unwrap(), "999998");
    assert!(ctribnac::normalizar("140100", false)
        .unwrap_err()
        .mensagem
        .contains("termina em 00"));
}

#[test]
fn contrato_dps_mesmo_xml() {
    let cs = casos();
    for c in cs["dps"]
        .as_array()
        .unwrap()
        .iter()
        .chain(cs["dps_nt009"].as_array().unwrap())
    {
        let d: Dps = serde_json::from_value(c["entrada"].clone()).unwrap();
        let xml = montar(&d).unwrap_or_else(|e| panic!("{}: {e}", c["nome"]));
        assert_eq!(xml, txt(&c["xml"]), "caso {}", c["nome"]);
    }
}

#[test]
fn contrato_dps_mesmo_erro() {
    let cs = casos();
    for c in cs["dps_erro"]
        .as_array()
        .unwrap()
        .iter()
        .chain(cs["dps_nt009_erro"].as_array().unwrap())
    {
        let d: Dps = serde_json::from_value(c["entrada"].clone()).unwrap();
        let e = montar(&d).expect_err(&format!("caso {} deveria recusar", c["nome"]));
        assert_eq!(e.campo, txt(&c["campo"]), "caso {}: {e}", c["nome"]);
        let regra = txt(&c["regra"]);
        if !regra.is_empty() {
            assert!(e.mensagem.contains(&regra), "caso {}: {e}", c["nome"]);
        }
    }
}

/// vServ = pCopropriedade x vTotOper meio-para-par (Anexo I PRODREST): 0,05 x 50% = 0,025 -> 0,02.
#[test]
fn locacao_proporcional_meio_para_par() {
    let cs = casos();
    let base = cs["dps_nt009"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["nome"] == "locacao")
        .unwrap()["entrada"]
        .clone();
    for (total, esperado) in [
        ("0.05", "0.02"),
        ("0.07", "0.04"),
        ("0.03", "0.02"),
        ("2000", "1000.00"),
    ] {
        let mut d = base.clone();
        d["ibscbs"]["imovel"]["locacao"]["valor_total"] = Value::from(total);
        d["ibscbs"]["imovel"]["locacao"]["desconto_incondicionado"] = Value::Null;
        d["ibscbs"]["imovel"]["unidades"] = serde_json::json!([{"cib": "12345678"}]);
        d["valores"] = serde_json::json!({"servico": "9999"});
        let d: Dps = serde_json::from_value(d).unwrap();
        let e = montar(&d).expect_err("vServ errado");
        assert!(
            e.mensagem.contains("E0433") && e.mensagem.contains(&format!("({esperado})")),
            "{total}: {e}"
        );
    }
}

#[test]
fn contrato_nt009_tem_casos_e_o_padrao_e_producao() {
    let cs = casos();
    assert!(cs["dps_nt009"].as_array().unwrap().len() >= 9);
    assert!(cs["dps_nt009_erro"].as_array().unwrap().len() >= 28);
    assert_eq!(Dps::default().leiaute, rustnf::nfse::dps::LEIAUTE_PRODUCAO);
}

#[test]
fn contrato_eventos() {
    for c in casos()["eventos"].as_array().unwrap() {
        let (tipo, chave, autor, motivo, texto, dh) = (
            txt(&c["tipo"]),
            txt(&c["chave"]),
            txt(&c["autor"]),
            txt(&c["motivo"]),
            txt(&c["texto"]),
            txt(&c["dh_evento"]),
        );
        let r = eventos::montar(&eventos::DadosEvento {
            tipo: &tipo,
            chave: &chave,
            autor: &autor,
            motivo: &motivo,
            justificativa: &texto,
            ambiente: c["ambiente"].as_u64().unwrap() as u8,
            dh_evento: Some(&dh),
            ver_aplic: "rustnf",
        });
        match c.get("erro") {
            None => {
                let p = r.unwrap_or_else(|e| panic!("{c}: {e}"));
                assert_eq!(p.xml, txt(&c["xml"]), "{}", c["tipo"]);
                assert_eq!(p.id, txt(&c["id"]));
                assert_eq!(ler_evento(&p.xml).unwrap().tipo, txt(&c["tipo"]));
            }
            Some(campo) => assert_eq!(r.unwrap_err().campo, txt(campo), "{c}"),
        }
    }
}

#[test]
fn dhemi_sem_fracao_e_sem_fuso_e_brasilia() {
    assert_eq!(
        data_hora(Some("2026-10-02T10:00:00.999999999"), "x").unwrap(),
        "2026-10-02T10:00:00-03:00"
    );
    let agora = data_hora(None, "x").unwrap();
    assert!(
        agora.ends_with("-03:00") && agora.len() == 25 && !agora.contains('.'),
        "{agora}"
    );
}

#[test]
fn mensagens_nas_duas_formas() {
    let (e, a) = mensagens(
        &serde_json::json!({"erros": [{"codigo": "E0014", "descricao": "ja existe"}],
                                                "alertas": [{"Codigo": "A1"}]}),
    );
    assert_eq!(e[0].to_string(), "E0014: ja existe");
    assert_eq!(a[0].codigo, "A1");
    let (e, _) = mensagens(
        &serde_json::json!({"erro": {"codigo": "E2", "descricao": "Chave nao encontrada",
                                                        "complemento": "x"}}),
    );
    assert_eq!(e[0].to_string(), "E2: Chave nao encontrada -- x");
}

#[test]
fn mensagens_erros_em_texto_e_vazios() {
    // `erros` como TEXTO e' recusa; null, lista vazia e texto em branco nao
    let (e, _) = mensagens(&serde_json::json!({"erros": "DPS invalida"}));
    assert_eq!(
        e.iter().map(ToString::to_string).collect::<Vec<_>>(),
        ["DPS invalida"]
    );
    for vazio in [
        serde_json::Value::Null,
        serde_json::json!([]),
        serde_json::json!(""),
        serde_json::json!("   "),
    ] {
        assert!(
            mensagens(&serde_json::json!({"erros": vazio.clone()}))
                .0
                .is_empty(),
            "{vazio}"
        );
    }
    let (e, _) =
        mensagens(&serde_json::json!({"erros": "", "erro": {"codigo": "E2", "descricao": "x"}}));
    assert_eq!(e[0].codigo, "E2");
}

#[test]
fn leitor_recusa_dtd() {
    assert!(ler_nfse("<!DOCTYPE a [<!ENTITY x \"y\">]><NFSe/>").is_err());
}

#[test]
fn pis_cofins_zerados_saem_omitidos() {
    // vPis/vCofins zerados saem omitidos ([0..1] no XSD)
    let c = casos()["dps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| !c["entrada"]["valores"]["federal"]["pis_cofins"].is_null())
        .unwrap()
        .clone();
    let mut e = c["entrada"].clone();
    e["valores"]["federal"]["pis_cofins"]["valor_pis"] = Value::from("0");
    e["valores"]["federal"]["pis_cofins"]["valor_cofins"] = Value::from(0);
    let dps: Dps = serde_json::from_value(e).unwrap();
    let xml = montar(&dps).unwrap();
    assert!(
        !xml.contains("<vPis>") && !xml.contains("<vCofins>"),
        "{xml}"
    );
    assert!(
        xml.contains("<pAliqPis>0.65</pAliqPis><pAliqCofins>3.00</pAliqCofins><tpRetPisCofins>")
    );
}

/// O Anexo B oficial nao traz aliquota de ISS: o dado embutido nao pode carregar uma de origem nao oficial.
#[test]
fn lista_nacional_nao_traz_aliquota() {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("dados/ctribnac.json");
    let v: Value = serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap();
    for c in v["codigos"].as_array().unwrap() {
        let chaves: Vec<&str> = c.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(chaves, ["ctribnac", "item_lc116", "descricao"], "{c}");
    }
}
