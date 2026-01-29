# Build btcminer with OpenCL support on Windows
# Run from PowerShell (not WSL)

param(
    [switch]$Release,
    [string]$Target = "x86_64-pc-windows-msvc"
)

$ErrorActionPreference = "Stop"

# Set OpenCL SDK paths
$OpenCLSDK = "$env:USERPROFILE\OpenCL-SDK\OpenCL-SDK-v2025.07.23-Win-x64"
if (-not (Test-Path $OpenCLSDK)) {
    Write-Error "OpenCL SDK not found at $OpenCLSDK. Please download from https://github.com/KhronosGroup/OpenCL-SDK/releases"
    exit 1
}

$env:LIB = "$env:LIB;$OpenCLSDK\lib"
$env:INCLUDE = "$env:INCLUDE;$OpenCLSDK\include"

# Initialize VS Developer environment
$VsDevShell = "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\Common7\Tools\Launch-VsDevShell.ps1"
if (-not (Test-Path $VsDevShell)) {
    Write-Error "Visual Studio Build Tools not found. Please install: winget install Microsoft.VisualStudio.2022.BuildTools"
    exit 1
}

Write-Host "Initializing Visual Studio environment..." -ForegroundColor Cyan
& $VsDevShell -Arch amd64 -SkipAutomaticLocation

# Navigate to project (works with both WSL and native paths)
$ProjectPath = $PSScriptRoot | Split-Path -Parent
if ($ProjectPath -match "wsl") {
    # WSL path - use as-is
    Push-Location $ProjectPath
} else {
    # Try WSL path first, fall back to script location
    $WslPath = "\\wsl.localhost\Ubuntu\home\fentas\github\fentas\miner"
    if (Test-Path $WslPath) {
        Push-Location $WslPath
    } else {
        Push-Location $ProjectPath
    }
}

Write-Host "Building in: $(Get-Location)" -ForegroundColor Cyan
Write-Host "OpenCL SDK: $OpenCLSDK" -ForegroundColor Cyan

# Build
$buildArgs = @("build", "--features", "opencl", "--target", $Target)
if ($Release) {
    $buildArgs += "--release"
    Write-Host "Building release with OpenCL..." -ForegroundColor Green
} else {
    Write-Host "Building debug with OpenCL..." -ForegroundColor Green
}

cargo @buildArgs

if ($LASTEXITCODE -eq 0) {
    $config = if ($Release) { "release" } else { "debug" }
    $exe = "target\$Target\$config\btcminer.exe"
    Write-Host "`nBuild successful!" -ForegroundColor Green
    Write-Host "Binary: $exe" -ForegroundColor Cyan
} else {
    Write-Error "Build failed with exit code $LASTEXITCODE"
}

Pop-Location
