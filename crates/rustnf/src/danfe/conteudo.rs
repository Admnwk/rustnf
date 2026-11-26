// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O que vai no papel: o conteudo da NF-e lida, ja formatado em texto, e a decisao de cada situacao (marca d'agua,
//! campos 1 e 2 conforme a forma de emissao, informacoes complementares, colunas dos itens).
//!
//! Regra de ouro do Anexo II, item 3.1: o DANFE so' imprime o que consta do arquivo da NF-e.

use serde_json::{json, Value};

use super::barras;
use super::formato::{brl, casas_unitario, cep, documento, fone, local, num9, qtd};
use super::{ErroDanfe, Opcoes};
use crate::chave_dfe;
use crate::nfe::modelo::{Endereco, Item, Nfe, Participante};
use crate::nfe::montagem::{na_uf, uf_codigo};
use crate::nfe::Situacao;

/// Consulta de autenticidade impressa no campo 1 (Anexo II, item 3.9.1).
const CONSULTA: &str =
    "Consulta de autenticidade no portal nacional da NF-e www.nfe.fazenda.gov.br/portal \
                        ou no site da Sefaz Autorizadora";
/// Consulta impressa no campo 1 da contingencia EPEC (Anexo II, item 3.9.3).
const CONSULTA_EPEC: &str =
    "Consulta de autenticidade no portal da NF-e www.nfe.fazenda.gov.br/portal";
/// Expressao que o DANFE da contingencia com formulario de seguranca leva no corpo (Anexo III, item 2.1.2).
const FRASE_FS: &str = "DANFE em Contingência - impresso em decorrência de problemas técnicos";
/// Expressao do DANFE da NF-e que consta do EPEC (Anexo III, item 2.1.4; Visao Geral, item 3.3.4).
const FRASE_EPEC: &str =
    "DANFE impresso em contingência - EPEC regularmente recebida pela Receita Federal do Brasil";

pub(super) fn uf_emitente(nfe: &Nfe) -> &str {
    nfe.emit.endereco.as_ref().map_or("", |e| e.uf.as_str())
}

/// Recusas do papel: a NFC-e (modelo 65 tem o DANFE NFC-e), chave que nao confere, nota rejeitada, protocolo de
/// outra chave, tpEmis 9 (so' da NFC-e) e EPEC sem o protocolo do evento.
pub(super) fn conferir(nfe: &Nfe, op: &Opcoes) -> Result<(), ErroDanfe> {
    if nfe.modelo != "55" {
        return Err(ErroDanfe::NaoEModelo55(nfe.modelo.clone()));
    }
    if let Some(e) = &nfe.chave_erro {
        return Err(ErroDanfe::Chave(e.to_string()));
    }
    match nfe.situacao() {
        Situacao::Rejeitada => return Err(ErroDanfe::Rejeitada),
        Situacao::ProtocoloDivergente => return Err(ErroDanfe::ProtocoloDeOutraChave),
        _ => {}
    }
    if nfe.ide.emissao.is_none() {
        return Err(ErroDanfe::SemEmissao);
    }
    match nfe.ide.tipo_emissao.as_str() {
        "9" => Err(ErroDanfe::EmissaoOffline),
        "4" => match op.protocolo_epec.map(str::trim) {
            None | Some("") => Err(ErroDanfe::EpecSemProtocolo),
            Some(p) if !protocolo_epec_valido(p) => Err(ErroDanfe::EpecProtocoloInvalido),
            Some(_) => Ok(()),
        },
        _ => Ok(()),
    }
}

/// `NNNNNNNNNNNNNNN dd/mm/aaaa hh:mm:ss`: o numero do protocolo e a data e a hora do registro do EPEC (Anexo II, 3.9.3).
fn protocolo_epec_valido(p: &str) -> bool {
    let Some((numero, quando)) = p.split_once(' ') else {
        return false;
    };
    let q = quando.as_bytes();
    !numero.is_empty()
        && numero.bytes().all(|c| c.is_ascii_digit())
        && q.len() == 19
        && q.iter().enumerate().all(|(i, c)| match i {
            2 | 5 => *c == b'/',
            10 => *c == b' ',
            13 | 16 => *c == b':',
            _ => c.is_ascii_digit(),
        })
}

/// Marca d'agua: (texto, corpo em pt) por linha; vazia = nota valida, sem marca.
pub(super) fn marca(nfe: &Nfe, op: &Opcoes) -> Vec<(&'static str, f64)> {
    let situacao = nfe.situacao();
    if op.cancelada || situacao == Situacao::Cancelada {
        vec![("NOTA CANCELADA", 48.0)]
    } else if situacao == Situacao::Denegada {
        vec![("NF-e USO DENEGADO", 40.0), ("SEM VALOR FISCAL", 30.0)]
    } else if nfe.homologacao() {
        vec![
            ("SEM VALOR FISCAL", 48.0),
            ("AMBIENTE DE HOMOLOGAÇÃO", 30.0),
        ]
    } else if situacao == Situacao::SemProtocolo
        && !matches!(nfe.ide.tipo_emissao.as_str(), "2" | "4" | "5")
    {
        vec![
            ("SEM VALOR FISCAL", 48.0),
            ("FALTA PROTOCOLO DE AUTORIZAÇÃO", 26.0),
        ]
    } else if nfe.ide.tipo_emissao != "1" {
        vec![("DANFE EM CONTINGÊNCIA", 40.0)]
    } else {
        vec![]
    }
}

fn endereco_linha(e: &Endereco) -> String {
    let mut s = e.logradouro.clone();
    if !e.numero.is_empty() {
        s.push_str(&format!(", {}", e.numero));
    }
    if !e.complemento.is_empty() {
        s.push_str(&format!(" - {}", e.complemento));
    }
    s.trim().to_string()
}

fn participante(p: &Participante) -> Value {
    let e = p.endereco.as_ref();
    json!({
        "nome": p.nome,
        "doc": documento(&p.tipo_documento, &p.documento),
        "ie": p.ie,
        "endereco": e.map_or(String::new(), endereco_linha),
        "bairro": e.map_or("", |e| e.bairro.as_str()),
        "cep": e.map_or(String::new(), |e| cep(&e.cep)),
        "municipio": e.map_or("", |e| e.municipio.as_str()),
        "uf": e.map_or("", |e| e.uf.as_str()),
        "fone": e.map_or(String::new(), |e| fone(&e.fone)),
    })
}

/// Identificacao do emitente (Anexo II, item 3.1.3): nome, endereco completo e telefone.
fn emitente(nfe: &Nfe) -> Value {
    let e = &nfe.emit;
    let linhas: Vec<String> = e
        .endereco
        .as_ref()
        .map(|en| {
            vec![
                endereco_linha(en),
                format!("{} - CEP {}", en.bairro, cep(&en.cep))
                    .trim_matches(|c| c == ' ' || c == '-')
                    .to_string(),
                format!("{} - {}", en.municipio, en.uf),
                if en.fone.is_empty() {
                    String::new()
                } else {
                    format!("Fone: {}", fone(&en.fone))
                },
            ]
        })
        .unwrap_or_default()
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect();
    let tipo = if e.tipo_documento.is_empty() {
        "cnpj"
    } else {
        &e.tipo_documento
    };
    json!({
        "nome": e.nome,
        "linhas": linhas.join("\n"),
        "ie": e.ie,
        "iest": e.iest,
        "cnpj": documento(tipo, &e.documento),
        "im": e.im,
    })
}

/// Dados da NF-e em contingencia com formulario de seguranca: 36 posicoes (Anexo II, item 3.9.2).
///
/// `cUF` e documento do destinatario (99 e zeros no exterior), `vNF` em centavos, `ICMSp` e `ICMSs` (1 = ha destaque,
/// 2 = nao ha), `DD` o dia da emissao no fuso da UF do emitente e o DV pelo modulo 11 da chave.
pub(super) fn dados_contingencia(nfe: &Nfe) -> Result<String, ErroDanfe> {
    let d = nfe.dest.as_ref();
    let uf = d
        .and_then(|d| d.endereco.as_ref())
        .map_or(String::new(), |e| e.uf.to_uppercase());
    let exterior = nfe.ide.destino == "3"
        || uf == "EX"
        || d.is_some_and(|d| d.tipo_documento == "estrangeiro");
    let (cuf, documento) = if exterior {
        ("99".to_string(), "0".repeat(14))
    } else {
        let Some(c) = uf_codigo(&uf) else {
            return Err(ErroDanfe::ContingenciaSemUf);
        };
        let s: String = d
            .map_or("", |d| d.documento.as_str())
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .map(|c| c.to_ascii_uppercase())
            .collect();
        (format!("{c:02}"), format!("{s:0>14}"))
    };
    let vnf: String = brl(Some(nfe.total.v("vNF")), 2)
        .chars()
        .filter(char::is_ascii_digit)
        .collect();
    let Some(emissao) = nfe.ide.emissao.as_ref() else {
        return Err(ErroDanfe::SemEmissao);
    };
    let dia = na_uf(&emissao.0, uf_emitente(nfe)).format("%d").to_string();
    let destaque = |tag: &str| {
        let v = nfe.total.v(tag).to_string();
        if v.bytes().any(|c| (b'1'..=b'9').contains(&c)) && !v.starts_with('-') {
            "1"
        } else {
            "2"
        }
    };
    let base = format!(
        "{cuf}{}{documento}{vnf:0>14}{}{}{dia}",
        nfe.ide.tipo_emissao,
        destaque("vICMS"),
        destaque("vST"),
    );
    if base.len() != 35 {
        return Err(ErroDanfe::ContingenciaTamanho(base.len()));
    }
    Ok(format!("{base}{}", dv_mod11(&base)))
}

/// Digito verificador pelo modulo 11 da chave de acesso: pesos de 2 a 9 da direita para a esquerda; resto 0 ou 1 = 0.
fn dv_mod11(base: &str) -> u8 {
    let soma: usize = base
        .bytes()
        .rev()
        .enumerate()
        .map(|(i, c)| usize::from(c - b'0') * (2 + i % 8))
        .sum();
    let resto = soma % 11;
    if resto < 2 {
        0
    } else {
        u8::try_from(11 - resto).unwrap_or(0)
    }
}

/// Campo 1 (consulta ou codigo de barras dos dados) e campo 2 (protocolo ou numeracao dos dados) do quadro da chave,
/// conforme a forma de emissao (Anexo II, itens 3.9.1 a 3.9.3). Devolve `(campo1, campo2)`.
pub(super) fn campos_1_e_2(nfe: &Nfe, op: &Opcoes) -> Result<(Value, Value), ErroDanfe> {
    let te = nfe.ide.tipo_emissao.as_str();
    let uf = uf_emitente(nfe);
    if te == "2" || te == "5" {
        let d = dados_contingencia(nfe)?;
        let grupos: Vec<&str> = (0..36).step_by(4).map(|i| &d[i..i + 4]).collect();
        let padrao = barras::padrao(&d).map_err(|e| ErroDanfe::Barras(e.to_string()))?;
        return Ok((
            json!({"barras": padrao}),
            json!(["Dados da NF-e", grupos.join(" ")]),
        ));
    }
    if te == "4" {
        return Ok((
            json!({"texto": CONSULTA_EPEC}),
            json!([
                "Protocolo de autorização do EPEC",
                op.protocolo_epec.map_or("", str::trim)
            ]),
        ));
    }
    let protocolo = nfe.protocolo.as_ref().map_or(String::new(), |p| {
        format!("{} {}", p.numero, local(p.recebido_em.as_ref(), uf))
            .trim()
            .to_string()
    });
    Ok((
        json!({"texto": CONSULTA}),
        json!(["Protocolo de autorização de uso", protocolo]),
    ))
}

/// Informacoes complementares: o que o XML traz em `infAdFisco` e `infCpl` (obrigatorio), mais `obsCont` (facultativo,
/// vai junto) (Anexo II, item 3.1.8). Na contingencia, a expressao que o Anexo III manda imprimir e o motivo e a hora
/// da entrada em contingencia (`xJust` e `dhCont`).
pub(super) fn complementares(nfe: &Nfe, op: &Opcoes) -> String {
    let mut partes = Vec::new();
    let ide = &nfe.ide;
    let uf = uf_emitente(nfe);
    match ide.tipo_emissao.as_str() {
        "2" | "5" => partes.push(FRASE_FS.to_string()),
        "4" => partes.push(FRASE_EPEC.to_string()),
        _ => {}
    }
    if ide.tipo_emissao != "1"
        && (ide.contingencia_em.is_some() || !ide.contingencia_justificativa.is_empty())
    {
        let mut s = String::from("Contingência");
        if ide.contingencia_em.is_some() {
            s.push_str(&format!(
                " desde {}",
                local(ide.contingencia_em.as_ref(), uf)
            ));
        }
        if !ide.contingencia_justificativa.is_empty() {
            s.push_str(&format!(": {}", ide.contingencia_justificativa));
        }
        partes.push(s);
    }
    if op.cancelada {
        partes.push("NOTA CANCELADA".to_string());
    }
    if let Some(ia) = &nfe.inf_adic {
        if !ia.fisco.is_empty() {
            partes.push(format!("Inf. Fisco: {}", ia.fisco));
        }
        if !ia.complementar.is_empty() {
            partes.push(format!("Inf. Contribuinte: {}", ia.complementar));
        }
        for o in &ia.obs_contribuinte {
            partes.push(format!("{}: {}", o.campo, o.texto));
        }
    }
    partes.join("\n")
}

/// Colunas do quadro dos produtos: (titulo, largura em mm, alinhamento `E`/`C`/`D`). A ordem e as colunas que nao se
/// suprimem sao as do item 3.1.7; a descricao leva o que sobra da largura util.
pub(super) fn colunas(nfe: &Nfe, largura_util: f64) -> Value {
    let com_icms: Vec<&Item> = nfe
        .itens
        .iter()
        .filter(|i| i.imposto.icms.is_some())
        .collect();
    let simples = !com_icms.is_empty()
        && com_icms.iter().all(|i| {
            i.imposto
                .icms
                .as_ref()
                .is_some_and(|g| g.texto("CST").is_empty())
        });
    let fixas: [(&str, f64, &str); 14] = [
        ("CÓDIGO PRODUTO", 16.0, "E"),
        ("DESCRIÇÃO DO PRODUTO / SERVIÇO", 0.0, "E"),
        ("NCM/SH", 11.0, "C"),
        (if simples { "O/CSOSN" } else { "O/CST" }, 10.0, "C"),
        ("CFOP", 7.0, "C"),
        ("UN", 7.0, "C"),
        ("QUANT.", 14.0, "D"),
        ("VALOR UNIT.", 15.0, "D"),
        ("VALOR TOTAL", 15.0, "D"),
        ("B.CÁLC. ICMS", 15.0, "D"),
        ("VALOR ICMS", 13.0, "D"),
        ("VALOR IPI", 12.0, "D"),
        ("ALÍQ. ICMS", 8.0, "D"),
        ("ALÍQ. IPI", 8.0, "D"),
    ];
    let resto = largura_util - fixas.iter().map(|c| c.1).sum::<f64>();
    Value::Array(
        fixas
            .iter()
            .map(|(t, w, a)| json!([t, if *w == 0.0 { resto } else { *w }, a]))
            .collect(),
    )
}

fn igual(a: Option<crate::nfe::Decimal>, b: Option<crate::nfe::Decimal>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => a == b,
        (None, None) => true,
        _ => false,
    }
}

fn opcional(v: Option<crate::nfe::Decimal>, casas: u32) -> String {
    v.map_or(String::new(), |v| brl(Some(v), casas))
}

/// As 14 celulas do item, ja formatadas. A descricao leva o `infAdProd` logo abaixo (item 3.1.7) e, quando a unidade,
/// a quantidade ou o valor tributavel diferem dos comerciais, a linha com os dados tributaveis (mesmo item).
pub(super) fn celulas(it: &Item) -> Vec<String> {
    let icms = it.imposto.icms.as_ref();
    let ipi = it.imposto.ipi.as_ref();
    let mut desc = vec![it.descricao.clone()];
    if !it.informacao_adicional.is_empty() {
        desc.push(it.informacao_adicional.clone());
    }
    let (ut, qt, vt) = (
        &it.unidade_tributavel,
        it.quantidade_tributavel,
        it.valor_unitario_tributavel,
    );
    if !ut.is_empty()
        && (*ut != it.unidade || !igual(qt, it.quantidade) || !igual(vt, it.valor_unitario))
    {
        desc.push(format!(
            "Unid. tributável: {} {ut} x {}",
            qtd(qt),
            brl(vt, casas_unitario(vt))
        ));
    }
    let valor = |g: Option<&crate::nfe::Grupo>, tag: &str| {
        opcional(g.and_then(|g| g.valores.get(tag).copied()), 2)
    };
    let mut vun = brl(it.valor_unitario, casas_unitario(it.valor_unitario));
    if let Some(d) = it.desconto.filter(|d| *d != crate::nfe::Decimal::ZERO) {
        vun.push_str(&format!("\nDesc. {}", brl(Some(d), 2)));
    }
    vec![
        it.codigo.clone(),
        desc.join("\n"),
        it.ncm.clone(),
        it.imposto.cst_icms(),
        it.cfop.clone(),
        it.unidade.clone(),
        qtd(it.quantidade),
        vun,
        brl(it.valor_produto, 2),
        valor(icms, "vBC"),
        valor(icms, "vICMS"),
        valor(ipi, "vIPI"),
        valor(icms, "pICMS"),
        valor(ipi, "pIPI"),
    ]
}

fn transporte(nfe: &Nfe) -> Value {
    // 0 a 4 e 9: Anexo II, item 3.1.10 (modFrete)
    const FRETE: [(&str, &str); 6] = [
        ("0", "0-Remetente"),
        ("1", "1-Destinatário"),
        ("2", "2-Terceiros"),
        ("3", "3-Próprio Remetente"),
        ("4", "4-Próprio Destinatário"),
        ("9", "9-Sem Transporte"),
    ];
    let tr = nfe.transporte.as_ref();
    let tp = tr.and_then(|t| t.transportadora.as_ref());
    let en = tp.and_then(|t| t.endereco.as_ref());
    let vei = |k: &str| {
        tr.and_then(|t| t.veiculo.get(k))
            .cloned()
            .unwrap_or_default()
    };
    let modal = tr.map_or("", |t| t.modalidade_frete.as_str());
    let frete = FRETE
        .iter()
        .find(|(k, _)| *k == modal)
        .map_or(modal, |(_, v)| v);
    let vols = tr.map_or(&[][..], |t| t.volumes.as_slice());
    let junta = |f: fn(&crate::nfe::Volume) -> &str| {
        let mut vistos: Vec<&str> = Vec::new();
        for v in vols {
            let s = f(v);
            if !s.is_empty() && !vistos.contains(&s) {
                vistos.push(s);
            }
        }
        vistos.join(" / ")
    };
    let quantidade: u64 = vols.iter().filter_map(|v| v.quantidade).sum();
    let soma = |f: fn(&crate::nfe::Volume) -> Option<crate::nfe::Decimal>| {
        if vols.iter().any(|v| f(v).is_some()) {
            brl(Some(vols.iter().filter_map(f).sum()), 3)
        } else {
            String::new()
        }
    };
    json!({
        "nome": tp.map_or("", |t| t.nome.as_str()),
        "frete": frete,
        "antt": vei("RNTC"),
        "placa": vei("placa"),
        "uf_veiculo": vei("UF"),
        "doc": tp.map_or(String::new(), |t| documento(&t.tipo_documento, &t.documento)),
        "endereco": en.map_or("", |e| e.logradouro.as_str()),
        "municipio": en.map_or("", |e| e.municipio.as_str()),
        "uf": en.map_or("", |e| e.uf.as_str()),
        "ie": tp.map_or("", |t| t.ie.as_str()),
        "quantidade": if vols.iter().any(|v| v.quantidade.is_some_and(|q| q > 0)) { quantidade.to_string() } else { String::new() },
        "especie": junta(|v| v.especie.as_str()),
        "marca": junta(|v| v.marca.as_str()),
        "numeracao": junta(|v| v.numeracao.as_str()),
        "peso_bruto": soma(|v| v.peso_bruto),
        "peso_liquido": soma(|v| v.peso_liquido),
    })
}

fn fatura(nfe: &Nfe) -> Value {
    let Some(c) = nfe
        .cobranca
        .as_ref()
        .filter(|c| !c.duplicatas.is_empty() || !c.fatura_numero.is_empty())
    else {
        return Value::Null;
    };
    let texto = if !c.fatura_numero.is_empty() || c.fatura_original.is_some() {
        format!(
            "Fatura: {}   Valor original: R$ {}   Desconto: R$ {}   Valor líquido: R$ {}",
            c.fatura_numero,
            brl(c.fatura_original, 2),
            brl(c.fatura_desconto, 2),
            brl(c.fatura_liquido, 2)
        )
    } else {
        String::new()
    };
    let duplicatas: Vec<String> = c
        .duplicatas
        .iter()
        .map(|d| {
            let venc = d
                .vencimento
                .map_or(String::new(), |v| v.format("%d/%m/%Y").to_string());
            format!(
                "Nº {}\nVenc. {venc}\nValor R$ {}",
                d.numero,
                brl(d.valor, 2)
            )
        })
        .collect();
    json!({ "texto": texto, "duplicatas": duplicatas })
}

/// Conteudo comum aos dois leiautes. O leiaute RTC acrescenta o seu bloco em `rtc`.
pub(super) fn conteudo(nfe: &Nfe, op: &Opcoes, largura_util: f64) -> Result<Value, ErroDanfe> {
    conferir(nfe, op)?;
    let uf = uf_emitente(nfe);
    let (campo1, campo2) = campos_1_e_2(nfe, op)?;
    let chave_barras = barras::padrao(&nfe.chave).map_err(|e| ErroDanfe::Barras(e.to_string()))?;
    let emissao = local(nfe.ide.emissao.as_ref(), uf);
    let saida = local(nfe.ide.saida_entrada.as_ref(), uf);
    let t = &nfe.total;
    let imposto: serde_json::Map<String, Value> = [
        "vBC", "vICMS", "vBCST", "vST", "vProd", "vFrete", "vSeg", "vDesc", "vOutro", "vIPI", "vNF",
    ]
    .iter()
    .map(|k| ((*k).to_string(), Value::String(brl(Some(t.v(k)), 2))))
    .collect();
    let issqn = t.issqn_tot.as_ref().map_or(Value::Null, |g| {
        json!({
            "im": nfe.emit.im,
            "vServ": brl(Some(g.valor("vServ")), 2),
            "vBC": brl(Some(g.valor("vBC")), 2),
            "vISS": brl(Some(g.valor("vISS")), 2),
        })
    });
    let dest_nome = nfe.dest.as_ref().map_or("", |d| d.nome.as_str());
    Ok(json!({
        "titulo": format!("DANFE {}", nfe.chave),
        "situacao": nfe.situacao().como_texto(),
        "ambiente": if nfe.homologacao() { "homologacao" } else { "producao" },
        "tipo_emissao": nfe.ide.tipo_emissao,
        "canhoto": op.canhoto,
        "logo": op.logo.is_some(),
        "marca": marca(nfe, op).into_iter().map(|(t, n)| json!([t, n])).collect::<Vec<_>>(),
        "canhoto_texto": format!(
            "RECEBEMOS DE {} OS PRODUTOS E/OU SERVIÇOS CONSTANTES DA NOTA FISCAL ELETRÔNICA INDICADA AO LADO. \
             EMISSÃO: {} VALOR TOTAL: R$ {} DESTINATÁRIO: {dest_nome}",
            nfe.emit.nome.to_uppercase(),
            emissao.get(..10).unwrap_or(""),
            brl(Some(t.v("vNF")), 2)
        ),
        "numero": num9(nfe.ide.numero),
        "serie": format!("{:03}", nfe.ide.serie),
        "tipo_nf": nfe.ide.tipo_nf,
        "emitente": emitente(nfe),
        "chave": chave_dfe::formatar(&nfe.chave),
        "chave_barras": chave_barras,
        "campo1": campo1,
        "campo2": campo2,
        "natureza": nfe.ide.natureza_operacao,
        "emissao": emissao.get(..10).unwrap_or(""),
        "saida_data": saida.get(..10).unwrap_or(""),
        "saida_hora": saida.get(11..).unwrap_or(""),
        "destinatario": nfe.dest.as_ref().map_or(Value::Null, participante),
        "retirada": nfe.retirada.as_ref().map_or(Value::Null, participante),
        "entrega": nfe.entrega.as_ref().map_or(Value::Null, participante),
        "fatura": fatura(nfe),
        "imposto": Value::Object(imposto),
        "transporte": transporte(nfe),
        "colunas": colunas(nfe, largura_util),
        "itens": nfe.itens.iter().map(celulas).collect::<Vec<_>>(),
        "complementares": complementares(nfe, op),
        "issqn": issqn,
    }))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod testes {
    use super::*;

    #[test]
    fn dv_da_contingencia_e_o_da_chave() {
        // o mesmo calculo do DV da chave: 4 digitos de base dao o DV do modulo 11
        let base = "3526101122233300018155001000000001123456701";
        let chave = format!("{base}{}", dv_mod11(base));
        assert!(chave_dfe::validar(&chave, false).is_ok(), "{chave}");
    }

    #[test]
    fn protocolo_do_epec_tem_forma_fixa() {
        assert!(protocolo_epec_valido("891260000000001 03/10/2026 09:00:05"));
        assert!(!protocolo_epec_valido("891260000000001"));
        assert!(!protocolo_epec_valido(
            "891260000000001 2026-10-03T09:00:05"
        ));
        assert!(!protocolo_epec_valido("89x 03/10/2026 09:00:05"));
    }
}
