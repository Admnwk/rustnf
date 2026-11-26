// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A distribuicao no cliente: `distribuir`, `distribuir_nsu`, `distribuir_chave` e `sincronizar`.
//!
//! A regra que o cliente nao quebra: o cursor so' anda para o `ultNSU` que o Ambiente Nacional DEVOLVEU, nunca pula um
//! NSU (o que faltar no meio do lote vira lacuna, buscada por `consNSU`), nunca entrega o mesmo NSU duas vezes, nunca
//! volta e nunca chama antes da espera da NT 2014.002 v1.02d (uma hora depois do 137, do `ultNSU` igual ao `maxNSU` e
//! do 656). Quem chama PERSISTE: a cada lote, os documentos e o cursor novo na MESMA gravacao.

use chrono::{DateTime, Utc};

use super::super::cliente::{ClienteSefaz, Resp};
use super::super::erro::ErroSefaz;
use super::super::tabela::Servico;
use super::super::transporte::Transporte;
use super::cursor::{Cursor, Espera, FaixaNsu, MotivoEspera, Nsu, ESPERA};
use super::documento::{abrir_doc, Doc};
use super::pedido::{dist_dfe_int, Consulta};
use super::resultado::{hora_de_brasilia, DesfechoDfe, ResultadoDfe};
use super::retorno::{ler_ret_dist_dfe, RetDistDfe};

/// A faixa de NSU que falta e e' maior que isto nao entra no cursor: o Ambiente Nacional so' guarda os documentos por 90
/// dias (NT 2014.002 v1.02d, item 3.5), entao um salto grande e' o vencimento do que nao foi buscado a tempo, nao uma
/// falha de entrega. Vai em [`ResultadoDfe::irrecuperaveis`].
pub const LACUNA_MAXIMA: u64 = 500;

/// Quantos NSU em lacuna `sincronizar` busca por chamada (o resto fica no cursor para a rodada seguinte).
pub const LACUNAS_POR_RODADA: usize = 50;

/// Falha de `sincronizar`: o pedido nao saiu da maquina, ou a aplicacao nao conseguiu gravar o lote.
#[derive(Debug)]
#[non_exhaustive]
pub enum ErroSincronizacao<E> {
    /// O pedido nao monta (CNPJ, UF, chave...).
    Sefaz(ErroSefaz),
    /// O `gravar` da aplicacao falhou: o laco parou SEM avancar o cursor desse lote.
    Gravar(E),
}

impl<E> From<ErroSefaz> for ErroSincronizacao<E> {
    fn from(e: ErroSefaz) -> Self {
        ErroSincronizacao::Sefaz(e)
    }
}

/// As faixas de NSU que faltam entre o que o cursor ja cobria e os documentos do lote (que chegam em ordem e sem repeticao).
fn lacunas_do_lote(anterior: Nsu, nsus: &[Nsu], ult_novo: Nsu) -> (Vec<FaixaNsu>, Vec<FaixaNsu>) {
    let mut faixas = Vec::new();
    let mut esperado = (anterior != Nsu::ZERO).then(|| anterior.proximo());
    for &n in nsus {
        if let Some(e) = esperado.filter(|e| n > *e) {
            faixas.push(FaixaNsu {
                inicio: e,
                fim: n.anterior(),
            });
        }
        esperado = Some(n.proximo());
    }
    if let Some(e) = esperado.filter(|e| ult_novo >= *e) {
        faixas.push(FaixaNsu {
            inicio: e,
            fim: ult_novo,
        });
    }
    faixas
        .into_iter()
        .partition(|f| f.tamanho() <= LACUNA_MAXIMA)
}

/// Junta as faixas novas as do cursor, em ordem e sem sobrepor.
fn somar_lacunas(cursor: &mut Cursor, novas: Vec<FaixaNsu>) {
    cursor.lacunas.extend(novas);
    cursor.lacunas.sort_by_key(|f| f.inicio);
    let mut juntas: Vec<FaixaNsu> = Vec::with_capacity(cursor.lacunas.len());
    for f in cursor.lacunas.drain(..) {
        match juntas.last_mut() {
            Some(u) if f.inicio <= u.fim.proximo() => u.fim = u.fim.max(f.fim),
            _ => juntas.push(f),
        }
    }
    cursor.lacunas = juntas;
}

fn frase_da_espera(ate: DateTime<Utc>, motivo: MotivoEspera) -> String {
    let hora = hora_de_brasilia(ate);
    match motivo {
        MotivoEspera::FilaEmDia => format!(
            "Aguarde ate {hora} (horario de Brasilia) para pedir a distribuicao de novo: a fila estava em dia e o Ambiente \
             Nacional manda esperar uma hora. Nada foi enviado."
        ),
        MotivoEspera::ConsumoIndevido => format!(
            "Aguarde ate {hora} (horario de Brasilia): o Ambiente Nacional bloqueou este CNPJ ou CPF por consumo \
             indevido (656). Nada foi enviado."
        ),
    }
}

impl<T: Transporte> ClienteSefaz<T> {
    fn agora_utc(&self) -> DateTime<Utc> {
        self.agora().with_timezone(&Utc)
    }

    fn sem_resposta(cursor: &Cursor, frase: &str, detalhe: String) -> ResultadoDfe {
        ResultadoDfe {
            detalhe,
            ..ResultadoDfe::novo(DesfechoDfe::Indisponivel, cursor.clone(), frase.to_string())
        }
    }

    /// UM lote da distribuicao a partir do cursor (`distNSU` com o `ultNSU` que o Ambiente Nacional devolveu da ultima
    /// vez). `documento` e' o CNPJ ou CPF do interessado (o CNPJ-base tem de ser o do certificado, rejeicao 593).
    ///
    /// # Errors
    ///
    /// [`ErroSefaz`] so' se o pedido nao monta (CNPJ ou CPF invalido, UF desconhecida); falha de rede e resposta ruim
    /// voltam em [`DesfechoDfe`].
    pub async fn distribuir(
        &self,
        documento: &str,
        cursor: &Cursor,
    ) -> Result<ResultadoDfe, ErroSefaz> {
        self.consultar_dfe(documento, cursor, Consulta::Distribuicao(cursor.ult_nsu))
            .await
    }

    /// `consNSU`: le UM NSU (o que faltou no meio, ou o que veio ilegivel). O NSU sai das lacunas do cursor; o `ultNSU`
    /// nao mexe.
    ///
    /// # Errors
    ///
    /// [`ErroSefaz`] so' se o pedido nao monta.
    pub async fn distribuir_nsu(
        &self,
        documento: &str,
        cursor: &Cursor,
        nsu: Nsu,
    ) -> Result<ResultadoDfe, ErroSefaz> {
        self.consultar_dfe(documento, cursor, Consulta::Nsu(nsu))
            .await
    }

    /// `consChNFe`: a NF-e da chave (so' ela, sem eventos). O destinatario que ainda nao manifestou recebe so' o resumo.
    /// Se o Ambiente Nacional responder 656, a espera vai no cursor devolvido.
    ///
    /// # Errors
    ///
    /// [`ErroSefaz`] so' se o pedido nao monta (a chave tem de ser de uma NF-e de modelo 55 valida).
    pub async fn distribuir_chave(
        &self,
        documento: &str,
        cursor: &Cursor,
        chave: &str,
    ) -> Result<ResultadoDfe, ErroSefaz> {
        self.consultar_dfe(documento, cursor, Consulta::Chave(chave))
            .await
    }

    async fn consultar_dfe(
        &self,
        documento: &str,
        cursor: &Cursor,
        consulta: Consulta<'_>,
    ) -> Result<ResultadoDfe, ErroSefaz> {
        let agora = self.agora_utc();
        let distribuicao = matches!(consulta, Consulta::Distribuicao(_));
        // o pedido se monta antes: pedido torto e' erro de uso, com ou sem espera
        let pedido = dist_dfe_int(self.ambiente, self.uf, documento, consulta)?;
        if let Some(e) = cursor.espera_para(distribuicao, agora) {
            return Ok(ResultadoDfe::novo(
                DesfechoDfe::Aguarde {
                    ate: e.ate,
                    motivo: e.motivo,
                },
                cursor.clone(),
                frase_da_espera(e.ate, e.motivo),
            ));
        }
        let r = match self.chamar(Servico::DistribuicaoDfe, &pedido, "AN").await? {
            Resp::Ok(r) => r,
            Resp::Sem(s) => {
                return Ok(Self::sem_resposta(
                    cursor,
                    "O Ambiente Nacional nao respondeu agora: nada foi gravado, tente de novo.",
                    s.detalhe,
                ));
            }
        };
        let ret = match ler_ret_dist_dfe(&r.texto) {
            Ok(x) => x,
            Err(e) => {
                return Ok(Self::sem_resposta(
                    cursor,
                    "A resposta do Ambiente Nacional veio ilegivel: nada foi gravado, tente de novo.",
                    e.to_string(),
                ));
            }
        };
        let mut res = self.desfecho_da_resposta(cursor, consulta, &ret, agora);
        res.cstat.clone_from(&ret.cstat);
        res.motivo.clone_from(&ret.motivo);
        res.xml_retorno = r.texto;
        Ok(res)
    }

    fn desfecho_da_resposta(
        &self,
        cursor: &Cursor,
        consulta: Consulta<'_>,
        ret: &RetDistDfe,
        agora: DateTime<Utc>,
    ) -> ResultadoDfe {
        let (cstat, motivo) = (ret.cstat.as_str(), ret.motivo.as_str());
        match cstat {
            "656" => {
                let ate = agora + ESPERA;
                let mut novo = cursor.clone();
                novo.espera = Some(Espera {
                    ate,
                    motivo: MotivoEspera::ConsumoIndevido,
                });
                ResultadoDfe::novo(
                    DesfechoDfe::ConsumoIndevido { ate },
                    novo,
                    format!(
                        "Consumo indevido (656): {motivo}. O Ambiente Nacional bloqueou este CNPJ ou CPF por uma hora; tente \
                         de novo depois das {} (horario de Brasilia) e nao antes.",
                        hora_de_brasilia(ate)
                    ),
                )
            }
            "108" | "109" => Self::sem_resposta(
                cursor,
                "O servico de distribuicao do Ambiente Nacional esta parado: nada foi gravado, tente mais tarde.",
                format!("{cstat}: {motivo}"),
            ),
            "137" | "138" => self.aplicar_lote(cursor, consulta, ret, agora),
            _ => ResultadoDfe::novo(
                DesfechoDfe::Rejeitada {
                    cstat: cstat.to_string(),
                    motivo: motivo.to_string(),
                },
                cursor.clone(),
                format!("O Ambiente Nacional recusou a consulta ({cstat}): {motivo}"),
            ),
        }
    }

    /// 137 ou 138: abre os documentos e decide o cursor novo.
    fn aplicar_lote(
        &self,
        cursor: &Cursor,
        consulta: Consulta<'_>,
        ret: &RetDistDfe,
        agora: DateTime<Utc>,
    ) -> ResultadoDfe {
        let distribuicao = matches!(consulta, Consulta::Distribuicao(_));
        let mut docs = Vec::with_capacity(ret.docs.len());
        for z in &ret.docs {
            let nsu = match Nsu::ler(&z.nsu) {
                Ok(n) => n,
                // o NSU do docZip e' opcional no esquema; na distribuicao sem ele nao ha como seguir a sequencia
                Err(_) if distribuicao => {
                    return Self::sem_resposta(
                        cursor,
                        "Um documento do lote veio sem NSU valido: nada foi gravado, tente de novo.",
                        format!("docZip com NSU {:?}", z.nsu.chars().take(20).collect::<String>()),
                    );
                }
                Err(_) => Nsu::ZERO,
            };
            docs.push(abrir_doc(nsu, &z.esquema, &z.base64));
        }
        let Consulta::Distribuicao(pedido_ult) = consulta else {
            return Self::aplicar_pontual(cursor, consulta, ret, docs);
        };
        self.aplicar_distribuicao(cursor, pedido_ult, ret, docs, agora)
    }

    /// `consNSU` e `consChNFe`: o cursor so' perde a lacuna, e o `ultNSU` nao mexe.
    fn aplicar_pontual(
        cursor: &Cursor,
        consulta: Consulta<'_>,
        ret: &RetDistDfe,
        docs: Vec<Doc>,
    ) -> ResultadoDfe {
        let mut novo = cursor.clone();
        let (onde, nada) = match consulta {
            Consulta::Nsu(n) => {
                novo.resolver(n);
                (
                    "NSU",
                    "O Ambiente Nacional nao tem esse NSU: desconsidere-o.",
                )
            }
            _ => (
                "chave",
                "O Ambiente Nacional nao tem NF-e dessa chave para este interessado.",
            ),
        };
        if ret.cstat == "137" || docs.is_empty() {
            return ResultadoDfe::novo(DesfechoDfe::NaoLocalizado, novo, nada.to_string());
        }
        let n = docs.len();
        ResultadoDfe {
            documentos: docs,
            ..ResultadoDfe::novo(
                DesfechoDfe::Localizado,
                novo,
                format!("{n} documento(s) localizado(s) pelo {onde}."),
            )
        }
    }

    fn aplicar_distribuicao(
        &self,
        cursor: &Cursor,
        pedido_ult: Nsu,
        ret: &RetDistDfe,
        mut docs: Vec<Doc>,
        agora: DateTime<Utc>,
    ) -> ResultadoDfe {
        let (Ok(ult), Ok(max)) = (Nsu::ler(&ret.ult_nsu), Nsu::ler(&ret.max_nsu)) else {
            return Self::sem_resposta(
                cursor,
                "A resposta do Ambiente Nacional veio sem NSU valido: nada foi gravado, tente de novo.",
                format!("ultNSU={:?} maxNSU={:?}", ret.ult_nsu, ret.max_nsu),
            );
        };
        docs.sort_by_key(|d| d.nsu);
        let ultimo_doc = docs.last().map(|d| d.nsu);
        if ult < pedido_ult || ultimo_doc.is_some_and(|u| u > ult) {
            return Self::sem_resposta(
                cursor,
                "O Ambiente Nacional devolveu NSU fora de ordem: nada foi gravado, tente de novo.",
                format!(
                    "pedido a partir de {pedido_ult}, ultNSU {ult}, ultimo documento {}",
                    ultimo_doc.map_or_else(|| "nenhum".to_string(), |u| u.to_string())
                ),
            );
        }
        if ret.cstat == "138" && ult == pedido_ult && ult < max {
            return Self::sem_resposta(
                cursor,
                "O Ambiente Nacional respondeu sem avancar o NSU: nada foi gravado, tente de novo.",
                format!("ultNSU {ult} igual ao pedido, maxNSU {max}"),
            );
        }
        // o que o cursor ja cobria nao e' entregue de novo; NSU repetido no mesmo lote tambem nao
        let mut repetidos = Vec::new();
        let mut novos: Vec<Doc> = Vec::with_capacity(docs.len());
        for d in docs {
            let ja = pedido_ult != Nsu::ZERO && d.nsu <= pedido_ult;
            if ja || novos.last().is_some_and(|u| u.nsu == d.nsu) {
                repetidos.push(d.nsu);
            } else {
                novos.push(d);
            }
        }
        let nsus: Vec<Nsu> = novos.iter().map(|d| d.nsu).collect();
        let (faixas, irrecuperaveis) = lacunas_do_lote(pedido_ult, &nsus, ult);
        let mut novo = cursor.clone();
        novo.ult_nsu = ult;
        novo.max_nsu = Some(max);
        somar_lacunas(&mut novo, faixas);
        let em_dia = ret.cstat == "137" || ult >= max;
        let n = novos.len();
        let (desfecho, frase) = if em_dia {
            let ate = agora + ESPERA;
            novo.espera = Some(Espera {
                ate,
                motivo: MotivoEspera::FilaEmDia,
            });
            (
                DesfechoDfe::EmDia,
                format!(
                    "{n} documento(s) novo(s); a fila esta em dia. Peca de novo so' depois das {} (horario de Brasilia): o \
                     Ambiente Nacional manda esperar uma hora.",
                    hora_de_brasilia(ate)
                ),
            )
        } else {
            novo.espera = None;
            (
                DesfechoDfe::Parcial,
                format!("{n} documento(s) novo(s); ha mais na fila: peca de novo com o cursor devolvido."),
            )
        };
        let tem_lacuna = novo.tem_lacunas();
        let mut r = ResultadoDfe {
            documentos: novos,
            repetidos,
            irrecuperaveis,
            ..ResultadoDfe::novo(desfecho, novo, frase)
        };
        if tem_lacuna {
            r.frase
                .push_str(" Faltaram NSU no meio: a biblioteca os busca um a um (consNSU).");
        }
        r
    }

    /// Le a fila inteira, lote a lote, ate ela acabar (ou `max_lotes`), e fecha as lacunas por `consNSU`. A CADA lote (e a
    /// cada `consNSU`) chama `gravar(documentos, cursor_novo)`: grave os dois na MESMA transacao; erro em `gravar` para o
    /// laco sem avancar. O 656 tambem passa por `gravar` (sem documentos), para a espera ficar gravada.
    ///
    /// Devolve o resultado do ultimo lote, com TODOS os documentos lidos e o cursor final.
    ///
    /// # Errors
    ///
    /// [`ErroSincronizacao::Sefaz`] se o pedido nao monta; [`ErroSincronizacao::Gravar`] se `gravar` falhou.
    pub async fn sincronizar<E>(
        &self,
        documento: &str,
        cursor: &Cursor,
        mut gravar: impl FnMut(&[Doc], &Cursor) -> Result<(), E>,
        max_lotes: usize,
    ) -> Result<ResultadoDfe, ErroSincronizacao<E>> {
        let mut cur = cursor.clone();
        let mut todos: Vec<Doc> = Vec::new();
        let mut lotes = 0;
        let mut lacunas_buscadas = 0;
        loop {
            let mut r = self.distribuir(documento, &cur).await?;
            if matches!(
                r.desfecho,
                DesfechoDfe::Parcial | DesfechoDfe::EmDia | DesfechoDfe::ConsumoIndevido { .. }
            ) {
                gravar(&r.documentos, &r.cursor).map_err(ErroSincronizacao::Gravar)?;
                cur = r.cursor.clone();
                todos.append(&mut r.documentos);
                lotes += 1;
            }
            // a espera da fila nao barra o consNSU: as lacunas se fecham mesmo com a distribuicao em espera
            if matches!(
                r.desfecho,
                DesfechoDfe::Parcial | DesfechoDfe::EmDia | DesfechoDfe::Aguarde { .. }
            ) {
                let falha = self
                    .fechar_lacunas(
                        documento,
                        &mut cur,
                        &mut todos,
                        &mut gravar,
                        &mut lacunas_buscadas,
                    )
                    .await?;
                if let Some(f) = falha {
                    r.detalhe = format!("{} | lacuna: {f}", r.detalhe);
                }
            }
            if r.desfecho != DesfechoDfe::Parcial || lotes >= max_lotes {
                r.lotes = lotes;
                r.cursor = cur;
                if r.desfecho == DesfechoDfe::Parcial {
                    r.frase = format!(
                        "{} documento(s) em {lotes} lote(s); ha mais na fila (proxima rodada).",
                        todos.len()
                    );
                }
                r.documentos = todos;
                return Ok(r);
            }
        }
    }

    /// Busca por `consNSU` as lacunas do cursor (ate [`LACUNAS_POR_RODADA`] por chamada de `sincronizar`). Devolve o
    /// motivo, em palavras, se parou antes de esvaziar a lista.
    async fn fechar_lacunas<E>(
        &self,
        documento: &str,
        cur: &mut Cursor,
        todos: &mut Vec<Doc>,
        gravar: &mut impl FnMut(&[Doc], &Cursor) -> Result<(), E>,
        buscadas: &mut usize,
    ) -> Result<Option<String>, ErroSincronizacao<E>> {
        while let Some(nsu) = cur.proxima_lacuna() {
            if *buscadas >= LACUNAS_POR_RODADA {
                return Ok(Some(format!(
                    "{LACUNAS_POR_RODADA} NSU buscados nesta rodada; o resto fica no cursor"
                )));
            }
            *buscadas += 1;
            let mut r = self.distribuir_nsu(documento, cur, nsu).await?;
            if !r.ok() {
                return Ok(Some(r.frase));
            }
            gravar(&r.documentos, &r.cursor).map_err(ErroSincronizacao::Gravar)?;
            *cur = r.cursor.clone();
            todos.append(&mut r.documentos);
        }
        Ok(None)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod testes {
    use super::*;

    fn n(v: u64) -> Nsu {
        Nsu::ler(&v.to_string()).unwrap()
    }

    #[test]
    fn lacuna_no_meio_do_lote_e_antes_do_primeiro_quando_o_cursor_ja_andou() {
        let (f, irrec) = lacunas_do_lote(n(10), &[n(12), n(13), n(16)], n(16));
        assert_eq!(
            f,
            vec![
                FaixaNsu {
                    inicio: n(11),
                    fim: n(11)
                },
                FaixaNsu {
                    inicio: n(14),
                    fim: n(15)
                },
            ]
        );
        assert!(irrec.is_empty());
    }

    #[test]
    fn primeiro_lote_nao_cobra_o_que_vem_antes_do_primeiro_documento() {
        let (f, _) = lacunas_do_lote(Nsu::ZERO, &[n(500), n(501)], n(501));
        assert!(f.is_empty());
        let (f, _) = lacunas_do_lote(n(7), &[n(8), n(9)], n(9));
        assert!(f.is_empty());
    }

    #[test]
    fn salto_grande_e_irrecuperavel_e_fica_fora_do_cursor() {
        let (f, irrec) = lacunas_do_lote(n(10), &[n(5000)], n(5000));
        assert!(f.is_empty());
        assert_eq!(
            irrec,
            vec![FaixaNsu {
                inicio: n(11),
                fim: n(4999)
            }]
        );
    }

    #[test]
    fn somar_junta_faixas_coladas() {
        let mut c = Cursor::novo();
        c.lacunas = vec![FaixaNsu {
            inicio: n(5),
            fim: n(6),
        }];
        somar_lacunas(
            &mut c,
            vec![
                FaixaNsu {
                    inicio: n(7),
                    fim: n(9),
                },
                FaixaNsu {
                    inicio: n(20),
                    fim: n(20),
                },
            ],
        );
        assert_eq!(
            c.lacunas,
            vec![
                FaixaNsu {
                    inicio: n(5),
                    fim: n(9)
                },
                FaixaNsu {
                    inicio: n(20),
                    fim: n(20)
                }
            ]
        );
    }
}
