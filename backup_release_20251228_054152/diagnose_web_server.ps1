# Diagnose Web Server Startup Issues
Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  WEB SERVER DIAGNOSTICS" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# Check 1: Binary exists
Write-Host "[1/6] Checking if binary exists..." -ForegroundColor Yellow
$debugBin = Test-Path "target\debug\web_server.exe"
$releaseBin = Test-Path "target\release\web_server.exe"
if ($debugBin) {
    Write-Host "  [OK] Debug binary exists: target\debug\web_server.exe" -ForegroundColor Green
} else {
    Write-Host "  [ERROR] Debug binary NOT found" -ForegroundColor Red
}
if ($releaseBin) {
    Write-Host "  [OK] Release binary exists: target\release\web_server.exe" -ForegroundColor Green
}

# Check 2: Cargo processes
Write-Host ""
Write-Host "[2/6] Checking cargo processes..." -ForegroundColor Yellow
$cargoProcs = Get-Process | Where-Object {$_.ProcessName -eq "cargo"} -ErrorAction SilentlyContinue
if ($cargoProcs) {
    Write-Host "  [INFO] Found $($cargoProcs.Count) cargo process(es):" -ForegroundColor Yellow
    $cargoProcs | ForEach-Object {
        $cpu = [math]::Round($_.CPU, 2)
        $memMB = [math]::Round($_.WorkingSet64 / 1MB, 2)
        Write-Host "    PID: $($_.Id), CPU: $cpu s, Memory: $memMB MB" -ForegroundColor Gray
    }
    Write-Host "  [INFO] These may be stuck or still compiling" -ForegroundColor Yellow
} else {
    Write-Host "  [OK] No cargo processes running" -ForegroundColor Green
}

# Check 3: Port 8080
Write-Host ""
Write-Host "[3/6] Checking port 8080..." -ForegroundColor Yellow
$port8080 = netstat -ano | findstr ":8080"
if ($port8080) {
    Write-Host "  [INFO] Port 8080 is in use:" -ForegroundColor Yellow
    $port8080 | ForEach-Object { Write-Host "    $_" -ForegroundColor Gray }
} else {
    Write-Host "  [OK] Port 8080 is available" -ForegroundColor Green
}

# Check 4: Web server process
Write-Host ""
Write-Host "[4/6] Checking web server process..." -ForegroundColor Yellow
$webServer = Get-Process | Where-Object {$_.ProcessName -eq "web_server"} -ErrorAction SilentlyContinue
if ($webServer) {
    Write-Host "  [OK] Web server process found (PID: $($webServer.Id))" -ForegroundColor Green
} else {
    Write-Host "  [ERROR] Web server process NOT running" -ForegroundColor Red
}

# Check 5: Compilation status
Write-Host ""
Write-Host "[5/6] Testing compilation..." -ForegroundColor Yellow
$compileOutput = cargo check --bin web_server 2>&1 | Select-Object -Last 5
if ($LASTEXITCODE -eq 0) {
    Write-Host "  [OK] Code compiles successfully" -ForegroundColor Green
} else {
    Write-Host "  [ERROR] Compilation failed!" -ForegroundColor Red
    $compileOutput | ForEach-Object { Write-Host "    $_" -ForegroundColor Red }
}

# Check 6: Try to run directly
Write-Host ""
Write-Host "[6/6] Attempting to start web server..." -ForegroundColor Yellow
Write-Host "  This will show any startup errors" -ForegroundColor Gray

# Check if there's a lock file
$lockFile = "Cargo.lock"
if (Test-Path $lockFile) {
    Write-Host "  [OK] Cargo.lock exists" -ForegroundColor Green
} else {
    Write-Host "  [WARN] Cargo.lock missing - may need cargo build first" -ForegroundColor Yellow
}

# Recommendations
Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  RECOMMENDATIONS" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

if ($cargoProcs) {
    Write-Host "[ACTION] Kill stuck cargo processes:" -ForegroundColor Yellow
    Write-Host "  Stop-Process -Id $($cargoProcs.Id -join ', ') -Force" -ForegroundColor White
    Write-Host ""
}

if (-not $webServer) {
    Write-Host "[ACTION] Start web server manually:" -ForegroundColor Yellow
    Write-Host "  `$env:BOOTSTRAP='/ip4/127.0.0.1/tcp/51820'" -ForegroundColor White
    Write-Host "  cargo run --bin web_server" -ForegroundColor White
    Write-Host ""
    Write-Host "  Watch the terminal for:" -ForegroundColor Gray
    Write-Host "    - Compilation messages" -ForegroundColor Gray
    Write-Host "    - 'Web Console: http://localhost:8080' message" -ForegroundColor Gray
    Write-Host "    - Any error messages" -ForegroundColor Gray
    Write-Host ""
}

if ($port8080 -and -not $webServer) {
    Write-Host "[ACTION] Port 8080 is in use by another process" -ForegroundColor Yellow
    Write-Host "  Find and kill the process using port 8080" -ForegroundColor White
    Write-Host ""
}

Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

