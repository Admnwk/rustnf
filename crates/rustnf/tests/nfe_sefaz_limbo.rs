// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Varredura das notas no limbo: as regras puras (espera, quem esta devido, estado novo, aviso) e a rodada sobre um
//! cliente falso e sobre o cliente de verdade com transporte falso.

mod nfe_sefaz_comum;

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::{TimeZone, Utc};
use nfe_sefaz_comum::*;
use rustnf::nfe::sefaz::limbo::{
    aviso, decidir, devidas, espera, proxima, varrer, Acao, Estado, NotaLocal, Politica, Sefaz,
};
use rustnf::nfe::sefaz::resposta::nfe_proc;
use rustnf::nfe::sefaz::{ClienteSefaz, Config, Desfecho, ErroSefaz, Resultado};
use rustnf::nfe::vigencias::Ambiente::Homologacao;

fn nota(chave: &str, estado: Estado, desde: i64) -> NotaLocal {
    NotaLocal {
        chave: chave.into(),
        estado,
        desde,
        tentativas: 0,
        ultima: None,
        nfe_assinada: String::new(),
    }
}

#[test]
fn espera_cresce_pelo_fator_ate_o_teto() {
    let p = Politica::default();
    assert_eq!(espera(0, &p), 60);
    assert_eq!(espera(1, &p), 120);
    assert_eq!(espera(2, &p), 240);
    assert_eq!(espera(3, &p), 480);
    assert_eq!(espera(6, &p), 3600);
    assert_eq!(espera(1000, &p), 3600);
    assert_eq!(espera(i64::MAX, &p), 3600);
}

#[test]
fn fator_1_nao_cresce_e_nao_trava_com_contador_gigante() {
    let p = Politica {
        fator: 1,
        ..Politica::default()
    };
    assert_eq!(espera(1, &p), 120);
    // o contador corrompido nao pode fazer a rodada rodar para sempre
    assert_eq!(espera(i64::MAX, &p), 120);
    let teto_menor = Politica {
        fator: 1,
        espera_inicial: 100,
        espera_max: 100,
        ..Politica::default()
    };
    assert_eq!(espera(i64::MAX, &teto_menor), 100);
}

#[test]
fn a_politica_confere() {
    assert!(Politica::default().validar().is_ok());
    for ruim in [
        Politica {
            primeira_apos: 0,
            ..Politica::default()
        },
        Politica {
            espera_inicial: -1,
            ..Politica::default()
        },
        Politica {
            max_por_rodada: 0,
            ..Politica::default()
        },
        Politica {
            fator: 0,
            ..Politica::default()
        },
        Politica {
            espera_max: 10,
            ..Politica::default()
        },
    ] {
        assert!(ruim.validar().is_err(), "{ruim:?}");
    }
}

#[test]
fn proxima_e_devidas_ordenadas_so_do_limbo() {
    let p = Politica::default();
    let mut a = nota("A", Estado::Pendente, 1000);
    let mut b = nota("B", Estado::Processando, 500);
    let c = nota("C", Estado::Autorizada, 0);
    let d = nota("D", Estado::Pendente, 100_000);
    a.tentativas = 1;
    a.ultima = Some(2000);
    b.tentativas = 0;
    assert_eq!(proxima(&a, &p), 2000 + 120);
    assert_eq!(proxima(&b, &p), 500 + 60);
    let todas = [a, b, c, d];
    // agora = 2200: A (devida em 2120) e B (devida em 560); C nao e' do limbo; D ainda nao
    let v: Vec<&str> = devidas(&todas, 2200, &p)
        .iter()
        .map(|n| n.chave.as_str())
        .collect();
    assert_eq!(v, ["B", "A"]);
    // o limite por rodada
    let pouco = Politica {
        max_por_rodada: 1,
        ..p
    };
    assert_eq!(devidas(&todas, 2200, &pouco).len(), 1);
    assert!(devidas(&todas, 0, &p).is_empty());
}

#[test]
fn contador_enorme_nao_estoura_a_conta_do_instante() {
    let p = Politica::default();
    let mut n = nota("A", Estado::Pendente, i64::MAX - 1);
    n.tentativas = i64::MAX;
    n.ultima = Some(i64::MAX - 5);
    assert_eq!(proxima(&n, &p), i64::MAX);
    assert!(aviso(&n, 0, &p, Estado::Pendente).is_none());
}

#[test]
fn estado_novo_pelo_desfecho() {
    let aut = Desfecho::Autorizada {
        cstat: "100".into(),
        motivo: String::new(),
        protocolo: "1".into(),
        nfe_proc: None,
    };
    assert_eq!(decidir(Estado::Pendente, &aut, true), Estado::Autorizada);
    let rej = Desfecho::Rejeitada {
        cstat: "225".into(),
        motivo: String::new(),
    };
    assert_eq!(decidir(Estado::Pendente, &rej, true), Estado::Rejeitada);
    let usado = Desfecho::NumeroJaUsado {
        motivo: String::new(),
        chave_existente: None,
        situacao_existente: None,
    };
    assert_eq!(decidir(Estado::Pendente, &usado, true), Estado::Rejeitada);
    let proc_ = Desfecho::EmProcessamento { recibo: "1".into() };
    assert_eq!(decidir(Estado::Pendente, &proc_, true), Estado::Processando);
    assert_eq!(
        decidir(Estado::Pendente, &Desfecho::NaoRecebida, true),
        Estado::Pendente
    );
    assert_eq!(
        decidir(Estado::Pendente, &Desfecho::NaoRecebida, false),
        Estado::SemXml
    );
    assert_eq!(
        decidir(Estado::Processando, &Desfecho::SemResposta, true),
        Estado::Processando
    );
    let indis = Desfecho::Indisponivel {
        cstat: "108".into(),
        motivo: String::new(),
    };
    assert_eq!(decidir(Estado::Pendente, &indis, true), Estado::Pendente);
}

#[test]
fn aviso_depois_de_30_minutos_ou_sem_a_nfe_guardada() {
    let p = Politica::default();
    let n = nota("CH", Estado::Pendente, 0);
    assert!(aviso(&n, 1799, &p, Estado::Pendente).is_none());
    let a = aviso(&n, 1800, &p, Estado::Pendente).unwrap();
    assert!(a.contains("CH") && a.contains("30 min"), "{a}");
    assert!(aviso(&n, 99_999, &p, Estado::Autorizada).is_none());
    let sem = aviso(&n, 0, &p, Estado::SemXml).unwrap();
    assert!(sem.contains("nao foi guardada"), "{sem}");
}

// ------------------------------------------------------------------------------------------------ varredura

/// Cliente falso: cada consulta e cada envio tira a proxima resposta.
#[derive(Default)]
struct Roteiro {
    consultas: Mutex<Vec<Result<Resultado, ErroSefaz>>>,
    envios: Mutex<Vec<Result<Resultado, ErroSefaz>>>,
    consultado: Mutex<Vec<(String, bool)>>,
    enviado: Mutex<Vec<String>>,
}

#[async_trait]
impl Sefaz for Roteiro {
    async fn consultar(&self, chave: &str, nfe: Option<&str>) -> Result<Resultado, ErroSefaz> {
        self.consultado
            .lock()
            .unwrap()
            .push((chave.into(), nfe.is_some()));
        self.consultas.lock().unwrap().remove(0)
    }
    async fn autorizar(&self, nfe: &str) -> Result<Resultado, ErroSefaz> {
        self.enviado.lock().unwrap().push(nfe.into());
        self.envios.lock().unwrap().remove(0)
    }
}

fn res(d: Desfecho, chave: &str) -> Resultado {
    Resultado {
        desfecho: d,
        chave: chave.into(),
        recibo: None,
        xml_retorno: String::new(),
        xml_enviado: String::new(),
        recuperada: false,
        frase: String::new(),
        detalhe: String::new(),
    }
}

#[tokio::test]
async fn a_rodada_consulta_so_as_devidas_e_atualiza_o_estado() {
    let p = Politica::default();
    let aut = Desfecho::Autorizada {
        cstat: "100".into(),
        motivo: String::new(),
        protocolo: "1".into(),
        nfe_proc: None,
    };
    let r = Arc::new(Roteiro::default());
    r.consultas.lock().unwrap().push(Ok(res(aut, "A")));
    r.consultas
        .lock()
        .unwrap()
        .push(Ok(res(Desfecho::SemResposta, "B")));
    let notas = vec![
        nota("A", Estado::Pendente, 0),
        nota("B", Estado::Processando, 10),
        nota("C", Estado::Autorizada, 0),
        nota("D", Estado::Pendente, 10_000),
    ];
    let passos = varrer(&*r, &notas, 3600, &p).await;
    assert_eq!(passos.len(), 2);
    assert_eq!(passos[0].chave, "A");
    assert_eq!(passos[0].estado, Estado::Autorizada);
    assert_eq!(passos[0].acao, Acao::Consultou);
    assert_eq!((passos[0].tentativas, passos[0].ultima), (1, 3600));
    assert!(passos[0].resultado.is_some());
    // B segue no limbo e, com mais de 30 minutos, ganha aviso
    assert_eq!(passos[1].estado, Estado::Processando);
    assert!(passos[1]
        .aviso
        .as_deref()
        .unwrap()
        .contains("sem confirmacao"));
    assert_eq!(r.consultado.lock().unwrap().len(), 2);
    assert!(r.enviado.lock().unwrap().is_empty());
}

#[tokio::test]
async fn nao_consta_com_a_nfe_guardada_reenvia_a_mesma_e_sem_ela_so_avisa() {
    let p = Politica::default();
    let aut = Desfecho::Autorizada {
        cstat: "100".into(),
        motivo: String::new(),
        protocolo: "1".into(),
        nfe_proc: None,
    };
    let r = Arc::new(Roteiro::default());
    r.consultas
        .lock()
        .unwrap()
        .push(Ok(res(Desfecho::NaoRecebida, "A")));
    r.consultas
        .lock()
        .unwrap()
        .push(Ok(res(Desfecho::NaoRecebida, "B")));
    r.envios.lock().unwrap().push(Ok(res(aut, "A")));
    let mut a = nota("A", Estado::Pendente, 0);
    a.nfe_assinada = "<NFe>a</NFe>".into();
    let b = nota("B", Estado::Pendente, 1);
    let passos = varrer(&*r, &[a, b], 100_000, &p).await;
    assert_eq!(passos[0].acao, Acao::Reenviou);
    assert_eq!(passos[0].estado, Estado::Autorizada);
    assert_eq!(*r.enviado.lock().unwrap(), ["<NFe>a</NFe>"]);
    // B nao tem a NFe: nada se reenvia no escuro
    assert_eq!(passos[1].acao, Acao::Consultou);
    assert_eq!(passos[1].estado, Estado::SemXml);
    assert!(passos[1]
        .aviso
        .as_deref()
        .unwrap()
        .contains("nao foi guardada"));
    assert_eq!(r.enviado.lock().unwrap().len(), 1);
    // a consulta da que tem a NFe leva a NFe (para montar o nfeProc)
    assert_eq!(r.consultado.lock().unwrap()[0], ("A".to_string(), true));
    assert_eq!(r.consultado.lock().unwrap()[1], ("B".to_string(), false));
}

#[tokio::test]
async fn erro_de_uso_vira_aviso_e_a_rodada_segue() {
    let p = Politica::default();
    let aut = Desfecho::Autorizada {
        cstat: "100".into(),
        motivo: String::new(),
        protocolo: "1".into(),
        nfe_proc: None,
    };
    let r = Arc::new(Roteiro::default());
    r.consultas.lock().unwrap().push(Err(ErroSefaz {
        campo: "chNFe".into(),
        mensagem: "chave de acesso invalida".into(),
    }));
    r.consultas.lock().unwrap().push(Ok(res(aut, "B")));
    let passos = varrer(
        &*r,
        &[
            nota("A", Estado::Pendente, 0),
            nota("B", Estado::Pendente, 1),
        ],
        100_000,
        &p,
    )
    .await;
    assert_eq!(passos[0].acao, Acao::Nada);
    assert_eq!(passos[0].estado, Estado::Pendente);
    assert!(passos[0].resultado.is_none());
    assert!(passos[0]
        .aviso
        .as_deref()
        .unwrap()
        .contains("chave de acesso invalida"));
    assert_eq!(passos[1].estado, Estado::Autorizada);
}

#[tokio::test]
async fn o_cliente_de_verdade_e_a_porta_da_varredura() {
    // transporte falso: a consulta diz 217 e o reenvio autoriza a MESMA nota guardada
    let (xml, chave) = nfe(71, 2);
    let f = Falso::novo(vec![
        (
            CON,
            ok(
                CON,
                &ret_sit(
                    &chave,
                    "217",
                    "Rejeicao: NF-e nao consta na base de dados da SEFAZ",
                    "",
                ),
            ),
        ),
        (AUT, ok(AUT, &ret_envi_sinc(&prot_ok(&chave)))),
    ]);
    let c = ClienteSefaz::com_transporte(Config::novo("SP", Homologacao), f.clone())
        .unwrap()
        .com_relogio(|| Utc.with_ymd_and_hms(2026, 10, 3, 12, 0, 0).unwrap());
    let mut n = nota(&chave, Estado::Pendente, 0);
    n.nfe_assinada = xml.clone();
    let passos = varrer(&c, &[n], 100_000, &Politica::default()).await;
    assert_eq!(passos[0].acao, Acao::Reenviou);
    assert_eq!(passos[0].estado, Estado::Autorizada);
    let r = passos[0].resultado.as_ref().unwrap();
    assert_eq!(
        r.nfe_proc(),
        Some(nfe_proc(&xml, &prot_ok(&chave)).as_str())
    );
    let ch = f.chamadas();
    assert!(ch[0].corpo.contains("<consSitNFe") && ch[1].corpo.contains(&xml));
}
