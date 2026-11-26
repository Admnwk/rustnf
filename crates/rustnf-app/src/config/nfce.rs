// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! `[nfce]`: o que a NFC-e (modelo 65) tem de proprio. O emitente, o endereco, a IE, o CRT, o certificado, o cofre, as
//! URLs de simulador (`[nfe.urls]`) e a pasta de notas sao os mesmos da NF-e; aqui ficam so' o ambiente e a serie do 65
//! (sao sequencias distintas das do 55), a versao do QR Code, o identificador do CSC (o VALOR do CSC nunca vai ao
//! arquivo: fica no cofre) e a forma do cupom.

use serde::{Deserialize, Serialize};

use super::{Config, ErroConfig};

/// `[nfce]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Nfce {
    /// `homologacao` (padrao) ou `producao`: o `tpAmb` da NFC-e e o ambiente da SEFAZ dela.
    pub ambiente: String,
    /// Serie da NFC-e, de 0 a 999. E' outra sequencia que a da NF-e: a serie 1 do 65 e a serie 1 do 55 numeram sozinhas.
    pub serie: String,
    /// Versao do QR Code: 3 (padrao, sem CSC) ou 2 (com o CSC que a SEFAZ da UF forneceu).
    pub qr_versao: u8,
    /// Identificador do CSC (`idCSC`), de 1 a 6 digitos. So' a versao 2 usa. O valor do CSC vai para o cofre
    /// (`rustnf nfce csc guardar`), nunca para este arquivo.
    pub id_csc: String,
    /// Largura da bobina do DANFC-e: 80 (padrao) ou 58 mm.
    pub largura_danfce: u8,
    /// Colunas do ESC/POS: 0 = pela largura (48 na de 80 mm, 32 na de 58 mm), ou 48, 42, 32.
    pub colunas_escpos: u8,
    /// Por que a nota saiu em contingencia off-line, de 15 a 255 caracteres (`xJust`); vazio = o texto padrao.
    pub motivo_offline: String,
}

impl Default for Nfce {
    fn default() -> Self {
        Self {
            ambiente: "homologacao".into(),
            serie: "1".into(),
            qr_versao: 3,
            id_csc: String::new(),
            largura_danfce: 80,
            colunas_escpos: 0,
            motivo_offline: String::new(),
        }
    }
}

/// O `xJust` da emissao off-line quando `[nfce].motivo_offline` esta vazio.
pub const MOTIVO_OFFLINE_PADRAO: &str = "Contingencia off-line: sem comunicacao com a SEFAZ";

impl Nfce {
    /// A secao e' igual ao padrao (o arquivo gravado nao a repete).
    #[must_use]
    pub fn e_padrao(&self) -> bool {
        *self == Self::default()
    }

    /// O `tpAmb`: 1 producao, 2 homologacao.
    #[must_use]
    pub fn tp_amb(&self) -> u8 {
        if self.ambiente == "producao" {
            1
        } else {
            2
        }
    }

    /// O `xJust` da contingencia off-line.
    #[must_use]
    pub fn motivo_offline(&self) -> &str {
        let m = self.motivo_offline.trim();
        if m.is_empty() {
            MOTIVO_OFFLINE_PADRAO
        } else {
            m
        }
    }
}

impl Config {
    /// A configuracao vista pela NFC-e: o mesmo emitente, certificado, endereco e URLs, com o ambiente e a serie de
    /// `[nfce]`, sem SVC (a NFC-e nao tem). As acoes da NF-e a usam sem saber que e' o 65.
    #[must_use]
    pub fn como_nfce(&self) -> Config {
        let mut c = self.clone();
        c.nfe.ambiente.clone_from(&self.nfce.ambiente);
        c.nfe.serie.clone_from(&self.nfce.serie);
        c.nfe.contingencia = "normal".into();
        c
    }
}

fn invalido(campo: &str, motivo: &str) -> ErroConfig {
    ErroConfig::invalido("nfce", campo, motivo)
}

/// Confere os valores de `[nfce]`: cada recusa aponta a secao e o campo (e nunca repete o valor lido).
pub(super) fn validar(n: &Nfce) -> Result<(), ErroConfig> {
    if !matches!(n.ambiente.as_str(), "homologacao" | "producao") {
        return Err(invalido("ambiente", "use \"homologacao\" ou \"producao\""));
    }
    if n.serie.is_empty() || n.serie.len() > 3 || !n.serie.bytes().all(|b| b.is_ascii_digit()) {
        return Err(invalido("serie", "de 0 a 999"));
    }
    if !matches!(n.qr_versao, 2 | 3) {
        return Err(invalido("qr_versao", "use 3 (sem CSC) ou 2 (com CSC)"));
    }
    let id_ok = n.id_csc.len() <= 6 && n.id_csc.bytes().all(|b| b.is_ascii_digit());
    if !id_ok {
        return Err(invalido("id_csc", "de 1 a 6 digitos (o idCSC da SEFAZ)"));
    }
    if n.qr_versao == 2 && n.id_csc.is_empty() {
        return Err(invalido(
            "id_csc",
            "o QR Code versao 2 pede o idCSC: informe-o aqui e guarde o CSC com `rustnf nfce csc guardar`, ou use qr_versao = 3",
        ));
    }
    if !matches!(n.largura_danfce, 58 | 80) {
        return Err(invalido("largura_danfce", "use 80 ou 58 (mm)"));
    }
    if !matches!(n.colunas_escpos, 0 | 32 | 42 | 48) {
        return Err(invalido(
            "colunas_escpos",
            "use 0 (automatico), 48, 42 ou 32",
        ));
    }
    let tam = n.motivo_offline.trim().chars().count();
    if tam != 0 && !(15..=255).contains(&tam) {
        return Err(invalido(
            "motivo_offline",
            "de 15 a 255 caracteres (ou vazio, para o texto padrao)",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;

    fn campo(n: &Nfce) -> String {
        match validar(n) {
            Err(ErroConfig::Invalido { secao, campo, .. }) => {
                assert_eq!(secao, "nfce");
                campo
            }
            outro => panic!("esperava Invalido, veio {outro:?}"),
        }
    }

    #[test]
    fn o_padrao_e_valido_homologacao_e_nao_e_gravado() {
        let n = Nfce::default();
        assert_eq!(validar(&n), Ok(()));
        assert_eq!((n.tp_amb(), n.qr_versao, n.largura_danfce), (2, 3, 80));
        assert!(n.e_padrao());
        let c = Config::default();
        assert!(!c.para_toml().unwrap().contains("[nfce]"));
    }

    #[test]
    fn a_secao_le_do_arquivo_e_recusa_campo_desconhecido_e_csc_no_arquivo() {
        let c = Config::de_texto(
            "[nfce]\nserie = \"7\"\nqr_versao = 2\nid_csc = \"000001\"\nlargura_danfce = 58\n",
        )
        .unwrap();
        assert_eq!(c.nfce.serie, "7");
        let d = c.como_nfce();
        assert_eq!(
            (d.nfe.serie.as_str(), d.nfe.ambiente.as_str()),
            ("7", "homologacao")
        );
        // o valor do CSC nao tem campo no arquivo
        assert!(Config::de_texto("[nfce]\ncsc = \"x\"\n").is_err());
        // a NF-e segue como estava
        assert_eq!(c.nfe.serie, "1");
    }

    #[test]
    fn cada_recusa_aponta_o_campo() {
        type Muda = Box<dyn Fn(&mut Nfce)>;
        let casos: [(&str, Muda); 8] = [
            ("ambiente", Box::new(|n| n.ambiente = "restrita".into())),
            ("serie", Box::new(|n| n.serie = "1000".into())),
            ("qr_versao", Box::new(|n| n.qr_versao = 4)),
            ("id_csc", Box::new(|n| n.id_csc = "1234567".into())),
            ("id_csc", Box::new(|n| n.qr_versao = 2)),
            ("largura_danfce", Box::new(|n| n.largura_danfce = 70)),
            ("colunas_escpos", Box::new(|n| n.colunas_escpos = 40)),
            (
                "motivo_offline",
                Box::new(|n| n.motivo_offline = "curto".into()),
            ),
        ];
        for (esperado, muda) in casos {
            let mut n = Nfce::default();
            muda(&mut n);
            assert_eq!(campo(&n), esperado);
        }
    }
}
