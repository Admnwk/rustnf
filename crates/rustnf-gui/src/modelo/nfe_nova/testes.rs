// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Testes da Nova NF-e no modelo: as respostas simples viram o pedido, e o pedido passa pela montagem da aplicacao
//! (a mesma conferencia que a janela usa) nos casos tipicos.

use chrono::{Duration, Utc};
use rustnf_app::config::Config;
use rustnf_app::nfe::{montar_pedido, Montado};
use rustnf_app::notas::nfe::NotasNfe;

use super::*;
use crate::testes_comuns::PastaTemporaria;

pub(crate) const CNPJ: &str = "11222333000181";
pub(crate) const CPF: &str = "52998224725";
const CPF2: &str = "12345678909";

pub(crate) fn config_de(pasta: &PastaTemporaria, crt: u8, regime: &str) -> Config {
    let texto = format!(
        "[emitente]\ncnpj = \"{CNPJ}\"\nnome = \"LOJA DE TESTE LTDA\"\ninscricao_municipal = \"123456\"\nmunicipio = \"3550308\"\nop_simples = 3\nregime_apuracao = 1\n\
         [notas]\npasta = '{}'\n\
         [nfe]\nuf = \"SP\"\nie = \"111111111111\"\ncrt = {crt}\nserie = \"1\"\nregime_pis_cofins = \"{regime}\"\n\
         [nfe.endereco]\nlogradouro = \"Rua das Flores\"\nnumero = \"100\"\nbairro = \"Centro\"\nmunicipio = \"Sao Paulo\"\ncep = \"01001000\"\n",
        pasta.caminho().join("notas").to_string_lossy()
    );
    Config::de_texto(&texto).unwrap()
}

pub(crate) fn contexto(crt: u8, regime: Option<RegimePisCofins>) -> ContextoNfe {
    ContextoNfe {
        crt,
        uf_emitente: "SP".into(),
        regime_pis: regime,
        hoje: Utc::now().date_naive(),
        numero_provisorio: 1,
    }
}

pub(crate) fn item(descricao: &str, qtd: &str, valor: &str) -> ItemNfe {
    ItemNfe {
        codigo: "P1".into(),
        descricao: descricao.into(),
        quantidade: qtd.into(),
        valor: valor.into(),
        ncm: "09012100".into(),
        ..ItemNfe::default()
    }
}

/// Um cliente pessoa fisica de Sao Paulo, uso proprio, com um produto.
pub(crate) fn dados_basicos() -> DadosNfe {
    DadosNfe {
        documento: CPF.into(),
        nome: "MARIA SOUZA DE TESTE".into(),
        consumo: Some(Consumo::Final),
        contribuinte: Some(Contribuinte::Nao),
        cep: "01310100".into(),
        logradouro: "Avenida Paulista".into(),
        numero: "1000".into(),
        bairro: "Bela Vista".into(),
        cod_municipio: "3550308".into(),
        itens: vec![item("Cafe torrado 500g", "2", "15,90")],
        ..DadosNfe::default()
    }
}

/// O pedido do assistente, completado e conferido pela camada de aplicacao, com o relogio do sistema.
fn montar(d: &DadosNfe, c: &ContextoNfe, config: &Config) -> Result<Montado, String> {
    let pedido = pedido(d, c).map_err(|p| format!("pendências: {p:?}"))?;
    let notas = NotasNfe::abrir(config.notas.pasta.trim()).map_err(|e| e.to_string())?;
    montar_pedido(config, Some(&notas), pedido, true).map_err(|e| e.mensagem())
}

fn tudo_certo(d: &DadosNfe, crt: u8, regime: &str) -> Montado {
    let pasta = PastaTemporaria::nova("nfe-nova");
    let config = config_de(&pasta, crt, regime);
    let c = contexto(crt, RegimePisCofins::da_config(regime));
    montar(d, &c, &config).unwrap_or_else(|e| panic!("{e}"))
}

#[test]
fn simples_dentro_do_estado_para_consumidor_final_passa_na_montagem() {
    let m = tudo_certo(&dados_basicos(), 1, "");
    assert_eq!(m.totais["vNF"], "31.80", "{:?}", m.totais);
    assert!(m.xml.contains("<CFOP>5102</CFOP>"), "{}", m.xml);
    assert!(m.xml.contains("<CSOSN>102</CSOSN>"));
    assert!(m.xml.contains("<idDest>1</idDest>"));
    assert!(m.xml.contains("<indFinal>1</indFinal>"));
    assert!(m.xml.contains("<indPres>1</indPres>"));
    assert!(m.xml.contains("<modFrete>9</modFrete>"));
    assert!(m.xml.contains("<tPag>01</tPag>"));
}

#[test]
fn regime_normal_com_icms_proprio_passa_na_montagem() {
    let mut d = dados_basicos();
    d.itens[0].aliquota_icms = "18".into();
    let m = tudo_certo(&d, 3, "nao_cumulativo");
    assert_eq!(m.totais["vNF"], "31.80");
    // 31,80 x 18% = 5,72 (5,724 arredondado)
    assert_eq!(m.totais["vICMS"], "5.72", "{:?}", m.totais);
    assert!(m.xml.contains("<CST>00</CST>"), "{}", m.xml);
    // PIS 1,65% = 0,52 e COFINS 7,6% = 2,42
    assert_eq!(m.totais["vPIS"], "0.52", "{:?}", m.totais);
    assert_eq!(m.totais["vCOFINS"], "2.42", "{:?}", m.totais);
}

#[test]
fn regime_normal_cumulativo_usa_as_aliquotas_do_cumulativo() {
    let mut d = dados_basicos();
    d.itens[0].aliquota_icms = "18".into();
    let m = tudo_certo(&d, 3, "cumulativo");
    // 0,65% de 31,80 = 0,21 e 3% = 0,95
    assert_eq!(m.totais["vPIS"], "0.21", "{:?}", m.totais);
    assert_eq!(m.totais["vCOFINS"], "0.95", "{:?}", m.totais);
}

#[test]
fn outro_estado_para_revenda_com_ie_usa_cfop_6102() {
    let mut d = dados_basicos();
    d.documento = CNPJ.into();
    d.nome = "COMERCIO DE TESTE LTDA".into();
    d.consumo = Some(Consumo::Revenda);
    d.contribuinte = Some(Contribuinte::ComIe);
    d.ie = "12345678".into();
    d.cod_municipio = "3304557".into();
    d.logradouro = "Avenida Brasil".into();
    d.bairro = "Jardim America".into();
    d.cep = "20040002".into();
    let m = tudo_certo(&d, 1, "");
    assert!(m.xml.contains("<CFOP>6102</CFOP>"), "{}", m.xml);
    assert!(m.xml.contains("<idDest>2</idDest>"));
    assert!(m.xml.contains("<indFinal>0</indFinal>"));
    assert!(m.xml.contains("<indIEDest>1</indIEDest>"));
}

#[test]
fn entrega_por_transportadora_leva_os_dados_e_os_volumes() {
    let mut d = dados_basicos();
    d.entrega = Entrega::TransportadoraPorMinhaConta;
    d.transp_documento = CNPJ.into();
    d.transp_nome = "TRANSPORTES DE TESTE LTDA".into();
    d.transp_uf = "SP".into();
    d.volumes = "3".into();
    d.especie = "CAIXA".into();
    d.peso_bruto = "12,5".into();
    let m = tudo_certo(&d, 1, "");
    assert!(m.xml.contains("<modFrete>0</modFrete>"), "{}", m.xml);
    assert!(m.xml.contains("<xNome>TRANSPORTES DE TESTE LTDA</xNome>"));
    assert!(m.xml.contains("<qVol>3</qVol>"));
    assert!(m.xml.contains("<pesoB>12.500</pesoB>"));
}

#[test]
fn entrega_propria_e_cliente_busca_passam() {
    for (e, mod_frete) in [(Entrega::Propria, 3), (Entrega::ClienteBusca, 4)] {
        let mut d = dados_basicos();
        d.entrega = e;
        let m = tudo_certo(&d, 1, "");
        assert!(m.xml.contains(&format!("<modFrete>{mod_frete}</modFrete>")));
    }
}

#[test]
fn a_prazo_em_parcelas_soma_o_total_e_passa() {
    let mut d = dados_basicos();
    d.pagamento = Pagamento::APrazo;
    d.parcelas = "3".into();
    let primeiro = Utc::now().date_naive() + Duration::days(30);
    d.primeiro_vencimento = primeiro.format("%d/%m/%Y").to_string();
    let parcelas = d.parcelas_da_nota();
    assert_eq!(parcelas.len(), 3);
    // 31,80 / 3 = 10,60 sem resto
    assert!(parcelas.iter().all(|(v, _)| *v == 1060), "{parcelas:?}");
    assert_eq!(parcelas.iter().map(|(v, _)| v).sum::<i64>(), 3180);
    assert!(parcelas[0].1 < parcelas[1].1 && parcelas[1].1 < parcelas[2].1);
    let m = tudo_certo(&d, 1, "");
    assert!(m.xml.contains("<nDup>001</nDup>"), "{}", m.xml);
    assert!(m.xml.contains("<nDup>003</nDup>"));
    assert!(m.xml.contains("<indPag>1</indPag>"));
}

#[test]
fn o_resto_da_divisao_vai_na_ultima_parcela() {
    let mut d = dados_basicos();
    d.itens = vec![item("Filtro", "1", "100,00")];
    d.pagamento = Pagamento::APrazo;
    d.parcelas = "3".into();
    d.primeiro_vencimento = "15/12/2026".into();
    let p = d.parcelas_da_nota();
    assert_eq!(
        p.iter().map(|(v, _)| *v).collect::<Vec<_>>(),
        vec![3333, 3333, 3334]
    );
    // um mes depois, mesmo dia
    assert_eq!(p[1].1.to_string(), "2027-01-15");
}

#[test]
fn cartao_leva_o_grupo_card() {
    for forma in [
        Pagamento::CartaoCredito,
        Pagamento::CartaoDebito,
        Pagamento::Pix,
    ] {
        let mut d = dados_basicos();
        d.pagamento = forma;
        let m = tudo_certo(&d, 1, "");
        assert!(m.xml.contains(&format!("<tPag>{}</tPag>", forma.t_pag())));
        assert_eq!(m.xml.contains("<tpIntegra>2</tpIntegra>"), forma.e_cartao());
    }
}

#[test]
fn venda_nao_presencial_leva_o_indicador_de_intermediador() {
    let mut d = dados_basicos();
    d.presenca = Presenca::Internet;
    let m = tudo_certo(&d, 1, "");
    assert!(m.xml.contains("<indPres>2</indPres>"), "{}", m.xml);
    assert!(m.xml.contains("<indIntermed>0</indIntermed>"));
}

#[test]
fn varios_itens_com_desconto_somam_certo() {
    let mut d = dados_basicos();
    d.itens = vec![
        item("Cafe torrado 500g", "2", "15,90"),
        ItemNfe {
            desconto: "1,00".into(),
            ..item("Filtro de papel 103", "1,5", "8,99")
        },
    ];
    // 31,80 + 13,485 (13,49) - 1,00
    let t = d.totais();
    assert_eq!(
        (t.produtos, t.desconto, t.total),
        (3180 + 1349, 100, 3180 + 1349 - 100)
    );
    let m = tudo_certo(&d, 1, "");
    assert_eq!(m.totais["vProd"], "45.29", "{:?}", m.totais);
    assert_eq!(m.totais["vDesc"], "1.00");
    assert_eq!(m.totais["vNF"], "44.29");
}

#[test]
fn icms_ja_cobrado_antes_no_simples_usa_o_csosn_500_e_o_cfop_5405() {
    let mut d = dados_basicos();
    d.itens[0].icms = Some(Icms::JaCobradoAntes);
    d.itens[0].st_base = "20,00".into();
    d.itens[0].st_aliquota = "18".into();
    d.itens[0].st_valor = "3,60".into();
    d.itens[0].cest = "1700100".into();
    let m = tudo_certo(&d, 1, "");
    assert!(m.xml.contains("<CSOSN>500</CSOSN>"), "{}", m.xml);
    assert!(m.xml.contains("<CFOP>5405</CFOP>"));
}

#[test]
fn o_que_a_pessoa_editou_a_mao_nunca_e_sobrescrito() {
    let c = contexto(1, None);
    let mut i = item("Cafe", "1", "10,00");
    // a tabela diz 5102 dentro do estado e 6102 fora
    assert_eq!(
        i.cfop_efetivo(Destino::DentroDoEstado, 1, None).as_deref(),
        Some("5102")
    );
    assert_eq!(
        i.cfop_efetivo(Destino::OutroEstado, 1, None).as_deref(),
        Some("6102")
    );
    // editado a mao: vale o da pessoa nos dois destinos
    i.cfop = Some("5949".into());
    i.icms_codigo = Some("900".into());
    i.pis_cst = Some("99".into());
    assert_eq!(
        i.cfop_efetivo(Destino::DentroDoEstado, 1, None).as_deref(),
        Some("5949")
    );
    assert_eq!(
        i.cfop_efetivo(Destino::OutroEstado, 1, None).as_deref(),
        Some("5949")
    );
    // mudar a origem, o ICMS ou o regime nao desfaz a edicao
    i.origem = Origem::Fabricou;
    i.icms = Some(Icms::NaoTributado);
    assert_eq!(
        i.cfop_efetivo(Destino::OutroEstado, 3, None).as_deref(),
        Some("5949")
    );
    assert_eq!(i.icms_codigo_efetivo(3).as_deref(), Some("900"));
    let d = DadosNfe {
        itens: vec![i],
        ..dados_basicos()
    };
    let p = pedido(&d, &c).unwrap();
    assert_eq!(p["itens"][0]["cfop"], "5949");
    assert_eq!(p["itens"][0]["icms"]["codigo"], "900");
    assert_eq!(p["itens"][0]["pis"]["cst"], "99");
}

#[test]
fn a_pergunta_simples_que_nao_decide_mostra_as_opcoes() {
    // regime normal sem o regime do PIS na configuracao: a tela pergunta, o pedido nao sai
    let mut d = dados_basicos();
    d.itens[0].aliquota_icms = "18".into();
    let c = contexto(3, None);
    assert!(c.pergunta_o_regime_pis());
    let falta = pedido(&d, &c).unwrap_err();
    assert!(
        falta
            .iter()
            .any(|p| p.passo == PassoNfe::OQueVendeu && p.mensagem.contains("PIS")),
        "{falta:?}"
    );
    // a pessoa escolhe: passa
    d.regime_pis = Some(RegimePisCofins::NaoCumulativo);
    assert!(pedido(&d, &c).is_ok());
    // com a configuracao dizendo, nao pergunta
    let c = contexto(3, Some(RegimePisCofins::Cumulativo));
    assert!(!c.pergunta_o_regime_pis());
    d.regime_pis = None;
    assert!(pedido(&d, &c).is_ok());
    // no Simples nunca pergunta
    assert!(!contexto(1, None).pergunta_o_regime_pis());
    // cliente CNPJ: o assistente nao adivinha se tem inscricao estadual
    let mut d = dados_basicos();
    d.documento = CNPJ.into();
    d.contribuinte = None;
    let p = pendencias(&d, &contexto(1, None));
    assert!(
        p.iter().any(|p| p.mensagem.contains("inscrição estadual")),
        "{p:?}"
    );
    // regime normal: o ICMS nao tem aliquota padrao, a tela pede
    let d = dados_basicos();
    let p = pendencias(&d, &contexto(3, Some(RegimePisCofins::Cumulativo)));
    assert!(
        p.iter().any(|p| p.mensagem.contains("alíquota do ICMS")),
        "{p:?}"
    );
}

#[test]
fn as_pendencias_dizem_o_passo_e_o_que_fazer() {
    let p = pendencias(&DadosNfe::default(), &contexto(1, None));
    let passos: std::collections::BTreeSet<PassoNfe> = p.iter().map(|p| p.passo).collect();
    assert!(passos.contains(&PassoNfe::ParaQuem));
    assert!(passos.contains(&PassoNfe::OQueVendeu));
    assert!(p
        .iter()
        .all(|p| !p.mensagem.is_empty() && !p.como_resolver.is_empty()));
    // documento que nao confere
    let mut d = dados_basicos();
    d.documento = "52998224724".into();
    let p = pendencias(&d, &contexto(1, None));
    assert!(p.iter().any(|p| p.mensagem.contains("não confere")));
    // item incompleto aponta o item
    let mut d = dados_basicos();
    d.itens.push(ItemNfe::default());
    let p = pendencias(&d, &contexto(1, None));
    assert!(
        p.iter()
            .any(|p| p.item == Some(1) && p.mensagem.contains("descrição")),
        "{p:?}"
    );
    assert!(
        p.iter()
            .any(|p| p.item == Some(1) && p.mensagem.contains("NCM")),
        "{p:?}"
    );
    // a prazo sem data
    let mut d = dados_basicos();
    d.pagamento = Pagamento::APrazo;
    let p = pendencias(&d, &contexto(1, None));
    assert!(p
        .iter()
        .any(|p| p.passo == PassoNfe::ComoRecebeu && p.mensagem.contains("primeira parcela")));
    // transportadora sem dados
    let mut d = dados_basicos();
    d.entrega = Entrega::TransportadoraPorContaDoCliente;
    let p = pendencias(&d, &contexto(1, None));
    assert!(p.iter().any(|p| p.passo == PassoNfe::ComoEntrega));
    // exterior: a tela diz o que falta em vez de fingir
    let mut d = dados_basicos();
    d.exterior = true;
    let p = pendencias(&d, &contexto(1, None));
    assert!(p.iter().any(|p| p.mensagem.contains("exterior")));
}

#[test]
fn a_matematica_do_dinheiro_e_exata() {
    assert_eq!(milesimos("2"), Some(2000));
    assert_eq!(milesimos("1,5"), Some(1500));
    assert_eq!(milesimos("0.250"), Some(250));
    assert_eq!(milesimos(",5"), Some(500));
    assert_eq!(milesimos("1,2345"), None);
    assert_eq!(milesimos("abc"), None);
    assert_eq!(milesimos(""), None);
    assert_eq!(milesimos("1,2,3"), None);
    assert_eq!(centesimos_de_percentual("18"), Some(1800));
    assert_eq!(centesimos_de_percentual("7,6"), Some(760));
    assert_eq!(centesimos_de_percentual("12.5"), Some(1250));
    assert_eq!(centesimos_de_percentual("1,234"), None);
    // 1,5 x 8,99 = 13,485: meio centavo sobe
    let i = ItemNfe {
        quantidade: "1,5".into(),
        valor: "8,99".into(),
        ..ItemNfe::default()
    };
    assert_eq!(i.produto_centavos(), Some(1349));
    assert_eq!(imposto_centavos(3180, 1800), 572);
    assert_eq!(imposto_centavos(10, 1800), 2);
    assert_eq!(decimal_de_milesimos(1500), "1.5");
    assert_eq!(decimal_de_milesimos(2000), "2");
    assert_eq!(decimal_de_percentual(760), "7.6");
}

#[test]
fn os_passos_andam_para_a_frente_e_para_tras() {
    assert_eq!(PassoNfe::ParaQuem.anterior(), None);
    assert_eq!(PassoNfe::ParaQuem.proximo(), Some(PassoNfe::OQueVendeu));
    assert_eq!(PassoNfe::Conferir.proximo(), None);
    assert_eq!(PassoNfe::ComoRecebeu.indicador(), "Passo 3 de 5");
    assert!(DadosNfe::default().vazio());
    assert!(!dados_basicos().vazio());
    let _ = CPF2;
}
