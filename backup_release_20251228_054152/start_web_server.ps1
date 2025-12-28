# Start Web Server for Inference Test
Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  STARTING WEB SERVER" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# Check if already running
$webServer = Get-Process | Where-Object {$_.ProcessName -eq "web_server"} -ErrorAction SilentlyContinue
if ($webServer) {
    Write-Host "[INFO] Web server is already running (PID: $($webServer.Id))" -ForegroundColor Yellow
    Write-Host "If you're getting connection refused, it may still be starting up." -ForegroundColor Yellow
    Write-Host "Wait 10-15 seconds and try again." -ForegroundColor Yellow
    exit 0
}

# Check if bootstrap is running
$bootstrap = Get-Process | Where-Object {$_.ProcessName -eq "server"} -ErrorAction SilentlyContinue
if (-not $bootstrap) {
    Write-Host "[WARN] Bootstrap server not running. Starting it first..." -ForegroundColor Yellow
    Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$PWD'; cargo run --bin server -- --listen-addr 0.0.0.0 --port 51820" -WindowStyle Minimized
    Start-Sleep -Seconds 5
    Write-Host "[OK] Bootstrap server started" -ForegroundColor Green
} else {
    Write-Host "[OK] Bootstrap server running (PID: $($bootstrap.Id))" -ForegroundColor Green
}

# Start web server
Write-Host ""
Write-Host "[1/2] Starting web server..." -ForegroundColor Yellow
$env:BOOTSTRAP = "/ip4/127.0.0.1/tcp/51820"
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$PWD'; `$env:BOOTSTRAP='/ip4/127.0.0.1/tcp/51820'; Write-Host 'Starting web server...'; cargo run --bin web_server" -WindowStyle Normal

Write-Host "[2/2] Waiting for web server to start (15 seconds)..." -ForegroundColor Yellow
Start-Sleep -Seconds 15

# Check if it's running
$webServer = Get-Process | Where-Object {$_.ProcessName -eq "web_server"} -ErrorAction SilentlyContinue
if ($webServer) {
    Write-Host "[OK] Web server process started (PID: $($webServer.Id))" -ForegroundColor Green
} else {
    Write-Host "[WARN] Web server process not found yet. It may still be compiling/starting." -ForegroundColor Yellow
    Write-Host "Check the web server terminal window for startup messages." -ForegroundColor Yellow
}

# Test HTTP endpoint
Write-Host ""
Write-Host "Testing HTTP endpoint..." -ForegroundColor Yellow
$maxAttempts = 5
$attempt = 0
$success = $false

while ($attempt -lt $maxAttempts -and -not $success) {
    $attempt++
    Start-Sleep -Seconds 3
    try {
        $response = Invoke-WebRequest -Uri "http://localhost:8080" -TimeoutSec 5 -UseBasicParsing -ErrorAction Stop
        Write-Host "[OK] HTTP server is responding! (Status: $($response.StatusCode))" -ForegroundColor Green
        $success = $true
    } catch {
        Write-Host "[ATTEMPT $attempt/$maxAttempts] Server not ready yet..." -ForegroundColor Gray
    }
}

if (-not $success) {
    Write-Host ""
    Write-Host "[ERROR] Web server is not responding after $($maxAttempts * 3) seconds" -ForegroundColor Red
    Write-Host ""
    Write-Host "Possible issues:" -ForegroundColor Yellow
    Write-Host "  1. Cargo is still compiling (check web server terminal)" -ForegroundColor Gray
    Write-Host "  2. Port 8080 is already in use by another process" -ForegroundColor Gray
    Write-Host "  3. Web server crashed during startup (check terminal for errors)" -ForegroundColor Gray
    Write-Host ""
    Write-Host "Check the web server terminal window for error messages." -ForegroundColor Yellow
    exit 1
}

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  WEB SERVER IS READY!" -ForegroundColor Green
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Open in browser: http://localhost:8080" -ForegroundColor Cyan
Write-Host ""
Write-Host "To use Cursor's browser:" -ForegroundColor Yellow
Write-Host "  1. Press Ctrl+Shift+P" -ForegroundColor White
Write-Host "  2. Type: Simple Browser: Show" -ForegroundColor White
Write-Host "  3. Enter: http://localhost:8080" -ForegroundColor Cyan
Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

