// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Abre o certificado A1 e monta o cliente real da Sefin Nacional: o que a linha de comando e a interface de desktop
//! compartilham. As mensagens de erro nunca citam o caminho do arquivo nem a senha.

use std::io::Read;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use rustnf::assinatura::cert_a1::{self, Certificado};
use rustnf::assinatura::xmldsig::Algoritmo;
use rustnf::nfse::sefin::ClienteNfse;
use zeroize::Zeroizing;

use crate::emissao::Emissor;
use crate::erro::Erro;
use crate::segredo::Segredo;

/// Teto do arquivo do certificado: 16 MiB.
pub const LIMITE_PFX: u64 = 16 * 1024 * 1024;

const O_QUE: &str = "o certificado";

/// Os bytes do arquivo `.pfx`/`.p12`, que sao zerados da memoria ao serem soltos.
///
/// # Errors
///
/// [`Erro::Entrada`] se o arquivo nao existir, nao abrir ou passar do teto.
pub fn ler_pfx(caminho: &Path) -> Result<Zeroizing<Vec<u8>>, Erro> {
    let arquivo = std::fs::File::open(caminho).map_err(|e| {
        Erro::Entrada(match e.kind() {
            std::io::ErrorKind::NotFound => format!("{O_QUE}: arquivo nao encontrado"),
            _ => format!("nao foi possivel abrir {O_QUE}"),
        })
    })?;
    let mut bytes = Zeroizing::new(Vec::new());
    arquivo
        .take(LIMITE_PFX + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Erro::Entrada(format!("nao foi possivel ler {O_QUE}")))?;
    if bytes.len() as u64 > LIMITE_PFX {
        return Err(Erro::Entrada(format!("{O_QUE} passa de 16 MiB")));
    }
    Ok(bytes)
}

/// Abre o `.pfx` com a senha.
///
/// # Errors
///
/// [`Erro::Entrada`] se o arquivo nao abrir ou a senha nao servir.
pub fn abrir(pfx: &Path, senha: &Segredo) -> Result<Certificado, Erro> {
    let bytes = ler_pfx(pfx)?;
    cert_a1::carregar(&bytes, senha.expor()).map_err(|e| Erro::Entrada(e.to_string()))
}

/// So' confere que a senha abre o certificado.
///
/// # Errors
///
/// Os mesmos de [`abrir`].
pub fn conferir(pfx: &Path, senha: &Segredo) -> Result<(), Erro> {
    abrir(pfx, senha).map(|_| ())
}

/// Monta o cliente real da Sefin Nacional (`ambiente` 1 producao, 2 producao restrita) com o certificado aberto.
///
/// # Errors
///
/// [`Erro`] se o certificado nao servir para o TLS mutuo.
pub fn cliente(
    certificado: Certificado,
    ambiente: u8,
    algoritmo: Algoritmo,
    tempo_limite: Duration,
) -> Result<ClienteNfse<rustnf::nfse::sefin::TransporteReqwest>, Erro> {
    let mut cliente = ClienteNfse::real(certificado, ambiente, None, None, tempo_limite)?;
    cliente.algoritmo = algoritmo;
    Ok(cliente)
}

/// Abre o `.pfx` e monta o emissor da Sefin para o ambiente.
///
/// # Errors
///
/// Os mesmos de [`abrir`] e de [`cliente`].
pub fn emissor(
    pfx: &Path,
    senha: &Segredo,
    ambiente: u8,
    algoritmo: Algoritmo,
    tempo_limite: Duration,
) -> Result<Arc<dyn Emissor>, Erro> {
    let certificado = abrir(pfx, senha)?;
    Ok(Arc::new(cliente(
        certificado,
        ambiente,
        algoritmo,
        tempo_limite,
    )?))
}

#[cfg(test)]
mod testes {
    use chrono::Utc;
    use rustnf::assinatura::cert_a1::gerar_teste;

    use super::*;
    use crate::testes_comuns::PastaTemporaria;

    fn pfx_de_teste(pasta: &PastaTemporaria, senha: &str) -> std::path::PathBuf {
        let bytes = gerar_teste(
            "11222333000181",
            "EMPRESA TESTE",
            senha,
            365,
            false,
            false,
            Utc::now(),
        )
        .unwrap();
        let caminho = pasta.caminho().join("teste.pfx");
        std::fs::write(&caminho, bytes).unwrap();
        caminho
    }

    #[test]
    fn abre_com_a_senha_certa_e_recusa_a_errada_sem_vazar() {
        let pasta = PastaTemporaria::nova("cert-abrir");
        let pfx = pfx_de_teste(&pasta, "senha-certa");
        let c = abrir(&pfx, &Segredo::novo("senha-certa".into())).unwrap();
        assert_eq!(c.documento(), "11222333000181");
        let erro = conferir(&pfx, &Segredo::novo("senha-errada".into())).unwrap_err();
        assert_eq!(erro.codigo_saida(), 2);
        assert!(!erro.to_string().contains("senha-errada"));
    }

    #[test]
    fn arquivo_ausente_e_ilegivel_dizem_so_o_que_houve() {
        let pasta = PastaTemporaria::nova("cert-ausente");
        let falta = pasta.caminho().join("nao-existe.pfx");
        let erro = abrir(&falta, &Segredo::novo("x".into())).unwrap_err();
        assert_eq!(erro.to_string(), "o certificado: arquivo nao encontrado");
        assert!(!erro.to_string().contains("nao-existe"));
        // uma pasta no lugar do arquivo
        let erro = abrir(pasta.caminho(), &Segredo::novo("x".into())).unwrap_err();
        assert!(matches!(erro, Erro::Entrada(_)));
    }
}
