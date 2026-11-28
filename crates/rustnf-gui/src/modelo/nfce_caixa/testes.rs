// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use chrono::NaiveDate;
use rustnf_app::config::Config;
use rustnf_app::nfe::montar_pedido;

use super::*;
use crate::modelo::nfe_fiscal::Pagamento;
use crate::modelo::nfe_nova::ContextoNfe;
use crate::modelo::nfe_produtos::Produto;
use crate::testes_comuns::PastaTemporaria;

fn config(crt: u8) -> Config {
    let texto = format!(
        "[emitente]\ncnpj = \"11222333000181\"\nnome = \"LOJA DE TESTE LTDA\"\ninscricao_municipal = \"123456\"\nmunicipio = \"3550308\"\nop_simples = 3\nregime_apuracao = 1\n\
         [nfe]\nuf = \"SP\"\nie = \"111111111111\"\ncrt = {crt}\nregime_pis_cofins = \"nao_cumulativo\"\n\
         [nfe.endereco]\nlogradouro = \"Rua das Flores\"\nnumero = \"100\"\nbairro = \"Centro\"\nmunicipio = \"Sao Paulo\"\ncep = \"01001000\"\n"
    );
    Config::de_texto(&texto).unwrap()
}

fn contexto(c: &Config) -> ContextoNfe {
    ContextoNfe::de_config(c, NaiveDate::from_ymd_opt(2026, 10, 3).unwrap(), 1)
}

fn produto(codigo: &str, descricao: &str, valor: &str) -> Produto {
    Produto {
        codigo: codigo.into(),
        descricao: descricao.into(),
        valor: valor.into(),
        ncm: "09012100".into(),
        ..Produto::default()
    }
}

fn cupom_de_cafe() -> Cupom {
    let mut c = Cupom::default();
    c.adicionar(
        item_do_produto(&produto("C1", "Cafe torrado 500g", "15,90")),
        2000,
    );
    c
}

/// O pedido passa no `montar_pedido` estrito da aplicacao, no contexto da NFC-e.
fn conferir(c: &Cupom, config: &Config) -> rustnf_app::nfe::Montado {
    let pasta = PastaTemporaria::nova("caixa-modelo");
    let nfce = config.como_nfce();
    let notas = rustnf_app::nfce::abrir_pasta(pasta.caminho()).unwrap();
    let mut pedido = c.pedido(&contexto(config)).unwrap();
    pedido["modelo"] = serde_json::json!(65);
    montar_pedido(&nfce, Some(&notas), pedido, true).unwrap()
}

#[test]
fn dinheiro_com_troco_o_pedido_passa_na_aplicacao() {
    let cfg = config(1);
    let mut c = cupom_de_cafe();
    assert_eq!(c.totais().total, 3180);
    assert!(c.pagar(Pagamento::Dinheiro, 5000));
    assert_eq!((c.falta(), c.troco()), (0, 1820));
    let m = conferir(&c, &cfg);
    assert_eq!(m.totais["vNF"], "31.80");
    assert!(m.xml.contains("<vTroco>18.20</vTroco>"), "{}", m.xml);
    assert!(m.xml.contains("<mod>65</mod>"));
    assert!(
        m.xml.contains("<tPag>01</tPag><vPag>50.00</vPag>"),
        "{}",
        m.xml
    );
}

#[test]
fn pix_cartao_e_duas_formas_passam_e_cartao_e_pix_levam_o_grupo_card() {
    let cfg = config(1);
    let mut c = cupom_de_cafe();
    assert!(c.pagar(Pagamento::Pix, 0));
    assert_eq!(c.troco(), 0);
    let pix = conferir(&c, &cfg).xml;
    let det = pix
        .split("<detPag>")
        .find(|d| d.contains("<tPag>17</tPag>"))
        .unwrap_or_else(|| panic!("{pix}"));
    assert!(
        det.contains("<card><tpIntegra>2</tpIntegra></card>"),
        "{pix}"
    );
    let mut c = cupom_de_cafe();
    assert!(c.pagar(Pagamento::CartaoCredito, 2000));
    assert_eq!(c.falta(), 1180);
    assert!(c.pagar(Pagamento::Dinheiro, 0));
    assert_eq!(c.pagamentos.len(), 2);
    let m = conferir(&c, &cfg);
    assert!(
        m.xml.contains("<card><tpIntegra>2</tpIntegra></card>"),
        "{}",
        m.xml
    );
    assert!(!m.xml.contains("<vTroco>"), "{}", m.xml);
}

#[test]
fn desconto_e_rateado_e_cpf_entra_so_quando_confere() {
    let cfg = config(1);
    let mut c = Cupom::default();
    c.adicionar(
        item_do_produto(&produto("C1", "Cafe torrado 500g", "15,90")),
        1000,
    );
    c.adicionar(
        item_do_produto(&produto("F1", "Filtro de papel", "8,99")),
        1000,
    );
    c.desconto = "5,00".into();
    let t = c.totais();
    assert_eq!((t.produtos, t.desconto, t.total), (2489, 500, 1989));
    let soma: i64 = c
        .itens_com_rateio()
        .iter()
        .map(|i| i.desconto_centavos().unwrap())
        .sum();
    assert_eq!(soma, 500, "o rateio fecha o desconto sem sobrar centavo");
    c.cpf = "52998224725".into();
    assert!(c.cpf_confere());
    assert!(c.pagar(Pagamento::Dinheiro, 0));
    let m = conferir(&c, &cfg);
    assert_eq!(m.totais["vNF"], "19.89");
    assert!(m.xml.contains("<CPF>52998224725</CPF>"));
    assert!(m.xml.contains("<indIEDest>9</indIEDest>"));
    c.cpf = "52998224726".into();
    assert!(!c.cpf_confere());
    assert!(c
        .pendencias(&contexto(&cfg))
        .iter()
        .any(|p| p.mensagem.contains("CPF")));
    // sem CPF o cupom sai sem destinatario
    c.cpf.clear();
    assert!(!conferir(&c, &cfg).xml.contains("<dest>"));
}

#[test]
fn regime_normal_pede_a_aliquota_e_passa_com_ela() {
    let cfg = config(3);
    let mut c = Cupom::default();
    c.adicionar(
        item_do_produto(&produto("C1", "Cafe torrado 500g", "10,00")),
        1000,
    );
    assert!(c.pagar(Pagamento::Pix, 0));
    let p = c.pendencias(&contexto(&cfg));
    assert!(p.iter().any(|p| p.mensagem.contains("alíquota")), "{p:?}");
    c.itens[0].aliquota_icms = "18".into();
    assert!(c.pendencias(&contexto(&cfg)).is_empty());
    let m = conferir(&c, &cfg);
    assert_eq!(m.totais["vICMS"], "1.80", "{:?}", m.totais);
}

#[test]
fn cfop_de_outro_estado_fixado_no_cadastro_nao_vale_no_caixa() {
    let cfg = config(1);
    let mut c = cupom_de_cafe();
    c.itens[0].cfop = Some("6108".into());
    assert!(c.pagar(Pagamento::Pix, 0));
    let m = conferir(&c, &cfg);
    assert!(m.xml.contains("<CFOP>5102</CFOP>"), "{}", m.xml);
}

#[test]
fn as_pendencias_dizem_o_que_fazer() {
    let cfg = config(1);
    let ctx = contexto(&cfg);
    let c = Cupom::default();
    assert!(c.pendencias(&ctx)[0].mensagem.contains("vazio"));
    let mut c = cupom_de_cafe();
    assert!(c.pendencias(&ctx)[0].mensagem.contains("pagamento"));
    c.pagar(Pagamento::Dinheiro, 1000);
    let p = c.pendencias(&ctx);
    assert!(p[0].mensagem.contains("Faltam R$ 21,80"), "{p:?}");
    // cartao acima do total nao tem troco
    let mut c = cupom_de_cafe();
    c.pagar(Pagamento::CartaoDebito, 4000);
    assert_eq!(c.troco(), 0);
    assert!(c.pendencias(&ctx)[0].mensagem.contains("passa do total"));
    // produto sem NCM
    let mut c = Cupom::default();
    c.adicionar(
        item_do_produto(&Produto {
            ncm: String::new(),
            ..produto("X", "Sem ncm", "1,00")
        }),
        1000,
    );
    c.pagar(Pagamento::Pix, 0);
    assert!(c
        .pendencias(&ctx)
        .iter()
        .any(|p| p.mensagem.contains("NCM")));
}

#[test]
fn quantidade_soma_na_mesma_linha_muda_e_tira() {
    let mut c = Cupom::default();
    let p = item_do_produto(&produto("C1", "Cafe", "15,90"));
    c.adicionar(p.clone(), 1000);
    c.adicionar(p, 500);
    assert_eq!(c.itens.len(), 1);
    assert_eq!(quantidade_em_texto(&c.itens[0]), "1,5");
    c.mudar_quantidade(0, -1000);
    assert_eq!(quantidade_em_texto(&c.itens[0]), "0,5");
    c.mudar_quantidade(0, -1000);
    assert_eq!(
        quantidade_em_texto(&c.itens[0]),
        "0,5",
        "nunca zera a linha"
    );
    c.mudar_quantidade(0, 2000);
    assert_eq!(quantidade_em_texto(&c.itens[0]), "2,5");
    c.tirar(0);
    assert!(c.vazio());
    assert_eq!(quantidade_digitada(""), Some(1000));
    assert_eq!(quantidade_digitada("0,5"), Some(500));
    assert_eq!(quantidade_digitada("0"), None);
    assert_eq!(quantidade_digitada("abc"), None);
}

#[test]
fn nao_se_lanca_pagamento_em_cupom_sem_total() {
    let mut c = Cupom::default();
    assert!(!c.pagar(Pagamento::Dinheiro, 5000));
    assert_eq!((c.pago(), c.troco()), (0, 0));
}
