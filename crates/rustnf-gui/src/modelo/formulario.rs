// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O estado dos formularios: o texto digitado em cada campo, a conversao de e para [`Config`] e a conferencia campo a
//! campo. Sem nenhuma dependencia de janela.
//!
//! A conferencia usa a mesma `Config::validar` da linha de comando: cada campo e' conferido sozinho (numa
//! configuracao padrao so' com ele preenchido), para a tela mostrar todos os erros de uma vez, e depois a
//! configuracao inteira e' conferida para pegar o que depende de mais de um campo (ano repetido, por exemplo).

use rustnf_app::cofre::chave_do_certificado;
use rustnf_app::config::{Aliquota, Config, ErroConfig};

use super::ambiente::{Ambiente, Tema};
use super::campo::{Campo, CampoAliquota, Erros};
use super::mascara::{filtrar_decimal, formatar_documento, limpar_documento, so_digitos};
use super::passos::Passo;

/// Algoritmo da assinatura (`[sefin].assinatura`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Assinatura {
    /// RSA com SHA-256 (padrao).
    Sha256,
    /// RSA com SHA-1.
    Sha1,
}

impl Assinatura {
    /// Os dois, na ordem da tela.
    pub const TODAS: [Assinatura; 2] = [Assinatura::Sha256, Assinatura::Sha1];

    /// Le o valor de `[sefin].assinatura`; texto desconhecido cai no padrao.
    #[must_use]
    pub fn de_config(texto: &str) -> Self {
        if texto == "sha1" {
            Self::Sha1
        } else {
            Self::Sha256
        }
    }

    /// O valor gravado.
    #[must_use]
    pub fn para_config(self) -> &'static str {
        match self {
            Self::Sha256 => "sha256",
            Self::Sha1 => "sha1",
        }
    }

    /// Rotulo da tela.
    #[must_use]
    pub fn rotulo(self) -> &'static str {
        match self {
            Self::Sha256 => "SHA-256 (padrão)",
            Self::Sha1 => "SHA-1",
        }
    }
}

/// As opcoes do Simples Nacional (`op_simples`).
pub const OPCOES_SIMPLES: [(u8, &str); 3] = [(1, "Não optante"), (2, "MEI"), (3, "ME/EPP")];

/// Regime de apuracao do Simples Nacional, so' para ME/EPP.
pub const OPCOES_APURACAO: [(u8, &str); 3] = [
    (1, "1 - Tributos federais e ISSQN pelo Simples"),
    (2, "2 - Federais pelo Simples, ISSQN por fora"),
    (3, "3 - Federais e ISSQN por fora do Simples"),
];

/// Regime especial de tributacao (`regime_especial`).
pub const OPCOES_REGIME_ESPECIAL: [(u8, &str); 8] = [
    (0, "Nenhum"),
    (1, "Ato cooperado (cooperativa)"),
    (2, "Estimativa"),
    (3, "Microempresa municipal"),
    (4, "Notário ou registrador"),
    (5, "Profissional autônomo"),
    (6, "Sociedade de profissionais"),
    (9, "Outros"),
];

/// Uma linha da tabela de aliquotas, com o texto de cada celula.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinhaAliquota {
    /// Ano.
    pub ano: String,
    /// CBS de referencia.
    pub cbs_referencia: String,
    /// CBS.
    pub cbs: String,
    /// IBS estadual.
    pub ibs_uf: String,
    /// IBS municipal.
    pub ibs_mun: String,
    /// Fonte.
    pub fonte: String,
}

impl LinhaAliquota {
    fn de_config(a: &Aliquota) -> Self {
        Self {
            ano: a.ano.to_string(),
            cbs_referencia: a.cbs_referencia.clone(),
            cbs: a.cbs.clone(),
            ibs_uf: a.ibs_uf.clone(),
            ibs_mun: a.ibs_mun.clone(),
            fonte: a.fonte.clone(),
        }
    }

    fn para_config(&self) -> Aliquota {
        let decimal = |t: &str| t.trim().replace(',', ".");
        Aliquota {
            ano: self.ano.trim().parse().unwrap_or(0),
            cbs_referencia: decimal(&self.cbs_referencia),
            cbs: decimal(&self.cbs),
            ibs_uf: decimal(&self.ibs_uf),
            ibs_mun: decimal(&self.ibs_mun),
            fonte: self.fonte.trim().to_string(),
        }
    }

    /// O texto de uma coluna.
    #[must_use]
    pub fn texto(&self, coluna: CampoAliquota) -> &str {
        match coluna {
            CampoAliquota::Ano => &self.ano,
            CampoAliquota::CbsReferencia => &self.cbs_referencia,
            CampoAliquota::Cbs => &self.cbs,
            CampoAliquota::IbsUf => &self.ibs_uf,
            CampoAliquota::IbsMun => &self.ibs_mun,
            CampoAliquota::Fonte => &self.fonte,
        }
    }

    fn texto_mut(&mut self, coluna: CampoAliquota) -> &mut String {
        match coluna {
            CampoAliquota::Ano => &mut self.ano,
            CampoAliquota::CbsReferencia => &mut self.cbs_referencia,
            CampoAliquota::Cbs => &mut self.cbs,
            CampoAliquota::IbsUf => &mut self.ibs_uf,
            CampoAliquota::IbsMun => &mut self.ibs_mun,
            CampoAliquota::Fonte => &mut self.fonte,
        }
    }
}

/// Tudo o que as telas editam. A senha do certificado nao esta aqui: ela vive so' no campo da tela e no cofre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dados {
    /// CNPJ ou CPF, com mascara.
    pub documento: String,
    /// Nome ou razao social.
    pub nome: String,
    /// Inscricao municipal.
    pub inscricao_municipal: String,
    /// Codigo IBGE (7 digitos).
    pub municipio: String,
    /// Serie da DPS.
    pub serie: String,
    /// Simples Nacional: 1, 2 ou 3.
    pub op_simples: u8,
    /// Regime de apuracao (so' ME/EPP); 0 = nao informar.
    pub regime_apuracao: u8,
    /// Regime especial; 0 = nenhum.
    pub regime_especial: u8,
    /// Ambiente da Sefin.
    pub ambiente: Ambiente,
    /// Caminho do certificado.
    pub pfx: String,
    /// Tempo limite da Sefin, em segundos.
    pub tempo_limite: String,
    /// Folga do relogio, em segundos.
    pub folga_relogio: String,
    /// Algoritmo da assinatura.
    pub assinatura: Assinatura,
    /// Pasta das notas.
    pub notas_pasta: String,
    /// Canhoto do DANFSe.
    pub canhoto: bool,
    /// Tema da interface.
    pub tema: Tema,
    /// Aliquotas publicadas.
    pub aliquotas: Vec<LinhaAliquota>,
}

/// O formulario: o que se edita, o ultimo estado gravado (para saber se mudou) e a configuracao lida, da qual saem
/// as secoes que a tela nao edita (`[servidor]`).
#[derive(Debug, Clone)]
pub struct Formulario {
    /// O que esta sendo editado.
    pub dados: Dados,
    inicial: Dados,
    base: Config,
}

fn documento_da_config(c: &Config) -> String {
    let bruto = if c.emitente.cnpj.is_empty() {
        &c.emitente.cpf
    } else {
        &c.emitente.cnpj
    };
    formatar_documento(bruto)
}

impl Dados {
    fn de_config(c: &Config) -> Self {
        Self {
            documento: documento_da_config(c),
            nome: c.emitente.nome.clone(),
            inscricao_municipal: c.emitente.inscricao_municipal.clone(),
            municipio: c.emitente.municipio.clone(),
            serie: c.emitente.serie.clone(),
            op_simples: c.emitente.op_simples,
            regime_apuracao: c.emitente.regime_apuracao,
            regime_especial: c.emitente.regime_especial,
            ambiente: Ambiente::de_config(&c.sefin.ambiente),
            pfx: c.sefin.pfx.clone(),
            tempo_limite: c.sefin.tempo_limite_s.to_string(),
            folga_relogio: c.sefin.folga_relogio_s.to_string(),
            assinatura: Assinatura::de_config(&c.sefin.assinatura),
            notas_pasta: c.notas.pasta.clone(),
            canhoto: c.danfse.canhoto,
            tema: Tema::de_config(&c.interface.tema),
            aliquotas: c.aliquotas.iter().map(LinhaAliquota::de_config).collect(),
        }
    }
}

/// Separa o documento digitado em `(cnpj, cpf)` limpos: 11 digitos sao CPF; todo o resto vai para o CNPJ, que a
/// conferencia recusa se nao servir.
fn separar_documento(texto: &str) -> (String, String) {
    let limpo = limpar_documento(texto);
    if limpo.is_empty() {
        (String::new(), String::new())
    } else if limpo.len() == 11 && limpo.bytes().all(|b| b.is_ascii_digit()) {
        (String::new(), limpo)
    } else {
        (limpo, String::new())
    }
}

fn numero<T: std::str::FromStr + Default>(texto: &str) -> T {
    texto.trim().parse().unwrap_or_default()
}

/// A primeira recusa de `Config::validar` numa configuracao padrao alterada por `ajuste`.
fn isolar(ajuste: impl FnOnce(&mut Config)) -> Option<ErroConfig> {
    let mut c = Config::default();
    ajuste(&mut c);
    c.validar().err()
}

fn motivo_de(erro: &ErroConfig) -> &str {
    match erro {
        ErroConfig::Invalido { motivo, .. } => motivo,
        _ => "",
    }
}

/// Mensagem para a tela, em portugues com acento, de um campo que a conferencia recusou.
fn mensagem(campo: Campo, motivo: &str, documento: &str) -> String {
    match campo {
        Campo::Documento => match limpar_documento(documento).len() {
            11 => "CPF inválido: confira os dígitos.".into(),
            14 => "CNPJ inválido: confira os dígitos.".into(),
            _ => "Informe um CPF (11 dígitos) ou um CNPJ (14 caracteres).".into(),
        },
        Campo::Municipio => "Informe o código IBGE com 7 dígitos.".into(),
        Campo::Serie => "Use até 5 dígitos.".into(),
        Campo::Pfx => "Caminho do certificado inválido.".into(),
        Campo::TempoLimite => "Use um valor entre 1 e 600 segundos.".into(),
        Campo::FolgaRelogio => "Use no máximo 3600 segundos.".into(),
        Campo::Aliquota(_, CampoAliquota::Ano) => {
            if motivo.contains("mais de uma vez") {
                "Este ano já está na tabela.".into()
            } else {
                "Use um ano entre 2026 e 2099.".into()
            }
        }
        Campo::Aliquota(_, CampoAliquota::Cbs) if motivo.contains("nao os dois") => {
            "Informe a CBS ou a CBS de referência, não as duas.".into()
        }
        Campo::Aliquota(_, CampoAliquota::Cbs) if motivo.contains("ao menos") => {
            "Informe ao menos uma alíquota.".into()
        }
        Campo::Aliquota(..) => "Use um número de 0 a 100, com até 4 casas (ex.: 8,8).".into(),
        _ => capitalizar(motivo),
    }
}

fn capitalizar(texto: &str) -> String {
    let mut c = texto.chars();
    match c.next() {
        Some(p) => p.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

/// Em que campo da tela cai a recusa da conferencia, ou a mensagem geral quando nao cai em nenhum.
enum Destino {
    Campo(Campo),
    Geral(String),
}

fn destino(erro: &ErroConfig) -> Destino {
    let ErroConfig::Invalido {
        secao,
        campo,
        motivo,
    } = erro
    else {
        return Destino::Geral(erro.to_string());
    };
    if let Some(indice) = secao
        .strip_prefix("aliquotas[")
        .and_then(|s| s.strip_suffix(']'))
        .and_then(|s| s.parse::<usize>().ok())
    {
        let coluna = match campo.as_str() {
            "ano" => CampoAliquota::Ano,
            "cbs_referencia" => CampoAliquota::CbsReferencia,
            "ibs_uf" => CampoAliquota::IbsUf,
            "ibs_mun" => CampoAliquota::IbsMun,
            "fonte" => CampoAliquota::Fonte,
            _ => CampoAliquota::Cbs,
        };
        return Destino::Campo(Campo::Aliquota(indice, coluna));
    }
    let campo_da_tela = match (secao.as_str(), campo.as_str()) {
        ("emitente", "cnpj" | "cpf") => Some(Campo::Documento),
        ("emitente", "municipio") => Some(Campo::Municipio),
        ("emitente", "op_simples") => Some(Campo::OpSimples),
        ("emitente", "regime_apuracao") => Some(Campo::RegimeApuracao),
        ("emitente", "serie") => Some(Campo::Serie),
        ("sefin", "pfx") => Some(Campo::Pfx),
        ("sefin", "tempo_limite_s") => Some(Campo::TempoLimite),
        ("sefin", "folga_relogio_s") => Some(Campo::FolgaRelogio),
        _ => None,
    };
    match campo_da_tela {
        Some(c) => Destino::Campo(c),
        None => Destino::Geral(format!(
            "O arquivo tem um valor inválido em [{secao}], campo \"{campo}\" ({motivo}). Corrija no arquivo."
        )),
    }
}

impl Formulario {
    /// Formulario com os valores de `config`; o que se edita ainda nao mudou.
    #[must_use]
    pub fn de_config(config: &Config) -> Self {
        let dados = Dados::de_config(config);
        Self {
            inicial: dados.clone(),
            dados,
            base: config.clone(),
        }
    }

    /// O texto atual de um campo de texto (vazio para os que nao sao de texto).
    #[must_use]
    pub fn texto(&self, campo: Campo) -> String {
        let d = &self.dados;
        match campo {
            Campo::Documento => d.documento.clone(),
            Campo::Nome => d.nome.clone(),
            Campo::InscricaoMunicipal => d.inscricao_municipal.clone(),
            Campo::Municipio => d.municipio.clone(),
            Campo::Serie => d.serie.clone(),
            Campo::Pfx => d.pfx.clone(),
            Campo::TempoLimite => d.tempo_limite.clone(),
            Campo::FolgaRelogio => d.folga_relogio.clone(),
            Campo::NotasPasta => d.notas_pasta.clone(),
            Campo::Aliquota(i, coluna) => d
                .aliquotas
                .get(i)
                .map(|l| l.texto(coluna).to_string())
                .unwrap_or_default(),
            Campo::OpSimples | Campo::RegimeApuracao | Campo::RegimeEspecial | Campo::Senha => {
                String::new()
            }
        }
    }

    /// O texto como a tela deve mostra-lo depois de digitado: mascara do documento, so' digitos onde so' cabem
    /// digitos. E' o que [`Formulario::definir`] guarda.
    #[must_use]
    pub fn normalizar(campo: Campo, texto: &str) -> String {
        match campo {
            Campo::Documento => formatar_documento(texto),
            Campo::Municipio => so_digitos(texto, 7),
            Campo::Serie => so_digitos(texto, 5),
            Campo::TempoLimite | Campo::FolgaRelogio => so_digitos(texto, 5),
            Campo::Aliquota(_, CampoAliquota::Ano) => so_digitos(texto, 4),
            Campo::Aliquota(
                _,
                CampoAliquota::CbsReferencia
                | CampoAliquota::Cbs
                | CampoAliquota::IbsUf
                | CampoAliquota::IbsMun,
            ) => filtrar_decimal(texto),
            _ => texto.to_string(),
        }
    }

    /// Guarda o texto digitado de um campo, ja normalizado, e o devolve (a tela troca o que mostra se for diferente).
    pub fn definir(&mut self, campo: Campo, texto: &str) -> String {
        let normal = Self::normalizar(campo, texto);
        let d = &mut self.dados;
        match campo {
            Campo::Documento => d.documento.clone_from(&normal),
            Campo::Nome => d.nome.clone_from(&normal),
            Campo::InscricaoMunicipal => d.inscricao_municipal.clone_from(&normal),
            Campo::Municipio => d.municipio.clone_from(&normal),
            Campo::Serie => d.serie.clone_from(&normal),
            Campo::Pfx => d.pfx.clone_from(&normal),
            Campo::TempoLimite => d.tempo_limite.clone_from(&normal),
            Campo::FolgaRelogio => d.folga_relogio.clone_from(&normal),
            Campo::NotasPasta => d.notas_pasta.clone_from(&normal),
            Campo::Aliquota(i, coluna) => {
                if let Some(linha) = d.aliquotas.get_mut(i) {
                    linha.texto_mut(coluna).clone_from(&normal);
                }
            }
            Campo::OpSimples | Campo::RegimeApuracao | Campo::RegimeEspecial | Campo::Senha => {}
        }
        normal
    }

    /// Escolhe o Simples Nacional. Fora de ME/EPP o regime de apuracao nao se aplica; MEI nao tem regime especial.
    pub fn definir_op_simples(&mut self, valor: u8) {
        self.dados.op_simples = valor;
        if valor != 3 {
            self.dados.regime_apuracao = 0;
        }
        if valor == 2 {
            self.dados.regime_especial = 0;
        }
    }

    /// Acrescenta uma linha vazia na tabela de aliquotas, com o ano seguinte ao maior que ja esta la (2027, o
    /// primeiro da CBS, se a tabela estiver vazia). Devolve o indice da linha.
    pub fn adicionar_aliquota(&mut self) -> usize {
        let proximo = self
            .dados
            .aliquotas
            .iter()
            .filter_map(|l| l.ano.parse::<i32>().ok())
            .max()
            .map_or(2027, |a| (a + 1).min(2099));
        self.dados.aliquotas.push(LinhaAliquota {
            ano: proximo.to_string(),
            cbs_referencia: String::new(),
            cbs: String::new(),
            ibs_uf: String::new(),
            ibs_mun: String::new(),
            fonte: String::new(),
        });
        self.dados.aliquotas.len() - 1
    }

    /// Tira uma linha da tabela de aliquotas.
    pub fn remover_aliquota(&mut self, indice: usize) {
        if indice < self.dados.aliquotas.len() {
            self.dados.aliquotas.remove(indice);
        }
    }

    /// A configuracao que o formulario representa, sem conferir. O que a tela nao edita (`[servidor]`) vem da
    /// configuracao lida.
    #[must_use]
    pub fn para_config(&self) -> Config {
        let d = &self.dados;
        let mut c = self.base.clone();
        let (cnpj, cpf) = separar_documento(&d.documento);
        c.emitente.cnpj = cnpj;
        c.emitente.cpf = cpf;
        c.emitente.nome = d.nome.trim().to_string();
        c.emitente.inscricao_municipal = d.inscricao_municipal.trim().to_string();
        c.emitente.municipio.clone_from(&d.municipio);
        c.emitente.serie.clone_from(&d.serie);
        c.emitente.op_simples = d.op_simples;
        c.emitente.regime_apuracao = d.regime_apuracao;
        c.emitente.regime_especial = d.regime_especial;
        c.sefin.ambiente = d.ambiente.para_config().into();
        c.sefin.pfx = d.pfx.trim().to_string();
        c.sefin.tempo_limite_s = numero(&d.tempo_limite);
        c.sefin.folga_relogio_s = numero(&d.folga_relogio);
        c.sefin.assinatura = d.assinatura.para_config().into();
        c.notas.pasta = d.notas_pasta.trim().to_string();
        c.danfse.canhoto = d.canhoto;
        c.interface.tema = d.tema.para_config().into();
        c.aliquotas = d.aliquotas.iter().map(LinhaAliquota::para_config).collect();
        c
    }

    /// Mudou alguma coisa desde que foi carregado ou gravado.
    #[must_use]
    pub fn alterado(&self) -> bool {
        self.dados != self.inicial
    }

    /// Marca o estado atual como o gravado (depois de `Config::salvar` dar certo com `salvo`).
    pub fn marcar_salvo(&mut self, salvo: &Config) {
        self.inicial = self.dados.clone();
        self.base = salvo.clone();
    }

    /// Joga fora as mudancas e volta ao ultimo estado gravado.
    pub fn descartar(&mut self) {
        self.dados = self.inicial.clone();
    }

    /// A chave do item da senha no cofre para o certificado deste formulario.
    #[must_use]
    pub fn chave_do_cofre(&self) -> Option<String> {
        let c = self.para_config();
        chave_do_certificado(&c.emitente, &c.sefin.pfx)
    }

    /// O que o nome do municipio digitado diz (nome e UF), para mostrar ao lado do campo.
    #[must_use]
    pub fn rotulo_do_municipio(&self) -> super::mascara::RotuloMunicipio {
        super::mascara::rotulo_do_municipio(&self.dados.municipio)
    }

    /// Confere todos os campos: cada um sozinho e, depois, a configuracao inteira.
    #[must_use]
    pub fn erros(&self) -> Erros {
        let d = &self.dados;
        let mut e = Erros::default();
        let mut registrar = |campo: Campo, erro: Option<ErroConfig>| {
            if let Some(erro) = erro {
                e.campos
                    .insert(campo, mensagem(campo, motivo_de(&erro), &d.documento));
            }
        };

        let (cnpj, cpf) = separar_documento(&d.documento);
        registrar(
            Campo::Documento,
            isolar(|c| {
                c.emitente.cnpj = cnpj;
                c.emitente.cpf = cpf;
            }),
        );
        registrar(
            Campo::Municipio,
            isolar(|c| c.emitente.municipio.clone_from(&d.municipio)),
        );
        registrar(
            Campo::Serie,
            isolar(|c| c.emitente.serie.clone_from(&d.serie)),
        );
        registrar(Campo::Pfx, isolar(|c| c.sefin.pfx.clone_from(&d.pfx)));
        registrar(
            Campo::TempoLimite,
            isolar(|c| c.sefin.tempo_limite_s = numero(&d.tempo_limite)),
        );
        registrar(
            Campo::FolgaRelogio,
            isolar(|c| c.sefin.folga_relogio_s = numero(&d.folga_relogio)),
        );
        for (i, linha) in d.aliquotas.iter().enumerate() {
            self.erros_da_linha(i, linha, &mut e);
        }
        self.regras_do_simples(&mut e);

        // o que depende de mais de um campo, e o que a tela nao edita
        if let Err(erro) = self.para_config().validar() {
            match destino(&erro) {
                Destino::Campo(campo) => {
                    e.campos
                        .entry(campo)
                        .or_insert_with(|| mensagem(campo, motivo_de(&erro), &d.documento));
                }
                Destino::Geral(texto) => e.gerais.push(texto),
            }
        }
        e
    }

    fn erros_da_linha(&self, i: usize, linha: &LinhaAliquota, e: &mut Erros) {
        let a = linha.para_config();
        if isolar(|c| {
            c.aliquotas.push(Aliquota {
                ano: a.ano,
                cbs: "1".into(),
                ..Aliquota::default()
            });
        })
        .is_some()
        {
            e.campos.insert(
                Campo::Aliquota(i, CampoAliquota::Ano),
                mensagem(Campo::Aliquota(i, CampoAliquota::Ano), "", ""),
            );
        }
        let decimais = [
            (CampoAliquota::CbsReferencia, &a.cbs_referencia),
            (CampoAliquota::Cbs, &a.cbs),
            (CampoAliquota::IbsUf, &a.ibs_uf),
            (CampoAliquota::IbsMun, &a.ibs_mun),
        ];
        let mut decimal_ruim = false;
        for (coluna, valor) in decimais {
            if !valor.is_empty()
                && isolar(|c| {
                    let mut isolada = Aliquota::default();
                    match coluna {
                        CampoAliquota::CbsReferencia => isolada.cbs_referencia.clone_from(valor),
                        CampoAliquota::Cbs => isolada.cbs.clone_from(valor),
                        CampoAliquota::IbsUf => isolada.ibs_uf.clone_from(valor),
                        _ => isolada.ibs_mun.clone_from(valor),
                    }
                    c.aliquotas.push(isolada);
                })
                .is_some()
            {
                decimal_ruim = true;
                e.campos.insert(
                    Campo::Aliquota(i, coluna),
                    mensagem(Campo::Aliquota(i, coluna), "", ""),
                );
            }
        }
        // a combinacao dos campos da linha (so' se cada um, sozinho, serve)
        if !decimal_ruim {
            if let Some(erro) = isolar(|c| {
                c.aliquotas.push(Aliquota {
                    ano: 2027,
                    fonte: a.fonte.clone(),
                    ..a.clone()
                });
            }) {
                e.campos
                    .entry(Campo::Aliquota(i, CampoAliquota::Cbs))
                    .or_insert_with(|| {
                        mensagem(Campo::Aliquota(i, CampoAliquota::Cbs), motivo_de(&erro), "")
                    });
            }
        }
    }

    /// Regras de combinacao do Simples Nacional que a Sefin recusa na emissao (E0174 e E0175): a tela avisa antes.
    fn regras_do_simples(&self, e: &mut Erros) {
        let d = &self.dados;
        if d.op_simples == 2 && d.regime_especial != 0 {
            e.campos.insert(
                Campo::RegimeEspecial,
                "MEI não tem regime especial: use \"Nenhum\".".into(),
            );
        }
        if d.op_simples == 3 && d.regime_apuracao == 1 && d.regime_especial != 0 {
            e.campos.insert(
                Campo::RegimeEspecial,
                "ME/EPP que apura tudo pelo Simples não tem regime especial: use \"Nenhum\"."
                    .into(),
            );
        }
    }

    /// Erros que impedem de sair de um passo do primeiro uso: os campos obrigatorios do passo que estao vazios e
    /// as recusas da conferencia nos campos dele. A senha do certificado e' conferida a parte (ela nao esta aqui).
    #[must_use]
    pub fn erros_do_passo(&self, passo: Passo) -> Erros {
        let d = &self.dados;
        let mut e = Erros::default();
        let vazio = |t: &str| t.trim().is_empty();
        let mut exigir = |campo: Campo, vazio: bool, texto: &str| {
            if vazio {
                e.campos.insert(campo, texto.into());
            }
        };
        match passo {
            Passo::Emitente => {
                exigir(
                    Campo::Documento,
                    vazio(&d.documento),
                    "Informe o CNPJ ou o CPF.",
                );
                exigir(
                    Campo::Nome,
                    vazio(&d.nome),
                    "Informe o nome ou a razão social.",
                );
                exigir(
                    Campo::InscricaoMunicipal,
                    vazio(&d.inscricao_municipal),
                    "Informe a inscrição municipal.",
                );
                exigir(
                    Campo::Municipio,
                    vazio(&d.municipio),
                    "Informe o código IBGE do município.",
                );
                exigir(
                    Campo::RegimeApuracao,
                    d.op_simples == 3 && d.regime_apuracao == 0,
                    "Escolha o regime de apuração (ME/EPP).",
                );
            }
            Passo::Certificado => exigir(
                Campo::Pfx,
                vazio(&d.pfx),
                "Escolha o arquivo do certificado (.pfx ou .p12).",
            ),
            Passo::Notas => exigir(
                Campo::NotasPasta,
                vazio(&d.notas_pasta),
                "Escolha a pasta onde guardar as notas.",
            ),
        }
        let mut todos = e;
        todos.unir(self.erros().restrito_a(passo.campos()));
        todos
    }

    /// Por que o botao Salvar esta desligado, ou `None` se pode salvar.
    #[must_use]
    pub fn motivo_sem_salvar(&self) -> Option<String> {
        if !self.alterado() {
            return Some("Nenhuma alteração para salvar.".into());
        }
        let erros = self.erros();
        let primeiro = erros.primeira()?;
        let n = erros.total();
        Some(if n == 1 {
            format!("Corrija antes de salvar: {primeiro}")
        } else {
            format!("Corrija {n} campos antes de salvar. Primeiro: {primeiro}")
        })
    }
}

#[cfg(test)]
mod testes {
    use rustnf_app::config::Servidor;

    use super::*;

    fn formulario() -> Formulario {
        Formulario::de_config(&Config::default())
    }

    fn preenchido() -> Formulario {
        let mut f = formulario();
        f.definir(Campo::Documento, "11222333000181");
        f.definir(Campo::Nome, "OFICINA TESTE");
        f.definir(Campo::InscricaoMunicipal, "123456");
        f.definir(Campo::Municipio, "3550308");
        f.definir(Campo::Pfx, "certificado.pfx");
        f.definir(Campo::NotasPasta, "notas");
        f
    }

    #[test]
    fn campos_digitados_passam_pela_mascara_e_pelo_filtro() {
        let mut f = formulario();
        assert_eq!(
            f.definir(Campo::Documento, "11222333000181"),
            "11.222.333/0001-81"
        );
        assert_eq!(f.texto(Campo::Documento), "11.222.333/0001-81");
        assert_eq!(f.definir(Campo::Municipio, "35-503.08"), "3550308");
        assert_eq!(f.definir(Campo::Serie, "123456"), "12345");
        assert_eq!(f.definir(Campo::TempoLimite, "9a0"), "90");
        f.adicionar_aliquota();
        assert_eq!(
            f.definir(Campo::Aliquota(0, CampoAliquota::Cbs), "8,8x"),
            "8,8"
        );
        assert_eq!(
            f.definir(Campo::Aliquota(0, CampoAliquota::Ano), "20a27"),
            "2027"
        );
        assert_eq!(f.definir(Campo::Nome, "Ana & Cia"), "Ana & Cia");
    }

    #[test]
    fn ida_e_volta_com_config_nao_perde_nada() {
        let mut c = Config::default();
        c.emitente.cnpj = "11222333000181".into();
        c.emitente.nome = "OFICINA TESTE".into();
        c.emitente.inscricao_municipal = "123456".into();
        c.emitente.municipio = "3550308".into();
        c.emitente.op_simples = 3;
        c.emitente.regime_apuracao = 2;
        c.emitente.regime_especial = 0;
        c.emitente.serie = "7".into();
        c.sefin.ambiente = "producao".into();
        c.sefin.pfx = "cert.pfx".into();
        c.sefin.tempo_limite_s = 90;
        c.sefin.folga_relogio_s = 30;
        c.sefin.assinatura = "sha1".into();
        c.notas.pasta = "notas".into();
        c.danfse.canhoto = false;
        c.interface.tema = "escuro".into();
        c.aliquotas.push(Aliquota {
            ano: 2029,
            cbs: "8.8".into(),
            ibs_uf: "17.7".into(),
            ibs_mun: "1".into(),
            fonte: "norma de exemplo".into(),
            ..Aliquota::default()
        });
        let f = Formulario::de_config(&c);
        assert_eq!(f.para_config(), c);
        assert!(!f.alterado());
        assert!(f.erros().vazio(), "{:?}", f.erros());
        // CPF tambem faz a ida e volta
        let mut c = Config::default();
        c.emitente.cpf = "52998224725".into();
        let f = Formulario::de_config(&c);
        assert_eq!(f.dados.documento, "529.982.247-25");
        assert_eq!(f.para_config().emitente.cpf, "52998224725");
        assert_eq!(f.para_config().emitente.cnpj, "");
    }

    #[test]
    fn o_servidor_e_o_que_a_tela_nao_edita_sobrevivem_ao_salvar() {
        let mut c = Config {
            servidor: Servidor {
                endereco: "10.0.0.5".into(),
                porta: 9999,
                limite_corpo: 2048,
                tempo_limite_s: 15,
                origens_permitidas: vec!["https://app.exemplo.com.br".into()],
            },
            ..Config::default()
        };
        c.aliquotas.push(Aliquota {
            ano: 2028,
            cbs_referencia: "8.8".into(),
            fonte: "norma".into(),
            ..Aliquota::default()
        });
        let mut f = Formulario::de_config(&c);
        f.definir(Campo::Nome, "NOVO NOME");
        f.definir_op_simples(2);
        let salvo = f.para_config();
        assert_eq!(salvo.servidor, c.servidor);
        assert_eq!(salvo.aliquotas, c.aliquotas);
        assert_eq!(salvo.emitente.nome, "NOVO NOME");
        // e continuam la depois de gravar e editar de novo
        f.marcar_salvo(&salvo);
        f.definir(Campo::Serie, "2");
        assert_eq!(f.para_config().servidor, c.servidor);
    }

    #[test]
    fn documento_errado_diz_se_e_cpf_ou_cnpj() {
        let mut f = formulario();
        assert!(
            f.erros().do_campo(Campo::Documento).is_none(),
            "vazio e' aceito"
        );
        f.definir(Campo::Documento, "52998224726");
        assert_eq!(
            f.erros().do_campo(Campo::Documento),
            Some("CPF inválido: confira os dígitos.")
        );
        f.definir(Campo::Documento, "11222333000182");
        assert_eq!(
            f.erros().do_campo(Campo::Documento),
            Some("CNPJ inválido: confira os dígitos.")
        );
        f.definir(Campo::Documento, "1122233");
        assert_eq!(
            f.erros().do_campo(Campo::Documento),
            Some("Informe um CPF (11 dígitos) ou um CNPJ (14 caracteres).")
        );
        f.definir(Campo::Documento, "11222333000181");
        assert!(f.erros().do_campo(Campo::Documento).is_none());
    }

    #[test]
    fn varios_campos_errados_aparecem_de_uma_vez() {
        let mut f = formulario();
        f.definir(Campo::Municipio, "355");
        f.definir(Campo::TempoLimite, "0");
        f.definir(Campo::FolgaRelogio, "99999");
        f.definir(Campo::Pfx, "");
        let e = f.erros();
        assert_eq!(
            e.do_campo(Campo::Municipio),
            Some("Informe o código IBGE com 7 dígitos.")
        );
        assert_eq!(
            e.do_campo(Campo::TempoLimite),
            Some("Use um valor entre 1 e 600 segundos.")
        );
        assert_eq!(
            e.do_campo(Campo::FolgaRelogio),
            Some("Use no máximo 3600 segundos.")
        );
        assert_eq!(e.total(), 3);
        assert!(e.gerais.is_empty());
    }

    #[test]
    fn erros_de_aliquota_apontam_a_celula() {
        let mut f = formulario();
        f.adicionar_aliquota();
        f.adicionar_aliquota();
        // linha 0: so' o ano errado; linha 1: duas colunas ruins
        f.definir(Campo::Aliquota(0, CampoAliquota::Ano), "1999");
        f.definir(Campo::Aliquota(0, CampoAliquota::Cbs), "8");
        f.definir(Campo::Aliquota(1, CampoAliquota::IbsUf), "101");
        f.definir(Campo::Aliquota(1, CampoAliquota::IbsMun), "1,23456");
        let e = f.erros();
        assert_eq!(
            e.da_aliquota(0, CampoAliquota::Ano),
            Some("Use um ano entre 2026 e 2099.")
        );
        assert!(e.da_aliquota(0, CampoAliquota::Cbs).is_none());
        assert!(e.da_aliquota(1, CampoAliquota::IbsUf).is_some());
        assert!(e.da_aliquota(1, CampoAliquota::IbsMun).is_some());
        assert!(e.da_aliquota(1, CampoAliquota::Ano).is_none());
    }

    #[test]
    fn aliquota_vazia_ou_com_as_duas_cbs_e_recusada_na_celula_da_cbs() {
        let mut f = formulario();
        f.adicionar_aliquota();
        assert_eq!(
            f.erros().da_aliquota(0, CampoAliquota::Cbs),
            Some("Informe ao menos uma alíquota.")
        );
        f.definir(Campo::Aliquota(0, CampoAliquota::Cbs), "8,8");
        f.definir(Campo::Aliquota(0, CampoAliquota::CbsReferencia), "8,8");
        assert_eq!(
            f.erros().da_aliquota(0, CampoAliquota::Cbs),
            Some("Informe a CBS ou a CBS de referência, não as duas.")
        );
        f.definir(Campo::Aliquota(0, CampoAliquota::CbsReferencia), "");
        assert!(f.erros().vazio());
    }

    #[test]
    fn ano_repetido_na_tabela_e_apontado_na_segunda_linha() {
        let mut f = formulario();
        f.adicionar_aliquota();
        f.adicionar_aliquota();
        f.definir(Campo::Aliquota(0, CampoAliquota::Cbs), "8");
        f.definir(Campo::Aliquota(1, CampoAliquota::Cbs), "9");
        // a linha nova ja vem com o ano seguinte
        assert_eq!(f.dados.aliquotas[1].ano, "2028");
        assert!(f.erros().vazio());
        f.definir(Campo::Aliquota(1, CampoAliquota::Ano), "2027");
        assert_eq!(
            f.erros().da_aliquota(1, CampoAliquota::Ano),
            Some("Este ano já está na tabela.")
        );
    }

    #[test]
    fn adicionar_e_remover_linhas_da_tabela() {
        let mut f = formulario();
        assert_eq!(f.adicionar_aliquota(), 0);
        assert_eq!(f.dados.aliquotas[0].ano, "2027");
        assert_eq!(f.adicionar_aliquota(), 1);
        f.remover_aliquota(0);
        assert_eq!(f.dados.aliquotas.len(), 1);
        assert_eq!(f.dados.aliquotas[0].ano, "2028");
        f.remover_aliquota(9);
        assert_eq!(f.dados.aliquotas.len(), 1);
        assert!(f.alterado());
    }

    #[test]
    fn decimal_com_virgula_vai_para_o_arquivo_com_ponto() {
        let mut f = formulario();
        f.adicionar_aliquota();
        f.definir(Campo::Aliquota(0, CampoAliquota::CbsReferencia), "8,8");
        assert_eq!(f.para_config().aliquotas[0].cbs_referencia, "8.8");
        assert!(f.para_config().validar().is_ok());
    }

    #[test]
    fn salvar_so_liga_com_mudanca_e_sem_erro_e_diz_por_que() {
        let mut f = formulario();
        assert_eq!(
            f.motivo_sem_salvar().as_deref(),
            Some("Nenhuma alteração para salvar.")
        );
        f.definir(Campo::Nome, "EMPRESA");
        assert_eq!(f.motivo_sem_salvar(), None);
        f.definir(Campo::Municipio, "12");
        let motivo = f.motivo_sem_salvar().unwrap();
        assert!(motivo.contains("Informe o código IBGE"), "{motivo}");
        f.definir(Campo::TempoLimite, "0");
        let motivo = f.motivo_sem_salvar().unwrap();
        assert!(motivo.starts_with("Corrija 2 campos"), "{motivo}");
        // voltar ao que estava desliga de novo
        f.descartar();
        assert!(!f.alterado());
        assert!(f.motivo_sem_salvar().is_some());
    }

    #[test]
    fn marcar_salvo_zera_a_mudanca() {
        let mut f = preenchido();
        assert!(f.alterado());
        let c = f.para_config();
        f.marcar_salvo(&c);
        assert!(!f.alterado());
        f.definir(Campo::Nome, "OUTRO");
        assert!(f.alterado());
        f.descartar();
        assert_eq!(f.dados.nome, "OFICINA TESTE");
    }

    #[test]
    fn mei_nao_tem_regime_especial_nem_apuracao_e_me_epp_que_apura_tudo_tambem_nao() {
        let mut f = formulario();
        f.definir_op_simples(3);
        f.dados.regime_apuracao = 1;
        f.dados.regime_especial = 6;
        assert!(f.erros().do_campo(Campo::RegimeEspecial).is_some());
        f.dados.regime_apuracao = 2;
        assert!(f.erros().do_campo(Campo::RegimeEspecial).is_none());
        // trocar para MEI limpa os dois
        f.definir_op_simples(2);
        assert_eq!(f.dados.regime_apuracao, 0);
        assert_eq!(f.dados.regime_especial, 0);
        assert!(f.erros().vazio());
    }

    #[test]
    fn erros_do_passo_pedem_o_que_falta_e_so_do_passo() {
        let mut f = formulario();
        let e = f.erros_do_passo(Passo::Emitente);
        assert_eq!(
            e.do_campo(Campo::Documento),
            Some("Informe o CNPJ ou o CPF.")
        );
        assert_eq!(
            e.do_campo(Campo::Nome),
            Some("Informe o nome ou a razão social.")
        );
        assert!(e.do_campo(Campo::Pfx).is_none());
        assert_eq!(
            f.erros_do_passo(Passo::Certificado).do_campo(Campo::Pfx),
            Some("Escolha o arquivo do certificado (.pfx ou .p12).")
        );
        assert!(f
            .erros_do_passo(Passo::Notas)
            .do_campo(Campo::NotasPasta)
            .is_some());
        f.definir(Campo::Documento, "11222333000181");
        f.definir(Campo::Nome, "X");
        f.definir(Campo::InscricaoMunicipal, "1");
        f.definir(Campo::Municipio, "3550308");
        assert!(f.erros_do_passo(Passo::Emitente).vazio());
        // ME/EPP exige o regime de apuracao no primeiro uso
        f.definir_op_simples(3);
        assert_eq!(
            f.erros_do_passo(Passo::Emitente)
                .do_campo(Campo::RegimeApuracao),
            Some("Escolha o regime de apuração (ME/EPP).")
        );
        // erro de valor num campo do passo tambem barra
        f.dados.regime_apuracao = 2;
        f.definir(Campo::Municipio, "355");
        assert!(f
            .erros_do_passo(Passo::Emitente)
            .do_campo(Campo::Municipio)
            .is_some());
    }

    #[test]
    fn chave_do_cofre_usa_o_documento_ou_o_nome_do_arquivo() {
        let mut f = formulario();
        assert_eq!(f.chave_do_cofre(), None);
        f.definir(Campo::Pfx, "pasta/meu certificado.pfx");
        assert_eq!(f.chave_do_cofre().as_deref(), Some("meu certificado.pfx"));
        f.definir(Campo::Documento, "11222333000181");
        assert_eq!(f.chave_do_cofre().as_deref(), Some("11222333000181"));
    }

    #[test]
    fn debug_do_formulario_nao_tem_campo_de_senha() {
        let f = preenchido();
        let texto = format!("{f:?}");
        assert!(!texto.to_lowercase().contains("senha"));
    }

    #[test]
    fn valor_invalido_do_servidor_vira_aviso_geral_sem_campo() {
        let mut c = Config::default();
        c.servidor.endereco = "localhost".into();
        let f = Formulario::de_config(&c);
        let e = f.erros();
        assert!(e.campos.is_empty());
        assert_eq!(e.gerais.len(), 1);
        assert!(e.gerais[0].contains("[servidor]"), "{:?}", e.gerais);
        assert!(f.erros_do_passo(Passo::Emitente).gerais.is_empty());
    }
}
