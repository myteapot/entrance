param(
    [string]$Destination = "A:\entrance(github)"
)

$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot

function Normalize-RelativePath {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path
    )

    return ($Path -replace "/", "\").TrimStart("\")
}

function Get-RelativePath {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Root,
        [Parameter(Mandatory = $true)]
        [string]$Path
    )

    $uriRoot = [System.Uri](([System.IO.Path]::GetFullPath($Root).TrimEnd("\") + "\"))
    $uriPath = [System.Uri]([System.IO.Path]::GetFullPath($Path))
    $relative = $uriRoot.MakeRelativeUri($uriPath).ToString()
    return Normalize-RelativePath ([System.Uri]::UnescapeDataString($relative))
}

$excludedDirectories = @(
    ".git",
    "node_modules",
    "dist",
    "dist-electron",
    "shell\gui\dist",
    "target",
    "releases\v0.3.1-headless-alpha.1\package"
)

$excludedFiles = @(
    "entrance.key",
    "entrance.toml",
    "shell\gui\.cargo\config.local.toml",
    "releases\export-public-snapshot.ps1"
)

$excludedPatterns = @(
    "releases\*.zip",
    "releases\*\*.zip",
    "releases\*SHA256SUMS.txt",
    "releases\*\SHA256SUMS.txt",
    "releases\*\package\*"
)

function Test-ExcludedRelativePath {
    param(
        [Parameter(Mandatory = $true)]
        [string]$RelativePath
    )

    $normalized = Normalize-RelativePath $RelativePath

    foreach ($directory in $excludedDirectories) {
        $candidate = Normalize-RelativePath $directory
        if ($normalized -eq $candidate -or $normalized.StartsWith("$candidate\")) {
            return $true
        }
    }

    foreach ($file in $excludedFiles) {
        if ($normalized -eq (Normalize-RelativePath $file)) {
            return $true
        }
    }

    foreach ($pattern in $excludedPatterns) {
        if ($normalized -like (Normalize-RelativePath $pattern)) {
            return $true
        }
    }

    return $false
}

$sourceRoot = [System.IO.Path]::GetFullPath($repoRoot)
$destinationRoot = [System.IO.Path]::GetFullPath($Destination)

if ($sourceRoot -eq $destinationRoot) {
    throw "Destination must not be the same as the source repository root."
}

if (-not (Test-Path -LiteralPath $destinationRoot)) {
    New-Item -ItemType Directory -Path $destinationRoot | Out-Null
}

$sourceFiles = Get-ChildItem -LiteralPath $sourceRoot -Recurse -File -Force | Where-Object {
    $relative = Get-RelativePath -Root $sourceRoot -Path $_.FullName
    -not (Test-ExcludedRelativePath -RelativePath $relative)
}

$copiedCount = 0
$sourceRelativeSet = [System.Collections.Generic.HashSet[string]]::new(
    [System.StringComparer]::OrdinalIgnoreCase
)

foreach ($file in $sourceFiles) {
    $relative = Get-RelativePath -Root $sourceRoot -Path $file.FullName
    $null = $sourceRelativeSet.Add($relative)
    $destinationPath = Join-Path $destinationRoot $relative
    $destinationParent = Split-Path -Parent $destinationPath

    if (-not (Test-Path -LiteralPath $destinationParent)) {
        New-Item -ItemType Directory -Path $destinationParent -Force | Out-Null
    }

    Copy-Item -LiteralPath $file.FullName -Destination $destinationPath -Force
    $copiedCount += 1
}

$removedFiles = 0
$destinationFiles = Get-ChildItem -LiteralPath $destinationRoot -Recurse -File -Force

foreach ($file in $destinationFiles) {
    $relative = Get-RelativePath -Root $destinationRoot -Path $file.FullName
    if (Test-ExcludedRelativePath -RelativePath $relative) {
        continue
    }

    if (-not $sourceRelativeSet.Contains($relative)) {
        Remove-Item -LiteralPath $file.FullName -Force
        $removedFiles += 1
    }
}

$removedDirectories = 0
$destinationDirectories = Get-ChildItem -LiteralPath $destinationRoot -Recurse -Directory -Force |
    Sort-Object FullName -Descending

foreach ($directory in $destinationDirectories) {
    $relative = Get-RelativePath -Root $destinationRoot -Path $directory.FullName
    if ([string]::IsNullOrWhiteSpace($relative)) {
        continue
    }

    if (Test-ExcludedRelativePath -RelativePath $relative) {
        continue
    }

    $remaining = Get-ChildItem -LiteralPath $directory.FullName -Force
    if ($remaining.Count -eq 0) {
        Remove-Item -LiteralPath $directory.FullName -Force
        $removedDirectories += 1
    }
}

Write-Output "Exported public snapshot."
Write-Output "Source: $sourceRoot"
Write-Output "Destination: $destinationRoot"
Write-Output "Copied files: $copiedCount"
Write-Output "Removed stale files: $removedFiles"
Write-Output "Removed empty directories: $removedDirectories"
