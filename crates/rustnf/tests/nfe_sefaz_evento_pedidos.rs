// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Os pedidos de evento (parte pura): cancelamento, carta de correcao, EPEC e os eventos da apuracao do IBS/CBS. Cada
//! regra tem o seu teste: tamanhos, sequencias, protocolo de 15 ou 17 digitos, autor, Id, assinatura e o texto fixo.

mod nfe_sefaz_comum;

use chrono::{DateTime, FixedOffset, TimeZone, Utc};
use nfe_sefaz_comum::eventos::*;
use nfe_sefaz_comum::*;
use rustnf::assinatura::xmldsig;
use rustnf::nfe::sefaz::evento::{
    campos_epec, cancelamento, carta_correcao, epec, evento_rtc, tipo_rtc, Cancelamento,
    CartaCorrecao, Epec, EventoRtc, ItemRtc, CANCELAVEIS, COND_USO, EVENTOS,
};
use rustnf::nfe::vigencias::Ambiente::Homologacao;

fn quando() -> DateTime<FixedOffset> {
    Utc.with_ymd_and_hms(2026, 10, 3, 13, 0, 0)
        .unwrap()
        .fixed_offset()
}

fn canc<'a>(chave: &'a str, prot: &'a str, just: &'a str) -> Cancelamento<'a> {
    Cancelamento {
        chave,
        protocolo: prot,
        justificativa: just,
        ambiente: Homologacao,
        quando: quando(),
    }
}

// ------------------------------------------------------------------------------------------------ cancelamento

#[test]
fn cancelamento_tem_o_id_o_orgao_o_autor_e_o_horario_da_uf_da_chave() {
    let (_, chave) = nfe(1, 2);
    let m = cancelamento(&canc(&chave, NPROT, "Erro de digitacao no pedido")).unwrap();
    assert_eq!(m.id, format!("ID110111{chave}01"));
    assert_eq!((m.tp_evento.as_str(), m.seq), ("110111", 1));
    assert!(m.xml.starts_with(&format!(
        "<evento xmlns=\"{NS}\" versao=\"1.00\"><infEvento Id=\"{}\"><cOrgao>35</cOrgao><tpAmb>2</tpAmb>\
         <CNPJ>11222333000181</CNPJ><chNFe>{chave}</chNFe><dhEvento>2026-10-03T10:00:00-03:00</dhEvento>\
         <tpEvento>110111</tpEvento><nSeqEvento>1</nSeqEvento><verEvento>1.00</verEvento>",
        m.id
    )));
    assert!(m.xml.ends_with(&format!(
        "<detEvento versao=\"1.00\"><descEvento>Cancelamento</descEvento><nProt>{NPROT}</nProt>\
         <xJust>Erro de digitacao no pedido</xJust></detEvento></infEvento></evento>"
    )));
}

#[test]
fn cancelamento_a_justificativa_vai_de_15_a_255_sem_acento_e_escapada() {
    let (_, chave) = nfe(2, 2);
    let curta = cancelamento(&canc(&chave, NPROT, "curta demais")).unwrap_err();
    assert_eq!(curta.campo, "xJust");
    let longa = "x".repeat(256);
    assert_eq!(
        cancelamento(&canc(&chave, NPROT, &longa))
            .unwrap_err()
            .campo,
        "xJust"
    );
    assert!(cancelamento(&canc(&chave, NPROT, &"x".repeat(255))).is_ok());
    // espaco nas pontas nao conta; acento sai; < & > ficam escapados
    let m = cancelamento(&canc(
        &chave,
        NPROT,
        "  Cancelamento p/ correcao & ajuste <ok> ção  ",
    ))
    .unwrap();
    assert!(
        m.xml
            .contains("<xJust>Cancelamento p/ correcao &amp; ajuste &lt;ok&gt; cao</xJust>"),
        "{}",
        m.xml
    );
}

#[test]
fn cancelamento_o_protocolo_tem_15_ou_17_digitos_nunca_16() {
    let (_, chave) = nfe(3, 2);
    for bom in ["135260000000001", "13526000000000123"] {
        assert!(cancelamento(&canc(&chave, bom, "Cancelamento de teste")).is_ok());
    }
    for ruim in [
        "",
        "1352600000000",
        "1352600000000012",
        "13526000000000a1",
        "135260000000001234",
    ] {
        let e = cancelamento(&canc(&chave, ruim, "Cancelamento de teste")).unwrap_err();
        assert_eq!(e.campo, "nProt", "{ruim}");
    }
}

#[test]
fn cancelamento_recusa_chave_ruim_e_de_nfce() {
    let e = cancelamento(&canc("123", NPROT, "Cancelamento de teste")).unwrap_err();
    assert_eq!(e.campo, "chNFe");
    // modelo 65 (NFC-e) nao e' deste cliente
    let (_, chave) = nfe(4, 2);
    let c = rustnf::chave_dfe::decompor(&chave, true).unwrap();
    let nfce = rustnf::chave_dfe::montar(&rustnf::chave_dfe::Montar {
        cuf: c.cuf,
        aamm: &c.aamm,
        emitente: &c.emitente,
        modelo: 65,
        serie: u64::from(c.serie),
        numero: c.numero,
        tp_emis: 1,
        codigo: Some(c.codigo),
    })
    .unwrap();
    let e = cancelamento(&canc(&nfce, NPROT, "Cancelamento de teste")).unwrap_err();
    assert!(e.mensagem.contains("modelo 65"), "{e}");
}

// ------------------------------------------------------------------------------------------------ carta de correcao

fn carta<'a>(chave: &'a str, texto: &'a str, seq: u32) -> CartaCorrecao<'a> {
    CartaCorrecao {
        chave,
        correcao: texto,
        seq,
        ambiente: Homologacao,
        quando: quando(),
    }
}

#[test]
fn carta_de_correcao_leva_o_texto_fixo_oficial_e_a_sequencia_no_id() {
    let (_, chave) = nfe(5, 2);
    let m = carta_correcao(&carta(
        &chave,
        "Corrigir o endereco de entrega para Rua B, 20",
        3,
    ))
    .unwrap();
    assert_eq!(m.id, format!("ID110110{chave}03"));
    assert!(m.xml.contains("<nSeqEvento>3</nSeqEvento>"));
    assert!(m.xml.contains(&format!(
        "<detEvento versao=\"1.00\"><descEvento>Carta de Correcao</descEvento>\
         <xCorrecao>Corrigir o endereco de entrega para Rua B, 20</xCorrecao><xCondUso>{COND_USO}</xCondUso></detEvento>"
    )));
    // o texto fixo e' exatamente um dos dois do XSD: o sem acento
    assert!(COND_USO.starts_with("A Carta de Correcao e disciplinada pelo paragrafo 1o-A do art. 7o do Convenio S/N, de 15 de dezembro de 1970"));
    assert!(COND_USO.ends_with("III - a data de emissao ou de saida."));
    assert_eq!(COND_USO.len(), 525);
}

#[test]
fn carta_de_correcao_a_sequencia_vai_de_1_a_20_e_o_texto_de_15_a_1000() {
    let (_, chave) = nfe(6, 2);
    for seq in [0, 21, 99] {
        let e = carta_correcao(&carta(&chave, "Texto da correcao valido", seq)).unwrap_err();
        assert_eq!(e.campo, "nSeqEvento", "{seq}");
    }
    assert!(carta_correcao(&carta(&chave, "Texto da correcao valido", 1)).is_ok());
    assert!(carta_correcao(&carta(&chave, "Texto da correcao valido", 20)).is_ok());
    assert_eq!(
        carta_correcao(&carta(&chave, "curto", 1))
            .unwrap_err()
            .campo,
        "xCorrecao"
    );
    assert!(carta_correcao(&carta(&chave, &"y".repeat(1000), 1)).is_ok());
    assert_eq!(
        carta_correcao(&carta(&chave, &"y".repeat(1001), 1))
            .unwrap_err()
            .campo,
        "xCorrecao"
    );
}

// ------------------------------------------------------------------------------------------------ assinatura

#[test]
fn o_evento_sai_assinado_com_o_a1_e_dentro_do_envevento() {
    let (_, chave) = nfe(7, 2);
    let m = cancelamento(&canc(&chave, NPROT, "Cancelamento de teste")).unwrap();
    let id = m.id.clone();
    let p = m.assinar(&certificado(), "17").unwrap();
    assert_eq!(p.id, id);
    assert!(p.env_evento.starts_with(&format!(
        "<envEvento xmlns=\"{NS}\" versao=\"1.00\"><idLote>17</idLote><evento "
    )));
    assert!(p.env_evento.ends_with("</evento></envEvento>"));
    assert!(p.evento.contains("<Signature ") && p.evento.ends_with("</Signature></evento>"));
    // a assinatura confere, no evento solto e dentro do envEvento
    assert!(xmldsig::conferido(&p.evento));
    assert!(xmldsig::conferido(&p.env_evento));
    // adulterar o texto depois de assinar quebra a conferencia
    let adulterado = p
        .env_evento
        .replace("Cancelamento de teste", "Cancelamento de outro");
    assert!(!xmldsig::conferido(&adulterado));
    // idLote fora da forma
    let m = cancelamento(&canc(&chave, NPROT, "Cancelamento de teste")).unwrap();
    assert_eq!(
        m.assinar(&certificado(), "1234567890123456")
            .unwrap_err()
            .campo,
        "idLote"
    );
}

// ------------------------------------------------------------------------------------------------ EPEC

fn epec_de<'a>(nfe: &'a str) -> Epec<'a> {
    Epec {
        nfe,
        ambiente: Homologacao,
        quando: Utc
            .with_ymd_and_hms(2026, 10, 3, 12, 30, 0)
            .unwrap()
            .fixed_offset(),
        ver_aplic: "rustnf 0.4",
    }
}

#[test]
fn epec_sai_da_nfe_vai_ao_ambiente_nacional_e_leva_os_campos_da_nota() {
    let (xml, chave) = nfe_em_contingencia(31, 4);
    let k = campos_epec(&xml).unwrap();
    assert_eq!(k.chave, chave);
    assert_eq!(
        (
            k.tp_nf.as_str(),
            k.ie.as_str(),
            k.dest_uf.as_str(),
            k.dest_tag.as_str()
        ),
        ("1", "111111111111", "SP", "CPF")
    );
    assert_eq!(
        (k.v_nf.as_str(), k.v_icms.as_str(), k.v_st.as_str()),
        ("31.80", "0.00", "0.00")
    );
    let m = epec(&epec_de(&xml)).unwrap();
    assert_eq!(m.id, format!("ID110140{chave}01"));
    // cOrgao 91 = Ambiente Nacional; cOrgaoAutor = a UF do emitente
    assert!(m.xml.contains("<infEvento Id=\"ID110140"));
    assert!(m
        .xml
        .contains("<cOrgao>91</cOrgao><tpAmb>2</tpAmb><CNPJ>11222333000181</CNPJ>"));
    assert!(m.xml.contains(&format!(
        "<descEvento>EPEC</descEvento><cOrgaoAutor>35</cOrgaoAutor><tpAutor>1</tpAutor><verAplic>rustnf 0.4</verAplic>\
         <dhEmi>{}</dhEmi><tpNF>1</tpNF><IE>111111111111</IE><dest><UF>SP</UF><CPF>52998224725</CPF>\
         <vNF>31.80</vNF><vICMS>0.00</vICMS><vST>0.00</vST></dest>",
        k.dh_emi
    )));
    assert!(m.xml.contains("<nSeqEvento>1</nSeqEvento>"));
}

#[test]
fn epec_exige_tpemis_4_e_recusa_o_que_o_ambiente_nacional_recusaria() {
    // tpEmis 1: rejeicao 484
    let (normal, _) = nfe(32, 2);
    let e = epec(&epec_de(&normal)).unwrap_err();
    assert_eq!(e.campo, "tpEmis");
    assert!(e.mensagem.contains("484"));
    let (xml, _) = nfe_em_contingencia(33, 4);
    // depois do evento (577) e ha mais de 1 dia (228)
    let cedo = Epec {
        quando: Utc
            .with_ymd_and_hms(2026, 10, 3, 11, 0, 0)
            .unwrap()
            .fixed_offset(),
        ..epec_de(&xml)
    };
    assert!(epec(&cedo).unwrap_err().mensagem.contains("577"));
    let tarde = Epec {
        quando: Utc
            .with_ymd_and_hms(2026, 10, 5, 12, 0, 0)
            .unwrap()
            .fixed_offset(),
        ..epec_de(&xml)
    };
    assert!(epec(&tarde).unwrap_err().mensagem.contains("228"));
    // sem destinatario e IE do destinatario no exterior
    let sem_dest = xml
        .replace("<dest>", "<semdest>")
        .replace("</dest>", "</semdest>");
    assert_eq!(epec(&epec_de(&sem_dest)).unwrap_err().campo, "dest");
    // verAplic vazio
    let e = epec(&Epec {
        ver_aplic: "  ",
        ..epec_de(&xml)
    })
    .unwrap_err();
    assert_eq!(e.campo, "verAplic");
    // nao e' NF-e
    assert_eq!(epec(&epec_de("<x/>")).unwrap_err().campo, "NFe");
}

// ------------------------------------------------------------------------------------------------ eventos da apuracao

fn rtc<'a>(chave: &'a str, tipo: &'a str) -> EventoRtc<'a> {
    EventoRtc::novo(chave, tipo, 1, Homologacao, quando(), "rustnf 0.4")
}

fn item(n: u32) -> ItemRtc {
    ItemRtc {
        item: n,
        v_ibs: Some("10.5".into()),
        v_cbs: Some("5".into()),
        quantidade: Some("2".into()),
        unidade: Some("UN".into()),
        ..ItemRtc::default()
    }
}

#[test]
fn a_lista_de_eventos_da_apuracao_e_a_da_nt() {
    let tipos: Vec<&str> = EVENTOS.iter().map(|e| e.tipo).collect();
    assert_eq!(
        tipos,
        [
            "110001", "112110", "112120", "112130", "112140", "112150", "211110", "211124",
            "211128", "211130", "211140", "211150", "212110", "212120"
        ]
    );
    // o 211120 foi eliminado pela NT, mas ainda e' cancelavel (esta no XSD do 110001)
    assert!(tipo_rtc("211120").is_none());
    assert!(CANCELAVEIS.contains(&"211120"));
    assert!(tipo_rtc("412120").is_none());
}

#[test]
fn evento_de_apuracao_por_tipo_leva_o_cabecalho_o_autor_e_o_miolo_do_xsd() {
    let (_, chave) = nfe(40, 2);
    // 112110: indQuitacao 1, autor emitente
    let m = evento_rtc(&rtc(&chave, "112110")).unwrap();
    assert_eq!(m.id, format!("ID112110{chave}01"));
    assert!(m
        .xml
        .contains("<cOrgao>35</cOrgao><tpAmb>2</tpAmb><CNPJ>11222333000181</CNPJ>"));
    assert!(m.xml.ends_with(
        "<tpAutor>1</tpAutor><verAplic>rustnf 0.4</verAplic><indQuitacao>1</indQuitacao></detEvento></infEvento></evento>"
    ));
    // 112150: data de previsao
    let e = EventoRtc {
        data_entrega: chrono::NaiveDate::from_ymd_opt(2026, 11, 20),
        ..rtc(&chave, "112150")
    };
    assert!(evento_rtc(&e)
        .unwrap()
        .xml
        .contains("<dPrevEntrega>2026-11-20</dPrevEntrega>"));
    assert_eq!(
        evento_rtc(&rtc(&chave, "112150")).unwrap_err().campo,
        "dPrevEntrega"
    );
    // 112120: item com estoque
    let e = EventoRtc {
        itens: vec![item(1)],
        ..rtc(&chave, "112120")
    };
    assert!(evento_rtc(&e).unwrap().xml.contains(
        "<gConsumo nItem=\"1\"><vIBS>10.50</vIBS><vCBS>5.00</vCBS><gControleEstoque><qtde>2.0000</qtde><unidade>UN</unidade></gControleEstoque></gConsumo>"
    ));
    // 112130: o estorno do credito vai dentro do controle de estoque
    let it = ItemRtc {
        v_ibs_estorno: Some("1".into()),
        v_cbs_estorno: Some("2".into()),
        ..item(2)
    };
    let e = EventoRtc {
        itens: vec![it],
        ..rtc(&chave, "112130")
    };
    assert!(evento_rtc(&e).unwrap().xml.contains(
        "<gPerecimento nItem=\"2\"><vIBS>10.50</vIBS><vCBS>5.00</vCBS><gControleEstoque><qPerecimento>2.0000</qPerecimento><uPerecimento>UN</uPerecimento><vIBS>1.00</vIBS><vCBS>2.00</vCBS></gControleEstoque></gPerecimento>"
    ));
}

#[test]
fn evento_de_apuracao_do_destinatario_pede_o_autor_e_a_uf_dele() {
    let (_, chave) = nfe(41, 2);
    let it = ItemRtc {
        item: 1,
        v_ibs: Some("3".into()),
        v_cbs: Some("4".into()),
        ..ItemRtc::default()
    };
    let base = EventoRtc {
        itens: vec![it],
        ..rtc(&chave, "211150")
    };
    // sem autor: o autor seria o emitente e o tipo e' do destinatario
    assert_eq!(evento_rtc(&base).unwrap_err().campo, "CNPJ");
    let ok = EventoRtc {
        autor: Some("52998224725"),
        uf_autor: Some("SP"),
        ..base.clone()
    };
    let m = evento_rtc(&ok).unwrap();
    assert!(m.xml.contains("<CPF>52998224725</CPF>"));
    assert!(m
        .xml
        .contains("<cOrgaoAutor>35</cOrgaoAutor><tpAutor>2</tpAutor>"));
    assert!(m.xml.contains(
        "<gCredito nItem=\"1\"><vCredIBS>3.00</vCredIBS><vCredCBS>4.00</vCredCBS></gCredito>"
    ));
    // autor diferente do emitente sem informar a UF
    let sem_uf = EventoRtc {
        autor: Some("52998224725"),
        ..base.clone()
    };
    assert_eq!(evento_rtc(&sem_uf).unwrap_err().campo, "cOrgaoAutor");
    // evento do emitente (tpAutor 1) com autor de outro documento: rejeicao 574
    let it2 = item(1);
    let e = EventoRtc {
        itens: vec![it2],
        autor: Some("52998224725"),
        uf_autor: Some("SP"),
        ..rtc(&chave, "112120")
    };
    assert!(evento_rtc(&e).unwrap_err().mensagem.contains("574"));
}

#[test]
fn evento_de_apuracao_confere_item_repetido_campo_trocado_e_aceite() {
    let (_, chave) = nfe(42, 2);
    let e = EventoRtc {
        itens: vec![item(1), item(1)],
        ..rtc(&chave, "112120")
    };
    assert!(evento_rtc(&e).unwrap_err().mensagem.contains("repetido"));
    let sobra = ItemRtc {
        v_bc: Some("1".into()),
        ..item(1)
    };
    let e = EventoRtc {
        itens: vec![sobra],
        ..rtc(&chave, "112120")
    };
    assert!(evento_rtc(&e).unwrap_err().mensagem.contains("v_bc"));
    assert_eq!(
        evento_rtc(&rtc(&chave, "112120")).unwrap_err().campo,
        "gConsumo"
    );
    let e = EventoRtc {
        itens: vec![item(991)],
        ..rtc(&chave, "112120")
    };
    assert_eq!(evento_rtc(&e).unwrap_err().campo, "nItem");
    // 211128 (destinatario) e 212110 (sucessora): so' o aceite
    let e = EventoRtc {
        autor: Some("52998224725"),
        uf_autor: Some("SP"),
        ..rtc(&chave, "211128")
    };
    assert_eq!(evento_rtc(&e).unwrap_err().campo, "indAceitacao");
    let e = EventoRtc {
        aceite: Some(false),
        ..e
    };
    assert!(evento_rtc(&e)
        .unwrap()
        .xml
        .contains("<indAceitacao>0</indAceitacao>"));
    // valor com casa a mais e negativo
    let it = ItemRtc {
        v_ibs: Some("1.234".into()),
        ..item(1)
    };
    let e = EventoRtc {
        itens: vec![it],
        ..rtc(&chave, "112120")
    };
    assert!(evento_rtc(&e).unwrap_err().mensagem.contains("casas"));
    let it = ItemRtc {
        v_cbs: Some("-1".into()),
        ..item(1)
    };
    let e = EventoRtc {
        itens: vec![it],
        ..rtc(&chave, "112120")
    };
    assert!(evento_rtc(&e).unwrap_err().mensagem.contains("negativo"));
}

#[test]
fn credito_presumido_leva_os_dois_grupos_so_quando_informados() {
    let (_, chave) = nfe(43, 2);
    let it = ItemRtc {
        item: 1,
        v_bc: Some("100".into()),
        c_cred_pres: Some("08".into()),
        p_cred_ibs: Some("1.5".into()),
        v_cred_ibs: Some("1.5".into()),
        ..ItemRtc::default()
    };
    let e = EventoRtc {
        itens: vec![it.clone()],
        ..rtc(&chave, "211110")
    };
    let m = evento_rtc(&e).unwrap();
    assert!(m.xml.contains(
        "<gCredPresOper nItem=\"1\"><vBCCredPres>100.00</vBCCredPres><cCredPres>08</cCredPres><gIBSCredPres><pCredPres>1.50</pCredPres><vCredPres>1.50</vCredPres></gIBSCredPres></gCredPresOper>"
    ));
    assert!(!m.xml.contains("gCBSCredPres"));
    // 211110 aceita os dois autores: sem autor distinto vira 1 (emitente)
    assert!(m.xml.contains("<tpAutor>1</tpAutor>"));
    let ruim = ItemRtc {
        c_cred_pres: Some("1".into()),
        ..it
    };
    let e = EventoRtc {
        itens: vec![ruim],
        ..rtc(&chave, "211110")
    };
    assert_eq!(evento_rtc(&e).unwrap_err().campo, "cCredPres");
}

/// Um item do 211110 com o codigo e os grupos do IBS e da CBS que se pedem (percentual 1.5, valor 1.5).
fn cred_pres(codigo: &str, ibs: bool, cbs: bool) -> ItemRtc {
    let g = |t: bool| t.then(|| "1.5".to_string());
    ItemRtc {
        item: 3,
        v_bc: Some("100".into()),
        c_cred_pres: Some(codigo.into()),
        p_cred_ibs: g(ibs),
        v_cred_ibs: g(ibs),
        p_cred_cbs: g(cbs),
        v_cred_cbs: g(cbs),
        ..ItemRtc::default()
    }
}

fn recusa_cred_pres(codigo: &str, ibs: bool, cbs: bool) -> rustnf::nfe::sefaz::ErroSefaz {
    let (_, chave) = nfe(45, 2);
    let e = EventoRtc {
        itens: vec![cred_pres(codigo, ibs, cbs)],
        ..rtc(&chave, "211110")
    };
    evento_rtc(&e).unwrap_err()
}

#[test]
fn credito_presumido_2p25a_10_codigo_inexistente_1055() {
    for c in ["00", "14", "99"] {
        let e = recusa_cred_pres(c, true, true);
        assert_eq!(e.campo, "cCredPres", "{c}: {e}");
        assert!(
            e.mensagem.contains("2P25a-10")
                && e.mensagem.contains("1055")
                && e.mensagem.contains("nItem 3"),
            "{e}"
        );
    }
}

#[test]
fn credito_presumido_2p26_10_ibs_informado_onde_o_codigo_nao_permite_1053() {
    // 05 (regime automotivo) so' tem a CBS
    let e = recusa_cred_pres("05", true, true);
    assert_eq!(e.campo, "gIBSCredPres", "{e}");
    assert!(
        e.mensagem.contains("2P26-10") && e.mensagem.contains("1053"),
        "{e}"
    );
}

#[test]
fn credito_presumido_2p26_20_ibs_ausente_onde_o_codigo_exige_1054() {
    let e = recusa_cred_pres("08", false, false);
    assert_eq!(e.campo, "gIBSCredPres", "{e}");
    assert!(
        e.mensagem.contains("2P26-20") && e.mensagem.contains("1054"),
        "{e}"
    );
}

#[test]
fn credito_presumido_2p30_10_cbs_informada_onde_o_codigo_nao_permite_1050() {
    let e = recusa_cred_pres("08", true, true);
    assert_eq!(e.campo, "gCBSCredPres", "{e}");
    assert!(
        e.mensagem.contains("2P30-10") && e.mensagem.contains("1050"),
        "{e}"
    );
}

#[test]
fn credito_presumido_2p30_20_cbs_ausente_onde_o_codigo_exige_1058() {
    // o codigo antigo do teste (01) exige os dois: so' o IBS nao basta
    let e = recusa_cred_pres("01", true, false);
    assert_eq!(e.campo, "gCBSCredPres", "{e}");
    assert!(
        e.mensagem.contains("2P30-20") && e.mensagem.contains("1058"),
        "{e}"
    );
}

#[test]
fn credito_presumido_conferido_passa_com_os_grupos_que_a_tabela_pede() {
    let (_, chave) = nfe(46, 2);
    for (c, ibs, cbs, grupos) in [
        ("01", true, true, ["gIBSCredPres", "gCBSCredPres"]),
        ("05", false, true, ["", "gCBSCredPres"]),
        ("08", true, false, ["gIBSCredPres", ""]),
    ] {
        let e = EventoRtc {
            itens: vec![cred_pres(c, ibs, cbs)],
            ..rtc(&chave, "211110")
        };
        let m = evento_rtc(&e).unwrap_or_else(|x| panic!("{c}: {x}"));
        assert!(
            m.xml.contains(&format!("<cCredPres>{c}</cCredPres>")),
            "{c}"
        );
        for g in grupos.iter().filter(|g| !g.is_empty()) {
            assert!(m.xml.contains(&format!("<{g}>")), "{c}: {g}");
        }
    }
}

#[test]
fn cancelamento_de_evento_leva_o_evento_e_o_protocolo_dele() {
    let (_, chave) = nfe(44, 2);
    let e = EventoRtc {
        evento_cancelado: "112110",
        protocolo_evento: "135260000000099",
        ..rtc(&chave, "110001")
    };
    let m = evento_rtc(&e).unwrap();
    assert_eq!(m.id, format!("ID110001{chave}01"));
    assert!(m.xml.ends_with(
        "<descEvento>Cancelamento de Evento</descEvento><cOrgaoAutor>35</cOrgaoAutor><verAplic>rustnf 0.4</verAplic>\
         <tpEventoAut>112110</tpEventoAut><nProtEvento>135260000000099</nProtEvento></detEvento></infEvento></evento>"
    ));
    assert!(!m.xml.contains("<tpAutor>"));
    // o proprio 110001 nao se cancela; protocolo de 16 digitos; tpAutor nao existe aqui
    let e2 = EventoRtc {
        evento_cancelado: "110001",
        ..e.clone()
    };
    assert_eq!(evento_rtc(&e2).unwrap_err().campo, "tpEventoAut");
    let e3 = EventoRtc {
        protocolo_evento: "1352600000000999",
        ..e.clone()
    };
    assert_eq!(evento_rtc(&e3).unwrap_err().campo, "nProtEvento");
    let e4 = EventoRtc {
        tp_autor: Some("1"),
        ..e
    };
    assert_eq!(evento_rtc(&e4).unwrap_err().campo, "tpAutor");
}

#[test]
fn evento_que_nao_e_de_apuracao_e_a_sequencia_fora_de_1_a_99_sao_recusados() {
    let (_, chave) = nfe(45, 2);
    assert_eq!(
        evento_rtc(&rtc(&chave, "211120")).unwrap_err().campo,
        "tpEvento"
    );
    assert_eq!(
        evento_rtc(&rtc(&chave, "412120")).unwrap_err().campo,
        "tpEvento"
    );
    let e = EventoRtc {
        seq: 100,
        ..rtc(&chave, "112110")
    };
    assert_eq!(evento_rtc(&e).unwrap_err().campo, "nSeqEvento");
}
