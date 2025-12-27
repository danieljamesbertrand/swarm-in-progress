# Check DHT Status by querying web server WebSocket
# This will show us what the coordinator actually sees

$ErrorActionPreference = "Continue"

Write-Host ""
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host "  CHECKING DHT STATUS VIA WEBSOCKET" -ForegroundColor Cyan
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host ""

try {
    # Use .NET WebSocket to connect and get status
    Add-Type -AssemblyName System.Net.WebSockets -ErrorAction SilentlyContinue
    Add-Type -AssemblyName System.Threading
    
    $uri = New-Object System.Uri("ws://localhost:8081")
    $client = New-Object System.Net.WebSockets.ClientWebSocket
    $cancellationToken = New-Object System.Threading.CancellationToken
    
    Write-Host "[1/3] Connecting to WebSocket..." -ForegroundColor Yellow
    $connectTask = $client.ConnectAsync($uri, $cancellationToken)
    $connectTask.Wait(5000)
    
    if ($client.State -ne [System.Net.WebSockets.WebSocketState]::Open) {
        Write-Host "  [FAILED] Could not connect" -ForegroundColor Red
        exit 1
    }
    
    Write-Host "  [SUCCESS] Connected" -ForegroundColor Green
    Write-Host ""
    
    Write-Host "[2/3] Waiting for pipeline status messages (10 seconds)..." -ForegroundColor Yellow
    
    $receiveBuffer = New-Object byte[] 65536
    $receiveSegment = New-Object System.ArraySegment[byte]($receiveBuffer, 0, $receiveBuffer.Length)
    $timeout = 10000 # 10 seconds
    $startTime = Get-Date
    $statusMessages = @()
    
    while (((Get-Date) - $startTime).TotalMilliseconds -lt $timeout) {
        try {
            $receiveTask = $client.ReceiveAsync($receiveSegment, $cancellationToken)
            $receiveTask.Wait(1000)
            
            if ($receiveTask.IsCompleted -and -not $receiveTask.IsFaulted) {
                $result = $receiveTask.Result
                if ($result.MessageType -eq [System.Net.WebSockets.WebSocketMessageType]::Text) {
                    $responseText = [System.Text.Encoding]::UTF8.GetString($receiveBuffer, 0, $result.Count)
                    try {
                        $obj = $responseText | ConvertFrom-Json
                        if ($obj.message_type -eq "pipeline_status") {
                            $statusMessages += $obj
                            $elapsed = [math]::Round(((Get-Date) - $startTime).TotalSeconds, 1)
                            Write-Host "  [STATUS] Received at ${elapsed}s: $($obj.online_nodes)/$($obj.total_nodes) nodes, complete: $($obj.is_complete)" -ForegroundColor Cyan
                        }
                    } catch {
                        # Not JSON, skip
                    }
                }
            }
        } catch {
            # Timeout, continue
        }
    }
    
    Write-Host ""
    Write-Host "[3/3] Final Status:" -ForegroundColor Yellow
    Write-Host ""
    
    if ($statusMessages.Count -gt 0) {
        $latest = $statusMessages[-1]
        Write-Host "================================================================" -ForegroundColor Cyan
        Write-Host "  PIPELINE STATUS" -ForegroundColor Cyan
        Write-Host "================================================================" -ForegroundColor Cyan
        Write-Host ""
        Write-Host "Online Nodes: $($latest.online_nodes)/$($latest.total_nodes)" -ForegroundColor $(if ($latest.online_nodes -eq 4) { 'Green' } elseif ($latest.online_nodes -gt 0) { 'Yellow' } else { 'Red' })
        Write-Host "Pipeline Complete: $($latest.is_complete)" -ForegroundColor $(if ($latest.is_complete) { 'Green' } else { 'Yellow' })
        
        if ($latest.missing_shards) {
            $missingStr = $latest.missing_shards -join ", "
            Write-Host "Missing Shards: $missingStr" -ForegroundColor $(if ($latest.missing_shards.Count -eq 0) { 'Green' } else { 'Yellow' })
        }
        
        Write-Host ""
        Write-Host "================================================================" -ForegroundColor Cyan
        
        if ($latest.online_nodes -eq 0) {
            Write-Host ""
            Write-Host "DIAGNOSIS: Coordinator sees 0 nodes" -ForegroundColor Red
            Write-Host ""
            Write-Host "This means:" -ForegroundColor Yellow
            Write-Host "  1. Nodes may not be announcing to DHT" -ForegroundColor White
            Write-Host "  2. Coordinator may not be querying DHT correctly" -ForegroundColor White
            Write-Host "  3. DHT routing may be broken (records exist but unreachable)" -ForegroundColor White
            Write-Host ""
            Write-Host "Check node console windows for:" -ForegroundColor Yellow
            Write-Host "  [DHT] ANNOUNCED SHARD X TO DHT" -ForegroundColor White
            Write-Host ""
            Write-Host "Check web server console for:" -ForegroundColor Yellow
            Write-Host "  [DHT] Querying for 4 shards..." -ForegroundColor White
            Write-Host "  [DHT] Discovered shard X from {peer_id}" -ForegroundColor White
        } elseif ($latest.online_nodes -lt 4) {
            Write-Host ""
            Write-Host "DIAGNOSIS: Partial discovery - $($latest.online_nodes)/4 nodes found" -ForegroundColor Yellow
            Write-Host "  Some nodes may not have announced yet" -ForegroundColor White
        } else {
            Write-Host ""
            Write-Host "DIAGNOSIS: All nodes discovered!" -ForegroundColor Green
        }
    } else {
        Write-Host "  [WARNING] No pipeline status messages received" -ForegroundColor Yellow
        Write-Host "  Web server may not be sending status updates" -ForegroundColor Gray
    }
    
    $client.CloseAsync([System.Net.WebSockets.WebSocketCloseStatus]::NormalClosure, "Done", $cancellationToken).Wait(2000)
    $client.Dispose()
    
} catch {
    Write-Host ""
    Write-Host "ERROR: $_" -ForegroundColor Red
    Write-Host $_.ScriptStackTrace -ForegroundColor Gray
    exit 1
}

Write-Host ""

