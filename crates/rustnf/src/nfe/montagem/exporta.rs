// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Os grupos ZA `exporta` (comercio exterior: local de saida do pais) e ZB `compra` (nota de empenho, pedido e
//! contrato), na ordem do XSD PL_010h. Fontes: MOC 7.00, Anexo I, grupo ZA (rejeicoes 355 e 356) e grupo ZB (as
//! rejeicoes 359 a 361, sobre a nota de empenho, sao opcionais a criterio de cada UF e dependem do cadastro dela).

use super::formato::*;
use super::modelo::Nota;
use super::X;

/// ZA01 `exporta`: o local de embarque ou de transposicao de fronteira. E' obrigatorio na exportacao (`tpNF` 1 e `idDest`
/// 3, rejeicao 355) e proibido fora dela (rejeicao 356).
#[derive(Debug, Clone, Default)]
pub struct Exportacao {
    /// ZA02 `UFSaidaPais`: sigla da UF de embarque (sem `EX`).
    pub uf_saida_pais: String,
    /// ZA03 `xLocExporta`: local de embarque, de 1 a 60 caracteres.
    pub x_loc_exporta: String,
    /// ZA04 `xLocDespacho`: local de despacho, de 1 a 60 caracteres.
    pub x_loc_despacho: Option<String>,
}

/// ZB01 `compra`: informacoes de compra. Pelo menos um campo.
#[derive(Debug, Clone, Default)]
pub struct Compra {
    /// ZB02 `xNEmp`: nota de empenho, de 1 a 22 caracteres.
    pub x_n_emp: Option<String>,
    /// ZB03 `xPed`: pedido, de 1 a 60 caracteres.
    pub x_ped: Option<String>,
    /// ZB04 `xCont`: contrato, de 1 a 60 caracteres.
    pub x_cont: Option<String>,
}

/// Escreve o `exporta`; confere a presenca contra a operacao (355 e 356).
pub(crate) fn exporta(x: &mut X, n: &Nota) -> Res<()> {
    let exportacao = n.tp_nf == 1 && n.id_dest == 3;
    match (&n.exporta, exportacao) {
        (None, true) => Err(erro_r(
            "exportacao (tpNF 1 e idDest 3) sem o local de saida do pais (exporta)",
            "exporta",
            "355",
        )),
        (Some(_), false) => Err(erro_r(
            "o local de saida do pais (exporta) so' vai na exportacao (tpNF 1 e idDest 3)",
            "exporta",
            "356",
        )),
        (None, false) => Ok(()),
        (Some(e), true) => {
            let ra = n.remover_acento;
            let uf = e.uf_saida_pais.trim().to_uppercase();
            if uf_codigo(&uf).is_none() {
                return Err(erro(
                    format!("UFSaidaPais: '{uf}' nao e' sigla de UF (o XSD nao aceita EX)"),
                    "UFSaidaPais",
                ));
            }
            x.abre("exporta");
            x.t("UFSaidaPais", &uf);
            x.t(
                "xLocExporta",
                &txt(&e.x_loc_exporta, "xLocExporta", 60, 1, ra)?,
            );
            x.o(
                "xLocDespacho",
                &txt_ou_none(&e.x_loc_despacho, "xLocDespacho", 60, 1, ra)?,
            );
            x.fecha("exporta");
            Ok(())
        }
    }
}

/// Escreve o `compra`, se a nota tem.
pub(crate) fn compra(x: &mut X, n: &Nota) -> Res<()> {
    let Some(c) = &n.compra else { return Ok(()) };
    let ra = n.remover_acento;
    let emp = txt_ou_none(&c.x_n_emp, "xNEmp", 22, 1, ra)?;
    let ped = txt_ou_none(&c.x_ped, "compra/xPed", 60, 1, ra)?;
    let cont = txt_ou_none(&c.x_cont, "xCont", 60, 1, ra)?;
    if emp.is_none() && ped.is_none() && cont.is_none() {
        return Err(erro("compra sem nenhum campo", "compra"));
    }
    x.abre("compra");
    x.o("xNEmp", &emp);
    x.o("xPed", &ped);
    x.o("xCont", &cont);
    x.fecha("compra");
    Ok(())
}
