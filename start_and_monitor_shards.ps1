# Start All Services and Monitor Until Shards Come Online
# This script starts all 4 shard nodes and web server, connecting to external bootstrap server
# Bootstrap server: eagleoneonline.ca:51820

param(
    [int]$MaxWaitMinutes = 10,
    [int]$CheckIntervalSeconds = 5
)

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  START AND MONITOR SHARD DISCOVERY" -ForegroundColor Cyan
Write-Host "  Bootstrap: eagleoneonline.ca:51820" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# Bootstrap server configuration
$bootstrapHost = "eagleoneonline.ca"
$bootstrapPort = 51820

# Step 1: Resolve hostname to IP address
Write-Host "[1/6] Resolving bootstrap server hostname..." -ForegroundColor Yellow
try {
    $bootstrapIP = [System.Net.Dns]::GetHostAddresses($bootstrapHost) | Where-Object { $_.AddressFamily -eq 'InterNetwork' } | Select-Object -First 1
    if (-not $bootstrapIP) {
        Write-Host "  [ERROR] Failed to resolve ${bootstrapHost} to IPv4 address" -ForegroundColor Red
        exit 1
    }
    $bootstrapIPString = $bootstrapIP.IPAddressToString
    Write-Host "  Resolved ${bootstrapHost} to ${bootstrapIPString}" -ForegroundColor Gray
} catch {
    Write-Host "  [ERROR] DNS resolution failed: $_" -ForegroundColor Red
    exit 1
}

$bootstrapAddr = "/ip4/$bootstrapIPString/udp/$bootstrapPort/quic-v1"  # QUIC preferred - must use IP, not hostname
$cluster = "llama-cluster"
$totalShards = 4
$totalLayers = 32
$modelName = "llama-8b"
$shardsDir = "models_cache/shards"

# Step 2: Verify bootstrap server configuration
Write-Host ""
Write-Host "[2/6] Bootstrap server configuration..." -ForegroundColor Yellow
Write-Host "  Bootstrap server: ${bootstrapHost} (${bootstrapIPString}:${bootstrapPort})" -ForegroundColor Gray
Write-Host "  Bootstrap address: $bootstrapAddr" -ForegroundColor Gray
Write-Host "  [OK] Using external bootstrap server (eagleoneonline.ca)" -ForegroundColor Green

# Step 3: Clean up existing shard nodes
Write-Host ""
Write-Host "[3/6] Cleaning up existing shard nodes..." -ForegroundColor Yellow
$existing = Get-Process | Where-Object {$_.ProcessName -eq "shard_listener"} -ErrorAction SilentlyContinue
if ($existing) {
    Write-Host "  Stopping $($existing.Count) existing shard node(s)..." -ForegroundColor Gray
    $existing | Stop-Process -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 2
}
Write-Host "  [OK] Cleanup complete" -ForegroundColor Green

# Step 4: Start all 4 shard nodes
Write-Host ""
Write-Host "[4/6] Starting all 4 shard nodes..." -ForegroundColor Yellow
Write-Host "  Connecting to bootstrap: ${bootstrapHost} (${bootstrapIPString}:${bootstrapPort})" -ForegroundColor Gray

for ($i = 0; $i -lt 4; $i++) {
    $port = 51821 + $i
    Write-Host "  Starting shard $i on port $port..." -ForegroundColor Gray
    $env:LLAMA_SHARD_ID = "$i"
    $env:LLAMA_TOTAL_SHARDS = "4"
    Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$PWD'; `$env:LLAMA_SHARD_ID='$i'; `$env:LLAMA_TOTAL_SHARDS='4'; Write-Host '=== SHARD NODE $i (Bootstrap: $bootstrapHost) ===' -ForegroundColor Cyan; cargo run --bin shard_listener -- --bootstrap $bootstrapAddr --cluster $cluster --shard-id $i --total-shards $totalShards --total-layers $totalLayers --model-name $modelName --port $port --shards-dir $shardsDir --transport dual" -WindowStyle Normal
    Start-Sleep -Seconds 3
}
Write-Host "  [OK] All 4 shard nodes started" -ForegroundColor Green

# Step 5: Start Web Server
Write-Host ""
Write-Host "[5/6] Starting web server..." -ForegroundColor Yellow
Write-Host "  Connecting to bootstrap: ${bootstrapHost} (${bootstrapIPString}:${bootstrapPort})" -ForegroundColor Gray
$webServer = Get-Process | Where-Object {$_.ProcessName -eq "web_server"} -ErrorAction SilentlyContinue
if (-not $webServer) {
    $env:BOOTSTRAP = $bootstrapAddr
    $webCmd = "cd '$PWD'; Write-Host '=== WEB SERVER (QUIC) - Bootstrap: $bootstrapHost ===' -ForegroundColor Cyan; `$env:BOOTSTRAP='$bootstrapAddr'; cargo run --bin web_server"
    Start-Process powershell -ArgumentList "-NoExit", "-Command", $webCmd -WindowStyle Normal
    Start-Sleep -Seconds 10
    Write-Host "  [OK] Web server started" -ForegroundColor Green
} else {
    Write-Host "  [OK] Web server already running (PID: $($webServer.Id))" -ForegroundColor Green
}

# Step 6: Wait for initial startup
Write-Host ""
Write-Host "[6/6] Waiting for initial startup (15 seconds)..." -ForegroundColor Yellow
Write-Host "  This allows nodes to compile and connect to bootstrap server..." -ForegroundColor Gray
Start-Sleep -Seconds 15
Write-Host "  [OK] Initial wait complete" -ForegroundColor Green

# Step 7: Monitor until shards come online
Write-Host ""
Write-Host "[7/7] Monitoring until all shards come online..." -ForegroundColor Yellow
Write-Host "  Check interval: $CheckIntervalSeconds seconds" -ForegroundColor Gray
Write-Host "  Max wait time: $MaxWaitMinutes minutes" -ForegroundColor Gray
Write-Host "  Bootstrap: ${bootstrapHost} (${bootstrapIPString}:${bootstrapPort})" -ForegroundColor Gray
Write-Host ""

$startTime = Get-Date
$maxWaitTime = $startTime.AddMinutes($MaxWaitMinutes)
$checkCount = 0

function Check-Processes {
    $webServer = Get-Process | Where-Object {$_.ProcessName -eq "web_server"} -ErrorAction SilentlyContinue
    $shardNodes = Get-Process | Where-Object {$_.ProcessName -eq "shard_listener"} -ErrorAction SilentlyContinue
    
    return @{
        WebServer = $webServer -ne $null
        ShardNodes = if ($shardNodes) { $shardNodes.Count } else { 0 }
        AllRunning = ($webServer -ne $null) -and ($shardNodes.Count -eq 4)
    }
}

# Main monitoring loop
while ((Get-Date) -lt $maxWaitTime) {
    $checkCount++
    $elapsed = (Get-Date) - $startTime
    $elapsedMinutes = [math]::Round($elapsed.TotalMinutes, 1)
    
    Write-Host "[Check #$checkCount] Elapsed: $elapsedMinutes minutes" -ForegroundColor Gray
    
    # Check processes
    $processStatus = Check-Processes
    Write-Host "  Processes: WebServer=$($processStatus.WebServer) ShardNodes=$($processStatus.ShardNodes)/4" -ForegroundColor $(if ($processStatus.AllRunning) { 'Green' } else { 'Yellow' })
    
    if (-not $processStatus.AllRunning) {
        Write-Host "  [WARN] Not all processes running. Waiting..." -ForegroundColor Yellow
        Start-Sleep -Seconds $CheckIntervalSeconds
        continue
    }
    
    # Check ports
    $port8081 = netstat -ano | findstr ":8081" | findstr "LISTENING"
    $port8080 = netstat -ano | findstr ":8080" | findstr "LISTENING"
    
    if (-not $port8081 -or -not $port8080) {
        Write-Host "  [WAIT] Web server ports not listening yet..." -ForegroundColor Yellow
        Start-Sleep -Seconds $CheckIntervalSeconds
        continue
    }
    
    # Check HTTP response
    try {
        $httpResponse = Invoke-WebRequest -Uri "http://localhost:8080" -TimeoutSec 2 -UseBasicParsing -ErrorAction Stop
        Write-Host "  [OK] Web server HTTP responding" -ForegroundColor Green
    } catch {
        Write-Host "  [WAIT] Web server HTTP not responding yet..." -ForegroundColor Yellow
        Start-Sleep -Seconds $CheckIntervalSeconds
        continue
    }
    
    # Try to check pipeline status via WebSocket
    Write-Host "  Checking pipeline status..." -ForegroundColor Gray
    try {
        $statusOutput = & cargo run --bin check_pipeline_status 2>&1 | Out-String
        
        if ($statusOutput -match "OK: System responding") {
            Write-Host "  [SUCCESS] System is responding to queries!" -ForegroundColor Green
            Write-Host ""
            Write-Host "========================================" -ForegroundColor Green
            Write-Host "  SHARDS ARE ONLINE!" -ForegroundColor Green
            Write-Host "========================================" -ForegroundColor Green
            Write-Host ""
            Write-Host "Total time: $elapsedMinutes minutes" -ForegroundColor White
            Write-Host "Total checks: $checkCount" -ForegroundColor White
            Write-Host "Bootstrap: ${bootstrapHost} (${bootstrapIPString}:${bootstrapPort})" -ForegroundColor White
            Write-Host ""
            Write-Host "System is ready for AI queries!" -ForegroundColor Green
            Write-Host ""
            Write-Host "Next steps:" -ForegroundColor Yellow
            Write-Host "  1. Open http://localhost:8080 in your browser" -ForegroundColor White
            Write-Host "  2. Or run: cargo run --example ai_query_client -- 'Your question here'" -ForegroundColor White
            Write-Host ""
            exit 0
        } elseif ($statusOutput -match "ERROR: Could not connect" -or $statusOutput -match "Connection refused") {
            Write-Host "  [WAIT] WebSocket not ready yet..." -ForegroundColor Yellow
        } elseif ($statusOutput -match "ERROR: Timeout") {
            Write-Host "  [WAIT] Shards not discovered yet (timeout). Waiting..." -ForegroundColor Yellow
        } else {
            Write-Host "  [INFO] Status check completed" -ForegroundColor Gray
        }
    } catch {
        Write-Host "  [WAIT] Could not check status: $_" -ForegroundColor Yellow
    }
    
    # Check if we should continue
    if ((Get-Date) -ge $maxWaitTime) {
        Write-Host ""
        Write-Host "========================================" -ForegroundColor Red
        Write-Host "  TIMEOUT REACHED" -ForegroundColor Red
        Write-Host "========================================" -ForegroundColor Red
        Write-Host ""
        Write-Host "Waited $MaxWaitMinutes minutes but shards did not come online." -ForegroundColor Yellow
        Write-Host ""
        Write-Host "Please check:" -ForegroundColor Yellow
        Write-Host "  1. Bootstrap server ${bootstrapHost} (${bootstrapIPString}:${bootstrapPort}) is accessible" -ForegroundColor White
        Write-Host "  2. Shard node terminals for '[DHT] ANNOUNCED SHARD X TO DHT' messages" -ForegroundColor White
        Write-Host "  3. Web server terminal for '[DHT] Discovered shard X' messages" -ForegroundColor White
        Write-Host "  4. All processes are running (web_server, 4x shard_listener)" -ForegroundColor White
        Write-Host ""
        exit 1
    }
    
    Write-Host "  Waiting $CheckIntervalSeconds seconds before next check..." -ForegroundColor Gray
    Write-Host ""
    Start-Sleep -Seconds $CheckIntervalSeconds
}

Write-Host ""
Write-Host "Monitoring stopped." -ForegroundColor Yellow
exit 1