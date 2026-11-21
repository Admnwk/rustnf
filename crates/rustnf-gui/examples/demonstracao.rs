// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A mesma janela do `rustnf-gui`, mas com um emissor de demonstracao: nada e' enviado a Sefin Nacional. Serve para
//! clicar na Nova nota inteira (preencher, emitir, ver a nota autorizada, a rejeicao e a tentativa sem resposta).
//!
//! ```text
//! cargo run -p rustnf-gui --features demonstracao --example demonstracao -- \
//!     --config rustnf.toml [--resultado autorizada|rejeitada|sem-resposta]
//! ```
//!
//! `autorizada` (padrao) autoriza a nota; `rejeitada` devolve duas mensagens de rejeicao; `sem-resposta` deixa a
//! primeira tentativa sem resposta e recupera a nota na segunda ("Tentar de novo"). A nota autorizada entra de
//! verdade na pasta de notas do arquivo de configuracao indicado: use uma pasta de ensaio.

use std::process::ExitCode;

#[cfg(windows)]
fn main() -> ExitCode {
    use std::sync::Arc;
    use std::time::Duration;

    use rustnf_gui::demonstracao::{roteiro_do_nome, EmissorDemonstracao, FabricaDemonstracao};
    use rustnf_gui::modelo::argumentos::{self, Pedido};
    use rustnf_gui::servicos::Servicos;

    let mut resto = Vec::new();
    let mut resultado = "autorizada".to_string();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        if a == "--resultado" {
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
    let ambiente = base.carregar().map_or(2, |carga| carga.ambiente.codigo());
    let emissor = EmissorDemonstracao::novo(ambiente, &roteiro, Duration::from_millis(1200));
    let servicos = base.com_fabrica(FabricaDemonstracao::nova(emissor));
    rustnf_gui::telas::executar_com(Arc::new(servicos));
    ExitCode::SUCCESS
}

#[cfg(not(windows))]
fn main() -> ExitCode {
    eprintln!("demonstracao: a interface de desktop so' existe para Windows nesta versao.");
    ExitCode::from(2)
}
