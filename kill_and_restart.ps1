# Kill All Processes and Restart System
# Kills all related processes, then starts everything fresh

Write-Host ""
Write-Host "========================================" -ForegroundColor Red
Write-Host "  KILLING ALL PROCESSES" -ForegroundColor Red
Write-Host "========================================" -ForegroundColor Red
Write-Host ""

# Kill all processes
Write-Host "[1/2] Killing all existing processes..." -ForegroundColor Yellow
$processes = Get-Process | Where-Object { 
    $_.ProcessName -match "bootstrap|web_server|shard_listener|server|node" -or
    $_.ProcessName -eq "cargo" -or
    $_.ProcessName -eq "rustc"
} -ErrorAction SilentlyContinue

if ($processes) {
    Write-Host "  Found $($processes.Count) process(es) to kill" -ForegroundColor Gray
    $processes | ForEach-Object {
        try {
            Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue
            Write-Host "    Killed: $($_.ProcessName) (PID: $($_.Id))" -ForegroundColor Gray
        } catch {
            Write-Host "    Failed to kill: $($_.ProcessName) (PID: $($_.Id))" -ForegroundColor Yellow
        }
    }
    Start-Sleep -Seconds 3
    Write-Host "  [OK] All processes killed" -ForegroundColor Green
} else {
    Write-Host "  [OK] No processes to kill" -ForegroundColor Green
}

# Wait a moment for cleanup
Start-Sleep -Seconds 2

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  RESTARTING SYSTEM" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# Restart everything
Write-Host "[2/2] Starting all services..." -ForegroundColor Yellow
& "$PSScriptRoot\start_and_monitor_shards.ps1"