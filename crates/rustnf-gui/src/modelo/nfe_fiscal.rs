// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A tabela das escolhas fiscais da Nova NF-e: cada pergunta simples (para onde vai, de onde veio a mercadoria, como o
//! ICMS se comporta, como o cliente leva, como pagou) vira o termo fiscal (CFOP, CST ou CSOSN, `modFrete`, `indPres`,
//! `tPag`...). Nada aqui calcula imposto nem confere regra: a biblioteca confere. Aqui so' se ESCOLHE o termo, e cada
//! escolha tem a regra e a fonte no comentario. Onde a pergunta simples nao decide (mais de uma opcao valida), a tela
//! mostra as opcoes e a pessoa escolhe: a tabela devolve a lista, nunca uma escolha calada.
//!
//! Fontes: MOC 7.00 (Manual de Orientacao do Contribuinte, Anexo I, leiaute da NF-e 4.00), com o codigo de cada campo
//! (B11 `idDest`, B25 `indFinal`, B25a `indPres`, C21 `CRT`, E16a `indIEDest`, N12 `CST`, N12a `CSOSN`, X02 `modFrete`,
//! YA03 `tPag`); a tabela de CFOP do Portal Nacional da NF-e (embutida na biblioteca, `rustnf::cfop`), que descende do
//! Convenio s/no de 15/12/1970 (SINIEF); a Lei Complementar 123/2006, art. 13 (o Simples Nacional recolhe PIS e COFINS no
//! DAS); a Lei 9.715/1998 e a Lei 9.718/1998 (PIS e COFINS cumulativos) e as Leis 10.637/2002 e 10.833/2003 (nao
//! cumulativos).

use serde::{Deserialize, Serialize};

/// Para onde a mercadoria vai. Decide o `idDest` e o primeiro algarismo do CFOP.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Destino {
    /// Cliente na mesma UF do emitente: `idDest` 1, CFOP 5.xxx.
    DentroDoEstado,
    /// Cliente em outra UF: `idDest` 2, CFOP 6.xxx.
    OutroEstado,
    /// Cliente no exterior: `idDest` 3, CFOP 7.xxx.
    Exterior,
}

impl Destino {
    /// O destino pelas UFs (a do emitente e a do cliente); `exterior` ganha de tudo. `None` = falta a UF.
    #[must_use]
    pub fn pelas_ufs(emitente: &str, cliente: &str, exterior: bool) -> Option<Self> {
        if exterior {
            return Some(Self::Exterior);
        }
        let (e, c) = (emitente.trim(), cliente.trim());
        if e.is_empty() || c.is_empty() {
            return None;
        }
        Some(if e.eq_ignore_ascii_case(c) {
            Self::DentroDoEstado
        } else {
            Self::OutroEstado
        })
    }

    /// `idDest` (B11): 1 operacao interna, 2 interestadual, 3 exterior.
    #[must_use]
    pub fn id_dest(self) -> u8 {
        match self {
            Self::DentroDoEstado => 1,
            Self::OutroEstado => 2,
            Self::Exterior => 3,
        }
    }

    /// O primeiro algarismo do CFOP de saida: 5 dentro da UF, 6 fora dela, 7 exterior (tabela de CFOP).
    #[must_use]
    pub fn digito_do_cfop(self) -> char {
        match self {
            Self::DentroDoEstado => '5',
            Self::OutroEstado => '6',
            Self::Exterior => '7',
        }
    }

    /// Como a tela diz.
    #[must_use]
    pub fn frase(self) -> &'static str {
        match self {
            Self::DentroDoEstado => "dentro do seu estado",
            Self::OutroEstado => "para outro estado",
            Self::Exterior => "para o exterior",
        }
    }
}

/// De onde veio a mercadoria que se vende.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Origem {
    /// Fabricada pelo proprio emitente (CFOP x101).
    Fabricou,
    /// Comprada para revender (CFOP x102).
    #[default]
    Revende,
}

impl Origem {
    /// As duas, na ordem da tela.
    pub const TODAS: [Origem; 2] = [Origem::Revende, Origem::Fabricou];

    /// O texto da escolha.
    #[must_use]
    pub fn rotulo(self) -> &'static str {
        match self {
            Self::Fabricou => "Eu fabrico",
            Self::Revende => "Comprei para revender",
        }
    }
}

/// Como o ICMS da mercadoria se comporta, na linguagem de quem vende. As opcoes que valem dependem do regime
/// ([`opcoes_de_icms`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Icms {
    /// Simples Nacional: a nota sai sem destaque de ICMS (CSOSN 102).
    SemDestaque,
    /// Regime normal: a mercadoria paga ICMS na operacao (CST 00); a aliquota a pessoa informa.
    Tributado,
    /// O ICMS ja foi cobrado antes, por substituicao tributaria (CSOSN 500 ou CST 60).
    JaCobradoAntes,
    /// Isenta (CST 40).
    Isento,
    /// Nao tributada, sem imposto na operacao (CSOSN 400 ou CST 41).
    NaoTributado,
}

impl Icms {
    /// O texto da escolha.
    #[must_use]
    pub fn rotulo(self) -> &'static str {
        match self {
            Self::SemDestaque => "Sem destaque de ICMS (Simples)",
            Self::Tributado => "Paga ICMS na venda",
            Self::JaCobradoAntes => "ICMS já cobrado antes (substituição tributaria)",
            Self::Isento => "Isenta de ICMS",
            Self::NaoTributado => "Sem ICMS na operação (não tributada)",
        }
    }

    /// O codigo do grupo de ICMS para o regime: CSOSN (3 digitos) com o CRT 1 e 4, CST (2 digitos) com o 2 e o 3 (N12 e
    /// N12a; a biblioteca recusa o CST no CRT 1 e o CSOSN no 3). `None` = a opcao nao existe nesse regime.
    #[must_use]
    pub fn codigo(self, crt: u8) -> Option<&'static str> {
        let simples = matches!(crt, 1 | 4);
        match (self, simples) {
            (Self::SemDestaque, true) => Some("102"),
            (Self::JaCobradoAntes, true) => Some("500"),
            (Self::NaoTributado, true) => Some("400"),
            (Self::Tributado, false) => Some("00"),
            (Self::JaCobradoAntes, false) => Some("60"),
            (Self::Isento, false) => Some("40"),
            (Self::NaoTributado, false) => Some("41"),
            _ => None,
        }
    }
}

/// As opcoes de ICMS que valem para o regime (`CRT` da `[nfe]`), na ordem da tela; a primeira e' a de uso comum.
#[must_use]
pub fn opcoes_de_icms(crt: u8) -> &'static [Icms] {
    match crt {
        1 | 4 => &[Icms::SemDestaque, Icms::JaCobradoAntes, Icms::NaoTributado],
        2 | 3 => &[
            Icms::Tributado,
            Icms::JaCobradoAntes,
            Icms::Isento,
            Icms::NaoTributado,
        ],
        _ => &[],
    }
}

/// Os CFOP que valem para a venda, na ordem em que a tela os mostra. Vazio = a combinacao nao existe na tabela (o imposto
/// ja cobrado antes nao se aplica a quem fabrica nem a exportacao): a tela mostra o porque. Mais de um = a pergunta simples
/// nao decide e a pessoa escolhe (nunca se escolhe calado).
///
/// * dentro da UF: 5.101 (venda de producao do estabelecimento), 5.102 (venda de mercadoria adquirida ou recebida de
///   terceiros) e 5.405 (venda de mercadoria de terceiros com ST, como substituido);
/// * outra UF, cliente contribuinte (ou ainda sem resposta): 6.101, 6.102 e 6.404 (venda de mercadoria sujeita a ST cujo
///   imposto ja foi retido);
/// * outra UF, cliente NAO contribuinte (`indIEDest` 9, o consumidor final de quem vende pela internet): 6.107 (venda de
///   producao do estabelecimento, destinada a nao contribuinte) e 6.108 (venda de mercadoria adquirida ou recebida de
///   terceiros, destinada a nao contribuinte). Com ST ja retido a tabela tem 6.404 e 6.108: a pessoa escolhe;
/// * outra UF, cliente contribuinte isento de inscricao (`indIEDest` 2): a tabela nao o separa nem do contribuinte
///   (6.101, 6.102) nem do nao contribuinte (6.107, 6.108): a pessoa escolhe;
/// * exterior: 7.101 e 7.102 (exportacao).
#[must_use]
pub fn cfops_da_venda(
    destino: Destino,
    origem: Origem,
    icms: Icms,
    cliente: Option<Contribuinte>,
) -> &'static [&'static str] {
    use Destino::{DentroDoEstado, Exterior, OutroEstado};
    let st = icms == Icms::JaCobradoAntes;
    match (destino, origem, st, cliente) {
        (DentroDoEstado, Origem::Fabricou, false, _) => &["5101"],
        (DentroDoEstado, Origem::Revende, false, _) => &["5102"],
        (DentroDoEstado, Origem::Revende, true, _) => &["5405"],
        (OutroEstado, Origem::Fabricou, false, Some(Contribuinte::Nao)) => &["6107"],
        (OutroEstado, Origem::Fabricou, false, Some(Contribuinte::Isento)) => &["6101", "6107"],
        (OutroEstado, Origem::Fabricou, false, _) => &["6101"],
        (OutroEstado, Origem::Revende, false, Some(Contribuinte::Nao)) => &["6108"],
        (OutroEstado, Origem::Revende, false, Some(Contribuinte::Isento)) => &["6102", "6108"],
        (OutroEstado, Origem::Revende, false, _) => &["6102"],
        (OutroEstado, Origem::Revende, true, Some(Contribuinte::Nao)) => &["6404", "6108"],
        (OutroEstado, Origem::Revende, true, _) => &["6404"],
        (Exterior, Origem::Fabricou, false, _) => &["7101"],
        (Exterior, Origem::Revende, false, _) => &["7102"],
        _ => &[],
    }
}

/// O CFOP da venda quando a tabela tem um so'; `None` = nao existe ou ha mais de um (veja [`cfops_da_venda`]).
#[must_use]
pub fn cfop_da_venda(
    destino: Destino,
    origem: Origem,
    icms: Icms,
    cliente: Option<Contribuinte>,
) -> Option<&'static str> {
    match cfops_da_venda(destino, origem, icms, cliente) {
        [unico] => Some(unico),
        _ => None,
    }
}

/// O CFOP em palavras curtas, para a pessoa escolher entre as opcoes da tabela: o numero e o que ele diz.
#[must_use]
pub fn rotulo_do_cfop(cfop: &str) -> String {
    let texto = match cfop {
        "6101" => "venda da sua produção, a contribuinte",
        "6102" => "venda de mercadoria de terceiros, a contribuinte",
        "6107" => "venda da sua produção, a não contribuinte",
        "6108" => "venda de mercadoria de terceiros, a não contribuinte",
        "6404" => "mercadoria com ICMS já cobrado antes (substituição tributária)",
        _ => return cfop.to_string(),
    };
    format!("{cfop} ({texto})")
}

/// O que explica a combinacao que nao existe.
#[must_use]
pub fn por_que_sem_cfop(destino: Destino, origem: Origem, icms: Icms) -> Option<&'static str> {
    if !cfops_da_venda(destino, origem, icms, None).is_empty() {
        return None;
    }
    Some(if destino == Destino::Exterior {
        "A exportação não tem imposto cobrado antes: escolha outra opção de ICMS."
    } else {
        "O ICMS já cobrado antes vale para quem compra para revender, não para quem fabrica: troque a origem ou a opção de ICMS."
    })
}

/// O regime do PIS e da COFINS (`[nfe].regime_pis_cofins`), so' do regime normal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RegimePisCofins {
    /// Cumulativo: PIS 0,65% (Lei 9.715/1998) e COFINS 3% (Lei 9.718/1998).
    Cumulativo,
    /// Nao cumulativo: PIS 1,65% (Lei 10.637/2002) e COFINS 7,6% (Lei 10.833/2003).
    NaoCumulativo,
}

impl RegimePisCofins {
    /// As duas, para a tela mostrar quando a configuracao nao diz qual e'.
    pub const TODOS: [RegimePisCofins; 2] = [Self::NaoCumulativo, Self::Cumulativo];

    /// O valor da configuracao.
    #[must_use]
    pub fn da_config(texto: &str) -> Option<Self> {
        match texto.trim() {
            "cumulativo" => Some(Self::Cumulativo),
            "nao_cumulativo" => Some(Self::NaoCumulativo),
            _ => None,
        }
    }

    /// O texto da escolha.
    #[must_use]
    pub fn rotulo(self) -> &'static str {
        match self {
            Self::Cumulativo => "Cumulativo (lucro presumido)",
            Self::NaoCumulativo => "Não cumulativo (lucro real)",
        }
    }

    /// `(PIS, COFINS)` em percentual, em texto.
    #[must_use]
    pub fn aliquotas(self) -> (&'static str, &'static str) {
        match self {
            Self::Cumulativo => ("0.65", "3"),
            Self::NaoCumulativo => ("1.65", "7.6"),
        }
    }
}

/// O CST do PIS e da COFINS. Simples Nacional (CRT 1 e 4): 49, outras operacoes de saida (as contribuicoes vao no DAS,
/// LC 123/2006, art. 13). Regime normal: 01, operacao tributavel com aliquota basica, com as aliquotas do regime.
#[must_use]
pub fn cst_pis_cofins(crt: u8) -> &'static str {
    if matches!(crt, 1 | 4) {
        "49"
    } else {
        "01"
    }
}

/// Como a mercadoria chega ao cliente: decide o `modFrete` (X02) e se a nota leva os dados de uma transportadora.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Entrega {
    /// O cliente leva na hora, no balcao: sem ocorrencia de transporte (9).
    #[default]
    Balcao,
    /// O cliente busca com veiculo proprio dele (4).
    ClienteBusca,
    /// A entrega e' sua, com veiculo seu (3).
    Propria,
    /// Transportadora, com o frete por sua conta, como remetente (0, CIF).
    TransportadoraPorMinhaConta,
    /// Transportadora, com o frete por conta do cliente (1, FOB).
    TransportadoraPorContaDoCliente,
    /// Transportadora contratada por um terceiro (2).
    TransportadoraDeTerceiro,
}

impl Entrega {
    /// As opcoes, na ordem da tela.
    pub const TODAS: [Entrega; 6] = [
        Self::Balcao,
        Self::ClienteBusca,
        Self::Propria,
        Self::TransportadoraPorMinhaConta,
        Self::TransportadoraPorContaDoCliente,
        Self::TransportadoraDeTerceiro,
    ];

    /// `modFrete` (X02): 0 remetente (CIF), 1 destinatario (FOB), 2 terceiros, 3 proprio do remetente, 4 proprio do
    /// destinatario, 9 sem ocorrencia de transporte.
    #[must_use]
    pub fn mod_frete(self) -> u8 {
        match self {
            Self::Balcao => 9,
            Self::ClienteBusca => 4,
            Self::Propria => 3,
            Self::TransportadoraPorMinhaConta => 0,
            Self::TransportadoraPorContaDoCliente => 1,
            Self::TransportadoraDeTerceiro => 2,
        }
    }

    /// A entrega e' por uma transportadora: a nota leva os dados dela.
    #[must_use]
    pub fn com_transportadora(self) -> bool {
        matches!(
            self,
            Self::TransportadoraPorMinhaConta
                | Self::TransportadoraPorContaDoCliente
                | Self::TransportadoraDeTerceiro
        )
    }

    /// O texto da escolha.
    #[must_use]
    pub fn rotulo(self) -> &'static str {
        match self {
            Self::Balcao => "O cliente leva na hora (balcão)",
            Self::ClienteBusca => "O cliente busca com veiculo dele",
            Self::Propria => "Eu entrego, com veiculo meu",
            Self::TransportadoraPorMinhaConta => "Transportadora, frete por minha conta",
            Self::TransportadoraPorContaDoCliente => "Transportadora, frete por conta do cliente",
            Self::TransportadoraDeTerceiro => "Transportadora contratada por outra pessoa",
        }
    }
}

/// Como o cliente fez a compra: `indPres` (B25a).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Presenca {
    /// Presencial, no balcao ou pessoalmente (1).
    #[default]
    Presencial,
    /// Pela internet (2).
    Internet,
    /// Por telefone (3).
    Telefone,
    /// Outro canal que nao e' presencial (9).
    Outro,
}

impl Presenca {
    /// As opcoes, na ordem da tela.
    pub const TODAS: [Presenca; 4] = [
        Self::Presencial,
        Self::Internet,
        Self::Telefone,
        Self::Outro,
    ];

    /// `indPres`: 1 presencial, 2 internet, 3 teleatendimento, 9 outros nao presenciais.
    #[must_use]
    pub fn ind_pres(self) -> u8 {
        match self {
            Self::Presencial => 1,
            Self::Internet => 2,
            Self::Telefone => 3,
            Self::Outro => 9,
        }
    }

    /// `indIntermed` (B25c): a venda nao presencial sem site de terceiros leva 0 (sem intermediador); a presencial nao
    /// leva o campo.
    #[must_use]
    pub fn ind_intermed(self) -> Option<u8> {
        (self != Self::Presencial).then_some(0)
    }

    /// O texto da escolha.
    #[must_use]
    pub fn rotulo(self) -> &'static str {
        match self {
            Self::Presencial => "No balcão, pessoalmente",
            Self::Internet => "Pela internet",
            Self::Telefone => "Por telefone",
            Self::Outro => "Outro canal (não presencial)",
        }
    }
}

/// Como o cliente pagou: `tPag` (YA03). A prazo vai como boleto em parcelas (`indPag` 1 e o grupo `cobr`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Pagamento {
    /// Dinheiro (01).
    #[default]
    Dinheiro,
    /// Pix (17).
    Pix,
    /// Cartao de credito (03).
    CartaoCredito,
    /// Cartao de debito (04).
    CartaoDebito,
    /// Cheque (02).
    Cheque,
    /// Boleto, a vista (15).
    Boleto,
    /// A prazo, em parcelas (boleto, 15, com `indPag` 1).
    APrazo,
}

impl Pagamento {
    /// As opcoes, na ordem da tela.
    pub const TODAS: [Pagamento; 7] = [
        Self::Dinheiro,
        Self::Pix,
        Self::CartaoCredito,
        Self::CartaoDebito,
        Self::Cheque,
        Self::Boleto,
        Self::APrazo,
    ];

    /// `tPag`.
    #[must_use]
    pub fn t_pag(self) -> &'static str {
        match self {
            Self::Dinheiro => "01",
            Self::Cheque => "02",
            Self::CartaoCredito => "03",
            Self::CartaoDebito => "04",
            Self::Boleto | Self::APrazo => "15",
            Self::Pix => "17",
        }
    }

    /// `indPag`: 0 a vista, 1 a prazo.
    #[must_use]
    pub fn ind_pag(self) -> u8 {
        u8::from(self == Self::APrazo)
    }

    /// Cartao: o grupo `card` leva `tpIntegra` 2 (pagamento nao integrado ao sistema de automacao, o caso de quem passa
    /// a maquininha a parte).
    #[must_use]
    pub fn e_cartao(self) -> bool {
        matches!(self, Self::CartaoCredito | Self::CartaoDebito)
    }

    /// O texto da escolha.
    #[must_use]
    pub fn rotulo(self) -> &'static str {
        match self {
            Self::Dinheiro => "Dinheiro",
            Self::Pix => "Pix",
            Self::CartaoCredito => "Cartão de crédito",
            Self::CartaoDebito => "Cartão de débito",
            Self::Cheque => "Cheque",
            Self::Boleto => "Boleto, a vista",
            Self::APrazo => "A prazo, em parcelas",
        }
    }

    /// O pagamento que o XML guardado diz: `tPag` e `indPag`.
    #[must_use]
    pub fn do_xml(t_pag: &str, ind_pag: &str) -> Self {
        match (t_pag.trim(), ind_pag.trim()) {
            ("15", "1") => Self::APrazo,
            ("15", _) => Self::Boleto,
            ("17", _) => Self::Pix,
            ("03", _) => Self::CartaoCredito,
            ("04", _) => Self::CartaoDebito,
            ("02", _) => Self::Cheque,
            _ => Self::Dinheiro,
        }
    }
}

/// Quem e' o cliente para o ICMS: `indIEDest` (E16a).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Contribuinte {
    /// Contribuinte do ICMS, com inscricao estadual (1).
    ComIe,
    /// Contribuinte isento de inscricao (2).
    Isento,
    /// Nao contribuinte (9): pessoa fisica ou quem nao tem inscricao.
    Nao,
}

impl Contribuinte {
    /// As opcoes, na ordem da tela.
    pub const TODOS: [Contribuinte; 3] = [Self::ComIe, Self::Nao, Self::Isento];

    /// `indIEDest`.
    #[must_use]
    pub fn ind_ie_dest(self) -> u8 {
        match self {
            Self::ComIe => 1,
            Self::Isento => 2,
            Self::Nao => 9,
        }
    }

    /// O texto da escolha.
    #[must_use]
    pub fn rotulo(self) -> &'static str {
        match self {
            Self::ComIe => "Tem inscrição estadual (contribuinte)",
            Self::Nao => "Não tem inscrição (não contribuinte)",
            Self::Isento => "Contribuinte isento de inscrição",
        }
    }

    /// O que o XML guardado diz.
    #[must_use]
    pub fn do_xml(ind: &str) -> Option<Self> {
        match ind.trim() {
            "1" => Some(Self::ComIe),
            "2" => Some(Self::Isento),
            "9" => Some(Self::Nao),
            _ => None,
        }
    }
}

#[cfg(test)]
mod testes {
    use chrono::NaiveDate;

    use super::*;

    fn hoje() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, 27).unwrap()
    }

    #[test]
    fn o_destino_sai_das_ufs() {
        assert_eq!(
            Destino::pelas_ufs("SP", "sp", false),
            Some(Destino::DentroDoEstado)
        );
        assert_eq!(
            Destino::pelas_ufs("SP", "RJ", false),
            Some(Destino::OutroEstado)
        );
        assert_eq!(Destino::pelas_ufs("SP", "", true), Some(Destino::Exterior));
        assert_eq!(Destino::pelas_ufs("SP", "", false), None);
        assert_eq!(Destino::pelas_ufs("", "SP", false), None);
        assert_eq!(Destino::DentroDoEstado.id_dest(), 1);
        assert_eq!(Destino::OutroEstado.id_dest(), 2);
        assert_eq!(Destino::Exterior.id_dest(), 3);
    }

    #[test]
    fn todo_cfop_escolhido_existe_na_tabela_e_vale_hoje() {
        let destinos = [
            Destino::DentroDoEstado,
            Destino::OutroEstado,
            Destino::Exterior,
        ];
        let icms = [
            Icms::SemDestaque,
            Icms::Tributado,
            Icms::JaCobradoAntes,
            Icms::Isento,
            Icms::NaoTributado,
        ];
        let clientes = [
            None,
            Some(Contribuinte::ComIe),
            Some(Contribuinte::Nao),
            Some(Contribuinte::Isento),
        ];
        let mut vistos = 0;
        for d in destinos {
            for o in Origem::TODAS {
                for i in icms {
                    for cli in clientes {
                        let opcoes = cfops_da_venda(d, o, i, cli);
                        if opcoes.is_empty() {
                            assert!(por_que_sem_cfop(d, o, i).is_some());
                            continue;
                        }
                        for cfop in opcoes {
                            vistos += 1;
                            let info = rustnf::cfop::info(cfop).unwrap_or_else(|| panic!("{cfop}"));
                            assert!(rustnf::cfop::vigente(cfop, hoje()), "{cfop} não vigente");
                            assert!(
                                cfop.starts_with(d.digito_do_cfop()),
                                "{cfop} não combina com {d:?}"
                            );
                            // venda, nunca devolucao
                            assert!(!rustnf::cfop::e_devolucao(cfop), "{cfop}");
                            // e' um CFOP que se usa em NF-e (indNFe)
                            assert_eq!(info.ind_nfe, 1, "{cfop} não é de NF-e: {}", info.titulo);
                        }
                    }
                }
            }
        }
        assert!(vistos > 0);
    }

    #[test]
    fn o_cfop_da_venda_a_nao_contribuinte_de_outro_estado_e_6107_ou_6108() {
        use Contribuinte::{ComIe, Isento, Nao};
        let (outro, f, r) = (Destino::OutroEstado, Origem::Fabricou, Origem::Revende);
        // tabela do Portal da NF-e: 6107 e 6108 sao "destinada a nao contribuinte"
        assert!(rustnf::cfop::info("6107")
            .unwrap()
            .titulo
            .contains("não contribuinte"));
        assert!(rustnf::cfop::info("6108")
            .unwrap()
            .titulo
            .contains("não contribuinte"));
        assert_eq!(
            cfop_da_venda(outro, f, Icms::Tributado, Some(Nao)),
            Some("6107")
        );
        assert_eq!(
            cfop_da_venda(outro, r, Icms::Tributado, Some(Nao)),
            Some("6108")
        );
        assert_eq!(
            cfop_da_venda(outro, r, Icms::SemDestaque, Some(Nao)),
            Some("6108")
        );
        // contribuinte com IE (ou ainda sem resposta): 6101 e 6102, como antes
        assert_eq!(
            cfop_da_venda(outro, f, Icms::Tributado, Some(ComIe)),
            Some("6101")
        );
        assert_eq!(
            cfop_da_venda(outro, r, Icms::Tributado, Some(ComIe)),
            Some("6102")
        );
        assert_eq!(cfop_da_venda(outro, r, Icms::Tributado, None), Some("6102"));
        // dentro do estado e exportacao nao olham o cliente
        for cli in [None, Some(Nao), Some(Isento), Some(ComIe)] {
            assert_eq!(
                cfop_da_venda(Destino::DentroDoEstado, r, Icms::Tributado, cli),
                Some("5102")
            );
            assert_eq!(
                cfop_da_venda(Destino::Exterior, r, Icms::Tributado, cli),
                Some("7102")
            );
        }
    }

    #[test]
    fn isento_e_st_a_nao_contribuinte_mostram_as_opcoes_em_vez_de_escolher() {
        use Contribuinte::{Isento, Nao};
        let (outro, f, r) = (Destino::OutroEstado, Origem::Fabricou, Origem::Revende);
        assert_eq!(
            cfops_da_venda(outro, f, Icms::Tributado, Some(Isento)),
            ["6101", "6107"]
        );
        assert_eq!(
            cfops_da_venda(outro, r, Icms::Tributado, Some(Isento)),
            ["6102", "6108"]
        );
        assert_eq!(
            cfops_da_venda(outro, r, Icms::JaCobradoAntes, Some(Nao)),
            ["6404", "6108"]
        );
        // sem uma unica resposta, a tabela nao escolhe
        assert_eq!(cfop_da_venda(outro, r, Icms::Tributado, Some(Isento)), None);
        assert_eq!(
            cfop_da_venda(outro, r, Icms::JaCobradoAntes, Some(Nao)),
            None
        );
        // ST de quem fabrica nao existe
        assert!(cfops_da_venda(outro, f, Icms::JaCobradoAntes, Some(Nao)).is_empty());
    }

    #[test]
    fn os_cfop_da_tabela_sao_estes() {
        let c = |d, o, i| cfop_da_venda(d, o, i, Some(Contribuinte::ComIe));
        assert_eq!(
            c(Destino::DentroDoEstado, Origem::Revende, Icms::SemDestaque),
            Some("5102")
        );
        assert_eq!(
            c(Destino::DentroDoEstado, Origem::Fabricou, Icms::Tributado),
            Some("5101")
        );
        assert_eq!(
            c(Destino::OutroEstado, Origem::Revende, Icms::Tributado),
            Some("6102")
        );
        assert_eq!(
            c(Destino::OutroEstado, Origem::Revende, Icms::JaCobradoAntes),
            Some("6404")
        );
        assert_eq!(
            c(
                Destino::DentroDoEstado,
                Origem::Revende,
                Icms::JaCobradoAntes
            ),
            Some("5405")
        );
        assert_eq!(
            c(Destino::Exterior, Origem::Fabricou, Icms::NaoTributado),
            Some("7101")
        );
        assert_eq!(
            c(
                Destino::DentroDoEstado,
                Origem::Fabricou,
                Icms::JaCobradoAntes
            ),
            None
        );
        assert_eq!(
            c(Destino::Exterior, Origem::Revende, Icms::JaCobradoAntes),
            None
        );
    }

    #[test]
    fn o_icms_por_regime_usa_csosn_no_simples_e_cst_no_normal() {
        for crt in [1, 4] {
            for o in opcoes_de_icms(crt) {
                let c = o.codigo(crt).unwrap();
                assert_eq!(c.len(), 3, "CSOSN tem 3 dígitos: {c}");
            }
        }
        for crt in [2, 3] {
            for o in opcoes_de_icms(crt) {
                let c = o.codigo(crt).unwrap();
                assert_eq!(c.len(), 2, "CST tem 2 dígitos: {c}");
            }
        }
        assert_eq!(Icms::SemDestaque.codigo(1), Some("102"));
        assert_eq!(Icms::SemDestaque.codigo(3), None);
        assert_eq!(Icms::Tributado.codigo(3), Some("00"));
        assert_eq!(Icms::Tributado.codigo(1), None);
        assert_eq!(Icms::JaCobradoAntes.codigo(1), Some("500"));
        assert_eq!(Icms::JaCobradoAntes.codigo(2), Some("60"));
        assert!(opcoes_de_icms(0).is_empty());
        // a primeira opcao e' a de uso comum
        assert_eq!(opcoes_de_icms(1)[0], Icms::SemDestaque);
        assert_eq!(opcoes_de_icms(3)[0], Icms::Tributado);
    }

    #[test]
    fn pis_e_cofins_pelo_regime() {
        assert_eq!(cst_pis_cofins(1), "49");
        assert_eq!(cst_pis_cofins(4), "49");
        assert_eq!(cst_pis_cofins(3), "01");
        assert_eq!(
            RegimePisCofins::da_config("cumulativo"),
            Some(RegimePisCofins::Cumulativo)
        );
        assert_eq!(
            RegimePisCofins::da_config("nao_cumulativo"),
            Some(RegimePisCofins::NaoCumulativo)
        );
        assert_eq!(RegimePisCofins::da_config(""), None);
        assert_eq!(RegimePisCofins::Cumulativo.aliquotas(), ("0.65", "3"));
        assert_eq!(RegimePisCofins::NaoCumulativo.aliquotas(), ("1.65", "7.6"));
    }

    #[test]
    fn a_entrega_escolhe_o_mod_frete() {
        let por: Vec<(Entrega, u8, bool)> = Entrega::TODAS
            .iter()
            .map(|e| (*e, e.mod_frete(), e.com_transportadora()))
            .collect();
        assert_eq!(
            por,
            vec![
                (Entrega::Balcao, 9, false),
                (Entrega::ClienteBusca, 4, false),
                (Entrega::Propria, 3, false),
                (Entrega::TransportadoraPorMinhaConta, 0, true),
                (Entrega::TransportadoraPorContaDoCliente, 1, true),
                (Entrega::TransportadoraDeTerceiro, 2, true),
            ]
        );
    }

    #[test]
    fn a_presenca_e_o_pagamento_escolhem_os_codigos() {
        let p: Vec<(u8, Option<u8>)> = Presenca::TODAS
            .iter()
            .map(|p| (p.ind_pres(), p.ind_intermed()))
            .collect();
        assert_eq!(p, vec![(1, None), (2, Some(0)), (3, Some(0)), (9, Some(0))]);
        let t: Vec<(&str, u8, bool)> = Pagamento::TODAS
            .iter()
            .map(|p| (p.t_pag(), p.ind_pag(), p.e_cartao()))
            .collect();
        assert_eq!(
            t,
            vec![
                ("01", 0, false),
                ("17", 0, false),
                ("03", 0, true),
                ("04", 0, true),
                ("02", 0, false),
                ("15", 0, false),
                ("15", 1, false),
            ]
        );
        assert_eq!(Pagamento::do_xml("15", "1"), Pagamento::APrazo);
        assert_eq!(Pagamento::do_xml("17", "0"), Pagamento::Pix);
        assert_eq!(Pagamento::do_xml("99", ""), Pagamento::Dinheiro);
        assert_eq!(Contribuinte::ComIe.ind_ie_dest(), 1);
        assert_eq!(Contribuinte::Isento.ind_ie_dest(), 2);
        assert_eq!(Contribuinte::Nao.ind_ie_dest(), 9);
        assert_eq!(Contribuinte::do_xml("9"), Some(Contribuinte::Nao));
        assert_eq!(Contribuinte::do_xml(""), None);
    }
}
