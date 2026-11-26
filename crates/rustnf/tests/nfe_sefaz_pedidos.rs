// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Os pedidos dos webservices (`consStatServ`, `enviNFe`, `consReciNFe`, `consSitNFe`), o envelope SOAP 1.2 e a leitura
//! das respostas. Parte pura, sem rede.

mod nfe_sefaz_comum;

use nfe_sefaz_comum::*;
use rustnf::nfe::sefaz::pedidos::{
    chave_valida, conferir_nfe, cons_reci, cons_sit, cons_stat, envelope, envi_nfe, sem_declaracao,
};
use rustnf::nfe::sefaz::resposta::{
    desembrulhar, ler_consulta, ler_envio, ler_recibo, ler_status, nfe_proc, ErroResposta,
};
use rustnf::nfe::sefaz::Servico;
use rustnf::nfe::vigencias::Ambiente::{Homologacao, Producao};

#[test]
fn consstatserv_leva_o_codigo_da_uf_e_o_ambiente() {
    assert_eq!(
        cons_stat(Homologacao, "SP").unwrap(),
        "<consStatServ xmlns=\"http://www.portalfiscal.inf.br/nfe\" versao=\"4.00\"><tpAmb>2</tpAmb><cUF>35</cUF>\
         <xServ>STATUS</xServ></consStatServ>"
    );
    // a UF consultada e' a do emitente, mesmo quando o autorizador e' a SVRS
    assert!(cons_stat(Producao, "RN")
        .unwrap()
        .contains("<tpAmb>1</tpAmb><cUF>24</cUF>"));
    assert_eq!(cons_stat(Producao, "xx").unwrap_err().campo, "uf");
}

#[test]
fn consreci_pede_o_recibo_de_15_digitos() {
    assert_eq!(
        cons_reci(Homologacao, " 351000000000042 ").unwrap(),
        "<consReciNFe xmlns=\"http://www.portalfiscal.inf.br/nfe\" versao=\"4.00\"><tpAmb>2</tpAmb>\
         <nRec>351000000000042</nRec></consReciNFe>"
    );
    for ruim in ["", "1234", "35100000000004a", "3510000000000421", "<x/>"] {
        assert_eq!(
            cons_reci(Homologacao, ruim).unwrap_err().campo,
            "nRec",
            "{ruim}"
        );
    }
}

#[test]
fn conssit_pede_a_chave_valida_de_nfe() {
    let (_, chave) = nfe(10, 2);
    assert_eq!(
        cons_sit(Producao, &chave).unwrap(),
        format!(
            "<consSitNFe xmlns=\"http://www.portalfiscal.inf.br/nfe\" versao=\"4.00\"><tpAmb>1</tpAmb>\
             <xServ>CONSULTAR</xServ><chNFe>{chave}</chNFe></consSitNFe>"
        )
    );
    // com espacos e pontuacao a chave sai limpa
    let formatada = rustnf::chave_dfe::formatar(&chave);
    assert!(cons_sit(Producao, &formatada).unwrap().contains(&chave));
    // digito verificador errado
    let mut ruim = chave.clone();
    ruim.replace_range(43.., if chave.ends_with('0') { "1" } else { "0" });
    assert_eq!(cons_sit(Producao, &ruim).unwrap_err().campo, "chNFe");
    assert!(cons_sit(Producao, "123").is_err());
    // NFC-e (modelo 65) nao e' deste cliente
    let nfce = rustnf::chave_dfe::montar(&rustnf::chave_dfe::Montar {
        cuf: 35,
        aamm: "2610",
        emitente: "11222333000181",
        modelo: 65,
        numero: 7,
        ..Default::default()
    })
    .unwrap();
    let e = chave_valida(&nfce).unwrap_err();
    assert_eq!(e.campo, "chNFe");
    assert!(e.mensagem.contains("modelo 65"), "{e}");
    assert!(cons_sit(Producao, &nfce).is_err());
}

#[test]
fn a_nfe_vai_no_envinfe_exatamente_como_foi_assinada() {
    let (xml, chave) = nfe(11, 2);
    let lote = envi_nfe(&[xml.as_str()], "42", true).unwrap();
    assert_eq!(
        lote,
        format!(
            "<enviNFe xmlns=\"http://www.portalfiscal.inf.br/nfe\" versao=\"4.00\"><idLote>42</idLote>\
             <indSinc>1</indSinc>{xml}</enviNFe>"
        )
    );
    assert!(lote.contains(&chave));
    // com a declaracao <?xml?> na frente, so' ela sai; o resto, byte a byte
    let com_decl = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{xml}\n");
    assert_eq!(envi_nfe(&[com_decl.as_str()], "42", true).unwrap(), lote);
    assert!(envi_nfe(&[xml.as_str()], "42", false)
        .unwrap()
        .contains("<indSinc>0</indSinc>"));
}

#[test]
fn texto_dentro_da_nfe_nao_e_reescrito() {
    // um comentario dentro da NFe e a ordem dos atributos ficam como estao: so' a declaracao do inicio sai
    let (xml, _) = nfe(12, 2);
    let com_comentario = xml.replacen("<mod>55</mod>", "<mod>55</mod><!-- nota -->", 1);
    let lote = envi_nfe(&[com_comentario.as_str()], "1", true).unwrap();
    assert!(lote.contains("<mod>55</mod><!-- nota -->"));
    assert!(lote.contains(&com_comentario));
}

#[test]
fn lote_sincrono_leva_uma_nota_e_o_lote_vai_de_1_a_50() {
    let (a, _) = nfe(21, 2);
    let (b, _) = nfe(22, 2);
    let e = envi_nfe(&[a.as_str(), b.as_str()], "1", true).unwrap_err();
    assert_eq!(e.campo, "indSinc");
    assert!(e.mensagem.contains("764"));
    let duas = envi_nfe(&[a.as_str(), b.as_str()], "1", false).unwrap();
    assert_eq!(duas.matches("<NFe ").count(), 2);
    assert_eq!(envi_nfe(&[], "1", false).unwrap_err().campo, "NFe");
    let muitas: Vec<&str> = (0..51).map(|_| a.as_str()).collect();
    assert!(envi_nfe(&muitas, "1", false)
        .unwrap_err()
        .mensagem
        .contains("50"));
    let cinquenta: Vec<&str> = (0..50).map(|_| a.as_str()).collect();
    assert!(envi_nfe(&cinquenta, "1", false).is_ok());
}

#[test]
fn id_do_lote_de_1_a_15_digitos() {
    let (a, _) = nfe(31, 2);
    for ruim in ["", "  ", "1234567890123456", "12a", "1 2", "-1"] {
        assert_eq!(
            envi_nfe(&[a.as_str()], ruim, true).unwrap_err().campo,
            "idLote",
            "{ruim}"
        );
    }
    assert!(envi_nfe(&[a.as_str()], "123456789012345", true).is_ok());
    assert!(envi_nfe(&[a.as_str()], "000000000000007", true)
        .unwrap()
        .contains("<idLote>000000000000007</idLote>"));
}

#[test]
fn nfe_que_nao_e_so_a_nfe_assinada_nao_entra_no_envelope() {
    let (xml, _) = nfe(41, 2);
    // elemento a mais depois da NFe (injecao no envelope)
    let extra = format!("{xml}<evil/>");
    assert_eq!(conferir_nfe(&extra).unwrap_err().campo, "NFe");
    assert!(envi_nfe(&[extra.as_str()], "1", true).is_err());
    // um elemento que so' comeca com NFe
    let outro = xml.replace("<NFe ", "<NFeX ").replace("</NFe>", "</NFeX>");
    assert!(conferir_nfe(&outro).is_err());
    // comentario antes da raiz
    let coment = format!("<!-- x -->{xml}");
    assert!(conferir_nfe(&coment).is_err());
    // mal formado, sem assinatura, sem infNFe, de outro espaco de nomes
    assert!(conferir_nfe("<NFe").is_err());
    assert_eq!(conferir_nfe("").unwrap_err().campo, "NFe");
    let sem_ass = {
        let i = xml.find("<Signature").unwrap();
        format!("{}</NFe>", &xml[..i])
    };
    assert_eq!(conferir_nfe(&sem_ass).unwrap_err().campo, "Signature");
    let outro_ns = xml.replace(
        "xmlns=\"http://www.portalfiscal.inf.br/nfe\"",
        "xmlns=\"http://exemplo\"",
    );
    assert_eq!(conferir_nfe(&outro_ns).unwrap_err().campo, "NFe");
    // DTD/entidade
    let dtd = format!("<!DOCTYPE NFe [<!ENTITY a \"b\">]>{xml}");
    assert!(conferir_nfe(&dtd).is_err());
}

#[test]
fn a_conferencia_devolve_o_texto_cru_a_chave_e_o_ambiente() {
    let (xml, chave) = nfe(51, 1);
    let c = conferir_nfe(&xml).unwrap();
    assert_eq!(c.texto, xml);
    assert_eq!(c.chave.chave, chave);
    assert_eq!(c.chave.uf, "SP");
    assert_eq!(c.tp_amb, "1");
}

#[test]
fn sem_declaracao_tira_bom_declaracao_e_espaco_das_pontas() {
    assert_eq!(
        sem_declaracao("\u{feff}  <?xml version=\"1.0\"?>\n<a/>\n"),
        "<a/>"
    );
    assert_eq!(sem_declaracao("<a/>"), "<a/>");
    assert_eq!(sem_declaracao("<?xml sem fim <a/>"), "<?xml sem fim <a/>");
}

#[test]
fn envelope_soap_12_com_o_namespace_do_servico_e_a_acao_no_content_type() {
    let e = envelope(
        "<?xml version=\"1.0\"?><consStatServ/>",
        Servico::StatusServico,
    );
    let corpo = String::from_utf8(e.corpo).unwrap();
    assert_eq!(
        corpo,
        "<soap12:Envelope xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" \
         xmlns:xsd=\"http://www.w3.org/2001/XMLSchema\" xmlns:soap12=\"http://www.w3.org/2003/05/soap-envelope\">\
         <soap12:Body><nfeDadosMsg xmlns=\"http://www.portalfiscal.inf.br/nfe/wsdl/NFeStatusServico4\">\
         <consStatServ/></nfeDadosMsg></soap12:Body></soap12:Envelope>"
    );
    // sem o cabecalho nfeCabecMsg (eliminado no leiaute 4.00)
    assert!(!corpo.contains("Header") && !corpo.contains("nfeCabecMsg"));
    assert_eq!(
        e.content_type,
        "application/soap+xml; charset=utf-8; action=\"http://www.portalfiscal.inf.br/nfe/wsdl/NFeStatusServico4/nfeStatusServicoNF\""
    );
    // o envelope e' XML bem formado
    assert!(roxmltree::Document::parse(&corpo).is_ok());
    for s in Servico::TODOS {
        let c = String::from_utf8(envelope("<x/>", s).corpo).unwrap();
        assert!(c.contains(&format!("<nfeDadosMsg xmlns=\"{}\">", s.namespace())));
    }
}

// ------------------------------------------------------------------------------------------------ respostas

#[test]
fn desembrulhar_tira_o_retorno_do_body() {
    let (_, chave) = nfe(61, 2);
    let ret = ret_envi_sinc(&prot_ok(&chave));
    let r = desembrulhar(&soap(AUT, &ret)).unwrap();
    assert_eq!(r.raiz, "retEnviNFe");
    // o texto e' o da SEFAZ, byte a byte
    assert_eq!(r.texto, ret);
}

#[test]
fn soap_fault_e_html_e_vazio_viram_erro_em_palavras() {
    let f = match fault("Falha no processamento") {
        Resp::R(_, b) => b,
        Resp::F(_) => unreachable!(),
    };
    let e = desembrulhar(&f).unwrap_err();
    assert_eq!(e, ErroResposta::Fault("Falha no processamento".into()));
    assert!(e.to_string().contains("falha SOAP"));
    let e = desembrulhar(b"<html><body>502 Bad Gateway</body></html>").unwrap_err();
    assert_eq!(e, ErroResposta::SemRetorno);
    let e = desembrulhar(b"<html><body>502 <br> Bad</body>").unwrap_err();
    assert!(matches!(e, ErroResposta::NaoEXml(_)));
    assert!(e.to_string().contains("nao e' um XML"));
    assert!(matches!(
        desembrulhar(b"").unwrap_err(),
        ErroResposta::NaoEXml(_)
    ));
    // Fault de SOAP 1.1
    let f11 = b"<s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\"><s:Body><s:Fault><faultcode>s:Server</faultcode>\
                <faultstring>Erro interno</faultstring></s:Fault></s:Body></s:Envelope>";
    assert_eq!(
        desembrulhar(f11).unwrap_err(),
        ErroResposta::Fault("Erro interno".into())
    );
}

#[test]
fn o_protnfe_cru_nao_e_reescrito_e_o_cdata_e_o_comentario_nao_enganam() {
    let (xml, chave) = nfe(62, 2);
    // protNFe com espacos, <X></X> e aspas simples: precisa voltar IGUAL
    let prot_estranho = format!(
        "<protNFe versao='4.00'><infProt  Id='ID1'><tpAmb>2</tpAmb><verAplic>SP</verAplic><chNFe>{chave}</chNFe>\
         <dhRecbto>2026-10-03T09:00:05-03:00</dhRecbto><nProt>{NPROT}</nProt><digVal>qZk+NkcGgWq6PiVxeFDCbJzQ2J0=</digVal>\
         <cStat>100</cStat><xMotivo></xMotivo></infProt></protNFe>"
    );
    // um comentario e um xMotivo escapado que citam <protNFe antes do verdadeiro
    let ret = ret_envi(
        "104",
        "Lote processado",
        &format!("<!-- <protNFe versao=\"4.00\"><infProt><chNFe>0</chNFe></infProt></protNFe> -->{prot_estranho}"),
    );
    let r = desembrulhar(&soap(AUT, &ret)).unwrap();
    let re = ler_envio(&r.texto).unwrap();
    assert_eq!(re.protocolos.len(), 1);
    assert_eq!(re.protocolos[0].cru, prot_estranho);
    assert_eq!(re.protocolos[0].chave, chave);
    let proc_ = nfe_proc(&xml, &re.protocolos[0].cru);
    assert!(proc_.contains(&xml));
    assert!(proc_.contains(&prot_estranho));
    assert!(roxmltree::Document::parse(&proc_).is_ok());
}

#[test]
fn protnfe_com_prefixo_herda_a_declaracao_do_envelope() {
    let (xml, chave) = nfe(63, 2);
    let corpo = format!(
        "<env:Envelope xmlns:env=\"http://www.w3.org/2003/05/soap-envelope\" xmlns:n=\"{NS}\"><env:Body>\
         <n:retEnviNFe versao=\"4.00\"><n:tpAmb>2</n:tpAmb><n:verAplic>SP</n:verAplic><n:cStat>104</n:cStat>\
         <n:xMotivo>Lote processado</n:xMotivo><n:cUF>35</n:cUF><n:dhRecbto>2026-10-03T09:00:05-03:00</n:dhRecbto>\
         <n:protNFe versao=\"4.00\"><n:infProt><n:tpAmb>2</n:tpAmb><n:verAplic>SP</n:verAplic><n:chNFe>{chave}</n:chNFe>\
         <n:dhRecbto>2026-10-03T09:00:05-03:00</n:dhRecbto><n:nProt>{NPROT}</n:nProt>\
         <n:digVal>qZk+NkcGgWq6PiVxeFDCbJzQ2J0=</n:digVal><n:cStat>100</n:cStat><n:xMotivo>Autorizado</n:xMotivo>\
         </n:infProt></n:protNFe></n:retEnviNFe></env:Body></env:Envelope>"
    );
    let r = desembrulhar(corpo.as_bytes()).unwrap();
    // o retorno ganha o xmlns:n que o envelope declarava; o resto e' o que veio
    assert!(r
        .texto
        .starts_with(&format!("<n:retEnviNFe xmlns:n=\"{NS}\" versao=\"4.00\">")));
    let re = ler_envio(&r.texto).unwrap();
    let p = &re.protocolos[0];
    assert!(p.cru.starts_with(&format!(
        "<n:protNFe xmlns:n=\"{NS}\" versao=\"4.00\"><n:infProt>"
    )));
    assert_eq!(p.numero, NPROT);
    // dentro do nfeProc o documento continua bem formado, no mesmo espaco de nomes
    let proc_ = nfe_proc(&xml, &p.cru);
    let doc = roxmltree::Document::parse(&proc_).unwrap();
    let pn = doc
        .descendants()
        .find(|n| n.tag_name().name() == "protNFe")
        .unwrap();
    assert_eq!(pn.tag_name().namespace(), Some(NS));
    let nfe_lida = rustnf::nfe::ler(proc_.as_bytes()).unwrap();
    assert_eq!(nfe_lida.protocolo.unwrap().numero, NPROT);
}

#[test]
fn ler_status_recibo_e_consulta() {
    let st = ler_status(&ret_status(
        "107",
        "Servico em Operacao",
        "<tMed>2</tMed><dhRetorno>2026-10-03T10:00:00</dhRetorno><xObs>teste</xObs>",
    ))
    .unwrap();
    assert_eq!(st.cab.cstat, "107");
    assert_eq!(st.cab.cuf, "35");
    assert_eq!(st.tempo_medio, Some(2));
    assert_eq!(st.retorno_previsto, "2026-10-03T10:00:00");
    assert_eq!(st.obs, "teste");

    let e = ler_envio(&ret_envi_recibo("351000000000042", 3)).unwrap();
    assert_eq!(e.cab.cstat, "103");
    let rec = e.recibo.unwrap();
    assert_eq!(
        (rec.numero.as_str(), rec.tempo_medio),
        ("351000000000042", Some(3))
    );
    assert!(e.protocolos.is_empty());

    let (_, chave) = nfe(64, 2);
    let rr = ler_recibo(&ret_reci(
        "351000000000042",
        "104",
        "Lote processado",
        &prot_ok(&chave),
    ))
    .unwrap();
    assert_eq!(rr.recibo, "351000000000042");
    assert_eq!(rr.protocolos[0].chave, chave);

    let eventos = "<procEventoNFe versao=\"1.00\"><evento versao=\"1.00\"><infEvento><tpEvento>110111</tpEvento></infEvento></evento></procEventoNFe>";
    let c = ler_consulta(&ret_sit(
        &chave,
        "100",
        "Autorizado o uso da NF-e",
        &format!("{}{eventos}", prot_ok(&chave)),
    ))
    .unwrap();
    assert_eq!(c.chave, chave);
    assert_eq!(c.protocolo.unwrap().numero, NPROT);
    assert_eq!(c.eventos, [eventos]);
}

#[test]
fn retorno_de_outro_servico_nao_e_lido_como_o_esperado() {
    let (_, chave) = nfe(65, 2);
    let e = ler_envio(&ret_sit(&chave, "217", "x", "")).unwrap_err();
    assert_eq!(
        e,
        ErroResposta::OutroRetorno {
            esperado: "retEnviNFe",
            veio: "retConsSitNFe".into()
        }
    );
    assert!(ler_status("<retConsStatServ").is_err());
}
