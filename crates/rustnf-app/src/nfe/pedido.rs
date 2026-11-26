// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O pedido da NF-e em JSON, completado com a configuracao: o que o pedido nao traz (emitente, UF, ambiente, serie,
//! numero, `cNF`, data de emissao, contingencia) vem do `[emitente]`, do `[nfe]` e da pasta de notas; o que o pedido
//! traz nunca e' sobrescrito.

use chrono::{DateTime, Duration, FixedOffset, Offset, SecondsFormat, Utc};
use rustnf::chave_dfe;
use rustnf::nfe::montagem::{self, Aviso, Nota};
use rustnf::nfe::sefaz::svc;
use serde_json::{json, Map, Value};

use crate::config::{ausente, completar, texto, Config};
use crate::erro::Erro;
use crate::notas::nfe::NotasNfe;

/// O pedido ja completado, lido e conferido, pronto para montar.
#[derive(Debug, Clone)]
pub struct Preparada {
    /// O pedido como a montagem o le (depois de completado).
    pub pedido: Value,
    /// A nota lida.
    pub nota: Nota,
    /// Serie da nota.
    pub serie: String,
    /// Numero da nota.
    pub numero: u64,
    /// Os campos do pedido que o leiaute nao conhece (ignorados, se a chamada os admitiu).
    pub desconhecidos: Vec<String>,
}

/// Horario de Brasilia (UTC-3, sem horario de verao desde 2019): o fuso em que se escreve o `dhEmi` que o pedido nao traz.
fn brasilia() -> FixedOffset {
    FixedOffset::west_opt(3 * 3600).unwrap_or(Utc.fix())
}

/// O instante em ISO 8601 com fuso, na precisao de segundos.
fn iso(t: DateTime<Utc>) -> String {
    t.with_timezone(&brasilia())
        .to_rfc3339_opts(SecondsFormat::Secs, false)
}

fn pedir(config: &str, campo: &str, o_que: &str) -> Erro {
    Erro::Uso(format!(
        "informe {o_que} no pedido ou em {config} do arquivo de configuracao (campo {campo})"
    ))
}

/// Completa o pedido. `numero` e' o proximo da serie (da pasta de notas), usado so' se o pedido nao traz o seu.
///
/// # Errors
///
/// [`Erro::Entrada`] se o pedido nao e' um objeto e [`Erro::Uso`] se falta um dado que nem o pedido nem a
/// configuracao tem (documento do emitente, UF, CRT, numero).
pub fn completar_pedido(
    pedido: &mut Value,
    config: &Config,
    numero: Option<u64>,
    agora: DateTime<Utc>,
    epec: bool,
) -> Result<(), Erro> {
    let Some(raiz) = pedido.as_object_mut() else {
        return Err(Erro::Entrada(
            "o pedido da NF-e tem de ser um objeto JSON".into(),
        ));
    };
    let nfe = &config.nfe;
    let folga = Duration::seconds(i64::from(config.sefin.folga_relogio_s));
    let emissao = iso(agora - folga);
    completar(raiz, "modelo", json!(55));
    completar(raiz, "tp_amb", json!(nfe.tp_amb()));
    completar(raiz, "emissao", json!(emissao));
    if let Some(v) = texto(&nfe.serie) {
        completar(raiz, "serie", v);
    }
    let municipio =
        texto(&nfe.endereco.cod_municipio).or_else(|| texto(&config.emitente.municipio));
    if let Some(v) = municipio {
        completar(raiz, "cmun_fg", v);
    }
    if !nfe.regime_pis_cofins.is_empty() {
        completar(raiz, "regime_pis_cofins", json!(nfe.regime_pis_cofins));
    }
    completar_emitente(raiz, config)?;
    if ausente(raiz, "numero") {
        let n = numero.ok_or_else(|| {
            Erro::Uso("informe o numero no pedido ou configure [notas].pasta para numerar".into())
        })?;
        raiz.insert("numero".into(), json!(n));
    }
    let n = numero_do_pedido(raiz);
    if ausente(raiz, "codigo") {
        raiz.insert("codigo".into(), json!(chave_dfe::gerar_codigo(n)));
    }
    contingencia(raiz, config, epec)
}

fn numero_do_pedido(raiz: &Map<String, Value>) -> u64 {
    match raiz.get("numero") {
        Some(Value::Number(n)) => n.as_u64().unwrap_or(0),
        Some(Value::String(s)) => s.trim().parse().unwrap_or(0),
        _ => 0,
    }
}

/// A SVC (`tpEmis` 6 ou 7, conforme a UF) pela configuracao, com a data de entrada e o motivo que o MOC manda.
fn contingencia(raiz: &mut Map<String, Value>, config: &Config, epec: bool) -> Result<(), Erro> {
    if !(epec || config.nfe.em_contingencia()) || !ausente(raiz, "tp_emis") {
        return Ok(());
    }
    let uf = raiz
        .get("emit")
        .and_then(|e| e.pointer("/endereco/uf"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    // o EPEC e' o tpEmis 4; a SVC, o 6 ou o 7 conforme a UF
    let tp = if epec {
        4
    } else {
        svc::svc_da_uf(&uf)?.tp_emis()
    };
    raiz.insert("tp_emis".into(), json!(tp));
    if let Some(e) = raiz.get("emissao").cloned() {
        completar(raiz, "dh_cont", e);
    }
    if ausente(raiz, "x_just") && config.nfe.motivo_contingencia.trim().is_empty() {
        return Err(Erro::Uso(
            "informe o motivo da contingencia: x_just no pedido ou [nfe].motivo_contingencia (15 a 255 caracteres)"
                .into(),
        ));
    }
    completar(raiz, "x_just", json!(config.nfe.motivo_contingencia.trim()));
    Ok(())
}

/// Entrega ao `f` o objeto `chave` do pedido (criando-o se falta ou se nao e' um objeto) e o poe de volta.
fn com_objeto<R>(
    pai: &mut Map<String, Value>,
    chave: &str,
    f: impl FnOnce(&mut Map<String, Value>) -> R,
) -> R {
    let mut obj = match pai.remove(chave) {
        Some(Value::Object(o)) => o,
        _ => Map::new(),
    };
    let r = f(&mut obj);
    pai.insert(chave.into(), Value::Object(obj));
    r
}

/// O `emit` do pedido: documento e nome do `[emitente]`, CRT, IE, fantasia e endereco do `[nfe]`.
fn completar_emitente(raiz: &mut Map<String, Value>, config: &Config) -> Result<(), Erro> {
    let (e, n) = (&config.emitente, &config.nfe);
    com_objeto(raiz, "emit", |emit| {
        let documento = if e.cnpj.is_empty() { &e.cpf } else { &e.cnpj };
        let simples = [
            ("documento", documento),
            ("nome", &e.nome),
            ("ie", &n.ie),
            ("fantasia", &n.fantasia),
        ];
        for (chave, valor) in simples {
            if let Some(v) = texto(valor) {
                completar(emit, chave, v);
            }
        }
        if n.crt != 0 {
            completar(emit, "crt", json!(n.crt));
        }
        let uf_ausente = com_objeto(emit, "endereco", |ender| {
            completar_endereco(ender, config);
            ausente(ender, "uf")
        });
        for (campo, onde, o_que) in [
            ("documento", "[emitente].cnpj", "o CNPJ ou CPF do emitente"),
            ("crt", "[nfe].crt", "o CRT do emitente"),
        ] {
            if ausente(emit, campo) {
                return Err(pedir(onde, &format!("emit.{campo}"), o_que));
            }
        }
        if uf_ausente {
            return Err(pedir("[nfe].uf", "emit.endereco.uf", "a UF do emitente"));
        }
        Ok(())
    })
}

fn completar_endereco(ender: &mut Map<String, Value>, config: &Config) {
    let (e, n) = (&config.emitente, &config.nfe);
    let a = &n.endereco;
    let cod = if a.cod_municipio.is_empty() {
        &e.municipio
    } else {
        &a.cod_municipio
    };
    for (chave, valor) in [
        ("logradouro", &a.logradouro),
        ("numero", &a.numero),
        ("complemento", &a.complemento),
        ("bairro", &a.bairro),
        ("cod_municipio", cod),
        ("municipio", &a.municipio),
        ("uf", &n.uf),
        ("cep", &a.cep),
        ("fone", &a.fone),
    ] {
        if let Some(v) = texto(valor) {
            completar(ender, chave, v);
        }
    }
}

/// Os numeros que o pedido da NF-e traz como texto ou inteiro: a serie que vai valer.
fn serie_do_pedido(pedido: &Value) -> String {
    match pedido.get("serie") {
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::String(s)) if !s.trim().is_empty() => s.trim().to_string(),
        _ => "1".to_string(),
    }
}

/// Completa o pedido, le a NF-e e a confere (as regras do MOC e das NT, sem rede e sem gastar numero). `estrito`
/// recusa o pedido com campo que o leiaute nao conhece; sem ele, o campo e' ignorado e volta em
/// [`Preparada::desconhecidos`].
///
/// # Errors
///
/// [`Erro::Entrada`] e [`Erro::Uso`] como em [`completar_pedido`]; [`Erro::Recusa`] com o campo e o codigo da rejeicao
/// que a SEFAZ devolveria.
pub fn preparar(
    config: &Config,
    notas: Option<&NotasNfe>,
    mut pedido: Value,
    agora: DateTime<Utc>,
    estrito: bool,
    epec: bool,
) -> Result<Preparada, Erro> {
    // a serie do pedido (ou a da configuracao) e' a que numera
    let serie = if ausente_no(&pedido, "serie") {
        config.nfe.serie.clone()
    } else {
        serie_do_pedido(&pedido)
    };
    let proximo = notas.map(|n| n.proximo_numero(&serie)).transpose()?;
    completar_pedido(&mut pedido, config, proximo, agora, epec)?;
    let desconhecidos = montagem::campos_desconhecidos(&pedido);
    if estrito {
        if let Some(primeiro) = desconhecidos.first() {
            return Err(Erro::recusa(
                primeiro.clone(),
                format!(
                    "campo desconhecido no pedido: {} (confira o nome em docs/NFE_PEDIDO.md)",
                    desconhecidos.join(", ")
                ),
            ));
        }
    }
    let nota = montagem::nota_de_json(&pedido)?;
    let rejeicoes = montagem::conferir(&nota);
    if !rejeicoes.is_empty() {
        return Err(Erro::recusa("", rejeicoes.join("; ")));
    }
    Ok(Preparada {
        serie: nota.serie.to_string(),
        numero: nota.numero.max(0) as u64,
        pedido,
        nota,
        desconhecidos,
    })
}

fn ausente_no(pedido: &Value, chave: &str) -> bool {
    pedido.as_object().is_none_or(|o| ausente(o, chave))
}

/// A NF-e montada a partir do pedido, SEM assinar, sem rede e sem gravar nada.
#[derive(Debug, Clone)]
pub struct Montado {
    /// O XML da `NFe` (sem `Signature`).
    pub xml: String,
    /// A chave de acesso.
    pub chave: String,
    /// Serie.
    pub serie: String,
    /// Numero.
    pub numero: u64,
    /// Os totais do `ICMSTot`, em texto.
    pub totais: std::collections::BTreeMap<String, String>,
    /// Os avisos da montagem.
    pub avisos: Vec<Aviso>,
    /// Campos do pedido que o leiaute nao conhece (ignorados).
    pub desconhecidos: Vec<String>,
}

/// Completa o pedido, confere e monta o XML da NF-e, com o relogio do sistema. Nao assina, nao envia e NAO reserva o
/// numero: o que a pasta diz e' so' o proximo livre, e o `cNF` sorteado muda a cada chamada.
///
/// # Errors
///
/// Os de [`preparar`].
pub fn montar_pedido(
    config: &Config,
    notas: Option<&NotasNfe>,
    pedido: Value,
    estrito: bool,
) -> Result<Montado, Erro> {
    let p = preparar(config, notas, pedido, Utc::now(), estrito, false)?;
    let m = montagem::montar(&p.nota)?;
    Ok(Montado {
        xml: m.xml,
        chave: m.chave,
        serie: p.serie,
        numero: p.numero,
        totais: m.totais,
        avisos: m.avisos,
        desconhecidos: p.desconhecidos,
    })
}

/// Os avisos da montagem em JSON: codigo, item e texto.
#[must_use]
pub fn avisos_json(avisos: &[Aviso]) -> Value {
    Value::Array(
        avisos
            .iter()
            .map(|a| json!({"codigo": a.tipo.codigo(), "item": a.item, "mensagem": a.mensagem}))
            .collect(),
    )
}

/// O resultado da conferencia em JSON: `ok`, `chave`, `serie`, `numero`, `totais`, `avisos` (e `campos_desconhecidos`)
/// se o pedido monta; a recusa (`codigo`, `mensagem`, `campo`) com `ok: false` se nao.
#[must_use]
pub fn conferencia_json(r: &Result<Montado, Erro>) -> Value {
    match r {
        Ok(m) => {
            let mut v = json!({
                "ok": true,
                "chave": m.chave,
                "serie": m.serie,
                "numero": m.numero,
                "totais": m.totais,
                "avisos": avisos_json(&m.avisos),
            });
            if !m.desconhecidos.is_empty() {
                v["campos_desconhecidos"] = json!(m.desconhecidos);
            }
            v
        }
        Err(e) => {
            let mut v = e.envelope()["erro"].clone();
            v["ok"] = json!(false);
            v
        }
    }
}

/// O `dhEmi` que a emissao usa quando o pedido nao traz o seu (agora, menos a folga do relogio), em Brasilia.
pub(crate) fn emissao_agora(config: &Config, agora: DateTime<Utc>) -> String {
    iso(agora - Duration::seconds(i64::from(config.sefin.folga_relogio_s)))
}
