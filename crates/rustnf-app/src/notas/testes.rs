// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use super::*;
use crate::testes_comuns::PastaTemporaria;

pub(super) fn chave(n: u64) -> String {
    chave_dfe::montar_nfse(3550308, "11222333000181", n, "2610", Some(1), 2).unwrap()
}

pub(super) fn registro(n: u64) -> Registro {
    Registro {
        chave: chave(n),
        numero: format!("{n}"),
        serie: "1".into(),
        numero_dps: format!("{n}"),
        emissao: "2026-10-02T10:00:00-03:00".into(),
        valor_servico: "150.00".into(),
        situacao: Situacao::Autorizada,
        tomador_documento: "52998224725".into(),
        tomador_nome: "Cliente".into(),
    }
}

fn nomes(pasta: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(pasta)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

#[test]
fn guardar_listar_e_ler() {
    let p = PastaTemporaria::nova("notas-basico");
    let notas = Notas::abrir(p.caminho().join("minhas")).unwrap();
    assert!(notas.listar().unwrap().is_empty());
    notas.guardar(&registro(42), "<NFSe>42</NFSe>").unwrap();
    notas.guardar(&registro(43), "<NFSe>43</NFSe>").unwrap();

    let esperado = notas
        .pasta()
        .join("2026")
        .join("10")
        .join(format!("{}.xml", chave(42)));
    assert_eq!(
        std::fs::read_to_string(esperado).unwrap(),
        "<NFSe>42</NFSe>"
    );
    assert_eq!(notas.ler_xml(&chave(43)).unwrap(), "<NFSe>43</NFSe>");

    let lista = notas.listar().unwrap();
    assert_eq!(lista.len(), 2);
    assert_eq!(lista[0].tomador_nome, "Cliente");
    assert_eq!(lista[0].valor_servico, "150.00");
    assert!(lista.iter().all(|r| r.situacao == Situacao::Autorizada));
    assert_eq!(
        notas.ler_xml(&chave(44)).unwrap_err(),
        ErroNotas::NaoEncontrada
    );
    // nada de temporario nem de trava sobrando
    assert_eq!(
        nomes(notas.pasta()),
        vec!["2026".to_string(), "indice.json".to_string()]
    );
}

#[test]
fn guardar_de_novo_troca_o_xml_e_cancelada_continua_cancelada() {
    let p = PastaTemporaria::nova("notas-cancelar");
    let notas = Notas::abrir(p.caminho()).unwrap();
    notas.guardar(&registro(42), "<v1/>").unwrap();
    notas.marcar_cancelada(&chave(42)).unwrap();
    assert_eq!(notas.listar().unwrap()[0].situacao, Situacao::Cancelada);
    notas.guardar(&registro(42), "<v2/>").unwrap();
    let lista = notas.listar().unwrap();
    assert_eq!(lista.len(), 1);
    assert_eq!(lista[0].situacao, Situacao::Cancelada);
    assert_eq!(notas.ler_xml(&chave(42)).unwrap(), "<v2/>");
    assert_eq!(
        notas.marcar_cancelada(&chave(99)).unwrap_err(),
        ErroNotas::NaoEncontrada
    );
}

#[test]
fn numeracao_so_avanca_quando_confirmada() {
    let p = PastaTemporaria::nova("notas-numero");
    let notas = Notas::abrir(p.caminho()).unwrap();
    assert_eq!(notas.proximo_numero("1").unwrap(), 1);
    // pedir de novo sem confirmar (nota rejeitada ou pendente) nao muda o numero
    assert_eq!(notas.proximo_numero("1").unwrap(), 1);
    notas.confirmar_numero("1", 5).unwrap();
    assert_eq!(notas.proximo_numero("1").unwrap(), 6);
    // nunca volta atras
    notas.confirmar_numero("1", 3).unwrap();
    assert_eq!(notas.proximo_numero("1").unwrap(), 6);
    // cada serie tem o seu; zeros a esquerda sao a mesma serie
    assert_eq!(notas.proximo_numero("2").unwrap(), 1);
    assert_eq!(notas.proximo_numero("001").unwrap(), 6);
    // uma nota guardada com numero maior tambem conta
    let mut r = registro(9);
    r.numero_dps = "9".into();
    notas.guardar(&r, "<x/>").unwrap();
    assert_eq!(notas.proximo_numero("1").unwrap(), 10);
}

#[test]
fn serie_e_numero_invalidos_sao_recusados() {
    let p = PastaTemporaria::nova("notas-numero-ruim");
    let notas = Notas::abrir(p.caminho()).unwrap();
    for s in ["", "abc", "123456", "../1", "-1"] {
        assert_eq!(
            notas.proximo_numero(s).unwrap_err(),
            ErroNotas::SerieInvalida
        );
    }
    assert_eq!(
        notas.confirmar_numero("1", 0).unwrap_err(),
        ErroNotas::NumeroInvalido
    );
    assert_eq!(
        notas.confirmar_numero("1", NUMERO_MAXIMO + 1).unwrap_err(),
        ErroNotas::NumeroInvalido
    );
}

#[test]
fn travessia_de_caminho_e_recusada() {
    let p = PastaTemporaria::nova("notas-travessia");
    let notas = Notas::abrir(p.caminho().join("notas")).unwrap();
    notas.guardar(&registro(42), "<ok/>").unwrap();
    let boa = chave(42);

    for ruim in [
        "../../fora".to_string(),
        format!("../{boa}"),
        format!("..\\{boa}"),
        format!("{boa}/.."),
        format!("a{}", &boa[1..]),
        format!("{}.", &boa[..49]),
        boa[..49].to_string(),
        String::new(),
        "..".to_string(),
    ] {
        assert_eq!(
            notas.ler_xml(&ruim).unwrap_err(),
            ErroNotas::ChaveInvalida,
            "{ruim}"
        );
        assert_eq!(
            notas.marcar_cancelada(&ruim).unwrap_err(),
            ErroNotas::ChaveInvalida,
            "{ruim}"
        );
        let mut r = registro(42);
        r.chave = ruim.clone();
        assert_eq!(
            notas.guardar(&r, "<mal/>").unwrap_err(),
            ErroNotas::ChaveInvalida,
            "{ruim}"
        );
    }
    for data in ["../../..", "2026/10/02", "2026-13-01", "26-10-02", ""] {
        let mut r = registro(42);
        r.emissao = data.into();
        assert_eq!(
            notas.guardar(&r, "<mal/>").unwrap_err(),
            ErroNotas::DataInvalida,
            "{data}"
        );
    }
    // nada foi escrito fora da pasta de notas, nem dentro dela alem do esperado
    assert_eq!(
        nomes(p.caminho()),
        vec!["notas".to_string()],
        "a pasta pai nao ganhou nada"
    );
    assert_eq!(notas.listar().unwrap().len(), 1);
}

#[test]
fn falha_de_gravacao_nao_deixa_temporario_nem_trava() {
    let p = PastaTemporaria::nova("notas-falha");
    let notas = Notas::abrir(p.caminho()).unwrap();
    // o destino do XML e' uma pasta: o rename final falha depois de o temporario ja estar escrito
    let destino = notas
        .pasta()
        .join("2026")
        .join("10")
        .join(format!("{}.xml", chave(42)));
    std::fs::create_dir_all(&destino).unwrap();
    assert_eq!(
        notas.guardar(&registro(42), "<x/>").unwrap_err(),
        ErroNotas::Gravacao
    );
    // sem temporario no lugar do XML, sem indice pela metade e sem a trava
    assert_eq!(
        nomes(destino.parent().unwrap()),
        vec![format!("{}.xml", chave(42))]
    );
    assert_eq!(nomes(notas.pasta()), vec!["2026".to_string()]);
    assert!(notas.listar().unwrap().is_empty());
}
#[test]
fn indice_corrompido_e_avisado_e_nao_sobrescrito() {
    let p = PastaTemporaria::nova("notas-corrompido");
    let notas = Notas::abrir(p.caminho()).unwrap();
    std::fs::write(notas.pasta().join("indice.json"), "{ nao e json").unwrap();
    assert_eq!(notas.listar().unwrap_err(), ErroNotas::Corrompido);
    assert_eq!(
        notas.guardar(&registro(42), "<x/>").unwrap_err(),
        ErroNotas::Corrompido
    );
    assert_eq!(
        std::fs::read_to_string(notas.pasta().join("indice.json")).unwrap(),
        "{ nao e json"
    );
}

#[test]
fn processos_concorrentes_nao_perdem_numero() {
    let p = PastaTemporaria::nova("notas-concorrente");
    let notas = Notas::abrir(p.caminho()).unwrap();
    let fios: Vec<_> = (1..=8u64)
        .map(|n| {
            let notas = notas.clone();
            std::thread::spawn(move || notas.confirmar_numero("1", n * 10).unwrap())
        })
        .collect();
    for f in fios {
        f.join().unwrap();
    }
    assert_eq!(notas.proximo_numero("1").unwrap(), 81);
}

#[test]
fn pasta_vazia_e_recusada() {
    assert_eq!(Notas::abrir("").unwrap_err(), ErroNotas::Pasta);
}

#[test]
fn a_mensagem_de_erro_nao_leva_caminho() {
    for e in [
        ErroNotas::ChaveInvalida,
        ErroNotas::Pasta,
        ErroNotas::Leitura,
        ErroNotas::Gravacao,
        ErroNotas::Corrompido,
    ] {
        let t = e.to_string();
        assert!(!t.contains('/'), "{t}");
        assert!(!t.contains('\\'), "{t}");
    }
}

#[test]
fn o_danfse_fica_ao_lado_do_xml_e_nao_aparece_na_lista() {
    let p = PastaTemporaria::nova("notas-danfse");
    let notas = Notas::abrir(p.caminho()).unwrap();
    notas.guardar(&registro(42), "<NFSe>42</NFSe>").unwrap();
    let xml = notas.caminho_do_xml_guardado(&chave(42)).unwrap();
    assert!(xml.ends_with(format!("{}.xml", chave(42))));
    let pdf = notas.guardar_danfse(&chave(42), b"%PDF-1.7 teste").unwrap();
    assert_eq!(pdf.parent(), xml.parent());
    assert_eq!(
        pdf.file_name().unwrap().to_string_lossy(),
        format!("{}.pdf", chave(42))
    );
    assert_eq!(std::fs::read(&pdf).unwrap(), b"%PDF-1.7 teste");
    // o indice, a lista e a numeracao nao mudam
    assert_eq!(notas.listar().unwrap().len(), 1);
    assert_eq!(notas.proximo_numero("1").unwrap(), 43);
    assert_eq!(notas.ler_xml(&chave(42)).unwrap(), "<NFSe>42</NFSe>");
    // gravar de novo troca o arquivo, sem sobra temporaria
    notas.guardar_danfse(&chave(42), b"%PDF-novo").unwrap();
    assert_eq!(std::fs::read(&pdf).unwrap(), b"%PDF-novo");
    let sobras: Vec<_> = nomes(pdf.parent().unwrap());
    assert_eq!(sobras.len(), 2, "{sobras:?}");
    // so' nota do indice e so' chave valida
    assert_eq!(
        notas.guardar_danfse(&chave(99), b"%PDF").unwrap_err(),
        ErroNotas::NaoEncontrada
    );
    assert_eq!(
        notas.guardar_danfse("../../x", b"%PDF").unwrap_err(),
        ErroNotas::ChaveInvalida
    );
}
