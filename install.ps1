# PowerShell installation script for brain on Windows
[CmdletBinding()]
param (
    [string]$Version = "latest",
    [string]$InstallDir = "$HOME\.local\bin"
)

$ErrorActionPreference = "Stop"

$Repo = "caiosaboia/agy-brain"
$BinaryName = "brain.exe"
$AliasBinary = "agy-brain.exe"

Write-Host "Installing brain..."

if (-not (Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
}

$Installed = $false

# Try GitHub Release download first
try {
    $ReleaseUrl = if ($Version -eq "latest") {
        "https://github.com/$Repo/releases/latest/download/agy-brain-x86_64-pc-windows-msvc.zip"
    } else {
        "https://github.com/$Repo/releases/download/$Version/agy-brain-x86_64-pc-windows-msvc.zip"
    }

    $ZipPath = Join-Path $env:TEMP "brain.zip"
    Write-Host "Downloading release from $ReleaseUrl..."
    Invoke-WebRequest -Uri $ReleaseUrl -OutFile $ZipPath -UseBasicParsing
    
    Expand-Archive -Path $ZipPath -DestinationPath $InstallDir -Force
    Remove-Item -Path $ZipPath -Force -ErrorAction SilentlyContinue
    $Installed = $true
} catch {
    Write-Host "Pre-compiled release download skipped or unavailable ($($_.Exception.Message))."
}

# Fallback: if cargo is present and binary was not downloaded, build from source
if (-not $Installed) {
    if (Get-Command cargo -ErrorAction SilentlyContinue) {
        Write-Host "Building from source using Cargo..."
        cargo install --git "https://github.com/$Repo" --force
        $Installed = $true
    } else {
        Write-Error "Could not download pre-built release and 'cargo' was not found on PATH."
        exit 1
    }
}

# Ensure destination directory is in user PATH if downloaded directly
$TargetExe = Join-Path $InstallDir $BinaryName
$AliasExe = Join-Path $InstallDir $AliasBinary

if ((Test-Path $TargetExe) -and (-not (Test-Path $AliasExe))) {
    Copy-Item -Path $TargetExe -Destination $AliasExe -Force
} elseif ((Test-Path $AliasExe) -and (-not (Test-Path $TargetExe))) {
    Copy-Item -Path $AliasExe -Destination $TargetExe -Force
}

if (Test-Path $TargetExe) {
    $CurrentPath = [Environment]::GetEnvironmentVariable("PATH", "User")
    if ($CurrentPath -split ';' -notcontains $InstallDir) {
        [Environment]::SetEnvironmentVariable("PATH", "$InstallDir;$CurrentPath", "User")
        $env:PATH = "$InstallDir;$env:PATH"
        Write-Host "Added $InstallDir to user PATH."
    }
}

# Run automated shell hook installation
Write-Host "Configuring shell hook in PowerShell profile..."
if (Get-Command brain -ErrorAction SilentlyContinue) {
    & brain install powershell
} elseif (Get-Command agy-brain -ErrorAction SilentlyContinue) {
    & agy-brain install powershell
} elseif (Test-Path $TargetExe) {
    & $TargetExe install powershell
}

Write-Host "Installation completed successfully."
Write-Host "Run 'brain doctor' or restart your terminal to use 'brain' and 'cd-brain'."
