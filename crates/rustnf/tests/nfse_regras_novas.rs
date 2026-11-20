// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Regras que o rustnf confere antes de enviar e que a origem do porte nao tinha: o `totTrib` por regime do
//! Simples Nacional (Anexo I v1.01: E0710, E0712, E0713) e as dicas de retorno (E0037, E0312).
//! Golden: `tests/casos/nfse_regras_novas.json`, cada recusa com a contraprova do caso valido.

use rustnf::nfse::{mensagens, montar, Dps};
use serde_json::Value;

fn ler(nome: &str) -> Value {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/casos")
        .join(nome);
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

/// A DPS-base (primeira de `dps`) com o regime e o grupo `total` trocados.
fn dps_com(op: u64, total: &Value) -> Dps {
    let mut e = ler("nfse_nacional.json")["dps"][0]["entrada"].clone();
    e["prestador"]["op_simples"] = Value::from(op);
    e["prestador"]["apuracao_sn"] = Value::Null;
    if op == 3 {
        e["prestador"]["apuracao_sn"] = Value::from(1);
    }
    e["valores"]["total"] = total.clone();
    serde_json::from_value(e).unwrap()
}

#[test]
fn tot_trib_recusa_a_forma_vedada_ao_regime() {
    let g = ler("nfse_regras_novas.json");
    let casos = g["tot_trib_recusa"].as_array().unwrap();
    assert_eq!(casos.len(), 4);
    for c in casos {
        let d = dps_com(c["op_simples"].as_u64().unwrap(), &c["total"]);
        let e = montar(&d).expect_err(&format!("{}: deveria recusar", c["nome"]));
        assert_eq!(e.campo, c["campo"].as_str().unwrap(), "{}: {e}", c["nome"]);
        let regra = format!("regra {} do SEFIN", c["regra"].as_str().unwrap());
        assert!(e.mensagem.contains(&regra), "{}: {e}", c["nome"]);
    }
}

#[test]
fn tot_trib_contraprova_a_forma_permitida_passa() {
    let g = ler("nfse_regras_novas.json");
    let casos = g["tot_trib_contraprova"].as_array().unwrap();
    assert_eq!(casos.len(), 8);
    for c in casos {
        let d = dps_com(c["op_simples"].as_u64().unwrap(), &c["total"]);
        let xml = montar(&d).unwrap_or_else(|e| panic!("{}: {e}", c["nome"]));
        let esperado = c["ok"].as_str().unwrap();
        assert!(
            xml.contains(esperado),
            "{}: faltou {esperado} em {xml}",
            c["nome"]
        );
        assert_eq!(xml.matches("<totTrib>").count(), 1, "{}", c["nome"]);
    }
}

#[test]
fn dicas_de_retorno_so_nos_codigos_de_configuracao() {
    let g = ler("nfse_regras_novas.json");
    for c in g["dicas"].as_array().unwrap() {
        let (erros, _) = mensagens(&c["json"]);
        let texto = erros[0].to_string();
        for trecho in c["contem"].as_array().unwrap() {
            assert!(
                texto.contains(trecho.as_str().unwrap()),
                "{}: faltou {trecho} em {texto}",
                c["nome"]
            );
        }
    }
    for c in g["dicas_contraprova"].as_array().unwrap() {
        let (erros, _) = mensagens(&c["json"]);
        assert_eq!(
            erros[0].to_string(),
            c["esperado"].as_str().unwrap(),
            "{}",
            c["nome"]
        );
        assert_eq!(erros[0].dica(), "");
    }
}

#[test]
fn agora_com_folga_fica_no_passado_e_no_formato_do_leiaute() {
    use chrono::{DateTime, Utc};
    use rustnf::nfse::base::{agora_com_folga, data_hora};
    let antes = Utc::now();
    let s = agora_com_folga(60);
    let depois = Utc::now();
    assert_eq!(s.len(), 25, "{s}");
    assert!(s.ends_with("-03:00") && !s.contains('.'), "{s}");
    // volta a passar pelo leitor do leiaute sem mudar
    assert_eq!(data_hora(Some(&s), "x").unwrap(), s);
    let t = DateTime::parse_from_rfc3339(&s)
        .unwrap()
        .with_timezone(&Utc);
    // truncado em segundos: entre (antes - 60 s - 1 s) e (depois - 60 s)
    assert!(
        t >= antes - chrono::Duration::seconds(61) && t <= depois - chrono::Duration::seconds(60),
        "{s}"
    );
    // folga zero = agora
    let z = DateTime::parse_from_rfc3339(&agora_com_folga(0))
        .unwrap()
        .with_timezone(&Utc);
    assert!((Utc::now() - z).num_seconds() <= 2);
}
