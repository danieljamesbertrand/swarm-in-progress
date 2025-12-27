# Comprehensive System Restart and Test Script
# Stops everything, restarts cleanly, and runs thorough tests

Write-Host "=== PROMETHOS-AI SYSTEM RESTART & TEST ===" -ForegroundColor Cyan
Write-Host ""

# Step 1: Stop all processes
Write-Host "[1/8] Stopping all processes..." -ForegroundColor Yellow
Get-Process | Where-Object {
    $_.ProcessName -eq "web_server" -or 
    $_.ProcessName -eq "shard_listener" -or
    ($_.ProcessName -like "*server*" -and $_.MainWindowTitle -like "*Bootstrap*")
} | Stop-Process -Force -ErrorAction SilentlyContinue
Start-Sleep -Seconds 2
Write-Host "  ✓ All processes stopped" -ForegroundColor Green
Write-Host ""

# Step 2: Start bootstrap server
Write-Host "[2/8] Starting bootstrap server..." -ForegroundColor Yellow
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$PWD'; Write-Host '=== BOOTSTRAP SERVER ===' -ForegroundColor Cyan; cargo run --bin server -- --listen-addr 0.0.0.0 --port 51820" -WindowStyle Normal
Start-Sleep -Seconds 3
Write-Host "  ✓ Bootstrap server starting" -ForegroundColor Green
Write-Host ""

# Step 3: Start web server
Write-Host "[3/8] Starting web server..." -ForegroundColor Yellow
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$PWD'; Write-Host '=== WEB SERVER ===' -ForegroundColor Green; cargo run --bin web_server" -WindowStyle Normal
Start-Sleep -Seconds 5
Write-Host "  ✓ Web server starting" -ForegroundColor Green
Write-Host ""

# Step 4: Wait for nodes to spawn
Write-Host "[4/8] Waiting for nodes to spawn (may take 30-90 seconds for first compile)..." -ForegroundColor Yellow
$maxWait = 120
$elapsed = 0
$nodeCount = 0
while ($elapsed -lt $maxWait) {
    $nodes = Get-Process | Where-Object {$_.ProcessName -eq "shard_listener"}
    $nodeCount = ($nodes | Measure-Object).Count
    $cargo = Get-Process | Where-Object {$_.ProcessName -eq "cargo"}
    $cargoCount = ($cargo | Measure-Object).Count
    
    $status = if ($nodeCount -eq 4) { "✓ COMPLETE" } elseif ($cargoCount -gt 0) { "Compiling..." } else { "Waiting..." }
    Write-Host "  [$elapsed s] Nodes: $nodeCount/4 | Cargo: $cargoCount | $status" -ForegroundColor $(if($nodeCount -eq 4){'Green'}elseif($cargoCount -gt 0){'Yellow'}else{'Gray'})
    
    if ($nodeCount -eq 4) {
        Write-Host "  ✓ All 4 nodes are running!" -ForegroundColor Green
        break
    }
    
    Start-Sleep -Seconds 3
    $elapsed += 3
}

if ($nodeCount -lt 4) {
    Write-Host "  ⚠️  Only $nodeCount/4 nodes after $maxWait seconds" -ForegroundColor Yellow
}
Write-Host ""

# Step 5: Verify DHT discovery
Write-Host "[5/8] Verifying DHT discovery..." -ForegroundColor Yellow
Write-Host "  Check web server console for '[DHT] ✓ Discovered shard' messages" -ForegroundColor Gray
Start-Sleep -Seconds 10
Write-Host "  ✓ Discovery check complete" -ForegroundColor Green
Write-Host ""

# Step 6: Check web interface
Write-Host "[6/8] Opening web interface..." -ForegroundColor Yellow
Start-Process "http://localhost:8080"
Start-Sleep -Seconds 2
Write-Host "  ✓ Web interface opened in browser" -ForegroundColor Green
Write-Host ""

# Step 7: System status check
Write-Host "[7/8] Final system status:" -ForegroundColor Yellow
$webServer = Get-Process | Where-Object {$_.ProcessName -eq "web_server"} | Select-Object -First 1
$bootstrap = Get-Process | Where-Object {$_.ProcessName -like "*server*" -and $_.Id -ne $webServer.Id} | Select-Object -First 1
$nodes = Get-Process | Where-Object {$_.ProcessName -eq "shard_listener"}
$nodeCount = ($nodes | Measure-Object).Count

Write-Host "  Bootstrap Server: $(if($bootstrap){'✓ Running (PID: ' + $bootstrap.Id + ')'}else{'✗ Not Running'})" -ForegroundColor $(if($bootstrap){'Green'}else{'Red'})
Write-Host "  Web Server:       $(if($webServer){'✓ Running (PID: ' + $webServer.Id + ')'}else{'✗ Not Running'})" -ForegroundColor $(if($webServer){'Green'}else{'Red'})
Write-Host "  Shard Nodes:      $nodeCount/4" -ForegroundColor $(if($nodeCount -eq 4){'Green'}elseif($nodeCount -gt 0){'Yellow'}else{'Red'})
if ($nodeCount -gt 0) {
    Write-Host "  Node PIDs:        $($nodes.Id -join ', ')" -ForegroundColor Gray
}
Write-Host ""

# Step 8: Test instructions
Write-Host "[8/8] Testing Instructions:" -ForegroundColor Yellow
Write-Host ""
Write-Host "  ✓ System Status:" -ForegroundColor Cyan
Write-Host "    - Bootstrap: http://127.0.0.1:51820" -ForegroundColor White
Write-Host "    - Web UI:    http://localhost:8080" -ForegroundColor White
Write-Host "    - WebSocket: ws://localhost:8081" -ForegroundColor White
Write-Host ""
Write-Host "  ✓ Test Coordinated Shard Assignment:" -ForegroundColor Cyan
Write-Host "    1. Check web server console for '[COORDINATOR] Last assigned shard: X'" -ForegroundColor White
Write-Host "    2. Verify nodes are assigned sequentially (0, 1, 2, 3)" -ForegroundColor White
Write-Host "    3. Check for '[COORDINATOR] Coordinated assignment' messages" -ForegroundColor White
Write-Host ""
Write-Host "  ✓ Test Web Interface:" -ForegroundColor Cyan
Write-Host "    1. Verify 'Nodes Online' shows 4/4" -ForegroundColor White
Write-Host "    2. Check connection status is 'Connected' (green)" -ForegroundColor White
Write-Host "    3. Verify all pipeline stages show as ready" -ForegroundColor White
Write-Host ""
Write-Host "  ✓ Test Inference:" -ForegroundColor Cyan
Write-Host "    1. Enter query: 'What is 2+2?'" -ForegroundColor White
Write-Host "    2. Watch pipeline stages light up in sequence" -ForegroundColor White
Write-Host "    3. Verify real-time updates in browser console (F12)" -ForegroundColor White
Write-Host "    4. Check response appears in output area" -ForegroundColor White
Write-Host ""
Write-Host "  ✓ Test Real-Time Updates:" -ForegroundColor Cyan
Write-Host "    1. Open browser console (F12)" -ForegroundColor White
Write-Host "    2. Look for '[WS] Stage update' messages" -ForegroundColor White
Write-Host "    3. Verify stages change: input → discovery → shard0 → shard1 → shard2 → shard3 → output" -ForegroundColor White
Write-Host ""
Write-Host "=== RESTART COMPLETE ===" -ForegroundColor Green
Write-Host ""

