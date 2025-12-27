# Comprehensive System Monitoring Script
Write-Host "`n========================================" -ForegroundColor Cyan
Write-Host "  SYSTEM MONITORING & DIAGNOSTICS" -ForegroundColor Cyan
Write-Host "========================================`n" -ForegroundColor Cyan

# Check processes
Write-Host "[1] Process Status:" -ForegroundColor Yellow
$server = Get-Process | Where-Object {$_.ProcessName -eq "server"} | Select-Object -First 1
$webServer = Get-Process | Where-Object {$_.ProcessName -eq "web_server"} | Select-Object -First 1
$nodes = Get-Process | Where-Object {$_.ProcessName -eq "shard_listener"}

Write-Host "  Bootstrap Server: $(if ($server) { 'RUNNING (PID: ' + $server.Id + ')' } else { 'NOT RUNNING' })" -ForegroundColor $(if ($server) { 'Green' } else { 'Red' })
Write-Host "  Web Server:       $(if ($webServer) { 'RUNNING (PID: ' + $webServer.Id + ')' } else { 'NOT RUNNING' })" -ForegroundColor $(if ($webServer) { 'Green' } else { 'Red' })
Write-Host "  Shard Nodes:      $($nodes.Count)/4" -ForegroundColor $(if ($nodes.Count -ge 4) { 'Green' } else { 'Yellow' })

# Check ports
Write-Host "`n[2] Port Status:" -ForegroundColor Yellow
$ports = netstat -ano | findstr "LISTENING" | findstr ":8080 :8081 :51820"
if ($ports) {
    $ports | ForEach-Object {
        if ($_ -match ":(\d+).*LISTENING.*(\d+)") {
            $port = $matches[1]
            $pid = $matches[2]
            $proc = Get-Process -Id $pid -ErrorAction SilentlyContinue
            Write-Host "  Port $port : LISTENING (PID: $pid - $($proc.ProcessName))" -ForegroundColor Green
        }
    }
} else {
    Write-Host "  No ports found listening" -ForegroundColor Red
}

# Test HTTP endpoint
Write-Host "`n[3] HTTP Endpoint Test:" -ForegroundColor Yellow
try {
    $response = Invoke-WebRequest -Uri "http://localhost:8080" -TimeoutSec 3 -UseBasicParsing -ErrorAction Stop
    Write-Host "  HTTP Status: $($response.StatusCode) - OK" -ForegroundColor Green
    Write-Host "  Content Length: $($response.Content.Length) bytes" -ForegroundColor Gray
} catch {
    Write-Host "  HTTP Error: $_" -ForegroundColor Red
}

# Check node connections to bootstrap
Write-Host "`n[4] Node Connections:" -ForegroundColor Yellow
$connections = netstat -ano | findstr "ESTABLISHED" | findstr ":51820"
$nodeConnections = ($connections | Measure-Object).Count
Write-Host "  Nodes connected to bootstrap: $nodeConnections" -ForegroundColor $(if ($nodeConnections -ge 4) { 'Green' } else { 'Yellow' })

# Try to get pipeline status via WebSocket (simplified check)
Write-Host "`n[5] Testing WebSocket Connection:" -ForegroundColor Yellow
try {
    $tcpClient = New-Object System.Net.Sockets.TcpClient
    $tcpClient.Connect("127.0.0.1", 8081)
    if ($tcpClient.Connected) {
        Write-Host "  WebSocket port (8081) is accepting connections" -ForegroundColor Green
        $tcpClient.Close()
    }
} catch {
    Write-Host "  WebSocket port not accessible: $_" -ForegroundColor Red
}

Write-Host "`n========================================" -ForegroundColor Cyan
Write-Host "  DIAGNOSTIC SUMMARY" -ForegroundColor Cyan
Write-Host "========================================`n" -ForegroundColor Cyan

# Summary
$allGood = $server -and $webServer -and ($nodes.Count -ge 4) -and ($nodeConnections -ge 4)
if ($allGood) {
    Write-Host "✓ System appears to be running" -ForegroundColor Green
    Write-Host "`nNext steps:" -ForegroundColor Yellow
    Write-Host "  1. Open http://localhost:8080 in browser" -ForegroundColor White
    Write-Host "  2. Check browser console for WebSocket connection status" -ForegroundColor White
    Write-Host "  3. Try sending a query and check for errors" -ForegroundColor White
} else {
    Write-Host "⚠ Some components may not be running properly" -ForegroundColor Yellow
    if (-not $server) { Write-Host "  - Bootstrap server is not running" -ForegroundColor Red }
    if (-not $webServer) { Write-Host "  - Web server is not running" -ForegroundColor Red }
    if ($nodes.Count -lt 4) { Write-Host "  - Only $($nodes.Count)/4 nodes are running" -ForegroundColor Red }
    if ($nodeConnections -lt 4) { Write-Host "  - Only $nodeConnections nodes connected to bootstrap" -ForegroundColor Red }
}

Write-Host ""
