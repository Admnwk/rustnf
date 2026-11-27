// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O DIFAL e o CFOP da venda interestadual a nao contribuinte, provados contra a biblioteca: o pedido do assistente passa
//! pela montagem ESTRITA da aplicacao (`rustnf_app::nfe::montar_pedido`), a mesma conferencia da janela. A condicao que
//! a tela usa (`nfe_difal::exige_o_grupo`) espelha `exige_grupo` da biblioteca (`difal.rs:174`); estes testes mostram
//! que o espelho bate: onde a tela pede o grupo a biblioteca recusa sem ele (rejeicao 694), e onde a tela nao pede a
//! biblioteca nao reclama dele.

use chrono::{Datelike, Utc};
use rustnf::nfe::montagem::{montar, nota_de_json};
use rustnf_app::config::Config;
use rustnf_app::nfe::{completar_pedido, montar_pedido, Montado};
use rustnf_app::notas::nfe::NotasNfe;
use serde_json::{json, Value};

use super::testes::{config_de, contexto, dados_basicos, CNPJ};
use super::*;
use crate::modelo::nfe_difal::{exige_o_grupo, nota_exige_o_grupo};
use crate::testes_comuns::PastaTemporaria;

/// Salvador (BA): de Sao Paulo a aliquota interestadual e' 7 % (SP esta entre os estados de Sul e Sudeste, BA nao).
const BA: &str = "2927408";

/// Um consumidor final pessoa fisica da Bahia, que nao tem inscricao, com um cafe de R$ 31,80.
fn cliente_da_bahia() -> DadosNfe {
    let mut d = dados_basicos();
    d.cod_municipio = BA.into();
    d.cep = "40020000".into();
    d.logradouro = "Rua Chile".into();
    d.bairro = "Centro".into();
    d.itens[0].aliquota_icms = "7".into();
    d
}

fn com_difal(mut d: DadosNfe) -> DadosNfe {
    for i in &mut d.itens {
        i.difal_aliquota = "18".into();
        i.difal_fcp = "2".into();
    }
    d
}

struct Banca {
    _pasta: PastaTemporaria,
    config: Config,
    notas: NotasNfe,
}

impl Banca {
    fn nova(crt: u8) -> Self {
        let pasta = PastaTemporaria::nova("nfe-difal");
        let regime = if matches!(crt, 1 | 4) {
            ""
        } else {
            "nao_cumulativo"
        };
        let config = config_de(&pasta, crt, regime);
        let notas = NotasNfe::abrir(config.notas.pasta.trim()).unwrap();
        Self {
            _pasta: pasta,
            config,
            notas,
        }
    }

    /// A montagem estrita da aplicacao, sobre o pedido como esta.
    fn montar(&self, pedido: Value) -> Result<Montado, String> {
        montar_pedido(&self.config, Some(&self.notas), pedido, true).map_err(|e| e.mensagem())
    }

    /// O codigo da rejeicao (`cStat`) com que a biblioteca recusa o pedido, ou vazio se ela o aceita. A aplicacao nao o
    /// repassa no erro, entao o teste pede a mesma montagem a biblioteca e le o `regra`.
    fn regra(&self, mut pedido: Value) -> String {
        let proximo = self.notas.proximo_numero("1").ok();
        completar_pedido(&mut pedido, &self.config, proximo, Utc::now(), false).unwrap();
        let nota = nota_de_json(&pedido).unwrap();
        montar(&nota).err().map(|e| e.regra).unwrap_or_default()
    }
}

fn ctx(crt: u8) -> ContextoNfe {
    contexto(
        crt,
        (!matches!(crt, 1 | 4)).then_some(RegimePisCofins::NaoCumulativo),
    )
}

fn sem_o_grupo(mut p: Value) -> Value {
    if let Some(itens) = p["itens"].as_array_mut() {
        for i in itens {
            if let Some(o) = i.as_object_mut() {
                o.remove("icms_uf_dest");
            }
        }
    }
    p
}

#[test]
fn regime_normal_interestadual_a_nao_contribuinte_com_o_grupo_passa() {
    let d = com_difal(cliente_da_bahia());
    let c = ctx(3);
    assert!(nota_exige_o_grupo(&d, &c));
    let p = pedido(&d, &c).unwrap();
    assert_eq!(p["itens"][0]["cfop"], "6108");
    assert_eq!(p["dest"]["ind_ie_dest"], 9);
    assert_eq!(
        p["itens"][0]["icms_uf_dest"],
        json!({
            "vbc": 3180, "aliquota_interna": "18", "aliquota_inter": "7.00",
            "ano": c.hoje.year(), "fcp": "2"
        })
    );
    let m = Banca::nova(3).montar(p).unwrap_or_else(|e| panic!("{e}"));
    assert!(m.xml.contains("<CFOP>6108</CFOP>"), "{}", m.xml);
    assert!(m.xml.contains("<ICMSUFDest>"), "{}", m.xml);
    // quem calculou foi a biblioteca (18 % - 7 % de R$ 31,80; 2 % de FCP): a tela so' mostra
    assert_eq!(m.totais["vICMSUFDest"], "3.50", "{:?}", m.totais);
    assert_eq!(m.totais["vFCPUFDest"], "0.64", "{:?}", m.totais);
}

#[test]
fn sem_o_grupo_a_biblioteca_recusa_com_a_694() {
    let d = cliente_da_bahia();
    let c = ctx(3);
    // a tela exige: sem a aliquota o pedido nem se monta
    assert!(pedido(&d, &c).is_err());
    let p = sem_o_grupo(pedido(&com_difal(d), &c).unwrap());
    let banca = Banca::nova(3);
    let erro = banca.montar(p.clone()).unwrap_err();
    assert!(erro.contains("sem o grupo ICMSUFDest"), "{erro}");
    assert_eq!(banca.regra(p), "694");
}

#[test]
fn simples_interestadual_a_nao_contribuinte_passa_sem_o_grupo_e_com_cfop_6108() {
    let d = cliente_da_bahia();
    let c = ctx(1);
    assert!(!nota_exige_o_grupo(&d, &c));
    let p = pedido(&d, &c).unwrap();
    assert_eq!(p["itens"][0]["cfop"], "6108");
    assert!(p["itens"][0].get("icms_uf_dest").is_none());
    let m = Banca::nova(1).montar(p).unwrap_or_else(|e| panic!("{e}"));
    assert!(m.xml.contains("<CFOP>6108</CFOP>"), "{}", m.xml);
    assert!(!m.xml.contains("<ICMSUFDest>"));
    assert!(!m.totais.contains_key("vICMSUFDest"));
}

#[test]
fn cst_isento_passa_sem_o_grupo() {
    for icms in [Icms::Isento, Icms::NaoTributado] {
        let mut d = cliente_da_bahia();
        d.itens[0].icms = Some(icms);
        let c = ctx(3);
        assert!(!nota_exige_o_grupo(&d, &c), "{icms:?}");
        let p = pedido(&d, &c).unwrap();
        assert!(p["itens"][0].get("icms_uf_dest").is_none());
        let m = Banca::nova(3)
            .montar(p)
            .unwrap_or_else(|e| panic!("{icms:?}: {e}"));
        assert!(m.xml.contains("<CFOP>6108</CFOP>"), "{}", m.xml);
    }
}

#[test]
fn contribuinte_com_ie_fora_da_uf_continua_6102_sem_o_grupo() {
    let mut d = cliente_da_bahia();
    d.documento = CNPJ.into();
    d.nome = "COMERCIO DE TESTE LTDA".into();
    d.consumo = Some(Consumo::Revenda);
    d.contribuinte = Some(Contribuinte::ComIe);
    d.ie = "12345678".into();
    d.cod_municipio = "3304557".into();
    d.cep = "20040002".into();
    d.logradouro = "Avenida Brasil".into();
    d.bairro = "Jardim America".into();
    let c = ctx(3);
    assert!(!nota_exige_o_grupo(&d, &c));
    let p = pedido(&d, &c).unwrap();
    assert_eq!(p["itens"][0]["cfop"], "6102");
    assert!(p["itens"][0].get("icms_uf_dest").is_none());
    let m = Banca::nova(3).montar(p).unwrap_or_else(|e| panic!("{e}"));
    assert!(m.xml.contains("<CFOP>6102</CFOP>"), "{}", m.xml);
}

#[test]
fn dentro_do_estado_nao_ha_difal() {
    let mut d = dados_basicos();
    d.itens[0].aliquota_icms = "18".into();
    assert!(!nota_exige_o_grupo(&d, &ctx(3)));
}

#[test]
fn a_aliquota_do_difal_vira_pendencia_com_o_que_fazer() {
    let c = ctx(3);
    let mut d = cliente_da_bahia();
    let falta = pendencias(&d, &c);
    let p = falta
        .iter()
        .find(|p| p.mensagem.contains("alíquota do ICMS de BA"))
        .unwrap();
    assert_eq!(p.item, Some(0));
    assert!(
        p.como_resolver.contains("Secretaria da Fazenda de BA"),
        "{}",
        p.como_resolver
    );
    d.itens[0].difal_aliquota = "0".into();
    assert!(pendencias(&d, &c)
        .iter()
        .any(|p| p.mensagem.contains("não serve")));
    d.itens[0].difal_aliquota = "18".into();
    d.itens[0].difal_fcp = "3".into();
    assert!(pendencias(&d, &c)
        .iter()
        .any(|p| p.mensagem.contains("combate à pobreza")));
    d.itens[0].difal_fcp = String::new();
    assert!(pendencias(&d, &c).is_empty());
}

#[test]
fn o_cfop_a_nao_contribuinte_de_outro_estado_sai_6107_ou_6108() {
    let c = ctx(3);
    let mut d = com_difal(cliente_da_bahia());
    assert_eq!(pedido(&d, &c).unwrap()["itens"][0]["cfop"], "6108");
    d.itens[0].origem = Origem::Fabricou;
    assert_eq!(pedido(&d, &c).unwrap()["itens"][0]["cfop"], "6107");
}

#[test]
fn cliente_isento_e_st_a_nao_contribuinte_pedem_a_escolha_do_cfop() {
    let c = ctx(3);
    // isento de inscricao: 6102 ou 6108
    let mut d = cliente_da_bahia();
    d.contribuinte = Some(Contribuinte::Isento);
    let falta = pendencias(&d, &c);
    let p = falta
        .iter()
        .find(|p| p.mensagem.contains("mais de um CFOP"))
        .unwrap();
    assert!(
        p.como_resolver.contains("6102") && p.como_resolver.contains("6108"),
        "{p:?}"
    );
    assert!(pedido(&d, &c).is_err());
    d.itens[0].cfop_escolhido = Some("6108".into());
    assert_eq!(pedido(&d, &c).unwrap()["itens"][0]["cfop"], "6108");
    // a escolha de outro cliente nao vale: voltando a contribuinte com IE, a tabela decide de novo
    d.contribuinte = Some(Contribuinte::ComIe);
    d.ie = "12345678".into();
    assert_eq!(pedido(&d, &c).unwrap()["itens"][0]["cfop"], "6102");
    d.itens[0].cfop_escolhido = Some("6108".into());
    assert_eq!(pedido(&d, &c).unwrap()["itens"][0]["cfop"], "6102");
    // ICMS ja cobrado antes, a nao contribuinte: 6404 ou 6108
    let mut s = cliente_da_bahia();
    s.itens[0].icms = Some(Icms::JaCobradoAntes);
    s.itens[0].st_base = "31,80".into();
    s.itens[0].st_aliquota = "18".into();
    s.itens[0].st_valor = "5,72".into();
    s.itens[0].difal_aliquota = "18".into();
    assert!(pendencias(&s, &c)
        .iter()
        .any(|p| p.mensagem.contains("mais de um CFOP")));
    s.itens[0].cfop_escolhido = Some("6404".into());
    assert_eq!(pedido(&s, &c).unwrap()["itens"][0]["cfop"], "6404");
    // o que a pessoa digitou nos detalhes fiscais vale sobre tudo
    s.itens[0].cfop = Some("6949".into());
    assert_eq!(pedido(&s, &c).unwrap()["itens"][0]["cfop"], "6949");
}

/// Uma combinacao de regime, ICMS, cliente, finalidade e destino.
struct Combinacao {
    crt: u8,
    icms: Icms,
    cliente: Contribuinte,
    consumo: Consumo,
    municipio: &'static str,
}

impl Combinacao {
    fn dados(&self) -> DadosNfe {
        let mut d = cliente_da_bahia();
        d.cod_municipio = self.municipio.into();
        d.consumo = Some(self.consumo);
        d.contribuinte = Some(self.cliente);
        if self.cliente == Contribuinte::ComIe {
            d.ie = "12345678".into();
        }
        let it = &mut d.itens[0];
        it.icms = Some(self.icms);
        it.difal_aliquota = "18".into();
        if self.icms == Icms::JaCobradoAntes {
            it.st_base = "31,80".into();
            it.st_aliquota = "18".into();
            it.st_valor = "5,72".into();
        }
        d
    }
}

/// A biblioteca recusou por causa do grupo do DIFAL (694 falta, 695 sobra, 697 a 699 valores)?
fn regra_do_difal(regra: &str) -> bool {
    ["694", "695", "697", "698", "699"].contains(&regra)
}

/// O espelho bate com a biblioteca em todas as combinacoes de regime, ICMS, cliente, finalidade e destino: onde a tela
/// pede o grupo, a montagem estrita sem ele recusa com a 694 e com ele nao reclama do grupo; onde a tela nao pede, a
/// biblioteca nao fala de DIFAL.
#[test]
fn o_espelho_da_condicao_bate_com_a_biblioteca_em_todas_as_combinacoes() {
    let (mut pedem, mut dispensam) = (0, 0);
    for crt in [1u8, 2, 3, 4] {
        let banca = Banca::nova(crt);
        let c = ctx(crt);
        for icms in opcoes_de_icms(crt) {
            for cliente in Contribuinte::TODOS {
                for consumo in Consumo::TODOS {
                    for municipio in [BA, "3550308"] {
                        let comb = Combinacao {
                            crt,
                            icms: *icms,
                            cliente,
                            consumo,
                            municipio,
                        };
                        let mut d = comb.dados();
                        let destino = d.destino(&c).unwrap();
                        for cfop in d.itens[0].cfops_da_tabela(destino, crt, d.contribuinte) {
                            d.itens[0].cfop_escolhido = Some((*cfop).to_string());
                            let rotulo = format!(
                                "crt {} {:?} {cliente:?} {consumo:?} {municipio} cfop {cfop}",
                                comb.crt, comb.icms
                            );
                            let exige = exige_o_grupo(&d, &d.itens[0], &c);
                            let p = pedido(&d, &c).unwrap_or_else(|e| panic!("{rotulo}: {e:?}"));
                            assert_eq!(
                                p["itens"][0].get("icms_uf_dest").is_some(),
                                exige,
                                "{rotulo}"
                            );
                            let sem = sem_o_grupo(p.clone());
                            if exige {
                                pedem += 1;
                                let e = banca.montar(sem.clone()).err();
                                let e = e.unwrap_or_else(|| panic!("{rotulo}: passou sem o grupo"));
                                assert!(e.contains("sem o grupo ICMSUFDest"), "{rotulo}: {e}");
                                assert_eq!(banca.regra(sem), "694", "{rotulo}");
                                let regra = banca.regra(p);
                                assert!(!regra_do_difal(&regra), "{rotulo}: com o grupo, {regra}");
                            } else {
                                dispensam += 1;
                                let regra = banca.regra(sem);
                                assert!(!regra_do_difal(&regra), "{rotulo}: {regra}");
                            }
                        }
                    }
                }
            }
        }
    }
    // os dois lados foram vistos
    assert!(pedem >= 4 && dispensam >= 20, "{pedem} {dispensam}");
}
