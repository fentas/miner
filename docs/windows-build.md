# Building btcminer on Windows with GPU (OpenCL) Support

This guide covers building btcminer natively on Windows with OpenCL support for GPU mining.

## Prerequisites

### 1. Visual Studio Build Tools

Install Visual Studio Build Tools with C++ workload:

```powershell
winget install Microsoft.VisualStudio.2022.BuildTools
```

Then add the C++ tools (run as Administrator):

```powershell
& "C:\Program Files (x86)\Microsoft Visual Studio\Installer\setup.exe" modify `
    --installPath "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools" `
    --add Microsoft.VisualStudio.Component.VC.Tools.x86.x64 `
    --add Microsoft.VisualStudio.Component.Windows11SDK.22621 `
    -q --norestart
```

### 2. Rust

Install Rust via rustup:

```powershell
winget install Rustlang.Rustup
```

### 3. OpenCL SDK

Download the Khronos OpenCL SDK:

1. Go to: https://github.com/KhronosGroup/OpenCL-SDK/releases
2. Download the latest `OpenCL-SDK-vX.X.X-Win-x64.zip`
3. Extract to `%USERPROFILE%\OpenCL-SDK\`

The expected path structure:
```
C:\Users\<username>\OpenCL-SDK\OpenCL-SDK-vX.X.X-Win-x64\
├── include\
│   └── CL\
└── lib\
    └── OpenCL.lib
```

### 4. GPU Drivers with OpenCL Runtime

#### AMD GPUs
Install AMD Adrenalin drivers from: https://www.amd.com/en/support

OpenCL runtime is included automatically with the driver.

#### NVIDIA GPUs
Install NVIDIA drivers with CUDA/OpenCL support from: https://www.nvidia.com/drivers

#### Intel GPUs
Install Intel Graphics drivers from: https://www.intel.com/content/www/us/en/download-center/home.html

### Verify OpenCL Installation

```powershell
# Check if OpenCL DLL exists
Get-ChildItem "C:\Windows\System32\*opencl*"

# Or install clinfo
winget install Khronos.OpenCLDebugger
clinfo
```

## Building

### Option 1: PowerShell Script

From the project directory:

```powershell
.\scripts\build-windows-opencl.ps1 -Release
```

### Option 2: Batch File

Create or use the existing batch file at `C:\Users\<username>\build_opencl.bat`:

```batch
@echo off
call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\Common7\Tools\VsDevCmd.bat" -arch=amd64
set PATH=%USERPROFILE%\.cargo\bin;%PATH%
set LIB=%LIB%;%USERPROFILE%\OpenCL-SDK\OpenCL-SDK-v2025.07.23-Win-x64\lib
set INCLUDE=%INCLUDE%;%USERPROFILE%\OpenCL-SDK\OpenCL-SDK-v2025.07.23-Win-x64\include
cd /d C:\Users\<username>\miner
cargo build --release --features opencl
```

Then run:
```cmd
C:\Users\<username>\build_opencl.bat
```

### Option 3: From WSL2

Use the helper script to sync and build:

```bash
./scripts/build-windows.sh
```

## Environment Variables

For manual builds, set these environment variables:

```powershell
$env:LIB = "$env:LIB;$env:USERPROFILE\OpenCL-SDK\OpenCL-SDK-v2025.07.23-Win-x64\lib"
$env:INCLUDE = "$env:INCLUDE;$env:USERPROFILE\OpenCL-SDK\OpenCL-SDK-v2025.07.23-Win-x64\include"
```

To set permanently (user-level):

```powershell
[Environment]::SetEnvironmentVariable("OPENCL_LIB", "$env:USERPROFILE\OpenCL-SDK\OpenCL-SDK-v2025.07.23-Win-x64\lib", "User")
[Environment]::SetEnvironmentVariable("OPENCL_INCLUDE", "$env:USERPROFILE\OpenCL-SDK\OpenCL-SDK-v2025.07.23-Win-x64\include", "User")
```

## Troubleshooting

### "link.exe not found"

The Visual Studio Developer environment isn't set up. Either:
- Run from "Developer Command Prompt for VS 2022"
- Use the batch file that calls `VsDevCmd.bat`
- Use the PowerShell script that calls `Launch-VsDevShell.ps1`

### "OpenCL.lib not found"

The OpenCL SDK path isn't set in LIB environment variable. Make sure:
1. OpenCL SDK is extracted to the expected location
2. LIB environment variable includes the SDK lib path

### "GPU enabled but no OpenCL devices found"

This means the binary compiled with OpenCL, but no runtime is available:
1. Install GPU drivers with OpenCL support
2. Verify with `clinfo` command
3. For AMD: Install AMD Adrenalin drivers
4. For NVIDIA: Install NVIDIA drivers with CUDA

### "Access denied" when building

The previous btcminer.exe is still running. Stop it first:
```powershell
Stop-Process -Name btcminer -Force
```

## Output

After successful build, the binary is at:
```
target\release\btcminer.exe
```

Copy to your desired location or run directly:
```powershell
.\target\release\btcminer.exe
```
