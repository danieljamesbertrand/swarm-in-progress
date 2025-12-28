# Send inference query via WebSocket
# Usage: .\send_query.ps1 "your query here"

param(
    [string]$Query = "what does a cow say"
)

Write-Host ""
Write-Host "=== SENDING INFERENCE QUERY ===" -ForegroundColor Cyan
Write-Host ""
Write-Host "Query: '$Query'" -ForegroundColor Yellow
Write-Host ""

# Check if websockets module is available
$hasWebSockets = $false
try {
    Add-Type -AssemblyName System.Net.WebSockets -ErrorAction Stop
    $hasWebSockets = $true
} catch {
    Write-Host "[INFO] System.Net.WebSockets not available" -ForegroundColor Yellow
}

if (-not $hasWebSockets) {
    Write-Host "[ERROR] WebSocket support not available in this PowerShell version" -ForegroundColor Red
    Write-Host ""
    Write-Host "Please use one of these options:" -ForegroundColor Yellow
    Write-Host "  1. Open http://localhost:8080 in your browser and enter the query" -ForegroundColor White
    Write-Host "  2. Install Python and run: python test_inference.py" -ForegroundColor White
    Write-Host "  3. Use a WebSocket client tool" -ForegroundColor White
    Write-Host ""
    Write-Host "Query to send: '$Query'" -ForegroundColor Cyan
    Write-Host "WebSocket URL: ws://localhost:8081" -ForegroundColor Cyan
    Write-Host "JSON payload:" -ForegroundColor Cyan
    $json = @{
        query = $Query
        request_id = "query-$(Get-Date -Format 'yyyyMMddHHmmss')"
    } | ConvertTo-Json -Compress
    Write-Host $json -ForegroundColor White
    Write-Host ""
    exit 1
}

# Try to connect and send
try {
    $uri = New-Object System.Uri("ws://localhost:8081")
    $client = New-Object System.Net.WebSockets.ClientWebSocket
    $cancellationToken = New-Object System.Threading.CancellationToken
    
    Write-Host "Connecting to ws://localhost:8081..." -ForegroundColor Gray
    $connectTask = $client.ConnectAsync($uri, $cancellationToken)
    $connectTask.Wait(5000)
    
    if ($client.State -ne [System.Net.WebSockets.WebSocketState]::Open) {
        Write-Host "  [ERROR] Failed to connect (State: $($client.State))" -ForegroundColor Red
        exit 1
    }
    
    Write-Host "  [OK] Connected" -ForegroundColor Green
    Write-Host ""
    
    # Create query request
    $queryRequest = @{
        query = $Query
        request_id = "query-$(Get-Date -Format 'yyyyMMddHHmmss')"
    } | ConvertTo-Json -Compress
    
    Write-Host "Sending query..." -ForegroundColor Gray
    $bytes = [System.Text.Encoding]::UTF8.GetBytes($queryRequest)
    $buffer = New-Object System.ArraySegment[byte] -ArgumentList $bytes
    $sendTask = $client.SendAsync($buffer, [System.Net.WebSockets.WebSocketMessageType]::Text, $true, $cancellationToken)
    $sendTask.Wait()
    Write-Host "  [OK] Query sent" -ForegroundColor Green
    Write-Host ""
    
    Write-Host "Waiting for response..." -ForegroundColor Gray
    $receiveBuffer = New-Object byte[] 16384
    $receiveSegment = New-Object System.ArraySegment[byte] -ArgumentList $receiveBuffer
    $receiveTask = $client.ReceiveAsync($receiveSegment, $cancellationToken)
    $receiveTask.Wait(120000)  # 120 second timeout
    
    if ($receiveTask.IsCompleted) {
        $result = $receiveTask.Result
        if ($result.MessageType -eq [System.Net.WebSockets.WebSocketMessageType]::Text) {
            $responseText = [System.Text.Encoding]::UTF8.GetString($receiveBuffer, 0, $result.Count)
            Write-Host ""
            Write-Host "=== RESPONSE ===" -ForegroundColor Green
            try {
                $responseObj = $responseText | ConvertFrom-Json
                if ($responseObj.response) {
                    Write-Host $responseObj.response -ForegroundColor White
                } else {
                    Write-Host $responseText -ForegroundColor White
                }
            } catch {
                Write-Host $responseText -ForegroundColor White
            }
            Write-Host "=================" -ForegroundColor Green
            Write-Host ""
        }
    } else {
        Write-Host "  [ERROR] Timeout waiting for response" -ForegroundColor Red
    }
    
    $client.CloseAsync([System.Net.WebSockets.WebSocketCloseStatus]::NormalClosure, "Done", $cancellationToken).Wait()
    
} catch {
    Write-Host "  [ERROR] $($_.Exception.Message)" -ForegroundColor Red
    Write-Host ""
    Write-Host "Make sure:" -ForegroundColor Yellow
    Write-Host "  - Web server is running on ws://localhost:8081" -ForegroundColor White
    Write-Host "  - Bootstrap server is running" -ForegroundColor White
    Write-Host "  - Shard nodes are running" -ForegroundColor White
    Write-Host ""
    exit 1
}

