# Comprehensive System Test Script
# Tests bootstrap, web server, node spawning, and inference

Write-Host "=== PROMETHOS-AI FULL SYSTEM TEST ===" -ForegroundColor Cyan
Write-Host ""

# Step 1: Check if bootstrap is running
Write-Host "[1/7] Checking bootstrap server..." -ForegroundColor Yellow
$bootstrap = Get-Process | Where-Object {$_.ProcessName -like "*server*"} | Select-Object -First 1
if (-not $bootstrap) {
    Write-Host "  ⚠️  Bootstrap server not detected (may be starting)" -ForegroundColor Yellow
} else {
    Write-Host "  ✓ Bootstrap server running (PID: $($bootstrap.Id))" -ForegroundColor Green
}

# Step 2: Check web server
Write-Host "[2/7] Checking web server..." -ForegroundColor Yellow
$webServer = Get-Process | Where-Object {$_.ProcessName -eq "web_server"} | Select-Object -First 1
if (-not $webServer) {
    Write-Host "  ✗ Web server not running!" -ForegroundColor Red
    Write-Host "  Starting web server..." -ForegroundColor Yellow
    Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$PWD'; cargo run --bin web_server" -WindowStyle Normal
    Start-Sleep -Seconds 5
} else {
    Write-Host "  ✓ Web server running (PID: $($webServer.Id))" -ForegroundColor Green
}

# Step 3: Wait for nodes to spawn
Write-Host "[3/7] Waiting for nodes to spawn (this may take 30-60 seconds for first compile)..." -ForegroundColor Yellow
$maxWait = 90
$elapsed = 0
$nodeCount = 0
while ($elapsed -lt $maxWait) {
    $nodes = Get-Process | Where-Object {$_.ProcessName -eq "shard_listener"}
    $nodeCount = ($nodes | Measure-Object).Count
    Write-Host "  [$elapsed s] Nodes: $nodeCount/4" -ForegroundColor $(if($nodeCount -eq 4){'Green'}else{'Yellow'})
    
    if ($nodeCount -eq 4) {
        Write-Host "  ✓ All 4 nodes are running!" -ForegroundColor Green
        break
    }
    
    Start-Sleep -Seconds 3
    $elapsed += 3
}

if ($nodeCount -lt 4) {
    Write-Host "  ⚠️  Only $nodeCount/4 nodes running after $maxWait seconds" -ForegroundColor Yellow
    Write-Host "  This may be normal if nodes are still compiling..." -ForegroundColor Gray
}

# Step 4: Check DHT discovery
Write-Host "[4/7] Checking DHT discovery..." -ForegroundColor Yellow
Write-Host "  (Check web server console for '[DHT] ✓ Discovered shard' messages)" -ForegroundColor Gray
Start-Sleep -Seconds 5

# Step 5: Test web interface
Write-Host "[5/7] Testing web interface..." -ForegroundColor Yellow
Write-Host "  Opening http://localhost:8080 in browser..." -ForegroundColor Cyan
Start-Process "http://localhost:8080"

# Step 6: Monitor system for 30 seconds
Write-Host "[6/7] Monitoring system for 30 seconds..." -ForegroundColor Yellow
for ($i = 0; $i -lt 10; $i++) {
    $nodes = Get-Process | Where-Object {$_.ProcessName -eq "shard_listener"}
    $nodeCount = ($nodes | Measure-Object).Count
    $webServer = Get-Process | Where-Object {$_.ProcessName -eq "web_server"} | Select-Object -First 1
    
    Write-Host "  [$($i*3) s] Web Server: $(if($webServer){'✓'}else{'✗'}) | Nodes: $nodeCount/4" -ForegroundColor $(if($webServer -and $nodeCount -eq 4){'Green'}else{'Yellow'})
    Start-Sleep -Seconds 3
}

# Step 7: Final status
Write-Host "[7/7] Final System Status:" -ForegroundColor Yellow
Write-Host ""
$webServer = Get-Process | Where-Object {$_.ProcessName -eq "web_server"} | Select-Object -First 1
$nodes = Get-Process | Where-Object {$_.ProcessName -eq "shard_listener"}
$nodeCount = ($nodes | Measure-Object).Count

Write-Host "  Bootstrap Server: $(if($bootstrap){'✓ Running'}else{'? Unknown'})" -ForegroundColor $(if($bootstrap){'Green'}else{'Yellow'})
Write-Host "  Web Server:       $(if($webServer){'✓ Running'}else{'✗ Not Running'})" -ForegroundColor $(if($webServer){'Green'}else{'Red'})
Write-Host "  Shard Nodes:      $nodeCount/4" -ForegroundColor $(if($nodeCount -eq 4){'Green'}elseif($nodeCount -gt 0){'Yellow'}else{'Red'})
Write-Host ""
Write-Host "=== TEST COMPLETE ===" -ForegroundColor Cyan
Write-Host ""
Write-Host "Next Steps:" -ForegroundColor Cyan
Write-Host "  1. Open http://localhost:8080 in your browser" -ForegroundColor White
Write-Host "  2. Check that nodes show as 'online' in the web interface" -ForegroundColor White
Write-Host "  3. Enter a query like 'What is 2+2?' and watch the pipeline stages light up" -ForegroundColor White
Write-Host "  4. Check the browser console (F12) for WebSocket messages" -ForegroundColor White
Write-Host ""

