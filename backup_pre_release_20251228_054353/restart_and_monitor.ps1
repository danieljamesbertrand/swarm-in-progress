# Restart System and Monitor Web Server
# Kills all processes, restarts, and monitors for errors

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  RESTARTING SYSTEM WITH MONITORING" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# Step 1: Kill all processes
Write-Host "[1/6] Killing all existing processes..." -ForegroundColor Yellow
$processes = Get-Process | Where-Object { 
    $_.ProcessName -match "bootstrap|web_server|shard_listener|server|node" -or
    $_.ProcessName -eq "cargo" -or
    $_.ProcessName -eq "rustc"
} -ErrorAction SilentlyContinue

if ($processes) {
    Write-Host "  Found $($processes.Count) process(es) to kill" -ForegroundColor Gray
    $processes | ForEach-Object {
        try {
            Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue
            Write-Host "    Killed: $($_.ProcessName) (PID: $($_.Id))" -ForegroundColor Gray
        } catch {
            Write-Host "    Failed to kill: $($_.ProcessName) (PID: $($_.Id))" -ForegroundColor Yellow
        }
    }
    Start-Sleep -Seconds 3
    Write-Host "  [OK] All processes killed" -ForegroundColor Green
} else {
    Write-Host "  [OK] No processes to kill" -ForegroundColor Green
}

# Step 2: Check shard files
Write-Host ""
Write-Host "[2/6] Checking shard files..." -ForegroundColor Yellow
$shardFiles = @()
for ($i = 0; $i -lt 4; $i++) {
    $shardFile = "models_cache/shards/shard-$i.gguf"
    if (Test-Path $shardFile) {
        $size = (Get-Item $shardFile).Length / 1MB
        $sizeRounded = [math]::Round($size, 2)
        Write-Host "  [OK] Shard ${i}: ${sizeRounded} MB" -ForegroundColor Green
        $shardFiles += $i
    } else {
        Write-Host "  [WARN] Shard ${i}: NOT FOUND" -ForegroundColor Yellow
    }
}
if ($shardFiles.Count -eq 0) {
    Write-Host "  [ERROR] No shard files found!" -ForegroundColor Red
    Write-Host "  Nodes will need to download shards via torrent" -ForegroundColor Yellow
}

# Step 3: Start bootstrap server
Write-Host ""
Write-Host "[3/6] Starting bootstrap server..." -ForegroundColor Yellow
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$PWD'; Write-Host '=== BOOTSTRAP SERVER ===' -ForegroundColor Cyan; cargo run --bin server -- --listen-addr 0.0.0.0 --port 51820" -WindowStyle Minimized
Start-Sleep -Seconds 5

$bootstrap = Get-Process | Where-Object {$_.ProcessName -eq "server"} -ErrorAction SilentlyContinue
if ($bootstrap) {
    Write-Host "  [OK] Bootstrap running (PID: $($bootstrap.Id))" -ForegroundColor Green
} else {
    Write-Host "  [ERROR] Bootstrap failed to start" -ForegroundColor Red
    exit 1
}

# Step 4: Start web server with monitoring
Write-Host ""
Write-Host "[4/6] Starting web server with monitoring..." -ForegroundColor Yellow
$env:BOOTSTRAP = "/ip4/127.0.0.1/tcp/51820"

# Create log file for web server
$webServerLog = "web_server_monitor.log"
Write-Host "  Log file: $webServerLog" -ForegroundColor Gray

# Start web server and capture output
$webServerJob = Start-Job -ScriptBlock {
    param($workDir, $bootstrap)
    Set-Location $workDir
    $env:BOOTSTRAP = $bootstrap
    cargo run --bin web_server 2>&1 | Tee-Object -FilePath "web_server_monitor.log"
} -ArgumentList $PWD, $env:BOOTSTRAP

# Also start in visible window for real-time monitoring
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$PWD'; Write-Host '=== WEB SERVER (MONITOR THIS WINDOW) ===' -ForegroundColor Cyan; Write-Host 'Watch for errors and startup messages' -ForegroundColor Yellow; Write-Host ''; `$env:BOOTSTRAP='/ip4/127.0.0.1/tcp/51820'; cargo run --bin web_server" -WindowStyle Normal

Start-Sleep -Seconds 10

$webServer = Get-Process | Where-Object {$_.ProcessName -eq "web_server"} -ErrorAction SilentlyContinue
if ($webServer) {
    Write-Host "  [OK] Web server process started (PID: $($webServer.Id))" -ForegroundColor Green
} else {
    Write-Host "  [WARN] Web server process not found yet (may still be compiling)" -ForegroundColor Yellow
}

# Step 5: Start all 4 shard nodes
Write-Host ""
Write-Host "[5/6] Starting all 4 shard nodes..." -ForegroundColor Yellow
Write-Host "  Nodes will automatically load their assigned shard files on startup" -ForegroundColor Gray
Write-Host "  Look for: [SHARD] SHARD X LOADED BEFORE JOINING NETWORK" -ForegroundColor Gray
Write-Host ""

$bootstrapAddr = "/ip4/127.0.0.1/tcp/51820"
$cluster = "llama-cluster"
$totalShards = 4
$totalLayers = 32
$modelName = "llama-8b"
$shardsDir = "models_cache/shards"

for ($i = 0; $i -lt 4; $i++) {
    $port = 51821 + $i
    $shardFile = "models_cache/shards/shard-$i.gguf"
    $shardExists = Test-Path $shardFile
    
    if ($shardExists) {
        $size = [math]::Round((Get-Item $shardFile).Length / 1MB, 2)
        Write-Host "  Starting shard $i on port $port (shard file: ${size} MB)" -ForegroundColor Green
    } else {
        Write-Host "  Starting shard $i on port $port (shard file: NOT FOUND - will download)" -ForegroundColor Yellow
    }
    
    $command = "cd '$PWD'; Write-Host '=== SHARD NODE $i ===' -ForegroundColor Cyan; Write-Host 'Watch for: [SHARD] SHARD $i LOADED' -ForegroundColor Yellow; Write-Host 'Watch for: [DHT] ANNOUNCED SHARD $i TO DHT' -ForegroundColor Yellow; Write-Host ''; `$env:LLAMA_SHARD_ID='$i'; `$env:LLAMA_TOTAL_SHARDS='$totalShards'; `$env:LLAMA_TOTAL_LAYERS='$totalLayers'; `$env:LLAMA_MODEL_NAME='$modelName'; cargo run --bin shard_listener -- --bootstrap $bootstrapAddr --cluster $cluster --shard-id $i --total-shards $totalShards --total-layers $totalLayers --model-name $modelName --port $port --shards-dir $shardsDir"
    
    Start-Process powershell -ArgumentList "-NoExit", "-Command", $command -WindowStyle Normal
    Start-Sleep -Seconds 3
}

Write-Host ""
Write-Host "  [OK] All 4 shard nodes starting" -ForegroundColor Green
Write-Host "  Each node will:" -ForegroundColor Gray
Write-Host "    1. Try to load shard-X.gguf from models_cache/shards/" -ForegroundColor Gray
Write-Host "    2. Print [SHARD] SHARD X LOADED if file exists" -ForegroundColor Gray
Write-Host "    3. Join DHT and announce shard availability" -ForegroundColor Gray

# Step 6: Monitor and wait
Write-Host ""
Write-Host "[6/6] Monitoring system startup..." -ForegroundColor Yellow
Write-Host ""

$maxWait = 120  # 2 minutes
$elapsed = 0
$webServerReady = $false
$nodesRunning = 0

while ($elapsed -lt $maxWait) {
    Start-Sleep -Seconds 3
    $elapsed += 3
    
    # Check web server
    if (-not $webServerReady) {
        try {
            $response = Invoke-WebRequest -Uri "http://localhost:8080" -TimeoutSec 2 -UseBasicParsing -ErrorAction Stop
            if (-not $webServerReady) {
                Write-Host "  [OK] Web server is responding! (Status: $($response.StatusCode))" -ForegroundColor Green
                $webServerReady = $true
            }
        } catch {
            # Not ready yet
        }
    }
    
    # Check shard nodes
    $currentNodes = (Get-Process | Where-Object {$_.ProcessName -eq "shard_listener"} -ErrorAction SilentlyContinue).Count
    if ($currentNodes -gt $nodesRunning) {
        Write-Host "  [INFO] $currentNodes/4 shard nodes running..." -ForegroundColor Cyan
        $nodesRunning = $currentNodes
    }
    
    # Check for errors in log file
    if (Test-Path $webServerLog) {
        $recentErrors = Get-Content $webServerLog -Tail 5 | Select-String -Pattern "error|ERROR|failed|Failed|panic" -CaseSensitive:$false
        if ($recentErrors) {
            Write-Host "  [WARN] Recent errors in web server log:" -ForegroundColor Yellow
            $recentErrors | ForEach-Object { Write-Host "    $_" -ForegroundColor Red }
        }
    }
    
    if ($elapsed % 15 -eq 0) {
        Write-Host "  Status: Web server: $(if ($webServerReady) { '[OK]' } else { '[WAITING]' }), Nodes: $currentNodes/4, Elapsed: $elapsed seconds" -ForegroundColor Gray
    }
    
    if ($webServerReady -and $currentNodes -eq 4) {
        Write-Host ""
        Write-Host "  [OK] System is ready!" -ForegroundColor Green
        break
    }
}

# Final status
Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  FINAL STATUS" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

$finalWebServer = Get-Process | Where-Object {$_.ProcessName -eq "web_server"} -ErrorAction SilentlyContinue
$finalNodes = Get-Process | Where-Object {$_.ProcessName -eq "shard_listener"} -ErrorAction SilentlyContinue
$finalBootstrap = Get-Process | Where-Object {$_.ProcessName -eq "server"} -ErrorAction SilentlyContinue

Write-Host "Bootstrap Server: $(if ($finalBootstrap) { "[OK] (PID: $($finalBootstrap.Id))" } else { "[ERROR]" })" -ForegroundColor $(if ($finalBootstrap) { 'Green' } else { 'Red' })
Write-Host "Web Server: $(if ($finalWebServer) { "[OK] (PID: $($finalWebServer.Id))" } else { "[ERROR]" })" -ForegroundColor $(if ($finalWebServer) { 'Green' } else { 'Red' })
$finalNodeCount = if ($finalNodes) { $finalNodes.Count } else { 0 }
Write-Host "Shard Nodes: $finalNodeCount/4" -ForegroundColor $(if ($finalNodeCount -eq 4) { 'Green' } elseif ($finalNodeCount -gt 0) { 'Yellow' } else { 'Red' })

# Check web server log for errors
Write-Host ""
Write-Host "Web Server Log Analysis:" -ForegroundColor Yellow
if (Test-Path $webServerLog) {
    $allErrors = Get-Content $webServerLog | Select-String -Pattern "error|ERROR|failed|Failed|panic|Error" -CaseSensitive:$false
    if ($allErrors) {
        Write-Host "  [WARN] Found $($allErrors.Count) error/warning messages in log" -ForegroundColor Yellow
        Write-Host "  Recent errors:" -ForegroundColor Gray
        $allErrors | Select-Object -Last 5 | ForEach-Object { Write-Host "    $_" -ForegroundColor Red }
    } else {
        Write-Host "  [OK] No errors found in log" -ForegroundColor Green
    }
    
    # Check for pipeline and shard messages
    $pipelineMessages = Get-Content $webServerLog | Select-String -Pattern "Pipeline|pipeline|shard|Shard|DHT|discovered" -CaseSensitive:$false
    if ($pipelineMessages) {
        Write-Host "  [INFO] Found pipeline/shard related messages:" -ForegroundColor Cyan
        $pipelineMessages | Select-Object -Last 5 | ForEach-Object { Write-Host "    $_" -ForegroundColor Gray }
    }
} else {
    Write-Host "  [WARN] Log file not found yet" -ForegroundColor Yellow
}

# Summary of what to watch for
Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  KEY MESSAGES TO WATCH FOR" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "In Shard Node Terminals:" -ForegroundColor Yellow
Write-Host "  [SHARD] SHARD X LOADED BEFORE JOINING NETWORK" -ForegroundColor Green
Write-Host "    - Confirms shard file was found and loaded" -ForegroundColor Gray
Write-Host ""
Write-Host "  [DHT] ANNOUNCED SHARD X TO DHT" -ForegroundColor Green
Write-Host "    - Confirms node registered with pipeline" -ForegroundColor Gray
Write-Host ""
Write-Host "  [LOAD_SHARD] Loaded shard X from: ..." -ForegroundColor Green
Write-Host "    - Appears when coordinator sends LOAD_SHARD command" -ForegroundColor Gray
Write-Host ""
Write-Host "In Web Server Terminal:" -ForegroundColor Yellow
Write-Host "  [DHT] Discovered shard X from ..." -ForegroundColor Green
Write-Host "    - Confirms nodes are being discovered" -ForegroundColor Gray
Write-Host ""
Write-Host "  [INFERENCE] Pipeline status: X/4 nodes online" -ForegroundColor Green
Write-Host "    - Shows how many nodes are registered" -ForegroundColor Gray
Write-Host ""
Write-Host "  [P2P] Matched response to waiting channel" -ForegroundColor Green
Write-Host "    - Confirms RequestId matching fix is working" -ForegroundColor Gray
Write-Host ""

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  MONITORING INSTRUCTIONS" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "1. Watch Web Server Terminal:" -ForegroundColor Yellow
Write-Host "   - Look for '[INFERENCE]' messages" -ForegroundColor White
Write-Host "   - Look for '[P2P]' messages" -ForegroundColor White
Write-Host "   - Look for error messages" -ForegroundColor White
Write-Host ""
Write-Host "2. Watch Shard Node Terminals:" -ForegroundColor Yellow
Write-Host "   - Look for 'Loading model' or 'tensor' messages" -ForegroundColor White
Write-Host "   - Look for 'Shard loaded' messages" -ForegroundColor White
Write-Host "   - Look for '[INFERENCE]' messages when processing" -ForegroundColor White
Write-Host ""
Write-Host "3. Check Log File:" -ForegroundColor Yellow
Write-Host "   - File: $webServerLog" -ForegroundColor White
Write-Host "   - Run: Get-Content $webServerLog -Tail 20" -ForegroundColor Gray
Write-Host ""
Write-Host "4. Test Inference:" -ForegroundColor Yellow
Write-Host "   - Open: http://localhost:8080" -ForegroundColor Cyan
Write-Host "   - Wait for 4/4 nodes online" -ForegroundColor White
Write-Host "   - Submit query: 'what do a cat and a snake have in common'" -ForegroundColor Cyan
Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

