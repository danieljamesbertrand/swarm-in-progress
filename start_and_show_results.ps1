# Start system and show how to see inference results
Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  STARTING SYSTEM FOR INFERENCE TEST" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# Check if bootstrap is running
$bootstrap = Get-Process | Where-Object {$_.ProcessName -eq "server"} -ErrorAction SilentlyContinue
if (-not $bootstrap) {
    Write-Host "[1/4] Starting bootstrap server..." -ForegroundColor Yellow
    Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$PWD'; cargo run --bin server -- --listen-addr 0.0.0.0 --port 51820" -WindowStyle Minimized
    Start-Sleep -Seconds 5
    Write-Host "  [OK] Bootstrap started" -ForegroundColor Green
} else {
    Write-Host "[1/4] Bootstrap already running (PID: $($bootstrap.Id))" -ForegroundColor Green
}

# Check if shard node is running
$node = Get-Process | Where-Object {$_.ProcessName -eq "shard_listener"} -ErrorAction SilentlyContinue
if (-not $node) {
    Write-Host "[2/4] Starting shard node (shard 0)..." -ForegroundColor Yellow
    $env:LLAMA_SHARD_ID = "0"
    Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$PWD'; `$env:LLAMA_SHARD_ID='0'; `$env:LLAMA_TOTAL_SHARDS='4'; cargo run --bin shard_listener -- --bootstrap /ip4/127.0.0.1/tcp/51820 --cluster llama-cluster --shard-id 0 --total-shards 4 --total-layers 32 --model-name llama-8b --port 51821 --shards-dir models_cache/shards" -WindowStyle Normal
    Start-Sleep -Seconds 8
    Write-Host "  [OK] Shard node started" -ForegroundColor Green
} else {
    Write-Host "[2/4] Shard node already running (PID: $($node.Id))" -ForegroundColor Green
}

# Check if web server is running
$webServer = Get-Process | Where-Object {$_.ProcessName -eq "web_server"} -ErrorAction SilentlyContinue
if (-not $webServer) {
    Write-Host "[3/4] Starting web server..." -ForegroundColor Yellow
    $env:BOOTSTRAP = "/ip4/127.0.0.1/tcp/51820"
    Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$PWD'; `$env:BOOTSTRAP='/ip4/127.0.0.1/tcp/51820'; cargo run --bin web_server" -WindowStyle Normal
    Start-Sleep -Seconds 10
    Write-Host "  [OK] Web server started" -ForegroundColor Green
} else {
    Write-Host "[3/4] Web server already running (PID: $($webServer.Id))" -ForegroundColor Green
}

Write-Host ""
Write-Host "[4/4] System Status" -ForegroundColor Yellow
Write-Host "  Bootstrap: $(if (Get-Process | Where-Object {$_.ProcessName -eq "server"}) { '[OK]' } else { '[ERROR]' })" -ForegroundColor $(if (Get-Process | Where-Object {$_.ProcessName -eq "server"}) { 'Green' } else { 'Red' })
Write-Host "  Shard Node: $(if (Get-Process | Where-Object {$_.ProcessName -eq "shard_listener"}) { '[OK]' } else { '[ERROR]' })" -ForegroundColor $(if (Get-Process | Where-Object {$_.ProcessName -eq "shard_listener"}) { 'Green' } else { 'Red' })
Write-Host "  Web Server: $(if (Get-Process | Where-Object {$_.ProcessName -eq "web_server"}) { '[OK]' } else { '[ERROR]' })" -ForegroundColor $(if (Get-Process | Where-Object {$_.ProcessName -eq "web_server"}) { 'Green' } else { 'Red' })

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  HOW TO SEE INFERENCE RESULTS" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "STEP 1: Open Browser" -ForegroundColor Yellow
Write-Host "  URL: http://localhost:8080" -ForegroundColor White
Write-Host ""
Write-Host "STEP 2: Wait 10-15 seconds" -ForegroundColor Yellow
Write-Host "  Wait for the shard node to register with the pipeline" -ForegroundColor Gray
Write-Host ""
Write-Host "STEP 3: Submit Query" -ForegroundColor Yellow
Write-Host "  Type: what do a cat and a snake have in common" -ForegroundColor Cyan
Write-Host "  Click Send or press Enter" -ForegroundColor White
Write-Host ""
Write-Host "STEP 4: View Results" -ForegroundColor Yellow
Write-Host "  Results will appear in the response area below the input" -ForegroundColor White
Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  TERMINAL OUTPUT TO WATCH FOR" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "In Web Server Terminal:" -ForegroundColor Yellow
Write-Host "  [WS] Processing query: what do a cat and a snake have in common" -ForegroundColor Gray
Write-Host "  [INFERENCE] Submitting inference request..." -ForegroundColor Gray
Write-Host "  [P2P] Sending command EXECUTE_TASK to node ..." -ForegroundColor Gray
Write-Host "  [P2P] [OK] Matched response to waiting channel" -ForegroundColor Green
Write-Host "  [INFERENCE] [OK] Shard 0 completed" -ForegroundColor Green
Write-Host ""
Write-Host "In Shard Node Terminal:" -ForegroundColor Yellow
Write-Host "  [COMMAND] [OK] Validation passed" -ForegroundColor Gray
Write-Host "  [INFERENCE] Processing inference request..." -ForegroundColor Gray
Write-Host "  [RESPONSE] [OK] Response sent successfully" -ForegroundColor Green
Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  SUCCESS INDICATORS" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "[OK] [P2P] Matched response to waiting channel" -ForegroundColor Green
Write-Host "  Confirms RequestId matching fix is working" -ForegroundColor Gray
Write-Host ""
Write-Host "[OK] [RESPONSE] Response sent successfully" -ForegroundColor Green
Write-Host "  Confirms shard node processed the inference" -ForegroundColor Gray
Write-Host ""
Write-Host "[OK] [INFERENCE] Shard 0 completed" -ForegroundColor Green
Write-Host "  Confirms coordinator received the response" -ForegroundColor Gray
Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

