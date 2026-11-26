// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O cliente dos webservices da NF-e: construcao, chamada SOAP e as operacoes `status`, `autorizar`, `retorno` e
//! `consultar`. A recuperacao da nota cuja resposta se perdeu esta em `fluxo`.

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use chrono::{DateTime, FixedOffset, NaiveDate, Utc};

use super::erro::ErroSefaz;
use super::pedidos::{self, conferir_nfe, NfeConferida};
use super::resposta::{self, Retorno};
use super::resultado::{Desfecho, Resultado, SituacaoServico, StatusServico};
use super::svc::{self, Svc};
use super::tabela::{self, Servico};
use super::transporte::{Transporte, TransporteReqwest};
use crate::assinatura::cert_a1::Certificado;
use crate::nfe::cstat;
use crate::nfe::vigencias::{self, Ambiente};

/// Quanto o cliente espera, no maximo, pelo tempo medio que a SEFAZ informa no recibo (a SEFAZ pode dizer qualquer
/// valor; o cliente nao dorme minutos por causa dele).
pub(super) const ESPERA_MAXIMA_RECIBO: Duration = Duration::from_secs(30);

/// Marca do destino que pede a tabela de enderecos da NFC-e em vez da da NF-e (`NFCE:SP`).
const PREFIXO_NFCE: &str = "NFCE:";

/// Parametros do cliente (um por emitente/UF/ambiente).
#[derive(Debug, Clone)]
pub struct Config {
    /// UF do emitente (sigla ou codigo IBGE): escolhe o autorizador e vai no `consStatServ`.
    pub uf: String,
    /// Producao ou homologacao.
    pub ambiente: Ambiente,
    /// Tempo limite de uma chamada (o transporte real o usa).
    pub timeout: Duration,
    /// Quantas vezes `retorno` consulta o recibo antes de devolver `EmProcessamento` (no minimo 1).
    pub tentativas: usize,
    /// Espera entre as consultas do recibo (no minimo 1 s: consulta em rajada leva a rejeicao 656).
    pub intervalo: Duration,
    /// Endereco que sobrepoe o da tabela para o servico (simulador local): `https`, ou `http` so' em `127.0.0.1`,
    /// `localhost` ou `[::1]`.
    pub urls: BTreeMap<Servico, String>,
}

impl Config {
    /// Configuracao padrao: 60 s de tempo limite, 5 consultas do recibo a cada 3 s, enderecos da tabela.
    #[must_use]
    pub fn novo(uf: &str, ambiente: Ambiente) -> Self {
        Config {
            uf: uf.to_string(),
            ambiente,
            timeout: Duration::from_secs(60),
            tentativas: 5,
            intervalo: Duration::from_secs(3),
            urls: BTreeMap::new(),
        }
    }
}

// O relogio do cliente (troca-se no teste).
type Relogio = Box<dyn Fn() -> DateTime<Utc> + Send + Sync>;
// O "dormir" do cliente: recebe quanto esperar (troca-se no teste).
type Dormir = Box<dyn Fn(Duration) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

/// A chamada que nao voltou com um retorno legivel.
pub(super) struct SemResposta {
    /// So' para o registro.
    pub(super) detalhe: String,
}

pub(super) enum Resp {
    Ok(Retorno),
    Sem(SemResposta),
}

/// Cliente dos webservices da SEFAZ para a NF-e modelo 55.
pub struct ClienteSefaz<T: Transporte> {
    pub(super) uf: &'static str,
    pub(super) ambiente: Ambiente,
    pub(super) tentativas: usize,
    pub(super) intervalo: Duration,
    urls: BTreeMap<Servico, String>,
    transporte: T,
    pub(super) dormir: Dormir,
    relogio: Relogio,
    lote: AtomicU32,
}

impl ClienteSefaz<TransporteReqwest> {
    /// Cliente real: `reqwest` com mTLS pelo A1, raizes ICP-Brasil e `webpki-roots`. `ca_extra_pem` e' a raiz do
    /// simulador local, se for o caso.
    ///
    /// # Errors
    ///
    /// [`ErroSefaz`] se a configuracao e' invalida ou o certificado/CA nao serve para TLS.
    pub fn real(
        certificado: &Certificado,
        config: Config,
        ca_extra_pem: Option<&[u8]>,
    ) -> Result<Self, ErroSefaz> {
        let t = TransporteReqwest::novo(certificado, ca_extra_pem, config.timeout)
            .map_err(|e| ErroSefaz::novo(e, "certificado"))?;
        Self::com_transporte(config, t)
    }
}

impl<T: Transporte> ClienteSefaz<T> {
    /// Cliente sobre um transporte qualquer (o falso, nos testes).
    ///
    /// # Errors
    ///
    /// [`ErroSefaz`] se a UF e' desconhecida ou algum endereco de `urls` nao e' seguro.
    pub fn com_transporte(config: Config, transporte: T) -> Result<Self, ErroSefaz> {
        let uf = tabela::uf_valida(&config.uf)?;
        let mut urls = BTreeMap::new();
        for (s, u) in config.urls {
            urls.insert(s, base_segura(&u)?);
        }
        Ok(ClienteSefaz {
            uf,
            ambiente: config.ambiente,
            tentativas: config.tentativas.max(1),
            intervalo: config.intervalo.max(Duration::from_secs(1)),
            urls,
            transporte,
            dormir: Box::new(|d| Box::pin(tokio::time::sleep(d))),
            relogio: Box::new(Utc::now),
            lote: AtomicU32::new(0),
        })
    }

    /// Troca o relogio (teste).
    #[must_use]
    pub fn com_relogio(mut self, f: impl Fn() -> DateTime<Utc> + Send + Sync + 'static) -> Self {
        self.relogio = Box::new(f);
        self
    }

    /// Troca o "dormir" (teste): recebe a espera de cada pausa.
    #[must_use]
    pub fn com_dormir(
        mut self,
        f: impl Fn(Duration) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync + 'static,
    ) -> Self {
        self.dormir = Box::new(f);
        self
    }

    /// A UF do emitente (sigla).
    #[must_use]
    pub fn uf(&self) -> &str {
        self.uf
    }

    /// O ambiente do cliente.
    #[must_use]
    pub fn ambiente(&self) -> Ambiente {
        self.ambiente
    }

    /// Hoje no horario de Brasilia: as datas das Notas Tecnicas sao do calendario nacional.
    pub(super) fn hoje(&self) -> NaiveDate {
        // Brasilia = UTC-3, sem horario de verao
        ((self.relogio)() - chrono::Duration::hours(3)).date_naive()
    }

    /// Agora, com o fuso de Greenwich (o `dhEvento` sai no fuso da UF da chave).
    pub(super) fn agora(&self) -> DateTime<FixedOffset> {
        (self.relogio)().fixed_offset()
    }

    /// A quem se pergunta ou se envia sobre a NF-e dessa chave: a SVC se a chave tem `tpEmis` 6 ou 7, senao o
    /// autorizador da UF. Devolve o nome que `chamar` entende (sigla da UF ou nome do autorizador).
    pub(super) fn destino_da_chave(&self, chave: &str, uf: &str) -> String {
        // a NFC-e (modelo 65, posicoes 20 e 21 da chave) tem tabela de enderecos propria e nao passa pela SVC
        if chave.get(20..22) == Some("65") {
            return format!("{PREFIXO_NFCE}{uf}");
        }
        let tp = chave
            .as_bytes()
            .get(34)
            .and_then(|b| char::from(*b).to_digit(10));
        match tp.and_then(Svc::do_tp_emis) {
            Some(svc) => svc.autorizador().to_string(),
            None => uf.to_string(),
        }
    }

    /// 15 digitos no maximo: relogio (AAMMDDhhmmss) + contador. Unico dentro do processo.
    pub(super) fn id_lote(&self) -> String {
        let n = (self.lote.fetch_add(1, Ordering::SeqCst) + 1) % 1000;
        let base: u64 = (self.relogio)()
            .format("%y%m%d%H%M%S")
            .to_string()
            .parse()
            .unwrap_or(0);
        (base * 1000 + u64::from(n)).to_string()
    }

    fn url_do(&self, servico: Servico, destino: &str) -> Result<String, ErroSefaz> {
        match self.urls.get(&servico) {
            Some(u) => Ok(u.clone()),
            None if destino.starts_with(PREFIXO_NFCE) => {
                tabela::url_nfce(&destino[PREFIXO_NFCE.len()..], self.ambiente, servico)
            }
            // `SVC-AN`, `SVC-RS`, `AN` e `SVRS` sao autorizadores; o resto e' sigla de UF
            None if matches!(destino, "SVC-AN" | "SVC-RS" | "AN" | "SVRS") => {
                tabela::url_do_autorizador(destino, self.ambiente, servico)
            }
            None => tabela::url(destino, self.ambiente, servico),
        }
    }

    /// Uma chamada SOAP. `uf` e' a UF cujo autorizador atende (a do emitente, ou a da chave na consulta) ou o nome de
    /// um autorizador (`SVC-AN`, `SVC-RS`, `AN`, `SVRS`).
    pub(super) async fn chamar(
        &self,
        servico: Servico,
        dados: &str,
        uf: &str,
    ) -> Result<Resp, ErroSefaz> {
        let url = self.url_do(servico, uf)?;
        let env = pedidos::envelope(dados, servico);
        let (st, corpo) = match self
            .transporte
            .enviar(&url, env.corpo, &env.content_type)
            .await
        {
            Ok(x) => x,
            Err(f) => {
                return Ok(Resp::Sem(SemResposta {
                    detalhe: f.to_string(),
                }))
            }
        };
        // 200 a 299 e o 500 do SOAP Fault; o resto (proxy, balanceador, pagina de manutencao) nao e' resposta da SEFAZ
        if !(200..300).contains(&st) && st != 500 {
            return Ok(Resp::Sem(SemResposta {
                detalhe: format!("HTTP {st}: {}", http_em_palavras(st)),
            }));
        }
        Ok(match resposta::desembrulhar(&corpo) {
            Ok(r) => Resp::Ok(r),
            Err(e) => Resp::Sem(SemResposta {
                detalhe: format!("HTTP {st}: {e}"),
            }),
        })
    }

    // ----------------------------------------------------------------------------------------------- servicos

    /// O status do servico de autorizacao da UF (`NfeStatusServico`).
    ///
    /// # Errors
    ///
    /// [`ErroSefaz`] so' se o pedido nao monta; falha de rede volta como [`SituacaoServico::SemResposta`].
    pub async fn status(&self) -> Result<StatusServico, ErroSefaz> {
        self.status_em(self.uf).await
    }

    /// O status da SVC que atende a UF do cliente (`NfeStatusServico` da SVC-AN ou da SVC-RS). Consulte-o so' quando o
    /// ambiente normal da SEFAZ estiver fora: a SVC so' serve quando responde 107; 113 e 114 dizem que ela esta sendo
    /// desativada ou foi desabilitada pela SEFAZ de origem (MOC 7.00, Anexo III, 2.1.3.4, item g).
    ///
    /// # Errors
    ///
    /// [`ErroSefaz`] so' se o pedido nao monta; falha de rede volta como [`SituacaoServico::SemResposta`].
    pub async fn status_svc(&self) -> Result<StatusServico, ErroSefaz> {
        let svc = svc::svc_da_uf(self.uf)?;
        self.status_em(svc.autorizador()).await
    }

    /// O status do servico de autorizacao da NFC-e da UF (`NfeStatusServico`, endereco da tabela da NFC-e). A NFC-e
    /// nao tem SVC: se o servico nao responde, a saida e' a contingencia off-line (`tpEmis` 9).
    ///
    /// # Errors
    ///
    /// [`ErroSefaz`] so' se o pedido nao monta; falha de rede volta como [`SituacaoServico::SemResposta`].
    pub async fn status_nfce(&self) -> Result<StatusServico, ErroSefaz> {
        self.status_em(&format!("{PREFIXO_NFCE}{}", self.uf)).await
    }

    async fn status_em(&self, destino: &str) -> Result<StatusServico, ErroSefaz> {
        let pedido = pedidos::cons_stat(self.ambiente, self.uf)?;
        let sem = |detalhe: String| StatusServico {
            situacao: SituacaoServico::SemResposta,
            cstat: String::new(),
            motivo: String::new(),
            tempo_medio: None,
            retorno_previsto: String::new(),
            frase: "A SEFAZ nao respondeu agora.".to_string(),
            detalhe,
        };
        let r = match self
            .chamar(Servico::StatusServico, &pedido, destino)
            .await?
        {
            Resp::Ok(r) => r,
            Resp::Sem(s) => return Ok(sem(s.detalhe)),
        };
        let ret = match resposta::ler_status(&r.texto) {
            Ok(x) => x,
            Err(e) => return Ok(sem(e.to_string())),
        };
        let situacao = match ret.cab.cstat.as_str() {
            "107" => SituacaoServico::EmOperacao,
            "108" | "109" => SituacaoServico::Paralisado,
            _ => SituacaoServico::Outra,
        };
        let motivo = if ret.cab.motivo.is_empty() {
            cstat::mensagem(&ret.cab.cstat).unwrap_or("").to_string()
        } else {
            ret.cab.motivo.clone()
        };
        let frase = match situacao {
            SituacaoServico::EmOperacao => "Servico da SEFAZ em operacao.".to_string(),
            SituacaoServico::Paralisado => format!("Servico da SEFAZ paralisado: {motivo}."),
            _ => format!("A SEFAZ respondeu {}: {motivo}", ret.cab.cstat),
        };
        Ok(StatusServico {
            situacao,
            cstat: ret.cab.cstat,
            motivo,
            tempo_medio: ret.tempo_medio,
            retorno_previsto: ret.retorno_previsto,
            frase,
            detalhe: String::new(),
        })
    }

    /// Envia UMA NF-e assinada em lote sincrono (`indSinc` 1, o padrao) e diz o que aconteceu com ela. A resposta que
    /// se perde nao vira nota nova: ver a documentacao do modulo.
    ///
    /// # Errors
    ///
    /// [`ErroSefaz`] se a NF-e nao passa na conferencia (sem assinatura, de outra UF ou ambiente, chave invalida...).
    pub async fn autorizar(&self, nfe_assinada: &str) -> Result<Resultado, ErroSefaz> {
        self.enviar(nfe_assinada, true).await
    }

    /// Envia a NF-e em lote assincrono (`indSinc` 0): a SEFAZ devolve o recibo e o cliente consulta o resultado.
    /// So' enquanto a NT 2023.002 v1.02 permite: depois da data de corte do ambiente a SEFAZ rejeita com 129 e este
    /// metodo recusa antes de chamar a rede.
    ///
    /// # Errors
    ///
    /// [`ErroSefaz`] se a NF-e nao passa na conferencia ou se o ambiente ja passou do corte do assincrono.
    pub async fn autorizar_assincrono(&self, nfe_assinada: &str) -> Result<Resultado, ErroSefaz> {
        if conferir_nfe(nfe_assinada).is_ok_and(|c| c.chave.modelo == "65") {
            return Err(ErroSefaz::novo(
                "a NFC-e nao tem lote assincrono: o envio e' sempre sincrono (NT 2025.001, item 02.3)",
                "indSinc",
            ));
        }
        let hoje = self.hoje();
        if vigencias::vigente("nt2023_002.assincrono", hoje, self.ambiente).unwrap_or(false) {
            let corte = vigencias::inicio("nt2023_002.assincrono", self.ambiente)
                .map(|d| d.format("%d/%m/%Y").to_string())
                .unwrap_or_default();
            return Err(ErroSefaz::novo(
                format!(
                    "o envio assincrono acabou em {corte} neste ambiente (rejeicao 129, NT 2023.002 v1.02): \
                     envie em lote sincrono"
                ),
                "indSinc",
            ));
        }
        self.enviar(nfe_assinada, false).await
    }

    /// O resultado do lote assincrono pelo recibo (`NfeRetAutorizacao`), com a espera e as tentativas da
    /// configuracao. Precisa da NF-e para achar o protocolo dela no lote e montar o `nfeProc`.
    ///
    /// # Errors
    ///
    /// [`ErroSefaz`] se o recibo nao tem 15 digitos ou a NF-e nao passa na conferencia.
    pub async fn retorno(&self, recibo: &str, nfe_assinada: &str) -> Result<Resultado, ErroSefaz> {
        let nfe = self.conferir(nfe_assinada)?;
        pedidos::cons_reci(self.ambiente, recibo)?;
        let chave = nfe.chave.chave.clone();
        self.aguardar_recibo(recibo.trim(), &chave, nfe.texto, "", None)
            .await
    }

    /// A situacao da NF-e pela chave (`NfeConsultaProtocolo`), no autorizador da UF da chave. Com a NF-e assinada, a
    /// nota autorizada ou denegada volta com o `nfeProc` montado.
    ///
    /// A consulta so' alcanca as notas dos ultimos 180 dias (MOC 7.00, Visao Geral, 5.4.3).
    ///
    /// # Errors
    ///
    /// [`ErroSefaz`] se a chave nao e' de uma NF-e valida, ou a NF-e dada e' de outra chave.
    pub async fn consultar(
        &self,
        chave: &str,
        nfe_assinada: Option<&str>,
    ) -> Result<Resultado, ErroSefaz> {
        let ch = pedidos::chave_valida_qualquer(chave)?;
        let nfe = match nfe_assinada {
            Some(n) => {
                let c = conferir_nfe(n)?;
                if c.chave.chave != ch.chave {
                    return Err(ErroSefaz::novo(
                        "a NF-e dada e' de outra chave que a consultada",
                        "chNFe",
                    ));
                }
                Some(c.texto)
            }
            None => None,
        };
        let destino = self.destino_da_chave(&ch.chave, ch.uf);
        self.consultar_chave(&ch.chave, &destino, nfe).await
    }

    /// Confere a NF-e contra o cliente: UF, ambiente e forma de emissao.
    pub(super) fn conferir<'a>(&self, nfe: &'a str) -> Result<NfeConferida<'a>, ErroSefaz> {
        let c = conferir_nfe(nfe)?;
        if c.chave.uf != self.uf {
            return Err(ErroSefaz::novo(
                format!(
                    "a nota e' da UF {} e este cliente e' da UF {}",
                    c.chave.uf, self.uf
                ),
                "cUF",
            ));
        }
        if c.tp_amb != pedidos::tp_amb(self.ambiente).to_string() {
            return Err(ErroSefaz::novo(
                format!(
                    "a nota e' do ambiente {} e este cliente e' do ambiente {}",
                    c.tp_amb,
                    pedidos::tp_amb(self.ambiente)
                ),
                "tpAmb",
            ));
        }
        // a NFC-e nao e' autorizada pela SVC (rejeicao 783, MOC 7.00, Anexo I, grupo B)
        if c.chave.modelo == "65" && matches!(c.chave.tp_emis, 6 | 7) {
            return Err(ErroSefaz::novo(
                "a NFC-e nao tem SVC: o tpEmis 6 ou 7 e' rejeitado (783); sem o servico use a contingencia off-line (tpEmis 9)",
                "tpEmis",
            ));
        }
        // na SVC a nota leva o tpEmis da SVC da UF (rejeicao 713, MOC 7.00, Anexo I, B25-40)
        if matches!(c.chave.tp_emis, 6 | 7) {
            let esperado = svc::svc_da_uf(self.uf)?;
            if c.chave.tp_emis != esperado.tp_emis()
                && !svc::homologacao_diverge_do_ato(self.uf, self.ambiente, c.chave.tp_emis)
            {
                return Err(ErroSefaz::novo(
                    format!(
                        "a UF {} e' atendida pela {} (tpEmis {}), e a nota tem tpEmis {} (rejeicao 713; {})",
                        self.uf,
                        esperado.autorizador(),
                        esperado.tp_emis(),
                        c.chave.tp_emis,
                        svc::ATO_COTEPE
                    ),
                    "tpEmis",
                ));
            }
        }
        Ok(c)
    }

    /// Monta o desfecho da consulta a partir do retorno (`retConsSitNFe`).
    pub(super) fn desfecho_da_consulta(
        ret: &resposta::RetConsulta,
        chave: &str,
        nfe: Option<&str>,
    ) -> Desfecho {
        let cs = ret.cab.cstat.as_str();
        let (cstat_, motivo) = (ret.cab.cstat.clone(), ret.cab.motivo.clone());
        // so' vale o protocolo que e' DESTA chave: o de outra nota nao autoriza esta
        let prot = ret.protocolo.as_ref().filter(|p| p.chave == chave);
        let proc_ = |p: &resposta::ProtNfe| {
            nfe.filter(|_| {
                cstat::AUTORIZADA.contains(&p.cstat.as_str())
                    || cstat::DENEGADA.contains(&p.cstat.as_str())
            })
            .map(|n| resposta::nfe_proc(n, &p.cru))
        };
        if cstat::AUTORIZADA.contains(&cs) {
            return match prot {
                Some(p)
                    if !p.numero.is_empty() && cstat::AUTORIZADA.contains(&p.cstat.as_str()) =>
                {
                    Desfecho::Autorizada {
                        cstat: cstat_,
                        motivo,
                        protocolo: p.numero.clone(),
                        nfe_proc: nfe.map(|n| resposta::nfe_proc(n, &p.cru)),
                    }
                }
                _ => Desfecho::SemResposta,
            };
        }
        if cstat::CANCELADA.contains(&cs) {
            return Desfecho::Cancelada {
                cstat: cstat_,
                motivo,
                protocolo: prot.map(|p| p.numero.clone()).unwrap_or_default(),
                nfe_proc: prot.and_then(proc_),
            };
        }
        if cstat::DENEGADA.contains(&cs) {
            return Desfecho::Denegada {
                cstat: cstat_,
                motivo,
                protocolo: prot.map(|p| p.numero.clone()).unwrap_or_default(),
                nfe_proc: prot.filter(|p| !p.numero.is_empty()).and_then(proc_),
            };
        }
        match cs {
            "217" => Desfecho::NaoRecebida,
            "124" => Desfecho::SoEpec { motivo },
            "108" | "109" => Desfecho::Indisponivel {
                cstat: cstat_,
                motivo,
            },
            _ => Desfecho::Rejeitada {
                cstat: cstat_,
                motivo,
            },
        }
    }
}

/// Em palavras, o que um HTTP fora do 200 a 299 (e do 500 do Fault) quer dizer.
fn http_em_palavras(st: u16) -> &'static str {
    match st {
        301 | 302 | 303 | 307 | 308 => "redirecionamento (o cliente nao segue redirecionamento)",
        400 => "pedido recusado pelo servidor",
        401 | 403 => "certificado ou acesso recusado",
        404 => "endereco nao encontrado",
        408 | 504 => "tempo esgotado no servidor",
        502 | 503 => "servico indisponivel",
        s if s >= 500 => "erro no servidor",
        _ => "resposta que nao e' da SEFAZ",
    }
}

/// `https` sempre; `http` so' no proprio computador (simulador).
fn base_segura(url: &str) -> Result<String, ErroSefaz> {
    let u = url.trim();
    let erro = || {
        ErroSefaz::novo(
            "endereco do servico invalido: use https (http so' no proprio computador, para simulador)",
            "urls",
        )
    };
    let host = |resto: &str| {
        resto
            .split(['/', '?', '#'])
            .next()
            .unwrap_or("")
            .rsplit('@')
            .next()
            .unwrap_or("")
            .to_string()
    };
    if let Some(r) = u.strip_prefix("https://") {
        return if host(r).is_empty() {
            Err(erro())
        } else {
            Ok(u.to_string())
        };
    }
    if let Some(r) = u.strip_prefix("http://") {
        let h = host(r);
        let nome = match h.strip_prefix('[') {
            Some(v6) => v6.split(']').next().unwrap_or("").to_string(),
            None => h.split(':').next().unwrap_or("").to_string(),
        };
        if matches!(nome.as_str(), "127.0.0.1" | "localhost" | "::1") && !r.contains('@') {
            return Ok(u.to_string());
        }
    }
    Err(erro())
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn http_so_no_proprio_computador() {
        let h = "http";
        assert!(base_segura("https://nfe.exemplo.gov.br/ws").is_ok());
        assert!(base_segura("http://127.0.0.1:4050/x").is_ok());
        assert!(base_segura("http://localhost/x").is_ok());
        assert!(base_segura("http://[::1]:8080/x").is_ok());
        assert!(base_segura(&format!("{h}://127.0.0.1.exemplo.com.br/x")).is_err());
        assert!(base_segura(&format!("{h}://localhost@exemplo.com.br/x")).is_err());
        assert!(base_segura(&format!("{h}://exemplo.com.br/x")).is_err());
        assert!(base_segura("ftp://127.0.0.1/x").is_err());
        assert!(base_segura("https://").is_err());
    }
}
