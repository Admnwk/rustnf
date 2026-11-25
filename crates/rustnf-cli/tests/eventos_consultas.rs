// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Os comandos de consulta de DPS, consulta e registro de evento e atualizacao das notas, executados em memoria
//! contra o transporte falso (a linha de comando por processo so' chega a Sefin com o cliente real, que o teste nao
//! usa): o que cada um faz, o que devolve e com que codigo de saida.

mod comum;

use clap::Parser;
use comum::*;
use rustnf::chave_dfe;
use rustnf_app::config::Efetiva;
use rustnf_app::notas::Notas;
use rustnf_cli::cli::{
    atualizar_com, consultar_com, evento_consultar_com, evento_registrar_com, AcaoEvento,
    AcaoNotas, Cli, Comando,
};
use rustnf_cli::erro::Erro;

fn analisar(args: &[&str]) -> Comando {
    let mut todos = vec!["rustnf"];
    todos.extend_from_slice(args);
    Cli::try_parse_from(todos).unwrap().comando
}

fn efetiva() -> Efetiva {
    Efetiva::de(config_de_exemplo(), None, &|_| None).unwrap()
}

#[tokio::test]
async fn consultar_pelo_id_da_dps_e_por_serie_e_numero() {
    let ch = chave();
    let falso = Falso::novo(vec![
        ("GET", format!("/dps/{ID_DPS}"), dps_achada(ID_DPS, &ch)),
        ("GET", format!("/dps/{ID_DPS}"), dps_achada(ID_DPS, &ch)),
        ("GET", format!("/nfse/{ch}"), nota(&ch)),
    ]);
    let emissor = emissor_falso(falso.clone());
    let Comando::Consultar(a) = analisar(&["consultar", "--dps", ID_DPS]) else {
        panic!("nao e' consultar");
    };
    consultar_com(emissor.as_ref(), &a, &efetiva())
        .await
        .unwrap();
    // pelo numero: o [emitente] do arquivo de exemplo da municipio, CNPJ e serie
    let Comando::Consultar(a) = analisar(&["consultar", "--numero", "42", "--com-nota", "--json"])
    else {
        panic!("nao e' consultar");
    };
    consultar_com(emissor.as_ref(), &a, &efetiva())
        .await
        .unwrap();
    let urls: Vec<String> = falso.chamadas().into_iter().map(|c| c.1).collect();
    assert_eq!(urls.len(), 3, "{urls:?}");
    assert!(urls[1].ends_with(&format!("/dps/{ID_DPS}")), "{urls:?}");
    assert!(urls[2].ends_with(&format!("/nfse/{ch}")), "{urls:?}");
}

#[tokio::test]
async fn dps_sem_nota_sai_com_4_e_sem_emitente_para_antes_da_rede() {
    let falso = Falso::novo(vec![]);
    let emissor = emissor_falso(falso.clone());
    let Comando::Consultar(a) = analisar(&["consultar", "--dps", ID_DPS]) else {
        panic!("nao e' consultar");
    };
    let e = consultar_com(emissor.as_ref(), &a, &efetiva())
        .await
        .unwrap_err();
    assert!(matches!(e, Erro::NaoEncontrada(_)), "{e:?}");
    assert_eq!(e.codigo_saida(), 4);
    // serie e numero sem o [emitente]: erro de entrada (2), nada sai
    let vazia = Efetiva::de(
        rustnf_app::config::Config::de_texto("").unwrap(),
        None,
        &|_| None,
    )
    .unwrap();
    let Comando::Consultar(a) = analisar(&["consultar", "--serie", "1", "--numero", "42"]) else {
        panic!("nao e' consultar");
    };
    let antes = falso.chamadas().len();
    let e = consultar_com(emissor.as_ref(), &a, &vazia)
        .await
        .unwrap_err();
    assert_eq!(e.codigo_saida(), 2, "{e:?}");
    assert_eq!(falso.chamadas().len(), antes);
}

#[tokio::test]
async fn evento_consultar_acha_nao_acha_e_confere() {
    let ch = chave();
    let outra = chave_dfe::montar_nfse(3550308, "11222333000181", 43, "2610", Some(1), 2).unwrap();
    let falso = Falso::novo(vec![
        (
            "GET",
            format!("/nfse/{ch}/eventos/101103/1"),
            evento_consultado(&ch, "101103", 1),
        ),
        (
            "GET",
            format!("/nfse/{outra}/eventos/101103/1"),
            evento_consultado(&ch, "101103", 1),
        ),
    ]);
    let emissor = emissor_falso(falso);
    let uso = |chave: &str, extra: &[&str]| {
        let mut args = vec!["evento", "consultar", "--chave", chave, "--tipo", "101103"];
        args.extend_from_slice(extra);
        let Comando::Evento {
            acao: AcaoEvento::Consultar(a),
        } = analisar(&args)
        else {
            panic!("nao e' evento consultar");
        };
        a
    };
    // a sequencia padrao e' 1
    let a = uso(&ch, &["--json"]);
    assert_eq!(a.sequencia, 1);
    evento_consultar_com(emissor.as_ref(), &a).await.unwrap();
    // sequencia que nao existe: nao encontrado, saida 4
    let e = evento_consultar_com(emissor.as_ref(), &uso(&ch, &["--sequencia", "2"]))
        .await
        .unwrap_err();
    assert!(matches!(e, Erro::NaoEncontrada(_)), "{e:?}");
    assert_eq!(e.codigo_saida(), 4);
    // a Sefin devolveu o evento de outra nota: nunca sucesso
    let e = evento_consultar_com(emissor.as_ref(), &uso(&outra, &[]))
        .await
        .unwrap_err();
    assert!(matches!(e, Erro::Rede(_)), "{e:?}");
}

#[tokio::test]
async fn evento_registrar_marca_a_nota_e_recusa_o_que_nao_e_do_contribuinte() {
    let p = Pasta::nova();
    let notas = Notas::abrir(p.caminho("notas")).unwrap();
    let ch = guardar_nota_do_danfse(&notas);
    let falso = Falso::novo(vec![(
        "POST",
        "/eventos".into(),
        evento_registrado_de(&ch, "101103"),
    )]);
    let emissor = emissor_falso(falso.clone());
    let uso = |tipo: &str, extra: &[&str]| {
        let mut args = vec![
            "evento",
            "registrar",
            "--chave",
            ch.as_str(),
            "--tipo",
            tipo,
        ];
        args.extend_from_slice(extra);
        let Comando::Evento {
            acao: AcaoEvento::Registrar(a),
        } = analisar(&args)
        else {
            panic!("nao e' evento registrar");
        };
        a
    };
    let a = uso(
        "101103",
        &[
            "--motivo",
            "1",
            "--justificativa",
            "Valor digitado errado na nota",
        ],
    );
    evento_registrar_com(emissor.as_ref(), &a, Some(&notas))
        .await
        .unwrap();
    assert_eq!(
        notas.listar().unwrap()[0].situacao.texto(),
        "em analise fiscal"
    );
    // cancelamento e eventos do fisco: recusa (3) antes de qualquer chamada
    let antes = falso.chamadas().len();
    for tipo in ["101101", "105104", "105102", "305101"] {
        let e = evento_registrar_com(emissor.as_ref(), &uso(tipo, &[]), Some(&notas))
            .await
            .unwrap_err();
        assert_eq!(e.codigo_saida(), 3, "{tipo}: {e:?}");
    }
    assert_eq!(falso.chamadas().len(), antes);
    // motivo fora da regra: recusa da biblioteca, nada sai
    let e = evento_registrar_com(emissor.as_ref(), &uso("101103", &["--motivo", "7"]), None)
        .await
        .unwrap_err();
    assert_eq!(e.codigo_saida(), 3, "{e:?}");
    assert_eq!(falso.chamadas().len(), antes);
}

#[tokio::test]
async fn notas_atualizar_mostra_o_que_mudou_e_sai_com_4_se_houve_falha() {
    let p = Pasta::nova();
    let notas = Notas::abrir(p.caminho("notas")).unwrap();
    let ch = guardar_nota_do_danfse(&notas);
    let uso = |extra: &[&str]| {
        let mut args = vec!["notas", "atualizar", "--intervalo-ms", "0"];
        args.extend_from_slice(extra);
        let Comando::Notas {
            acao: AcaoNotas::Atualizar(a),
        } = analisar(&args)
        else {
            panic!("nao e' notas atualizar");
        };
        a
    };
    // a Sefin mostra o cancelamento: a nota muda e a saida e' 0
    let falso = Falso::novo(vec![(
        "GET",
        format!("/nfse/{ch}/eventos/101101/1"),
        evento_consultado(&ch, "101101", 1),
    )]);
    atualizar_com(emissor_falso(falso).as_ref(), &uso(&["--json"]), &notas)
        .await
        .unwrap();
    assert_eq!(notas.listar().unwrap()[0].situacao.texto(), "cancelada");
    // agora a Sefin nao mostra evento nenhum: divergencia, saida 4 e a pasta fica como estava
    let falso = Falso::novo(vec![("GET", format!("/nfse/{ch}"), nota(&ch))]);
    let e = atualizar_com(emissor_falso(falso).as_ref(), &uso(&[]), &notas)
        .await
        .unwrap_err();
    assert!(matches!(e, Erro::Rede(_)), "{e:?}");
    assert_eq!(e.codigo_saida(), 4);
    assert_eq!(notas.listar().unwrap()[0].situacao.texto(), "cancelada");
    // filtro que nao pega nota nenhuma: nada a conferir, saida 0
    let falso = Falso::novo(vec![]);
    atualizar_com(
        emissor_falso(falso.clone()).as_ref(),
        &uso(&["--situacao", "autorizada"]),
        &notas,
    )
    .await
    .unwrap();
    assert!(falso.chamadas().is_empty());
    // data fora do formato: entrada invalida (2)
    let e = atualizar_com(
        emissor_falso(Falso::novo(vec![])).as_ref(),
        &uso(&["--de", "ontem"]),
        &notas,
    )
    .await
    .unwrap_err();
    assert_eq!(e.codigo_saida(), 2, "{e:?}");
}

#[test]
fn serie_sem_numero_e_chave_com_dps_nao_se_combinam() {
    for args in [
        vec!["consultar", "--serie", "1"],
        vec!["consultar", "--chave", "1", "--dps", "2"],
        vec!["consultar", "--dps", "2", "--numero", "3"],
        vec!["consultar", "--chave", "1", "--com-nota"],
        vec!["consultar"],
    ] {
        let mut todos = vec!["rustnf"];
        todos.extend_from_slice(&args);
        assert!(Cli::try_parse_from(todos).is_err(), "{args:?}");
    }
}
