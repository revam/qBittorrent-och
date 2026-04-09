$ErrorActionPreference = "Stop"

$Repo = "revam/qBittorrent-och"
$DefaultInstallDir = Join-Path $env:LOCALAPPDATA "qb-och"

function Get-Usage {
    @"
Usage: install.ps1 [OPTIONS]

Install qb-och binary for your system.

OPTIONS:
    -InstallPath <path>  Directory to install binary (default: $DefaultInstallDir)
    -Force              Overwrite existing binary
    -Help               Show this help message

EXAMPLES:
    .\install.ps1
    .\install.ps1 -InstallPath "C:\Program Files\qb-och"
    .\install.ps1 -Force
"@
}

$InstallDir = $DefaultInstallDir
$Force = $false

$args = $args | ForEach-Object { $_ }

for ($i = 0; $i -lt $args.Count; $i++) {
    switch ($args[$i]) {
        "-InstallPath" {
            $InstallDir = $args[$i + 1]
            $i++
        }
        "-Force" { $Force = $true }
        "-Help" { Get-Usage; exit 0 }
        default {
            Write-Error "Unknown option: $($args[$i])"
            exit 1
        }
    }
}

$Architecture = if ($env:PROCESSOR_ARCHITECTURE -eq "AMD64" -or $env:PROCESSOR_ARCHITECTURE -eq "x86_64") { "x64" } else { "arm64" }
$Artifact = "qb-och-windows-${Architecture}.zip"

Write-Host "Detecting system..."
Write-Host "Detected: OS=windows, Arch=$Architecture"

Write-Host "Using artifact: $Artifact"

Write-Host "Fetching latest release info..."
$Response = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest" -UseBasicParsing
$Tag = $Response.tag_name

Write-Host "Latest version: $Tag"

$Url = "https://github.com/$Repo/releases/download/$Tag/$Artifact"
Write-Host "Downloading from: $Url"

$TempDir = [System.IO.Path]::GetTempPath() + [System.Guid]::NewGuid().ToString()
New-Item -ItemType Directory -Path $TempDir -Force | Out-Null
$ZipPath = Join-Path $TempDir "qb-och.zip"

try {
    Invoke-WebRequest -Uri $Url -OutFile $ZipPath -UseBasicParsing
}
catch {
    Write-Error "Failed to download: $_"
    exit 1
}

if ((Test-Path $ZipPath) -and (Get-Item $ZipPath).Length -eq 0) {
    Write-Error "Download failed or empty file"
    exit 1
}

if (-not (Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
}

$ExistingBinary = Join-Path $InstallDir "qb-och.exe"
if ((Test-Path $ExistingBinary) -and -not $Force) {
    Write-Error "Binary already exists at $ExistingBinary. Use -Force to overwrite."
    exit 1
}

Expand-Archive -Path $ZipPath -DestinationPath $TempDir -Force
$ExtractedBinary = Get-ChildItem -Path $TempDir -Filter "qb-och.exe" -Recurse | Select-Object -First 1

if (-not $ExtractedBinary) {
    Write-Error "Could not find qb-och.exe in archive"
    exit 1
}

Copy-Item -Path $ExtractedBinary.FullName -Destination $ExistingBinary -Force

$Version = & $ExistingBinary --version 2>&1 | Select-Object -First 1

Write-Host ""
Write-Host "Installed qb-och $Version to $ExistingBinary"
Write-Host "Add $InstallDir to your PATH if not already included."

Remove-Item -Path $TempDir -Recurse -Force