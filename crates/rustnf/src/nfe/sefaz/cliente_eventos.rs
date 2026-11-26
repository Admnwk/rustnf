// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Os eventos no cliente: `cancelar`, `corrigir`, `manifestar`, `registrar_evento_rtc`, `registrar_epec` e `inutilizar`.
//!
//! A regra que o cliente nao quebra, a mesma da nota: resposta perdida NAO quer dizer evento nao registrado. Antes de
//! qualquer reenvio o cliente consulta a NF-e (`consSitNFe` devolve os eventos dela): achou o evento, devolve-o
//! (`recuperado`); a consulta respondeu e ele nao esta la, o reenvio do MESMO evento (mesmo tipo, mesmo `nSeqEvento`)
//! e' seguro; sem resposta nem da consulta, o resultado manda consultar de novo. Nenhum caminho cria um segundo
//! cancelamento ou uma carta com sequencia nova por causa de uma resposta perdida.
//!
//! Onde cada evento e' recebido: cancelamento, no autorizador da chave (a UF, ou a SVC se a chave tem `tpEmis` 6 ou 7);
//! carta de correcao, sempre na UF (a SVC nao a registra, MOC 7.00, Anexo III, 2.1.3.4); EPEC e manifestacao do
//! destinatario, no Ambiente Nacional (MOC 7.00, Visao Geral, 3.3 e 5.11); eventos da apuracao do IBS/CBS, na SVRS (NT
//! 2025.002 v1.53, item 8.1). A inutilizacao vai ao
//! autorizador da UF do emitente (a SVC nao a oferece, item e).

use chrono::Datelike;

use super::cliente::{ClienteSefaz, Resp};
use super::dfe::pedido::interessado;
use super::erro::ErroSefaz;
use super::evento::{
    self, Cancelamento, CartaCorrecao, Epec, EventoRtc, Manifestacao, PedidoEvento,
};
use super::inutilizacao::{self, Inutilizacao, PedidoInutilizacao, PedidoInutilizar};
use super::pedidos::chave_valida;
use super::resposta_evento::{
    self as re, ler_evento, ler_inutilizacao, proc_evento, proc_inutilizacao, InfRetEvento,
};
use super::resultado::Desfecho;
use super::resultado_evento::{
    frase_da_inutilizacao, frase_do_evento, DesfechoEvento, DesfechoInutilizacao, ResultadoEvento,
    ResultadoInutilizacao,
};
use super::tabela::Servico;
use super::transporte::Transporte;
use crate::assinatura::cert_a1::Certificado;

const DECLARACAO: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>";

/// O desfecho de um `retEvento` que registrou: 135, 155 ou 136 (que e' registro no EPEC e "sem vinculo" no resto).
fn registrado_do(ev: &InfRetEvento, tp_evento: &str) -> Option<DesfechoEvento> {
    let (cstat, motivo, protocolo) = (ev.cstat.clone(), ev.motivo.clone(), ev.protocolo.clone());
    match ev.cstat.as_str() {
        "135" => Some(DesfechoEvento::Registrado {
            fora_do_prazo: false,
            cstat,
            motivo,
            protocolo,
        }),
        // 155 e' "cancelamento homologado fora de prazo": so' o cancelamento o recebe
        "155" if tp_evento == evento::CANCELAMENTO => Some(DesfechoEvento::Registrado {
            fora_do_prazo: true,
            cstat,
            motivo,
            protocolo,
        }),
        "136" if tp_evento == evento::EPEC => Some(DesfechoEvento::Registrado {
            cstat,
            motivo,
            protocolo,
            fora_do_prazo: false,
        }),
        "136" => Some(DesfechoEvento::SemVinculo {
            cstat,
            motivo,
            protocolo,
        }),
        _ => None,
    }
}

/// A manifestacao e' assinada com o certificado do CNPJ-base (ou do CPF) do destinatario (MOC 7.00, Visao Geral, 5.11).
/// So' recusa quando o certificado DIZ de quem e' e nao bate: o que nao traz CNPJ nem CPF legivel fica para o Ambiente
/// Nacional decidir.
fn conferir_certificado(cert: &Certificado, documento: &str) -> Result<(), ErroSefaz> {
    let (tag, doc) = interessado(documento)?;
    let sem_identidade = cert.cnpj.is_empty() && cert.cpf.is_empty();
    let bate = sem_identidade
        || if tag == "CNPJ" {
            cert.cnpj
                .get(..8)
                .is_some_and(|base| base.eq_ignore_ascii_case(&doc[..8]))
        } else {
            cert.cpf == doc
        };
    if bate {
        return Ok(());
    }
    Err(ErroSefaz::novo(
        "o certificado nao e' do destinatario: a manifestacao e' assinada com o certificado do mesmo CNPJ-base (ou CPF) \
         do destinatario (MOC 7.00, Visao Geral, 5.11)",
        "certificado",
    ))
}

/// O `retEvento` que e' deste pedido: mesmo tipo, mesma sequencia, mesma chave (o que a SEFAZ nao mandou nao conta).
fn do_pedido<'a>(eventos: &'a [InfRetEvento], p: &PedidoEvento) -> Option<&'a InfRetEvento> {
    eventos.iter().find(|e| {
        (e.tp_evento.is_empty() || e.tp_evento == p.tp_evento)
            && (e.seq.is_none() || e.seq == Some(p.seq))
            && (e.chave.is_empty() || e.chave == p.chave)
    })
}

impl<T: Transporte> ClienteSefaz<T> {
    // ------------------------------------------------------------------------------------------------- eventos

    /// Cancela a NF-e (evento 110111) no autorizador da chave. `protocolo` e' o `nProt` da AUTORIZACAO da nota.
    ///
    /// O prazo nao e' conferido aqui: e' o da legislacao da UF (MOC 7.00, Visao Geral, regra 2P12-14 e item 5.9.4). Fora
    /// dele a SEFAZ responde 501 ([`DesfechoEvento::PrazoVencido`]) ou homologa mesmo assim com 155
    /// ([`DesfechoEvento::Registrado`] com `fora_do_prazo`).
    ///
    /// # Errors
    ///
    /// [`ErroSefaz`] se o pedido nao monta (chave, protocolo, justificativa) ou o certificado nao assina.
    pub async fn cancelar(
        &self,
        cert: &Certificado,
        chave: &str,
        protocolo: &str,
        justificativa: &str,
    ) -> Result<ResultadoEvento, ErroSefaz> {
        let c = chave_valida(chave)?;
        let m = evento::cancelamento(&Cancelamento {
            chave: &c.chave,
            protocolo,
            justificativa,
            ambiente: self.ambiente,
            quando: self.agora(),
        })?;
        let destino = self.destino_da_chave(&c.chave, c.uf);
        self.registrar(m.assinar(cert, &self.id_lote())?, &destino, c.uf)
            .await
    }

    /// Registra uma carta de correcao (evento 110110) no autorizador da chave. `seq` e' 1 na primeira carta e sobe de 1
    /// em 1 ate 20; **a carta nova substitui as anteriores**, entao ela leva TODAS as correcoes que ainda valem.
    ///
    /// # Errors
    ///
    /// [`ErroSefaz`] se o pedido nao monta (chave, sequencia, texto) ou o certificado nao assina.
    pub async fn corrigir(
        &self,
        cert: &Certificado,
        chave: &str,
        correcao: &str,
        seq: u32,
    ) -> Result<ResultadoEvento, ErroSefaz> {
        let c = chave_valida(chave)?;
        let m = evento::carta_correcao(&CartaCorrecao {
            chave: &c.chave,
            correcao,
            seq,
            ambiente: self.ambiente,
            quando: self.agora(),
        })?;
        // a SVC nao registra carta de correcao (MOC 7.00, Anexo III, 2.1.3.4, item d): a carta de uma nota autorizada na
        // SVC vai a SEFAZ de origem, que ja a tem na base (item h) quando volta
        self.registrar(m.assinar(cert, &self.id_lote())?, c.uf, c.uf)
            .await
    }

    /// Registra um evento da apuracao do IBS/CBS na SVRS. O ambiente e a hora do evento sao os do cliente (os de `e` sao
    /// trocados).
    ///
    /// # Errors
    ///
    /// [`ErroSefaz`] se o pedido nao monta ou o certificado nao assina.
    pub async fn registrar_evento_rtc(
        &self,
        cert: &Certificado,
        e: &EventoRtc<'_>,
    ) -> Result<ResultadoEvento, ErroSefaz> {
        let c = chave_valida(e.chave)?;
        let ev = EventoRtc {
            chave: &c.chave,
            ambiente: self.ambiente,
            quando: self.agora(),
            ..e.clone()
        };
        let m = evento::evento_rtc(&ev)?;
        self.registrar(m.assinar(cert, &self.id_lote())?, "SVRS", c.uf)
            .await
    }

    /// Registra a manifestacao do destinatario (210200, 210210, 210220 ou 210240) no Ambiente Nacional. O ambiente, a hora
    /// do evento e a UF do autor sao os do cliente (os de `m` sao trocados). O certificado tem de ser do mesmo CNPJ-base
    /// (ou CPF) do destinatario (MOC 7.00, Visao Geral, 5.11).
    ///
    /// A mesma regra dos outros eventos: resposta perdida NAO quer dizer manifestacao nao registrada. O cliente consulta a
    /// NF-e antes de qualquer reenvio e, se a duplicidade (573) voltar, devolve a manifestacao que ja esta registrada
    /// (`recuperado`); repetir o MESMO pedido e' sempre seguro, porque `nSeqEvento` e' sempre 1.
    ///
    /// # Errors
    ///
    /// [`ErroSefaz`] se o pedido nao monta (chave, destinatario, justificativa, combinacao recusada pelo MOC), o
    /// certificado e' de outro CNPJ-base ou nao assina.
    pub async fn manifestar(
        &self,
        cert: &Certificado,
        m: &Manifestacao<'_>,
    ) -> Result<ResultadoEvento, ErroSefaz> {
        let c = chave_valida(m.chave)?;
        let pedido = Manifestacao {
            chave: &c.chave,
            uf_autor: self.uf,
            ambiente: self.ambiente,
            quando: self.agora(),
            ..m.clone()
        };
        let montado = evento::manifestacao(&pedido)?;
        conferir_certificado(cert, m.documento)?;
        self.registrar(montado.assinar(cert, &self.id_lote())?, "AN", c.uf)
            .await
    }

    /// Registra o EPEC (evento 110140) da NF-e com `tpEmis` 4 no Ambiente Nacional. Depois, a MESMA NF-e tem de ser
    /// transmitida a SEFAZ da UF (`autorizar`) quando ela voltar.
    ///
    /// # Errors
    ///
    /// [`ErroSefaz`] se o EPEC nao monta ou o certificado nao assina.
    pub async fn registrar_epec(
        &self,
        cert: &Certificado,
        nfe: &str,
        ver_aplic: &str,
    ) -> Result<ResultadoEvento, ErroSefaz> {
        let m = evento::epec(&Epec {
            nfe,
            ambiente: self.ambiente,
            quando: self.agora(),
            ver_aplic,
        })?;
        let uf = chave_valida(&m.chave)?.uf;
        self.registrar(m.assinar(cert, &self.id_lote())?, "AN", uf)
            .await
    }

    fn resultado_do_evento(
        p: &PedidoEvento,
        destino: &str,
        desfecho: DesfechoEvento,
    ) -> ResultadoEvento {
        ResultadoEvento {
            frase: frase_do_evento(&desfecho, &p.tp_evento),
            desfecho,
            chave: p.chave.clone(),
            tp_evento: p.tp_evento.clone(),
            seq: p.seq,
            autorizador: destino.to_string(),
            xml_enviado: p.env_evento.clone(),
            xml_retorno: String::new(),
            proc_evento: None,
            recuperado: false,
            detalhe: String::new(),
        }
    }

    async fn registrar(
        &self,
        p: PedidoEvento,
        destino: &str,
        uf: &str,
    ) -> Result<ResultadoEvento, ErroSefaz> {
        let r = match self
            .chamar(Servico::RecepcaoEvento, &p.env_evento, destino)
            .await?
        {
            Resp::Ok(r) => r,
            Resp::Sem(s) => {
                let porque = format!("o envio ficou sem resposta ({})", s.detalhe);
                return self.recuperar_evento(&p, destino, uf, &porque, false).await;
            }
        };
        let ret = match ler_evento(&r.texto) {
            Ok(x) => x,
            Err(e) => {
                let porque = format!("retorno do envio ilegivel ({e})");
                return self.recuperar_evento(&p, destino, uf, &porque, false).await;
            }
        };
        let (lote, motivo_lote) = (ret.cstat.clone(), ret.motivo.clone());
        let mut res = match lote.as_str() {
            "128" => match do_pedido(&ret.eventos, &p) {
                Some(ev) => return self.do_retevento(&p, destino, uf, &r.texto, ev).await,
                None => {
                    let porque = "o lote voltou sem o registro deste evento";
                    return self.recuperar_evento(&p, destino, uf, porque, false).await;
                }
            },
            "108" | "109" => Self::resultado_do_evento(
                &p,
                destino,
                DesfechoEvento::Indisponivel {
                    cstat: lote,
                    motivo: motivo_lote,
                },
            ),
            "" => {
                return self
                    .recuperar_evento(&p, destino, uf, "retorno sem cStat", false)
                    .await
            }
            _ => Self::resultado_do_evento(
                &p,
                destino,
                DesfechoEvento::Rejeitado {
                    cstat: lote,
                    motivo: motivo_lote,
                },
            ),
        };
        res.xml_retorno = r.texto;
        Ok(res)
    }

    /// Interpreta o `retEvento` deste pedido (o lote ja foi 128).
    async fn do_retevento(
        &self,
        p: &PedidoEvento,
        destino: &str,
        uf: &str,
        retorno: &str,
        ev: &InfRetEvento,
    ) -> Result<ResultadoEvento, ErroSefaz> {
        let desfecho = if let Some(d) = registrado_do(ev, &p.tp_evento) {
            d
        } else {
            let (cstat, motivo) = (ev.cstat.clone(), ev.motivo.clone());
            match cstat.as_str() {
                "501" if p.tp_evento == evento::CANCELAMENTO => {
                    DesfechoEvento::PrazoVencido { cstat, motivo }
                }
                "142" if p.tp_evento == evento::EPEC => DesfechoEvento::EpecBloqueado {
                    motivo,
                    pendentes: ev.pendentes.clone(),
                },
                // o evento ja esta na base: a consulta devolve o registro, nunca um evento novo
                "573" => {
                    let porque = format!("573 duplicidade ({motivo})");
                    return self.recuperar_evento(p, destino, uf, &porque, true).await;
                }
                "108" | "109" => DesfechoEvento::Indisponivel { cstat, motivo },
                _ => DesfechoEvento::Rejeitado { cstat, motivo },
            }
        };
        let registrou = matches!(
            desfecho,
            DesfechoEvento::Registrado { .. } | DesfechoEvento::SemVinculo { .. }
        );
        let mut r = Self::resultado_do_evento(p, destino, desfecho);
        r.xml_retorno = retorno.to_string();
        if registrou {
            r.proc_evento = Some(proc_evento(&p.evento, &ev.cru));
        }
        Ok(r)
    }

    /// A resposta nao decidiu: consulta a NF-e e procura o evento (mesmo tipo e sequencia) entre os dela.
    /// `deve_existir`: a SEFAZ ja disse que o evento esta na base (573).
    async fn recuperar_evento(
        &self,
        p: &PedidoEvento,
        destino: &str,
        uf: &str,
        porque: &str,
        deve_existir: bool,
    ) -> Result<ResultadoEvento, ErroSefaz> {
        // o evento que foi a uma UF ou a SVC se procura la; o do Ambiente Nacional e o da SVRS, onde a nota esta
        let onde = if matches!(destino, "AN" | "SVRS") {
            self.destino_da_chave(&p.chave, uf)
        } else {
            destino.to_string()
        };
        let c = self.consultar_chave(&p.chave, &onde, None).await?;
        let da_consulta = format!("consulta: {}", c.detalhe);
        let sem_informacao = matches!(
            c.desfecho,
            Desfecho::SemResposta | Desfecho::Indisponivel { .. }
        );
        let mut r = if sem_informacao {
            Self::resultado_do_evento(p, destino, DesfechoEvento::SemResposta)
        } else {
            self.achar_na_consulta(p, destino, &c, deve_existir)
        };
        r.detalhe = format!("{porque} | {da_consulta}");
        Ok(r)
    }

    fn achar_na_consulta(
        &self,
        p: &PedidoEvento,
        destino: &str,
        c: &super::resultado::Resultado,
        deve_existir: bool,
    ) -> ResultadoEvento {
        let eventos = re::eventos_da_consulta(&c.xml_retorno).unwrap_or_default();
        let achado = eventos.iter().find_map(|e| {
            let mesmo = e.tp_evento == p.tp_evento && e.seq == Some(p.seq);
            registrado_do(&e.registro, &p.tp_evento)
                .filter(|_| mesmo)
                .map(|d| (d, e))
        });
        if let Some((d, e)) = achado {
            let mut r = Self::resultado_do_evento(p, destino, d);
            r.proc_evento = Some(format!("{DECLARACAO}{}", e.cru));
            r.xml_retorno.clone_from(&c.xml_retorno);
            r.recuperado = true;
            return r;
        }
        // o cancelamento tambem se confirma pelo estado da nota, mesmo sem o evento na lista
        if let (true, Desfecho::Cancelada { cstat, motivo, .. }) =
            (p.tp_evento == evento::CANCELAMENTO, &c.desfecho)
        {
            let d = DesfechoEvento::Registrado {
                fora_do_prazo: cstat == "151",
                cstat: cstat.clone(),
                motivo: motivo.clone(),
                protocolo: String::new(),
            };
            let mut r = Self::resultado_do_evento(p, destino, d);
            r.xml_retorno.clone_from(&c.xml_retorno);
            r.recuperado = true;
            return r;
        }
        let desfecho = if deve_existir || matches!(c.desfecho, Desfecho::SoEpec { .. }) {
            // a SEFAZ diz que o evento esta la e a consulta ainda nao o mostra (o AN e as UF sincronizam em minutos)
            DesfechoEvento::SemResposta
        } else {
            DesfechoEvento::NaoRegistrado
        };
        let mut r = Self::resultado_do_evento(p, destino, desfecho);
        r.xml_retorno.clone_from(&c.xml_retorno);
        r
    }

    // ------------------------------------------------------------------------------------------- inutilizacao

    /// Inutiliza a faixa de numeracao da serie (modelo 55) do emitente da UF do cliente.
    ///
    /// Nas UF que eliminaram o servico (AC, BA e SP desde 11/11/2026, NT 2023.002 v1.02) devolve o erro SEM chamar a
    /// rede: veja [`inutilizacao::disponivel`] para esconder o botao.
    ///
    /// # Errors
    ///
    /// [`ErroSefaz`] se a UF ja nao tem o servico, se o pedido nao monta (CNPJ, faixa, ano, justificativa) ou o
    /// certificado nao assina.
    pub async fn inutilizar(
        &self,
        cert: &Certificado,
        faixa: &PedidoInutilizar<'_>,
    ) -> Result<ResultadoInutilizacao, ErroSefaz> {
        let hoje = self.hoje();
        if !inutilizacao::disponivel(self.uf, self.ambiente, hoje)? {
            let corte = inutilizacao::eliminada_em(self.uf, self.ambiente)
                .map(|d| d.format("%d/%m/%Y").to_string())
                .unwrap_or_default();
            return Err(ErroSefaz::novo(
                format!(
                    "{} nao tem mais o servico de inutilizacao desde {corte} (NT 2023.002 v1.02, item 2.5, e Ajustes \
                     SINIEF 38/2026 e 40/2026): o pedido nao foi enviado. A NT nao indica outro servico no lugar: \
                     consulte a SEFAZ da UF",
                    self.uf
                ),
                "uf",
            ));
        }
        let m = inutilizacao::montar(
            &Inutilizacao {
                uf: self.uf,
                ambiente: self.ambiente,
                cnpj: faixa.cnpj,
                serie: faixa.serie,
                inicio: faixa.inicio,
                fim: faixa.fim,
                justificativa: faixa.justificativa,
                ano: faixa.ano.unwrap_or_else(|| hoje.year()),
            },
            hoje,
        )?;
        let pedido = m.assinar(cert)?;
        self.enviar_inutilizacao(&pedido, faixa.serie, faixa.inicio, faixa.fim)
            .await
    }

    fn resultado_da_inutilizacao(
        p: &PedidoInutilizacao,
        desfecho: DesfechoInutilizacao,
    ) -> ResultadoInutilizacao {
        ResultadoInutilizacao {
            frase: frase_da_inutilizacao(&desfecho),
            desfecho,
            id: p.id.clone(),
            xml_enviado: p.xml.clone(),
            xml_retorno: String::new(),
            proc_inut: None,
            detalhe: String::new(),
        }
    }

    async fn enviar_inutilizacao(
        &self,
        p: &PedidoInutilizacao,
        serie: u32,
        inicio: u64,
        fim: u64,
    ) -> Result<ResultadoInutilizacao, ErroSefaz> {
        let sem = |detalhe: String, d: DesfechoInutilizacao| {
            let mut r = Self::resultado_da_inutilizacao(p, d);
            r.detalhe = detalhe;
            r
        };
        let r = match self.chamar(Servico::Inutilizacao, &p.xml, self.uf).await? {
            Resp::Ok(r) => r,
            Resp::Sem(s) if s.detalhe.starts_with("HTTP 404") => {
                return Ok(sem(
                    s.detalhe,
                    DesfechoInutilizacao::ServicoDesativado {
                        motivo: "HTTP 404: o endereco do servico nao existe mais".into(),
                    },
                ))
            }
            Resp::Sem(s) => return Ok(sem(s.detalhe, DesfechoInutilizacao::SemResposta)),
        };
        let ret = match ler_inutilizacao(&r.texto) {
            Ok(x) => x,
            Err(e) => return Ok(sem(e.to_string(), DesfechoInutilizacao::SemResposta)),
        };
        let mesma_faixa = |v: &str, n: u64| v.is_empty() || v.parse::<u64>().ok() == Some(n);
        let confere = mesma_faixa(&ret.serie, u64::from(serie))
            && mesma_faixa(&ret.inicio, inicio)
            && mesma_faixa(&ret.fim, fim);
        let (cstat, motivo, protocolo) =
            (ret.cstat.clone(), ret.motivo.clone(), ret.protocolo.clone());
        let desfecho = match cstat.as_str() {
            "102" | "563" if !confere => {
                return Ok(sem(
                    format!("a resposta ({cstat}) e' de outra faixa que a pedida"),
                    DesfechoInutilizacao::SemResposta,
                ))
            }
            "102" => DesfechoInutilizacao::Inutilizada {
                cstat,
                motivo,
                protocolo,
            },
            "563" => DesfechoInutilizacao::JaInutilizada { motivo, protocolo },
            "998" => DesfechoInutilizacao::ServicoDesativado { motivo },
            "108" | "109" => DesfechoInutilizacao::Indisponivel { cstat, motivo },
            _ => DesfechoInutilizacao::Rejeitada { cstat, motivo },
        };
        let inutilizada = matches!(
            desfecho,
            DesfechoInutilizacao::Inutilizada { .. } | DesfechoInutilizacao::JaInutilizada { .. }
        );
        let mut res = Self::resultado_da_inutilizacao(p, desfecho);
        res.xml_retorno.clone_from(&ret.cru);
        if inutilizada {
            res.proc_inut = Some(proc_inutilizacao(&p.xml, &ret.cru));
        }
        Ok(res)
    }
}

/// O protocolo do EPEC como o DANFE o imprime: `NNNNNNNNNNNNNNN dd/mm/aaaa hh:mm:ss` (numero do protocolo e a data e hora do
/// registro, como a SEFAZ as mandou). O MOC 7.00, Visao Geral, 3.3.4, manda imprimir no DANFE o numero do protocolo de
/// autorizacao do EPEC.
///
/// # Errors
///
/// [`ErroSefaz`] se o resultado nao e' de um EPEC registrado com o `procEventoNFe`.
pub fn protocolo_epec_danfe(r: &ResultadoEvento) -> Result<String, ErroSefaz> {
    let erro = || {
        ErroSefaz::novo(
            "o protocolo do EPEC so' sai de um EPEC registrado, com o procEventoNFe",
            "nProt",
        )
    };
    let DesfechoEvento::Registrado { protocolo, .. } = &r.desfecho else {
        return Err(erro());
    };
    if r.tp_evento != evento::EPEC || protocolo.is_empty() {
        return Err(erro());
    }
    let proc = r.proc_evento.as_deref().ok_or_else(erro)?;
    let lido = re::ler_proc_evento(proc).map_err(|_| erro())?;
    let d = lido.registro.data.as_str();
    let ok = d.len() >= 19
        && d.is_char_boundary(19)
        && d.as_bytes()[..19].iter().enumerate().all(|(k, b)| match k {
            4 | 7 => *b == b'-',
            10 => *b == b'T',
            13 | 16 => *b == b':',
            _ => b.is_ascii_digit(),
        });
    if !ok {
        return Err(erro());
    }
    Ok(format!(
        "{protocolo} {}/{}/{} {}",
        &d[8..10],
        &d[5..7],
        &d[0..4],
        &d[11..19]
    ))
}
