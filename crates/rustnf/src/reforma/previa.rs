// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Previa do IBS e da CBS de um servico: a base de calculo e os valores que a Sefin Nacional vai gravar na NFS-e.
//!
//! A DPS nao leva esses valores; a Sefin Nacional os calcula ao autorizar a nota, chamando a calculadora da
//! Receita Federal (Anexo VI do leiaute, regras E1539 a E1578). Este modulo refaz a mesma conta para quem quer
//! saber o resultado antes de enviar. E' PREVIA: o que vale e' o que voltar na NFS-e autorizada.
//!
//! * [`bases`] -- base de calculo do regime regular e do Simples Nacional, como o servico
//!   `base-calculo-unificada-nfse` da calculadora oficial: valor do servico menos desconto incondicionado,
//!   ajustes por documento, ISSQN (ate 2032), PIS e COFINS (so' 2026); locacao de imovel proporcional a
//!   copropriedade.
//! * [`tributos`] -- aliquota, reducao do cClassTrib, aliquota efetiva e valor de IBS estadual, IBS municipal e
//!   CBS, como o grupo `gIBSCBS` do regime geral.
//!
//! Politica numerica (registro de decisao ADR003 da calculadora oficial,
//! <https://github.com/ReceitaFederal/calculadora-rtc-regime-regular>): conta interna exata; cada numero
//! publicado e' arredondado uma vez, com "meio para o par"; a base (2 casas) e a aliquota efetiva (4 casas)
//! publicadas sao as que entram na multiplicacao; o IBS e' a soma do estadual e do municipal ja arredondados.
//!
//! Fica de fora, com recusa explicita ([`ErroPrevia::NaoCoberto`]): compra governamental, tributacao regular e
//! cClassTrib de aliquota fixa ou uniforme setorial sem a aliquota informada. Contrato:
//! `tests/casos/reforma_previa.json`, com os casos de teste da propria calculadora oficial.

use std::fmt;

use serde::{Deserialize, Serialize, Serializer};

use super::aliquotas::{self, Publicadas};
use super::decimal::Dec;
use super::{tabela, ErroReforma};
use crate::nfse::Valor;

/// Primeiro ano do IBS e da CBS (LC 214/2025).
const ANO_INICIO: i32 = 2026;
/// PIS e COFINS deixam de reduzir a base a partir deste ano.
const ANO_FIM_PIS_COFINS: i32 = 2027;
/// O ISSQN deixa de reduzir a base a partir deste ano.
const ANO_FIM_ISSQN: i32 = 2033;

// ---------------------------------------------------------------- numeros publicados

/// Valor em centavos. Escrito com 2 casas (`"35249.00"`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Centavos(pub i64);

impl fmt::Display for Centavos {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let abs = self.0.unsigned_abs();
        let sinal = if self.0 < 0 { "-" } else { "" };
        write!(f, "{sinal}{}.{:02}", abs / 100, abs % 100)
    }
}

impl Serialize for Centavos {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

/// Percentual em pontos percentuais x 10000 (4 casas). Escrito com 2 a 4 casas (`"0.90"`, `"3.5249"`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Percentual(pub i64);

impl fmt::Display for Percentual {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let abs = self.0.unsigned_abs();
        let sinal = if self.0 < 0 { "-" } else { "" };
        let mut fracao = format!("{:04}", abs % 10_000);
        while fracao.len() > 2 && fracao.ends_with('0') {
            fracao.pop();
        }
        write!(f, "{sinal}{}.{fracao}", abs / 10_000)
    }
}

impl Serialize for Percentual {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

// ---------------------------------------------------------------- erro

/// Por que a previa nao saiu.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErroPrevia {
    /// Campo de entrada ausente, que nao e' numero, negativo ou fora da faixa.
    Entrada {
        /// Nome do campo.
        campo: &'static str,
        /// O que ha de errado com ele.
        motivo: String,
    },
    /// CST e cClassTrib recusados pela tabela oficial (inexistente, par trocado, fora da NFS-e ou da vigencia).
    Classificacao(ErroReforma),
    /// A aliquota do ano ainda nao foi publicada e nao veio na entrada.
    AliquotaNaoPublicada(String),
    /// O cClassTrib tem aliquota propria (fixa ou uniforme setorial) e ela nao veio na entrada.
    AliquotaNaoInformada {
        /// O cClassTrib.
        cclass_trib: String,
        /// O tipo de aliquota dele na tabela oficial.
        tipo: String,
    },
    /// A base de calculo ficou negativa.
    BaseNegativa,
    /// Caso que este modulo nao calcula.
    NaoCoberto(&'static str),
    /// Numero grande demais para a conta.
    Estouro,
}

impl fmt::Display for ErroPrevia {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Entrada { campo, motivo } => write!(f, "{campo}: {motivo}"),
            Self::Classificacao(e) => write!(f, "{e}"),
            Self::AliquotaNaoPublicada(motivo) => f.write_str(motivo),
            Self::AliquotaNaoInformada { cclass_trib, tipo } => write!(
                f,
                "o cClassTrib {cclass_trib} tem aliquota do tipo '{tipo}': informe as aliquotas na entrada"
            ),
            Self::BaseNegativa => f.write_str("a base de calculo do IBS/CBS ficou negativa"),
            Self::NaoCoberto(o_que) => write!(f, "a previa nao calcula {o_que}"),
            Self::Estouro => f.write_str("numero grande demais para a conta"),
        }
    }
}

impl std::error::Error for ErroPrevia {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Classificacao(e) => Some(e),
            _ => None,
        }
    }
}

impl From<ErroReforma> for ErroPrevia {
    fn from(e: ErroReforma) -> Self {
        Self::Classificacao(e)
    }
}

type Resultado<T> = Result<T, ErroPrevia>;

fn decimal(campo: &'static str, v: &Valor) -> Resultado<Dec> {
    let d = Dec::de_texto(&v.0).ok_or_else(|| ErroPrevia::Entrada {
        campo,
        motivo: "nao e' um numero decimal aceito".into(),
    })?;
    if d.negativo() {
        return Err(ErroPrevia::Entrada {
            campo,
            motivo: "nao pode ser negativo".into(),
        });
    }
    Ok(d)
}

fn exato(d: Option<Dec>) -> Resultado<Dec> {
    d.ok_or(ErroPrevia::Estouro)
}

fn centavos(d: Dec) -> Resultado<Centavos> {
    exato(d.arredondar(2))?
        .mantissa_i64()
        .map(Centavos)
        .ok_or(ErroPrevia::Estouro)
}

// ---------------------------------------------------------------- base de calculo

/// Situacao do prestador no Simples Nacional (`opSimpNac` da DPS).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(try_from = "u8")]
pub enum OpcaoSimples {
    /// 1: nao optante.
    NaoOptante,
    /// 2: microempreendedor individual.
    Mei,
    /// 3: optante (ME ou EPP).
    Optante,
    /// 4: optante com a opcao pendente.
    Pendente,
}

impl TryFrom<u8> for OpcaoSimples {
    type Error = String;

    fn try_from(v: u8) -> Result<Self, String> {
        match v {
            1 => Ok(Self::NaoOptante),
            2 => Ok(Self::Mei),
            3 => Ok(Self::Optante),
            4 => Ok(Self::Pendente),
            _ => Err(format!("op_simples: 1, 2, 3 ou 4 (veio {v})")),
        }
    }
}

/// Como o optante do Simples Nacional apura o IBS e a CBS (`regApIBSCBSSN` da DPS).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(try_from = "u8")]
pub enum RegimeApuracao {
    /// 1: IBS e CBS pelo Simples Nacional.
    PeloSimples,
    /// 2: CBS pelo Simples Nacional e IBS pelo regime regular.
    CbsPeloSimples,
    /// 3: IBS e CBS pelo regime regular.
    AmbosRegulares,
}

impl TryFrom<u8> for RegimeApuracao {
    type Error = String;

    fn try_from(v: u8) -> Result<Self, String> {
        match v {
            1 => Ok(Self::PeloSimples),
            2 => Ok(Self::CbsPeloSimples),
            3 => Ok(Self::AmbosRegulares),
            _ => Err(format!("regime_apuracao: 1, 2 ou 3 (veio {v})")),
        }
    }
}

/// Tipo de operacao, que muda a composicao da base.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Situacao {
    /// Servico comum.
    #[default]
    Comum,
    /// Salao parceiro: o repasse ao profissional parceiro (ajuste do tipo 9) sai da base do Simples Nacional.
    SalaoParceiro,
    /// Locacao, cessao onerosa ou arrendamento de imovel (subitem 99.03): base proporcional a copropriedade.
    #[serde(rename = "locacao9903")]
    Locacao9903,
}

/// Um documento de ajuste da base (`docAjusteBC` da DPS).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DocumentoAjuste {
    /// `tpAjusteBC`: 101 a 105 e 199 reduzem a base do IBS/CBS; 9 (profissional parceiro) so' a do Simples.
    pub tipo: String,
    /// Valor do ajuste aplicado.
    pub valor: Valor,
    /// Descricao, obrigatoria no tipo 199.
    pub descricao: String,
}

/// Dados da locacao de imovel (subitem 99.03).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Locacao {
    /// Percentual de copropriedade do emitente.
    pub copropriedade: Valor,
    /// Valor total da operacao.
    pub total_operacao: Valor,
    /// Desconto incondicionado total.
    pub desconto_total: Valor,
    /// Soma dos ajustes do grupo de locacao.
    pub ajustes: Valor,
}

/// Entrada de [`bases`]. Valor em texto decimal ou numero; ausente vale zero.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntradaBase {
    /// Ano do fato gerador.
    pub ano: i32,
    /// Situacao do prestador no Simples Nacional (1 a 4).
    pub op_simples: OpcaoSimples,
    /// Tipo de operacao.
    #[serde(default)]
    pub situacao: Situacao,
    /// Regime de apuracao do IBS/CBS do optante (1 a 3).
    #[serde(default)]
    pub regime_apuracao: Option<RegimeApuracao>,
    /// Valor do servico.
    #[serde(default)]
    pub servico: Valor,
    /// Desconto incondicionado.
    #[serde(default)]
    pub desconto_incondicionado: Valor,
    /// Valor do ISSQN.
    #[serde(default)]
    pub issqn: Valor,
    /// Valor do PIS.
    #[serde(default)]
    pub pis: Valor,
    /// Valor da COFINS.
    #[serde(default)]
    pub cofins: Valor,
    /// Documentos de ajuste da base.
    #[serde(default)]
    pub documentos: Vec<DocumentoAjuste>,
    /// Dados da locacao, obrigatorios quando a situacao e' `locacao9903`.
    #[serde(default)]
    pub locacao: Option<Locacao>,
}

/// Recusa de negocio apontada no calculo da base, com o codigo que a calculadora oficial usa.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Ocorrencia {
    /// Codigo (`E1534`, `BASE-NEGATIVA`, `LEIAUTE-XTP`...).
    pub codigo: &'static str,
    /// Explicacao.
    pub mensagem: String,
}

fn ocorrencia(codigo: &'static str, mensagem: impl Into<String>) -> Ocorrencia {
    Ocorrencia {
        codigo,
        mensagem: mensagem.into(),
    }
}

/// Resultado de [`bases`]. `None` = a base nao se aplica (ex.: Simples Nacional para o nao optante).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Bases {
    /// Base de calculo do IBS/CBS no regime regular (`vBC` do grupo IBSCBS da NFS-e).
    pub regime_regular: Option<Centavos>,
    /// Base de calculo no Simples Nacional (`vReceitaBrutaSN`).
    pub simples: Option<Centavos>,
    /// Observacoes que nao impedem o calculo.
    pub avisos: Vec<String>,
    /// Recusas de negocio: a base e' calculada mesmo assim, para mostrar de onde vem o problema.
    pub erros: Vec<Ocorrencia>,
}

struct TipoAjuste {
    reduz_ibscbs: bool,
    reduz_simples: bool,
    exige_descricao: bool,
}

fn tipo_ajuste(codigo: &str) -> Option<TipoAjuste> {
    let (reduz_ibscbs, reduz_simples, exige_descricao) = match codigo {
        "101" | "102" | "103" | "104" | "105" => (true, false, false),
        "199" => (true, false, true),
        "9" => (false, true, false),
        _ => return None,
    };
    Some(TipoAjuste {
        reduz_ibscbs,
        reduz_simples,
        exige_descricao,
    })
}

/// Bases de calculo de uma NFS-e: regime regular e Simples Nacional.
///
/// Recusa de negocio (ajuste maior que o servico, base negativa, locacao por optante) nao interrompe a conta:
/// vem em [`Bases::erros`]. `Err` e' so' para entrada que nao da para calcular.
pub fn bases(e: &EntradaBase) -> Resultado<Bases> {
    if e.ano < ANO_INICIO {
        return Err(ErroPrevia::Entrada {
            campo: "ano",
            motivo: format!("o IBS e a CBS comecam em {ANO_INICIO}"),
        });
    }
    let em_locacao = e.situacao == Situacao::Locacao9903;
    let locacao = match (&e.locacao, em_locacao) {
        (None, true) => {
            return Err(ErroPrevia::Entrada {
                campo: "locacao",
                motivo: "obrigatorio quando a situacao e' locacao9903".into(),
            })
        }
        (Some(l), true) => Some(l),
        _ => None,
    };
    let mut avisos = Vec::new();
    let mut erros = Vec::new();

    let optante = matches!(e.op_simples, OpcaoSimples::Optante | OpcaoSimples::Pendente);
    if optante && e.regime_apuracao.is_none() {
        erros.push(ocorrencia(
            "EXXX-REGAP",
            "regime de apuracao do IBS/CBS obrigatorio para optante do Simples Nacional",
        ));
    }
    if e.op_simples == OpcaoSimples::NaoOptante && e.regime_apuracao.is_some() {
        erros.push(ocorrencia(
            "EXXX-REGAP",
            "regime de apuracao do IBS/CBS nao se informa para nao optante do Simples Nacional",
        ));
    }
    if em_locacao && matches!(e.op_simples, OpcaoSimples::Mei | OpcaoSimples::Optante) {
        erros.push(ocorrencia(
            "EXXX-LOC-SN",
            "locacao de imovel incompativel com optante do Simples Nacional",
        ));
    }

    // documentos de ajuste: tipo desconhecido e tipo 199 sem descricao ficam fora das somas
    let mut ajustes_ibscbs = Dec::ZERO;
    let mut ajustes_parceiro = Dec::ZERO;
    for doc in &e.documentos {
        let codigo = doc.tipo.trim();
        let Some(tipo) = tipo_ajuste(codigo) else {
            avisos.push(format!(
                "tipo de ajuste '{codigo}' desconhecido: documento ignorado"
            ));
            continue;
        };
        if tipo.exige_descricao && doc.descricao.trim().is_empty() {
            erros.push(ocorrencia(
                "LEIAUTE-XTP",
                "descricao obrigatoria para o tipo de ajuste 199",
            ));
            continue;
        }
        let valor = decimal("documentos.valor", &doc.valor)?;
        if em_locacao {
            continue; // na locacao os documentos nao entram na conta
        }
        if tipo.reduz_ibscbs {
            ajustes_ibscbs = exato(ajustes_ibscbs.somar(valor))?;
        }
        if tipo.reduz_simples {
            ajustes_parceiro = exato(ajustes_parceiro.somar(valor))?;
        }
    }

    if e.op_simples == OpcaoSimples::Mei {
        avisos.push("E1302: operacao de MEI: bases de calculo nao aplicaveis".into());
        if !e.documentos.is_empty() {
            erros.push(ocorrencia(
                "E0436",
                "documentos de ajuste nao sao permitidos para MEI",
            ));
        }
        return Ok(Bases {
            regime_regular: None,
            simples: None,
            avisos,
            erros,
        });
    }

    let servico = decimal("servico", &e.servico)?;
    let desconto = decimal("desconto_incondicionado", &e.desconto_incondicionado)?;
    if ajustes_ibscbs.positivo()
        && ajustes_ibscbs
            .maior_ou_igual(servico)
            .ok_or(ErroPrevia::Estouro)?
    {
        erros.push(ocorrencia(
            "E1534",
            format!(
                "a soma dos ajustes ({}) passa do valor do servico ({}) ou e' igual a ele",
                centavos(ajustes_ibscbs)?,
                centavos(servico)?
            ),
        ));
    }
    if em_locacao && !e.documentos.is_empty() {
        avisos.push("documentos de ajuste ignorados na locacao de imovel".into());
    }
    if !em_locacao && e.locacao.is_some() {
        avisos.push("grupo de locacao ignorado fora da locacao de imovel".into());
    }

    let pis_cofins = if e.ano < ANO_FIM_PIS_COFINS {
        exato(decimal("pis", &e.pis)?.somar(decimal("cofins", &e.cofins)?))?
    } else {
        Dec::ZERO
    };
    // (servico, desconto) efetivos e o ajuste proprio da locacao, proporcionais a copropriedade
    let (servico_ef, desconto_ef, ajuste_locacao) = match locacao {
        Some(l) => {
            let fator = exato(decimal("locacao.copropriedade", &l.copropriedade)?.por_cento())?;
            let parte = |campo, v: &Valor| -> Resultado<Dec> {
                exato(fator.multiplicar(decimal(campo, v)?))
            };
            (
                parte("locacao.total_operacao", &l.total_operacao)?,
                parte("locacao.desconto_total", &l.desconto_total)?,
                parte("locacao.ajustes", &l.ajustes)?,
            )
        }
        None => (servico, desconto, Dec::ZERO),
    };

    let mut regular = exato(servico_ef.subtrair(desconto_ef))?;
    if em_locacao {
        // subitem 99.03 nao tem ISSQN: so' o ajuste da locacao sai da base
        regular = exato(regular.subtrair(ajuste_locacao))?;
    } else {
        regular = exato(regular.subtrair(ajustes_ibscbs))?;
        if e.ano < ANO_FIM_ISSQN {
            regular = exato(regular.subtrair(decimal("issqn", &e.issqn)?))?;
        }
    }
    regular = exato(regular.subtrair(pis_cofins))?;

    let simples = match (e.op_simples, e.regime_apuracao) {
        (OpcaoSimples::Optante | OpcaoSimples::Pendente, Some(regime)) => {
            let mut base = exato(servico_ef.subtrair(desconto_ef))?;
            base = exato(base.subtrair(ajustes_ibscbs))?;
            if e.situacao == Situacao::SalaoParceiro && regime != RegimeApuracao::AmbosRegulares {
                base = exato(base.subtrair(ajustes_parceiro))?;
            }
            Some(base)
        }
        _ => None,
    };
    if e.op_simples == OpcaoSimples::Pendente {
        avisos.push("opcao pelo Simples Nacional pendente: calculo feito com ressalva".into());
    }

    let regular = centavos(regular)?;
    let simples = simples.map(centavos).transpose()?;
    if regular.0 < 0 {
        erros.push(ocorrencia(
            "BASE-NEGATIVA",
            format!("base de calculo do regime regular negativa: {regular}"),
        ));
    }
    if let Some(s) = simples.filter(|s| s.0 < 0) {
        erros.push(ocorrencia(
            "BASE-NEGATIVA",
            format!("base de calculo do Simples Nacional negativa: {s}"),
        ));
    }
    Ok(Bases {
        regime_regular: Some(regular),
        simples,
        avisos,
        erros,
    })
}

// ---------------------------------------------------------------- tributos

/// Aliquotas em pontos percentuais informadas por quem chama (ex.: `"8.8"`). Campo vazio usa a do calendario.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AliquotasInformadas {
    /// IBS estadual.
    pub ibs_uf: Valor,
    /// IBS municipal.
    pub ibs_mun: Valor,
    /// CBS.
    pub cbs: Valor,
}

/// Percentuais de diferimento informados na DPS (grupo `gDif`). Campo vazio = sem diferimento naquele tributo.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Diferimento {
    /// `pDifUF`.
    pub ibs_uf: Valor,
    /// `pDifMun`.
    pub ibs_mun: Valor,
    /// `pDifCBS`.
    pub cbs: Valor,
}

fn publicadas_opcionais<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<Publicadas>, D::Error> {
    let v = Option::<serde_json::Value>::deserialize(d)?;
    Ok(v.as_ref().map(aliquotas::publicadas_de_json))
}

/// Entrada de [`tributos`].
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntradaTributos {
    /// Data do fato gerador (`AAAA-MM-DD`): decide o ano das aliquotas e a vigencia do cClassTrib.
    pub data: String,
    /// CST do IBS/CBS.
    pub cst: String,
    /// cClassTrib.
    pub cclass_trib: String,
    /// Base de calculo ([`Bases::regime_regular`]).
    pub base: Valor,
    /// Aliquotas informadas; sem elas vale o calendario da LC 214/2025.
    #[serde(default)]
    pub aliquotas: Option<AliquotasInformadas>,
    /// Aliquotas publicadas (resolucao do Senado ou lei do ente) para os anos que o calendario nao fixa.
    #[serde(default, deserialize_with = "publicadas_opcionais")]
    pub publicadas: Option<Publicadas>,
    /// O prestador e' optante do Simples Nacional.
    #[serde(default)]
    pub simples_nacional: bool,
    /// Diferimento informado na DPS.
    #[serde(default)]
    pub diferimento: Option<Diferimento>,
}

/// Parte diferida de um tributo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Diferido {
    /// Percentual de diferimento.
    pub percentual: Percentual,
    /// Valor diferido.
    pub valor: Centavos,
}

/// IBS estadual, IBS municipal ou CBS de um servico.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Tributo {
    /// Aliquota do ente (`pIBSUF`, `pIBSMun`, `pCBS`).
    pub aliquota: Percentual,
    /// Reducao de aliquota do cClassTrib (`pRedAliq`), quando ha.
    pub reducao: Option<Percentual>,
    /// Aliquota efetiva (`pAliqEfet`): a que multiplica a base.
    pub aliquota_efetiva: Percentual,
    /// Base x aliquota efetiva.
    pub calculado: Centavos,
    /// Diferimento, quando informado.
    pub diferido: Option<Diferido>,
    /// Valor devido: o calculado menos o diferido.
    pub devido: Centavos,
}

/// Resultado de [`tributos`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Tributos {
    /// Base de calculo (`vBC`).
    pub base: Centavos,
    /// IBS estadual.
    pub ibs_uf: Tributo,
    /// IBS municipal.
    pub ibs_mun: Tributo,
    /// CBS.
    pub cbs: Tributo,
    /// IBS total (`vIBSTot`): estadual + municipal, ja arredondados.
    pub ibs: Centavos,
    /// Observacoes (ex.: aliquota de teste de 2026, CST sem valores).
    pub avisos: Vec<String>,
}

fn percentual(d: Dec) -> Resultado<Percentual> {
    exato(d.arredondar(4))?
        .mantissa_i64()
        .map(Percentual)
        .ok_or(ErroPrevia::Estouro)
}

fn informado(campo: &'static str, v: Option<&Valor>) -> Resultado<Option<Dec>> {
    match v {
        Some(v) if !v.0.trim().is_empty() => Ok(Some(decimal(campo, v)?)),
        _ => Ok(None),
    }
}

/// Um tributo: `aliquota` e `reducao` em pontos percentuais x 10000, `base` ja com 2 casas.
fn tributo(
    base: Dec,
    aliquota: Percentual,
    reducao: Percentual,
    diferimento: Option<Dec>,
) -> Resultado<Tributo> {
    let p = Dec::novo(i128::from(aliquota.0), 4);
    let efetiva = if reducao.0 == 0 {
        aliquota
    } else {
        // p x (1 - reducao/100), publicada com 4 casas
        let fator = exato(
            Dec::novo(100, 0)
                .subtrair(Dec::novo(i128::from(reducao.0), 4))
                .and_then(Dec::por_cento),
        )?;
        percentual(exato(p.multiplicar(fator))?)?
    };
    let calculado_exato = exato(
        base.multiplicar(Dec::novo(i128::from(efetiva.0), 4))
            .and_then(Dec::por_cento),
    )?;
    let calculado = centavos(calculado_exato)?;
    let diferido = match diferimento {
        None => None,
        Some(p_dif) => {
            let percentual_dif = percentual(p_dif)?;
            let fator = exato(Dec::novo(i128::from(percentual_dif.0), 4).por_cento())?;
            Some(Diferido {
                percentual: percentual_dif,
                valor: centavos(exato(calculado_exato.multiplicar(fator))?)?,
            })
        }
    };
    let devido = Centavos((calculado.0 - diferido.map_or(0, |d| d.valor.0)).max(0));
    Ok(Tributo {
        aliquota,
        reducao: (reducao.0 != 0).then_some(reducao),
        aliquota_efetiva: efetiva,
        calculado,
        diferido,
        devido,
    })
}

/// IBS e CBS de um servico: aliquota do ano, reducao do cClassTrib, aliquota efetiva e valor.
pub fn tributos(e: &EntradaTributos) -> Resultado<Tributos> {
    let (cst, classe) = tabela::validar(&e.cst, &e.cclass_trib, "nfse", Some(&e.data))?;
    let base_exata = Dec::de_texto(&e.base.0).ok_or_else(|| ErroPrevia::Entrada {
        campo: "base",
        motivo: "nao e' um numero decimal aceito".into(),
    })?;
    if base_exata.negativo() {
        return Err(ErroPrevia::BaseNegativa);
    }
    let base = centavos(base_exata)?;
    let base_dec = Dec::novo(i128::from(base.0), 2);
    let mut avisos = Vec::new();

    if !cst.ind_g_ibscbs {
        avisos.push(format!(
            "o CST {} nao leva valores de IBS/CBS na nota",
            cst.cst
        ));
        return Ok(Tributos {
            base,
            ibs_uf: Tributo::default(),
            ibs_mun: Tributo::default(),
            cbs: Tributo::default(),
            ibs: Centavos(0),
            avisos,
        });
    }
    if classe.ind_g_trib_regular {
        return Err(ErroPrevia::NaoCoberto(
            "cClassTrib que exige o grupo de tributacao regular",
        ));
    }

    let dif = e.diferimento.as_ref();
    let dif_uf = informado("diferimento.ibs_uf", dif.map(|d| &d.ibs_uf))?;
    let dif_mun = informado("diferimento.ibs_mun", dif.map(|d| &d.ibs_mun))?;
    let dif_cbs = informado("diferimento.cbs", dif.map(|d| &d.cbs))?;
    if (dif_uf.is_some() || dif_mun.is_some() || dif_cbs.is_some()) && !cst.ind_g_dif {
        return Err(ErroPrevia::Entrada {
            campo: "diferimento",
            motivo: format!("o CST {} nao admite diferimento", cst.cst),
        });
    }

    let inf = e.aliquotas.as_ref();
    let inf_uf = informado("aliquotas.ibs_uf", inf.map(|a| &a.ibs_uf))?;
    let inf_mun = informado("aliquotas.ibs_mun", inf.map(|a| &a.ibs_mun))?;
    let inf_cbs = informado("aliquotas.cbs", inf.map(|a| &a.cbs))?;
    let todas_informadas = inf_uf.is_some() && inf_mun.is_some() && inf_cbs.is_some();

    // de onde vem a aliquota que nao foi informada: do calendario, zero, ou nao ha de onde tirar
    let (cal_uf, cal_mun, cal_cbs) = if todas_informadas {
        (0, 0, 0)
    } else {
        match classe.tipo_aliquota.as_str() {
            "sem_aliquota" => (0, 0, 0),
            "padrao" | "uniforme_nacional" => {
                let a = aliquotas::aliquotas(&e.data, e.publicadas.as_ref(), e.simples_nacional)?;
                if !a.confirmado {
                    return Err(ErroPrevia::AliquotaNaoPublicada(a.motivo));
                }
                avisos.extend(a.avisos.iter().cloned());
                if a.aplica {
                    (a.ibs_uf, a.ibs_mun, a.cbs)
                } else {
                    avisos.push(a.motivo.clone());
                    (0, 0, 0)
                }
            }
            outro => {
                return Err(ErroPrevia::AliquotaNaoInformada {
                    cclass_trib: classe.codigo.clone(),
                    tipo: outro.to_string(),
                })
            }
        }
    };
    let aliquota = |informada: Option<Dec>, calendario: i64| -> Resultado<Percentual> {
        match informada {
            Some(d) => percentual(d),
            None => Ok(Percentual(calendario)),
        }
    };
    let red_ibs = Percentual(classe.p_red("ibs"));
    let red_cbs = Percentual(classe.p_red("cbs"));

    let ibs_uf = tributo(base_dec, aliquota(inf_uf, cal_uf)?, red_ibs, dif_uf)?;
    let ibs_mun = tributo(base_dec, aliquota(inf_mun, cal_mun)?, red_ibs, dif_mun)?;
    let cbs = tributo(base_dec, aliquota(inf_cbs, cal_cbs)?, red_cbs, dif_cbs)?;
    let ibs = ibs_uf
        .devido
        .0
        .checked_add(ibs_mun.devido.0)
        .map(Centavos)
        .ok_or(ErroPrevia::Estouro)?;
    Ok(Tributos {
        base,
        ibs_uf,
        ibs_mun,
        cbs,
        ibs,
        avisos,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod testes {
    use super::*;

    #[test]
    fn escrita_dos_numeros_publicados() {
        assert_eq!(Centavos(3_524_900).to_string(), "35249.00");
        assert_eq!(Centavos(-131).to_string(), "-1.31");
        assert_eq!(Centavos(5).to_string(), "0.05");
        assert_eq!(Percentual(9_000).to_string(), "0.90");
        assert_eq!(Percentual(88_123).to_string(), "8.8123");
        assert_eq!(Percentual(600_000).to_string(), "60.00");
        assert_eq!(Percentual(200).to_string(), "0.02");
        assert_eq!(Percentual(0).to_string(), "0.00");
        assert_eq!(
            serde_json::to_string(&Percentual(35_249)).unwrap(),
            "\"3.5249\""
        );
    }

    #[test]
    fn diferimento_sai_do_valor_exato_uma_unica_vez() {
        // 1000,00 x 1,2345% = 12,345 -> 12,34 (meio para o par); 50% diferido = 6,1725 -> 6,17; devido 6,17
        let t = tributo(
            Dec::novo(100_000, 2),
            Percentual(12_345),
            Percentual(0),
            Some(Dec::novo(50, 0)),
        )
        .unwrap();
        assert_eq!(t.calculado, Centavos(1_234));
        assert_eq!(
            t.diferido,
            Some(Diferido {
                percentual: Percentual(500_000),
                valor: Centavos(617)
            })
        );
        assert_eq!(t.devido, Centavos(617));
        assert_eq!(t.reducao, None);
    }
}
