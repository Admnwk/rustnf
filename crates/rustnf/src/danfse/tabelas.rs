// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Tabelas de dominio do leiaute da NFS-e (XSD v1.01) e os textos fixos do DANFSe (NT 008).

use super::formato::traco;

pub(super) fn tab(v: &[(i64, &str)], k: i64) -> String {
    v.iter()
        .find(|x| x.0 == k)
        .map_or("-".into(), |x| x.1.to_string())
}

pub(super) const TRIB_ISSQN: [(i64, &str); 4] = [
    (1, "Operação Tributável"),
    (2, "Imunidade"),
    (3, "Exportação de Serviço"),
    (4, "Não Incidência"),
];
pub(super) const OP_SIMPLES: [(i64, &str); 3] = [
    (1, "1 - Não Optante"),
    (2, "2 - Optante - MEI"),
    (3, "3 - Optante - ME/EPP"),
];
pub(super) const SUSPENSAO: [(i64, &str); 2] = [
    (1, "Exigibilidade Suspensa por Decisão Judicial"),
    (2, "Exigibilidade Suspensa por Processo Administrativo"),
];
pub(super) const BENEFICIO: [(i64, &str); 4] = [
    (1, "Isenção"),
    (2, "Redução da BC em 'ppBM' %'"),
    (3, "Redução da BC em R$ 'vInfoBM'"),
    (4, "Alíquota Diferenciada de 'aliqDifBM' %"),
];
pub(super) const RET_ISSQN: [(i64, &str); 3] = [
    (1, "Não Retido"),
    (2, "Retido pelo Tomador"),
    (3, "Retido pelo Intermediário"),
];
pub(super) const RET_PISCOFINS: [(i64, &str); 10] = [
    (0, "PIS/COFINS/CSLL Não Retidos"),
    (1, "PIS/COFINS Retidos"),
    (2, "PIS/COFINS Não Retidos"),
    (3, "PIS/COFINS/CSLL Retidos"),
    (4, "PIS/COFINS Retidos, CSLL Não Retido"),
    (5, "PIS Retido, COFINS/CSLL Não Retido"),
    (6, "COFINS Retido, PIS/CSLL Não Retido"),
    (7, "PIS Não Retido, COFINS/CSLL Retidos"),
    (8, "PIS/COFINS Não Retidos, CSLL Retido"),
    (9, "COFINS Não Retido, PIS/CSLL Retidos"),
];
pub(super) const IMUNIDADE: [(i64, &str); 5] = [
    (1, "Patrimônio, renda ou serviços, uns dos outros (CF88, Art 150, VI, a)"),
    (2, "Templos de qualquer culto (CF88, Art 150, VI, b)"),
    (3, "Patrimônio, renda ou serviços dos partidos políticos, inclusive suas fundações, das entidades sindicais dos \
         trabalhadores, das instituições de educação e de assistência social, sem fins lucrativos, atendidos os \
         requisitos da lei (CF88, Art 150, VI, c)"),
    (4, "Livros, jornais, periódicos e o papel destinado a sua impressão (CF88, Art 150, VI, d)"),
    (5, "Fonogramas e videofonogramas musicais produzidos no Brasil contendo obras musicais ou literomusicais de \
         autores brasileiros e/ou obras em geral interpretadas por artistas brasileiros bem como os suportes \
         materiais ou arquivos digitais que os contenham, salvo na etapa de replicação industrial de mídias ópticas \
         de leitura a laser.   (CF88, Art 150, VI, e)"),
];
pub(super) const REG_ESP: [(i64, &str); 8] = [
    (0, "Nenhum"),
    (1, "Ato Cooperado (Cooperativa)"),
    (2, "Estimativa"),
    (3, "Microempresa Municipal"),
    (4, "Notário ou Registrador"),
    (5, "Profissional Autônomo"),
    (6, "Sociedade de Profissionais"),
    (9, "Outros"),
];
pub(super) const REG_AP_SN: [(i64, &str); 2] = [
    (
        1,
        "1 - Regime de apuração dos tributos federais e municipal pelo SN",
    ),
    (
        2,
        "2 - Regime de apuração dos tributos federais pelo SN e ISSQN por fora do SN",
    ),
];
pub(super) const TP_EMIT: [(i64, &str); 3] =
    [(1, "Prestador"), (2, "Tomador"), (3, "Intermediário")];
pub(super) const FIN_NFSE: [(i64, &str); 3] = [
    (0, "NFS-e regular"),
    (1, "NFS-e de substituição"),
    (2, "NFS-e de ajuste"),
];
pub(super) const TP_AMB: [(i64, &str); 2] = [(1, "Produção"), (2, "Homologação")];
pub(super) const SEM_TOMADOR: &str = "TOMADOR/ADQUIRENTE DA OPERAÇÃO NÃO IDENTIFICADO NA NFS-e";
pub(super) const SEM_DESTINATARIO: &str = "DESTINATÁRIO DA OPERAÇÃO NÃO IDENTIFICADO NA NFS-e";
pub(super) const DEST_TOMADOR: &str = "O DESTINATÁRIO É O PRÓPRIO TOMADOR/ADQUIRENTE DA OPERAÇÃO";
pub(super) const SEM_INTERMEDIARIO: &str = "INTERMEDIÁRIO DA OPERAÇÃO NÃO IDENTIFICADO NA NFS-e";
pub(super) const SEM_ISSQN: &str = "TRIBUTAÇÃO MUNICIPAL (ISSQN) - OPERAÇÃO NÃO SUJEITA AO ISSQN";
pub(super) const LEI_TRIBUTOS: &str = "Totais aproximados dos Tributos cfe. Lei nº 12.741/2012: ";
pub(super) const MAX_DESCRICAO: usize = 1300;
pub(super) const MAX_COMPLEMENTAR: usize = 2000;

pub(super) fn cstat(s: &str) -> String {
    match s {
        "100" => "NFS-e Gerada".into(),
        "102" => "NFS-e de Decisão Judicial".into(),
        "103" => "NFS-e Avulsa".into(),
        "107" => "NFS-e MEI".into(),
        _ => traco(s),
    }
}

pub(super) fn amb_ger(s: &str) -> String {
    match s {
        "1" => "Ambiente de Produção".into(),
        "2" => "Ambiente de Homologação".into(),
        _ => "-".into(),
    }
}
