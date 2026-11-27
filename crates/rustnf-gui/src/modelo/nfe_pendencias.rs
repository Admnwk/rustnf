// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O que falta para montar o pedido da Nova NF-e, em portugues comum, com o que fazer e o passo onde se corrige. So' o que
//! impede de MONTAR o pedido: as regras do leiaute e da SEFAZ sao da conferencia da aplicacao.

use rustnf::chave_dfe;

use super::mascara::{data_br_para_iso, dinheiro_para_centavos, so_digitos};
use super::nfe_fiscal::{por_que_sem_cfop, Contribuinte, Destino, Icms, Pagamento};
use super::nfe_nova::{
    centesimos_de_percentual, municipio_do_codigo, ContextoNfe, DadosNfe, ItemNfe, PassoNfe,
    PendenciaNfe, MAX_PARCELAS,
};
use chrono::NaiveDate;

pub(super) fn so_letras_e_digitos(t: &str) -> String {
    t.chars().filter(char::is_ascii_alphanumeric).collect()
}

/// O documento (so' digitos para CPF/CNPJ numerico) confere pelo digito verificador.
fn documento_confere(d: &str) -> bool {
    match d.len() {
        11 => chave_dfe::cpf_valido(d),
        14 => chave_dfe::cnpj_valido(d),
        _ => false,
    }
}

/// O que falta, so' para montar o pedido. Os erros do leiaute e das regras da SEFAZ vem da conferencia da aplicacao.
#[must_use]
pub fn pendencias(d: &DadosNfe, c: &ContextoNfe) -> Vec<PendenciaNfe> {
    let mut p = Vec::new();
    para_quem(d, c, &mut p);
    o_que_vendeu(d, c, &mut p);
    como_recebeu(d, c, &mut p);
    como_entrega(d, &mut p);
    p
}

fn para_quem(d: &DadosNfe, c: &ContextoNfe, p: &mut Vec<PendenciaNfe>) {
    use PassoNfe::ParaQuem as Q;
    let doc = d.documento_limpo();
    if doc.is_empty() {
        p.push(PendenciaNfe::nova(
            Q,
            "Falta o CPF ou o CNPJ de quem compra.",
            "Digite o CPF ou o CNPJ do cliente.",
        ));
    } else if !documento_confere(&doc) {
        p.push(PendenciaNfe::nova(
            Q,
            "O CPF ou o CNPJ não confere.",
            "Confira os números: o dígito final não bate.",
        ));
    }
    if d.nome.trim().chars().count() < 2 {
        p.push(PendenciaNfe::nova(
            Q,
            "Falta o nome de quem compra.",
            "Digite o nome ou a razão social do cliente.",
        ));
    }
    if d.consumo.is_none() {
        p.push(PendenciaNfe::nova(
            Q,
            "Falta dizer para que o cliente compra.",
            "Escolha entre uso próprio e revenda.",
        ));
    }
    match d.contribuinte {
        None => p.push(PendenciaNfe::nova(
            Q,
            "Falta dizer se o cliente tem inscrição estadual.",
            "Escolha se ele tem inscrição estadual, se é isento ou se não é contribuinte.",
        )),
        Some(Contribuinte::ComIe) if d.ie.trim().is_empty() => p.push(PendenciaNfe::nova(
            Q,
            "Falta a inscrição estadual do cliente.",
            "Digite a inscrição estadual, ou troque a escolha para quem não tem.",
        )),
        _ => {}
    }
    if d.exterior {
        p.push(PendenciaNfe::nova(
            Q,
            "A venda ao exterior pede o país, o documento estrangeiro e os dados de embarque.",
            "Este assistente ainda não leva esses dados: emita pelo pedido em JSON (rustnf nfe emitir).",
        ));
        return;
    }
    for (vazio, o_que) in [
        (d.logradouro.trim().is_empty(), "a rua"),
        (d.numero.trim().is_empty(), "o número (S/N se não tem)"),
        (d.bairro.trim().is_empty(), "o bairro"),
    ] {
        if vazio {
            p.push(PendenciaNfe::nova(
                Q,
                format!("Falta {o_que} do endereço do cliente."),
                "Digite o CEP e use Buscar endereço, ou preencha a mão.",
            ));
        }
    }
    if municipio_do_codigo(&d.cod_municipio).is_none() {
        p.push(PendenciaNfe::nova(
            Q,
            "Falta o município do cliente (código IBGE de 7 dígitos).",
            "Digite o CEP e use Buscar endereço; ou digite o código do município.",
        ));
    } else if d.destino(c).is_none() {
        p.push(PendenciaNfe::nova(
            Q,
            "Falta a UF do emitente para saber se a venda é dentro do estado.",
            "Informe a UF na seção NF-e das Configurações.",
        ));
    }
}

fn o_que_vendeu(d: &DadosNfe, c: &ContextoNfe, p: &mut Vec<PendenciaNfe>) {
    use PassoNfe::OQueVendeu as V;
    if d.itens.is_empty() {
        p.push(PendenciaNfe::nova(
            V,
            "Nenhum produto na nota.",
            "Escolha um produto do cadastro ou inclua um produto novo.",
        ));
    }
    if c.pergunta_o_regime_pis() && c.regime_pis_efetivo(d).is_none() {
        p.push(PendenciaNfe::nova(
            V,
            "Falta dizer o regime do PIS e da COFINS.",
            "Escolha cumulativo ou não cumulativo (ou guarde na seção NF-e das Configurações).",
        ));
    }
    let destino = d.destino(c);
    for (n, i) in d.itens.iter().enumerate() {
        item_pendente(i, n, destino, c, p);
    }
}

fn item_pendente(
    i: &ItemNfe,
    n: usize,
    destino: Option<Destino>,
    c: &ContextoNfe,
    p: &mut Vec<PendenciaNfe>,
) {
    use PassoNfe::OQueVendeu as V;
    let nome = if i.descricao.trim().is_empty() {
        format!("\"Produto {}\"", n + 1)
    } else {
        format!("\"{}\"", i.descricao.trim())
    };
    let mut falta = |msg: String, como: &str| {
        p.push(PendenciaNfe::nova(V, msg, como).do_item(n));
    };
    if i.descricao.trim().is_empty() {
        falta(
            format!("O produto {} não tem descrição.", n + 1),
            "Descreva o que foi vendido.",
        );
    }
    if i.unidade.trim().is_empty() {
        falta(
            format!("{nome} não tem unidade."),
            "Digite a unidade (UN, KG, CX...).",
        );
    }
    if i.quantidade_milesimos().is_none() {
        falta(
            format!("A quantidade de {nome} não serve."),
            "Digite um número maior que zero (até 3 casas).",
        );
    }
    if i.valor_centavos().is_none() {
        falta(
            format!("Falta o valor de {nome}."),
            "Digite o valor de uma unidade.",
        );
    }
    match (i.desconto_centavos(), i.produto_centavos()) {
        (None, _) => falta(
            format!("O desconto de {nome} não serve."),
            "Digite um valor em dinheiro.",
        ),
        (Some(d), Some(prod)) if d > prod => falta(
            format!("O desconto de {nome} é maior que o valor do produto."),
            "Diminua o desconto.",
        ),
        _ => {}
    }
    if so_digitos(&i.ncm, 8).len() != 8 {
        falta(
            format!("Falta o NCM de {nome} (8 dígitos)."),
            "Procure o NCM na tabela da Receita (TIPI); o programa não tem a tabela. Fica guardado no cadastro.",
        );
    }
    let Some(icms) = i.icms_efetivo(c.crt) else {
        falta(
            "O regime da empresa (CRT) não está na configuração.".to_string(),
            "Escolha o regime na seção NF-e das Configurações.",
        );
        return;
    };
    if i.icms_codigo.is_none() && icms.codigo(c.crt).is_none() {
        falta(
            format!("A opção de ICMS de {nome} não existe no regime da empresa."),
            "Abra os detalhes fiscais e escolha outra opção.",
        );
        return;
    }
    if icms == Icms::Tributado
        && i.icms_codigo.is_none()
        && centesimos_de_percentual(&i.aliquota_icms).is_none_or(|a| a == 0 || a > 10_000)
    {
        falta(
            format!("Falta a alíquota do ICMS de {nome}."),
            "Digite a alíquota (por exemplo 18); o programa não chuta a alíquota do seu estado.",
        );
    }
    if icms == Icms::JaCobradoAntes && i.icms_codigo.is_none() {
        for (texto, o_que, dinheiro) in [
            (&i.st_base, "a base", true),
            (&i.st_aliquota, "a alíquota", false),
            (&i.st_valor, "o valor", true),
        ] {
            let bom = if dinheiro {
                dinheiro_para_centavos(texto).is_some()
            } else {
                centesimos_de_percentual(texto).is_some()
            };
            if !bom {
                falta(
                    format!("Falta {o_que} do ICMS já cobrado antes de {nome}."),
                    "Esta na nota de compra da mercadoria; digite nos detalhes fiscais.",
                );
            }
        }
    }
    if let Some(destino) = destino {
        if i.cfop.is_none() {
            if let Some(porque) = por_que_sem_cfop(destino, i.origem, icms) {
                falta(
                    format!("{nome}: {porque}"),
                    "Mude a origem ou a opção de ICMS.",
                );
            }
        }
    }
}

fn como_recebeu(d: &DadosNfe, c: &ContextoNfe, p: &mut Vec<PendenciaNfe>) {
    use PassoNfe::ComoRecebeu as R;
    if d.pagamento != Pagamento::APrazo {
        return;
    }
    let n = d.parcelas.trim().parse::<u32>().unwrap_or(0);
    if !(2..=MAX_PARCELAS).contains(&n) {
        p.push(PendenciaNfe::nova(
            R,
            "O número de parcelas não serve.",
            format!("Digite de 2 a {MAX_PARCELAS} parcelas."),
        ));
    }
    match data_br_para_iso(&d.primeiro_vencimento)
        .and_then(|iso| NaiveDate::parse_from_str(&iso, "%Y-%m-%d").ok())
    {
        None => p.push(PendenciaNfe::nova(
            R,
            "Falta a data da primeira parcela.",
            "Digite o vencimento no formato DD/MM/AAAA.",
        )),
        Some(v) if v < c.hoje => p.push(PendenciaNfe::nova(
            R,
            "A primeira parcela vence antes de hoje.",
            "Escolha uma data de hoje em diante.",
        )),
        Some(_) => {}
    }
}

fn como_entrega(d: &DadosNfe, p: &mut Vec<PendenciaNfe>) {
    use PassoNfe::ComoEntrega as E;
    if !d.entrega.com_transportadora() {
        return;
    }
    let doc = so_letras_e_digitos(&d.transp_documento);
    if doc.is_empty() && d.transp_nome.trim().is_empty() {
        p.push(PendenciaNfe::nova(
            E,
            "Faltam os dados da transportadora.",
            "Digite o CNPJ ou o nome da transportadora.",
        ));
    }
    if !doc.is_empty() && !documento_confere(&doc) {
        p.push(PendenciaNfe::nova(
            E,
            "O CNPJ da transportadora não confere.",
            "Confira os números: o dígito final não bate.",
        ));
    }
    if !d.transp_nome.trim().is_empty() && d.transp_nome.trim().chars().count() < 2 {
        p.push(PendenciaNfe::nova(
            E,
            "O nome da transportadora é curto demais.",
            "Digite o nome inteiro.",
        ));
    }
}
