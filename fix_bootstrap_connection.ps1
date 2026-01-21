# Start bootstrap server
Write-Host ""
Write-Host "[2/3] Starting bootstrap server..." -ForegroundColor Yellow
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$PWD'; Write-Host '=== BOOTSTRAP SERVER (QUIC) ===' -ForegroundColor Cyan; cargo run --bin server -- --listen-addr 0.0.0.0 --port 51820 --transport dual" -WindowStyle Minimized
Start-Sleep -Seconds 5
Write-Host "  [OK] Bootstrap server starting (QUIC enabled)" -ForegroundColor Green