// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A emissao: pedido -> completa com a configuracao -> confere -> monta e assina -> GRAVA como pendente -> envia -> grava o
//! resultado.
//!
//! A gravacao antes do envio e' o que impede a nota gemea: a chave e o XML assinado ja estao na pasta quando a rede cai, e
//! o que se faz depois (retomada) e' consultar a MESMA chave ou reenviar o MESMO XML.

use rustnf::nfe::montagem::{self, Aviso};
use rustnf::nfe::sefaz::{Desfecho, Resultado, Transporte};
use serde_json::{json, Value};

use super::pedido::{avisos_json, preparar, Preparada};
use super::resposta::{erro_do_resultado, json_do_resultado};
use super::Contexto;
use crate::erro::Erro;
use crate::notas::nfe::{NotasNfe, Ocupante, Registro, Situacao};
use crate::notas::ErroNotas;

/// O que a emissao fez: o resultado da biblioteca e o que ficou na pasta.
#[derive(Debug, Clone)]
pub struct Emitida {
    /// Chave de acesso (44 posicoes).
    pub chave: String,
    /// Serie.
    pub serie: String,
    /// Numero da nota.
    pub numero: u64,
    /// O resultado da biblioteca (com o `nfeProc`, se autorizada).
    pub resultado: Resultado,
    /// Os avisos da montagem: nunca impedem a nota.
    pub avisos: Vec<Aviso>,
    /// Campos do pedido que o leiaute nao conhece e foram ignorados.
    pub desconhecidos: Vec<String>,
    /// A situacao da nota na pasta depois do resultado; `None` = a SEFAZ rejeitou e a nota saiu da pasta (o numero esta
    /// livre).
    pub situacao: Option<Situacao>,
}

impl Emitida {
    /// A frase para a pessoa: o resultado da SEFAZ e o que aconteceu com a nota na pasta.
    #[must_use]
    pub fn frase(&self) -> String {
        let r = &self.resultado;
        match (&r.desfecho, self.situacao) {
            (Desfecho::Autorizada { .. }, _) => {
                format!("{} A nota esta guardada na pasta de notas.", r.frase)
            }
            (Desfecho::Rejeitada { .. }, None) => format!(
                "{} O numero {} da serie {} continua livre: corrija o pedido e emita de novo.",
                r.frase, self.numero, self.serie
            ),
            (_, Some(Situacao::Pendente)) => format!(
                "{} A NF-e ficou PENDENTE na pasta de notas (chave {}): retome os pendentes para consultar a MESMA \
                 chave e nao emita outra nota no lugar.",
                r.frase, self.chave
            ),
            _ => r.frase.clone(),
        }
    }

    /// O JSON da emissao: o do resultado mais `serie`, `numero`, `situacao`, `guardada`, `avisos` e
    /// `campos_desconhecidos`.
    #[must_use]
    pub fn para_json(&self) -> Value {
        let mut v = json_do_resultado(&self.resultado);
        v["frase"] = json!(self.frase());
        v["serie"] = json!(self.serie);
        v["numero"] = json!(self.numero);
        v["situacao"] = json!(self.situacao.map(Situacao::texto));
        v["guardada"] = json!(self.situacao.is_some());
        v["avisos"] = avisos_json(&self.avisos);
        if !self.desconhecidos.is_empty() {
            v["campos_desconhecidos"] = json!(self.desconhecidos);
        }
        v
    }

    /// O erro do resultado (`None` = autorizada): recusa de regra (saida 3) ou falha de rede (saida 4), com a frase
    /// completa.
    #[must_use]
    pub fn erro(&self) -> Option<Erro> {
        erro_do_resultado(&self.resultado).map(|e| match e {
            Erro::Recusa { campo, .. } => Erro::recusa(campo, self.frase()),
            Erro::Rede(_) => Erro::Rede(self.frase()),
            outro => outro,
        })
    }
}

/// O numero esta com alguem? A resposta e' uma recusa em palavras, antes de assinar e de gastar qualquer coisa.
fn conferir_o_numero(notas: &NotasNfe, p: &Preparada) -> Result<(), Erro> {
    let proximo = notas.proximo_numero(&p.serie)?;
    let msg = match notas.ocupante(&p.serie, p.numero)? {
        None => return Ok(()),
        Some(Ocupante::Nota(chave, Situacao::Pendente)) => format!(
            "o numero {} da serie {} esta com uma NF-e pendente (chave {chave}): retome os pendentes para saber o que a \
             SEFAZ fez antes de emitir",
            p.numero, p.serie
        ),
        Some(Ocupante::Nota(chave, situacao)) => format!(
            "o numero {} da serie {} ja foi gasto pela nota de chave {chave} ({}): o proximo livre e' {proximo}",
            p.numero,
            p.serie,
            situacao.texto()
        ),
        Some(Ocupante::Inutilizada) => format!(
            "o numero {} da serie {} foi inutilizado: o proximo livre e' {proximo}",
            p.numero, p.serie
        ),
    };
    Err(Erro::recusa("nNF", msg))
}

/// Monta, assina e GRAVA como pendente a nota do pedido. Devolve o pedido preparado, a NF-e assinada e a linha.
pub(super) fn preparar_e_gravar<T: Transporte>(
    ctx: &Contexto<'_, T>,
    pedido: Value,
    estrito: bool,
    epec: bool,
) -> Result<(Preparada, String, Registro), Erro> {
    let notas = ctx.pasta()?;
    let p = preparar(ctx.config, Some(notas), pedido, ctx.agora, estrito, epec)?;
    conferir_o_numero(notas, &p)?;
    let montada = montagem::montar(&p.nota)?;
    let assinada = montagem::assinar(&montada, ctx.certificado)?;
    let lida = rustnf::nfe::ler(assinada.as_bytes()).map_err(|_| Erro::Interno)?;
    let registro = Registro {
        desde: ctx.segundos(),
        ..Registro::da_nfe(&lida, Situacao::Pendente)
    };
    notas.guardar_pendente(&registro, &assinada)?;
    Ok((p, assinada, registro))
}

/// Emite a NF-e do pedido. O pedido e' completado com `[emitente]` e `[nfe]` (o que ele traz vale) e numerado pela pasta
/// de notas; `estrito` recusa campo desconhecido.
///
/// # Errors
///
/// [`Erro::Uso`] sem pasta de notas ou sem dado de configuracao; [`Erro::Entrada`] para pedido ilegivel; [`Erro::Recusa`]
/// para o que o leiaute ou as regras recusam (antes de gastar numero) ou numero ja gasto. O que a SEFAZ responde volta
/// em [`Emitida`], inclusive a rejeicao.
pub async fn emitir<T: Transporte>(
    ctx: &Contexto<'_, T>,
    pedido: Value,
    estrito: bool,
) -> Result<Emitida, Erro> {
    let (p, assinada, registro) = preparar_e_gravar(ctx, pedido, estrito, false)?;
    let notas = ctx.pasta()?;
    let montada_avisos = montagem::avisos(&p.nota);
    let resultado = ctx.cliente.autorizar(&assinada).await?;
    let situacao = aplicar_resultado(notas, &registro, &resultado)?;
    Ok(Emitida {
        chave: registro.chave,
        serie: p.serie,
        numero: p.numero,
        resultado,
        avisos: montada_avisos,
        desconhecidos: p.desconhecidos,
        situacao,
    })
}

/// Grava na pasta o que o resultado da SEFAZ diz da nota e devolve a situacao dela depois: `None` quando a nota foi
/// rejeitada e saiu da pasta.
///
/// * autorizada, denegada, cancelada: guarda o `nfeProc` e gasta o numero;
/// * rejeitada: tira a pendente, o numero volta a ficar livre;
/// * numero ja autorizado com outra chave (539): tira a pendente e GASTA o numero (a SEFAZ o tem);
/// * sem resposta, servico parado, lote em processamento, so' EPEC: fica como esta.
pub(super) fn aplicar_resultado(
    notas: &NotasNfe,
    nota: &Registro,
    r: &Resultado,
) -> Result<Option<Situacao>, ErroNotas> {
    let atual = notas.registro(&nota.chave)?;
    // a consulta nunca desfaz um cancelamento
    if atual.situacao == Situacao::Cancelada && matches!(r.desfecho, Desfecho::Autorizada { .. }) {
        return Ok(Some(atual.situacao));
    }
    let (situacao, proc, protocolo, cstat) = match &r.desfecho {
        Desfecho::Autorizada {
            cstat,
            protocolo,
            nfe_proc,
            ..
        } => (Situacao::Autorizada, nfe_proc.as_deref(), protocolo, cstat),
        Desfecho::Denegada {
            cstat,
            protocolo,
            nfe_proc,
            ..
        } => (Situacao::Denegada, nfe_proc.as_deref(), protocolo, cstat),
        Desfecho::Cancelada {
            cstat,
            protocolo,
            nfe_proc,
            ..
        } => (Situacao::Cancelada, nfe_proc.as_deref(), protocolo, cstat),
        Desfecho::Rejeitada { .. } | Desfecho::NumeroJaUsado { .. } => {
            if atual.situacao != Situacao::Pendente {
                // a nota em EPEC segue la: o EPEC existe no Ambiente Nacional
                return Ok(Some(atual.situacao));
            }
            notas.descartar_pendente(&nota.chave)?;
            if matches!(r.desfecho, Desfecho::NumeroJaUsado { .. }) {
                notas.consumir_numero(&nota.serie, nota.numero_inteiro())?;
            }
            return Ok(None);
        }
        _ => return Ok(Some(atual.situacao)),
    };
    notas.concluir(&nota.chave, situacao, proc, protocolo, cstat)?;
    Ok(Some(situacao))
}
