// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Leitura do XML da NF-e (modelo 55, leiaute 3.10 e 4.00) para a struct [`Nfe`]: o `nfeProc` / `procNFe` (nota com
//! o protocolo de autorizacao) e a `NFe` solta (sem protocolo).
//!
//! A leitura NAO confere a assinatura XML nem o `digVal` do protocolo: ela le o que o arquivo diz. Quem precisa de
//! prova de autenticidade confere a assinatura e consulta a SEFAZ pela chave.
//!
//! Campo ausente vira vazio ou `None`; campo presente fora do formato do leiaute e' erro, nunca zero nem texto cru.

use std::collections::BTreeMap;

use super::decimal::Decimal;
use super::erro::ErroLeitura;
use super::modelo::{
    Cobranca, Duplicata, Endereco, Folhas, FormaPagamento, Grupo, Ide, Imposto, InfAdic, Item, Nfe,
    Obs, Participante, Protocolo, Rastro, Total, Transporte, Volume,
};
use super::xml::{
    abrir, corta, data, data_hora, decimal, filho, filhos, inteiro, nome, t, tem_filhos, texto_de,
    texto_xml, todos, No, R,
};
use crate::chave_dfe;

/// Folhas que o leiaute tipa como texto (codigo) mesmo quando parecem numero: nao viram [`Decimal`].
const TEXTOS_FISCAIS: &[&str] = &[
    "orig",
    "CST",
    "CSOSN",
    "modBC",
    "modBCST",
    "UFST",
    "motDesICMS",
    "motDesICMSST",
    "motRedAdRem",
    "indDeduzDeson",
    "cBenefRBC",
    "clEnq",
    "CNPJProd",
    "cSelo",
    "qSelo",
    "cEnq",
    "cClassTrib",
    "indDoacao",
    "CSTReg",
    "cClassTribReg",
    "tpALCZFMCBS",
    "nProcSuframa",
    "cCredPres",
    "competApur",
    "tpCredPresIBSZFM",
    "CSTIS",
    "cClassTribIS",
    "uTrib",
    "cMunFG",
    "cListServ",
    "indISS",
    "cServico",
    "cMun",
    "cPais",
    "nProcesso",
    "indIncentivo",
    "indSomaPISST",
    "indSomaCOFINSST",
    "cRegTrib",
    "dCompet",
    "cSitTrib",
];

// ---------------------------------------------------------------- grupos genericos

/// Texto que tem cara de numero (digitos, `.` e `,`, com sinal opcional) mas nao e' um decimal do leiaute.
fn numero_mal_escrito(s: &str) -> bool {
    let t = s.strip_prefix(['-', '+']).unwrap_or(s);
    !t.is_empty()
        && t.bytes()
            .all(|c| c.is_ascii_digit() || c == b'.' || c == b',')
}

/// `k`, ou `k[2]`, `k[3]`... se ja existe (tag repetida no grupo).
fn chave_livre<V>(d: &BTreeMap<String, V>, k: &str) -> String {
    if !d.contains_key(k) {
        return k.to_string();
    }
    let mut n = 2;
    while d.contains_key(&format!("{k}[{n}]")) {
        n += 1;
    }
    format!("{k}[{n}]")
}

fn novo_grupo(nome: &str) -> Grupo {
    Grupo {
        nome: nome.to_string(),
        ..Grupo::default()
    }
}

fn guardar_folha(g: &mut Grupo, caminho: &str, tag: &str, s: String) -> R<()> {
    if !TEXTOS_FISCAIS.contains(&tag) {
        if let Some(v) = Decimal::ler(&s) {
            let k = chave_livre(&g.valores, caminho);
            g.valores.insert(k, v);
            return Ok(());
        }
        if numero_mal_escrito(&s) && !s.bytes().all(|c| c.is_ascii_digit()) {
            return Err(ErroLeitura::Campo {
                tipo: super::erro::TipoCampo::Decimal,
                onde: format!("{}/{caminho}", g.nome),
                lido: corta(&s, 30),
            });
        }
    }
    let k = chave_livre(&g.textos, caminho);
    g.textos.insert(k, s);
    Ok(())
}

fn achatar(n: No, g: &mut Grupo, prefixo: &str) -> R<()> {
    for c in filhos(Some(n), None) {
        let caminho = format!("{prefixo}{}", nome(&c));
        if tem_filhos(c) {
            achatar(c, g, &format!("{caminho}/"))?;
        } else {
            guardar_folha(g, &caminho, nome(&c), texto_de(c))?;
        }
    }
    Ok(())
}

/// O grupo `n` inteiro, com as folhas por caminho.
fn grupo(n: Option<No>) -> R<Option<Grupo>> {
    let Some(n) = n else { return Ok(None) };
    let mut g = novo_grupo(nome(&n));
    achatar(n, &mut g, "")?;
    Ok(Some(g))
}

fn folhas_em(n: No, prefixo: &str, d: &mut Folhas) {
    for c in filhos(Some(n), None) {
        let caminho = format!("{prefixo}{}", nome(&c));
        if tem_filhos(c) {
            folhas_em(c, &format!("{caminho}/"), d);
        } else {
            let k = chave_livre(d, &caminho);
            d.insert(k, texto_de(c));
        }
    }
}

/// As folhas do grupo `n` por caminho (vazio se `n` nao existe).
fn folhas(n: Option<No>) -> Folhas {
    let mut d = Folhas::new();
    if let Some(n) = n {
        folhas_em(n, "", &mut d);
    }
    d
}

/// As folhas de cada filho `so` de `n`.
fn folhas_de_cada(n: No, so: &str) -> Vec<Folhas> {
    todos(n, so).into_iter().map(|c| folhas(Some(c))).collect()
}

/// Grupo escolhido por tag (ICMS, PIS, COFINS): o conteiner tem um unico filho, o grupo da situacao tributaria.
fn grupo_unico(conteiner: Option<No>) -> R<Option<Grupo>> {
    let Some(c) = conteiner else { return Ok(None) };
    match filhos(Some(c), None).into_iter().next() {
        Some(g) => grupo(Some(g)),
        None => Ok(Some(novo_grupo(nome(&c)))),
    }
}

/// `IPI`: o `IPITrib` ou o `IPINT` achatado junto das folhas do proprio `IPI`.
fn grupo_ipi(ipi: Option<No>) -> R<Option<Grupo>> {
    let Some(ipi) = ipi else { return Ok(None) };
    let sub = filhos(Some(ipi), None).into_iter().find(|c| tem_filhos(*c));
    let mut g = novo_grupo(sub.as_ref().map_or("IPI", nome));
    for c in filhos(Some(ipi), None) {
        if tem_filhos(c) {
            achatar(c, &mut g, "")?;
        } else {
            guardar_folha(&mut g, nome(&c), nome(&c), texto_de(c))?;
        }
    }
    Ok(Some(g))
}

// ---------------------------------------------------------------- partes

/// (documento, tipo) do primeiro de `CNPJ`, `CPF` ou `idEstrangeiro`.
fn documento(n: Option<No>) -> (String, String) {
    for (tag, tipo) in [
        ("CNPJ", "cnpj"),
        ("CPF", "cpf"),
        ("idEstrangeiro", "estrangeiro"),
    ] {
        if let Some(c) = filho(n, tag) {
            let v = texto_de(c);
            let v = if tipo == "estrangeiro" {
                v
            } else {
                v.to_uppercase()
                    .chars()
                    .filter(|ch| ch.is_ascii_digit() || ch.is_ascii_uppercase())
                    .collect()
            };
            return (v, tipo.to_string());
        }
    }
    (String::new(), String::new())
}

fn endereco(n: Option<No>) -> Option<Endereco> {
    let n = n?;
    let e = |x| t(Some(n), x);
    Some(Endereco {
        logradouro: e("xLgr"),
        numero: e("nro"),
        complemento: e("xCpl"),
        bairro: e("xBairro"),
        cmun: e("cMun"),
        municipio: e("xMun"),
        uf: e("UF"),
        cep: e("CEP"),
        cpais: e("cPais"),
        pais: e("xPais"),
        fone: e("fone"),
    })
}

/// `ender`: nome do filho com o endereco; `None` = o endereco esta nos filhos do proprio `n` (retirada e entrega).
fn participante(n: Option<No>, ender: Option<&str>) -> Option<Participante> {
    let n = n?;
    let (documento, tipo_documento) = documento(Some(n));
    let e = |x| t(Some(n), x);
    Some(Participante {
        documento,
        tipo_documento,
        nome: e("xNome"),
        fantasia: e("xFant"),
        ie: e("IE"),
        iest: e("IEST"),
        im: e("IM"),
        cnae: e("CNAE"),
        crt: e("CRT"),
        ind_ie_dest: e("indIEDest"),
        isuf: e("ISUF"),
        email: e("email"),
        endereco: endereco(match ender {
            Some(x) => filho(Some(n), x),
            None => Some(n),
        }),
    })
}

fn ler_ide(n: No) -> R<Ide> {
    let o = Some(n);
    let (Some(serie), Some(numero)) = (inteiro(o, "serie")?, inteiro(o, "nNF")?) else {
        return Err(ErroLeitura::IdeIncompleto);
    };
    let e = |x| t(o, x);
    Ok(Ide {
        cuf: e("cUF"),
        cnf: e("cNF"),
        natureza_operacao: e("natOp"),
        modelo: e("mod"),
        serie,
        numero,
        emissao: data_hora(o, "dhEmi")?,
        saida_entrada: data_hora(o, "dhSaiEnt")?,
        tipo_nf: e("tpNF"),
        destino: e("idDest"),
        cmun_fg: e("cMunFG"),
        cmun_fg_ibs: e("cMunFGIBS"),
        tipo_impressao: e("tpImp"),
        tipo_emissao: e("tpEmis"),
        cdv: e("cDV"),
        ambiente: e("tpAmb"),
        finalidade: e("finNFe"),
        tp_nf_debito: e("tpNFDebito"),
        tp_nf_credito: e("tpNFCredito"),
        consumidor_final: e("indFinal"),
        presenca: e("indPres"),
        intermediador: e("indIntermed"),
        processo_emissao: e("procEmi"),
        versao_processo: e("verProc"),
        contingencia_em: data_hora(o, "dhCont")?,
        contingencia_justificativa: e("xJust"),
        previsao_entrega: data(o, "dPrevEntrega")?,
        referencias: folhas_de_cada(n, "NFref"),
        compra_governamental: folhas(filho(o, "gCompraGov")),
        pagamento_antecipado: filhos(filho(o, "gPagAntecipado"), Some("refNFe"))
            .into_iter()
            .map(texto_de)
            .collect(),
    })
}

// ---------------------------------------------------------------- itens

fn ler_imposto(n: Option<No>) -> R<Imposto> {
    if n.is_none() {
        return Ok(Imposto::default());
    }
    Ok(Imposto {
        total_tributos: decimal(n, "vTotTrib")?,
        icms: grupo_unico(filho(n, "ICMS"))?,
        icms_uf_dest: grupo(filho(n, "ICMSUFDest"))?,
        ipi: grupo_ipi(filho(n, "IPI"))?,
        ii: grupo(filho(n, "II"))?,
        pis: grupo_unico(filho(n, "PIS"))?,
        pis_st: grupo(filho(n, "PISST"))?,
        cofins: grupo_unico(filho(n, "COFINS"))?,
        cofins_st: grupo(filho(n, "COFINSST"))?,
        issqn: grupo(filho(n, "ISSQN"))?,
        seletivo: grupo(filho(n, "IS"))?,
        ibscbs: grupo(filho(n, "IBSCBS"))?,
    })
}

fn ler_rastros(p: No) -> R<Vec<Rastro>> {
    todos(p, "rastro")
        .into_iter()
        .map(|r| {
            let ro = Some(r);
            Ok(Rastro {
                lote: t(ro, "nLote"),
                quantidade: decimal(ro, "qLote")?,
                fabricacao: data(ro, "dFab")?,
                validade: data(ro, "dVal")?,
                agregacao: t(ro, "cAgreg"),
            })
        })
        .collect()
}

fn ler_item(det: No, posicao: u64) -> R<Item> {
    let Some(p) = filho(Some(det), "prod") else {
        return Err(ErroLeitura::ItemSemProd(posicao));
    };
    let o = Some(p);
    let e = |x| t(o, x);
    let n = det.attribute("nItem").unwrap_or("");
    let n_item = if !n.is_empty() && n.bytes().all(|c| c.is_ascii_digit()) {
        n.parse().unwrap_or(posicao)
    } else {
        posicao
    };
    Ok(Item {
        n_item,
        codigo: e("cProd"),
        gtin: e("cEAN"),
        codigo_barras: e("cBarra"),
        descricao: e("xProd"),
        ncm: e("NCM"),
        nve: todos(p, "NVE").into_iter().map(texto_de).collect(),
        cest: e("CEST"),
        escala_relevante: e("indEscala"),
        cnpj_fabricante: e("CNPJFab"),
        beneficio_fiscal: e("cBenef"),
        credito_presumido: folhas_de_cada(p, "gCred"),
        extipi: e("EXTIPI"),
        cfop: e("CFOP"),
        unidade: e("uCom"),
        quantidade: decimal(o, "qCom")?,
        valor_unitario: decimal(o, "vUnCom")?,
        valor_produto: decimal(o, "vProd")?,
        gtin_tributavel: e("cEANTrib"),
        codigo_barras_tributavel: e("cBarraTrib"),
        unidade_tributavel: e("uTrib"),
        quantidade_tributavel: decimal(o, "qTrib")?,
        valor_unitario_tributavel: decimal(o, "vUnTrib")?,
        frete: decimal(o, "vFrete")?,
        seguro: decimal(o, "vSeg")?,
        desconto: decimal(o, "vDesc")?,
        outros: decimal(o, "vOutro")?,
        compoe_total: e("indTot"),
        pedido: e("xPed"),
        item_pedido: e("nItemPed"),
        fci: e("nFCI"),
        recopi: e("nRECOPI"),
        rastros: ler_rastros(p)?,
        importacao: folhas_de_cada(p, "DI"),
        exportacao: folhas_de_cada(p, "detExport"),
        veiculo: folhas(filho(o, "veicProd")),
        medicamentos: folhas_de_cada(p, "med"),
        armas: folhas_de_cada(p, "arma"),
        combustivel: folhas(filho(o, "comb")),
        imposto: ler_imposto(filho(Some(det), "imposto"))?,
        devolucao: folhas(filho(Some(det), "impostoDevol")),
        informacao_adicional: t(Some(det), "infAdProd"),
        observacoes: folhas(filho(Some(det), "obsItem")),
        valor_item: decimal(Some(det), "vItem")?,
        dfe_referenciado: folhas(filho(Some(det), "DFeReferenciado")),
    })
}

// ---------------------------------------------------------------- total, transporte, cobranca, pagamento

fn ler_total(n: Option<No>) -> R<Total> {
    Ok(Total {
        icms_tot: grupo(filho(n, "ICMSTot"))?,
        issqn_tot: grupo(filho(n, "ISSQNtot"))?,
        retencoes: grupo(filho(n, "retTrib"))?,
        is_tot: grupo(filho(n, "ISTot"))?,
        ibscbs_tot: grupo(filho(n, "IBSCBSTot"))?,
        valor_total_com_tributos: decimal(n, "vNFTot")?,
    })
}

fn ler_transportadora(x: No) -> Participante {
    let (documento, tipo_documento) = documento(Some(x));
    Participante {
        documento,
        tipo_documento,
        nome: t(Some(x), "xNome"),
        ie: t(Some(x), "IE"),
        endereco: Some(Endereco {
            logradouro: t(Some(x), "xEnder"),
            municipio: t(Some(x), "xMun"),
            uf: t(Some(x), "UF"),
            ..Endereco::default()
        }),
        ..Participante::default()
    }
}

fn ler_volumes(tr: No) -> R<Vec<Volume>> {
    todos(tr, "vol")
        .into_iter()
        .map(|v| {
            let vo = Some(v);
            Ok(Volume {
                quantidade: inteiro(vo, "qVol")?,
                especie: t(vo, "esp"),
                marca: t(vo, "marca"),
                numeracao: t(vo, "nVol"),
                peso_liquido: decimal(vo, "pesoL")?,
                peso_bruto: decimal(vo, "pesoB")?,
                lacres: todos(v, "lacres")
                    .into_iter()
                    .map(|l| t(Some(l), "nLacre"))
                    .collect(),
            })
        })
        .collect()
}

fn ler_transporte(n: Option<No>) -> R<Option<Transporte>> {
    let Some(tr) = n else { return Ok(None) };
    Ok(Some(Transporte {
        modalidade_frete: t(n, "modFrete"),
        transportadora: filho(n, "transporta").map(ler_transportadora),
        retencao_icms: folhas(filho(n, "retTransp")),
        veiculo: folhas(filho(n, "veicTransp")),
        reboques: folhas_de_cada(tr, "reboque"),
        vagao: t(n, "vagao"),
        balsa: t(n, "balsa"),
        volumes: ler_volumes(tr)?,
    }))
}

fn ler_cobranca(n: Option<No>) -> R<Option<Cobranca>> {
    let Some(c) = n else { return Ok(None) };
    let fat = filho(n, "fat");
    let duplicatas = todos(c, "dup")
        .into_iter()
        .map(|d| {
            Ok(Duplicata {
                numero: t(Some(d), "nDup"),
                vencimento: data(Some(d), "dVenc")?,
                valor: decimal(Some(d), "vDup")?,
            })
        })
        .collect::<R<Vec<_>>>()?;
    Ok(Some(Cobranca {
        fatura_numero: t(fat, "nFat"),
        fatura_original: decimal(fat, "vOrig")?,
        fatura_desconto: decimal(fat, "vDesc")?,
        fatura_liquido: decimal(fat, "vLiq")?,
        duplicatas,
    }))
}

fn ler_pagamentos(n: Option<No>) -> R<Vec<FormaPagamento>> {
    filhos(n, Some("detPag"))
        .into_iter()
        .map(|d| {
            let o = Some(d);
            Ok(FormaPagamento {
                indicador: t(o, "indPag"),
                meio: t(o, "tPag"),
                descricao: t(o, "xPag"),
                valor: decimal(o, "vPag")?,
                data: data(o, "dPag")?,
                cnpj_pagamento: t(o, "CNPJPag"),
                uf_pagamento: t(o, "UFPag"),
                cartao: folhas(filho(o, "card")),
            })
        })
        .collect()
}

fn observacoes(n: No, so: &str) -> Vec<Obs> {
    todos(n, so)
        .into_iter()
        .map(|o| Obs {
            campo: o.attribute("xCampo").unwrap_or("").to_string(),
            texto: t(Some(o), "xTexto"),
        })
        .collect()
}

fn ler_inf_adic(n: Option<No>) -> Option<InfAdic> {
    let a = n?;
    Some(InfAdic {
        fisco: t(n, "infAdFisco"),
        complementar: t(n, "infCpl"),
        obs_contribuinte: observacoes(a, "obsCont"),
        obs_fisco: observacoes(a, "obsFisco"),
        processos: folhas_de_cada(a, "procRef"),
    })
}

fn ler_protocolo(prot: Option<No>) -> R<Option<Protocolo>> {
    let inf = filho(prot, "infProt");
    if inf.is_none() {
        return Ok(None);
    }
    Ok(Some(Protocolo {
        ambiente: t(inf, "tpAmb"),
        versao_aplicativo: t(inf, "verAplic"),
        chave: chave_dfe::limpar(&t(inf, "chNFe")),
        recebido_em: data_hora(inf, "dhRecbto")?,
        numero: t(inf, "nProt"),
        digest: t(inf, "digVal"),
        cstat: t(inf, "cStat"),
        motivo: t(inf, "xMotivo"),
    }))
}

// ---------------------------------------------------------------- a nota

/// `3.10` e `4.00` sao os leiautes que a leitura cobre.
fn leiaute_suportado(versao: &str) -> bool {
    matches!(versao, "3.10" | "4.00")
}

fn ler_nfe(nfe: No, prot: Option<No>) -> R<Nfe> {
    let Some(inf) = filho(Some(nfe), "infNFe") else {
        return Err(ErroLeitura::GrupoAusente("infNFe"));
    };
    let (Some(ident), Some(versao)) = (
        inf.attribute("Id").filter(|s| !s.is_empty()),
        inf.attribute("versao").filter(|s| !s.is_empty()),
    ) else {
        return Err(ErroLeitura::SemIdOuVersao);
    };
    if !leiaute_suportado(versao) {
        return Err(ErroLeitura::Leiaute(corta(versao, 8)));
    }
    let Some(ide) = filho(Some(inf), "ide") else {
        return Err(ErroLeitura::GrupoAusente("ide"));
    };
    let modelo = t(Some(ide), "mod");
    if modelo != "55" {
        let m = corta(&modelo, 4);
        return Err(ErroLeitura::Modelo(if m.is_empty() {
            "vazio".into()
        } else {
            m
        }));
    }
    let Some(emit) = filho(Some(inf), "emit") else {
        return Err(ErroLeitura::GrupoAusente("emit"));
    };
    let i = Some(inf);
    let chave = chave_dfe::limpar(ident);
    let pag = filho(i, "pag");
    let supl = filho(Some(nfe), "infNFeSupl");
    let itens = todos(inf, "det")
        .into_iter()
        .enumerate()
        .map(|(k, d)| ler_item(d, k as u64 + 1))
        .collect::<R<Vec<_>>>()?;
    Ok(Nfe {
        versao: versao.to_string(),
        chave_erro: chave_dfe::validar(&chave, true).err(),
        chave,
        modelo,
        ide: ler_ide(ide)?,
        emit: participante(Some(emit), Some("enderEmit")).unwrap_or_default(),
        dest: participante(filho(i, "dest"), Some("enderDest")),
        retirada: participante(filho(i, "retirada"), None),
        entrega: participante(filho(i, "entrega"), None),
        autorizados_xml: todos(inf, "autXML")
            .into_iter()
            .map(|a| documento(Some(a)).0)
            .collect(),
        itens,
        total: ler_total(filho(i, "total"))?,
        transporte: ler_transporte(filho(i, "transp"))?,
        cobranca: ler_cobranca(filho(i, "cobr"))?,
        pagamentos: ler_pagamentos(pag)?,
        troco: decimal(pag, "vTroco")?,
        intermediador: folhas(filho(i, "infIntermed")),
        inf_adic: ler_inf_adic(filho(i, "infAdic")),
        exportacao: folhas(filho(i, "exporta")),
        compra: folhas(filho(i, "compra")),
        cana: folhas(filho(i, "cana")),
        responsavel_tecnico: folhas(filho(i, "infRespTec")),
        agropecuario: folhas(filho(i, "agropecuario")),
        qr_code: t(supl, "qrCode"),
        url_chave: t(supl, "urlChave"),
        protocolo: ler_protocolo(prot)?,
    })
}

/// Le o XML de uma NF-e modelo 55: `nfeProc` ou `procNFe` (com o `protNFe`) ou `NFe` solta.
///
/// `xml` sao os bytes do arquivo; UTF-8, UTF-16 com BOM, ISO-8859-1 e Windows-1252 (declarado) sao aceitos.
pub fn ler(xml: &[u8]) -> Result<Nfe, ErroLeitura> {
    let texto = texto_xml(xml)?;
    let d = abrir(&texto)?;
    let raiz = d.root_element();
    match nome(&raiz) {
        "nfeProc" | "procNFe" => {
            let Some(nfe) = filho(Some(raiz), "NFe") else {
                return Err(ErroLeitura::GrupoAusente("NFe"));
            };
            ler_nfe(nfe, filho(Some(raiz), "protNFe"))
        }
        "NFe" => ler_nfe(raiz, None),
        outro => Err(ErroLeitura::OutroDocumento {
            raiz: corta(outro, 40),
        }),
    }
}
