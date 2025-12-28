# Test WebSocket inference request
$ErrorActionPreference = "Continue"

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  TESTING INFERENCE VIA WEBSOCKET" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Question: what do a cat and a snake have in common" -ForegroundColor Yellow
Write-Host ""

# Try using websockets library if available, otherwise use .NET
try {
    # Try to use websockets module
    if (Get-Module -ListAvailable -Name PowerShellWebSocket) {
        Import-Module PowerShellWebSocket
        Write-Host "[1/5] Using PowerShellWebSocket module" -ForegroundColor Yellow
    } else {
        throw "Module not found"
    }
} catch {
    # Fallback: Use .NET WebSocket client
    Write-Host "[1/5] Using .NET WebSocket client" -ForegroundColor Yellow
    
    Add-Type -AssemblyName System.Net.WebSockets
    Add-Type -AssemblyName System.Threading
    
    $uri = New-Object System.Uri("ws://localhost:8081")
    $client = New-Object System.Net.WebSockets.ClientWebSocket
    $cancellationToken = New-Object System.Threading.CancellationToken
    
    try {
        Write-Host "[2/5] Connecting to WebSocket..." -ForegroundColor Yellow
        $connectTask = $client.ConnectAsync($uri, $cancellationToken)
        $connectTask.Wait(10000) # 10 second timeout
        
        if (-not $client.State -eq [System.Net.WebSockets.WebSocketState]::Open) {
            throw "Connection failed - state: $($client.State)"
        }
        
        Write-Host "  Connected successfully" -ForegroundColor Green
        
        # Create query request
        $queryRequest = @{
            query = "what do a cat and a snake have in common"
            request_id = "test-$(Get-Date -Format 'yyyyMMddHHmmss')"
        } | ConvertTo-Json -Compress
        
        Write-Host ""
        Write-Host "[3/5] Sending inference request..." -ForegroundColor Yellow
        Write-Host "  Request: $queryRequest" -ForegroundColor Gray
        
        $bytes = [System.Text.Encoding]::UTF8.GetBytes($queryRequest)
        $buffer = New-Object System.ArraySegment[byte] -ArgumentList $bytes
        $sendTask = $client.SendAsync($buffer, [System.Net.WebSockets.WebSocketMessageType]::Text, $true, $cancellationToken)
        $sendTask.Wait(5000)
        
        if ($sendTask.IsFaulted) {
            throw "Send failed: $($sendTask.Exception)"
        }
        
        Write-Host "  Request sent" -ForegroundColor Green
        
        Write-Host ""
        Write-Host "[4/5] Waiting for response (30 second timeout)..." -ForegroundColor Yellow
        
        # Receive response with timeout
        $receiveBuffer = New-Object byte[] 32768
        $receiveSegment = New-Object System.ArraySegment[byte] -ArgumentList $receiveBuffer
        $timeout = 30000 # 30 seconds
        $startTime = Get-Date
        
        $responseReceived = $false
        $allMessages = @()
        
        while (-not $responseReceived -and ((Get-Date) - $startTime).TotalMilliseconds -lt $timeout) {
            try {
                $receiveTask = $client.ReceiveAsync($receiveSegment, $cancellationToken)
                $receiveTask.Wait(1000) # 1 second wait
                
                if ($receiveTask.IsCompleted -and -not $receiveTask.IsFaulted) {
                    $result = $receiveTask.Result
                    
                    if ($result.MessageType -eq [System.Net.WebSockets.WebSocketMessageType]::Text) {
                        $responseText = [System.Text.Encoding]::UTF8.GetString($receiveBuffer, 0, $result.Count)
                        $allMessages += $responseText
                        
                        Write-Host "  Received message: $($responseText.Substring(0, [Math]::Min(100, $responseText.Length)))..." -ForegroundColor Gray
                        
                        # Check if this is a QueryResponse
                        try {
                            $responseObj = $responseText | ConvertFrom-Json
                            if ($responseObj.response) {
                                $responseReceived = $true
                            }
                        } catch {
                            # Not a JSON response, continue waiting
                        }
                    } elseif ($result.MessageType -eq [System.Net.WebSockets.WebSocketMessageType]::Close) {
                        Write-Host "  Connection closed by server" -ForegroundColor Yellow
                        break
                    }
                }
            } catch {
                # Timeout or error, continue
                Start-Sleep -Milliseconds 100
            }
        }
        
        Write-Host ""
        Write-Host "[5/5] Response received!" -ForegroundColor Green
        Write-Host ""
        Write-Host "========================================" -ForegroundColor Cyan
        Write-Host "  INFERENCE RESPONSE" -ForegroundColor Cyan
        Write-Host "========================================" -ForegroundColor Cyan
        Write-Host ""
        
        # Parse and display all messages
        foreach ($msg in $allMessages) {
            try {
                $obj = $msg | ConvertFrom-Json
                if ($obj.response) {
                    Write-Host "Response:" -ForegroundColor Yellow
                    Write-Host $obj.response -ForegroundColor White
                    Write-Host ""
                    Write-Host "Tokens: $($obj.tokens)" -ForegroundColor Gray
                    Write-Host "Latency: $($obj.latency_ms)ms" -ForegroundColor Gray
                    Write-Host "Success: $($obj.success)" -ForegroundColor $(if ($obj.success) { 'Green' } else { 'Red' })
                } elseif ($obj.message_type) {
                    Write-Host "Message Type: $($obj.message_type)" -ForegroundColor Gray
                    if ($obj | Get-Member -Name "online_nodes") {
                        Write-Host "  Online Nodes: $($obj.online_nodes)/$($obj.total_nodes)" -ForegroundColor Gray
                        Write-Host "  Pipeline Complete: $($obj.is_complete)" -ForegroundColor Gray
                    }
                }
            } catch {
                Write-Host "Raw message: $msg" -ForegroundColor Gray
            }
        }
        
        Write-Host "========================================" -ForegroundColor Cyan
        Write-Host ""
        
        # Close connection
        if ($client.State -eq [System.Net.WebSockets.WebSocketState]::Open) {
            $closeStatus = [System.Net.WebSockets.WebSocketCloseStatus]::NormalClosure
            $client.CloseAsync($closeStatus, "Done", $cancellationToken).Wait(5000)
        }
        
        Write-Host "Test completed!" -ForegroundColor Green
        
    } catch {
        Write-Host ""
        Write-Host "ERROR: $_" -ForegroundColor Red
        Write-Host "Stack: $($_.ScriptStackTrace)" -ForegroundColor Red
        exit 1
    } finally {
        if ($client) {
            $client.Dispose()
        }
    }
}

