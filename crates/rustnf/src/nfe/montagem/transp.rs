// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Grupo X `transp` (modalidade do frete, transportadora, retencao do ICMS do transporte, veiculo, reboques, vagao,
//! balsa e volumes com lacres, X01 a X34), na ordem do XSD PL_010h. Fontes: MOC 7.00, Anexo I, grupo X: rejeicoes 542 e
//! 543 (documento da transportadora), 559 (UF da transportadora com IE), 868 (veiculo e reboque na operacao
//! interestadual) e 722 (CFOP da retencao do ICMS do transporte: tem de ser da Tabela CFOP com `indTransp` 1). A
//! inscricao estadual da transportadora (544) e o municipio do fato gerador da retencao (288) dependem de cadastro ou
//! de tabela publicada a parte: aqui so' se confere o formato do XSD.

use super::formato::*;
use super::item::digitos_ate;
use super::modelo::Nota;
use super::X;
use crate::chave_dfe;

/// X03 `transporta`: a transportadora; todos os campos sao opcionais no XSD, mas o grupo nao vai vazio.
#[derive(Debug, Clone, Default)]
pub struct Transportadora {
    /// X04 `CNPJ` ou X05 `CPF`: com digito verificador valido (rejeicoes 542 e 543).
    pub documento: Option<String>,
    /// X06 `xNome`: de 2 a 60 caracteres.
    pub nome: Option<String>,
    /// X07 `IE`: de 2 a 14 digitos ou `ISENTO`; com ela a UF e' obrigatoria (rejeicao 559).
    pub ie: Option<String>,
    /// X08 `xEnder`: endereco completo, de 1 a 60 caracteres.
    pub endereco: Option<String>,
    /// X09 `xMun`: de 1 a 60 caracteres.
    pub municipio: Option<String>,
    /// X10 `UF`: sigla, ou `EX` para o exterior.
    pub uf: Option<String>,
}

/// X11 `retTransp`: retencao do ICMS do transporte. Todos os campos sao obrigatorios.
#[derive(Debug, Clone, Default)]
pub struct RetencaoIcms {
    /// X12 `vServ`: valor do servico (2 casas).
    pub v_serv: Option<String>,
    /// X13 `vBCRet`: base da retencao (2 casas).
    pub v_bc_ret: Option<String>,
    /// X14 `pICMSRet`: aliquota da retencao, de 0 a 100.
    pub p_icms_ret: Option<String>,
    /// X15 `vICMSRet`: valor do ICMS retido (2 casas).
    pub v_icms_ret: Option<String>,
    /// X16 `CFOP`: 4 digitos, iniciado por 1, 2, 3, 5, 6 ou 7.
    pub cfop: String,
    /// X17 `cMunFG`: codigo IBGE, 7 digitos.
    pub c_mun_fg: String,
}

/// X18 `veicTransp` ou X22 `reboque`: placa, UF e registro na ANTT.
#[derive(Debug, Clone, Default)]
pub struct VeiculoTransp {
    /// X19 ou X23 `placa`: `XXX9999`, `XXX999`, `XX9999`, `XXXX999` ou 7 caracteres `[A-Z0-9]` (placa Mercosul).
    pub placa: String,
    /// X20 ou X24 `UF`: sigla, ou `EX`.
    pub uf: Option<String>,
    /// X21 ou X25 `RNTC`: de 1 a 20 caracteres.
    pub rntc: Option<String>,
}

/// X26 `vol`: um volume transportado. Todos os campos sao opcionais no XSD, mas o volume nao vai vazio.
#[derive(Debug, Clone, Default)]
pub struct VolumeTransp {
    /// X27 `qVol`: de 1 a 15 digitos.
    pub q_vol: Option<String>,
    /// X28 `esp`: especie, de 1 a 60 caracteres.
    pub esp: Option<String>,
    /// X29 `marca`: de 1 a 60 caracteres.
    pub marca: Option<String>,
    /// X30 `nVol`: de 1 a 60 caracteres.
    pub n_vol: Option<String>,
    /// X31 `pesoL`: peso liquido em kg (ate 12 inteiros e 3 casas).
    pub peso_l: Option<String>,
    /// X32 `pesoB`: peso bruto em kg (ate 12 inteiros e 3 casas).
    pub peso_b: Option<String>,
    /// X33 `lacres` / X34 `nLacre`: numero de cada lacre, de 1 a 60 caracteres, ate 5000.
    pub lacres: Vec<String>,
}

/// X01 a X34 alem do `modFrete` (que fica em [`Nota::mod_frete`]).
#[derive(Debug, Clone, Default)]
pub struct Transporte {
    /// X03 `transporta`.
    pub transporta: Option<Transportadora>,
    /// X11 `retTransp`.
    pub ret_transp: Option<RetencaoIcms>,
    /// X18 `veicTransp`: o veiculo trator.
    pub veic: Option<VeiculoTransp>,
    /// X22 `reboque`: reboques e dolly, ate 5.
    pub reboque: Vec<VeiculoTransp>,
    /// X25a `vagao`: identificacao do vagao (de 1 a 20 caracteres).
    pub vagao: Option<String>,
    /// X25b `balsa`: identificacao da balsa (de 1 a 20 caracteres).
    pub balsa: Option<String>,
    /// X26 `vol`, de 0 a 5000.
    pub vol: Vec<VolumeTransp>,
}

fn uf_ou_ex(v: &str, campo: &str) -> Res<String> {
    let uf = v.trim().to_uppercase();
    if uf_codigo(&uf).is_none() && uf != "EX" {
        return Err(erro(
            format!("{campo}: '{uf}' nao e' sigla de UF (nem EX)"),
            campo,
        ));
    }
    Ok(uf)
}

fn cheio(v: &Option<String>) -> Option<&str> {
    v.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

fn transportadora(x: &mut X, t: &Transportadora, ra: bool) -> Res<()> {
    let campos = [
        cheio(&t.documento),
        cheio(&t.nome),
        cheio(&t.ie),
        cheio(&t.endereco),
        cheio(&t.municipio),
        cheio(&t.uf),
    ];
    if campos.iter().all(Option::is_none) {
        return Err(erro("transporta sem nenhum campo", "transporta"));
    }
    x.abre("transporta");
    if let Some(d) = cheio(&t.documento) {
        let (tipo, doc) = documento(d, "transporta/CNPJ")?;
        let (ok, regra) = if tipo == "CPF" {
            (chave_dfe::cpf_valido(&doc), "543")
        } else {
            (chave_dfe::cnpj_valido(&doc), "542")
        };
        if !ok {
            return Err(erro_r(
                format!(
                    "{tipo} da transportadora invalido (zeros, repetido ou digito verificador)"
                ),
                &format!("transporta/{tipo}"),
                regra,
            ));
        }
        x.t(tipo, &doc);
    }
    x.o(
        "xNome",
        &txt_ou_none(&t.nome, "transporta/xNome", 60, 2, ra)?,
    );
    if let Some(ie) = cheio(&t.ie) {
        if cheio(&t.uf).is_none() {
            return Err(erro_r(
                "UF da transportadora nao informada: com a IE, a UF e' obrigatoria",
                "transporta/UF",
                "559",
            ));
        }
        let ie = ie.to_uppercase();
        let v = if ie == "ISENTO" {
            ie
        } else {
            digitos(&ie, "transporta/IE", &(2..15).collect::<Vec<_>>())?
        };
        x.t("IE", &v);
    }
    x.o(
        "xEnder",
        &txt_ou_none(&t.endereco, "transporta/xEnder", 60, 1, ra)?,
    );
    x.o(
        "xMun",
        &txt_ou_none(&t.municipio, "transporta/xMun", 60, 1, ra)?,
    );
    if let Some(uf) = cheio(&t.uf) {
        x.t("UF", &uf_ou_ex(uf, "transporta/UF")?);
    }
    x.fecha("transporta");
    Ok(())
}

/// Valor obrigatorio do grupo, em decimal.
fn exigido(v: &Option<String>, campo: &str) -> Res<Dec> {
    match cheio(v) {
        Some(s) => Dec::parse(s, campo),
        None => Err(erro(format!("retTransp/{campo}: obrigatorio"), campo)),
    }
}

fn retencao(x: &mut X, r: &RetencaoIcms) -> Res<()> {
    let serv = num_lim(exigido(&r.v_serv, "vServ")?, 13, 2, "vServ")?;
    let base = num_lim(exigido(&r.v_bc_ret, "vBCRet")?, 13, 2, "vBCRet")?;
    let p = exigido(&r.p_icms_ret, "pICMSRet")?;
    if p.negativo() || p.maior(Dec::de_int(100)) {
        return Err(erro("pICMSRet fora de 0 a 100", "pICMSRet"));
    }
    let p = num(p, if p.casas() <= 2 { 2 } else { 4 }, "pICMSRet")?;
    let valor = num_lim(exigido(&r.v_icms_ret, "vICMSRet")?, 13, 2, "vICMSRet")?;
    let cfop = digitos(&r.cfop, "retTransp/CFOP", &[4])?;
    if !"123567".contains(&cfop[..1]) {
        return Err(erro(
            format!("retTransp/CFOP '{cfop}': o primeiro digito e' 1, 2, 3, 5, 6 ou 7"),
            "retTransp/CFOP",
        ));
    }
    // 722 (MOC 7.00, Anexo I, grupo X): o CFOP tem de existir na Tabela CFOP com indTransp=1
    if !crate::cfop::info(&cfop).is_some_and(|c| c.ind_transp == 1) {
        return Err(erro_r(
            format!("retTransp/CFOP '{cfop}': CFOP de Transporte inexistente ou que nao pode ser usado na retencao do ICMS do transporte"),
            "retTransp/CFOP",
            "722",
        ));
    }
    let mun = digitos(&r.c_mun_fg, "retTransp/cMunFG", &[7])?;
    x.abre("retTransp");
    x.t("vServ", &serv);
    x.t("vBCRet", &base);
    x.t("pICMSRet", &p);
    x.t("vICMSRet", &valor);
    x.t("CFOP", &cfop);
    x.t("cMunFG", &mun);
    x.fecha("retTransp");
    Ok(())
}

/// `XXX9999`, `XXX999`, `XX9999`, `XXXX999` ou 7 caracteres `[A-Z0-9]` (os formatos do XSD).
fn placa_valida(p: &str) -> bool {
    let b = p.as_bytes();
    let letras = |s: &[u8]| s.iter().all(u8::is_ascii_uppercase);
    let numeros = |s: &[u8]| s.iter().all(u8::is_ascii_digit);
    match b.len() {
        6 => (letras(&b[..2]) && numeros(&b[2..])) || (letras(&b[..3]) && numeros(&b[3..])),
        7 => b
            .iter()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()),
        _ => false,
    }
}

fn veiculo(x: &mut X, tag: &str, v: &VeiculoTransp, ra: bool) -> Res<()> {
    let placa: String = v
        .placa
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .collect::<String>()
        .to_uppercase();
    if !placa_valida(&placa) {
        return Err(erro(
            format!("{tag}/placa '{}' fora dos formatos do XSD (XXX9999, XXX999, XX9999, XXXX999 ou 7 letras e numeros)", corta(&v.placa, 12)),
            &format!("{tag}/placa"),
        ));
    }
    x.abre(tag);
    x.t("placa", &placa);
    if let Some(uf) = cheio(&v.uf) {
        x.t("UF", &uf_ou_ex(uf, &format!("{tag}/UF"))?);
    }
    x.o(
        "RNTC",
        &txt_ou_none(&v.rntc, &format!("{tag}/RNTC"), 20, 1, ra)?,
    );
    x.fecha(tag);
    Ok(())
}

fn volume(x: &mut X, v: &VolumeTransp, ra: bool) -> Res<()> {
    let tem = [&v.q_vol, &v.esp, &v.marca, &v.n_vol, &v.peso_l, &v.peso_b]
        .iter()
        .any(|c| cheio(c).is_some())
        || !v.lacres.is_empty();
    if !tem {
        return Err(erro("vol sem nenhum campo", "vol"));
    }
    if v.lacres.len() > 5000 {
        return Err(erro(
            format!("vol/lacres: {} lacres (maximo 5000)", v.lacres.len()),
            "lacres",
        ));
    }
    x.abre("vol");
    if let Some(q) = cheio(&v.q_vol) {
        x.t("qVol", &digitos_ate(q, "vol/qVol", 15)?);
    }
    x.o("esp", &txt_ou_none(&v.esp, "vol/esp", 60, 1, ra)?);
    x.o("marca", &txt_ou_none(&v.marca, "vol/marca", 60, 1, ra)?);
    x.o("nVol", &txt_ou_none(&v.n_vol, "vol/nVol", 60, 1, ra)?);
    for (tag, p) in [("pesoL", &v.peso_l), ("pesoB", &v.peso_b)] {
        if let Some(s) = cheio(p) {
            x.t(tag, &num_lim(Dec::parse(s, tag)?, 12, 3, tag)?);
        }
    }
    for l in &v.lacres {
        x.abre("lacres");
        x.t("nLacre", &txt(l, "vol/nLacre", 60, 1, ra)?);
        x.fecha("lacres");
    }
    x.fecha("vol");
    Ok(())
}

/// O que o XSD aceita junto: o veiculo com os reboques, ou o vagao, ou a balsa.
fn um_meio_so(t: &Transporte) -> Res<()> {
    let meios = [
        (t.veic.is_some() || !t.reboque.is_empty(), "veicTransp"),
        (t.vagao.is_some(), "vagao"),
        (t.balsa.is_some(), "balsa"),
    ];
    let usados: Vec<&str> = meios.iter().filter(|m| m.0).map(|m| m.1).collect();
    if usados.len() > 1 {
        return Err(erro(
            format!(
                "transp: {} no mesmo transporte; o XSD aceita so' o veiculo (com reboques), o vagao ou a balsa",
                usados.join(" e ")
            ),
            usados[1],
        ));
    }
    Ok(())
}

/// Escreve o `transp`: o `modFrete` e, se o pedido traz, o resto do grupo.
pub(crate) fn transp(x: &mut X, n: &Nota) -> Res<()> {
    if ![0, 1, 2, 3, 4, 9].contains(&n.mod_frete) {
        return Err(erro(
            format!("modFrete invalido: {}", n.mod_frete),
            "modFrete",
        ));
    }
    let ra = n.remover_acento;
    x.abre("transp");
    x.t("modFrete", &n.mod_frete.to_string());
    if let Some(t) = &n.transp {
        um_meio_so(t)?;
        if n.id_dest == 2 && (t.veic.is_some() || !t.reboque.is_empty()) {
            return Err(erro_r(
                "operacao interestadual (idDest 2) nao leva veicTransp nem reboque",
                "veicTransp",
                "868",
            ));
        }
        if t.reboque.len() > 5 {
            return Err(erro(
                format!("reboque: {} reboques (maximo 5)", t.reboque.len()),
                "reboque",
            ));
        }
        if t.vol.len() > 5000 {
            return Err(erro(
                format!("vol: {} volumes (maximo 5000)", t.vol.len()),
                "vol",
            ));
        }
        if let Some(tr) = &t.transporta {
            transportadora(x, tr, ra)?;
        }
        if let Some(r) = &t.ret_transp {
            retencao(x, r)?;
        }
        if let Some(v) = &t.veic {
            veiculo(x, "veicTransp", v, ra)?;
        }
        for r in &t.reboque {
            veiculo(x, "reboque", r, ra)?;
        }
        x.o("vagao", &txt_ou_none(&t.vagao, "vagao", 20, 1, ra)?);
        x.o("balsa", &txt_ou_none(&t.balsa, "balsa", 20, 1, ra)?);
        for v in &t.vol {
            volume(x, v, ra)?;
        }
    }
    x.fecha("transp");
    Ok(())
}
