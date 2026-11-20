// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Leiaute: onde cada bloco comeca e quantas linhas a descricao do servico e as informacoes complementares ocupam.

use serde_json::{json, Value};

use super::medidas::*;
use super::metrica::{caber, limitar, quebrar};

fn ajustar_compl(linhas: Vec<String>, linha_trib: &str, n: usize) -> Vec<String> {
    if linhas.len() <= n {
        return linhas;
    }
    let trib = quebrar(linha_trib, T_COMPL, larg_texto());
    let sobra = n.saturating_sub(trib.len());
    if sobra == 0 {
        return limitar(trib, n, T_COMPL, larg_texto());
    }
    let mut corpo: Vec<String> = linhas.into_iter().take(sobra).collect();
    let ult = corpo.len() - 1;
    corpo[ult] = caber(
        &format!("{}...", corpo[ult].trim_end_matches([' ', '|'])),
        T_COMPL,
        larg_texto(),
        false,
    );
    corpo.extend(trib);
    corpo
}

pub(super) fn leiaute(c: &Value, canhoto: bool) -> Value {
    let alt = |v: bool| if v { 1.94 } else { 0.40 };
    let pt = &c["partes"];
    let vis = |k: &str| pt[k]["visivel"].as_bool().unwrap_or(false);
    let tomador = 6.92;
    let destinatario = tomador + alt(vis("tomador"));
    let intermediario = destinatario + alt(vis("destinatario"));
    let servico = intermediario + alt(vis("intermediario"));
    let municipal = servico + (14.43 - 12.74);
    let federal = municipal
        + if c["municipal"]["visivel"].as_bool().unwrap_or(false) {
            2.59
        } else {
            0.32
        };
    let ibscbs = federal + (18.32 - 17.02);
    let totais = ibscbs + (20.90 - 18.32);
    let complementares = totais + (22.27 - 20.90);

    let desc = quebrar(s(&c["servico"]["descricao"]), T_VAL, larg_texto());
    let compl = quebrar(s(&c["complementares"]["texto"]), T_COMPL, larg_texto());
    let mut n_desc = desc.len().max(1);
    let n_compl = compl.len().max(1);
    let reserva = n_compl.min(MIN_LINHAS_COMPL);
    let fim = if canhoto { Y_CANHOTO } else { Y_CANHOTO_FIM };
    let base = complementares + 0.41;
    let disp = (fim - base - FOLGA_INFO).max(0.0);
    let extra = (n_desc - 1) as f64 * passo_linha();
    let pedido = reserva as f64 * passo_compl();
    if extra + pedido > disp {
        let max_extra = (disp - pedido).max(0.0);
        n_desc = n_desc.min(1 + (max_extra / passo_linha() + 1e-9).floor() as usize);
    }
    n_desc = n_desc.max(1);
    let desloc = (n_desc - 1) as f64 * passo_linha();
    let topo_c = base + desloc;
    let n_c = (((fim - topo_c - FOLGA_INFO) / passo_compl() + 1e-9).floor() as i64).max(1) as usize;
    json!({
        "tomador": r4(tomador), "destinatario": r4(destinatario), "intermediario": r4(intermediario),
        "servico": r4(servico), "municipal": r4(municipal), "federal": r4(federal), "ibscbs": r4(ibscbs),
        "totais": r4(totais), "complementares": r4(complementares), "desloc": r4(desloc),
        "descricao": limitar(desc, n_desc, T_VAL, larg_texto()),
        "complementares_linhas": ajustar_compl(compl, s(&c["complementares"]["linha_tributos"]), n_c),
    })
}
