# PowerShell script to start an intensive 16-node Kademlia P2P network test
# Usage: .\start_intensive_16.ps1

Write-Host "=== Starting Intensive 16-Node Kademlia P2P Network Test ===" -ForegroundColor Green
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

$namespace = "intensive-test"
$bootstrap = "/ip4/127.0.0.1/tcp/51820"

Write-Host "[1/17] Starting Network Monitor (Bootstrap + Web Dashboard)..." -ForegroundColor Cyan
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$scriptDir'; Write-Host '=== NETWORK MONITOR ===' -ForegroundColor Green; Write-Host 'Dashboard: http://localhost:8080' -ForegroundColor Yellow; Write-Host ''; cargo run --release --bin monitor"
Start-Sleep -Seconds 5

# Start 8 listeners (they register in DHT and wait)
Write-Host "[2/17] Starting Listener 1..." -ForegroundColor Cyan
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$scriptDir'; Write-Host '=== LISTENER 1 ===' -ForegroundColor Yellow; Write-Host 'Namespace: $namespace' -ForegroundColor Gray; Write-Host ''; cargo run --release --bin listener -- --bootstrap $bootstrap --namespace $namespace"
Start-Sleep -Milliseconds 500

Write-Host "[3/17] Starting Listener 2..." -ForegroundColor Cyan
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$scriptDir'; Write-Host '=== LISTENER 2 ===' -ForegroundColor Yellow; Write-Host 'Namespace: $namespace' -ForegroundColor Gray; Write-Host ''; cargo run --release --bin listener -- --bootstrap $bootstrap --namespace $namespace"
Start-Sleep -Milliseconds 500

Write-Host "[4/17] Starting Listener 3..." -ForegroundColor Cyan
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$scriptDir'; Write-Host '=== LISTENER 3 ===' -ForegroundColor Yellow; Write-Host 'Namespace: $namespace' -ForegroundColor Gray; Write-Host ''; cargo run --release --bin listener -- --bootstrap $bootstrap --namespace $namespace"
Start-Sleep -Milliseconds 500

Write-Host "[5/17] Starting Listener 4..." -ForegroundColor Cyan
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$scriptDir'; Write-Host '=== LISTENER 4 ===' -ForegroundColor Yellow; Write-Host 'Namespace: $namespace' -ForegroundColor Gray; Write-Host ''; cargo run --release --bin listener -- --bootstrap $bootstrap --namespace $namespace"
Start-Sleep -Milliseconds 500

Write-Host "[6/17] Starting Listener 5..." -ForegroundColor Cyan
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$scriptDir'; Write-Host '=== LISTENER 5 ===' -ForegroundColor Yellow; Write-Host 'Namespace: $namespace' -ForegroundColor Gray; Write-Host ''; cargo run --release --bin listener -- --bootstrap $bootstrap --namespace $namespace"
Start-Sleep -Milliseconds 500

Write-Host "[7/17] Starting Listener 6..." -ForegroundColor Cyan
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$scriptDir'; Write-Host '=== LISTENER 6 ===' -ForegroundColor Yellow; Write-Host 'Namespace: $namespace' -ForegroundColor Gray; Write-Host ''; cargo run --release --bin listener -- --bootstrap $bootstrap --namespace $namespace"
Start-Sleep -Milliseconds 500

Write-Host "[8/17] Starting Listener 7..." -ForegroundColor Cyan
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$scriptDir'; Write-Host '=== LISTENER 7 ===' -ForegroundColor Yellow; Write-Host 'Namespace: $namespace' -ForegroundColor Gray; Write-Host ''; cargo run --release --bin listener -- --bootstrap $bootstrap --namespace $namespace"
Start-Sleep -Milliseconds 500

Write-Host "[9/17] Starting Listener 8..." -ForegroundColor Cyan
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$scriptDir'; Write-Host '=== LISTENER 8 ===' -ForegroundColor Yellow; Write-Host 'Namespace: $namespace' -ForegroundColor Gray; Write-Host ''; cargo run --release --bin listener -- --bootstrap $bootstrap --namespace $namespace"
Start-Sleep -Seconds 2

# Start 8 dialers (they actively discover and connect)
Write-Host "[10/17] Starting Dialer 1..." -ForegroundColor Cyan
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$scriptDir'; Write-Host '=== DIALER 1 ===' -ForegroundColor Magenta; Write-Host 'Namespace: $namespace' -ForegroundColor Gray; Write-Host ''; cargo run --release --bin dialer -- --bootstrap $bootstrap --namespace $namespace"
Start-Sleep -Milliseconds 500

Write-Host "[11/17] Starting Dialer 2..." -ForegroundColor Cyan
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$scriptDir'; Write-Host '=== DIALER 2 ===' -ForegroundColor Magenta; Write-Host 'Namespace: $namespace' -ForegroundColor Gray; Write-Host ''; cargo run --release --bin dialer -- --bootstrap $bootstrap --namespace $namespace"
Start-Sleep -Milliseconds 500

Write-Host "[12/17] Starting Dialer 3..." -ForegroundColor Cyan
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$scriptDir'; Write-Host '=== DIALER 3 ===' -ForegroundColor Magenta; Write-Host 'Namespace: $namespace' -ForegroundColor Gray; Write-Host ''; cargo run --release --bin dialer -- --bootstrap $bootstrap --namespace $namespace"
Start-Sleep -Milliseconds 500

Write-Host "[13/17] Starting Dialer 4..." -ForegroundColor Cyan
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$scriptDir'; Write-Host '=== DIALER 4 ===' -ForegroundColor Magenta; Write-Host 'Namespace: $namespace' -ForegroundColor Gray; Write-Host ''; cargo run --release --bin dialer -- --bootstrap $bootstrap --namespace $namespace"
Start-Sleep -Milliseconds 500

Write-Host "[14/17] Starting Dialer 5..." -ForegroundColor Cyan
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$scriptDir'; Write-Host '=== DIALER 5 ===' -ForegroundColor Magenta; Write-Host 'Namespace: $namespace' -ForegroundColor Gray; Write-Host ''; cargo run --release --bin dialer -- --bootstrap $bootstrap --namespace $namespace"
Start-Sleep -Milliseconds 500

Write-Host "[15/17] Starting Dialer 6..." -ForegroundColor Cyan
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$scriptDir'; Write-Host '=== DIALER 6 ===' -ForegroundColor Magenta; Write-Host 'Namespace: $namespace' -ForegroundColor Gray; Write-Host ''; cargo run --release --bin dialer -- --bootstrap $bootstrap --namespace $namespace"
Start-Sleep -Milliseconds 500

Write-Host "[16/17] Starting Dialer 7..." -ForegroundColor Cyan
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$scriptDir'; Write-Host '=== DIALER 7 ===' -ForegroundColor Magenta; Write-Host 'Namespace: $namespace' -ForegroundColor Gray; Write-Host ''; cargo run --release --bin dialer -- --bootstrap $bootstrap --namespace $namespace"
Start-Sleep -Milliseconds 500

Write-Host "[17/17] Starting Dialer 8..." -ForegroundColor Cyan
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$scriptDir'; Write-Host '=== DIALER 8 ===' -ForegroundColor Magenta; Write-Host 'Namespace: $namespace' -ForegroundColor Gray; Write-Host ''; cargo run --release --bin dialer -- --bootstrap $bootstrap --namespace $namespace"
Start-Sleep -Seconds 2

Write-Host ""
Write-Host "=== Intensive 16-Node Network Started! ===" -ForegroundColor Green
Write-Host ""
Write-Host "Network Configuration:" -ForegroundColor White
Write-Host "  - 1 Monitor (Bootstrap + Dashboard)" -ForegroundColor Gray
Write-Host "  - 8 Listeners (Register in DHT)" -ForegroundColor Yellow
Write-Host "  - 8 Dialers (Discover and Connect)" -ForegroundColor Magenta
Write-Host "  - Total: 16 P2P Nodes + 1 Monitor" -ForegroundColor Cyan
Write-Host "  - Namespace: $namespace" -ForegroundColor Gray
Write-Host ""
Write-Host "🌐 Web Dashboard: http://localhost:8080" -ForegroundColor Cyan
Write-Host ""
Write-Host "Expected Behavior:" -ForegroundColor White
Write-Host "  - All 16 nodes connect to bootstrap (monitor)" -ForegroundColor Gray
Write-Host "  - Listeners register their peer info in DHT" -ForegroundColor Gray
Write-Host "  - Dialers query DHT and discover all listeners" -ForegroundColor Gray
Write-Host "  - Dialers connect to discovered listeners" -ForegroundColor Gray
Write-Host "  - Listeners also discover each other via DHT" -ForegroundColor Gray
Write-Host "  - Network forms a mesh of connections" -ForegroundColor Gray
Write-Host "  - Dashboard shows all nodes and connections" -ForegroundColor Gray
Write-Host ""
Write-Host "Discovery Timeline:" -ForegroundColor White
Write-Host "  0-5s:   Monitor starts, nodes begin connecting" -ForegroundColor Gray
Write-Host "  5-15s:  All nodes bootstrap to DHT" -ForegroundColor Gray
Write-Host "  15-30s: Listeners register in DHT" -ForegroundColor Gray
Write-Host "  30-60s: Dialers discover and connect to listeners" -ForegroundColor Gray
Write-Host "  60s+:   Full mesh network established" -ForegroundColor Gray
Write-Host ""
Write-Host "Opening dashboard in browser..." -ForegroundColor Yellow
Start-Sleep -Seconds 3
Start-Process "http://localhost:8080"

Write-Host ""
Write-Host "⚠️  WARNING: 17 PowerShell windows will open!" -ForegroundColor Yellow
Write-Host "   You may want to arrange them or minimize some." -ForegroundColor Gray
Write-Host ""
Write-Host "Press any key to exit (windows will remain open)..." -ForegroundColor White
$null = $Host.UI.RawUI.ReadKey("NoEcho,IncludeKeyDown")













