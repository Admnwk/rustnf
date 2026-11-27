// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A secao "NFC-e (cupom do caixa)" das Configuracoes: o que se edita de `[nfce]` (ambiente, serie, `idCSC`, versao do QR
//! Code, largura do cupom, colunas do ESC/POS e o motivo da contingencia off-line), com as conferencias campo a campo, e o
//! estado do CSC no cofre (guardado ou nao, nunca o valor). Sem janela.
//!
//! O VALOR do CSC nunca entra aqui: o tipo [`EstadoCsc`] so' diz se ele esta guardado, e nenhum `Debug` deste modulo o
//! carrega.

use std::collections::BTreeMap;

use rustnf_app::config::{Config, Nfce};

use super::ambiente::Ambiente;
use super::mascara::{frase, so_digitos};

/// Os campos de texto da secao.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CampoNfce {
    /// Serie da NFC-e (outra sequencia que a da NF-e).
    Serie,
    /// O `idCSC` que a SEFAZ deu junto com o CSC.
    IdCsc,
    /// Por que a nota saiu em contingencia off-line.
    Motivo,
}

impl CampoNfce {
    /// Os campos, na ordem da tela.
    pub const TODOS: [CampoNfce; 3] = [Self::Serie, Self::IdCsc, Self::Motivo];
}

/// As larguras de bobina que o cupom tem.
pub const LARGURAS: [(u8, &str); 2] = [(80, "80 mm"), (58, "58 mm")];

/// As colunas do ESC/POS (0 = pela largura).
pub const COLUNAS: [(u8, &str); 4] = [(0, "Automático"), (48, "48"), (42, "42"), (32, "32")];

/// As versoes do QR Code.
pub const VERSOES_DO_QR: [(u8, &str); 2] = [(3, "Versão 3 (sem CSC)"), (2, "Versão 2 (com CSC)")];

/// O que a tela edita de `[nfce]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DadosNfce {
    /// Producao ou nao.
    pub producao: bool,
    /// Serie.
    pub serie: String,
    /// Versao do QR Code (2 ou 3).
    pub qr_versao: u8,
    /// `idCSC`.
    pub id_csc: String,
    /// Largura da bobina.
    pub largura: u8,
    /// Colunas do ESC/POS.
    pub colunas: u8,
    /// Motivo da contingencia off-line.
    pub motivo: String,
}

impl DadosNfce {
    /// Os valores de `[nfce]`.
    #[must_use]
    pub fn de_config(n: &Nfce) -> Self {
        Self {
            producao: n.ambiente == "producao",
            serie: n.serie.clone(),
            qr_versao: n.qr_versao,
            id_csc: n.id_csc.clone(),
            largura: n.largura_danfce,
            colunas: n.colunas_escpos,
            motivo: n.motivo_offline.clone(),
        }
    }

    fn campo(&mut self, c: CampoNfce) -> &mut String {
        match c {
            CampoNfce::Serie => &mut self.serie,
            CampoNfce::IdCsc => &mut self.id_csc,
            CampoNfce::Motivo => &mut self.motivo,
        }
    }

    /// O texto de um campo.
    #[must_use]
    pub fn texto(&self, c: CampoNfce) -> &str {
        match c {
            CampoNfce::Serie => &self.serie,
            CampoNfce::IdCsc => &self.id_csc,
            CampoNfce::Motivo => &self.motivo,
        }
    }
}

/// O texto que fica no campo depois de digitar.
#[must_use]
pub fn normalizar(c: CampoNfce, texto: &str) -> String {
    match c {
        CampoNfce::Serie => so_digitos(texto, 3),
        CampoNfce::IdCsc => so_digitos(texto, 6),
        CampoNfce::Motivo => texto.chars().take(255).collect(),
    }
}

/// O formulario da secao: o que se edita e o ultimo estado gravado.
#[derive(Debug, Clone)]
pub struct FormularioNfce {
    /// O que esta sendo editado.
    pub dados: DadosNfce,
    inicial: DadosNfce,
}

impl FormularioNfce {
    /// O formulario com os valores da configuracao.
    #[must_use]
    pub fn de_config(config: &Config) -> Self {
        let dados = DadosNfce::de_config(&config.nfce);
        Self {
            inicial: dados.clone(),
            dados,
        }
    }

    /// Muda um campo e devolve o texto normalizado (para a tela repor no campo).
    pub fn definir(&mut self, c: CampoNfce, texto: &str) -> String {
        let normal = normalizar(c, texto);
        self.dados.campo(c).clone_from(&normal);
        normal
    }

    /// Poe a secao na configuracao; o resto de `[nfce]` fica como estava.
    pub fn aplicar(&self, config: &mut Config) {
        let d = &self.dados;
        let n = &mut config.nfce;
        n.ambiente = if d.producao {
            "producao"
        } else {
            "homologacao"
        }
        .into();
        n.serie.clone_from(&d.serie);
        n.qr_versao = d.qr_versao;
        n.id_csc.clone_from(&d.id_csc);
        n.largura_danfce = d.largura;
        n.colunas_escpos = d.colunas;
        n.motivo_offline = d.motivo.trim().to_string();
    }

    /// Mudou alguma coisa desde que foi carregado ou gravado.
    #[must_use]
    pub fn alterado(&self) -> bool {
        self.dados != self.inicial
    }

    /// Marca o estado atual como o gravado.
    pub fn marcar_salvo(&mut self) {
        self.inicial = self.dados.clone();
    }

    /// Joga fora as mudancas.
    pub fn descartar(&mut self) {
        self.dados = self.inicial.clone();
    }

    /// O erro de cada campo que nao serve, em frase para quem nao e' contador.
    #[must_use]
    pub fn erros(&self) -> BTreeMap<CampoNfce, String> {
        let d = &self.dados;
        let mut e = BTreeMap::new();
        if d.serie.is_empty() {
            e.insert(
                CampoNfce::Serie,
                "Informe a série (de 0 a 999).".to_string(),
            );
        }
        if d.qr_versao == 2 && d.id_csc.is_empty() {
            e.insert(
                CampoNfce::IdCsc,
                "O QR Code versão 2 pede o número do CSC (o idCSC): a SEFAZ da sua UF o dá junto com o CSC."
                    .to_string(),
            );
        }
        let tam = d.motivo.trim().chars().count();
        if tam != 0 && !(15..=255).contains(&tam) {
            e.insert(
                CampoNfce::Motivo,
                "De 15 a 255 letras (ou deixe vazio para o texto padrão).".to_string(),
            );
        }
        e
    }

    /// Por que a secao nao pode ser gravada agora, ou `None`.
    #[must_use]
    pub fn motivo_sem_salvar(&self, config: &Config) -> Option<String> {
        if let Some((_, t)) = self.erros().into_iter().next() {
            return Some(format!("NFC-e: {t}"));
        }
        let mut c = config.clone();
        self.aplicar(&mut c);
        c.validar().err().map(|e| frase(&e.to_string()))
    }
}

/// O ambiente da NFC-e, no selo da janela (o da NFS-e e o da NF-e sao outros campos).
#[must_use]
pub fn ambiente_da_nfce(n: &Nfce) -> Ambiente {
    if n.ambiente == "producao" {
        Ambiente::Producao
    } else {
        Ambiente::Restrita
    }
}

/// O que se sabe do CSC no cofre. Nunca o valor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstadoCsc {
    /// Ainda lendo o cofre.
    Verificando,
    /// Esta guardado para este emitente, ambiente e `idCSC`.
    Guardado,
    /// Nao esta guardado.
    NaoGuardado,
    /// Falta o `idCSC` (ou o documento do emitente): sem eles nao ha onde guardar.
    SemIdCsc,
    /// O cofre esta desligado (`RUSTNF_COFRE=desligado`): o CSC nao pode ser guardado.
    SemCofre,
    /// O cofre do sistema nao respondeu.
    CofreFora,
}

impl EstadoCsc {
    /// A frase da tela.
    #[must_use]
    pub fn texto(self) -> &'static str {
        match self {
            Self::Verificando => "Conferindo o cofre...",
            Self::Guardado => "CSC guardado no cofre deste computador.",
            Self::NaoGuardado => "CSC não guardado.",
            Self::SemIdCsc => {
                "Informe o idCSC (e o CNPJ do emitente, na seção Emitente) para guardar o CSC."
            }
            Self::SemCofre => {
                "O cofre de senhas está desligado (RUSTNF_COFRE=desligado): o CSC não pode ser guardado."
            }
            Self::CofreFora => "O cofre de senhas do Windows não respondeu.",
        }
    }
}

/// Por que o caixa nao emite por causa do CSC; `None` = o QR Code nao precisa de CSC ou ele esta guardado. A versao 3 do QR
/// Code dispensa o CSC (a biblioteca monta a versao 3 sem ele); so' a versao 2 o usa.
#[must_use]
pub fn por_que_o_csc_trava_o_caixa(qr_versao: u8, estado: EstadoCsc) -> Option<String> {
    if qr_versao != 2 {
        return None;
    }
    Some(match estado {
        EstadoCsc::Guardado => return None,
        EstadoCsc::SemCofre => "O QR Code versão 2 precisa do CSC, e o cofre de senhas está desligado: o CSC não pode ser guardado, então o caixa não emite. Ligue o cofre ou use o QR Code versão 3 (sem CSC), em Configurações, NFC-e.".to_string(),
        EstadoCsc::SemIdCsc => "O QR Code versão 2 precisa do idCSC e do CSC: informe-os em Configurações, NFC-e.".to_string(),
        EstadoCsc::Verificando => "Conferindo o CSC no cofre...".to_string(),
        EstadoCsc::NaoGuardado | EstadoCsc::CofreFora => "O QR Code versão 2 precisa do CSC e ele não está guardado neste computador: guarde-o em Configurações, NFC-e (ou use o QR Code versão 3, que não usa CSC).".to_string(),
    })
}

/// O que a pessoa digitou como CSC serve para guardar (de 16 a 36 letras e numeros)? A conferencia final e' a da
/// aplicacao; esta evita o clique inutil.
#[must_use]
pub fn csc_parece_valido(texto: &str) -> bool {
    let t = texto.trim();
    (16..=36).contains(&t.len()) && t.bytes().all(|b| b.is_ascii_alphanumeric())
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn o_padrao_nao_tem_erro_e_aplicar_nao_muda_nada() {
        let c = Config::default();
        let f = FormularioNfce::de_config(&c);
        assert!(f.erros().is_empty(), "{:?}", f.erros());
        let mut c2 = c.clone();
        f.aplicar(&mut c2);
        assert_eq!(c2.nfce, c.nfce);
        assert!(!f.alterado());
    }

    #[test]
    fn a_versao_2_pede_o_id_do_csc_e_o_motivo_tem_tamanho() {
        let mut f = FormularioNfce::de_config(&Config::default());
        f.dados.qr_versao = 2;
        assert!(f.erros().contains_key(&CampoNfce::IdCsc));
        f.definir(CampoNfce::IdCsc, "0001a2");
        assert_eq!(f.dados.id_csc, "00012");
        assert!(f.erros().is_empty());
        f.definir(CampoNfce::Motivo, "curto");
        assert!(f.erros().contains_key(&CampoNfce::Motivo));
        f.definir(CampoNfce::Motivo, "Sem internet no estabelecimento");
        assert!(f.erros().is_empty());
        let mut c = Config::default();
        f.aplicar(&mut c);
        assert_eq!((c.nfce.qr_versao, c.nfce.id_csc.as_str()), (2, "00012"));
        assert!(f.motivo_sem_salvar(&Config::default()).is_none());
    }

    #[test]
    fn o_csc_so_trava_o_caixa_na_versao_2_e_o_cofre_desligado_diz_por_que() {
        assert!(por_que_o_csc_trava_o_caixa(3, EstadoCsc::SemCofre).is_none());
        assert!(por_que_o_csc_trava_o_caixa(2, EstadoCsc::Guardado).is_none());
        let t = por_que_o_csc_trava_o_caixa(2, EstadoCsc::SemCofre).unwrap();
        assert!(t.contains("cofre de senhas está desligado"), "{t}");
        assert!(por_que_o_csc_trava_o_caixa(2, EstadoCsc::NaoGuardado)
            .unwrap()
            .contains("não está guardado"));
    }

    #[test]
    fn o_csc_digitado_nao_vai_a_nenhum_debug() {
        let f = FormularioNfce::de_config(&Config::default());
        let texto = format!("{f:?} {:?}", EstadoCsc::Guardado);
        assert!(!texto.contains("ZZTESTECSC"), "{texto}");
        assert!(csc_parece_valido("ZZTESTECSC0123456789ABCDEF000000"));
        assert!(!csc_parece_valido("curto"));
    }
}
