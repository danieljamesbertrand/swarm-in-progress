# PowerShell test script for Kademlia P2P testbed
# Usage: .\test.ps1

Write-Host "=== Starting Kademlia P2P Testbed ===" -ForegroundColor Green
Write-Host ""

# Get current directory
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $scriptDir

# Check if binaries exist
if (-not (Test-Path "target\release\server.exe")) {
    Write-Host "Binaries not found. Building..." -ForegroundColor Yellow
    cargo build --release
    if ($LASTEXITCODE -ne 0) {
        Write-Host "Build failed!" -ForegroundColor Red
        exit 1
    }
}

Write-Host "[1/4] Starting bootstrap node..." -ForegroundColor Cyan
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$scriptDir'; Write-Host '=== BOOTSTRAP NODE ===' -ForegroundColor Green; cargo run --release --bin server -- --listen-addr 0.0.0.0 --port 51820"
Start-Sleep -Seconds 3

Write-Host "[2/4] Starting listener (Peer A)..." -ForegroundColor Cyan
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$scriptDir'; Write-Host '=== LISTENER (PEER A) ===' -ForegroundColor Yellow; cargo run --release --bin listener -- --bootstrap /ip4/127.0.0.1/tcp/51820 --namespace test-room"
Start-Sleep -Seconds 5

Write-Host "[3/4] Starting dialer (Peer B)..." -ForegroundColor Cyan
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$scriptDir'; Write-Host '=== DIALER (PEER B) ===' -ForegroundColor Magenta; cargo run --release --bin dialer -- --bootstrap /ip4/127.0.0.1/tcp/51820 --namespace test-room"
Start-Sleep -Seconds 2

Write-Host "[4/4] Starting additional listener (Peer C)..." -ForegroundColor Cyan
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$scriptDir'; Write-Host '=== LISTENER (PEER C) ===' -ForegroundColor Yellow; cargo run --release --bin listener -- --bootstrap /ip4/127.0.0.1/tcp/51820 --namespace test-room"
Start-Sleep -Seconds 2

Write-Host ""
Write-Host "=== Testbed Started Successfully ===" -ForegroundColor Green
Write-Host ""
Write-Host "You should see 4 PowerShell windows:" -ForegroundColor White
Write-Host "  1. Bootstrap Node (Green)" -ForegroundColor Green
Write-Host "  2. Listener - Peer A (Yellow)" -ForegroundColor Yellow
Write-Host "  3. Dialer - Peer B (Magenta)" -ForegroundColor Magenta
Write-Host "  4. Listener - Peer C (Yellow)" -ForegroundColor Yellow
Write-Host ""
Write-Host "Expected behavior:" -ForegroundColor White
Write-Host "  - Dialer should discover and connect to both listeners" -ForegroundColor Gray
Write-Host "  - Messages should be exchanged between peers" -ForegroundColor Gray
Write-Host "  - All peers should be in the same namespace (test-room)" -ForegroundColor Gray
Write-Host ""
Write-Host "Press any key to exit (windows will remain open)..." -ForegroundColor White
$null = $Host.UI.RawUI.ReadKey("NoEcho,IncludeKeyDown")














