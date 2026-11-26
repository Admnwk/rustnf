// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A NFC-e na aplicacao: numeracao propria, CSC, emissao nos quatro desfechos, retomada, contingencia off-line, aviso de
//! prazo do cancelamento e o aviso "ao fechar o caixa". Transporte falso: nenhum teste toca a rede, o cofre real nem a
//! configuracao do usuario.

use std::path::Path;

use chrono::Duration;
use serde_json::{json, Value};

use super::danfce::{self, Pedido};
use super::{
    aviso_de_fechamento, aviso_de_prazo, emitir, erro_de_fechamento, listar_pendentes, opcoes_qr,
    Emissao, Modo, Prazo,
};
use crate::cofre::{Cofre, CofreEmMemoria};
use crate::config::Config;
use crate::nfe::testes_comuns::{
    agora, cliente, config_com, consulta, envio, evento, ok, pedido, ret_status, tempo, Falso,
    Resp, AUT, CON, EVT, NPROT, STA,
};
use crate::nfe::{self, Contexto};
use crate::notas::nfe::{NotasNfe, Situacao};
use crate::segredo::Segredo;
use crate::testes_comuns::PastaTemporaria;

/// O valor do CSC dos testes: ficticio, so' para procurar vazamento.
const CSC_TESTE: &str = "ZZTESTECSC0123456789ABCDEF000000";

/// Tudo de um teste da NFC-e: pasta, configuracao (vista pela NFC-e), certificado, pastas dos dois modelos, cliente falso.
struct C65 {
    pasta: PastaTemporaria,
    config: Config,
    certificado: rustnf::assinatura::cert_a1::Certificado,
    notas: NotasNfe,
    notas55: NotasNfe,
    cliente: rustnf::nfe::sefaz::ClienteSefaz<Falso>,
    falso: Falso,
    cofre: CofreEmMemoria,
}

impl C65 {
    fn novo(rotulo: &str, extra: &str, roteiro: Vec<(&'static str, Resp)>) -> Self {
        let pasta = PastaTemporaria::nova(rotulo);
        let config = config_com(&pasta, extra).como_nfce();
        let raiz = pasta.caminho().join("notas");
        let falso = Falso::novo(roteiro);
        Self {
            cliente: cliente(&config, falso.clone()),
            certificado: crate::nfe::testes_comuns::certificado(),
            notas: NotasNfe::abrir_nfce(&raiz).unwrap(),
            notas55: NotasNfe::abrir(&raiz).unwrap(),
            config,
            falso,
            cofre: CofreEmMemoria::novo(),
            pasta,
        }
    }

    fn ctx(&self) -> Contexto<'_, Falso> {
        Contexto::novo(
            &self.cliente,
            &self.certificado,
            &self.config,
            Some(&self.notas),
        )
        .com_agora(agora())
        .com_qr(opcoes_qr(&self.cofre, &self.config).unwrap())
    }

    fn ctx55(&self) -> Contexto<'_, Falso> {
        Contexto::novo(
            &self.cliente,
            &self.certificado,
            &self.config,
            Some(&self.notas55),
        )
        .com_agora(agora())
    }

    fn depois(&self, s: i64) -> Contexto<'_, Falso> {
        self.ctx().com_agora(agora() + Duration::seconds(s))
    }

    fn roteiro(&self, r: Vec<(&'static str, Resp)>) {
        self.falso.programar(r);
    }
}

/// O pedido da NFC-e: o da NF-e sem destinatario.
fn pedido65() -> Value {
    let mut p = pedido();
    p.as_object_mut().unwrap().remove("dest");
    p
}

fn todos_os_arquivos(dir: &Path, saida: &mut Vec<std::path::PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            todos_os_arquivos(&p, saida);
        } else {
            saida.push(p);
        }
    }
}

fn enviada(e: &Emissao) -> &nfe::Emitida {
    match e {
        Emissao::Enviada(x) => x,
        Emissao::Offline(_) => panic!("era para ter ido a SEFAZ"),
    }
}

fn pedido65_com_modelo(m: u32) -> Value {
    let mut p = pedido65();
    p["modelo"] = json!(m);
    p
}

#[tokio::test]
async fn a_numeracao_do_65_e_separada_da_do_55_na_mesma_pasta() {
    let c = C65::novo(
        "nfce-mista",
        "",
        vec![
            (AUT, envio("100", "Autorizado o uso da NF-e", NPROT)),
            (AUT, envio("100", "Autorizado o uso da NF-e", NPROT)),
            (AUT, envio("100", "Autorizado o uso da NF-e", NPROT)),
        ],
    );
    let n55 = nfe::emitir(&c.ctx55(), pedido(), true).await.unwrap();
    assert_eq!((n55.serie.as_str(), n55.numero), ("1", 1));
    let indice55 = std::fs::read(c.pasta.caminho().join("notas/nfe/indice.json")).unwrap();

    // a serie 1 do 65 comeca em 1 mesmo com a serie 1 do 55 ja em 1
    let n65 = emitir(&c.ctx(), pedido65(), true, Modo::Normal)
        .await
        .unwrap();
    let e65 = enviada(&n65);
    assert_eq!((e65.serie.as_str(), e65.numero), ("1", 1));
    assert_ne!(e65.chave, n55.chave);
    assert_eq!(&e65.chave[20..22], "65");
    let n65b = emitir(&c.ctx(), pedido65(), true, Modo::Normal)
        .await
        .unwrap();
    assert_eq!(enviada(&n65b).numero, 2);

    assert_eq!(c.notas.proximo_numero("1").unwrap(), 3);
    assert_eq!(c.notas55.proximo_numero("1").unwrap(), 2);
    // o 65 nao encosta no que o 55 guardou
    let raiz = c.pasta.caminho().join("notas");
    assert_eq!(
        std::fs::read(raiz.join("nfe/indice.json")).unwrap(),
        indice55
    );
    assert!(raiz.join("nfce/indice.json").is_file());
    assert_eq!(c.notas55.listar().unwrap().len(), 1);
    assert_eq!(c.notas.listar().unwrap().len(), 2);
    // cada pasta so' aceita a chave do seu modelo
    assert!(c.notas.registro(&n55.chave).is_err());
    assert!(c.notas55.registro(&e65.chave).is_err());
    // e o pedido do modelo errado e' recusado em palavras
    let erro = nfe::emitir(&c.ctx55(), pedido65_com_modelo(65), true)
        .await
        .unwrap_err();
    assert!(erro.to_string().contains("rustnf nfce"), "{erro}");
}

#[test]
fn pasta_antiga_so_com_o_55_le_e_numera_igual_e_o_indice_nao_ganha_campos() {
    let pasta = PastaTemporaria::nova("nfce-antiga");
    let raiz = pasta.caminho().join("notas");
    std::fs::create_dir_all(raiz.join("nfe")).unwrap();
    // exatamente o que a versao anterior gravava: sem os campos da NFC-e
    let chave = "35261011222333000181550010000000411123456705";
    let antigo = format!(
        "{{\n  \"versao\": 1,\n  \"notas\": [\n    {{\n      \"chave\": \"{chave}\",\n      \"serie\": \"1\",\n      \"numero\": \"41\",\n      \"emissao\": \"2026-10-03T09:00:00-03:00\",\n      \"valor\": \"31.80\",\n      \"ambiente\": \"homologacao\",\n      \"situacao\": \"autorizada\",\n      \"protocolo\": \"{NPROT}\",\n      \"cstat\": \"100\",\n      \"destinatario_documento\": \"\",\n      \"destinatario_nome\": \"\",\n      \"protocolo_epec\": \"\",\n      \"desde\": 0,\n      \"tentativas\": 0,\n      \"ultima\": null,\n      \"eventos\": []\n    }}\n  ],\n  \"inutilizacoes\": []\n}}\n"
    );
    std::fs::write(raiz.join("nfe/indice.json"), &antigo).unwrap();
    std::fs::write(
        raiz.join("nfe/estado.json"),
        "{\"versao\":1,\"series\":{\"1\":41},\"bases\":{}}",
    )
    .unwrap();
    let antes_estado = std::fs::read(raiz.join("nfe/estado.json")).unwrap();

    let n55 = NotasNfe::abrir(&raiz).unwrap();
    assert_eq!(n55.proximo_numero("1").unwrap(), 42);
    let lista = n55.listar().unwrap();
    assert_eq!((lista.len(), lista[0].numero.as_str()), (1, "41"));
    let json = serde_json::to_value(&lista).unwrap();
    for novo in ["offline", "prazo", "recusa"] {
        assert!(
            json[0].get(novo).is_none(),
            "a saida do 55 nao pode ganhar {novo}"
        );
    }
    // abrir a pasta da NFC-e nao muda nada do 55; ela comeca do 1
    let n65 = NotasNfe::abrir_nfce(&raiz).unwrap();
    assert_eq!(n65.proximo_numero("1").unwrap(), 1);
    assert_eq!(
        std::fs::read_to_string(raiz.join("nfe/indice.json")).unwrap(),
        antigo
    );
    assert_eq!(
        std::fs::read(raiz.join("nfe/estado.json")).unwrap(),
        antes_estado
    );
}

#[test]
fn o_65_sem_csc_na_versao_2_recusa_antes_de_numerar() {
    let c = C65::novo(
        "nfce-v2-sem-csc",
        "[nfce]\nqr_versao = 2\nid_csc = \"1\"\n",
        vec![],
    );
    let erro = opcoes_qr(&c.cofre, &c.config).unwrap_err();
    assert!(erro.to_string().contains("nfce csc guardar"), "{erro}");
    // nada foi numerado nem gravado
    assert!(!c.pasta.caminho().join("notas/nfce/indice.json").exists());
    assert_eq!(c.notas.proximo_numero("1").unwrap(), 1);
}

#[tokio::test]
async fn o_csc_nunca_aparece_no_json_no_debug_nem_em_arquivo_da_pasta() {
    let c = C65::novo(
        "nfce-csc-vaza",
        "[nfce]\nqr_versao = 2\nid_csc = \"1\"\n",
        vec![(AUT, envio("100", "Autorizado o uso da NF-e", NPROT))],
    );
    crate::nfce::csc::guardar(&c.cofre, &c.config, &Segredo::novo(CSC_TESTE.into())).unwrap();
    let ctx = c.ctx();
    assert!(!format!("{:?}", ctx.qr).contains(CSC_TESTE));
    let e = emitir(&ctx, pedido65(), true, Modo::Normal).await.unwrap();
    assert_eq!(enviada(&e).situacao, Some(Situacao::Autorizada));
    assert!(!e.para_json().to_string().contains(CSC_TESTE));
    assert!(!format!("{e:?}").contains(CSC_TESTE));
    // o QR Code v2 leva o idCSC e o hash, nunca o valor
    let xml = c.notas.ler_xml(e.chave()).unwrap();
    assert!(xml.contains("|2|2|1|"), "QR Code v2 com idCSC 1");
    // nenhum arquivo da pasta temporaria (notas, indice, estado) tem o CSC
    let mut arquivos = Vec::new();
    todos_os_arquivos(c.pasta.caminho(), &mut arquivos);
    assert!(arquivos.len() >= 3);
    for a in arquivos {
        let bytes = std::fs::read(&a).unwrap();
        assert!(
            !String::from_utf8_lossy(&bytes).contains(CSC_TESTE),
            "o CSC vazou em {a:?}"
        );
    }
}

#[tokio::test]
async fn autorizada_guarda_a_nota_gasta_o_numero_e_o_cupom_sai_em_pdf_e_escpos() {
    let c = C65::novo(
        "nfce-aut",
        "",
        vec![(AUT, envio("100", "Autorizado o uso da NF-e", NPROT))],
    );
    let e = emitir(&c.ctx(), pedido65(), true, Modo::Normal)
        .await
        .unwrap();
    let x = enviada(&e);
    assert_eq!(x.situacao, Some(Situacao::Autorizada));
    assert!(x.erro().is_none());
    assert_eq!(e.para_json()["offline"], false);
    assert!(c.notas.ler_xml(&x.chave).unwrap().contains("<qrCode>"));
    assert_eq!(c.notas.proximo_numero("1").unwrap(), 2);

    let (xml, cancelada) = danfce::xml_da_pasta(&c.notas, &x.chave).unwrap();
    assert!(!cancelada);
    let op = danfce::opcoes(&c.config, Pedido::default(), false).unwrap();
    assert!(danfce::pdf(&xml, &op).unwrap().starts_with(b"%PDF"));
    let estreita = Pedido {
        largura: Some(58),
        ..Pedido::default()
    };
    let op58 = danfce::opcoes(&c.config, estreita, false).unwrap();
    assert_ne!(
        danfce::pdf(&xml, &op).unwrap(),
        danfce::pdf(&xml, &op58).unwrap()
    );
    assert!(!danfce::escpos(&xml, &op).unwrap().is_empty());
    let caminho = danfce::guardar(&c.notas, &x.chave, b"%PDF-1.4").unwrap();
    assert!(caminho.is_file());
    // colunas que a impressora nao tem
    let quarenta = Pedido {
        colunas: Some(40),
        ..Pedido::default()
    };
    assert!(danfce::opcoes(&c.config, quarenta, false).is_err());
}

#[tokio::test]
async fn rejeitada_nao_gasta_o_numero_e_denegada_gasta() {
    let c = C65::novo(
        "nfce-rej",
        "",
        vec![(
            AUT,
            envio("225", "Rejeicao: Falha no Schema XML do lote de NFe", ""),
        )],
    );
    let e = emitir(&c.ctx(), pedido65(), true, Modo::Normal)
        .await
        .unwrap();
    assert_eq!(enviada(&e).situacao, None);
    assert!(e.erro().is_some());
    assert!(c.notas.listar().unwrap().is_empty());
    assert_eq!(c.notas.proximo_numero("1").unwrap(), 1);

    c.roteiro(vec![(AUT, envio("110", "Uso Denegado", NPROT))]);
    let d = emitir(&c.ctx(), pedido65(), true, Modo::Normal)
        .await
        .unwrap();
    assert_eq!(enviada(&d).situacao, Some(Situacao::Denegada));
    assert_eq!(c.notas.proximo_numero("1").unwrap(), 2);
}

#[tokio::test]
async fn sem_resposta_fica_pendente_e_a_retomada_fecha_a_mesma_chave() {
    let c = C65::novo("nfce-retoma", "", vec![(AUT, tempo()), (CON, tempo())]);
    let e = emitir(&c.ctx(), pedido65(), true, Modo::Normal)
        .await
        .unwrap();
    let chave = e.chave().to_string();
    assert_eq!(enviada(&e).situacao, Some(Situacao::Pendente));
    let lista = listar_pendentes(&c.notas, agora()).unwrap();
    assert_eq!((lista.len(), lista[0].prazo), (1, Prazo::Sem));
    assert!(lista[0].frase().contains("PODE ter sido autorizada"));

    c.roteiro(vec![(
        CON,
        consulta("100", "Autorizado o uso da NF-e", NPROT),
    )]);
    let feitos = nfe::retomar(&c.depois(5)).await.unwrap();
    assert_eq!(
        (feitos[0].chave.as_str(), feitos[0].status),
        (chave.as_str(), "autorizada")
    );
    assert_eq!(c.notas.listar().unwrap().len(), 1, "nao nasceu outra nota");
    assert!(listar_pendentes(&c.notas, agora()).unwrap().is_empty());
    assert!(c.notas.ler_xml(&chave).unwrap().contains("<nfeProc"));
}

#[tokio::test]
async fn offline_grava_pendente_com_o_prazo_e_a_retomada_transmite() {
    let c = C65::novo("nfce-offline", "", vec![]);
    let e = emitir(&c.ctx(), pedido65(), true, Modo::Offline)
        .await
        .unwrap();
    let Emissao::Offline(g) = &e else {
        panic!("era para ficar off-line")
    };
    // 03/10/2026 e' sabado: o primeiro dia util depois e' segunda, 05/10, ate 23:59
    assert_eq!(g.prazo, "05/10/2026 23:59");
    assert!(g.frase().contains("NAO foi enviada"));
    assert_eq!(c.falso.servicos().len(), 0, "off-line nao fala com a SEFAZ");
    let r = c.notas.registro(&g.chave).unwrap();
    assert_eq!(r.situacao, Situacao::Pendente);
    assert!(r.offline && r.prazo.is_some());
    let xml = c.notas.ler_xml(&g.chave).unwrap();
    assert!(xml.contains("<tpEmis>9</tpEmis>") && xml.contains("<dhCont>"));
    assert_eq!(&g.chave[34..35], "9", "o tpEmis 9 vai na chave");
    // o cupom da off-line sai, com a via do estabelecimento
    let (texto, _) = danfce::xml_da_pasta(&c.notas, &g.chave).unwrap();
    for via in [false, true] {
        let p = Pedido {
            via_estabelecimento: via,
            ..Pedido::default()
        };
        let op = danfce::opcoes(&c.config, p, false).unwrap();
        assert!(danfce::pdf(&texto, &op).unwrap().starts_with(b"%PDF"));
    }

    // a retomada consulta a chave (nao consta) e transmite a MESMA nota
    c.roteiro(vec![
        (
            CON,
            consulta(
                "217",
                "Rejeicao: NF-e nao consta na base de dados da SEFAZ",
                "",
            ),
        ),
        (AUT, envio("100", "Autorizado o uso da NF-e", NPROT)),
    ]);
    let feitos = nfe::retomar(&c.depois(5)).await.unwrap();
    assert_eq!(
        (feitos[0].status, feitos[0].situacao),
        ("autorizada", Some(Situacao::Autorizada))
    );
    assert_eq!(
        c.notas.registro(&g.chave).unwrap().situacao,
        Situacao::Autorizada
    );
    assert_eq!(c.notas.listar().unwrap().len(), 1);
}

#[tokio::test]
async fn offline_recusada_pela_sefaz_segue_pendente_com_a_recusa_a_vista() {
    let c = C65::novo("nfce-offline-rej", "", vec![]);
    let e = emitir(&c.ctx(), pedido65(), true, Modo::Offline)
        .await
        .unwrap();
    c.roteiro(vec![
        (
            CON,
            consulta(
                "217",
                "Rejeicao: NF-e nao consta na base de dados da SEFAZ",
                "",
            ),
        ),
        (
            AUT,
            envio("225", "Rejeicao: Falha no Schema XML do lote de NFe", ""),
        ),
    ]);
    let feitos = nfe::retomar(&c.depois(5)).await.unwrap();
    assert_eq!(feitos[0].situacao, Some(Situacao::Pendente));
    let r = c.notas.registro(e.chave()).unwrap();
    assert!(r.recusa.contains("225"), "{}", r.recusa);
    assert_eq!(
        c.notas.proximo_numero("1").unwrap(),
        2,
        "o numero ja foi entregue ao consumidor"
    );
    let p = &listar_pendentes(&c.notas, agora()).unwrap()[0];
    assert!(p.destaque() && p.frase().contains("RECUSOU") && p.frase().contains(e.chave()));
}

#[tokio::test]
async fn sefaz_fora_do_ar_antes_de_enviar_emite_off_line_e_no_ar_envia() {
    let c = C65::novo("nfce-auto", "", vec![(STA, tempo())]);
    let e = emitir(&c.ctx(), pedido65(), true, Modo::OfflineSeNecessario)
        .await
        .unwrap();
    assert!(matches!(e, Emissao::Offline(_)));
    assert_eq!(c.falso.servicos(), vec![STA]);

    c.roteiro(vec![
        (STA, ok(STA, &ret_status("107", "Servico em Operacao"))),
        (AUT, envio("100", "Autorizado o uso da NF-e", NPROT)),
    ]);
    let e = emitir(&c.ctx(), pedido65(), true, Modo::OfflineSeNecessario)
        .await
        .unwrap();
    assert_eq!(enviada(&e).situacao, Some(Situacao::Autorizada));
}

#[tokio::test]
async fn cancelar_avisa_do_prazo_nacional_antes_de_enviar_sem_bloquear() {
    let c = C65::novo(
        "nfce-cancela",
        "",
        vec![(AUT, envio("100", "Autorizado o uso da NF-e", NPROT))],
    );
    let e = emitir(&c.ctx(), pedido65(), true, Modo::Normal)
        .await
        .unwrap();
    let chave = e.chave().to_string();
    // autorizada as 09:00:05 de 03/10: dentro das 24 horas nao ha aviso
    assert!(aviso_de_prazo(&c.notas, &chave, agora() + Duration::hours(1)).is_none());
    // depois, o aviso diz o prazo nacional, a fonte e que a UF pode ter outro
    let aviso = aviso_de_prazo(&c.notas, &chave, agora() + Duration::hours(30)).unwrap();
    assert!(
        aviso.contains("04/10/2026 09:00") && aviso.contains("2P12-14"),
        "{aviso}"
    );
    assert!(aviso.contains("legislacao estadual"));
    // o aviso nao impede: o cancelamento sai e a SEFAZ decide
    c.roteiro(vec![(
        EVT,
        evento(
            "135",
            "Evento registrado e vinculado a NF-e",
            "135260000000099",
        ),
    )]);
    let r = nfe::cancelar(&c.ctx(), &chave, "Cupom emitido em duplicidade", None)
        .await
        .unwrap();
    assert!(r.erro().is_none());
    assert_eq!(
        c.notas.registro(&chave).unwrap().situacao,
        Situacao::Cancelada
    );
    // nota que nao esta na pasta: sem data de autorizacao nao ha o que dizer
    assert!(aviso_de_prazo(&c.notas, &"0".repeat(44), agora()).is_none());
}

#[tokio::test]
async fn fechar_o_caixa_avisa_com_a_acao_e_o_prazo_e_sai_com_codigo_diferente_de_zero() {
    let c = C65::novo("nfce-caixa", "", vec![]);
    // sem pendente: nada a avisar
    let vazia = listar_pendentes(&c.notas, agora()).unwrap();
    assert!(aviso_de_fechamento(&vazia).is_none() && erro_de_fechamento(&vazia).is_none());

    let e = emitir(&c.ctx(), pedido65(), true, Modo::Offline)
        .await
        .unwrap();
    let lista = listar_pendentes(&c.notas, agora()).unwrap();
    assert!(matches!(lista[0].prazo, Prazo::Folga(_)) && !lista[0].destaque());
    let aviso = aviso_de_fechamento(&lista).unwrap();
    assert!(
        aviso.contains("rustnf nfce pendentes --retomar")
            && aviso.contains("antes de 05/10/2026 23:59"),
        "{aviso}"
    );
    assert_eq!(erro_de_fechamento(&lista).unwrap().codigo_saida(), 4);
    assert_eq!(lista[0].para_json()["prazo"], "no_prazo");

    // menos de 1 hora para o prazo: destaque; depois do prazo: vencido, com destaque
    let fim = c.notas.registro(e.chave()).unwrap().prazo.unwrap();
    let quase = chrono::DateTime::from_timestamp(fim - 1800, 0).unwrap();
    let p = &listar_pendentes(&c.notas, quase).unwrap()[0];
    assert!(matches!(p.prazo, Prazo::AVencer(_)) && p.destaque());
    assert!(p.frase().contains("AGORA"));
    let tarde = chrono::DateTime::from_timestamp(fim + 7200, 0).unwrap();
    let lista = listar_pendentes(&c.notas, tarde).unwrap();
    assert!(matches!(lista[0].prazo, Prazo::Vencido(_)) && lista[0].destaque());
    assert!(
        lista[0].frase().contains("VENCEU")
            && lista[0].para_json()["segundos"].as_i64().unwrap() < 0
    );
    assert!(aviso_de_fechamento(&lista).unwrap().contains("JA VENCEU"));
}

#[test]
fn csc_guardar_testar_apagar_pelo_cofre_em_memoria() {
    let c = C65::novo(
        "nfce-csc",
        "[nfce]\nqr_versao = 2\nid_csc = \"1\"\n",
        vec![],
    );
    assert!(!crate::nfce::csc::existe(&c.cofre, &c.config).unwrap());
    crate::nfce::csc::guardar(&c.cofre, &c.config, &Segredo::novo(CSC_TESTE.into())).unwrap();
    assert!(crate::nfce::csc::existe(&c.cofre, &c.config).unwrap());
    let chave = crate::nfce::csc::chave_do_csc(&c.config).unwrap();
    assert!(!chave.contains(CSC_TESTE) && chave.contains("homologacao") && chave.ends_with(":1"));
    assert_eq!(c.cofre.ler(&chave).unwrap().unwrap().expor(), CSC_TESTE);
    crate::nfce::csc::apagar(&c.cofre, &c.config).unwrap();
    assert!(!crate::nfce::csc::existe(&c.cofre, &c.config).unwrap());
}
