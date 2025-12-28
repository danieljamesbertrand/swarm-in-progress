# Wait for Web Server to be Ready and Open Browser
Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  WAITING FOR WEB SERVER" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# Check if web server is already running
$webServer = Get-Process | Where-Object {$_.ProcessName -eq "web_server"} -ErrorAction SilentlyContinue
if ($webServer) {
    try {
        $response = Invoke-WebRequest -Uri "http://localhost:8080" -TimeoutSec 3 -UseBasicParsing -ErrorAction Stop
        Write-Host "[OK] Web server is already running and responding!" -ForegroundColor Green
        Write-Host "Opening browser..." -ForegroundColor Yellow
        Start-Process "http://localhost:8080"
        exit 0
    } catch {
        Write-Host "[INFO] Web server process exists but not responding yet..." -ForegroundColor Yellow
    }
}

# Check if cargo is compiling
Write-Host "[1/3] Checking compilation status..." -ForegroundColor Yellow
$cargo = Get-Process | Where-Object {$_.ProcessName -eq "cargo"} -ErrorAction SilentlyContinue
$rustc = Get-Process | Where-Object {$_.ProcessName -eq "rustc"} -ErrorAction SilentlyContinue

if ($cargo -or $rustc) {
    Write-Host "  [INFO] Cargo/Rustc is running - compilation in progress" -ForegroundColor Yellow
    Write-Host "  Waiting for compilation to complete..." -ForegroundColor Gray
    
    # Wait for compilation to finish
    $maxCompileWait = 300  # 5 minutes max
    $compileElapsed = 0
    while (($cargo -or $rustc) -and $compileElapsed -lt $maxCompileWait) {
        Start-Sleep -Seconds 5
        $compileElapsed += 5
        $cargo = Get-Process | Where-Object {$_.ProcessName -eq "cargo"} -ErrorAction SilentlyContinue
        $rustc = Get-Process | Where-Object {$_.ProcessName -eq "rustc"} -ErrorAction SilentlyContinue
        
        if ($compileElapsed % 30 -eq 0) {
            Write-Host "  Still compiling... ($compileElapsed seconds elapsed)" -ForegroundColor Gray
        }
    }
    
    if ($compileElapsed -ge $maxCompileWait) {
        Write-Host "  [WARN] Compilation taking longer than expected" -ForegroundColor Yellow
        Write-Host "  Continuing anyway..." -ForegroundColor Gray
    } else {
        Write-Host "  [OK] Compilation appears to be complete" -ForegroundColor Green
    }
} else {
    Write-Host "  [OK] No compilation in progress" -ForegroundColor Green
}

# Wait for web server to start
Write-Host ""
Write-Host "[2/3] Waiting for web server to start..." -ForegroundColor Yellow
$maxWait = 120  # 2 minutes max
$elapsed = 0
$serverReady = $false

while (-not $serverReady -and $elapsed -lt $maxWait) {
    Start-Sleep -Seconds 2
    $elapsed += 2
    
    # Check if process exists
    $webServer = Get-Process | Where-Object {$_.ProcessName -eq "web_server"} -ErrorAction SilentlyContinue
    
    # Check if port is listening
    $port8080 = netstat -ano | findstr ":8080" | findstr "LISTENING"
    
    # Test HTTP connection
    try {
        $response = Invoke-WebRequest -Uri "http://localhost:8080" -TimeoutSec 2 -UseBasicParsing -ErrorAction Stop
        $serverReady = $true
        Write-Host "  [OK] Web server is responding! (Status: $($response.StatusCode))" -ForegroundColor Green
        break
    } catch {
        # Not ready yet
        if ($elapsed % 10 -eq 0) {
            if ($webServer) {
                Write-Host "  Web server process running, waiting for HTTP response... ($elapsed seconds)" -ForegroundColor Gray
            } elseif ($port8080) {
                Write-Host "  Port 8080 is listening, testing connection... ($elapsed seconds)" -ForegroundColor Gray
            } else {
                Write-Host "  Waiting for web server to start... ($elapsed seconds)" -ForegroundColor Gray
            }
        }
    }
}

if (-not $serverReady) {
    Write-Host ""
    Write-Host "[ERROR] Web server did not become ready after $maxWait seconds" -ForegroundColor Red
    Write-Host ""
    Write-Host "Troubleshooting:" -ForegroundColor Yellow
    Write-Host "  1. Check the web server terminal for error messages" -ForegroundColor White
    Write-Host "  2. Verify port 8080 is not in use by another process" -ForegroundColor White
    Write-Host "  3. Try starting web server manually: cargo run --bin web_server" -ForegroundColor White
    exit 1
}

# Open browser
Write-Host ""
Write-Host "[3/3] Opening browser..." -ForegroundColor Yellow
Start-Sleep -Seconds 2  # Small delay to ensure server is fully ready
Start-Process "http://localhost:8080"

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  WEB SERVER IS READY!" -ForegroundColor Green
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Browser should have opened automatically" -ForegroundColor Green
Write-Host ""
Write-Host "Next steps:" -ForegroundColor Yellow
Write-Host "  1. Wait 10-15 seconds for shard node to register" -ForegroundColor White
Write-Host "  2. Type query: 'what do a cat and a snake have in common'" -ForegroundColor Cyan
Write-Host "  3. Click Send or press Enter" -ForegroundColor White
Write-Host "  4. Watch for results to appear" -ForegroundColor White
Write-Host ""
Write-Host "To use Cursor's Simple Browser instead:" -ForegroundColor Yellow
Write-Host "  1. Press Ctrl+Shift+P" -ForegroundColor White
Write-Host "  2. Type: Simple Browser: Show" -ForegroundColor White
Write-Host "  3. Enter: http://localhost:8080" -ForegroundColor Cyan
Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

