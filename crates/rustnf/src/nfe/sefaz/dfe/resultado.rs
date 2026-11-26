// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O que a distribuicao devolve: o desfecho em palavras, o cursor novo e os documentos.

use chrono::{DateTime, FixedOffset, Offset, Utc};

use super::cursor::{Cursor, FaixaNsu, MotivoEspera, Nsu};
use super::documento::{Conteudo, Doc};

/// O desfecho de uma chamada da distribuicao.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DesfechoDfe {
    /// `distNSU` respondeu 138 e AINDA HA MAIS (`ultNSU` menor que `maxNSU`): grave e chame de novo com o cursor
    /// devolvido, sem esperar.
    Parcial,
    /// `distNSU`: a fila acabou (137, ou `ultNSU` igual a `maxNSU`). O cursor ja traz a espera de uma hora.
    EmDia,
    /// `consNSU` ou `consChNFe` respondeu 138: o documento esta em `documentos`.
    Localizado,
    /// `consNSU` ou `consChNFe` respondeu 137: nao ha documento (o NSU nao existe, ou a NF-e nao e' do interessado).
    NaoLocalizado,
    /// Nenhuma chamada saiu: o cursor diz que ainda e' cedo.
    Aguarde {
        /// Instante a partir do qual se pode chamar.
        ate: DateTime<Utc>,
        /// Por que.
        motivo: MotivoEspera,
    },
    /// 656: o Ambiente Nacional bloqueou o interessado por uma hora. O cursor ja traz a espera.
    ConsumoIndevido {
        /// Instante a partir do qual se pode chamar.
        ate: DateTime<Utc>,
    },
    /// O Ambiente Nacional recusou a consulta (qualquer `cStat` que nao e' 137, 138, 656, 108 ou 109). O cursor nao mexe.
    Rejeitada {
        /// O codigo da rejeicao.
        cstat: String,
        /// O motivo, como o Ambiente Nacional o escreveu.
        motivo: String,
    },
    /// Sem resposta que sirva (rede, 108 ou 109, retorno ilegivel, NSU fora de ordem). O cursor nao mexe e nada foi
    /// entregue: repita depois.
    Indisponivel,
}

/// O resultado de `distribuir`, `distribuir_nsu`, `distribuir_chave` e `sincronizar`.
#[derive(Debug, Clone, PartialEq)]
pub struct ResultadoDfe {
    /// O que aconteceu.
    pub desfecho: DesfechoDfe,
    /// O cursor para a aplicacao PERSISTIR (na mesma gravacao dos `documentos`). Igual ao de entrada quando nada andou.
    pub cursor: Cursor,
    /// Os documentos NOVOS, em ordem de NSU. O que veio com NSU que o cursor ja cobria nao entra (ja foi entregue).
    pub documentos: Vec<Doc>,
    /// Frase para a tela: sem URL, sem corpo da resposta, com o que fazer.
    pub frase: String,
    /// `cStat` da resposta (vazio se nao houve).
    pub cstat: String,
    /// `xMotivo` da resposta.
    pub motivo: String,
    /// Quantos lotes `sincronizar` leu (0 nas outras chamadas).
    pub lotes: usize,
    /// NSU que vieram repetidos (ja cobertos pelo cursor) e por isso nao estao em `documentos`.
    pub repetidos: Vec<Nsu>,
    /// Faixas de NSU maiores que o limite de recuperacao: ficaram FORA do cursor e o Ambiente Nacional so' guarda 90
    /// dias; confira com quem usa o mesmo interessado.
    pub irrecuperaveis: Vec<FaixaNsu>,
    /// O `retDistDFeInt` cru da ultima resposta util (vazio sem resposta).
    pub xml_retorno: String,
    /// So' para o registro (rede, o que a leitura achou de estranho...).
    pub detalhe: String,
}

impl ResultadoDfe {
    pub(super) fn novo(desfecho: DesfechoDfe, cursor: Cursor, frase: String) -> Self {
        ResultadoDfe {
            desfecho,
            cursor,
            documentos: Vec::new(),
            frase,
            cstat: String::new(),
            motivo: String::new(),
            lotes: 0,
            repetidos: Vec::new(),
            irrecuperaveis: Vec::new(),
            xml_retorno: String::new(),
            detalhe: String::new(),
        }
    }

    /// A chamada entregou documentos ou disse que nao ha (137/138): o cursor devolvido vale.
    #[must_use]
    pub fn ok(&self) -> bool {
        matches!(
            self.desfecho,
            DesfechoDfe::Parcial
                | DesfechoDfe::EmDia
                | DesfechoDfe::Localizado
                | DesfechoDfe::NaoLocalizado
        )
    }

    /// Ha mais na fila: chame `distribuir` de novo com o cursor devolvido.
    #[must_use]
    pub fn ha_mais(&self) -> bool {
        self.desfecho == DesfechoDfe::Parcial
    }

    /// Os NSU dos documentos que nao abriram ou nao foram lidos: guarde-os e releia com `distribuir_nsu`.
    #[must_use]
    pub fn ilegiveis(&self) -> Vec<Nsu> {
        self.documentos
            .iter()
            .filter(|d| matches!(d.conteudo, Conteudo::Ilegivel { .. }))
            .map(|d| d.nsu)
            .collect()
    }
}

/// A hora de Brasilia (`HH:MM`) de um instante: as regras do Ambiente Nacional contam em horas, e a frase e' para
/// quem trabalha no horario do pais.
pub(super) fn hora_de_brasilia(t: DateTime<Utc>) -> String {
    let fuso = FixedOffset::west_opt(3 * 3600).unwrap_or_else(|| Utc.fix());
    t.with_timezone(&fuso).format("%H:%M").to_string()
}
