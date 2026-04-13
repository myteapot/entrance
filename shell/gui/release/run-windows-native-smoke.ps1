param(
    [string]$EntranceExePath = "",
    [ValidateSet("Debug", "Release")]
    [string]$Configuration = "Release",
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = [System.IO.Path]::GetFullPath((Join-Path $scriptDir "..\..\.."))
$nativeProject = Join-Path $repoRoot "shell\gui\platform\windows\tests\native\EntranceNativeTests.csproj"

if ([string]::IsNullOrWhiteSpace($EntranceExePath)) {
    $profileDir = $Configuration.ToLowerInvariant()
    $EntranceExePath = Join-Path $repoRoot "target\$profileDir\entrance-gui.exe"
} elseif (-not [System.IO.Path]::IsPathRooted($EntranceExePath)) {
    $EntranceExePath = Join-Path $repoRoot $EntranceExePath
}

if (-not $SkipBuild.IsPresent) {
    if ($Configuration -eq "Release") {
        & cargo build --locked --release -p entrance-gui --bin entrance-gui
    } else {
        & cargo build --locked -p entrance-gui --bin entrance-gui
    }
}

if (-not (Test-Path $EntranceExePath)) {
    throw "Expected Entrance GUI executable at '$EntranceExePath'"
}

$env:ENTRANCE_EXE_PATH = $EntranceExePath
Write-Host "[windows-native] ENTRANCE_EXE_PATH=$EntranceExePath"

& dotnet test $nativeProject --configuration $Configuration --logger "trx;LogFileName=EntranceNativeTests.trx"
