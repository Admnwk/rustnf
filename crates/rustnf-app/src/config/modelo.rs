// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A forma do arquivo de configuracao. Nao ha campo de senha nem de token: segredo nunca fica no arquivo.

use rustnf::assinatura::xmldsig::Algoritmo;
use serde::{Deserialize, Serialize};

/// O arquivo inteiro. Toda secao e todo campo tem padrao; campo desconhecido e' recusado.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Dados do prestador que completam o pedido da DPS.
    pub emitente: Emitente,
    /// Certificado e ambiente da Sefin Nacional.
    pub sefin: Sefin,
    /// Pasta das notas autorizadas.
    pub notas: Notas,
    /// Opcoes do DANFSe.
    pub danfse: Danfse,
    /// API HTTP e MCP.
    pub servidor: Servidor,
    /// Preferencias da interface de desktop (a linha de comando nao as usa).
    pub interface: Interface,
    /// Aliquotas publicadas por ano, para a previa do IBS/CBS.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub aliquotas: Vec<Aliquota>,
}

/// `[emitente]`: so' completa o que o pedido nao traz (nunca sobrescreve).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Emitente {
    /// CNPJ do prestador (`prestador.documento`). Use este OU `cpf`.
    pub cnpj: String,
    /// CPF do prestador (`prestador.documento`).
    pub cpf: String,
    /// Razao social ou nome (`prestador.nome`).
    pub nome: String,
    /// Inscricao municipal (`prestador.im`).
    pub inscricao_municipal: String,
    /// Codigo IBGE do municipio emissor, 7 digitos (`cmun_emissor`).
    pub municipio: String,
    /// 1 nao optante, 2 MEI, 3 ME/EPP (`prestador.op_simples`).
    pub op_simples: u8,
    /// Regime de apuracao do Simples Nacional (`prestador.apuracao_sn`); 0 = nao informar.
    pub regime_apuracao: u8,
    /// Regime especial de tributacao (`prestador.regime_especial`); 0 = nenhum.
    pub regime_especial: u8,
    /// Serie da DPS (`serie`), ate 5 digitos; vazio = a do pedido ou 1.
    pub serie: String,
}

impl Default for Emitente {
    fn default() -> Self {
        Self {
            cnpj: String::new(),
            cpf: String::new(),
            nome: String::new(),
            inscricao_municipal: String::new(),
            municipio: String::new(),
            op_simples: 1,
            regime_apuracao: 0,
            regime_especial: 0,
            serie: String::new(),
        }
    }
}

/// `[sefin]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Sefin {
    /// `producao` ou `restrita`.
    pub ambiente: String,
    /// Caminho do certificado A1 (`.pfx` ou `.p12`). A senha nao vem para ca: fica no cofre do sistema ou no
    /// ambiente.
    pub pfx: String,
    /// Tempo limite de cada chamada a Sefin, em segundos.
    pub tempo_limite_s: u64,
    /// Segundos subtraidos do relogio ao preencher o `dhEmi` que o pedido nao traz.
    pub folga_relogio_s: u32,
    /// Algoritmo da assinatura da DPS e dos eventos: `sha256` (padrao) ou `sha1`.
    pub assinatura: String,
}

impl Sefin {
    /// O algoritmo de assinatura configurado. Texto fora de `sha1` e `sha256`, que [`Config::validar`] recusa, cai no
    /// padrao.
    #[must_use]
    pub fn algoritmo(&self) -> Algoritmo {
        Algoritmo::de_nome(&self.assinatura).unwrap_or_default()
    }
}

impl Default for Sefin {
    fn default() -> Self {
        Self {
            ambiente: "restrita".into(),
            pfx: String::new(),
            tempo_limite_s: 60,
            folga_relogio_s: 60,
            assinatura: Algoritmo::default().nome().into(),
        }
    }
}

/// `[notas]`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Notas {
    /// Pasta onde guardar as notas autorizadas e a numeracao; vazio = nao guardar.
    pub pasta: String,
}

/// `[danfse]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Danfse {
    /// Canhoto de cientificacao no rodape.
    pub canhoto: bool,
}

impl Default for Danfse {
    fn default() -> Self {
        Self { canhoto: true }
    }
}

/// Os temas aceitos em `[interface].tema`.
pub const TEMAS: [&str; 3] = ["sistema", "claro", "escuro"];

/// `[interface]`: preferencias da interface de desktop.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Interface {
    /// Tema da interface: `sistema` (segue o do Windows, padrao), `claro` ou `escuro`.
    pub tema: String,
}

impl Default for Interface {
    fn default() -> Self {
        Self {
            tema: TEMAS[0].into(),
        }
    }
}

/// `[servidor]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Servidor {
    /// Endereco IP de escuta.
    pub endereco: String,
    /// Porta de escuta (0 = qualquer porta livre).
    pub porta: u16,
    /// Teto do corpo da requisicao, em bytes.
    pub limite_corpo: usize,
    /// Tempo limite de cada requisicao, em segundos.
    pub tempo_limite_s: u64,
    /// Origens extras aceitas no cabecalho `Origin`, alem do proprio computador.
    pub origens_permitidas: Vec<String>,
}

impl Default for Servidor {
    fn default() -> Self {
        Self {
            endereco: "127.0.0.1".into(),
            porta: 8080,
            limite_corpo: 1024 * 1024,
            tempo_limite_s: 60,
            origens_permitidas: Vec::new(),
        }
    }
}

/// `[[aliquotas]]`: o que o Senado ou o ente publicou para um ano (texto decimal, percentual).
/// 2027-2028: `cbs_referencia` (a biblioteca tira 0,1 ponto percentual) ou `cbs`; 2029 em diante: `cbs`, `ibs_uf`
/// e `ibs_mun`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Aliquota {
    /// Ano do fato gerador.
    pub ano: i32,
    /// CBS de referencia (2027-2028).
    pub cbs_referencia: String,
    /// CBS.
    pub cbs: String,
    /// IBS estadual.
    pub ibs_uf: String,
    /// IBS municipal.
    pub ibs_mun: String,
    /// Norma que publicou as aliquotas.
    pub fonte: String,
}

impl Default for Aliquota {
    fn default() -> Self {
        Self {
            ano: 2027,
            cbs_referencia: String::new(),
            cbs: String::new(),
            ibs_uf: String::new(),
            ibs_mun: String::new(),
            fonte: String::new(),
        }
    }
}
