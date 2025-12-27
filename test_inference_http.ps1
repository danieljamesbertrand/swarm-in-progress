# Test inference via HTTP/WebSocket using curl-like approach
$ErrorActionPreference = "Stop"

Write-Host "`n========================================" -ForegroundColor Cyan
Write-Host "  TESTING INFERENCE REQUEST" -ForegroundColor Cyan
Write-Host "========================================`n" -ForegroundColor Cyan

Write-Host "Question: How are a cat and a snake related?" -ForegroundColor Yellow
Write-Host ""

# Check if web server is running
$webServer = Get-Process | Where-Object {$_.ProcessName -eq "web_server"}
if (-not $webServer) {
    Write-Host "ERROR: Web server is not running!" -ForegroundColor Red
    exit 1
}

Write-Host "[1/3] Web server is running (PID: $($webServer.Id))" -ForegroundColor Green

# Check if nodes are running
$nodes = Get-Process | Where-Object {$_.ProcessName -eq "shard_listener"}
Write-Host "[2/3] Shard nodes running: $($nodes.Count)/4" -ForegroundColor $(if ($nodes.Count -ge 4) { 'Green' } else { 'Yellow' })

Write-Host "`n[3/3] Testing HTTP endpoint..." -ForegroundColor Yellow
try {
    $response = Invoke-WebRequest -Uri "http://localhost:8080" -TimeoutSec 5 -UseBasicParsing
    Write-Host "  ✓ HTTP server responding (Status: $($response.StatusCode))" -ForegroundColor Green
    Write-Host "  Content length: $($response.Content.Length) bytes" -ForegroundColor Gray
} catch {
    Write-Host "  ✗ HTTP server error: $_" -ForegroundColor Red
    exit 1
}

Write-Host "`n========================================" -ForegroundColor Cyan
Write-Host "  TEST INSTRUCTIONS" -ForegroundColor Cyan
Write-Host "========================================`n" -ForegroundColor Cyan

Write-Host "To test inference:" -ForegroundColor Yellow
Write-Host "  1. Open browser: http://localhost:8080" -ForegroundColor White
Write-Host "  2. Enter question: How are a cat and a snake related?" -ForegroundColor White
Write-Host "  3. Click Send or press Enter" -ForegroundColor White
Write-Host "  4. Check the response in the web console" -ForegroundColor White
Write-Host "`nThe web console uses WebSocket for real-time inference." -ForegroundColor Gray
Write-Host "Check the web server console window for detailed logs." -ForegroundColor Gray
Write-Host ""

# Check system status
Write-Host "`n========================================" -ForegroundColor Cyan
Write-Host "  SYSTEM STATUS" -ForegroundColor Cyan
Write-Host "========================================`n" -ForegroundColor Cyan

$server = Get-Process | Where-Object {$_.ProcessName -eq "server"} | Select-Object -First 1
Write-Host "Bootstrap Server: $(if ($server) { "Running (PID: $($server.Id))" } else { "Not Running" })" -ForegroundColor $(if ($server) { 'Green' } else { 'Red' })
Write-Host "Web Server:       Running (PID: $($webServer.Id))" -ForegroundColor Green
Write-Host "Shard Nodes:      $($nodes.Count)/4" -ForegroundColor $(if ($nodes.Count -ge 4) { 'Green' } else { 'Yellow' })

# Check ports
Write-Host "`nPort Status:" -ForegroundColor Yellow
$ports = netstat -ano | findstr "LISTENING" | findstr ":8080 :8081 :51820"
if ($ports) {
    $ports | ForEach-Object {
        if ($_ -match ":(\d+)") {
            Write-Host "  Port $($matches[1]): LISTENING" -ForegroundColor Green
        }
    }
}

Write-Host "`n========================================" -ForegroundColor Cyan
Write-Host "  TEST READY" -ForegroundColor Cyan
Write-Host "========================================`n" -ForegroundColor Cyan

Write-Host "System is ready for testing!" -ForegroundColor Green
Write-Host "Open http://localhost:8080 in your browser to test inference." -ForegroundColor Yellow
Write-Host ""

