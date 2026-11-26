// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A manifestacao do destinatario: o evento montado (os quatro tipos, `cOrgao` 91, `nSeqEvento` 1, justificativa por
//! tipo), as combinacoes que o MOC recusa e o cliente com transporte falso (registrada, sem vinculo, rejeitada, resposta
//! perdida com a consulta que acha, que nao acha e que tambem nao responde, duplicidade, certificado de outro CNPJ-base).

mod nfe_sefaz_comum;

use chrono::{DateTime, FixedOffset, TimeZone};
use nfe_sefaz_comum::dfe::{CNPJ, CPF};
use nfe_sefaz_comum::eventos::*;
use nfe_sefaz_comum::*;
use rustnf::assinatura::cert_a1;
use rustnf::nfe::sefaz::dfe::SituacaoResumo;
use rustnf::nfe::sefaz::evento::{
    conferir_combinacao, manifestacao, Manifestacao, TipoManifestacao as T, CIENCIA, CONFIRMACAO,
    DESCONHECIMENTO, NAO_REALIZADA,
};
use rustnf::nfe::sefaz::resposta_evento::ler_proc_evento;
use rustnf::nfe::sefaz::DesfechoEvento;
use rustnf::nfe::vigencias::Ambiente::Homologacao;

const URL_EVENTO_AN: &str =
    "https://hom1.nfe.fazenda.gov.br/NFeRecepcaoEvento4/NFeRecepcaoEvento4.asmx";
const JUST: &str = "Mercadoria nao foi entregue no endereco";

fn quando() -> DateTime<FixedOffset> {
    FixedOffset::west_opt(3 * 3600)
        .unwrap()
        .with_ymd_and_hms(2026, 11, 26, 9, 0, 0)
        .unwrap()
}

fn pedido_de<'a>(chave: &'a str, tipo: T, just: Option<&'a str>) -> Manifestacao<'a> {
    Manifestacao {
        documento: CNPJ,
        chave,
        tipo,
        justificativa: just,
        situacao_nfe: None,
        anterior: None,
        uf_autor: "SP",
        ambiente: Homologacao,
        quando: quando(),
    }
}

// ------------------------------------------------------------------------------------------------ o evento montado

#[test]
fn os_quatro_eventos_saem_com_orgao_91_sequencia_1_e_o_destinatario_como_autor() {
    let (_, chave) = nfe(1, 2);
    for (tipo, tp, desc) in [
        (T::Confirmacao, CONFIRMACAO, "Confirmacao da Operacao"),
        (T::Ciencia, CIENCIA, "Ciencia da Operacao"),
        (
            T::Desconhecimento,
            DESCONHECIMENTO,
            "Desconhecimento da Operacao",
        ),
        (T::NaoRealizada, NAO_REALIZADA, "Operacao nao Realizada"),
    ] {
        let just = (tipo == T::NaoRealizada).then_some(JUST);
        let m = manifestacao(&pedido_de(&chave, tipo, just)).unwrap();
        assert_eq!(m.id, format!("ID{tp}{chave}01"));
        assert_eq!((m.tp_evento.as_str(), m.seq), (tp, 1));
        let detalhe = match just {
            Some(j) => format!("<detEvento versao=\"1.00\"><descEvento>{desc}</descEvento><xJust>{j}</xJust></detEvento>"),
            None => format!("<detEvento versao=\"1.00\"><descEvento>{desc}</descEvento></detEvento>"),
        };
        let esperado = format!(
            "<evento xmlns=\"{NS}\" versao=\"1.00\"><infEvento Id=\"ID{tp}{chave}01\"><cOrgao>91</cOrgao><tpAmb>2</tpAmb>\
             <CNPJ>{CNPJ}</CNPJ><chNFe>{chave}</chNFe><dhEvento>2026-11-26T09:00:00-03:00</dhEvento>\
             <tpEvento>{tp}</tpEvento><nSeqEvento>1</nSeqEvento><verEvento>1.00</verEvento>{detalhe}</infEvento></evento>"
        );
        assert_eq!(m.xml, esperado, "{tp}");
        assert_eq!(T::de_codigo(tp), Some(tipo));
    }
    assert_eq!(T::de_codigo("110111"), None);
}

#[test]
fn destinatario_pessoa_fisica_leva_cpf_e_o_fuso_e_o_da_uf_do_autor() {
    let (_, chave) = nfe(2, 2);
    let mut p = pedido_de(&chave, T::Ciencia, None);
    p.documento = CPF;
    p.uf_autor = "AM";
    let m = manifestacao(&p).unwrap();
    assert!(
        m.xml.contains(&format!("<CPF>{CPF}</CPF><chNFe>")),
        "{}",
        m.xml
    );
    // Manaus: o mesmo instante, escrito em -04:00
    assert!(
        m.xml
            .contains("<dhEvento>2026-11-26T08:00:00-04:00</dhEvento>"),
        "{}",
        m.xml
    );
}

#[test]
fn justificativa_obrigatoria_na_nao_realizada_opcional_no_desconhecimento_proibida_nas_outras() {
    let (_, chave) = nfe(3, 2);
    let erro = |t: T, j: Option<&str>| manifestacao(&pedido_de(&chave, t, j)).unwrap_err();
    // 595
    let e = erro(T::NaoRealizada, None);
    assert_eq!(e.campo, "xJust");
    assert!(e.mensagem.contains("595"), "{e}");
    assert_eq!(erro(T::NaoRealizada, Some("   ")).campo, "xJust");
    // 15 a 255 caracteres
    assert!(erro(T::NaoRealizada, Some("curta demais"))
        .mensagem
        .contains("minimo de 15"));
    assert!(erro(T::Desconhecimento, Some("curta demais"))
        .mensagem
        .contains("minimo de 15"));
    let longa = "x".repeat(256);
    assert!(erro(T::NaoRealizada, Some(&longa))
        .mensagem
        .contains("maximo 255"));
    let limite = "x".repeat(255);
    assert!(manifestacao(&pedido_de(&chave, T::NaoRealizada, Some(&limite))).is_ok());
    // o desconhecimento aceita com e sem (o esquema e210220 a tem como opcional)
    assert!(manifestacao(&pedido_de(&chave, T::Desconhecimento, None)).is_ok());
    let m = manifestacao(&pedido_de(&chave, T::Desconhecimento, Some(JUST))).unwrap();
    assert!(m.xml.contains(&format!("<xJust>{JUST}</xJust>")));
    // confirmacao e ciencia nao levam
    for t in [T::Confirmacao, T::Ciencia] {
        let e = erro(t, Some(JUST));
        assert_eq!(e.campo, "xJust", "{t:?}");
        assert!(e.mensagem.contains("nao leva justificativa"), "{e}");
    }
}

#[test]
fn texto_da_justificativa_sai_sem_acento_e_escapado() {
    let (_, chave) = nfe(4, 2);
    let m = manifestacao(&pedido_de(
        &chave,
        T::NaoRealizada,
        Some("Nao recebi: a mercadoria veio \u{e0} <outra> loja"),
    ))
    .unwrap();
    assert!(
        m.xml
            .contains("<xJust>Nao recebi: a mercadoria veio a &lt;outra&gt; loja</xJust>"),
        "{}",
        m.xml
    );
}

#[test]
fn chave_destinatario_e_uf_invalidos_sao_recusados_antes_de_montar() {
    let (_, chave) = nfe(5, 2);
    let mut p = pedido_de("123", T::Ciencia, None);
    assert_eq!(manifestacao(&p).unwrap_err().campo, "chNFe");
    p.chave = &chave;
    p.documento = "11222333000182";
    assert_eq!(manifestacao(&p).unwrap_err().campo, "CNPJ");
    p.documento = "52998224726";
    assert_eq!(manifestacao(&p).unwrap_err().campo, "CPF");
    p.documento = CNPJ;
    p.uf_autor = "XX";
    assert_eq!(manifestacao(&p).unwrap_err().campo, "uf");
    // chave de NFC-e (modelo 65): a manifestacao e' so' da NF-e
    let nfce = format!("{}65{}", &chave[..20], &chave[22..]);
    p.uf_autor = "SP";
    p.chave = &nfce;
    assert_eq!(manifestacao(&p).unwrap_err().campo, "chNFe");
}

// ------------------------------------------------------------------------------------------------ as combinacoes do MOC

#[test]
fn combinacoes_que_o_ambiente_nacional_recusaria_nao_saem_da_maquina() {
    let canc = SituacaoResumo::Cancelada;
    let nega = SituacaoResumo::Denegada;
    let aut = SituacaoResumo::Autorizada;
    let erro =
        |t, s: Option<&SituacaoResumo>, a| conferir_combinacao(t, s, a).unwrap_err().mensagem;
    // H04 e H05: ciencia e desconhecimento de nota cancelada ou denegada
    assert!(erro(T::Ciencia, Some(&canc), None).contains("650"));
    assert!(erro(T::Ciencia, Some(&nega), None).contains("650"));
    assert!(erro(T::Desconhecimento, Some(&canc), None).contains("651"));
    assert!(erro(T::Desconhecimento, Some(&nega), None).contains("651"));
    // a confirmacao e a nao realizada de nota cancelada nao tem regra no MOC: a SEFAZ decide
    assert!(conferir_combinacao(T::Confirmacao, Some(&canc), None).is_ok());
    assert!(conferir_combinacao(T::NaoRealizada, Some(&nega), None).is_ok());
    // H06: ciencia depois da manifestacao conclusiva
    for conclusiva in [T::Confirmacao, T::Desconhecimento, T::NaoRealizada] {
        assert!(
            erro(T::Ciencia, Some(&aut), Some(conclusiva)).contains("655"),
            "{conclusiva:?}"
        );
    }
    // o mesmo evento duas vezes: 573 (nSeqEvento e' sempre 1)
    for t in [
        T::Confirmacao,
        T::Ciencia,
        T::Desconhecimento,
        T::NaoRealizada,
    ] {
        assert!(erro(t, None, Some(t)).contains("573"), "{t:?}");
    }
    // mudar de ideia entre as conclusivas vale (MOC 3.2.1.5), e a ciencia abre caminho para qualquer uma
    assert!(conferir_combinacao(T::Confirmacao, Some(&aut), Some(T::Desconhecimento)).is_ok());
    assert!(conferir_combinacao(T::Desconhecimento, Some(&aut), Some(T::Confirmacao)).is_ok());
    for t in [T::Confirmacao, T::Desconhecimento, T::NaoRealizada] {
        assert!(conferir_combinacao(t, Some(&aut), Some(T::Ciencia)).is_ok());
    }
}

#[test]
fn a_combinacao_recusada_para_a_montagem_do_evento() {
    let (_, chave) = nfe(6, 2);
    let mut p = pedido_de(&chave, T::Ciencia, None);
    p.situacao_nfe = Some(SituacaoResumo::Cancelada);
    let e = manifestacao(&p).unwrap_err();
    assert_eq!(e.campo, "tpEvento");
    assert!(
        e.mensagem.contains("650") && e.mensagem.contains("nao foi enviado"),
        "{e}"
    );
}

// ------------------------------------------------------------------------------------------------ o cliente

fn ret_manifestacao(chave: &str, tp: &str, cstat: &str, motivo: &str, nprot: &str) -> String {
    ret_lote(chave, tp, 1, cstat, motivo, nprot)
}

#[tokio::test]
async fn ciencia_registrada_vai_assinada_ao_ambiente_nacional_e_guarda_o_proc_evento() {
    let (_, chave) = nfe(10, 2);
    let f = Falso::novo(vec![(
        EVT,
        ok(
            EVT,
            &ret_manifestacao(
                &chave,
                CIENCIA,
                "135",
                "Evento registrado e vinculado a NF-e",
                NPROT_EVT,
            ),
        ),
    )]);
    let (c, _) = cliente(f.clone());
    let r = c
        .manifestar(&certificado(), &pedido_de(&chave, T::Ciencia, None))
        .await
        .unwrap();

    assert!(r.registrado());
    assert_eq!(
        r.frase,
        format!("Evento de ciencia da operacao registrado: protocolo {NPROT_EVT}.")
    );
    assert_eq!(
        (r.tp_evento.as_str(), r.seq, r.autorizador.as_str()),
        (CIENCIA, 1, "AN")
    );
    let proc_ = r.proc_evento.as_deref().unwrap();
    let lido = ler_proc_evento(proc_).unwrap();
    assert_eq!((lido.tp_evento.as_str(), lido.seq), (CIENCIA, Some(1)));
    assert_eq!(lido.id, format!("ID{CIENCIA}{chave}01"));
    assert!(proc_.contains("<Signature "));
    let ch = f.chamadas();
    assert_eq!(ch.len(), 1);
    assert_eq!(ch[0].url, URL_EVENTO_AN);
    assert_eq!(
        ch[0].content_type,
        "application/soap+xml; charset=utf-8; action=\"http://www.portalfiscal.inf.br/nfe/wsdl/NFeRecepcaoEvento4/nfeRecepcaoEvento\""
    );
    assert!(ch[0].corpo.contains("<cOrgao>91</cOrgao>"));
    assert!(ch[0]
        .corpo
        .contains(&format!("<CNPJ>{CNPJ}</CNPJ><chNFe>{chave}</chNFe>")));
    assert!(rustnf::assinatura::xmldsig::conferido(&r.xml_enviado));
}

#[tokio::test]
async fn o_cliente_usa_o_ambiente_e_a_hora_dele_e_nao_os_do_pedido() {
    let (_, chave) = nfe(11, 2);
    let f = Falso::novo(vec![(
        EVT,
        ok(
            EVT,
            &ret_manifestacao(&chave, CONFIRMACAO, "135", "ok", NPROT_EVT),
        ),
    )]);
    let (c, _) = cliente(f.clone());
    let mut p = pedido_de(&chave, T::Confirmacao, None);
    p.ambiente = rustnf::nfe::vigencias::Ambiente::Producao;
    p.uf_autor = "AM";
    p.quando = FixedOffset::east_opt(0)
        .unwrap()
        .with_ymd_and_hms(2001, 1, 1, 0, 0, 0)
        .unwrap();
    c.manifestar(&certificado(), &p).await.unwrap();
    let corpo = &f.chamadas()[0].corpo;
    // o cliente do teste e' de SP, em homologacao, as 12:00 UTC de 03/10/2026
    assert!(corpo.contains("<tpAmb>2</tpAmb>"), "{corpo}");
    assert!(
        corpo.contains("<dhEvento>2026-10-03T09:00:00-03:00</dhEvento>"),
        "{corpo}"
    );
}

#[tokio::test]
async fn manifestacao_136_nao_e_registro_e_o_155_nao_e_da_manifestacao() {
    let (_, chave) = nfe(12, 2);
    // 136: a SEFAZ guardou o evento mas a NF-e nao existia: sem vinculo
    let f = Falso::novo(vec![(
        EVT,
        ok(
            EVT,
            &ret_manifestacao(
                &chave,
                CONFIRMACAO,
                "136",
                "Evento registrado, mas nao vinculado a NF-e",
                NPROT_EVT,
            ),
        ),
    )]);
    let (c, _) = cliente(f);
    let r = c
        .manifestar(&certificado(), &pedido_de(&chave, T::Confirmacao, None))
        .await
        .unwrap();
    assert!(matches!(r.desfecho, DesfechoEvento::SemVinculo { .. }));
    assert!(!r.registrado());
    // 155 e' "cancelamento homologado fora de prazo": numa manifestacao nao e' registro
    let f = Falso::novo(vec![(
        EVT,
        ok(
            EVT,
            &ret_manifestacao(
                &chave,
                CONFIRMACAO,
                "155",
                "Cancelamento homologado fora de prazo",
                NPROT_EVT,
            ),
        ),
    )]);
    let (c, _) = cliente(f);
    let r = c
        .manifestar(&certificado(), &pedido_de(&chave, T::Confirmacao, None))
        .await
        .unwrap();
    assert!(
        matches!(r.desfecho, DesfechoEvento::Rejeitado { ref cstat, .. } if cstat == "155"),
        "{:?}",
        r.desfecho
    );
    assert!(!r.registrado() && r.proc_evento.is_none());
}

#[tokio::test]
async fn rejeicoes_do_moc_voltam_com_o_codigo_e_o_motivo() {
    let (_, chave) = nfe(13, 2);
    for (tipo, tp, cstat, motivo) in [
        (T::NaoRealizada, NAO_REALIZADA, "596", "Rejeicao: Evento apresentado fora do prazo: [180 dias]"),
        (T::Ciencia, CIENCIA, "655", "Rejeicao: Evento de Ciencia da Emissao informado apos a manifestacao final do destinatario"),
        (T::Desconhecimento, DESCONHECIMENTO, "651", "Rejeicao: Evento de Desconhecimento da Operacao para NF-e Cancelada ou Denegada"),
        (T::Confirmacao, CONFIRMACAO, "658", "Rejeicao: UF do destinatario da Chave de Acesso diverge da UF autorizadora"),
    ] {
        let f = Falso::novo(vec![(EVT, ok(EVT, &ret_manifestacao(&chave, tp, cstat, motivo, "")))]);
        let (c, _) = cliente(f.clone());
        let just = (tipo == T::NaoRealizada).then_some(JUST);
        let r = c
            .manifestar(&certificado(), &pedido_de(&chave, tipo, just))
            .await
            .unwrap();
        assert_eq!(r.desfecho, DesfechoEvento::Rejeitado { cstat: cstat.into(), motivo: motivo.into() }, "{cstat}");
        assert!(r.frase.contains(cstat) && r.frase.contains(motivo), "{}", r.frase);
        assert!(!r.pode_reenviar() && !r.registrado());
        assert_eq!(f.chamadas().len(), 1, "{cstat}: rejeicao nao consulta nem reenvia");
    }
}

// ------------------------------------------------------------------------------------------------ resposta perdida

fn consulta_com(chave: &str, tp: &str, nprot: &str) -> String {
    let miolo = format!(
        "{}{}",
        prot_ok(chave),
        proc_na_consulta(chave, tp, 1, "135", nprot)
    );
    ret_sit(chave, "100", "Autorizado o uso da NF-e", &miolo)
}

#[tokio::test]
async fn sem_resposta_a_consulta_acha_a_manifestacao_e_nao_se_envia_outra() {
    let (_, chave) = nfe(14, 2);
    let f = Falso::novo(vec![
        (EVT, tempo()),
        (CON, ok(CON, &consulta_com(&chave, CONFIRMACAO, NPROT_EVT))),
    ]);
    let (c, _) = cliente(f.clone());
    let r = c
        .manifestar(&certificado(), &pedido_de(&chave, T::Confirmacao, None))
        .await
        .unwrap();
    assert!(r.registrado() && r.recuperado);
    assert!(r.detalhe.contains("sem resposta"), "{}", r.detalhe);
    assert_eq!(f.chamadas().len(), 2);
    assert_eq!(f.restantes(), 0);
    // a consulta vai ao autorizador da UF da chave (SP), nao ao Ambiente Nacional
    assert!(
        f.chamadas()[1].url.contains("sp.gov.br"),
        "{}",
        f.chamadas()[1].url
    );
}

#[tokio::test]
async fn sem_resposta_e_a_consulta_sem_o_evento_diz_para_reenviar_o_mesmo() {
    let (_, chave) = nfe(15, 2);
    let sem = ret_sit(&chave, "100", "Autorizado o uso da NF-e", &prot_ok(&chave));
    let f = Falso::novo(vec![(EVT, tempo()), (CON, ok(CON, &sem))]);
    let (c, _) = cliente(f);
    let r = c
        .manifestar(&certificado(), &pedido_de(&chave, T::Ciencia, None))
        .await
        .unwrap();
    assert_eq!(r.desfecho, DesfechoEvento::NaoRegistrado);
    assert!(r.pode_reenviar());
    assert_eq!(r.seq, 1);
    assert!(
        r.frase.contains("MESMO") && r.frase.contains("ciencia"),
        "{}",
        r.frase
    );
}

#[tokio::test]
async fn sem_resposta_nem_da_consulta_manda_consultar_de_novo() {
    let (_, chave) = nfe(16, 2);
    let f = Falso::novo(vec![(EVT, tempo()), (CON, tempo())]);
    let (c, _) = cliente(f);
    let r = c
        .manifestar(&certificado(), &pedido_de(&chave, T::Confirmacao, None))
        .await
        .unwrap();
    assert!(r.deve_consultar() && !r.pode_reenviar());
}

#[tokio::test]
async fn duplicidade_573_devolve_a_manifestacao_que_ja_esta_registrada() {
    let (_, chave) = nfe(17, 2);
    let dup = ret_manifestacao(
        &chave,
        CONFIRMACAO,
        "573",
        "Rejeicao: Duplicidade de Evento",
        "",
    );
    let f = Falso::novo(vec![
        (EVT, ok(EVT, &dup)),
        (CON, ok(CON, &consulta_com(&chave, CONFIRMACAO, NPROT_EVT))),
    ]);
    let (c, _) = cliente(f);
    let r = c
        .manifestar(&certificado(), &pedido_de(&chave, T::Confirmacao, None))
        .await
        .unwrap();
    assert!(r.registrado() && r.recuperado);
}

// ------------------------------------------------------------------------------------------------ certificado e combinacao no cliente

#[tokio::test]
async fn certificado_de_outro_cnpj_base_ou_de_cpf_nao_assina_pelo_destinatario() {
    let (_, chave) = nfe(18, 2);
    let f = Falso::novo(vec![]);
    let (c, _) = cliente(f.clone());
    // o certificado do teste e' do CNPJ 11222333000181; o destinatario e' o CPF
    let mut p = pedido_de(&chave, T::Ciencia, None);
    p.documento = CPF;
    let e = c.manifestar(&certificado(), &p).await.unwrap_err();
    assert_eq!(e.campo, "certificado");
    // certificado de PF no CNPJ
    let pfx = cert_a1::gerar_teste(
        CPF,
        "PESSOA TESTE",
        "teste",
        365,
        true,
        false,
        chrono::Utc::now(),
    )
    .unwrap();
    let cert_pf = cert_a1::carregar(&pfx, "teste").unwrap();
    let e = c
        .manifestar(&cert_pf, &pedido_de(&chave, T::Ciencia, None))
        .await
        .unwrap_err();
    assert_eq!(e.campo, "certificado");
    assert!(f.chamadas().is_empty());
}

#[tokio::test]
async fn combinacao_recusada_nem_chega_a_rede() {
    let (_, chave) = nfe(19, 2);
    let f = Falso::novo(vec![]);
    let (c, _) = cliente(f.clone());
    let mut p = pedido_de(&chave, T::Ciencia, None);
    p.anterior = Some(T::Confirmacao);
    let e = c.manifestar(&certificado(), &p).await.unwrap_err();
    assert!(e.mensagem.contains("655"), "{e}");
    assert!(f.chamadas().is_empty());
}
