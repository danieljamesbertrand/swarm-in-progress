# WebSocket Test Client for Promethos-AI
Add-Type -AssemblyName System.Net.WebSockets
Add-Type -AssemblyName System.Threading

Write-Host '=== WebSocket Connection Test ===' -ForegroundColor Cyan
Write-Host ''

$wsUrl = 'ws://localhost:8081'
Write-Host "Connecting to $wsUrl..." -ForegroundColor Yellow

try {
    # Create WebSocket client using .NET
    $client = New-Object System.Net.WebSockets.ClientWebSocket
    $cancellationToken = New-Object System.Threading.CancellationToken
    
    # Connect
    $uri = New-Object System.Uri($wsUrl)
    $connectTask = $client.ConnectAsync($uri, $cancellationToken)
    $connectTask.Wait(5000)
    
    if ($client.State -eq 'Open') {
        Write-Host '  WebSocket connected successfully!' -ForegroundColor Green
        
        # Send a test query
        $query = @{
            query = 'What is Promethos?'
            request_id = [DateTimeOffset]::Now.ToUnixTimeMilliseconds().ToString()
        } | ConvertTo-Json
        
        $bytes = [System.Text.Encoding]::UTF8.GetBytes($query)
        $buffer = New-Object System.ArraySegment[byte]($bytes)
        $sendTask = $client.SendAsync($buffer, [System.Net.WebSockets.WebSocketMessageType]::Text, $true, $cancellationToken)
        $sendTask.Wait(2000)
        
        Write-Host '  Test query sent' -ForegroundColor Green
        
        # Wait for response
        Write-Host '  Waiting for response (5 seconds)...' -ForegroundColor Yellow
        Start-Sleep -Seconds 5
        
        # Close connection
        $closeTask = $client.CloseAsync([System.Net.WebSockets.WebSocketCloseStatus]::NormalClosure, 'Test complete', $cancellationToken)
        $closeTask.Wait(2000)
        
        Write-Host '  Connection closed' -ForegroundColor Green
    } else {
        Write-Host "  Connection failed. State: $($client.State)" -ForegroundColor Red
    }
} catch {
    Write-Host "  Error: $_" -ForegroundColor Red
    Write-Host '  Note: PowerShell WebSocket support may be limited. Use browser for full testing.' -ForegroundColor Yellow
}

Write-Host ''
Write-Host '=== Test Complete ===' -ForegroundColor Cyan
Write-Host ''
Write-Host 'For full WebSocket testing, use browser developer tools:' -ForegroundColor Yellow
Write-Host '1. Open http://localhost:8080 in browser'
Write-Host '2. Press F12 to open developer tools'
Write-Host '3. Go to Console tab'
Write-Host '4. Check for WebSocket connection messages'
Write-Host '5. Submit a query and watch for messages'


