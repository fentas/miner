#!/bin/bash
# Test CAT/QoS availability and benchmark on native Windows
# This runs outside WSL2 to access hardware features
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
WIN_DIR="/mnt/c/Users/fentas/miner"

echo "=== CAT/QoS and Benchmark Test on Native Windows ==="
echo ""

echo "1. Syncing project to Windows..."
rsync -av --delete \
    --exclude 'target' \
    --exclude '.git' \
    --exclude 'dist' \
    --exclude '*.txt' \
    --exclude '*.log' \
    "$PROJECT_DIR/" "$WIN_DIR/"

echo ""
echo "2. Checking CPU info on Windows..."
powershell.exe -NoProfile -Command "
Get-WmiObject -Class Win32_Processor | Select-Object Name, NumberOfCores, NumberOfLogicalProcessors, MaxClockSpeed | Format-List
"

echo ""
echo "3. Checking for RDT/CAT support..."
# On Windows, RDT features are exposed via specific MSRs and WMI
powershell.exe -NoProfile -Command "
Write-Host 'Checking Windows resource control features...'

# Check if RDT features are available via CPUID (would need admin)
# For now, just note it's Windows native (not virtualized)
\$vm = Get-WmiObject -Class Win32_ComputerSystem
if (\$vm.Model -match 'Virtual') {
    Write-Host 'Running in a VM - CAT/RDT NOT available'
} else {
    Write-Host 'Running on bare metal - CAT/RDT potentially available'
    Write-Host 'Note: AMD Platform QoS requires specific kernel driver support'
}
"

echo ""
echo "4. Building release binary..."
powershell.exe -NoProfile -Command "
cd C:\Users\fentas\miner
cargo build --release 2>&1
"

echo ""
echo "5. Running v10 research test (cache analysis)..."
powershell.exe -NoProfile -Command "
cd C:\Users\fentas\miner
cargo test --release --test research_v10_test -- --nocapture 2>&1
"

echo ""
echo "6. Running v11 compiler test..."
powershell.exe -NoProfile -Command "
cd C:\Users\fentas\miner
cargo test --release --test research_v11_test -- --nocapture 2>&1
"

echo ""
echo "=== Done! Compare results with WSL2 ==="
