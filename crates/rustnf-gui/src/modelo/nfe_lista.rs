// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A lista das NF-e da pasta de notas: uma linha por nota com o texto da tela (data, numero e serie, destinatario com
//! o CPF mascarado, valor e situacao), a busca e o filtro por situacao, e a escolha "NFS-e | NF-e" que a tela Notas
//! lembra de uma vez para outra. Sem janela.

use rustnf_app::notas::nfe::{Registro, Situacao};
use rustnf_app::notas::sem_acento;
use serde::{Deserialize, Serialize};

use super::mascara::{
    decimal_para_dinheiro, documento_para_resumo, iso_para_data_br, limpar_documento, so_digitos,
};

/// Qual documento a tela Notas mostra.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EscolhaDeNotas {
    /// A NFS-e (nota de servico): o padrao de quem nunca escolheu.
    #[default]
    Nfse,
    /// A NF-e (venda de mercadoria).
    Nfe,
    /// A NFC-e (o cupom do caixa).
    Nfce,
}

impl EscolhaDeNotas {
    /// As escolhas, na ordem da tela.
    pub const TODAS: [EscolhaDeNotas; 3] = [Self::Nfse, Self::Nfe, Self::Nfce];

    /// O rotulo do botao.
    #[must_use]
    pub fn rotulo(self) -> &'static str {
        match self {
            Self::Nfse => "NFS-e (serviço)",
            Self::Nfe => "NF-e (venda de mercadoria)",
            Self::Nfce => "NFC-e (cupom do caixa)",
        }
    }
}

/// O nome do arquivo das preferencias, na pasta de notas.
pub const ARQUIVO_DE_PREFERENCIAS: &str = "preferencias.json";

/// Teto de leitura do arquivo: 64 KiB (ele tem uma linha).
pub const LIMITE_DAS_PREFERENCIAS: u64 = 64 * 1024;

#[derive(Serialize, Deserialize)]
struct Preferencias {
    versao: u32,
    notas: EscolhaDeNotas,
}

/// Os bytes do arquivo das preferencias.
#[must_use]
pub fn codificar_escolha(escolha: EscolhaDeNotas) -> Vec<u8> {
    let mut texto = serde_json::to_string_pretty(&Preferencias {
        versao: 1,
        notas: escolha,
    })
    .unwrap_or_default();
    texto.push('\n');
    texto.into_bytes()
}

/// A escolha guardada; arquivo ilegivel ou de versao desconhecida e' ignorado (volta o padrao, sem erro na cara).
#[must_use]
pub fn decodificar_escolha(bytes: &[u8]) -> Option<EscolhaDeNotas> {
    let p: Preferencias = serde_json::from_slice(bytes).ok()?;
    (p.versao == 1).then_some(p.notas)
}

/// O filtro por situacao da NF-e.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FiltroNfe {
    /// Todas.
    #[default]
    Todas,
    /// So' as autorizadas.
    Autorizadas,
    /// So' as canceladas.
    Canceladas,
    /// As que ficaram sem resposta (pendentes e em contingencia EPEC).
    Pendentes,
    /// As denegadas.
    Denegadas,
}

impl FiltroNfe {
    /// Os filtros, na ordem da tela.
    pub const TODOS: [FiltroNfe; 5] = [
        Self::Todas,
        Self::Autorizadas,
        Self::Canceladas,
        Self::Pendentes,
        Self::Denegadas,
    ];

    /// O rotulo do filtro.
    #[must_use]
    pub fn rotulo(self) -> &'static str {
        match self {
            Self::Todas => "Todas",
            Self::Autorizadas => "Autorizadas",
            Self::Canceladas => "Canceladas",
            Self::Pendentes => "Sem resposta",
            Self::Denegadas => "Denegadas",
        }
    }

    fn deixa_passar(self, s: Situacao) -> bool {
        match self {
            Self::Todas => true,
            Self::Autorizadas => s == Situacao::Autorizada,
            Self::Canceladas => s == Situacao::Cancelada,
            Self::Pendentes => matches!(s, Situacao::Pendente | Situacao::Epec),
            Self::Denegadas => s == Situacao::Denegada,
        }
    }
}

/// O rotulo da situacao de uma NF-e, em palavras de quem nao e' contador.
#[must_use]
pub fn rotulo_da_situacao(s: Situacao) -> &'static str {
    match s {
        Situacao::Autorizada => "Autorizada",
        Situacao::Cancelada => "Cancelada",
        Situacao::Denegada => "Uso denegado",
        Situacao::Pendente => "Sem resposta",
        Situacao::Epec => "Contingência EPEC",
        Situacao::Inutilizada => "Inutilizada",
    }
}

/// O rotulo da nota da pasta: o da situacao, menos a NFC-e emitida off-line e ainda por transmitir, que nao e' "sem
/// resposta" (nunca foi enviada). O mesmo na lista e no detalhe.
#[must_use]
pub fn rotulo_do_registro(r: &Registro) -> &'static str {
    if r.offline && r.situacao == Situacao::Pendente {
        "Sem internet (off-line)"
    } else {
        rotulo_da_situacao(r.situacao)
    }
}

/// A frase que explica o que aconteceu com a nota quando ela exige uma acao (so' a pendente, em EPEC ou com
/// recusa na transmissao); `None` nas demais.
#[must_use]
pub fn aviso_da_situacao(r: &Registro) -> Option<String> {
    match r.situacao {
        Situacao::Pendente if r.offline && r.recusa.is_empty() => Some(
            "A NFC-e foi emitida sem internet (contingência off-line) e ainda não foi transmitida à SEFAZ. O cupom entregue ao cliente vale, mas a nota tem prazo para ser transmitida: use Retomar (ou Fechar o caixa, no Caixa)."
                .into(),
        ),
        Situacao::Pendente if !r.recusa.is_empty() => Some(format!(
            "A SEFAZ recusou esta nota na última transmissão ({}). Ela segue pendente até ser corrigida.",
            r.recusa
        )),
        Situacao::Pendente => Some(
            "A nota foi enviada, mas a resposta da SEFAZ não chegou: ela pode ou não ter sido autorizada. Use Retomar: o programa consulta esta mesma nota e, se a SEFAZ não a recebeu, reenvia a mesma, nunca uma nota nova."
                .into(),
        ),
        Situacao::Epec => Some(
            "A nota foi emitida em contingência (EPEC) e ainda não foi transmitida à SEFAZ do estado. Use Retomar para transmiti-la."
                .into(),
        ),
        Situacao::Denegada => Some(
            "A SEFAZ negou o uso desta nota: o número foi gasto e a nota não vale."
                .into(),
        ),
        _ => None,
    }
}

/// Uma NF-e da lista, com os textos prontos para a tela.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinhaNfe {
    /// A chave de acesso (44 posicoes).
    pub chave: String,
    /// A data de emissao, `DD/MM/AAAA`.
    pub data: String,
    /// O numero da nota.
    pub numero: String,
    /// A serie.
    pub serie: String,
    /// O nome do destinatario (vazio se a nota nao o identifica).
    pub destinatario: String,
    /// O documento do destinatario: CPF mascarado, CNPJ inteiro e formatado.
    pub documento: String,
    /// O valor da nota, em dinheiro brasileiro.
    pub valor: String,
    /// A situacao.
    pub situacao: Situacao,
    /// O ambiente em que a nota foi emitida: `producao` ou `homologacao`.
    pub producao: bool,
    /// O que aconteceu com a nota e o que fazer, quando exige acao.
    pub aviso: Option<String>,
    /// A NFC-e emitida em contingencia off-line, ainda por transmitir.
    pub offline: bool,
    /// O rotulo da situacao: o da tabela, ou "Sem internet" para a off-line pendente.
    pub rotulo: &'static str,
    nome: String,
    numeros: String,
    doc: String,
    dia: String,
}

impl LinhaNfe {
    /// A linha de um registro do indice.
    #[must_use]
    pub fn do_registro(r: &Registro) -> Self {
        let documento = if limpar_documento(&r.destinatario_documento).is_empty() {
            String::new()
        } else {
            documento_para_resumo(&r.destinatario_documento)
        };
        Self {
            chave: r.chave.clone(),
            data: r
                .emissao
                .get(..10)
                .map_or_else(|| r.emissao.clone(), iso_para_data_br),
            numero: r.numero.clone(),
            serie: r.serie.clone(),
            destinatario: r.destinatario_nome.trim().to_string(),
            documento,
            valor: decimal_para_dinheiro(&r.valor),
            situacao: r.situacao,
            producao: r.ambiente == "producao",
            aviso: aviso_da_situacao(r),
            offline: r.offline && r.situacao == Situacao::Pendente,
            rotulo: rotulo_do_registro(r),
            nome: sem_acento(&r.destinatario_nome),
            numeros: so_digitos(&r.numero, 9),
            doc: limpar_documento(&r.destinatario_documento),
            dia: r.emissao.get(..10).unwrap_or(&r.emissao).to_string(),
        }
    }

    /// O dia da emissao, `AAAA-MM-DD`.
    #[must_use]
    pub fn dia(&self) -> &str {
        &self.dia
    }

    fn casa(&self, palavras: &[String], chave: Option<&str>) -> bool {
        if let Some(c) = chave {
            return self.chave.contains(c);
        }
        palavras.iter().all(|p| {
            let digitos = so_digitos(p, 44);
            self.nome.contains(p.as_str())
                || (!digitos.is_empty() && (self.doc.contains(&digitos) || self.numeros == digitos))
        })
    }
}

/// A lista das NF-e da pasta, com a busca e o filtro.
#[derive(Debug, Clone, Default)]
pub struct ListaNfe {
    linhas: Vec<LinhaNfe>,
    busca: String,
    filtro: FiltroNfe,
    visiveis: Vec<usize>,
}

impl ListaNfe {
    /// A lista dos registros do indice, da nota mais recente para a mais antiga (data, depois chave).
    #[must_use]
    pub fn nova(registros: &[Registro]) -> Self {
        let mut ordem: Vec<&Registro> = registros.iter().collect();
        ordem.sort_by(|a, b| (&b.emissao, &b.chave).cmp(&(&a.emissao, &a.chave)));
        let mut l = Self {
            linhas: ordem.into_iter().map(LinhaNfe::do_registro).collect(),
            ..Self::default()
        };
        l.refazer();
        l
    }

    /// A lista nova com a busca e o filtro que a antiga tinha.
    #[must_use]
    pub fn recarregada(&self, registros: &[Registro]) -> Self {
        let mut nova = Self::nova(registros);
        nova.busca.clone_from(&self.busca);
        nova.filtro = self.filtro;
        nova.refazer();
        nova
    }

    fn refazer(&mut self) {
        let palavras: Vec<String> = sem_acento(&self.busca)
            .split_whitespace()
            .map(str::to_string)
            .collect();
        let digitos = so_digitos(&self.busca, 44);
        // uma chave colada (ou o comeco dela, 20 posicoes ou mais) acha a nota pela chave
        let chave = (digitos.len() >= 20).then_some(digitos);
        self.visiveis = self
            .linhas
            .iter()
            .enumerate()
            .filter(|(_, l)| {
                self.filtro.deixa_passar(l.situacao) && l.casa(&palavras, chave.as_deref())
            })
            .map(|(i, _)| i)
            .collect();
    }

    /// Muda o texto da busca.
    pub fn definir_busca(&mut self, busca: &str) {
        if self.busca != busca {
            self.busca = busca.to_string();
            self.refazer();
        }
    }

    /// Muda o filtro.
    pub fn definir_filtro(&mut self, filtro: FiltroNfe) {
        if self.filtro != filtro {
            self.filtro = filtro;
            self.refazer();
        }
    }

    /// O filtro.
    #[must_use]
    pub fn filtro(&self) -> FiltroNfe {
        self.filtro
    }

    /// Quantas notas ha na pasta.
    #[must_use]
    pub fn total(&self) -> usize {
        self.linhas.len()
    }

    /// Quantas passam pela busca e pelo filtro.
    #[must_use]
    pub fn quantas(&self) -> usize {
        self.visiveis.len()
    }

    /// A pasta nao tem NF-e.
    #[must_use]
    pub fn vazia(&self) -> bool {
        self.linhas.is_empty()
    }

    /// Ha notas, mas nenhuma passa pela busca e pelo filtro.
    #[must_use]
    pub fn sem_resultado(&self) -> bool {
        !self.linhas.is_empty() && self.visiveis.is_empty()
    }

    /// A linha `i` entre as que passam.
    #[must_use]
    pub fn linha(&self, i: usize) -> Option<&LinhaNfe> {
        self.visiveis.get(i).and_then(|k| self.linhas.get(*k))
    }

    /// Quantas notas estao sem resposta (pendentes ou em EPEC): o que o botao Retomar vai tratar.
    #[must_use]
    pub fn a_resolver(&self) -> usize {
        self.linhas
            .iter()
            .filter(|l| matches!(l.situacao, Situacao::Pendente | Situacao::Epec))
            .count()
    }

    /// "12 de 30 notas" ou "30 notas".
    #[must_use]
    pub fn contagem(&self) -> String {
        let (q, t) = (self.quantas(), self.total());
        let notas = |n: usize| if n == 1 { "nota" } else { "notas" };
        if q == t {
            format!("{t} {}", notas(t))
        } else {
            format!("{q} de {t} {}", notas(t))
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn a_nfce_off_line_por_transmitir_nao_e_sem_resposta() {
        let mut r = reg("k", "3", "2026-11-27", "A", "", Situacao::Pendente);
        assert_eq!(rotulo_do_registro(&r), "Sem resposta");
        r.offline = true;
        assert_eq!(rotulo_do_registro(&r), "Sem internet (off-line)");
        r.situacao = Situacao::Autorizada;
        assert_eq!(rotulo_do_registro(&r), "Autorizada");
    }

    fn reg(chave: &str, numero: &str, dia: &str, nome: &str, doc: &str, s: Situacao) -> Registro {
        Registro {
            chave: chave.into(),
            serie: "1".into(),
            numero: numero.into(),
            emissao: format!("{dia}T09:00:00-03:00"),
            valor: "1234.50".into(),
            ambiente: "homologacao".into(),
            situacao: s,
            destinatario_documento: doc.into(),
            destinatario_nome: nome.into(),
            ..Registro::default()
        }
    }

    fn tres() -> Vec<Registro> {
        vec![
            reg(
                "35261011222333000181550010000000011000000011",
                "1",
                "2026-10-01",
                "Maria da Conceição",
                "52998224725",
                Situacao::Autorizada,
            ),
            reg(
                "35261011222333000181550010000000021000000022",
                "2",
                "2026-10-03",
                "Padaria Estrela",
                "11222333000181",
                Situacao::Cancelada,
            ),
            reg(
                "35261011222333000181550010000000031000000033",
                "3",
                "2026-10-02",
                "",
                "",
                Situacao::Pendente,
            ),
        ]
    }

    #[test]
    fn a_lista_vem_da_mais_recente_e_mascara_o_cpf() {
        let l = ListaNfe::nova(&tres());
        assert_eq!(l.total(), 3);
        let n: Vec<_> = (0..3).map(|i| l.linha(i).unwrap().numero.clone()).collect();
        assert_eq!(n, ["2", "3", "1"]);
        let maria = l.linha(2).unwrap();
        assert_eq!(maria.data, "01/10/2026");
        assert_eq!(maria.valor, "1.234,50");
        assert!(maria.documento.starts_with("***."), "{}", maria.documento);
        assert!(!maria.documento.contains("52998"), "{}", maria.documento);
        assert_eq!(l.linha(0).unwrap().documento, "11.222.333/0001-81");
    }

    #[test]
    fn a_busca_acha_por_nome_sem_acento_documento_numero_e_chave() {
        let mut l = ListaNfe::nova(&tres());
        l.definir_busca("conceicao");
        assert_eq!(l.quantas(), 1);
        l.definir_busca("52998224725");
        assert_eq!(l.quantas(), 1);
        l.definir_busca("35261011222333000181550010000000021");
        assert_eq!(l.quantas(), 1);
        l.definir_busca("nada disso");
        assert!(l.sem_resultado());
    }

    #[test]
    fn o_filtro_separa_as_situacoes_e_a_pendente_tem_o_que_fazer() {
        let mut l = ListaNfe::nova(&tres());
        l.definir_filtro(FiltroNfe::Pendentes);
        assert_eq!(l.quantas(), 1);
        let p = l.linha(0).unwrap();
        assert_eq!(p.situacao, Situacao::Pendente);
        assert!(p.aviso.as_deref().unwrap().contains("Retomar"));
        assert_eq!(l.a_resolver(), 1);
        l.definir_filtro(FiltroNfe::Canceladas);
        assert_eq!(l.linha(0).unwrap().numero, "2");
        assert_eq!(l.contagem(), "1 de 3 notas");
    }

    #[test]
    fn recarregar_mantem_a_busca_e_o_filtro() {
        let mut l = ListaNfe::nova(&tres());
        l.definir_busca("padaria");
        l.definir_filtro(FiltroNfe::Canceladas);
        let nova = l.recarregada(&tres());
        assert_eq!(nova.quantas(), 1);
        assert_eq!(nova.filtro(), FiltroNfe::Canceladas);
    }

    #[test]
    fn a_escolha_de_notas_e_lembrada_e_arquivo_ruim_e_ignorado() {
        assert_eq!(EscolhaDeNotas::default(), EscolhaDeNotas::Nfse);
        let b = codificar_escolha(EscolhaDeNotas::Nfe);
        assert_eq!(decodificar_escolha(&b), Some(EscolhaDeNotas::Nfe));
        assert_eq!(decodificar_escolha(b"nao e json"), None);
        assert_eq!(
            decodificar_escolha(br#"{"versao":9,"notas":"nfe"}"#),
            None,
            "versao desconhecida"
        );
    }
}
