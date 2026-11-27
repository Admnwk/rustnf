// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Testes do cliente ja usado e da nota anterior como modelo: a NF-e montada pelo assistente volta a ser o assistente.

use chrono::{Duration, Utc};
use rustnf::nfe;
use rustnf_app::nfe::montar_pedido;
use rustnf_app::notas::nfe::NotasNfe;

use super::*;
use crate::modelo::nfe_nova::testes::{config_de, contexto, dados_basicos, item, CNPJ};
use crate::modelo::nfe_nova::{pedido, pendencias, PassoNfe};
use crate::testes_comuns::PastaTemporaria;

fn nfe_de(d: &DadosNfe, crt: u8, regime: &str) -> Nfe {
    let pasta = PastaTemporaria::nova("nfe-modelo");
    let config = config_de(&pasta, crt, regime);
    let c = contexto(
        crt,
        crate::modelo::nfe_fiscal::RegimePisCofins::da_config(regime),
    );
    let p = pedido(d, &c).unwrap();
    let notas = NotasNfe::abrir(config.notas.pasta.trim()).unwrap();
    let m = montar_pedido(&config, Some(&notas), p, true).unwrap();
    nfe::ler(m.xml.as_bytes()).unwrap()
}

#[test]
fn a_nota_anterior_volta_a_ser_o_assistente() {
    let mut d = dados_basicos();
    d.email = "maria@exemplo.com.br".into();
    d.complemento = "Sala 2".into();
    d.itens = vec![
        item("Cafe torrado 500g", "2", "15,90"),
        crate::modelo::nfe_nova::ItemNfe {
            desconto: "1,00".into(),
            ..item("Filtro de papel 103", "1,5", "8,99")
        },
    ];
    d.entrega = Entrega::TransportadoraPorContaDoCliente;
    d.transp_documento = CNPJ.into();
    d.transp_nome = "TRANSPORTES DE TESTE LTDA".into();
    d.transp_uf = "SP".into();
    d.volumes = "2".into();
    d.especie = "CAIXA".into();
    d.pagamento = Pagamento::Pix;
    let n = nfe_de(&d, 1, "");
    let m = ModeloNfe::da_nfe(&n, 1);
    let r = &m.dados;
    assert_eq!(r.documento, d.documento);
    // a biblioteca poe o texto de teste no nome do cliente das notas de homologacao: nao se copia
    assert_eq!(r.nome, "");
    assert!(
        m.avisos.iter().any(|a| a.contains("nome do cliente")),
        "{:?}",
        m.avisos
    );
    assert_eq!(r.itens[0].descricao, "Cafe torrado 500g");
    assert_eq!(r.email, d.email);
    assert_eq!(r.logradouro, "Avenida Paulista");
    assert_eq!(r.cod_municipio, "3550308");
    assert_eq!(r.consumo, Some(Consumo::Final));
    assert_eq!(r.contribuinte, Some(Contribuinte::Nao));
    assert_eq!(r.itens.len(), 2);
    assert_eq!(r.itens[0].quantidade, "2");
    assert_eq!(r.itens[0].valor, "15,90");
    assert_eq!(r.itens[0].ncm, "09012100");
    assert_eq!(r.itens[0].icms, Some(Icms::SemDestaque));
    assert_eq!(r.itens[1].quantidade, "1,5");
    assert_eq!(r.itens[1].desconto, "1,00");
    assert_eq!(r.pagamento, Pagamento::Pix);
    assert_eq!(r.entrega, Entrega::TransportadoraPorContaDoCliente);
    assert_eq!(r.transp_nome, "TRANSPORTES DE TESTE LTDA");
    assert_eq!(r.volumes, "2");
    assert_eq!(r.especie, "CAIXA");
    // os totais batem com os da nota original
    assert_eq!(r.totais(), d.totais());
    // sem o nome de teste, a conferencia aponta; com ele, o modelo gera pedido
    let faltas = pendencias(r, &contexto(1, None));
    assert_eq!(faltas.len(), 1, "{faltas:?}");
    assert_eq!(faltas[0].passo, PassoNfe::ParaQuem);
    let mut pronto = r.clone();
    pronto.nome = "MARIA SOUZA DE TESTE".into();
    assert!(pedido(&pronto, &contexto(1, None)).is_ok());
}

#[test]
fn nota_a_prazo_volta_sem_data_e_a_conferencia_aponta() {
    let mut d = dados_basicos();
    d.pagamento = Pagamento::APrazo;
    d.parcelas = "3".into();
    d.primeiro_vencimento = (Utc::now().date_naive() + Duration::days(30))
        .format("%d/%m/%Y")
        .to_string();
    let n = nfe_de(&d, 1, "");
    let m = ModeloNfe::da_nfe(&n, 1);
    assert_eq!(m.dados.pagamento, Pagamento::APrazo);
    assert_eq!(m.dados.parcelas, "3");
    assert!(m.dados.primeiro_vencimento.is_empty());
    assert!(m.avisos.iter().any(|a| a.contains("já venceram")));
    let p = pendencias(&m.dados, &contexto(1, None));
    assert!(p
        .iter()
        .any(|p| p.passo == PassoNfe::ComoRecebeu && p.mensagem.contains("primeira parcela")));
}

#[test]
fn regime_normal_guarda_a_aliquota_e_o_icms_de_codigo_estranho_fica_como_estava() {
    let mut d = dados_basicos();
    d.itens[0].aliquota_icms = "18".into();
    let n = nfe_de(&d, 3, "cumulativo");
    let m = ModeloNfe::da_nfe(&n, 3);
    assert_eq!(m.dados.itens[0].icms, Some(Icms::Tributado));
    assert_eq!(m.dados.itens[0].aliquota_icms, "18");
    assert!(m.dados.itens[0].icms_codigo.is_none());
    // um item com CST que a tabela nao oferece vira "escolhido a mao", e nao e' trocado
    let mut g = Grupo::default();
    g.textos.insert("CST".into(), "20".into());
    g.textos.insert("orig".into(), "1".into());
    let mut it = n.itens[0].clone();
    it.imposto.icms = Some(g);
    let i = item_do_xml(&it, false, &mut Vec::new());
    assert_eq!(i.icms_codigo.as_deref(), Some("20"));
    assert_eq!(i.origem_mercadoria, 1);
    assert_eq!(i.icms, None);
}

#[test]
fn cliente_ja_usado_so_completa_o_que_esta_vazio() {
    let n = nfe_de(&dados_basicos(), 1, "");
    let c = ClienteConhecido::da_nfe(&n).unwrap();
    assert_eq!(c.documento, "52998224725");
    let mut d = DadosNfe {
        documento: c.documento.clone(),
        nome: "Nome que a pessoa digitou".into(),
        ..DadosNfe::default()
    };
    let feitos = c.completar(&mut d);
    // o nome digitado fica; o resto vem
    assert_eq!(d.nome, "Nome que a pessoa digitou");
    assert!(!feitos.contains(&"nome"));
    assert!(feitos.contains(&"rua"));
    assert_eq!(c.nome, "", "o nome de homologação não vira nome de cliente");
    assert_eq!(d.logradouro, "Avenida Paulista");
    assert_eq!(d.cod_municipio, "3550308");
    assert_eq!(d.consumo, Some(Consumo::Final));
    // completar de novo nao muda nada
    assert!(c.completar(&mut d).is_empty());
    assert!(
        c.aviso(&feitos).contains("529.982.247-25"),
        "{}",
        c.aviso(&feitos)
    );
}

#[test]
fn valor_com_mais_de_duas_casas_nao_e_arredondado_calado() {
    let n = nfe_de(&dados_basicos(), 1, "");
    let mut it = n.itens[0].clone();
    it.valor_unitario = Decimal::ler("123.4567");
    let mut avisos = Vec::new();
    let i = item_do_xml(&it, true, &mut avisos);
    assert!(i.valor.is_empty());
    assert!(avisos[0].contains("mais de 2 casas"), "{avisos:?}");
    assert_eq!(
        dinheiro_exato(Decimal::ler("15.90")).as_deref(),
        Some("15,90")
    );
    assert_eq!(
        dinheiro_exato(Decimal::ler("15.900")).as_deref(),
        Some("15,90")
    );
    assert_eq!(quantidade(Decimal::ler("1.500")), "1,5");
    assert_eq!(quantidade(Decimal::ler("2")), "2");
}
