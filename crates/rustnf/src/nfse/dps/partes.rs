// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Montagem das partes da DPS na ordem do XSD: pessoas, enderecos, prestador, servico, deducoes e ajustes.

use super::*;

pub(super) fn en(v: impl ToString, validos: &[&str], campo: &str) -> R<String> {
    let s = v.to_string().trim().to_string();
    if validos.contains(&s.as_str()) {
        Ok(s)
    } else {
        Err(ErroNfse::novo(
            campo,
            format!("valor {s:?} fora da lista ({})", validos.join(", ")),
        ))
    }
}

pub(super) fn en_opt(v: Option<u8>, validos: &[&str], campo: &str) -> R<String> {
    match v {
        Some(x) => en(x, validos, campo),
        None => en("None", validos, campo),
    }
}

pub(super) const SIM_NAO: &[&str] = &["0", "1"];
pub(super) fn faixa(a: u8, b: u8) -> Vec<String> {
    (a..=b).map(|x| x.to_string()).collect()
}
pub(super) fn refs(v: &[String]) -> Vec<&str> {
    v.iter().map(String::as_str).collect()
}

pub(super) fn ident(no: &mut No, p: &Pessoa, caminho: &str) -> R<()> {
    ident_dv(no, p, caminho, None)
}

/// `dv` = (regra do CNPJ, regra do CPF) quando o grupo tem codigo proprio para o DV (dest: E0911/E0913).
pub(super) fn ident_dv(no: &mut No, p: &Pessoa, caminho: &str, dv: Option<(&str, &str)>) -> R<()> {
    if !p.documento.is_empty() {
        let (tipo, doc) = documento(&p.documento).map_err(|_| {
            ErroNfse::novo(
                format!("{caminho}/CNPJ|CPF"),
                format!("documento {:?} invalido", p.documento),
            )
        })?;
        let ok = if tipo == "CPF" {
            chave_dfe::cpf_valido(&doc)
        } else {
            chave_dfe::cnpj_valido(&doc)
        };
        let cod = match dv {
            Some((cnpj, cpf)) => {
                if tipo == "CPF" {
                    cpf
                } else {
                    cnpj
                }
            }
            None => "E0080/E0096/E0188/E0206",
        };
        regra(
            !ok,
            cod,
            &format!("{caminho}/{tipo}"),
            &format!("digito verificador do {tipo} nao confere"),
        )?;
        no.add(tipo, doc);
    } else if !p.nif.is_empty() {
        no.add(
            "NIF",
            texto(&p.nif, &format!("{caminho}/NIF"), Txt::max(40))?,
        );
    } else if let Some(s) = p.sem_nif {
        regra(
            s == 0 && caminho.ends_with("prest"),
            "E0115",
            &format!("{caminho}/cNaoNIF"),
            "motivo 0 (nao informado na nota de origem) nao e' aceito para o prestador",
        )?;
        no.add(
            "cNaoNIF",
            en(s, &["0", "1", "2"], &format!("{caminho}/cNaoNIF"))?,
        );
    } else {
        return Err(ErroNfse::novo(
            caminho,
            "identifique a pessoa: CPF/CNPJ, NIF (estrangeiro) ou o motivo de nao ter NIF",
        ));
    }
    Ok(())
}

pub(super) fn logradouro(
    n: &mut No,
    lgr: &str,
    nro: &str,
    cpl: &str,
    bairro: &str,
    c: &str,
) -> R<()> {
    n.add("xLgr", texto(lgr, &format!("{c}/xLgr"), Txt::max(255))?);
    n.add(
        "nro",
        texto(
            if nro.is_empty() { "S/N" } else { nro },
            &format!("{c}/nro"),
            Txt::max(60),
        )?,
    );
    n.opc("xCpl", &texto(cpl, &format!("{c}/xCpl"), Txt::opc(156))?);
    n.add(
        "xBairro",
        texto(bairro, &format!("{c}/xBairro"), Txt::max(60))?,
    );
    Ok(())
}

pub(super) fn exterior(n: &mut No, postal: &str, cidade: &str, estado: &str, c: &str) -> R<()> {
    n.add(
        "cEndPost",
        texto(postal, &format!("{c}/endExt/cEndPost"), Txt::max(11))?,
    );
    n.add(
        "xCidade",
        texto(cidade, &format!("{c}/endExt/xCidade"), Txt::max(60))?,
    );
    n.add(
        "xEstProvReg",
        texto(estado, &format!("{c}/endExt/xEstProvReg"), Txt::max(60))?,
    );
    Ok(())
}

pub(super) fn endereco(e: &Endereco, c: &str) -> R<No> {
    let mut n = No::new("end");
    if !e.pais.is_empty() && e.pais.to_uppercase() != "BR" {
        let x = n.grupo("endExt");
        x.add(
            "cPais",
            confere(
                "TSCodPaisISO",
                &e.pais.to_uppercase(),
                &format!("{c}/endExt/cPais"),
                "",
            )?,
        );
        exterior(x, &e.cod_postal, &e.cidade, &e.estado, c)?;
    } else {
        let x = n.grupo("endNac");
        x.add(
            "cMun",
            confere(
                "TSCodMunIBGE",
                &so_digitos(&e.cmun),
                &format!("{c}/endNac/cMun"),
                "",
            )?,
        );
        x.add(
            "CEP",
            confere("TSCEP", &so_digitos(&e.cep), &format!("{c}/endNac/CEP"), "")?,
        );
    }
    logradouro(
        &mut n,
        &e.logradouro,
        &e.numero,
        &e.complemento,
        &e.bairro,
        c,
    )?;
    Ok(n)
}

pub(super) fn endereco_simples(e: &EnderecoSimples, c: &str) -> R<No> {
    let mut n = No::new("end");
    if !e.cep.is_empty() {
        n.add(
            "CEP",
            confere("TSCEP", &so_digitos(&e.cep), &format!("{c}/CEP"), "")?,
        );
    } else {
        let x = n.grupo("endExt");
        exterior(x, &e.cod_postal, &e.cidade, &e.estado, c)?;
    }
    logradouro(
        &mut n,
        &e.logradouro,
        &e.numero,
        &e.complemento,
        &e.bairro,
        c,
    )?;
    Ok(n)
}

pub(super) fn contato(no: &mut No, p: &Pessoa, c: &str) -> R<()> {
    if let Some(e) = &p.endereco {
        no.anexar(endereco(e, &format!("{c}/end"))?);
    }
    fone_email(no, p, c)
}

pub(super) fn fone_email(no: &mut No, p: &Pessoa, c: &str) -> R<()> {
    if !p.fone.is_empty() {
        no.add(
            "fone",
            confere(
                "TSTelefone",
                &so_digitos(&p.fone),
                &format!("{c}/fone"),
                "6 a 20 digitos",
            )?,
        );
    }
    no.opc(
        "email",
        &texto(&p.email, &format!("{c}/email"), Txt::opc(80))?,
    );
    Ok(())
}

pub(super) fn pessoa(tag: &str, p: &Pessoa, c: &str) -> R<No> {
    let mut n = No::new(tag);
    ident(&mut n, p, c)?;
    if !p.caepf.is_empty() {
        n.add(
            "CAEPF",
            confere("TSCAEPF", &so_digitos(&p.caepf), &format!("{c}/CAEPF"), "")?,
        );
    }
    n.opc("IM", &texto(&p.im, &format!("{c}/IM"), Txt::opc(15))?);
    n.add(
        "xNome",
        texto(&p.nome, &format!("{c}/xNome"), Txt::max(300))?,
    );
    contato(&mut n, p, c)?;
    Ok(n)
}

pub(super) fn prestador(pr: &Prestador, emitente: u8) -> R<No> {
    let c = "infDPS/prest";
    let p = &pr.pessoa;
    let mut n = No::new("prest");
    regra(
        emitente == EMITENTE_PRESTADOR && p.documento.is_empty(),
        "E0112/E0114",
        c,
        "quando o prestador emite, ele se identifica por CPF/CNPJ (NIF/cNaoNIF nao)",
    )?;
    ident(&mut n, p, c)?;
    if !p.caepf.is_empty() {
        n.add(
            "CAEPF",
            confere("TSCAEPF", &so_digitos(&p.caepf), &format!("{c}/CAEPF"), "")?,
        );
    }
    n.opc("IM", &texto(&p.im, &format!("{c}/IM"), Txt::opc(15))?);
    if emitente == EMITENTE_PRESTADOR {
        // E0121 + E0128: quando o proprio prestador emite, nome e endereco NAO vao (o SEFIN usa o cadastro).
        fone_email(&mut n, p, c)?;
    } else {
        regra(
            p.nome.trim().is_empty(),
            "E0122",
            &format!("{c}/xNome"),
            "quando o tomador/intermediario emite, o nome do prestador e' obrigatorio",
        )?;
        regra(
            p.endereco.is_none(),
            "E0129",
            &format!("{c}/end"),
            "quando o tomador/intermediario emite, o endereco do prestador e' obrigatorio",
        )?;
        n.add(
            "xNome",
            texto(&p.nome, &format!("{c}/xNome"), Txt::max(300))?,
        );
        contato(&mut n, p, c)?;
    }
    let r = n.grupo("regTrib");
    let op: u8 = en(
        pr.op_simples,
        &["1", "2", "3"],
        &format!("{c}/regTrib/opSimpNac"),
    )?
    .parse()
    .unwrap_or(0);
    r.add("opSimpNac", op.to_string());
    regra(
        op != 3 && pr.apuracao_sn.is_some(),
        "E0162",
        &format!("{c}/regTrib/regApTribSN"),
        "regime de apuracao do Simples so' existe para optante ME/EPP",
    )?;
    regra(
        op == 3 && pr.apuracao_sn.is_none(),
        "E0166",
        &format!("{c}/regTrib/regApTribSN"),
        "optante ME/EPP precisa do regime de apuracao (1, 2 ou 3; pergunte ao contador)",
    )?;
    if let Some(a) = pr.apuracao_sn {
        r.add(
            "regApTribSN",
            en(a, &["1", "2", "3"], &format!("{c}/regTrib/regApTribSN"))?,
        );
    }
    let esp = en(
        pr.regime_especial,
        &["0", "1", "2", "3", "4", "5", "6", "9"],
        &format!("{c}/regTrib/regEspTrib"),
    )?;
    regra(
        op == 2 && esp != "0",
        "E0174",
        &format!("{c}/regTrib/regEspTrib"),
        "MEI nao tem regime especial (use 0)",
    )?;
    regra(
        op == 3 && pr.apuracao_sn == Some(1) && esp != "0",
        "E0175",
        &format!("{c}/regTrib/regEspTrib"),
        "ME/EPP que apura tudo pelo Simples nao tem regime especial (use 0)",
    )?;
    r.add("regEspTrib", esp);
    Ok(n)
}

pub(super) fn servico(s: &Servico, cmun_emissor: &str, estrito: bool, leiaute: &str) -> R<No> {
    let c = "infDPS/serv";
    let mut n = No::new("serv");
    {
        let loc = n.grupo("locPrest");
        if !s.pais_prestacao.is_empty() && s.pais_prestacao.to_uppercase() != "BR" {
            loc.add(
                "cPaisPrestacao",
                confere(
                    "TSCodPaisISO",
                    &s.pais_prestacao.to_uppercase(),
                    &format!("{c}/locPrest/cPaisPrestacao"),
                    "",
                )?,
            );
        } else {
            let m = if s.cmun_prestacao.is_empty() {
                cmun_emissor
            } else {
                &s.cmun_prestacao
            };
            loc.add(
                "cLocPrestacao",
                confere(
                    "TSCodMunIBGE",
                    &so_digitos(m),
                    &format!("{c}/locPrest/cLocPrestacao"),
                    "",
                )?,
            );
        }
    }
    {
        let cs = n.grupo("cServ");
        cs.add(
            "cTribNac",
            ctribnac::normalizar(s.ctribnac.as_str(), estrito)?,
        );
        if !s.ctribmun.is_empty() {
            cs.add(
                "cTribMun",
                confere(
                    "TSCodTribMun",
                    &so_digitos(&s.ctribmun),
                    &format!("{c}/cServ/cTribMun"),
                    "",
                )?,
            );
        }
        let desc = texto(
            &s.descricao,
            &format!("{c}/cServ/xDescServ"),
            Txt {
                minimo: 1,
                maximo: 2000,
                quebra: true,
                obrigatorio: true,
                so_latin1: false,
            },
        )?;
        let nt009 = leiaute == LEIAUTE_NT009;
        if !nt009 {
            cs.add("xDescServ", desc.clone());
        }
        if !s.cnbs.is_empty() {
            cs.add(
                "cNBS",
                confere(
                    "TSCodNBS",
                    &so_digitos(&s.cnbs),
                    &format!("{c}/cServ/cNBS"),
                    "",
                )?,
            );
        }
        if !s.cod_interno.is_empty() {
            cs.add(
                "cIntContrib",
                confere(
                    "TSCIntContrib",
                    &s.cod_interno,
                    &format!("{c}/cServ/cIntContrib"),
                    "",
                )?,
            );
        }
        if nt009 {
            // XSD prodrest 20/10/2026: xDescServ vai para o fim do cServ
            cs.add("xDescServ", desc);
        }
    }
    if let Some(e) = &s.comercio_exterior {
        let ce = format!("{c}/comExt");
        let r2 = leiaute == LEIAUTE_NT009;
        conferir_comext_do_leiaute(e, &ce, r2)?;
        let x = n.grupo("comExt");
        // XSD prodrest 24/11/2026: TSModoPrestacao ganha o 5 (consumo concomitante no pais e no exterior)
        x.add(
            "mdPrestacao",
            en(
                e.modo,
                &refs(&faixa(0, if r2 { 5 } else { 4 })),
                &format!("{ce}/mdPrestacao"),
            )?,
        );
        x.add(
            "vincPrest",
            en(
                e.vinculo,
                &["0", "1", "2", "3", "4", "5", "6", "9"],
                &format!("{ce}/vincPrest"),
            )?,
        );
        x.add(
            "tpMoeda",
            confere(
                "TSCodMoeda",
                &so_digitos(e.moeda.as_str()),
                &format!("{ce}/tpMoeda"),
                "",
            )?,
        );
        if r2 {
            // TCComExterior do XSD prodrest 24/11/2026: nFatura e vFatura (opcionais) no lugar do vServMoeda
            if !e.num_fatura.trim().is_empty() {
                x.add(
                    "nFatura",
                    confere(
                        "TSNumFatura",
                        e.num_fatura.trim(),
                        &format!("{ce}/nFatura"),
                        "ate 20 digitos",
                    )?,
                );
            }
            if let Some(vf) = &e.valor_fatura {
                x.add(
                    "vFatura",
                    dinheiro(vf.as_str(), &format!("{ce}/vFatura"), 15, true)?,
                );
            }
        } else {
            x.add(
                "vServMoeda",
                dinheiro(
                    e.valor_moeda.as_str(),
                    &format!("{ce}/vServMoeda"),
                    15,
                    true,
                )?,
            );
        }
        let p: Vec<String> = (0..9).map(|i| format!("{i:02}")).collect();
        let t: Vec<String> = (0..27).map(|i| format!("{i:02}")).collect();
        x.add(
            "mecAFComexP",
            en(
                format!("{:0>2}", e.mec_prestador.as_str()),
                &refs(&p),
                &format!("{ce}/mecAFComexP"),
            )?,
        );
        x.add(
            "mecAFComexT",
            en(
                format!("{:0>2}", e.mec_tomador.as_str()),
                &refs(&t),
                &format!("{ce}/mecAFComexT"),
            )?,
        );
        x.add(
            "movTempBens",
            en(
                e.mov_temp_bens,
                &refs(&faixa(0, 3)),
                &format!("{ce}/movTempBens"),
            )?,
        );
        if r2 {
            // TSNumDUIMP (15) e TSNumDUE (14): tamanho exato, sem padrao de caracteres no XSD
            let exato = |n: usize| Txt {
                minimo: n,
                ..Txt::opc(n)
            };
            let duimp = texto(&e.num_duimp, &format!("{ce}/nDUIMP"), exato(15))?;
            let due = texto(&e.num_due, &format!("{ce}/nDUE"), exato(14))?;
            regras_mov_temp_bens(e.mov_temp_bens, &duimp, &due, &ce)?;
            x.opc("nDUIMP", &duimp);
            x.opc("nDUE", &due);
        } else {
            x.opc(
                "nDI",
                &texto(&e.num_di, &format!("{ce}/nDI"), Txt::opc(12))?,
            );
            x.opc(
                "nRE",
                &texto(&e.num_re, &format!("{ce}/nRE"), Txt::opc(12))?,
            );
        }
        x.add("mdic", en(e.mdic, SIM_NAO, &format!("{ce}/mdic"))?);
        if r2 && !e.contrato_cambio.trim().is_empty() {
            x.add(
                "nContCambio",
                confere(
                    "TSNumContCambio",
                    e.contrato_cambio.trim(),
                    &format!("{ce}/nContCambio"),
                    "ate 20 digitos",
                )?,
            );
        }
    }
    if let Some(o) = &s.obra {
        let co = format!("{c}/obra");
        let x = n.grupo("obra");
        x.opc(
            "inscImobFisc",
            &texto(
                &o.inscricao_imobiliaria,
                &format!("{co}/inscImobFisc"),
                Txt::opc(30),
            )?,
        );
        if !o.codigo.is_empty() {
            x.add(
                "cObra",
                texto(&o.codigo, &format!("{co}/cObra"), Txt::max(30))?,
            );
        } else if !o.cib.is_empty() {
            x.add(
                "cCIB",
                texto(
                    &o.cib,
                    &format!("{co}/cCIB"),
                    Txt {
                        minimo: 8,
                        ..Txt::max(8)
                    },
                )?,
            );
        } else if let Some(e) = &o.endereco {
            x.anexar(endereco_simples(e, &format!("{co}/end"))?);
        } else {
            return Err(ErroNfse::novo(
                co,
                "identifique a obra: codigo (CNO), CIB ou endereco",
            ));
        }
    }
    if let Some(e) = &s.evento {
        let cv = format!("{c}/atvEvento");
        let x = n.grupo("atvEvento");
        x.add(
            "xNome",
            texto(&e.nome, &format!("{cv}/xNome"), Txt::max(255))?,
        );
        let ini = data(&e.inicio, &format!("{cv}/dtIni"))?;
        let fim = data(&e.fim, &format!("{cv}/dtFim"))?;
        if fim < ini {
            return Err(ErroNfse::novo(
                format!("{cv}/dtFim"),
                "termina antes de comecar",
            ));
        }
        x.add("dtIni", ini);
        x.add("dtFim", fim);
        if !e.cmun.trim().is_empty() {
            if leiaute != LEIAUTE_NT009 {
                return Err(ErroNfse::novo(
                    format!("{cv}/cMun"),
                    "o municipio do evento (cMun) so' existe no leiaute nt009",
                ));
            }
            // XSD prodrest 24/11/2026: atvEvento/cMun entre dtFim e o choice idAtvEvt/end
            x.add(
                "cMun",
                confere(
                    "TSCodMunIBGE",
                    &so_digitos(&e.cmun),
                    &format!("{cv}/cMun"),
                    "codigo IBGE de 7 digitos",
                )?,
            );
        }
        if !e.identificacao.is_empty() {
            x.add(
                "idAtvEvt",
                texto(&e.identificacao, &format!("{cv}/idAtvEvt"), Txt::max(30))?,
            );
        } else if let Some(en_) = &e.endereco {
            x.anexar(endereco_simples(en_, &format!("{cv}/end"))?);
        } else {
            return Err(ErroNfse::novo(
                cv,
                "identifique o evento: codigo ou endereco",
            ));
        }
    }
    if let Some(i) = &s.complemento {
        let ci = format!("{c}/infoCompl");
        let mut x = No::new("infoCompl");
        x.opc(
            "idDocTec",
            &texto(&i.id_doc_tecnico, &format!("{ci}/idDocTec"), Txt::opc(40))?,
        );
        x.opc(
            "docRef",
            &texto(&i.doc_referencia, &format!("{ci}/docRef"), Txt::opc(255))?,
        );
        x.opc(
            "xPed",
            &texto(&i.pedido, &format!("{ci}/xPed"), Txt::opc(60))?,
        );
        let mut itens = Vec::new();
        for t in i.itens_pedido.iter().filter(|t| !t.trim().is_empty()) {
            itens.push(texto(t, &format!("{ci}/gItemPed/xItemPed"), Txt::max(60))?);
        }
        if itens.len() > 99 {
            return Err(ErroNfse::novo(
                format!("{ci}/gItemPed"),
                "no maximo 99 itens de pedido",
            ));
        }
        if !itens.is_empty() {
            let g = x.grupo("gItemPed");
            for t in itens {
                g.add("xItemPed", t);
            }
        }
        x.opc(
            "xInfComp",
            &texto(&i.texto, &format!("{ci}/xInfComp"), Txt::opc(2000))?,
        );
        if !x.filhos.is_empty() {
            n.anexar(x);
        }
    }
    Ok(n)
}

pub(super) fn alnum_ascii(v: &str, max: usize) -> bool {
    !v.is_empty() && v.chars().count() <= max && v.chars().all(|c| c.is_ascii_alphanumeric())
}

pub(super) fn doc_deducao(d: &DocDeducao, c: &str) -> R<No> {
    let mut n = No::new("docDedRed");
    if !d.chave_nfse.is_empty() {
        n.add(
            "chNFSe",
            confere(
                "TSChaveNFSe",
                &so_alnum(&d.chave_nfse),
                &format!("{c}/chNFSe"),
                "",
            )?,
        );
    } else if !d.chave_nfe.is_empty() {
        n.add(
            "chNFe",
            confere(
                "TSChaveNFe",
                &so_alnum(&d.chave_nfe),
                &format!("{c}/chNFe"),
                "",
            )?,
        );
    } else if !d.nfse_mun_numero.is_empty() {
        let x = n.grupo("NFSeMun");
        x.add(
            "cMunNFSeMun",
            confere(
                "TSCodMunIBGE",
                &so_digitos(&d.nfse_mun_cmun),
                &format!("{c}/NFSeMun/cMunNFSeMun"),
                "",
            )?,
        );
        let num = so_digitos(&d.nfse_mun_numero);
        if num.is_empty() || num.len() > 15 {
            return Err(ErroNfse::novo(
                format!("{c}/NFSeMun/nNFSeMun"),
                "ate 15 digitos",
            ));
        }
        x.add("nNFSeMun", format!("{num:0>15}"));
        if !alnum_ascii(&d.nfse_mun_verif, 9) {
            return Err(ErroNfse::novo(
                format!("{c}/NFSeMun/cVerifNFSeMun"),
                "codigo de verificacao de 1 a 9 letras/digitos",
            ));
        }
        x.add("cVerifNFSeMun", d.nfse_mun_verif.clone());
    } else if !d.nfnfs_numero.is_empty() {
        let x = n.grupo("NFNFS");
        let ult = |s: String, k: usize| -> String {
            let p = format!("{s:0>k$}");
            p[p.len() - k..].to_string()
        };
        x.add("nNFS", ult(so_digitos(&d.nfnfs_numero), 7));
        x.add("modNFS", ult(so_digitos(&d.nfnfs_modelo), 15));
        if !alnum_ascii(&d.nfnfs_serie, 15) {
            return Err(ErroNfse::novo(
                format!("{c}/NFNFS/serieNFS"),
                "serie de 1 a 15 letras/digitos",
            ));
        }
        x.add("serieNFS", d.nfnfs_serie.clone());
    } else if !d.doc_fiscal.is_empty() {
        n.add(
            "nDocFisc",
            texto(&d.doc_fiscal, &format!("{c}/nDocFisc"), Txt::max(255))?,
        );
    } else if !d.doc.is_empty() {
        n.add("nDoc", texto(&d.doc, &format!("{c}/nDoc"), Txt::max(255))?);
    } else {
        return Err(ErroNfse::novo(c, "identifique o documento da deducao"));
    }
    n.add(
        "tpDedRed",
        en(
            d.tipo,
            &["1", "2", "3", "4", "5", "6", "7", "8", "9", "99"],
            &format!("{c}/tpDedRed"),
        )?,
    );
    n.opc(
        "xDescOutDed",
        &texto(
            &d.descricao_outras,
            &format!("{c}/xDescOutDed"),
            Txt::opc(150),
        )?,
    );
    n.add("dtEmiDoc", data(&d.emissao, &format!("{c}/dtEmiDoc"))?);
    n.add(
        "vDedutivelRedutivel",
        dinheiro(
            d.valor_dedutivel.as_str(),
            &format!("{c}/vDedutivelRedutivel"),
            15,
            true,
        )?,
    );
    n.add(
        "vDeducaoReducao",
        dinheiro(
            d.valor_deducao.as_str(),
            &format!("{c}/vDeducaoReducao"),
            15,
            true,
        )?,
    );
    if let Some(f) = &d.fornecedor {
        n.anexar(pessoa("fornec", f, &format!("{c}/fornec"))?);
    }
    Ok(n)
}

/// pAliq: TSDec1V2 (UM digito inteiro) e teto legal de 5% (CF art. 156 par. 3o I + LC 116/2003 art. 8o II; Anexo I
/// E0595) [lido 2026-10-03 em planalto.gov.br]. O minimo de 2% (LC 116 art. 8o-A, excecoes 7.02/7.05/16.01) NAO e'
/// conferido aqui (subitem, regime e beneficio municipal; o Anexo I admite 1,8% no E0621) -- fica com o SEFIN.
pub fn aliquota_iss(v: &str) -> R<String> {
    let campo = "infDPS/valores/trib/tribMun/pAliq";
    let s = dinheiro(v, campo, 1, true)?;
    if centavos(&s)? > 500 {
        return Err(ErroNfse::novo(campo, format!(
            "aliquota de ISS {s}% acima do teto de 5% (CF art. 156 par. 3o, I + LC 116/2003 art. 8o, II; regra E0595 do SEFIN)")));
    }
    Ok(s)
}

/// Campos de identificacao de documento comuns a DocReembolso (producao) e DocAjusteBC (nt009).
pub(super) struct RefDoc<'a> {
    dfe_tipo: Option<u8>,
    dfe_tipo_descricao: &'a str,
    dfe_chave: &'a str,
    outro_cmun: &'a str,
    outro_numero: &'a str,
    outro_descricao: &'a str,
    doc_numero: &'a str,
    doc_descricao: &'a str,
}

/// dFeNacional / docFiscalOutro / docOutro: gReeRepRes (producao) e docAjusteBC (nt009) usam o mesmo trio.
pub(super) fn doc_ref(x: &mut No, d: &RefDoc, cr: &str) -> R<()> {
    if !d.dfe_chave.is_empty() {
        let y = x.grupo("dFeNacional");
        y.add(
            "tipoChaveDFe",
            en_opt(
                d.dfe_tipo,
                &["1", "2", "3", "9"],
                &format!("{cr}/dFeNacional/tipoChaveDFe"),
            )?,
        );
        y.opc(
            "xTipoChaveDFe",
            &texto(
                d.dfe_tipo_descricao,
                &format!("{cr}/dFeNacional/xTipoChaveDFe"),
                Txt::opc(255),
            )?,
        );
        y.add(
            "chaveDFe",
            texto(
                d.dfe_chave,
                &format!("{cr}/dFeNacional/chaveDFe"),
                Txt::max(50),
            )?,
        );
    } else if !d.outro_numero.is_empty() {
        let y = x.grupo("docFiscalOutro");
        y.add(
            "cMunDocFiscal",
            confere(
                "TSCodMunIBGE",
                &so_digitos(d.outro_cmun),
                &format!("{cr}/cMunDocFiscal"),
                "",
            )?,
        );
        y.add(
            "nDocFiscal",
            texto(d.outro_numero, &format!("{cr}/nDocFiscal"), Txt::max(255))?,
        );
        y.add(
            "xDocFiscal",
            texto(
                d.outro_descricao,
                &format!("{cr}/xDocFiscal"),
                Txt::max(255),
            )?,
        );
    } else if !d.doc_numero.is_empty() {
        let y = x.grupo("docOutro");
        y.add(
            "nDoc",
            texto(d.doc_numero, &format!("{cr}/nDoc"), Txt::max(255))?,
        );
        y.add(
            "xDoc",
            texto(d.doc_descricao, &format!("{cr}/xDoc"), Txt::max(255))?,
        );
    } else {
        return Err(ErroNfse::novo(
            cr,
            "identifique o documento: chave DF-e, documento fiscal ou outro",
        ));
    }
    Ok(())
}

impl DocAjusteBC {
    pub(super) fn refdoc(&self) -> RefDoc<'_> {
        RefDoc {
            dfe_tipo: self.dfe_tipo,
            dfe_tipo_descricao: &self.dfe_tipo_descricao,
            dfe_chave: &self.dfe_chave,
            outro_cmun: &self.outro_cmun,
            outro_numero: &self.outro_numero,
            outro_descricao: &self.outro_descricao,
            doc_numero: &self.doc_numero,
            doc_descricao: &self.doc_descricao,
        }
    }
}

impl DocReembolso {
    pub(super) fn refdoc(&self) -> RefDoc<'_> {
        RefDoc {
            dfe_tipo: self.dfe_tipo,
            dfe_tipo_descricao: &self.dfe_tipo_descricao,
            dfe_chave: &self.dfe_chave,
            outro_cmun: &self.outro_cmun,
            outro_numero: &self.outro_numero,
            outro_descricao: &self.outro_descricao,
            doc_numero: &self.doc_numero,
            doc_descricao: &self.doc_descricao,
        }
    }
}

/// docAjusteBC (TCDocAjusteBC do XSD prodrest 20/10/2026), na ordem do XSD.
pub(super) fn doc_ajuste(d: &DocAjusteBC, c: &str) -> R<No> {
    let mut n = No::new("docAjusteBC");
    n.add(
        "tpAjusteBC",
        en(d.tipo.as_str(), TIPO_AJUSTE_BC, &format!("{c}/tpAjusteBC"))?,
    );
    n.opc(
        "xTpAjusteBC",
        &texto(
            &d.tipo_descricao,
            &format!("{c}/xTpAjusteBC"),
            Txt::opc(150),
        )?,
    );
    n.add(
        "vTotDoc",
        dinheiro(d.valor_total.as_str(), &format!("{c}/vTotDoc"), 15, true)?,
    );
    n.add(
        "vAjusteAplic",
        dinheiro(
            d.valor_aplicado.as_str(),
            &format!("{c}/vAjusteAplic"),
            15,
            true,
        )?,
    );
    if !d.emissao.trim().is_empty() {
        n.add("dtEmiDoc", data(&d.emissao, &format!("{c}/dtEmiDoc"))?);
    }
    if !d.competencia.trim().is_empty() {
        n.add(
            "dtCompDoc",
            data(&d.competencia, &format!("{c}/dtCompDoc"))?,
        );
    }
    doc_ref(&mut n, &d.refdoc(), c)?;
    if let Some(f) = &d.fornecedor {
        n.anexar(pessoa("fornec", f, &format!("{c}/fornec"))?);
    }
    Ok(n)
}

/// vAjusteBC (leiaute nt009, TCInfoAjusteBC do XSD prodrest 24/11/2026 `tiposComplexos_v1.01.xsd:1896-1926`):
/// percentual OU valor do ISSQN (choice opcional) E/OU documentos (docAjusteBC) E/OU o ajuste do comercio exterior
/// (vAjusteBCIBSCBSComExt, Release 2). O E0462 (AI:433) so' veda os tipos do ISSQN quando os dois vem juntos.
pub(super) fn ajuste_bc(dd: &Deducao, c: &str) -> R<No> {
    let cd = format!("{c}/vAjusteBC");
    if dd.percentual.is_some() && dd.valor.is_some() {
        return Err(ErroNfse::novo(
            cd,
            "informe percentual OU valor do ajuste do ISSQN, nao os dois",
        ));
    }
    if dd.percentual.is_none()
        && dd.valor.is_none()
        && dd.ajustes.is_empty()
        && dd.comercio_exterior.is_none()
    {
        return Err(ErroNfse::novo(
            cd,
            "informe o ajuste: percentual, valor, documentos ou o ajuste do comercio exterior",
        ));
    }
    let mut x = No::new("vAjusteBC");
    if let Some(p) = &dd.percentual {
        x.add(
            "pAjusteBCISSQN",
            dinheiro(p.as_str(), &format!("{cd}/pAjusteBCISSQN"), 3, true)?,
        );
    } else if let Some(p) = &dd.valor {
        x.add(
            "vAjusteBCISSQN",
            dinheiro(p.as_str(), &format!("{cd}/vAjusteBCISSQN"), 15, true)?,
        );
    }
    if !dd.ajustes.is_empty() {
        if dd.ajustes.len() > 1000 {
            return Err(ErroNfse::novo(
                format!("{cd}/documentos"),
                "no maximo 1000 documentos",
            ));
        }
        let lst = x.grupo("documentos");
        for (i, d) in dd.ajustes.iter().enumerate() {
            lst.anexar(doc_ajuste(d, &format!("{cd}/documentos/docAjusteBC[{i}]"))?);
        }
    }
    if let Some(aj) = &dd.comercio_exterior {
        // XSD prodrest 24/11/2026: TSDec15V2Neg, depois dos documentos
        let (s, _) = dinheiro_com_sinal(aj.as_str(), &format!("{cd}/vAjusteBCIBSCBSComExt"), 15)?;
        x.add("vAjusteBCIBSCBSComExt", s);
    }
    Ok(x)
}
