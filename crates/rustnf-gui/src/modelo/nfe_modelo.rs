// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A NF-e guardada na pasta como ponto de partida: o "cliente ja usado" (quem ja recebeu NF-e completa o cadastro dele) e
//! o "usar como modelo" (a nota anterior vira o assistente preenchido). So' le o que o XML guardado traz; numero, data,
//! chave e protocolo nao se copiam (a aplicacao numera e data a nota nova), e o que nao da para ler fica vazio, para a
//! conferencia apontar. Mesmo principio do `reaproveitar` da NFS-e. Sem janela.

use rustnf::nfe::{Decimal, Grupo, Item, Nfe, Participante};

use super::mascara::{decimal_para_dinheiro, formatar_documento, so_digitos};
use super::nfe_fiscal::{Contribuinte, Entrega, Icms, Pagamento, Presenca};
use super::nfe_nova::{Consumo, DadosNfe, ItemNfe};

/// O que se sabe de um cliente que ja recebeu NF-e.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClienteConhecido {
    /// O documento (so' digitos e letras).
    pub documento: String,
    /// O nome.
    pub nome: String,
    /// A inscricao estadual.
    pub ie: String,
    /// O e-mail.
    pub email: String,
    /// O CEP.
    pub cep: String,
    /// O logradouro.
    pub logradouro: String,
    /// O numero.
    pub numero: String,
    /// O complemento.
    pub complemento: String,
    /// O bairro.
    pub bairro: String,
    /// O codigo IBGE do municipio.
    pub cod_municipio: String,
    /// Uso proprio ou revenda.
    pub consumo: Option<Consumo>,
    /// Contribuinte ou nao.
    pub contribuinte: Option<Contribuinte>,
}

/// O texto que a biblioteca poe no destinatario e no primeiro produto das notas de homologacao (a SEFAZ exige).
fn texto_de_homologacao(t: &str) -> bool {
    t.to_uppercase()
        .contains("AMBIENTE DE HOMOLOGACAO - SEM VALOR FISCAL")
}

fn consumo_do_xml(ind: &str) -> Option<Consumo> {
    match ind.trim() {
        "1" => Some(Consumo::Final),
        "0" => Some(Consumo::Revenda),
        _ => None,
    }
}

impl ClienteConhecido {
    /// O cliente que a NF-e guardada diz; `None` se a nota nao tem destinatario.
    #[must_use]
    pub fn da_nfe(n: &Nfe) -> Option<Self> {
        let d = n.dest.as_ref()?;
        let e = d.endereco.clone().unwrap_or_default();
        Some(Self {
            documento: d.documento.clone(),
            nome: if texto_de_homologacao(&d.nome) {
                String::new()
            } else {
                d.nome.clone()
            },
            ie: d.ie.clone(),
            email: d.email.clone(),
            cep: e.cep,
            logradouro: e.logradouro,
            numero: e.numero,
            complemento: e.complemento,
            bairro: e.bairro,
            cod_municipio: e.cmun,
            consumo: consumo_do_xml(&n.ide.consumidor_final),
            contribuinte: Contribuinte::do_xml(&d.ind_ie_dest),
        })
    }

    /// Completa os campos VAZIOS do assistente com o que se sabe do cliente (o que a pessoa digitou fica). Devolve o
    /// nome dos campos preenchidos, para a tela dizer.
    pub fn completar(&self, d: &mut DadosNfe) -> Vec<&'static str> {
        let mut feitos = Vec::new();
        let mut texto = |campo: &mut String, valor: &str, nome: &'static str| {
            if campo.trim().is_empty() && !valor.trim().is_empty() {
                *campo = valor.trim().to_string();
                feitos.push(nome);
            }
        };
        texto(&mut d.nome, &self.nome, "nome");
        texto(&mut d.ie, &self.ie, "inscrição estadual");
        texto(&mut d.email, &self.email, "e-mail");
        texto(&mut d.cep, &so_digitos(&self.cep, 8), "CEP");
        texto(&mut d.logradouro, &self.logradouro, "rua");
        texto(&mut d.numero, &self.numero, "numero");
        texto(&mut d.complemento, &self.complemento, "complemento");
        texto(&mut d.bairro, &self.bairro, "bairro");
        texto(
            &mut d.cod_municipio,
            &so_digitos(&self.cod_municipio, 7),
            "municipio",
        );
        if d.consumo.is_none() && self.consumo.is_some() {
            d.consumo = self.consumo;
            feitos.push("uso próprio ou revenda");
        }
        if d.contribuinte.is_none() && self.contribuinte.is_some() {
            d.contribuinte = self.contribuinte;
            feitos.push("inscrição estadual do cliente");
        }
        feitos
    }

    /// A frase que diz de onde veio o que foi preenchido.
    #[must_use]
    pub fn aviso(&self, feitos: &[&'static str]) -> String {
        format!(
            "Completei {} com a NF-e mais recente de {}. Confira e corrija o que mudou.",
            feitos.join(", "),
            if self.nome.trim().is_empty() {
                formatar_documento(&self.documento)
            } else {
                self.nome.trim().to_string()
            }
        )
    }
}

/// A NF-e anterior como assistente: os dados e o que nao deu para ler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModeloNfe {
    /// Os dados do assistente.
    pub dados: DadosNfe,
    /// O que ficou vazio ou mudou e a pessoa precisa ver.
    pub avisos: Vec<String>,
}

/// Decimal do XML como a tela digita quantidade: virgula decimal, sem zeros a mais.
fn quantidade(q: Option<Decimal>) -> String {
    q.map(|d| {
        let t = d.to_string();
        let t = if t.contains('.') {
            t.trim_end_matches('0').trim_end_matches('.').to_string()
        } else {
            t
        };
        t.replace('.', ",")
    })
    .unwrap_or_default()
}

/// Decimal do XML como dinheiro da tela, so' se couber em centavos (mais casas: `None`).
fn dinheiro_exato(v: Option<Decimal>) -> Option<String> {
    let t = v?.to_string();
    let casas = t
        .split_once('.')
        .map_or(0, |(_, f)| f.trim_end_matches('0').len());
    (casas <= 2).then(|| decimal_para_dinheiro(&t))
}

fn icms_do_xml(g: &Grupo, i: &mut ItemNfe, crt_simples: bool) {
    let codigo = if g.texto("CSOSN").is_empty() {
        g.texto("CST")
    } else {
        g.texto("CSOSN")
    };
    i.origem_mercadoria = g.texto("orig").trim().parse().unwrap_or(0);
    let conhecido = match (codigo, crt_simples) {
        ("102", true) => Some(Icms::SemDestaque),
        ("500", true) | ("60", false) => Some(Icms::JaCobradoAntes),
        ("400", true) | ("41", false) => Some(Icms::NaoTributado),
        ("00", false) => Some(Icms::Tributado),
        ("40", false) => Some(Icms::Isento),
        _ => None,
    };
    match conhecido {
        Some(Icms::Tributado) => {
            i.icms = Some(Icms::Tributado);
            i.aliquota_icms = quantidade(Some(g.valor("pICMS")));
        }
        // a ST retida depende da compra: nao se copia, a conferencia pede
        Some(Icms::JaCobradoAntes) => i.icms = Some(Icms::JaCobradoAntes),
        Some(outro) => i.icms = Some(outro),
        None if !codigo.is_empty() => i.icms_codigo = Some(codigo.to_string()),
        None => {}
    }
}

fn item_do_xml(it: &Item, crt_simples: bool, avisos: &mut Vec<String>) -> ItemNfe {
    let homologacao = texto_de_homologacao(&it.descricao);
    if homologacao {
        avisos.push(
            "O primeiro produto da nota anterior (de homologação) trazia o texto de teste no lugar da descrição: descreva o produto.".into(),
        );
    }
    let mut i = ItemNfe {
        codigo: it.codigo.clone(),
        descricao: if homologacao {
            String::new()
        } else {
            it.descricao.clone()
        },
        unidade: it.unidade.clone(),
        quantidade: quantidade(it.quantidade),
        ncm: it.ncm.clone(),
        cest: it.cest.clone(),
        ..ItemNfe::default()
    };
    match dinheiro_exato(it.valor_unitario) {
        Some(v) => i.valor = v,
        None => avisos.push(format!(
            "O valor unitário de \"{}\" tinha mais de 2 casas decimais: digite o valor.",
            it.descricao
        )),
    }
    if let Some(d) = it.desconto.filter(|d| *d != Decimal::ZERO) {
        i.desconto = decimal_para_dinheiro(&d.to_string());
    }
    if let Some(g) = &it.imposto.icms {
        icms_do_xml(g, &mut i, crt_simples);
    }
    i
}

fn presenca_do_xml(ind: &str) -> Presenca {
    match ind.trim() {
        "2" => Presenca::Internet,
        "3" => Presenca::Telefone,
        "9" => Presenca::Outro,
        _ => Presenca::Presencial,
    }
}

fn entrega_do_xml(mod_frete: &str) -> Entrega {
    match mod_frete.trim() {
        "0" => Entrega::TransportadoraPorMinhaConta,
        "1" => Entrega::TransportadoraPorContaDoCliente,
        "2" => Entrega::TransportadoraDeTerceiro,
        "3" => Entrega::Propria,
        "4" => Entrega::ClienteBusca,
        _ => Entrega::Balcao,
    }
}

fn transportadora(d: &mut DadosNfe, t: &Participante) {
    d.transp_documento.clone_from(&t.documento);
    d.transp_nome.clone_from(&t.nome);
    d.transp_ie.clone_from(&t.ie);
    if let Some(e) = &t.endereco {
        d.transp_endereco.clone_from(&e.logradouro);
        d.transp_municipio.clone_from(&e.municipio);
        d.transp_uf.clone_from(&e.uf);
    }
}

impl ModeloNfe {
    /// A NF-e guardada como assistente. `crt` e' o regime da configuracao de hoje (decide como se le o ICMS da nota).
    #[must_use]
    pub fn da_nfe(n: &Nfe, crt: u8) -> Self {
        let mut avisos = Vec::new();
        let mut d = DadosNfe::default();
        match ClienteConhecido::da_nfe(n) {
            Some(c) => {
                c.completar(&mut d);
                d.documento = c.documento;
            }
            None => avisos
                .push("A nota anterior não tinha destinatario: digite os dados do cliente.".into()),
        }
        let simples = matches!(crt, 1 | 4);
        d.itens = n
            .itens
            .iter()
            .map(|i| item_do_xml(i, simples, &mut avisos))
            .collect();
        d.presenca = presenca_do_xml(&n.ide.presenca);
        if let Some(p) = n.pagamentos.first() {
            d.pagamento = Pagamento::do_xml(&p.meio, &p.indicador);
        }
        if d.pagamento == Pagamento::APrazo {
            let parcelas = n.cobranca.as_ref().map_or(0, |c| c.duplicatas.len());
            if parcelas >= 2 {
                d.parcelas = parcelas.to_string();
            }
            avisos.push(
                "As parcelas da nota anterior já venceram: digite o vencimento da primeira parcela.".into(),
            );
        }
        if let Some(t) = &n.transporte {
            d.entrega = entrega_do_xml(&t.modalidade_frete);
            if let Some(p) = &t.transportadora {
                transportadora(&mut d, p);
            }
            if let Some(v) = t.volumes.first() {
                d.volumes = v.quantidade.map(|q| q.to_string()).unwrap_or_default();
                d.especie.clone_from(&v.especie);
                d.peso_bruto = quantidade(v.peso_bruto);
            }
        }
        if d.itens.iter().any(|i| i.icms == Some(Icms::JaCobradoAntes)) {
            avisos.push(
                "O ICMS já cobrado antes depende da compra de cada mercadoria: digite a base, a alíquota e o valor nos detalhes fiscais.".into(),
            );
        }
        Self { dados: d, avisos }
    }
}

#[cfg(test)]
mod testes;
