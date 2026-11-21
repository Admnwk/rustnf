# Confere, no executavel da interface, o icone (recurso ICON de id 1, o que a janela carrega) e a versao gravada no
# recurso VERSIONINFO. Sai com erro se faltar um dos dois ou se a versao nao for a esperada.
#   pwsh -File .github/conferir_recursos.ps1 -Exe target/release/rustnf-gui.exe -Versao 0.3.0
param([Parameter(Mandatory)][string]$Exe, [Parameter(Mandatory)][string]$Versao)

$ErrorActionPreference = 'Stop'
$caminho = (Resolve-Path $Exe).Path

Add-Type -Namespace Recursos -Name Nativo -MemberDefinition @'
[System.Runtime.InteropServices.DllImport("kernel32.dll", SetLastError = true, CharSet = System.Runtime.InteropServices.CharSet.Unicode)]
public static extern System.IntPtr LoadLibraryExW(string arquivo, System.IntPtr reservado, uint opcoes);
[System.Runtime.InteropServices.DllImport("kernel32.dll", SetLastError = true)]
public static extern System.IntPtr FindResourceW(System.IntPtr modulo, System.IntPtr nome, System.IntPtr tipo);
[System.Runtime.InteropServices.DllImport("kernel32.dll")]
public static extern bool FreeLibrary(System.IntPtr modulo);
'@

# LOAD_LIBRARY_AS_DATAFILE: le os recursos sem executar nada do arquivo
$modulo = [Recursos.Nativo]::LoadLibraryExW($caminho, [IntPtr]::Zero, 2)
if ($modulo -eq [IntPtr]::Zero) { throw "nao foi possivel abrir $Exe" }
try {
    # RT_GROUP_ICON = 14, id 1
    $icone = [Recursos.Nativo]::FindResourceW($modulo, [IntPtr]1, [IntPtr]14)
    if ($icone -eq [IntPtr]::Zero) { throw "$Exe nao tem o icone (recurso ICON de id 1)" }
}
finally {
    [void][Recursos.Nativo]::FreeLibrary($modulo)
}

$info = (Get-Item $caminho).VersionInfo
"icone: ok; versao do arquivo: $($info.FileVersion); versao do produto: $($info.ProductVersion)"
if ($info.ProductVersion -ne $Versao -or $info.FileVersion -ne $Versao) {
    throw "a versao gravada em $Exe ($($info.ProductVersion)) nao e' a esperada ($Versao)"
}
