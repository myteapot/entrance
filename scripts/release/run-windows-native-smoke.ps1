param(
    [string]$EntranceExePath = "",
    [ValidateSet("Debug", "Release")]
    [string]$Configuration = "Release",
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = [System.IO.Path]::GetFullPath((Join-Path $scriptDir "..\.."))
$manifestPath = Join-Path $repoRoot "src-tauri\Cargo.toml"
$nativeProject = Join-Path $repoRoot "tests\native\EntranceNativeTests.csproj"

if ([string]::IsNullOrWhiteSpace($EntranceExePath)) {
    $profileDir = $Configuration.ToLowerInvariant()
    $EntranceExePath = Join-Path $repoRoot "src-tauri\target\$profileDir\entrance.exe"
} elseif (-not [System.IO.Path]::IsPathRooted($EntranceExePath)) {
    $EntranceExePath = Join-Path $repoRoot $EntranceExePath
}

if (-not $SkipBuild.IsPresent) {
    if ($Configuration -eq "Release") {
        & cargo build --locked --manifest-path $manifestPath --release
    } else {
        & cargo build --locked --manifest-path $manifestPath
    }
}

if (-not (Test-Path $EntranceExePath)) {
    throw "Expected Entrance executable at '$EntranceExePath'"
}

$env:ENTRANCE_EXE_PATH = $EntranceExePath
Write-Host "[windows-native] ENTRANCE_EXE_PATH=$EntranceExePath"

& dotnet test $nativeProject --configuration $Configuration --logger "trx;LogFileName=EntranceNativeTests.trx"
