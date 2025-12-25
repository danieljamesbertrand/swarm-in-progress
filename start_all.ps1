# PowerShell script to start the complete Kademlia P2P network with monitoring
# Usage: .\start_all.ps1

Write-Host "=== Starting Complete Kademlia P2P Network ===" -ForegroundColor Green
Write-Host ""

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $scriptDir

# Check if binaries exist
if (-not (Test-Path "target\release\monitor.exe")) {
    Write-Host "Binaries not found. Building..." -ForegroundColor Yellow
    cargo build --release
    if ($LASTEXITCODE -ne 0) {
        Write-Host "Build failed!" -ForegroundColor Red
        exit 1
    }
}

Write-Host "[1/5] Starting Network Monitor (Bootstrap + Web Dashboard)..." -ForegroundColor Cyan
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$scriptDir'; Write-Host '=== NETWORK MONITOR ===' -ForegroundColor Green; Write-Host 'Dashboard: http://localhost:8080' -ForegroundColor Yellow; Write-Host ''; cargo run --release --bin monitor"
Start-Sleep -Seconds 5

Write-Host "[2/5] Starting Listener 1 (Peer A)..." -ForegroundColor Cyan
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$scriptDir'; Write-Host '=== LISTENER 1 (PEER A) ===' -ForegroundColor Yellow; Write-Host 'Namespace: test-room' -ForegroundColor Gray; Write-Host ''; cargo run --release --bin listener -- --bootstrap /ip4/127.0.0.1/tcp/51820 --namespace test-room"
Start-Sleep -Seconds 3

Write-Host "[3/5] Starting Listener 2 (Peer B)..." -ForegroundColor Cyan
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$scriptDir'; Write-Host '=== LISTENER 2 (PEER B) ===' -ForegroundColor Yellow; Write-Host 'Namespace: test-room' -ForegroundColor Gray; Write-Host ''; cargo run --release --bin listener -- --bootstrap /ip4/127.0.0.1/tcp/51820 --namespace test-room"
Start-Sleep -Seconds 3

Write-Host "[4/5] Starting Dialer 1 (Peer C)..." -ForegroundColor Cyan
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$scriptDir'; Write-Host '=== DIALER 1 (PEER C) ===' -ForegroundColor Magenta; Write-Host 'Namespace: test-room' -ForegroundColor Gray; Write-Host ''; cargo run --release --bin dialer -- --bootstrap /ip4/127.0.0.1/tcp/51820 --namespace test-room"
Start-Sleep -Seconds 2

Write-Host "[5/5] Starting Dialer 2 (Peer D)..." -ForegroundColor Cyan
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$scriptDir'; Write-Host '=== DIALER 2 (PEER D) ===' -ForegroundColor Magenta; Write-Host 'Namespace: test-room' -ForegroundColor Gray; Write-Host ''; cargo run --release --bin dialer -- --bootstrap /ip4/127.0.0.1/tcp/51820 --namespace test-room"
Start-Sleep -Seconds 2

Write-Host ""
Write-Host "=== Network Started Successfully! ===" -ForegroundColor Green
Write-Host ""
Write-Host "You should see 5 PowerShell windows:" -ForegroundColor White
Write-Host "  1. Network Monitor (Green) - Bootstrap + Web Dashboard" -ForegroundColor Green
Write-Host "  2. Listener 1 - Peer A (Yellow)" -ForegroundColor Yellow
Write-Host "  3. Listener 2 - Peer B (Yellow)" -ForegroundColor Yellow
Write-Host "  4. Dialer 1 - Peer C (Magenta)" -ForegroundColor Magenta
Write-Host "  5. Dialer 2 - Peer D (Magenta)" -ForegroundColor Magenta
Write-Host ""
Write-Host "🌐 Web Dashboard: http://localhost:8080" -ForegroundColor Cyan
Write-Host ""
Write-Host "Expected behavior:" -ForegroundColor White
Write-Host "  - Monitor window shows bootstrap node running" -ForegroundColor Gray
Write-Host "  - Listeners register in DHT and wait for connections" -ForegroundColor Gray
Write-Host "  - Dialers discover and connect to listeners" -ForegroundColor Gray
Write-Host "  - Dashboard shows all nodes and connections in real-time" -ForegroundColor Gray
Write-Host ""
Write-Host "Opening dashboard in browser..." -ForegroundColor Yellow
Start-Sleep -Seconds 2
Start-Process "http://localhost:8080"

Write-Host ""
Write-Host "Press any key to exit (windows will remain open)..." -ForegroundColor White
$null = $Host.UI.RawUI.ReadKey("NoEcho,IncludeKeyDown")

