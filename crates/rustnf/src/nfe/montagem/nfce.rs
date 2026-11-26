// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Regras da NFC-e (modelo 65) e da NF-e com DANFE Simplificado Tipo 2 (`tpImp` 6), as duas do varejo presencial ou
//! de entrega: o que o MOC 7.00 (Anexo I) e a NT 2026.002 v1.11 mandam rejeitar nelas, com o codigo da rejeicao.
//!
//! A NT 2026.002 reescreveu as regras da NFC-e para valerem tambem para a NF-e com `tpImp` 6 (campo "Modelo" `55/65`),
//! por isso uma lista so' serve aos dois. As que dependem do relogio da SEFAZ (atraso da data de emissao, 704) ou de
//! parametro da UF (limite de valor, a UF que nao aceita a NFC-e ou a entrega a domicilio, 702 e 785) ficam para a
//! SEFAZ: a montagem aplica o padrao da NT onde ele existe (R$ 10.000,00 para o destinatario nao identificado e R$
//! 200.000,00 para o total) e nao inventa o resto.
//!
//! Ficam de fora, por nao existir o campo na montagem, as regras dos grupos que ela nao monta: arma (738), RECOPI
//! (348), cana (763), PIS-ST e COFINS-ST (746 e 749), II (743) e devolucao de tributos (390). A regra G01-10 (670,
//! local de entrega) esta marcada "implementacao futura" na NT e tambem fica de fora. As regras facultativas por UF
//! (751, 391 e 392) nao sao aplicadas: recusar o que a UF aceita seria inventar restricao.

use super::formato::Dec;
use super::modelo::{Icms, Item, Nota};
use super::{ind_final, ind_pres, tp_imp};

/// Uma rejeicao: o `cStat`, o campo do leiaute e o texto do MOC.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Rej {
    pub(crate) cstat: &'static str,
    pub(crate) campo: &'static str,
    pub(crate) msg: String,
}

fn r(cstat: &'static str, campo: &'static str, msg: impl Into<String>) -> Rej {
    Rej {
        cstat,
        campo,
        msg: msg.into(),
    }
}

/// A nota e' do varejo: NFC-e ou NF-e com `tpImp` 6.
pub(crate) fn varejo(n: &Nota) -> bool {
    n.modelo == 65 || (n.modelo == 55 && tp_imp(n) == 6)
}

/// Limite de valor do destinatario nao identificado (NT 2026.002, W16-40) e do total (W16-30), em centavos.
const LIMITE_SEM_DEST: i128 = 1_000_000;
const LIMITE_TOTAL: i128 = 20_000_000;

const CFOP_NOTA: [&str; 10] = [
    "5101", "5102", "5103", "5104", "5115", "5405", "5656", "5667", "5910", "5933",
];
const CFOP_TRIBUTADO: [&str; 6] = ["5101", "5102", "5103", "5104", "5115", "5910"];
const CFOP_ST: [&str; 4] = ["5405", "5656", "5667", "5910"];
const CST_OK: [&str; 7] = ["00", "20", "40", "41", "60", "61", "90"];
const CSOSN_OK: [&str; 6] = ["102", "103", "300", "400", "500", "900"];

fn cfop_de(it: &Item) -> &str {
    it.cfop.trim()
}

/// O CFOP 5.949 e o 5.405 que algumas UF admitem com certo CST (NT 2026.002, observacoes do I08-150 e do N12-40).
fn excecao_de_uf(uf: &str, cfop: &str, cod: &str) -> bool {
    matches!(
        (uf, cfop, cod),
        ("RS", "5949", "90" | "900") | ("SP", "5949", "40" | "900") | ("CE", "5405", "90" | "900")
    )
}

fn icms_do_item(i: usize, it: &Item, n: &Nota, uf: &str, out: &mut Vec<Rej>) {
    let ni = i + 1;
    let Some(ic) = &it.icms else { return };
    let cod = ic.codigo.trim();
    let csosn = cod.len() == 3;
    let cfop = cfop_de(it);
    let lista: &[&str] = if csosn { &CSOSN_OK } else { &CST_OK };
    if !lista.contains(&cod) {
        out.push(if csosn {
            r("383", "ICMS", format!("Item com CSOSN indevido [nItem: {ni}]"))
        } else {
            r("766", "ICMS", format!("Item com CST indevido [nItem:{ni}]"))
        });
    }
    if (cod == "90" || cod == "900") && ic.mod_bc_st.is_some() {
        out.push(if csosn {
            r("385", "ICMS", format!("Grupo de tributacao ICMSSN900, informando dados do ICMS-ST [nItem: {ni}]"))
        } else {
            r("381", "ICMS", format!("Grupo de tributacao ICMS90, informando dados do ICMS-ST [nItem:{ni}]"))
        });
    }
    cfop_do_icms(ni, cfop, cod, csosn, uf, out);
    if ic.partilha {
        out.push(r("741", "ICMSPart", "NFC-e ou NF-e com DANFE Simplificado Tipo 2 com Partilha de ICMS entre UF"));
    }
    if ic.repasse {
        out.push(r("740", "ICMSST", format!("Item com Repasse de ICMS retido por Substituto Tributario [nItem: {ni}]")));
    }
    if n.emit.crt == 4 && n.id_dest != 3 {
        if csosn && !["102", "300"].contains(&cod) {
            out.push(r("782", "ICMS", format!("CSOSN invalido para emitente MEI (CRT=4) [nItem:{ni}]")));
        }
        if cfop != "5102" {
            out.push(r("337", "CFOP", format!("CFOP invalido para emitente MEI (CRT=4) [nItem:{ni}]")));
        }
    }
}

/// N12-40, N12-44, N12a-40 e N12a-44: o CFOP que cada CST ou CSOSN admite (382 e 386).
fn cfop_do_icms(ni: usize, cfop: &str, cod: &str, csosn: bool, uf: &str, out: &mut Vec<Rej>) {
    let tributado = if csosn {
        ["102", "103", "300", "400", "900"].contains(&cod)
    } else {
        ["00", "20", "40", "41", "90"].contains(&cod)
    };
    let st = cod == "60" || (csosn && cod == "500");
    let ok = (tributado && CFOP_TRIBUTADO.contains(&cfop)) || (st && CFOP_ST.contains(&cfop));
    if (tributado || st) && !ok && !excecao_de_uf(uf, cfop, cod) {
        let (cs, doc) = if csosn { ("386", "CSOSN") } else { ("382", "CST") };
        out.push(r(cs, "CFOP", format!("CFOP nao permitido para o {doc} informado [nItem:{ni}]")));
    }
}

fn item(i: usize, it: &Item, n: &Nota, uf: &str, out: &mut Vec<Rej>) {
    let ni = i + 1;
    let cfop = cfop_de(it);
    let cod = it.icms.as_ref().map_or("", |c: &Icms| c.codigo.trim());
    if !CFOP_NOTA.contains(&cfop) && !excecao_de_uf(uf, cfop, cod) {
        out.push(r("725", "CFOP", format!("NFC-e ou NF-e com DANFE Simplificado Tipo 2 com CFOP invalido[nItem:{ni}]")));
    }
    if it.ind_tot == 0 {
        out.push(r("774", "indTot", format!("NFC-e ou NF-e com DANFE Simplificado Tipo 2 com indicador de item nao participante do total [nItem: {ni}]")));
    }
    if it.veiculo.is_some() {
        out.push(r("736", "veicProd", "NFC-e ou NF-e com DANFE Simplificado Tipo 2 com grupo de Veiculos novos"));
    }
    if it.ipi.is_some() {
        out.push(r("742", "IPI", "NFC-e ou NF-e com DANFE Simplificado Tipo 2 com grupo do IPI"));
    }
    if it.icms_uf_dest.is_some() {
        out.push(r("807", "ICMSUFDest", "NFC-e ou NF-e com DANFE Simplificado Tipo 2 com grupo de ICMS para a UF do destinatario"));
    }
    if it.dfe_ref.is_some() {
        out.push(r("708", "DFeReferenciado", "NFC-e ou NF-e com DANFE Simplificado Tipo 2 nao pode referenciar documento fiscal"));
    }
    icms_do_item(i, it, n, uf, out);
}

fn tem_destinatario(n: &Nota) -> bool {
    n.dest.as_ref().is_some_and(|d| {
        d.documento.as_deref().is_some_and(|s| !s.trim().is_empty()) || d.id_estrangeiro.is_some()
    })
}

fn ide(n: &Nota, out: &mut Vec<Rej>) {
    const T2: &str = "NFC-e ou NF-e com DANFE Simplificado Tipo 2";
    if n.saida.is_some() {
        out.push(r("705", "dhSaiEnt", format!("{T2} com data de entrada/saida")));
    }
    if n.tp_nf == 0 {
        out.push(r("706", "tpNF", format!("{T2} para operacao de entrada")));
    }
    if n.id_dest != 1 {
        out.push(r("707", "idDest", format!("{T2} para operacao interestadual ou com o exterior")));
    }
    if n.fin_nfe != 1 {
        out.push(r("715", "finNFe", format!("{T2} com finalidade invalida")));
    }
    if ind_final(n) == 0 {
        out.push(r("716", "indFinal", format!("{T2} em operacao nao destinada a consumidor final")));
    }
    if ![1, 4, 5].contains(&ind_pres(n)) {
        out.push(r("717", "indPres", format!("{T2} em operacao nao permitida")));
    }
    if ind_pres(n) == 4 && !tem_destinatario(n) {
        out.push(r("787", "dest", format!("{T2} (tpImp 6) em operacao nao presencial sem a identificacao do destinatario")));
    }
    if !n.refs_nfe.is_empty() {
        out.push(r("708", "NFref", format!("{T2} nao pode referenciar documento fiscal")));
    }
    if n.emit.iest.as_deref().is_some_and(|s| !s.trim().is_empty()) {
        out.push(r("718", "IEST", format!("{T2} nao deve informar IE de Substituto Tributario")));
    }
    if n.retirada.is_some() {
        out.push(r("669", "retirada", "Local de retirada informado indevidamente"));
    }
    if n.modelo == 65 && n.dest.as_ref().is_some_and(|d| d.ind_ie_dest != 9) && tem_destinatario(n) {
        out.push(r("789", "indIEDest", "NFC-e para destinatario contribuinte de ICMS"));
    }
}

/// Emissao: `tpImp` e `tpEmis` que cada modelo aceita.
fn emissao(n: &Nota, out: &mut Vec<Rej>) {
    let (imp, emis) = (tp_imp(n), n.tp_emis);
    if n.modelo == 65 {
        if imp != 4 && imp != 5 {
            out.push(r("709", "tpImp", "NFC-e com formato de DANFE invalido"));
        }
        if matches!(emis, 2 | 5) {
            out.push(r("714", "tpEmis", "NFC-e com contingencia invalida (tpEmis=2, 4 (a criterio da UF) ou 5)"));
        }
        if matches!(emis, 3 | 6 | 7) {
            out.push(r("783", "tpEmis", "NFC-e nao e' autorizada pela SVC"));
        }
    } else if matches!(imp, 4 | 5) {
        out.push(r("710", "tpImp", "NF-e com formato de DANFE invalido"));
    }
    if n.modelo == 55 && emis == 9 && imp != 6 {
        out.push(r("711", "tpEmis", "NF-e com contingencia off-line em operacao nao permitida"));
    }
}

fn transporte_e_grupos(n: &Nota, out: &mut Vec<Rej>) {
    const T2: &str = "NFC-e ou NF-e com DANFE Simplificado Tipo 2";
    if n.mod_frete != 9 && ind_pres(n) != 4 {
        out.push(r("753", "modFrete", format!("{T2} (tpImp=6) com Frete")));
    }
    if let Some(t) = &n.transp {
        if t.ret_transp.is_some() {
            out.push(r("755", "retTransp", format!("{T2} com dados de Retencao do ICMS no Transporte")));
        }
        if t.veic.is_some() {
            out.push(r("756", "veicTransp", format!("{T2} com dados do veiculo de Transporte")));
        }
        if !t.reboque.is_empty() {
            out.push(r("757", "reboque", format!("{T2} com dados de Reboque do veiculo de Transporte")));
        }
        if t.vagao.is_some() {
            out.push(r("758", "vagao", format!("{T2} com dados do Vagao de Transporte")));
        }
        if t.balsa.is_some() {
            out.push(r("759", "balsa", format!("{T2} com dados da Balsa de Transporte")));
        }
    }
    if n.cobr.is_some() {
        out.push(r("760", "cobr", format!("{T2} com dados de cobranca (Fatura, Duplicata)")));
    }
    if n.compra.is_some() {
        out.push(r("762", "compra", format!("{T2} com dados de compras (Empenho, Pedido, Contrato)")));
    }
    if n.modelo == 65 && n.pagamentos.iter().any(|p| format!("{:0>2}", p.t_pag.trim()) == "99") {
        out.push(r("436", "tPag", "Informado 99-Outros como meio de pagamento"));
    }
}

/// As rejeicoes que a nota do varejo levaria e que se veem so' no pedido, na ordem do leiaute. Vazia fora do varejo.
#[must_use]
pub(crate) fn regras(n: &Nota) -> Vec<Rej> {
    let mut out = Vec::new();
    if n.modelo == 65 || n.modelo == 55 {
        emissao(n, &mut out);
    }
    if !varejo(n) {
        return out;
    }
    let uf = n.emit.endereco.uf.trim().to_uppercase();
    ide(n, &mut out);
    for (i, it) in n.itens.iter().enumerate() {
        item(i, it, n, &uf, &mut out);
    }
    transporte_e_grupos(n, &mut out);
    out
}

/// As rejeicoes do valor total (W16-30, W16-40 e W16-60). `v_nf` e' o `vNF` montado, em texto.
#[must_use]
pub(crate) fn regras_do_total(n: &Nota, v_nf: &str) -> Vec<Rej> {
    let mut out = Vec::new();
    if !varejo(n) {
        return out;
    }
    let Ok(v) = Dec::parse(v_nf, "vNF") else {
        return out;
    };
    let cent = v.arred(2).m;
    if cent > LIMITE_TOTAL {
        out.push(r("780", "vNF", "Total da NFC-e ou NF-e com DANFE Simplificado Tipo 2 superior ao valor limite estabelecido pela SEFAZ [Limite: R$ 200.000,00]"));
    }
    if cent > LIMITE_SEM_DEST {
        if !tem_destinatario(n) {
            out.push(r("750", "dest", "NFC-e ou NF-e com DANFE Simplificado Tipo 2 com valor total superior ao permitido para destinatario nao identificado (Codigo) [Limite: R$ 10.000,00]"));
        } else if n.dest.as_ref().is_some_and(|d| d.endereco.is_none()) {
            out.push(r("752", "enderDest", "NFC-e ou NF-e com DANFE Simplificado Tipo 2 com valor total superior ao permitido para destinatario nao identificado (Endereco) [Limite: R$ 10.000,00]"));
        }
    }
    out
}

/// A primeira rejeicao como erro de montagem.
pub(crate) fn primeira(rs: Vec<Rej>) -> super::Res<()> {
    match rs.into_iter().next() {
        None => Ok(()),
        Some(x) => Err(super::formato::erro_r(x.msg, x.campo, x.cstat)),
    }
}

/// Cada rejeicao na forma da lista do `conferir`: `<cStat>-Rejeicao: <texto>`.
pub(crate) fn como_lista(rs: &[Rej]) -> Vec<String> {
    rs.iter()
        .map(|x| format!("{}-Rejeicao: {}", x.cstat, x.msg))
        .collect()
}
