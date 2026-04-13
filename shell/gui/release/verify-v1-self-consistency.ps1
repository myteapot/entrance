param(
    [string]$ManifestPath = "",
    [string]$OutputDir = "",
    [switch]$SkipE2E
)

$ErrorActionPreference = "Stop"

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = [System.IO.Path]::GetFullPath((Join-Path $scriptDir "..\.."))

if ([string]::IsNullOrWhiteSpace($ManifestPath)) {
    $ManifestPath = Join-Path $repoRoot "scripts\release\reconciliation-batch-01.json"
} elseif (-not [System.IO.Path]::IsPathRooted($ManifestPath)) {
    $ManifestPath = Join-Path $repoRoot $ManifestPath
}

if ([string]::IsNullOrWhiteSpace($OutputDir)) {
    if ($env:VERIFY_V1_OUTPUT_DIR) {
        $OutputDir = $env:VERIFY_V1_OUTPUT_DIR
    } else {
        $OutputDir = Join-Path $repoRoot "test-results\release-self-consistency"
    }
}
if (-not [System.IO.Path]::IsPathRooted($OutputDir)) {
    $OutputDir = Join-Path $repoRoot $OutputDir
}
New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null

$entranceBin = $env:ENTRANCE_EXE_PATH
$useCargo = $true
if (-not [string]::IsNullOrWhiteSpace($entranceBin)) {
    $useCargo = $false
} else {
    $candidateExe = Join-Path $repoRoot "hosts/desktop/tauri\target\debug\entrance.exe"
    $candidatePosix = Join-Path $repoRoot "hosts/desktop/tauri/target/debug/entrance"
    if (Test-Path $candidateExe) {
        $entranceBin = $candidateExe
        $useCargo = $false
    } elseif (Test-Path $candidatePosix) {
        $entranceBin = $candidatePosix
        $useCargo = $false
    }
}

function Invoke-Entrance {
    param([string[]]$CommandArgs)

    if ($useCargo) {
        & cargo run --manifest-path (Join-Path $repoRoot "hosts/desktop/tauri/Cargo.toml") -- @CommandArgs
    } else {
        & $entranceBin @CommandArgs
    }
}

function Write-JsonSnapshot {
    param(
        [string]$FileName,
        [string[]]$CommandArgs
    )

    $output = Invoke-Entrance -CommandArgs $CommandArgs
    $path = Join-Path $OutputDir $FileName
    [System.IO.File]::WriteAllText($path, ($output -join [Environment]::NewLine), [System.Text.Encoding]::UTF8)
    return $path
}

Write-Host "[verify] capturing runtime closure status"
$statusPath = Write-JsonSnapshot -FileName "nota-status.json" -CommandArgs @("nota", "status")
$status = Get-Content $statusPath -Raw | ConvertFrom-Json
if ($status.round_state.state -ne "fully_settled") {
    throw "Expected round_state.state=fully_settled, got '$($status.round_state.state)'"
}
if ($status.round_state.carry_forward_checkpointed -ne $true) {
    throw "Expected carry_forward_checkpointed=true"
}

$invariantsPath = Write-JsonSnapshot -FileName "nota-invariants.json" -CommandArgs @("nota", "invariants")
$invariants = Get-Content $invariantsPath -Raw | ConvertFrom-Json
if ($invariants.failed_count -ne 0) {
    throw "Expected nota invariants failed_count=0, got $($invariants.failed_count)"
}

$repairPath = Write-JsonSnapshot -FileName "nota-repair.json" -CommandArgs @("nota", "repair")
$repair = Get-Content $repairPath -Raw | ConvertFrom-Json
if ($repair.open_count -ne 0) {
    throw "Expected nota repair open_count=0, got $($repair.open_count)"
}

Write-Host "[verify] applying reconciliation batch"
$batchPath = Write-JsonSnapshot -FileName "landing-reconcile-batch-apply.json" -CommandArgs @("landing", "reconcile", "batch-apply", "--file", $ManifestPath)
$reportPath = Write-JsonSnapshot -FileName "landing-reconcile-report.json" -CommandArgs @("landing", "reconcile", "report")
$planningPath = Write-JsonSnapshot -FileName "landing-planning.json" -CommandArgs @("landing", "planning")

$report = Get-Content $reportPath -Raw | ConvertFrom-Json
if ($report.unreconciled_count -gt 38) {
    throw "Expected unreconciled_count <= 38, got $($report.unreconciled_count)"
}

$requiredKeys = @(
    "linear:microt:Entrance:issue:MYT-56",
    "linear:microt:Entrance:issue:MYT-61",
    "linear:microt:Entrance:issue:MYT-63",
    "linear:microt:Entrance:issue:MYT-64",
    "linear:microt:Entrance:issue:MYT-65"
)
$planning = Get-Content $planningPath -Raw | ConvertFrom-Json
foreach ($key in $requiredKeys) {
    $item = $planning | Where-Object { $_.canonical_key -eq $key }
    if (-not $item) {
        throw "Missing planning item for $key"
    }
    if ($item.reconciliation_status -eq "unreconciled") {
        throw "Expected $key to be reconciled"
    }
}

Write-Host "[verify] running type + rust baselines"
& cargo test --manifest-path (Join-Path $repoRoot "hosts/desktop/tauri/Cargo.toml") --lib
& pnpm -C $repoRoot check

if (-not $SkipE2E.IsPresent) {
    Write-Host "[verify] running browser e2e"
    if ($IsLinux) {
        & pnpm -C $repoRoot exec vite --version
        if ($LASTEXITCODE -ne 0) {
            Write-Host "vite/rollup runtime probe failed; reinstalling dependencies"
            if (Test-Path (Join-Path $repoRoot "node_modules")) {
                Remove-Item -Recurse -Force (Join-Path $repoRoot "node_modules")
            }
            & pnpm -C $repoRoot install --frozen-lockfile
            & pnpm -C $repoRoot exec vite --version
        }
    }
    & pnpm -C $repoRoot test:e2e
}

Write-Host "[verify] complete; snapshots written to $OutputDir"
