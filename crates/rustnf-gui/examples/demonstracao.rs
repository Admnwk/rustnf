// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A mesma janela do `rustnf-gui`, mas com um emissor de demonstracao: nada e' enviado a Sefin Nacional. Serve para
//! clicar na interface inteira: emitir (autorizada, rejeitada, sem resposta), ver as notas da pasta, consultar pela
//! chave ou pela DPS, cancelar, conferir a situacao das notas na Sefin, ver o historico de eventos de uma nota, pedir a
//! analise fiscal do cancelamento e confirmar ou recusar uma nota recebida; e, para a NF-e (venda de mercadoria), ver as
//! notas da pasta, abrir o DANFE, conferir na SEFAZ, cancelar, corrigir por carta, inutilizar numeros, retomar a nota
//! sem resposta e testar a conexao, tudo com uma SEFAZ de mentira.
//!
//! ```text
//! cargo run -p rustnf-gui --features demonstracao --example demonstracao -- \
//!     --config rustnf.toml [--resultado autorizada|rejeitada|sem-resposta] \
//!     [--cancelamento registrado|rejeitado|sem-resposta] [--evento registrado|rejeitado|sem-resposta] \
//!     [--nfe-evento registrado|rejeitado|sem-resposta] [--nfe-emissao autorizada|rejeitada|sem-resposta|fora] \
//!     [--atraso-ms 1200]
//! ```
//!
//! `--resultado` e' o roteiro da emissao: `autorizada` (padrao) autoriza a nota; `rejeitada` devolve duas mensagens de
//! rejeicao; `sem-resposta` deixa a primeira tentativa sem resposta e recupera a nota na segunda ("Tentar de novo").
//! `--cancelamento` e' o roteiro do cancelamento: `registrado` (padrao); `rejeitado` devolve duas mensagens;
//! `sem-resposta` deixa o primeiro pedido sem resposta e registra no segundo; `rejeitado` reproduz a recusa do
//! cancelamento direto (a mensagem diz que o prazo passou). `--evento` e' o mesmo roteiro para os pedidos de analise
//! fiscal, de confirmacao e de recusa. A demonstracao tambem "recebeu" uma nota de outra pessoa, com o emitente das
//! Configuracoes como tomador: o programa escreve a chave dela ao abrir; cole-a em Consultar para ver Confirmar e
//! Recusar. A consulta devolve as notas que a
//! demonstracao autorizou e as que estao na pasta de notas do ensaio. A nota autorizada entra de verdade na pasta de
//! notas do arquivo de configuracao indicado: use uma pasta de ensaio.
//!
//! `--nfe-emissao` e' o roteiro da SEFAZ de mentira para a EMISSAO da NF-e pela janela (Nova nota, NF-e): `autorizada`
//! (padrao); `rejeitada` recusa a primeira tentativa (NCM inexistente) e autoriza a seguinte com o mesmo numero;
//! `sem-resposta` deixa a primeira sem resposta e a mesma chave e' achada autorizada no Retomar; `fora` diz que o servico
//! esta parado (a pasta nao ganha a NF-e sem resposta, para a contingencia EPEC poder ser oferecida). A pasta de ensaio
//! tambem ganha um cadastro com tres produtos.
//!
//! `--nfe-evento` e' o roteiro da SEFAZ de mentira da NF-e para o cancelamento, a carta de correcao e a inutilizacao:
//! `registrado` (padrao); `rejeitado` recusa (o cancelamento com 501, o prazo; a carta e a inutilizacao com outro
//! codigo); `sem-resposta` deixa a primeira tentativa sem resposta e registra na segunda. Com a `[nfe]` configurada
//! (pelo menos a UF), o programa semeia a pasta de ensaio com tres NF-e: uma autorizada, uma cancelada e uma sem
//! resposta (o "Retomar" a resolve).

use std::process::ExitCode;

#[cfg(windows)]
fn main() -> ExitCode {
    use std::sync::Arc;
    use std::time::Duration;

    use rustnf_gui::demonstracao::{
        cancelamento_do_nome, roteiro_do_nome, EmissorDemonstracao, Ensaio, FabricaDemonstracao,
    };
    use rustnf_gui::demonstracao_eventos::evento_do_nome;
    use rustnf_gui::demonstracao_nfe::{
        roteiro_emissao_do_nome, roteiro_nfe_do_nome, semear_com, FabricaNfeDemonstracao,
        RoteiroEmissao, TransporteNfeDemonstracao,
    };
    use rustnf_gui::modelo::argumentos::{self, Pedido};
    use rustnf_gui::servicos::Servicos;

    let mut resto = Vec::new();
    let mut resultado = "autorizada".to_string();
    let mut cancelamento = "registrado".to_string();
    let mut evento = "registrado".to_string();
    let mut nfe_evento = "registrado".to_string();
    let mut nfe_emissao = "autorizada".to_string();
    let mut atraso_ms: u64 = 1200;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        if a == "--atraso-ms" {
            match args.next().and_then(|v| v.parse().ok()) {
                Some(v) => atraso_ms = v,
                None => {
                    eprintln!("demonstracao: --atraso-ms precisa de um numero");
                    return ExitCode::from(2);
                }
            }
        } else if a == "--cancelamento" {
            match args.next() {
                Some(v) => cancelamento = v,
                None => {
                    eprintln!("demonstracao: --cancelamento precisa de um valor");
                    return ExitCode::from(2);
                }
            }
        } else if a == "--nfe-evento" {
            match args.next() {
                Some(v) => nfe_evento = v,
                None => {
                    eprintln!("demonstracao: --nfe-evento precisa de um valor");
                    return ExitCode::from(2);
                }
            }
        } else if a == "--nfe-emissao" {
            match args.next() {
                Some(v) => nfe_emissao = v,
                None => {
                    eprintln!("demonstracao: --nfe-emissao precisa de um valor");
                    return ExitCode::from(2);
                }
            }
        } else if a == "--evento" {
            match args.next() {
                Some(v) => evento = v,
                None => {
                    eprintln!("demonstracao: --evento precisa de um valor");
                    return ExitCode::from(2);
                }
            }
        } else if a == "--resultado" {
            match args.next() {
                Some(v) => resultado = v,
                None => {
                    eprintln!("demonstracao: --resultado precisa de um valor");
                    return ExitCode::from(2);
                }
            }
        } else {
            resto.push(a);
        }
    }
    let Some(roteiro) = roteiro_do_nome(&resultado) else {
        eprintln!("demonstracao: --resultado aceita autorizada, rejeitada ou sem-resposta");
        return ExitCode::from(2);
    };
    let Some(roteiro_do_cancelamento) = cancelamento_do_nome(&cancelamento) else {
        eprintln!("demonstracao: --cancelamento aceita registrado, rejeitado ou sem-resposta");
        return ExitCode::from(2);
    };
    let Some(roteiro_do_evento) = evento_do_nome(&evento) else {
        eprintln!("demonstracao: --evento aceita registrado, rejeitado ou sem-resposta");
        return ExitCode::from(2);
    };
    let Some(roteiro_da_nfe) = roteiro_nfe_do_nome(&nfe_evento) else {
        eprintln!("demonstracao: --nfe-evento aceita registrado, rejeitado ou sem-resposta");
        return ExitCode::from(2);
    };
    let Some(roteiro_da_emissao) = roteiro_emissao_do_nome(&nfe_emissao) else {
        eprintln!("demonstracao: --nfe-emissao aceita autorizada, rejeitada, sem-resposta ou fora");
        return ExitCode::from(2);
    };
    let config = match argumentos::interpretar(resto) {
        Ok(Pedido::Abrir { config }) => config,
        Ok(_) => {
            eprintln!("demonstracao: use --config <arquivo> e, se quiser, --resultado <nome>");
            return ExitCode::from(2);
        }
        Err(e) => {
            eprintln!("demonstracao: {e}");
            return ExitCode::from(2);
        }
    };
    let base = Servicos::do_sistema(config);
    // o emissor de demonstracao fala o mesmo ambiente que a configuracao (o selo mostra "Demonstracao" por cima)
    let (ambiente, pasta, documento) = base.carregar().map_or((2, None, None), |carga| {
        let pasta = carga.config.notas.pasta.trim().to_string();
        let e = &carga.config.emitente;
        let documento = if e.cnpj.trim().is_empty() {
            e.cpf.trim()
        } else {
            e.cnpj.trim()
        }
        .to_string();
        (
            carga.ambiente.codigo(),
            (!pasta.is_empty()).then(|| std::path::PathBuf::from(pasta)),
            (!documento.is_empty()).then_some(documento),
        )
    });
    let ensaio = Ensaio {
        cancelamento: roteiro_do_cancelamento,
        evento: roteiro_do_evento,
        pasta,
        recebida_para: documento,
    };
    let emissor = EmissorDemonstracao::com_ensaio(
        ambiente,
        &roteiro,
        ensaio,
        Duration::from_millis(atraso_ms),
    );
    if let Some(chave) = emissor.chave_da_nota_recebida() {
        println!(
            "demonstracao: nota recebida de outro emitente, para Confirmar e Recusar em Consultar: {chave}"
        );
    }
    let nfe = TransporteNfeDemonstracao::novo(roteiro_da_nfe, Duration::from_millis(atraso_ms))
        .com_emissao(roteiro_da_emissao);
    match base.carregar() {
        Ok(carga) if !carga.config.nfe.uf.trim().is_empty() => match semear_com(
            &carga.config,
            &nfe,
            roteiro_da_emissao != RoteiroEmissao::Fora,
        ) {
            Ok(n) => println!(
                "demonstracao: NF-e de ensaio na pasta: autorizada {}, cancelada {}, sem resposta {}",
                n.autorizada, n.cancelada, n.pendente
            ),
            Err(e) => eprintln!("demonstracao: nao foi possivel semear as NF-e de ensaio: {e}"),
        },
        _ => println!("demonstracao: sem [nfe].uf no arquivo, as NF-e de ensaio nao foram criadas"),
    }
    let servicos = base
        .com_fabrica(FabricaDemonstracao::nova(emissor))
        .com_fabrica_nfe(FabricaNfeDemonstracao::nova(nfe));
    rustnf_gui::telas::executar_com(Arc::new(servicos));
    ExitCode::SUCCESS
}

#[cfg(not(windows))]
fn main() -> ExitCode {
    eprintln!("demonstracao: a interface de desktop so' existe para Windows nesta versao.");
    ExitCode::from(2)
}
