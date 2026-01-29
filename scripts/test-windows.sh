#!/bin/bash
# Test btcminer on Windows with debug logging
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TIMEOUT=${1:-120}  # Default 2 minutes, or pass as argument

echo "Testing btcminer on Windows with debug logging (timeout: ${TIMEOUT}s)..."
powershell.exe -NoProfile -Command "
taskkill /F /IM btcminer.exe 2>\$null
taskkill /F /IM bitcoind.exe 2>\$null
Start-Sleep 2
cd C:\Users\fentas\miner
Start-Process -FilePath '.\target\release\btcminer.exe' -ArgumentList '--no-hud --debug' -RedirectStandardOutput 'test.log' -NoNewWindow
Start-Sleep $TIMEOUT
taskkill /F /IM btcminer.exe 2>\$null
taskkill /F /IM bitcoind.exe 2>\$null
Get-Content test.log
"
