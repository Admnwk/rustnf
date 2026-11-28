// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! DPS v1.01 COMPLETA: modelo + montagem na ordem do XSD + Id de 45 + regras do Anexo I sem rede.
//! Dois leiautes por `Dps.leiaute`: "producao" (padrao) e "nt009" (XSD da Producao Restrita de 24/11/2026, Releases 1
//! e 2 da NT 009).
//! Os nomes de campo seguem o JSON do contrato (entra direto por serde); a ordem de recusa e o XML gerado,
//! byte a byte, estao em `tests/casos/nfse_nacional.json` (secoes `dps` e `dps_erro`).

use serde::{Deserialize, Serialize};

use super::base::{
    casa_id_dps, centavos, confere, data, data_hora, dinheiro, dinheiro_com_sinal, documento,
    regra, so_alnum, so_digitos, texto, ErroNfse, No, Txt, Valor, NS, R, VERSAO,
};
use super::ctribnac;
use crate::chave_dfe;

mod ibscbs;
mod modelo;
mod partes;
mod regras;
mod valores;

pub use modelo::*;
pub use partes::aliquota_iss;
pub use regras::{CINDOP_IMOVEL, ITENS_COM_TP_OPER, SUBITENS_OBRA};

use ibscbs::*;
use partes::*;
use regras::*;
use valores::*;

/// Id de 45 (TSIdDPS): 'DPS' + cMun(7) + tpInsc(1 CPF/2 CNPJ) + inscricao(14) + serie(5) + nDPS(15). A letra do CNPJ
/// alfanumerico e' preservada (nao se removem caracteres nao numericos).
pub fn id_dps(cmun: &str, inscricao: &str, serie: &str, numero: &str) -> R<String> {
    let (tipo, doc) = documento(inscricao)?;
    let s = serie.trim();
    let n = numero.trim();
    confere("TSCodMunIBGE", cmun, "infDPS/cLocEmi", "")?;
    confere(
        "TSSerieDPS",
        s,
        "infDPS/serie",
        "Serie de 1 a 5 digitos; de 90000 a 99999 e' reservada.",
    )?;
    confere(
        "TSNumDPS",
        n,
        "infDPS/nDPS",
        "Numero de 1 a 15 digitos, sem zero a esquerda.",
    )?;
    let r = format!(
        "DPS{cmun}{}{doc:0>14}{s:0>5}{n:0>15}",
        if tipo == "CPF" { "1" } else { "2" }
    );
    if !casa_id_dps(&r) {
        return Err(ErroNfse::novo(
            "infDPS/@Id",
            "CNPJ alfanumerico so' com letra nas 12 primeiras posicoes",
        ));
    }
    Ok(r)
}

fn competencia(dps: &Dps) -> String {
    match dps
        .competencia
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(c) => c.to_string(),
        None => chrono::Utc::now()
            .with_timezone(&super::base::brasilia())
            .format("%Y-%m-%d")
            .to_string(),
    }
}

/// XML da DPS (NAO assinado), com declaracao UTF-8. Erro = campo + motivo.
pub fn montar(dps: &Dps) -> R<String> {
    let lei = conferir_leiaute(dps)?;
    let emit: u8 = en(dps.emitente, &["1", "2", "3"], "infDPS/tpEmit")?
        .parse()
        .unwrap_or(1);
    let quem = match emit {
        1 => Some(&dps.prestador.pessoa),
        2 => dps.tomador.as_ref(),
        _ => dps.intermediario.as_ref(),
    };
    let doc_quem = quem
        .map(|q| q.documento.clone())
        .filter(|d| !d.is_empty())
        .ok_or_else(|| {
            ErroNfse::novo(
                "infDPS/@Id",
                "quem emite a DPS precisa de CPF/CNPJ (o Id leva a inscricao do emitente)",
            )
        })?;
    if emit != 1 && dps.motivo_emissao_ti.is_none() {
        return Err(ErroNfse::novo(
            "infDPS/cMotivoEmisTI",
            "emissao pelo tomador/intermediario exige o motivo",
        ));
    }
    if emit == 3 && dps.intermediario.is_none() {
        return Err(ErroNfse::novo(
            "infDPS/interm",
            "emissao pelo intermediario exige o grupo do intermediario",
        ));
    }
    let cmun = so_digitos(dps.cmun_emissor.as_str());
    let ident_ = id_dps(&cmun, &doc_quem, dps.serie.as_str(), dps.numero.as_str())?;
    let dh_emi = data_hora(dps.emissao.as_deref(), "infDPS/dhEmi")?;
    regras_cruzadas(dps, &ident_, &dh_emi)?;
    let mut raiz = No::new("DPS").attr("versao", VERSAO);
    let mut inf = No::new("infDPS").attr("Id", &ident_);
    inf.add("tpAmb", en(dps.ambiente, &["1", "2"], "infDPS/tpAmb")?);
    inf.add("dhEmi", dh_emi);
    inf.add(
        "verAplic",
        texto(&dps.ver_aplic, "infDPS/verAplic", Txt::max(20))?,
    );
    inf.add("serie", dps.serie.as_str().trim());
    inf.add("nDPS", dps.numero.as_str().trim());
    inf.add("dCompet", data(&competencia(dps), "infDPS/dCompet")?);
    inf.add("tpEmit", emit.to_string());
    if let Some(mt) = dps.motivo_emissao_ti {
        inf.add(
            "cMotivoEmisTI",
            en(mt, &["1", "2", "3", "4"], "infDPS/cMotivoEmisTI")?,
        );
    }
    if lei == LEIAUTE_NT009 {
        // NT 009 v1.01 item 2.2: finNFSe sai do IBSCBS, obrigatorio mesmo sem o grupo
        inf.add(
            "finNFSe",
            en(
                dps.ibscbs.as_ref().map_or(0, |g| g.fin),
                &["0"],
                "infDPS/finNFSe",
            )?,
        );
    }
    if !dps.chave_rejeitada.is_empty() {
        inf.add(
            "chNFSeRej",
            confere(
                "TSChaveNFSe",
                &so_alnum(&dps.chave_rejeitada),
                "infDPS/chNFSeRej",
                "",
            )?,
        );
    }
    inf.add("cLocEmi", cmun.clone());
    if let Some(s) = &dps.substituicao {
        let x = inf.grupo("subst");
        x.add(
            "chSubstda",
            confere(
                "TSChaveNFSe",
                &so_alnum(&s.chave),
                "infDPS/subst/chSubstda",
                "",
            )?,
        );
        x.add(
            "cMotivo",
            en(
                format!("{:0>2}", s.motivo.as_str()),
                &["01", "02", "03", "04", "05", "99"],
                "infDPS/subst/cMotivo",
            )?,
        );
        x.opc(
            "xMotivo",
            &texto(
                &s.texto,
                "infDPS/subst/xMotivo",
                Txt {
                    minimo: 15,
                    ..Txt::opc(255)
                },
            )?,
        );
    }
    inf.anexar(prestador(&dps.prestador, emit)?);
    if let Some(t) = &dps.tomador {
        inf.anexar(pessoa("toma", t, "infDPS/toma")?);
    }
    if lei == LEIAUTE_NT009 {
        destinatario_infdps(&mut inf, dps.ibscbs.as_ref())?;
    }
    if let Some(i) = &dps.intermediario {
        inf.anexar(pessoa("interm", i, "infDPS/interm")?);
    }
    inf.anexar(servico(&dps.servico, &cmun, dps.ctribnac_estrito, lei)?);
    inf.anexar(valores(&dps.valores, lei)?);
    if let Some(g) = &dps.ibscbs {
        inf.anexar(ibscbs(g, lei)?);
    }
    raiz.anexar(inf);
    regras_tot_trib(dps)?;
    Ok(format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>{}",
        raiz.xml(Some(NS))
    ))
}
