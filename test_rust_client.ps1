# Test Rust AI Query Client
# This script waits for the web server to be ready, then runs the Rust client

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  TESTING RUST AI QUERY CLIENT" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# Get query from command line or use default
$query = if ($args.Count -gt 0) { $args[0] } else { "What is artificial intelligence?" }

Write-Host "Query: '$query'" -ForegroundColor Yellow
Write-Host ""

# Check if web server is running
Write-Host "[1/3] Checking if web server is running..." -ForegroundColor Yellow
$webServer = Get-Process | Where-Object {$_.ProcessName -eq "web_server"} -ErrorAction SilentlyContinue

if (-not $webServer) {
    Write-Host "  [WARN] Web server not running!" -ForegroundColor Red
    Write-Host ""
    Write-Host "To start the web server, run:" -ForegroundColor Yellow
    Write-Host "  powershell -ExecutionPolicy Bypass -File start_web_server.ps1" -ForegroundColor White
    Write-Host ""
    Write-Host "Or manually:" -ForegroundColor Yellow
    Write-Host "  1. Start bootstrap: cargo run --bin server -- --listen-addr 0.0.0.0 --port 51820" -ForegroundColor White
    Write-Host "  2. Start web server: `$env:BOOTSTRAP='/ip4/127.0.0.1/tcp/51820'; cargo run --bin web_server" -ForegroundColor White
    Write-Host ""
    exit 1
}

Write-Host "  [OK] Web server is running (PID: $($webServer.Id))" -ForegroundColor Green

# Check if WebSocket port is listening
Write-Host ""
Write-Host "[2/3] Checking WebSocket port (8081)..." -ForegroundColor Yellow
$portCheck = netstat -ano | findstr ":8081" | findstr "LISTENING"
if (-not $portCheck) {
    Write-Host "  [WARN] Port 8081 not listening yet. Web server may still be starting..." -ForegroundColor Yellow
    Write-Host "  Waiting 5 seconds and checking again..." -ForegroundColor Gray
    Start-Sleep -Seconds 5
    $portCheck = netstat -ano | findstr ":8081" | findstr "LISTENING"
    if (-not $portCheck) {
        Write-Host "  [ERROR] Port 8081 still not listening. Check web server logs." -ForegroundColor Red
        exit 1
    }
}
Write-Host "  [OK] Port 8081 is listening" -ForegroundColor Green

# Run the Rust client
Write-Host ""
Write-Host "[3/3] Running Rust AI query client..." -ForegroundColor Yellow
Write-Host ""

cargo run --example ai_query_client -- "$query"

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  TEST COMPLETE" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
