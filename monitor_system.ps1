# System Monitoring Script
# Monitors nodes, web server, and shows real-time status

Write-Host "=== PROMETHOS-AI SYSTEM MONITOR ===" -ForegroundColor Cyan
Write-Host ""

$maxChecks = 30
$checkCount = 0

while ($checkCount -lt $maxChecks) {
    Clear-Host
    Write-Host "=== PROMETHOS-AI SYSTEM STATUS ===" -ForegroundColor Cyan
    Write-Host "Check: $($checkCount + 1)/$maxChecks" -ForegroundColor Gray
    Write-Host ""
    
    # Check bootstrap server
    $bootstrap = Get-Process | Where-Object {$_.ProcessName -like "*server*" -and $_.MainWindowTitle -like "*Bootstrap*"} | Select-Object -First 1
    Write-Host "Bootstrap Server: " -NoNewline
    if ($bootstrap) {
        Write-Host "✓ Running (PID: $($bootstrap.Id))" -ForegroundColor Green
    } else {
        Write-Host "✗ Not Running" -ForegroundColor Red
    }
    
    # Check web server
    $webServer = Get-Process | Where-Object {$_.ProcessName -eq "web_server"} | Select-Object -First 1
    Write-Host "Web Server:       " -NoNewline
    if ($webServer) {
        Write-Host "✓ Running (PID: $($webServer.Id))" -ForegroundColor Green
    } else {
        Write-Host "✗ Not Running" -ForegroundColor Red
    }
    
    # Check nodes
    $nodes = Get-Process | Where-Object {$_.ProcessName -eq "shard_listener"}
    $nodeCount = ($nodes | Measure-Object).Count
    Write-Host "Shard Nodes:     " -NoNewline
    if ($nodeCount -eq 4) {
        Write-Host "✓ $nodeCount/4 Running" -ForegroundColor Green
    } elseif ($nodeCount -gt 0) {
        Write-Host "⚠ $nodeCount/4 Running" -ForegroundColor Yellow
    } else {
        Write-Host "✗ 0/4 Running" -ForegroundColor Red
    }
    
    Write-Host ""
    Write-Host "Node Details:" -ForegroundColor Cyan
    if ($nodeCount -gt 0) {
        $nodes | ForEach-Object {
            Write-Host "  • PID: $($_.Id) | Memory: $([math]::Round($_.WorkingSet64/1MB,2)) MB" -ForegroundColor Gray
        }
    } else {
        Write-Host "  No nodes running" -ForegroundColor Red
    }
    
    Write-Host ""
    Write-Host "Web Interface: http://localhost:8080" -ForegroundColor Cyan
    Write-Host "WebSocket:     ws://localhost:8081" -ForegroundColor Cyan
    Write-Host ""
    Write-Host "Press Ctrl+C to stop monitoring (servers will continue running)" -ForegroundColor Gray
    
    Start-Sleep -Seconds 3
    $checkCount++
}

Write-Host ""
Write-Host "Monitoring complete. Servers are still running." -ForegroundColor Green

