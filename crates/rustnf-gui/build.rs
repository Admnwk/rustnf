// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! No Windows, poe no executavel o icone (recurso ICON de id 1, o que a janela do GPUI carrega) e a versao (recurso
//! VERSIONINFO, a aba "Detalhes" das propriedades do arquivo). O `.rc` e' gerado aqui, com a versao do pacote: ela
//! nunca e' escrita a mao. Sem manifesto: o GPUI ja poe o dele.

use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=recursos/rustnf.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let versao = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
    let numeros: Vec<u32> = versao
        .split(['.', '-', '+'])
        .take(3)
        .map(|n| n.parse().unwrap_or(0))
        .chain(std::iter::repeat(0))
        .take(3)
        .collect();
    let numerica = format!("{},{},{},0", numeros[0], numeros[1], numeros[2]);
    let icone = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default())
        .join("recursos")
        .join("rustnf.ico");
    // no .rc a barra invertida e' escape: o caminho vai com barras normais, que o compilador de recursos aceita
    let icone = icone.to_string_lossy().replace('\\', "/");
    let rc = format!(
        r#"1 ICON "{icone}"

1 VERSIONINFO
FILEVERSION {numerica}
PRODUCTVERSION {numerica}
FILEFLAGSMASK 0x3F
FILEFLAGS 0x0
FILEOS 0x40004
FILETYPE 0x1
FILESUBTYPE 0x0
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "041604B0"
    BEGIN
      VALUE "CompanyName", "Os autores do rustnf"
      VALUE "FileDescription", "rustnf - emissor de NFS-e nacional"
      VALUE "FileVersion", "{versao}"
      VALUE "InternalName", "rustnf-gui"
      VALUE "LegalCopyright", "MPL-2.0"
      VALUE "OriginalFilename", "rustnf-gui.exe"
      VALUE "ProductName", "rustnf"
      VALUE "ProductVersion", "{versao}"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x416, 1200
  END
END
"#
    );
    let saida = PathBuf::from(std::env::var("OUT_DIR").unwrap_or_default()).join("rustnf-gui.rc");
    if std::fs::write(&saida, rc).is_err() {
        println!("cargo:warning=nao foi possivel gravar o recurso do executavel (icone e versao)");
        return;
    }
    // sem o compilador de recursos (ou com falha nele) o programa compila sem icone nem versao: o aviso aparece, e o
    // CI e o fluxo de release conferem o executavel e reprovam
    match embed_resource::compile(&saida, embed_resource::NONE) {
        embed_resource::CompilationResult::Ok | embed_resource::CompilationResult::NotWindows => {}
        outro => {
            println!("cargo:warning=icone e versao do executavel nao foram embutidos: {outro}")
        }
    }
}
