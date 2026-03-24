param(
    [string]$Version = "v0.3.1-headless-alpha.1",
    [string]$BinaryPath = ""
)

$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
$resolvedBinaryPath = $BinaryPath

if ([string]::IsNullOrWhiteSpace($resolvedBinaryPath)) {
    $resolvedBinaryPath = Join-Path $repoRoot "src-tauri\target\release\entrance.exe"
} elseif (-not [System.IO.Path]::IsPathRooted($resolvedBinaryPath)) {
    $resolvedBinaryPath = Join-Path $repoRoot $resolvedBinaryPath
}

$releaseRoot = Join-Path $repoRoot "releases\$Version"
$stageRoot = Join-Path $releaseRoot "package"
$assetRoot = Join-Path $stageRoot "entrance-$Version-windows-x64-headless"
$zipPath = Join-Path $releaseRoot "entrance-$Version-windows-x64-headless.zip"
$shaPath = Join-Path $releaseRoot "SHA256SUMS.txt"

if (-not (Test-Path $resolvedBinaryPath)) {
    throw "Binary not found at $resolvedBinaryPath. Build the release binary first."
}

if (Test-Path $stageRoot) {
    Remove-Item -Recurse -Force $stageRoot
}

if (Test-Path $zipPath) {
    Remove-Item -Force $zipPath
}

New-Item -ItemType Directory -Path $assetRoot | Out-Null

Copy-Item $resolvedBinaryPath (Join-Path $assetRoot "entrance.exe")
Copy-Item (Join-Path $repoRoot "README.md") (Join-Path $assetRoot "README.md")
Copy-Item (Join-Path $repoRoot "LICENSE") (Join-Path $assetRoot "LICENSE")
Copy-Item (Join-Path $repoRoot "LICENSES.md") (Join-Path $assetRoot "LICENSES.md")
Copy-Item (Join-Path $repoRoot "TRADEMARKS.md") (Join-Path $assetRoot "TRADEMARKS.md")
Copy-Item (Join-Path $repoRoot "CONTRIBUTING.md") (Join-Path $assetRoot "CONTRIBUTING.md")
Copy-Item (Join-Path $releaseRoot "RELEASE_NOTES.md") (Join-Path $assetRoot "RELEASE_NOTES.md")

Compress-Archive -Path (Join-Path $assetRoot "*") -DestinationPath $zipPath -Force

$exeHash = (Get-FileHash (Join-Path $assetRoot "entrance.exe") -Algorithm SHA256).Hash.ToLower()
$zipHash = (Get-FileHash $zipPath -Algorithm SHA256).Hash.ToLower()

@(
    "$exeHash *package/entrance-$Version-windows-x64-headless/entrance.exe"
    "$zipHash *entrance-$Version-windows-x64-headless.zip"
) | Set-Content -Path $shaPath

Write-Output "Packaged $Version"
Write-Output "ZIP: $zipPath"
Write-Output "SHA256: $shaPath"
