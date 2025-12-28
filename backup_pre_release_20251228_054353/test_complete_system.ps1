# Complete System Test - Runs everything in order and produces results

Write-Host ""
Write-Host "COMPLETE SYSTEM TEST - RED BUTTONS AND NODE REGISTRATION" -ForegroundColor Cyan
Write-Host "========================================================" -ForegroundColor Cyan
Write-Host ""

# Pre-check: Warn if processes are already running
$existing = Get-Process | Where-Object { 
    $_.ProcessName -match "bootstrap|web_server|shard_listener|server"
} -ErrorAction SilentlyContinue

if ($existing) {
    Write-Host "WARNING: Found $($existing.Count) existing process(es) that will be stopped:" -ForegroundColor Yellow
    $existing | ForEach-Object {
        Write-Host "  - $($_.ProcessName) (PID: $($_.Id))" -ForegroundColor Yellow
    }
    Write-Host ""
}

# Step 1: Clean up
Write-Host "[STEP 1/7] Cleaning up existing processes..." -ForegroundColor Yellow

# Kill processes more aggressively
$processes = Get-Process | Where-Object { 
    $_.ProcessName -match "bootstrap|web_server|shard_listener|server" -or
    $_.ProcessName -eq "cargo" -or
    $_.MainWindowTitle -like "*cargo*"
} -ErrorAction SilentlyContinue

if ($processes) {
    Write-Host "  Found $($processes.Count) process(es) to stop..." -ForegroundColor Gray
    $processes | ForEach-Object {
        try {
            Stop-Process -Id $_.Id -Force -ErrorAction Stop
            Write-Host "    Stopped: $($_.ProcessName) (PID: $($_.Id))" -ForegroundColor Gray
        } catch {
            Write-Host "    Failed to stop: $($_.ProcessName) (PID: $($_.Id))" -ForegroundColor Yellow
        }
    }
}

# Wait longer for file locks to be released
Write-Host "  Waiting for file locks to be released..." -ForegroundColor Gray
Start-Sleep -Seconds 5

# Verify processes are actually gone
$remaining = Get-Process | Where-Object { 
    $_.ProcessName -match "bootstrap|web_server|shard_listener|server"
} -ErrorAction SilentlyContinue

if ($remaining) {
    Write-Host "  Warning: $($remaining.Count) process(es) still running" -ForegroundColor Yellow
    $remaining | ForEach-Object {
        Write-Host "    Still running: $($_.ProcessName) (PID: $($_.Id))" -ForegroundColor Yellow
    }
    Write-Host "  Attempting force kill with taskkill..." -ForegroundColor Yellow
    $remaining | ForEach-Object {
        try {
            taskkill /F /PID $_.Id 2>&1 | Out-Null
        } catch {
            # Ignore errors
        }
    }
    Start-Sleep -Seconds 3
    
    # Final check
    $stillRunning = Get-Process | Where-Object { 
        $_.ProcessName -match "bootstrap|web_server|shard_listener|server"
    } -ErrorAction SilentlyContinue
    
    if ($stillRunning) {
        Write-Host "  ERROR: Cannot stop all processes. Please manually close:" -ForegroundColor Red
        $stillRunning | ForEach-Object {
            Write-Host "    $($_.ProcessName) (PID: $($_.Id))" -ForegroundColor Red
        }
        Write-Host "  Then run this script again." -ForegroundColor Red
        exit 1
    }
}

Write-Host "  All processes stopped" -ForegroundColor Green

# Step 2: Check shard files
Write-Host ""
Write-Host "[STEP 2/7] Checking shard files..." -ForegroundColor Yellow
$shardFiles = Get-ChildItem models_cache/shards/shard-[0-3].gguf -ErrorAction SilentlyContinue
if ($shardFiles.Count -eq 4) {
    Write-Host "  Found 4/4 shard files" -ForegroundColor Green
} else {
    Write-Host "  Missing shard files: $($shardFiles.Count)/4 found" -ForegroundColor Red
}

# Step 3: Start bootstrap
Write-Host ""
Write-Host "[STEP 3/7] Starting bootstrap server..." -ForegroundColor Yellow
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$PWD'; cargo run --bin server -- --listen-addr 0.0.0.0 --port 51820" -WindowStyle Minimized
Start-Sleep -Seconds 5

$bootstrapProcess = Get-Process | Where-Object {$_.ProcessName -eq "server"} -ErrorAction SilentlyContinue
if ($bootstrapProcess) {
    Write-Host "  Bootstrap server running (PID: $($bootstrapProcess.Id))" -ForegroundColor Green
} else {
    Write-Host "  Bootstrap server failed to start" -ForegroundColor Red
    exit 1
}

# Step 4: Start web server
Write-Host ""
Write-Host "[STEP 4/7] Starting web server..." -ForegroundColor Yellow
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$PWD'; cargo run --bin web_server" -WindowStyle Normal
Start-Sleep -Seconds 10

$webProcess = Get-Process | Where-Object {$_.ProcessName -eq "web_server"} -ErrorAction SilentlyContinue
if ($webProcess) {
    Write-Host "  Web server running (PID: $($webProcess.Id))" -ForegroundColor Green
} else {
    Write-Host "  Web server failed to start" -ForegroundColor Red
    exit 1
}

# Step 5: Wait for nodes
Write-Host ""
Write-Host "[STEP 5/7] Waiting for nodes to spawn..." -ForegroundColor Yellow
$maxWait = 30
$elapsed = 0
$nodeCount = 0
while ($elapsed -lt $maxWait) {
    Start-Sleep -Seconds 2
    $elapsed += 2
    $nodes = Get-Process | Where-Object {$_.ProcessName -eq "shard_listener"} -ErrorAction SilentlyContinue
    $currentCount = if ($nodes) { $nodes.Count } else { 0 }
    
    if ($currentCount -gt $nodeCount) {
        Write-Host "  $currentCount node(s) running..." -ForegroundColor Cyan
        $nodeCount = $currentCount
    }
    
    if ($nodeCount -eq 4) {
        Write-Host "  All 4 nodes spawned!" -ForegroundColor Green
        break
    }
    
    Write-Host "  Waiting... ($elapsed seconds, $currentCount/4 nodes)" -ForegroundColor Gray
}

# Step 6: Test web console
Write-Host ""
Write-Host "[STEP 6/7] Testing web console..." -ForegroundColor Yellow
try {
    $response = Invoke-WebRequest -Uri "http://localhost:8080" -TimeoutSec 5 -UseBasicParsing -ErrorAction Stop
    if ($response.StatusCode -eq 200) {
        Write-Host "  Web console accessible" -ForegroundColor Green
    }
} catch {
    Write-Host "  Web console not accessible" -ForegroundColor Red
}

# Step 7: Final results
Write-Host ""
Write-Host "[STEP 7/7] Final System Status:" -ForegroundColor Yellow
Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "TEST RESULTS" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

$bootstrap = Get-Process | Where-Object {$_.ProcessName -eq "server"} -ErrorAction SilentlyContinue
$web = Get-Process | Where-Object {$_.ProcessName -eq "web_server"} -ErrorAction SilentlyContinue
$nodes = Get-Process | Where-Object {$_.ProcessName -eq "shard_listener"} -ErrorAction SilentlyContinue

Write-Host "Processes:" -ForegroundColor Yellow
Write-Host "  Bootstrap: $(if ($bootstrap) { "RUNNING (PID: $($bootstrap.Id))" } else { "NOT RUNNING" })" -ForegroundColor $(if ($bootstrap) { 'Green' } else { 'Red' })
Write-Host "  Web Server: $(if ($web) { "RUNNING (PID: $($web.Id))" } else { "NOT RUNNING" })" -ForegroundColor $(if ($web) { 'Green' } else { 'Red' })
$nodeCount = if ($nodes) { $nodes.Count } else { 0 }
Write-Host "  Shard Nodes: $(if ($nodeCount -eq 4) { "$nodeCount/4 RUNNING" } elseif ($nodeCount -gt 0) { "$nodeCount/4 RUNNING" } else { "0/4 RUNNING" })" -ForegroundColor $(if ($nodeCount -eq 4) { 'Green' } elseif ($nodeCount -gt 0) { 'Yellow' } else { 'Red' })

Write-Host ""
Write-Host "Web Console:" -ForegroundColor Yellow
Write-Host "  URL: http://localhost:8080" -ForegroundColor Cyan
Write-Host "  WebSocket: ws://localhost:8081" -ForegroundColor Cyan

Write-Host ""
Write-Host "What to Check:" -ForegroundColor Yellow
Write-Host "  1. Open http://localhost:8080 in browser" -ForegroundColor White
Write-Host "  2. REFRESH the page (F5) to load new JavaScript" -ForegroundColor Red
Write-Host "  3. Scroll to Pipeline Status section" -ForegroundColor White
Write-Host "  4. Look for Shard 0, 1, 2, 3 buttons" -ForegroundColor White
Write-Host "  5. Watch for buttons turning RED" -ForegroundColor Red
Write-Host "  6. Check for node IDs below red buttons" -ForegroundColor Red
Write-Host "  7. Open browser console (F12) for debug messages" -ForegroundColor White

Write-Host ""
Write-Host "Expected Behavior:" -ForegroundColor Yellow
Write-Host "  - Buttons turn RED within 10-20 seconds" -ForegroundColor White
Write-Host "  - Each button shows node identifier" -ForegroundColor White
Write-Host "  - Red pulsing glow animation" -ForegroundColor White
Write-Host "  - Browser console shows node joined messages" -ForegroundColor Gray

Write-Host ""
Write-Host "Browser Console Check:" -ForegroundColor Yellow
Write-Host "  Press F12, then look for:" -ForegroundColor White
Write-Host "    [WS] Connected" -ForegroundColor Gray
Write-Host "    [WS] Received node event: node_joined" -ForegroundColor Gray
Write-Host "    [WS] Node joined - Shard X button turned red" -ForegroundColor Gray

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "Test Complete - Check the web console!" -ForegroundColor Green
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
