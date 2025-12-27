# Distributed Inference Test
# Tests the complete distributed inference pipeline with a specific question

$ErrorActionPreference = "Continue"

$question = "What do a snake and a cat have in common?"

Write-Host ""
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host "  DISTRIBUTED INFERENCE TEST" -ForegroundColor Cyan
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Question: $question" -ForegroundColor Yellow
Write-Host ""

# Check prerequisites
Write-Host "[PRE-CHECK] Verifying system status..." -ForegroundColor Yellow

$bootstrap = Get-Process -Name "server" -ErrorAction SilentlyContinue
$web = Get-Process -Name "web_server" -ErrorAction SilentlyContinue
$nodes = Get-Process -Name "shard_listener" -ErrorAction SilentlyContinue
$nodeCount = if ($nodes) { $nodes.Count } else { 0 }

if (-not $bootstrap) {
    Write-Host "  ERROR: Bootstrap server not running!" -ForegroundColor Red
    Write-Host "  Please start: cargo run --bin server -- --listen-addr 0.0.0.0 --port 51820" -ForegroundColor Yellow
    exit 1
}

if (-not $web) {
    Write-Host "  ERROR: Web server not running!" -ForegroundColor Red
    Write-Host "  Please start: cargo run --bin web_server" -ForegroundColor Yellow
    exit 1
}

if ($nodeCount -lt 4) {
    Write-Host "  WARNING: Only $nodeCount/4 nodes running" -ForegroundColor Yellow
    Write-Host "  Inference may fail if pipeline is incomplete" -ForegroundColor Yellow
} else {
    Write-Host "  Bootstrap: Running" -ForegroundColor Green
    Write-Host "  Web Server: Running" -ForegroundColor Green
    Write-Host "  Nodes: $nodeCount/4" -ForegroundColor Green
}

Write-Host ""
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host ""

# Use .NET WebSocket client
Add-Type -AssemblyName System.Net.WebSockets
Add-Type -AssemblyName System.Threading

$uri = New-Object System.Uri("ws://localhost:8081")
$client = New-Object System.Net.WebSockets.ClientWebSocket
$cancellationToken = New-Object System.Threading.CancellationToken

try {
    Write-Host "[1/6] Connecting to WebSocket server (ws://localhost:8081)..." -ForegroundColor Yellow
    $connectTask = $client.ConnectAsync($uri, $cancellationToken)
    $connectTask.Wait(10000) # 10 second timeout
    
    if ($client.State -ne [System.Net.WebSockets.WebSocketState]::Open) {
        throw "Connection failed - state: $($client.State)"
    }
    
    Write-Host "  [SUCCESS] Connected to WebSocket" -ForegroundColor Green
    
    # Create query request
    $requestId = "test-$(Get-Date -Format 'yyyyMMddHHmmss')"
    $queryRequest = @{
        query = $question
        request_id = $requestId
    } | ConvertTo-Json -Compress
    
    Write-Host ""
    Write-Host "[2/6] Sending inference request..." -ForegroundColor Yellow
    Write-Host "  Request ID: $requestId" -ForegroundColor Gray
    Write-Host "  Query: $question" -ForegroundColor Gray
    
    $bytes = [System.Text.Encoding]::UTF8.GetBytes($queryRequest)
    $buffer = New-Object System.ArraySegment[byte]($bytes, 0, $bytes.Length)
    $sendTask = $client.SendAsync($buffer, [System.Net.WebSockets.WebSocketMessageType]::Text, $true, $cancellationToken)
    $sendTask.Wait(5000)
    
    if ($sendTask.IsFaulted) {
        throw "Send failed: $($sendTask.Exception)"
    }
    
    Write-Host "  [SUCCESS] Request sent" -ForegroundColor Green
    
    Write-Host ""
    Write-Host "[3/6] Waiting for response (120 second timeout)..." -ForegroundColor Yellow
    Write-Host "  This may take 30-90 seconds for distributed inference..." -ForegroundColor Gray
    
    # Receive response with timeout
    $receiveBuffer = New-Object byte[] 65536
    $receiveSegment = New-Object System.ArraySegment[byte]($receiveBuffer, 0, $receiveBuffer.Length)
    $timeout = 120000 # 120 seconds
    $startTime = Get-Date
    
    $responseReceived = $false
    $allMessages = @()
    $pipelineStatus = $null
    $finalResponse = $null
    
    while (-not $responseReceived -and ((Get-Date) - $startTime).TotalMilliseconds -lt $timeout) {
        try {
            $receiveTask = $client.ReceiveAsync($receiveSegment, $cancellationToken)
            $receiveTask.Wait(1000) # 1 second wait
            
            if ($receiveTask.IsCompleted -and -not $receiveTask.IsFaulted) {
                $result = $receiveTask.Result
                
                if ($result.MessageType -eq [System.Net.WebSockets.WebSocketMessageType]::Text) {
                    $responseText = [System.Text.Encoding]::UTF8.GetString($receiveBuffer, 0, $result.Count)
                    $allMessages += $responseText
                    
                    # Try to parse as JSON
                    try {
                        $obj = $responseText | ConvertFrom-Json
                        
                        # Check for pipeline status updates
                        if ($obj.message_type -eq "pipeline_status") {
                            $pipelineStatus = $obj
                            $elapsed = [math]::Round(((Get-Date) - $startTime).TotalSeconds, 1)
                            Write-Host "  [STATUS] Pipeline: $($obj.online_nodes)/$($obj.total_nodes) nodes, Complete: $($obj.is_complete) (${elapsed}s)" -ForegroundColor Cyan
                        }
                        # Check for QueryResponse (final response)
                        elseif ($obj.response) {
                            $finalResponse = $obj
                            $responseReceived = $true
                            Write-Host "  [SUCCESS] Final response received!" -ForegroundColor Green
                        }
                        # Check for error
                        elseif ($obj.error) {
                            Write-Host "  [ERROR] $($obj.error)" -ForegroundColor Red
                            $responseReceived = $true
                            $finalResponse = $obj
                        }
                    } catch {
                        # Not JSON, just log it
                        $preview = if ($responseText.Length -gt 100) { $responseText.Substring(0, 100) + "..." } else { $responseText }
                        Write-Host "  [MESSAGE] $preview" -ForegroundColor Gray
                    }
                } elseif ($result.MessageType -eq [System.Net.WebSockets.WebSocketMessageType]::Close) {
                    Write-Host "  [WARNING] Connection closed by server" -ForegroundColor Yellow
                    break
                }
            }
        } catch {
            # Timeout or error, continue waiting
            $elapsed = [math]::Round(((Get-Date) - $startTime).TotalSeconds, 1)
            if ($elapsed % 10 -eq 0) {
                Write-Host "  [WAITING] Still processing... (${elapsed}s)" -ForegroundColor Gray
            }
            Start-Sleep -Milliseconds 100
        }
    }
    
    Write-Host ""
    Write-Host "[4/6] Processing complete!" -ForegroundColor Green
    Write-Host ""
    
    # Display results
    Write-Host "================================================================" -ForegroundColor Cyan
    Write-Host "  INFERENCE RESULTS" -ForegroundColor Cyan
    Write-Host "================================================================" -ForegroundColor Cyan
    Write-Host ""
    
    if ($finalResponse) {
        if ($finalResponse.response) {
            Write-Host "Question:" -ForegroundColor Yellow
            Write-Host "  $question" -ForegroundColor White
            Write-Host ""
            Write-Host "Answer:" -ForegroundColor Yellow
            Write-Host "  $($finalResponse.response)" -ForegroundColor White
            Write-Host ""
            
            if ($finalResponse.tokens) {
                Write-Host "Tokens Generated: $($finalResponse.tokens)" -ForegroundColor Gray
            }
            if ($finalResponse.latency_ms) {
                $latencySec = [math]::Round($finalResponse.latency_ms / 1000.0, 2)
                Write-Host "Latency: $($finalResponse.latency_ms)ms ($latencySec seconds)" -ForegroundColor Gray
            }
            if ($finalResponse.success -ne $null) {
                $successColor = if ($finalResponse.success) { 'Green' } else { 'Red' }
                Write-Host "Success: $($finalResponse.success)" -ForegroundColor $successColor
            }
            if ($finalResponse.shards_used) {
                $shardIds = $finalResponse.shards_used | ForEach-Object { $_.shard_id } | Sort-Object
                Write-Host "Shards Used: $($shardIds -join ', ')" -ForegroundColor Gray
            }
        } elseif ($finalResponse.error) {
            Write-Host "ERROR: $($finalResponse.error)" -ForegroundColor Red
            if ($finalResponse.status) {
                Write-Host "Status: $($finalResponse.status)" -ForegroundColor Yellow
            }
        }
    } else {
        Write-Host "WARNING: No final response received" -ForegroundColor Yellow
        Write-Host "Received $($allMessages.Count) message(s)" -ForegroundColor Gray
        if ($allMessages.Count -gt 0) {
            Write-Host ""
            Write-Host "Last message:" -ForegroundColor Yellow
            Write-Host $allMessages[-1] -ForegroundColor Gray
        }
    }
    
    Write-Host ""
    Write-Host "================================================================" -ForegroundColor Cyan
    Write-Host ""
    
    # Display pipeline status if available
    if ($pipelineStatus) {
        Write-Host "Pipeline Status:" -ForegroundColor Yellow
        Write-Host "  Online Nodes: $($pipelineStatus.online_nodes)/$($pipelineStatus.total_nodes)" -ForegroundColor Gray
        Write-Host "  Pipeline Complete: $($pipelineStatus.is_complete)" -ForegroundColor Gray
        if ($pipelineStatus.missing_shards) {
            Write-Host "  Missing Shards: $($pipelineStatus.missing_shards -join ', ')" -ForegroundColor $(if ($pipelineStatus.missing_shards.Count -eq 0) { 'Green' } else { 'Yellow' })
        }
        Write-Host ""
    }
    
    # Close connection
    Write-Host "[5/6] Closing connection..." -ForegroundColor Yellow
    if ($client.State -eq [System.Net.WebSockets.WebSocketState]::Open) {
        $closeStatus = [System.Net.WebSockets.WebSocketCloseStatus]::NormalClosure
        $client.CloseAsync($closeStatus, "Done", $cancellationToken).Wait(5000)
    }
    Write-Host "  [SUCCESS] Connection closed" -ForegroundColor Green
    
    Write-Host ""
    Write-Host "[6/6] Test completed!" -ForegroundColor Green
    Write-Host ""
    
    # Final status
    if ($finalResponse -and $finalResponse.success) {
        Write-Host "================================================================" -ForegroundColor Green
        Write-Host "  DISTRIBUTED INFERENCE TEST: SUCCESS" -ForegroundColor Green
        Write-Host "================================================================" -ForegroundColor Green
        exit 0
    } elseif ($finalResponse -and -not $finalResponse.success) {
        Write-Host "================================================================" -ForegroundColor Red
        Write-Host "  DISTRIBUTED INFERENCE TEST: FAILED" -ForegroundColor Red
        Write-Host "================================================================" -ForegroundColor Red
        exit 1
    } else {
        Write-Host "================================================================" -ForegroundColor Yellow
        Write-Host "  DISTRIBUTED INFERENCE TEST: INCOMPLETE" -ForegroundColor Yellow
        Write-Host "================================================================" -ForegroundColor Yellow
        exit 2
    }
    
} catch {
    Write-Host ""
    Write-Host "================================================================" -ForegroundColor Red
    Write-Host "  ERROR: $_" -ForegroundColor Red
    Write-Host "================================================================" -ForegroundColor Red
    Write-Host ""
    Write-Host "Stack Trace:" -ForegroundColor Yellow
    Write-Host $_.ScriptStackTrace -ForegroundColor Gray
    exit 1
} finally {
    if ($client) {
        $client.Dispose()
    }
}

