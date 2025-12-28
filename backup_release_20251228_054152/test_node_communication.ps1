# Test Node Communication - Start, Monitor, and Recover
Write-Host "=== NODE COMMUNICATION TEST ===" -ForegroundColor Cyan
Write-Host ""

# Cleanup
Write-Host "[1] Cleaning up..." -ForegroundColor Yellow
Get-Process | Where-Object {$_.ProcessName -like "*cargo*" -or $_.ProcessName -like "*rustc*"} | Stop-Process -Force -ErrorAction SilentlyContinue
Start-Sleep -Seconds 2

# Build
Write-Host "[2] Building..." -ForegroundColor Yellow
cargo build 2>&1 | Out-Null
if ($LASTEXITCODE -ne 0) {
    Write-Host "Build failed!" -ForegroundColor Red
    exit 1
}
Write-Host "Build complete" -ForegroundColor Green

# Start bootstrap
Write-Host ""
Write-Host "[3] Starting bootstrap server..." -ForegroundColor Yellow
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$PWD'; cargo run --bin node -- bootstrap --listen-addr 0.0.0.0 --port 51820" -WindowStyle Minimized
Start-Sleep -Seconds 5

# Start web server
Write-Host "[4] Starting web server..." -ForegroundColor Yellow
$env:BOOTSTRAP = "/ip4/127.0.0.1/tcp/51820"
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$PWD'; `$env:BOOTSTRAP='/ip4/127.0.0.1/tcp/51820'; cargo run --bin node -- web-server --bootstrap /ip4/127.0.0.1/tcp/51820" -WindowStyle Normal
Start-Sleep -Seconds 10

# Start shard listener
Write-Host "[5] Starting shard_listener..." -ForegroundColor Yellow
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$PWD'; cargo run --bin node -- shard-listener --bootstrap /ip4/127.0.0.1/tcp/51820 --cluster llama-cluster --shard-id 0 --total-shards 4" -WindowStyle Normal
Start-Sleep -Seconds 10

# Monitor
Write-Host ""
Write-Host "[6] Monitoring (checking every 3 seconds)..." -ForegroundColor Yellow
Write-Host ""

$maxChecks = 20
$check = 0
$webReady = $false

while ($check -lt $maxChecks) {
    $check++
    
    # Test web server
    try {
        $response = Invoke-WebRequest -Uri "http://localhost:8080" -TimeoutSec 2 -UseBasicParsing -ErrorAction Stop
        if (-not $webReady) {
            Write-Host "[$check/$maxChecks] Web server is responding!" -ForegroundColor Green
            $webReady = $true
        }
    } catch {
        Write-Host "[$check/$maxChecks] Web server not ready yet..." -ForegroundColor Yellow
    }
    
    if ($webReady) {
        Write-Host ""
        Write-Host "=== SUCCESS ===" -ForegroundColor Green
        Write-Host "Web server: http://localhost:8080" -ForegroundColor Cyan
        Write-Host "WebSocket: ws://localhost:8081" -ForegroundColor Cyan
        Write-Host ""
        Write-Host "Check the PowerShell windows for:" -ForegroundColor Yellow
        Write-Host "  - Shard listener: Look for 'ANNOUNCED SHARD 0 TO DHT'" -ForegroundColor White
        Write-Host "  - Web server: Look for 'Processed shard 0'" -ForegroundColor White
        Write-Host ""
        Write-Host "Open http://localhost:8080 in your browser!" -ForegroundColor Cyan
        Write-Host ""
        Write-Host "Monitoring will continue. Press Ctrl+C to stop." -ForegroundColor Yellow
        
        # Keep checking
        while ($true) {
            Start-Sleep -Seconds 5
            try {
                $null = Invoke-WebRequest -Uri "http://localhost:8080" -TimeoutSec 2 -UseBasicParsing -ErrorAction Stop
            } catch {
                Write-Host "Web server stopped responding!" -ForegroundColor Red
            }
        }
    }
    
    Start-Sleep -Seconds 3
}

Write-Host ""
Write-Host "System may not be fully ready. Check the PowerShell windows manually." -ForegroundColor Yellow
