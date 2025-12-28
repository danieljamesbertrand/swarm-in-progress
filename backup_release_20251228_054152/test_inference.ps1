# Test inference request via WebSocket
$ErrorActionPreference = "Stop"

Write-Host ""
Write-Host "=== TESTING INFERENCE REQUEST ===" -ForegroundColor Cyan
Write-Host "Question: How are a cat and a snake related?" -ForegroundColor Yellow
Write-Host ""

# Create WebSocket client using .NET
Add-Type -AssemblyName System.Net.WebSockets

$uri = New-Object System.Uri("ws://localhost:8081")
$client = New-Object System.Net.WebSockets.ClientWebSocket

try {
    Write-Host "[1/4] Connecting to WebSocket server..." -ForegroundColor Yellow
    $cancellationToken = New-Object System.Threading.CancellationToken
    $client.ConnectAsync($uri, $cancellationToken).Wait()
    Write-Host "  Connected to WebSocket" -ForegroundColor Green
    
    # Create query request
    $queryRequest = @{
        query = "How are a cat and a snake related?"
        request_id = "test-$(Get-Date -Format 'yyyyMMddHHmmss')"
    } | ConvertTo-Json
    
    Write-Host ""
    Write-Host "[2/4] Sending inference request..." -ForegroundColor Yellow
    Write-Host "  Request: $queryRequest" -ForegroundColor Gray
    
    $bytes = [System.Text.Encoding]::UTF8.GetBytes($queryRequest)
    $buffer = New-Object System.ArraySegment[byte] -ArgumentList $bytes
    $client.SendAsync($buffer, [System.Net.WebSockets.WebSocketMessageType]::Text, $true, $cancellationToken).Wait()
    Write-Host "  Request sent" -ForegroundColor Green
    
    Write-Host ""
    Write-Host "[3/4] Waiting for response..." -ForegroundColor Yellow
    
    # Receive response
    $receiveBuffer = New-Object byte[] 8192
    $receiveSegment = New-Object System.ArraySegment[byte] -ArgumentList $receiveBuffer
    $result = $client.ReceiveAsync($receiveSegment, $cancellationToken).Result
    
    if ($result.MessageType -eq [System.Net.WebSockets.WebSocketMessageType]::Text) {
        $responseText = [System.Text.Encoding]::UTF8.GetString($receiveBuffer, 0, $result.Count)
        Write-Host ""
        Write-Host "[4/4] Response received!" -ForegroundColor Green
        Write-Host ""
        Write-Host "========================================" -ForegroundColor Cyan
        Write-Host "AI RESPONSE:" -ForegroundColor Yellow
        Write-Host "========================================" -ForegroundColor Cyan
        
        # Parse and display response
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
        
        Write-Host "========================================" -ForegroundColor Cyan
        Write-Host ""
    } else {
        Write-Host "  Received non-text message" -ForegroundColor Yellow
    }
    
    # Close connection
    $closeStatus = [System.Net.WebSockets.WebSocketCloseStatus]::NormalClosure
    $client.CloseAsync($closeStatus, 'Done', $cancellationToken).Wait()
    Write-Host ""
    Write-Host "Test completed successfully!" -ForegroundColor Green
    
} catch {
    Write-Host ""
    Write-Host "Error: $_" -ForegroundColor Red
    exit 1
} finally {
    if ($client.State -eq [System.Net.WebSockets.WebSocketState]::Open) {
        $closeStatus = [System.Net.WebSockets.WebSocketCloseStatus]::NormalClosure
        $client.CloseAsync($closeStatus, 'Done', $cancellationToken).Wait()
    }
    $client.Dispose()
}
