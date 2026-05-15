param(
    [string]$Version = "v2.0.0-preview",
    [string]$BinaryPath = ""
)

& (Join-Path $PSScriptRoot "package-release.ps1") `
    -Version $Version `
    -BinaryPath $BinaryPath `
    -AssetName "entrance-$Version-windows-x64-headless"
