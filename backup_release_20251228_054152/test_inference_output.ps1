# Test Inference and Show Full Output
# Sends query "what do a cat and a snake have in common" and displays full results

Write-Host ""
Write-Host "═══════════════════════════════════════════════════════════════" -ForegroundColor Cyan
Write-Host "  INFERENCE TEST - FULL OUTPUT" -ForegroundColor Cyan
Write-Host "═══════════════════════════════════════════════════════════════" -ForegroundColor Cyan
Write-Host ""

# Check if web server is running
Write-Host "[1/4] Checking web server..." -ForegroundColor Yellow
$webServer = Get-Process | Where-Object {$_.ProcessName -eq "web_server"} -ErrorAction SilentlyContinue
if (-not $webServer) {
    Write-Host "  [ERROR] Web server is not running!" -ForegroundColor Red
    Write-Host "  Please start the web server first:" -ForegroundColor Yellow
    Write-Host "    cargo run --bin web_server" -ForegroundColor White
    exit 1
}
Write-Host "  [OK] Web server running (PID: $($webServer.Id))" -ForegroundColor Green

# Check if nodes are running
Write-Host ""
Write-Host "[2/4] Checking shard nodes..." -ForegroundColor Yellow
$nodes = Get-Process | Where-Object {$_.ProcessName -eq "shard_listener"} -ErrorAction SilentlyContinue
$nodeCount = if ($nodes) { $nodes.Count } else { 0 }
Write-Host "  Shard nodes running: $nodeCount" -ForegroundColor $(if ($nodeCount -gt 0) { 'Green' } else { 'Yellow' })

# Check HTTP endpoint
Write-Host ""
Write-Host "[3/4] Testing HTTP endpoint..." -ForegroundColor Yellow
try {
    $response = Invoke-WebRequest -Uri "http://localhost:8080" -TimeoutSec 5 -UseBasicParsing -ErrorAction Stop
    Write-Host "  [OK] HTTP server responding (Status: $($response.StatusCode))" -ForegroundColor Green
} catch {
    Write-Host "  [ERROR] HTTP server error: $_" -ForegroundColor Red
    exit 1
}

# Send inference query via WebSocket
Write-Host ""
Write-Host "[4/4] Sending inference query..." -ForegroundColor Yellow
Write-Host ""
Write-Host "Query: 'what do a cat and a snake have in common'" -ForegroundColor Cyan
Write-Host ""

# Use .NET WebSocket client to send query
Add-Type -AssemblyName System.Net.WebSockets
Add-Type -AssemblyName System.Threading

$uri = New-Object System.Uri("ws://localhost:8081")
$client = New-Object System.Net.WebSockets.ClientWebSocket
$cancellationToken = New-Object System.Threading.CancellationToken

try {
    # Connect to WebSocket
    Write-Host "Connecting to WebSocket: ws://localhost:8081" -ForegroundColor Gray
    $connectTask = $client.ConnectAsync($uri, $cancellationToken)
    $connectTask.Wait(5000)
    
    if (-not $client.State -eq [System.Net.WebSockets.WebSocketState]::Open) {
        Write-Host "  [ERROR] Failed to connect to WebSocket" -ForegroundColor Red
        Write-Host "  State: $($client.State)" -ForegroundColor Yellow
        exit 1
    }
    
    Write-Host "  [OK] Connected to WebSocket" -ForegroundColor Green
    Write-Host ""
    
    # Send query
    $query = @{
        query = "what do a cat and a snake have in common"
        request_id = [System.Guid]::NewGuid().ToString()
    } | ConvertTo-Json -Compress
    
    Write-Host "Sending query..." -ForegroundColor Gray
    $queryBytes = [System.Text.Encoding]::UTF8.GetBytes($query)
    $buffer = New-Object System.ArraySegment[byte] -ArgumentList $queryBytes
    $sendTask = $client.SendAsync($buffer, [System.Net.WebSockets.WebSocketMessageType]::Text, $true, $cancellationToken)
    $sendTask.Wait(5000)
    Write-Host "  [OK] Query sent" -ForegroundColor Green
    Write-Host ""
    
    # Receive responses
    Write-Host "═══════════════════════════════════════════════════════════════" -ForegroundColor Cyan
    Write-Host "  RECEIVING RESPONSES" -ForegroundColor Cyan
    Write-Host "═══════════════════════════════════════════════════════════════" -ForegroundColor Cyan
    Write-Host ""
    
    $maxWait = 60  # Wait up to 60 seconds for response
    $startTime = Get-Date
    $responseCount = 0
    
    while (((Get-Date) - $startTime).TotalSeconds -lt $maxWait) {
        # Check if data is available
        if ($client.State -eq [System.Net.WebSockets.WebSocketState]::Open) {
            $receiveBuffer = New-Object byte[] 8192
            $arrayBuffer = New-Object System.ArraySegment[byte] -ArgumentList $receiveBuffer
            $receiveTask = $client.ReceiveAsync($arrayBuffer, $cancellationToken)
            
            # Wait with timeout
            $timeout = 2000  # 2 seconds
            $completed = $receiveTask.Wait($timeout)
            
            if ($completed -and $receiveTask.Result.Count -gt 0) {
                $responseCount++
                $messageBytes = $receiveBuffer[0..($receiveTask.Result.Count - 1)]
                $messageText = [System.Text.Encoding]::UTF8.GetString($messageBytes)
                
                Write-Host "[RESPONSE $responseCount]" -ForegroundColor Yellow
                Write-Host "───────────────────────────────────────────────────────────" -ForegroundColor Gray
                
                # Try to parse as JSON
                try {
                    $jsonResponse = $messageText | ConvertFrom-Json
                    
                    # Check response type
                    if ($jsonResponse.type) {
                        Write-Host "Type: $($jsonResponse.type)" -ForegroundColor Cyan
                    }
                    
                    # Display response content
                    if ($jsonResponse.response) {
                        Write-Host ""
                        Write-Host "RESPONSE:" -ForegroundColor Green
                        Write-Host $jsonResponse.response -ForegroundColor White
                        Write-Host ""
                    }
                    
                    # Display other fields
                    if ($jsonResponse.tokens) {
                        Write-Host "Tokens: $($jsonResponse.tokens)" -ForegroundColor Gray
                    }
                    if ($jsonResponse.latency_ms) {
                        Write-Host "Latency: $($jsonResponse.latency_ms) ms" -ForegroundColor Gray
                    }
                    if ($jsonResponse.success) {
                        Write-Host "Success: $($jsonResponse.success)" -ForegroundColor $(if ($jsonResponse.success) { 'Green' } else { 'Red' })
                    }
                    if ($jsonResponse.shards_used) {
                        Write-Host "Shards Used: $($jsonResponse.shards_used.Count)" -ForegroundColor Gray
                        $jsonResponse.shards_used | ForEach-Object {
                            Write-Host "  - Shard $($_.shard_id): $($_.latency_ms) ms" -ForegroundColor Gray
                        }
                    }
                    
                    # Display full JSON for debugging
                    Write-Host ""
                    Write-Host "Full JSON Response:" -ForegroundColor Gray
                    Write-Host ($jsonResponse | ConvertTo-Json -Depth 10) -ForegroundColor DarkGray
                    
                } catch {
                    # Not JSON, display as text
                    Write-Host "Raw Response:" -ForegroundColor Green
                    Write-Host $messageText -ForegroundColor White
                }
                
                Write-Host "───────────────────────────────────────────────────────────" -ForegroundColor Gray
                Write-Host ""
                
                # If we got a query response, we're done
                if ($jsonResponse.response -or $jsonResponse.type -eq "query_response") {
                    Write-Host "[OK] Received complete response" -ForegroundColor Green
                    break
                }
            }
        } else {
            Write-Host "  [WARN] WebSocket closed. State: $($client.State)" -ForegroundColor Yellow
            break
        }
        
        Start-Sleep -Milliseconds 100
    }
    
    if ($responseCount -eq 0) {
        Write-Host "[WARN] No responses received within timeout period" -ForegroundColor Yellow
        Write-Host "  This may indicate:" -ForegroundColor Yellow
        Write-Host "    - Nodes are not ready yet" -ForegroundColor Gray
        Write-Host "    - Inference is still processing" -ForegroundColor Gray
        Write-Host "    - Network connectivity issues" -ForegroundColor Gray
    }
    
} catch {
    Write-Host ""
    Write-Host "[ERROR] Exception occurred:" -ForegroundColor Red
    Write-Host $_.Exception.Message -ForegroundColor Red
    Write-Host $_.Exception.StackTrace -ForegroundColor DarkGray
} finally {
    if ($client.State -eq [System.Net.WebSockets.WebSocketState]::Open) {
        Write-Host ""
        Write-Host "Closing WebSocket connection..." -ForegroundColor Gray
        $closeTask = $client.CloseAsync([System.Net.WebSockets.WebSocketCloseStatus]::NormalClosure, "Done", $cancellationToken)
        $closeTask.Wait(2000)
    }
    $client.Dispose()
}

Write-Host ""
Write-Host "═══════════════════════════════════════════════════════════════" -ForegroundColor Cyan
Write-Host "  TEST COMPLETE" -ForegroundColor Cyan
Write-Host "═══════════════════════════════════════════════════════════════" -ForegroundColor Cyan
Write-Host ""

