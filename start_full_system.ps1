# Full System Startup and Monitoring Script
# Starts bootstrap server, web server, and monitors everything

Write-Host "=== PROMETHOS-AI FULL SYSTEM STARTUP ===" -ForegroundColor Cyan
Write-Host ""

# Kill existing processes
Write-Host "[1/6] Cleaning up existing processes..." -ForegroundColor Yellow
Get-Process | Where-Object {$_.ProcessName -like "*web_server*" -or $_.ProcessName -like "*shard_listener*"} | Stop-Process -Force -ErrorAction SilentlyContinue
Start-Sleep -Seconds 1

# Start bootstrap server
Write-Host "[2/6] Starting bootstrap server on port 51820..." -ForegroundColor Yellow
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$PWD'; cargo run --bin server -- --listen-addr 0.0.0.0 --port 51820" -WindowStyle Minimized
Start-Sleep -Seconds 3

# Start web server
Write-Host "[3/6] Starting web server..." -ForegroundColor Yellow
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$PWD'; cargo run --bin web_server" -WindowStyle Normal

Write-Host ""
Write-Host "=== SYSTEM STARTED ===" -ForegroundColor Green
Write-Host "Bootstrap Server: http://127.0.0.1:51820" -ForegroundColor Cyan
Write-Host "Web Server:       http://localhost:8080" -ForegroundColor Cyan
Write-Host "WebSocket:        ws://localhost:8081" -ForegroundColor Cyan
Write-Host ""
Write-Host "Waiting 10 seconds for servers to initialize..." -ForegroundColor Yellow
Start-Sleep -Seconds 10

# Monitor processes
Write-Host ""
Write-Host "[4/6] Monitoring system status..." -ForegroundColor Yellow
Write-Host ""

$maxWait = 60
$elapsed = 0
while ($elapsed -lt $maxWait) {
    $webServer = Get-Process | Where-Object {$_.ProcessName -eq "web_server"} | Select-Object -First 1
    $nodes = Get-Process | Where-Object {$_.ProcessName -eq "shard_listener"} | Measure-Object
    
    Write-Host "[$($elapsed)s] Web Server: $(if($webServer){'Running'}else{'Not Running'}) | Nodes: $($nodes.Count)/4" -ForegroundColor $(if($nodes.Count -eq 4 -and $webServer){'Green'}else{'Yellow'})
    
    if ($nodes.Count -eq 4 -and $webServer) {
        Write-Host ""
        Write-Host "=== SYSTEM READY ===" -ForegroundColor Green
        Write-Host "✓ Bootstrap server running" -ForegroundColor Green
        Write-Host "✓ Web server running" -ForegroundColor Green
        Write-Host "✓ 4 nodes spawned and running" -ForegroundColor Green
        Write-Host ""
        Write-Host "Open http://localhost:8080 in your browser!" -ForegroundColor Cyan
        break
    }
    
    Start-Sleep -Seconds 2
    $elapsed += 2
}

if ($elapsed -ge $maxWait) {
    Write-Host ""
    Write-Host "⚠️  System may not be fully ready. Check logs manually." -ForegroundColor Yellow
}

Write-Host ""
Write-Host "Press any key to exit monitoring (servers will continue running)..." -ForegroundColor Gray
$null = $Host.UI.RawUI.ReadKey("NoEcho,IncludeKeyDown")

