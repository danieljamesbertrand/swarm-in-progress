# Test Inference in Cursor Browser
# This script will help you test inference using Cursor's browser

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  INFERENCE TEST - CURSOR BROWSER" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# Check if web server is running
$webServer = Get-Process | Where-Object {$_.ProcessName -eq "web_server"} -ErrorAction SilentlyContinue
if (-not $webServer) {
    Write-Host "[ERROR] Web server is not running!" -ForegroundColor Red
    Write-Host "Starting web server..." -ForegroundColor Yellow
    $env:BOOTSTRAP = "/ip4/127.0.0.1/tcp/51820"
    Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$PWD'; `$env:BOOTSTRAP='/ip4/127.0.0.1/tcp/51820'; cargo run --bin web_server" -WindowStyle Normal
    Write-Host "Waiting 10 seconds for web server to start..." -ForegroundColor Yellow
    Start-Sleep -Seconds 10
} else {
    Write-Host "[OK] Web server running (PID: $($webServer.Id))" -ForegroundColor Green
}

# Check if shard node is running
$node = Get-Process | Where-Object {$_.ProcessName -eq "shard_listener"} -ErrorAction SilentlyContinue
if (-not $node) {
    Write-Host "[WARN] Shard node not running" -ForegroundColor Yellow
    Write-Host "Starting shard node..." -ForegroundColor Yellow
    $env:LLAMA_SHARD_ID = "0"
    Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$PWD'; `$env:LLAMA_SHARD_ID='0'; `$env:LLAMA_TOTAL_SHARDS='4'; cargo run --bin shard_listener -- --bootstrap /ip4/127.0.0.1/tcp/51820 --cluster llama-cluster --shard-id 0 --total-shards 4 --total-layers 32 --model-name llama-8b --port 51821 --shards-dir models_cache/shards" -WindowStyle Normal
    Write-Host "Waiting 8 seconds for shard node to start..." -ForegroundColor Yellow
    Start-Sleep -Seconds 8
} else {
    Write-Host "[OK] Shard node running (PID: $($node.Id))" -ForegroundColor Green
}

# Test HTTP endpoint
Write-Host ""
Write-Host "Testing HTTP endpoint..." -ForegroundColor Yellow
try {
    $response = Invoke-WebRequest -Uri "http://localhost:8080" -TimeoutSec 5 -UseBasicParsing -ErrorAction Stop
    Write-Host "[OK] HTTP server responding (Status: $($response.StatusCode))" -ForegroundColor Green
} catch {
    Write-Host "[ERROR] HTTP server not accessible: $_" -ForegroundColor Red
    Write-Host ""
    Write-Host "Please wait a few more seconds and try again" -ForegroundColor Yellow
    exit 1
}

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  READY FOR INFERENCE TEST" -ForegroundColor Green
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "To use Cursor's browser:" -ForegroundColor Yellow
Write-Host "  1. In Cursor, press Ctrl+Shift+P (or Cmd+Shift+P on Mac)" -ForegroundColor White
Write-Host "  2. Type 'Simple Browser' or 'Open Preview'" -ForegroundColor White
Write-Host "  3. Enter URL: http://localhost:8080" -ForegroundColor Cyan
Write-Host ""
Write-Host "OR use the command:" -ForegroundColor Yellow
Write-Host "  Start-Process 'http://localhost:8080'" -ForegroundColor White
Write-Host ""

# Try to open in default browser as fallback
Write-Host "Opening in default browser..." -ForegroundColor Yellow
Start-Process "http://localhost:8080"

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  INFERENCE TEST STEPS" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "1. Wait 10-15 seconds for the shard node to register" -ForegroundColor White
Write-Host "2. In the browser, type this query:" -ForegroundColor White
Write-Host "   'what do a cat and a snake have in common'" -ForegroundColor Cyan
Write-Host "3. Click Send or press Enter" -ForegroundColor White
Write-Host "4. Watch for the response to appear below" -ForegroundColor White
Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  WHAT TO WATCH FOR" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "In the browser:" -ForegroundColor Yellow
Write-Host "  - Pipeline status showing 1/4 nodes online" -ForegroundColor Gray
Write-Host "  - Query input field at the top" -ForegroundColor Gray
Write-Host "  - Response area below where results will appear" -ForegroundColor Gray
Write-Host ""
Write-Host "In terminal windows:" -ForegroundColor Yellow
Write-Host "  - [P2P] [OK] Matched response to waiting channel" -ForegroundColor Green
Write-Host "  - [RESPONSE] [OK] Response sent successfully" -ForegroundColor Green
Write-Host "  - [INFERENCE] [OK] Shard 0 completed" -ForegroundColor Green
Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

