// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Os comandos `config`, `senha` e `notas`, e o carregamento da configuracao que os outros comandos usam.

use std::path::Path;

use rustnf_app::cofre::{chave_do_certificado, Cofre};
use rustnf_app::config::{caminho_do_arquivo, Efetiva};
use rustnf_app::notas::{Filtro, Notas, Situacao, INDICE_ANTERIOR, LISTA_CSV};
use serde_json::json;

use super::comandos::{escrever_json, escrever_stdout};
use super::{
    certificado, resolver, AcaoConfig, AcaoNotas, AcaoSenha, ArgCofre, ArgGuardar, SituacaoDaNota,
};
use crate::erro::Erro;

/// Acha o arquivo, aplica o ambiente e confere os valores.
pub(super) fn carregar(explicito: Option<&Path>) -> Result<Efetiva, Erro> {
    Ok(Efetiva::carregar(explicito)?)
}

/// A pasta de notas, se a configuracao tiver uma.
pub(super) fn notas_da(e: &Efetiva) -> Result<Option<Notas>, Erro> {
    if e.config.notas.pasta.is_empty() {
        return Ok(None);
    }
    Ok(Some(Notas::abrir(&e.config.notas.pasta)?))
}

pub(super) fn config(acao: AcaoConfig, explicito: Option<&Path>) -> Result<(), Erro> {
    match acao {
        AcaoConfig::Mostrar { json } => mostrar(&carregar(explicito)?, json),
        AcaoConfig::Caminho => caminho(explicito),
    }
}

fn mostrar(e: &Efetiva, json: bool) -> Result<(), Erro> {
    if json {
        return escrever_json(&json!({
            "arquivo": e.arquivo.as_ref().map(|p| p.to_string_lossy().into_owned()),
            "variaveis": e.variaveis,
            "config": serde_json::to_value(&e.config)?,
        }));
    }
    let mut texto = match &e.arquivo {
        Some(p) => format!("# arquivo: {}\n", p.to_string_lossy()),
        None => "# arquivo: nenhum (valores padrao)\n".to_string(),
    };
    if !e.variaveis.is_empty() {
        texto.push_str(&format!(
            "# variaveis de ambiente aplicadas: {}\n",
            e.variaveis.join(", ")
        ));
    }
    texto.push('\n');
    texto.push_str(&e.config.para_toml()?);
    escrever_stdout(texto.as_bytes())
}

fn caminho(explicito: Option<&Path>) -> Result<(), Erro> {
    let Some(p) = caminho_do_arquivo(explicito, &|n| std::env::var_os(n)) else {
        return Err(Erro::Uso(
            "nao foi possivel achar a pasta de configuracao do usuario (APPDATA, XDG_CONFIG_HOME ou HOME)"
                .into(),
        ));
    };
    if !p.is_file() {
        eprintln!("o arquivo ainda nao existe");
    }
    escrever_stdout(format!("{}\n", p.to_string_lossy()).as_bytes())
}

pub(super) fn senha(
    acao: AcaoSenha,
    explicito: Option<&Path>,
    cofre: &dyn Cofre,
) -> Result<(), Erro> {
    let e = carregar(explicito)?;
    match acao {
        AcaoSenha::Guardar(a) => guardar_senha(&a, &e, cofre),
        AcaoSenha::Apagar(a) => {
            cofre.apagar(&chave_do_item(&a, &e)?.0)?;
            eprintln!("senha apagada do cofre do sistema");
            Ok(())
        }
    }
}

/// A chave do item no cofre e o caminho do certificado, se houver.
fn chave_do_item(a: &ArgCofre, e: &Efetiva) -> Result<(String, Option<String>), Erro> {
    let pfx = resolver::pfx(e, a.pfx.as_deref()).ok();
    let chave = chave_do_certificado(&e.config.emitente, pfx.as_deref().unwrap_or(""))
        .ok_or_else(|| {
            Erro::Uso(
                "informe --pfx (ou [sefin].pfx) ou preencha [emitente].cnpj para identificar o certificado"
                    .into(),
            )
        })?;
    Ok((chave, pfx))
}

fn guardar_senha(a: &ArgGuardar, e: &Efetiva, cofre: &dyn Cofre) -> Result<(), Erro> {
    let (chave, pfx) = chave_do_item(&a.cofre, e)?;
    let senha = certificado::senha_explicita(&a.senha, false)?.ok_or_else(|| {
        Erro::Uso("informe a senha por --senha-env <VARIAVEL> ou --senha-stdin".into())
    })?;
    // so' guarda a senha que de fato abre o certificado
    if let Some(pfx) = pfx {
        certificado::conferir(&pfx, &senha)?;
    }
    cofre.guardar(&chave, &senha)?;
    eprintln!("senha guardada no cofre do sistema");
    Ok(())
}

/// A pasta de notas da configuracao; sem ela, erro de uso.
pub(super) fn notas_exigida(e: &Efetiva) -> Result<Notas, Erro> {
    notas_da(e)?.ok_or_else(|| {
        Erro::Uso(
            "sem pasta de notas: configure [notas].pasta no arquivo ou RUSTNF_NOTAS_PASTA".into(),
        )
    })
}

pub(super) fn notas(acao: AcaoNotas, explicito: Option<&Path>) -> Result<(), Erro> {
    let e = carregar(explicito)?;
    let notas = notas_exigida(&e)?;
    match acao {
        AcaoNotas::Exportar { mes, para } => {
            let lista = Filtro::do_mes(&mes)?.aplicar(notas.listar()?);
            if lista.is_empty() {
                eprintln!("nenhuma nota nesse mes: nada foi copiado");
                return Ok(());
            }
            let feito = notas.exportar(&lista, Path::new(&para))?;
            for chave in &feito.sem_xml {
                eprintln!("aviso: a nota {chave} esta na lista, mas o XML dela nao esta na pasta de notas");
            }
            escrever_stdout(
                format!(
                    "{} nota(s) na lista ({LISTA_CSV}), {} XML copiado(s)\n",
                    lista.len(),
                    feito.copiadas
                )
                .as_bytes(),
            )
        }
        AcaoNotas::Reindexar => {
            let feito = notas.reindexar()?;
            if feito.indice_ilegivel {
                eprintln!(
                    "aviso: o indice que havia estava ilegivel (ficou guardado como {INDICE_ANTERIOR}). O XML nao diz se a nota foi cancelada: todas voltaram como autorizadas; confira as canceladas na Sefin"
                );
            }
            if feito.ignorados > 0 {
                eprintln!(
                    "aviso: {} arquivo(s) .xml ficaram de fora (nao sao uma NFS-e legivel, ou estao fora da pasta do mes)",
                    feito.ignorados
                );
            }
            escrever_stdout(
                format!(
                    "indice refeito: {} nota(s), {} cancelada(s)\n",
                    feito.notas, feito.canceladas
                )
                .as_bytes(),
            )
        }
        AcaoNotas::Listar {
            json,
            csv,
            de,
            ate,
            situacao,
            busca,
        } => {
            let filtro = Filtro::novo(
                de.as_deref(),
                ate.as_deref(),
                situacao.map(|s| match s {
                    SituacaoDaNota::Autorizada => Situacao::Autorizada,
                    SituacaoDaNota::Cancelada => Situacao::Cancelada,
                }),
                busca.as_deref().unwrap_or(""),
            )?;
            let lista = filtro.aplicar(notas.listar()?);
            if json {
                return escrever_json(&serde_json::to_value(&lista)?);
            }
            if csv {
                return escrever_stdout(rustnf_app::notas::csv(&lista).as_bytes());
            }
            if lista.is_empty() {
                eprintln!("nenhuma nota na pasta");
            }
            for r in &lista {
                let data = r.emissao.get(..10).unwrap_or(&r.emissao);
                let linha = format!(
                    "{data}\tnfse {}\tserie {}\t{}\t{}\t{}\t{}\n",
                    r.numero,
                    r.serie,
                    r.valor_servico,
                    r.situacao.texto(),
                    r.tomador_nome,
                    r.chave
                );
                escrever_stdout(linha.as_bytes())?;
            }
            Ok(())
        }
    }
}
