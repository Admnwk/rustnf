// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Aleatorio do SISTEMA, um so' lugar do crate: `getrandom` 0.4 `SysRng`, a mesma fonte que o `crypto-common` 0.2
//! do rsa 0.10 usa.
//!
//! Falha do RNG do sistema = panico: segredo com byte fixo e' pior que parar.

pub use getrandom::rand_core::{CryptoRng, Rng, UnwrapErr};
pub use getrandom::SysRng;

/// RNG criptografico infalivel (entra em `RsaPrivateKey::new`, `sign_with_rng`, `encrypt`, `random_range`).
#[must_use]
pub fn sistema() -> UnwrapErr<SysRng> {
    UnwrapErr(SysRng)
}

/// Preenche `b` com bytes do sistema.
pub fn preencher(b: &mut [u8]) {
    sistema().fill_bytes(b);
}

/// `N` bytes do sistema.
#[must_use]
pub fn bytes<const N: usize>() -> [u8; N] {
    let mut b = [0u8; N];
    preencher(&mut b);
    b
}
