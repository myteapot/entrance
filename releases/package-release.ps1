param(
    [string]$Version = "v1.0.0-rc.1",
    [string]$BinaryPath = "",
    [string]$AssetName = ""
)

$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
$resolvedBinaryPath = $BinaryPath

if ([string]::IsNullOrWhiteSpace($resolvedBinaryPath)) {
    $resolvedBinaryPath = Join-Path $repoRoot "target\release\entrance.exe"
} elseif (-not [System.IO.Path]::IsPathRooted($resolvedBinaryPath)) {
    $resolvedBinaryPath = Join-Path $repoRoot $resolvedBinaryPath
}

if ([string]::IsNullOrWhiteSpace($AssetName)) {
    $AssetName = "entrance-$Version-windows-x64"
}

$binaryRoot = Split-Path -Parent $resolvedBinaryPath
$releaseRoot = Join-Path $repoRoot "releases\$Version"
$stageRoot = Join-Path $releaseRoot "package"
$assetRoot = Join-Path $stageRoot $AssetName
$zipPath = Join-Path $releaseRoot "$AssetName.zip"
$shaPath = Join-Path $releaseRoot "SHA256SUMS.txt"
$releaseNotesPath = Join-Path $releaseRoot "RELEASE_NOTES.md"

$requiredBinaries = @("entrance.exe")
$optionalBinaries = @()

foreach ($binary in $requiredBinaries) {
    $candidate = Join-Path $binaryRoot $binary
    if (-not (Test-Path $candidate)) {
        throw "Required binary not found at $candidate. Build the workspace release binaries first."
    }
}

if (-not (Test-Path $releaseNotesPath)) {
    throw "Release notes not found at $releaseNotesPath."
}

if (Test-Path $stageRoot) {
    Remove-Item -Recurse -Force $stageRoot
}

if (Test-Path $zipPath) {
    Remove-Item -Force $zipPath
}

New-Item -ItemType Directory -Path $assetRoot -Force | Out-Null

foreach ($binary in ($requiredBinaries + $optionalBinaries)) {
    $candidate = Join-Path $binaryRoot $binary
    if (Test-Path $candidate) {
        Copy-Item $candidate (Join-Path $assetRoot $binary)
    }
}

Copy-Item (Join-Path $repoRoot "README.md") (Join-Path $assetRoot "README.md")
Copy-Item (Join-Path $repoRoot "LICENSE") (Join-Path $assetRoot "LICENSE")
Copy-Item (Join-Path $repoRoot "LICENSES.md") (Join-Path $assetRoot "LICENSES.md")
Copy-Item (Join-Path $repoRoot "TRADEMARKS.md") (Join-Path $assetRoot "TRADEMARKS.md")
Copy-Item (Join-Path $repoRoot "CONTRIBUTING.md") (Join-Path $assetRoot "CONTRIBUTING.md")
Copy-Item $releaseNotesPath (Join-Path $assetRoot "RELEASE_NOTES.md")

Compress-Archive -Path (Join-Path $assetRoot "*") -DestinationPath $zipPath -Force

$hashLines = @()
Get-ChildItem $assetRoot -Filter "*.exe" | Sort-Object Name | ForEach-Object {
    $hash = (Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLower()
    $hashLines += "$hash *package/$AssetName/$($_.Name)"
}
$zipHash = (Get-FileHash $zipPath -Algorithm SHA256).Hash.ToLower()
$hashLines += "$zipHash *$AssetName.zip"

python -c "import pathlib, sys; pathlib.Path(sys.argv[1]).write_text('\n'.join(sys.argv[2:]) + '\n', encoding='utf-8')" `
    $shaPath `
    $hashLines

Write-Output "Packaged $Version"
Write-Output "Asset root: $assetRoot"
Write-Output "ZIP: $zipPath"
Write-Output "SHA256: $shaPath"
