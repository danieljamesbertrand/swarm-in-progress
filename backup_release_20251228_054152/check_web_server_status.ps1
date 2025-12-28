# Check Web Server Status
Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  WEB SERVER STATUS CHECK" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# Check processes
Write-Host "[1/4] Checking processes..." -ForegroundColor Yellow
$webServer = Get-Process | Where-Object {$_.ProcessName -eq "web_server"} -ErrorAction SilentlyContinue
$cargo = Get-Process | Where-Object {$_.ProcessName -eq "cargo"} -ErrorAction SilentlyContinue
$rustc = Get-Process | Where-Object {$_.ProcessName -eq "rustc"} -ErrorAction SilentlyContinue

if ($webServer) {
    Write-Host "  [OK] Web server process found (PID: $($webServer.Id))" -ForegroundColor Green
} else {
    Write-Host "  [ERROR] Web server process NOT running" -ForegroundColor Red
}

if ($cargo) {
    Write-Host "  [INFO] Cargo process running (PID: $($cargo.Id)) - may be compiling" -ForegroundColor Yellow
}

if ($rustc) {
    Write-Host "  [INFO] Rust compiler running - still compiling" -ForegroundColor Yellow
}

# Check port
Write-Host ""
Write-Host "[2/4] Checking port 8080..." -ForegroundColor Yellow
$port8080 = netstat -ano | findstr ":8080"
if ($port8080) {
    Write-Host "  [INFO] Port 8080 is in use:" -ForegroundColor Yellow
    $port8080 | ForEach-Object { Write-Host "    $_" -ForegroundColor Gray }
} else {
    Write-Host "  [ERROR] Port 8080 is NOT in use - server not listening" -ForegroundColor Red
}

# Test HTTP
Write-Host ""
Write-Host "[3/4] Testing HTTP connection..." -ForegroundColor Yellow
try {
    $response = Invoke-WebRequest -Uri "http://localhost:8080" -TimeoutSec 3 -UseBasicParsing -ErrorAction Stop
    Write-Host "  [OK] HTTP server responding (Status: $($response.StatusCode))" -ForegroundColor Green
} catch {
    Write-Host "  [ERROR] Connection refused: $_" -ForegroundColor Red
}

# Recommendations
Write-Host ""
Write-Host "[4/4] Recommendations:" -ForegroundColor Yellow
if (-not $webServer -and -not $cargo) {
    Write-Host "  - Web server is not running and not compiling" -ForegroundColor Red
    Write-Host "  - Start it manually: cargo run --bin web_server" -ForegroundColor White
} elseif ($cargo -or $rustc) {
    Write-Host "  - Cargo/Rustc is still running - compilation in progress" -ForegroundColor Yellow
    Write-Host "  - Wait for compilation to complete (may take 1-2 minutes)" -ForegroundColor White
    Write-Host "  - Check the terminal window where you started web_server" -ForegroundColor White
} elseif ($webServer -and -not $port8080) {
    Write-Host "  - Web server process exists but not listening on port 8080" -ForegroundColor Red
    Write-Host "  - Check web server terminal for startup errors" -ForegroundColor White
} elseif ($port8080 -and -not $webServer) {
    Write-Host "  - Port 8080 is in use by another process" -ForegroundColor Red
    Write-Host "  - Kill the process using port 8080 or use a different port" -ForegroundColor White
} else {
    Write-Host "  - Everything looks good! Try refreshing the browser" -ForegroundColor Green
}

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

