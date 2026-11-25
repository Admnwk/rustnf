// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use rustnf::nfse::leitura::ler_nfse;

use super::testes::{chave, registro};
use super::*;
use crate::testes_comuns::PastaTemporaria;

/// Uma nota com outra data, outro tomador e outra situacao.
fn nota(n: u64, emissao: &str, nome: &str, documento: &str, situacao: Situacao) -> Registro {
    Registro {
        emissao: emissao.into(),
        tomador_nome: nome.into(),
        tomador_documento: documento.into(),
        situacao,
        ..registro(n)
    }
}

fn numeros(filtro: &Filtro, notas: &[Registro]) -> Vec<String> {
    filtro
        .aplicar(notas.to_vec())
        .into_iter()
        .map(|r| r.numero)
        .collect()
}

fn exemplos() -> Vec<Registro> {
    vec![
        nota(
            1,
            "2026-09-30T23:59:00-03:00",
            "João da Silva",
            "52998224725",
            Situacao::Autorizada,
        ),
        nota(
            2,
            "2026-10-01T08:00:00-03:00",
            "Padaria Pão Quente Ltda",
            "11222333000181",
            Situacao::Cancelada,
        ),
        nota(
            3,
            "2026-10-31",
            "Clinica 24 Horas Sao Paulo Centro Ltda",
            "11222333000181",
            Situacao::Autorizada,
        ),
        nota(4, "2026-11-01", "", "", Situacao::Autorizada),
    ]
}

#[test]
fn filtro_vazio_deixa_tudo_e_o_periodo_inclui_as_pontas() {
    let notas = exemplos();
    let todas = Filtro::novo(None, None, None, "").unwrap();
    assert_eq!(numeros(&todas, &notas), ["1", "2", "3", "4"]);
    assert_eq!(todas, Filtro::default());

    let outubro = Filtro::novo(Some("2026-10-01"), Some("2026-10-31"), None, "").unwrap();
    assert_eq!(numeros(&outubro, &notas), ["2", "3"]);
    assert_eq!(
        numeros(&Filtro::do_mes("2026-10").unwrap(), &notas),
        ["2", "3"]
    );
    let desde = Filtro::novo(Some("2026-10-31"), None, None, "").unwrap();
    assert_eq!(numeros(&desde, &notas), ["3", "4"]);
    let ate = Filtro::novo(None, Some(" 2026-09-30 "), None, "").unwrap();
    assert_eq!(numeros(&ate, &notas), ["1"]);
}

#[test]
fn data_que_nao_e_um_dia_e_recusada() {
    for ruim in [
        "2026-13-01",
        "2026-10-32",
        "01/10/2026",
        "2026-10",
        "2026-1-1",
        "1999-10-01",
    ] {
        assert_eq!(
            Filtro::novo(Some(ruim), None, None, "").unwrap_err(),
            ErroNotas::DataInvalida,
            "{ruim}"
        );
        assert_eq!(
            Filtro::novo(None, Some(ruim), None, "").unwrap_err(),
            ErroNotas::DataInvalida,
            "{ruim}"
        );
    }
    for ruim in ["2026-13", "2026-1", "2026-10-01", "outubro", ""] {
        assert_eq!(
            Filtro::do_mes(ruim).unwrap_err(),
            ErroNotas::DataInvalida,
            "{ruim}"
        );
    }
}

#[test]
fn situacao_e_busca_valem_juntas() {
    let notas = exemplos();
    let canceladas = Filtro::novo(None, None, Some(Situacao::Cancelada), "").unwrap();
    assert_eq!(numeros(&canceladas, &notas), ["2"]);

    let busca = |texto: &str| numeros(&Filtro::novo(None, None, None, texto).unwrap(), &notas);
    // nome sem acento e sem caixa, palavras em qualquer ordem
    assert_eq!(busca("joao"), ["1"]);
    assert_eq!(busca("QUENTE pao"), ["2"]);
    // documento com ou sem mascara, e numero da nota
    assert_eq!(busca("529.982.247-25"), ["1"]);
    assert_eq!(busca("11222333000181"), ["2", "3"]);
    assert_eq!(
        busca("4"),
        ["1", "3", "4"],
        "no CPF da 1, no nome da 3 e no numero da 4; o CNPJ da 2 nao tem 4"
    );
    assert!(busca("nao existe").is_empty());
    // as duas juntas
    let f = Filtro::novo(None, None, Some(Situacao::Autorizada), "11222333000181").unwrap();
    assert_eq!(numeros(&f, &notas), ["3"]);
}

#[test]
fn chave_colada_acha_a_nota_e_nome_comprido_com_numero_nao_vira_chave() {
    let notas = exemplos();
    let busca = |texto: &str| numeros(&Filtro::novo(None, None, None, texto).unwrap(), &notas);
    // a chave inteira, em grupos de 5 como a tela mostra, e um trecho do comeco
    let inteira = chave(3);
    let em_grupos = inteira
        .as_bytes()
        .chunks(5)
        .map(|g| String::from_utf8_lossy(g).into_owned())
        .collect::<Vec<_>>()
        .join(" ");
    assert_eq!(busca(&inteira), ["3"]);
    assert_eq!(busca(&em_grupos), ["3"]);
    assert_eq!(
        busca(&inteira[..25]),
        ["1", "2", "3", "4"],
        "o comeco e' igual nas quatro"
    );
    // nome comprido com numero no meio: continua busca por nome
    assert_eq!(busca("Clinica 24 Horas Sao Paulo Centro"), ["3"]);
    assert_eq!(busca("24 horas sao paulo centro ltda"), ["3"]);
}

#[test]
fn csv_para_planilha() {
    let mut notas = exemplos();
    notas[1].tomador_nome = "Pao; \"Quente\"".into();
    notas[2].tomador_nome = "=SOMA(A1:A9)".into();
    notas[2].valor_servico = "1234.5".into();
    let texto = csv(&notas);
    let linhas: Vec<&str> = texto.split("\r\n").collect();
    assert_eq!(
        linhas[0],
        "emissao;numero;serie;numero_dps;valor_servico;situacao;tomador_documento;tomador_nome;chave"
    );
    assert_eq!(
        linhas[1],
        format!(
            "2026-09-30;1;1;1;150,00;autorizada;52998224725;João da Silva;{}",
            chave(1)
        )
    );
    // `;` e aspas no nome: campo entre aspas, aspas dobradas
    assert!(
        linhas[2].contains(";cancelada;11222333000181;\"Pao; \"\"Quente\"\"\";"),
        "{}",
        linhas[2]
    );
    // texto que a planilha leria como formula ganha um apostrofo
    assert!(
        linhas[3].contains(";1234,5;") && linhas[3].contains(";'=SOMA(A1:A9);"),
        "{}",
        linhas[3]
    );
    assert_eq!(linhas.len(), 6, "titulos, 4 notas e o fim");
    assert_eq!(linhas[5], "");
    assert_eq!(csv(&[]).lines().count(), 1);
}

#[test]
fn registro_de_uma_nota_do_indice() {
    let p = PastaTemporaria::nova("notas-registro");
    let notas = Notas::abrir(p.caminho()).unwrap();
    notas.guardar(&registro(42), "<NFSe>42</NFSe>").unwrap();
    assert_eq!(notas.registro(&chave(42)).unwrap(), registro(42));
    assert_eq!(
        notas.registro(&chave(43)).unwrap_err(),
        ErroNotas::NaoEncontrada
    );
    assert_eq!(
        notas.registro("../../x").unwrap_err(),
        ErroNotas::ChaveInvalida
    );
}

#[test]
fn exportar_copia_os_xml_e_grava_a_lista() {
    let p = PastaTemporaria::nova("notas-exportar");
    let notas = Notas::abrir(p.caminho().join("notas")).unwrap();
    notas.guardar(&registro(42), "<NFSe>42</NFSe>").unwrap();
    notas.guardar(&registro(43), "<NFSe>43</NFSe>").unwrap();
    // a 43 perde o XML: continua na lista, e a exportacao diz que faltou
    std::fs::remove_file(notas.caminho_do_xml_guardado(&chave(43)).unwrap()).unwrap();
    let destino = p.caminho().join("para").join("o contador");

    let lista = notas.listar().unwrap();
    let feito = notas.exportar(&lista, &destino).unwrap();
    assert_eq!(feito.copiadas, 1);
    assert_eq!(feito.sem_xml, [chave(43)]);
    assert_eq!(
        std::fs::read_to_string(destino.join(format!("{}.xml", chave(42)))).unwrap(),
        "<NFSe>42</NFSe>"
    );
    assert_eq!(
        std::fs::read_to_string(destino.join(LISTA_CSV)).unwrap(),
        csv(&lista)
    );
    let mut nomes: Vec<String> = std::fs::read_dir(&destino)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    nomes.sort();
    assert_eq!(nomes, [format!("{}.xml", chave(42)), LISTA_CSV.to_string()]);
    // a pasta de notas fica como estava
    assert_eq!(notas.listar().unwrap(), lista);
    assert_eq!(notas.ler_xml(&chave(42)).unwrap(), "<NFSe>42</NFSe>");
}

/// O XML de uma NFS-e autorizada, como a Sefin devolve (so' o que a leitura usa).
fn xml_de_nota(n: u64) -> String {
    format!(
        "<NFSe xmlns=\"http://www.sped.fazenda.gov.br/nfse\" versao=\"1.01\"><infNFSe Id=\"NFS{}\">\
         <nNFSe>{n}</nNFSe><dhProc>2026-10-02T10:00:01-03:00</dhProc><cStat>100</cStat><ambGer>2</ambGer>\
         <valores><vLiq>150.00</vLiq></valores><DPS versao=\"1.01\"><infDPS Id=\"DPS1\"><tpAmb>2</tpAmb>\
         <serie>1</serie><nDPS>{n}</nDPS><dCompet>2026-10-01</dCompet>\
         <toma><CNPJ>11222333000181</CNPJ><xNome>Cliente</xNome></toma>\
         <valores><vServPrest><vServ>150.00</vServ></vServPrest></valores></infDPS></DPS></infNFSe></NFSe>",
        chave(n)
    )
}

fn guardar_nota(notas: &Notas, n: u64) {
    let xml = xml_de_nota(n);
    let registro = Registro::da_nfse(&ler_nfse(&xml).unwrap());
    assert_eq!(
        registro.chave,
        chave(n),
        "a leitura acha a chave do exemplo"
    );
    notas.guardar(&registro, &xml).unwrap();
}

#[test]
fn reindexar_refaz_o_indice_perdido_igual_ao_que_era() {
    let p = PastaTemporaria::nova("notas-reindexar");
    let notas = Notas::abrir(p.caminho()).unwrap();
    for n in [42, 43, 44] {
        guardar_nota(&notas, n);
    }
    notas.confirmar_numero("1", 44).unwrap();
    let antes = notas.listar().unwrap();
    assert_eq!(antes.len(), 3);

    // o indice some
    std::fs::remove_file(notas.pasta().join(INDICE)).unwrap();
    assert!(notas.listar().unwrap().is_empty());
    let feito = notas.reindexar().unwrap();
    assert_eq!(
        feito,
        Reindexado {
            notas: 3,
            ignorados: 0,
            canceladas: 0,
            indice_ilegivel: false
        }
    );
    assert_eq!(notas.listar().unwrap(), antes);
    assert!(notas.ler_xml(&chave(43)).is_ok());
    assert_eq!(
        notas.proximo_numero("1").unwrap(),
        45,
        "a numeracao nao muda"
    );
    assert!(
        !notas.pasta().join(INDICE_ANTERIOR).exists(),
        "nao havia indice para copiar"
    );
}

#[test]
fn reindexar_aproveita_a_marca_de_cancelada_e_guarda_o_indice_anterior() {
    let p = PastaTemporaria::nova("notas-reindexar-cancelada");
    let notas = Notas::abrir(p.caminho()).unwrap();
    for n in [42, 43] {
        guardar_nota(&notas, n);
    }
    notas.marcar_cancelada(&chave(43)).unwrap();
    let antes = notas.listar().unwrap();
    let indice_antes = std::fs::read(notas.pasta().join(INDICE)).unwrap();
    // um DANFSe ao lado, um XML que nao e' nota e um arquivo de outra coisa: so' o XML estranho conta como ignorado
    notas.guardar_danfse(&chave(42), b"%PDF").unwrap();
    let mes = notas.pasta().join("2026").join("10");
    std::fs::write(mes.join(format!("{}.xml", chave(99))), "<outra-coisa/>").unwrap();
    std::fs::write(mes.join("anotacao.txt"), "x").unwrap();

    let feito = notas.reindexar().unwrap();
    assert_eq!(
        feito,
        Reindexado {
            notas: 2,
            ignorados: 1,
            canceladas: 1,
            indice_ilegivel: false
        }
    );
    assert_eq!(notas.listar().unwrap(), antes);
    assert_eq!(
        notas.registro(&chave(43)).unwrap().situacao,
        Situacao::Cancelada
    );
    assert_eq!(
        std::fs::read(notas.pasta().join(INDICE_ANTERIOR)).unwrap(),
        indice_antes
    );
}

#[test]
fn reindexar_com_o_indice_ilegivel_avisa_que_a_marca_de_cancelada_se_perdeu() {
    let p = PastaTemporaria::nova("notas-reindexar-ilegivel");
    let notas = Notas::abrir(p.caminho()).unwrap();
    for n in [42, 43] {
        guardar_nota(&notas, n);
    }
    notas.marcar_cancelada(&chave(43)).unwrap();
    std::fs::write(notas.pasta().join(INDICE), "{ isto nao e' JSON").unwrap();
    assert_eq!(notas.listar().unwrap_err(), ErroNotas::Corrompido);

    let feito = notas.reindexar().unwrap();
    assert_eq!(
        feito,
        Reindexado {
            notas: 2,
            ignorados: 0,
            canceladas: 0,
            indice_ilegivel: true
        }
    );
    let lista = notas.listar().unwrap();
    assert_eq!(lista.len(), 2);
    assert!(lista.iter().all(|r| r.situacao == Situacao::Autorizada));
    // o arquivo ilegivel ficou guardado ao lado
    assert_eq!(
        std::fs::read_to_string(notas.pasta().join(INDICE_ANTERIOR)).unwrap(),
        "{ isto nao e' JSON"
    );
}

#[test]
fn reindexar_ignora_xml_fora_da_pasta_do_mes_dele() {
    let p = PastaTemporaria::nova("notas-reindexar-lugar");
    let notas = Notas::abrir(p.caminho()).unwrap();
    guardar_nota(&notas, 42);
    // a nota 43 (de outubro) largada na pasta de novembro, e um XML com o nome de outra chave
    let novembro = notas.pasta().join("2026").join("11");
    std::fs::create_dir_all(&novembro).unwrap();
    std::fs::write(novembro.join(format!("{}.xml", chave(43))), xml_de_nota(43)).unwrap();
    let outubro = notas.pasta().join("2026").join("10");
    std::fs::write(outubro.join(format!("{}.xml", chave(44))), xml_de_nota(45)).unwrap();

    let feito = notas.reindexar().unwrap();
    assert_eq!((feito.notas, feito.ignorados), (1, 2));
    assert_eq!(notas.listar().unwrap()[0].chave, chave(42));
}

#[test]
fn reindexar_aproveita_tambem_a_substituida_e_a_em_analise_fiscal() {
    let p = PastaTemporaria::nova("notas-reindexar-situacoes");
    let notas = Notas::abrir(p.caminho()).unwrap();
    for n in [42, 43, 44] {
        guardar_nota(&notas, n);
    }
    notas.marcar_cancelada(&chave(42)).unwrap();
    notas
        .definir_situacao(&chave(43), Situacao::Substituida)
        .unwrap();
    notas
        .definir_situacao(&chave(44), Situacao::AnaliseFiscal)
        .unwrap();
    let antes = notas.listar().unwrap();
    let feito = notas.reindexar().unwrap();
    assert_eq!(feito.notas, 3);
    assert_eq!(feito.canceladas, 1);
    assert_eq!(notas.listar().unwrap(), antes);
}
