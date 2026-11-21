// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Trabalho de rede fora da linha da tela. Os executores do GPUI nao sao do tokio, e o cliente da Sefin (reqwest)
//! precisa de um: este modulo guarda um runtime proprio, pequeno (2 linhas de trabalho), criado so' no primeiro uso.
//! Quem chama recebe um `JoinHandle`, que se aguarda de qualquer executor (inclusive do da tela) sem travar a tela.

use std::future::Future;
use std::sync::OnceLock;

use tokio::runtime::{Builder, Runtime};
use tokio::task::JoinHandle;

fn runtime() -> Option<&'static Runtime> {
    static RUNTIME: OnceLock<Option<Runtime>> = OnceLock::new();
    RUNTIME
        .get_or_init(|| {
            Builder::new_multi_thread()
                .worker_threads(2)
                .thread_name("rustnf-rede")
                .enable_all()
                .build()
                .ok()
        })
        .as_ref()
}

/// Roda `trabalho` no runtime proprio. `None` se o runtime nao pode ser criado.
pub fn rede<T>(trabalho: impl Future<Output = T> + Send + 'static) -> Option<JoinHandle<T>>
where
    T: Send + 'static,
{
    runtime().map(|rt| rt.spawn(trabalho))
}

#[cfg(test)]
mod testes {
    use std::pin::pin;
    use std::sync::Arc;
    use std::task::{Context, Poll, Wake, Waker};
    use std::thread::{self, Thread};
    use std::time::Duration;

    use super::*;

    struct Acordar(Thread);

    impl Wake for Acordar {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }

    /// Um executor minimo que nao e' do tokio: prova que o `JoinHandle` acorda quem o aguarda de fora.
    fn bloquear<T>(futuro: impl Future<Output = T>) -> T {
        let mut futuro = pin!(futuro);
        let waker = Waker::from(Arc::new(Acordar(thread::current())));
        let mut cx = Context::from_waker(&waker);
        loop {
            if let Poll::Ready(v) = futuro.as_mut().poll(&mut cx) {
                return v;
            }
            thread::park_timeout(Duration::from_secs(10));
        }
    }

    #[test]
    fn o_trabalho_roda_no_runtime_proprio_e_e_aguardado_de_um_executor_de_fora() {
        let origem = thread::current().id();
        let handle = rede(async move {
            // o runtime nao e' o da thread que chamou
            let aqui = thread::current();
            tokio::time::sleep(Duration::from_millis(30)).await;
            (aqui.id() != origem, aqui.name().map(str::to_owned))
        })
        .unwrap();
        let (outra_thread, nome) = bloquear(handle).unwrap();
        assert!(outra_thread);
        assert_eq!(nome.as_deref(), Some("rustnf-rede"));
    }

    #[test]
    fn uma_chamada_de_rede_bloqueada_nao_trava_quem_espera() {
        // a chamada demora 300 ms; quem chama volta na hora e so' depois aguarda o resultado
        let t0 = std::time::Instant::now();
        let handle = rede(async {
            tokio::time::sleep(Duration::from_millis(300)).await;
            7
        })
        .unwrap();
        assert!(t0.elapsed() < Duration::from_millis(100));
        assert_eq!(bloquear(handle).unwrap(), 7);
    }
}
