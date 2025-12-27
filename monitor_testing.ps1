# Real-time monitoring during testing
# Monitors system health while you test

Write-Host "=== REAL-TIME SYSTEM MONITOR ===" -ForegroundColor Cyan
Write-Host "Press Ctrl+C to stop monitoring" -ForegroundColor Gray
Write-Host ""

$iteration = 0
while ($true) {
    Clear-Host
    Write-Host "=== PROMETHOS-AI SYSTEM MONITOR ===" -ForegroundColor Cyan
    Write-Host "Iteration: $iteration | Time: $(Get-Date -Format 'HH:mm:ss')" -ForegroundColor Gray
    Write-Host ""
    
    # Process status
    $webServer = Get-Process | Where-Object {$_.ProcessName -eq "web_server"} | Select-Object -First 1
    $bootstrap = Get-Process | Where-Object {$_.ProcessName -like "*server*" -and $_.Id -ne $webServer.Id} | Select-Object -First 1
    $nodes = Get-Process | Where-Object {$_.ProcessName -eq "shard_listener"}
    $nodeCount = ($nodes | Measure-Object).Count
    $cargo = Get-Process | Where-Object {$_.ProcessName -eq "cargo"}
    $cargoCount = ($cargo | Measure-Object).Count
    
    Write-Host "Process Status:" -ForegroundColor Yellow
    Write-Host "  Bootstrap: $(if($bootstrap){'✓ (PID: ' + $bootstrap.Id + ')'}else{'✗'})" -ForegroundColor $(if($bootstrap){'Green'}else{'Red'})
    Write-Host "  Web Server: $(if($webServer){'✓ (PID: ' + $webServer.Id + ')'}else{'✗'})" -ForegroundColor $(if($webServer){'Green'}else{'Red'})
    Write-Host "  Nodes:      $nodeCount/4" -ForegroundColor $(if($nodeCount -eq 4){'Green'}elseif($nodeCount -gt 0){'Yellow'}else{'Red'})
    Write-Host "  Compiling:  $cargoCount cargo processes" -ForegroundColor $(if($cargoCount -gt 0){'Yellow'}else{'Gray'})
    Write-Host ""
    
    # Node details
    if ($nodeCount -gt 0) {
        Write-Host "Node Details:" -ForegroundColor Yellow
        $nodes | ForEach-Object {
            $memMB = [math]::Round($_.WorkingSet64/1MB, 2)
            Write-Host "  PID $($_.Id): $memMB MB" -ForegroundColor Gray
        }
        Write-Host ""
    }
    
    # Resource usage
    Write-Host "Resource Usage:" -ForegroundColor Yellow
    if ($webServer) {
        $wsCPU = [math]::Round($webServer.CPU, 2)
        $wsMem = [math]::Round($webServer.WorkingSet64/1MB, 2)
        Write-Host "  Web Server: CPU: $wsCPU% | Memory: $wsMem MB" -ForegroundColor Gray
    }
    if ($nodeCount -gt 0) {
        $totalNodeMem = ($nodes | Measure-Object -Property WorkingSet64 -Sum).Sum / 1MB
        Write-Host "  Nodes Total: $([math]::Round($totalNodeMem, 2)) MB" -ForegroundColor Gray
    }
    Write-Host ""
    
    # Testing checklist
    Write-Host "Testing Checklist:" -ForegroundColor Yellow
    Write-Host "  [$(if($webServer){'✓'}else{' '})] Web server running" -ForegroundColor $(if($webServer){'Green'}else{'Red'})
    Write-Host "  [$(if($nodeCount -eq 4){'✓'}else{' '})] All 4 nodes running" -ForegroundColor $(if($nodeCount -eq 4){'Green'}else{'Yellow'})
    Write-Host "  [ ] Web interface accessible" -ForegroundColor Gray
    Write-Host "  [ ] WebSocket connected" -ForegroundColor Gray
    Write-Host "  [ ] Inference query tested" -ForegroundColor Gray
    Write-Host "  [ ] Real-time updates working" -ForegroundColor Gray
    Write-Host ""
    
    Write-Host "Web Interface: http://localhost:8080" -ForegroundColor Cyan
    Write-Host ""
    
    $iteration++
    Start-Sleep -Seconds 5
}


