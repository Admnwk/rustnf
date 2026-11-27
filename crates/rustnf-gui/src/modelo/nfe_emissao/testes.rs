// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Testes dos textos da conferencia e do desfecho da NF-e: os erros reais da montagem viram pendencias com o que fazer,
//! e cada desfecho do envio tem o seu texto.

use rustnf::nfe::montagem::{Aviso, TipoAviso};
use rustnf::nfe::sefaz::{Desfecho, Resultado};
use rustnf_app::nfe::{montar_pedido, Emitida};
use rustnf_app::notas::nfe::{NotasNfe, Situacao};
use serde_json::json;

use super::*;
use crate::modelo::nfe_nova::testes::{config_de, contexto, dados_basicos};
use crate::modelo::nfe_nova::{pedido, ItemNfe};
use crate::testes_comuns::PastaTemporaria;

fn conferir_o_pedido(
    mexer: impl FnOnce(&mut serde_json::Value),
) -> (ConferenciaNfe, PastaTemporaria) {
    let pasta = PastaTemporaria::nova("nfe-conf");
    let config = config_de(&pasta, 1, "");
    let mut p = pedido(&dados_basicos(), &contexto(1, None)).unwrap();
    mexer(&mut p);
    let notas = NotasNfe::abrir(config.notas.pasta.trim()).unwrap();
    (
        conferencia(montar_pedido(&config, Some(&notas), p, false)),
        pasta,
    )
}

#[test]
fn a_conferencia_limpa_traz_o_total_e_o_numero() {
    let (c, _p) = conferir_o_pedido(|_| {});
    assert!(c.limpa(), "{c:?}");
    assert_eq!(c.total, "31,80");
    assert_eq!(c.numero, 1);
    assert_eq!(c.serie, "1");
    // Simples: nao ha imposto destacado
    assert!(c.impostos.is_empty(), "{:?}", c.impostos);
}

#[test]
fn erro_da_montagem_vira_pendencia_em_portugues_com_o_que_fazer() {
    type Mutacao = fn(&mut serde_json::Value);
    let mutacoes: [(&str, Mutacao); 4] = [
        ("id_dest", |p| p["id_dest"] = json!(7)),
        ("ind_pres", |p| p["ind_pres"] = json!(6)),
        ("mod_frete", |p| p["mod_frete"] = json!(7)),
        ("ncm", |p| p["itens"][0]["ncm"] = json!("1")),
    ];
    let mut recusadas = 0;
    for (nome, f) in mutacoes {
        let (c, _p) = conferir_o_pedido(f);
        if c.limpa() {
            continue;
        }
        recusadas += 1;
        assert!(!c.erros.is_empty(), "{nome}: {c:?}");
        for e in &c.erros {
            assert!(
                !e.mensagem.is_empty() && !e.como_resolver.is_empty(),
                "{nome}: {e:?}"
            );
            // nada de codigo cru no inicio da mensagem
            assert!(!e.mensagem.contains("Rejeicao:"), "{nome}: {}", e.mensagem);
            assert!(
                e.mensagem.chars().next().is_some_and(char::is_uppercase),
                "{nome}: {}",
                e.mensagem
            );
        }
    }
    assert!(
        recusadas >= 1,
        "a biblioteca não recusou nenhuma das mutacoes"
    );
}

#[test]
fn cada_rejeicao_listada_vira_uma_pendencia_so_dela() {
    let e = Erro::recusa(
        "",
        "999-Rejeicao: Destinatario com IE inválida; 888-Rejeicao: Valor do pagamento diferente do total",
    );
    let p = erros_da_recusa(&e);
    assert_eq!(p.len(), 2);
    assert_eq!(p[0].passo, PassoNfe::ParaQuem);
    assert_eq!(p[0].mensagem, "Destinatario com IE inválida");
    assert!(
        p[0].como_resolver.contains("inscrição estadual"),
        "{}",
        p[0].como_resolver
    );
    assert_eq!(p[1].passo, PassoNfe::ComoRecebeu);
    // erro que nao e' recusa tambem vira pendencia
    let p = erros_da_recusa(&Erro::Uso("informe [notas].pasta".into()));
    assert_eq!(p.len(), 1);
    // a regra desconhecida cita o codigo da SEFAZ
    let p = erros_da_recusa(&Erro::recusa("", "777-Rejeicao: Algo no item"));
    assert!(
        p[0].como_resolver.contains("regra 777"),
        "{}",
        p[0].como_resolver
    );
}

#[test]
fn o_numero_com_nota_pendente_manda_retomar() {
    let e = Erro::recusa(
        "nNF",
        "o número 5 da serie 1 está com uma NF-e pendente (chave 35): retome os pendentes para saber o que a SEFAZ fez antes de emitir",
    );
    let p = erros_da_recusa(&e);
    assert!(
        p[0].como_resolver.contains("Retomar"),
        "{}",
        p[0].como_resolver
    );
}

fn emitida(desfecho: Desfecho, situacao: Option<Situacao>) -> Emitida {
    Emitida {
        chave: "35261011222333000181550010000000051123456704".into(),
        serie: "1".into(),
        numero: 5,
        resultado: Resultado {
            desfecho,
            chave: "35261011222333000181550010000000051123456704".into(),
            recibo: None,
            xml_retorno: String::new(),
            xml_enviado: String::new(),
            recuperada: false,
            frase: "frase".into(),
            detalhe: String::new(),
        },
        avisos: vec![Aviso {
            tipo: TipoAviso::SimplesSemGIbscbs,
            item: None,
            mensagem: "x".into(),
        }],
        desconhecidos: Vec::new(),
        situacao,
    }
}

#[test]
fn cada_desfecho_do_envio_tem_o_seu_texto() {
    let d = DesfechoNfeEmissao::de(Ok(emitida(
        Desfecho::Autorizada {
            cstat: "100".into(),
            motivo: "Autorizado".into(),
            protocolo: "135260000000001".into(),
            nfe_proc: None,
        },
        Some(Situacao::Autorizada),
    )));
    match &d {
        DesfechoNfeEmissao::Autorizada {
            numero,
            serie,
            protocolo,
            guardada,
            avisos,
            ..
        } => {
            assert_eq!((numero.as_str(), serie.as_str()), ("5", "1"));
            assert_eq!(protocolo, "135260000000001");
            assert!(guardada);
            assert_eq!(avisos.len(), 1);
        }
        outro => panic!("{outro:?}"),
    }
    assert!(d.concluiu() && !d.deixa_pendente());

    let d = DesfechoNfeEmissao::de(Ok(emitida(
        Desfecho::Rejeitada {
            cstat: "233".into(),
            motivo: "Rejeicao: IE do destinatario não cadastrada".into(),
        },
        None,
    )));
    match &d {
        DesfechoNfeEmissao::Rejeitada {
            o_que_fazer,
            passo,
            numero,
            cstat,
            ..
        } => {
            assert_eq!(cstat, "233");
            assert_eq!(*passo, PassoNfe::ParaQuem);
            assert_eq!(numero, "5");
            assert!(o_que_fazer.contains("continua livre"), "{o_que_fazer}");
        }
        outro => panic!("{outro:?}"),
    }
    assert!(!d.concluiu() && !d.deixa_pendente());

    let d = DesfechoNfeEmissao::de(Ok(emitida(Desfecho::SemResposta, Some(Situacao::Pendente))));
    assert!(matches!(&d, DesfechoNfeEmissao::SemResposta { numero, .. } if numero == "5"));
    assert!(d.deixa_pendente());

    let d = DesfechoNfeEmissao::de(Ok(emitida(
        Desfecho::Indisponivel {
            cstat: "108".into(),
            motivo: "Serviço paralisado momentaneamente".into(),
        },
        Some(Situacao::Pendente),
    )));
    assert!(
        matches!(&d, DesfechoNfeEmissao::SefazFora { motivo, .. } if motivo.starts_with("Serviço"))
    );
    assert!(d.deixa_pendente());

    let d = DesfechoNfeEmissao::de(Ok(emitida(
        Desfecho::Denegada {
            cstat: "110".into(),
            motivo: "Uso Denegado".into(),
            protocolo: String::new(),
            nfe_proc: None,
        },
        Some(Situacao::Denegada),
    )));
    assert!(matches!(d, DesfechoNfeEmissao::Denegada { .. }));

    let d = DesfechoNfeEmissao::de(Err(Erro::recusa(
        "nNF",
        "o número 5 da serie 1 já foi gasto",
    )));
    assert!(matches!(&d, DesfechoNfeEmissao::Recusada { pendencias } if !pendencias.is_empty()));
    assert!(!d.deixa_pendente());
}

#[test]
fn o_epec_diz_o_prazo_que_a_aplicacao_anota_e_o_que_fazer() {
    let t = depois_do_epec();
    assert!(t.contains("168 horas"), "{t}");
    assert!(t.contains("Retomar"));
    assert!(O_QUE_E_EPEC.contains("fora do ar"));
    let por_que = por_que_nao_ha_epec(1);
    assert!(por_que.contains("1 nota sem resposta"), "{por_que}");
    assert!(por_que.contains("OUTRA nota"));
    assert!(por_que_nao_ha_epec(3).contains("3 notas sem resposta"));
    assert_eq!(
        numero_e_serie("35261011222333000181550010000000051123456704"),
        ("5".into(), "1".into())
    );
    let _ = ItemNfe::default();
}
