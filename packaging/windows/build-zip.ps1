<#
.SYNOPSIS
  Zip a Flutter Windows release runner directory for GitHub Releases / CI.

.PARAMETER BundleDir
  Path to build/windows/x64/runner/Release (contains umbra.exe / the Flutter app).

.PARAMETER OutFile
  Output .zip path. Default: umbra-<Version>-windows-x64.zip in the current directory.

.PARAMETER Version
  Version embedded in the default OutFile name. Falls back to $env:UMBRA_VERSION or 0.0.0-dev.

.EXAMPLE
  packaging/windows/build-zip.ps1 -BundleDir app/build/windows/x64/runner/Release
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$BundleDir,

    [Parameter(Mandatory = $false)]
    [string]$OutFile = "",

    [Parameter(Mandatory = $false)]
    [string]$Version = ""
)

$ErrorActionPreference = "Stop"

if (-not (Test-Path -LiteralPath $BundleDir -PathType Container)) {
    Write-Error "Flutter Windows bundle directory not found: $BundleDir"
    exit 1
}

if ([string]::IsNullOrWhiteSpace($Version)) {
    if ($env:UMBRA_VERSION) { $Version = $env:UMBRA_VERSION } else { $Version = "0.0.0-dev" }
}

if ([string]::IsNullOrWhiteSpace($OutFile)) {
    $OutFile = Join-Path (Get-Location) ("umbra-{0}-windows-x64.zip" -f $Version)
}

$OutDir = Split-Path -Parent $OutFile
if ($OutDir -and -not (Test-Path -LiteralPath $OutDir)) {
    New-Item -ItemType Directory -Path $OutDir | Out-Null
}

if (Test-Path -LiteralPath $OutFile) {
    Remove-Item -LiteralPath $OutFile -Force
}

# Compress the contents of Release/ so extracting yields the exe at the top level.
Compress-Archive -Path (Join-Path $BundleDir "*") -DestinationPath $OutFile -CompressionLevel Optimal
Write-Host "Wrote $OutFile"
