// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O plano das folhas e o desenho de cada uma (MOC 7.00, Anexo II, itens 3.5 e 3.10.2).
//!
//! A primeira folha leva todos os blocos; as seguintes repetem o cabecalho de identificacao (emitente, "DANFE",
//! numero, serie, folha, codigos de barras, natureza, chave e identificacao do emitente) e usam o resto da area so'
//! para os itens que nao couberam, com as mesmas colunas e larguras, e para a continuacao das informacoes
//! complementares. Toda folha, inclusive a unica, traz "FOLHA nn/total".

use serde_json::{json, Value};

use super::blocos::{self, s};
use super::blocos_rtc;
use super::desenho::{quebrar, Caneta};
use super::medidas::*;
use super::tabela::{self, Coluna, ItemPronto};
use super::ErroDanfe;

/// Um bloco da primeira folha, na ordem em que aparece.
#[derive(Clone, Copy)]
enum Bloco {
    Destinatario,
    Retirada,
    Entrega,
    Fatura,
    Imposto,
    Transporte,
    TotaisRtc,
    IcmsIpiRtc,
    IbsCbsRtc,
    IssqnRtc,
}

fn desenhar(c: &mut Caneta, y: f64, b: Bloco, d: &Value) -> f64 {
    match b {
        Bloco::Destinatario => blocos::destinatario(c, y, d),
        Bloco::Retirada => blocos::local(c, y, "INFORMAÇÕES DO LOCAL DE RETIRADA", &d["retirada"]),
        Bloco::Entrega => blocos::local(c, y, "INFORMAÇÕES DO LOCAL DE ENTREGA", &d["entrega"]),
        Bloco::Fatura => blocos::fatura(c, y, &d["fatura"]),
        Bloco::Imposto => blocos::imposto(c, y, d),
        Bloco::Transporte => blocos::transporte(c, y, d),
        Bloco::TotaisRtc => blocos_rtc::totais(c, y, d),
        Bloco::IcmsIpiRtc => blocos_rtc::icms_ipi(c, y, d),
        Bloco::IbsCbsRtc => blocos_rtc::ibscbs(c, y, d),
        Bloco::IssqnRtc => blocos::issqn(c, y, &d["issqn"]),
    }
}

/// Os blocos da primeira folha. No leiaute MOC a fatura e o ISSQN saem quando a nota nao os tem (itens 3.3.2 e 3.3.3);
/// no RTC os facultativos so' aparecem com dado (NT 2026.010, item 4.4).
fn blocos_da_primeira(d: &Value, rtc: bool) -> Vec<Bloco> {
    let mut b = vec![Bloco::Destinatario];
    if !d["retirada"].is_null() {
        b.push(Bloco::Retirada);
    }
    if !d["entrega"].is_null() {
        b.push(Bloco::Entrega);
    }
    let r = &d["rtc"];
    if !d["fatura"].is_null() && (!rtc || r["fatura"].as_bool().unwrap_or(false)) {
        b.push(Bloco::Fatura);
    }
    if rtc {
        b.extend([Bloco::TotaisRtc, Bloco::IcmsIpiRtc, Bloco::IbsCbsRtc]);
        if r["issqn"].as_bool().unwrap_or(false) {
            b.push(Bloco::IssqnRtc);
        }
        if r["transporte"].as_bool().unwrap_or(false) {
            b.push(Bloco::Transporte);
        }
    } else {
        b.extend([Bloco::Imposto, Bloco::Transporte]);
    }
    b
}

/// O que uma folha leva alem do cabecalho.
struct Folha {
    /// Itens (indices) desta folha.
    itens: std::ops::Range<usize>,
    /// Linhas da continuacao das informacoes complementares.
    continuacao: Vec<String>,
}

/// Resultado do plano: as folhas e as medidas da primeira.
struct Plano {
    folhas: Vec<Folha>,
    y_titulo_itens: f64,
    fim_itens: f64,
    h_cabecalho: f64,
    primeiras_linhas: Vec<String>,
    rtc: bool,
    tem_qr: bool,
    issqn_h: f64,
}

fn altura_do_cabecalho(d: &Value) -> f64 {
    blocos::cabecalho(&mut Caneta::default(), 0.0, d, 1, 1)
}

fn planejar(d: &Value, itens: &[ItemPronto]) -> Result<Plano, ErroDanfe> {
    let rtc = !d["rtc"].is_null();
    let tem_qr = rtc && !d["qr"].is_null();
    let h_cabecalho = altura_do_cabecalho(d);
    let blocos_1: f64 = blocos_da_primeira(d, rtc)
        .into_iter()
        .map(|b| desenhar(&mut Caneta::default(), 0.0, b, d))
        .sum();
    let canhoto = if d["canhoto"].as_bool().unwrap_or(false) {
        21.2
    } else {
        0.0
    };
    let y_titulo_itens = TOPO + canhoto + h_cabecalho + blocos_1;
    let issqn_h = if !rtc && !d["issqn"].is_null() {
        blocos::issqn(&mut Caneta::default(), 0.0, &d["issqn"])
    } else {
        0.0
    };
    let fim_itens = FUNDO - (HT + H_COMPL) - issqn_h;
    let w_compl = if rtc {
        blocos_rtc::w_complementares(tem_qr)
    } else {
        129.5
    };
    let compl = quebrar(s(&d["complementares"]), w_compl - 1.2, T_ITEM, false);
    let cabe = (((H_COMPL - 3.0) / LH).floor() as usize).min(compl.len());
    let primeiras_linhas = compl[..cabe].to_vec();
    let mut resto: Vec<String> = compl[cabe..].to_vec();

    // itens: a primeira folha tem o que sobra entre o cabecalho de colunas e o fim; as outras, a folha inteira
    let topo_n = TOPO + h_cabecalho + HT;
    let capacidade_n = FUNDO - (topo_n + H_COLUNAS);
    let mut livre = fim_itens - (y_titulo_itens + HT + H_COLUNAS);
    let mut folhas = vec![Folha {
        itens: 0..0,
        continuacao: vec![],
    }];
    for (i, it) in itens.iter().enumerate() {
        if it.h > capacidade_n && it.h > livre {
            return Err(ErroDanfe::ItemMaiorQueAFolha(i + 1));
        }
        let atual = folhas.len() - 1;
        if it.h > livre && !folhas[atual].itens.is_empty() {
            folhas.push(Folha {
                itens: i..i,
                continuacao: vec![],
            });
            livre = capacidade_n;
        } else if it.h > livre {
            // a primeira folha nao comporta nem um item: o item vai para a folha seguinte
            folhas.push(Folha {
                itens: i..i,
                continuacao: vec![],
            });
            livre = capacidade_n;
        }
        let k = folhas.len() - 1;
        folhas[k].itens.end = i + 1;
        livre -= it.h;
    }
    // continuacao das informacoes complementares (item 3.5): depois do ultimo item; a primeira folha esta cheia
    let mut y_fim = if folhas.len() == 1 {
        FUNDO
    } else {
        let ultima = folhas.last().map_or(0..0, |f| f.itens.clone());
        topo_n + H_COLUNAS + itens[ultima].iter().map(|i| i.h).sum::<f64>()
    };
    while !resto.is_empty() {
        let n = ((FUNDO - y_fim - 1.0 - HT - 2.0) / LH).floor();
        if n < 1.0 {
            folhas.push(Folha {
                itens: itens.len()..itens.len(),
                continuacao: vec![],
            });
            y_fim = TOPO + h_cabecalho;
            continue;
        }
        let m = (n as usize).min(resto.len());
        let k = folhas.len() - 1;
        folhas[k].continuacao = resto.drain(..m).collect();
        y_fim = FUNDO;
    }
    Ok(Plano {
        folhas,
        y_titulo_itens,
        fim_itens,
        h_cabecalho,
        primeiras_linhas,
        rtc,
        tem_qr,
        issqn_h,
    })
}

fn desenhar_folha(
    p: &Plano,
    d: &Value,
    cols: &[Coluna],
    itens: &[ItemPronto],
    i: usize,
) -> Vec<Value> {
    let total = p.folhas.len();
    let folha = &p.folhas[i];
    let mut c = Caneta::default();
    blocos::marca(&mut c, d["marca"].as_array().map_or(&[][..], Vec::as_slice));
    let mut y = TOPO;
    if i == 0 && d["canhoto"].as_bool().unwrap_or(false) {
        y += blocos::canhoto(&mut c, y, d);
    }
    y += blocos::cabecalho(&mut c, y, d, i + 1, total);
    if i == 0 {
        for b in blocos_da_primeira(d, p.rtc) {
            y += desenhar(&mut c, y, b, d);
        }
    }
    let ultima_dos_itens = &itens[folha.itens.clone()];
    if i == 0 || !folha.itens.is_empty() {
        c.titulo(X0, y, LARG, "DADOS DOS PRODUTOS / SERVIÇOS");
        y += HT;
        let fim = (i == 0).then_some(p.fim_itens);
        tabela::tabela(&mut c, y, cols, ultima_dos_itens, fim);
        y = fim.unwrap_or(y + H_COLUNAS + ultima_dos_itens.iter().map(|x| x.h).sum::<f64>());
    }
    if !folha.continuacao.is_empty() {
        let yc = if i == 0 { y } else { y + 1.0 };
        blocos::continuacao(&mut c, yc, &folha.continuacao);
    }
    if i == 0 {
        let y_dados = FUNDO - (HT + H_COMPL);
        if p.rtc {
            let w = blocos_rtc::w_complementares(p.tem_qr);
            blocos_rtc::dados_adicionais(&mut c, y_dados, &p.primeiras_linhas, w, p.tem_qr);
        } else {
            if !d["issqn"].is_null() {
                blocos::issqn(&mut c, y_dados - p.issqn_h, &d["issqn"]);
            }
            blocos::dados_adicionais(&mut c, y_dados, &p.primeiras_linhas, 129.5, LARG - 129.5);
        }
    }
    c.ops
}

/// As folhas desenhadas (operacoes por folha) e o plano em JSON.
pub(super) fn montar(d: &Value) -> Result<(Vec<Vec<Value>>, Value), ErroDanfe> {
    let rtc = !d["rtc"].is_null();
    let cols = if rtc {
        tabela::colunas_rtc()
    } else {
        tabela::colunas_moc(d)
    };
    let prontos: Vec<ItemPronto> = if rtc {
        d["rtc"]["itens"]
            .as_array()
            .map_or(&[][..], Vec::as_slice)
            .iter()
            .map(|it| tabela::preparar_rtc(&cols, it))
            .collect()
    } else {
        d["itens"]
            .as_array()
            .map_or(&[][..], Vec::as_slice)
            .iter()
            .map(|it| tabela::preparar_moc(&cols, it))
            .collect()
    };
    let plano = planejar(d, &prontos)?;
    let folhas: Vec<Vec<Value>> = (0..plano.folhas.len())
        .map(|i| desenhar_folha(&plano, d, &cols, &prontos, i))
        .collect();
    let json_plano = json!({
        "folhas": plano.folhas.len(),
        "topo_itens": plano.y_titulo_itens,
        "fim_itens": plano.fim_itens,
        "altura_cabecalho": plano.h_cabecalho,
        "itens_por_folha": plano.folhas.iter().map(|f| f.itens.len()).collect::<Vec<_>>(),
        "continuacao_por_folha": plano.folhas.iter().map(|f| f.continuacao.len()).collect::<Vec<_>>(),
    });
    Ok((folhas, json_plano))
}
