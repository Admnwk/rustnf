// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! O fluxo de emissao e de cancelamento com a configuracao: completa o pedido com o `[emitente]`, numera a DPS pela
//! pasta de notas, emite e guarda a nota autorizada.

use rustnf::nfse::leitura::Nfse;
use rustnf::nfse::sefin::{Resultado, Status};
use rustnf::nfse::Dps;
use serde_json::{json, Value};

use crate::config::{aplicar_emitente, Emitente};
use crate::emissao::{self, Emissor};
use crate::erro::Erro;
use crate::notas::{Notas, Registro};
use crate::nucleo;

/// Campo ausente do pedido: nao existe, e' `null` ou e' texto vazio.
fn ausente(pedido: &Value, chave: &str) -> bool {
    match pedido.get(chave) {
        None | Some(Value::Null) => true,
        Some(Value::String(s)) => s.trim().is_empty(),
        Some(_) => false,
    }
}

/// A serie do pedido como texto; sem ela, 1 (o padrao da DPS).
fn serie_do_pedido(pedido: &Value) -> String {
    match pedido.get("serie") {
        Some(Value::String(s)) if !s.trim().is_empty() => s.trim().to_string(),
        Some(Value::Number(n)) => n.to_string(),
        _ => "1".to_string(),
    }
}

/// O que `emitir_pedido_completo` devolve: o resultado inteiro do cliente e o que se fez com a pasta de notas.
#[derive(Debug, Clone)]
pub struct Emitida {
    /// O resultado do cliente da Sefin (com o XML da nota, se autorizada, e as mensagens de rejeicao).
    pub resultado: Resultado,
    /// O numero da DPS, quando veio da pasta de notas (o pedido nao trazia).
    pub numero_dps: Option<u64>,
    /// Nota autorizada ou recuperada e pasta de notas em uso: o XML foi guardado? `None` nos outros casos.
    pub guardada: Option<bool>,
    /// Aviso quando a nota foi autorizada mas a pasta nao pode ser atualizada por completo.
    pub aviso: Option<&'static str>,
    /// Nota RECUPERADA (ja existia na Sefin com esta serie e este numero de DPS): em que ela difere do pedido
    /// enviado agora. Vazio quando a nota e' a do pedido. Com diferenca, a nota que existe NAO e' a que se quis
    /// emitir: sobrou de uma tentativa anterior com outros dados.
    pub diferencas: Vec<&'static str>,
}

const AVISO_PASTA: &str = "a nota foi autorizada, mas a pasta de notas nao pode ser atualizada por completo; consulte a nota pela chave e confira a numeracao";

/// O que do pedido se compara com a nota recuperada.
struct Enviado {
    tomador: String,
    valor: String,
    servico: String,
    competencia: Option<String>,
}

impl Enviado {
    fn da_dps(dps: &Dps) -> Self {
        Self {
            tomador: documento(dps.tomador.as_ref().map_or("", |t| t.documento.as_str())),
            valor: dps.valores.servico.as_str().to_string(),
            servico: so_digitos(dps.servico.ctribnac.as_str()),
            competencia: dps.competencia.clone(),
        }
    }

    /// Em que a `nfse` recuperada difere do que foi enviado.
    fn diferencas(&self, nfse: &Nfse) -> Vec<&'static str> {
        let mut saida = Vec::new();
        if self.tomador != documento(&nfse.tomador_documento) {
            saida.push("tomador");
        }
        if centavos(&self.valor) != centavos(&nfse.valor_servico) {
            saida.push("valor do servico");
        }
        if self.servico != so_digitos(&nfse.ctribnac) {
            saida.push("codigo do servico");
        }
        let dia = |texto: &str| texto.trim().chars().take(10).collect::<String>();
        if let Some(c) = &self.competencia {
            if !c.trim().is_empty() && dia(c) != dia(&nfse.competencia) {
                saida.push("competencia");
            }
        }
        saida
    }
}

fn so_digitos(texto: &str) -> String {
    texto.chars().filter(char::is_ascii_digit).collect()
}

/// CPF ou CNPJ sem a mascara. As letras ficam (em maiuscula): o CNPJ pode ser alfanumerico.
fn documento(texto: &str) -> String {
    texto
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

/// Texto decimal (`150`, `150.5`, `150,50`) em centavos; `None` se nao for um valor com ate 2 casas.
fn centavos(texto: &str) -> Option<i128> {
    let t = texto.trim().replace(',', ".");
    let (inteiro, fracao) = t.split_once('.').unwrap_or((&t, ""));
    let fracao = fracao.trim_end_matches('0');
    let so_numeros = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
    if inteiro.is_empty() || fracao.len() > 2 || !so_numeros(inteiro) || !so_numeros(fracao) {
        return None;
    }
    format!("{inteiro}{fracao:0<2}").parse().ok()
}

/// Emite a DPS do `pedido` (JSON).
///
/// 1. completa so' os campos ausentes com o `[emitente]`;
/// 2. sem `numero` no pedido e com `notas`, usa o proximo numero da serie;
/// 3. emite (veja [`emissao::emitir`]);
/// 4. nota autorizada ou recuperada: guarda o XML na pasta de notas e confirma o numero da DPS. Se a guarda falhar,
///    a emissao vale (a nota existe na Sefin): a resposta traz `guardada: false` e o `aviso`.
///
/// A resposta e' a de [`emissao::emitir`]; com `notas`, ganha `guardada` (e `numero_dps`, quando o numero veio da
/// pasta). Pedido com campo que a DPS nao le ([`nucleo::campos_desconhecidos`]): `campos_desconhecidos` os lista. Nota recuperada com dados diferentes dos do pedido (sobra de uma tentativa anterior com a mesma serie e o
/// mesmo numero): `recuperada_difere` lista o que difere, e a nota que existe NAO e' a do pedido.
///
/// # Errors
///
/// Como [`emissao::emitir`], mais o erro de ler o numero da pasta de notas ou da leitura do pedido.
pub async fn emitir_pedido(
    emissor: &dyn Emissor,
    pedido: Value,
    emitente: &Emitente,
    folga_segundos: u32,
    notas: Option<&Notas>,
) -> Result<Value, Erro> {
    let desconhecidos = nucleo::campos_desconhecidos(&pedido);
    let emitida = emitir_pedido_completo(emissor, pedido, emitente, folga_segundos, notas).await?;
    let mut saida = emissao::resultado_json(&emitida.resultado)?;
    if !desconhecidos.is_empty() {
        saida["campos_desconhecidos"] = json!(desconhecidos);
    }
    if let Some(guardada) = emitida.guardada {
        saida["guardada"] = json!(guardada);
    }
    if let Some(aviso) = emitida.aviso {
        saida["aviso"] = json!(aviso);
    }
    if !emitida.diferencas.is_empty() {
        saida["recuperada_difere"] = json!(emitida.diferencas);
    }
    if let Some(n) = emitida.numero_dps {
        saida["numero_dps"] = json!(n);
    }
    Ok(saida)
}

/// Como [`emitir_pedido`], devolvendo o resultado do cliente como dado tipado (a rejeicao da Sefin chega com os
/// codigos e as descricoes, sem virar texto) em vez de JSON. E' o que uma interface usa para mostrar cada desfecho.
///
/// # Errors
///
/// So' a recusa da propria montagem, o erro de ler o numero da pasta de notas e o da leitura do pedido; o que a
/// Sefin responde vem em `Emitida::resultado`.
pub async fn emitir_pedido_completo(
    emissor: &dyn Emissor,
    mut pedido: Value,
    emitente: &Emitente,
    folga_segundos: u32,
    notas: Option<&Notas>,
) -> Result<Emitida, Erro> {
    aplicar_emitente(&mut pedido, emitente);
    let mut numero_da_pasta = None;
    if let (Some(notas), true) = (notas, pedido.is_object() && ausente(&pedido, "numero")) {
        let numero = notas.proximo_numero(&serie_do_pedido(&pedido))?;
        pedido["numero"] = json!(numero);
        numero_da_pasta = Some(numero);
    }
    let dps: Dps = serde_json::from_value(pedido)?;
    let serie = dps.serie.as_str().to_string();
    let numero = dps.numero.as_str().to_string();
    let enviado = Enviado::da_dps(&dps);
    let resultado = emissao::enviar(emissor, dps, folga_segundos).await?;
    let diferencas = match (&resultado.nfse, resultado.recuperada) {
        (Some(nfse), true) => enviado.diferencas(nfse),
        _ => Vec::new(),
    };
    let mut emitida = Emitida {
        resultado,
        numero_dps: numero_da_pasta,
        guardada: None,
        aviso: None,
        diferencas,
    };
    if let (Some(notas), Status::Autorizada) = (notas, emitida.resultado.status) {
        // o numero foi gasto na Sefin mesmo que a guarda do XML falhe
        let confirmado = confirmar(notas, &serie, &numero);
        let guardada = guardar(notas, &emitida.resultado, &serie, &numero);
        emitida.guardada = Some(guardada);
        if !(guardada && confirmado) {
            emitida.aviso = Some(AVISO_PASTA);
        }
    }
    Ok(emitida)
}

fn guardar(notas: &Notas, r: &Resultado, serie: &str, numero: &str) -> bool {
    let Some(nfse) = &r.nfse else {
        return false;
    };
    if r.xml_retorno.is_empty() {
        return false;
    }
    let mut registro = Registro::da_nfse(nfse);
    if registro.chave.is_empty() {
        registro.chave.clone_from(&r.chave);
    }
    if registro.serie.is_empty() {
        registro.serie = serie.to_string();
    }
    if registro.numero_dps.is_empty() {
        registro.numero_dps = numero.to_string();
    }
    notas.guardar(&registro, &r.xml_retorno).is_ok()
}

fn confirmar(notas: &Notas, serie: &str, numero: &str) -> bool {
    numero
        .parse::<u64>()
        .is_ok_and(|n| notas.confirmar_numero(serie, n).is_ok())
}

/// O que `cancelar_pedido_completo` devolve: o resultado do cliente e o que se fez com a pasta de notas.
#[derive(Debug, Clone)]
pub struct Cancelada {
    /// O resultado do cliente da Sefin (registrado, rejeitado com as mensagens, ou sem confirmacao).
    pub resultado: Resultado,
    /// Cancelamento registrado e pasta de notas em uso: a nota foi marcada como cancelada na pasta? `Some(false)`
    /// quando a nota nao esta na pasta ou a pasta nao pode ser atualizada; `None` nos outros casos.
    pub marcada: Option<bool>,
}

/// Cancela a NFS-e (veja [`emissao::cancelar`]) e, com `notas`, marca a nota como cancelada na pasta. Nota que nao
/// esta na pasta nao e' erro.
///
/// # Errors
///
/// Como [`emissao::cancelar`].
pub async fn cancelar_pedido(
    emissor: &dyn Emissor,
    chave: &str,
    motivo: &str,
    justificativa: &str,
    notas: Option<&Notas>,
) -> Result<Value, Erro> {
    let cancelada = cancelar_pedido_completo(emissor, chave, motivo, justificativa, notas).await;
    emissao::resultado_json(&cancelada.resultado)
}

/// Como [`cancelar_pedido`], devolvendo o resultado do cliente como dado tipado (a rejeicao chega com os codigos e
/// as descricoes, e a falta de resposta continua sendo "sem confirmacao") em vez de JSON ou erro. E' o que uma
/// interface usa para mostrar cada desfecho. So' o cancelamento registrado marca a nota na pasta.
///
/// A Sefin recusa o evento repetido. Por isso, diante de uma recusa, a funcao confere se a nota JA tem o
/// cancelamento (de um envio anterior cuja resposta se perdeu, ou feito em outro lugar): tendo, o desfecho e'
/// "registrado" (`recuperada`), a pasta e' marcada e as mensagens da recusa viram alertas.
pub async fn cancelar_pedido_completo(
    emissor: &dyn Emissor,
    chave: &str,
    motivo: &str,
    justificativa: &str,
    notas: Option<&Notas>,
) -> Cancelada {
    let mut resultado = emissor.cancelar(chave, motivo, justificativa).await;
    if resultado.status == Status::Rejeitada && emissor.cancelada(chave).await.status == Status::Ok
    {
        resultado.status = Status::Registrado;
        resultado.recuperada = true;
        resultado.frase = "A nota ja estava cancelada na Sefin.".into();
        let recusa = std::mem::take(&mut resultado.erros);
        resultado.alertas.extend(recusa);
    }
    let marcada = match notas {
        Some(notas) if resultado.ok() => Some(notas.marcar_cancelada(chave).is_ok()),
        _ => None,
    };
    Cancelada { resultado, marcada }
}

#[cfg(test)]
mod testes {
    use std::future::Future;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::task::{Context, Poll, Waker};

    use async_trait::async_trait;
    use rustnf::nfse::leitura::{ler_nfse, Mensagem};

    use super::*;
    use crate::testes_comuns::PastaTemporaria;

    const DPS: &str = include_str!("../../rustnf-cli/exemplos/dps.json");
    const NFSE_BASE: &str = include_str!("../../rustnf-cli/exemplos/nfse.xml");

    /// A NFS-e de exemplo com uma chave de acesso de digito verificador certo (a do exemplo nao tem).
    fn nfse_valida() -> String {
        let chave = rustnf::chave_dfe::montar_nfse(
            1_400_159,
            "11222333000181",
            22,
            "2609",
            Some(97_579_757),
            2,
        )
        .unwrap();
        NFSE_BASE.replace("14001591299999999999999000000000000022097579757684", &chave)
    }

    /// O emissor desses testes responde na hora, sem rede.
    struct Falso {
        status: Status,
        recuperada: bool,
        chamadas: AtomicUsize,
    }

    #[async_trait]
    impl Emissor for Falso {
        fn ambiente(&self) -> u8 {
            2
        }

        async fn emitir(&self, _dps: &Dps) -> Resultado {
            self.chamadas.fetch_add(1, Ordering::Relaxed);
            let xml = nfse_valida();
            let nfse = ler_nfse(&xml).unwrap();
            let autorizada = self.status == Status::Autorizada;
            Resultado {
                status: self.status,
                frase: "resposta de teste".into(),
                chave: if autorizada {
                    nfse.chave.clone()
                } else {
                    String::new()
                },
                id_dps: String::new(),
                nfse: autorizada.then_some(nfse),
                xml_assinado: String::new(),
                xml_retorno: if autorizada { xml } else { String::new() },
                erros: if autorizada {
                    vec![]
                } else {
                    vec![Mensagem {
                        codigo: "E0312".into(),
                        descricao: "codigo de tributacao".into(),
                        complemento: "serv".into(),
                    }]
                },
                alertas: vec![],
                http: 200,
                recuperada: self.recuperada,
                corpo: None,
                detalhe: String::new(),
            }
        }

        async fn consultar(&self, _chave: &str) -> Resultado {
            unreachable!("os testes nao consultam")
        }

        async fn cancelar(&self, _chave: &str, _motivo: &str, _justificativa: &str) -> Resultado {
            unreachable!("os testes nao cancelam")
        }

        async fn cancelada(&self, _chave: &str) -> Resultado {
            unreachable!("os testes nao conferem cancelamento")
        }

        async fn consultar_dps(&self, _id_dps: &str, _com_nota: bool) -> Resultado {
            unreachable!("os testes nao consultam a DPS")
        }

        async fn consultar_evento(&self, _chave: &str, _tipo: &str, _sequencia: u32) -> Resultado {
            unreachable!("os testes nao consultam evento")
        }

        async fn registrar_evento(
            &self,
            _tipo: &str,
            _chave: &str,
            _motivo: &str,
            _justificativa: &str,
        ) -> Resultado {
            unreachable!("os testes nao registram evento")
        }
    }

    fn bloquear<T>(futuro: impl Future<Output = T>) -> T {
        let mut futuro = std::pin::pin!(futuro);
        let mut cx = Context::from_waker(Waker::noop());
        loop {
            if let Poll::Ready(v) = futuro.as_mut().poll(&mut cx) {
                return v;
            }
        }
    }

    fn pedido_sem_numero() -> Value {
        let mut p: Value = serde_json::from_str(DPS).unwrap();
        p.as_object_mut().unwrap().remove("numero");
        p
    }

    #[test]
    fn completo_devolve_a_rejeicao_com_codigo_e_a_forma_json_continua_recusa() {
        let emissor = Falso {
            status: Status::Rejeitada,
            recuperada: false,
            chamadas: AtomicUsize::new(0),
        };
        let emitente = Emitente::default();
        let pasta = PastaTemporaria::nova("pedido-rejeitada");
        let notas = Notas::abrir(pasta.caminho()).unwrap();
        let e = bloquear(emitir_pedido_completo(
            &emissor,
            pedido_sem_numero(),
            &emitente,
            60,
            Some(&notas),
        ))
        .unwrap();
        assert_eq!(e.resultado.status, Status::Rejeitada);
        assert_eq!(e.resultado.erros[0].codigo, "E0312");
        assert_eq!(e.numero_dps, Some(1));
        assert_eq!(e.guardada, None);
        // rejeitada nao gasta numero
        assert_eq!(notas.proximo_numero("1").unwrap(), 1);
        // a funcao de sempre continua devolvendo a recusa em texto
        let erro = bloquear(emitir_pedido(
            &emissor,
            pedido_sem_numero(),
            &emitente,
            60,
            Some(&notas),
        ))
        .unwrap_err();
        assert!(matches!(erro, Erro::Recusa { campo, .. } if campo == "sefin"));
        assert_eq!(emissor.chamadas.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn completo_guarda_a_nota_autorizada_e_a_forma_json_diz_o_mesmo() {
        let emissor = Falso {
            status: Status::Autorizada,
            recuperada: false,
            chamadas: AtomicUsize::new(0),
        };
        let emitente = Emitente::default();
        let pasta = PastaTemporaria::nova("pedido-autorizada");
        let notas = Notas::abrir(pasta.caminho()).unwrap();
        let e = bloquear(emitir_pedido_completo(
            &emissor,
            pedido_sem_numero(),
            &emitente,
            60,
            Some(&notas),
        ))
        .unwrap();
        assert_eq!(e.resultado.status, Status::Autorizada);
        assert_eq!(e.numero_dps, Some(1));
        assert_eq!(e.guardada, Some(true));
        assert_eq!(e.aviso, None);
        assert!(e.diferencas.is_empty());
        assert_eq!(notas.listar().unwrap().len(), 1);
        assert_eq!(notas.proximo_numero("1").unwrap(), 2);
        // a mesma DPS, na forma de sempre: JSON com `guardada` e `numero_dps`
        let json = bloquear(emitir_pedido(
            &emissor,
            pedido_sem_numero(),
            &emitente,
            60,
            Some(&notas),
        ))
        .unwrap();
        assert_eq!(json["guardada"], true);
        assert_eq!(json["numero_dps"], 2);
        assert_eq!(json["status"], "autorizada");
        // sem pasta de notas nao ha o que guardar
        let sem = bloquear(emitir_pedido_completo(
            &emissor,
            serde_json::from_str(DPS).unwrap(),
            &emitente,
            60,
            None,
        ))
        .unwrap();
        assert_eq!(sem.guardada, None);
        assert_eq!(sem.numero_dps, None);
    }

    fn recuperador() -> Falso {
        Falso {
            status: Status::Autorizada,
            recuperada: true,
            chamadas: AtomicUsize::new(0),
        }
    }

    #[test]
    fn nota_recuperada_com_outros_dados_diz_em_que_difere() {
        let emissor = recuperador();
        let emitente = Emitente::default();
        // o pedido de exemplo nao e' a nota de exemplo: tomador, valor, servico e competencia sao outros
        let pedido: Value = serde_json::from_str(DPS).unwrap();
        let e = bloquear(emitir_pedido_completo(
            &emissor,
            pedido.clone(),
            &emitente,
            60,
            None,
        ))
        .unwrap();
        assert!(e.resultado.recuperada);
        assert_eq!(
            e.diferencas,
            [
                "tomador",
                "valor do servico",
                "codigo do servico",
                "competencia"
            ]
        );
        let json = bloquear(emitir_pedido(&emissor, pedido, &emitente, 60, None)).unwrap();
        assert_eq!(json["recuperada"], true);
        assert_eq!(json["recuperada_difere"][0], "tomador");
        assert_eq!(json["recuperada_difere"].as_array().unwrap().len(), 4);
    }

    #[test]
    fn a_mesma_nota_escrita_de_outro_jeito_nao_acusa_diferenca() {
        let nfse = ler_nfse(&nfse_valida()).unwrap();
        // os dados da nota de exemplo com mascara, virgula e zero a direita
        let enviado = Enviado {
            tomador: documento("33.444.555/0001-72"),
            valor: "12,450".into(),
            servico: so_digitos("01.03.01"),
            competencia: Some("2026-09-23".into()),
        };
        assert!(enviado.diferencas(&nfse).is_empty());
        // sem competencia no pedido (vale a de hoje), ela nao entra na comparacao
        let sem = Enviado {
            competencia: None,
            ..enviado
        };
        assert!(sem.diferencas(&nfse).is_empty());
        // um centavo a mais ja e' outra nota
        let outro_valor = Enviado {
            valor: "12.46".into(),
            ..sem
        };
        assert_eq!(outro_valor.diferencas(&nfse), ["valor do servico"]);
        // o CNPJ alfanumerico nao perde as letras na comparacao
        assert_eq!(documento("12.abc.345/01de-35"), "12ABC34501DE35");
        let alfanumerico = Enviado {
            tomador: documento("12.ABC.345/01DE-35"),
            ..outro_valor
        };
        assert_eq!(
            alfanumerico.diferencas(&nfse),
            ["tomador", "valor do servico"]
        );
    }

    #[test]
    fn centavos_le_o_valor_escrito_de_varios_jeitos() {
        assert_eq!(centavos("150"), Some(15_000));
        assert_eq!(centavos("150.5"), Some(15_050));
        assert_eq!(centavos(" 150,50 "), Some(15_050));
        assert_eq!(centavos("12.450"), Some(1_245));
        assert_eq!(centavos("0.07"), Some(7));
        for ruim in ["", "abc", "1.234", ".5", "1.2.3", "-1"] {
            assert_eq!(centavos(ruim), None, "{ruim}");
        }
    }

    /// Um emissor que so' cancela e consulta: responde o `status` dado e conta as chamadas.
    struct DeCancelamento {
        status: Status,
        cancelamentos: AtomicUsize,
        cancelada: Status,
    }

    #[async_trait]
    impl Emissor for DeCancelamento {
        fn ambiente(&self) -> u8 {
            2
        }

        async fn emitir(&self, _dps: &Dps) -> Resultado {
            unreachable!("esses testes nao emitem")
        }

        async fn consultar(&self, _chave: &str) -> Resultado {
            let xml = nfse_valida();
            let nfse = ler_nfse(&xml).unwrap();
            let mut r = resposta(Status::Autorizada);
            r.chave = nfse.chave.clone();
            r.nfse = Some(nfse);
            r.xml_retorno = xml;
            r
        }

        async fn cancelar(&self, _chave: &str, _motivo: &str, _justificativa: &str) -> Resultado {
            self.cancelamentos.fetch_add(1, Ordering::Relaxed);
            let mut r = resposta(self.status);
            if self.status == Status::Rejeitada {
                r.erros = vec![Mensagem {
                    codigo: "E0001".into(),
                    descricao: "recusado".into(),
                    complemento: String::new(),
                }];
            }
            r
        }

        async fn cancelada(&self, _chave: &str) -> Resultado {
            resposta(self.cancelada)
        }

        async fn consultar_dps(&self, _id_dps: &str, _com_nota: bool) -> Resultado {
            unreachable!("os testes nao consultam a DPS")
        }

        async fn consultar_evento(&self, _chave: &str, _tipo: &str, _sequencia: u32) -> Resultado {
            unreachable!("os testes nao consultam evento")
        }

        async fn registrar_evento(
            &self,
            _tipo: &str,
            _chave: &str,
            _motivo: &str,
            _justificativa: &str,
        ) -> Resultado {
            unreachable!("os testes nao registram evento")
        }
    }

    fn resposta(status: Status) -> Resultado {
        Resultado {
            status,
            frase: "resposta de teste".into(),
            chave: String::new(),
            id_dps: String::new(),
            nfse: None,
            xml_assinado: String::new(),
            xml_retorno: String::new(),
            erros: vec![],
            alertas: vec![],
            http: 200,
            recuperada: false,
            corpo: None,
            detalhe: String::new(),
        }
    }

    fn pasta_com_a_nota(rotulo: &str) -> (PastaTemporaria, Notas, String) {
        let pasta = PastaTemporaria::nova(rotulo);
        let notas = Notas::abrir(pasta.caminho()).unwrap();
        let xml = nfse_valida();
        let nfse = ler_nfse(&xml).unwrap();
        notas.guardar(&Registro::da_nfse(&nfse), &xml).unwrap();
        (pasta, notas, nfse.chave)
    }

    #[test]
    fn cancelar_completo_so_marca_a_pasta_quando_o_cancelamento_foi_registrado() {
        for (status, marca) in [
            (Status::Registrado, Some(true)),
            (Status::Rejeitada, None),
            (Status::Pendente, None),
            (Status::Indisponivel, None),
        ] {
            let (_pasta, notas, chave) = pasta_com_a_nota("cancelar-completo");
            let emissor = DeCancelamento {
                status,
                cancelamentos: AtomicUsize::new(0),
                cancelada: Status::NaoEncontrada,
            };
            let c = bloquear(cancelar_pedido_completo(
                &emissor,
                &chave,
                "1",
                "justificativa de teste",
                Some(&notas),
            ));
            assert_eq!(c.resultado.status, status);
            assert_eq!(c.marcada, marca, "{status:?}");
            let situacao = notas.listar().unwrap()[0].situacao;
            assert_eq!(
                situacao == crate::notas::Situacao::Cancelada,
                marca == Some(true),
                "{status:?}"
            );
            assert_eq!(emissor.cancelamentos.load(Ordering::Relaxed), 1);
            // a forma JSON de sempre: so' o registrado e' sucesso
            let json = bloquear(cancelar_pedido(
                &emissor,
                &chave,
                "1",
                "justificativa de teste",
                None,
            ));
            assert_eq!(json.is_ok(), status == Status::Registrado, "{status:?}");
        }
        // recusa de evento repetido com a nota JA cancelada: o desfecho e' "registrado", e a pasta e' marcada
        let (_pasta, notas, chave) = pasta_com_a_nota("cancelar-repetido");
        let repetido = DeCancelamento {
            status: Status::Rejeitada,
            cancelamentos: AtomicUsize::new(0),
            cancelada: Status::Ok,
        };
        let c = bloquear(cancelar_pedido_completo(
            &repetido,
            &chave,
            "1",
            "justificativa de teste",
            Some(&notas),
        ));
        assert_eq!(c.resultado.status, Status::Registrado);
        assert!(c.resultado.recuperada);
        assert!(c.resultado.erros.is_empty());
        assert_eq!(c.resultado.alertas[0].codigo, "E0001");
        assert_eq!(c.marcada, Some(true));
        assert_eq!(
            notas.listar().unwrap()[0].situacao,
            crate::notas::Situacao::Cancelada
        );
        let json = bloquear(cancelar_pedido(
            &repetido,
            &chave,
            "1",
            "justificativa de teste",
            None,
        ))
        .unwrap();
        assert_eq!(json["status"], "registrado");
        assert_eq!(json["recuperada"], true);
        // nota fora da pasta: o cancelamento vale, a marca nao
        let (_pasta, notas, _) = pasta_com_a_nota("cancelar-fora");
        let emissor = DeCancelamento {
            status: Status::Registrado,
            cancelamentos: AtomicUsize::new(0),
            cancelada: Status::NaoEncontrada,
        };
        let outra =
            rustnf::chave_dfe::montar_nfse(1_400_159, "11222333000181", 77, "2609", Some(1), 2)
                .unwrap();
        let c = bloquear(cancelar_pedido_completo(
            &emissor,
            &outra,
            "1",
            "justificativa de teste",
            Some(&notas),
        ));
        assert_eq!(c.marcada, Some(false));
    }

    #[test]
    fn consultar_completa_diz_se_a_nota_achada_esta_cancelada() {
        for (cancelada, esperado) in [
            (Status::Ok, Some(true)),
            (Status::NaoEncontrada, Some(false)),
            (Status::Pendente, None),
            (Status::Erro, None),
        ] {
            let emissor = DeCancelamento {
                status: Status::Registrado,
                cancelamentos: AtomicUsize::new(0),
                cancelada,
            };
            let c = bloquear(crate::emissao::consultar_completa(&emissor, "x"));
            assert_eq!(c.resultado.status, Status::Autorizada);
            assert!(c.resultado.nfse.is_some());
            assert_eq!(c.cancelada, esperado, "{cancelada:?}");
        }
    }
}
