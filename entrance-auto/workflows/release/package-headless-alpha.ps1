param(
    [string]$Version = "v0.3.1-headless-alpha.1",
    [string]$BinaryPath = ""
)

& (Join-Path $PSScriptRoot "package-release.ps1") `
    -Version $Version `
    -BinaryPath $BinaryPath `
    -AssetName "entrance-$Version-windows-x64-headless"
