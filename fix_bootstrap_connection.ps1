# Fix Bootstrap Connection Issue
# Restarts bootstrap server to ensure it's ready for connections

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  FIXING BOOTSTRAP CONNECTION" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# Stop existing bootstrap server
Write-Host "[1/3] Stopping existing bootstrap server..." -ForegroundColor Yellow
$bootstrap = Get-Process | Where-Object {$_.ProcessName -eq "server"} -ErrorAction SilentlyContinue
if ($bootstrap) {
    Write-Host "  Found bootstrap server (PID: $($bootstrap.Id))" -ForegroundColor Gray
    Stop-Process -Id $bootstrap.Id -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 2
    Write-Host "  [OK] Bootstrap server stopped" -ForegroundColor Green
} else {
    Write-Host "  [INFO] No bootstrap server running" -ForegroundColor Gray
}

# Wait a moment
Start-Sleep -Seconds 1

# Start bootstrap server
Write-Host ""
Write-Host "[2/3] Starting bootstrap server..." -ForegroundColor Yellow
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$PWD'; Write-Host '=== BOOTSTRAP SERVER ===' -ForegroundColor Cyan; cargo run --bin server -- --listen-addr 0.0.0.0 --port 51820" -WindowStyle Minimized
Start-Sleep -Seconds 5
Write-Host "  [OK] Bootstrap server starting" -ForegroundColor Green

# Verify it's listening
Write-Host ""
Write-Host "[3/3] Verifying bootstrap server..." -ForegroundColor Yellow
Start-Sleep -Seconds 3
$portCheck = netstat -ano | findstr ":51820" | findstr "LISTENING"
if ($portCheck) {
    Write-Host "  [OK] Bootstrap server is listening on port 51820" -ForegroundColor Green
} else {
    Write-Host "  [WARN] Port 51820 not listening yet (may still be compiling)" -ForegroundColor Yellow
    Write-Host "  Wait 10-20 seconds and check again" -ForegroundColor Gray
}

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  NEXT STEPS" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "1. Wait 10-20 seconds for bootstrap server to fully start" -ForegroundColor White
Write-Host "2. Check bootstrap server terminal for 'Bootstrap node started!' message" -ForegroundColor White
Write-Host "3. Shard nodes should now be able to connect" -ForegroundColor White
Write-Host ""
Write-Host "If shard nodes still show connection errors:" -ForegroundColor Yellow
Write-Host "  - Wait a bit longer (bootstrap may still be compiling)" -ForegroundColor Gray
Write-Host "  - Check bootstrap server terminal for errors" -ForegroundColor Gray
Write-Host "  - Restart shard nodes after bootstrap is ready" -ForegroundColor Gray
Write-Host ""
