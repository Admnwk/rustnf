// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Tabela de enderecos dos webservices da NF-e: UF -> autorizador -> URL por ambiente, lida das paginas "Relacao de
//! Servicos Web" do Portal Nacional da NF-e (producao e homologacao).

use rustnf::nfe::sefaz::tabela::{
    autorizador_da_uf, autorizadores, codigo_da_uf, endereco, fonte, servicos, uf_valida, url,
};
use rustnf::nfe::sefaz::Servico;
use rustnf::nfe::vigencias::Ambiente::{Homologacao, Producao};

const UFS: [&str; 27] = [
    "AC", "AL", "AM", "AP", "BA", "CE", "DF", "ES", "GO", "MA", "MG", "MS", "MT", "PA", "PB", "PE",
    "PI", "PR", "RJ", "RN", "RO", "RR", "RS", "SC", "SE", "SP", "TO",
];

#[test]
fn a_tabela_tem_15_autorizadores_e_96_enderecos_por_ambiente() {
    let a: Vec<&str> = autorizadores().collect();
    assert_eq!(a.len(), 15, "{a:?}");
    for amb in [Producao, Homologacao] {
        let total: usize = a.iter().map(|x| servicos(x, amb).unwrap().count()).sum();
        assert_eq!(total, 96, "{amb:?}");
    }
}

#[test]
fn toda_uf_tem_autorizador_e_o_autorizador_esta_na_tabela() {
    let aut: Vec<&str> = autorizadores().collect();
    for uf in UFS {
        let a = autorizador_da_uf(uf).unwrap();
        assert!(aut.contains(&a), "{uf} -> {a}");
    }
}

#[test]
fn uf_por_autorizador_conforme_a_pagina() {
    // UF com webservice proprio
    for uf in ["AM", "BA", "GO", "MG", "MS", "MT", "PE", "PR", "RS", "SP"] {
        assert_eq!(autorizador_da_uf(uf).unwrap(), uf);
    }
    assert_eq!(autorizador_da_uf("MA").unwrap(), "SVAN");
    for uf in [
        "AC", "AL", "AP", "CE", "DF", "ES", "PA", "PB", "PI", "RJ", "RN", "RO", "RR", "SC", "SE",
        "TO",
    ] {
        assert_eq!(autorizador_da_uf(uf).unwrap(), "SVRS", "{uf}");
    }
}

#[test]
fn a_uf_vem_pela_sigla_em_qualquer_caixa_ou_pelo_codigo_ibge() {
    assert_eq!(uf_valida("sp").unwrap(), "SP");
    assert_eq!(uf_valida(" SP ").unwrap(), "SP");
    assert_eq!(uf_valida("35").unwrap(), "SP");
    assert_eq!(codigo_da_uf("SP").unwrap(), 35);
    assert_eq!(codigo_da_uf("43").unwrap(), 43);
    assert_eq!(autorizador_da_uf("24").unwrap(), "SVRS");
}

#[test]
fn uf_desconhecida_e_erro_e_nao_cai_em_outro_autorizador() {
    for ruim in ["", "XX", "99", "0", "S", "SPP", "35x", "EX", "AN", "SVRS"] {
        let e = uf_valida(ruim).unwrap_err();
        assert_eq!(e.campo, "uf", "{ruim}");
        assert!(e.mensagem.contains("desconhecida"), "{ruim}: {e}");
        assert!(autorizador_da_uf(ruim).is_err());
        assert!(url(ruim, Producao, Servico::Autorizacao).is_err());
    }
    // a mensagem nao repete texto longo
    let e = uf_valida("um texto muito comprido que nao e' UF").unwrap_err();
    assert!(e.mensagem.len() < 80, "{e}");
}

#[test]
fn url_por_uf_ambiente_e_servico() {
    assert_eq!(
        url("SP", Producao, Servico::Autorizacao).unwrap(),
        "https://nfe.fazenda.sp.gov.br/ws/nfeautorizacao4.asmx"
    );
    assert_eq!(
        url("SP", Homologacao, Servico::Autorizacao).unwrap(),
        "https://homologacao.nfe.fazenda.sp.gov.br/ws/nfeautorizacao4.asmx"
    );
    assert_eq!(
        url("sp", Homologacao, Servico::RetAutorizacao).unwrap(),
        "https://homologacao.nfe.fazenda.sp.gov.br/ws/nferetautorizacao4.asmx"
    );
    // Rio Grande do Norte usa a SVRS
    assert_eq!(
        url("RN", Producao, Servico::ConsultaProtocolo).unwrap(),
        "https://nfe.svrs.rs.gov.br/ws/NfeConsulta/NfeConsulta4.asmx"
    );
    assert_eq!(
        url("24", Homologacao, Servico::StatusServico).unwrap(),
        "https://nfe-homologacao.svrs.rs.gov.br/ws/NfeStatusServico/NfeStatusServico4.asmx"
    );
    // Maranhao usa a SVAN
    assert_eq!(
        url("MA", Producao, Servico::Autorizacao).unwrap(),
        "https://www.sefazvirtual.fazenda.gov.br/NFeAutorizacao4/NFeAutorizacao4.asmx"
    );
}

#[test]
fn o_wsdl_do_fim_da_pagina_sai_do_endereco_da_chamada_mas_fica_na_tabela() {
    let e = endereco("GO", Producao, "NFeAutorizacao").unwrap();
    assert_eq!(
        e.url,
        "https://nfe.sefaz.go.gov.br/nfe/services/NFeAutorizacao4?wsdl"
    );
    assert_eq!(
        e.url_da_chamada(),
        "https://nfe.sefaz.go.gov.br/nfe/services/NFeAutorizacao4"
    );
    assert_eq!(
        url("GO", Producao, Servico::Autorizacao).unwrap(),
        "https://nfe.sefaz.go.gov.br/nfe/services/NFeAutorizacao4"
    );
    // GO, MT e PR publicam todos com ?wsdl; nenhum endereco de chamada sai com ele
    for uf in UFS {
        for amb in [Producao, Homologacao] {
            for s in Servico::TODOS {
                let u = url(uf, amb, s).unwrap();
                assert!(!u.contains('?'), "{uf} {amb:?} {s:?}: {u}");
                assert!(u.starts_with("https://"), "{u}");
            }
        }
    }
}

#[test]
fn os_quatro_servicos_do_cliente_existem_em_todo_autorizador_de_uf() {
    for uf in UFS {
        for amb in [Producao, Homologacao] {
            for s in Servico::TODOS {
                let e = endereco(autorizador_da_uf(uf).unwrap(), amb, s.nome_na_pagina()).unwrap();
                assert_eq!(e.versao, "4.00");
            }
        }
    }
}

#[test]
fn servico_que_o_autorizador_nao_oferece_e_erro_e_nao_se_inventa_host() {
    // a SVC-RS nao tem inutilizacao
    let e = endereco("SVC-RS", Producao, "NfeInutilizacao").unwrap_err();
    assert_eq!(e.campo, "servico");
    assert!(endereco("XX", Producao, "NFeAutorizacao").is_err());
    assert!(endereco("SP", Producao, "NaoExiste").is_err());
}

#[test]
fn os_outros_servicos_da_pagina_ficam_na_tabela_para_a_parte_seguinte() {
    let nomes: Vec<&str> = servicos("SP", Producao).unwrap().map(|(n, _)| n).collect();
    for esperado in [
        "NfeInutilizacao",
        "NfeConsultaCadastro",
        "RecepcaoEvento",
        "NFeAutorizacao",
    ] {
        assert!(nomes.contains(&esperado), "{nomes:?}");
    }
    let an: Vec<&str> = servicos("AN", Homologacao)
        .unwrap()
        .map(|(n, _)| n)
        .collect();
    assert_eq!(an, ["NFeDistribuicaoDFe", "RecepcaoEvento"]);
}

#[test]
fn namespace_e_acao_de_cada_servico() {
    let casos = [
        (
            Servico::StatusServico,
            "NFeStatusServico4",
            "nfeStatusServicoNF",
        ),
        (
            Servico::Autorizacao,
            "NFeAutorizacao4",
            "nfeAutorizacaoLote",
        ),
        (
            Servico::RetAutorizacao,
            "NFeRetAutorizacao4",
            "nfeRetAutorizacaoLote",
        ),
        (
            Servico::ConsultaProtocolo,
            "NFeConsultaProtocolo4",
            "nfeConsultaNF",
        ),
    ];
    for (s, wsdl, metodo) in casos {
        let ns = format!("http://www.portalfiscal.inf.br/nfe/wsdl/{wsdl}");
        assert_eq!(s.namespace(), ns);
        assert_eq!(s.acao(), format!("{ns}/{metodo}"));
    }
}

#[test]
fn a_fonte_diz_as_duas_paginas_e_o_dia_da_leitura() {
    let f = fonte();
    assert_eq!(
        f.producao,
        "https://www.nfe.fazenda.gov.br/portal/webServices.aspx?tipoConteudo=OUC/YVNWZfo="
    );
    assert_eq!(
        f.homologacao,
        "https://hom.nfe.fazenda.gov.br/portal/WebServices.aspx?tipoConteudo=OUC/YVNWZfo="
    );
    assert_eq!(f.lido_em, "2026-11-25");
}
