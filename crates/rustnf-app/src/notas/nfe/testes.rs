// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A pasta de notas da NF-e: numeracao (autorizada e denegada gastam, rejeitada nao, pulado se inutiliza), gravacao antes do
//! envio, eventos, inutilizacao e a convivencia com a pasta da NFS-e.

use rustnf::chave_dfe::{self, Montar};

use super::*;
use crate::notas::Notas;
use crate::testes_comuns::PastaTemporaria;

pub(super) fn chave(serie: u64, numero: u64, cnf: u64) -> String {
    chave_dfe::montar(&Montar {
        cuf: 35,
        aamm: "2610",
        emitente: "11222333000181",
        modelo: 55,
        serie,
        numero,
        tp_emis: 1,
        codigo: Some(cnf),
    })
    .unwrap()
}

pub(super) fn registro(serie: u64, numero: u64) -> Registro {
    Registro {
        chave: chave(serie, numero, 12_345_670 + numero),
        serie: serie.to_string(),
        numero: numero.to_string(),
        emissao: "2026-10-03T09:00:00-03:00".into(),
        valor: "31.80".into(),
        ambiente: "homologacao".into(),
        destinatario_documento: "52998224725".into(),
        destinatario_nome: "CLIENTE TESTE".into(),
        desde: 100,
        ..Registro::default()
    }
}

fn abrir(rotulo: &str) -> (PastaTemporaria, NotasNfe) {
    let p = PastaTemporaria::nova(rotulo);
    let n = NotasNfe::abrir(p.caminho().join("notas")).unwrap();
    (p, n)
}

#[test]
fn a_pasta_da_nfe_fica_dentro_da_pasta_de_notas_e_comeca_no_numero_1() {
    let (p, n) = abrir("nfe-pasta-abre");
    assert_eq!(n.pasta(), p.caminho().join("notas").join("nfe"));
    assert!(n.pasta().is_dir());
    assert_eq!(n.proximo_numero("1").unwrap(), 1);
    assert_eq!(n.proximo_numero("007").unwrap(), 1);
    assert_eq!(
        n.proximo_numero("1000").unwrap_err(),
        ErroNotas::SerieInvalida
    );
    assert_eq!(NotasNfe::abrir("").unwrap_err(), ErroNotas::Pasta);
}

#[test]
fn a_nota_assinada_e_gravada_como_pendente_e_reserva_o_numero() {
    let (_p, n) = abrir("nfe-pendente");
    let r = registro(1, 1);
    n.guardar_pendente(&r, "<NFe>assinada</NFe>").unwrap();
    let arquivo = n
        .pasta()
        .join("2026")
        .join("10")
        .join(format!("{}-NFe.xml", r.chave));
    assert_eq!(
        std::fs::read_to_string(arquivo).unwrap(),
        "<NFe>assinada</NFe>"
    );
    assert_eq!(n.registro(&r.chave).unwrap().situacao, Situacao::Pendente);
    assert_eq!(
        n.ler_assinada(&r.chave).unwrap().unwrap(),
        "<NFe>assinada</NFe>"
    );
    assert_eq!(
        n.proximo_numero("1").unwrap(),
        2,
        "a pendente conta como gasta"
    );
    assert_eq!(n.a_resolver().unwrap().len(), 1);
    // gravar a mesma pendente de novo nao duplica
    n.guardar_pendente(&r, "<NFe>assinada</NFe>").unwrap();
    assert_eq!(n.listar().unwrap().len(), 1);
}

#[test]
fn outra_chave_com_o_mesmo_numero_e_recusada() {
    let (_p, n) = abrir("nfe-ocupado");
    n.guardar_pendente(&registro(1, 5), "<NFe/>").unwrap();
    let outra = Registro {
        chave: chave(1, 5, 87_654_321),
        ..registro(1, 5)
    };
    assert_eq!(
        n.guardar_pendente(&outra, "<NFe/>").unwrap_err(),
        ErroNotas::NumeroNfeOcupado
    );
    // outra serie, mesmo numero: livre
    n.guardar_pendente(&registro(2, 5), "<NFe/>").unwrap();
    assert_eq!(n.proximo_numero("2").unwrap(), 6);
}

#[test]
fn rejeitada_nao_consome_o_numero() {
    let (_p, n) = abrir("nfe-rejeitada");
    let r = registro(1, 1);
    n.guardar_pendente(&r, "<NFe/>").unwrap();
    assert_eq!(n.proximo_numero("1").unwrap(), 2);
    n.descartar_pendente(&r.chave).unwrap();
    assert_eq!(
        n.proximo_numero("1").unwrap(),
        1,
        "rejeitada: o numero volta"
    );
    assert!(n.listar().unwrap().is_empty());
    assert!(!n
        .pasta()
        .join("2026/10")
        .join(format!("{}-NFe.xml", r.chave))
        .exists());
    // pode reenviar o mesmo numero, corrigido, com outra chave
    let corrigida = Registro {
        chave: chave(1, 1, 99_999_990),
        ..r
    };
    n.guardar_pendente(&corrigida, "<NFe/>").unwrap();
}

#[test]
fn autorizada_e_denegada_consomem_o_numero_e_nao_se_descartam() {
    let (_p, n) = abrir("nfe-consome");
    let (a, d) = (registro(1, 1), registro(1, 2));
    n.guardar_pendente(&a, "<NFe/>").unwrap();
    n.guardar_pendente(&d, "<NFe/>").unwrap();
    n.concluir(
        &a.chave,
        Situacao::Autorizada,
        Some("<nfeProc>a</nfeProc>"),
        "135260000000001",
        "100",
    )
    .unwrap();
    n.concluir(
        &d.chave,
        Situacao::Denegada,
        Some("<nfeProc>d</nfeProc>"),
        "135260000000002",
        "302",
    )
    .unwrap();
    assert_eq!(n.proximo_numero("1").unwrap(), 3);
    assert_eq!(n.ler_proc(&a.chave).unwrap(), "<nfeProc>a</nfeProc>");
    assert_eq!(n.ler_xml(&d.chave).unwrap(), "<nfeProc>d</nfeProc>");
    // a copia assinada saiu com a chegada do nfeProc
    assert_eq!(n.ler_assinada(&a.chave).unwrap(), None);
    let ra = n.registro(&a.chave).unwrap();
    assert_eq!(
        (ra.situacao, ra.protocolo.as_str(), ra.cstat.as_str()),
        (Situacao::Autorizada, "135260000000001", "100")
    );
    assert_eq!(n.registro(&d.chave).unwrap().situacao, Situacao::Denegada);
    assert_eq!(
        n.descartar_pendente(&a.chave).unwrap_err(),
        ErroNotas::NumeroNfeOcupado,
        "autorizada nao se descarta"
    );
    assert_eq!(n.a_resolver().unwrap().len(), 0);
}

#[test]
fn numero_com_outra_chave_na_sefaz_e_consumido_sem_nota_na_pasta() {
    let (_p, n) = abrir("nfe-539");
    n.consumir_numero("1", 7).unwrap();
    assert_eq!(n.proximo_numero("1").unwrap(), 8);
    // nunca volta atras
    n.consumir_numero("1", 3).unwrap();
    assert_eq!(n.proximo_numero("1").unwrap(), 8);
    assert_eq!(
        n.consumir_numero("1", 0).unwrap_err(),
        ErroNotas::NumeroInvalido
    );
}

#[test]
fn numero_pulado_se_inutiliza_e_a_faixa_aparece_no_indice() {
    let (_p, n) = abrir("nfe-inutiliza");
    for numero in [1, 4] {
        let r = registro(1, numero);
        n.guardar_pendente(&r, "<NFe/>").unwrap();
        n.concluir(
            &r.chave,
            Situacao::Autorizada,
            Some("<nfeProc/>"),
            "1",
            "100",
        )
        .unwrap();
    }
    assert_eq!(n.pulados("1").unwrap(), vec![2, 3]);
    assert_eq!(n.proximo_numero("1").unwrap(), 5);

    let faixa = Inutilizacao {
        id: "ID35261122233300018155001000000002000000003".into(),
        serie: "1".into(),
        inicio: 2,
        fim: 3,
        ano: "2026".into(),
        protocolo: "135260000000009".into(),
        situacao: Situacao::Inutilizada,
    };
    n.guardar_inutilizacao(&faixa, "<ProcInutNFe/>").unwrap();
    assert!(n.pulados("1").unwrap().is_empty());
    assert_eq!(n.inutilizacoes().unwrap(), vec![faixa.clone()]);
    assert_eq!(
        n.inutilizacoes().unwrap()[0].situacao.texto(),
        "inutilizada"
    );
    assert_eq!(
        std::fs::read_to_string(n.pasta().join("inutilizacao/2026/1-2-3-procInutNFe.xml")).unwrap(),
        "<ProcInutNFe/>"
    );
    assert_eq!(n.ocupante("1", 2).unwrap(), Some(Ocupante::Inutilizada));
    // um item por nota e um por faixa inutilizada que toca o intervalo
    assert_eq!(n.ocupados_na_faixa("1", 1, 4).unwrap().len(), 3);
    // o numero inutilizado nao se reemite
    let reemissao = Registro {
        chave: chave(1, 2, 55_555_550),
        ..registro(1, 2)
    };
    assert_eq!(
        n.guardar_pendente(&reemissao, "<NFe/>").unwrap_err(),
        ErroNotas::NumeroNfeOcupado
    );
    // e nao se inutiliza faixa com nota
    let com_nota = Inutilizacao {
        inicio: 4,
        fim: 6,
        id: "ID2".into(),
        ..faixa
    };
    assert_eq!(
        n.guardar_inutilizacao(&com_nota, "<ProcInutNFe/>")
            .unwrap_err(),
        ErroNotas::NumeroNfeOcupado
    );
}

#[test]
fn a_faixa_inutilizada_alem_do_ultimo_numero_empurra_o_proximo() {
    let (_p, n) = abrir("nfe-inutiliza-alem");
    let faixa = Inutilizacao {
        id: "ID1".into(),
        serie: "3".into(),
        inicio: 10,
        fim: 20,
        ano: "2026".into(),
        protocolo: String::new(),
        situacao: Situacao::Inutilizada,
    };
    n.guardar_inutilizacao(&faixa, "<x/>").unwrap();
    assert_eq!(n.proximo_numero("3").unwrap(), 21);
    assert_eq!(n.proximo_numero("1").unwrap(), 1, "outra serie nao muda");
}

#[test]
fn a_carta_de_correcao_numera_1_2_3_pela_pasta_e_o_cancelamento_marca_cancelada() {
    let (_p, n) = abrir("nfe-eventos");
    let r = registro(1, 1);
    n.guardar_pendente(&r, "<NFe/>").unwrap();
    n.concluir(
        &r.chave,
        Situacao::Autorizada,
        Some("<nfeProc/>"),
        "1",
        "100",
    )
    .unwrap();
    assert_eq!(n.proxima_sequencia(&r.chave, "110110").unwrap(), 1);
    for seq in 1..=3 {
        let ev = EventoGuardado {
            tipo: "110110".into(),
            sequencia: seq,
            protocolo: format!("13526000000010{seq}"),
        };
        n.guardar_evento(
            &r.chave,
            &ev,
            &format!("<procEventoNFe>{seq}</procEventoNFe>"),
            "",
        )
        .unwrap();
        assert_eq!(n.proxima_sequencia(&r.chave, "110110").unwrap(), seq + 1);
    }
    assert_eq!(n.registro(&r.chave).unwrap().situacao, Situacao::Autorizada);
    assert_eq!(
        std::fs::read_to_string(
            n.pasta()
                .join("2026/10")
                .join(format!("{}-110110-2-procEventoNFe.xml", r.chave))
        )
        .unwrap(),
        "<procEventoNFe>2</procEventoNFe>"
    );
    // o cancelamento tem a sequencia propria
    assert_eq!(n.proxima_sequencia(&r.chave, "110111").unwrap(), 1);
    let cancel = EventoGuardado {
        tipo: "110111".into(),
        sequencia: 1,
        protocolo: "135260000000200".into(),
    };
    n.guardar_evento(&r.chave, &cancel, "<procEventoNFe>c</procEventoNFe>", "")
        .unwrap();
    let r2 = n.registro(&r.chave).unwrap();
    assert_eq!(r2.situacao, Situacao::Cancelada);
    assert_eq!(r2.eventos.len(), 4);
    assert_eq!(
        n.proximo_numero("1").unwrap(),
        2,
        "cancelada segue gastando o numero"
    );
}

#[test]
fn o_epec_poe_a_nota_pendente_em_epec_com_o_protocolo_do_danfe() {
    let (_p, n) = abrir("nfe-epec");
    let r = registro(1, 1);
    n.guardar_pendente(&r, "<NFe/>").unwrap();
    let ev = EventoGuardado {
        tipo: "110140".into(),
        sequencia: 1,
        protocolo: "891260000000001".into(),
    };
    n.guardar_evento(
        &r.chave,
        &ev,
        "<procEventoNFe/>",
        "891260000000001 03/10/2026 09:00:03",
    )
    .unwrap();
    let r2 = n.registro(&r.chave).unwrap();
    assert_eq!(r2.situacao, Situacao::Epec);
    assert_eq!(r2.protocolo_epec, "891260000000001 03/10/2026 09:00:03");
    assert_eq!(
        n.a_resolver().unwrap().len(),
        1,
        "o EPEC ainda tem de ser transmitido"
    );
    assert_eq!(n.ler_assinada(&r.chave).unwrap().unwrap(), "<NFe/>");
}

#[test]
fn chave_que_nao_e_de_nfe_nunca_vira_caminho() {
    let (_p, n) = abrir("nfe-chave");
    for ruim in ["", "../../x", "1234", &"1".repeat(44), &"A".repeat(44)] {
        assert_eq!(
            n.registro(ruim).unwrap_err(),
            ErroNotas::ChaveInvalida,
            "{ruim}"
        );
        assert_eq!(n.ler_xml(ruim).unwrap_err(), ErroNotas::ChaveInvalida);
    }
    let torta = Registro {
        chave: "../../fora".into(),
        ..registro(1, 1)
    };
    assert_eq!(
        n.guardar_pendente(&torta, "<NFe/>").unwrap_err(),
        ErroNotas::ChaveInvalida
    );
    // a chave de 50 posicoes da NFS-e tambem nao serve aqui
    let nfse = chave_dfe::montar_nfse(3550308, "11222333000181", 1, "2610", Some(1), 2).unwrap();
    assert_eq!(n.registro(&nfse).unwrap_err(), ErroNotas::ChaveInvalida);
}

#[test]
fn gravacao_atomica_nao_deixa_temporario_nem_trava() {
    let (_p, n) = abrir("nfe-atomico");
    let r = registro(1, 1);
    n.guardar_pendente(&r, "<NFe/>").unwrap();
    n.concluir(
        &r.chave,
        Situacao::Autorizada,
        Some("<nfeProc/>"),
        "1",
        "100",
    )
    .unwrap();
    let mut achados = Vec::new();
    let mut pilha = vec![n.pasta().to_path_buf()];
    while let Some(d) = pilha.pop() {
        for e in std::fs::read_dir(d).unwrap() {
            let e = e.unwrap().path();
            if e.is_dir() {
                pilha.push(e);
            } else {
                achados.push(e.file_name().unwrap().to_string_lossy().into_owned());
            }
        }
    }
    assert!(
        achados
            .iter()
            .all(|a| !a.ends_with(".tmp") && a != ".trava"),
        "{achados:?}"
    );
}

#[test]
fn a_pasta_da_versao_0_4_0_continua_lida_e_a_nfe_nao_mexe_nela() {
    let p = PastaTemporaria::nova("nfe-pasta-040");
    let raiz = p.caminho().join("notas");
    std::fs::create_dir_all(&raiz).unwrap();
    // o indice e o estado como a 0.4.0 os gravava (so' autorizada e cancelada)
    let ch = chave_dfe::montar_nfse(3550308, "11222333000181", 42, "2610", Some(1), 2).unwrap();
    let indice = format!(
        "{{\n  \"versao\": 1,\n  \"notas\": [\n    {{\n      \"chave\": \"{ch}\",\n      \"numero\": \"42\",\n      \"serie\": \"1\",\n      \"numero_dps\": \"42\",\n      \"emissao\": \"2026-10-02T10:00:00-03:00\",\n      \"valor_servico\": \"150.00\",\n      \"situacao\": \"cancelada\",\n      \"tomador_documento\": \"52998224725\",\n      \"tomador_nome\": \"Cliente\"\n    }}\n  ]\n}}\n"
    );
    let estado = "{\n  \"versao\": 1,\n  \"series\": {\n    \"1\": 42\n  }\n}\n";
    std::fs::write(raiz.join("indice.json"), &indice).unwrap();
    std::fs::write(raiz.join("estado.json"), estado).unwrap();

    let nfse = Notas::abrir(&raiz).unwrap();
    assert_eq!(
        nfse.listar().unwrap()[0].situacao,
        crate::notas::Situacao::Cancelada
    );
    assert_eq!(nfse.proximo_numero("1").unwrap(), 43);

    // a NF-e entra na mesma pasta, trabalha e nao toca nos arquivos da NFS-e
    let n = NotasNfe::abrir(&raiz).unwrap();
    let r = registro(1, 1);
    n.guardar_pendente(&r, "<NFe/>").unwrap();
    n.concluir(
        &r.chave,
        Situacao::Autorizada,
        Some("<nfeProc/>"),
        "1",
        "100",
    )
    .unwrap();
    assert_eq!(
        std::fs::read_to_string(raiz.join("indice.json")).unwrap(),
        indice
    );
    assert_eq!(
        std::fs::read_to_string(raiz.join("estado.json")).unwrap(),
        estado
    );
    assert_eq!(nfse.listar().unwrap().len(), 1);
    assert_eq!(
        nfse.proximo_numero("1").unwrap(),
        43,
        "a numeracao da DPS e a da NF-e sao separadas"
    );
    assert_eq!(n.proximo_numero("1").unwrap(), 2);

    // e o `reindexar` da NFS-e nao toma a pasta da NF-e por uma pasta de notas
    let feito = nfse.reindexar().unwrap();
    assert_eq!(feito.ignorados, 0);
    assert_eq!(n.listar().unwrap().len(), 1);
}
