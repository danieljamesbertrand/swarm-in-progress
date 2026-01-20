# Monitor Shard Discovery - Keep checking until shards come online
# This script continuously monitors the system until all shards are discovered

param(
    [int]$MaxWaitMinutes = 10,
    [int]$CheckIntervalSeconds = 5
)

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  SHARD DISCOVERY MONITOR" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Monitoring until all 4 shards come online..." -ForegroundColor Yellow
Write-Host "Check interval: $CheckIntervalSeconds seconds" -ForegroundColor Gray
Write-Host "Max wait time: $MaxWaitMinutes minutes" -ForegroundColor Gray
Write-Host ""

$startTime = Get-Date
$maxWaitTime = $startTime.AddMinutes($MaxWaitMinutes)
$checkCount = 0

function Check-PipelineStatus {
    param([string]$WebSocketUrl = "ws://localhost:8081")
    
    # Try to get status via HTTP first (simpler)
    try {
        $response = Invoke-WebRequest -Uri "http://localhost:8080" -TimeoutSec 2 -UseBasicParsing -ErrorAction Stop
        # If web server is responding, that's a good sign
        return $true
    } catch {
        return $false
    }
}

function Check-Processes {
    $bootstrap = Get-Process | Where-Object {$_.ProcessName -eq "server"} -ErrorAction SilentlyContinue
    $webServer = Get-Process | Where-Object {$_.ProcessName -eq "web_server"} -ErrorAction SilentlyContinue
    $shardNodes = Get-Process | Where-Object {$_.ProcessName -eq "shard_listener"} -ErrorAction SilentlyContinue
    
    return @{
        Bootstrap = $bootstrap -ne $null
        WebServer = $webServer -ne $null
        ShardNodes = if ($shardNodes) { $shardNodes.Count } else { 0 }
        AllRunning = ($bootstrap -ne $null) -and ($webServer -ne $null) -and ($shardNodes.Count -eq 4)
    }
}

function Get-PipelineStatusFromWebSocket {
    # Use the Rust client to query status
    # We'll run it and parse output, or use a simpler HTTP check
    try {
        # Try to use curl or Invoke-WebRequest to check if we can get status
        # For now, just check if web server is responding
        $response = Invoke-WebRequest -Uri "http://localhost:8080" -TimeoutSec 2 -UseBasicParsing -ErrorAction Stop
        return @{
            Online = $true
            WebServerResponding = $true
        }
    } catch {
        return @{
            Online = $false
            WebServerResponding = $false
        }
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
    Write-Host "  Processes: Bootstrap=$($processStatus.Bootstrap) WebServer=$($processStatus.WebServer) ShardNodes=$($processStatus.ShardNodes)/4" -ForegroundColor $(if ($processStatus.AllRunning) { 'Green' } else { 'Yellow' })
    
    if (-not $processStatus.AllRunning) {
        Write-Host "  [WARN] Not all processes running. Waiting for processes to start..." -ForegroundColor Yellow
        Start-Sleep -Seconds $CheckIntervalSeconds
        continue
    }
    
    # Check ports
    $port8081 = netstat -ano | findstr ":8081" | findstr "LISTENING"
    $port8080 = netstat -ano | findstr ":8080" | findstr "LISTENING"
    
    if (-not $port8081 -or -not $port8080) {
        Write-Host "  [WARN] Web server ports not listening yet..." -ForegroundColor Yellow
        Start-Sleep -Seconds $CheckIntervalSeconds
        continue
    }
    
    # Try to get pipeline status using Rust status checker
    Write-Host "  Checking pipeline status..." -ForegroundColor Gray
    
    # First check if web server is responding
    try {
        $httpResponse = Invoke-WebRequest -Uri "http://localhost:8080" -TimeoutSec 2 -UseBasicParsing -ErrorAction Stop
        Write-Host "  [OK] Web server HTTP responding" -ForegroundColor Green
    } catch {
        Write-Host "  [WAIT] Web server HTTP not responding yet..." -ForegroundColor Yellow
        Start-Sleep -Seconds $CheckIntervalSeconds
        continue
    }
    
    # Try to check pipeline status via WebSocket
    try {
        # Use the status checker binary
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
            Write-Host ""
            Write-Host "System is ready for AI queries!" -ForegroundColor Green
            Write-Host ""
            exit 0
        } elseif ($statusOutput -match "ERROR: Could not connect" -or $statusOutput -match "Connection refused") {
            Write-Host "  [WAIT] WebSocket not ready yet..." -ForegroundColor Yellow
        } elseif ($statusOutput -match "ERROR: Timeout") {
            Write-Host "  [WAIT] Shards not discovered yet (timeout). Waiting..." -ForegroundColor Yellow
        } else {
            Write-Host "  [INFO] Status: $statusOutput" -ForegroundColor Gray
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
        Write-Host "  1. Shard node terminals for '[DHT] ANNOUNCED SHARD X TO DHT' messages" -ForegroundColor White
        Write-Host "  2. Web server terminal for '[DHT] Discovered shard X' messages" -ForegroundColor White
        Write-Host "  3. All processes are running (bootstrap, web_server, 4x shard_listener)" -ForegroundColor White
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
