# Simple test runner
Write-Host ""
Write-Host "=== SYSTEM TEST ===" -ForegroundColor Cyan
Write-Host ""

# Check processes
$server = Get-Process | Where-Object {$_.ProcessName -eq "server"} | Select-Object -First 1
$webServer = Get-Process | Where-Object {$_.ProcessName -eq "web_server"} | Select-Object -First 1
$nodes = Get-Process | Where-Object {$_.ProcessName -eq "shard_listener"}

Write-Host "Bootstrap Server: " -NoNewline
if ($server) { Write-Host "Running (PID: $($server.Id))" -ForegroundColor Green } else { Write-Host "Not Running" -ForegroundColor Red }

Write-Host "Web Server:       " -NoNewline
if ($webServer) { Write-Host "Running (PID: $($webServer.Id))" -ForegroundColor Green } else { Write-Host "Not Running" -ForegroundColor Red }

Write-Host "Shard Nodes:      " -NoNewline
Write-Host "$($nodes.Count)/4" -ForegroundColor $(if ($nodes.Count -ge 4) { 'Green' } else { 'Yellow' })

Write-Host ""
Write-Host "=== TEST INSTRUCTIONS ===" -ForegroundColor Cyan
Write-Host ""
Write-Host "1. Open browser: http://localhost:8080" -ForegroundColor White
Write-Host "2. Enter question: How are a cat and a snake related?" -ForegroundColor White
Write-Host "3. Click Send" -ForegroundColor White
Write-Host "4. Check response in web console" -ForegroundColor White
Write-Host ""
Write-Host "System is ready for testing!" -ForegroundColor Green
Write-Host ""

