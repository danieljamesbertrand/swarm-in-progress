# Start Web Server Fresh
Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  STARTING WEB SERVER (FRESH START)" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# Kill any stuck processes
Write-Host "[1/4] Cleaning up stuck processes..." -ForegroundColor Yellow
$cargoProcs = Get-Process | Where-Object {$_.ProcessName -eq "cargo"} -ErrorAction SilentlyContinue
if ($cargoProcs) {
    Write-Host "  Killing $($cargoProcs.Count) stuck cargo process(es)..." -ForegroundColor Gray
    $cargoProcs | Stop-Process -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 2
}
$webServer = Get-Process | Where-Object {$_.ProcessName -eq "web_server"} -ErrorAction SilentlyContinue
if ($webServer) {
    Write-Host "  Killing existing web server process..." -ForegroundColor Gray
    $webServer | Stop-Process -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 2
}
Write-Host "  [OK] Cleanup complete" -ForegroundColor Green

# Check bootstrap
Write-Host ""
Write-Host "[2/4] Checking bootstrap server..." -ForegroundColor Yellow
$bootstrap = Get-Process | Where-Object {$_.ProcessName -eq "server"} -ErrorAction SilentlyContinue
if (-not $bootstrap) {
    Write-Host "  [WARN] Bootstrap server not running" -ForegroundColor Yellow
    Write-Host "  Starting bootstrap server..." -ForegroundColor Gray
    Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$PWD'; cargo run --bin server -- --listen-addr 0.0.0.0 --port 51820" -WindowStyle Minimized
    Start-Sleep -Seconds 5
    Write-Host "  [OK] Bootstrap server started" -ForegroundColor Green
} else {
    Write-Host "  [OK] Bootstrap server running (PID: $($bootstrap.Id))" -ForegroundColor Green
}

# Start web server
Write-Host ""
Write-Host "[3/4] Starting web server..." -ForegroundColor Yellow
$env:BOOTSTRAP = "/ip4/127.0.0.1/tcp/51820"
Write-Host "  Command: cargo run --bin web_server" -ForegroundColor Gray
Write-Host "  Bootstrap: $env:BOOTSTRAP" -ForegroundColor Gray
Write-Host ""
Write-Host "  Opening new terminal window..." -ForegroundColor Gray
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$PWD'; `$env:BOOTSTRAP='/ip4/127.0.0.1/tcp/51820'; Write-Host 'Starting web server...'; Write-Host 'Bootstrap: ' `$env:BOOTSTRAP; Write-Host ''; cargo run --bin web_server" -WindowStyle Normal

Write-Host ""
Write-Host "[4/4] Waiting for web server to start..." -ForegroundColor Yellow
Write-Host "  Watch the web server terminal window for:" -ForegroundColor Gray
Write-Host "    - Compilation progress" -ForegroundColor Gray
Write-Host "    - 'Web Console: http://localhost:8080' message" -ForegroundColor Green
Write-Host "    - Any error messages" -ForegroundColor Gray
Write-Host ""

# Wait and test
$maxWait = 180  # 3 minutes
$elapsed = 0
$serverReady = $false

while (-not $serverReady -and $elapsed -lt $maxWait) {
    Start-Sleep -Seconds 3
    $elapsed += 3
    
    try {
        $response = Invoke-WebRequest -Uri "http://localhost:8080" -TimeoutSec 2 -UseBasicParsing -ErrorAction Stop
        $serverReady = $true
        Write-Host "  [OK] Web server is responding! (Status: $($response.StatusCode))" -ForegroundColor Green
        break
    } catch {
        if ($elapsed % 15 -eq 0) {
            $webServer = Get-Process | Where-Object {$_.ProcessName -eq "web_server"} -ErrorAction SilentlyContinue
            if ($webServer) {
                Write-Host "  Web server process running, waiting for HTTP... ($elapsed seconds)" -ForegroundColor Gray
            } else {
                Write-Host "  Still compiling/starting... ($elapsed seconds)" -ForegroundColor Gray
            }
        }
    }
}

if ($serverReady) {
    Write-Host ""
    Write-Host "========================================" -ForegroundColor Cyan
    Write-Host "  WEB SERVER IS READY!" -ForegroundColor Green
    Write-Host "========================================" -ForegroundColor Cyan
    Write-Host ""
    Write-Host "Opening browser..." -ForegroundColor Yellow
    Start-Sleep -Seconds 2
    Start-Process "http://localhost:8080"
    Write-Host ""
    Write-Host "Browser should open automatically" -ForegroundColor Green
    Write-Host "If not, open: http://localhost:8080" -ForegroundColor Cyan
} else {
    Write-Host ""
    Write-Host "  [WARN] Web server did not become ready after $maxWait seconds" -ForegroundColor Yellow
    Write-Host "  Check the web server terminal window for:" -ForegroundColor Yellow
    Write-Host "    - Compilation errors" -ForegroundColor Gray
    Write-Host "    - Runtime errors" -ForegroundColor Gray
    Write-Host "    - Port binding errors" -ForegroundColor Gray
}

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

