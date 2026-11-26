// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Da NFC-e lida para o conteudo do DANFC-e, divisao por divisao (Manual do DANFE NFC-e e QR Code 6.0, item 3.1).
//! Nada aqui imprime: o texto sai formatado e o PDF e o ESC/POS so' o dispõem no papel.

use super::modelo::{
    Cabecalho, Consulta, Consumidor, Danfce, Destaque, Identificacao, LinhaItem, Pagamento,
    SituacaoCupom, Totais, CANCELADA, CONTINGENCIA, HOMOLOGACAO, PENDENTE, TITULO, VIA_CONSUMIDOR,
    VIA_ESTABELECIMENTO,
};
use super::{ErroDanfce, Opcoes};
use crate::chave_dfe;
use crate::nfe::impressao::{brl, casas_unitario, documento, local, qtd};
use crate::nfe::{Decimal, Endereco, Nfe, Participante, Situacao};

/// Nome do meio de pagamento (`tPag`, campo YA02 do MOC 7.00, Anexo I). O 99 usa o `xPag`. Codigo fora da tabela do
/// MOC 7.00 sai com o `xPag`, se veio, ou com o numero.
fn forma(meio: &str, descricao: &str) -> String {
    let nome = match meio {
        "01" => "Dinheiro",
        "02" => "Cheque",
        "03" => "Cartão de Crédito",
        "04" => "Cartão de Débito",
        "05" => "Crédito Loja",
        "10" => "Vale Alimentação",
        "11" => "Vale Refeição",
        "12" => "Vale Presente",
        "13" => "Vale Combustível",
        "14" => "Duplicata Mercantil",
        "15" => "Boleto Bancário",
        "16" => "Depósito Bancário",
        "17" => "Pagamento Instantâneo (PIX)",
        "18" => "Transferência bancária, Carteira Digital",
        "19" => "Programa de fidelidade, Cashback, Crédito Virtual",
        "90" => "Sem pagamento",
        "99" => "Outros",
        _ => "",
    };
    let d = descricao.trim();
    if !d.is_empty() && (meio == "99" || nome.is_empty()) {
        return d.to_string();
    }
    if nome.is_empty() {
        format!("Forma de pagamento {meio}")
    } else {
        nome.to_string()
    }
}

fn positivo(v: Decimal) -> bool {
    v != Decimal::ZERO && !v.to_string().starts_with('-')
}

/// `logradouro, numero, complemento, bairro, municipio, UF` (sem o pais, sem o CEP).
fn endereco(e: &Endereco) -> String {
    let mut partes: Vec<&str> = [
        &e.logradouro,
        &e.numero,
        &e.complemento,
        &e.bairro,
        &e.municipio,
        &e.uf,
    ]
    .into_iter()
    .map(|p| p.trim())
    .filter(|p| !p.is_empty())
    .collect();
    partes.dedup();
    partes.join(", ")
}

fn cabecalho(nfe: &Nfe) -> Cabecalho {
    let e = &nfe.emit;
    let (rotulo, tipo) = if e.tipo_documento == "cpf" {
        ("CPF", "cpf")
    } else {
        ("CNPJ", "cnpj")
    };
    Cabecalho {
        rotulo_documento: rotulo.into(),
        documento: documento(tipo, &e.documento),
        nome: e.nome.trim().into(),
        endereco: e.endereco.as_ref().map(endereco).unwrap_or_default(),
        titulo: TITULO.into(),
    }
}

fn itens(nfe: &Nfe) -> Vec<LinhaItem> {
    nfe.itens
        .iter()
        .map(|i| LinhaItem {
            codigo: i.codigo.trim().into(),
            descricao: i.descricao.trim().into(),
            quantidade: qtd(i.quantidade),
            unidade: i.unidade.trim().into(),
            valor_unitario: brl(i.valor_unitario, casas_unitario(i.valor_unitario)),
            valor_total: brl(i.valor_produto, 2),
        })
        .collect()
}

/// Divisao III (Manual, 3.1.3): "Valor Total" e' a soma dos itens; acrescimos e desconto so' se existem; "Valor a Pagar"
/// (`vNF`) so' com acrescimo ou desconto. Se o `vNF` nao fecha com soma, acrescimos e desconto (desoneracao do ICMS
/// abatida no total), o "Valor a Pagar" tambem sai: o cupom nunca mostra contas que nao fecham.
fn totais(nfe: &Nfe) -> Totais {
    let t = &nfe.total;
    let soma: Decimal = nfe.itens.iter().filter_map(|i| i.valor_produto).sum();
    let valor = if t.icms_tot.is_some() {
        t.v("vProd")
    } else {
        soma
    };
    let acrescimos = t.v("vFrete").mais(t.v("vSeg")).mais(t.v("vOutro"));
    let desconto = t.v("vDesc");
    let a_pagar = t.v("vNF");
    let fecha = valor.mais(acrescimos).menos(desconto) == a_pagar;
    let mostra = positivo(acrescimos) || positivo(desconto) || (!fecha && t.icms_tot.is_some());
    Totais {
        quantidade_itens: nfe.itens.len().to_string(),
        valor_total: brl(Some(valor), 2),
        acrescimos: positivo(acrescimos).then(|| brl(Some(acrescimos), 2)),
        desconto: positivo(desconto).then(|| brl(Some(desconto), 2)),
        valor_a_pagar: mostra.then(|| brl(Some(a_pagar), 2)),
    }
}

fn consumidor(nfe: &Nfe, op: &Opcoes) -> Consumidor {
    let entrega = nfe.ide.presenca == "4";
    let nome_e_endereco = op.consumidor_com_nome_e_endereco || entrega;
    let Some(d) = nfe.dest.as_ref() else {
        return nao_identificado();
    };
    let tem_doc = !d.documento.is_empty();
    if !tem_doc && !entrega {
        return nao_identificado();
    }
    let (rotulo, doc) = if tem_doc {
        let r = match d.tipo_documento.as_str() {
            "cnpj" => "CONSUMIDOR CNPJ:",
            "estrangeiro" => "CONSUMIDOR Id. Estrangeiro:",
            _ => "CONSUMIDOR CPF:",
        };
        (r, Some(documento(&d.tipo_documento, &d.documento)))
    } else {
        ("CONSUMIDOR", None)
    };
    Consumidor {
        rotulo: rotulo.into(),
        documento: doc,
        nome: nome_do(d).filter(|_| nome_e_endereco),
        endereco: d
            .endereco
            .as_ref()
            .map(endereco)
            .filter(|e| !e.is_empty() && nome_e_endereco),
    }
}

fn nome_do(d: &Participante) -> Option<String> {
    let n = d.nome.trim();
    (!n.is_empty()).then(|| n.to_string())
}

fn nao_identificado() -> Consumidor {
    Consumidor {
        rotulo: "CONSUMIDOR NÃO IDENTIFICADO".into(),
        documento: None,
        nome: None,
        endereco: None,
    }
}

/// A situacao em que o cupom sai, ou a razao de nao haver cupom (Manual, 3.1.7 e 3.1.8).
fn situacao(nfe: &Nfe, op: &Opcoes) -> Result<SituacaoCupom, ErroDanfce> {
    let offline = nfe.ide.tipo_emissao == "9";
    match nfe.situacao() {
        Situacao::Autorizada if op.cancelada => Ok(SituacaoCupom::Cancelada),
        Situacao::Autorizada => Ok(SituacaoCupom::Autorizada),
        Situacao::Cancelada => Ok(SituacaoCupom::Cancelada),
        Situacao::SemProtocolo if op.cancelada => Err(ErroDanfce::CanceladaSemAutorizacao),
        Situacao::SemProtocolo if offline => Ok(SituacaoCupom::ContingenciaPendente),
        Situacao::SemProtocolo => Err(ErroDanfce::SemProtocolo),
        Situacao::Denegada => Err(ErroDanfce::Denegada),
        Situacao::ProtocoloDivergente => Err(ErroDanfce::ProtocoloDeOutraChave),
        _ => Err(ErroDanfce::Rejeitada(
            nfe.protocolo
                .as_ref()
                .map(|p| p.cstat.clone())
                .unwrap_or_default(),
        )),
    }
}

pub(super) fn conteudo(nfe: &Nfe, op: &Opcoes) -> Result<Danfce, ErroDanfce> {
    if nfe.modelo != "65" {
        return Err(ErroDanfce::NaoENfce(nfe.modelo.clone()));
    }
    if let Some(e) = &nfe.chave_erro {
        return Err(ErroDanfce::Chave(e.to_string()));
    }
    if nfe.qr_code.trim().is_empty() {
        return Err(ErroDanfce::SemQr);
    }
    if nfe.url_chave.trim().is_empty() {
        return Err(ErroDanfce::SemUrlChave);
    }
    let Some(emissao) = nfe.ide.emissao.as_ref() else {
        return Err(ErroDanfce::SemEmissao);
    };
    if !matches!(nfe.ide.tipo_emissao.as_str(), "1" | "9") {
        return Err(ErroDanfce::EmissaoInexistente(nfe.ide.tipo_emissao.clone()));
    }
    let sit = situacao(nfe, op)?;
    let contingencia = sit == SituacaoCupom::ContingenciaPendente;
    if op.via_estabelecimento && !contingencia {
        return Err(ErroDanfce::ViaSemContingencia);
    }
    let uf = nfe.emit.endereco.as_ref().map_or("", |e| e.uf.as_str());
    // na contingencia o protocolo nao se imprime (Manual, 3.1.7)
    let prot = nfe.protocolo.as_ref().filter(|_| !contingencia);
    let destaque = match sit {
        SituacaoCupom::Autorizada => None,
        SituacaoCupom::Cancelada => Some(Destaque {
            linha1: CANCELADA.into(),
            linha2: None,
        }),
        SituacaoCupom::ContingenciaPendente => Some(Destaque {
            linha1: CONTINGENCIA.into(),
            linha2: Some(PENDENTE.into()),
        }),
    };
    let texto = |s: Option<&str>| {
        s.map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };
    let inf = nfe.inf_adic.as_ref();
    let trib = nfe.total.v("vTotTrib");
    Ok(Danfce {
        chave: nfe.chave.clone(),
        homologacao: nfe.homologacao(),
        situacao: sit,
        cabecalho: cabecalho(nfe),
        destaque,
        itens: (!op.resumido).then(|| itens(nfe)),
        totais: totais(nfe),
        pagamentos: nfe
            .pagamentos
            .iter()
            .map(|p| Pagamento {
                forma: forma(&p.meio, &p.descricao),
                valor: brl(p.valor, 2),
            })
            .collect(),
        troco: nfe.troco.filter(|t| positivo(*t)).map(|t| brl(Some(t), 2)),
        consulta: Consulta {
            url: nfe.url_chave.trim().into(),
            chave: chave_dfe::formatar(&nfe.chave),
        },
        qr_code: nfe.qr_code.trim().into(),
        consumidor: consumidor(nfe, op),
        identificacao: Identificacao {
            numero: format!("{:09}", nfe.ide.numero),
            serie: format!("{:03}", nfe.ide.serie),
            emissao: local(Some(emissao), uf),
            via: contingencia.then(|| {
                if op.via_estabelecimento {
                    VIA_ESTABELECIMENTO.into()
                } else {
                    VIA_CONSUMIDOR.into()
                }
            }),
            protocolo: prot
                .map(|p| p.numero.trim().to_string())
                .filter(|n| !n.is_empty()),
            autorizacao: prot
                .and_then(|p| p.recebido_em.as_ref())
                .map(|d| local(Some(d), uf)),
        },
        mensagem_fiscal: texto(inf.map(|i| i.fisco.as_str())),
        homologacao_texto: nfe.homologacao().then(|| HOMOLOGACAO.to_string()),
        interesse_do_contribuinte: texto(inf.map(|i| i.complementar.as_str())),
        tributos: (op.tributos && positivo(trib)).then(|| format!("R${}", brl(Some(trib), 2))),
        mensagem_institucional: texto(op.mensagem_institucional.as_deref()),
    })
}
