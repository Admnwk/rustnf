// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A conferencia da nota, em duas camadas: (a) o que falta ou esta fora de forma no que foi preenchido, todas as
//! pendencias de uma vez, cada uma ligada ao seu campo; (b) quando (a) esta limpa, a conferencia da biblioteca
//! (`rustnf_app::nucleo::conferir_dps`), cuja recusa vira pendencia no campo da tela que corresponde ao campo da
//! recusa.

use rustnf::chave_dfe::{cnpj_valido, cpf_valido};
use rustnf::nfse::{ctribnac, Dps};
use rustnf::reforma::{indop, tabela};
use rustnf_app::config::aplicar_emitente;
use rustnf_app::erro::Erro;
use rustnf_app::nucleo;

use super::mascara::{
    data_br_para_iso, dinheiro_para_centavos, rotulo_do_municipio, RotuloMunicipio,
};
use super::nota::{CampoNota, DadosNota};
use super::pedido::{montar_pedido, percentual, Contexto};
use super::previa::totais;

/// Uma coisa a corrigir antes de emitir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pendencia {
    /// O campo da tela, quando se sabe qual e'.
    pub campo: Option<CampoNota>,
    /// A frase para a pessoa.
    pub mensagem: String,
    /// O codigo da regra (do Anexo I, por exemplo), quando a biblioteca disse qual.
    pub regra: Option<String>,
}

impl Pendencia {
    fn de_campo(campo: CampoNota, mensagem: impl Into<String>) -> Self {
        Self {
            campo: Some(campo),
            mensagem: mensagem.into(),
            regra: None,
        }
    }

    /// Pendencia que nao e' de campo nenhum da tela.
    #[must_use]
    pub fn geral(mensagem: impl Into<String>) -> Self {
        Self {
            campo: None,
            mensagem: mensagem.into(),
            regra: None,
        }
    }
}

/// O resultado da conferencia: as pendencias, na ordem da pagina (as gerais por ultimo).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Conferencia {
    /// As pendencias.
    pub pendencias: Vec<Pendencia>,
    /// A biblioteca ja conferiu (a conferencia propria estava limpa). Sem pendencia e com isto, a nota pode ir.
    pub da_biblioteca: bool,
}

impl Conferencia {
    /// Nao ha nada a corrigir.
    #[must_use]
    pub fn limpa(&self) -> bool {
        self.pendencias.is_empty()
    }

    /// A pendencia do campo, se ele tem uma.
    #[must_use]
    pub fn do_campo(&self, campo: CampoNota) -> Option<&Pendencia> {
        self.pendencias.iter().find(|p| p.campo == Some(campo))
    }

    /// A primeira pendencia.
    #[must_use]
    pub fn primeira(&self) -> Option<&Pendencia> {
        self.pendencias.first()
    }

    /// O motivo, em uma frase, de a nota nao poder ser emitida.
    #[must_use]
    pub fn motivo(&self) -> Option<String> {
        let primeira = self.primeira()?;
        Some(match self.pendencias.len() {
            1 => primeira.mensagem.clone(),
            n => format!("{} (e mais {})", primeira.mensagem, n - 1),
        })
    }
}

fn email_ok(email: &str) -> bool {
    let Some((local, dominio)) = email.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && !email.chars().any(char::is_whitespace)
        && !dominio.contains('@')
        && dominio.contains('.')
        && !dominio.starts_with('.')
        && !dominio.ends_with('.')
}

fn municipio_ok(texto: &str) -> Option<&'static str> {
    if texto.len() != 7 {
        return Some("Informe o código IBGE do município, com 7 dígitos.");
    }
    match rotulo_do_municipio(texto) {
        RotuloMunicipio::NaoConsta => Some("Código não encontrado na tabela de municípios."),
        _ => None,
    }
}

fn do_tomador(d: &DadosNota, saida: &mut Vec<Pendencia>) {
    if d.sem_tomador {
        return;
    }
    let doc = d.documento_do_tomador();
    let documento = match doc.len() {
        0 => {
            Some("Informe o CNPJ ou o CPF do tomador, ou marque \"Nota sem tomador identificado\".")
        }
        11 if doc.bytes().all(|b| b.is_ascii_digit()) => {
            (!cpf_valido(&doc)).then_some("CPF inválido: confira os dígitos.")
        }
        14 => (!cnpj_valido(&doc)).then_some("CNPJ inválido: confira os dígitos."),
        _ => Some("O CPF tem 11 dígitos e o CNPJ, 14."),
    };
    if let Some(m) = documento {
        saida.push(Pendencia::de_campo(CampoNota::TomadorDocumento, m));
    }
    if d.tomador_nome.trim().is_empty() {
        saida.push(Pendencia::de_campo(
            CampoNota::TomadorNome,
            "Informe o nome ou a razão social do tomador.",
        ));
    }
    let email = d.tomador_email.trim();
    if !email.is_empty() && !email_ok(email) {
        saida.push(Pendencia::de_campo(
            CampoNota::TomadorEmail,
            "E-mail inválido: confira o endereço.",
        ));
    }
    let fone = d.tomador_fone.chars().filter(char::is_ascii_digit).count();
    if fone > 0 && !(10..=11).contains(&fone) {
        saida.push(Pendencia::de_campo(
            CampoNota::TomadorFone,
            "Telefone incompleto: informe o DDD e o número.",
        ));
    }
    if d.tem_endereco() {
        if d.end_cep.chars().filter(char::is_ascii_digit).count() != 8 {
            saida.push(Pendencia::de_campo(
                CampoNota::EnderecoCep,
                "Informe o CEP, com 8 dígitos.",
            ));
        }
        if d.end_logradouro.trim().is_empty() {
            saida.push(Pendencia::de_campo(
                CampoNota::EnderecoLogradouro,
                "Informe o logradouro.",
            ));
        }
        if d.end_bairro.trim().is_empty() {
            saida.push(Pendencia::de_campo(
                CampoNota::EnderecoBairro,
                "Informe o bairro.",
            ));
        }
        if let Some(m) = municipio_ok(&d.end_municipio) {
            saida.push(Pendencia::de_campo(CampoNota::EnderecoMunicipio, m));
        }
    }
}

fn do_servico(d: &DadosNota, saida: &mut Vec<Pendencia>) {
    let codigo = d.ctribnac_digitos();
    if codigo.len() != 6 {
        saida.push(Pendencia::de_campo(
            CampoNota::Ctribnac,
            "Escolha o código de tributação nacional: busque pelo serviço ou digite os 6 dígitos.",
        ));
    } else if !ctribnac::existe(&codigo) {
        saida.push(Pendencia::de_campo(
            CampoNota::Ctribnac,
            "Esse código não está na Lista Nacional de Serviços: busque pelo serviço.",
        ));
    }
    if d.descricao.trim().is_empty() {
        saida.push(Pendencia::de_campo(
            CampoNota::Descricao,
            "Descreva o serviço prestado.",
        ));
    }
    let prestacao = d.municipio_prestacao.trim();
    if !prestacao.is_empty() {
        if let Some(m) = municipio_ok(prestacao) {
            saida.push(Pendencia::de_campo(CampoNota::MunicipioPrestacao, m));
        }
    }
    let nbs = d.nbs_digitos();
    if nbs.is_empty() && d.ibscbs_ativo {
        saida.push(Pendencia::de_campo(
            CampoNota::Nbs,
            "O NBS é obrigatório com o grupo IBS/CBS.",
        ));
    } else if !nbs.is_empty() && nbs.len() != 9 {
        saida.push(Pendencia::de_campo(CampoNota::Nbs, "O NBS tem 9 dígitos."));
    }
}

fn do_valor(campo: CampoNota, texto: &str, servico: Option<i64>, saida: &mut Vec<Pendencia>) {
    if texto.trim().is_empty() {
        return;
    }
    match (dinheiro_para_centavos(texto), servico) {
        (None, _) => saida.push(Pendencia::de_campo(campo, "Valor inválido.")),
        (Some(v), Some(s)) if v > s => saida.push(Pendencia::de_campo(
            campo,
            "O desconto não pode passar do valor do serviço.",
        )),
        _ => {}
    }
}

fn dos_valores(d: &DadosNota, saida: &mut Vec<Pendencia>) {
    let servico = dinheiro_para_centavos(&d.valor_servico);
    match servico {
        None => saida.push(Pendencia::de_campo(
            CampoNota::ValorServico,
            "Informe o valor do serviço.",
        )),
        Some(0) => saida.push(Pendencia::de_campo(
            CampoNota::ValorServico,
            "O valor do serviço precisa ser maior que zero.",
        )),
        Some(_) => {}
    }
    do_valor(
        CampoNota::DescontoIncondicionado,
        &d.desc_incondicionado,
        servico,
        saida,
    );
    do_valor(
        CampoNota::DescontoCondicionado,
        &d.desc_condicionado,
        servico,
        saida,
    );
    if data_br_para_iso(&d.competencia).is_none() {
        saida.push(Pendencia::de_campo(
            CampoNota::Competencia,
            "Informe uma data válida, no formato DD/MM/AAAA.",
        ));
    }
}

/// O percentual digitado em milesimos de ponto (`2,5` = 25000): 4 casas, sem ponto flutuante. `None` se nao for numero.
fn percentual_x10000(texto: &str) -> Option<u64> {
    let p = percentual(texto)?;
    let (inteira, fracao) = p.split_once('.').unwrap_or((&p, ""));
    if fracao.len() > 4
        || !inteira
            .bytes()
            .chain(fracao.bytes())
            .all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let inteira: u64 = inteira.parse().ok()?;
    let fracao: u64 = format!("{fracao:0<4}").parse().ok()?;
    inteira.checked_mul(10_000)?.checked_add(fracao)
}

/// O percentual nao e' numero ou passa de `maximo` (em milesimos de ponto percentual: 5% = 50_000).
fn pct_fora(texto: &str, maximo: u64) -> bool {
    percentual_x10000(texto).is_none_or(|v| v > maximo)
}

fn do_iss(d: &DadosNota, ctx: &Contexto, saida: &mut Vec<Pendencia>) {
    let regime = ctx.regime();
    if d.retencao == 2 && (d.sem_tomador || d.documento_do_tomador().is_empty()) {
        saida.push(Pendencia::de_campo(
            CampoNota::Retencao,
            "Para o tomador reter o ISS, ele precisa ter CNPJ ou CPF informado.",
        ));
    }
    if regime.mostra_aliquota_iss(d.tributacao)
        && !d.aliquota_iss.trim().is_empty()
        && pct_fora(&d.aliquota_iss, 50_000)
    {
        saida.push(Pendencia::de_campo(
            CampoNota::AliquotaIss,
            "A alíquota do ISS vai de 0 a 5%.",
        ));
    }
    if regime.pede_percentual_sn() {
        if percentual(&d.percentual_sn).is_none() {
            saida.push(Pendencia::de_campo(
                CampoNota::PercentualSn,
                "Informe o percentual de tributos do Simples Nacional (pergunte ao contador).",
            ));
        } else if pct_fora(&d.percentual_sn, 999_900) {
            saida.push(Pendencia::de_campo(
                CampoNota::PercentualSn,
                "O percentual vai de 0 a 99,99%.",
            ));
        }
    }
    if regime.pede_totais_aproximados() {
        let campos = [
            (CampoNota::PercentualFederal, &d.percentual_federal),
            (CampoNota::PercentualEstadual, &d.percentual_estadual),
            (CampoNota::PercentualMunicipal, &d.percentual_municipal),
        ];
        if campos.iter().all(|(_, t)| percentual(t).is_none()) {
            saida.push(Pendencia::de_campo(
                CampoNota::PercentualFederal,
                "Informe o percentual aproximado de tributos (federal, estadual ou municipal).",
            ));
        }
        for (campo, texto) in campos {
            if percentual(texto).is_some() && pct_fora(texto, 1_000_000) {
                saida.push(Pendencia::de_campo(campo, "O percentual vai de 0 a 100%."));
            }
        }
    }
}

fn dos_federais(d: &DadosNota, saida: &mut Vec<Pendencia>) {
    for (campo, texto) in [
        (CampoNota::Irrf, &d.irrf),
        (CampoNota::Csll, &d.csll),
        (CampoNota::Previdencia, &d.previdencia),
        (CampoNota::PisBase, &d.pis_base),
    ] {
        if !texto.trim().is_empty() && dinheiro_para_centavos(texto).is_none() {
            saida.push(Pendencia::de_campo(campo, "Valor inválido."));
        }
    }
    let pis_preenchido = [
        &d.pis_base,
        &d.pis_aliquota_pis,
        &d.pis_aliquota_cofins,
        &d.pis_tipo_retencao,
    ]
    .iter()
    .any(|t| !t.trim().is_empty());
    if d.pis_cst.trim().is_empty() && pis_preenchido {
        saida.push(Pendencia::de_campo(
            CampoNota::PisCst,
            "Informe o CST do PIS/COFINS, ou apague os outros campos do grupo.",
        ));
    } else if !d.pis_cst.trim().is_empty() && d.pis_cst.len() != 2 {
        saida.push(Pendencia::de_campo(
            CampoNota::PisCst,
            "O CST do PIS/COFINS tem 2 dígitos.",
        ));
    }
    for (campo, texto) in [
        (CampoNota::PisAliquotaPis, &d.pis_aliquota_pis),
        (CampoNota::PisAliquotaCofins, &d.pis_aliquota_cofins),
    ] {
        if !texto.trim().is_empty() && pct_fora(texto, 999_900) {
            saida.push(Pendencia::de_campo(campo, "A alíquota vai de 0 a 99,99%."));
        }
    }
}

fn do_ibscbs(d: &DadosNota, saida: &mut Vec<Pendencia>) {
    if !d.ibscbs_ativo {
        return;
    }
    if d.cind_op.len() != 6 {
        saida.push(Pendencia::de_campo(
            CampoNota::CindOp,
            "Informe o cIndOp, com 6 dígitos (a sugestão preenche).",
        ));
    } else if !indop::permitido(&d.cind_op, "nfse").unwrap_or(false) {
        saida.push(Pendencia::de_campo(
            CampoNota::CindOp,
            "Esse cIndOp não vale para NFS-e (Anexo VII).",
        ));
    }
    if d.cclass_trib.len() != 6 {
        saida.push(Pendencia::de_campo(
            CampoNota::ClassTrib,
            "Informe o cClassTrib, com 6 dígitos (a sugestão preenche).",
        ));
    } else {
        match tabela::classificacao(&d.cclass_trib) {
            Err(_) => saida.push(Pendencia::de_campo(
                CampoNota::ClassTrib,
                "Esse cClassTrib não existe na tabela oficial.",
            )),
            Ok(c) if !c.vale_em("nfse") => saida.push(Pendencia::de_campo(
                CampoNota::ClassTrib,
                "Esse cClassTrib não vale para NFS-e na tabela oficial.",
            )),
            Ok(_) => {}
        }
    }
}

/// (a) As pendencias do preenchimento, todas de uma vez, na ordem da pagina.
#[must_use]
pub fn do_preenchimento(d: &DadosNota, ctx: &Contexto) -> Vec<Pendencia> {
    let mut saida = Vec::new();
    do_tomador(d, &mut saida);
    do_servico(d, &mut saida);
    dos_valores(d, &mut saida);
    do_iss(d, ctx, &mut saida);
    dos_federais(d, &mut saida);
    do_ibscbs(d, &mut saida);
    if saida.is_empty() && totais(d).liquido.is_some_and(|l| l < 0) {
        saida.push(Pendencia::de_campo(
            CampoNota::ValorServico,
            "O valor líquido ficou negativo: confira os descontos e as retenções.",
        ));
    }
    saida.sort_by_key(|p| p.campo.map_or(usize::MAX, posicao));
    saida
}

fn posicao(campo: CampoNota) -> usize {
    CampoNota::TODOS
        .iter()
        .position(|c| *c == campo)
        .unwrap_or(usize::MAX)
}

/// Uma linha da tabela "campo da recusa -> campo da tela". O caminho e' o campo que a biblioteca devolve em
/// `Erro::Recusa` (sem os indices `[i]`); a linha vale para o caminho exato.
pub const MAPA_DE_RECUSAS: &[(&str, CampoNota)] = &[
    ("infDPS/toma", CampoNota::TomadorDocumento),
    ("infDPS/toma/CNPJ", CampoNota::TomadorDocumento),
    ("infDPS/toma/CPF", CampoNota::TomadorDocumento),
    ("infDPS/toma/CNPJ|CPF", CampoNota::TomadorDocumento),
    ("infDPS/toma/xNome", CampoNota::TomadorNome),
    ("infDPS/toma/IM", CampoNota::TomadorIm),
    ("infDPS/toma/email", CampoNota::TomadorEmail),
    ("infDPS/toma/fone", CampoNota::TomadorFone),
    ("infDPS/toma/end/endNac/CEP", CampoNota::EnderecoCep),
    ("infDPS/toma/end/endNac/cMun", CampoNota::EnderecoMunicipio),
    ("infDPS/toma/end/xLgr", CampoNota::EnderecoLogradouro),
    ("infDPS/toma/end/nro", CampoNota::EnderecoNumero),
    ("infDPS/toma/end/xCpl", CampoNota::EnderecoComplemento),
    ("infDPS/toma/end/xBairro", CampoNota::EnderecoBairro),
    ("infDPS/serv/cServ/cTribNac", CampoNota::Ctribnac),
    ("infDPS/serv/cServ/xDescServ", CampoNota::Descricao),
    (
        "infDPS/serv/locPrest/cLocPrestacao",
        CampoNota::MunicipioPrestacao,
    ),
    ("infDPS/serv/cServ/cNBS", CampoNota::Nbs),
    ("infDPS/serv/cServ/cTribMun", CampoNota::CtribMun),
    ("infDPS/serv/cServ/cIntContrib", CampoNota::CodInterno),
    ("infDPS/valores/vServPrest/vServ", CampoNota::ValorServico),
    (
        "infDPS/valores/vDescCondIncond/vDescIncond",
        CampoNota::DescontoIncondicionado,
    ),
    (
        "infDPS/valores/vDescCondIncond/vDescCond",
        CampoNota::DescontoCondicionado,
    ),
    ("infDPS/dCompet", CampoNota::Competencia),
    (
        "infDPS/valores/trib/tribMun/tribISSQN",
        CampoNota::Tributacao,
    ),
    (
        "infDPS/valores/trib/tribMun/tpRetISSQN",
        CampoNota::Retencao,
    ),
    ("infDPS/valores/trib/tribMun/pAliq", CampoNota::AliquotaIss),
    (
        "infDPS/valores/trib/totTrib/pTotTribSN",
        CampoNota::PercentualSn,
    ),
    (
        "infDPS/valores/trib/totTrib/pTotTrib/pTotTribFed",
        CampoNota::PercentualFederal,
    ),
    (
        "infDPS/valores/trib/totTrib/pTotTrib/pTotTribEst",
        CampoNota::PercentualEstadual,
    ),
    (
        "infDPS/valores/trib/totTrib/pTotTrib/pTotTribMun",
        CampoNota::PercentualMunicipal,
    ),
    ("infDPS/valores/trib/tribFed/vRetIRRF", CampoNota::Irrf),
    ("infDPS/valores/trib/tribFed/vRetCSLL", CampoNota::Csll),
    ("infDPS/valores/trib/tribFed/vRetCP", CampoNota::Previdencia),
    (
        "infDPS/valores/trib/tribFed/piscofins/CST",
        CampoNota::PisCst,
    ),
    (
        "infDPS/valores/trib/tribFed/piscofins/vBCPisCofins",
        CampoNota::PisBase,
    ),
    (
        "infDPS/valores/trib/tribFed/piscofins/pAliqPis",
        CampoNota::PisAliquotaPis,
    ),
    (
        "infDPS/valores/trib/tribFed/piscofins/pAliqCofins",
        CampoNota::PisAliquotaCofins,
    ),
    (
        "infDPS/valores/trib/tribFed/piscofins/tpRetPisCofins",
        CampoNota::PisTipoRetencao,
    ),
    ("infDPS/IBSCBS", CampoNota::Competencia),
    ("infDPS/IBSCBS/cIndOp", CampoNota::CindOp),
    ("infDPS/IBSCBS/indFinal", CampoNota::IndFinal),
    (
        "infDPS/IBSCBS/valores/trib/cClassTrib",
        CampoNota::ClassTrib,
    ),
    ("infDPS/IBSCBS/valores/trib/CST", CampoNota::ClassTrib),
    (
        "infDPS/IBSCBS/valores/trib/gIBSCBS/cClassTrib",
        CampoNota::ClassTrib,
    ),
    (
        "infDPS/IBSCBS/valores/trib/gIBSCBS/CST",
        CampoNota::ClassTrib,
    ),
];

/// O campo da tela de uma recusa da biblioteca. `op_simples` e' o regime do emitente, que decide de que campo e' o
/// `indTotTrib`: do percentual do Simples (ME/EPP, regra E0712) ou dos percentuais por esfera (nao optante, E0713).
#[must_use]
pub fn campo_da_recusa(caminho: &str, op_simples: u8) -> Option<CampoNota> {
    let limpo = sem_indices(caminho);
    if limpo == "infDPS/valores/trib/totTrib/indTotTrib" {
        return Some(if op_simples == 3 {
            CampoNota::PercentualSn
        } else {
            CampoNota::PercentualFederal
        });
    }
    MAPA_DE_RECUSAS
        .iter()
        .find(|(c, _)| *c == limpo)
        .map(|(_, campo)| *campo)
}

fn sem_indices(caminho: &str) -> String {
    let mut saida = String::with_capacity(caminho.len());
    let mut dentro = false;
    for c in caminho.chars() {
        match c {
            '[' => dentro = true,
            ']' => dentro = false,
            _ if !dentro => saida.push(c),
            _ => {}
        }
    }
    saida
}

/// Uma linha da tabela "codigo de rejeicao da Sefin -> campo da tela", para o que a Sefin recusa depois do envio.
/// So' entram os codigos do Anexo I que a biblioteca tambem confere antes (as mesmas regras, o mesmo campo).
pub const MAPA_DE_CODIGOS_DA_SEFIN: &[(&str, CampoNota)] = &[
    ("E0015", CampoNota::Competencia),
    ("E0202", CampoNota::TomadorDocumento),
    ("E0204", CampoNota::TomadorDocumento),
    ("E0312", CampoNota::Ctribnac),
    ("E0315", CampoNota::CtribMun),
    ("E0322", CampoNota::Nbs),
    ("E0580", CampoNota::Retencao),
    ("E0595", CampoNota::AliquotaIss),
    ("E0600", CampoNota::AliquotaIss),
    ("E0602", CampoNota::AliquotaIss),
    ("E0604", CampoNota::AliquotaIss),
    ("E0850", CampoNota::Competencia),
    ("E0901", CampoNota::CindOp),
    ("E0958", CampoNota::ClassTrib),
    ("E0959", CampoNota::ClassTrib),
];

/// O campo da tela de um codigo de rejeicao da Sefin, se o mapa o conhece.
#[must_use]
pub fn campo_do_codigo(codigo: &str) -> Option<CampoNota> {
    let codigo = codigo.trim();
    MAPA_DE_CODIGOS_DA_SEFIN
        .iter()
        .find(|(c, _)| *c == codigo)
        .map(|(_, campo)| *campo)
}

/// "texto (regra E0204 do SEFIN)" -> ("texto", Some("E0204")).
fn separar_regra(mensagem: &str) -> (String, Option<String>) {
    if let Some(i) = mensagem.rfind(" (regra ") {
        let resto = &mensagem[i + " (regra ".len()..];
        if let Some(codigo) = resto.strip_suffix(" do SEFIN)") {
            return (mensagem[..i].to_string(), Some(codigo.to_string()));
        }
    }
    (mensagem.to_string(), None)
}

/// A recusa da biblioteca como pendencia: o campo da tela (ou geral, se o mapa nao conhece o caminho), a frase e o
/// codigo da regra em segundo plano.
#[must_use]
pub fn pendencia_da_recusa(campo: &str, mensagem: &str, op_simples: u8) -> Pendencia {
    let (frase, regra) = separar_regra(mensagem);
    let frase = super::mascara::frase(&frase);
    let da_tela = campo_da_recusa(campo, op_simples);
    let mensagem = match (da_tela, campo.is_empty()) {
        (Some(_), _) | (None, true) => frase,
        (None, false) => {
            format!("{frase} Esse ponto não é um campo desta tela: se precisar dele, use a linha de comando.")
        }
    };
    Pendencia {
        campo: da_tela,
        mensagem,
        regra,
    }
}

/// (b) Confere o pedido na biblioteca: completa com o emitente, numera, converte em `Dps`, prepara e confere. `numero`
/// e' o numero provisorio da DPS (o definitivo e' o da pasta de notas, na hora de emitir).
#[must_use]
pub fn da_biblioteca(d: &DadosNota, ctx: &Contexto, numero: u64) -> Option<Pendencia> {
    let mut pedido = montar_pedido(d, ctx);
    aplicar_emitente(&mut pedido, &ctx.emitente);
    if let Some(raiz) = pedido.as_object_mut() {
        raiz.insert("numero".into(), numero.into());
    }
    let op = ctx.regime().op_simples;
    let mut dps: Dps = match serde_json::from_value(pedido) {
        Ok(dps) => dps,
        Err(_) => {
            return Some(Pendencia::geral(
                "Não foi possível montar o pedido com o que foi preenchido.",
            ))
        }
    };
    nucleo::preparar_emissao(&mut dps, ctx.ambiente.codigo(), ctx.folga_s);
    match nucleo::conferir_dps(&dps) {
        Ok(()) => None,
        Err(Erro::Recusa { campo, mensagem }) => Some(pendencia_da_recusa(&campo, &mensagem, op)),
        Err(outro) => Some(Pendencia::geral(outro.mensagem())),
    }
}

/// A conferencia completa: a propria e, se estiver limpa, a da biblioteca.
#[must_use]
pub fn conferir(d: &DadosNota, ctx: &Contexto, numero: u64) -> Conferencia {
    let pendencias = do_preenchimento(d, ctx);
    if !pendencias.is_empty() {
        return Conferencia {
            pendencias,
            da_biblioteca: false,
        };
    }
    Conferencia {
        pendencias: da_biblioteca(d, ctx, numero).into_iter().collect(),
        da_biblioteca: true,
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::modelo::pedido::testes::{config_do_caso, ctx_do_caso, nota_do_caso};
    use rustnf_app::config::Config;
    use serde_json::json;

    fn campos(p: &[Pendencia]) -> Vec<CampoNota> {
        p.iter().filter_map(|p| p.campo).collect()
    }

    #[test]
    fn o_caso_comum_passa_nas_duas_conferencias() {
        let c = conferir(&nota_do_caso(), &ctx_do_caso(), 1);
        assert!(c.limpa(), "{:?}", c.pendencias);
        assert!(c.da_biblioteca);
        assert_eq!(c.motivo(), None);
    }

    #[test]
    fn formulario_vazio_lista_todas_as_pendencias_de_uma_vez_na_ordem_da_pagina() {
        let d = DadosNota::novo("21/11/2026");
        let p = do_preenchimento(&d, &ctx_do_caso());
        assert_eq!(
            campos(&p),
            vec![
                CampoNota::TomadorDocumento,
                CampoNota::TomadorNome,
                CampoNota::Ctribnac,
                CampoNota::Descricao,
                CampoNota::Nbs,
                CampoNota::ValorServico,
                CampoNota::PercentualSn,
                CampoNota::CindOp,
                CampoNota::ClassTrib,
            ]
        );
        let c = conferir(&d, &ctx_do_caso(), 1);
        assert!(!c.da_biblioteca);
        assert_eq!(c.pendencias, p);
        assert!(c.motivo().unwrap().ends_with("(e mais 8)"));
    }

    #[test]
    fn documento_com_digito_errado_ou_tamanho_estranho() {
        let mut d = nota_do_caso();
        let ctx = ctx_do_caso();
        let casos = [
            ("52998224726", "CPF inválido"),
            ("11222333000199", "CNPJ inválido"),
            ("5299822", "O CPF tem 11 dígitos"),
            ("", "Informe o CNPJ"),
        ];
        for (doc, trecho) in casos {
            d.definir(CampoNota::TomadorDocumento, doc);
            let p = do_preenchimento(&d, &ctx);
            let achada = p
                .iter()
                .find(|p| p.campo == Some(CampoNota::TomadorDocumento))
                .unwrap_or_else(|| panic!("sem pendencia para {doc:?}"));
            assert!(
                achada.mensagem.contains(trecho),
                "{doc:?}: {}",
                achada.mensagem
            );
        }
        // sem tomador, o documento nao e' cobrado
        d.sem_tomador = true;
        assert!(do_preenchimento(&d, &ctx).is_empty());
    }

    #[test]
    fn data_invalida_valor_zero_e_desconto_maior_que_o_servico() {
        let ctx = ctx_do_caso();
        let mut d = nota_do_caso();
        d.definir(CampoNota::Competencia, "31/02/2026");
        d.definir(CampoNota::ValorServico, "0");
        let p = do_preenchimento(&d, &ctx);
        assert_eq!(
            campos(&p),
            vec![CampoNota::ValorServico, CampoNota::Competencia]
        );
        let mut d = nota_do_caso();
        d.definir(CampoNota::DescontoCondicionado, "2000");
        assert_eq!(
            campos(&do_preenchimento(&d, &ctx)),
            vec![CampoNota::DescontoCondicionado]
        );
    }

    #[test]
    fn endereco_so_e_cobrado_se_algo_foi_digitado_e_pede_o_conjunto() {
        let ctx = ctx_do_caso();
        let mut d = nota_do_caso();
        for c in [
            CampoNota::EnderecoCep,
            CampoNota::EnderecoLogradouro,
            CampoNota::EnderecoNumero,
            CampoNota::EnderecoBairro,
            CampoNota::EnderecoMunicipio,
        ] {
            d.definir(c, "");
        }
        assert!(do_preenchimento(&d, &ctx).is_empty());
        d.definir(CampoNota::EnderecoNumero, "10");
        assert_eq!(
            campos(&do_preenchimento(&d, &ctx)),
            vec![
                CampoNota::EnderecoCep,
                CampoNota::EnderecoLogradouro,
                CampoNota::EnderecoBairro,
                CampoNota::EnderecoMunicipio
            ]
        );
        d.definir(CampoNota::EnderecoMunicipio, "0000000");
        let p = do_preenchimento(&d, &ctx);
        assert!(p
            .iter()
            .any(|p| p.campo == Some(CampoNota::EnderecoMunicipio)
                && p.mensagem.contains("não encontrado")));
    }

    #[test]
    fn email_e_telefone_so_se_digitados() {
        let ctx = ctx_do_caso();
        let mut d = nota_do_caso();
        d.definir(CampoNota::TomadorEmail, "sem-arroba");
        d.definir(CampoNota::TomadorFone, "11988");
        assert_eq!(
            campos(&do_preenchimento(&d, &ctx)),
            vec![CampoNota::TomadorEmail, CampoNota::TomadorFone]
        );
        for ok in ["a@b.co", "maria.silva@exemplo.com.br"] {
            assert!(email_ok(ok), "{ok}");
        }
        for ruim in ["a@b", "@b.com", "a b@c.com", "a@@b.com", "a@.com", "a@b."] {
            assert!(!email_ok(ruim), "{ruim}");
        }
    }

    #[test]
    fn o_regime_decide_o_que_e_obrigatorio_nos_tributos() {
        let mut d = nota_do_caso();
        d.definir(CampoNota::PercentualSn, "");
        // ME/EPP: percentual do Simples
        assert_eq!(
            campos(&do_preenchimento(&d, &ctx_do_caso())),
            vec![CampoNota::PercentualSn]
        );
        // nao optante: um dos tres percentuais
        let mut config = config_do_caso();
        config.emitente.op_simples = 1;
        config.emitente.regime_apuracao = 0;
        let ctx = Contexto::de_config(&config, "2026-11-21T15:00:00Z".parse().unwrap());
        assert_eq!(
            campos(&do_preenchimento(&d, &ctx)),
            vec![CampoNota::PercentualFederal]
        );
        d.definir(CampoNota::PercentualMunicipal, "2");
        assert!(do_preenchimento(&d, &ctx).is_empty());
        // MEI: nada
        config.emitente.op_simples = 2;
        let ctx = Contexto::de_config(&config, "2026-11-21T15:00:00Z".parse().unwrap());
        d.definir(CampoNota::PercentualMunicipal, "");
        assert!(do_preenchimento(&d, &ctx).is_empty());
    }

    #[test]
    fn retencao_sem_tomador_identificado_e_aliquota_acima_do_teto() {
        let ctx = ctx_do_caso();
        let mut d = nota_do_caso();
        d.retencao = 2;
        d.sem_tomador = true;
        assert_eq!(
            campos(&do_preenchimento(&d, &ctx)),
            vec![CampoNota::Retencao]
        );
        d.sem_tomador = false;
        d.definir(CampoNota::AliquotaIss, "5,01");
        assert_eq!(
            campos(&do_preenchimento(&d, &ctx)),
            vec![CampoNota::AliquotaIss]
        );
        d.definir(CampoNota::AliquotaIss, "5");
        assert!(do_preenchimento(&d, &ctx).is_empty());
    }

    #[test]
    fn grupo_do_pis_cofins_pede_o_cst_e_o_ibscbs_confere_a_tabela() {
        let ctx = ctx_do_caso();
        let mut d = nota_do_caso();
        d.definir(CampoNota::PisBase, "100");
        assert_eq!(campos(&do_preenchimento(&d, &ctx)), vec![CampoNota::PisCst]);
        d.definir(CampoNota::PisCst, "01");
        assert!(do_preenchimento(&d, &ctx).is_empty());
        d.definir(CampoNota::CindOp, "999999");
        d.definir(CampoNota::ClassTrib, "999999");
        assert_eq!(
            campos(&do_preenchimento(&d, &ctx)),
            vec![CampoNota::CindOp, CampoNota::ClassTrib]
        );
        // com o grupo desligado, nada disso e' cobrado (nem o NBS)
        d.ibscbs_ativo = false;
        d.definir(CampoNota::Nbs, "");
        assert!(do_preenchimento(&d, &ctx).is_empty());
    }

    #[test]
    fn valor_liquido_negativo_e_pendencia() {
        let ctx = ctx_do_caso();
        let mut d = nota_do_caso();
        d.definir(CampoNota::Irrf, "5000");
        let p = do_preenchimento(&d, &ctx);
        assert_eq!(campos(&p), vec![CampoNota::ValorServico]);
        assert!(p[0].mensagem.contains("líquido"));
    }

    // ----- a conferencia da biblioteca e o mapa de campos

    /// Uma linha da tabela por vez: o caminho que a biblioteca devolve cai no campo da tela.
    #[test]
    fn mapa_de_recusas_cada_linha_cai_no_campo_certo() {
        for (caminho, campo) in MAPA_DE_RECUSAS {
            assert_eq!(campo_da_recusa(caminho, 1), Some(*campo), "linha {caminho}");
        }
        assert_eq!(MAPA_DE_RECUSAS.len(), 46);
    }

    #[test]
    fn mapa_de_recusas_nao_repete_caminho() {
        let mut vistos = std::collections::BTreeSet::new();
        for (c, _) in MAPA_DE_RECUSAS {
            assert!(vistos.insert(*c), "caminho repetido: {c}");
        }
    }

    #[test]
    fn mapa_de_recusas_indice_e_regime_e_caminho_desconhecido() {
        assert_eq!(
            campo_da_recusa("infDPS/valores/trib/totTrib/indTotTrib", 3),
            Some(CampoNota::PercentualSn)
        );
        assert_eq!(
            campo_da_recusa("infDPS/valores/trib/totTrib/indTotTrib", 1),
            Some(CampoNota::PercentualFederal)
        );
        assert_eq!(
            campo_da_recusa("infDPS/IBSCBS/gRefNFSe", 1),
            None,
            "o que a tela nao cobre fica sem campo"
        );
        assert_eq!(campo_da_recusa("infDPS/prest/IM", 1), None);
        assert_eq!(campo_da_recusa("", 1), None);
        assert_eq!(
            campo_da_recusa("infDPS/toma/end/endNac/CEP", 1),
            campo_da_recusa("infDPS/toma/end/endNac/CEP[0]", 1)
        );
    }

    #[test]
    fn mapa_de_codigos_da_sefin_cada_linha() {
        for (codigo, campo) in MAPA_DE_CODIGOS_DA_SEFIN {
            assert_eq!(campo_do_codigo(codigo), Some(*campo), "linha {codigo}");
            assert_eq!(campo_do_codigo(&format!(" {codigo} ")), Some(*campo));
        }
        assert_eq!(campo_do_codigo("E9999"), None);
        assert_eq!(campo_do_codigo(""), None);
    }

    /// Recusas de verdade da biblioteca (sem passar pela conferencia propria) caem no campo que a tabela promete.
    #[test]
    fn recusas_reais_da_biblioteca_caem_no_campo_da_tela() {
        let ctx = ctx_do_caso();
        type Ajuste = Box<dyn Fn(&mut DadosNota)>;
        let casos: Vec<(&str, Ajuste, CampoNota, &str)> = vec![
            (
                "competencia depois da emissao",
                Box::new(|d| {
                    d.definir(CampoNota::Competencia, "21/12/2099");
                }),
                CampoNota::Competencia,
                "E0015",
            ),
            (
                "tomador igual ao prestador",
                Box::new(|d| {
                    d.definir(CampoNota::TomadorDocumento, "11222333000181");
                }),
                CampoNota::TomadorDocumento,
                "E0202",
            ),
            (
                "cClassTrib que nao existe",
                Box::new(|d| {
                    d.cclass_trib = "999999".into();
                }),
                CampoNota::ClassTrib,
                "",
            ),
            (
                "retencao sem documento",
                Box::new(|d| {
                    d.retencao = 2;
                    d.tomador_documento.clear();
                }),
                CampoNota::TomadorDocumento,
                "E0204",
            ),
        ];
        for (nome, ajuste, esperado, regra) in casos {
            let mut d = nota_do_caso();
            ajuste(&mut d);
            let p = da_biblioteca(&d, &ctx, 1).unwrap_or_else(|| panic!("{nome}: sem recusa"));
            assert_eq!(p.campo, Some(esperado), "{nome}: {p:?}");
            if !regra.is_empty() {
                assert_eq!(p.regra.as_deref(), Some(regra), "{nome}");
                assert!(!p.mensagem.contains("regra E"), "{nome}: {}", p.mensagem);
            }
        }
    }

    #[test]
    fn recusa_de_campo_desconhecido_vira_pendencia_geral_com_aviso() {
        let p = pendencia_da_recusa(
            "infDPS/IBSCBS/gRefNFSe",
            "com tpOper 2 ou 3, informe as notas referenciadas (gRefNFSe) (regra E0905 do SEFIN)",
            3,
        );
        assert_eq!(p.campo, None);
        assert_eq!(p.regra.as_deref(), Some("E0905"));
        assert!(p.mensagem.contains("linha de comando"));
        let vazia = pendencia_da_recusa("", "falha qualquer", 1);
        assert_eq!(vazia.campo, None);
        assert_eq!(vazia.mensagem, "Falha qualquer");
        assert_eq!(vazia.regra, None);
    }

    #[test]
    fn regra_e_separada_so_no_formato_da_biblioteca() {
        assert_eq!(
            separar_regra("frase (regra E0204 do SEFIN)"),
            ("frase".into(), Some("E0204".into()))
        );
        assert_eq!(separar_regra("frase (regra"), ("frase (regra".into(), None));
        assert_eq!(separar_regra("so frase"), ("so frase".into(), None));
    }

    #[test]
    fn pedido_que_nao_vira_dps_e_pendencia_geral() {
        // um `Contexto` com emitente sem nada: a biblioteca recusa pelo prestador (nao e' campo desta tela)
        let ctx = Contexto::de_config(&Config::default(), "2026-11-21T15:00:00Z".parse().unwrap());
        let p = da_biblioteca(&nota_do_caso(), &ctx, 1).unwrap();
        assert_eq!(p.campo, None);
        let _ = json!(null);
    }

    #[test]
    fn a_conferencia_nao_trava_a_digitacao() {
        // meca: a conferencia completa (propria + biblioteca) de uma nota cheia
        let d = nota_do_caso();
        let ctx = ctx_do_caso();
        let n = 200;
        let t0 = std::time::Instant::now();
        for _ in 0..n {
            assert!(conferir(&d, &ctx, 1).limpa());
        }
        let media = t0.elapsed() / n;
        println!("conferencia completa: {media:?} por chamada");
        assert!(media.as_millis() < 20, "{media:?}");
    }
}
